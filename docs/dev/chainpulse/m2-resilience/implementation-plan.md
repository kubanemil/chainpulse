---
date: 2026-03-26
author: claude-opus-4-6
status: draft
branch: main
related_research: docs/dev/chainpulse/m2-resilience/research.md
---

# Plan: M2 Resilience (Reconnection, Catch-Up, Dedup)

## Overview

Transform ChainPulse from a fragile linear flow (connect once, die on disconnect) into a self-healing indexer that automatically reconnects with exponential backoff, catches up on missed blocks via `eth_getLogs`, and tracks indexing progress continuously. The DB already handles dedup via `ON CONFLICT DO NOTHING` -- no application-level dedup work is needed.

The implementation restructures `main.rs` from a single-pass flow into a reconnection loop, adds a catch-up module for historical log fetching, and wires up the existing but unused `get_last_indexed_block`/`upsert_last_indexed_block` DB functions.

## Architecture Decisions

- **Manual backoff over crate dependency**: Use `tokio::time::sleep` with manual doubling (1s initial, 2x multiplier, 60s cap). The `backoff` crate adds unnecessary complexity for hardcoded parameters.
- **Outer reconnection loop in main.rs**: Wrap the entire WS connect + subscribe + process cycle in a `loop {}`. Alloy v1 WS providers are one-shot -- the whole provider + subscription must be recreated on reconnect.
- **Sequential catch-up then live**: Run catch-up to completion before starting the live subscription. No interleaving, no race conditions, simpler reasoning.
- **Chunked eth_getLogs with 2,000-block default**: Safe across all major RPC providers (Alchemy free tier, Infura, Ankr, public endpoints).
- **Per-chunk block tracking during catch-up**: Update `last_indexed_block` after each 2,000-block chunk. On crash, at most one chunk is re-processed -- safe due to `ON CONFLICT DO NOTHING`.
- **Per-block tracking during live indexing**: Update `last_indexed_block` after processing each log event's block. This adds one DB write per event but ensures minimal re-processing on restart.
- **Live subscription from last_indexed_block + 1**: After catch-up, subscribe from the last tracked block rather than `Latest` to avoid a gap between catch-up end and subscription start. Dedup handles overlap.
- **Continue exiting on DB errors**: M2 focuses on WS resilience. DB errors remain fatal (exit). External process supervision handles DB failures. Changing this adds complexity without matching the M2 scope.
- **No new crates**: Everything needed is available in existing dependencies (alloy, tokio, tracing).
- **Extract indexer module**: Move the reconnection loop and catch-up logic into a `chainpulse/src/indexer.rs` module to keep `main.rs` clean and testable.

## Prerequisites

- [ ] M1 complete: single-chain indexer working with WS subscription and DB writes
- [ ] PostgreSQL running with migrations applied (`indexed_blocks` and `events` tables exist)
- [ ] WS RPC endpoint accessible (e.g., Alchemy, Infura)

---

## Phase 1: Block Tracking in Live Event Loop

### Goal

Wire up the existing `upsert_last_indexed_block` DB function so that `last_indexed_block` is updated as events are processed during normal live indexing. This is the foundation for gap detection on reconnect.

### Files to Modify

- `chainpulse/src/main.rs` -- Add `upsert_last_indexed_block` call after each successful `insert_event`

### Module Design

No new modules. Changes are inline in the existing event processing loop.

Key changes:
- After `insert_event` succeeds (line 93-99 in current main.rs), call `chainpulse_db::upsert_last_indexed_block(&pool, 1, event.block_number)` where `1` is the Ethereum chain_id.
- Log at `DEBUG` level when block tracking is updated.
- DB errors from `upsert_last_indexed_block` are fatal (same as `insert_event` errors).

### Integration

- Uses the existing `upsert_last_indexed_block` function from `chainpulse-db`
- The `chain_id` is hardcoded to `1` (Ethereum mainnet), matching `decode_log`'s behavior
- No changes to DB schema or types crate

### Success Criteria

**Automated Verification:**
- [x] `cargo build` succeeds
- [x] `cargo test --workspace` passes (existing tests unaffected)

**Manual Verification:**
- [ ] Start the indexer, let it process a few events, then check `SELECT * FROM indexed_blocks WHERE chain_id = 1` -- should have a row with a recent block number
- [ ] The `last_block` value increases as new events arrive
- [ ] Stopping and restarting the indexer does not cause errors

---

## Phase 2: Reconnection Loop with Exponential Backoff

### Goal

Wrap the WS connection, subscription, and event loop in an outer reconnection loop so that when the WS stream ends, the indexer automatically reconnects with exponential backoff instead of exiting.

### Files to Modify

- `chainpulse/src/main.rs` -- Restructure into reconnection loop
- `chainpulse/src/indexer.rs` -- **New file**: Extract indexer logic (connect, subscribe, process) into a module

### Module Design

**`indexer`** -- Encapsulates the WS connection lifecycle and event processing loop.

Key functions:

- `run_indexer(pool: &PgPool, config: &Config) -> !` -- The outer reconnection loop. Never returns (runs forever). Contains the backoff state and calls `run_session` in a loop.
- `run_session(pool: &PgPool, ws_url: &str, contracts: Vec<Address>, chain_id: i32) -> Result<(), IndexerError>` -- Single connection session: connect WS, subscribe, process events until stream ends. Returns `Ok(())` when stream ends (triggering reconnect) or `Err` on fatal errors.

**`IndexerError`** -- enum distinguishing recoverable (WS disconnect, connection failure) from fatal (DB error) failures.

```
IndexerError {
    Recoverable(String)  // WS disconnect, connection refused, subscription failure
    Fatal(String)        // DB errors, migration failures
}
```

### Data Structures

```
Backoff state (local variables in run_indexer):
  current: Duration     // starts at 1s
  max: Duration         // 60s
  multiplier: u32       // 2
```

### Integration

- `main.rs` becomes: load config, init tracing, create DB pool, run migrations, call `indexer::run_indexer`
- The event processing code from current main.rs moves into `run_session`
- `run_indexer` resets backoff to 1s on successful connection (i.e., when `run_session` is entered)
- On `Recoverable` error: log WARN with retry count and backoff duration, sleep, double backoff
- On `Fatal` error: log ERROR, exit(1)
- When stream ends normally (WS disconnect): treat as recoverable, apply backoff

### Success Criteria

**Automated Verification:**
- `cargo build` succeeds
- `cargo test --workspace` passes

**Manual Verification:**
- [ ] Start indexer, kill the WS endpoint (or disconnect network briefly) -- indexer logs WARN and retries
- [ ] Backoff increases: first retry after ~1s, second after ~2s, third after ~4s
- [ ] After reconnection succeeds, backoff resets to 1s
- [ ] After reconnection succeeds, live event indexing resumes
- [ ] DB errors still cause process exit

---

## Phase 3: Catch-Up Indexing via eth_getLogs

### Goal

On each reconnection, detect the gap between `last_indexed_block` and the current chain head, then fetch and process all missed events via chunked `eth_getLogs` calls before resuming the live subscription.

### Files to Modify

- `chainpulse/src/indexer.rs` -- Add catch-up logic within `run_session`, before subscribing to live logs

### Module Design

Key functions added to `indexer`:

- `catch_up(provider: &impl Provider, pool: &PgPool, contracts: &[Address], chain_id: i32) -> Result<(), IndexerError>` -- Reads `last_indexed_block` from DB, gets chain head from provider, fetches missed logs in chunks, processes each through `decode_log` + `insert_event`, updates `last_indexed_block` per chunk.
- `fetch_logs_chunk(provider: &impl Provider, pool: &PgPool, contracts: &[Address], chain_id: i32, from: u64, to: u64) -> Result<(), IndexerError>` -- Fetches one chunk of logs via `provider.get_logs(&filter)`, decodes and inserts each, updates `last_indexed_block` to `to`.

Constants:
- `CATCH_UP_CHUNK_SIZE: u64 = 2_000`

### Data Structures

No new structs. Uses existing `Filter`, `NormalizedEvent`, DB functions.

### Integration

- `catch_up` is called at the start of `run_session`, after WS provider is created but before subscribing to live logs
- If `get_last_indexed_block` returns `None` (fresh DB): skip catch-up, log INFO "No previous indexed block, starting from latest"
- If `last_indexed_block >= head`: skip catch-up, log INFO "Already up to date"
- If gap exists: log INFO with block range (`last_indexed_block + 1` to `head`), process chunks
- After catch-up completes, the live subscription `from_block` is set to `last_indexed_block + 1` instead of `Latest`
- Each chunk: build `Filter` with `.from_block(from).to_block(to).address(contracts)`, call `provider.get_logs()`, iterate results through `decode_log` + `insert_event`, then `upsert_last_indexed_block(pool, chain_id, to as i64)`
- If `get_logs` fails for a chunk, return `Recoverable` error (triggers reconnect with backoff; progress is saved from prior chunks)

### Success Criteria

**Automated Verification:**
- `cargo build` succeeds
- `cargo test --workspace` passes

**Manual Verification:**
- [ ] Start indexer, let it index a few blocks, stop it, wait 5+ minutes, restart -- catch-up log messages appear showing the block range being fetched
- [ ] After catch-up, `SELECT count(*) FROM events WHERE block_number BETWEEN [gap_start] AND [gap_end]` returns events
- [ ] `last_indexed_block` is updated to the chain head after catch-up
- [ ] Live indexing resumes after catch-up completes
- [ ] No duplicate rows in events table (verify with `SELECT chain_id, block_number, log_index, count(*) FROM events GROUP BY 1,2,3 HAVING count(*) > 1`)

---

## Phase 4: Structured Logging for Resilience Events

### Goal

Add clear, structured log entries for all reconnection and catch-up events so operators have visibility into the indexer's resilience behavior.

### Files to Modify

- `chainpulse/src/indexer.rs` -- Add/refine logging at key points

### Log Events

All logs use `tracing` macros with structured fields:

| Event | Level | Fields | Message |
|-------|-------|--------|---------|
| WS connected | INFO | ws_url | "WebSocket connected" |
| WS disconnected | WARN | retry_count | "WebSocket disconnected, will reconnect" |
| Reconnect attempt | WARN | retry_count, backoff_secs | "Reconnecting..." |
| Reconnect failed | WARN | retry_count, backoff_secs, error | "Reconnection failed" |
| Reconnect succeeded | INFO | retry_count | "Reconnected successfully" |
| Catch-up started | INFO | from_block, to_block, gap_blocks | "Starting catch-up indexing" |
| Catch-up chunk | INFO | chunk_from, chunk_to, events_count | "Catch-up chunk processed" |
| Catch-up complete | INFO | total_blocks, total_events | "Catch-up complete" |
| Catch-up skipped (fresh) | INFO | - | "No previous block, starting from latest" |
| Catch-up skipped (current) | INFO | last_block | "Already up to date" |
| Fatal error | ERROR | error | "Fatal error, exiting" |

### Integration

- These log entries are emitted naturally within the functions from Phases 2 and 3
- Phase 4 is a refinement pass to ensure all log entries are consistent, structured, and operator-friendly
- Existing event-indexed logs remain unchanged

### Success Criteria

**Automated Verification:**
- `cargo build` succeeds

**Manual Verification:**
- [ ] Trigger a WS disconnect -- see WARN log with retry_count and backoff_secs
- [ ] See INFO log on successful reconnect with retry_count
- [ ] See INFO logs for catch-up start, each chunk, and completion
- [ ] All log entries are valid JSON (structured logging via tracing-subscriber json format)

---

## Phase 5: Final Verification

### Goal

Ensure all components work together end-to-end and satisfy the user stories from the press release.

### Success Criteria

**Automated Verification:**

- `cargo build` succeeds with no warnings
- `cargo test --workspace` -- all tests pass
- `cargo clippy --workspace` -- no warnings

**E2E Test Scenarios (from Press Release user stories):**

- [ ] **Automatic recovery**: Start indexer, disconnect WS (e.g., kill proxy or network), verify it reconnects and resumes within 60 seconds
- [ ] **Zero data gaps**: After reconnect + catch-up, verify no block gaps exist: query `SELECT block_number FROM events WHERE chain_id = 1 ORDER BY block_number` and check for continuity in the range
- [ ] **No duplicates**: After catch-up with overlapping ranges, verify: `SELECT chain_id, block_number, log_index, count(*) FROM events GROUP BY 1,2,3 HAVING count(*) > 1` returns empty
- [ ] **Crash recovery**: Stop indexer mid-catch-up (SIGKILL), restart, verify it resumes from last completed chunk (not from beginning)
- [ ] **Fresh DB start**: Start with empty DB, verify indexer skips catch-up and subscribes from latest
- [ ] **Repeated disconnects**: Trigger 3+ disconnects in sequence, verify indexer recovers each time without memory leaks or panics
- [ ] **Logging visibility**: Verify all resilience events appear in logs with correct levels and structured fields

**Manual Verification:**

- [ ] Run indexer for 30+ minutes with at least one forced disconnect -- no panics, no data gaps
- [ ] Verify `indexed_blocks` table is continuously updated during live indexing

---

## Rollback Plan

All changes are in `chainpulse/src/main.rs` and the new `chainpulse/src/indexer.rs`. No DB schema changes, no new dependencies, no config changes.

To rollback: revert `main.rs` to its M1 state and delete `indexer.rs`. The DB schema is fully backward compatible.

## Open Questions

- **Large gap threshold**: Should the indexer warn when the catch-up gap exceeds a threshold (e.g., 100,000 blocks)? Deferred -- the chunked approach handles any gap size, just takes longer.
- **Configurable backoff/chunk size**: Deferred to a future milestone per press release scope. The hardcoded defaults (1s/60s backoff, 2,000-block chunks) work for all major providers.

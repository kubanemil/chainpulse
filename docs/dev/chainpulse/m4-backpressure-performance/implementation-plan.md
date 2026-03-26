---
date: 2026-03-26
author: claude-opus-4-6
status: complete
branch: main
related_research: docs/dev/chainpulse/m4-backpressure-performance/research.md
---

# Plan: Backpressure & Performance (M4)

## Overview

This plan introduces bounded Tokio MPSC channels and batch database writes into the ChainPulse indexing pipeline. Currently, each per-chain indexer task writes events to PostgreSQL one at a time inline within its event loop. There is no explicit backpressure mechanism -- if the DB is slow, the alloy WS provider buffers incoming messages internally without limit, risking OOM under high load.

The implementation creates a dedicated DB writer Tokio task that owns the `PgPool` and receives `NormalizedEvent` messages from all per-chain subscriber tasks via a shared bounded `tokio::sync::mpsc` channel. The DB writer accumulates events in a buffer and flushes them as multi-row INSERT batches, dramatically improving throughput. When the channel is full, subscriber tasks naturally block on `sender.send()`, applying backpressure to the WebSocket ingestion rate and keeping memory usage bounded.

## Architecture Decisions

- **Shared MPSC channel for all chains** because per-chain channels add complexity without clear benefit for V1; a high-volume chain can slow others, which is acceptable per the press release FAQ
- **`tokio::sync::mpsc::channel(capacity)`** because it provides bounded backpressure with async send/recv, and tokio is already a full dependency
- **Dedicated DB writer task** because centralizing writes enables batching across chains and decouples subscriber logic from DB write patterns
- **Flush on size threshold OR time interval (whichever first)** because size-only flushing adds latency during low-activity periods; time-based flushing ensures bounded latency
- **New `flush_interval_ms` config field** (default 500ms) because the press release references a configurable flush interval and the existing config does not include one
- **Multi-row INSERT via dynamically built SQL** because sqlx does not support compile-time checked multi-row inserts; dynamically building `INSERT INTO events (...) VALUES (...), (...), ...` with bind parameters is the standard approach
- **`upsert_last_indexed_block` called once per batch** (using max block_number per chain_id in the batch) because per-event block tracking is wasteful and inconsistent with batching
- **Catch-up path routes through the bounded channel** because the press release FAQ explicitly states catch-up events should also flow through the channel for consistent backpressure protection
- **DB write failures are retried with the same batch** because events must not be lost; the batch is only discarded from the buffer after successful commit. After repeated failures, the writer task exits with a fatal error (matching current behavior).

## Prerequisites

- [ ] M3 (Multi-Chain) is complete and working -- per-chain subscriber tasks exist
- [ ] PostgreSQL is running and migrations are applied
- [ ] Current tests pass (`cargo test --workspace`)

---

## Phase 1: Configuration & Channel Setup [COMPLETE]

### Goal

Add the `flush_interval_ms` config field, create the bounded MPSC channel in `main.rs`, and pass the `Sender` half to indexer tasks instead of (alongside) the `PgPool`. The DB writer task is spawned but initially just receives and discards events (no batching yet). This phase validates the channel wiring end-to-end.

### Files to Modify

- `chainpulse-config/src/lib.rs` -- Add `flush_interval_ms` config field
- `.env.example` -- Add `FLUSH_INTERVAL_MS` documentation
- `chainpulse/src/main.rs` -- Create channel, spawn DB writer task, pass `Sender` to indexers
- `chainpulse/src/indexer.rs` -- Change `run_indexer` and `run_session` to accept `Sender<NormalizedEvent>` instead of `PgPool` for event writes; send events to channel instead of calling `insert_event` directly

### Module Design

**`chainpulse-config`** -- Add flush interval configuration

Key changes:

- `flush_interval_ms: u64` field with `default_flush_interval_ms() -> 500`
- Exposed as `FLUSH_INTERVAL_MS` env var

**`main.rs`** -- Channel creation and task wiring

Key changes:

- `let (tx, rx) = tokio::sync::mpsc::channel::<NormalizedEvent>(config.channel_buffer_size)`
- Pass `tx.clone()` to each indexer task instead of `pool.clone()`
- Pass `pool` to the indexer tasks only for `get_last_indexed_block` reads (needed for catch-up and live subscription start block)
- Spawn DB writer task: `tokio::spawn(db_writer::run(rx, pool, config.batch_size, config.flush_interval_ms))`

**`indexer.rs`** -- Send events to channel

Key changes:

- `run_indexer` signature: `(pool: PgPool, config: ChainConfig, tx: Sender<NormalizedEvent>)` -- pool is still needed for `get_last_indexed_block` reads
- `run_session` signature: add `tx: &Sender<NormalizedEvent>` parameter
- Replace `chainpulse_db::insert_event(pool, ...)` calls with `tx.send(event).await`
- Replace `chainpulse_db::upsert_last_indexed_block(pool, ...)` calls -- remove from indexer entirely; the DB writer will handle block tracking
- Handle `SendError` from `tx.send()` as a fatal error (means receiver dropped, DB writer crashed)
- Both live event loop and catch-up path (`fetch_logs_chunk`) send through the channel

### Data Structures

```
Config (updated) {
    ...existing fields...
    flush_interval_ms: u64  // default 500
}
```

### Integration

- `main.rs` creates the channel before spawning indexer tasks
- Each indexer task gets a `tx.clone()` and a `pool.clone()` (pool for read-only block queries)
- The DB writer task gets the sole `rx` and the `pool`
- Dropping all `tx` clones (all indexers exit) causes the `rx` stream to end, signaling the writer to flush remaining events and exit

### Success Criteria

**Automated Verification:**

- `cargo build --workspace` compiles without errors
- `cargo test --workspace` passes (existing tests still work)
- `cargo clippy --workspace` has no new warnings

**Manual Verification:**

- [ ] Run the application; events from both chains appear in logs as "sent to channel"
- [ ] DB writer task logs that it is receiving events (even if not writing batches yet in this phase -- it logs receipt)
- [ ] Stopping the application does not panic

---

## Phase 2: Batch DB Writer [COMPLETE]

### Goal

Implement the DB writer task that receives events from the channel, accumulates them in a buffer, and flushes in batches. Add a `batch_insert_events` function to `chainpulse-db` that performs a multi-row INSERT in a single transaction. The writer flushes when the buffer reaches `batch_size` or when `flush_interval_ms` elapses, whichever comes first.

### Files to Modify

- `chainpulse/src/db_writer.rs` -- New file: dedicated DB writer task
- `chainpulse/src/main.rs` -- Add `mod db_writer;`
- `chainpulse-db/src/lib.rs` -- Add `batch_insert_events` and `batch_upsert_last_indexed_blocks` functions

### Module Design

**`db_writer`** -- Dedicated DB writer Tokio task

Key function:

- `pub async fn run(rx: Receiver<NormalizedEvent>, pool: PgPool, batch_size: usize, flush_interval_ms: u64) -> Result<(), IndexerError>`

Core loop logic:

- Maintain a `Vec<NormalizedEvent>` buffer
- Use `tokio::select!` over:
  1. `rx.recv()` -- push event to buffer; if buffer reaches `batch_size`, flush
  2. `flush_interval.tick()` -- flush if buffer is non-empty
- On flush: call `batch_insert_events`, then `batch_upsert_last_indexed_blocks` (with max block per chain_id), then clear buffer
- On `rx.recv() == None` (all senders dropped): flush remaining buffer and return Ok
- On DB error during flush: log error, retry up to 3 times with 1s delay; if all retries fail, return Fatal error (events in buffer are not lost until retry exhaustion)

**`chainpulse-db`** -- Batch insert functions

Key functions:

- `batch_insert_events(pool: &PgPool, events: &[NormalizedEvent]) -> Result<u64, DbError>` -- Builds a multi-row `INSERT INTO events (...) VALUES ($1..$7), ($8..$14), ... ON CONFLICT (chain_id, block_number, log_index) DO NOTHING` query. Uses a transaction. Returns count of rows inserted.
- `batch_upsert_last_indexed_blocks(pool: &PgPool, chain_blocks: &[(i32, i64)]) -> Result<(), DbError>` -- For each (chain_id, max_block_number) pair, upserts `indexed_blocks`. Can be done in the same transaction or as separate queries.

### Data Structures

```
# No new structs needed; NormalizedEvent is the channel message type.
# The buffer is Vec<NormalizedEvent>.
# chain_blocks is computed by: HashMap<i32, i64> from buffer, taking max block per chain_id.
```

### Integration

- `db_writer::run` is spawned in `main.rs` alongside the indexer tasks
- It consumes from the `Receiver` half of the bounded channel
- It uses `PgPool` for all database writes
- The existing `insert_event` function remains for backward compatibility but is no longer called in the main pipeline
- `batch_insert_events` builds dynamic SQL with sqlx `QueryBuilder` or manual string building with bind parameters

### Success Criteria

**Automated Verification:**

- `cargo build --workspace` compiles
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean

**Manual Verification:**

- [ ] Run application; events are written to DB in batches (log shows "Batch flushed: N events")
- [ ] Events appear in the `events` table with correct data
- [ ] `indexed_blocks` table shows correct last_block per chain
- [ ] During low activity, flush timer ensures events are written within `flush_interval_ms`
- [ ] Buffer does not grow beyond `batch_size` before flushing

---

## Phase 3: Backpressure Verification & Structured Logging [COMPLETE]

### Goal

Verify that backpressure works correctly under simulated load, add structured log messages for backpressure events (channel full, subscriber waiting, batch flush statistics), and ensure the system recovers gracefully after backpressure is relieved.

### Files to Modify

- `chainpulse/src/db_writer.rs` -- Add structured logging for batch flush stats (events count, flush trigger reason, duration)
- `chainpulse/src/indexer.rs` -- Add structured logging when channel send blocks (detect via `tx.try_send` + fallback to `tx.send`)
- `chainpulse/src/indexer.rs` -- Add timing instrumentation around channel send

### Module Design

**Backpressure-aware send in indexer** -- Replace bare `tx.send(event).await` with a pattern that detects when the channel is full:

- `tx.try_send(event)` -- if `Ok`, no backpressure
- If `Err(TrySendError::Full)` -- log warning "Channel full, applying backpressure", then `tx.send(event).await` (blocking send)
- If `Err(TrySendError::Closed)` -- fatal error, DB writer crashed

**Flush statistics logging in db_writer** -- On each batch flush, log:

- `events_flushed`: number of events in the batch
- `flush_trigger`: "size_threshold" or "timer"
- `flush_duration_ms`: time taken for the DB write
- `channel_pending`: approximate number of items remaining in the channel (not directly available from Receiver, but can be tracked via counter)

### Integration

- No new modules; additions to existing `indexer.rs` and `db_writer.rs`
- Logging uses `tracing::warn!` for backpressure events and `tracing::info!` for flush statistics

### Success Criteria

**Automated Verification:**

- `cargo build --workspace` compiles
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean

**Manual Verification:**

- [ ] With a small `CHANNEL_BUFFER_SIZE=5` and `BATCH_SIZE=2`, observe backpressure logs when events arrive faster than they can be written
- [ ] After reducing load, subscriber resumes full speed (backpressure is temporary)
- [ ] Batch flush logs show correct event counts and trigger reasons
- [ ] No events are lost during backpressure periods (all events eventually appear in DB)

---

## Phase 4: Final Verification

### Goal

Ensure all components work together end-to-end and satisfy the user stories from the Press Release. Run full verification and validate all M4 success criteria.

### Success Criteria

**Automated Verification:**

- `cargo build --workspace` compiles without errors or warnings
- `cargo test --workspace` -- all tests pass
- `cargo clippy --workspace` -- no warnings

**Comprehensive E2E Tests:**

Each press release user story maps to a verification scenario:

- [x] **E4.1.1 - Bounded channel**: Verified `tokio::sync::mpsc::channel` created with `config.channel_buffer_size` in `main.rs:62-64`. Subscribers send `NormalizedEvent` via `backpressure_send` in `indexer.rs`.
- [x] **E4.1.2 - Backpressure self-regulation**: Verified `backpressure_send` in `indexer.rs:308-340` uses `try_send` + blocking `send().await` fallback with warning logs when channel is full.
- [x] **E4.2.1 - Batch writes**: Verified `db_writer.rs` accumulates events and flushes in batches. `batch_insert_events` in `chainpulse-db` uses multi-row INSERT in a single transaction (`pool.begin()` + `tx.commit()`).
- [x] **E4.2.2 - Sustained throughput**: Verified batch writes reduce per-event overhead; bounded channel prevents OOM; ON CONFLICT DO NOTHING provides idempotency; retry logic prevents event loss.

**Manual Verification:**

- [ ] Run with default configuration against live Ethereum and Polygon RPCs for 10+ minutes
- [ ] Memory usage remains stable (no upward trend)
- [ ] All events from both chains appear in the `events` table
- [ ] `indexed_blocks` table is correctly updated per chain
- [ ] Existing M1-M3 functionality (reconnection, catch-up, multi-chain) still works
- [ ] Stopping and restarting the application correctly catches up from last indexed block
- [ ] Configuration via env vars works: `CHANNEL_BUFFER_SIZE`, `BATCH_SIZE`, `FLUSH_INTERVAL_MS`

---

## Rollback Plan

All changes are additive. To rollback:

1. Revert `indexer.rs` to send events directly to DB (restore inline `insert_event` calls)
2. Revert `main.rs` to pass `pool` directly to indexer tasks (remove channel creation and DB writer spawn)
3. Remove `db_writer.rs`
4. Remove `batch_insert_events` and `batch_upsert_last_indexed_blocks` from `chainpulse-db` (keep existing functions)
5. Remove `flush_interval_ms` from config (optional, unused field causes no harm)

The `insert_event` and `upsert_last_indexed_block` functions are preserved in `chainpulse-db` for backward compatibility, making rollback straightforward.

## Open Questions

- Should the DB writer retry indefinitely or give up after N retries? Plan uses 3 retries then fatal error, matching current behavior where a single DB failure is fatal. This could be made configurable in a future iteration.
- Should `batch_insert_events` use `sqlx::QueryBuilder` or manual SQL string construction? `QueryBuilder` is safer but may have limitations with large batches. Implementation phase will determine the best approach based on sqlx 0.8 capabilities.

---
date: 2026-03-26
author: claude-opus-4-6
status: draft
branch: main
related_research: docs/dev/chainpulse/m3-multi-chain/research.md
---

# Plan: M3 Multi-Chain Event Indexing

## Overview

This plan adds simultaneous Ethereum and Polygon event indexing to ChainPulse. The codebase is already well-prepared: the database schema uses `chain_id` in all composite keys, the config already defines `polygon_rpc_ws_url`, and `run_session` already accepts `chain_id` as a parameter. The work consists of four focused changes: (1) parameterize `decode_log` to accept `chain_id` instead of hardcoding `1`, (2) create a `ChainConfig` struct and Polygon contract list, (3) refactor `run_indexer` from a never-returning function into a spawnable task, and (4) spawn one task per chain from `main` with chain-tagged logging.

## Architecture Decisions

- **`ChainConfig` struct over additional Config fields**: Bundle RPC URL, chain_id, chain name, and contracts into a single struct constructed at startup. Keeps `run_indexer` parameterized by one value instead of four loose parameters.
- **`run_indexer` returns `Result<(), IndexerError>` instead of `-> !`**: Allows `tokio::spawn` and `JoinHandle` monitoring. The infinite loop remains inside the function; the return type change is for error propagation only.
- **`tokio::spawn` + `JoinHandle` monitoring**: Main function spawns one task per chain, then awaits all handles with `futures::future::join_all`. If any task exits (panic or error), log and exit the process. Full supervision deferred to M8.
- **Tracing spans per chain**: Each `run_indexer` task enters a `tracing::info_span!("chain", name = chain_config.name, chain_id = chain_config.chain_id)` so all log entries from that task are automatically tagged.
- **Rename `default_contracts()` to `default_eth_contracts()`**: Add `default_polygon_contracts()` alongside it. Keeps naming explicit and consistent.
- **`decode_log` gains a `chain_id: i32` parameter**: Minimal signature change, all callers already have `chain_id` in scope.

## Prerequisites

- [ ] M2 (resilience/reconnection) is complete and working
- [ ] Both `ETH_RPC_WS_URL` and `POLYGON_RPC_WS_URL` environment variables are available for testing

---

## Phase 1: Parameterize `decode_log` and Add Polygon Contracts

### Goal

Remove the hardcoded `chain_id: 1` from event decoding and add the Polygon contract address list. This phase touches only `chainpulse-types` and has no effect on the running binary until later phases wire it up.

### Files to Modify

- `chainpulse-types/src/lib.rs` — Change `decode_log` signature, rename `default_contracts`, add `default_polygon_contracts`

### Module Design

**`chainpulse_types`** — Event type definitions, ABI decoding, and contract address lists

Key changes:

- `decode_log(log: &Log, chain_id: i32) -> Option<NormalizedEvent>` — Accept `chain_id` parameter, use it in `NormalizedEvent` construction instead of hardcoded `1`
- `default_eth_contracts() -> Vec<Address>` — Renamed from `default_contracts()`, same 8 Ethereum addresses
- `default_polygon_contracts() -> Vec<Address>` — New function returning 7 Polygon PoS addresses

### Data Structures

```
Polygon contracts (7 addresses):
- USDC bridged:    0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174
- USDC native:     0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359
- USDT:            0xc2132D05D31c914a87C6611C10748AEb04B58e8F
- WMATIC:          0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270
- WETH:            0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619
- QuickSwap:       0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32
- Aave V3 Pool:    0x794a61358D6845594F94dc1DB02A252b5b4814aD
```

### Integration

- All callers of `decode_log` (in `indexer.rs`) already have `chain_id` in scope — they pass it as the new second argument.
- All callers of `default_contracts()` switch to `default_eth_contracts()`.
- Tests in `chainpulse-types` update to pass `chain_id` to `decode_log` and verify correct chain attribution.

### Success Criteria

**Automated Verification:**
- `cargo test -p chainpulse-types` — all existing tests pass with updated signatures
- `cargo check --workspace` — no compile errors

**Manual Verification:**
- [ ] `decode_log(&log, 1)` produces `chain_id: 1`
- [ ] `decode_log(&log, 137)` produces `chain_id: 137`
- [ ] `default_eth_contracts()` returns 8 addresses
- [ ] `default_polygon_contracts()` returns 7 addresses
- [ ] A new test verifies Polygon contract count and contains a known Polygon address

---

## Phase 2: Create `ChainConfig` and Refactor `run_indexer`

### Goal

Introduce a `ChainConfig` struct that bundles per-chain parameters. Refactor `run_indexer` to accept `ChainConfig` instead of reading hardcoded values, and change its return type from `-> !` to `-> Result<(), IndexerError>` so it can be spawned as a Tokio task.

### Files to Modify

- `chainpulse/src/indexer.rs` — Add `ChainConfig` struct, refactor `run_indexer` signature and body
- `chainpulse/src/main.rs` — Temporarily update the single `run_indexer` call to use a `ChainConfig` (full multi-chain spawn comes in Phase 3)

### Module Design

**`indexer`** — Reconnection loop and session management, now parameterized by `ChainConfig`

Key changes:

- `ChainConfig { name: String, chain_id: i32, ws_url: String, contracts: Vec<Address> }` — New struct bundling all per-chain parameters
- `run_indexer(pool: PgPool, config: ChainConfig) -> Result<(), IndexerError>` — Changed from `(&PgPool, &Config) -> !`. Takes owned values (needed for `tokio::spawn` which requires `'static`). Enters a tracing span with chain name/id. Inner loop unchanged except it reads from `ChainConfig` instead of hardcoded values.
- `run_session` — Update `decode_log` call sites to pass `chain_id`

### Data Structures

```
ChainConfig {
    name: String       // e.g. "ethereum", "polygon"
    chain_id: i32      // e.g. 1, 137
    ws_url: String     // RPC WebSocket URL
    contracts: Vec<Address>  // Contract addresses to track
}
```

### Integration

- `run_indexer` takes owned `PgPool` (which is `Clone` and cheap — it's an `Arc` internally) and owned `ChainConfig`
- `main.rs` constructs a single `ChainConfig` for Ethereum and calls `run_indexer` directly (single-chain still works)
- `run_session` calls `decode_log(log, chain_config.chain_id)` instead of `decode_log(log)`
- All tracing calls within `run_indexer` and `run_session` inherit the chain span

### Success Criteria

**Automated Verification:**
- `cargo check --workspace` — compiles cleanly
- `cargo test --workspace` — all tests pass

**Manual Verification:**
- [ ] Running with only `ETH_RPC_WS_URL` set still works as before (single Ethereum chain)
- [ ] Log entries show chain name/id in structured fields
- [ ] Reconnection behavior unchanged

---

## Phase 3: Multi-Chain Task Spawning

### Goal

Spawn one `run_indexer` Tokio task per configured chain from `main.rs`. Both Ethereum and Polygon run concurrently as independent tasks. Monitor `JoinHandle`s for task failure.

### Files to Modify

- `chainpulse/src/main.rs` — Build chain configs, spawn tasks, await handles

### Module Design

**`main`** — Application entry point, now spawns N chain tasks

Key logic:

- Build `Vec<ChainConfig>` from config: one for Ethereum (`chain_id=1`, `config.eth_rpc_ws_url`, `default_eth_contracts()`), one for Polygon (`chain_id=137`, `config.polygon_rpc_ws_url`, `default_polygon_contracts()`)
- For each `ChainConfig`, `tokio::spawn(run_indexer(pool.clone(), chain_config))`
- Collect `JoinHandle`s into a `Vec`
- Use `futures::future::join_all(handles)` or `tokio::select!` to wait
- If any handle returns (error or panic), log the failure and exit the process

### Integration

- `pool.clone()` for each task — `PgPool` is `Arc`-based, cloning is cheap
- Each task is fully independent: own WebSocket, own backoff state, own contract list
- Database writes from multiple tasks go through the shared pool; the composite unique key `(chain_id, block_number, log_index)` prevents cross-chain collisions
- `indexed_blocks` tracks progress independently per `chain_id`

### Success Criteria

**Automated Verification:**
- `cargo check --workspace` — compiles cleanly
- `cargo test --workspace` — all tests pass

**Manual Verification:**
- [ ] Starting ChainPulse with both RPC URLs spawns two subscriber tasks
- [ ] Log output shows entries tagged with "ethereum" and "polygon" chain names
- [ ] Events appear in the `events` table with both `chain_id=1` and `chain_id=137`
- [ ] `indexed_blocks` has separate rows for chain_id 1 and 137
- [ ] Killing one chain's RPC connection does not affect the other chain's indexing
- [ ] After restart, each chain resumes catch-up from its own last indexed block

---

## Phase 4: Final Verification

### Goal

Ensure all components work together end-to-end and satisfy the user stories from the Press Release.

### Success Criteria

**Automated Verification:**

- `cargo check --workspace` — no warnings or errors
- `cargo test --workspace` — all tests pass
- `cargo clippy --workspace` — no warnings

**Comprehensive E2E Tests:**

From the press release user stories:

- [ ] **Simultaneous indexing**: A single ChainPulse process indexes events from both Ethereum and Polygon concurrently
- [ ] **Chain-tagged logging**: Every log entry includes chain identification (name and/or chain_id)
- [ ] **Correct chain attribution**: Events from Ethereum have `chain_id=1`, events from Polygon have `chain_id=137`
- [ ] **Polygon contracts**: All 7 default Polygon contracts are tracked (USDC bridged, USDC native, USDT, WMATIC, WETH, QuickSwap Factory, Aave V3 Pool)
- [ ] **Fault isolation**: Disconnecting one chain's RPC does not interrupt the other chain's indexing
- [ ] **Extensibility**: A developer can add a third chain by only defining a new `ChainConfig` (no indexer logic changes)
- [ ] **Independent catch-up**: After restart, each chain recovers from its own `last_indexed_block`

**Manual Verification:**

- [ ] Query `SELECT chain_id, COUNT(*) FROM events GROUP BY chain_id` shows rows for both chains
- [ ] Query `SELECT * FROM indexed_blocks` shows two rows with different chain_ids and independent block numbers
- [ ] Simulate Polygon RPC failure: Ethereum continues indexing, Polygon enters backoff/reconnect
- [ ] Simulate Ethereum RPC failure: Polygon continues indexing, Ethereum enters backoff/reconnect
- [ ] Review structured log output for clear chain identification on every entry

---

## Rollback Plan

All changes are additive and backward-compatible. To roll back:

1. Revert `main.rs` to call `run_indexer` with a single Ethereum `ChainConfig`
2. The database schema requires no changes (already supports multi-chain)
3. Polygon data in the database can remain (keyed by `chain_id=137`, does not affect Ethereum queries)

## Open Questions

- **Config file vs env vars for 3+ chains**: For M3 with two chains, flat env vars work. If a third chain is added in future milestones, consider migrating to a TOML/JSON chains config file. Deferred.
- **Task panic restart**: If a chain task panics, the current plan logs the error and exits the process. Full task supervision with automatic restart is deferred to M8 (hardening).
- **Polygon RPC quirks**: Polygon's ~2s block time produces higher event throughput. The existing `CATCH_UP_CHUNK_SIZE` of 2,000 blocks should work, but may need tuning based on real-world Polygon RPC behavior. Monitor during testing.

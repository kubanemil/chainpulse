---
date: 2026-03-25
author: Claude Opus 4.6 (1M context)
status: draft
branch: main
related_research: docs/dev/chainpulse/m1-single-chain-indexer/research.md
---

# Plan: M1 — Single-Chain Indexer

## Overview

Build the end-to-end event indexing pipeline for Ethereum: WebSocket subscription to contract logs via Alloy, ABI decoding of seven event types (Transfer, Swap V2, Swap V3, Supply, Withdraw, Staked, PairCreated, PoolCreated) using the `sol!` macro, and database writes with upsert deduplication. This milestone transforms ChainPulse from a skeleton into a working indexer that continuously captures on-chain events into PostgreSQL.

The implementation spans three phases: (1) event type definitions and decoding logic in `chainpulse-types`, (2) database writer in `chainpulse-db`, and (3) WebSocket subscriber and main event loop in the `chainpulse` binary.

## Architecture Decisions

- **Single Tokio task, inline writes**: No channel between subscriber and writer for M1. The main loop calls `stream.next().await`, decodes, writes to DB, and repeats. Channel-based separation is deferred to M4 when backpressure and batching are needed.
- **`chainpulse-types` owns all event definitions and decoding**: The `sol!` macro declarations, `NormalizedEvent` struct, and `decode_log` function all live in `chainpulse-types`. This keeps the Alloy/blockchain domain isolated from the binary and DB crates.
- **Alloy meta-crate with feature flags**: Use `alloy = { version = "1.0", features = ["provider-ws", "pubsub", "rpc-types"] }` plus `futures-util` for stream consumption. Avoids managing individual sub-crate versions.
- **Rust modules for V2/V3 Swap disambiguation**: Since both V2 and V3 have events named `Swap` on-chain (different parameter signatures = different topic0), wrap each in a Rust module (`uniswap_v2::Swap`, `uniswap_v3::Swap`) so the `sol!` macro generates correct distinct signature hashes.
- **Hardcoded contract addresses in `chainpulse-types`**: For M1, default Ethereum mainnet contracts are constants. Config-driven loading from DB or env deferred to later milestones.
- **`ON CONFLICT DO NOTHING` for event upserts**: Events are immutable; if the same `(chain_id, block_number, log_index)` exists, the data is identical. `DO NOTHING` is cheaper than `DO UPDATE`.
- **`block_timestamp` left NULL**: `subscribe_logs` does not provide block timestamps. Fetching per-block timestamps adds RPC calls and complexity. Leave NULL for M1.
- **Exit on error**: WS disconnection or DB write failure causes `process::exit(1)`. No retry or reconnection logic (deferred to M2).

## Prerequisites

- [ ] M0 complete: workspace builds, DB pool creates, migrations run
- [ ] PostgreSQL running with M0 schema applied
- [ ] Access to an Ethereum WebSocket RPC endpoint (e.g., Alchemy, Infura)
- [ ] `ETH_RPC_WS_URL` set in `.env`

---

## Phase 1: Event Type Definitions and Decoding

### Goal

Define all supported event types using the Alloy `sol!` macro, create the `NormalizedEvent` struct, and implement the `decode_log` function that converts raw `alloy::rpc::types::Log` into `NormalizedEvent`.

### Files to Modify

- `Cargo.toml` (workspace root) — add `alloy` and `futures-util` to `[workspace.dependencies]`
- `chainpulse-types/Cargo.toml` — add `alloy` dependency
- `chainpulse-types/src/lib.rs` — event definitions, NormalizedEvent, decode_log

### Module Design

**`chainpulse-types`** — Owns event ABI definitions, normalization, and contract address constants.

Submodules within `lib.rs` (or inline):

- `events` module containing `sol!` declarations organized by protocol:
  - Top-level: `Transfer` (ERC-20)
  - `uniswap_v2` module: `Swap`, `PairCreated`
  - `uniswap_v3` module: `Swap`, `PoolCreated`
  - `aave` module: `Supply`, `Withdraw`
  - `stkaave` module: `Staked`

Key functions:

- `decode_log(log: &alloy::rpc::types::Log) -> Option<NormalizedEvent>` — matches `topic0` against all known `SIGNATURE_HASH` constants, decodes the matching event, returns `NormalizedEvent` with event type name and JSONB-ready payload
- `default_contracts() -> Vec<Address>` — returns the hardcoded list of Ethereum mainnet contract addresses from the project plan

### Data Structures

```
NormalizedEvent {
    event_type: String        // "Transfer", "SwapV2", "SwapV3", "Supply", "Withdraw", "Staked", "PairCreated", "PoolCreated"
    contract_address: String  // checksummed hex
    tx_hash: String           // 0x-prefixed hex
    block_number: i64
    log_index: i32
    chain_id: i32             // always 1 for M1
    payload: serde_json::Value // event-specific fields as JSON object
}
```

Payload structures per event type:

- **Transfer**: `{ "from": "0x...", "to": "0x...", "value": "123456" }`
- **SwapV2**: `{ "sender": "0x...", "amount0_in": "...", "amount1_in": "...", "amount0_out": "...", "amount1_out": "...", "to": "0x..." }`
- **SwapV3**: `{ "sender": "0x...", "recipient": "0x...", "amount0": "...", "amount1": "...", "sqrt_price_x96": "...", "liquidity": "...", "tick": N }`
- **Supply**: `{ "reserve": "0x...", "user": "0x...", "on_behalf_of": "0x...", "amount": "...", "referral_code": N }`
- **Withdraw**: `{ "reserve": "0x...", "user": "0x...", "to": "0x...", "amount": "..." }`
- **Staked**: `{ "from": "0x...", "on_behalf_of": "0x...", "amount": "..." }`
- **PairCreated**: `{ "token0": "0x...", "token1": "0x...", "pair": "0x...", "pair_index": "..." }`
- **PoolCreated**: `{ "token0": "0x...", "token1": "0x...", "fee": N, "tick_spacing": N, "pool": "0x..." }`

All `U256`/`I256`/`U160` values serialized as decimal strings. Addresses as `0x`-prefixed lowercase hex. Tick values as integers.

### Integration

- `chainpulse-types` has no dependency on `chainpulse-db` or `chainpulse-config`
- The binary crate will call `decode_log` on each log from the WS stream
- `NormalizedEvent` fields map directly to the `events` table columns

### Success Criteria

**Automated Verification:**

- `cargo build -p chainpulse-types` compiles without errors
- `cargo test -p chainpulse-types` passes

**Manual Verification:**

- [ ] Unit tests: construct mock `Log` objects with known topic0/data bytes for each event type, verify `decode_log` returns correct `NormalizedEvent` with expected field values
- [ ] Unit test: `Log` with unknown topic0 returns `None`
- [ ] Unit test: `Log` with correct topic0 but malformed data returns `None` (graceful skip)
- [ ] `default_contracts()` returns all 8 Ethereum mainnet addresses from the project plan

---

## Phase 2: Database Writer

### Goal

Add the `insert_event` function to `chainpulse-db` that writes a `NormalizedEvent` to the `events` table with upsert deduplication.

### Files to Modify

- `chainpulse-db/Cargo.toml` — add `serde_json` dependency (for JSONB payload binding)
- `chainpulse-db/src/lib.rs` — add `insert_event` function

### Module Design

**`chainpulse-db`** — Extends with event insertion capability.

Key functions:

- `insert_event(pool: &PgPool, chain_id: i32, block_number: i64, log_index: i32, tx_hash: &str, contract_address: &str, event_type: &str, payload: &serde_json::Value) -> Result<bool, DbError>` — inserts event using `ON CONFLICT (chain_id, block_number, log_index) DO NOTHING`. Returns `Ok(true)` if inserted, `Ok(false)` if duplicate was skipped. Uses `sqlx::query` with positional bind parameters.

### Integration

- The binary crate extracts fields from `NormalizedEvent` and passes them to `insert_event`
- Follows existing patterns: standalone `pub async fn`, `&PgPool` first parameter, returns `Result<_, DbError>`
- `serde_json::Value` binds directly to the `JSONB` column via sqlx's `json` feature (already enabled in workspace `Cargo.toml`)

### Success Criteria

**Automated Verification:**

- `cargo build -p chainpulse-db` compiles without errors

**Manual Verification:**

- [ ] Integration test: insert an event, verify it appears in the `events` table with correct fields
- [ ] Integration test: insert the same event twice (same chain_id, block_number, log_index), verify no error and only one row exists
- [ ] Integration test: insert events with different composite keys, verify all are stored

---

## Phase 3: WebSocket Subscriber and Main Event Loop

### Goal

Connect to Ethereum via WebSocket using Alloy, subscribe to logs filtered by contract addresses, decode each log using `chainpulse-types`, write to DB using `chainpulse-db`, and log processing activity via `tracing`.

### Files to Modify

- `chainpulse/Cargo.toml` — add `alloy`, `futures-util`, `chainpulse-types` dependencies
- `chainpulse/src/main.rs` — add WS connection, subscription, and event processing loop after DB setup

### Module Design

**`chainpulse` (binary)** — Orchestrates the indexing pipeline.

The main function extends the existing M0 startup sequence with:

1. Create WS provider: `ProviderBuilder::new().connect_ws(WsConnect::new(&config.eth_rpc_ws_url)).await`
2. Build log filter: `Filter::new().address(chainpulse_types::default_contracts()).from_block(BlockNumberOrTag::Latest)`
3. Subscribe: `provider.subscribe_logs(&filter).await` -> `sub.into_stream()`
4. Event loop: `while let Some(log) = stream.next().await` -> `decode_log(&log)` -> `insert_event(...)` -> log result
5. On stream end (WS disconnect): log error, exit
6. On DB write error: log error, exit

### Integration

- Extends `main.rs` after the existing `"Database connected, migrations applied"` log line
- Uses `config.eth_rpc_ws_url` (already in Config struct)
- `chainpulse-types::decode_log` converts raw logs to `NormalizedEvent`
- `chainpulse-db::insert_event` writes to PostgreSQL
- `tracing::info!` for each processed event (event_type, block_number, contract_address)
- `tracing::debug!` for skipped unknown events
- `tracing::error!` + `process::exit(1)` for WS connection failure, subscription failure, DB write failure, and WS disconnect

### Data Flow

```
config.eth_rpc_ws_url
    -> WsConnect::new() -> ProviderBuilder::connect_ws()
    -> Filter::new().address(default_contracts())
    -> provider.subscribe_logs(&filter)
    -> sub.into_stream()
    -> while let Some(log) = stream.next().await
        -> chainpulse_types::decode_log(&log)
            -> Some(event) => chainpulse_db::insert_event(pool, event fields...)
            -> None => tracing::debug!(skipped)
    -> stream ends => tracing::error! + exit(1)
```

### Success Criteria

**Automated Verification:**

- `cargo build` (full workspace) compiles without errors
- `cargo clippy` passes without warnings

**Manual Verification:**

- [ ] Start ChainPulse with a valid `ETH_RPC_WS_URL` and `DATABASE_URL`; verify logs show "WebSocket connected" and "Subscribed to logs"
- [ ] Wait for events; verify `tracing` output shows decoded events with type, block number, and contract address
- [ ] Query `SELECT * FROM events LIMIT 10;` and verify rows contain correct `event_type`, `chain_id = 1`, populated `payload` JSONB
- [ ] Verify `block_timestamp` is NULL (expected for M1)
- [ ] Stop and restart the process; verify duplicate events are silently skipped (row count does not inflate)
- [ ] Test with invalid `ETH_RPC_WS_URL`; verify clear error message and exit

---

## Phase 4: Final Verification

### Goal

Ensure all components work together end-to-end and satisfy the user stories from the press release.

### Success Criteria

**Automated Verification:**

- `cargo build` succeeds
- `cargo test` passes (all unit tests from Phase 1 + integration tests from Phase 2)
- `cargo clippy` clean

**End-to-End Verification:**

- [ ] Start ChainPulse pointed at a live Ethereum WS RPC endpoint with all 8 default contracts configured
- [ ] Let it run for several minutes; verify events appear in PostgreSQL
- [ ] Verify Transfer events from USDC/USDT/WETH/DAI have correct `from`, `to`, `value` fields in payload
- [ ] Verify Swap events from Uniswap contracts decode correctly (V2 and V3 distinguished by `event_type`)
- [ ] Verify Supply/Withdraw events from Aave V3 Pool decode correctly
- [ ] Verify Staked events from stkAAVE decode correctly
- [ ] Verify PairCreated/PoolCreated factory events decode correctly (if any occur during test window)
- [ ] Verify unknown events from tracked contracts are skipped with debug log, no crash
- [ ] Verify duplicate handling: stop and restart, confirm no duplicate rows created
- [ ] Verify error behavior: disconnect DB, confirm error log and exit
- [ ] Verify error behavior: use invalid WS URL, confirm error log and exit on startup

**User Story Coverage:**

- [ ] E1.1.1/E1.1.2: System connects to Ethereum WS and subscribes filtered by contract addresses
- [ ] E1.1.3: System receives raw log events in real-time
- [ ] E1.2.1: Transfer events decoded into typed structs
- [ ] E1.2.2: Swap events from V2/V3 decoded correctly
- [ ] E1.2.3: Staked events from stkAAVE decoded correctly
- [ ] E1.2.4: Adding a new event type requires only a `sol!` declaration and mapping in `decode_log`
- [ ] E1.3.1: Normalized events written to PostgreSQL
- [ ] E1.3.2: Upsert dedup via composite key works correctly

---

## Rollback Plan

All M1 changes are additive (new functions, new dependencies, extended main.rs). Rollback is reverting the M1 commits. No schema migrations are needed since the `events` table already exists from M0.

## Open Questions

1. **High-volume contracts**: USDC and USDT Transfer events are extremely high-volume on mainnet. M1 writes one event at a time without batching. If the single-write approach cannot keep up, the immediate mitigation is to reduce the contract list to fewer addresses. Batch writes come in M4.

2. **Factory events rarity**: PairCreated and PoolCreated events are infrequent (only when new pairs/pools are deployed). They may not appear during short test runs. Verify via unit tests with mock data rather than relying on live events.

3. **Alloy `Log` field optionality**: Fields like `block_number`, `log_index`, and `transaction_hash` on `alloy::rpc::types::Log` may be `Option`. The decode function should handle `None` values gracefully (skip the log with a warning rather than panicking).

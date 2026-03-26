---
date: 2026-03-26
researcher: claude-opus-4-6
branch: main
repository: chainpulse
tags: multi-chain, polygon, indexer, tokio, websocket
status: complete
---

# Research: M3 Multi-Chain Support

## Research Question

What is the current architecture of ChainPulse, and what specific code changes are required to support simultaneous indexing of Ethereum and Polygon from a single process?

## Summary

ChainPulse is a Cargo workspace with four crates: `chainpulse` (binary), `chainpulse-config`, `chainpulse-db`, and `chainpulse-types`. The current implementation runs a single Ethereum indexer as a blocking reconnection loop in `run_indexer`, which calls `run_session` in a `loop {}` with exponential backoff. The indexer connects via WebSocket, runs catch-up from the last indexed block, then subscribes to live logs and processes events through `decode_log` into PostgreSQL.

The codebase is well-prepared for multi-chain. The database schema already uses `chain_id` as part of composite keys (`events` table unique on `(chain_id, block_number, log_index)`, `indexed_blocks` keyed by `chain_id`). The config already has `polygon_rpc_ws_url` defined. The `run_session` function already accepts `chain_id` as a parameter. The primary gaps are: (1) `decode_log` hardcodes `chain_id: 1` in every `NormalizedEvent`, (2) there is no Polygon contract address list, (3) `run_indexer` is a `-> !` (never-returning) function called directly from `main`, making it impossible to spawn multiple chains, and (4) logs do not identify which chain produced events.

The changes required are structurally minimal: parameterize `decode_log` to accept `chain_id`, create a chain config struct, add Polygon default contracts, refactor `run_indexer` to be spawnable as a Tokio task, and spawn one task per configured chain from `main`.

## Detailed Findings

### Workspace Structure

The project is organized as a Cargo workspace with four member crates:

- `chainpulse/` — Binary crate containing `main.rs` and `indexer.rs`
- `chainpulse-config/` — Configuration loading via `dotenvy` + `envy`
- `chainpulse-db/` — PostgreSQL operations via `sqlx`
- `chainpulse-types/` — Event type definitions, ABI decoding, and contract address lists

All crates use workspace-level dependency declarations in the root `Cargo.toml`.

### Entry Point and Indexer Loop (`chainpulse/src/main.rs`)

The `main` function at `chainpulse/src/main.rs:6` loads config, initializes tracing with JSON output, creates a database pool, runs migrations, and then calls `indexer::run_indexer(&pool, &config).await` at line 44. This call never returns (the function signature is `-> !`), which means only one indexer can run.

Key observation: `run_indexer` is called with `await` directly in `main`, blocking the async runtime on a single chain. For multi-chain, this must change to `tokio::spawn` for each chain, with the main function awaiting all handles.

### Reconnection Loop (`chainpulse/src/indexer.rs:35-82`)

`run_indexer` is the outer reconnection loop. At line 41, it calls `chainpulse_types::default_contracts()` to get Ethereum addresses and hardcodes `chain_id: i32 = 1` at line 42. It then loops forever, calling `run_session` at line 54 with these parameters.

The function signature is `pub async fn run_indexer(pool: &PgPool, config: &Config) -> !`. The `-> !` return type means it never returns. For multi-chain, this needs to change to a return type compatible with `tokio::spawn` (e.g., returning a `Result` or being an infinite loop inside a spawned task).

The backoff logic (lines 36-38) uses `initial_backoff = 1s`, `max_backoff = 60s`, doubling on each failure. This is per-chain and does not need modification — each chain will have its own backoff state.

### Session Logic (`chainpulse/src/indexer.rs:87-199`)

`run_session` accepts `pool`, `ws_url`, `contracts`, `chain_id`, and `retry_count`. It:
1. Connects WebSocket via `WsConnect` and `ProviderBuilder` (lines 95-99)
2. Runs `catch_up` (line 109)
3. Determines live subscription starting block from `get_last_indexed_block` (lines 114-130)
4. Subscribes to `eth_subscribe("logs")` filtered by contract addresses (lines 133-141)
5. Processes events in a `while let Some(log)` loop (lines 152-195), calling `decode_log` and then `insert_event` + `upsert_last_indexed_block`

This function is already parameterized by `ws_url`, `contracts`, and `chain_id`. It does not depend on any Ethereum-specific logic beyond what is passed in. It is ready for multi-chain use as-is.

### Catch-Up Logic (`chainpulse/src/indexer.rs:209-268`)

The `catch_up` function queries `get_last_indexed_block(pool, chain_id)` and fetches missed logs via chunked `eth_getLogs` calls with `CATCH_UP_CHUNK_SIZE = 2_000`. Each chunk is processed through `fetch_logs_chunk` which calls `decode_log` and `insert_event`. The `last_indexed_block` is updated after each chunk for crash recovery.

This function is already parameterized by `chain_id` and `contracts`. No changes needed for multi-chain.

### Event Decoding (`chainpulse-types/src/lib.rs:134-252`)

The `decode_log` function at line 134 matches `topic0` against known event signature hashes (Transfer, SwapV2, SwapV3, Supply, Withdraw, Staked, PairCreated, PoolCreated). It constructs a `NormalizedEvent` at lines 243-251.

**Critical finding**: Line 249 hardcodes `chain_id: 1` in every `NormalizedEvent`. This is the primary code change needed — `decode_log` must accept `chain_id` as a parameter.

The function signature is `pub fn decode_log(log: &alloy::rpc::types::Log) -> Option<NormalizedEvent>`. It needs to become `pub fn decode_log(log: &alloy::rpc::types::Log, chain_id: i32) -> Option<NormalizedEvent>`.

### Contract Address Lists (`chainpulse-types/src/lib.rs:257-269`)

The `default_contracts()` function returns 8 Ethereum mainnet addresses (USDC, USDT, WETH, DAI, Uniswap V2 Factory, Uniswap V3 Factory, Aave V3 Pool, stkAAVE). There is no Polygon equivalent.

The project plan specifies 7 Polygon PoS contracts:
- USDC (bridged): `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`
- USDC (native): `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359`
- USDT: `0xc2132D05D31c914a87C6611C10748AEb04B58e8F`
- WMATIC: `0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270`
- WETH: `0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619`
- QuickSwap Factory: `0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32`
- Aave V3 Pool: `0x794a61358D6845594F94dc1DB02A252b5b4814aD`

A `default_polygon_contracts()` function needs to be created, and the existing function should be renamed to `default_eth_contracts()`.

### NormalizedEvent Struct (`chainpulse-types/src/lib.rs:114-123`)

The `NormalizedEvent` struct has fields: `event_type`, `contract_address`, `tx_hash`, `block_number`, `log_index`, `chain_id`, and `payload`. The `chain_id` field is already present and typed as `i32`. No schema change needed.

### Configuration (`chainpulse-config/src/lib.rs:34-57`)

The `Config` struct already has both `eth_rpc_ws_url: String` (line 37) and `polygon_rpc_ws_url: String` (line 38). Both are required (no `#[serde(default)]`). The `.env.example` already documents `POLYGON_RPC_WS_URL`.

No changes needed to the config crate for basic multi-chain support.

### Database Layer (`chainpulse-db/src/lib.rs`)

All database functions already accept `chain_id` as a parameter:
- `get_last_indexed_block(pool, chain_id)` — line 27
- `upsert_last_indexed_block(pool, chain_id, block)` — line 37
- `insert_event(pool, chain_id, ...)` — line 54

The SQL uses `chain_id` in all queries. The `events` table has `UNIQUE(chain_id, block_number, log_index)` for deduplication. The `indexed_blocks` table has `chain_id` as `PRIMARY KEY`.

No changes needed to the database layer.

### Database Schema (`chainpulse-db/migrations/001_initial_schema.sql`)

Three tables exist:
- `events` — unique on `(chain_id, block_number, log_index)`, indexed on `(chain_id, event_type)`, `(contract_address)`, and `(chain_id, block_number)`
- `indexed_blocks` — keyed by `chain_id`
- `tracked_contracts` — unique on `(chain_id, address)`

All tables already support multi-chain. No migration changes needed.

### Test Coverage (`chainpulse-types/src/lib.rs:274-595`)

There are 11 tests in `chainpulse-types` covering all event types (Transfer, SwapV2, SwapV3, Supply, Withdraw, Staked, PairCreated, PoolCreated), unknown topics, malformed data, empty topics, contract count, and USDC presence. Tests at line 327 assert `event.chain_id == 1`, which will need updating when `decode_log` signature changes.

### Logging

The binary uses `tracing_subscriber` with JSON output (line 15-21 of `main.rs`). The indexer logs at `indexer.rs` use `tracing::info!` and `tracing::warn!` with structured fields. Current log entries do not include chain name or chain_id fields — they only log `ws_url`, `retry_count`, `backoff_secs`, `contract_count`, `block_number`, etc.

For multi-chain, each subscriber task should add chain identification (name and/or chain_id) to its log spans so all log entries from that task are tagged with the chain.

## Code References

- `chainpulse/src/main.rs:44` — Single `run_indexer` call that blocks on one chain
- `chainpulse/src/indexer.rs:35` — `run_indexer` function signature with `-> !` return type
- `chainpulse/src/indexer.rs:41-42` — Hardcoded Ethereum contracts and `chain_id = 1`
- `chainpulse/src/indexer.rs:54` — `run_session` call with `config.eth_rpc_ws_url`
- `chainpulse/src/indexer.rs:87-93` — `run_session` signature (already parameterized)
- `chainpulse-types/src/lib.rs:134` — `decode_log` function signature (missing `chain_id` param)
- `chainpulse-types/src/lib.rs:249` — Hardcoded `chain_id: 1` in NormalizedEvent
- `chainpulse-types/src/lib.rs:257-269` — `default_contracts()` (Ethereum only, needs rename)
- `chainpulse-config/src/lib.rs:37-38` — Both RPC URLs already in config
- `chainpulse-db/src/lib.rs:27-52` — DB functions already parameterized by chain_id
- `chainpulse-db/migrations/001_initial_schema.sql:13` — Composite unique key includes chain_id

## Architecture Notes

The current architecture follows a linear flow: `main` -> `run_indexer` (reconnect loop) -> `run_session` (connect, catch-up, subscribe, process). For multi-chain, this becomes a fan-out pattern: `main` spawns N `run_indexer` tasks (one per chain), each running independently with its own reconnection state, WebSocket connection, and catch-up progress.

The shared resources are:
- **Database pool** (`PgPool`) — shared across all chain tasks via `Arc` or reference. This is safe because `sqlx::PgPool` is already `Clone` and internally uses connection pooling.
- **Config** — read once at startup, then distributed to each chain task.

Each chain task owns:
- Its own WebSocket connection
- Its own backoff/retry state
- Its own contract address list
- Its own `chain_id`

The independence of chain tasks is already implied by the existing code structure — `run_session` has no shared mutable state between calls. The only coordination point is the database, which handles concurrency via PostgreSQL's own transaction isolation.

A chain configuration struct is needed to bundle: RPC WebSocket URL, chain_id, chain name (for logging), and contract address list. This struct can be constructed in `main.rs` from the existing `Config` fields.

For task lifecycle management, `tokio::spawn` returns a `JoinHandle`. The main function should use `tokio::select!` or `futures::future::join_all` to wait on all handles. If a handle completes (due to panic), the main function can log the failure. Full restart logic for panicked tasks is deferred to M8 (hardening).

## Open Questions

- Should chain configuration use a TOML/JSON file rather than flat env vars? For M3 with only two chains, flat env vars work fine. A config file format would be better at 5+ chains but is out of scope for M3.
- Polygon PoS uses chain_id 137. This is confirmed as the standard chain_id returned by Polygon PoS mainnet RPC nodes.
- Should `run_indexer` return a `JoinHandle` that main monitors for panics? For M3, logging the panic via `JoinHandle` error is sufficient. Full supervision is deferred to M8.
- Are there Polygon-specific RPC quirks? Polygon's ~2s block time means higher event throughput than Ethereum's ~12s. The `CATCH_UP_CHUNK_SIZE` of 2,000 blocks should work for both chains with standard RPC providers. No known WebSocket subscription behavioral differences for EVM-compatible chains.

---

## Next Steps

If you're satisfied with this research, proceed to planning:

/pipe-plan-implementation --project chainpulse --ref m3 --research docs/dev/chainpulse/m3-multi-chain/research.md multi-chain

If you need more research:

- Ask follow-up questions and I'll append to this document
- Or request a new research topic with /pipe-research --project chainpulse --ref m3 [new-topic]

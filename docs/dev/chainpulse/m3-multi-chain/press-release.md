# Multi-Chain Event Indexing - PR FAQ

> **Milestone**: M3 > **Created**: 2026-03-26 > **Status**: Draft

---

## Press Release

### Multi-Chain Event Indexing

**For** protocol analytics teams and developers using ChainPulse
**Who** can currently only index events from a single Ethereum chain, missing activity on Polygon and other EVM networks
**The** Multi-Chain Event Indexing upgrade
**Is a** new capability for the ChainPulse indexer
**That** enables simultaneous real-time event indexing across Ethereum and Polygon from a single ChainPulse instance
**Unlike** the current M2 state where only one hardcoded Ethereum subscriber exists, requiring separate deployments or custom code to monitor additional chains
**Our solution** runs each chain as an independent Tokio task with its own WebSocket connection, reconnection logic, and catch-up state, while sharing a single database and event pipeline — making it trivial to add new EVM chains via configuration alone.

---

ChainPulse now indexes on-chain events from both Ethereum and Polygon simultaneously. Each chain runs as an independent subscriber task with its own WebSocket connection, reconnection with exponential backoff, and catch-up indexing — all within a single process. A failure or reconnection on one chain does not affect the other. Events from every chain flow through the same normalization and database pipeline, tagged with their `chain_id` for clear disambiguation.

Adding a new EVM chain no longer requires code changes. Developers configure an RPC WebSocket URL and a list of contract addresses per chain, and ChainPulse spawns a new subscriber task automatically. The default configuration ships with well-known contracts on both Ethereum mainnet (USDC, USDT, WETH, DAI, Uniswap V2/V3, Aave V3, stkAAVE) and Polygon PoS (USDC bridged/native, USDT, WMATIC, WETH, QuickSwap, Aave V3). Operators can query events across all chains or filter by a specific chain using the existing `chain_id` column, and the `indexed_blocks` table tracks progress independently per chain.

## FAQ

### Customer Experience

Q: How does an operator enable multi-chain indexing?
A: The operator provides a WebSocket RPC URL for each chain they want to index. In the default configuration, `ETH_RPC_WS_URL` and `POLYGON_RPC_WS_URL` are both required environment variables. On startup, ChainPulse spawns one subscriber task per configured chain. No code changes are needed — it is purely configuration-driven.

Q: What does the happy path look like?
A: On startup, ChainPulse reads the configuration, identifies all configured chains (Ethereum chain_id=1, Polygon chain_id=137), and spawns an independent Tokio task for each. Each task connects to its chain's WebSocket RPC, runs catch-up from its own `last_indexed_block`, and then subscribes to live logs. Events from all chains are written to the same `events` table, differentiated by `chain_id`. The operator sees structured log entries tagged with the chain name for each subscriber.

Q: What happens when one chain's RPC goes down?
A: Only that chain's subscriber is affected. It enters the existing exponential backoff reconnection loop (from M2) while the other chain continues indexing normally. Each chain's reconnection and catch-up state is fully independent. Log entries clearly indicate which chain is reconnecting.

Q: How does this interact with existing features?
A: Multi-chain builds directly on top of M1 (single-chain indexer) and M2 (resilience). The event normalization pipeline, database schema, deduplication logic, and reconnection with catch-up all remain unchanged. The key change is that the indexer loop is now parameterized by chain configuration and spawned once per chain rather than running once for a hardcoded Ethereum setup.

### Scope & Non-Goals

Q: What is explicitly NOT included in V1?
A: The following are out of scope for M3:
- Support for chains beyond Ethereum and Polygon (architecture supports it, but only two ship as defaults)
- Dynamic chain addition at runtime (chains are configured at startup; adding a new chain requires a restart)
- Cross-chain event correlation or aggregation (events are stored independently per chain)
- Chain-specific event types (all chains share the same event type definitions)
- Per-chain performance tuning or separate database pools
- Non-EVM chain support

Q: What is the minimal version that still delivers the core benefit?
A: The minimal version is: (1) refactor `run_indexer` to accept chain-specific configuration (RPC URL, chain_id, contract list) instead of hardcoded Ethereum values, (2) spawn one Tokio task per configured chain, (3) provide default Polygon contract addresses, (4) ensure `decode_log` produces correct `chain_id` based on which subscriber decoded the event. Everything else (reconnection, catch-up, dedup) is already built.

Q: What are we deferring to future iterations?
A: Dynamic chain addition/removal without restart, per-chain backpressure tuning (deferred to M4), chain-specific API filtering endpoints (deferred to M5), per-chain health status in the health endpoint (deferred to M7), and chain-specific metrics (deferred to M7).

### Technical Constraint FAQs

Q: What existing system capabilities does this depend on?
A: M3 depends on:
- `run_session` and the reconnection loop from M2 — already handles connect, catch-up, subscribe, and reconnect for a single chain
- `decode_log` from `chainpulse-types` — already decodes all supported event types
- `insert_event` and `upsert_last_indexed_block` from `chainpulse-db` — already parameterized by `chain_id`
- The `events` table composite unique key `(chain_id, block_number, log_index)` — already supports multi-chain dedup
- The `indexed_blocks` table keyed by `chain_id` — already tracks per-chain progress
- Config struct with `polygon_rpc_ws_url` — already defined in `chainpulse-config`

Q: What new capabilities need to be built?
A: New components:
- A chain configuration struct that bundles RPC URL, chain_id, chain name, and contract address list
- Default contract lists for Polygon PoS (7 contracts matching the project plan)
- A `decode_log` variant or parameterization that sets the correct `chain_id` instead of hardcoding `1`
- Task spawning logic in `main.rs` or `indexer.rs` that creates one `run_indexer` task per chain and runs them concurrently
- Structured log fields that include chain name/id for operator clarity

Q: Are there performance, scale, or data constraints?
A: Each chain adds one WebSocket connection and one Tokio task. For two chains this is negligible. Database writes from both chains share the same connection pool, which is acceptable at M3 scale. The existing single-event write pattern (from M1, batch writes deferred to M4) means write throughput is shared, but Polygon and Ethereum combined event rates for the tracked contracts are well within PostgreSQL's single-connection capacity. No additional database pooling or partitioning is needed at this stage.

Q: What is the simplest implementation that delivers the PR outcome?
A: Extract the chain-specific parameters (RPC URL, chain_id, contract list) from `run_indexer` into a struct. Create a `default_polygon_contracts()` function. Fix `decode_log` to accept `chain_id` as a parameter. In `main.rs`, build two chain configs and spawn `tokio::spawn(run_indexer(...))` for each. Use `tokio::select!` or `futures::future::join_all` to run them concurrently. The reconnection loop, catch-up, and event processing code remain identical.

### Edge Cases & Error States

Q: What happens if one chain's RPC URL is missing or invalid?
A: If a required RPC URL environment variable is missing, the config loading fails at startup (existing `envy` behavior) and the process exits with an error. If the URL is present but the endpoint is unreachable, that chain's subscriber enters the reconnection backoff loop while other chains operate normally.

Q: What happens if both chains disconnect simultaneously?
A: Each chain's reconnection loop is independent. Both will backoff and reconnect on their own schedules. There is no coordination or shared state between chain tasks for reconnection. This is by design — chain failures are uncorrelated.

Q: What happens if Polygon produces events much faster than Ethereum?
A: Each chain writes events independently through the shared database pool. At M3 scale (pre-backpressure, pre-batch-writes), this is fine. If one chain's write rate becomes a bottleneck, it will manifest as slower event processing for that chain only. Backpressure and batch writes (M4) will address throughput concerns.

Q: What happens if a chain's subscriber task panics?
A: A panic in a spawned Tokio task terminates that task but does not crash the process. The other chain continues. However, the panicked chain stops indexing silently. The system should detect this (via `JoinHandle` monitoring) and either restart the task or log a fatal error. This is an edge case to address in implementation.

Q: What if log_index values collide across chains for the same block number?
A: The composite unique key is `(chain_id, block_number, log_index)`, so two events from different chains with the same block number and log index are stored as separate rows. There is no collision risk.

### Business & Operational FAQs

Q: How will we know if this is successful?
A: Success means: (1) a single ChainPulse process indexes events from both Ethereum and Polygon simultaneously, (2) the `events` table contains rows with `chain_id=1` (Ethereum) and `chain_id=137` (Polygon), (3) disconnecting one chain's RPC does not affect the other chain's indexing, (4) a developer can add a third chain by only changing configuration, (5) `indexed_blocks` shows independent progress tracking per chain.

Q: What could go wrong post-launch?
A: Potential issues: (1) Polygon's higher block production rate (~2s vs ~12s) generates more events, potentially revealing write throughput bottlenecks before M4 batch writes, (2) different RPC providers may have different rate limits or WebSocket behaviors requiring chain-specific tuning, (3) Polygon contract addresses may differ from what's listed in the project plan (bridged vs native USDC addresses). All are addressable through configuration.

Q: Does this require changes to support/ops processes?
A: Operators must now provide two RPC WebSocket URLs instead of one. The `.env.example` file needs updating with `POLYGON_RPC_WS_URL`. Log monitoring should account for chain-tagged log entries. No other operational changes are needed.

## Implementation Scope

### Current State -> End State

| Current State | End State |
| --- | --- |
| Single hardcoded Ethereum subscriber with chain_id=1 | Independent subscriber tasks for Ethereum (chain_id=1) and Polygon (chain_id=137) |
| `default_contracts()` returns only Ethereum addresses | `default_eth_contracts()` and `default_polygon_contracts()` return chain-specific address lists |
| `decode_log` hardcodes `chain_id: 1` in every NormalizedEvent | `decode_log` accepts `chain_id` as a parameter, producing correct chain attribution |
| `run_indexer` takes a single RPC URL from config | `run_indexer` is parameterized by a chain config struct and spawned per chain |
| Config has `eth_rpc_ws_url` as the only RPC field | Config has both `eth_rpc_ws_url` and `polygon_rpc_ws_url` (polygon already defined) |
| Adding a new chain requires code changes to the indexer | Adding a new chain requires only a new RPC URL and contract list in configuration |
| Logs do not indicate which chain produced an event | Structured logs include chain name and chain_id fields |

### User Stories

- [ ] As an operator, I can start ChainPulse and it simultaneously indexes events from both Ethereum and Polygon so that I get cross-chain visibility from a single process
- [ ] As an operator, I can see which chain each log entry belongs to so that I can monitor and troubleshoot per-chain behavior
- [ ] As a data consumer, I can query events with chain_id=1 for Ethereum and chain_id=137 for Polygon so that I can analyze each chain independently or together
- [ ] As a data consumer, I can see Polygon-specific contracts (USDC bridged, USDC native, USDT, WMATIC, WETH, QuickSwap, Aave V3) being indexed with correct chain attribution
- [ ] As an operator, I can observe that a disconnection on Polygon's RPC does not interrupt Ethereum indexing, and vice versa, so that chain failures are isolated
- [ ] As a developer, I can add a new EVM chain by defining an RPC URL, chain_id, and contract list in configuration so that extending to new chains does not require indexer code changes
- [ ] As an operator, I can see independent catch-up progress per chain after a restart so that each chain recovers from its own last indexed block

### Success Criteria

- [ ] A single ChainPulse process runs subscriber tasks for both Ethereum and Polygon concurrently
- [ ] The `events` table contains rows with both `chain_id=1` and `chain_id=137`
- [ ] The `indexed_blocks` table has separate entries tracking progress for chain_id=1 and chain_id=137
- [ ] Killing the Polygon RPC connection does not affect Ethereum event indexing (and vice versa)
- [ ] `decode_log` produces the correct `chain_id` for events from each chain
- [ ] Default Polygon contracts match the project plan (7 contracts: USDC bridged, USDC native, USDT, WMATIC, WETH, QuickSwap Factory, Aave V3 Pool)
- [ ] Structured log entries include chain identification (name or chain_id) for every subscriber action
- [ ] A developer can add a hypothetical third chain by only modifying configuration (no indexer logic changes)

### Known Compromises

- [ ] Only two chains ship as defaults (Ethereum and Polygon) — architecture supports more, but we are not validating with additional chains in M3
- [ ] Chains are configured at startup only — no hot-reloading or dynamic chain addition without restart
- [ ] All chains share a single database connection pool — per-chain pools deferred until performance data indicates a need
- [ ] No per-chain metrics or health status — deferred to M7 observability milestone
- [ ] Single-event writes continue (no batching) — deferred to M4 backpressure and performance milestone
- [ ] Spawned task panic detection is best-effort — full supervision/restart logic deferred to M8 hardening

### Open Questions / Needs Research

- [ ] Should chain configuration be a TOML/JSON file rather than flat environment variables, to support arbitrary numbers of chains cleanly? Flat env vars work for two chains but become unwieldy at 5+.
- [ ] What is the correct Polygon chain_id to use — 137 (Polygon PoS mainnet)? Confirm this matches what Polygon RPC nodes return in event logs.
- [ ] Should `run_indexer` return a `JoinHandle` that the main function monitors, so that a panicked chain task can be detected and restarted? Or is logging sufficient for M3?
- [ ] Are there Polygon-specific RPC quirks (different `eth_getLogs` limits, different WebSocket subscription behavior) that need chain-specific handling?

---

## Next Steps

When ready to implement:

1. Run `/pipe-research --project chainpulse --ref m3 multi-chain` to analyze codebase
2. Run `/pipe-plan-implementation --project chainpulse --ref m3 multi-chain` to create implementation plan
3. Run `/pipe-implement docs/dev/chainpulse/m3-multi-chain/implementation-plan.md` to execute the plan

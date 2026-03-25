# Single-Chain Indexer - PR FAQ

> **Milestone**: M1 | **Created**: 2026-03-25 | **Status**: Draft

---

## Press Release

### Single-Chain Indexer

**For** protocol analytics teams and blockchain data engineers
**Who** need reliable, real-time indexing of on-chain events from Ethereum but currently face data gaps from unstable Node.js indexers
**The** Single-Chain Indexer
**Is a** new core capability of ChainPulse
**That** continuously captures, decodes, and stores Ethereum contract events (Transfers, Swaps, Stakes) into PostgreSQL in real-time with zero data loss
**Unlike** manually polling RPC endpoints or running fragile Node.js scripts that crash under load and miss events
**Our solution** uses a persistent WebSocket subscription with compile-time ABI decoding in Rust, delivering typed event data into PostgreSQL with upsert-based deduplication so every event is captured exactly once

---

ChainPulse now indexes Ethereum on-chain events in real-time. Analytics teams can point ChainPulse at an Ethereum WebSocket RPC endpoint, specify which contracts to track, and immediately begin receiving decoded event data in their PostgreSQL database. The system subscribes to contract logs via `eth_subscribe`, decodes them into typed structs (Transfer, Swap, Stake), and writes them to the database with composite-key deduplication to prevent duplicates.

This is the first milestone that delivers end-to-end value: events flow from the Ethereum blockchain through WebSocket subscription, through ABI decoding, and into queryable rows in PostgreSQL. Teams no longer need to write custom decoding scripts or worry about whether their indexer silently dropped events. Every event that matches a tracked contract is decoded, normalized, and persisted.

## FAQ

### Customer Experience

Q: How does a developer set up the single-chain indexer?
A: The developer configures their Ethereum WebSocket RPC URL and database connection string via environment variables (already established in M0). They specify which contract addresses to track in the configuration. Then they run the ChainPulse binary, and it begins indexing immediately.

Q: What does the happy path look like?
A: On startup, ChainPulse connects to the Ethereum WebSocket endpoint, subscribes to logs filtered by the configured contract addresses, and begins receiving events. Each raw log is matched against known ABI definitions (Transfer, Swap, Stake), decoded into a typed Rust struct, and inserted into PostgreSQL. The operator sees structured log output confirming events are being processed.

Q: What event types are supported?
A: V1 supports three event types: ERC-20 Transfer events (from USDC, USDT, WETH, DAI), Swap events (from Uniswap V2/V3), and Stake events (from Aave stkAAVE). The system is designed so adding new event types requires only a new `sol!` declaration and a mapping function.

Q: How does a developer know the indexer is working?
A: Structured logs via `tracing` show connection status, subscription confirmations, and event processing counts. Each decoded event is logged with its type, contract address, and block number.

Q: What happens if an event from a tracked contract cannot be decoded?
A: The system logs a warning with the raw log data and skips the event. It does not crash or stop processing other events. Unknown or malformed events are gracefully ignored.

### Scope & Non-Goals

Q: What is explicitly NOT included in V1?
A: The following are out of scope for M1:
- Reconnection logic and catch-up from missed blocks (M2)
- Multi-chain support / Polygon (M3)
- Backpressure via bounded channels and batch writes (M4)
- REST API or WebSocket streaming API (M5/M6)
- Prometheus metrics (M7)
- Graceful shutdown (M8)
- Historical backfill of past blocks

Q: What is the minimal version that still delivers the core benefit?
A: A single Tokio task that subscribes to Ethereum logs, decodes Transfer events only, and writes them to PostgreSQL. Swap and Stake decoding add breadth but Transfer alone proves the pipeline works end-to-end.

Q: What would make us cut scope further if needed?
A: We would drop Swap and Stake event decoding, shipping with only Transfer support. The event type system and extensibility would remain, but only one event type would be implemented.

Q: What are we deferring to future iterations?
A: Reconnection with catch-up (M2), multi-chain (M3), backpressure and batching (M4), and all API endpoints (M5/M6). The M1 indexer assumes a stable WebSocket connection and writes events one at a time.

### Technical Constraint FAQs

Q: What existing system capabilities does this depend on?
A: M0 must be complete: Cargo workspace structure, configuration loading (dotenvy + envy), database schema (events, indexed_blocks, contracts tables), sqlx migrations, and the PostgreSQL connection pool.

Q: What new capabilities need to be built?
A: Three new components:
1. WebSocket subscriber - connects to Ethereum WS RPC and subscribes to filtered logs using Alloy
2. Event normalization layer - decodes raw logs into typed structs using Alloy `sol!` macro
3. Database writer - inserts normalized events into PostgreSQL using sqlx with upsert dedup

Q: Are there performance or scale constraints for M1?
A: M1 does not target high-throughput performance. Events are written individually (not batched). The focus is correctness of the end-to-end pipeline. Batch writes and backpressure come in M4.

Q: What is the simplest implementation that delivers the PR outcome?
A: A single async Tokio task that: (1) opens a WS connection via Alloy, (2) calls `eth_subscribe("logs", {address: [...]})`, (3) for each log, attempts ABI decode via `sol!`-generated types, (4) on success, inserts into the events table via sqlx with ON CONFLICT DO NOTHING.

### Edge Cases & Error States FAQs

Q: What happens if the WebSocket connection drops?
A: In M1, the process logs the disconnection error and exits. Reconnection with exponential backoff is deferred to M2. The operator must restart the process manually.

Q: What happens if the database is unreachable?
A: The process logs the error and exits. There is no retry logic for DB connection failures in M1.

Q: What happens if the RPC URL is invalid or the node is unreachable?
A: The process logs a clear error message and exits on startup.

Q: What happens if a log matches a tracked contract but has an unknown event signature?
A: The log is skipped with a debug-level log. Only logs matching known event topic0 hashes are decoded.

Q: What happens if two logs have the same (chain_id, block_number, log_index)?
A: The upsert uses ON CONFLICT DO NOTHING on the composite key, so the duplicate is silently ignored. This ensures idempotency.

### Business & Operational FAQs

Q: How will we know if M1 is successful?
A: Success means: the indexer runs, connects to an Ethereum WS endpoint, and we can query the PostgreSQL events table and see correctly decoded Transfer, Swap, and Stake events with accurate field values matching on-chain data.

Q: What could go wrong post-launch of M1?
A: The WebSocket connection could drop and without reconnection logic the indexer stops. This is a known limitation addressed in M2. Also, RPC providers may rate-limit or close idle connections.

Q: Does this require any infrastructure beyond what M0 provides?
A: It requires access to an Ethereum WebSocket RPC endpoint (e.g., Alchemy, Infura, or a self-hosted node). The PostgreSQL database is already provisioned in M0.

Q: Are there compliance or security considerations?
A: The RPC endpoint URL may contain API keys and must be kept in environment variables, never committed to source control. All data indexed is publicly available on-chain, so there are no data privacy concerns.

## Implementation Scope

### Current State -> End State

| Current State | End State |
| --- | --- |
| M0 foundation exists: workspace, config, DB schema, migrations, connection pool | End-to-end event indexing pipeline: WS subscription -> ABI decode -> DB write |
| No connection to any blockchain node | Live WebSocket connection to Ethereum, subscribed to contract logs |
| Empty events table in PostgreSQL | Events table populated with decoded Transfer, Swap, and Stake events in real-time |
| No event type definitions in code | Typed event structs via Alloy `sol!` macro for Transfer, Swap, and Stake |
| Developer cannot observe on-chain activity through ChainPulse | Developer runs ChainPulse and sees decoded events flowing into the database |

### User Stories

- [ ] As the system, I can connect to an Ethereum node via WebSocket and subscribe to `eth_subscribe("logs")` filtered by configured contract addresses (E1.1.1, E1.1.2)
- [ ] As the system, I can receive raw log events in real-time from the WebSocket subscription (E1.1.3)
- [ ] As the system, I can decode Transfer events from raw ABI-encoded logs into typed structs (E1.2.1)
- [ ] As the system, I can decode Swap events from Uniswap V2/V3 pair contracts (E1.2.2)
- [ ] As the system, I can decode Stake events from Aave stkAAVE (E1.2.3)
- [ ] As a developer, I can add a new event type by defining a `sol!` declaration and a mapping function (E1.2.4)
- [ ] As the system, I can write normalized events to PostgreSQL (E1.3.1)
- [ ] As the system, I can upsert events without duplicates using the composite key (chain_id, block_number, log_index) (E1.3.2)

### Success Criteria

- [ ] ChainPulse connects to an Ethereum WebSocket RPC and subscribes to logs for configured contracts
- [ ] Transfer events from USDC/USDT/WETH/DAI contracts are decoded with correct from, to, and value fields
- [ ] Swap events from Uniswap contracts are decoded with correct field values
- [ ] Stake events from stkAAVE are decoded with correct field values
- [ ] Decoded events appear as rows in the PostgreSQL events table with all fields populated
- [ ] Duplicate events (same chain_id, block_number, log_index) are handled via upsert without errors
- [ ] Unknown or malformed events are logged and skipped without crashing
- [ ] Structured logs show connection status, subscription state, and event processing activity

### Known Compromises

- [ ] No reconnection logic - if the WebSocket drops, the process exits (deferred to M2 for proper exponential backoff and catch-up)
- [ ] No batch writes - events are written one at a time, which is slower but simpler and correct (deferred to M4)
- [ ] No backpressure - if the DB is slow, events may queue unbounded in memory (deferred to M4 for bounded channels)
- [ ] No graceful shutdown - process termination may lose in-flight events (deferred to M8)
- [ ] Process exits on DB connection failure rather than retrying (acceptable for M1, improved in M2)

### Open Questions / Needs Research

- [ ] Which Alloy crate(s) and version(s) provide WebSocket subscription support (`eth_subscribe` for logs)?
- [ ] What is the exact Alloy API for subscribing to filtered logs via WebSocket (provider setup, filter construction, stream consumption)?
- [ ] How should the `sol!` macro declarations be structured for Transfer, Swap (V2 and V3 differ), and Stake events?
- [ ] What is the best sqlx pattern for upsert (ON CONFLICT DO NOTHING vs ON CONFLICT DO UPDATE) given the M0 schema?
- [ ] Should the subscriber task and DB writer task communicate via a channel even in M1, or can M1 use direct inline writes within the subscriber loop?

---

## Next Steps

When ready to implement:

1. Run `/pipe-research --project chainpulse --ref m1 Single-Chain Indexer` to analyze codebase and resolve open questions
2. Run `/pipe-plan-implementation --project chainpulse --ref m1 Single-Chain Indexer` to create implementation plan
3. Run `/pipe-implement docs/dev/chainpulse/m1-single-chain-indexer/implementation-plan.md` to execute the plan

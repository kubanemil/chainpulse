# ChainPulse: Project Plan

## Vision & Goals

**Problem**: Protocol analytics teams need reliable, low-latency indexing of on-chain events across multiple EVM chains. Existing Node.js indexers crash under load and produce data gaps.

**Solution**: ChainPulse — a multi-chain on-chain event indexer written in async Rust that subscribes to contract event logs in real-time via WebSocket, normalizes them into typed structs, stores them in PostgreSQL, and exposes them via REST + WebSocket API.

**Goals**:

- Index events from Ethereum and Polygon simultaneously in real-time
- Zero data gaps — automatic catch-up on reconnection
- Handle thousands of events/minute at peak without OOM or crashes
- Expose indexed data via REST API and live WebSocket streaming
- Extensible event type system — easy to add new event definitions

**Non-Goals**:

- Historical full-chain backfill (only catch-up from last indexed block)
- Frontend / dashboard UI
- Support for non-EVM chains
- Transaction tracing or state reconstruction

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          ChainPulse System                              │
│                                                                         │
│  ┌───────────────┐   ┌───────────────┐                                  │
│  │  Ethereum WS  │   │  Polygon WS   │   ... (extensible per chain)     │
│  │  Subscriber   │   │  Subscriber   │                                  │
│  └──────┬────────┘   └──────┬────────┘                                  │
│         │                   │                                           │
│         │  eth_subscribe    │  eth_subscribe                            │
│         │  (logs)           │  (logs)                                   │
│         ▼                   ▼                                           │
│  ┌─────────────────────────────────────┐                                │
│  │       Normalization Layer           │                                │
│  │  (ABI decode → typed Rust structs)  │                                │
│  │  Transfer | Swap | Stake | ...      │                                │
│  └──────────────┬──────────────────────┘                                │
│                 │                                                        │
│                 │  bounded Tokio channel                                 │
│                 │  (backpressure)                                        │
│                 ▼                                                        │
│  ┌─────────────────────────┐    ┌──────────────────────────────┐        │
│  │     DB Writer Task      │    │       API Server (axum)      │        │
│  │  (batch upsert to PG)   │    │                              │        │
│  │                         │    │  GET /events?chain=&type=    │        │
│  │  Dedup via composite    │    │  GET /events/:id             │        │
│  │  key (chain_id,         │    │  GET /health                 │        │
│  │   block_number, log_idx)│    │  WS  /ws/events (streaming)  │        │
│  └───────────┬─────────────┘    └──────────────┬───────────────┘        │
│              │                                  │                        │
│              ▼                                  │                        │
│  ┌─────────────────────────┐                    │                        │
│  │      PostgreSQL         │◄───────────────────┘                        │
│  │                         │                                             │
│  │  events table           │                                             │
│  │  indexed_blocks table   │                                             │
│  │  contracts table        │                                             │
│  └─────────────────────────┘                                             │
│                                                                          │
│  ┌─────────────────────────────────────────┐                             │
│  │          Observability                  │                             │
│  │  tracing (structured logs)              │                             │
│  │  metrics + Prometheus /metrics endpoint │                             │
│  │  /health endpoint                       │                             │
│  └─────────────────────────────────────────┘                             │
└──────────────────────────────────────────────────────────────────────────┘
```

### Key Architecture Decisions

- **One Tokio task per chain** — each chain subscriber runs independently, can reconnect without affecting others
- **Bounded channel for backpressure** — if DB writer falls behind, subscribers slow down rather than buffering unbounded in memory
- **Composite key deduplication** — `(chain_id, block_number, log_index)` ensures idempotent writes on reconnect catch-up
- **Batch writes** — DB writer accumulates events and flushes in batches for throughput
- **Shared API server** — single axum server serves both REST queries and WebSocket streaming

---

## Technical Decisions

### Decided

| ID  | Decision | Choice | Rationale |
|-----|----------|--------|-----------|
| TD1 | Language & runtime | Rust + Tokio | Performance, safety, async ecosystem |
| TD2 | EVM RPC client | Alloy (`alloy-rs`) | Successor to ethers-rs, actively maintained, used by Reth |
| TD3 | ABI decoding | Alloy `sol!` macro | Compile-time typed event structs, zero runtime overhead |
| TD4 | Database | PostgreSQL | Relational, strong indexing, battle-tested for event stores |
| TD5 | Postgres driver | `sqlx` | Async-native, compile-time checked queries, built-in migrations |
| TD6 | API framework | `axum` | Tokio-native, Tower middleware, WebSocket support built-in |
| TD7 | Observability - logging | `tracing` + `tracing-subscriber` | De facto Rust standard, structured + span-based |
| TD8 | Observability - metrics | `metrics` + `metrics-exporter-prometheus` | Simple, Prometheus-native |
| TD9 | Configuration | `dotenvy` + `envy` | 12-factor env vars, typed deserialization |
| TD10 | Contract scope | Configurable with defaults | Well-known contracts ship as defaults, new ones addable via config |
| TD11 | API scope | REST + WebSocket streaming | REST for queries, WS for live event feed |

---

## Default Contracts

### Ethereum Mainnet

| Protocol | Contract | Address | Events |
|----------|----------|---------|--------|
| USDC | ERC-20 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | Transfer |
| USDT | ERC-20 | `0xdAC17F958D2ee523a2206206994597C13D831ec7` | Transfer |
| WETH | ERC-20 | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` | Transfer |
| DAI | ERC-20 | `0x6B175474E89094C44Da98b954EedeAC495271d0F` | Transfer |
| Uniswap V2 Factory | Factory | `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f` | PairCreated |
| Uniswap V3 Factory | Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` | PoolCreated |
| Aave V3 Pool | Lending | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` | Supply, Withdraw |
| stkAAVE | Staking | `0x4da27a545c0c5B758a6BA100e3a049001de870f5` | Staked |

### Polygon PoS

| Protocol | Contract | Address | Events |
|----------|----------|---------|--------|
| USDC (bridged) | ERC-20 | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` | Transfer |
| USDC (native) | ERC-20 | `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` | Transfer |
| USDT | ERC-20 | `0xc2132D05D31c914a87C6611C10748AEb04B58e8F` | Transfer |
| WMATIC | ERC-20 | `0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270` | Transfer |
| WETH | ERC-20 | `0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619` | Transfer |
| QuickSwap Factory | Factory | `0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32` | PairCreated |
| Aave V3 Pool | Lending | `0x794a61358D6845594F94dc1DB02A252b5b4814aD` | Supply, Withdraw |

---

## Milestones

| ID | Milestone | Description | Dependencies |
|----|-----------|-------------|--------------|
| M0 | Project Foundation | Cargo workspace, config, DB schema, CI | — |
| M1 | Single-Chain Indexer | WebSocket subscriber + ABI decoding + DB writes for Ethereum | M0 |
| M2 | Resilience | Exponential backoff reconnection, catch-up from last block, dedup | M1 |
| M3 | Multi-Chain | Add Polygon subscriber, per-chain task management | M2 |
| M4 | Backpressure & Performance | Bounded channels, batch DB writes, throughput tuning | M3 |
| M5 | REST API | Query endpoints for indexed events with filtering/pagination | M4 |
| M6 | WebSocket Streaming | Live event feed via WS for consuming services | M5 |
| M7 | Observability | Structured logging, Prometheus metrics, health endpoint | M4 |
| M8 | Hardening | Graceful shutdown, error handling audit, integration tests | M7 |

---

## Epics & User Stories

### M0: Project Foundation

#### E0.1: Project Setup

| ID | Story | Size |
|----|-------|------|
| E0.1.1 | As a developer, I can clone the repo and `cargo build` successfully | S |
| E0.1.2 | As a developer, I can run `sqlx migrate run` to set up the database schema | S |
| E0.1.3 | As a developer, I can configure chain RPCs, DB URL, and contract addresses via `.env` | S |

#### E0.2: Database Schema

| ID | Story | Size |
|----|-------|------|
| E0.2.1 | As the system, I can store normalized events with chain_id, block_number, log_index, tx_hash, event_type, and decoded payload | M |
| E0.2.2 | As the system, I can track the last indexed block per chain for catch-up | S |
| E0.2.3 | As the system, I can store tracked contract addresses per chain | S |

---

### M1: Single-Chain Indexer

#### E1.1: WebSocket Subscriber

| ID | Story | Size |
|----|-------|------|
| E1.1.1 | As the system, I can connect to an Ethereum node via WebSocket and subscribe to `eth_subscribe("logs")` | M |
| E1.1.2 | As the system, I can filter subscriptions by contract addresses | S |
| E1.1.3 | As the system, I can receive raw log events in real-time | M |

#### E1.2: Event Normalization

| ID | Story | Size |
|----|-------|------|
| E1.2.1 | As the system, I can decode Transfer events from raw ABI-encoded logs into typed structs | M |
| E1.2.2 | As the system, I can decode Swap events from Uniswap V2/V3 pair contracts | M |
| E1.2.3 | As the system, I can decode Stake events from Aave stkAAVE | M |
| E1.2.4 | As a developer, I can add a new event type by defining a `sol!` declaration and a mapping function | M |

#### E1.3: Database Writer

| ID | Story | Size |
|----|-------|------|
| E1.3.1 | As the system, I can write normalized events to PostgreSQL | M |
| E1.3.2 | As the system, I can upsert events without duplicates using the composite key | M |

---

### M2: Resilience

#### E2.1: Reconnection

| ID | Story | Size |
|----|-------|------|
| E2.1.1 | As the system, I can detect WebSocket disconnection and reconnect with exponential backoff | M |
| E2.1.2 | As the system, I can query the last indexed block on reconnect and request missed logs via `eth_getLogs` | L |
| E2.1.3 | As the system, I can process catch-up logs without creating duplicate entries | M |

---

### M3: Multi-Chain

#### E3.1: Parallel Chain Subscribers

| ID | Story | Size |
|----|-------|------|
| E3.1.1 | As the system, I can run Ethereum and Polygon subscribers as independent Tokio tasks | M |
| E3.1.2 | As the system, I can tag all events with chain_id for disambiguation | S |
| E3.1.3 | As a developer, I can add a new chain by adding an RPC URL and contract list to config | M |

---

### M4: Backpressure & Performance

#### E4.1: Bounded Channel

| ID | Story | Size |
|----|-------|------|
| E4.1.1 | As the system, I use a bounded Tokio channel between subscribers and the DB writer | M |
| E4.1.2 | As the system, I slow down event ingestion when the DB writer is behind (backpressure) | M |

#### E4.2: Batch Writes

| ID | Story | Size |
|----|-------|------|
| E4.2.1 | As the system, I batch multiple events into a single DB transaction for throughput | M |
| E4.2.2 | As the system, I can sustain thousands of events per minute at peak | L |

---

### M5: REST API

#### E5.1: Query Endpoints

| ID | Story | Size |
|----|-------|------|
| E5.1.1 | As an API consumer, I can query events filtered by chain, event type, contract address, and block range | L |
| E5.1.2 | As an API consumer, I can paginate through large result sets | M |
| E5.1.3 | As an API consumer, I can get a single event by ID | S |

---

### M6: WebSocket Streaming

#### E6.1: Live Event Feed

| ID | Story | Size |
|----|-------|------|
| E6.1.1 | As an API consumer, I can connect to a WebSocket endpoint and receive new events in real-time | L |
| E6.1.2 | As an API consumer, I can filter the live stream by chain and event type | M |

---

### M7: Observability

#### E7.1: Logging & Metrics

| ID | Story | Size |
|----|-------|------|
| E7.1.1 | As an operator, I can see structured JSON logs with chain, block number, and event type context | M |
| E7.1.2 | As an operator, I can scrape Prometheus metrics (events/sec, DB write latency, channel utilization, WS connection status) | L |
| E7.1.3 | As an operator, I can hit a `/health` endpoint to check system liveness and per-chain subscriber status | M |

---

### M8: Hardening

#### E8.1: Production Readiness

| ID | Story | Size |
|----|-------|------|
| E8.1.1 | As the system, I can shut down gracefully — drain channels, flush pending writes, close DB connections | M |
| E8.1.2 | As a developer, I can run integration tests against a local PostgreSQL and mock RPC | L |
| E8.1.3 | As a developer, I have confidence in error handling — all error paths are logged and non-panicking | M |

---

## Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Third-party RPC rate limits / throttling | Missed events, slow catch-up | High | Use provider with generous WS limits; implement backoff; track rate limit headers |
| WebSocket instability on public RPC endpoints | Frequent reconnections | Medium | Exponential backoff + catch-up logic (M2); consider paid tier providers |
| Alloy breaking changes (pre-1.0) | Build failures after dependency updates | Medium | Pin versions in Cargo.lock; update deliberately |
| High-activity periods overwhelming DB writes | Backpressure stalls indexing | Medium | Batch writes (M4), connection pooling, DB indexing tuning |
| ABI changes in tracked contracts (upgradeable proxies) | Failed event decoding | Low | Log decode failures as warnings; skip unknown events gracefully |

---

## Tech Stack Summary

| Layer | Technology |
|-------|-----------|
| Language | Rust (2021 edition) |
| Async runtime | Tokio |
| EVM RPC + ABI | Alloy (`alloy-rs`) |
| Database | PostgreSQL + `sqlx` |
| API server | `axum` |
| Logging | `tracing` + `tracing-subscriber` |
| Metrics | `metrics` + `metrics-exporter-prometheus` |
| Config | `dotenvy` + `envy` |
| Serialization | `serde` + `serde_json` |

---

## Next Steps

The project plan is complete. To start implementation:

1. **Start with M0** (Project Foundation) — Cargo workspace, config, DB schema
2. **For each milestone/epic**, use the workflow:
   - `/pipe-research` — research implementation patterns
   - `/pipe-plan-implementation` — create detailed implementation plan
   - `/pipe-implement` — implement the code
3. **Recommended first command**: `/pipe-plan-implementation --project chainpulse M0: Project Foundation`

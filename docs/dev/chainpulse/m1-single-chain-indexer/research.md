---
date: 2026-03-25
researcher: Claude Opus 4.6 (1M context)
branch: main
repository: chainpulse
tags: alloy, websocket, eth_subscribe, abi-decoding, sol-macro, event-indexing, postgresql
status: complete
---

# Research: M1 Single-Chain Indexer

## Research Question

What are the exact Alloy crate APIs, event signatures, and codebase patterns needed to implement the M1 Single-Chain Indexer — WebSocket subscription to Ethereum logs, ABI decoding of Transfer/Swap/Supply/Withdraw/Staked/PairCreated/PoolCreated events via the `sol!` macro, and database writes with upsert deduplication?

## Summary

The Alloy ecosystem (v1.0.x, currently 1.0.41 on crates.io) provides all capabilities needed for M1 through a single meta-crate with feature flags. The `alloy` crate with features `["provider-ws", "pubsub"]` gives WebSocket provider construction via `ProviderBuilder::new().connect_ws(WsConnect::new(url))`, log subscription via `provider.subscribe_logs(&filter)` which returns an async stream, and compile-time event type generation via the `sol!` macro. Raw logs from the subscription stream can be decoded into typed Rust structs using the `SolEvent::decode_log_data(&log.inner.data)` method, which returns a `Result` with the decoded event or an error for non-matching topics.

The existing M0 codebase provides a clean foundation: `chainpulse-config` already loads `eth_rpc_ws_url` from environment, `chainpulse-db` has `PgPool` creation with migrations, and the `events` table schema with its `UNIQUE(chain_id, block_number, log_index)` constraint supports `ON CONFLICT DO NOTHING` upsert semantics. The `chainpulse-types` crate (currently empty) is the natural home for `sol!`-generated event types and normalization logic. The subscriber, decoder, and writer can be implemented as a single Tokio task in `main.rs` for M1 simplicity, with channel-based separation deferred to M4.

## Detailed Findings

### 1. Alloy Crate Dependencies

The recommended dependency for M1 is the `alloy` meta-crate with specific feature flags. Using the meta-crate avoids managing individual sub-crate versions.

**Required Cargo.toml addition (workspace level):**

```toml
alloy = { version = "1.0", features = ["provider-ws", "pubsub", "rpc-types"] }
futures-util = "0.3"  # for StreamExt on subscription streams
```

Feature breakdown:
- `provider-ws` — enables `WsConnect` and `ProviderBuilder::connect_ws()`, includes `alloy-transport-ws`
- `pubsub` — enables `subscribe_logs` and other `eth_subscribe` methods via `alloy-pubsub`
- `rpc-types` — provides `Filter`, `Log`, and other RPC type definitions

The `sol!` macro and `SolEvent` trait come from `alloy-sol-types`, which is included by default in the `alloy` meta-crate (part of `alloy-core`). No additional feature flag is needed for ABI decoding.

`futures-util` is needed for `.into_stream()` and `StreamExt::next()` on the subscription object.

### 2. WebSocket Provider Setup

The Alloy API for creating a WebSocket provider and subscribing to logs:

```rust
use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::{BlockNumberOrTag, Filter},
};
use futures_util::stream::StreamExt;

// Create WS provider
let ws = WsConnect::new(&config.eth_rpc_ws_url);
let provider = ProviderBuilder::new().connect_ws(ws).await?;

// Build filter for multiple contract addresses
let addresses: Vec<Address> = vec![
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
    address!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
    // ... more addresses
];

let filter = Filter::new()
    .address(addresses)
    .from_block(BlockNumberOrTag::Latest);

// Subscribe — returns a Subscription<Log>
let sub = provider.subscribe_logs(&filter).await?;
let mut stream = sub.into_stream();

// Consume the stream
while let Some(log) = stream.next().await {
    // log is alloy::rpc::types::Log
    // Attempt to decode into known event types
}
```

Key observations:
- `Filter::new().address(vec)` accepts a `Vec<Address>` for multiple contract addresses in a single subscription
- NOT specifying `.event()` on the filter means ALL events from those contracts are received (we filter by topic0 during decoding)
- `subscribe_logs` requires a pubsub-capable transport (WebSocket or IPC)
- The stream yields `alloy::rpc::types::Log` objects containing `address`, `topics`, `data`, `block_number`, `transaction_hash`, `log_index`, etc.

### 3. Event Type Definitions via sol! Macro

The `sol!` macro generates Rust structs that implement the `SolEvent` trait. Each event type gets a generated struct with typed fields and a `SIGNATURE_HASH` constant (the Keccak-256 of the event signature, i.e., topic0).

**ERC-20 Transfer:**

```rust
use alloy::sol;

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
}
```

This generates a `Transfer` struct with fields `from: Address`, `to: Address`, `value: U256`, plus the `SolEvent` trait implementation with `Transfer::SIGNATURE_HASH` equal to `keccak256("Transfer(address,address,uint256)")`.

**Uniswap V2 Swap:**

```rust
sol! {
    #[derive(Debug)]
    event Swap(
        address indexed sender,
        uint256 amount0In,
        uint256 amount1In,
        uint256 amount0Out,
        uint256 amount1Out,
        address indexed to
    );
}
```

**Uniswap V3 Swap:**

V3 Swap has a different signature from V2 (different topic0 hash), so it must be defined separately. Using a module to namespace:

```rust
sol! {
    #[derive(Debug)]
    event SwapV3(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );
}
```

Note: The event name in `sol!` determines the generated struct name but does NOT affect the ABI signature hash. The ABI signature is derived from the canonical form. Since the on-chain event is still called `Swap`, we need to use `Swap` as the event name inside `sol!` for the topic0 to match. To distinguish V2 and V3, we can use Rust modules:

```rust
mod uniswap_v2 {
    use alloy::sol;
    sol! {
        #[derive(Debug)]
        event Swap(
            address indexed sender,
            uint256 amount0In,
            uint256 amount1In,
            uint256 amount0Out,
            uint256 amount1Out,
            address indexed to
        );
    }
}

mod uniswap_v3 {
    use alloy::sol;
    sol! {
        #[derive(Debug)]
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );
    }
}
```

This gives `uniswap_v2::Swap::SIGNATURE_HASH` and `uniswap_v3::Swap::SIGNATURE_HASH` with the correct distinct topic0 values.

**Aave V3 Supply:**

```rust
sol! {
    #[derive(Debug)]
    event Supply(
        address indexed reserve,
        address user,
        address indexed onBehalfOf,
        uint256 amount,
        uint16 indexed referralCode
    );
}
```

**Aave V3 Withdraw:**

```rust
sol! {
    #[derive(Debug)]
    event Withdraw(
        address indexed reserve,
        address indexed user,
        address indexed to,
        uint256 amount
    );
}
```

**stkAAVE Staked:**

```rust
sol! {
    #[derive(Debug)]
    event Staked(
        address indexed from,
        address indexed onBehalfOf,
        uint256 amount
    );
}
```

**Uniswap V2 PairCreated:**

```rust
sol! {
    #[derive(Debug)]
    event PairCreated(
        address indexed token0,
        address indexed token1,
        address pair,
        uint256
    );
}
```

**Uniswap V3 PoolCreated:**

```rust
sol! {
    #[derive(Debug)]
    event PoolCreated(
        address indexed token0,
        address indexed token1,
        uint24 indexed fee,
        int24 tickSpacing,
        address pool
    );
}
```

### 4. Decoding Raw Logs into Typed Events

The `SolEvent` trait provides several decode methods. The most relevant for this use case:

```rust
// SolEvent trait methods (from alloy-sol-types):
fn decode_log_data(log: &LogData) -> Result<Self>
fn decode_raw_log<I, D>(topics: I, data: &[u8]) -> Result<Self>
fn decode_log(log: &Log) -> Result<Log<Self>>
```

The subscription stream yields `alloy::rpc::types::Log` objects. To decode:

```rust
use alloy::sol_types::SolEvent;

// log: alloy::rpc::types::Log (from the subscription stream)
// log.topic0() returns Option<&B256> — the event signature hash
// log.inner.data is LogData containing topics and data bytes

// Pattern: match on topic0, then decode
fn decode_event(log: &alloy::rpc::types::Log) -> Option<NormalizedEvent> {
    let topic0 = log.topic0()?;

    if *topic0 == Transfer::SIGNATURE_HASH {
        let decoded = Transfer::decode_log_data(&log.inner.data).ok()?;
        return Some(NormalizedEvent {
            event_type: "Transfer".to_string(),
            payload: serde_json::json!({
                "from": decoded.from.to_string(),
                "to": decoded.to.to_string(),
                "value": decoded.value.to_string(),
            }),
        });
    }

    if *topic0 == uniswap_v2::Swap::SIGNATURE_HASH {
        let decoded = uniswap_v2::Swap::decode_log_data(&log.inner.data).ok()?;
        // ... normalize
    }

    // ... more event types

    None // unknown event
}
```

Key observations:
- `log.topic0()` returns `Option<&B256>` — the first topic (event signature hash)
- `SIGNATURE_HASH` is a `const B256` on each generated event type
- `decode_log_data` takes `&LogData` and returns `Result<Self, alloy_sol_types::Error>`
- On decode failure (wrong event, malformed data), it returns `Err` — we use `.ok()?` to skip
- The decoded struct fields use Alloy primitive types: `Address`, `U256`, `I256`, etc.
- These must be converted to strings or serialized for the JSONB `payload` column

### 5. Database Writer Pattern

The existing M0 schema and code patterns guide the writer implementation.

**Events table schema** (`chainpulse-db/migrations/001_initial_schema.sql:1-14`):
- `chain_id INTEGER NOT NULL` — always 1 for Ethereum mainnet in M1
- `block_number BIGINT NOT NULL` — from `log.block_number`
- `log_index INTEGER NOT NULL` — from `log.log_index`
- `tx_hash VARCHAR(66) NOT NULL` — from `log.transaction_hash`
- `contract_address VARCHAR(42) NOT NULL` — from `log.address()`
- `event_type VARCHAR(50) NOT NULL` — the decoded event type name
- `payload JSONB NOT NULL` — event-specific decoded fields
- `block_timestamp TIMESTAMPTZ` — nullable, may not be available from log subscription (logs don't include block timestamp directly)
- `UNIQUE(chain_id, block_number, log_index)` — deduplication key

**Upsert pattern using sqlx:**

```rust
pub async fn insert_event(
    pool: &PgPool,
    chain_id: i32,
    block_number: i64,
    log_index: i32,
    tx_hash: &str,
    contract_address: &str,
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO events (chain_id, block_number, log_index, tx_hash, contract_address, event_type, payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (chain_id, block_number, log_index) DO NOTHING"
    )
    .bind(chain_id)
    .bind(block_number)
    .bind(log_index)
    .bind(tx_hash)
    .bind(contract_address)
    .bind(event_type)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}
```

`ON CONFLICT DO NOTHING` is preferred over `DO UPDATE` for M1 because:
- Events are immutable — if the same (chain_id, block_number, log_index) exists, the data is identical
- `DO NOTHING` is cheaper (no write amplification)
- Aligns with the press release decision: "the duplicate is silently ignored"

The existing `upsert_last_indexed_block` function at `chainpulse-db/src/lib.rs:36-51` uses `ON CONFLICT DO UPDATE`, which is the correct pattern for that table since the `last_block` value changes.

**Block timestamp**: The `Log` object from `subscribe_logs` does not directly include the block timestamp. Options for M1:
1. Leave `block_timestamp` as NULL (simplest, acceptable for M1)
2. Fetch block by number to get timestamp (adds RPC calls, complexity)

Recommendation: leave NULL for M1, populate via backfill or enrichment in a later milestone.

### 6. Subscriber-to-Writer Data Flow for M1

The press release FAQ asks: "Should the subscriber task and DB writer task communicate via a channel even in M1, or can M1 use direct inline writes within the subscriber loop?"

Based on the codebase patterns and M1 scope:

**Recommended M1 approach: direct inline writes (no channel)**

```rust
// In main.rs, after pool and provider are ready:
let mut stream = sub.into_stream();

while let Some(log) = stream.next().await {
    if let Some(event) = decode_event(&log) {
        if let Err(e) = chainpulse_db::insert_event(
            &pool,
            1, // Ethereum mainnet chain_id
            log.block_number.unwrap_or_default() as i64,
            log.log_index.unwrap_or_default() as i32,
            &format!("{:?}", log.transaction_hash.unwrap_or_default()),
            &format!("{:?}", log.address()),
            &event.event_type,
            &event.payload,
        ).await {
            tracing::error!(error = %e, "Failed to write event to database");
            std::process::exit(1); // M1: exit on DB error
        }
        tracing::info!(
            event_type = %event.event_type,
            block = ?log.block_number,
            "Event indexed"
        );
    } else {
        tracing::debug!(topic0 = ?log.topic0(), "Unknown event, skipping");
    }
}

// Stream ended — WS disconnected
tracing::error!("WebSocket subscription ended");
std::process::exit(1); // M1: exit on disconnect
```

Rationale:
- M1 has no backpressure requirements (deferred to M4)
- M1 has no batch writes (deferred to M4)
- Direct inline writes are simpler and correct
- Channel separation adds complexity with no M1 benefit
- The single-task pattern matches the existing `main.rs` sequential startup style

### 7. Existing Codebase Patterns to Follow

**Error handling** (`chainpulse/src/main.rs:5-11, 27-33, 35-38`):
- Pattern: `match` on `Result`, log error with `tracing::error!`, call `std::process::exit(1)`
- M1 should follow this same pattern for WS connection failure and DB write failure

**Config access** (`chainpulse-config/src/lib.rs:37`):
- `config.eth_rpc_ws_url` is already defined as `pub eth_rpc_ws_url: String`
- No config changes needed for the WS URL

**DB module** (`chainpulse-db/src/lib.rs`):
- Functions are standalone `pub async fn` taking `&PgPool` as first arg
- Error type is `DbError` with `#[from] sqlx::Error`
- New `insert_event` function should follow the same pattern

**Types crate** (`chainpulse-types/src/lib.rs`):
- Currently empty — this is where `sol!` event definitions and the `NormalizedEvent` struct should live
- The `chainpulse-types` Cargo.toml already has `serde` and `serde_json` dependencies

**Workspace dependencies** (`Cargo.toml:10-20`):
- New dependencies (`alloy`, `futures-util`) should be added to `[workspace.dependencies]`
- Crate-level `Cargo.toml` files reference them via `{ workspace = true }`

### 8. Crate Modification Plan

Based on the research, the following crate changes are needed:

| Crate | Changes |
|-------|---------|
| Root `Cargo.toml` | Add `alloy` and `futures-util` to `[workspace.dependencies]` |
| `chainpulse-types` | Add `alloy` dep; define `sol!` event types; define `NormalizedEvent` struct; implement `decode_event` function |
| `chainpulse-db` | Add `insert_event` function; add `serde_json` dep for JSONB payload |
| `chainpulse` (binary) | Add `alloy`, `futures-util`, `chainpulse-types` deps; implement WS connection, subscription, and main event loop |
| `chainpulse-config` | No changes needed — `eth_rpc_ws_url` already exists |

## Code References

- `Cargo.toml:10-20` — workspace dependency definitions (add `alloy`, `futures-util` here)
- `chainpulse-config/src/lib.rs:37` — `eth_rpc_ws_url: String` field already defined
- `chainpulse-db/src/lib.rs:1-19` — `PgPool`, `DbError`, `create_pool` pattern to follow
- `chainpulse-db/src/lib.rs:36-51` — `upsert_last_indexed_block` as upsert pattern reference
- `chainpulse-db/migrations/001_initial_schema.sql:1-14` — `events` table with `UNIQUE(chain_id, block_number, log_index)`
- `chainpulse-types/src/lib.rs` — empty, target for `sol!` event definitions
- `chainpulse-types/Cargo.toml:7-9` — already has `serde`, `serde_json`, `chrono`
- `chainpulse/src/main.rs:4-41` — startup sequence, error handling pattern, tracing setup
- `chainpulse/Cargo.toml:10-15` — binary crate deps (add `alloy`, `futures-util`, `chainpulse-types`)

## Architecture Notes

The M1 data flow is linear and single-threaded:

```
Ethereum WS RPC
      │
      │  eth_subscribe("logs", {address: [contracts...]})
      ▼
  WS Provider (alloy)
      │
      │  Stream<Log>
      ▼
  Event Decoder (chainpulse-types)
      │  match topic0 → sol! decode_log_data
      │
      ▼
  NormalizedEvent { event_type, payload: serde_json::Value }
      │
      │  insert_event (chainpulse-db)
      ▼
  PostgreSQL events table
      (ON CONFLICT DO NOTHING)
```

All processing happens in a single `tokio::main` async task. The subscriber loop is blocking on `stream.next().await`, processes one event at a time, writes to DB, then proceeds to the next event. This is simple and correct for M1.

The `chainpulse-types` crate is the natural boundary between the Alloy/blockchain domain and the database domain. It takes raw `alloy::rpc::types::Log` and produces a `NormalizedEvent` with string-based fields suitable for sqlx binding.

## Open Questions

1. **Block timestamp population**: The `events.block_timestamp` column exists but `subscribe_logs` does not provide it. Should M1 leave it NULL, or add an `eth_getBlockByNumber` call per unique block? Recommendation: leave NULL for M1 to keep the implementation simple.

2. **Contract address list source**: The project plan lists default contracts in a table. For M1, should these be hardcoded in the binary, loaded from the `tracked_contracts` DB table, or added to the config? Recommendation: hardcode as constants in `chainpulse-types` for M1 simplicity; config-driven loading can come later.

3. **Alloy version pinning**: The current stable is 1.0.41. The project plan notes "Alloy breaking changes (pre-1.0)" as a risk, but Alloy is now post-1.0 stable. Pin to `"1.0"` in Cargo.toml (SemVer compatible range) with `Cargo.lock` for reproducibility.

4. **V2 vs V3 Swap disambiguation**: Since V2 and V3 Swap events have different signatures (different parameters = different topic0), they are naturally distinguished during decoding. However, if a contract address could emit both (unlikely in practice), the topic0 match handles it correctly.

---

## Next Steps

If you're satisfied with this research, proceed to planning:

/pipe-plan-implementation --project chainpulse --ref m1 --research docs/dev/chainpulse/m1-single-chain-indexer/research.md Single-Chain Indexer

If you need more research:

- Ask follow-up questions and I'll append to this document
- Or request a new research topic with /pipe-research --project chainpulse --ref m1 [new-topic]

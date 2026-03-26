---
date: 2026-03-26
researcher: claude-opus-4-6
branch: N/A (not a git repo)
repository: chainpulse
tags: resilience, reconnection, catch-up, dedup, eth_getLogs, backoff, alloy
status: complete
---

# Research: M2 Resilience (Reconnection, Catch-Up, Dedup)

## Research Question

How should ChainPulse implement automatic WebSocket reconnection with exponential backoff, historical log catch-up via `eth_getLogs`, and deduplication to ensure zero data gaps after connection drops?

## Summary

The current M1 codebase in `chainpulse/src/main.rs` uses a linear flow: connect WS, subscribe to logs, process events in a `while let Some(log)` loop, and `exit(1)` when the stream ends. There is no reconnection, no catch-up, and `last_indexed_block` is never written during normal operation. The database layer (`chainpulse-db`) already provides `get_last_indexed_block`, `upsert_last_indexed_block`, and `insert_event` with `ON CONFLICT DO NOTHING` — all the primitives needed for M2.

Alloy v1 (the version used by this project) does not provide built-in WebSocket reconnection. The `ProviderBuilder::new().connect_ws(ws)` call creates a one-shot connection; when it drops, the provider is dead. Reconnection must be implemented manually by wrapping the entire connect-subscribe-process cycle in an outer loop with backoff. For historical log fetching, Alloy's `Provider` trait exposes `get_logs(&filter)` which maps to `eth_getLogs` and accepts `Filter` with `.from_block()` and `.to_block()` for block-range queries. The existing `Filter` construction in main.rs already demonstrates this pattern — it just needs `from_block`/`to_block` set to the catch-up range instead of `Latest`.

For exponential backoff, `tokio::time::sleep` with manual doubling (1s initial, 2x multiplier, 60s cap) is the simplest approach and avoids adding a new dependency. The `backoff` crate is an alternative but adds complexity for a pattern that is straightforward to implement manually.

## Detailed Findings

### 1. Current main.rs Event Loop Structure

**File**: `chainpulse/src/main.rs:1-113`

The current flow is strictly sequential and non-resilient:

1. **Lines 9-15**: Load config; `exit(1)` on failure.
2. **Lines 31-37**: Create DB pool; `exit(1)` on failure.
3. **Lines 39-42**: Run migrations; `exit(1)` on failure.
4. **Lines 47-54**: Create WS connection and provider; `exit(1)` on failure.
5. **Lines 59-64**: Build `Filter` with `from_block(BlockNumberOrTag::Latest)` and contract addresses.
6. **Lines 66-72**: Subscribe to logs; `exit(1)` on failure.
7. **Lines 79-109**: `while let Some(log) = stream.next().await` — process each log through `decode_log` then `insert_event`. On DB write error: `exit(1)`.
8. **Lines 111-112**: When stream ends (WS disconnect), log error and `exit(1)`.

Key observations:
- `last_indexed_block` is **never read or written** during the event loop. The DB functions exist but are unused.
- The `Filter` uses `BlockNumberOrTag::Latest`, meaning the subscription starts from the current head — no historical awareness.
- DB write errors cause immediate process exit (line 103).
- The stream ending (line 111) is treated as a fatal error.

### 2. Alloy WS Provider Reconnection Patterns

**File**: `Cargo.toml:21` — `alloy = { version = "1", features = ["provider-ws", "pubsub", "rpc-types"] }`

Alloy v1's WS transport does **not** include built-in reconnection logic. The relevant types are:

- `alloy::providers::WsConnect` — a connection configuration struct, not a persistent connection manager.
- `ProviderBuilder::new().connect_ws(ws)` — returns `Result<impl Provider>`. This is a one-shot connection.
- When the WS connection drops, `stream.next().await` returns `None`, ending the `while let` loop.

There is no `ReconnectingProvider` or automatic retry in Alloy. The standard pattern in the Alloy ecosystem is to implement reconnection at the application level:

```
loop {
    let provider = connect_ws().await;  // with backoff on failure
    let sub = provider.subscribe_logs(&filter).await;
    let mut stream = sub.into_stream();
    while let Some(log) = stream.next().await {
        // process
    }
    // stream ended — reconnect
}
```

This means the entire provider + subscription must be recreated on each reconnect, not just the stream.

### 3. eth_getLogs for Catch-Up

Alloy's `Provider` trait includes `get_logs(&self, filter: &Filter) -> Result<Vec<Log>>`. This maps directly to the JSON-RPC `eth_getLogs` method.

The existing code already constructs a `Filter` at `main.rs:62-64`:

```rust
let filter = Filter::new()
    .address(contracts)
    .from_block(BlockNumberOrTag::Latest);
```

For catch-up, the same `Filter` builder supports:
- `.from_block(BlockNumberOrTag::Number(start))` — set the start block
- `.to_block(BlockNumberOrTag::Number(end))` — set the end block

To get the current chain head, use `provider.get_block_number().await` which returns `Result<u64>`.

The catch-up flow is:
1. Read `last_indexed_block` from DB via `chainpulse_db::get_last_indexed_block(&pool, chain_id)`.
2. Get current head via `provider.get_block_number().await`.
3. Build a `Filter` with `.from_block(last_indexed + 1)` and `.to_block(head)`.
4. Call `provider.get_logs(&filter).await` to get `Vec<Log>`.
5. Process each log through `decode_log` + `insert_event` (same pipeline as live events).
6. Update `last_indexed_block` to `head`.

### 4. Chunked eth_getLogs — Provider Limits

RPC providers impose limits on `eth_getLogs`:

| Provider | Block Range Limit | Response Size Limit |
|----------|------------------|-------------------|
| Alchemy | 2,000 blocks (free), up to 100K (growth+) | 10,000 logs |
| Infura | 100,000 blocks | 10,000 logs |
| Public endpoints | Varies, often 1,000-5,000 blocks | Varies |
| Ankr | 3,000 blocks | 10,000 logs |

A safe default chunk size is **2,000 blocks** per request, which works with all major providers. The chunking algorithm:

```
let chunk_size = 2000;
let mut from = last_indexed + 1;
while from <= head {
    let to = std::cmp::min(from + chunk_size - 1, head);
    let filter = Filter::new()
        .address(contracts.clone())
        .from_block(from)
        .to_block(to);
    let logs = provider.get_logs(&filter).await?;
    // process logs...
    upsert_last_indexed_block(&pool, chain_id, to).await?;
    from = to + 1;
}
```

Updating `last_indexed_block` after each chunk ensures crash resilience — if the process dies mid-catch-up, it resumes from the last completed chunk.

### 5. Exponential Backoff Strategies

Three approaches available in the Rust ecosystem:

**A. Manual with tokio::time::sleep (recommended for M2)**

No additional dependency. Straightforward:

```rust
let mut backoff = Duration::from_secs(1);
let max_backoff = Duration::from_secs(60);

loop {
    match try_connect().await {
        Ok(provider) => {
            backoff = Duration::from_secs(1); // reset on success
            run_event_loop(provider).await;
        }
        Err(e) => {
            tracing::warn!(error = %e, backoff_secs = backoff.as_secs(), "Reconnecting...");
            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    }
}
```

**B. `backoff` crate**

Provides `ExponentialBackoff` and `retry` function. Adds a dependency but more configurable. Overkill for M2's hardcoded parameters.

**C. `tokio-retry` crate**

Similar to `backoff` but Tokio-native. Also adds a dependency.

The press release specifies hardcoded defaults (1s initial, 60s max, 2x multiplier), which aligns with manual implementation.

### 6. Block Tracking Patterns

**File**: `chainpulse-db/src/lib.rs:27-51`

The DB layer provides two functions for block tracking:

- `get_last_indexed_block(pool, chain_id)` at line 27 — returns `Option<i64>`. Returns `None` if no row exists for the chain (fresh DB).
- `upsert_last_indexed_block(pool, chain_id, block)` at line 37 — inserts or updates the `last_block` for a chain.

**Current usage**: Neither function is called from `main.rs`. This is a gap that M2 must address.

**Required changes for M2**:
1. After processing each log (or each chunk of logs), call `upsert_last_indexed_block` with the block number of the last processed event.
2. On reconnect, call `get_last_indexed_block` to determine where to start catch-up.
3. If `get_last_indexed_block` returns `None` (fresh DB), skip catch-up and subscribe from `Latest`.

The `indexed_blocks` table schema (`chainpulse-db/migrations/001_initial_schema.sql:20-24`):
```sql
CREATE TABLE indexed_blocks (
    chain_id    INTEGER PRIMARY KEY,
    last_block  BIGINT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

This tracks one `last_block` per `chain_id`, which is sufficient for M2 (single-chain).

### 7. Gap Detection

Gap detection for M2 is straightforward:

1. On reconnect: `last_indexed = get_last_indexed_block(pool, 1).await?`
2. Get chain head: `head = provider.get_block_number().await?`
3. If `last_indexed.is_some() && last_indexed.unwrap() < head as i64`: there is a gap.
4. Gap range: `[last_indexed + 1, head]`.

No complex gap detection is needed for M2. The single `last_indexed_block` value is sufficient because:
- The indexer processes blocks sequentially.
- `ON CONFLICT DO NOTHING` handles any overlapping re-fetches.
- Block-level gaps within a range are not possible with `eth_getLogs` (it returns all matching logs in the range).

### 8. Deduplication

**File**: `chainpulse-db/src/lib.rs:64-67`

The `insert_event` function already uses:
```sql
ON CONFLICT (chain_id, block_number, log_index) DO NOTHING
```

This means:
- If catch-up fetches logs that overlap with already-indexed live events, duplicates are silently dropped.
- If the live subscription starts before catch-up finishes processing its range, overlapping events are safe.
- No additional dedup logic is needed in application code.

The unique constraint is defined in the migration (`001_initial_schema.sql:13`):
```sql
UNIQUE(chain_id, block_number, log_index)
```

### 9. Required Changes to main.rs

The current `main.rs` needs the following structural changes:

**A. Outer reconnection loop**: Wrap lines 47-112 (WS connect through stream end) in a `loop {}` with backoff.

**B. Block tracking during live indexing**: After each successful `insert_event` call (line 81-99), call `upsert_last_indexed_block` with the event's `block_number`.

**C. Catch-up on reconnect**: After WS reconnect succeeds, before subscribing to live logs:
1. Call `get_last_indexed_block`.
2. If a previous block exists, call `provider.get_block_number()` for the head.
3. Fetch logs in chunks via `provider.get_logs()`.
4. Process through the same `decode_log` + `insert_event` pipeline.

**D. Subscription filter change**: When `last_indexed_block` exists, the live subscription `from_block` could be set to `last_indexed_block + 1` instead of `Latest` to avoid a brief gap between catch-up completion and subscription start. However, this may cause the subscription to receive events that were already caught up — safe due to dedup.

**E. DB error handling change**: Lines 101-104 currently `exit(1)` on any DB write error. For M2, transient DB errors during the event loop should trigger a reconnect cycle rather than a process exit. Persistent DB errors (pool exhausted, connection refused) should still exit.

### 10. Config Considerations

**File**: `chainpulse-config/src/lib.rs:34-57`

The current config has no backoff or reconnection parameters. Per the press release, M2 uses hardcoded defaults:
- Initial backoff: 1 second
- Max backoff: 60 seconds
- Multiplier: 2x

No config changes are needed for M2. These can be made configurable in a future milestone.

The config already includes `eth_rpc_ws_url` (line 37) which is the WS endpoint used for both subscription and `eth_getLogs` calls.

### 11. Alloy Feature Flags

**File**: `Cargo.toml:21`

Current features: `["provider-ws", "pubsub", "rpc-types"]`

For `eth_getLogs` via `provider.get_logs()`, no additional Alloy features are needed. The `Provider` trait's `get_logs` method is available with the existing `provider-ws` feature. The `get_block_number` method is similarly available.

## Code References

- `chainpulse/src/main.rs:47-54` — WS connection (one-shot, no reconnect)
- `chainpulse/src/main.rs:62-64` — Filter construction with `from_block(Latest)`
- `chainpulse/src/main.rs:79-109` — Event processing loop (exits on stream end or DB error)
- `chainpulse/src/main.rs:111-112` — Stream end handler (exit(1))
- `chainpulse-db/src/lib.rs:27-35` — `get_last_indexed_block` (unused in main.rs)
- `chainpulse-db/src/lib.rs:37-52` — `upsert_last_indexed_block` (unused in main.rs)
- `chainpulse-db/src/lib.rs:64-67` — `ON CONFLICT DO NOTHING` in insert_event
- `chainpulse-db/migrations/001_initial_schema.sql:13` — UNIQUE constraint on (chain_id, block_number, log_index)
- `chainpulse-db/migrations/001_initial_schema.sql:20-24` — indexed_blocks table
- `chainpulse-types/src/lib.rs:134` — `decode_log` function (reusable for catch-up logs)
- `chainpulse-types/src/lib.rs:257-269` — `default_contracts` (reusable for catch-up filter)
- `chainpulse-config/src/lib.rs:37` — `eth_rpc_ws_url` config field
- `Cargo.toml:21` — Alloy v1 with provider-ws, pubsub, rpc-types features

## Architecture Notes

The M2 resilience implementation changes `main.rs` from a linear flow to a loop-based architecture:

```
┌─────────────────────────────────────────┐
│            Outer Reconnection Loop       │
│                                         │
│  ┌─────────────────────────────────┐    │
│  │  1. Connect WS (with backoff)   │    │
│  └──────────┬──────────────────────┘    │
│             │                           │
│  ┌──────────▼──────────────────────┐    │
│  │  2. Catch-up (if last_indexed   │    │
│  │     block exists)               │    │
│  │     - get_last_indexed_block    │    │
│  │     - get_block_number (head)   │    │
│  │     - chunked eth_getLogs       │    │
│  │     - decode + insert each log  │    │
│  │     - update last_indexed_block │    │
│  └──────────┬──────────────────────┘    │
│             │                           │
│  ┌──────────▼──────────────────────┐    │
│  │  3. Subscribe to live logs      │    │
│  │     (from Latest or last+1)     │    │
│  └──────────┬──────────────────────┘    │
│             │                           │
│  ┌──────────▼──────────────────────┐    │
│  │  4. Process events in stream    │    │
│  │     - decode_log + insert_event │    │
│  │     - upsert_last_indexed_block │    │
│  └──────────┬──────────────────────┘    │
│             │                           │
│     Stream ends (disconnect)            │
│             │                           │
│  ┌──────────▼──────────────────────┐    │
│  │  5. Apply backoff delay         │    │
│  │     - log WARN                  │    │
│  │     - sleep(backoff)            │    │
│  │     - backoff = min(2x, 60s)   │    │
│  └──────────┬──────────────────────┘    │
│             │                           │
│             └───── loop back to 1 ──────┘
└─────────────────────────────────────────┘
```

Key architectural properties:
- Catch-up runs **before** live subscription (sequential, no interleaving).
- The same `decode_log` + `insert_event` pipeline is used for both catch-up and live events.
- `ON CONFLICT DO NOTHING` provides dedup at the DB level — no application-level dedup needed.
- `last_indexed_block` is updated continuously (per-event or per-chunk), enabling crash-safe resumption.
- Backoff resets on successful connection, so transient issues resolve quickly.

## Open Questions

- **Per-event vs per-chunk block tracking**: Updating `last_indexed_block` after every single event adds a DB write per event (doubling write load). Updating per-chunk (e.g., every 2,000 blocks during catch-up) risks re-processing one chunk on crash. The press release FAQ suggests per-chunk is acceptable since `ON CONFLICT DO NOTHING` handles re-processing safely. During live indexing, updating per-block (after all events in a block are processed) would be a reasonable middle ground but requires grouping events by block.

- **eth_getLogs chunk size**: 2,000 blocks is a safe default. If the project targets specific providers (e.g., Alchemy paid tier allows larger ranges), this could be increased. The press release defers configurable chunk size to a future milestone.

- **Live subscription from_block**: After catch-up completes at block N, the live subscription could start from block N+1 (to avoid a gap) or from `Latest` (simpler but risks missing blocks between catch-up end and subscription start). Starting from N+1 is safer and dedup handles any overlap.

- **DB error handling during event loop**: The current code exits on any DB error. For M2, transient DB errors should trigger the reconnect loop rather than a hard exit. However, distinguishing transient from permanent DB errors adds complexity. The simplest M2 approach may be to continue exiting on DB errors and rely on external process supervision for DB-related failures, focusing M2 solely on WS resilience.

---

## Next Steps

If you're satisfied with this research, proceed to planning:

/pipe-plan-implementation --project chainpulse --ref m2 --research docs/dev/chainpulse/m2-resilience/research.md resilience

If you need more research:

- Ask follow-up questions and I'll append to this document
- Or request a new research topic with /pipe-research --project chainpulse --ref m2 [new-topic]

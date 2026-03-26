---
date: 2026-03-26
researcher: claude-opus-4-6
branch: main
repository: chainpulse
tags: backpressure, performance, bounded-channels, batch-writes, tokio, mpsc
status: complete
---

# Research: Backpressure & Performance (M4)

## Research Question

How do events currently flow from chain subscribers to the database writer in ChainPulse? What is the current channel usage, DB write pattern, and what needs to change to introduce bounded Tokio channels with backpressure and batch database writes?

## Summary

The ChainPulse codebase currently has **no channel** between subscribers and the DB writer. Each per-chain indexer task (`run_indexer` / `run_session` in `chainpulse/src/indexer.rs`) directly calls `chainpulse_db::insert_event()` inline within the event processing loop. Events are written to PostgreSQL one at a time via individual `INSERT ... ON CONFLICT DO NOTHING` statements. There is no separate DB writer task, no channel, no batching, and no backpressure mechanism.

The configuration layer (`chainpulse-config/src/lib.rs`) already defines `channel_buffer_size` (default 1000) and `batch_size` (default 100) fields, and `.env.example` exposes `CHANNEL_BUFFER_SIZE` and `BATCH_SIZE` environment variables. However, these configuration values are not referenced anywhere in the indexer or DB code -- they are pre-provisioned but unused.

The M4 implementation requires: (1) introducing a bounded `tokio::sync::mpsc` channel that all per-chain subscriber tasks send `NormalizedEvent` messages into, (2) a dedicated DB writer Tokio task that receives from this channel, accumulates events in a buffer, and flushes them in batches, and (3) batch INSERT SQL queries that write multiple events in a single transaction.

## Detailed Findings

### Event Flow Architecture (Current State)

The current event flow is entirely inline within each per-chain indexer task:

1. `main.rs:62-71` spawns one `tokio::spawn(indexer::run_indexer(pool_clone, chain_config))` per chain (Ethereum chain_id=1, Polygon chain_id=137).
2. `run_indexer` (indexer.rs:44-94) is the outer reconnection loop with exponential backoff (1s initial, 60s max).
3. `run_session` (indexer.rs:99-211) handles a single WebSocket connection session:
   - Connects via `ProviderBuilder::new().connect_ws(ws)` (line 108-111).
   - Runs catch-up via `catch_up()` (line 121).
   - Subscribes to live logs via `provider.subscribe_logs(&filter)` (line 150-153).
   - Processes events in a `while let Some(log) = stream.next().await` loop (line 164-207).
4. **Inside the event loop** (line 164-207), each event is:
   - Decoded via `chainpulse_types::decode_log(&log, chain_id)` (line 165).
   - Written to DB via `chainpulse_db::insert_event(pool, ...)` (line 166-177) -- **single event, single INSERT, awaited inline**.
   - Block tracking updated via `chainpulse_db::upsert_last_indexed_block(pool, chain_id, event.block_number)` (line 185-192) -- **separate query per event**.

### Catch-Up Path

The catch-up path in `catch_up()` (indexer.rs:221-280) and `fetch_logs_chunk()` (indexer.rs:288-341) follows the same pattern:

- Fetches logs in chunks of `CATCH_UP_CHUNK_SIZE = 2000` blocks (line 12).
- Each log within a chunk is individually inserted via `chainpulse_db::insert_event()` (line 310-321).
- `upsert_last_indexed_block` is called once per chunk (line 329), not per event.

### Database Write Pattern

The `chainpulse_db` crate (`chainpulse-db/src/lib.rs`) provides these write functions:

- `insert_event()` (lines 53-79): Single-row `INSERT INTO events (...) VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (chain_id, block_number, log_index) DO NOTHING`. Returns `bool` indicating whether a row was actually inserted. Takes 7 bind parameters.
- `upsert_last_indexed_block()` (lines 36-51): Single-row `INSERT INTO indexed_blocks ... ON CONFLICT (chain_id) DO UPDATE SET last_block = $2, updated_at = NOW()`.

There are no batch/multi-row insert functions. Each event incurs a full round-trip to PostgreSQL.

### Database Schema

The migration at `chainpulse-db/migrations/001_initial_schema.sql` defines:

- `events` table: `BIGSERIAL` primary key, composite unique constraint on `(chain_id, block_number, log_index)`, three indexes (`idx_events_chain_type`, `idx_events_contract`, `idx_events_block`).
- `indexed_blocks` table: `chain_id` as primary key, `last_block` column.
- `tracked_contracts` table: composite unique on `(chain_id, address)`.

The `ON CONFLICT ... DO NOTHING` clause in `insert_event` provides idempotent upserts using the composite unique constraint.

### Channel and Batch Configuration (Pre-provisioned but Unused)

`chainpulse-config/src/lib.rs` defines:

- `channel_buffer_size: usize` with `default_channel_buffer_size() -> 1000` (lines 22-24, 49-50).
- `batch_size: usize` with `default_batch_size() -> 100` (lines 30-32, 55-56).

These are deserialized from environment variables `CHANNEL_BUFFER_SIZE` and `BATCH_SIZE` respectively, and are documented in `.env.example` (lines 10-12). However, neither `channel_buffer_size` nor `batch_size` is referenced in `main.rs` or `indexer.rs`. The config is loaded but these fields are never read.

### NormalizedEvent Type

`chainpulse-types/src/lib.rs:114-123` defines:

```rust
pub struct NormalizedEvent {
    pub event_type: String,
    pub contract_address: String,
    pub tx_hash: String,
    pub block_number: i64,
    pub log_index: i32,
    pub chain_id: i32,
    pub payload: serde_json::Value,
}
```

This struct derives `Debug, Clone, Serialize, Deserialize` and is the natural message type for the bounded channel.

### Workspace and Dependencies

The workspace `Cargo.toml` already includes `tokio = { version = "1", features = ["full"] }`, which includes `tokio::sync::mpsc`. No additional dependencies are needed for bounded channels.

`sqlx` version 0.8 is used with features `runtime-tokio`, `postgres`, `json`, `migrate`. It supports multi-row operations and transactions via `sqlx::query` with dynamically built SQL, or via `PgPool::begin()` for explicit transactions.

### Per-Chain Task Independence

Each chain runs as an independent Tokio task (spawned at `main.rs:70`). Each task receives its own `pool.clone()` (a `PgPool` which is an `Arc` internally). The tasks share no state with each other -- they each run their own reconnection loop and event processing loop.

### Error Handling Pattern

The indexer distinguishes between `Recoverable` and `Fatal` errors (indexer.rs:24-30). Recoverable errors (WS disconnects, RPC failures) trigger reconnection with backoff. Fatal errors (DB failures) cause the task to exit. The live event loop treats any DB write failure as fatal (line 199), while the catch-up path does the same (line 321).

## Code References

- `chainpulse/src/main.rs:62-71` - Per-chain task spawning, pool cloning
- `chainpulse/src/main.rs:29` - Pool creation using `config.db_max_connections`
- `chainpulse/src/indexer.rs:44-94` - `run_indexer` outer reconnection loop
- `chainpulse/src/indexer.rs:99-211` - `run_session` single connection session
- `chainpulse/src/indexer.rs:164-207` - Live event processing loop with inline DB writes
- `chainpulse/src/indexer.rs:221-280` - `catch_up` function
- `chainpulse/src/indexer.rs:288-341` - `fetch_logs_chunk` with per-event inline inserts
- `chainpulse/src/indexer.rs:12` - `CATCH_UP_CHUNK_SIZE = 2000`
- `chainpulse-db/src/lib.rs:54-79` - `insert_event` single-row INSERT
- `chainpulse-db/src/lib.rs:36-51` - `upsert_last_indexed_block` single-row UPSERT
- `chainpulse-db/src/lib.rs:13-19` - `create_pool` with `max_connections`
- `chainpulse-db/migrations/001_initial_schema.sql:1-14` - Events table with composite unique constraint
- `chainpulse-types/src/lib.rs:114-123` - `NormalizedEvent` struct definition
- `chainpulse-config/src/lib.rs:22-24` - `default_channel_buffer_size() -> 1000`
- `chainpulse-config/src/lib.rs:30-32` - `default_batch_size() -> 100`
- `chainpulse-config/src/lib.rs:49-50` - `channel_buffer_size` config field
- `chainpulse-config/src/lib.rs:55-56` - `batch_size` config field
- `.env.example:10-12` - `CHANNEL_BUFFER_SIZE` and `BATCH_SIZE` env vars

## Architecture Notes

The current architecture has a tight coupling between the subscriber event loop and the database writer. Each subscriber task acts as both the event consumer (from WebSocket) and the DB writer (via direct `insert_event` calls). This means:

1. **No backpressure**: If the WebSocket delivers events faster than the DB can write them, the Tokio task simply blocks on the `insert_event` await, which in turn blocks `stream.next()` from being polled. This provides implicit backpressure at the WebSocket level via Tokio's task scheduling, but there is no explicit bounded buffer. The alloy WS provider may buffer incoming messages internally, and if the DB is slow for an extended period, this internal buffering is unbounded.

2. **No batching**: Each event is a separate `INSERT` statement, each requiring a full PostgreSQL round-trip. This is the primary throughput bottleneck.

3. **No shared writer**: Each chain's Tokio task writes to the DB independently. There is no centralized DB writer task that could optimize write patterns across chains.

4. **Config is ready**: The `channel_buffer_size` and `batch_size` configuration values already exist with sensible defaults (1000 and 100 respectively), ready to be wired into the implementation.

5. **NormalizedEvent is channel-ready**: The `NormalizedEvent` struct is `Clone + Send + Sync` (via derive), making it suitable as the channel message type for `tokio::sync::mpsc::Sender<NormalizedEvent>`.

The transition to bounded channels + batch writes requires:
- Creating a `tokio::sync::mpsc::channel(config.channel_buffer_size)` in `main.rs`.
- Passing the `Sender` half to each indexer task instead of (or alongside) the `PgPool`.
- Spawning a dedicated DB writer task that owns the `Receiver` half and the `PgPool`.
- The DB writer accumulates events into a `Vec<NormalizedEvent>` buffer, flushing when `buffer.len() >= batch_size` or a flush timeout fires.
- A new `batch_insert_events` function in `chainpulse-db` that builds a multi-row INSERT statement.

## Open Questions

- What flush timeout interval is appropriate? The press release mentions a configurable flush interval but `chainpulse-config` does not currently define a `flush_interval` or `batch_flush_interval_ms` configuration field.
- Should `upsert_last_indexed_block` be called once per batch (using the max block number in the batch) or once per event? Per-batch is more efficient and consistent with the batching approach.
- Should the catch-up path (`fetch_logs_chunk`) also route events through the bounded channel and batch writer, or should it continue to write directly to the DB? Routing through the channel would provide consistent backpressure protection during catch-up bursts, as noted in the press release FAQ.
- The current `insert_event` returns `bool` to indicate whether a row was inserted (for dedup). Batch inserts could return a count of rows affected. Does any caller depend on the per-event boolean result? Currently, only the live event loop checks `Ok(_)` without using the boolean value, so the answer is no.

---

## Next Steps

If you are satisfied with this research, proceed to planning:

/pipe-plan-implementation --project chainpulse --ref m4 --research docs/dev/chainpulse/m4-backpressure-performance/research.md Backpressure & Performance

If you need more research:

- Ask follow-up questions and I will append to this document
- Or request a new research topic with /pipe-research --project chainpulse --ref m4 [new-topic]

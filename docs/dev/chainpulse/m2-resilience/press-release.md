# Resilient Event Indexing - PR FAQ

> **Milestone**: M2 > **Created**: 2026-03-26 > **Status**: Draft

---

## Press Release

### Resilient Event Indexing

**For** protocol analytics teams and developers running ChainPulse
**Who** lose indexed event data whenever their WebSocket connection drops
**The** Resilient Event Indexing upgrade
**Is a** reliability improvement to the ChainPulse indexer
**That** ensures zero data gaps by automatically reconnecting and catching up on missed blocks
**Unlike** the current M1 behavior where a WebSocket disconnect kills the process and requires manual restart with potential data loss
**Our solution** automatically reconnects with exponential backoff, detects the gap between the last indexed block and the current chain head, fetches all missed logs, and deduplicates overlapping data — all without operator intervention.

---

ChainPulse now recovers automatically from WebSocket disconnections. When a connection drops — whether due to provider maintenance, network instability, or rate limiting — the indexer reconnects with exponential backoff (starting at 1 second, capping at 60 seconds) and resumes indexing without missing a single event.

On reconnect, ChainPulse queries the database for the last successfully indexed block, compares it to the current chain head, and uses `eth_getLogs` to fetch all events from the gap range. This catch-up process runs to completion before resuming the live subscription stream. Because the database already enforces uniqueness on `(chain_id, block_number, log_index)`, any overlapping events between the catch-up batch and the live stream are silently deduplicated — no duplicate rows, no manual cleanup.

Operators no longer need to babysit the indexer process or set up external restart mechanisms. ChainPulse handles transient failures internally, logs every reconnection attempt and catch-up range for observability, and only exits on truly unrecoverable errors like persistent database failures.

## FAQ

### Customer Experience

Q: How does an operator know when a reconnection happens?
A: ChainPulse emits structured log entries at `WARN` level for each disconnection and reconnection attempt, including the retry count and backoff duration. A successful reconnect logs at `INFO` level with the catch-up block range. Operators monitoring logs or forwarding to a log aggregator will see these events clearly.

Q: What does the happy path look like for catch-up?
A: On reconnect, ChainPulse reads `last_indexed_block` from the database, fetches the current chain head via `eth_getBlockNumber`, and issues `eth_getLogs` calls for the missed range. Events are decoded and inserted using the same pipeline as live events. Once catch-up completes, the live subscription resumes from the current head. The entire process is automatic and requires no operator action.

Q: What happens if the gap is very large (e.g., hours of downtime)?
A: The catch-up logic processes the gap in chunked `eth_getLogs` requests to avoid hitting RPC response size limits. Each chunk is processed and written to the database before requesting the next. Progress is tracked by updating `last_indexed_block` after each chunk, so if another disconnect happens during catch-up, it resumes from where it left off rather than restarting the entire range.

Q: Does catch-up slow down live indexing?
A: Catch-up runs to completion before the live subscription starts, so there is no interleaving. Once catch-up finishes, live indexing resumes at full speed. This sequential approach is simpler and avoids race conditions between catch-up writes and live writes.

### Scope & Non-Goals

Q: What is explicitly NOT included in V1?
A: The following are out of scope for M2:
- Historical full-chain backfill (only catches up from last indexed block, not from genesis)
- Multi-chain resilience (M2 targets the single Ethereum subscriber from M1; multi-chain comes in M3)
- Metrics/Prometheus integration for reconnection counts (deferred to M7)
- Alerting or notification systems for prolonged disconnections
- Configurable backoff parameters (hardcoded sensible defaults)

Q: What is the minimal version that still delivers the core benefit?
A: The minimal viable version is: (1) detect WS disconnect, (2) reconnect with basic exponential backoff, (3) query `last_indexed_block` and `eth_getLogs` for the gap, (4) insert catch-up events through the existing pipeline. Gap detection beyond simple last-block comparison and chunked fetching are enhancements that strengthen the core but could be simplified if needed.

Q: What are we deferring to future iterations?
A: Configurable backoff parameters, reconnection metrics exposed via Prometheus, multi-chain reconnection coordination, and block reorganization (reorg) handling. Reorg handling is a separate concern — M2 assumes finalized blocks do not change.

### Technical Constraint FAQs

Q: What existing system capabilities does this depend on?
A: M2 depends on:
- `chainpulse-db::get_last_indexed_block` and `upsert_last_indexed_block` — already implemented in M0
- `chainpulse-db::insert_event` with `ON CONFLICT DO NOTHING` — already implemented in M1
- `chainpulse-types::decode_log` and `default_contracts()` — already implemented in M1
- Alloy provider's `eth_getLogs` RPC method for historical log queries
- The existing `events` table composite unique key `(chain_id, block_number, log_index)`

Q: What new capabilities need to be built?
A: New components:
- Reconnection loop with exponential backoff wrapping the WS provider connection
- Catch-up function that queries `eth_getLogs` for a block range and processes results through the existing decode/insert pipeline
- Gap detection: compare `last_indexed_block` to current chain head on each reconnect
- Chunked `eth_getLogs` fetching to handle large block ranges without exceeding RPC limits
- Update `last_indexed_block` after processing each batch of events (not just on disconnect)

Q: Are there performance, scale, or data constraints?
A: RPC providers typically limit `eth_getLogs` responses to 10,000 logs or a 2,000-block range per request. The chunked fetching approach must respect these limits. Additionally, catch-up for very long outages (thousands of blocks) will take time proportional to the gap size. The existing single-event-at-a-time DB write pattern from M1 applies during catch-up; batch writes come in M4.

Q: What is the simplest implementation that delivers the PR outcome?
A: Wrap the current `main.rs` event loop in an outer reconnection loop. On WS stream end: (1) read last indexed block from DB, (2) reconnect WS with backoff, (3) get current head, (4) call `eth_getLogs` for the gap range, (5) process each log through `decode_log` + `insert_event`, (6) update `last_indexed_block`, (7) resubscribe to live logs and re-enter the event loop.

### Edge Cases & Error States

Q: What happens if the RPC endpoint is down for an extended period?
A: The exponential backoff continues retrying with increasing delays up to the maximum (60 seconds). The indexer logs each failed attempt. It will keep retrying indefinitely — the assumption is that transient outages resolve eventually. If an operator needs to stop the indexer, they send SIGTERM as usual.

Q: What happens if `eth_getLogs` returns an error for the catch-up range?
A: If a chunk fails, the catch-up retries that specific chunk with backoff. Since `last_indexed_block` is updated after each successful chunk, progress is not lost. If the RPC consistently rejects the range (e.g., range too large), the chunk size is reduced and retried.

Q: What happens if catch-up events overlap with events already in the database?
A: The `ON CONFLICT (chain_id, block_number, log_index) DO NOTHING` constraint silently skips duplicates. This is already implemented and tested in M1. Catch-up can safely request overlapping block ranges without creating duplicate rows.

Q: What happens if the process crashes mid-catch-up?
A: On restart, `get_last_indexed_block` returns the last block for which events were fully committed. Catch-up resumes from that point. There is no partial state to clean up because each chunk's block tracking update happens after its events are written.

Q: What happens if no events were ever indexed (fresh database)?
A: If `get_last_indexed_block` returns `None`, there is no gap to fill. The indexer subscribes from the latest block as it does today in M1. Catch-up only activates when there is a known last indexed block to compare against.

### Business & Operational FAQs

Q: How will we know if this is successful?
A: Success means: (1) the indexer process can run continuously for days without manual restarts, (2) after any WS disconnection, the `events` table has no block gaps for tracked contracts, (3) operators see clear reconnection/catch-up log entries. We can verify by intentionally killing the WS connection and confirming seamless recovery.

Q: What could go wrong post-launch?
A: Potential issues: (1) RPC provider changes their `eth_getLogs` limits, requiring chunk size adjustment, (2) very long outages create large catch-up backlogs that delay live indexing resumption, (3) the backoff ceiling may be too aggressive or too conservative for certain providers. All are addressable through configuration changes in future milestones.

Q: Does this require changes to support/ops processes?
A: This reduces operational burden. Operators no longer need external process supervisors (systemd restart, Docker restart policies) as the primary resilience mechanism. However, external restarts remain a useful safety net for truly catastrophic failures (DB permanently down, OOM). No new operational procedures are needed.

## Implementation Scope

### Current State -> End State

| Current State | End State |
| --- | --- |
| WebSocket disconnect kills the process with `exit(1)` | WebSocket disconnect triggers automatic reconnection with exponential backoff |
| Manual restart required after any connection drop | Indexer self-heals and resumes without operator intervention |
| Events during downtime are permanently lost | Catch-up indexing fetches all missed events via `eth_getLogs` on reconnect |
| `last_indexed_block` is never written during normal operation | `last_indexed_block` is updated continuously, enabling gap detection |
| No awareness of block gaps | Gap detection compares last indexed block to chain head on every reconnect |
| Overlapping data could theoretically cause issues on restart | Deduplication via `ON CONFLICT DO NOTHING` handles all overlap gracefully |

### User Stories

- [ ] As an operator, I can leave ChainPulse running and it automatically recovers from WebSocket disconnections so that I do not need to manually restart the process
- [ ] As an operator, I can see structured log entries for disconnection, reconnection attempts, and catch-up progress so that I have visibility into the indexer's resilience behavior
- [ ] As a data consumer, I can trust that the events table has no block gaps for tracked contracts so that my analytics queries return complete data
- [ ] As a data consumer, I can see events that occurred during indexer downtime appear in the database after reconnection so that no data is lost
- [ ] As an operator, I can restart ChainPulse after a crash and it resumes from the last indexed block so that catch-up is efficient and does not reprocess the entire history
- [ ] As a developer, I can verify that duplicate events from overlapping catch-up ranges do not create duplicate database rows so that data integrity is maintained

### Success Criteria

- [ ] The indexer reconnects automatically within 60 seconds of a WebSocket disconnection
- [ ] After reconnection, all events from the missed block range are present in the database
- [ ] No duplicate rows exist in the events table after catch-up processing
- [ ] `last_indexed_block` is updated after each batch of events during both live indexing and catch-up
- [ ] Catch-up handles block ranges exceeding 2,000 blocks by chunking requests
- [ ] The indexer logs all reconnection attempts, catch-up ranges, and completion status
- [ ] The indexer survives repeated disconnect/reconnect cycles without memory leaks or panics

### Known Compromises

- [ ] No configurable backoff parameters — hardcoded defaults (1s initial, 60s max, 2x multiplier) are sufficient for M2; configuration deferred to later milestone
- [ ] Single-event DB writes during catch-up — batch writes would be faster but are deferred to M4; the existing `insert_event` function is reused for simplicity
- [ ] No reorg handling — M2 assumes blocks are final once indexed; chain reorganizations are a separate concern
- [ ] No multi-chain coordination — reconnection logic applies to the single Ethereum subscriber; multi-chain resilience comes with M3
- [ ] No metrics exposure — reconnection counts and catch-up durations are logged but not exposed as Prometheus metrics (deferred to M7)

### Open Questions / Needs Research

- [ ] What are the exact `eth_getLogs` limits for common providers (Alchemy, Infura, public endpoints)? This determines the optimal chunk size for catch-up.
- [ ] Should `last_indexed_block` be updated per-event or per-chunk during catch-up? Per-chunk is simpler but risks re-processing a chunk on crash; per-event is more granular but adds DB write overhead.
- [ ] How should the system handle the edge case where `last_indexed_block` is far behind head (e.g., >100,000 blocks)? Should there be a configurable maximum catch-up range with a warning?
- [ ] What is the best Alloy API for `eth_getLogs` with block range parameters? Need to verify the exact method signature and filter construction.

---

## Next Steps

When ready to implement:

1. Run `/pipe-research --project chainpulse --ref m2 resilience` to analyze codebase
2. Run `/pipe-plan-implementation --project chainpulse --ref m2 resilience` to create implementation plan
3. Run `/pipe-implement docs/dev/chainpulse/m2-resilience/implementation-plan.md` to execute the plan

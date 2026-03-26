# Backpressure & Performance - PR FAQ

> **Milestone**: M4 > **Created**: 2026-03-26 > **Status**: Draft

---

## Press Release

### Backpressure & Performance

**For** protocol analytics teams and operators running ChainPulse in production
**Who** need reliable, high-throughput event indexing that stays stable under peak chain activity
**The** Backpressure & Performance upgrade
**Is a** reliability and throughput improvement to the ChainPulse indexing pipeline
**That** prevents memory exhaustion during traffic spikes and increases sustained write throughput by batching events into efficient database transactions
**Unlike** the current implementation where unbounded channels can grow without limit when the database writer falls behind, causing out-of-memory crashes during high-activity periods
**Our solution** introduces bounded Tokio channels that apply natural backpressure to upstream subscribers, combined with batch database writes that dramatically improve throughput per transaction

---

ChainPulse operators can now run their indexers through the most demanding on-chain activity periods without worrying about memory exhaustion or degraded performance. When chain activity spikes — during token launches, liquidation cascades, or airdrop claims — the system automatically throttles event ingestion to match the database writer's capacity, keeping memory usage predictable and stable.

The database writer now accumulates events and flushes them in configurable batches, reducing per-event overhead and enabling sustained throughput of thousands of events per minute at peak. Operators see consistent, predictable resource consumption regardless of chain activity levels, and the system gracefully handles load without operator intervention.

## FAQ

### Customer Experience

Q: How does an operator know backpressure is being applied?
A: When the bounded channel reaches capacity, subscriber tasks will naturally slow down their ingestion rate. This is observable through metrics — channel utilization percentage, subscriber wait times, and events-per-second rates. Structured logs will indicate when a subscriber is waiting on channel capacity. No operator action is required; the system self-regulates.

Q: What does the happy path look like during normal operation?
A: During normal load, events flow through the bounded channel without hitting capacity limits. The DB writer accumulates events up to a configurable batch size or a flush timeout (whichever comes first), then writes them in a single transaction. From the operator's perspective, behavior is identical to before — events are indexed in near-real-time with slightly improved throughput.

Q: What happens during a traffic spike?
A: When event volume exceeds the DB writer's throughput, the bounded channel fills up. Subscribers then block on channel send, naturally reducing their consumption rate from the WebSocket. This is intentional — it prevents memory from growing unboundedly. Once the DB writer catches up (batch flushes clear channel capacity), subscribers resume full-speed ingestion. Events are never dropped; they remain in the RPC node's buffer until consumed.

Q: How does this interact with the existing reconnection and catch-up logic from M2?
A: Backpressure and reconnection work together. If a subscriber is slowed by backpressure and the WebSocket connection drops, the existing M2 catch-up logic kicks in on reconnect, fetching missed events from the last indexed block. The catch-up events also flow through the bounded channel, so the system remains protected during catch-up bursts as well.

### Scope & Non-Goals

Q: What is explicitly NOT included in V1?
A: The following are not in scope: (1) Dynamic channel sizing that auto-adjusts based on load, (2) Priority queuing for certain event types or chains, (3) Disk-based overflow buffering for when channels are full, (4) Per-chain independent channel tuning at runtime. These are potential future enhancements.

Q: What is the minimal version that delivers the core benefit?
A: A single bounded Tokio MPSC channel replacing the current unbounded channel, plus a DB writer that accumulates events into batches before flushing. Even with fixed/default configuration values, this delivers the core memory safety and throughput benefits.

Q: What would make us cut scope further if needed?
A: Batch writes (E4.2) could be simplified to fixed-size batches without a flush timeout if needed. The bounded channel (E4.1) is non-negotiable — it is the core memory safety mechanism.

Q: What are we deferring to future iterations?
A: Prometheus metrics for channel utilization and batch write performance are deferred to M7 (Observability). Graceful shutdown with channel draining is deferred to M8 (Hardening).

### Technical Constraint FAQs

Q: What existing system capabilities does this depend on?
A: This depends on M3 (Multi-Chain) being complete — specifically, the per-chain subscriber Tokio tasks and the shared DB writer. The channel replaces whatever current mechanism passes events from subscribers to the writer.

Q: What new capabilities need to be built?
A: (1) A bounded Tokio MPSC channel replacing the current event passing mechanism, (2) A batching accumulator in the DB writer that collects events and flushes on size threshold or time interval, (3) Batch INSERT/UPSERT SQL queries that handle multiple events in a single transaction.

Q: Are there performance, scale, or data constraints?
A: The channel capacity and batch size are the key tuning parameters. Channel capacity too small causes excessive backpressure and slows indexing unnecessarily. Too large defeats the purpose of bounding memory. Batch size too large increases latency between event receipt and database availability. The system must sustain thousands of events per minute at peak (per project goals).

Q: What is the simplest implementation that delivers the PR outcome?
A: Replace unbounded channel with `tokio::sync::mpsc::channel(capacity)` for the bounded channel. For batch writes, use `tokio::time::interval` combined with a Vec buffer that flushes when either the buffer reaches a size threshold or the interval ticks, whichever comes first. Use a single multi-row INSERT statement for the batch.

### Edge Cases & Error States FAQs

Q: What happens if the DB writer crashes or the database becomes unavailable?
A: The bounded channel fills up, which causes all subscribers to block. This is the correct behavior — it prevents event loss. When the DB writer recovers, it resumes draining the channel. If the DB is down for an extended period, WebSocket connections may time out, and the M2 reconnection logic handles recovery.

Q: What happens if a batch write partially fails?
A: The entire batch should be wrapped in a database transaction. If the transaction fails, the batch is retried. Events in the batch are not acknowledged (removed from the channel) until the transaction succeeds. This ensures no event loss.

Q: What happens if one chain produces far more events than another?
A: With a shared MPSC channel, all chains compete for the same channel capacity. A high-volume chain could slow down a low-volume chain's ability to send to the channel. This is acceptable for V1 — per-chain channels could be explored in a future iteration if this becomes a problem.

Q: What are the boundary conditions for batch sizing?
A: Batch of 1 event (degenerate case) should still work, equivalent to single-row inserts. Empty batch (flush timer fires with no events) should be a no-op. Very large batches should be bounded by maximum batch size to keep transaction sizes reasonable.

### Business & Operational FAQs

Q: How will we know if this is successful?
A: Success is measured by: (1) Zero OOM crashes during peak activity periods, (2) Memory usage remains bounded and predictable under load, (3) Sustained throughput of thousands of events/minute, (4) No dropped events during backpressure periods — all events eventually indexed.

Q: What could go wrong post-launch?
A: (1) Channel capacity set too low, causing unnecessary backpressure during normal operation, (2) Batch size too large, causing noticeable indexing latency, (3) A sustained overload where backpressure causes WebSocket timeouts faster than catch-up can recover. All of these are tunable via configuration.

Q: Does this require changes to support/ops processes?
A: Operators should be aware of new configuration parameters (channel capacity, batch size, flush interval) and their impact. Default values should work for typical deployments without tuning.

Q: Are there compliance or security considerations?
A: No. This is an internal pipeline optimization that does not change the data model, API surface, or access patterns.

## Implementation Scope

### Current State -> End State

| Current State | End State |
| --- | --- |
| Unbounded channel between subscribers and DB writer; memory grows without limit under load | Bounded Tokio channel applies backpressure; memory usage is predictable and capped |
| Single-event database writes; each event is a separate INSERT | Batch writes accumulate events and flush in configurable batches for higher throughput |
| System can OOM during high-activity periods | System self-regulates under load, slowing ingestion to match write capacity |
| No visibility into channel utilization or write throughput | Structured log messages indicate backpressure events and batch flush statistics |

### User Stories

- [ ] As the system, I use a bounded Tokio channel between subscribers and the DB writer so that memory usage is capped regardless of event volume (E4.1.1)
- [ ] As the system, I slow down event ingestion when the DB writer is behind so that the pipeline self-regulates without operator intervention (E4.1.2)
- [ ] As the system, I batch multiple events into a single DB transaction so that write throughput is significantly improved (E4.2.1)
- [ ] As the system, I can sustain thousands of events per minute at peak so that no events are lost during high-activity periods (E4.2.2)

### Success Criteria

- [ ] Memory usage remains bounded under sustained high-throughput event streams
- [ ] Subscriber tasks block when channel is full rather than buffering unboundedly
- [ ] DB writer flushes events in configurable batches (by size threshold and time interval)
- [ ] Batch writes use a single transaction per batch for atomicity
- [ ] System sustains thousands of events per minute without event loss
- [ ] Backpressure does not cause permanent stalls — system recovers when load decreases
- [ ] All existing M1-M3 functionality continues to work correctly

### Known Compromises

- [ ] Shared channel for all chains — a high-volume chain can slow others; per-chain channels deferred
- [ ] Static configuration for channel capacity and batch size — dynamic auto-tuning deferred
- [ ] Observability of backpressure limited to structured logs until M7 adds Prometheus metrics
- [ ] No disk-based overflow buffer — if channel is full, subscribers block (acceptable given M2 catch-up)

### Open Questions / Needs Research

- [ ] What is the current mechanism for passing events from subscribers to the DB writer? (unbounded channel, direct call, etc.)
- [ ] What is the optimal default channel capacity for typical multi-chain indexing workloads?
- [ ] What is the optimal default batch size and flush interval for PostgreSQL bulk inserts via sqlx?
- [ ] Should the batch writer use multi-row INSERT VALUES or COPY for maximum throughput?
- [ ] Does the current DB writer run as a dedicated Tokio task, or is writing done inline in subscriber tasks?

---

## Next Steps

When ready to implement:

1. Run `/pipe-research --project chainpulse --ref m4 Backpressure & Performance` to analyze codebase
2. Run `/pipe-plan-implementation --project chainpulse --ref m4 Backpressure & Performance` to create implementation plan
3. Run `/pipe-implement docs/dev/chainpulse/m4-backpressure-performance/implementation-plan.md` to execute the plan

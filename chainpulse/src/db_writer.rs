use chainpulse_types::NormalizedEvent;
use metrics::{counter, gauge, histogram};
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Receiver;
use tokio::time::{interval, Interval};
use tokio_util::sync::CancellationToken;

use crate::indexer::IndexerError;

/// Map a chain_id to a human-readable chain name for metric labels.
fn chain_name(chain_id: i32) -> &'static str {
    match chain_id {
        1 => "ethereum",
        137 => "polygon",
        _ => "unknown",
    }
}

/// Maximum number of retry attempts for a failed batch flush before giving up.
const MAX_FLUSH_RETRIES: u32 = 3;

/// Delay between retry attempts for a failed batch flush.
const RETRY_DELAY: Duration = Duration::from_secs(1);

/// Dedicated DB writer task that receives events from the bounded MPSC channel,
/// accumulates them in a buffer, and flushes in batches.
///
/// Flush triggers:
/// - Buffer reaches `batch_size` events (size threshold)
/// - `flush_interval_ms` elapses since the last flush (timer)
///
/// On flush: calls `batch_insert_events`, then `batch_upsert_last_indexed_blocks`,
/// then clears the buffer. On DB error, retries up to `MAX_FLUSH_RETRIES` times.
/// If all retries fail, returns a fatal error.
///
/// When all senders are dropped (all indexers exit), flushes remaining buffer and
/// returns Ok.
pub async fn run(
    mut rx: Receiver<NormalizedEvent>,
    pool: PgPool,
    batch_size: usize,
    flush_interval_ms: u64,
    event_tx: broadcast::Sender<NormalizedEvent>,
    token: CancellationToken,
) -> Result<(), IndexerError> {
    tracing::info!(
        batch_size = batch_size,
        flush_interval_ms = flush_interval_ms,
        "DB writer task started"
    );

    let mut buffer: Vec<NormalizedEvent> = Vec::with_capacity(batch_size);
    let mut flush_timer: Interval = interval(Duration::from_millis(flush_interval_ms));
    // The first tick completes immediately; consume it so we don't flush an empty buffer right away.
    flush_timer.tick().await;

    loop {
        tokio::select! {
            // Bias towards receiving events to keep the buffer filling efficiently.
            // The timer branch will still fire when the receive is idle.
            biased;

            _ = token.cancelled() => {
                // Drain remaining events from the channel
                let mut drained: usize = 0;
                while let Ok(event) = rx.try_recv() {
                    buffer.push(event);
                    drained += 1;
                }
                tracing::info!(
                    drained = drained,
                    buffered = buffer.len(),
                    "DB writer: shutdown requested, draining channel"
                );
                if !buffer.is_empty() {
                    flush_batch(&pool, &mut buffer, "shutdown", &event_tx).await?;
                }
                tracing::info!("DB writer: shutdown complete");
                return Ok(());
            }

            event = rx.recv() => {
                match event {
                    Some(event) => {
                        buffer.push(event);
                        if buffer.len() >= batch_size {
                            flush_batch(&pool, &mut buffer, "size_threshold", &event_tx).await?;
                            flush_timer.reset();
                        }
                    }
                    None => {
                        // All senders dropped -- flush remaining events and exit.
                        if !buffer.is_empty() {
                            flush_batch(&pool, &mut buffer, "shutdown", &event_tx).await?;
                        }
                        tracing::info!("DB writer: all senders dropped, shutting down");
                        return Ok(());
                    }
                }
            }

            _ = flush_timer.tick() => {
                if !buffer.is_empty() {
                    flush_batch(&pool, &mut buffer, "timer", &event_tx).await?;
                }
            }
        }
    }
}

/// Flush the buffer to the database with retry logic.
/// On success, clears the buffer. On repeated failure, returns a fatal error.
async fn flush_batch(
    pool: &PgPool,
    buffer: &mut Vec<NormalizedEvent>,
    trigger: &str,
    event_tx: &broadcast::Sender<NormalizedEvent>,
) -> Result<(), IndexerError> {
    let event_count = buffer.len();
    let start = std::time::Instant::now();

    for attempt in 1..=MAX_FLUSH_RETRIES {
        match chainpulse_db::batch_insert_events(pool, buffer).await {
            Ok(rows_inserted) => {
                // Now update block tracking
                if let Err(e) = chainpulse_db::batch_upsert_last_indexed_blocks(pool, buffer).await {
                    tracing::error!(
                        error = %e,
                        attempt = attempt,
                        "Failed to upsert last indexed blocks"
                    );
                    // Retry the whole operation if block tracking fails
                    if attempt < MAX_FLUSH_RETRIES {
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    }
                    return Err(IndexerError::Fatal(format!(
                        "Failed to upsert last indexed blocks after {MAX_FLUSH_RETRIES} retries: {e}"
                    )));
                }

                let duration = start.elapsed();
                let duration_ms = duration.as_millis();

                // Record DB write duration histogram
                histogram!("chainpulse_db_write_duration_seconds", "trigger" => trigger.to_string())
                    .record(duration.as_secs_f64());

                // Increment events_processed_total counter per chain and event_type
                let mut events_by_key: HashMap<(i32, &str), u64> = HashMap::new();
                for event in buffer.iter() {
                    *events_by_key
                        .entry((event.chain_id, event.event_type.as_str()))
                        .or_default() += 1;
                }
                for ((chain_id, event_type), count) in &events_by_key {
                    counter!(
                        "chainpulse_events_processed_total",
                        "chain" => chain_name(*chain_id).to_string(),
                        "event_type" => event_type.to_string(),
                    )
                    .increment(*count);
                }

                // Set last_indexed_block gauge per chain
                let mut chain_max_blocks: HashMap<i32, i64> = HashMap::new();
                for event in buffer.iter() {
                    let entry = chain_max_blocks.entry(event.chain_id).or_insert(event.block_number);
                    if event.block_number > *entry {
                        *entry = event.block_number;
                    }
                }
                for (chain_id, block_number) in &chain_max_blocks {
                    gauge!(
                        "chainpulse_last_indexed_block",
                        "chain" => chain_name(*chain_id).to_string(),
                    )
                    .set(*block_number as f64);
                }

                tracing::info!(
                    events_flushed = event_count,
                    rows_inserted = rows_inserted,
                    flush_trigger = trigger,
                    flush_duration_ms = duration_ms,
                    "Batch flushed: {} events",
                    event_count,
                );
                // Publish events to broadcast channel before clearing buffer
                for event in buffer.iter() {
                    let _ = event_tx.send(event.clone());
                }
                buffer.clear();
                return Ok(());
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    attempt = attempt,
                    max_retries = MAX_FLUSH_RETRIES,
                    events_in_batch = event_count,
                    "Batch insert failed, retrying"
                );
                if attempt < MAX_FLUSH_RETRIES {
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
    }

    Err(IndexerError::Fatal(format!(
        "Batch insert failed after {MAX_FLUSH_RETRIES} retries, {} events lost",
        event_count,
    )))
}

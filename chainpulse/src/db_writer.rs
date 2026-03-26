use chainpulse_types::NormalizedEvent;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::time::{interval, Interval};

use crate::indexer::IndexerError;

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

            event = rx.recv() => {
                match event {
                    Some(event) => {
                        buffer.push(event);
                        if buffer.len() >= batch_size {
                            flush_batch(&pool, &mut buffer, "size_threshold").await?;
                            flush_timer.reset();
                        }
                    }
                    None => {
                        // All senders dropped -- flush remaining events and exit.
                        if !buffer.is_empty() {
                            flush_batch(&pool, &mut buffer, "shutdown").await?;
                        }
                        tracing::info!("DB writer: all senders dropped, shutting down");
                        return Ok(());
                    }
                }
            }

            _ = flush_timer.tick() => {
                if !buffer.is_empty() {
                    flush_batch(&pool, &mut buffer, "timer").await?;
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

                let duration_ms = start.elapsed().as_millis();
                tracing::info!(
                    events_flushed = event_count,
                    rows_inserted = rows_inserted,
                    flush_trigger = trigger,
                    flush_duration_ms = duration_ms,
                    "Batch flushed: {} events",
                    event_count,
                );
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

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::eips::BlockNumberOrTag;
use chainpulse_types::NormalizedEvent;
use futures_util::StreamExt;
use sqlx::PgPool;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;
use tracing::Instrument;

/// Number of blocks to fetch per `eth_getLogs` chunk during catch-up.
/// 2,000 is safe for all major RPC providers (Alchemy free tier, Infura, Ankr, public nodes).
const CATCH_UP_CHUNK_SIZE: u64 = 2_000;

/// Per-chain configuration bundling all parameters needed by the indexer.
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub name: String,
    pub chain_id: i32,
    pub ws_url: String,
    pub contracts: Vec<Address>,
}

/// Errors from a single indexer session.
#[derive(Debug)]
pub enum IndexerError {
    /// Recoverable: WS disconnect, connection failure, subscription failure.
    Recoverable(String),
    /// Fatal: DB errors, irrecoverable failures.
    Fatal(String),
}

impl std::fmt::Display for IndexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexerError::Recoverable(msg) => write!(f, "recoverable: {msg}"),
            IndexerError::Fatal(msg) => write!(f, "fatal: {msg}"),
        }
    }
}

/// Outer reconnection loop. Contains backoff state and calls
/// `run_session` in a loop, applying exponential backoff on failure.
/// Returns `Result<(), IndexerError>` so it can be spawned as a Tokio task.
pub async fn run_indexer(pool: PgPool, config: ChainConfig, tx: Sender<NormalizedEvent>) -> Result<(), IndexerError> {
    let span = tracing::info_span!("chain", name = %config.name, chain_id = config.chain_id);

    async {
        let initial_backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(60);
        let mut backoff = initial_backoff;
        let mut retry_count: u32 = 0;

        loop {
            // Log reconnect attempt only for actual retries (not the first connection attempt)
            if retry_count > 0 {
                tracing::warn!(
                    retry_count = retry_count,
                    backoff_secs = backoff.as_secs(),
                    "Reconnecting..."
                );
            }

            match run_session(&pool, &config.ws_url, config.contracts.clone(), config.chain_id, retry_count, &tx).await {
                Ok(()) => {
                    // Stream ended normally (WS disconnect) — treat as recoverable
                    tracing::warn!(
                        retry_count = retry_count,
                        "WebSocket disconnected, will reconnect"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                    retry_count += 1;
                }
                Err(IndexerError::Recoverable(msg)) => {
                    tracing::warn!(
                        retry_count = retry_count,
                        backoff_secs = backoff.as_secs(),
                        error = %msg,
                        "Reconnection failed"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                    retry_count += 1;
                }
                Err(IndexerError::Fatal(msg)) => {
                    tracing::error!(error = %msg, "Fatal error, exiting");
                    return Err(IndexerError::Fatal(msg));
                }
            }
        }
    }
    .instrument(span)
    .await
}

/// Single connection session: connect WS, run catch-up, subscribe to live logs,
/// and process events until stream ends. Returns `Ok(())` when stream ends
/// (triggering reconnect) or `Err` on error.
pub async fn run_session(
    pool: &PgPool,
    ws_url: &str,
    contracts: Vec<Address>,
    chain_id: i32,
    retry_count: u32,
    tx: &Sender<NormalizedEvent>,
) -> Result<(), IndexerError> {
    // Connect WebSocket
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new()
        .connect_ws(ws)
        .await
        .map_err(|e| IndexerError::Recoverable(format!("WebSocket connect failed: {e}")))?;

    tracing::info!(ws_url = %ws_url, "WebSocket connected");

    // Log successful reconnect when this is a retry (not the initial connection)
    if retry_count > 0 {
        tracing::info!(retry_count = retry_count, "Reconnected successfully");
    }

    // Run catch-up before subscribing to live logs
    catch_up(&provider, pool, &contracts, chain_id, tx).await?;

    // Determine the starting block for the live subscription.
    // If we have a last indexed block from DB (set by catch_up or prior runs),
    // start from there + 1 to avoid a gap. Otherwise fall back to Latest.
    let live_from_block = match chainpulse_db::get_last_indexed_block(pool, chain_id)
        .await
        .map_err(|e| IndexerError::Fatal(format!("DB error reading last indexed block: {e}")))?
    {
        Some(last) => {
            let from = (last as u64).saturating_add(1);
            tracing::info!(
                from_block = from,
                "Starting live subscription from block"
            );
            BlockNumberOrTag::Number(from)
        }
        None => {
            tracing::info!("Starting live subscription from latest block");
            BlockNumberOrTag::Latest
        }
    };

    // Subscribe to live logs
    let num_contracts = contracts.len();
    let filter = Filter::new()
        .address(contracts)
        .from_block(live_from_block);

    let sub = provider
        .subscribe_logs(&filter)
        .await
        .map_err(|e| IndexerError::Recoverable(format!("Failed to subscribe to logs: {e}")))?;

    let mut stream = sub.into_stream();

    tracing::info!(
        contract_count = num_contracts,
        "Subscribed to logs for {} contracts",
        num_contracts
    );

    // Event processing loop -- send events to bounded channel with backpressure detection
    while let Some(log) = stream.next().await {
        if let Some(event) = chainpulse_types::decode_log(&log, chain_id) {
            tracing::info!(
                event_type = %event.event_type,
                block_number = event.block_number,
                contract_address = %event.contract_address,
                "Sending event to channel"
            );
            backpressure_send(tx, event, chain_id).await?;
        } else {
            tracing::debug!("Skipping unknown event");
        }
    }

    // Stream ended normally (WS disconnected)
    Ok(())
}

/// Detect the gap between `last_indexed_block` and the current chain head,
/// then fetch and process all missed events via chunked `eth_getLogs` calls.
///
/// - If no `last_indexed_block` exists (fresh DB): skip and log INFO.
/// - If already up to date: skip and log INFO.
/// - Otherwise: fetch missed logs in `CATCH_UP_CHUNK_SIZE`-block chunks,
///   processing each through `decode_log` + `insert_event`, and update
///   `last_indexed_block` after each chunk.
async fn catch_up(
    provider: &impl Provider,
    pool: &PgPool,
    contracts: &[Address],
    chain_id: i32,
    tx: &Sender<NormalizedEvent>,
) -> Result<(), IndexerError> {
    let last_indexed = chainpulse_db::get_last_indexed_block(pool, chain_id)
        .await
        .map_err(|e| IndexerError::Fatal(format!("DB error reading last indexed block: {e}")))?;

    let Some(last_block) = last_indexed else {
        tracing::info!("No previous block, starting from latest");
        return Ok(());
    };

    let head = provider
        .get_block_number()
        .await
        .map_err(|e| IndexerError::Recoverable(format!("Failed to get block number: {e}")))?;

    if last_block as u64 >= head {
        tracing::info!(last_block = last_block, "Already up to date");
        return Ok(());
    }

    let from_block = (last_block as u64).saturating_add(1);
    let to_block = head;
    let gap_blocks = to_block.saturating_sub(from_block).saturating_add(1);

    tracing::info!(
        from_block = from_block,
        to_block = to_block,
        gap_blocks = gap_blocks,
        "Starting catch-up indexing"
    );

    let mut total_events: u64 = 0;
    let mut chunk_from = from_block;

    while chunk_from <= to_block {
        let chunk_to = std::cmp::min(
            chunk_from.saturating_add(CATCH_UP_CHUNK_SIZE).saturating_sub(1),
            to_block,
        );

        let chunk_events =
            fetch_logs_chunk(provider, pool, contracts, chain_id, chunk_from, chunk_to, tx).await?;

        total_events = total_events.saturating_add(chunk_events);
        chunk_from = chunk_to.saturating_add(1);
    }

    tracing::info!(
        total_blocks = gap_blocks,
        total_events = total_events,
        "Catch-up complete"
    );

    Ok(())
}

/// Fetch one chunk of logs via `eth_getLogs` for the block range `[from, to]`,
/// decode and insert each log, then update `last_indexed_block` to `to`.
///
/// Returns the number of events inserted in this chunk.
/// If `get_logs` fails, returns a `Recoverable` error so the reconnection loop
/// can retry from the last completed chunk.
async fn fetch_logs_chunk(
    provider: &impl Provider,
    _pool: &PgPool,
    contracts: &[Address],
    chain_id: i32,
    from: u64,
    to: u64,
    tx: &Sender<NormalizedEvent>,
) -> Result<u64, IndexerError> {
    let filter = Filter::new()
        .address(contracts.to_vec())
        .from_block(BlockNumberOrTag::Number(from))
        .to_block(BlockNumberOrTag::Number(to));

    let logs = provider
        .get_logs(&filter)
        .await
        .map_err(|e| IndexerError::Recoverable(format!("eth_getLogs failed for blocks {from}-{to}: {e}")))?;

    let mut events_count: u64 = 0;

    for log in &logs {
        if let Some(event) = chainpulse_types::decode_log(log, chain_id) {
            backpressure_send(tx, event, chain_id).await?;
            events_count = events_count.saturating_add(1);
        }
    }

    // Block tracking is handled by the DB writer task via batch_upsert_last_indexed_blocks.

    tracing::info!(
        chunk_from = from,
        chunk_to = to,
        events_count = events_count,
        "Catch-up chunk processed"
    );

    Ok(events_count)
}

/// Send an event to the bounded channel with backpressure detection and logging.
///
/// Uses `try_send` first to detect whether the channel is full. If the channel
/// is at capacity, logs a warning and falls back to the blocking `send().await`,
/// which applies backpressure by suspending the calling task until space is available.
/// Also instruments the total send duration so operators can observe backpressure latency.
async fn backpressure_send(
    tx: &Sender<NormalizedEvent>,
    event: NormalizedEvent,
    chain_id: i32,
) -> Result<(), IndexerError> {
    match tx.try_send(event) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(event)) => {
            tracing::warn!(
                chain_id = chain_id,
                "Channel full, applying backpressure — subscriber will block until space is available"
            );
            let start = Instant::now();
            tx.send(event).await.map_err(|e| {
                IndexerError::Fatal(format!(
                    "Channel send failed (DB writer crashed?): {e}"
                ))
            })?;
            let wait_ms = start.elapsed().as_millis();
            tracing::info!(
                chain_id = chain_id,
                backpressure_wait_ms = wait_ms,
                "Backpressure relieved, send completed"
            );
            Ok(())
        }
        Err(TrySendError::Closed(_)) => {
            Err(IndexerError::Fatal(
                "Channel closed (DB writer crashed?)".to_string(),
            ))
        }
    }
}

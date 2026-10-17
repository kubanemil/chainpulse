use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::B256;
use alloy::providers::{Provider, RootProvider};
use alloy::rpc::types::Log;
use alloy::transports::TransportError;
use chainpulse_config::ChainConfig;
use chainpulse_db::{self as db, Commit, DbError, PgPool};
use chainpulse_types::{Block, BlockRef};
use futures_util::{StreamExt, TryStreamExt, stream};
use tokio::sync::{broadcast, watch};
use tokio_util::sync::CancellationToken;

use crate::logs::{self, Inconsistent, Watched};
use crate::reorg::{self, Broken, Plan};
use crate::status::{ChainStatus, Progress};
use crate::telemetry;
use crate::ws::{Notification, Reorg};

const RPC_CONCURRENCY: usize = 8;
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("rpc: {0}")]
    Rpc(#[from] TransportError),
    #[error("node returned inconsistent data: {0}")]
    Inconsistent(#[from] Inconsistent),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("node serves chain id {actual}, config says {expected}")]
    WrongChain { expected: u64, actual: u64 },
    #[error(
        "reorg on {chain} is deeper than its {confirmations} confirmations: no stored block is still canonical"
    )]
    ReorgTooDeep { chain: String, confirmations: u64 },
}

impl IndexError {
    /// Node trouble is retried with backoff. Anything else stops the process.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Rpc(_) | Self::Inconsistent(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Idle,
    Advanced {
        to: u64,
        events: usize,
    },
    RolledBack {
        from: u64,
        to: u64,
        removed_events: u64,
    },
}

pub async fn connect(rpc_url: &str) -> Result<RootProvider, TransportError> {
    RootProvider::connect(rpc_url).await
}

/// Indexes one chain. Each step polls the head, fetches what is new, checks it links onto the
/// stored chain, and commits blocks, events and cursor in one transaction.
pub struct Indexer {
    chain: ChainConfig,
    provider: RootProvider,
    pool: PgPool,
    watched: Watched,
    feed: broadcast::Sender<Notification>,
    progress: watch::Sender<Progress>,
    cursor: Option<BlockRef>,
    chain_verified: bool,
    /// Block range per `eth_getLogs` call. Halves when the node rejects a range, grows back on success.
    span: u64,
}

impl Indexer {
    pub async fn new(
        chain: ChainConfig,
        provider: RootProvider,
        pool: PgPool,
        feed: broadcast::Sender<Notification>,
    ) -> Result<Self, IndexError> {
        let cursor = db::load_cursor(&pool, chain.id).await?;
        let (progress, _) = watch::channel(Progress {
            indexed: cursor.map(|c| c.number),
            ..Progress::default()
        });
        Ok(Self {
            watched: Watched::new(&chain.contracts),
            span: chain.max_block_range,
            chain,
            provider,
            pool,
            feed,
            progress,
            cursor,
            chain_verified: false,
        })
    }

    pub fn status(&self) -> ChainStatus {
        ChainStatus {
            id: self.chain.id,
            name: self.chain.name.clone(),
            max_lag_blocks: self.chain.max_lag_blocks,
            stale_after: (self.chain.poll_interval() * 10).max(Duration::from_secs(30)),
            progress: self.progress.subscribe(),
        }
    }

    pub fn cursor(&self) -> Option<BlockRef> {
        self.cursor
    }

    /// Runs until `shutdown` fires. Returns an error only for failures retrying cannot fix.
    pub async fn run(mut self, shutdown: CancellationToken) -> Result<(), IndexError> {
        let mut failures: u32 = 0;
        loop {
            let step = tokio::select! {
                () = shutdown.cancelled() => return Ok(()),
                step = self.step() => step,
            };
            let pause = match step {
                Ok(step) => {
                    failures = 0;
                    if step == Step::Idle {
                        self.chain.poll_interval()
                    } else {
                        Duration::ZERO
                    }
                }
                Err(err) if err.is_transient() => {
                    telemetry::rpc_error(&self.chain.name);
                    let delay = backoff(failures);
                    failures = failures.saturating_add(1);
                    tracing::warn!(chain = %self.chain.name, error = %err, retry_in = ?delay, "step failed");
                    delay
                }
                Err(err) => return Err(err),
            };
            if !pause.is_zero() {
                tokio::select! {
                    () = shutdown.cancelled() => return Ok(()),
                    () = tokio::time::sleep(pause) => {}
                }
            }
        }
    }

    pub async fn step(&mut self) -> Result<Step, IndexError> {
        if !self.chain_verified {
            let actual = self.provider.get_chain_id().await?;
            if actual != self.chain.id {
                return Err(IndexError::WrongChain {
                    expected: self.chain.id,
                    actual,
                });
            }
            self.chain_verified = true;
        }

        let head = self.provider.get_block_number().await?;
        telemetry::head(&self.chain.name, head);
        self.progress.send_modify(|p| {
            p.head = Some(head);
            p.polled_at = Some(Instant::now());
        });

        let cursor = self.cursor.map(|c| c.number);
        let plan = reorg::plan(
            cursor,
            self.chain.start_block,
            head,
            self.chain.confirmations,
            self.span,
        );
        match plan {
            Plan::Wait => Ok(Step::Idle),
            Plan::Verify(number) => self.verify(number).await,
            Plan::Fetch { from, to, dense } => self.advance(from, to, dense, head).await,
        }
    }

    /// Nothing new to index; a reorg may still have replaced blocks at or below the head.
    async fn verify(&mut self, number: u64) -> Result<Step, IndexError> {
        let stored = match self.cursor {
            Some(cursor) if cursor.number == number => Some(cursor.hash),
            _ => db::block_hash(&self.pool, self.chain.id, number).await?,
        };
        let Some(stored) = stored else {
            return Ok(Step::Idle);
        };
        if self.block(number).await?.hash == stored {
            Ok(Step::Idle)
        } else {
            self.roll_back().await
        }
    }

    async fn advance(
        &mut self,
        from: u64,
        to: u64,
        dense: bool,
        head: u64,
    ) -> Result<Step, IndexError> {
        let (blocks, logs) = if dense {
            let blocks = self.blocks(from..=to).await?;
            let logs = self.logs_by_block(&blocks).await?;
            (blocks, logs)
        } else {
            let logs = self.logs_in_range(from, to).await?;
            let mut numbers: BTreeSet<u64> =
                logs.iter().filter_map(|log| log.block_number).collect();
            numbers.extend([from, to]);
            (self.blocks(numbers).await?, logs)
        };

        match reorg::check_links(self.cursor, &blocks) {
            Ok(()) => {}
            Err(Broken::Cursor) => return self.roll_back().await,
            Err(Broken::Within(number)) => {
                return Err(
                    Inconsistent(format!("block {number} does not link to its parent")).into(),
                );
            }
        }
        let events = logs::assemble(self.chain.id, &blocks, &logs, &self.watched)?;
        let Some(tip) = blocks.last().map(Block::to_ref) else {
            return Err(Inconsistent("no blocks fetched".to_owned()).into());
        };
        let confirmed_block = head.saturating_sub(self.chain.confirmations);

        let records = db::commit(
            &self.pool,
            &Commit {
                chain_id: self.chain.id,
                parent: self.cursor,
                cursor: tip,
                blocks: &blocks,
                events: &events,
                confirmed_block,
                prune_below: tip.number.min(confirmed_block),
            },
        )
        .await?;
        self.cursor = Some(tip);

        let mut per_type: HashMap<&str, u64> = HashMap::new();
        for event in &events {
            *per_type.entry(event.kind.name()).or_default() += 1;
        }
        for (event_type, count) in per_type {
            telemetry::events_indexed(&self.chain.name, event_type, count);
        }
        telemetry::progress(&self.chain.name, head, tip.number);
        self.progress.send_modify(|p| p.indexed = Some(tip.number));
        tracing::debug!(chain = %self.chain.name, from, to, events = records.len(), "indexed");

        let events = records.len();
        for record in records {
            let _ = self.feed.send(Notification::Event(Arc::new(record)));
        }
        Ok(Step::Advanced {
            to: tip.number,
            events,
        })
    }

    /// The cursor is no longer canonical. Walk stored blocks from newest to oldest until the node
    /// agrees with one, then drop everything above it.
    async fn roll_back(&mut self) -> Result<Step, IndexError> {
        let Some(cursor) = self.cursor else {
            return Err(
                Inconsistent("reorg reported before anything was indexed".to_owned()).into(),
            );
        };
        let stored = db::recent_blocks(&self.pool, self.chain.id).await?;
        for chunk in stored.chunks(RPC_CONCURRENCY) {
            let canonical =
                concurrently(chunk.iter().map(|block| self.canonical_hash(block.number))).await?;
            let Some(ancestor) = reorg::common_ancestor(chunk, &canonical) else {
                continue;
            };
            if ancestor == cursor {
                return Err(
                    Inconsistent("node reported a reorg it no longer shows".to_owned()).into(),
                );
            }

            let removed_events = db::rollback(&self.pool, self.chain.id, cursor, ancestor).await?;
            let depth = cursor.number - ancestor.number;
            self.cursor = Some(ancestor);
            telemetry::reorg(&self.chain.name, depth);
            self.progress
                .send_modify(|p| p.indexed = Some(ancestor.number));
            tracing::warn!(
                chain = %self.chain.name, from = cursor.number, to = ancestor.number, depth, removed_events,
                "reorg: rolled back to common ancestor"
            );
            let _ = self.feed.send(Notification::Reorg(Reorg {
                chain_id: self.chain.id,
                common_ancestor: ancestor.number,
                depth,
                removed_events,
            }));
            return Ok(Step::RolledBack {
                from: cursor.number,
                to: ancestor.number,
                removed_events,
            });
        }
        Err(IndexError::ReorgTooDeep {
            chain: self.chain.name.clone(),
            confirmations: self.chain.confirmations,
        })
    }

    async fn logs_in_range(&mut self, from: u64, to: u64) -> Result<Vec<Log>, IndexError> {
        let filter = self.watched.filter().from_block(from).to_block(to);
        match self.provider.get_logs(&filter).await {
            Ok(logs) => {
                self.span = self.span.saturating_mul(2).min(self.chain.max_block_range);
                Ok(logs)
            }
            Err(err) => {
                // Providers cap results per call; a smaller range is the usual fix.
                self.span = (self.span / 2).max(1);
                Err(err.into())
            }
        }
    }

    /// Fetching by block hash ties the logs to exactly the header we linked.
    async fn logs_by_block(&self, blocks: &[Block]) -> Result<Vec<Log>, IndexError> {
        let per_block = concurrently(blocks.iter().map(|block| {
            let filter = self.watched.filter().at_block_hash(block.hash);
            async move {
                self.provider
                    .get_logs(&filter)
                    .await
                    .map_err(IndexError::from)
            }
        }))
        .await?;
        Ok(per_block.into_iter().flatten().collect())
    }

    async fn blocks(
        &self,
        numbers: impl IntoIterator<Item = u64>,
    ) -> Result<Vec<Block>, IndexError> {
        concurrently(numbers.into_iter().map(|number| self.block(number))).await
    }

    async fn block(&self, number: u64) -> Result<Block, IndexError> {
        let header = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .await?
            .filter(|block| block.header.number == number)
            .ok_or_else(|| Inconsistent(format!("node has no block {number}")))?
            .header;
        Ok(Block {
            number,
            hash: header.hash,
            parent_hash: header.parent_hash,
            timestamp: header.timestamp,
        })
    }

    async fn canonical_hash(&self, number: u64) -> Result<Option<B256>, IndexError> {
        let block = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .await?;
        Ok(block.map(|block| block.header.hash))
    }
}

/// Runs up to `RPC_CONCURRENCY` requests at once, keeping results in order.
async fn concurrently<T, F>(requests: impl Iterator<Item = F>) -> Result<Vec<T>, IndexError>
where
    F: Future<Output = Result<T, IndexError>>,
{
    let requests: Vec<F> = requests.collect();
    stream::iter(requests)
        .buffered(RPC_CONCURRENCY)
        .try_collect()
        .await
}

fn backoff(failures: u32) -> Duration {
    Duration::from_millis(500)
        .saturating_mul(2u32.saturating_pow(failures))
        .min(MAX_BACKOFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff(0), Duration::from_millis(500));
        assert_eq!(backoff(3), Duration::from_secs(4));
        assert_eq!(backoff(40), MAX_BACKOFF);
    }
}

use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::watch;

/// What an indexer last saw, published for `/ready`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Progress {
    pub head: Option<u64>,
    pub indexed: Option<u64>,
    pub polled_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct ChainStatus {
    pub id: u64,
    pub name: String,
    pub max_lag_blocks: u64,
    /// Without a successful poll for this long the chain is not ready, whatever its lag.
    pub stale_after: Duration,
    pub progress: watch::Receiver<Progress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainReport {
    pub id: u64,
    pub name: String,
    pub ready: bool,
    pub head: Option<u64>,
    pub indexed: Option<u64>,
    pub lag: Option<u64>,
    pub seconds_since_poll: Option<u64>,
}

impl ChainStatus {
    pub fn report(&self, now: Instant) -> ChainReport {
        let progress = *self.progress.borrow();
        let lag = progress
            .head
            .zip(progress.indexed)
            .map(|(head, indexed)| head.saturating_sub(indexed));
        let since_poll = progress
            .polled_at
            .map(|at| now.saturating_duration_since(at));
        let fresh = since_poll.is_some_and(|age| age <= self.stale_after);
        ChainReport {
            id: self.id,
            name: self.name.clone(),
            ready: fresh && lag.is_some_and(|lag| lag <= self.max_lag_blocks),
            head: progress.head,
            indexed: progress.indexed,
            lag,
            seconds_since_poll: since_poll.map(|age| age.as_secs()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(progress: Progress) -> ChainStatus {
        let (_, progress) = watch::channel(progress);
        ChainStatus {
            id: 1,
            name: "ethereum".into(),
            max_lag_blocks: 5,
            stale_after: Duration::from_secs(30),
            progress,
        }
    }

    #[test]
    fn is_ready_when_recently_polled_and_within_the_lag_limit() {
        let now = Instant::now();
        let report = status(Progress {
            head: Some(105),
            indexed: Some(100),
            polled_at: Some(now),
        })
        .report(now);

        assert!(report.ready);
        assert_eq!(report.lag, Some(5));
    }

    #[test]
    fn is_not_ready_while_lagging() {
        let now = Instant::now();
        let report = status(Progress {
            head: Some(106),
            indexed: Some(100),
            polled_at: Some(now),
        })
        .report(now);

        assert!(!report.ready);
    }

    #[test]
    fn is_not_ready_when_the_node_stopped_answering() {
        let polled = Instant::now();
        let progress = Progress {
            head: Some(100),
            indexed: Some(100),
            polled_at: Some(polled),
        };

        let report = status(progress).report(polled + Duration::from_secs(31));

        assert!(!report.ready);
        assert_eq!(report.seconds_since_poll, Some(31));
    }

    #[test]
    fn is_not_ready_before_anything_is_indexed() {
        let now = Instant::now();
        let report = status(Progress {
            head: Some(100),
            indexed: None,
            polled_at: Some(now),
        })
        .report(now);

        assert!(!report.ready);
        assert_eq!(report.lag, None);
    }
}

use std::time::Duration;

use alloy::eips::BlockNumberOrTag;
use alloy::providers::{Provider, RootProvider, WsConnect};
use alloy::rpc::client::ClientBuilder;
use alloy::rpc::types::{Filter, Log};
use alloy::transports::{RpcError, TransportError, TransportErrorKind};
use chainpulse_types::Block;

// Short so a dead socket surfaces as BackendGone quickly; the indexer then rebuilds the client.
const WS_RETRIES: u32 = 3;
const WS_RETRY_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)]
    Rpc(#[from] TransportError),
    #[error("no response within {0:?}")]
    Timeout(Duration),
}

impl NodeError {
    /// The WebSocket client gave up reconnecting; every later call on it fails.
    pub fn is_backend_gone(&self) -> bool {
        matches!(
            self,
            Self::Rpc(RpcError::Transport(TransportErrorKind::BackendGone))
        )
    }

    /// Providers cap `eth_getLogs` by block range or result count, each with its own wording.
    pub fn is_range_too_large(&self) -> bool {
        let text = match self {
            Self::Rpc(RpcError::ErrorResp(payload)) => payload.message.to_lowercase(),
            Self::Rpc(RpcError::Transport(TransportErrorKind::HttpError(http)))
                if (400..500).contains(&http.status) && http.status != 429 =>
            {
                http.body.to_lowercase()
            }
            _ => return false,
        };
        ["range", "results", "response size", "too large"]
            .iter()
            .any(|hint| text.contains(hint))
    }
}

pub struct Node {
    url: String,
    timeout: Duration,
    provider: RootProvider,
}

impl Node {
    pub async fn connect(url: &str, timeout: Duration) -> Result<Self, NodeError> {
        let provider = open(url, timeout).await?;
        Ok(Self {
            url: url.to_owned(),
            timeout,
            provider,
        })
    }

    pub async fn reconnect(&mut self) -> Result<(), NodeError> {
        self.provider = open(&self.url, self.timeout).await?;
        Ok(())
    }

    pub async fn chain_id(&self) -> Result<u64, NodeError> {
        self.call(self.provider.get_chain_id()).await
    }

    pub async fn block_number(&self) -> Result<u64, NodeError> {
        self.call(self.provider.get_block_number()).await
    }

    pub async fn block(&self, number: u64) -> Result<Option<Block>, NodeError> {
        let block = self
            .call(
                self.provider
                    .get_block_by_number(BlockNumberOrTag::Number(number)),
            )
            .await?;
        Ok(block.map(|block| Block {
            number: block.header.number,
            hash: block.header.hash,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        }))
    }

    pub async fn logs(&self, filter: &Filter) -> Result<Vec<Log>, NodeError> {
        self.call(self.provider.get_logs(filter)).await
    }

    async fn call<T>(
        &self,
        request: impl IntoFuture<Output = Result<T, TransportError>>,
    ) -> Result<T, NodeError> {
        tokio::time::timeout(self.timeout, request)
            .await
            .map_err(|_| NodeError::Timeout(self.timeout))?
            .map_err(NodeError::Rpc)
    }
}

async fn open(url: &str, timeout: Duration) -> Result<RootProvider, NodeError> {
    let connect = async {
        if url.starts_with("ws://") || url.starts_with("wss://") {
            let ws = WsConnect::new(url)
                .with_max_retries(WS_RETRIES)
                .with_retry_interval(WS_RETRY_INTERVAL);
            ClientBuilder::default().ws(ws).await.map(RootProvider::new)
        } else {
            RootProvider::connect(url).await
        }
    };
    tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| NodeError::Timeout(timeout))?
        .map_err(NodeError::Rpc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::json_rpc::ErrorPayload;
    use std::time::Instant;
    use tokio::net::TcpListener;

    fn error_response(message: &str) -> NodeError {
        NodeError::Rpc(RpcError::ErrorResp(ErrorPayload {
            code: -32005,
            message: message.to_owned().into(),
            data: None,
        }))
    }

    fn http_error(status: u16, body: &str) -> NodeError {
        NodeError::Rpc(TransportErrorKind::http_error(status, body.to_owned()))
    }

    #[test]
    fn recognises_range_and_size_rejections_from_common_providers() {
        for message in [
            "query returned more than 10000 results",
            "Log response size exceeded. You can make eth_getLogs requests with up to a 2K block range",
            "eth_getLogs is limited to a 10,000 range",
            "query exceeds max results 20000",
        ] {
            assert!(error_response(message).is_range_too_large(), "{message}");
        }
        let drpc = r#"{"error":{"message":"ranges over 10000 blocks are not supported on free plan","code":35}}"#;
        assert!(http_error(400, drpc).is_range_too_large());
    }

    #[test]
    fn does_not_shrink_the_range_for_rate_limits_or_outages() {
        assert!(
            !http_error(429, "too many requests, retry the block range later").is_range_too_large()
        );
        assert!(!http_error(503, "service unavailable").is_range_too_large());
        assert!(!error_response("rate limit exceeded").is_range_too_large());
        assert!(!NodeError::Timeout(Duration::from_secs(1)).is_range_too_large());
    }

    #[test]
    fn recognises_a_websocket_client_that_gave_up() {
        assert!(NodeError::Rpc(TransportErrorKind::backend_gone()).is_backend_gone());
        assert!(!http_error(502, "bad gateway").is_backend_gone());
    }

    #[tokio::test]
    async fn a_node_that_never_answers_times_out() {
        let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", silent.local_addr().unwrap());
        let _accepting = tokio::spawn(async move {
            let mut open = Vec::new();
            while let Ok((socket, _)) = silent.accept().await {
                open.push(socket);
            }
        });
        let node = Node::connect(&url, Duration::from_millis(200))
            .await
            .unwrap();
        let started = Instant::now();

        let err = node.block_number().await.unwrap_err();

        assert!(matches!(err, NodeError::Timeout(_)), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

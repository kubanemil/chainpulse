#![allow(dead_code, clippy::expect_used)]

use std::time::Duration;

use alloy::eips::BlockNumberOrTag;
use alloy::node_bindings::{Anvil, AnvilInstance};
use alloy::primitives::{Address, U256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use chainpulse::indexer::{Indexer, Step};
use chainpulse::node::Node;
use chainpulse::ws::Notification;
use chainpulse_config::{ChainConfig, ContractConfig};
use chainpulse_db::PgPool;
use chainpulse_types::{BlockRef, EventKind};
use tokio::sync::broadcast;

alloy::sol!(
    #[sol(rpc)]
    TestToken,
    "tests/contracts/TestToken.json"
);

pub const RECIPIENT: Address = Address::repeat_byte(0x42);

pub struct Chain {
    _anvil: AnvilInstance,
    pub chain_id: u64,
    pub rpc_url: String,
    pub provider: DynProvider,
    pub token: TestToken::TestTokenInstance<DynProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRow {
    pub block_number: i64,
    pub log_index: i32,
    pub block_hash: String,
    pub tx_hash: String,
    pub value: String,
    pub timestamp: i64,
}

impl Chain {
    pub async fn start() -> Self {
        let anvil = Anvil::new().spawn();
        let wallet = anvil.wallet().expect("anvil has dev accounts");
        // The default cached nonce goes stale when a test reverts the chain.
        let provider = ProviderBuilder::new()
            .disable_recommended_fillers()
            .with_gas_estimation()
            .with_simple_nonce_management()
            .fetch_chain_id()
            .wallet(wallet)
            .connect_http(anvil.endpoint_url())
            .erased();
        let token = TestToken::deploy(provider.clone(), U256::from(1_000_000u64))
            .await
            .expect("deploy token");
        Self {
            chain_id: anvil.chain_id(),
            rpc_url: anvil.endpoint(),
            _anvil: anvil,
            provider,
            token,
        }
    }

    pub fn config(&self, confirmations: u64) -> ChainConfig {
        chain_config(
            self.chain_id,
            &self.rpc_url,
            *self.token.address(),
            confirmations,
        )
    }

    pub async fn indexer(&self, pool: &PgPool, config: ChainConfig) -> Indexer {
        let (feed, _) = broadcast::channel(1_024);
        self.indexer_with_feed(pool, config, feed).await
    }

    pub async fn indexer_with_feed(
        &self,
        pool: &PgPool,
        config: ChainConfig,
        feed: broadcast::Sender<Notification>,
    ) -> Indexer {
        let node = Node::connect(&self.rpc_url, Duration::from_secs(5))
            .await
            .expect("connect to anvil");
        Indexer::new(config, node, pool.clone(), feed)
            .await
            .expect("create indexer")
    }

    /// Sends one transfer; anvil mines it into its own block.
    pub async fn transfer(&self, value: u64) -> u64 {
        let receipt = self
            .token
            .transfer(RECIPIENT, U256::from(value))
            .send()
            .await
            .expect("send transfer")
            .with_timeout(Some(Duration::from_secs(10)))
            .get_receipt()
            .await
            .expect("transfer receipt");
        receipt.block_number.expect("mined")
    }

    pub async fn mine(&self, blocks: u64) {
        self.provider
            .anvil_mine(Some(blocks), None)
            .await
            .expect("mine");
    }

    pub async fn snapshot(&self) -> U256 {
        self.provider.anvil_snapshot().await.expect("snapshot")
    }

    pub async fn revert(&self, snapshot: U256) {
        assert!(self.provider.anvil_revert(snapshot).await.expect("revert"));
    }

    pub async fn head(&self) -> BlockRef {
        let block = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await
            .expect("latest block")
            .expect("chain has a head");
        BlockRef {
            number: block.header.number,
            hash: block.header.hash,
        }
    }

    pub async fn transfers(&self) -> Vec<TransferRow> {
        let filter = Filter::new()
            .address(*self.token.address())
            .event_signature(TestToken::Transfer::SIGNATURE_HASH)
            .from_block(0)
            .to_block(BlockNumberOrTag::Latest);
        let logs = self.provider.get_logs(&filter).await.expect("get logs");
        let mut rows = Vec::new();
        for log in logs {
            let number = log.block_number.expect("mined log");
            let block = self
                .provider
                .get_block_by_number(number.into())
                .await
                .expect("block")
                .expect("block exists");
            let transfer =
                TestToken::Transfer::decode_log_data(&log.inner.data).expect("decode transfer");
            rows.push(TransferRow {
                block_number: number as i64,
                log_index: log.log_index.expect("mined log") as i32,
                block_hash: log.block_hash.expect("mined log").to_string(),
                tx_hash: log.transaction_hash.expect("mined log").to_string(),
                value: transfer.value.to_string(),
                timestamp: block.header.timestamp as i64,
            });
        }
        rows
    }
}

pub fn chain_config(id: u64, rpc_url: &str, token: Address, confirmations: u64) -> ChainConfig {
    ChainConfig {
        id,
        name: "anvil".into(),
        rpc_url: rpc_url.to_owned(),
        confirmations,
        start_block: Some(0),
        poll_interval_ms: 50,
        rpc_timeout_ms: 1_000,
        max_block_range: 1_000,
        max_lag_blocks: 5,
        contracts: vec![ContractConfig {
            name: "token".into(),
            address: token,
            events: vec![EventKind::Transfer],
        }],
    }
}

pub async fn migrated(pool: &PgPool) {
    chainpulse_db::migrate(pool).await.expect("migrate");
}

pub async fn stored_transfers(pool: &PgPool) -> Vec<TransferRow> {
    let rows: Vec<(i64, i32, String, String, String, i64)> = sqlx::query_as(
        "SELECT block_number, log_index, block_hash, tx_hash, payload->>'value',
                extract(epoch FROM block_timestamp)::bigint
         FROM events ORDER BY block_number, log_index",
    )
    .fetch_all(pool)
    .await
    .expect("read events");
    rows.into_iter()
        .map(
            |(block_number, log_index, block_hash, tx_hash, value, timestamp)| TransferRow {
                block_number,
                log_index,
                block_hash,
                tx_hash,
                value,
                timestamp,
            },
        )
        .collect()
}

pub async fn stored_cursor(pool: &PgPool) -> (i64, String) {
    sqlx::query_as("SELECT block_number, block_hash FROM chain_cursors")
        .fetch_one(pool)
        .await
        .expect("read cursor")
}

pub async fn sync(indexer: &mut Indexer) -> Vec<Step> {
    let mut steps = Vec::new();
    loop {
        let step = indexer.step().await.expect("indexer step");
        if step == Step::Idle {
            return steps;
        }
        steps.push(step);
        assert!(steps.len() < 100, "indexer did not settle: {steps:?}");
    }
}

pub async fn assert_matches_chain(pool: &PgPool, chain: &Chain) {
    assert_eq!(stored_transfers(pool).await, chain.transfers().await);
    let head = chain.head().await;
    assert_eq!(
        stored_cursor(pool).await,
        (head.number as i64, head.hash.to_string())
    );
}

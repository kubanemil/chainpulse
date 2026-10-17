//! Its own test binary because it installs the process-wide metrics recorder.
#![allow(clippy::expect_used)]

mod common;

use chainpulse_db::PgPool;
use common::{Chain, migrated, sync};

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn reports_lag_reorgs_and_rollback_depth(pool: PgPool) {
    let metrics = chainpulse::telemetry::install().expect("install recorder");
    migrated(&pool).await;
    let chain = Chain::start().await;
    let snapshot = chain.snapshot().await;
    chain.transfer(1).await;
    chain.transfer(2).await;
    let mut indexer = chain.indexer(&pool, chain.config(10)).await;
    sync(&mut indexer).await;

    chain.revert(snapshot).await;
    chain.mine(3).await;
    sync(&mut indexer).await;
    let rendered = metrics.render();

    for line in [
        "chainpulse_lag_blocks{chain=\"anvil\"} 0",
        "chainpulse_reorgs_total{chain=\"anvil\"} 1",
        "chainpulse_reorg_depth_blocks_bucket{chain=\"anvil\",le=\"1\"} 0",
        "chainpulse_reorg_depth_blocks_bucket{chain=\"anvil\",le=\"2\"} 1",
        "chainpulse_events_indexed_total{chain=\"anvil\",event_type=\"Transfer\"} 3",
    ] {
        assert!(rendered.contains(line), "missing `{line}` in:\n{rendered}");
    }
}

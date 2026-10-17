mod common;

use alloy::providers::ext::AnvilApi;
use alloy::rpc::types::anvil::ReorgOptions;
use chainpulse::indexer::{IndexError, Step};
use chainpulse_db::{DbError, EventQuery, PgPool};
use common::{Chain, assert_matches_chain, migrated, stored_cursor, stored_transfers, sync};

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn indexes_transfers_with_block_hashes_and_timestamps(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    for value in 1..=3 {
        chain.transfer(value).await;
    }
    let mut indexer = chain.indexer(&pool, chain.config(2)).await;

    sync(&mut indexer).await;

    assert_eq!(
        stored_transfers(&pool).await.len(),
        4,
        "mint plus three transfers"
    );
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn cursor_advances_over_blocks_without_events(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let mut indexer = chain.indexer(&pool, chain.config(2)).await;
    sync(&mut indexer).await;

    chain.mine(25).await;
    sync(&mut indexer).await;

    assert_eq!(indexer.cursor(), Some(chain.head().await));
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn restart_resumes_from_the_stored_cursor(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    chain.transfer(1).await;
    let mut first = chain.indexer(&pool, chain.config(2)).await;
    sync(&mut first).await;
    let stopped_at = first.cursor();
    drop(first);

    chain.transfer(2).await;
    chain.transfer(3).await;
    let mut second = chain.indexer(&pool, chain.config(2)).await;

    assert_eq!(second.cursor(), stopped_at);
    sync(&mut second).await;
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn reorg_onto_a_longer_fork_rolls_back_to_the_common_ancestor(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    chain.transfer(1).await;
    let ancestor = chain.head().await;
    let snapshot = chain.snapshot().await;
    chain.transfer(2).await;
    chain.transfer(3).await;
    let mut indexer = chain.indexer(&pool, chain.config(10)).await;
    sync(&mut indexer).await;
    let orphaned_tip = indexer.cursor().expect("indexed").number;

    chain.revert(snapshot).await;
    for value in [20, 30, 40] {
        chain.transfer(value).await;
    }
    let steps = sync(&mut indexer).await;

    assert_eq!(
        steps[0],
        Step::RolledBack {
            from: orphaned_tip,
            to: ancestor.number,
            removed_events: 2
        }
    );
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn reorg_at_the_same_height_is_caught_while_idle(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    chain.transfer(1).await;
    chain.transfer(2).await;
    let mut indexer = chain.indexer(&pool, chain.config(10)).await;
    sync(&mut indexer).await;
    let head = chain.head().await;

    chain
        .provider
        .anvil_reorg(ReorgOptions {
            depth: 2,
            tx_block_pairs: Vec::new(),
        })
        .await
        .expect("anvil_reorg");
    let steps = sync(&mut indexer).await;

    assert_eq!(chain.head().await.number, head.number);
    assert_ne!(chain.head().await.hash, head.hash);
    assert_eq!(
        steps[0],
        Step::RolledBack {
            from: head.number,
            to: head.number - 2,
            removed_events: 2
        }
    );
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn reorg_onto_a_shorter_fork_is_caught(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let snapshot = chain.snapshot().await;
    for value in 1..=3 {
        chain.transfer(value).await;
    }
    let mut indexer = chain.indexer(&pool, chain.config(10)).await;
    sync(&mut indexer).await;

    chain.revert(snapshot).await;
    chain.transfer(99).await;
    let steps = sync(&mut indexer).await;

    assert!(
        matches!(
            steps[0],
            Step::RolledBack {
                removed_events: 3,
                ..
            }
        ),
        "{steps:?}"
    );
    assert_matches_chain(&pool, &chain).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn events_become_confirmed_at_the_configured_depth(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let mut indexer = chain.indexer(&pool, chain.config(3)).await;
    let block = chain.transfer(7).await;
    let latest = async || {
        let query = EventQuery {
            from_block: Some(block),
            to_block: Some(block),
            limit: 10,
            ..EventQuery::default()
        };
        let events = chainpulse_db::list_events(&pool, &query)
            .await
            .expect("list events");
        assert_eq!(events.len(), 1);
        events[0].confirmed
    };

    sync(&mut indexer).await;
    assert!(!latest().await);

    chain.mine(2).await;
    sync(&mut indexer).await;
    assert!(
        !latest().await,
        "two blocks deep is not final with three confirmations"
    );

    chain.mine(1).await;
    sync(&mut indexer).await;
    assert!(latest().await);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn reorg_deeper_than_the_confirmations_stops_the_indexer(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let snapshot = chain.snapshot().await;
    for value in 1..=5 {
        chain.transfer(value).await;
    }
    let mut indexer = chain.indexer(&pool, chain.config(2)).await;
    sync(&mut indexer).await;
    let before = stored_transfers(&pool).await;

    chain.revert(snapshot).await;
    for value in 10..=15 {
        chain.transfer(value).await;
    }
    let err = loop {
        match indexer.step().await {
            Ok(Step::Idle) => panic!("indexer missed the reorg"),
            Ok(_) => {}
            Err(err) => break err,
        }
    };

    assert!(
        matches!(
            err,
            IndexError::ReorgTooDeep {
                confirmations: 2,
                ..
            }
        ),
        "{err}"
    );
    assert!(!err.is_transient());
    assert_eq!(
        stored_transfers(&pool).await,
        before,
        "nothing is written past a failed rollback"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn a_second_writer_for_the_same_chain_is_refused(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let mut first = chain.indexer(&pool, chain.config(2)).await;
    let mut second = chain.indexer(&pool, chain.config(2)).await;
    sync(&mut first).await;
    let cursor = stored_cursor(&pool).await;

    let err = second.step().await.expect_err("second writer must fail");

    assert!(
        matches!(err, IndexError::Db(DbError::CursorConflict(_))),
        "{err}"
    );
    assert!(!err.is_transient());
    assert_eq!(stored_cursor(&pool).await, cursor);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn refuses_a_node_serving_another_chain(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let mut config = chain.config(2);
    config.id = 1;
    let mut indexer = chain.indexer(&pool, config).await;

    let err = indexer.step().await.expect_err("wrong chain must fail");

    assert!(
        matches!(err, IndexError::WrongChain { expected: 1, .. }),
        "{err}"
    );
}

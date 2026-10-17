#![allow(clippy::expect_used)]

use chainpulse_db::{Commit, EventQuery, EventRecord, PgPool};
use chainpulse_types::primitives::{Address, B256};
use chainpulse_types::{Block, BlockRef, Event, EventKind};
use serde_json::json;

const CHAIN: u64 = 1;

fn hash(fork: u8, number: u64) -> B256 {
    let mut bytes = [fork; 32];
    bytes[24..].copy_from_slice(&number.to_be_bytes());
    B256::from(bytes)
}

fn block(number: u64, fork: u8) -> Block {
    Block {
        number,
        hash: hash(fork, number),
        parent_hash: hash(fork, number - 1),
        timestamp: 1_700_000_000 + number,
    }
}

fn event_in(block: &Block) -> Event {
    Event {
        chain_id: CHAIN,
        block_number: block.number,
        block_hash: block.hash,
        block_timestamp: block.timestamp,
        log_index: 0,
        tx_hash: B256::repeat_byte(0xee),
        contract: Address::repeat_byte(0x11),
        kind: EventKind::Transfer,
        payload: json!({ "value": "1" }),
    }
}

async fn commit(
    pool: &PgPool,
    parent: Option<BlockRef>,
    block: Block,
    confirmed_block: u64,
) -> Vec<EventRecord> {
    chainpulse_db::commit(
        pool,
        &Commit {
            chain_id: CHAIN,
            parent,
            cursor: block.to_ref(),
            blocks: &[block],
            events: &[event_in(&block)],
            confirmed_block,
        },
    )
    .await
    .expect("commit")
}

async fn served(pool: &PgPool, number: u64) -> bool {
    let query = EventQuery {
        from_block: Some(number as i64),
        to_block: Some(number as i64),
        limit: 1,
        ..EventQuery::default()
    };
    chainpulse_db::list_events(pool, &query)
        .await
        .expect("list")[0]
        .confirmed
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres: ./do itest"]
async fn a_lower_reported_height_does_not_unconfirm_streamed_events(pool: PgPool) {
    chainpulse_db::migrate(&pool).await.expect("migrate");
    let first = block(10, 0xa0);
    commit(&pool, None, first, 20).await;

    let streamed = commit(&pool, Some(first.to_ref()), block(11, 0xa0), 5).await;

    assert!(streamed[0].confirmed);
    assert_eq!(served(&pool, 11).await, streamed[0].confirmed);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres: ./do itest"]
async fn rollback_below_the_confirmed_height_lowers_it(pool: PgPool) {
    chainpulse_db::migrate(&pool).await.expect("migrate");
    let (eight, nine, ten) = (block(8, 0xa0), block(9, 0xa0), block(10, 0xa0));
    commit(&pool, None, eight, 0).await;
    commit(&pool, Some(eight.to_ref()), nine, 0).await;
    commit(&pool, Some(nine.to_ref()), ten, 9).await;

    chainpulse_db::rollback(&pool, CHAIN, ten.to_ref(), eight.to_ref())
        .await
        .expect("rollback");
    let replacement = commit(&pool, Some(eight.to_ref()), block(9, 0xb0), 3).await;

    assert!(served(&pool, 8).await, "the ancestor stays confirmed");
    assert!(!replacement[0].confirmed);
    assert!(!served(&pool, 9).await);
}

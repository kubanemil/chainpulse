use std::time::Duration;

use chainpulse_types::primitives::{Address, B256};
use chainpulse_types::{Block, BlockRef, Event, EventKind, hex};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, Postgres, Transaction};

pub use sqlx::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(
        "cursor for chain {0} changed underneath this writer; is another indexer using this database?"
    )]
    CursorConflict(u64),
    #[error("value out of range: {0}")]
    OutOfRange(String),
}

pub async fn connect(url: &str, max_connections: u32) -> Result<PgPool, DbError> {
    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(url)
        .await?)
}

pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    Ok(sqlx::migrate!().run(pool).await?)
}

pub async fn ping(pool: &PgPool) -> Result<(), DbError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

pub async fn load_cursor(pool: &PgPool, chain_id: u64) -> Result<Option<BlockRef>, DbError> {
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT block_number, block_hash FROM chain_cursors WHERE chain_id = $1")
            .bind(int(chain_id)?)
            .fetch_optional(pool)
            .await?;
    row.map(|(number, hash)| block_ref(number, &hash))
        .transpose()
}

/// Stored blocks, newest first. Rollback searches these for the common ancestor.
pub async fn recent_blocks(pool: &PgPool, chain_id: u64) -> Result<Vec<BlockRef>, DbError> {
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT number, hash FROM blocks WHERE chain_id = $1 ORDER BY number DESC")
            .bind(int(chain_id)?)
            .fetch_all(pool)
            .await?;
    rows.iter()
        .map(|(number, hash)| block_ref(*number, hash))
        .collect()
}

pub async fn block_hash(
    pool: &PgPool,
    chain_id: u64,
    number: u64,
) -> Result<Option<B256>, DbError> {
    let hash: Option<String> =
        sqlx::query_scalar("SELECT hash FROM blocks WHERE chain_id = $1 AND number = $2")
            .bind(int(chain_id)?)
            .bind(int(number)?)
            .fetch_optional(pool)
            .await?;
    hash.map(|hash| parse_hash(&hash)).transpose()
}

/// One indexing step, written atomically: the blocks, their events and the cursor move together.
#[derive(Debug)]
pub struct Commit<'a> {
    pub chain_id: u64,
    /// The cursor this step was built on. The write fails if the stored cursor differs.
    pub parent: Option<BlockRef>,
    pub cursor: BlockRef,
    pub blocks: &'a [Block],
    pub events: &'a [Event],
    /// Highest block that is final. Never moves backwards.
    pub confirmed_block: u64,
    /// Blocks below this height are no longer needed to resolve a reorg.
    pub prune_below: u64,
}

pub async fn commit(pool: &PgPool, commit: &Commit<'_>) -> Result<Vec<EventRecord>, DbError> {
    let chain_id = int(commit.chain_id)?;
    let confirmed_block = int(commit.confirmed_block)?;
    let mut tx = pool.begin().await?;

    let moved =
        match commit.parent {
            Some(parent) => {
                sqlx::query(
                    "UPDATE chain_cursors
                 SET block_number = $2, block_hash = $3,
                     confirmed_block = GREATEST(confirmed_block, $4), updated_at = now()
                 WHERE chain_id = $1 AND block_number = $5 AND block_hash = $6",
                )
                .bind(chain_id)
                .bind(int(commit.cursor.number)?)
                .bind(commit.cursor.hash.to_string())
                .bind(confirmed_block)
                .bind(int(parent.number)?)
                .bind(parent.hash.to_string())
                .execute(&mut *tx)
                .await?
            }
            None => sqlx::query(
                "INSERT INTO chain_cursors (chain_id, block_number, block_hash, confirmed_block)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (chain_id) DO NOTHING",
            )
            .bind(chain_id)
            .bind(int(commit.cursor.number)?)
            .bind(commit.cursor.hash.to_string())
            .bind(confirmed_block)
            .execute(&mut *tx)
            .await?,
        };
    if moved.rows_affected() != 1 {
        return Err(DbError::CursorConflict(commit.chain_id));
    }

    insert_blocks(&mut tx, chain_id, commit.blocks).await?;
    sqlx::query("DELETE FROM blocks WHERE chain_id = $1 AND number < $2")
        .bind(chain_id)
        .bind(int(commit.prune_below)?)
        .execute(&mut *tx)
        .await?;
    let mut records = insert_events(&mut tx, chain_id, commit.events, confirmed_block).await?;

    tx.commit().await?;
    records.sort_by_key(|r| (r.block_number, r.log_index));
    Ok(records)
}

async fn insert_blocks(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: i64,
    blocks: &[Block],
) -> Result<(), DbError> {
    let mut numbers = Vec::with_capacity(blocks.len());
    let mut timestamps = Vec::with_capacity(blocks.len());
    for block in blocks {
        numbers.push(int(block.number)?);
        timestamps.push(timestamp(block.timestamp)?);
    }
    let hashes: Vec<String> = blocks.iter().map(|b| b.hash.to_string()).collect();
    let parents: Vec<String> = blocks.iter().map(|b| b.parent_hash.to_string()).collect();

    sqlx::query(
        "INSERT INTO blocks (chain_id, number, hash, parent_hash, timestamp)
         SELECT $1, * FROM UNNEST($2::bigint[], $3::text[], $4::text[], $5::timestamptz[])
         ON CONFLICT (chain_id, number) DO UPDATE
         SET hash = EXCLUDED.hash, parent_hash = EXCLUDED.parent_hash, timestamp = EXCLUDED.timestamp",
    )
    .bind(chain_id)
    .bind(numbers)
    .bind(hashes)
    .bind(parents)
    .bind(timestamps)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_events(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: i64,
    events: &[Event],
    confirmed_block: i64,
) -> Result<Vec<EventRecord>, DbError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let mut numbers = Vec::with_capacity(events.len());
    let mut timestamps = Vec::with_capacity(events.len());
    let mut log_indexes = Vec::with_capacity(events.len());
    for event in events {
        numbers.push(int(event.block_number)?);
        timestamps.push(timestamp(event.block_timestamp)?);
        log_indexes.push(
            i32::try_from(event.log_index)
                .map_err(|_| DbError::OutOfRange(format!("log index {}", event.log_index)))?,
        );
    }
    let block_hashes: Vec<String> = events.iter().map(|e| e.block_hash.to_string()).collect();
    let tx_hashes: Vec<String> = events.iter().map(|e| e.tx_hash.to_string()).collect();
    let contracts: Vec<String> = events.iter().map(|e| hex(&e.contract)).collect();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.name()).collect();
    let payloads: Vec<&serde_json::Value> = events.iter().map(|e| &e.payload).collect();

    let records = sqlx::query_as(
        "INSERT INTO events (chain_id, block_number, block_hash, block_timestamp, log_index,
                             tx_hash, contract_address, event_type, payload)
         SELECT $1, * FROM UNNEST($2::bigint[], $3::text[], $4::timestamptz[], $5::int[],
                                  $6::text[], $7::text[], $8::text[], $9::jsonb[])
         ON CONFLICT (chain_id, block_number, log_index) DO UPDATE
         SET block_hash = EXCLUDED.block_hash, block_timestamp = EXCLUDED.block_timestamp,
             tx_hash = EXCLUDED.tx_hash, contract_address = EXCLUDED.contract_address,
             event_type = EXCLUDED.event_type, payload = EXCLUDED.payload
         RETURNING id, chain_id, block_number, block_hash, block_timestamp, log_index, tx_hash,
                   contract_address, event_type, payload, block_number <= $10 AS confirmed",
    )
    .bind(chain_id)
    .bind(numbers)
    .bind(block_hashes)
    .bind(timestamps)
    .bind(log_indexes)
    .bind(tx_hashes)
    .bind(contracts)
    .bind(kinds)
    .bind(payloads)
    .bind(confirmed_block)
    .fetch_all(&mut **tx)
    .await?;
    Ok(records)
}

/// Moves the cursor back from `from` to `ancestor` and deletes everything above it. Returns
/// how many events were removed.
pub async fn rollback(
    pool: &PgPool,
    chain_id: u64,
    from: BlockRef,
    ancestor: BlockRef,
) -> Result<u64, DbError> {
    let id = int(chain_id)?;
    let height = int(ancestor.number)?;
    let mut tx = pool.begin().await?;

    let moved = sqlx::query(
        "UPDATE chain_cursors SET block_number = $2, block_hash = $3, updated_at = now()
         WHERE chain_id = $1 AND block_number = $4 AND block_hash = $5",
    )
    .bind(id)
    .bind(height)
    .bind(ancestor.hash.to_string())
    .bind(int(from.number)?)
    .bind(from.hash.to_string())
    .execute(&mut *tx)
    .await?;
    if moved.rows_affected() != 1 {
        return Err(DbError::CursorConflict(chain_id));
    }

    let removed = sqlx::query("DELETE FROM events WHERE chain_id = $1 AND block_number > $2")
        .bind(id)
        .bind(height)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM blocks WHERE chain_id = $1 AND number > $2")
        .bind(id)
        .bind(height)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(removed)
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct EventRecord {
    pub id: i64,
    pub chain_id: i64,
    pub block_number: i64,
    pub block_hash: String,
    pub block_timestamp: DateTime<Utc>,
    pub log_index: i32,
    pub tx_hash: String,
    pub contract_address: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    pub chain_id: Option<u64>,
    pub event_type: Option<EventKind>,
    pub contract: Option<Address>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
    pub confirmed: Option<bool>,
    /// Keyset cursor: only events with a smaller id.
    pub before_id: Option<i64>,
    pub limit: u32,
}

const SELECT_EVENTS: &str =
    "SELECT e.id, e.chain_id, e.block_number, e.block_hash, e.block_timestamp,
        e.log_index, e.tx_hash, e.contract_address, e.event_type, e.payload,
        e.block_number <= c.confirmed_block AS confirmed
    FROM events e JOIN chain_cursors c ON c.chain_id = e.chain_id";

/// Newest first. Pages are stable while new events arrive because they are keyed on id.
pub async fn list_events(pool: &PgPool, query: &EventQuery) -> Result<Vec<EventRecord>, DbError> {
    let sql = format!(
        "{SELECT_EVENTS}
         WHERE ($1::bigint IS NULL OR e.chain_id = $1)
           AND ($2::text IS NULL OR e.event_type = $2)
           AND ($3::text IS NULL OR e.contract_address = $3)
           AND ($4::bigint IS NULL OR e.block_number >= $4)
           AND ($5::bigint IS NULL OR e.block_number <= $5)
           AND ($6::boolean IS NULL OR (e.block_number <= c.confirmed_block) = $6)
           AND ($7::bigint IS NULL OR e.id < $7)
         ORDER BY e.id DESC
         LIMIT $8"
    );
    Ok(sqlx::query_as(&sql)
        .bind(query.chain_id.map(int).transpose()?)
        .bind(query.event_type.map(EventKind::name))
        .bind(query.contract.as_ref().map(hex))
        .bind(query.from_block.map(int).transpose()?)
        .bind(query.to_block.map(int).transpose()?)
        .bind(query.confirmed)
        .bind(query.before_id)
        .bind(i64::from(query.limit))
        .fetch_all(pool)
        .await?)
}

pub async fn event_by_id(pool: &PgPool, id: i64) -> Result<Option<EventRecord>, DbError> {
    Ok(sqlx::query_as(&format!("{SELECT_EVENTS} WHERE e.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

fn int(n: u64) -> Result<i64, DbError> {
    i64::try_from(n).map_err(|_| DbError::OutOfRange(n.to_string()))
}

fn timestamp(secs: u64) -> Result<DateTime<Utc>, DbError> {
    i64::try_from(secs)
        .ok()
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .ok_or_else(|| DbError::OutOfRange(format!("timestamp {secs}")))
}

fn parse_hash(hash: &str) -> Result<B256, DbError> {
    hash.parse()
        .map_err(|_| DbError::OutOfRange(format!("block hash {hash}")))
}

fn block_ref(number: i64, hash: &str) -> Result<BlockRef, DbError> {
    let number = u64::try_from(number).map_err(|_| DbError::OutOfRange(number.to_string()))?;
    Ok(BlockRef {
        number,
        hash: parse_hash(hash)?,
    })
}

use chainpulse_types::NormalizedEvent;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

pub async fn create_pool(database_url: &str, max_connections: u32) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn get_last_indexed_block(pool: &PgPool, chain_id: i32) -> Result<Option<i64>, DbError> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT last_block FROM indexed_blocks WHERE chain_id = $1",
    )
    .bind(chain_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_last_indexed_block(
    pool: &PgPool,
    chain_id: i32,
    block: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO indexed_blocks (chain_id, last_block, updated_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT (chain_id) DO UPDATE SET last_block = $2, updated_at = NOW()",
    )
    .bind(chain_id)
    .bind(block)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_event(
    pool: &PgPool,
    chain_id: i32,
    block_number: i64,
    log_index: i32,
    tx_hash: &str,
    contract_address: &str,
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<bool, DbError> {
    let result = sqlx::query(
        "INSERT INTO events (chain_id, block_number, log_index, tx_hash, contract_address, event_type, payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (chain_id, block_number, log_index) DO NOTHING",
    )
    .bind(chain_id)
    .bind(block_number)
    .bind(log_index)
    .bind(tx_hash)
    .bind(contract_address)
    .bind(event_type)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Insert multiple events in a single multi-row INSERT statement within a transaction.
/// Uses `ON CONFLICT (chain_id, block_number, log_index) DO NOTHING` for idempotency.
/// Returns the number of rows actually inserted.
pub async fn batch_insert_events(pool: &PgPool, events: &[NormalizedEvent]) -> Result<u64, DbError> {
    if events.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;

    // Build multi-row INSERT: INSERT INTO events (...) VALUES ($1..$7), ($8..$14), ...
    // Each event has 7 bind parameters.
    let mut sql = String::from(
        "INSERT INTO events (chain_id, block_number, log_index, tx_hash, contract_address, event_type, payload) VALUES ",
    );

    for (i, _) in events.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        let base = i * 7;
        sql.push_str(&format!(
            "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
        ));
    }

    sql.push_str(" ON CONFLICT (chain_id, block_number, log_index) DO NOTHING");

    let mut query = sqlx::query(&sql);
    for event in events {
        query = query
            .bind(event.chain_id)
            .bind(event.block_number)
            .bind(event.log_index)
            .bind(&event.tx_hash)
            .bind(&event.contract_address)
            .bind(&event.event_type)
            .bind(&event.payload);
    }

    let result = query.execute(&mut *tx).await?;
    tx.commit().await?;

    Ok(result.rows_affected())
}

/// For each (chain_id, max_block_number) pair, upsert the `indexed_blocks` table.
/// Computes max block per chain_id from the provided events.
pub async fn batch_upsert_last_indexed_blocks(
    pool: &PgPool,
    events: &[NormalizedEvent],
) -> Result<(), DbError> {
    if events.is_empty() {
        return Ok(());
    }

    // Compute max block per chain_id
    let mut chain_blocks: HashMap<i32, i64> = HashMap::new();
    for event in events {
        let entry = chain_blocks.entry(event.chain_id).or_insert(event.block_number);
        if event.block_number > *entry {
            *entry = event.block_number;
        }
    }

    for (chain_id, block) in &chain_blocks {
        upsert_last_indexed_block(pool, *chain_id, *block).await?;
    }

    Ok(())
}

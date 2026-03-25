use serde_json;
use sqlx::postgres::{PgPool, PgPoolOptions};
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

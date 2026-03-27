#![deny(clippy::unwrap_used, clippy::expect_used)]

use chainpulse_types::NormalizedEvent;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::FromRow;
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

/// Full database row representation of an event, used for API responses.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EventRow {
    pub id: i64,
    pub chain_id: i32,
    pub block_number: i64,
    pub log_index: i32,
    pub tx_hash: String,
    pub contract_address: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Optional filter parameters for querying events.
#[derive(Debug, Default)]
pub struct EventFilter {
    pub chain_id: Option<i32>,
    pub event_type: Option<String>,
    pub contract_address: Option<String>,
    pub block_min: Option<i64>,
    pub block_max: Option<i64>,
}

/// Build the WHERE clause and collect bind parameter values.
/// Returns (where_clause, bind_values) where bind_values is a vec of boxed
/// values that must be bound in order.
fn build_where_clause(filter: &EventFilter) -> (String, Vec<BindValue>) {
    let mut clauses = Vec::new();
    let mut binds: Vec<BindValue> = Vec::new();
    let mut idx = 1u32;

    if let Some(chain_id) = filter.chain_id {
        clauses.push(format!("chain_id = ${idx}"));
        binds.push(BindValue::I32(chain_id));
        idx += 1;
    }
    if let Some(ref event_type) = filter.event_type {
        clauses.push(format!("event_type = ${idx}"));
        binds.push(BindValue::String(event_type.clone()));
        idx += 1;
    }
    if let Some(ref contract_address) = filter.contract_address {
        clauses.push(format!("contract_address = ${idx}"));
        binds.push(BindValue::String(contract_address.clone()));
        idx += 1;
    }
    if let Some(block_min) = filter.block_min {
        clauses.push(format!("block_number >= ${idx}"));
        binds.push(BindValue::I64(block_min));
        idx += 1;
    }
    if let Some(block_max) = filter.block_max {
        clauses.push(format!("block_number <= ${idx}"));
        binds.push(BindValue::I64(block_max));
        #[allow(unused_assignments)]
        { idx += 1; }
    }

    let where_sql = if clauses.is_empty() {
        String::from("WHERE 1=1")
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    (where_sql, binds)
}

/// Internal enum to hold typed bind values for dynamic query construction.
enum BindValue {
    I32(i32),
    I64(i64),
    String(String),
}

/// Bind all values from a `Vec<BindValue>` to a sqlx query.
fn bind_values<'q>(
    mut query: sqlx::query::QueryAs<'q, sqlx::Postgres, EventRow, sqlx::postgres::PgArguments>,
    binds: &'q [BindValue],
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, EventRow, sqlx::postgres::PgArguments> {
    for bind in binds {
        query = match bind {
            BindValue::I32(v) => query.bind(v),
            BindValue::I64(v) => query.bind(v),
            BindValue::String(v) => query.bind(v),
        };
    }
    query
}

/// Bind all values to a scalar (i64) query.
fn bind_values_scalar<'q>(
    mut query: sqlx::query::QueryScalar<'q, sqlx::Postgres, i64, sqlx::postgres::PgArguments>,
    binds: &'q [BindValue],
) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, i64, sqlx::postgres::PgArguments> {
    for bind in binds {
        query = match bind {
            BindValue::I32(v) => query.bind(v),
            BindValue::I64(v) => query.bind(v),
            BindValue::String(v) => query.bind(v),
        };
    }
    query
}

/// Query events with optional filters, pagination (LIMIT/OFFSET), ordered by id DESC.
pub async fn query_events(
    pool: &PgPool,
    filter: &EventFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<EventRow>, DbError> {
    let (where_sql, binds) = build_where_clause(filter);
    let next_idx = binds.len() as u32 + 1;

    let sql = format!(
        "SELECT id, chain_id, block_number, log_index, tx_hash, contract_address, event_type, payload, block_timestamp, created_at \
         FROM events {where_sql} ORDER BY id DESC LIMIT ${} OFFSET ${}",
        next_idx,
        next_idx + 1,
    );

    let query = sqlx::query_as::<_, EventRow>(&sql);
    let query = bind_values(query, &binds);
    let rows = query.bind(limit).bind(offset).fetch_all(pool).await?;

    Ok(rows)
}

/// Count events matching the given filter (same WHERE clause as query_events, no LIMIT/OFFSET).
pub async fn count_events(pool: &PgPool, filter: &EventFilter) -> Result<i64, DbError> {
    let (where_sql, binds) = build_where_clause(filter);

    let sql = format!("SELECT COUNT(*) FROM events {where_sql}");

    let query = sqlx::query_scalar::<_, i64>(&sql);
    let query = bind_values_scalar(query, &binds);
    let count = query.fetch_one(pool).await?;

    Ok(count)
}

/// Get a single event by its primary key ID.
pub async fn get_event_by_id(pool: &PgPool, id: i64) -> Result<Option<EventRow>, DbError> {
    let row = sqlx::query_as::<_, EventRow>(
        "SELECT id, chain_id, block_number, log_index, tx_hash, contract_address, event_type, payload, block_timestamp, created_at \
         FROM events WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

//! Integration tests for the ChainPulse pipeline.
//!
//! These tests require a running PostgreSQL instance. Set `DATABASE_URL` to point
//! at a test database (default: `postgres://postgres:postgres@localhost:5433/chainpulse_test`).
//!
//! Start the test database with:
//!   docker compose -f docker-compose.test.yml up -d
//!
//! All tests are marked `#[ignore]` so they are skipped by default when no DB is available.
//! Run them explicitly with:
//!   cargo test --test integration -- --ignored

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chainpulse_types::NormalizedEvent;
use serde_json::json;
use sqlx::PgPool;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Default test database URL when `DATABASE_URL` is not set.
const DEFAULT_DATABASE_URL: &str =
    "postgres://postgres:postgres@localhost:5433/chainpulse_test";

/// Connect to the test database and run migrations.
async fn setup_pool() -> PgPool {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("Failed to connect to test database");
    sqlx::migrate!("../chainpulse-db/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    pool
}

/// Clean the events and indexed_blocks tables so tests are isolated.
async fn clean_tables(pool: &PgPool) {
    sqlx::query("DELETE FROM events")
        .execute(pool)
        .await
        .expect("Failed to clean events table");
    sqlx::query("DELETE FROM indexed_blocks")
        .execute(pool)
        .await
        .expect("Failed to clean indexed_blocks table");
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Create a Transfer-type `NormalizedEvent` fixture.
fn make_transfer_event(chain_id: i32, block: i64, log_idx: i32) -> NormalizedEvent {
    NormalizedEvent {
        event_type: "Transfer".to_string(),
        contract_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        tx_hash: format!(
            "0x{:064x}",
            (chain_id as u64) * 1_000_000 + (block as u64) * 1_000 + (log_idx as u64)
        ),
        block_number: block,
        log_index: log_idx,
        chain_id,
        payload: json!({
            "from": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "to": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "value": "1000000"
        }),
    }
}

/// Create a SwapV2-type `NormalizedEvent` fixture.
fn make_swap_event(chain_id: i32, block: i64, log_idx: i32) -> NormalizedEvent {
    NormalizedEvent {
        event_type: "SwapV2".to_string(),
        contract_address: "0x5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f".to_string(),
        tx_hash: format!(
            "0x{:064x}",
            (chain_id as u64) * 2_000_000 + (block as u64) * 1_000 + (log_idx as u64)
        ),
        block_number: block,
        log_index: log_idx,
        chain_id,
        payload: json!({
            "sender": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "amount0_in": "100",
            "amount1_in": "0",
            "amount0_out": "0",
            "amount1_out": "200",
            "to": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        }),
    }
}

// ---------------------------------------------------------------------------
// Test 1: Pipeline end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_pipeline_end_to_end() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    // Create MPSC channel and broadcast channel (mirroring main.rs)
    let (tx, rx) =
        tokio::sync::mpsc::channel::<NormalizedEvent>(100);
    let (broadcast_tx, _broadcast_rx) =
        tokio::sync::broadcast::channel::<NormalizedEvent>(64);
    let token = tokio_util::sync::CancellationToken::new();

    // Spawn the DB writer
    let writer_pool = pool.clone();
    let writer_token = token.clone();
    let writer_broadcast = broadcast_tx.clone();
    let writer_handle = tokio::spawn(async move {
        chainpulse::db_writer::run(rx, writer_pool, 10, 200, writer_broadcast, writer_token).await
    });

    // Send fixture events through the channel
    let events = vec![
        make_transfer_event(1, 18_000_001, 0),
        make_transfer_event(1, 18_000_001, 1),
        make_swap_event(1, 18_000_002, 0),
    ];
    for event in &events {
        tx.send(event.clone()).await.expect("channel send failed");
    }

    // Signal shutdown and wait for writer to flush
    token.cancel();
    let result = writer_handle.await.expect("writer task panicked");
    result.expect("writer task returned error");

    // Query events from DB and verify
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(1),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query_events failed");

    assert_eq!(rows.len(), 3, "expected 3 events in DB");

    // Verify event types are present
    let types: Vec<&str> = rows.iter().map(|r| r.event_type.as_str()).collect();
    assert!(types.contains(&"Transfer"), "missing Transfer events");
    assert!(types.contains(&"SwapV2"), "missing SwapV2 events");

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 2: Batch insert idempotency
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_batch_insert_idempotency() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    let events = vec![
        make_transfer_event(1, 18_000_010, 0),
        make_transfer_event(1, 18_000_010, 1),
    ];

    // Insert once
    let inserted_first =
        chainpulse_db::batch_insert_events(&pool, &events)
            .await
            .expect("first batch insert failed");
    assert_eq!(inserted_first, 2);

    // Insert the same events again
    let inserted_second =
        chainpulse_db::batch_insert_events(&pool, &events)
            .await
            .expect("second batch insert failed");
    assert_eq!(inserted_second, 0, "duplicate insert should affect 0 rows");

    // Verify total count is still 2
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(1),
        ..Default::default()
    };
    let count = chainpulse_db::count_events(&pool, &filter)
        .await
        .expect("count_events failed");
    assert_eq!(count, 2);

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 3: Block tracking
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_block_tracking() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    let events = vec![
        make_transfer_event(1, 18_000_100, 0),
        make_transfer_event(1, 18_000_200, 0),
        make_transfer_event(137, 50_000_001, 0),
    ];

    chainpulse_db::batch_insert_events(&pool, &events)
        .await
        .expect("batch insert failed");
    chainpulse_db::batch_upsert_last_indexed_blocks(&pool, &events)
        .await
        .expect("batch upsert blocks failed");

    let eth_block = chainpulse_db::get_last_indexed_block(&pool, 1)
        .await
        .expect("get_last_indexed_block failed for chain 1");
    assert_eq!(eth_block, Some(18_000_200));

    let poly_block = chainpulse_db::get_last_indexed_block(&pool, 137)
        .await
        .expect("get_last_indexed_block failed for chain 137");
    assert_eq!(poly_block, Some(50_000_001));

    // Non-existent chain returns None
    let missing = chainpulse_db::get_last_indexed_block(&pool, 999)
        .await
        .expect("get_last_indexed_block failed for chain 999");
    assert_eq!(missing, None);

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 4: REST API query
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_query() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    // Insert test events directly
    let events = vec![
        make_transfer_event(1, 18_000_300, 0),
        make_swap_event(1, 18_000_301, 0),
        make_transfer_event(137, 50_000_100, 0),
    ];
    chainpulse_db::batch_insert_events(&pool, &events)
        .await
        .expect("batch insert failed");

    // Create broadcast channel and prometheus handle
    let (broadcast_tx, _) =
        tokio::sync::broadcast::channel::<NormalizedEvent>(64);

    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();

    let health_state = chainpulse::api::HealthState::new(vec![
        ("ethereum".to_string(), 1),
        ("polygon".to_string(), 137),
    ]);

    let router = chainpulse::api::create_router(
        pool.clone(),
        broadcast_tx,
        100,
        prometheus_handle,
        health_state,
    );

    // Bind to a random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind listener");
    let addr = listener.local_addr().expect("failed to get local addr");
    let base_url = format!("http://{}", addr);

    // Spawn the server
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Query events for chain_id=1
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/events?chain_id=1", base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    let total = body["total"].as_i64().expect("missing total field");
    assert_eq!(total, 2, "expected 2 events for chain_id=1");

    let events_arr = body["events"].as_array().expect("missing events array");
    assert_eq!(events_arr.len(), 2);

    // Verify all returned events have chain_id=1
    for event in events_arr {
        assert_eq!(event["chain_id"].as_i64().unwrap(), 1);
    }

    // Abort the server
    server_handle.abort();

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 5: Event filtering
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_event_filtering() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    // Insert a variety of events
    let events = vec![
        make_transfer_event(1, 18_000_400, 0),
        make_transfer_event(1, 18_000_401, 0),
        make_swap_event(1, 18_000_402, 0),
        make_transfer_event(137, 50_000_200, 0),
        make_swap_event(137, 50_000_201, 0),
    ];
    chainpulse_db::batch_insert_events(&pool, &events)
        .await
        .expect("batch insert failed");

    // Filter by event_type only
    let filter = chainpulse_db::EventFilter {
        event_type: Some("Transfer".to_string()),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query failed");
    assert_eq!(rows.len(), 3, "expected 3 Transfer events across all chains");

    // Filter by chain_id + event_type
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(1),
        event_type: Some("SwapV2".to_string()),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query failed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event_type, "SwapV2");
    assert_eq!(rows[0].chain_id, 1);

    // Filter by block range
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(1),
        block_min: Some(18_000_401),
        block_max: Some(18_000_402),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query failed");
    assert_eq!(rows.len(), 2, "expected 2 events in block range 401-402");

    // Filter by contract_address
    let filter = chainpulse_db::EventFilter {
        contract_address: Some(
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        ),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query failed");
    // Only Transfer events use this address in our fixtures
    assert_eq!(rows.len(), 3);

    // Empty result
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(999),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query failed");
    assert_eq!(rows.len(), 0);

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 6: Graceful shutdown pipeline
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_graceful_shutdown_flushes_events() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    let (tx, rx) =
        tokio::sync::mpsc::channel::<NormalizedEvent>(100);
    let (broadcast_tx, _) =
        tokio::sync::broadcast::channel::<NormalizedEvent>(64);
    let token = tokio_util::sync::CancellationToken::new();

    let writer_pool = pool.clone();
    let writer_token = token.clone();
    let writer_broadcast = broadcast_tx.clone();
    let writer_handle = tokio::spawn(async move {
        chainpulse::db_writer::run(rx, writer_pool, 1000, 60_000, writer_broadcast, writer_token)
            .await
    });

    // Send events — use large batch_size and long flush interval so nothing
    // flushes until shutdown triggers the drain path.
    let events = vec![
        make_transfer_event(1, 18_000_500, 0),
        make_transfer_event(1, 18_000_500, 1),
        make_swap_event(1, 18_000_501, 0),
    ];
    for event in &events {
        tx.send(event.clone()).await.expect("channel send failed");
    }

    // Small delay to ensure events are received by the writer
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Cancel the token to trigger graceful shutdown
    token.cancel();

    let result = writer_handle.await.expect("writer task panicked");
    result.expect("writer task returned error");

    // Verify all events were flushed to DB
    let filter = chainpulse_db::EventFilter {
        chain_id: Some(1),
        ..Default::default()
    };
    let rows = chainpulse_db::query_events(&pool, &filter, 100, 0)
        .await
        .expect("query_events failed");
    assert_eq!(
        rows.len(),
        3,
        "all events should be flushed on graceful shutdown"
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Test 7: Empty batch — DB writer exits cleanly with no events
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_empty_batch_shutdown() {
    let pool = setup_pool().await;
    clean_tables(&pool).await;

    let (_tx, rx) =
        tokio::sync::mpsc::channel::<NormalizedEvent>(100);
    let (broadcast_tx, _) =
        tokio::sync::broadcast::channel::<NormalizedEvent>(64);
    let token = tokio_util::sync::CancellationToken::new();

    let writer_pool = pool.clone();
    let writer_token = token.clone();
    let writer_broadcast = broadcast_tx.clone();
    let writer_handle = tokio::spawn(async move {
        chainpulse::db_writer::run(rx, writer_pool, 100, 500, writer_broadcast, writer_token).await
    });

    // Cancel immediately — no events sent
    token.cancel();

    let result = writer_handle.await.expect("writer task panicked");
    result.expect("writer task should exit cleanly with no events");

    // Verify no events in DB
    let filter = chainpulse_db::EventFilter::default();
    let count = chainpulse_db::count_events(&pool, &filter)
        .await
        .expect("count_events failed");
    assert_eq!(count, 0);

    pool.close().await;
}

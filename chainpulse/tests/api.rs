#![allow(clippy::expect_used)]

mod common;

use std::time::Duration;

use chainpulse::api::{self, AppState};
use chainpulse::status::ChainStatus;
use chainpulse::ws::Notification;
use chainpulse_db::PgPool;
use common::{Chain, migrated, sync};
use futures_util::{SinkExt, StreamExt};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

async fn serve(
    pool: &PgPool,
    chains: Vec<ChainStatus>,
    feed: broadcast::Sender<Notification>,
    max_ws_clients: usize,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("local address");
    let metrics = PrometheusBuilder::new().build_recorder().handle();
    let state = AppState::new(
        pool.clone(),
        feed,
        chains,
        metrics,
        max_ws_clients,
        CancellationToken::new(),
    );
    tokio::spawn(api::serve(listener, state));
    format!("{address}")
}

async fn get(address: &str, path: &str) -> (u16, Value) {
    let response = reqwest::get(format!("http://{address}{path}"))
        .await
        .expect("request");
    let status = response.status().as_u16();
    let body = response.text().await.expect("body");
    (
        status,
        serde_json::from_str(&body).unwrap_or(Value::String(body)),
    )
}

async fn indexed_chain(pool: &PgPool) -> (Chain, String) {
    migrated(pool).await;
    let chain = Chain::start().await;
    for value in 1..=4 {
        chain.transfer(value).await;
    }
    let mut indexer = chain.indexer(pool, chain.config(2)).await;
    sync(&mut indexer).await;
    let (feed, _) = broadcast::channel(16);
    let address = serve(pool, vec![indexer.status()], feed, 4).await;
    (chain, address)
}

fn ids(page: &Value) -> Vec<i64> {
    page["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|e| e["id"].as_i64().expect("id"))
        .collect()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn pages_through_events_newest_first_with_a_cursor(pool: PgPool) {
    let (_chain, address) = indexed_chain(&pool).await;

    let (status, first) = get(&address, "/events?limit=3").await;
    assert_eq!(status, 200);
    let cursor = first["next_cursor"].as_i64().expect("more pages");
    let (_, second) = get(&address, &format!("/events?limit=3&cursor={cursor}")).await;

    let all: Vec<i64> = ids(&first).into_iter().chain(ids(&second)).collect();
    assert_eq!(all.len(), 5);
    assert!(all.windows(2).all(|pair| pair[0] > pair[1]), "{all:?}");
    assert_eq!(second["next_cursor"], Value::Null);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn filters_events(pool: PgPool) {
    let (chain, address) = indexed_chain(&pool).await;
    let token = chain.token.address().to_checksum(None);
    let count = async |query: &str| ids(&get(&address, &format!("/events?{query}")).await.1).len();

    assert_eq!(count(&format!("contract={token}")).await, 5);
    assert_eq!(
        count("contract=0x0000000000000000000000000000000000000001").await,
        0
    );
    assert_eq!(count("event_type=Transfer&chain_id=31337").await, 5);
    assert_eq!(count("from_block=2&to_block=3").await, 2);
    assert_eq!(
        count("confirmed=false").await,
        2,
        "the top two blocks are within two confirmations"
    );
    assert_eq!(count("confirmed=true").await, 3);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn serves_single_events_and_rejects_bad_requests(pool: PgPool) {
    let (_chain, address) = indexed_chain(&pool).await;
    let (_, page) = get(&address, "/events?limit=1").await;
    let id = ids(&page)[0];

    let (status, event) = get(&address, &format!("/events/{id}")).await;
    assert_eq!(status, 200);
    assert_eq!(event, page["events"][0]);
    assert_eq!(event["event_type"], "Transfer");
    assert!(event["block_timestamp"].is_string());

    assert_eq!(
        get(&address, "/events/999999").await,
        (404, json!({ "error": "not found" }))
    );
    assert_eq!(get(&address, "/events?event_type=Mint").await.0, 400);
    assert_eq!(get(&address, "/events?limit=many").await.0, 400);
    assert_eq!(get(&address, "/events?offset=10").await.0, 400);
    assert_eq!(
        get(&address, "/events?chain_id=18446744073709551615").await,
        (
            400,
            json!({ "error": "chain_id is larger than 9223372036854775807" })
        )
    );
    assert_eq!(
        get(&address, "/events?from_block=9223372036854775808")
            .await
            .0,
        400
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn readiness_follows_indexer_lag(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    chain.mine(20).await;
    let mut config = chain.config(0);
    config.max_block_range = 1;
    let mut indexer = chain.indexer(&pool, config).await;
    let (feed, _) = broadcast::channel(16);
    let address = serve(&pool, vec![indexer.status()], feed, 4).await;

    assert_eq!(
        get(&address, "/health").await,
        (200, json!({ "status": "ok" }))
    );
    assert_eq!(get(&address, "/ready").await.0, 503, "nothing indexed yet");

    indexer.step().await.expect("step");
    let (status, lagging) = get(&address, "/ready").await;
    assert_eq!(status, 503);
    assert_eq!(lagging["chains"][0]["lag"], 21);

    sync(&mut indexer).await;
    let (status, caught_up) = get(&address, "/ready").await;
    assert_eq!(status, 200);
    assert_eq!(caught_up["chains"][0]["lag"], 0);
    assert_eq!(caught_up["database"], true);
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(socket: &mut Socket) -> Value {
    let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("message within 5s")
        .expect("socket open")
        .expect("valid frame");
    serde_json::from_str(message.to_text().expect("text frame")).expect("json")
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn websocket_streams_new_events_and_reorgs(pool: PgPool) {
    migrated(&pool).await;
    let chain = Chain::start().await;
    let (feed, _) = broadcast::channel(64);
    let mut indexer = chain
        .indexer_with_feed(&pool, chain.config(10), feed.clone())
        .await;
    sync(&mut indexer).await;
    let address = serve(&pool, vec![indexer.status()], feed, 4).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws/events"))
        .await
        .expect("connect");

    let subscribe =
        json!({ "subscribe": { "chain_id": chain.chain_id, "event_type": "Transfer" } });
    socket
        .send(Message::text(subscribe.to_string()))
        .await
        .expect("subscribe");
    assert_eq!(next_json(&mut socket).await["type"], "subscribed");

    let snapshot = chain.snapshot().await;
    let block = chain.transfer(5).await;
    sync(&mut indexer).await;
    let event = next_json(&mut socket).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["block_number"], block);
    assert_eq!(event["payload"]["value"], "5");
    assert_eq!(event["confirmed"], false);

    chain.revert(snapshot).await;
    chain.transfer(6).await;
    sync(&mut indexer).await;
    let reorg = next_json(&mut socket).await;
    assert_eq!(reorg["type"], "reorg");
    assert_eq!(reorg["common_ancestor"], block - 1);
    assert_eq!(reorg["removed_events"], 1);
    assert_eq!(next_json(&mut socket).await["payload"]["value"], "6");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs Postgres and anvil: ./do itest"]
async fn refuses_websocket_clients_over_the_limit(pool: PgPool) {
    migrated(&pool).await;
    let (feed, _) = broadcast::channel(16);
    let address = serve(&pool, Vec::new(), feed, 1).await;
    let url = format!("ws://{address}/ws/events");

    let _first = tokio_tungstenite::connect_async(&url)
        .await
        .expect("first client");
    let second = tokio_tungstenite::connect_async(&url).await;

    match second {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 503)
        }
        other => panic!("expected 503, got {other:?}"),
    }
}

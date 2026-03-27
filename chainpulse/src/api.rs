use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chainpulse_db::{EventFilter, EventRow};
use chainpulse_types::NormalizedEvent;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, watch, RwLock};

/// Per-chain health status.
#[derive(Debug, Clone, Serialize)]
pub struct ChainHealth {
    pub chain_name: String,
    pub chain_id: i32,
    pub connected: bool,
    pub last_block_number: Option<i64>,
    pub last_event_at: Option<DateTime<Utc>>,
}

/// Top-level health response returned by the `/health` endpoint.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub chains: Vec<ChainHealth>,
}

/// Shared health state tracking per-chain subscriber status.
#[derive(Clone)]
pub struct HealthState {
    inner: Arc<RwLock<HashMap<String, ChainHealth>>>,
}

impl HealthState {
    /// Create a new HealthState initialized with all chains as disconnected.
    pub fn new(chains: Vec<(String, i32)>) -> Self {
        let mut map = HashMap::new();
        for (name, chain_id) in chains {
            map.insert(
                name.clone(),
                ChainHealth {
                    chain_name: name,
                    chain_id,
                    connected: false,
                    last_block_number: None,
                    last_event_at: None,
                },
            );
        }
        Self {
            inner: Arc::new(RwLock::new(map)),
        }
    }

    /// Update the connection status for a chain.
    pub async fn set_connected(&self, chain: &str, connected: bool) {
        if let Some(health) = self.inner.write().await.get_mut(chain) {
            health.connected = connected;
        }
    }

    /// Update the last indexed block number for a chain.
    pub async fn set_last_block(&self, chain: &str, block: i64) {
        if let Some(health) = self.inner.write().await.get_mut(chain) {
            health.last_block_number = Some(block);
        }
    }

    /// Update the last event timestamp for a chain.
    pub async fn set_last_event_at(&self, chain: &str, ts: DateTime<Utc>) {
        if let Some(health) = self.inner.write().await.get_mut(chain) {
            health.last_event_at = Some(ts);
        }
    }
}

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 1000;
const DEFAULT_OFFSET: i64 = 0;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    event_tx: broadcast::Sender<NormalizedEvent>,
    ws_connection_count: Arc<AtomicUsize>,
    ws_max_connections: usize,
    prometheus_handle: PrometheusHandle,
    health_state: HealthState,
}

/// Guard that decrements the WebSocket connection counter when dropped.
struct WsConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for WsConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Outgoing WebSocket message types.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum WsOutgoingMessage {
    Event(NormalizedEvent),
    Error { error: String },
    Lagged { lagged: LaggedInfo },
}

#[derive(Debug, Serialize)]
struct LaggedInfo {
    skipped: u64,
}

/// Subscription filter for WebSocket event streaming.
/// Both fields use AND logic: if set, the event must match.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionFilter {
    pub chain_id: Option<i32>,
    pub event_type: Option<String>,
}

/// Incoming client message over WebSocket.
#[derive(Debug, Deserialize)]
struct ClientMessage {
    subscribe: Option<SubscriptionFilter>,
}

/// Server acknowledgement of a subscription update.
#[derive(Debug, Serialize)]
struct ServerAck {
    subscribed: SubscriptionFilter,
}

/// Check whether a NormalizedEvent matches the given subscription filter.
/// If chain_id is set, event.chain_id must match.
/// If event_type is set, event.event_type must match.
/// Both conditions use AND logic. An empty filter matches all events.
fn matches_filter(event: &NormalizedEvent, filter: &SubscriptionFilter) -> bool {
    if let Some(chain_id) = filter.chain_id {
        if event.chain_id != chain_id {
            return false;
        }
    }
    if let Some(ref event_type) = filter.event_type {
        if event.event_type != *event_type {
            return false;
        }
    }
    true
}

#[derive(Debug, Deserialize)]
pub struct EventQueryParams {
    pub chain_id: Option<i32>,
    pub event_type: Option<String>,
    pub contract_address: Option<String>,
    pub block_min: Option<i64>,
    pub block_max: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct EventListResponse {
    pub events: Vec<EventRow>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(ErrorBody { error: message });
        (status, body).into_response()
    }
}

impl From<chainpulse_db::DbError> for ApiError {
    fn from(err: chainpulse_db::DbError) -> Self {
        tracing::error!(error = %err, "Database error in API handler");
        ApiError::Internal("internal server error".to_string())
    }
}

pub fn create_router(
    pool: PgPool,
    event_tx: broadcast::Sender<NormalizedEvent>,
    ws_max_connections: usize,
    prometheus_handle: PrometheusHandle,
    health_state: HealthState,
) -> Router {
    let state = AppState {
        pool,
        event_tx,
        ws_connection_count: Arc::new(AtomicUsize::new(0)),
        ws_max_connections,
        prometheus_handle,
        health_state,
    };
    Router::new()
        .route("/events", get(list_events))
        .route("/events/{id}", get(get_event))
        .route("/ws/events", get(ws_events))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(state)
}

async fn metrics_handler(State(state): State<AppState>) -> String {
    state.prometheus_handle.render()
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let map = state.health_state.inner.read().await;
    let chains: Vec<ChainHealth> = map.values().cloned().collect();
    let all_connected = chains.iter().all(|c| c.connected);
    let status = if all_connected {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };
    Json(HealthResponse { status, chains })
}

async fn list_events(
    Query(params): Query<EventQueryParams>,
    State(state): State<AppState>,
) -> Result<Json<EventListResponse>, ApiError> {
    // Validate limit
    let limit = match params.limit {
        Some(l) if l < 0 => {
            return Err(ApiError::BadRequest("limit must be non-negative".to_string()));
        }
        Some(l) if l > MAX_LIMIT => MAX_LIMIT,
        Some(l) => l,
        None => DEFAULT_LIMIT,
    };

    // Validate offset
    let offset = match params.offset {
        Some(o) if o < 0 => {
            return Err(ApiError::BadRequest("offset must be non-negative".to_string()));
        }
        Some(o) => o,
        None => DEFAULT_OFFSET,
    };

    let filter = EventFilter {
        chain_id: params.chain_id,
        event_type: params.event_type,
        contract_address: params.contract_address,
        block_min: params.block_min,
        block_max: params.block_max,
    };

    let events = chainpulse_db::query_events(&state.pool, &filter, limit, offset).await?;
    let total = chainpulse_db::count_events(&state.pool, &filter).await?;

    Ok(Json(EventListResponse {
        events,
        total,
        limit,
        offset,
    }))
}

async fn get_event(
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<EventRow>, ApiError> {
    let event = chainpulse_db::get_event_by_id(&state.pool, id).await?;
    match event {
        Some(row) => Ok(Json(row)),
        None => Err(ApiError::NotFound(format!("event with id {id} not found"))),
    }
}

async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let current = state.ws_connection_count.load(Ordering::SeqCst);
    if current >= state.ws_max_connections {
        tracing::warn!(
            current_connections = current,
            max_connections = state.ws_max_connections,
            "WebSocket connection rejected: at capacity"
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "too many WebSocket connections".to_string(),
            }),
        )
            .into_response();
    }

    ws.on_upgrade(|socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    // Increment connection count and create drop guard
    state.ws_connection_count.fetch_add(1, Ordering::SeqCst);
    let _guard = WsConnectionGuard {
        counter: Arc::clone(&state.ws_connection_count),
    };

    let current = state.ws_connection_count.load(Ordering::SeqCst);
    tracing::info!(active_connections = current, "WebSocket client connected");

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut broadcast_rx = state.event_tx.subscribe();

    // Filter state shared between recv task (writer) and send task (reader)
    let (filter_tx, filter_rx) = watch::channel(SubscriptionFilter::default());

    // Channel for the recv task to send response messages back through the WebSocket
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<String>(16);

    // Send task: forward broadcast events and response messages to the WebSocket client
    let send_task = tokio::spawn(async move {
        let mut filter_rx = filter_rx;
        loop {
            tokio::select! {
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(event) => {
                            // Read the current filter
                            let filter = filter_rx.borrow_and_update().clone();
                            if !matches_filter(&event, &filter) {
                                continue;
                            }
                            let msg = WsOutgoingMessage::Event(event);
                            match serde_json::to_string(&msg) {
                                Ok(json) => {
                                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "Failed to serialize event for WebSocket");
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "WebSocket client lagged behind");
                            let msg = WsOutgoingMessage::Lagged {
                                lagged: LaggedInfo { skipped: n },
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                Some(response_json) = resp_rx.recv() => {
                    if ws_sender.send(Message::Text(response_json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Recv task: read client messages and update subscription filters
    let recv_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            if let Some(new_filter) = client_msg.subscribe {
                                tracing::debug!(?new_filter, "Client updated subscription filter");
                                // Send ack before updating filter so client knows the change took effect
                                let ack = ServerAck {
                                    subscribed: new_filter.clone(),
                                };
                                if let Ok(json) = serde_json::to_string(&ack) {
                                    let _ = resp_tx.send(json).await;
                                }
                                let _ = filter_tx.send(new_filter);
                            } else {
                                // Valid JSON but no subscribe field
                                let err_msg = WsOutgoingMessage::Error {
                                    error: "missing 'subscribe' field".to_string(),
                                };
                                if let Ok(json) = serde_json::to_string(&err_msg) {
                                    let _ = resp_tx.send(json).await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, message = %text, "Invalid client message");
                            let err_msg = WsOutgoingMessage::Error {
                                error: format!("invalid message: {e}"),
                            };
                            if let Ok(json) = serde_json::to_string(&err_msg) {
                                let _ = resp_tx.send(json).await;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Ok(_) => {
                    // Ignore binary, ping, pong frames
                }
                Err(e) => {
                    tracing::debug!(error = %e, "WebSocket receive error");
                    break;
                }
            }
        }
    });

    // Wait for either task to finish, then cancel the other
    tokio::select! {
        _ = send_task => {
            tracing::debug!("WebSocket send task ended");
        }
        _ = recv_task => {
            tracing::debug!("WebSocket recv task ended");
        }
    }

    let remaining = state.ws_connection_count.load(Ordering::SeqCst);
    tracing::info!(active_connections = remaining.saturating_sub(1), "WebSocket client disconnected");
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Helper to create a sample NormalizedEvent for testing.
    fn sample_event(chain_id: i32, event_type: &str) -> NormalizedEvent {
        NormalizedEvent {
            event_type: event_type.to_string(),
            contract_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tx_hash: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            block_number: 18_000_000,
            log_index: 0,
            chain_id,
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn test_matches_filter_no_filter_matches_all() {
        let filter = SubscriptionFilter::default();
        let event = sample_event(1, "Transfer");
        assert!(matches_filter(&event, &filter));

        let event2 = sample_event(137, "SwapV2");
        assert!(matches_filter(&event2, &filter));
    }

    #[test]
    fn test_matches_filter_chain_id_only() {
        let filter = SubscriptionFilter {
            chain_id: Some(1),
            event_type: None,
        };

        assert!(matches_filter(&sample_event(1, "Transfer"), &filter));
        assert!(matches_filter(&sample_event(1, "SwapV2"), &filter));
        assert!(!matches_filter(&sample_event(137, "Transfer"), &filter));
        assert!(!matches_filter(&sample_event(42, "SwapV3"), &filter));
    }

    #[test]
    fn test_matches_filter_event_type_only() {
        let filter = SubscriptionFilter {
            chain_id: None,
            event_type: Some("Transfer".to_string()),
        };

        assert!(matches_filter(&sample_event(1, "Transfer"), &filter));
        assert!(matches_filter(&sample_event(137, "Transfer"), &filter));
        assert!(!matches_filter(&sample_event(1, "SwapV2"), &filter));
        assert!(!matches_filter(&sample_event(137, "Supply"), &filter));
    }

    #[test]
    fn test_matches_filter_both_fields_and_logic() {
        let filter = SubscriptionFilter {
            chain_id: Some(1),
            event_type: Some("Transfer".to_string()),
        };

        // Both match -> true
        assert!(matches_filter(&sample_event(1, "Transfer"), &filter));
        // chain_id matches, event_type does not -> false
        assert!(!matches_filter(&sample_event(1, "SwapV2"), &filter));
        // event_type matches, chain_id does not -> false
        assert!(!matches_filter(&sample_event(137, "Transfer"), &filter));
        // Neither matches -> false
        assert!(!matches_filter(&sample_event(137, "SwapV2"), &filter));
    }

    #[test]
    fn test_matches_filter_event_type_case_sensitive() {
        let filter = SubscriptionFilter {
            chain_id: None,
            event_type: Some("Transfer".to_string()),
        };

        assert!(matches_filter(&sample_event(1, "Transfer"), &filter));
        assert!(!matches_filter(&sample_event(1, "transfer"), &filter));
        assert!(!matches_filter(&sample_event(1, "TRANSFER"), &filter));
    }

    #[test]
    fn test_client_message_deserialization_valid_subscribe() {
        let json = r#"{"subscribe": {"chain_id": 1, "event_type": "Transfer"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        let filter = msg.subscribe.unwrap();
        assert_eq!(filter.chain_id, Some(1));
        assert_eq!(filter.event_type, Some("Transfer".to_string()));
    }

    #[test]
    fn test_client_message_deserialization_empty_subscribe() {
        let json = r#"{"subscribe": {}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        let filter = msg.subscribe.unwrap();
        assert_eq!(filter.chain_id, None);
        assert_eq!(filter.event_type, None);
    }

    #[test]
    fn test_client_message_deserialization_partial_subscribe() {
        let json = r#"{"subscribe": {"chain_id": 137}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        let filter = msg.subscribe.unwrap();
        assert_eq!(filter.chain_id, Some(137));
        assert_eq!(filter.event_type, None);
    }

    #[test]
    fn test_client_message_deserialization_no_subscribe_field() {
        let json = r#"{"other": "value"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(msg.subscribe.is_none());
    }

    #[test]
    fn test_client_message_deserialization_invalid_json() {
        let json = r#"not json at all"#;
        assert!(serde_json::from_str::<ClientMessage>(json).is_err());
    }

    #[test]
    fn test_broadcast_send_no_receivers_does_not_panic() {
        let (tx, _) = broadcast::channel::<NormalizedEvent>(16);
        // Drop the only receiver
        drop(tx.subscribe());

        let event = sample_event(1, "Transfer");
        // Send with no receivers -- should return Err but not panic
        let result = tx.send(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_broadcast_send_with_receiver() {
        let (tx, mut rx) = broadcast::channel::<NormalizedEvent>(16);
        let event = sample_event(1, "Transfer");
        let _ = tx.send(event.clone());

        let received = rx.try_recv().unwrap();
        assert_eq!(received.chain_id, 1);
        assert_eq!(received.event_type, "Transfer");
    }

    #[test]
    fn test_ws_outgoing_message_event_serialization() {
        let event = sample_event(1, "Transfer");
        let msg = WsOutgoingMessage::Event(event);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["event_type"], "Transfer");
        assert_eq!(parsed["chain_id"], 1);
    }

    #[test]
    fn test_ws_outgoing_message_error_serialization() {
        let msg = WsOutgoingMessage::Error {
            error: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"], "something went wrong");
    }

    #[test]
    fn test_ws_outgoing_message_lagged_serialization() {
        let msg = WsOutgoingMessage::Lagged {
            lagged: LaggedInfo { skipped: 42 },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["lagged"]["skipped"], 42);
    }

    #[test]
    fn test_server_ack_serialization() {
        let ack = ServerAck {
            subscribed: SubscriptionFilter {
                chain_id: Some(1),
                event_type: Some("Transfer".to_string()),
            },
        };
        let json = serde_json::to_string(&ack).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["subscribed"]["chain_id"], 1);
        assert_eq!(parsed["subscribed"]["event_type"], "Transfer");
    }

    #[test]
    fn test_ws_connection_guard_decrements_counter() {
        let counter = Arc::new(AtomicUsize::new(5));
        {
            let _guard = WsConnectionGuard {
                counter: Arc::clone(&counter),
            };
            assert_eq!(counter.load(Ordering::SeqCst), 5);
        }
        // After guard is dropped, counter should be decremented
        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_health_state_new_initializes_disconnected() {
        let state = HealthState::new(vec![
            ("ethereum".to_string(), 1),
            ("polygon".to_string(), 137),
        ]);
        let map = state.inner.read().await;
        assert_eq!(map.len(), 2);

        let eth = map.get("ethereum").unwrap();
        assert_eq!(eth.chain_name, "ethereum");
        assert_eq!(eth.chain_id, 1);
        assert!(!eth.connected);
        assert!(eth.last_block_number.is_none());
        assert!(eth.last_event_at.is_none());

        let poly = map.get("polygon").unwrap();
        assert_eq!(poly.chain_name, "polygon");
        assert_eq!(poly.chain_id, 137);
        assert!(!poly.connected);
    }

    #[tokio::test]
    async fn test_health_state_set_connected() {
        let state = HealthState::new(vec![("ethereum".to_string(), 1)]);
        state.set_connected("ethereum", true).await;
        {
            let map = state.inner.read().await;
            assert!(map.get("ethereum").unwrap().connected);
        }
        state.set_connected("ethereum", false).await;
        {
            let map = state.inner.read().await;
            assert!(!map.get("ethereum").unwrap().connected);
        }
    }

    #[tokio::test]
    async fn test_health_state_set_last_block() {
        let state = HealthState::new(vec![("ethereum".to_string(), 1)]);
        state.set_last_block("ethereum", 18_000_000).await;
        let map = state.inner.read().await;
        assert_eq!(map.get("ethereum").unwrap().last_block_number, Some(18_000_000));
    }

    #[tokio::test]
    async fn test_health_state_set_last_event_at() {
        let state = HealthState::new(vec![("ethereum".to_string(), 1)]);
        let now = Utc::now();
        state.set_last_event_at("ethereum", now).await;
        let map = state.inner.read().await;
        assert_eq!(map.get("ethereum").unwrap().last_event_at, Some(now));
    }

    #[tokio::test]
    async fn test_health_state_unknown_chain_ignored() {
        let state = HealthState::new(vec![("ethereum".to_string(), 1)]);
        // These should not panic or create new entries
        state.set_connected("unknown", true).await;
        state.set_last_block("unknown", 100).await;
        state.set_last_event_at("unknown", Utc::now()).await;
        let map = state.inner.read().await;
        assert_eq!(map.len(), 1);
    }

    #[tokio::test]
    async fn test_health_response_healthy_when_all_connected() {
        let state = HealthState::new(vec![
            ("ethereum".to_string(), 1),
            ("polygon".to_string(), 137),
        ]);
        state.set_connected("ethereum", true).await;
        state.set_connected("polygon", true).await;

        let map = state.inner.read().await;
        let chains: Vec<ChainHealth> = map.values().cloned().collect();
        let all_connected = chains.iter().all(|c| c.connected);
        assert!(all_connected);
    }

    #[tokio::test]
    async fn test_health_response_degraded_when_partial() {
        let state = HealthState::new(vec![
            ("ethereum".to_string(), 1),
            ("polygon".to_string(), 137),
        ]);
        state.set_connected("ethereum", true).await;
        // polygon remains disconnected

        let map = state.inner.read().await;
        let chains: Vec<ChainHealth> = map.values().cloned().collect();
        let all_connected = chains.iter().all(|c| c.connected);
        assert!(!all_connected);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn start_server(
    host: String,
    port: u16,
    pool: PgPool,
    event_tx: broadcast::Sender<NormalizedEvent>,
    ws_max_connections: usize,
    prometheus_handle: PrometheusHandle,
    health_state: HealthState,
    token: tokio_util::sync::CancellationToken,
) -> Result<(), std::io::Error> {
    let router = create_router(pool, event_tx, ws_max_connections, prometheus_handle, health_state);
    let addr = format!("{host}:{port}");
    tracing::info!(address = %addr, "Starting API server");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            token.cancelled().await;
            tracing::info!("API server: graceful shutdown initiated");
        })
        .await?;
    Ok(())
}

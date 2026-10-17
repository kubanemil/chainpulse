use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy::primitives::Address;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chainpulse_db::{self as db, DbError, EventQuery, EventRecord, PgPool};
use chainpulse_types::EventKind;
use metrics_exporter_prometheus::PrometheusHandle;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use crate::status::{ChainReport, ChainStatus};
use crate::ws::{self, Notification};

const DEFAULT_PAGE: u32 = 50;
const MAX_PAGE: u32 = 1_000;
const DB_PING_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct AppState {
    pub(crate) pool: PgPool,
    pub(crate) feed: broadcast::Sender<Notification>,
    pub(crate) ws_slots: Arc<Semaphore>,
    pub(crate) shutdown: CancellationToken,
    chains: Arc<[ChainStatus]>,
    metrics: PrometheusHandle,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        feed: broadcast::Sender<Notification>,
        chains: Vec<ChainStatus>,
        metrics: PrometheusHandle,
        max_ws_clients: usize,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            pool,
            feed,
            ws_slots: Arc::new(Semaphore::new(max_ws_clients)),
            shutdown,
            chains: chains.into(),
            metrics,
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/events", get(list_events))
        .route("/events/{id}", get(get_event))
        .route("/ws/events", get(ws::upgrade))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .with_state(state)
}

pub async fn serve(listener: TcpListener, state: AppState) -> std::io::Result<()> {
    let shutdown = state.shutdown.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    Unavailable(&'static str),
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
            Self::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message.to_owned()),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_owned(),
            ),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<DbError> for ApiError {
    fn from(error: DbError) -> Self {
        tracing::error!(%error, "database error while serving a request");
        Self::Internal
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    chain_id: Option<u64>,
    event_type: Option<EventKind>,
    contract: Option<Address>,
    from_block: Option<u64>,
    to_block: Option<u64>,
    confirmed: Option<bool>,
    cursor: Option<i64>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct EventPage {
    events: Vec<EventRecord>,
    next_cursor: Option<i64>,
}

async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<EventPage>, ApiError> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let query = EventQuery {
        chain_id: signed("chain_id", params.chain_id)?,
        event_type: params.event_type,
        contract: params.contract,
        from_block: signed("from_block", params.from_block)?,
        to_block: signed("to_block", params.to_block)?,
        confirmed: params.confirmed,
        before_id: params.cursor,
        limit: limit + 1,
    };
    let mut events = db::list_events(&state.pool, &query).await?;
    let next_cursor = if events.len() > limit as usize {
        events.truncate(limit as usize);
        events.last().map(|e| e.id)
    } else {
        None
    };
    Ok(Json(EventPage {
        events,
        next_cursor,
    }))
}

fn signed(name: &str, value: Option<u64>) -> Result<Option<i64>, ApiError> {
    value
        .map(|value| {
            i64::try_from(value)
                .map_err(|_| ApiError::BadRequest(format!("{name} is larger than {}", i64::MAX)))
        })
        .transpose()
}

async fn get_event(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<EventRecord>, ApiError> {
    db::event_by_id(&state.pool, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(Debug, Serialize)]
struct Readiness {
    ready: bool,
    database: bool,
    chains: Vec<ChainReport>,
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    let database = matches!(
        tokio::time::timeout(DB_PING_TIMEOUT, db::ping(&state.pool)).await,
        Ok(Ok(()))
    );
    let now = Instant::now();
    let chains: Vec<ChainReport> = state.chains.iter().map(|chain| chain.report(now)).collect();
    let ready = database && chains.iter().all(|chain| chain.ready);
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(Readiness {
            ready,
            database,
            chains,
        }),
    )
}

async fn metrics(State(state): State<AppState>) -> String {
    state.metrics.render()
}

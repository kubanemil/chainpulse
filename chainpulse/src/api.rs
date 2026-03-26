use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chainpulse_db::{EventFilter, EventRow};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 1000;
const DEFAULT_OFFSET: i64 = 0;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
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

pub fn create_router(pool: PgPool) -> Router {
    let state = AppState { pool };
    Router::new()
        .route("/events", get(list_events))
        .route("/events/{id}", get(get_event))
        .with_state(state)
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

pub async fn start_server(host: String, port: u16, pool: PgPool) -> Result<(), std::io::Error> {
    let router = create_router(pool);
    let addr = format!("{host}:{port}");
    tracing::info!(address = %addr, "Starting API server");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

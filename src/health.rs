use axum::{Router, routing::get, Json, response::IntoResponse, extract::State, http::StatusCode};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use utoipa::ToSchema;
use crate::stream::registry::StreamRegistry;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub active_streams: usize,
    pub uptime_seconds: u64,
    pub version: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct ReadyResponse {
    pub ready: bool,
}

pub struct HealthState {
    pub ready: AtomicBool,
    pub started_at: std::time::Instant,
    pub registry: Option<Arc<StreamRegistry>>,
}

impl HealthState {
    pub fn new(registry: Option<Arc<StreamRegistry>>) -> Arc<Self> {
        Arc::new(Self {
            ready: AtomicBool::new(true),
            started_at: std::time::Instant::now(),
            registry,
        })
    }
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service health information", body = HealthResponse),
    )
)]
pub async fn health_handler(State(state): State<Arc<HealthState>>) -> Json<HealthResponse> {
    let active_streams = state.registry
        .as_ref()
        .map(|r| r.len())
        .unwrap_or(0);
    Json(HealthResponse {
        status: "healthy".into(),
        active_streams,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[utoipa::path(
    get,
    path = "/ready",
    tag = "health",
    responses(
        (status = 200, description = "Service is ready", body = ReadyResponse),
        (status = 503, description = "Service is not ready", body = ReadyResponse),
    )
)]
pub async fn ready_handler(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    if state.ready.load(Ordering::Relaxed) {
        Json(ReadyResponse { ready: true }).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(ReadyResponse { ready: false })).into_response()
    }
}

pub fn health_router(state: Arc<HealthState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .with_state(state)
}

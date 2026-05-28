use std::sync::Arc;
use axum::{extract::{Query, State}, http::StatusCode, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct HistoryQuery {
    #[serde(default = "default_minutes")]
    pub minutes: i64,
}

fn default_minutes() -> i64 { 30 }

#[derive(Serialize, ToSchema)]
pub struct MetricsHistoryResponse {
    pub points: Vec<MetricsPointResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct MetricsPointResponse {
    pub recorded_at: String,
    pub streams_active: i32,
    pub frames_ps: f64,
    pub errors_decode: i32,
    pub errors_storage: i32,
    pub errors_kafka: i32,
    pub streams_claimed: i32,
}

pub fn metrics_routes(pool: Arc<MySqlPool>) -> Router {
    Router::new()
        .route("/history", axum::routing::get(history_handler))
        .with_state(pool)
}

#[utoipa::path(
    get,
    path = "/api/v1/metrics/history",
    tag = "metrics",
    params(
        ("minutes" = i64, Query, description = "Minutes of history to return (default 30)"),
    ),
    responses(
        (status = 200, description = "Metrics history", body = MetricsHistoryResponse),
    )
)]
pub async fn history_handler(
    State(pool): State<Arc<MySqlPool>>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<MetricsHistoryResponse>, (StatusCode, Json<serde_json::Value>)> {
    let rows = crate::db::metrics_history::query_recent(&pool, q.minutes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let points = rows.into_iter().map(|r| {
        MetricsPointResponse {
            recorded_at: r.recorded_at.to_rfc3339(),
            streams_active: r.streams_active,
            frames_ps: r.frames_ps,
            errors_decode: r.errors_decode,
            errors_storage: r.errors_storage,
            errors_kafka: r.errors_kafka,
            streams_claimed: r.streams_claimed,
        }
    }).collect();

    Ok(Json(MetricsHistoryResponse { points }))
}

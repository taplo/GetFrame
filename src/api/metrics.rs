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
    pub kafka_ps: f64,
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
            kafka_ps: r.kafka_ps,
            streams_claimed: r.streams_claimed,
        }
    }).collect();

    Ok(Json(MetricsHistoryResponse { points }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_point_response_serialization() {
        let p = MetricsPointResponse {
            recorded_at: "2026-06-03T12:00:00+00:00".into(),
            streams_active: 10,
            frames_ps: 1.5,
            errors_decode: 0,
            errors_storage: 1,
            errors_kafka: 2,
            kafka_ps: 0.5,
            streams_claimed: 8,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["recorded_at"], "2026-06-03T12:00:00+00:00");
        assert_eq!(json["streams_active"], 10);
        assert!((json["frames_ps"].as_f64().unwrap() - 1.5).abs() < f64::EPSILON);
        assert!((json["kafka_ps"].as_f64().unwrap() - 0.5).abs() < f64::EPSILON);
        assert_eq!(json["errors_kafka"], 2);
        assert_eq!(json["streams_claimed"], 8);
        assert!(json.get("kafka_ps").is_some());
    }

    #[test]
    fn test_metrics_point_response_zero_kafka() {
        let p = MetricsPointResponse {
            recorded_at: "2026-06-03T12:00:00+00:00".into(),
            streams_active: 5,
            frames_ps: 0.0,
            errors_decode: 0,
            errors_storage: 0,
            errors_kafka: 0,
            kafka_ps: 0.0,
            streams_claimed: 5,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert!((json["kafka_ps"].as_f64().unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((json["frames_ps"].as_f64().unwrap() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_history_response_wrapper() {
        let resp = MetricsHistoryResponse {
            points: vec![
                MetricsPointResponse {
                    recorded_at: "2026-06-03T12:00:00+00:00".into(),
                    streams_active: 10,
                    frames_ps: 2.0,
                    errors_decode: 0,
                    errors_storage: 1,
                    errors_kafka: 2,
                    kafka_ps: 0.5,
                    streams_claimed: 8,
                },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["points"].is_array());
        assert_eq!(json["points"][0]["kafka_ps"], 0.5);
        assert_eq!(json["points"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_metrics_point_response_all_fields_present() {
        let p = MetricsPointResponse {
            recorded_at: "2026-06-03T12:00:00+00:00".into(),
            streams_active: 10,
            frames_ps: 1.5,
            errors_decode: 0,
            errors_storage: 1,
            errors_kafka: 2,
            kafka_ps: 0.5,
            streams_claimed: 8,
        };
        let json = serde_json::to_value(&p).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("recorded_at"));
        assert!(obj.contains_key("streams_active"));
        assert!(obj.contains_key("frames_ps"));
        assert!(obj.contains_key("errors_decode"));
        assert!(obj.contains_key("errors_storage"));
        assert!(obj.contains_key("errors_kafka"));
        assert!(obj.contains_key("kafka_ps"));
        assert!(obj.contains_key("streams_claimed"));
        assert_eq!(obj.len(), 8);
    }
}

use metrics::{counter, gauge};
use std::sync::LazyLock;

pub static STREAMS_ACTIVE: LazyLock<metrics::Gauge> = LazyLock::new(|| {
    gauge!("getframe_streams_active")
});
pub static STREAMS_TOTAL: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_streams_total")
});
pub static FRAMES_PROCESSED: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_frames_processed_total")
});
#[allow(dead_code)]
pub static DECODE_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_decode_errors_total")
});
pub static STORAGE_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_storage_errors_total")
});
pub static KAFKA_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_kafka_errors_total")
});
pub static KAFKA_MESSAGES: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_kafka_messages_total")
});

pub static CLAIMED_STREAMS: LazyLock<metrics::Gauge> = LazyLock::new(|| {
    gauge!("getframe_streams_claimed")
});
pub static CLAIM_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_claim_errors_total")
});
pub static KAFKA_LAG: LazyLock<metrics::Gauge> = LazyLock::new(|| {
    gauge!("getframe_kafka_lag")
});

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
pub fn force_prometheus() -> PrometheusHandle {
    tracing::debug!("Forcing Prometheus recorder initialization");
    let handle = (*PROMETHEUS_HANDLE).clone();
    tracing::debug!("Prometheus recorder initialized");
    handle
}
static PROMETHEUS_HANDLE: LazyLock<PrometheusHandle> = LazyLock::new(|| {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
});

pub async fn metrics_handler() -> Result<String, (axum::http::StatusCode, &'static str)> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(|| PROMETHEUS_HANDLE.render()),
    )
    .await
    {
        Ok(Ok(text)) => Ok(text),
        _ => Err((axum::http::StatusCode::SERVICE_UNAVAILABLE, "metrics timeout")),
    }
}

use chrono::Utc;
use tokio::time::interval;

pub struct MetricsRecorder {
    pool: sqlx::MySqlPool,
    handle: PrometheusHandle,
    last_frames: i64,
    last_decode: i64,
    last_storage: i64,
    last_kafka: i64,
    last_kafka_messages: i64,
}

impl MetricsRecorder {
    pub fn new(pool: sqlx::MySqlPool) -> Self {
        let handle = PROMETHEUS_HANDLE.clone();
        let raw = handle.render();
        let last_frames = Self::extract_counter(&raw, "getframe_frames_processed_total");
        let last_decode = Self::extract_counter(&raw, "getframe_decode_errors_total");
        let last_storage = Self::extract_counter(&raw, "getframe_storage_errors_total");
        let last_kafka = Self::extract_counter(&raw, "getframe_kafka_errors_total");
        let last_kafka_messages = Self::extract_counter(&raw, "getframe_kafka_messages_total");
        Self { pool, handle, last_frames, last_decode, last_storage, last_kafka, last_kafka_messages }
    }

    pub fn sample(&mut self) -> crate::db::metrics_history::MetricsPoint {
        let raw = self.handle.render();
        let now = Utc::now();

        let frames = Self::extract_counter(&raw, "getframe_frames_processed_total");
        let dec = Self::extract_counter(&raw, "getframe_decode_errors_total");
        let st = Self::extract_counter(&raw, "getframe_storage_errors_total");
        let kaf = Self::extract_counter(&raw, "getframe_kafka_errors_total");
        let kaf_msg = Self::extract_counter(&raw, "getframe_kafka_messages_total");
        let active = Self::extract_gauge(&raw, "getframe_streams_active") as i32;
        let claimed = Self::extract_gauge(&raw, "getframe_streams_claimed") as i32;

        let frames_delta = (frames - self.last_frames).max(0) as i32;
        let errors_decode = (dec - self.last_decode).max(0) as i32;
        let errors_storage = (st - self.last_storage).max(0) as i32;
        let errors_kafka = (kaf - self.last_kafka).max(0) as i32;
        let kafka_delta = (kaf_msg - self.last_kafka_messages).max(0) as i32;

        self.last_frames = frames;
        self.last_decode = dec;
        self.last_storage = st;
        self.last_kafka = kaf;
        self.last_kafka_messages = kaf_msg;

        crate::db::metrics_history::MetricsPoint {
            recorded_at: now,
            streams_active: active,
            frames_delta,
            errors_decode,
            errors_storage,
            errors_kafka,
            kafka_delta,
            streams_claimed: claimed,
        }
    }

    pub async fn run(mut self, shutdown: tokio_util::sync::CancellationToken) {
        let mut tick = interval(std::time::Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tick.tick().await;
        let mut cleanup_tick = interval(std::time::Duration::from_secs(3600));
        cleanup_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        cleanup_tick.tick().await;

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("MetricsRecorder shutting down");
                    break;
                }
                _ = tick.tick() => {
                    let point = self.sample();
                    if let Err(e) = crate::db::metrics_history::insert(&self.pool, &point).await {
                        tracing::error!(error = %e, "Failed to record metrics snapshot");
                    }
                }
                _ = cleanup_tick.tick() => {
                    match crate::db::metrics_history::cleanup_old(&self.pool, 7).await {
                        Ok(n) => tracing::debug!(deleted = n, "Cleaned old metrics"),
                        Err(e) => tracing::error!(error = %e, "Metrics cleanup failed"),
                    }
                }
            }
        }
    }

    #[allow(clippy::collapsible_if)]
    fn extract_counter(raw: &str, name: &str) -> i64 {
        for line in raw.lines() {
            if !line.starts_with('#') && line.starts_with(name) {
                let after = line.as_bytes().get(name.len()).copied();
                if after.is_some_and(|c| c != b'{' && c != b' ' && c != b'\t') {
                    continue;
                }
                if let Some(val) = line.split_whitespace().last() {
                    if let Ok(v) = val.parse::<f64>() {
                        return v as i64;
                    }
                }
            }
        }
        0
    }

    #[allow(clippy::collapsible_if)]
    fn extract_gauge(raw: &str, name: &str) -> f64 {
        for line in raw.lines() {
            if !line.starts_with('#') && line.starts_with(name) {
                let after = line.as_bytes().get(name.len()).copied();
                if after.is_some_and(|c| c != b'{' && c != b' ' && c != b'\t') {
                    continue;
                }
                if let Some(val) = line.split_whitespace().last() {
                    if let Ok(v) = val.parse::<f64>() {
                        return v;
                    }
                }
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_prometheus_text() -> &'static str {
        r#"# HELP getframe_frames_processed_total Total frames processed
# TYPE getframe_frames_processed_total counter
getframe_frames_processed_total 42
# HELP getframe_kafka_messages_total Total Kafka messages published
# TYPE getframe_kafka_messages_total counter
getframe_kafka_messages_total 7
getframe_kafka_messages_total{stream_id="abc"} 3
# HELP getframe_streams_active Active streams
# TYPE getframe_streams_active gauge
getframe_streams_active 5
getframe_decode_errors_total 2
getframe_storage_errors_total 1
getframe_kafka_errors_total 0
getframe_streams_claimed 3"#
    }

    #[test]
    fn test_extract_counter_found() {
        assert_eq!(MetricsRecorder::extract_counter(sample_prometheus_text(), "getframe_frames_processed_total"), 42);
    }

    #[test]
    fn test_extract_counter_with_label() {
        assert_eq!(MetricsRecorder::extract_counter(sample_prometheus_text(), "getframe_kafka_messages_total"), 7);
    }

    #[test]
    fn test_extract_counter_not_found() {
        assert_eq!(MetricsRecorder::extract_counter(sample_prometheus_text(), "getframe_nonexistent"), 0);
    }

    #[test]
    fn test_extract_counter_partial_prefix_skipped() {
        assert_eq!(MetricsRecorder::extract_counter(sample_prometheus_text(), "getframe_decode_errors"), 0);
    }

    #[test]
    fn test_extract_gauge_found() {
        assert!((MetricsRecorder::extract_gauge(sample_prometheus_text(), "getframe_streams_active") - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_gauge_not_found() {
        assert!((MetricsRecorder::extract_gauge(sample_prometheus_text(), "getframe_nonexistent") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_all_metrics_present() {
        let raw = sample_prometheus_text();
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_frames_processed_total"), 42);
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_kafka_messages_total"), 7);
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_decode_errors_total"), 2);
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_storage_errors_total"), 1);
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_kafka_errors_total"), 0);
        assert!((MetricsRecorder::extract_gauge(raw, "getframe_streams_active") - 5.0).abs() < f64::EPSILON);
        assert!((MetricsRecorder::extract_gauge(raw, "getframe_streams_claimed") - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_handles_empty_string() {
        assert_eq!(MetricsRecorder::extract_counter("", "getframe_frames_processed_total"), 0);
        assert!((MetricsRecorder::extract_gauge("", "getframe_streams_active") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_handles_only_comments() {
        let raw = "# this is a comment\n# another comment\n";
        assert_eq!(MetricsRecorder::extract_counter(raw, "getframe_frames_processed_total"), 0);
    }
}

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
pub static DECODE_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_decode_errors_total")
});
pub static STORAGE_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_storage_errors_total")
});
pub static KAFKA_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_kafka_errors_total")
});

pub static CLAIMED_STREAMS: LazyLock<metrics::Gauge> = LazyLock::new(|| {
    gauge!("getframe_streams_claimed")
});
pub static CLAIM_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_claim_errors_total")
});

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
static PROMETHEUS_HANDLE: LazyLock<PrometheusHandle> = LazyLock::new(|| {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
});

pub async fn metrics_handler() -> String {
    PROMETHEUS_HANDLE.render()
}

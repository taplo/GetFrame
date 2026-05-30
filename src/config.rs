use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub preload_streams: Vec<StreamConfig>,
    pub storage: StorageConfig,
    pub kafka: KafkaConfig,
    pub http: HttpConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub database: Option<DatabaseConfig>,
    #[serde(default)]
    pub worker: Option<WorkerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,
}

fn default_db_max_connections() -> u32 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_claim_batch_size")]
    pub claim_batch_size: u32,
    #[serde(default = "default_claim_timeout")]
    pub claim_timeout_secs: u64,
    #[serde(default)]
    pub cpu_cores: String,
}

fn default_heartbeat_interval() -> u64 { 15 }
fn default_claim_batch_size() -> u32 { 5 }
fn default_claim_timeout() -> u64 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StreamConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: std::collections::HashMap<String, String>,
    pub source_url: String,
    pub source_type: String,
    pub stream_type: Option<String>,
    #[serde(default = "default_extract_interval")]
    pub extract_interval_seconds: f64,
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u8,
    #[serde(default = "default_ffmpeg_threads")]
    pub ffmpeg_threads: i32,
    #[serde(default)]
    pub rtsp_transport: String,
    #[serde(default)]
    pub storage: Option<StorageConfig>,
    #[serde(default)]
    pub kafka: Option<KafkaConfig>,
}

fn default_extract_interval() -> f64 { 5.0 }
fn default_jpeg_quality() -> u8 { 85 }
fn default_ffmpeg_threads() -> i32 { 1 }

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorageConfig {
    pub bucket: String,
    pub endpoint_url: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub retention_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KafkaConfig {
    pub brokers: String,
    pub topic: String,
    #[serde(default = "default_kafka_acks")]
    pub acks: String,
    #[serde(default = "default_kafka_compression")]
    pub compression: String,
    #[serde(default)]
    pub schema_registry_url: Option<String>,
    #[serde(default)]
    pub partition_key_field: Option<String>,
    #[serde(default = "default_kafka_consumer_group")]
    pub consumer_group: String,
}

fn default_kafka_consumer_group() -> String { "getframe-workers".into() }

fn default_kafka_acks() -> String { "all".into() }
fn default_kafka_compression() -> String { "zstd".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_http_addr")]
    pub bind_address: String,
    #[serde(default = "default_http_port")]
    pub bind_port: u16,
}

fn default_http_addr() -> String { "0.0.0.0".into() }
fn default_http_port() -> u16 { 8080 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_json")]
    pub json: bool,
}

fn default_log_level() -> String { "info".into() }
fn default_log_json() -> bool { true }

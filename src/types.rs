use uuid::Uuid;
use bytes::Bytes;

pub type StreamId = Uuid;
pub type FrameNumber = u64;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DecodedFrame {
    pub stream_id: StreamId,
    pub pts: i64,
    pub time_base: (i32, i32),
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    pub y_stride: i32,
    pub u_stride: i32,
    pub v_stride: i32,
    pub is_keyframe: bool,
    pub frame_number: FrameNumber,
    pub scene_change_score: Option<f64>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExtractedFrame {
    pub stream_id: StreamId,
    pub frame_number: FrameNumber,
    pub pts: i64,
    pub timestamp_seconds: f64,
    pub jpeg_bytes: Bytes,
    pub rule_trigger: String,
    pub jpeg_quality: u8,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrameMetadata {
    pub stream_id: String,
    pub source_type: String,
    pub timestamp: String,
    pub frame_number: u64,
    pub rule_trigger: String,
    pub pts: i64,
    pub storage_url: String,
    pub storage_bucket: String,
    pub storage_key: String,
    pub jpeg_size_bytes: u64,
    pub jpeg_width: u32,
    pub jpeg_height: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct KafkaHeaders {
    pub stream_id: String,
    pub source_type: String,
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum PipelineError {
    #[error("FFmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Kafka error: {0}")]
    Kafka(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

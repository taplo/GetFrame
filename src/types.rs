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

#[derive(Debug, Clone)]
pub enum PipelineExitReason {
    UserInitiated,
    Error(String),
    Eof,
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use bytes::Bytes;

    #[test]
    fn test_pipeline_exit_reason_user_initiated() {
        let reason = PipelineExitReason::UserInitiated;
        match &reason {
            PipelineExitReason::UserInitiated => (),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_pipeline_exit_reason_error() {
        let reason = PipelineExitReason::Error("test error".into());
        match &reason {
            PipelineExitReason::Error(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_pipeline_exit_reason_eof() {
        let reason = PipelineExitReason::Eof;
        match &reason {
            PipelineExitReason::Eof => (),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_stream_id_is_uuid() {
        let id = StreamId::new_v4();
        assert_eq!(id.to_string().len(), 36);
    }

    #[test]
    fn test_decoded_frame_defaults() {
        let frame = DecodedFrame {
            stream_id: Uuid::nil(),
            pts: 0,
            time_base: (1, 30),
            width: 640,
            height: 480,
            y_plane: vec![0u8; 640 * 480],
            u_plane: vec![128u8; 640 * 480 / 4],
            v_plane: vec![128u8; 640 * 480 / 4],
            y_stride: 640,
            u_stride: 320,
            v_stride: 320,
            is_keyframe: false,
            frame_number: 0,
            scene_change_score: None,
        };
        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 480);
        assert!(!frame.is_keyframe);
        assert!(frame.scene_change_score.is_none());
    }

    #[test]
    fn test_extracted_frame_creation() {
        let frame = ExtractedFrame {
            stream_id: Uuid::new_v4(),
            frame_number: 42,
            pts: 1260,
            timestamp_seconds: 42.0,
            jpeg_bytes: Bytes::from(vec![0xFF, 0xD8, 0xFF, 0x00]),
            rule_trigger: "scene_change".into(),
            jpeg_quality: 85,
            width: 1920,
            height: 1080,
        };
        assert_eq!(frame.frame_number, 42);
        assert_eq!(frame.rule_trigger, "scene_change");
        assert_eq!(frame.width, 1920);
    }

    #[test]
    fn test_frame_metadata_serialization() {
        let meta = FrameMetadata {
            stream_id: "test-stream".into(),
            source_type: "rtsp".into(),
            timestamp: "2026-06-01T00:00:00Z".into(),
            frame_number: 1,
            rule_trigger: "interval".into(),
            pts: 30,
            storage_url: "http://minio:9000/frames/test.jpg".into(),
            storage_bucket: "getframe-frames".into(),
            storage_key: "test-stream/1.jpg".into(),
            jpeg_size_bytes: 50000,
            jpeg_width: 1920,
            jpeg_height: 1080,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("test-stream"));
        assert!(json.contains("getframe-frames"));
        let deserialized: FrameMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stream_id, meta.stream_id);
        assert_eq!(deserialized.frame_number, meta.frame_number);
    }

    #[test]
    fn test_kafka_headers_serialization() {
        let headers = KafkaHeaders {
            stream_id: "stream-1".into(),
            source_type: "rtsp".into(),
        };
        let json = serde_json::to_string(&headers).unwrap();
        assert!(json.contains("stream-1"));
        let deserialized: KafkaHeaders = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_type, "rtsp");
    }

    #[test]
    fn test_pipeline_error_ffmpeg() {
        let err = PipelineError::Ffmpeg(ffmpeg_next::Error::Eof);
        assert!(err.to_string().contains("FFmpeg error"));
    }

    #[test]
    fn test_pipeline_error_storage() {
        let err = PipelineError::Storage("timeout".into());
        assert_eq!(err.to_string(), "Storage error: timeout");
    }

    #[test]
    fn test_pipeline_error_kafka() {
        let err = PipelineError::Kafka("broker unavailable".into());
        assert_eq!(err.to_string(), "Kafka error: broker unavailable");
    }

    #[test]
    fn test_pipeline_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = PipelineError::Io(io_err);
        assert!(err.to_string().contains("IO error"));
    }
}

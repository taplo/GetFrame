use crate::config::StreamConfig;

pub fn create_synthetic_config(index: usize, jpeg_quality: u8) -> StreamConfig {
    StreamConfig {
        name: format!("benchmark-stream-{}", index),
        description: String::new(),
        tags: std::collections::HashMap::new(),
        source_url: "lavfi://testsrc2=size=1920x1080:rate=30:duration=99999".into(),
        source_type: "lavfi".into(),
        stream_type: Some("benchmark".into()),
        extract_interval_seconds: 1.0,
        jpeg_quality,
        ffmpeg_threads: 1,
        rtsp_transport: "tcp".into(),
        storage: None,
        kafka: None,
    }
}

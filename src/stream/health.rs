use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum StreamStatus {
    Online,
    #[allow(dead_code)]
    Offline,
    Error(String),
    Connecting,
}

#[derive(Debug, Clone)]
pub struct StreamHealth {
    pub status: StreamStatus,
    pub last_online: Option<DateTime<Utc>>,
    pub last_error: Option<DateTime<Utc>>,
    pub error_count: u64,
    pub uptime_seconds: u64,
    pub frames_decoded: u64,
    pub frames_extracted: u64,
    pub reconnect_count: u64,
    pub last_pts: Option<i64>,
    pub latest_frame_key: Option<String>,
}

impl StreamHealth {
    pub fn new() -> Self {
        Self {
            status: StreamStatus::Connecting,
            last_online: None,
            last_error: None,
            error_count: 0,
            uptime_seconds: 0,
            frames_decoded: 0,
            frames_extracted: 0,
            reconnect_count: 0,
            last_pts: None,
            latest_frame_key: None,
        }
    }

    pub fn record_frame_stored(&mut self, key: String) {
        self.frames_extracted += 1;
        self.latest_frame_key = Some(key);
    }

    #[allow(dead_code)]
    pub fn record_decode_frame(&mut self) {
        self.frames_decoded += 1;
    }

    #[allow(dead_code)]
    pub fn record_extracted_frame(&mut self) {
        self.frames_extracted += 1;
    }

    #[allow(dead_code)]
    pub fn record_pts(&mut self, pts: i64) {
        self.last_pts = Some(pts);
    }

    pub fn mark_online(&mut self) {
        self.status = StreamStatus::Online;
        self.last_online = Some(Utc::now());
    }

    pub fn mark_error(&mut self, error: &str) {
        self.status = StreamStatus::Error(error.to_string());
        self.last_error = Some(Utc::now());
        self.error_count += 1;
    }

    pub fn mark_connecting(&mut self) {
        self.status = StreamStatus::Connecting;
    }

    pub fn mark_reconnected(&mut self) {
        self.reconnect_count += 1;
    }
}

impl Default for StreamHealth {
    fn default() -> Self {
        Self::new()
    }
}

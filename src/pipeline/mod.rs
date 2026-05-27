pub mod ingest;
pub mod decode;
pub mod rule;
pub mod encode;
pub mod filter;
pub mod pin;
pub use pin::parse_cpu_cores;

use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::AtomicU64;
use crossbeam::channel::{bounded, Receiver};
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::types::*;
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    pub decode_handle: Option<JoinHandle<()>>,
    pub extracted_rx: Receiver<ExtractedFrame>,
    pub shutdown_token: tokio_util::sync::CancellationToken,
    pub frames_decoded: Arc<AtomicU64>,
    pub frames_extracted: Arc<AtomicU64>,
}

const DECODE_TO_EXTRACT_CAPACITY: usize = 8;

impl Pipeline {
    pub fn start(
        stream_config: &crate::config::StreamConfig,
        stream_id: StreamId,
        shutdown_token: tokio_util::sync::CancellationToken,
        health_handle: Arc<Mutex<StreamHealth>>,
        rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
        core_id: Option<usize>,
    ) -> Self {
        let (extract_tx, extract_rx) = bounded::<ExtractedFrame>(DECODE_TO_EXTRACT_CAPACITY);

        let source_url = stream_config.source_url.clone();
        let source_type = stream_config.source_type.clone();
        let interval = stream_config.extract_interval_seconds;
        let jpeg_quality = stream_config.jpeg_quality;
        let ffmpeg_threads = stream_config.ffmpeg_threads;
        let rtsp_transport = stream_config.rtsp_transport.clone();
        let stream_id_clone = stream_id;
        let shutdown = shutdown_token.clone();

        let frames_decoded = Arc::new(AtomicU64::new(0));
        let frames_extracted = Arc::new(AtomicU64::new(0));
        let fd = frames_decoded.clone();
        let fe = frames_extracted.clone();

        let handle = thread::Builder::new()
            .name(format!("stream-{}", stream_id))
            .spawn(move || {
                if let Some(cid) = core_id {
                    pin::pin_current_thread(cid);
                }
                let result = decode::run_decode_pipeline(
                    &source_url,
                    &source_type,
                    &rtsp_transport,
                    ffmpeg_threads,
                    stream_id_clone,
                    interval,
                    jpeg_quality,
                    extract_tx,
                    shutdown,
                    health_handle,
                    rules_shared,
                    fd,
                    fe,
                );
                if let Err(e) = result {
                    tracing::error!(error = %e, stream_id = %stream_id_clone, "Pipeline terminated with error");
                }
            })
            .expect("Failed to spawn pipeline thread");

        Pipeline {
            decode_handle: Some(handle),
            extracted_rx: extract_rx,
            shutdown_token,
            frames_decoded,
            frames_extracted,
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown_token.cancel();
        if let Some(handle) = self.decode_handle.take() {
            let _ = handle.join();
        }
    }
}

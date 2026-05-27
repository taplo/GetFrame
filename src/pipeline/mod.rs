pub mod ingest;
pub mod decode;
pub mod rule;
pub mod encode;
pub mod filter;

use std::sync::{Arc, Mutex, RwLock};
use crossbeam::channel::{bounded, Receiver};
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::types::*;
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    pub decode_handle: Option<JoinHandle<()>>,
    pub extracted_rx: Receiver<ExtractedFrame>,
    pub shutdown_token: tokio_util::sync::CancellationToken,
}

const DECODE_TO_EXTRACT_CAPACITY: usize = 8;

impl Pipeline {
    pub fn start(
        stream_config: &crate::config::StreamConfig,
        stream_id: StreamId,
        shutdown_token: tokio_util::sync::CancellationToken,
        health_handle: Arc<Mutex<StreamHealth>>,
        rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
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

        let handle = thread::Builder::new()
            .name(format!("stream-{}", stream_id))
            .spawn(move || {
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
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown_token.cancel();
        if let Some(handle) = self.decode_handle.take() {
            let _ = handle.join();
        }
    }
}

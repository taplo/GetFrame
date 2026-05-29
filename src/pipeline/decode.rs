use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use ffmpeg_next as ffmpeg;
use crossbeam::channel::Sender;
use std::collections::BTreeMap;
use crate::pipeline::rule::{RuleConfig, RuleEngine};
use crate::stream::health::StreamHealth;
use crate::types::*;
use crate::pipeline::{ingest, encode};

#[allow(clippy::too_many_arguments)]
pub fn run_decode_pipeline(
    source_url: &str,
    source_type: &str,
    rtsp_transport: &str,
    ffmpeg_threads: i32,
    stream_id: StreamId,
    _interval_seconds: f64,
    jpeg_quality: u8,
    frame_tx: Sender<ExtractedFrame>,
    shutdown: tokio_util::sync::CancellationToken,
    health_handle: Arc<Mutex<StreamHealth>>,
    rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
    frames_decoded_counter: Arc<AtomicU64>,
    frames_extracted_counter: Arc<AtomicU64>,
) -> Result<(), anyhow::Error> {
    tracing::info!(stream_id = %stream_id, source_url = %source_url, "Starting decode pipeline");

    let mut demuxed = ingest::open_video_source(source_url, source_type, rtsp_transport, ffmpeg_threads)?;
    let time_base = demuxed.time_base;
    let tb_f = time_base.0 as f64 / time_base.1 as f64;

    // Mark online
    {
        let mut h = health_handle.lock().unwrap();
        h.mark_online();
    }

    let mut frame_number: u64 = 0;
    let mut pts_queue: BTreeMap<i64, DecodedFrame> = BTreeMap::new();
    let mut reorder_depth: usize = 0;
    let mut first_keyframe_seen = false;
    let mut total_frames_decoded: u64 = 0;
    let mut health_counter: u64 = 0;

    let mut rule_engine = {
        let rules = rules_shared.read().unwrap().clone();
        RuleEngine::new(&rules, (time_base.0, time_base.1))
    };

    // Initialize scene detection filter if rules require it
    // Must happen after decoder is opened (pixel format known)
    if rule_engine.scd_enabled() {
        rule_engine.init_scdet_filter(
            demuxed.width,
            demuxed.height,
            demuxed.decoder.format(),
            demuxed.time_base,
        );
    }

    for (stream_idx, recv_packet) in demuxed.ictx.packets() {
        if shutdown.is_cancelled() {
            tracing::info!("Decode pipeline shutting down");
            break;
        }

        if stream_idx.index() != demuxed.video_stream_index {
            continue;
        }

        if let Err(e) = demuxed.decoder.send_packet(&recv_packet) {
            tracing::warn!(stream_id = %stream_id, error = %e, "Failed to send packet to decoder, skipping");
            continue;
        }

        let mut frame = ffmpeg::util::frame::Video::empty();

        loop {
            match demuxed.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    total_frames_decoded += 1;
                    frames_decoded_counter.fetch_add(1, Ordering::Relaxed);
                    let pts = frame.pts().unwrap_or(0);
                    let is_key = frame.is_key();

                    if !first_keyframe_seen {
                        if is_key {
                            first_keyframe_seen = true;
                            tracing::info!(stream_id = %stream_id, pts = pts, "First keyframe received");
                        } else {
                            continue;
                        }
                    }

                    // Scene detection: push raw frame through scdet filter if enabled
                    let scene_change_score = if rule_engine.scd_enabled() {
                        match &mut rule_engine.scdet_filter {
                            Some(filter) => filter.filter(&frame).ok(),
                            None => None,
                        }
                    } else {
                        None
                    };

                    let decoded = DecodedFrame {
                        stream_id,
                        pts,
                        time_base: (time_base.0, time_base.1),
                        width: demuxed.width,
                        height: demuxed.height,
                        y_plane: frame.data(0).to_vec(),
                        u_plane: frame.data(1).to_vec(),
                        v_plane: frame.data(2).to_vec(),
                        y_stride: frame.stride(0) as i32,
                        u_stride: frame.stride(1) as i32,
                        v_stride: frame.stride(2) as i32,
                        is_keyframe: is_key,
                        frame_number: total_frames_decoded - 1,
                        scene_change_score,
                    };

                    pts_queue.insert(pts, decoded);
                    reorder_depth = std::cmp::max(reorder_depth, pts_queue.len());

                    while pts_queue.len() > 2 {
                        if let Some((_, ready_frame)) = pts_queue.pop_first() {
                            // Hot-reload: rebuild engine from shared rules every frame
                            // (lock is uncontended; overhead neglible vs JPEG encode)
                            {
                                let rules = rules_shared.read().unwrap();
                                rule_engine.rebuild(&rules, (time_base.0, time_base.1));
                                // Re-init scdet filter if SCD rules changed
                                if rule_engine.scd_enabled() && rule_engine.scdet_filter.is_none() {
                                    rule_engine.init_scdet_filter(
                                        demuxed.width,
                                        demuxed.height,
                                        demuxed.decoder.format(),
                                        demuxed.time_base,
                                    );
                                }
                            }

                            if rule_engine.evaluate(&ready_frame) {
                                match encode::encode_jpeg(&ready_frame, jpeg_quality) {
                                    Ok(jpeg_bytes) => {
                                        let timestamp_seconds = ready_frame.pts as f64 * tb_f;
                                        let extracted = ExtractedFrame {
                                            stream_id,
                                            frame_number,
                                            pts: ready_frame.pts,
                                            timestamp_seconds,
                                            jpeg_bytes,
                                            rule_trigger: "rule".to_string(),
                                            jpeg_quality,
                                            width: ready_frame.width,
                                            height: ready_frame.height,
                                        };
                                        frame_number += 1;
                                        frames_extracted_counter.fetch_add(1, Ordering::Relaxed);

                                        if frame_tx.send(extracted).is_err() {
                                            tracing::warn!("Extracted frame channel closed, stopping pipeline");
                                            return Ok(());
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(stream_id = %stream_id, error = %e, "JPEG encoding failed, skipping frame");
                                    }
                                }
                            }

                            // Periodic health update (every 30 frames)
                            health_counter += 1;
                            #[allow(clippy::manual_is_multiple_of)]
                            if health_counter % 30 == 0 {
                                let mut h = health_handle.lock().unwrap();
                                h.frames_decoded = total_frames_decoded;
                                h.frames_extracted = frame_number;
                                h.last_pts = Some(pts);
                            }
                        }
                    }
                }
                Err(ffmpeg::Error::Eof) | Err(ffmpeg::Error::Other { errno: ffmpeg::error::EAGAIN }) => break,
                Err(e) => {
                    tracing::warn!(stream_id = %stream_id, error = %e, "Error receiving frame, skipping");
                    break;
                }
            }
        }
    }

    // Flush decoder to release delayed frames (e.g. B-frames)
    if let Err(e) = demuxed.decoder.send_eof() {
        tracing::warn!(stream_id = %stream_id, error = %e, "Failed to send EOF to decoder");
    } else {
        let mut frame = ffmpeg::util::frame::Video::empty();
        loop {
            match demuxed.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    total_frames_decoded += 1;
                    frames_decoded_counter.fetch_add(1, Ordering::Relaxed);
                    let pts = frame.pts().unwrap_or(0);
                    let is_key = frame.is_key();
                    let decoded = DecodedFrame {
                        stream_id,
                        pts,
                        time_base: (time_base.0, time_base.1),
                        width: demuxed.width,
                        height: demuxed.height,
                        y_plane: frame.data(0).to_vec(),
                        u_plane: frame.data(1).to_vec(),
                        v_plane: frame.data(2).to_vec(),
                        y_stride: frame.stride(0) as i32,
                        u_stride: frame.stride(1) as i32,
                        v_stride: frame.stride(2) as i32,
                        is_keyframe: is_key,
                        frame_number: total_frames_decoded - 1,
                        scene_change_score: None,
                    };
                    pts_queue.insert(pts, decoded);
                }
                Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    tracing::warn!(stream_id = %stream_id, error = %e, "Error receiving flushed frame");
                    break;
                }
            }
        }
    }

    // Drain remaining frames
    while let Some((_, ready_frame)) = pts_queue.pop_first() {
        if rule_engine.evaluate(&ready_frame) {
            if let Ok(jpeg_bytes) = encode::encode_jpeg(&ready_frame, jpeg_quality) {
                let timestamp_seconds = ready_frame.pts as f64 * tb_f;
                let extracted = ExtractedFrame {
                    stream_id,
                    frame_number,
                    pts: ready_frame.pts,
                    timestamp_seconds,
                    jpeg_bytes,
                    rule_trigger: "rule".to_string(),
                    jpeg_quality,
                    width: ready_frame.width,
                    height: ready_frame.height,
                };
                frame_number += 1;
                frames_extracted_counter.fetch_add(1, Ordering::Relaxed);
                let _ = frame_tx.send(extracted);
            }
        }
    }

    tracing::info!(
        stream_id = %stream_id,
        frames_decoded = total_frames_decoded,
        frames_extracted = frame_number,
        reorder_depth = reorder_depth,
        "Decode pipeline finished"
    );
    Ok(())
}

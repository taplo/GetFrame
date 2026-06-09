use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::AtomicU64;
use crossbeam::channel::bounded;
use tokio_util::sync::CancellationToken;
use getframe_worker::pipeline::ingest::open_video_source;

fn get_test_video_path() -> String {
    std::env::var("TEST_VIDEO_PATH")
        .unwrap_or_else(|_| "tests/fixtures/test.mp4".into())
}

#[test]
fn test_open_video_source() {
    ffmpeg_next::init().unwrap();
    let demuxed = open_video_source(&get_test_video_path(), "file", "tcp", 1)
        .expect("open_video_source failed");
    assert_eq!(demuxed.width, 960);
    assert_eq!(demuxed.height, 540);

    assert!(demuxed.time_base.0 > 0 && demuxed.time_base.1 > 0);
}

#[tokio::test]
async fn test_encode_jpeg() {
    use getframe_worker::pipeline::encode::encode_jpeg;
    use getframe_worker::types::DecodedFrame;
    let frame = DecodedFrame {
        stream_id: uuid::Uuid::new_v4(),
        pts: 0,
        time_base: (1, 30),
        width: 320,
        height: 240,
        y_plane: vec![128u8; 320 * 240],
        u_plane: vec![128u8; 320 * 240 / 4],
        v_plane: vec![128u8; 320 * 240 / 4],
        y_stride: 320,
        u_stride: 160,
        v_stride: 160,
        is_keyframe: true,
        frame_number: 0,
        scene_change_score: None,
        static_frame_score: None,
    };
    let bytes = encode_jpeg(&frame, 85).unwrap();
    assert!(!bytes.is_empty(), "JPEG output is empty");
    assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF], "Not a valid JPEG header");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_decode_pipeline_full() {
    use getframe_worker::pipeline::decode::run_decode_pipeline;
    use getframe_worker::pipeline::rule::RuleConfig;
    use getframe_worker::stream::health::StreamHealth;
    ffmpeg_next::init().unwrap();

    let stream_id = uuid::Uuid::new_v4();
    let (tx, rx) = bounded(16);
    let shutdown = CancellationToken::new();
    let health = Arc::new(Mutex::new(StreamHealth::new()));
    let rules = Arc::new(RwLock::new(vec![
        RuleConfig::Interval { interval_seconds: 0.0 },
    ]));
    let decoded = Arc::new(AtomicU64::new(0));
    let extracted = Arc::new(AtomicU64::new(0));

    let path = get_test_video_path();
    let tx_clone = tx.clone();
    let shutdown_clone = shutdown.clone();
    let h = health.clone();
    let r = rules.clone();
    let d = decoded.clone();
    let e = extracted.clone();

    std::thread::spawn(move || {
        let _ = run_decode_pipeline(
            &path, "file", "tcp", 1, stream_id, 0.0, 85, tx_clone,
            shutdown_clone, h, r, d, e,
        );
    });

    drop(tx);
    let mut frames = Vec::new();
    for _ in 0..5 {
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(frame) => frames.push(frame),
            Err(_) => break,
        }
    }

    shutdown.cancel();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    assert!(!frames.is_empty(), "Pipeline should produce at least 1 frame");
    assert!(
        frames.len() >= 3,
        "Expected at least 3 frames, got {}",
        frames.len()
    );
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f.width, 960);
        assert_eq!(f.height, 540);
        assert!(!f.jpeg_bytes.is_empty());
        if i > 0 {
            assert!(f.frame_number > frames[i - 1].frame_number, "Frame number should increase");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_decode_pipeline_early_cancel() {
    use getframe_worker::pipeline::decode::run_decode_pipeline;
    use getframe_worker::pipeline::rule::RuleConfig;
    use getframe_worker::stream::health::StreamHealth;
    ffmpeg_next::init().unwrap();

    let stream_id = uuid::Uuid::new_v4();
    let (tx, rx) = bounded(16);
    let shutdown = CancellationToken::new();
    let health = Arc::new(Mutex::new(StreamHealth::new()));
    let rules = Arc::new(RwLock::new(vec![
        RuleConfig::Interval { interval_seconds: 0.0 },
    ]));
    let decoded = Arc::new(AtomicU64::new(0));
    let extracted = Arc::new(AtomicU64::new(0));

    let path = get_test_video_path();
    let shutdown_clone = shutdown.clone();

    std::thread::spawn(move || {
        let _ = run_decode_pipeline(
            &path, "file", "tcp", 1, stream_id, 0.0, 85, tx,
            shutdown_clone, health, rules, decoded, extracted,
        );
    });

    let mut count = 0;
    while rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok() {
        count += 1;
        if count >= 2 {
            shutdown.cancel();
            break;
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(count, 2, "Should have received exactly 2 frames before cancel");
}

#[test]
fn test_scene_detection_filter_detects_cut() {
    ffmpeg_next::init().unwrap();

    let path = "tests/fixtures/scene_change.mp4";
    let mut demuxed = getframe_worker::pipeline::ingest::open_video_source(path, "file", "tcp", 1)
        .expect("Failed to open scene_change.mp4");

    let mut filter = getframe_worker::pipeline::filter::SceneDetectFilter::new(
        demuxed.width,
        demuxed.height,
        demuxed.decoder.format(),
        demuxed.time_base,
        0.3,
    )
    .expect("Failed to create SceneDetectFilter");

    let mut scores = Vec::new();
    let mut frame = ffmpeg_next::util::frame::Video::empty();

    'outer: for (stream_idx, recv_packet) in demuxed.ictx.packets() {
        if stream_idx.index() != demuxed.video_stream_index {
            continue;
        }
        let _ = demuxed.decoder.send_packet(&recv_packet);
        loop {
            match demuxed.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    let score = filter.filter(&frame).unwrap_or(0.0);
                    assert!(score >= 0.0, "Score {} should be non-negative", score);
                    scores.push(score);
                    if scores.len() >= 60 {
                        break 'outer;
                    }
                }
                Err(ffmpeg_next::Error::Eof) => break,
                Err(_) => break,
            }
        }
    }

    assert!(scores.len() >= 30, "Expected >=30 frames, got {}", scores.len());

    // Early frames (all red) should have low scores (same scene, no change)
    let early_avg: f64 = scores.iter().take(10).sum::<f64>() / 10.0;
    assert!(
        early_avg < 1.0,
        "Early frames (constant color) should avg <1.0, got {}",
        early_avg
    );

    // At least one frame should spike significantly (red→blue transition)
    let max_score = scores.iter().copied().fold(0.0f64, f64::max);
    assert!(
        max_score > 5.0,
        "Scene cut should produce score >5.0, got max {} (scores: {:?})",
        max_score,
        scores
    );

    // The spike should be 10x+ the early average
    assert!(
        max_score > early_avg * 10.0,
        "Scene cut score {} should be 10x early avg {}",
        max_score,
        early_avg
    );

    // Late frames (all blue) should return to low scores
    if scores.len() > 50 {
        let late_avg: f64 = scores
            .iter()
            .skip(scores.len().saturating_sub(10))
            .sum::<f64>()
            / 10.0;
        assert!(
            late_avg < 1.0,
            "Late frames (constant color) should avg <1.0, got {}",
            late_avg
        );
    }
}

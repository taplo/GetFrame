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
    let result = open_video_source(&get_test_video_path(), "file", "tcp", 1);
    assert!(result.is_ok(), "open_video_source failed: {:?}", result.err());
    let demuxed = result.unwrap();
    assert_eq!(demuxed.width, 320);
    assert_eq!(demuxed.height, 240);

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
    };
    let jpeg = encode_jpeg(&frame, 85);
    assert!(jpeg.is_ok(), "encode_jpeg failed: {:?}", jpeg.err());
    let bytes = jpeg.unwrap();
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

    let mut frames = Vec::new();
    for _ in 0..5 {
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(frame) => frames.push(frame),
            Err(_) => break,
        }
    }

    shutdown.cancel();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    assert_eq!(frames.len(), 5, "Expected 5 frames, got {}", frames.len());
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f.width, 320);
        assert_eq!(f.height, 240);
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

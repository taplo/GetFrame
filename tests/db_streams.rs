mod common;

use uuid::Uuid;
use getframe_worker::db::streams::{load_all, upsert, delete};
use getframe_worker::config::StreamConfig;

fn make_stream(name: &str) -> (Uuid, StreamConfig) {
    let id = Uuid::new_v4();
    let config = StreamConfig {
        name: name.into(),
        description: String::new(),
        tags: std::collections::HashMap::new(),
        source_url: "file:///tmp/test.h264".into(),
        source_type: "file".into(),
        stream_type: None,
        extract_interval_seconds: 1.0,
        jpeg_quality: 85,
        ffmpeg_threads: 1,
        rtsp_transport: "tcp".into(),
        storage: None,
        kafka: None,
    };
    (id, config)
}

#[tokio::test]
async fn test_db_streams_empty_list() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let streams = load_all(&pool).await.unwrap();
    assert!(streams.is_empty());
}

#[tokio::test]
async fn test_db_streams_insert_and_list() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let (id, config) = make_stream("test-stream");
    upsert(&pool, &id, &config).await.unwrap();

    let all = load_all(&pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0, id);
    assert_eq!(all[0].1.name, "test-stream");

    delete(&pool, &id).await.unwrap();
    assert!(load_all(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_db_streams_upsert_update() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let (id, _) = make_stream("original");
    upsert(&pool, &id, &make_stream("original").1).await.unwrap();
    upsert(&pool, &id, &make_stream("updated").1).await.unwrap();

    let all = load_all(&pool).await.unwrap();
    assert_eq!(all[0].1.name, "updated");
}

#[tokio::test]
async fn test_db_streams_json_fields() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let id = Uuid::new_v4();
    let mut tags = std::collections::HashMap::new();
    tags.insert("env".into(), "test".into());
    tags.insert("region".into(), "us-east-1".into());
    let config = StreamConfig {
        tags,
        ..make_stream("json-test").1
    };
    upsert(&pool, &id, &config).await.unwrap();

    let all = load_all(&pool).await.unwrap();
    assert_eq!(all[0].1.tags.get("env").unwrap(), "test");
    assert_eq!(all[0].1.tags.get("region").unwrap(), "us-east-1");
}

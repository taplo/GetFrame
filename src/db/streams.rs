use sqlx::{FromRow, MySqlPool};
use crate::config::StreamConfig;
use crate::types::StreamId;

#[derive(FromRow)]
struct StreamRow {
    id: uuid::Uuid,
    name: String,
    description: String,
    tags: serde_json::Value,
    source_url: String,
    source_type: String,
    stream_type: Option<String>,
    extract_interval_seconds: f64,
    jpeg_quality: i32,
    ffmpeg_threads: i32,
    rtsp_transport: String,
    storage_config: Option<serde_json::Value>,
    kafka_config: Option<serde_json::Value>,
}

pub async fn load_all(pool: &MySqlPool) -> Result<Vec<(StreamId, StreamConfig)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, StreamRow>(
        r#"SELECT id, name, description, tags, source_url, source_type, stream_type,
                  extract_interval_seconds, jpeg_quality, ffmpeg_threads, rtsp_transport,
                  storage_config, kafka_config
           FROM streams
           ORDER BY created_at"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(|r| {
        let tags: std::collections::HashMap<String, String> =
            serde_json::from_value(r.tags).unwrap_or_default();
        let storage = r.storage_config
            .and_then(|v| serde_json::from_value(v).ok());
        let kafka = r.kafka_config
            .and_then(|v| serde_json::from_value(v).ok());
        Some((
            r.id,
            StreamConfig {
                name: r.name,
                description: r.description,
                tags,
                source_url: r.source_url,
                source_type: r.source_type,
                stream_type: r.stream_type,
                extract_interval_seconds: r.extract_interval_seconds,
                jpeg_quality: r.jpeg_quality as u8,
                ffmpeg_threads: r.ffmpeg_threads,
                rtsp_transport: r.rtsp_transport,
                storage,
                kafka,
            },
        ))
    }).collect())
}

pub async fn upsert(pool: &MySqlPool, id: &StreamId, config: &StreamConfig) -> Result<(), sqlx::Error> {
    let tags = serde_json::to_value(&config.tags).unwrap_or_default();
    let storage_json = config.storage.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default());
    let kafka_json = config.kafka.as_ref().map(|k| serde_json::to_value(k).unwrap_or_default());

    sqlx::query(
        r#"INSERT INTO streams (id, name, description, tags, source_url, source_type, stream_type,
                                extract_interval_seconds, jpeg_quality, ffmpeg_threads, rtsp_transport,
                                storage_config, kafka_config)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
               name = VALUES(name), description = VALUES(description),
               tags = VALUES(tags), source_url = VALUES(source_url),
               source_type = VALUES(source_type), stream_type = VALUES(stream_type),
               extract_interval_seconds = VALUES(extract_interval_seconds),
               jpeg_quality = VALUES(jpeg_quality), ffmpeg_threads = VALUES(ffmpeg_threads),
               rtsp_transport = VALUES(rtsp_transport),
               storage_config = VALUES(storage_config), kafka_config = VALUES(kafka_config)"#
    )
    .bind(id)
    .bind(&config.name)
    .bind(&config.description)
    .bind(tags)
    .bind(&config.source_url)
    .bind(&config.source_type)
    .bind(&config.stream_type)
    .bind(config.extract_interval_seconds)
    .bind(config.jpeg_quality as i32)
    .bind(config.ffmpeg_threads)
    .bind(&config.rtsp_transport)
    .bind(storage_json)
    .bind(kafka_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &MySqlPool, id: &StreamId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM streams WHERE id = ?")
    .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

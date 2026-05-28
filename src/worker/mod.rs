use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use sqlx::MySqlPool;
use std::collections::HashSet;

use crate::config::{StreamConfig, WorkerConfig};
use crate::stream::StreamManager;
use crate::types::StreamId;

pub struct WorkerManager {
    pub worker_id: String,
    db_pool: MySqlPool,
    stream_manager: StreamManager,
    config: WorkerConfig,
    shutdown_token: CancellationToken,
}

impl WorkerManager {
    pub fn new(
        worker_id: String,
        db_pool: MySqlPool,
        stream_manager: StreamManager,
        config: WorkerConfig,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self { worker_id, db_pool, stream_manager, config, shutdown_token }
    }

    pub async fn run(self: Arc<Self>) {
        tracing::info!(worker_id = %self.worker_id, "WorkerManager started");

        let _ = sqlx::query(
            "INSERT INTO workers (id, heartbeat_at) VALUES (?, NOW()) \
             ON DUPLICATE KEY UPDATE heartbeat_at = NOW()"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await;

        self.claim_loop_iteration().await;

        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.heartbeat_interval_secs)
        );
        interval.tick().await;

        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    tracing::info!(worker_id = %self.worker_id, "WorkerManager shutting down");
                    self.release_all_claims().await;
                    break;
                }
                _ = interval.tick() => {
                    self.claim_loop_iteration().await;
                }
            }
        }
    }

    async fn claim_loop_iteration(&self) {
        self.heartbeat().await;
        self.claim_streams().await;
        self.cleanup_orphaned_streams().await;
    }

    async fn heartbeat(&self) {
        let result = sqlx::query(
            "UPDATE workers SET heartbeat_at = NOW() WHERE id = ?"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await;

        match result {
            Ok(_) => tracing::trace!(worker_id = %self.worker_id, "Heartbeat"),
            Err(e) => tracing::warn!(error = %e, worker_id = %self.worker_id, "Heartbeat failed"),
        }
    }

    async fn claim_streams(&self) {
        let timeout_interval = self.config.claim_timeout_secs as i64;
        let batch_size = self.config.claim_batch_size as i64;

        // Step 1: Atomically claim expired or unclaimed streams
        let updated = sqlx::query(
            r#"UPDATE streams SET claimed_by = ?, claimed_at = NOW()
               WHERE id IN (
                   SELECT id FROM (
                       SELECT id FROM streams
                       WHERE claimed_by IS NULL
                          OR claimed_at < NOW() - INTERVAL ? SECOND
                       ORDER BY created_at ASC
                       LIMIT ?
                   ) AS tmp
               )"#
        )
        .bind(&self.worker_id)
        .bind(timeout_interval)
        .bind(batch_size)
        .execute(&self.db_pool)
        .await;

        let affected = match updated {
            Ok(r) => r.rows_affected() as usize,
            Err(e) => {
                tracing::error!(error = %e, worker_id = %self.worker_id, "Failed to claim streams");
                crate::metrics::CLAIM_ERRORS.increment(1);
                return;
            }
        };

        if affected == 0 {
            return;
        }

        // Step 2: Fetch the newly claimed streams
        let rows = match sqlx::query_as::<_, ClaimRow>(
            r#"SELECT id, name, description, tags, source_url, source_type, stream_type,
                      extract_interval_seconds, jpeg_quality, ffmpeg_threads, rtsp_transport,
                      storage_config, kafka_config
               FROM streams
               WHERE claimed_by = ? AND claimed_at >= NOW() - INTERVAL 2 SECOND
               ORDER BY created_at ASC"#
        )
        .bind(&self.worker_id)
        .fetch_all(&self.db_pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!(error = %e, worker_id = %self.worker_id, "Failed to fetch claimed streams");
                crate::metrics::CLAIM_ERRORS.increment(1);
                return;
            }
        };

        for row in &rows {
            let id: StreamId = row.id;
            if self.stream_manager.registry().exists(&id) {
                self.stream_manager.start_pipeline(&id);
                tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Claimed existing stream");
            } else {
                let config = row_to_config(row);
                self.stream_manager.registry().add(id, config.clone());
                self.stream_manager.start_pipeline(&id);
                tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Claimed new stream");
            }
            crate::metrics::CLAIMED_STREAMS.increment(1.0);
        }
    }

    async fn cleanup_orphaned_streams(&self) {
        let rows = match sqlx::query_as::<_, (StreamId,)>(
            "SELECT id FROM streams WHERE claimed_by = ?"
        )
        .bind(&self.worker_id)
        .fetch_all(&self.db_pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!(error = %e, worker_id = %self.worker_id, "Failed to query claimed streams");
                crate::metrics::CLAIM_ERRORS.increment(1);
                return;
            }
        };

        let active_ids: HashSet<StreamId> = rows.into_iter().map(|r| r.0).collect();

        let stale_ids: Vec<StreamId> = self.stream_manager.registry().all_ids()
            .into_iter()
            .filter(|id| !active_ids.contains(id))
            .collect();

        for id in stale_ids {
            self.stream_manager.stop_pipeline(&id);
            tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Released stream (no longer claimed)");
        }
    }

    async fn release_all_claims(&self) {
        for id in self.stream_manager.registry().all_ids() {
            self.stream_manager.stop_pipeline(&id);
        }

        let _ = sqlx::query(
            "UPDATE streams SET claimed_by = NULL, claimed_at = NULL WHERE claimed_by = ?"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await;

        let _ = sqlx::query("DELETE FROM workers WHERE id = ?")
            .bind(&self.worker_id)
            .execute(&self.db_pool)
            .await;

        tracing::info!(worker_id = %self.worker_id, "All claims released");
    }
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
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

fn row_to_config(row: &ClaimRow) -> StreamConfig {
    let tags: std::collections::HashMap<String, String> =
        serde_json::from_value(row.tags.clone()).unwrap_or_default();
    let storage = row.storage_config.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let kafka = row.kafka_config.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    StreamConfig {
        name: row.name.clone(),
        description: row.description.clone(),
        tags,
        source_url: row.source_url.clone(),
        source_type: row.source_type.clone(),
        stream_type: row.stream_type.clone(),
        extract_interval_seconds: row.extract_interval_seconds,
        jpeg_quality: row.jpeg_quality as u8,
        ffmpeg_threads: row.ffmpeg_threads,
        rtsp_transport: row.rtsp_transport.clone(),
        storage,
        kafka,
    }
}

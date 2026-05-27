use aws_sdk_s3::Client;
use anyhow::Result;
use chrono::{Duration, Utc};

pub struct RetentionCleaner {
    client: Client,
    bucket: String,
    retention_days: u64,
}

impl RetentionCleaner {
    pub fn new(client: Client, bucket: String, retention_days: u64) -> Self {
        Self { client, bucket, retention_days }
    }

    pub async fn run_once(&self) -> Result<usize> {
        let cutoff = Utc::now() - Duration::days(self.retention_days as i64);
        let mut deleted = 0usize;
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self.client
                .list_objects_v2()
                .bucket(&self.bucket);

            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let resp = req.send().await?;
            let keys: Vec<String> = resp.contents()
                .iter()
                .filter_map(|obj| {
                    let key = obj.key()?;
                    let last_modified = obj.last_modified()?;
                    let lm = last_modified.as_secs_f64();
                    let lm_chrono = chrono::DateTime::from_timestamp(lm as i64, 0)
                        .unwrap_or_default();
                    if lm_chrono < cutoff { Some(key.to_string()) } else { None }
                })
                .collect();

            if !keys.is_empty() {
                for chunk in keys.chunks(1000) {
                    let objects: Vec<aws_sdk_s3::types::ObjectIdentifier> = chunk.iter()
                        .filter_map(|k| {
                            aws_sdk_s3::types::ObjectIdentifier::builder()
                                .key(k)
                                .build()
                                .ok()
                        })
                        .collect();
                    let delete = aws_sdk_s3::types::Delete::builder()
                        .set_objects(Some(objects))
                        .build()?;
                    let del = self.client
                        .delete_objects()
                        .bucket(&self.bucket)
                        .delete(delete)
                        .send()
                        .await?;

                    let err_count = del.errors().len();
                    if err_count > 0 {
                        for err in del.errors() {
                            tracing::warn!(
                                key = ?err.key(),
                                code = ?err.code(),
                                message = ?err.message(),
                                "Retention cleanup failed to delete object"
                            );
                        }
                    }
                    deleted += chunk.len() - err_count;
                }
            }

            match resp.continuation_token() {
                Some(t) if !t.is_empty() => continuation_token = Some(t.to_string()),
                _ => break,
            }
        }

        tracing::info!(
            bucket = %self.bucket,
            retention_days = self.retention_days,
            deleted_objects = deleted,
            "Retention cleanup pass completed"
        );
        Ok(deleted)
    }

    pub fn start_periodic(self, interval: std::time::Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                if let Err(e) = self.run_once().await {
                    tracing::error!(error = %e, "Retention cleaner pass failed");
                }
            }
        })
    }
}

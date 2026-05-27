pub mod retention;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Config, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_config::timeout::TimeoutConfig;
use anyhow::Result;
use crate::types::{ExtractedFrame, StreamId};
use chrono::Utc;

#[derive(Clone)]
pub struct StorageClient {
    client: Client,
    bucket: String,
    endpoint_url: Option<String>,
}

impl StorageClient {
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn new(config: &crate::config::StorageConfig) -> Self {
        let mut cfg_builder = Config::builder()
            .region(Region::new(config.region.clone().unwrap_or_else(|| "us-east-1".into())))
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .read_timeout(std::time::Duration::from_secs(30))
                    .build()
            );

        if let Some(endpoint) = &config.endpoint_url {
            cfg_builder = cfg_builder.endpoint_url(endpoint.clone());
            if let (Some(ak), Some(sk)) = (&config.access_key_id, &config.secret_access_key) {
                cfg_builder = cfg_builder.credentials_provider(
                    Credentials::new(ak.clone(), sk.clone(), None, None, "s3")
                );
            }
        }

        let client = Client::from_conf(cfg_builder.build());
        let bucket = config.bucket.clone();

        Self {
            client,
            bucket,
            endpoint_url: config.endpoint_url.clone(),
        }
    }

    pub fn generate_key(stream_id: &StreamId, frame: &ExtractedFrame) -> String {
        let date = Utc::now().format("%Y-%m-%d");
        let ts_ms = (frame.timestamp_seconds * 1000.0) as u64;
        format!("{}/{}/{}_{}.jpg", stream_id, date, ts_ms, frame.frame_number)
    }

    pub async fn get_object_bytes(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        let data = resp.body.collect().await?;
        Ok(data.to_vec())
    }

    pub async fn upload_frame(
        &self,
        frame: &ExtractedFrame,
    ) -> Result<(String, String)> {
        let key = Self::generate_key(&frame.stream_id, frame);

        let mut last_error = None;
        let retry_delays = [std::time::Duration::from_secs(1),
                            std::time::Duration::from_secs(2),
                            std::time::Duration::from_secs(4)];

        for (attempt, delay) in retry_delays.iter().enumerate() {
            let body = ByteStream::from(frame.jpeg_bytes.clone());
            match self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&key)
                .body(body)
                .content_type("image/jpeg")
                .send()
                .await
            {
                Ok(_response) => {
                    let storage_url = match &self.endpoint_url {
                        Some(ep) => format!("{}/{}/{}", ep.trim_end_matches('/'), &self.bucket, key),
                        None => format!("s3://{}/{}", &self.bucket, key),
                    };

                    tracing::info!(
                        stream_id = %frame.stream_id,
                        frame_number = frame.frame_number,
                        bucket = %self.bucket,
                        key = %key,
                        size_bytes = frame.jpeg_bytes.len(),
                        attempt = attempt + 1,
                        "Frame uploaded to S3"
                    );

                    return Ok((storage_url, key));
                }
                Err(e) => {
                    tracing::warn!(
                        stream_id = %frame.stream_id,
                        frame_number = frame.frame_number,
                        attempt = attempt + 1,
                        max_retries = retry_delays.len(),
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "S3 upload failed, retrying"
                    );
                    last_error = Some(e);
                    tokio::time::sleep(*delay).await;
                }
            }
        }

        Err(anyhow::anyhow!("S3 upload failed after {} retries: {:?}",
            retry_delays.len(), last_error))
    }
}

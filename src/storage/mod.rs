pub mod retention;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Config, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_config::timeout::TimeoutConfig;
use anyhow::Result;
use crate::types::{ExtractedFrame, StreamId};
use chrono::Utc;

/// Pre-resolve hostname to IP to avoid DNS issues with hyper connector in Docker.
async fn resolve_endpoint(endpoint: &str) -> String {
    let parts: Vec<&str> = endpoint.splitn(2, "://").collect();
    let scheme = parts.first().copied().unwrap_or("http");
    let rest = parts.get(1).copied().unwrap_or(endpoint);
    let host_port: Vec<&str> = rest.splitn(2, ':').collect();
    let host = host_port[0];
    let port: u16 = host_port.get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(if scheme == "https" { 443 } else { 9000 });
    match tokio::net::lookup_host((host, port)).await {
        Ok(mut addrs) => {
            if let Some(addr) = addrs.next() {
                let resolved = format!("{}://{}:{}", scheme, addr.ip(), addr.port());
                tracing::info!(original = %endpoint, resolved = %resolved, "Resolved S3 endpoint");
                return resolved;
            }
            tracing::warn!(endpoint = %endpoint, "No addresses found for S3 endpoint host");
        }
        Err(e) => {
            tracing::warn!(endpoint = %endpoint, error = %e, "Failed to resolve S3 endpoint, using original");
        }
    }
    endpoint.to_string()
}

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

        let endpoint_url = if let Some(endpoint) = &config.endpoint_url {
            let resolved = resolve_endpoint(endpoint).await;
            cfg_builder = cfg_builder.endpoint_url(resolved.clone());
            cfg_builder = cfg_builder.force_path_style(true);
            if let (Some(ak), Some(sk)) = (&config.access_key_id, &config.secret_access_key) {
                cfg_builder = cfg_builder.credentials_provider(
                    Credentials::new(ak.clone(), sk.clone(), None, None, "s3")
                );
            }
            Some(resolved)
        } else {
            None
        };

        let client = Client::from_conf(cfg_builder.build());
        let bucket = config.bucket.clone();

        Self {
            client,
            bucket,
            endpoint_url,
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

impl StorageClient {
    #[allow(dead_code)]
    pub fn noop() -> Self {
        let config = Config::builder()
            .endpoint_url("http://127.0.0.1:0")
            .credentials_provider(Credentials::new("minio", "minio123", None, None, "test"))
            .region(Region::new("us-east-1"))
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(std::time::Duration::from_secs(3))
                    .read_timeout(std::time::Duration::from_secs(5))
                    .build()
            )
            .build();
        Self {
            client: aws_sdk_s3::Client::from_conf(config),
            bucket: "test-bucket".into(),
            endpoint_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExtractedFrame;
    use bytes::Bytes;
    use uuid::Uuid;

    fn make_frame(stream_id: StreamId, frame_number: u64) -> ExtractedFrame {
        ExtractedFrame {
            stream_id,
            frame_number,
            pts: frame_number as i64 * 30,
            timestamp_seconds: frame_number as f64,
            jpeg_bytes: Bytes::from(vec![0xFF, 0xD8, 0xFF]),
            rule_trigger: "test".into(),
            jpeg_quality: 85,
            width: 1920,
            height: 1080,
        }
    }

    #[test]
    fn test_generate_key_format() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let frame = make_frame(id, 42);
        let key = StorageClient::generate_key(&id, &frame);

        assert!(key.starts_with("550e8400-e29b-41d4-a716-446655440000/"));
        assert!(key.ends_with("_42.jpg"));

        let parts: Vec<&str> = key.split('/').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(parts[1].len(), 10);
        assert!(parts[2].ends_with("_42.jpg"));
    }

    #[test]
    fn test_generate_key_different_streams() {
        let id1 = StreamId::new_v4();
        let id2 = StreamId::new_v4();
        let frame = make_frame(id1, 1);
        let key1 = StorageClient::generate_key(&id1, &frame);
        let key2 = StorageClient::generate_key(&id2, &frame);
        assert_ne!(key1, key2);
        assert!(key1.starts_with(&id1.to_string()));
        assert!(key2.starts_with(&id2.to_string()));
    }

    #[test]
    fn test_generate_key_different_frames() {
        let id = StreamId::new_v4();
        let frame1 = make_frame(id, 1);
        let frame2 = make_frame(id, 100);
        let key1 = StorageClient::generate_key(&id, &frame1);
        let key2 = StorageClient::generate_key(&id, &frame2);
        assert_ne!(key1, key2);
        assert!(key1.ends_with("_1.jpg"));
        assert!(key2.ends_with("_100.jpg"));
    }

    #[test]
    fn test_noop_client() {
        let client = StorageClient::noop();
        assert_eq!(client.bucket(), "test-bucket");
        assert!(client.endpoint_url.is_none());
    }
}

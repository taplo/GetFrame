pub mod schema;
pub mod schema_registry;

use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use anyhow::Result;
use crate::types::{ExtractedFrame, FrameMetadata};
use crate::kafka::schema::{SCHEMA, SCHEMA_RAW, frame_metadata_to_avro_value};
use crate::kafka::schema_registry::SchemaRegistryClient;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct KafkaProducer {
    producer: FutureProducer,
    brokers: String,
    topic: String,
    #[allow(dead_code)]
    schema_registry_client: Option<Arc<SchemaRegistryClient>>,
    schema_id: Option<u32>,
}

impl KafkaProducer {
    pub fn new(config: &crate::config::KafkaConfig) -> Result<Self> {
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .set("compression.type", &config.compression)
            .set("message.timeout.ms", "30000")
            .set("queue.buffering.max.ms", "20")
            .set("batch.size", "131072")
            .set("linger.ms", "20");

        if config.acks != "all" {
            client_config.set("acks", &config.acks);
        }

        let producer: FutureProducer = client_config.create()?;

        let (schema_registry_client, schema_id) = if let Some(ref url) = config.schema_registry_url {
            let subject = "getframe-frame-metadata-value";
            let client = SchemaRegistryClient::new(url, subject);
            let id = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(
                    client.register_schema(SCHEMA_RAW)
                )
            })?;
            tracing::info!(
                schema_registry_url = %url,
                schema_id = id,
                subject = %subject,
                "Avro schema registered with Schema Registry"
            );
            (Some(Arc::new(client)), Some(id))
        } else {
            tracing::info!("No Schema Registry configured; using JSON serialization");
            (None, None)
        };

        Ok(Self {
            producer,
            brokers: config.brokers.clone(),
            topic: config.topic.clone(),
            schema_registry_client,
            schema_id,
        })
    }

    pub async fn publish_metadata(
        &self,
        frame: &ExtractedFrame,
        storage_url: &str,
        storage_bucket: &str,
        storage_key: &str,
        topic_override: Option<&str>,
        partition_key: &str,
    ) -> Result<()> {
        let metadata = FrameMetadata {
            stream_id: frame.stream_id.to_string(),
            source_type: "stream".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            frame_number: frame.frame_number,
            rule_trigger: frame.rule_trigger.clone(),
            pts: frame.pts,
            storage_url: storage_url.to_string(),
            storage_bucket: storage_bucket.to_string(),
            storage_key: storage_key.to_string(),
            jpeg_size_bytes: frame.jpeg_bytes.len() as u64,
            jpeg_width: frame.width,
            jpeg_height: frame.height,
        };

        let payload_bytes: Vec<u8> = if let Some(schema_id) = self.schema_id {
            self.serialize_avro(&metadata, schema_id)?
        } else {
            serde_json::to_string(&metadata)?.into_bytes()
        };

        let effective_topic = topic_override.unwrap_or(&self.topic);
        let key_str = partition_key.to_string();

        let mut last_error = None;
        let retry_delays = [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
        ];

        for (attempt, delay) in retry_delays.iter().enumerate() {
            let headers = OwnedHeaders::new()
                .insert(Header { key: "stream_id", value: Some(&key_str) })
                .insert(Header { key: "source_type", value: Some("stream") })
                .insert(Header { key: "content_type", value: Some(
                    if self.schema_id.is_some() { "avro/binary" } else { "application/json" }
                )});

            let record = FutureRecord::to(effective_topic)
                .key(&key_str)
                .payload(&payload_bytes)
                .headers(headers);

            match self.producer.send(record, *delay).await {
                Ok(delivery) => {
                    tracing::info!(
                        stream_id = %frame.stream_id,
                        frame_number = frame.frame_number,
                        topic = %effective_topic,
                        partition = delivery.partition,
                        offset = delivery.offset,
                        attempt = attempt + 1,
                        "Frame metadata published to Kafka"
                    );
                    return Ok(());
                }
                Err((e, _)) => {
                    tracing::warn!(
                        stream_id = %frame.stream_id,
                        frame_number = frame.frame_number,
                        topic = %effective_topic,
                        attempt = attempt + 1,
                        max_retries = retry_delays.len(),
                        error = %e,
                        "Kafka publish failed, retrying"
                    );
                    last_error = Some(e);
                    tokio::time::sleep(*delay).await;
                }
            }
        }

        Err(anyhow::anyhow!(
            "Kafka publish failed after {} retries: {:?}",
            retry_delays.len(), last_error
        ))
    }

    fn serialize_avro(&self, metadata: &FrameMetadata, schema_id: u32) -> Result<Vec<u8>> {
        let value = frame_metadata_to_avro_value(metadata);
        let mut writer = apache_avro::Writer::new(&SCHEMA, Vec::new());
        writer.append(value)?;
        let avro_payload = writer.into_inner()?;

        let mut wire_payload = Vec::with_capacity(5 + avro_payload.len());
        wire_payload.push(0x00);
        wire_payload.extend_from_slice(&schema_id.to_be_bytes());
        wire_payload.extend_from_slice(&avro_payload);

        Ok(wire_payload)
    }

    pub fn query_lag(&self, consumer_group: &str, timeout: Duration) -> i64 {
        let consumer = match ClientConfig::new()
            .set("bootstrap.servers", &self.brokers)
            .set("group.id", consumer_group)
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "false")
            .create::<BaseConsumer>()
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to create Kafka consumer for lag query");
            }) {
            Ok(c) => c,
            Err(_) => return -1,
        };

        let metadata = match consumer.fetch_metadata(Some(&self.topic), timeout) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, topic = %self.topic, "Failed to fetch Kafka metadata for lag");
                return -1;
            }
        };

        let mut total_lag: i64 = 0;
        for topic in metadata.topics() {
            for partition in topic.partitions() {
                if partition.id() < 0 {
                    continue;
                }
                let (_, high) = match consumer.fetch_watermarks(&self.topic, partition.id(), timeout) {
                    Ok(w) => w,
                    Err(_) => continue,
                };

                let mut tpl = rdkafka::topic_partition_list::TopicPartitionList::new();
                tpl.add_partition(&self.topic, partition.id());
                let committed = match consumer.committed_offsets(tpl, timeout) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let committed_offset = committed
                    .elements()
                    .first()
                    .map(|e| e.offset().to_raw().unwrap_or(0))
                    .unwrap_or(0);

                let partition_lag = high.saturating_sub(committed_offset);
                total_lag += partition_lag;
            }
        }

        total_lag
    }
}

impl KafkaProducer {
    #[allow(dead_code)]
    pub fn noop() -> Self {
        let producer: rdkafka::producer::FutureProducer =
            rdkafka::ClientConfig::new()
                .set("bootstrap.servers", "127.0.0.1:1")
                .set("message.timeout.ms", "1000")
                .create()
                .expect("Failed to create noop Kafka producer");
        Self {
            producer,
            brokers: "127.0.0.1:1".into(),
            topic: "test".into(),
            schema_registry_client: None,
            schema_id: None,
        }
    }
}

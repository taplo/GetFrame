# Phase 5 Execution Summary

**Executed:** 2026-05-25
**Plan:** 05-01-PLAN.md
**Result:** All 3 tasks completed successfully — `cargo build` passes with 0 errors.

## Requirements Covered

| ID | Status | Implementation |
|----|--------|---------------|
| KAFKA-04 | ✅ | `acks=all` + `enable.idempotence=true` + `message.timeout.ms=30000` in rdkafka config; 3-retry safety net preserved |
| KAFKA-05 | ✅ | Avro schema + Confluent Schema Registry via `SchemaRegistryClient` (HTTP REST API); JSON fallback when `schema_registry_url` not set |
| KAFKA-06 | ✅ | `topic_override: Option<&str>` + `partition_key: &str` on `publish_metadata`; per-stream `StreamConfig.kafka` wired in both consumer paths |
| FRAME-04 | ✅ | `RetentionCleaner` runs periodic S3 ListObjectsV2 + DeleteObjects; configurable via `StorageConfig.retention_days` |

## Files Modified/Created

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modified | Added `apache-avro = "0.17"`, `reqwest = {..., features = [json, rustls-tls]} |
| `src/config.rs` | Modified | Added `schema_registry_url`, `partition_key_field` to `KafkaConfig`; `retention_days` to `StorageConfig`; default `acks` changed to `"all"` |
| `src/kafka/schema.rs` | **Created** | Avro `FrameMetadata` schema + `frame_metadata_to_avro_value()` conversion |
| `src/kafka/schema_registry.rs` | **Created** | `SchemaRegistryClient` — Confluent Schema Registry REST API with register + lookup |
| `src/kafka/mod.rs` | Rewritten | Production `KafkaProducer` with idempotent producer, dual Avro/JSON serialization, configurable topics/keys |
| `src/storage/retention.rs` | **Created** | `RetentionCleaner` with `run_once()` and `start_periodic()` — ListObjectsV2 + DeleteObjects |
| `src/storage/mod.rs` | Modified | Added `pub mod retention;` + `client()` accessor |
| `src/stream/mod.rs` | Modified | Wired per-stream `StreamConfig.kafka` (topic override + partition key) into both frame consumer paths |
| `src/main.rs` | Modified | Spawns `RetentionCleaner::start_periodic` when `retention_days` configured |

## Design Decisions

1. **Avro + Confluent Schema Registry**: `apache-avro` crate for binary serialization; `reqwest` for Schema Registry HTTP; wire format: `0x00 + 4-byte BE schema_id + Avro payload`; falls back to JSON when no URL configured
2. **Idempotent delivery**: `enable.idempotence=true` forces rdkafka internal retries; app-level 3-retry (1s/2s/4s) kept as safety net for non-retriable errors
3. **Per-stream config**: `StreamConfig.kafka` field (existing) now wired in both `spawn_frame_consumer` and reconnection inline consumer; partition key strategy configurable (`stream_id`/`frame_number`/`timestamp`)
4. **Retention cleaner**: Application-level S3 List+Delete for S3-compatible stores (MinIO, Ceph, AWS); runs every 60 minutes; errors logged non-fatally

## Verification

- `cargo build` — 0 errors, 1 new dead_code warning (`schema_registry_client` field, stored for Arc lifetime)
- All 10 pre-existing warnings unchanged
- Backward compatible: existing `config.yaml` files without new fields parse correctly via `#[serde(default)]`
- JSON fallback active when `schema_registry_url` is not set

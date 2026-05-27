# Phase 5: Kafka Production Readiness - Context

**Gathered:** 2026-05-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Upgrade the existing Kafka integration from a simple JSON-publishing prototype to production-ready: Schema Registry integration with structured serialization, at-least-once delivery guarantees, configurable topics and partition keys per stream/task, and S3 frame retention policy.

**Requirements (4 total):**

| ID | Description | Priority |
|----|-------------|----------|
| KAFKA-04 | At-least-once delivery semantics | High |
| KAFKA-05 | Schema Registry (Avro or Protobuf) for structured metadata | High |
| KAFKA-06 | Configurable topic and partition key per stream/task | Medium |
| FRAME-04 | Frame storage configurable retention policy (auto-cleanup after N days) | Medium |

</domain>

<decisions>
## Implementation Decisions

### The agent's Discretion

All implementation decisions are delegated to the agent for this phase:

1. **Schema Registry (KAFKA-05)**: Choose Avro vs Protobuf vs continuing with JSON if Schema Registry integration proves too heavyweight for current deployment scale. If Schema Registry is chosen, decide on integration approach (Confluent Schema Registry REST API, raw Avro serialization, or Serde-based approach).

2. **At-least-once delivery (KAFKA-04)**: Decide on `acks=all` vs `acks=1` + retries. Consider `enable.idempotence=true` for exactly-once-in-order semantics (pairs naturally with at-least-once). Design backpressure for persistent delivery failures (local disk queue vs in-memory bounded channel vs blocking on send).

3. **Configurable topics (KAFKA-06)**: Design the per-stream topic override mechanism — extend `KafkaConfig` per-stream, topic naming convention, partition key strategy (stream_id vs frame_number vs timestamp).

4. **Retention policy (FRAME-04)**: Choose between S3 Lifecycle rules (recommended for S3-compatible stores), application-layer periodic cleanup task, or both. Configurable retention days per-stream.

5. **Error handling**: Enhance the current retry mechanism (3 retries 1s/2s/4s) — consider adding a configurable dead letter topic or local fail queue for frames that can't be delivered.

### Existing Code That Must Be Preserved or Extended

- Current `KafkaProducer` in `src/kafka/mod.rs` uses `rdkafka::FutureProducer`
- JSON metadata payload via `serde_json`
- 3 retries with exponential backoff (1s/2s/4s)
- Config structure in `src/config.rs` (`KafkaConfig` with `brokers`, `topic`, `acks`, `compression`)
- Per-stream `kafka: Option<KafkaConfig>` field in `StreamConfig` (already defined, not yet wired)
- Frame storage uses Claim-Check pattern: `{stream_id}/{date}/{timestamp_ms}_{frame_number}.jpg`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & Requirements
- `.planning/PROJECT.md` — Project context, core value, constraints
- `.planning/REQUIREMENTS.md` — Full requirements (KAFKA-04/05/06, FRAME-04)
- `.planning/ROADMAP.md` §Phase 5 — Phase goal, success criteria

### Existing Code
- `src/kafka/mod.rs` — Current KafkaProducer implementation
- `src/config.rs` — KafkaConfig and per-stream kafka override field
- `src/stream/mod.rs` — How kafka_producer is used in the pipeline
- `src/types.rs` — FrameMetadata, KafkaHeaders structs

### Prior Phase Context
- `.planning/phases/01-core-pipeline-single-stream-foundation/01-CONTEXT.md` §D-04, D-05, D-12, D-14, D-16 — Kafka client choice, Claim-Check pattern, S3 key convention, message format, retry strategy

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `KafkaProducer` struct (`src/kafka/mod.rs`) — Wraps rdkafka FutureProducer with retry logic; can be extended with Schema Registry and configurable topics
- `KafkaConfig` struct (`src/config.rs`) — Already has `topic`, `acks`, `compression` fields
- `StreamConfig.kafka: Option<KafkaConfig>` — Per-stream Kafka override field already defined but unwired

### Established Patterns
- **Claim-Check pattern**: Frame metadata (small JSON) → Kafka, frame bytes → S3. This pattern continues unchanged.
- **Retry with backoff**: 3 retries at 1s/2s/4s (defined in `KafkaProducer::publish_metadata`). Can be extended.
- **OS thread decode + tokio async I/O**: Kafka send is tokio-based via FutureProducer; this pattern continues.

### Integration Points
- `src/stream/mod.rs` lines ~240 — `KafkaProducer::publish_metadata` call site (where at-least-once and topic config wire in)
- `src/main.rs` line ~37 — KafkaProducer creation from config (where Schema Registry init would go)
- `src/config.rs` — KafkaConfig and per-stream overrides (extend with topic, schema registry URL, retention days)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches as defined in requirements.

</specifics>

<deferred>
## Deferred Ideas

- Dead letter queue for failed message delivery (KAFKA-07) — v2 requirement, out of scope for Phase 5
- Batch delivery option (KAFKA-08) — v2 requirement
- Exactly-once delivery semantics option (KAFKA-09) — v2 requirement

</deferred>

---

*Phase: 5-Kafka Production Readiness*
*Context gathered: 2026-05-25*

# Phase 1: Core Pipeline — Single Stream Foundation - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the end-to-end core pipeline for a single video stream: ingest (RTSP/RTMP/HLS/file) → H.264 software decode → fix-interval rule evaluation → JPEG encode → store in MinIO/S3 → push frame metadata to Kafka. This phase proves the architecture works end-to-end before scaling to multiple streams.

**In scope:** Single-stream pipeline, config file for source config, fixed-interval extraction, MinIO/S3 storage with deterministic keys, Kafka metadata delivery, health check endpoints, structured logging, Docker multi-stage build.

**Out of scope:** Multi-stream management, REST API, Web UI, scene detection, composite rules, Schema Registry, KEDA auto-scaling, production Grafana dashboards.

</domain>

<decisions>
## Implementation Decisions

### Technology Stack (locked via research)
- **D-01:** Language — Rust (Edition 2024, 1.85+)
- **D-02:** Video decoding — FFmpeg libavcodec via `ffmpeg-next` 8.1.0 (as library, NOT CLI subprocess)
- **D-03:** YUV→RGB conversion — `yuvutils-rs` 0.8+ with AVX2 SIMD (not FFmpeg swscale)
- **D-04:** Kafka client — `rdkafka` 0.39.0 (librdkafka bindings)
- **D-05:** Frame storage — MinIO/S3 (Claim-Check pattern: store images, send metadata+URL via Kafka)
- **D-06:** Concurrency — Hybrid: dedicated OS threads for CPU-bound FFmpeg decode + tokio async for I/O (Kafka, MinIO, health HTTP)
- **D-07:** Container — Multi-stage Docker: `rust:1.85-slim` builder + `mwader/static-ffmpeg:8.1` + `gcr.io/distroless/cc-debian12` runtime

### Configuration
- **D-08:** Initial config file format — YAML (recommended by research, supports nested stream configs natively). CLI args via `clap` for overrides (--config path).

### Pipeline Architecture
- **D-09:** Decode loop — Use raw `avcodec_send_packet` / `avcodec_receive_frame` for explicit control (not high-level ffmpeg-next decoder wrapper). Required for proper PTS reordering and memory management.
- **D-10:** Bounded channels — `crossbeam::bounded` between pipeline stages (ingest→decode→rule→kafka). Sizes: 64/8/256 frames per channel.
- **D-11:** Frame buffer — Start with `Vec<u8>` reuse (allocate once, reuse per frame). Arena allocator (`bumpalo`) can be introduced as optimization in Phase 9.

### Frame Storage
- **D-12:** S3 key convention — `{stream_id}/{date}/{timestamp_ms}_{frame_number}.jpg` (date prefix for partitioning, human-readable timestamps)
- **D-13:** JPEG quality — Default Q=85. Configurable per-stream in YAML config.

### Kafka Message
- **D-14:** Message format — JSON metadata in record value. Headers for routing (stream_id, source_type). Fields: `stream_id`, `timestamp`, `frame_number`, `rule_trigger` (`"interval"`), `pts`, `storage_url` (full MinIO/S3 URL), `storage_bucket`, `storage_key`, `jpeg_size_bytes`, `jpeg_width`, `jpeg_height`.

### Error Handling
- **D-15:** On decode corruption — Log the error with stream context, skip the corrupt frame, continue decoding. No crash, no pipeline halt.
- **D-16:** On Kafka send failure — Log error, retry with exponential backoff (3 retries, 1s/2s/4s). After exhaustion, log as ERROR and continue (frame is lost — acceptable for at-least-once).
- **D-17:** On MinIO upload failure — Similar retry strategy. Store attempt is critical; failure means frame is lost.

### Agent's Discretion
- Exact Rust crate version pinning (beyond the research recommendations above)
- MinIO client crate choice (aws-sdk-s3 vs raw HTTP — aws-sdk-s3 recommended for completeness)
- `clap` CLI argument design details
- `tracing` subscriber configuration (JSON format, log level defaults)
- Exact health check endpoint paths (standard `/health` and `/ready`)
- Binary naming (suggested: `getframe-worker`)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & Requirements
- `.planning/PROJECT.md` — Project context, core value, constraints
- `.planning/REQUIREMENTS.md` — Full v1 requirements with traceability
- `.planning/ROADMAP.md` §Phase 1 — Phase goal, success criteria, requirements list

### Research (Phase 1 relevant sections)
- `.planning/research/STACK.md` — Full technology stack with rationale and version pins
- `.planning/research/ARCHITECTURE.md` — Pipeline architecture, bounded channels, concurrency model
- `.planning/research/PITFALLS.md` §Pitfall 1 (FFmpeg as library, not subprocess), §Pitfall 2 (B-frame PTS reordering), §Pitfall 3 (FFmpeg memory leaks), §Pitfall 8 (keyframe alignment)
- `.planning/research/SUMMARY.md` — Synthesized research overview

### Configuration
- `.planning/config.json` — Workflow preferences (mode, granularity, agent settings)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing codebase — greenfield project. All code is new.

### Established Patterns
- Research established the Rust + FFmpeg + rdkafka pattern. Phase 1 implements this pattern from scratch.

### Integration Points
- Phase 1 produces a standalone binary (`getframe-worker`). No integration with other services yet.
- MinIO/S3 is the first external dependency integration point.
- Kafka is the second external dependency integration point.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches as defined in research.

Key research finding to respect: Phase 1 is a **benchmark phase**. The actual 1080p H.264 decode throughput on target CPU hardware must be measured. This drives all future capacity planning.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 1-Core Pipeline — Single Stream Foundation*
*Context gathered: 2026-05-24*

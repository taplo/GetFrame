# Project Research Summary

**Project:** GetFrame — High-Performance Video Frame Extraction Platform
**Domain:** CPU-only H.264 frame extraction at 200-1000+ concurrent streams, Kubernetes-native
**Researched:** 2026-05-24
**Confidence:** HIGH

## Executive Summary

GetFrame is a **distributed, CPU-only video frame extraction platform** that ingests 200-1000+ concurrent 1080p H.264 streams (RTSP/RTMP/HLS/file), decodes them entirely in software, evaluates configurable extraction rules, and pushes extracted JPEG frames to Kafka. No existing open-source platform directly solves this "persistent stream ingestion + programmable rule engine + Kafka output" use case at scale. The closest alternatives are surveillance NVRs (recording-focused, no Kafka), CV frameworks (GPU-dependent or Python-limited), and custom FFmpeg scripts (no management UI or rule engine).

**The recommended approach** is a **Rust-native architecture** with a hybrid concurrency model: dedicated OS threads for CPU-bound FFmpeg decoding (via `ffmpeg-next`/`libavcodec`) plus a tokio async runtime for network I/O and Kafka production. This avoids the GC pressure that would cripple Go at the allocation rates required (1.8GB/sec at 200 streams × 1fps), leverages zero-cost FFmpeg FFI, and enables deterministic memory management via arena allocators (`bumpalo`) and object pools. The stack is Rust all the way down except the TypeScript/React web UI.

**Key risks and mitigations:** (1) **FFmpeg per-process model** — using `libavcodec` as a library (not CLI subprocesses) prevents memory exhaustion at scale; (2) **Kafka message size limits** — JPEG frames at 200-500KB each can overwhelm default 1MB message limits and producer buffers; use claim-check pattern (store frames externally, send metadata) or carefully size `buffer.memory`/`max.request.size`; (3) **Kubernetes CPU throttling** — video decoding is bursty (I-frames need 3-10x more CPU), and CFS quota throttling silently drops frames; use Guaranteed QoS with `limits.cpu = requests.cpu` and CPU Manager static policy; (4) **B-frame PTS/DTS ordering** — all extracted frames get wrong timestamps if PTS reordering is ignored; always use `av_frame_get_best_effort_timestamp()`.

---

## Key Findings

### Recommended Stack

**Language: Rust (Edition 2024 / 1.85+)** — The single most critical decision. Rust's zero-GC-pause model directly addresses the core challenge. Go introduces latency spikes at the allocation rates required (confirmed by Discord's real-world Go→Rust migration, 2026 benchmarks showing Go degrading at 35K+ RPS). C/C++ is rejected for memory safety concerns in a networked service. Rust provides SIMD control, deterministic tail latency, and 2-4x less memory than Go for equivalent workloads — directly increasing Kubernetes pod density.

**Video decoding: FFmpeg `libavcodec` 7.x via `ffmpeg-next` 8.1.0** — The most mature Rust FFmpeg binding (3.5M+ downloads). Coupled with `yuvutils-rs` for SIMD-accelerated YUV→RGB conversion (23x faster than FFmpeg's swscale on AVX2 at ~320µs per 1080p frame). Scene detection uses FFmpeg's built-in `scdet` filter (essentially free computation during decode) or the pure-Rust `scenesdetect` crate.

**Kafka: `rdkafka` 0.39.0 (librdkafka bindings)** — Battle-tested at 1M+ msg/sec, 28M+ downloads, 236 reverse deps. Pure-Rust clients (`krafka`) are too new for production at this scale.

**HTTP API: Axum 0.8.x** — Tokio-native, Tower-based middleware, 780K req/s. Management API sees <<500 RPS so Actix-web's 10% performance advantage is irrelevant. Better long-term maintainability.

**Frontend: TypeScript 5.x + React 19 + Vite 6 + shadcn/ui + Tailwind 4 + TanStack Query 5** — Industry standard dashboard stack. Svelte/HTMX insufficient for complex management UI with real-time monitoring.

**Database: PostgreSQL 16 + SQLx 0.8** — Stream configs, rules, task definitions. Not on hot path. Compile-time checked queries with zero ORM overhead.

**Container: Multi-stage Docker (rust:1.85-slim → gcr.io/distroless/cc-debian12)** — Final image ~140MB (15-25MB Rust binary + 117MB static FFmpeg from `mwader/static-ffmpeg`). Use `cargo-chef` for Docker layer caching.

**K8s: Deployments + KEDA 2.16+** — Standard Deployment (not Operator — overkill for single workload class). KEDA autoscaling on Kafka consumer lag + CPU metrics. Helm chart for repeatable deployments.

**Observability: Prometheus + structured JSON logging (tracing) + OpenTelemetry OTLP** — Key custom metrics: `streams_active`, `frames_processed_total`, `kafka_produce_latency`, `scene_changes_detected`, `memory_pool_usage`.

---

### Expected Features

**Must-have (table stakes — P0, Phase 1):**
- **STREAM-01:** Source management CRUD with health status (URL validation, type identification)
- **STREAM-02:** Automatic reconnection with exponential backoff, graceful failure isolation
- **TASK-01:** Task lifecycle (create/start/pause/stop/status) with stream + rule assignment
- **RULE-01:** Time-interval extraction (fixed interval, FPS-based, per-stream configurable rate)
- **KAFKA-01:** Basic Kafka producer (JPEG payload + metadata headers, at-least-once delivery)
- **OPS-01:** Prometheus metrics endpoint, health check endpoints, structured JSON logging
- **API-01:** RESTful API for streams, tasks, rules, status, with authentication (API key/JWT)
- **UI-01:** Stream list, task list, create forms, basic health dashboard
- **PLAT-01:** Helm chart, Docker images, configurable resource limits, HPA support

**Should-have (differentiators — P1, Phase 2+):**
- **DIFF-01:** CPU-optimized scene change detection via FFmpeg `scdet` filter (configurable threshold, composite rules — "extract every 5s OR on scene change")
- **DIFF-02:** Rich rule engine (scheduled/cron extraction, day/time windows, rate limiting, composite rules, rule templates)
- **DIFF-03:** Kafka Schema Registry integration (Avro/Protobuf), configurable partition keys, batch delivery, producer metrics, dead letter queues
- **DIFF-04:** Horizontal scalability — task distribution across nodes, consistent hashing, graceful pod shutdown, resource-aware scheduling
- **DIFF-05:** Stream health intelligence — composite health scoring, frame rate tracking, connection latency tracking, anomaly detection
- **DIFF-06:** Rich Web UI — real-time WebSocket updates, per-stream frame preview, metrics dashboards, activity logs, bulk task editor
- **DIFF-07:** Multi-tenancy with teams/projects isolation, RBAC (admin/operator/viewer), per-tenant resource quotas (P2/Enterprise)

**Defer (v2+/Never Build):**
- Video transcoding/re-encoding (ANTI-01)
- AI/ML visual analysis (ANTI-02 — it's a downstream consumer responsibility)
- Video storage/archival (ANTI-03)
- GPU acceleration (ANTI-04 — CPU-only by design)
- Frame post-processing/resizing/watermarking (ANTI-06)
- Custom Kafka Connect sinks (ANTI-07)
- Comprehensive data pipeline UI (ANTI-05)

---

### Architecture Approach

GetFrame uses a **hybrid concurrency model** that separates CPU-bound decode from I/O-bound network operations:

- **Dedicated OS threads per stream pipeline** (or round-robin scheduling across ~6-7 streams per thread) for FFmpeg `libavcodec` H.264 decoding. Decode calls are blocking CPU work (5-30ms per frame) and cannot yield cooperatively — async/await would starve other streams on the same tokio task.
- **Tokio async runtime** for network I/O, Kafka production, HTTP health endpoints, configuration watches, and Prometheus metrics export.
- **Bounded channel ring buffers** (`crossbeam::bounded`) between pipeline stages create a virtuous backpressure chain: if Kafka is slow, the producer buffer blocks → rule eval blocks → decode thread blocks → ingest thread blocks → TCP backpressure slows the source.
- **Memory architecture** uses `bumpalo` arena allocators for decoded frames (per-stream, O(1) reset between frames) + object pools for `AVPacket` structs + recycled libjpeg-turbo compression contexts.

**Major components:**
1. **Stream Instance (per-stream pipeline)** — Ingestion (network read + demux) → H.264 decode (libavcodec) → Rule evaluation (interval/scene-change) → JPEG encode → Kafka send. Each stage connected by bounded channels with small capacities (64/8/256 frames).
2. **Worker Process (data plane)** — Hosts N stream instances. Supervisor task manages stream lifecycle, reconnection, scaling. Shared `rdkafka::FutureProducer` pool (N = min(4, num_cpu_cores)). Core-pinned threads with round-robin scheduling across assigned streams.
3. **Stream Manager (control plane)** — Stream lifecycle management, configuration distribution, health monitoring. Stateless (PostgreSQL-backed). Runs as 1-2 replicas.
4. **REST API Gateway** — Axum-based, Tower middleware stack. Stream/task/rule CRUD, status queries, API auth.
5. **Web UI** — React dashboard consuming REST API + WebSocket for real-time updates.
6. **PostgreSQL** — Persistent store for stream configs, rules, task definitions.

**Key throughput numbers:**
- 1080p H.264 decode: ~10ms per frame per thread
- JPEG encode (Q=85): ~25ms per frame (main bottleneck)
- Kafka send: ~2ms per frame (batched)
- At 1fps per stream: ~37ms CPU per extracted frame → ~3.7% CPU per stream → ~27 streams per core → ~200 streams per 8-core worker

---

### Critical Pitfalls

1. **FFmpeg per-process model (Pitfall 1)** — Spawning one FFmpeg CLI process per stream (200 processes × 150-300MB RSS = 30-60GB memory) with `-threads auto` creating 1200+ competing threads. **Prevention:** Use FFmpeg as a library (libavcodec), not CLI subprocesses. Pool codec contexts. Pin threads to cores.
2. **B-frame PTS/DTS ordering (Pitfall 2)** — H.264 decodes in DTS order but presents in PTS order. Using decode output order directly produces frames off by 1-16 frame periods (33-533ms). **Prevention:** Always use `av_frame_get_best_effort_timestamp()` or `pkt->pts`. Use PTS-based reordering queue.
3. **Kafka message size & producer buffer backpressure (Pitfall 4)** — JPEG frames at 200-500KB each exhaust default 32MB `buffer.memory` in <1 second at 200 streams × 1fps. The failure cascade: producer blocks → decoder stalls → RTSP drops. **Prevention:** Use claim-check pattern (store in blob store, send metadata through Kafka) OR increase `message.max.bytes`/`buffer.memory`/`max.request.size` to 5-10MB. Implement bounded queue with frame dropping on overflow.
4. **CPU throttling in Kubernetes (Pitfall 5)** — CFS quota throttles bursty I-frame decode, causing silent frame loss. Thread migration destroys CPU cache locality. **Prevention:** Set `resources.limits.cpu = resources.requests.cpu` (Guaranteed QoS). Enable CPU Manager static policy (`--cpu-manager-policy=static`). Set FFmpeg threads explicitly to match CPU limit.
5. **FFmpeg memory leaks (Pitfall 3)** — Long-running decoder instances grow memory monotonically (200MB → 2-8GB over 6-72 hours) due to SPS/PPS buffer leaks, missing `av_frame_unref()`, and resolution change handling gaps. **Prevention:** Audit all FFmpeg API alloc/free pairs. Run 24-hour ASAN soak test before production. Consider periodic decoder reset as last resort.
6. **RTSP reconnection thundering herd (Pitfall 6)** — After transient network blip, all 200+ streams reconnect simultaneously, overwhelming cameras (typical limit: 4-20 RTSP sessions). **Prevention:** Exponential backoff (1s → 120s max) with jitter, staggered startup, TCP keepalive with aggressive timeouts, per-camera connection limits.
7. **Over-engineering rule engine before pipeline works (Pitfall 10)** — Building flexible rule DSL while streams can't stay connected for 2 hours. **Prevention:** Phase ordering — simple interval config (not engine) → scene detection → composite rules. Validate against real RTSP camera feeds from day 1.

---

## Implications for Roadmap

Based on combined research from stack, features, architecture, and pitfalls, the recommended build order is:

### Phase 1: Core Pipeline (Foundation)
**Rationale:** Must prove the fundamental decode→extract→deliver pipeline works before scaling. Every downstream feature depends on correct frame decoding. This phase validates architecture assumptions and benchmarks real throughput.
**Delivers:** Single-stream decode loop with correct PTS handling, JPEG encoding, bounded channel backpressure, basic Kafka output (simple producer, fixed topic), simple interval extraction (config parameter, not engine), stream validation (video stream detection, keyframe alignment).
**Addresses features:** STREAM-01 (partial — single stream CRUD), RULE-01 (basic interval config), KAFKA-01 (basic producer), OPS-01 (baseline metrics).
**Avoids pitfalls:** Pitfall 1 (library not subprocess), Pitfall 2 (PTS reordering), Pitfall 3 (FFmpeg API audit), Pitfall 8 (keyframe discard), Pitfall 11 (VFR detection), Pitfall 16 (resolution changes), Pitfall 19 (custom decoder), Pitfall 20 (corruption detection).
**Research flag:** Needs benchmarking on target CPU hardware — actual 1080p decode throughput per core varies significantly. Phase 1 should measure real numbers before Phase 2 planning.

### Phase 2: Multi-Stream Scaling
**Rationale:** The 200-1000+ stream target requires deliberate concurrent design. Phase 1 proves single-stream; Phase 2 proves N-streams-on-M-cores. Discover the saturation point and backpressure behavior before adding rules or optimization.
**Delivers:** Per-core thread scheduling with core pinning, round-robin stream multiplexing (6-7 streams per thread), stream supervisor, stream lifecycle (create/destroy/reconnect with backoff), per-camera connection limits, resource limit configuration.
**Addresses features:** STREAM-01 (full), STREAM-02 (reconnection), TASK-01 (basic lifecycle), PLAT-01 (resource config).
**Avoids pitfalls:** Pitfall 6 (reconnection thundering herd — exponential backoff + staggered startup), Pitfall 9 (HLS retry storms), Pitfall 12 (false sharing — thread-local contexts + cache-aligned structs), Pitfall 13 (OS resource limits), Pitfall 14 (audio-only streams).
**Research flag:** NUMA topology and cache behavior on target cloud instance types will determine thread pinning strategy. May need deeper research on specific K8s node hardware.

### Phase 3: Rule Engine & Scene Detection
**Rationale:** Now that the pipeline is proven at scale, add extraction intelligence. The rule engine should start as configuration (YAML → compiled evaluation tree) and graduate to richer expressions. Scene detection via FFmpeg `scdet` filter is essentially free during decode.
**Delivers:** YAML rule configuration compiled to evaluation tree, interval-based rules, scene change detection (scdet filter integration), composite rules (any/all operators), scheduled extraction (cron/day-time windows), rate limiting.
**Addresses features:** DIFF-01 (scene detection), DIFF-02 (rich rule engine), RULE-01 (enhanced).
**Avoids pitfalls:** Pitfall 10 (engine after pipeline works — enforced by phase ordering).
**Research flag:** `scenesdetect` crate (pure-Rust, Apr 2026) is new — evaluate as alternative to scdet filter during this phase. Compare accuracy and CPU cost.

### Phase 4: Kafka Delivery Optimization
**Rationale:** Basic Kafka output works from Phase 1. Phase 4 optimizes for scale: producer pools, delivery guarantees, schema integration. Addresses the critical Pitfall 4 (message size, backpressure) before production.
**Delivers:** Shared producer pool (N = min(4, cpu_cores)), Schema Registry integration (Avro/Protobuf), configurable partition keys (by stream_id), delivery guarantees (at-least-once default, exactly-once optional), dead letter queue, producer metrics dashboard, batch delivery option, JPEG quality configuration per stream.
**Addresses features:** DIFF-03 (Kafka-native architecture), KAFKA-01 (enhanced).
**Avoids pitfalls:** Pitfall 4 (message size — claim-check pattern or tuned buffer sizes, bounded queues), Pitfall 15 (JPEG quality trade-off), Pitfall 17 (consumer rebalance).
**Research flag:** The claim-check pattern (store frames in S3/MinIO, send metadata via Kafka) vs direct image transport needs a decision. Direct transport is simpler but has hard scaling limits. Benchmark both approaches at 200+ stream scale.

### Phase 5: Management API & Control Plane
**Rationale:** With the data plane proven at scale, build the control plane. The REST API and Stream Manager enable programmatic management, which the UI and K8s integration depend on.
**Delivers:** Axum REST API with Tower middleware (CORS, tracing, rate limiting, auth), PostgreSQL integration via SQLx (compile-time checked queries), Stream Manager service (stream lifecycle, config distribution, health monitoring), API authentication (API key/JWT), OpenAPI/Swagger documentation, K8s health probes (liveness/readiness).
**Addresses features:** API-01 (full REST API), STREAM-01 (management layer).
**Avoids pitfalls:** Pitfall 21 (wrong monitoring — instrument the decoder pipeline, not just infrastructure).
**Research flag:** Config distribution strategy (PostgreSQL + watch vs ConfigMap watch) needs validation at 1000+ stream update rates. PostgreSQL-backed config service is safer at scale than K8s ConfigMaps.

### Phase 6: Web UI
**Rationale:** The UI consumes the Phase 5 API. Building it earlier would mean rework as the API changes. Now the API surface is stable.
**Delivers:** React + TypeScript + Vite frontend, shadcn/ui component library, stream CRUD interface, task management UI (create with rule config), rule management UI, live status dashboard (WebSocket updates), activity log viewer, basic metrics display.
**Addresses features:** UI-01 (dashboard essentials), DIFF-06 (rich UI — real-time updates, metrics, activity logs).
**Research flag:** Well-documented patterns (React + REST + WebSocket). Skip deeper research — standard implementation.

### Phase 7: Kubernetes & Production Readiness
**Rationale:** Containerization and K8s deployment wrap everything together. Can be partially parallelized with Phases 5 and 6, but full production readiness (KEDA autoscaling, chaos testing) depends on stable data and control planes.
**Delivers:** Multi-stage Dockerfile (cargo-chef + distroless base + static FFmpeg), Helm chart (Deployments, Services, ConfigMaps, secrets), KEDA ScaledObject (Kafka consumer lag + CPU autoscaling), Prometheus metrics + Grafana dashboard, resource limit tuning (Guaranteed QoS, CPU Manager), chaos testing (network partitions, pod failures, streaming disconnects), 24-hour soak test with ASAN.
**Addresses features:** PLAT-01 (K8s deployment), DIFF-04 (horizontal scaling), OPS-01 (full monitoring).
**Avoids pitfalls:** Pitfall 5 (CPU throttling — Guaranteed QoS + CPU Manager), Pitfall 13 (OS resource limits in K8s).
**Research flag:** KEDA configuration at scale (lagThreshold, cooldown periods) needs iterative tuning. No research-phase needed — standard K8s+KEDA patterns.

### Phase Ordering Rationale

- **Phase 1 first** because B-frame handling, memory management, and pipeline backpressure are foundational and affect every other component. Building rules or UI before the decoder is correct means rework.
- **Phase 2 before rules** because scaling behavior (saturation point, backpressure dynamics) changes when going from 1 to 200 streams. Rules evaluate correctly at small scale but performance characteristics change at full load.
- **Phase 3 before Kafka optimization** because rule output volume determines Kafka load. You can't tune Kafka throughput without knowing your extraction rate.
- **Phase 4 before management API** because the API needs to expose stream metrics that include Kafka health (delivery rates, lag, errors).
- **Phases 5-6-7** can partially overlap — the API is a prerequisite for the UI, and K8s deployment needs both data and control planes to be stable.

### Research Flags

- **Phase 1:** Needs `/gsd-plan-phase --research-phase 1` — benchmark actual 1080p decode throughput on target cloud CPU (e.g., Intel Xeon 4th/5th Gen). The 10ms/frame estimate needs validation.
- **Phase 3:** Needs `/gsd-plan-phase --research-phase 3` — evaluate `scenesdetect` crate vs FFmpeg `scdet` filter for accuracy and CPU cost.
- **Phase 4:** Needs `/gsd-plan-phase --research-phase 4` — benchmark claim-check vs direct image transport through Kafka at 200+ stream scale. Also test `rdkafka::FutureProducer` with 200+ concurrent senders.
- **Phase 5:** Consider `/gsd-plan-phase --research-phase 5` — validate PostgreSQL-backed config distribution at 1000+ stream update rates.

**Skip research for:**
- **Phase 6:** Web UI — standard React/REST/WebSocket patterns.
- **Phase 7:** K8s deployment — standard Helm + KEDA patterns (though Chaos testing design may need iteration).

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| **Stack** | HIGH | Rust for CPU-bound decode is well-supported by benchmarks (Discord, Cloudflare) and direct comparisons. FFmpeg bindings mature. All library choices have high download counts and active maintenance. |
| **Features** | HIGH | Competitive landscape is clear — no existing platform combines persistent stream ingestion + programmable rules + Kafka output. Feature categories are well-differentiated with clear P0/P1/P2 priorities. |
| **Architecture** | MEDIUM | Hybrid thread+async model is theoretically sound but needs validation at 200+ streams. Key uncertainty: actual decode throughput per core on target hardware (estimated 10ms/frame). JPEG encode bottleneck estimate (25ms) needs profiling. |
| **Pitfalls** | HIGH | Well-documented from FFmpeg upstream discussions, Kafka source code, Kubernetes production post-mortems, and RTSP deployment war stories. Top 5 pitfalls have clear prevention strategies. |

**Overall confidence: HIGH** for technology choices and pitfall awareness. **MEDIUM** for architecture-level throughput numbers, which need Phase 1 benchmarking on target hardware.

### Gaps to Address

- **Real-world decode throughput:** The 10ms/frame 1080p decode estimate is based on general benchmarks. Must measure on target cloud CPU (e.g., Intel Xeon 4th/5th Gen) during Phase 1. This affects all capacity planning (streams-per-core, memory sizing, K8s resource requests).
- **JPEG encode vs memory bandwidth tradeoff:** Current plan puts JPEG encode on the decode thread (saving a cross-thread frame copy). If encode (~25ms) dominates decode (~10ms), a dedicated encode thread pool with frame handoff may be faster despite the copy cost. Measure both approaches.
- **`rdkafka` thread model at 200+ producers:** librdkafka's internal I/O threads and `poll()`-based model need validation with tokio at 200+ concurrent senders. The `FutureProducer` integration may need tuning.
- **claim-check vs direct image transport:** This is a foundational Kafka architecture decision that affects message sizing, broker load, and consumer complexity. Needs benchmarking at Phase 4.
- **Intel QSV (`h264_qsv`):** The project says "no GPU/NPU" but integrated iGPU might be available on target hardware. Worth evaluating as a potential performance multiplier even if CPU-only is the primary target.
- **`edge264` alternative decoder:** Claims competitive performance to FFmpeg's `libavcodec`. Worth evaluating as a drop-in replacement during Phase 1 if decode throughput is lower than expected.

---

## Sources

### Primary (HIGH confidence)
- Rust vs Go HTTP Server Benchmark (2026) — K8s comparison: Rust 57% less memory, 74% throughput improvement
- Discord: Why Discord migrated from Go to Rust — confirmed GC latency spikes at scale
- Cloudflare Pingora: Rust at 1T req/day — 70% less CPU, 67% less memory
- ffmpeg-next v8.1.0 — 3.5M+ downloads, maintained Rust FFmpeg binding
- yuvutils-rs benchmarks — AVX2 YUV420→RGB ~375µs (15-23x faster than swscale/libyuv)
- rdkafka v0.39.0 — 28M+ downloads, 236 reverse deps, battle-tested at 1M+ msg/sec
- Axum vs Actix-web 2026 comparison — Axum preferred for maintainability at management API scale
- FFmpeg-devel ML (2007-2023) — Memory leak threads, decoder behavior
- Apache Kafka source code (BufferPool.java), KIP-782, PR #20358 — Producer backpressure behavior
- MainConcept Kubernetes benchmarks — CPU throttling impact on video encoding
- Kubernetes CPU Manager documentation — CFS quota behavior
- FFmpeg scdet filter documentation — Built-in scene detection since FFmpeg 4.3

### Secondary (MEDIUM confidence)
- FFmpeg H.264 decode time (~10ms per 1080p frame) — general knowledge, needs target hardware validation
- JPEG encode time (~25ms per 1080p frame with libjpeg-turbo) — general knowledge, depends on CPU gen
- scenesdetect (pure-Rust scene detection) — new (Apr 2026), 1 star, well-architected but unproven
- rtsp-kafka-ingestion-template production post-mortem (2025) — single source but detailed
- KEDA for video streaming autoscaling case study — 5M concurrent users, 30% cost savings

### Tertiary (LOW confidence)
- krafka pure-Rust Kafka client — too new (Feb 2026) for production at scale, promising future option
- RTSP reconnection strategies (NikhilBudaniya/rtsp-stream) — small project, but patterns are standard
- edge264 alternative H.264 decoder — claims competitive performance, needs evaluation

---

*Research completed: 2026-05-24*
*Ready for roadmap: yes*

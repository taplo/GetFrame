# Domain Pitfalls: CPU-Only Video Frame Extraction at Scale

**Domain:** High-performance video frame extraction (200-1000+ concurrent 1080P H.264 streams, CPU-only)
**Researched:** 2026-05-24
**Overall confidence:** HIGH (sourced from FFmpeg upstream discussions, Kafka source code, Kubernetes troubleshooting, and production post-mortems)

---

## Critical Pitfalls

Mistakes that cause rewrites, catastrophic failure, or major operational incidents.

### Pitfall 1: FFmpeg Per-Process Model (Thread Safety & Resource Explosion)

**What goes wrong:** Spawning one FFmpeg CLI process per stream. With 200+ streams, the system creates 200+ separate OS processes, each loading the FFmpeg shared libraries, allocating their own codec contexts, frame buffers, and thread pools. This leads to rapid memory exhaustion, context-switching thrash, and eventual OOM.

**Why it happens:** FFmpeg CLI is the easiest path to "hello world" frame extraction. Most prototypes start with `exec("ffmpeg -i rtsp://... -vf fps=1 frame_%d.jpg")`. The convenience hides that each process is fully isolated: separate memory-mapped FFmpeg binary, separate codec initialization, separate thread pool (FFmpeg's `-threads auto` typically uses 1.5x logical cores per instance).

**Consequences:**
- **Memory:** Each FFmpeg process decoding 1080p H.264 uses approximately 150-300MB RSS (codec state + frame buffers + x264 lookahead). 200 processes = 30-60GB minimum. See "Reducing FFmpeg Memory Usage" benchmarks: single 8K transcode consumed 9GB virtual memory; at 1080p expect ~200-300MB per stream.
- **CPU thrash:** With `-threads auto`, 200 processes × ~6 threads each = 1200 threads competing for physical cores. The OS scheduler spends more time context-switching than decoding. Measured: a 16-core Xeon handles 4-6 simultaneous FFmpeg encodes before throughput collapses (GetStream.io benchmarks).
- **GOP/b-frame sync loss:** Each process independently opens the stream, misses the first keyframe, and produces 3-15 seconds of blank frames while waiting for the next IDR.

**Prevention:**
- Use FFmpeg **as a library** (libavcodec/libavformat directly) rather than CLI subprocesses. This lets you pool codec contexts, share reference data, and control threading centrally.
- Alternatively, use a worker-pool model: a fixed pool of N FFmpeg processes (matching physical cores, e.g., 16 processes on a 16-core node) multiplexed across streams via a scheduler. Never exceed `cpu_count - 1` processes.
- Pin each worker process to a NUMA node or CPU set (via `numactl` or cpuset cgroups) to prevent scheduler thrashing.

**Warning signs:**
- `vmstat` showing context switches > 100K/sec per core
- `top` showing 200+ processes with individual RSS > 200MB
- `dmesg` showing `oom_killer` activity
- FFmpeg log lines: `[h264 @ 0x...] get_buffer() failed` (indicates memory pressure on codec buffer pool)

**Which phase should address it:** Phase 1 (Architecture Decision). The process model is a foundational choice. Do not start coding until this is decided.

**Confidence:** HIGH — GetStream.io benchmarks, FFmpeg upstream ML, multiple production post-mortems.

---

### Pitfall 2: B-Frame Reordering & PTS/DTS Confusion Yielding Wrong Timestamped Frames

**What goes wrong:** Extracted frames are tagged with incorrect timestamps (off by 1-16 frames) because the system doesn't account for B-frame reordering. The frame decoded from packet N may actually need to be presented at position N-2 (or later).

**Why it happens:** In H.264 with B-frames (bi-directional predicted frames), the decoding order (DTS) differs from presentation order (PTS). A typical GOP pattern like `I B B P B B P...` has DTS order different from PTS order. FFmpeg's decoders decode in DTS order but produce frames that must be reordered by PTS. If the extraction code uses `avcodec_decode_video2()` output order directly without PTS-based reordering, frames acquire wrong timestamps.

**Compounding factors:**
- **Open GOPs** (where B-frames reference frames across GOP boundaries) introduce additional complexity. FFmpeg has known issues with `closed_gop` and `broken_link` flags — it ignores them during cut operations (FFmpeg-user ML, May 2024).
- **`-fflags +igndts`** (ignore DTS) makes FFmpeg synthesize DTS from PTS, but when B-frames are present, this generates non-monotonic DTS that gets corrected by muxers, producing duplicated or dropped frames.
- **AVI container format** has no native timestamps — duration is inferred from frame count, making B-frame handling especially fragile.
- **MPEG-TS streams** often omit PTS/DTS from P/B-frame PES headers entirely (flags `PTS_DTS_flags == 00`), forcing FFmpeg to interpolate timestamps (FFmpeg-user ML, Feb 2024).

**Consequences:**
- Frame N's actual wall-clock time is off by 1-16 frame periods (33-533ms at 30fps). For time-interval extraction rules, this accumulates error.
- Scene-change detection triggers on the wrong frame because the PTS used for comparison is stale.
- Concat operations produce A/V desync or visual glitches because the first PTS is non-zero when cut at non-IDR positions.

**Prevention:**
- Always use `av_frame_get_best_effort_timestamp()` or `pkt->pts` for the extracted frame's timestamp, NOT the packet index or DTS.
- Enable `-vsync vfr` (variable frame rate) to prevent FFmpeg from duplicating frames to fill CFR gaps. Without it, FFmpeg emits duplicate frames when B-frame reordering leaves gaps.
- For live streams without DTS (common in RTSP/RTP), implement a PTS-based frame queue that reorders by PTS before timestamp assignment.
- Use the `dts2pts` bitstream filter (`-bsf dts2pts`) for H.264/HEVC streams where DTS is missing or unreliable. This reorders DTS into PTS using POC (picture order count).
- Never use `-fflags +igndts` with streams that contain B-frames. The resulting synthetic DTS values will be incorrect and cause muxer warnings/errors.

**Warning signs:**
- FFmpeg logs: `Non-monotonous DTS in output stream`
- Extracted frames with `pkt_pts_time` that jump backward or skip
- `ffprobe -show_frames` showing `pkt_dts != pkt_pts` for B-frames but extraction code ignoring the difference
- Scene-change-based rules triggering on frames that look visually similar (duplicates)

**Which phase should address it:** Phase 1 (Core Decoder). Must be correct before rule engine or Kafka integration is built.

**Confidence:** HIGH — FFmpeg-user ML extensive discussion, FFmpeg source code (`libavcodec/bsf/dts2pts.c`), Stack Overflow production reports.

---

### Pitfall 3: Memory Leaks in Long-Running FFmpeg Decoder Instances

**What goes wrong:** Over hours or days of continuous stream decoding, memory usage grows monotonically until OOM. Reported in FFmpeg upstream since at least 2007, with recurring patches for specific leak patterns.

**Why it happens:** FFmpeg's libavcodec has historically had subtle memory leaks in H.264 decoding paths:

- **SPS/PPS buffer leaks** — `free_tables()` historically didn't free `sps_buffers[MAX_SPS_COUNT]` and `pps_buffers[MAX_PPS_COUNT]` allocations. Fixed in 2010 but similar patterns recur in new codec features.
- **`av_frame_alloc()` without paired `av_frame_free()`** — Each decoded frame allocates AVBufferRefs for data planes. Calling `av_frame_unref()` without freeing the frame object leaves the reference counts in an inconsistent state on error paths (Stack Overflow, rapid TS fragment decoding).
- **`av_packet_alloc()` without `av_packet_unref()`** — AVPackets from `av_read_frame()` must be unreffed (not just freed) to release internal references. Using `av_free_packet()` (deprecated) instead of `av_packet_unref()` leaves reference-counted buffers alive.
- **HEVC concat memory leak** — Open FFmpeg ticket #10554 (2023): HEVC concat operations accumulate memory until exhaustion. Only HEVC triggers it, suggesting specific codec module leaks.
- **Dynamic resolution changes** — When a stream changes resolution mid-stream (common with adaptive bitrate), FFmpeg's frame-threaded decoder can leak due to async between main-thread and child-thread context updates (FFmpeg-devel ML, June 2019).

**Consequences:**
- After 6-72 hours of continuous operation, a 200MB baseline decoder grows to 2-8GB before OOM.
- In Kubernetes, this manifests as `OOMKilled` with exit code 137, forced restart, and loss of in-flight frames.
- The restart cycle loses the codec's reference frame cache, causing 1-3 seconds of I-frame wait before valid frames resume.

**Prevention:**
- **Audit all FFmpeg API calls against current documentation:**
  - Every `av_frame_alloc()` must have a matching `av_frame_free()` even on error paths
  - Every `av_read_frame()` packet must be unreffed with `av_packet_unref()` before reusing the packet
  - Use `av_packet_alloc()` at init time and reuse the same packet (calling `av_packet_unref()` between reads) rather than allocating per-frame
- **Track allocated memory internally:** Maintain a counter of allocated decode buffers. If it grows monotonically over 10,000 frames without correlating to bitrate changes, flag for investigation.
- **Periodic decoder reset:** For streams known to have resolution changes or corrupted bitstreams, force a decoder re-init every N frames (e.g., 500,000 frames ≈ 4.6 hours at 30fps). This is a last resort — prefer fixing the leak.
- **Run valgrind or ASAN in staging:** Before production deployment, run a 24-hour soak test with AddressSanitizer. FFmpeg's own `libavutil/mem.c` has historically had realloc leaks (realloc failure returning NULL without freeing original pointer — FFmpeg-devel ML, April 2010).

**Warning signs:**
- `container_memory_working_set_bytes` Prometheus metric showing linear growth over hours
- Valgrind showing `definitely lost` or `indirectly lost` bytes in `av_malloc`/`av_frame_alloc`
- FFmpeg log: `get_buffer() failed` — the decoder's internal buffer pool is exhausted
- `RES` column in `top` steadily increasing without plateaus

**Which phase should address it:** Phase 1 (Core Decoder) — initial implementation must use correct FFmpeg API patterns. Phase 6 (Stability Testing) — leak detection in long-running soak tests.

**Confidence:** HIGH — Multiple FFmpeg-devel ML threads (2007-2023), Stack Overflow production reports, OBS Studio FFmpeg plugin fix PR #7306.

---

### Pitfall 4: Kafka Message Size & Producer Buffer Backpressure Crash

**What goes wrong:** The producer crashes with `BufferExhaustedException`, OOM, or infinite retry loop because JPEG frames exceed Kafka's 1MB default message limit or the producer's internal buffer fills faster than the broker can drain.

**Why it happens:** A single 1080p JPEG frame at quality level 5 is approximately 200-500KB. At 200 streams × 1 fps × 200KB = 40MB/sec of raw frame data entering the producer's `RecordAccumulator`. The default `buffer.memory` is 32MB, meaning the buffer fills in under 1 second. When full, the producer either blocks (up to `max.block.ms`, default 60s) or throws `BufferExhaustedException`.

**The failure cascade:**

1. Producer buffer fills (send rate > broker drain rate, typically 5-8 MB/sec per broker partition with `acks=all`)
2. `KafkaProducer.send()` blocks waiting for buffer space
3. Decoder threads cannot enqueue frames → decoder pipeline stalls
4. RTSP/RTP kernel socket buffers fill → packets dropped at the NIC level
5. When producer eventually unblocks, it finds stale decoder frames and must skip or re-decode

**Documented real behavior** (rtsp-kafka-ingestion-template production post-mortem): "Under-provisioned brokers resulted in producer-side timeouts after prolonged sustained load. Frame drops occurred when broker ACKs could not keep pace with ingestion rate. Increasing FPS exposed Kafka disk and network bottlenecks faster than synthetic message tests."

**Kafka infinite retry bug** (KAFKA-8350, fixed in KIP-782): When `batch.size` >> topic `message.max.bytes`, the producer enters an infinite retry loop attempting to split batches that can never fit. Stack overflow in `FutureRecordMetadata.chain()` from recursive splitting attempts. Fixed by progressive batch size reduction (Kafka PR #20358, August 2025).

**Prevention:**

- **Understand: Kafka is not the right transport for large image payloads.** The recommended architecture for frame extraction is the **claim-check pattern**: store frame images in a blob store (S3/MinIO/NFS) and send only metadata (stream ID, timestamp, frame sequence number, blob path) through Kafka. This keeps Kafka message sizes under 1KB.
- **If sending frames through Kafka is required:**
  - Increase `message.max.bytes` (broker), `max.request.size` (producer), `max.partition.fetch.bytes` (consumer), and `fetch.max.bytes` (consumer) to 5-10MB
  - Set `buffer.memory` to at least 256MB (calculated as: max throughput per partition × broker latency × 2 for headroom)
  - Set `max.block.ms` to 5000ms (don't let the producer block decoder threads indefinitely)
  - Enable compression: `compression.type=snappy` (low CPU) or `zstd` (better ratio)
  - Set `batch.size` to match expected frame size (e.g., 524288 = 512KB)
  - Set `linger.ms` to 10-50ms to batch within reason without adding decoder latency
- **Implement backpressure:** Use a bounded queue between decoder and Kafka producer. When the queue reaches capacity, the decoder has two choices: drop the frame (acceptable for frame extraction) or block (risky for live streams). Prefer dropping the oldest buffered frame (producer side) when buffer is full.
- **Monitor:**
  - `buffer-available-bytes` — alert when < 20% of `buffer.memory`
  - `record-queue-time-avg` — alert when > 1000ms (indicates broker backpressure)
  - `records-lag-max` — consumer lag per partition

**Warning signs:**
- `BufferExhaustedException` in logs
- Kafka producer metrics showing `buffer-available-bytes` trending to zero
- Decoder frame rate dropping while CPU is idle (decoder waiting on Kafka send)
- Broker logs: `MESSAGE_TOO_LARGE` or `Record batch too large`
- Producer thread stuck in `org.apache.kafka.clients.producer.internals.BufferPool`

**Which phase should address it:** Phase 1 (Architecture Decision) — choose claim-check pattern or direct image transport. Phase 2 (Kafka Integration) — configure producer correctly.

**Confidence:** HIGH — Apache Kafka source code (BufferPool.java), KIP-782 discussion, Kafka PR #20358, multiple production post-mortems.

---

### Pitfall 5: CPU Throttling in Kubernetes Breaks Video Decoding

**What goes wrong:** Video decoder containers are CPU-throttled by Kubernetes CFS quota, causing frame drops, decoder stalls, and cascading stream reconnections. The throttling is invisible in standard CPU metrics (`container_cpu_usage_seconds_total` reports only usage, not throttling).

**Why it happens:** Kubernetes enforces CPU limits using Completely Fair Scheduler (CFS) quota. When a container exceeds its CPU limit, the kernel throttles it — the container's threads are paused and rescheduled. For video decoding:

- **Burst nature of decoding:** H.264 decoding is not uniform. I-frames (keyframes) require 3-10x more CPU than P/B-frames. A container at 80% average utilization will hit its CFS quota during every I-frame burst, causing micro-throttling.
- **Thread migration:** When throttled threads resume, they may be scheduled on a different physical core. This destroys CPU cache locality — the decoder's motion compensation data, reference frames, and entropy decoding context are all cold.
- **FFmpeg's `-threads auto`** detects all logical cores on the node (e.g., 48), not the container's CPU limit (e.g., 4). FFmpeg creates 72 threads (48 × 1.5), which all compete for the container's 4-CPU quota, amplifying throttling.
- **Cascading impact:** A throttled decoder misses RTSP RTP sequence numbers. It must send RTSP keepalive or reconnect. If multiple streams hit I-frames simultaneously (common at scene changes), the thundering herd of reconnections overwhelms the cameras.

**The numbers** (MainConcept Kubernetes benchmarks): Video encoders with `cpu_count` > available quota showed 40-60% throughput degradation versus bare metal on the same hardware.

**CPUs are NOT compressible in video decoding.** The common wisdom "CPU is compressible, memory is not" from web application design is wrong for video decoding. When a web server is throttled, requests queue up. When a video decoder is throttled, real-time frames are permanently lost — you cannot "catch up" by processing faster later.

**Prevention:**

- **Set `resources.limits.cpu = resources.requests.cpu`** (Guaranteed QoS with equal request/limit). This prevents CFS throttling entirely when the CPU Manager static policy is enabled, as the cpuset cgroup gives exclusive cores.
- **Enable CPU Manager static policy** on Kubernetes nodes (`--cpu-manager-policy=static` kubelet flag). This pins containers with integer CPU requests to dedicated physical cores, eliminating cache thrashing.
- **Set FFmpeg threads explicitly:** `-threads N` where N = the CPU limit (not auto). This prevents FFmpeg from creating more threads than available CPUs.
- **Set `GOMEMLIMIT`** (if using Go) to 90% of memory limit so GC responds to container limits and doesn't contribute to pressure.
- **Use pod-level CPU resources** (K8s 1.36+) for multi-container decoder pods to ensure aggregate limits are respected.
- **Right-size limits:** A single 1080p H.264 decode thread consumes approximately 20-40% of a modern x86 core at 30fps. Therefore, a 4-CPU container handles roughly 10-20 concurrent decode threads. Factor 2x headroom for I-frame bursts.

**Warning signs:**
- `container_cpu_cfs_throttled_seconds_total` Prometheus metric > 0 (any non-zero value means throttling is occurring)
- `kubectl top pod` showing CPU usage consistently near the limit
- FFmpeg logs showing `real-time buffer [video] too full or near too full` during I-frames
- Stream reconnection count spiking during high-motion periods
- Frame rate dropping below target while CPU is at 100% (not throttled)

**Which phase should address it:** Phase 2 (Kubernetes Configuration). Must be tested in Phase 6 (Load Testing) with realistic I-frame burst patterns.

**Confidence:** HIGH — MainConcept Kubernetes benchmarks, FFmpeg-user ML reports, Kubernetes documentation on CPU Manager, multiple production video Kubernetes war stories.

---

### Pitfall 6: RTSP Half-Open Connections & Reconnection Thundering Herd

**What goes wrong:** After a transient network blip (1-30 seconds), all 200+ streams attempt to reconnect simultaneously, overwhelming both the Kubernetes node and the upstream camera/NVR. Most reconnections fail because the camera can only serve 4-8 concurrent RTSP sessions. The system enters a reconnect storm loop.

**Why it happens:** RTSP uses TCP, and TCP handles transient disconnects by design — it keeps the connection half-open indefinitely (Live555 discussion, 2018). The RTSP server doesn't know the client disconnected until it tries to send data. Detection mechanisms vary:

- **No TCP keepalive by default:** Without `SO_KEEPALIVE` with aggressive timeouts, a disconnected RTSP client's session stays alive on the server for 60+ seconds (Live555 default liveness timeout).
- **UDP-based RTP:** When RTSP uses UDP for media transport, the RTSP control connection stays open even after RTP packets stop arriving. The server may not detect the client lost for minutes.
- **FFmpeg's `-timeout`:** This only sets the initial connection timeout. It does NOT help with mid-stream disconnects. FFmpeg has no built-in reconnection logic for RTSP — the process must be restarted externally.
- **Simultaneous detection:** When the network recovers, all decoder processes detect the stream interruption within the same monitoring interval, triggering N simultaneous `avformat_open_input()` calls to the same camera.

**Consequences:**
- Camera RTSP session limit exhausted (typical: 4-20 sessions for IP cameras)
- Connection refusals cascade → some streams never recover
- CPU spikes from N simultaneous FFmpeg process creations
- If using systemd-style restart (Restart=always with RestartSec=5), 200 processes restarting every 5 seconds creates a sustained load of 40 process creations/second

**Prevention:**

- **Exponential backoff on reconnection:** Minimum backoff 1s, maximum 120s, with jitter (random delay ±25%). Formula: `backoff = min(1 * 2^attempt, 120) + random(-0.25*backoff, 0.25*backoff)`
- **Staggered startup:** On initial deployment or after full outage, inject a per-stream startup delay: `delay = streamIndex * totalBackoff / streamCount`. Spreads 200 connections over 10-30 seconds.
- **TCP keepalive on RTSP socket:** Set `SO_KEEPALIVE` with `tcp_keepalive_time=10`, `tcp_keepalive_intvl=3`, `tcp_keepalive_probes=3`. This detects dead connections within ~19 seconds instead of OS default 2 hours 11 minutes (Linux defaults).
- **RTSP transport forcing:** Use `-rtsp_transport tcp` to force RTP over TCP (interleaved mode). This avoids the UDP half-open problem entirely, at the cost of slightly higher overhead. TCP gives you reliable connection state tracking.
- **Per-camera connection limits:** Track active connections per camera IP. If a camera maxes out (e.g., 8 sessions), queue new connections with backoff rather than hammering the camera.
- **Health check before reconnect:** Ensure network is up (ping the gateway, check DNS) before attempting RTSP reconnection. Prevents pointless reconnect storms during prolonged outages.

**Warning signs:**
- RTSP response code `453 Not Enough Bandwidth` or `503 Service Unavailable`
- Camera vendor logs showing repeated authentication attempts
- Network monitoring showing SYN packets with no SYN-ACK response
- System log: `rtsp://...: Connection timed out` from multiple streams at identical timestamps
- `ss -s` showing many `CLOSE_WAIT` sockets on the node

**Which phase should address it:** Phase 1 (Core Decoder) — reconnection logic is foundational. Phase 3 (Stability) — exponential backoff and staggered startup.

**Confidence:** HIGH — Live555 mailing list discussions (2013-2024), FFmpeg-user reconnection solutions, SRS issue #3972, Janus Gateway RTSP reconnection commit, go2rtc stability improvements.

---

### Pitfall 7: GC Pressure & Frame Allocation in Garbage-Collected Languages

**What goes wrong:** In Go, Java, or Python implementations, the garbage collector cannot keep up with the allocation rate of decoded frames. The application spends more time in GC than in actual frame processing, causing frame drops and latency spikes.

**Why it happens:** Video decoding at scale creates extreme allocation patterns:

- Each decoded 1080p frame is approximately 3MB in YUV420P format (1920×1080 × 1.5 bytes/pixel)
- Converting to RGB for JPEG encoding produces another 6MB per frame
- A single stream at 1 fps generates ~9MB/sec of allocations, not counting metadata objects
- At 200 streams × 1 fps = 1.8GB/sec allocation rate

For **Go** specifically:
- Go's GC is a non-generational, concurrent mark-sweep collector. It stops-the-world for mark termination (typically <500µs per cycle), but at 1.8GB/sec allocation, GC cycles trigger every 1-2 seconds.
- Every GC cycle scans all allocated memory to find live objects. With gigabytes of frame buffers, this scan itself consumes significant CPU.
- `GOMEMLIMIT` (Go 1.19+) helps by triggering GC earlier, but cannot reduce the fundamental allocation rate.

For **Python**: The GIL means only one thread decodes at a time. Real-world Python+OpenCV frame extraction from RTSP streams achieves 18-20 fps at 640×480 (documented in rtsp-kafka-ingestion-template). Scaling to 200 1080p streams requires multiprocessing, which multiplies memory proportionally.

For **Java/JVM**: The JVM's garbage collectors (G1GC, ZGC, Shenandoah) handle throughput well but have high baseline memory requirements (4-8GB heap minimum for ZGC). Each card-marking and remembered-set scan adds CPU overhead proportional to allocation rate.

**Prevention:**

- **Use Rust or C/C++ for the decoder pipeline.** This eliminates GC entirely. Frame buffers are explicitly managed (pool allocated) and freed deterministically. This is the single strongest argument for a systems language in this domain.
- **If using Go:** 
  - Use `GOMEMLIMIT` set to 90% of container memory limit
  - Pre-allocate frame buffer pools (sync.Pool with manual reset) to avoid per-frame allocations
  - Use `runtime.GC()` triggered on buffer pool pressure, not on timer
  - Keep hot path allocation-free: reuse `image.Image` structs, use `unsafe` casts to YUV buffers
  - Monitor `runtime.ReadMemStats()`: `NumGC` rate should be < 10/sec, `PauseTotalNs` should be < 1% of wall time
- **Use frame pool pattern:** Pre-allocate N frame buffers (e.g., buffer pool for 30 frames per stream) and rotate rather than allocate-free. This converts GC pressure into deterministic memory reuse.
- **Prefer JPEG encoding at the FFmpeg/C level** before the frame enters the managed language runtime. `sws_scale` direct to JPEG in C avoids allocating 6MB RGB frames in Go/Python memory.

**Warning signs:**
- Prometheus `go_gc_duration_seconds_sum` / `go_gc_duration_seconds_count` showing average GC pause > 1ms
- `container_memory_working_set_bytes` oscillating between high and low (GC thrashing pattern)
- CPU profile showing > 20% time in GC functions (`gcBgMarkWorker`, `gcDrain`, `scanobject`)
- Frame rate dropping every N seconds in a sawtooth pattern matching GC cycle timing
- Python: GIL contention visible in `perf top` with `PyEval_EvalFrameEx` taking > 50%

**Which phase should address it:** Phase 0 (Language Decision). This is a foundational architecture choice. GC pressure is the primary reason to choose Rust/C++ over managed languages for this workload.

**Confidence:** HIGH — Go GC benchmarks at 1GB+/sec allocation rates, Python GIL limitations documented in production frame extraction systems, JVM GC tuning guides for high-throughput media processing.

---

### Pitfall 8: Keyframe Alignment & Initial Decode Delay Causing Blank Frames

**What goes wrong:** When connecting to an RTSP stream or mid-stream after a seek, the system outputs blank/corrupted frames for the first 0.5-3 seconds because it hasn't received a keyframe (IDR) yet.

**Why it happens:** H.264's decoding dependency chain means you cannot decode P or B frames without a reference I-frame. When connecting mid-stream:

- **RTSP:** The server sends packets from wherever it is in the stream. The first packet is likely a P-frame referencing a previous I-frame that was never received.
- **FFmpeg behavior:** `avcodec_decode_video2()` returns frames with `got_picture=0` for corrupted/unreferenced frames. The caller must skip these until an I-frame is successfully decoded.
- **H.264 SPS/PPS:** Before any frame can be decoded, the decoder needs Sequence Parameter Set and Picture Parameter Set NAL units. These may arrive in a separate packet before the IDR, or may be sent as part of the RTSP DESCRIBE response (SDP). If SPS/PPS are missing, decoding fails silently.
- **Seek behavior:** `av_seek_frame()` to a non-keyframe position (e.g., time-based seek with `-ss` after `-i` on a file) starts decoding from the previous keyframe but outputs frames starting at the requested PTS. However, for RTSP/live streams, seeking is meaningless.

**Consequences:**
- Rule engine fires on corrupted/blank frames because timestamp-based extraction doesn't wait for valid content
- Empty or green frames pushed to Kafka, wasting storage and confusing downstream consumers
- Automated quality checks on Kafka consumer side flag "all black frames" as camera failure even though the camera is fine
- For every reconnection, 0.5-3 seconds of usable frames are lost

**Prevention:**
- **Discard frames until first keyframe decoded:** Keep a `framesDecoded` counter. Only start rule engine evaluation after the first frame with `pkt->flags & AV_PKT_FLAG_KEY`.
- **Send SPS/PPS in-band:** When connecting via RTSP, ensure the transport (TCP interleaved) includes SPS/PPS. Some cameras send SPS/PPS only in DESCRIBE response, which can be missed on reconnect.
- **Use `-analyzeduration` and `-probesize`:** For file-based sources, set these high enough (e.g., `-analyzeduration 100M -probesize 100M`) to ensure FFmpeg reads enough data to find the stream parameters before starting decode.
- **Send a "no-frame available" sentinel to Kafka:** For metadata-based frame tracking, push a heartbeat per stream every 5 seconds with `frame_valid: false` until the first valid keyframe is decoded. This allows the consumer to distinguish "camera down" from "still buffering."
- **On reconnection:** Wait for the first IDR frame before resuming frame delivery. This adds 0.5-3 seconds of "dead time" per reconnect but avoids pushing bad data.

**Warning signs:**
- FFmpeg logs: `decode_slice_header error`, `no frame!` at stream start
- First N frames of every stream are identical (green/gray test pattern or last-frame-hold from decoder)
- Kafka topic contains frames with `streamN_frame_001.jpg` that are all identical (corrupted decode state)
- `pkt_pts_time` of first valid frame > `stream_start_time + 3s`

**Which phase should address it:** Phase 1 (Core Decoder). Must be correct from day one — it's the foundation of every other feature.

**Confidence:** HIGH — FFmpeg/libavcodec documented behavior, production reports from RTSP ingestion systems.

---

### Pitfall 9: HLS Segment Download Failures & Retry Storms

**What goes wrong:** When processing HLS streams, the system downloads playlist and segment files via HTTP. Transient network issues trigger retries, which queue up, causing a retry storm that overwhelms the HTTP server and cascades into massive frame delivery delays.

**Why it happens:** HLS processing involves periodically fetching the `.m3u8` playlist (every 2-10 seconds) and downloading new `.ts` or `.m4s` segments. With 200+ streams:

- Each stream fetches a playlist every N seconds and up to M segments per interval
- Default segment duration is 6 seconds → 1-3 segments per fetch interval
- Total HTTP requests at steady state: 200 streams × (1 playlist + 2 segments) / 6s ≈ 100 req/sec
- Each failed segment triggers a retry, and retries may be uncoordinated

**Fan-out on failure:** When the HTTP server serving the HLS manifests has a hiccup (e.g., CDN edge node failover), 200 streams × 3 fetches each × 3 retries = 1800 HTTP requests in a few seconds, all targeting the same origin. This can trigger CDN rate limiting, origin overload, and cascading failure.

**Keyframe alignment in segments:** HLS segments do not necessarily start with a keyframe. If extraction logic assumes every segment begins with an IDR, the first 1-6 seconds of each segment produce garbled frames or wrong timestamps.

**Prevention:**
- **Use persistent HTTP connections (HTTP/1.1 keepalive or HTTP/2)** to avoid TCP connection setup overhead per segment fetch.
- **Throttle retries per origin:** Use a token bucket per origin server (cameras/NVRs). Maximum 10 simultaneous connections per origin.
- **Segment prefetch with overlap:** Download segments with a 1-segment overlap. If segment N+1 download fails, continue extracting from segment N's cached frames until retry succeeds or timeout.
- **Keyframe-aware extraction:** Before processing a segment, scan for the first IDR NAL unit. Discard frames before it. For MPEG-TS segments, this requires parsing the PES packet headers.
- **Fail-open on segment failure:** If a segment can't be downloaded after max retries, skip it (with a log entry and metric) rather than blocking extraction. A missing 6-second segment is acceptable; a stuck pipeline is not.

**Warning signs:**
- HTTP response: `429 Too Many Requests`, `503 Service Unavailable`
- Segment download times increasing linearly (indicates origin queueing)
- `kProbe` (if using Go) showing growing HTTP connection pool waiters
- CDN logs showing spike in 5xx errors from your origin IP range

**Which phase should address it:** Phase 1 (Core Decoder) — HLS is a primary input format. Phase 3 (Stability) — retry throttling.

**Confidence:** MEDIUM — patterns derived from CDN/HTTP client best practices and HLS specification behavior. Less documented than RTSP pitfalls because HLS for frame extraction is less common.

---

### Pitfall 10: Over-Engineering the Rule Engine Before Basic Extraction Works

**What goes wrong:** The team spends weeks building a flexible, programmable rule engine (time-based + scene-change + ML-based + custom expressions) before achieving reliable frame extraction, stream connectivity, or Kafka delivery. The result: a beautiful rule engine that processes corrupted frames from a broken pipeline.

**Why it happens:** The rule engine is the most intellectually interesting component. It's where "smart" logic lives. In contrast, getting FFmpeg to reliably decode 200 streams from flaky RTSP connections is grungy, defensive engineering. Teams naturally gravitate toward the interesting problem and defer the unglamorous reliability work.

**Consequences:**
- When the rule engine finally has data to process, it discovers that 30% of frames have wrong timestamps (B-frame pitfall), 10% are corrupted (keyframe alignment), and 5% are duplicated (vsync CFR default)
- The scene-change detection logic must be rewritten because it was tuned on clean files, not on RTSP streams with packet loss
- The elegant YAML-based rule DSL has no users because the system can't stay connected to streams for more than 2 hours

**Prevention:**

- **Phase ordering is critical:**
  1. First: Open ONE RTSP stream, decode frames, extract to file. Prove the pipeline works.
  2. Next: 10 streams to Kafka with raw timestamps. Prove delivery reliability.
  3. Then: Simple time-based rule (extract every N seconds). This is a configuration parameter, not an engine.
  4. Finally: Scene detection, composite rules, rule DSL.
- **Rule engine should be configuration, not code.** Start with a JSON/TOML config file that specifies `extract_interval_seconds` and `scene_threshold`. If this covers 80% of use cases, the complex rule engine is a future problem.
- **Validate against real camera feeds from day 1.** Synthetic test videos with clean cuts and constant frame rates mask every meaningful edge case. Have a test harness that connects to actual RTSP cameras (or a simulated flaky RTSP server) during development.

**Warning signs:**
- Sprint demos showing "rule engine v3" but the system can only process local MP4 files
- Unit tests for scene detection using pre-extracted frames (avoiding the real pipeline)
- "We'll fix stream handling after the rule engine is done" — the canonical last words of a doomed project
- More code in the rule engine package than in the decoder package

**Which phase should address it:** Phase 0 (Project Planning). Must be enforced by the project roadmap. The rule engine is Phase 4+, not Phase 1.

**Confidence:** HIGH — Universal software engineering anti-pattern with specific video processing context.

---

## Moderate Pitfalls

### Pitfall 11: Variable Frame Rate (VFR) Streams Breaking Time-Interval Extraction

**What goes wrong:** A stream advertised as 30fps actually has VFR (variable frame rate), with frame intervals ranging from 16ms to 500ms. Time-based extraction rules (`extract every 1 second`) produce irregular results — sometimes 0 frames, sometimes 2 frames per "second."

**Why it happens:** Many IP cameras, screen recordings, and video conferencing feeds use VFR to save bandwidth during static scenes. FFmpeg reports a nominal average frame rate in `ffprobe` output, but the actual frame intervals vary. Common causes:
- Cameras that skip frames in low-light/low-motion conditions (common with Hikvision, Dahua, and other surveillance cameras)
- H.264 scene-based encoding where the encoder drops frames when nothing changes
- RTP packet loss causing frame skips (decoder drops unrecoverable frames)
- Variable frame rate from screen capture/camera hybrid sources

**FFmpeg's `-vsync` defaults are dangerous in this context:**
- `-vsync cfr` (default) — drops or duplicates frames to enforce constant frame rate. This silently creates phantom frames (duplicates) that look like valid extractions. With VFR input, `dup=245` and `drop=37` in the log means you're getting 245 useless frames.
- `-vsync vfr` — preserves original timestamps. Use this, but understand that the output frame count will not match `duration × fps` calculations.

**Prevention:**
- **Always add `vf vfrdet` to your test commands.** This filter outputs `VFR:0.000000 (0/14670)` for true CFR, or `VFR:0.150000 (2200/14670)` for VFR. The second number is the number of non-uniform frame intervals.
- **Use `-vsync vfr` (or equivalently `-vsync 0`) for all frame extraction.** Never use the default `-vsync cfr` (or `-vsync 1`).
- **For time-based rules, use PTS timestamps as the extraction signal, not frame count.** The rule should say "extract when PTS crosses N-second boundary" rather than "extract every Nth frame."
- **If VFR is causing problems with the MJPEG encoder** (known issue: `fps_mode passthrough` with VFR can fail), use the filter approach: `-vf "settb=AVTB,setpts=N/TB" -fps_mode passthrough` to retime frames before encoding.

**Warning signs:**
- `ffprobe -v debug -show_streams` showing `avg_frame_rate` != `r_frame_rate` (indicates VFR)
- `vf vfrdet` output showing the VFR ratio > 0.01
- Frame count not matching `duration × fps` by more than 1%
- Duplicate frames outnumbering unique frames in extracted output

**Which phase should address it:** Phase 1 (Core Decoder). Include VFR detection in the frame pipeline from the start.

**Confidence:** HIGH — Documented FFmpeg behavior with VFR, repeated questions on FFmpeg-user and Super User communities.

---

### Pitfall 12: False Sharing & Cache Misses in Concurrent Stream Processing

**What goes wrong:** A multi-threaded decoder pool shows unexpectedly poor scaling — 8 threads process only 3x faster than 1, and 16 threads show regression. The bottleneck is CPU cache contention, not algorithmic efficiency.

**Why it happens:** Video decoding involves frequent writes to shared data structures: frame buffer pools, codec context state, packet queues. When these structures share cache lines:

- **False sharing:** Two threads modify different variables that happen to share a CPU cache line. The cache coherence protocol invalidates the cache line for both threads on every write, forcing a reload from L3 or RAM. A single cache line invalidation costs ~100 cycles for L3, ~300 cycles for local DRAM.
- **Allocator contention:** If all decoder threads share a global frame buffer pool with a single mutex, the mutex contention alone can limit scaling to 4-8 cores.
- **NUMA violations:** On multi-socket systems, a thread pinned to socket 0 reading frame data allocated on socket 1 DRAM pays a 1.3-2x latency penalty per memory access.

**Prevention:**
- **Thread-local decoder contexts:** Each decode thread should have its own `AVCodecContext`, packet queue, and frame pool. Minimize shared mutable state.
- **Separate frame pool per thread:** Pre-allocate N frame buffers per thread (not globally), sized for worst-case decoding latency.
- **NUMA-aware allocation:** Use `libnuma` or `numactl` with `--membind` to ensure memory is allocated on the same NUMA node as the decode thread. Kubernetes with CPU Manager static policy + Guaranteed QoS handles this for containerized workloads.
- **Align structures to cache lines:** In C/Rust, use `#[repr(align(64))]` or `__attribute__((aligned(64)))` on hot data structures (frame pool indices, mutexes, atomic counters).
- **Separate producer/consumer cores:** In NUMA architectures, dedicate a subset of cores to decoding (frame producers) and another subset to JPEG encoding + Kafka push (frame consumers). This prevents decode threads from being preempted by I/O.

**Warning signs:**
- Linear scaling stops at 4-8 threads regardless of available cores
- `perf stat -e cache-misses,cache-references` showing cache miss rate > 10%
- `perf c2c` (Cache-to-Cache) analysis showing high cross-socket transfers
- Thread profiling showing time spent in `lock prefix` instructions or `__pthread_mutex_lock` > 5%

**Which phase should address it:** Phase 1 (Architecture Decision) — thread model is foundational. Phase 5 (Performance Optimization) — cache topology optimization.

**Confidence:** MEDIUM — General concurrency architecture best practices. Specific video decoding numbers would require profiling on the target hardware.

---

### Pitfall 13: OS Resource Limits (ulimit, File Descriptors, Epoll)

**What goes wrong:** The system hits OS-level resource limits at 150-300 streams, causing "too many open files" errors, connection refusals, or silent packet drops.

**Why it happens:** Each video stream consumes multiple OS resources:
- **File descriptors:** Each RTSP TCP connection uses 1 FD. If using TCP transport for RTP, that's 1 connection for RTSP + 2 sockets for RTP/RTCP (even with interleaved mode) per stream. 200 streams × 2 FDs = 400 FDs minimum, plus Kafka producer connections, HTTP health checks, log files, etc.
- **Epoll instances:** FFmpeg may use epoll for I/O event handling. Each FFmpeg process creates its own epoll FD. 200 processes = 200 epoll FDs just for video.
- **Memory map areas:** FFmpeg's frame buffer pools can consume significant mmap'd memory, hitting `vm.max_map_count` limits.
- **Threads:** Each process with `-threads auto` creates ~6 threads on a 4-core system. 200 processes = 1200 threads. Default `threads-max` on Linux = ~126000 (kernel 5.x), but thread stacks consume memory (8MB default virtual = 9.6GB virtual for 1200 threads).

**Prevention:**
- **Calculate FD requirements per stream before deployment:** RTSP TCP + optional RTP sockets + control socket = 2-4 FDs. Multiply by max stream count, add 50% headroom for Kafka, metrics, health checks.
- **Set system limits higher:**
  ```
  # /etc/security/limits.conf
  * soft nofile 1048576
  * hard nofile 1048576
  
  # /etc/sysctl.conf
  fs.file-max = 2097152
  net.core.somaxconn = 65535
  ```
- **In Kubernetes:** Set `resources.limits.ephemeral-storage` appropriately for frame temporary storage. Set proper `Pod.Spec.HostAliases` for DNS resolution stability. Ensure `enableServiceLinks: false` to reduce FD usage from environment variables.
- **Monitor FD usage:** Prometheus `process_open_fds` metric with alert at 80% of limit. At container level, monitor `/sys/fs/cgroup/pids/pids.current`.
- **Use `SO_REUSEPORT` and `SO_REUSEADDR`** to avoid TCP bind failures during rapid reconnection cycles.

**Warning signs:**
- `kubectl logs` showing `Too many open files`
- `dmesg` showing `VFS: file-max limit <number> reached`
- System logs: `socket: Too many open files`
- Pod status: `CrashLoopBackOff` with OOM or "can't open file" errors
- Monitoring showing `process_open_fds` steadily increasing without corresponding stream count

**Which phase should address it:** Phase 2 (Kubernetes Configuration). Document in deployment runbook.

**Confidence:** HIGH — Standard OS administration knowledge. Limits are deterministic per-stream.

---

### Pitfall 14: Audio-Only Streams Wasting Decode Resources

**What goes wrong:** The system silently decodes audio-only streams (or streams where the audio track happens to be stream 0 but video is stream 1, or streams where the video track is disabled/misconfigured), wasting CPU and producing no frames.

**Why it happens:** FFmpeg may auto-select the first stream it finds, which could be audio. Common scenarios:
- Camera configured for audio-only recording
- SDP in RTSP DESCRIBE response lists audio track before video track
- HLS manifest that includes both audio and video renditions, and the playlist parser selects the audio-only variant
- Stream with `codec_type == AVMEDIA_TYPE_VIDEO` but `codec_id == AV_CODEC_ID_NONE` (broken camera firmware)

**Prevention:**
- **Always select video streams explicitly:** `-map 0:v:0` or programmatically via `av_find_best_stream(fmt_ctx, AVMEDIA_TYPE_VIDEO, -1, -1, NULL, 0)`. Never rely on stream index.
- **Validate the video stream:** After selecting, check that `codec_id != AV_CODEC_ID_NONE`, `width > 0`, `height > 0`.
- **Set minimum resolution threshold:** If a "video" stream has resolution < 64×64, it's probably a thumbnail or metadata track. Skip it.
- **Skip cameras in discovery:** When registering RTSP sources, probe with `ffprobe` first and reject streams with no valid video track. Log this clearly with the stream ID.

**Warning signs:**
- Zero frames extracted from a stream that appears to be "connected"
- FFmpeg logs showing audio frames decoded but none/every few video frames
- `ffprobe -show_streams` showing the only video stream has `codec_name=none` or `width=0`
- Successfully connected streams with "no frame" metric but active audio decoding

**Which phase should address it:** Phase 1 (Core Decoder) — stream selection logic.

**Confidence:** HIGH — Known issue in video surveillance systems, documented in multiple production deployment reports.

---

### Pitfall 15: JPEG Compression & Kafka Storage Trade-off Miscalculation

**What goes wrong:** The JPEG compression level used for frame extraction is set incorrectly, causing either:
- **Too high quality (~q:v 1-2):** Each frame is 400-600KB, making Kafka storage untenable at scale. 200 streams × 1 fps × 500KB = 100MB/sec = 8.6 TB/day.
- **Too low quality (~q:v 20-31):** Frames are useless for downstream processing (face recognition, OCR, analytics) due to compression artifacts, but the team doesn't realize until weeks later.

**Why it happens:** JPEG quality is often set to a "safe" default without understanding the trade-off. The `-q:v` flag ranges from 2 (best) to 31 (worst). The relationship is non-linear: the difference between q:v 2 and q:v 5 is ~10% quality loss but 40% size reduction. Between q:v 5 and q:v 15, quality degrades visibly while size drops another 50%.

**The numbers** (from FFmpeg benchmarks):
- 640×360 frame: q:v 2 = 36-38KB, q:v 5 = 23-28KB, q:v 10 = 12-15KB
- 1920×1080 frame: q:v 2 = 400-600KB, q:v 5 = 200-350KB, q:v 10 = 80-150KB

**Prevention:**
- **Default to `-q:v 5` for general-purpose frame extraction.** This provides visually lossless quality for human viewing and is adequate for most ML pipelines.
- **Set quality per use case via configuration:** The rule engine or stream configuration should allow specifying `jpeg_quality` per stream or per rule, because different consumers have different needs.
- **Downscale before encoding:** If the frame is for thumbnail/overview purposes (not ML), downscale to 640×360 before JPEG encoding. This reduces size by 85% with minimal information loss for the use case. Use `-vf scale=640:-2` (maintains aspect ratio, ensures even dimensions).
- **Consider WebP format:** WebP provides 25-35% better compression than JPEG at equivalent visual quality, with slightly higher encoding CPU cost. Only use if all downstream consumers support it.
- **Test with actual downstream pipeline:** Run 10,000 frames through your end-to-end system at each quality level and measure both throughput and downstream task accuracy (if applicable).

**Warning signs:**
- Kafka storage growing faster than expected — check average message size
- Downstream team complaining about "blocky" or "blurry" frames months after deployment
- Average frame size > 500KB for 1080p (indicates q:v < 3 or no downscaling)
- Average frame size < 50KB for 1080p (indicates q:v > 15, too lossy)
- Kafka broker disk filling at > 100GB/day unexpectedly

**Which phase should address it:** Phase 2 (Kafka Integration). Establish the baseline before production.

**Confidence:** HIGH — FFmpeg JPEG quality benchmarks, multiple production deployments documented.

---

### Pitfall 16: Codec Negotiation & Resolution Changes Mid-Stream

**What goes wrong:** A stream starts in H.264 1080p, then changes to H.265 4K (or drops to 720p, or changes pixel format). The decoder crashes, leaks memory, or produces corrupted frames until manually reset.

**Why it happens:** Adaptive Bitrate (ABR) and camera reconfiguration can change codec parameters mid-stream. FFmpeg's `AVCodecContext` is initialized with the original stream's parameters. When a resolution change occurs:

- **`avcodec_flush_buffers()` must be called** but is often missed. Without it, the decoder maintains state for the old resolution and new packets produce garbage.
- **Frame-threaded decoder race:** FFmpeg's frame-threaded decoding can leak memory during resolution changes because the main thread updates its context from the child thread asynchronously, potentially using stale or double-freed pointers (FFmpeg-devel ML, June 2019 patch).
- **`avcodec_receive_frame()` API** (FFmpeg 3.1+) handles resolution changes more gracefully by returning `AVERROR(EAGAIN)` until the decoder reinitializes internally, but older `avcodec_decode_video2()` does not.

**Prevention:**
- **Detect resolution changes:** After each `avcodec_send_packet()` + `avcodec_receive_frame()`, compare the frame's `width`/`height` with the current codec context's values. If they differ, the stream parameters changed.
- **Handle resolution changes gracefully:**
  1. Call `avcodec_flush_buffers(decoder_ctx)`
  2. Update `decoder_ctx->width`, `decoder_ctx->height`, `pix_fmt` from the new frame
  3. Reallocate any dependent buffers (SWS context, frame pool)
  4. Continue decoding
- **For pixel format changes** (e.g., yuv420p to yuvj420p): Recreate the `SwsContext` with `sws_getContext()` for the new format.
- **For codec changes** (H.264 → H.265): This requires creating a new `AVCodecContext` with the new `AVCodec`. The old context must be fully drained and closed.
- **Set `AV_CODEC_FLAG_CHANNELS` or `AV_CODEC_FLAG_GLOBAL_HEADER`** as appropriate for the stream format to avoid extradata changes.

**Warning signs:**
- FFmpeg log: `[h264 @ 0x...] Increasing reorder buffer to 1` (indicates resolution change detected)
- FFmpeg log: `SPS/PPS changed in the middle of the stream`
- Decoder returning frames with `width != decoder_ctx->width`
- Memory growth without stream reconnection (resolution change without buffer flush)
- `avcodec_receive_frame()` returning `AVERROR_INVALIDDATA` after many successful decodes

**Which phase should address it:** Phase 1 (Core Decoder) — robust decoder loop handles parameter changes. Phase 6 (Stability Testing) — test with ABR streams.

**Confidence:** MEDIUM — Well-documented FFmpeg behavior but resolution change handling is implementation-specific and often overlooked in tutorials.

---

### Pitfall 17: Thundering Herd on Kafka Consumer Rebalance

**What goes wrong:** When a Kafka consumer group rebalances (new consumer joins, existing one crashes), all frame processing pauses for 5-60 seconds. When processing resumes, all buffered frames arrive simultaneously, overwhelming the downstream system.

**Why it happens:** Kafka consumer group rebalancing uses a `stop-the-world` protocol: all consumers in the group stop processing, surrender their partition assignments, and the group coordinator reassigns partitions. During this time:
- No frames are consumed from Kafka
- Producers keep writing frames, building up broker-side storage
- When rebalance completes, consumers read from the end offset (if `auto.offset.reset=latest`) or process the backlog (if `earliest` or committed offset)

**For frame extraction:** This means a downstream analytics system sees a 5-60 second gap in frames, followed by a burst of frames for the accumulated period. For time-sensitive applications (alerts based on frame analysis), this gap is invisible data loss.

**Prevention:**
- **Prefer `StaticGroupMembership`** (ConsumerConfig `GROUP_INSTANCE_ID_CONFIG`): Available since Kafka 2.3. This assigns a fixed instance ID per consumer, preventing rebalances when consumers restart.
- **Set `session.timeout.ms` higher** (e.g., 60s instead of default 10s) to avoid spurious rebalances from GC pauses or brief network blips.
- **Set `max.poll.interval.ms` higher** (e.g., 300s) to accommodate frame processing time without triggering rebalance.
- **Use separate consumer group per stream** if each stream produces to its own topic/partition. This isolates rebalance impact.
- **For critical streams:** Implement a local frame buffer in the decoder that stores the last N seconds of extracted frame metadata, so Kafka rebalance doesn't cause data loss — the decoder can replay from its buffer if the consumer reconnects quickly.

**Warning signs:**
- Kafka consumer group metrics: `kafka_consumer_group_rebalance_rate_per_second` > 0.01 (rebalances more than once per 100 seconds)
- Consumer logs: `RebalanceInProgressException` or `CommitFailedException`
- Processing latency spikes at regular intervals matching rebalance frequency
- Downstream system receiving bursts of frames after quiet periods

**Which phase should address it:** Phase 2 (Kafka Integration) — consumer group configuration. Phase 4 (Production Readiness) — rebalance monitoring.

**Confidence:** MEDIUM — Well-known Kafka behavior but specific impact on frame extraction systems derived from experience with similar pipelines.

---

## Minor Pitfalls

### Pitfall 18: Wrong FFmpeg Seek Mode for Timestamp-Accurate Extraction

**What goes wrong:** Frame extraction for a specific timestamp (e.g., "extract frame at 00:01:30") takes 10x longer than necessary because `-ss` is placed after `-i` instead of before it.

**Why it happens:** Two modes for seeking with FFmpeg:
- **Input seeking** (`-ss 00:01:30 -i input.mp4`): FFmpeg seeks using keyframe index, starts decoding from the nearest keyframe before the target timestamp. Extremely fast (milliseconds on hours-long video).
- **Output seeking** (`-i input.mp4 -ss 00:01:30`): FFmpeg decodes every frame from the beginning but only outputs frames after the timestamp. Slow (minutes on hours-long video).

The intuitive placement (`-i input.mp4 -ss ...`) is the slow one. This is a common mistake even in production systems. Verified: "Put `-ss` before `-i`... On a 2-hour file, that's the difference between milliseconds and minutes" (DEV.to FFmpeg guide, 2026).

**Prevention:**
- Always place `-ss TIMESTAMP` before `-i INPUT_FILE` for timestamp-accurate seeking
- For frame-accurate extraction (specific frame number), use `-vf "select=eq(n\,FRAME_NUM)"` with `-vsync vfr` after input seeking
- Input seeking lands on the nearest keyframe; add `-frames:v 1` to output exactly one frame

**Warning signs:**
- Seeking to a timestamp takes > 2 seconds for a < 30-minute video
- FFmpeg log showing thousands of decoded frames before the first output frame
- CPU pegged at 100% during seek (decoding all prior frames)

**Which phase should address it:** Phase 1 (Core Decoder) — code review of seek logic.

**Confidence:** HIGH — FFmpeg documentation and community consensus.

---

### Pitfall 19: Custom Decoder Instead of Leveraging FFmpeg

**What goes wrong:** The team decides to write a custom H.264 decoder "for performance" or "to avoid FFmpeg complexity." Six months later, they have a barely-functional decoder with 10% of FFmpeg's codec support and zero performance advantage.

**Why it happens:** H.264 decoding is deceptively complex. The spec is 550+ pages. Production-quality decoding requires:
- Entropy decoding (CABAC and CAVLC)
- Motion compensation (quarter-pixel interpolation, weighted prediction)
- Deblocking filter (adaptive, luma/chroma)
- Reference frame management (DPB buffer, memory management control operations)
- Error concealment (conceal corrupted slices without crashing)
- Multi-reference frame handling (up to 16 reference frames)

FFmpeg's software H.264 decoder has ~30,000 lines of hand-optimized assembly (for x86 SIMD) and C code. It has been tuned by video codec experts for 15+ years. A custom decoder will not match its performance or correctness.

**Consequences:** Building custom video decoding is a project-ending mistake for most teams. The time and expertise required is vastly underestimated.

**Prevention:**
- Use FFmpeg libavcodec as a library. It is the correct choice for CPU-only H.264 decoding.
- If you need to avoid GPL/LGPL licensing issues, use the FFmpeg fork `libav` (less maintained) or use FFmpeg's `--enable-gpl` and comply with the license rather than reimplementing.
- The only valid reason for custom decoding is extreme edge cases (e.g., decoding on hardware that FFmpeg doesn't support, or embedding in a GPL-incompatible product).

**Which phase should address it:** Phase 0 (Technology Decision). Document as a project principle.

**Confidence:** HIGH — Industry consensus. FFmpeg is the de facto standard for software video decoding.

---

### Pitfall 20: Ignoring Corrupted Frames Instead of Signaling Them

**What goes wrong:** The decoder encounters a corrupted frame (transmission error, camera glitch) and silently drops it, causing the next frame to be tagged with the wrong timestamp downstream. The consumer never knows corruption occurred.

**Why it happens:** FFmpeg's decoder typically returns corrupted frames with `got_picture=1` but with pixel artifacts. The caller sees a valid-looking frame and processes it. The corruption is only visible to a human inspector or automated quality check.

**FFmpeg error propagation:**
- `avcodec_decode_video2()` (deprecated): Returns a positive value even on decode errors, with `got_picture` potentially set for partially-decoded frames
- `avcodec_receive_frame()` (current API): Returns `AVERROR(EAGAIN)` for missing data, `AVERROR_EOF` for drain complete, but 0 for frames that may still be corrupted internally

**Prevention:**
- **Check `pkt->flags & AV_PKT_FLAG_CORRUPT`** before passing a packet to the decoder. If set, the packet has known corruption (detected by transport layer).
- **Track per-stream frame decode statistics:** Count failed decodes, skipped P-frames, and concealment events. Expose these as metrics.
- **Signal corruption to Kafka messages:** Include a `corrupted: true` field in the frame metadata when the decoder reports errors. Consumers can then choose to skip or flag these frames.
- **Implement frame hash consistency:** If feasible, compute the frame's SHA-256 at the source and include it in the Kafka message. Consumers can verify integrity after JPEG decode.

**Warning signs:**
- FFmpeg logs: `concealing N DC, N AC, N MV errors in I frame`
- FFmpeg logs: `error while decoding MB N` or `ac-tex damaged at position N`
- Frame hash comparison showing mismatches between source verification and received frames
- Visual inspection revealing green/magenta macroblocks or torn frames

**Which phase should address it:** Phase 1 (Core Decoder) — corruption detection must be built into the decode loop.

**Confidence:** MEDIUM — Corruption handling varies by stream transport; RTSP/UDP has more corruption than RTSP/TCP or HLS.

---

### Pitfall 21: Monitoring for the Wrong Things

**What goes wrong:** Dashboards show "all streams connected" and "CPU at 60%" but every stream is producing duplicate frames because of VFR misconfiguration. Nobody notices until downstream consumers complain about "blurry" output.

**Why it happens:** Standard infrastructure metrics (CPU, memory, disk, network) don't capture frame extraction quality. A system can be perfectly healthy by infrastructure metrics while producing 100% useless output.

**Critical missing metrics:**
- `frames_decoded_per_second` vs `frames_extracted_per_second` (drop ratio)
- `frame_duplicate_count` (from vsync/VFR issues)
- `stream_uptime_seconds` vs `valid_frame_uptime_seconds` (time before first IDR)
- `average_frame_size_bytes` (detects JPEG quality changes or empty frames)
- `streams_with_valid_video` vs `streams_connected` (audio-only detection)
- `decoder_error_count` (corrupted frames per minute)

**Prevention:**
- **Instrument the decoder pipeline, not just the infrastructure.** Every frame that enters the system should be accounted for: received → decoded → evaluated by rules → sent to Kafka.
- **Create a "frame quality" Prometheus metric:** Track `getframe_frame_corrupted_total`, `getframe_frame_duplicate_total`, `getframe_frame_timestamp_anomaly_total`.
- **Set up synthetic end-to-end tests:** Send a known test pattern through an RTSP simulator, extract frames, and verify they match expected output. Run this every 5 minutes.
- **Create a "zero frame output" alert:** If any stream produces 0 frames for > 60 seconds while being in "connected" state, page the on-call engineer.

**Warning signs:**
- Infrastructure dashboard: Everything green. Frame dashboard: "3,000 duplicates detected in last 5 minutes."
- Ops team unaware of extraction quality issues until downstream teams escalate
- $200K/month wasted on compute for corrupted/empty frames pushed to Kafka

**Which phase should address it:** Phase 5 (Observability). Instrumentation must be designed alongside the decoder, not bolted on after.

**Confidence:** HIGH — Universal observability anti-pattern with specific video extraction metrics.

---

## Phase-Specific Warnings

| Phase | Topic | Likely Pitfall | Mitigation |
|-------|-------|---------------|------------|
| **Phase 0/1** | Language choice | GC pressure from high frame allocation rates | Use Rust/C++ or implement frame pooling in GC language |
| **Phase 0/1** | Architecture | Building custom decoder instead of using FFmpeg libs | Commit to FFmpeg libavcodec from day one |
| **Phase 1** | Core Decoder | FFmpeg API misuse causing memory leaks | Audit all alloc/free pairs, use current API (not deprecated) |
| **Phase 1** | Core Decoder | B-frame PTS/DTS ordering confusion | Use `pkt->pts` with proper reordering queue |
| **Phase 1** | Core Decoder | Keyframe alignment / initial decode delay | Discard frames until first IDR decoded |
| **Phase 1** | Stream Connection | Per-process FFmpeg spawning for 200+ streams | Use libavcodec library or worker pool model |
| **Phase 3** | Stability | RTSP reconnection thundering herd | Exponential backoff + staggered startup |
| **Phase 3** | Stability | HLS retry storm from segment download failures | Per-origin token bucket for throttling |
| **Phase 2** | Kafka Integration | Large frame images exceeding 1MB message limit | Claim-check pattern (store frames externally, send metadata) |
| **Phase 2** | Kafka Integration | Producer buffer backpressure crashing decoder | Bounded queue between decoder and producer |
| **Phase 2** | K8s Configuration | CPU throttling of decode containers under CFS quota | Guaranteed QoS, CPU Manager static policy |
| **Phase 4** | Rule Engine | Over-engineering rules before basic pipeline works | Phase ordering: simple time-based → scene detection → composite |
| **Phase 5** | Observability | Monitoring infrastructure but not frame quality metrics | Instrument decoder pipeline: decoded vs extracted vs corrupted |
| **Phase 6** | Load Testing | Testing with clean MP4 files, not RTSP streams | Test harness must use real/simulated camera feeds from day 1 |

---

## Summary: Top 5 Most Dangerous Pitfalls

| Rank | Pitfall | Why It's Dangerous | When It Strikes |
|------|---------|--------------------|-----------------|
| 1 | **Per-process FFmpeg model** (Pitfall 1) | Immediately exhausts memory at scale; requires full rewrite to fix | At 50-100 streams |
| 2 | **Kafka message size + backpressure** (Pitfall 4) | Causes cascading failure: buffer full → decoder stalls → RTSP drops | Under sustained load |
| 3 | **CPU throttling in Kubernetes** (Pitfall 5) | Silent frame loss with zero infrastructure alerts | In production under load |
| 4 | **B-frame PTS/DTS ordering** (Pitfall 2) | All extracted frames have wrong timestamps; corrupts every downstream consumer | From day one |
| 5 | **FFmpeg memory leaks** (Pitfall 3) | Predictable OOM after N hours; hard to detect until production | After 6-72 hours of uptime |

---

## Sources

- **FFmpeg memory leaks:** FFmpeg-devel ML (2007-2023): `[FFmpeg-devel] memory leak in h264 decoder` (2007), `[FFmpeg-devel] Significant memory leak when calling avcodec_decode_video2` (2011), `[Libav-user] Rapid TS fragment ffmpeg decoding memory leak` (2017), OBS Studio FFmpeg plugin PR #7306 (2022), FFmpeg trac #10554 (2023). *HIGH confidence.*
- **FFmpeg thread safety & process model:** GetStream.io FFmpeg in Production benchmarks, MainConcept Kubernetes benchmarks. *HIGH confidence.*
- **B-frame handling & PTS/DTS:** FFmpeg-user ML (2024-2025): `DTSes & PTSes` discussion, `Non-monotonic DTS errors` (2024), FFmpeg source `libavcodec/bsf/dts2pts.c`. *HIGH confidence.*
- **Kafka producer internals:** Apache Kafka source code (BufferPool.java), KIP-782 (expandable batch size), Kafka PR #20358 (KAFKA-8350 fix). *HIGH confidence.*
- **Kubernetes CPU throttling:** MainConcept Kubernetes tuning guide, Kubernetes CPU Manager documentation, video Kubernetes production post-mortem (2026). *HIGH confidence.*
- **RTSP reconnection:** Live555 mailing list (2013-2024), SRS issue #3972, Janus Gateway commit c3be8e7, go2rtc stability improvements. *HIGH confidence.*
- **VFR handling:** FFmpeg-user ML `vfrdet` discussions, Super User VFR export failures (2022). *HIGH confidence.*
- **Frame extraction benchmarks:** DEV.to FFmpeg guide (2026), Sebi.io seeking benchmarks (2024), Codegenes.net optimization guide (2025). *HIGH confidence.*
- **Kubernetes OOM & memory:** Kubernetes OOMKilled deep-dive (2026), ScaleOps swap analysis (2026), sidecar memory accounting regression post-mortem (2026). *HIGH confidence.*
- **RTSP-Kafka production experience:** rtsp-kafka-ingestion-template production post-mortem (2025). *MEDIUM confidence (single source, but detailed).*
- **Distributed media pipeline patterns:** MpegFlow K8s Operator pattern guide (2026), Kafka media inferencing architecture (2025). *MEDIUM confidence.*

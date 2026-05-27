# Phase 1 Summary — Core Pipeline: Single Stream Foundation

## Actual Files Created

```
D:\projects\GetFrame\
├── Cargo.toml                      # Project manifest with all dependencies
├── Dockerfile                      # Multi-stage build (Debian FFmpeg dev pkgs + distroless)
├── config.example.yaml             # Example configuration with comments
├── AGENTS.md                       # Project conventions and workflow
├── .cargo/
│   └── patches/
│       ├── ffmpeg-next/            # Patched ffmpeg-next 8.1.0
│       └── ffmpeg-sys-next/        # Patched ffmpeg-sys-next 8.1.0
└── src/
    ├── main.rs                     # Entry point, orchestration, graceful shutdown
    ├── lib.rs                      # Module declarations
    ├── config.rs                   # YAML config parser (serde + clap CLI)
    ├── types.rs                    # Domain types (DecodedFrame, ExtractedFrame, FrameMetadata)
    ├── logging.rs                  # Tracing subscriber with JSON output
    ├── health.rs                   # Axum /health + /ready endpoints
    ├── pipeline/
    │   ├── mod.rs                  # Pipeline orchestrator (thread spawn, channels)
    │   ├── ingest.rs               # FFmpeg source demuxer (RTSP/RTMP/HLS/file)
    │   ├── decode.rs               # Raw avcodec_send_packet/receive_frame + PTS reorder
    │   ├── rule.rs                 # Fixed-interval rule evaluator
    │   └── encode.rs               # YUV→RGB (yuvutils-rs SIMD) + JPEG encode (image crate)
    ├── storage/
    │   └── mod.rs                  # MinIO/S3 client with retry + deterministic keys
    └── kafka/
        └── mod.rs                  # Kafka producer with retry + JSON metadata
```

## Deviations from Plan

1. **FFmpeg build approach**: Plan assumed `ffmpeg-sys-next` static build or `mwader/static-ffmpeg:8.1`. Actual build uses `FFMPEG_DIR` pointing to a prebuilt FFmpeg master shared library (BtbN Windows build 2026-05-23). Dockerfile adapted to use Debian `libav*-dev` packages instead.

2. **ffmpeg-next API changes**: Several APIs in ffmpeg-next 8.1.0 differ from the plan's assumptions:
   - `Stream::codec()` removed for FFmpeg ≥ 5.0 → use `Stream::parameters()`
   - `Id::NONE` → `Id::None`
   - `Error::Eagain` → `Error::Other { errno: EAGAIN }`
   - Three enum match statements needed wildcard arms (patched ffmpeg-next source)

3. **Dockerfile**: Simplified to use Debian FFmpeg dev packages instead of static FFmpeg image. No cargo-chef caching (for simplicity).

4. **rdkafka-sys**: Required cmake on Windows to build librdkafka from source.

## Build Status

| Check | Status |
|-------|--------|
| `cargo check` (lib) | ✅ Pass (0 errors, 0 warnings) |
| `cargo build` (debug) | ✅ Pass (0 errors, 0 warnings) |
| `cargo build --release` | ⏳ Not tested |
| `cargo test` | ⏳ Not tested (no tests yet) |

## Key Learnings for Phase 2

1. **FFMPEG_DIR trailing space**: Environment variable with trailing space broke `check_features()` C compilation and bindgen. Always trim env var values.

2. **cargo build script cache**: On cargo 1.95 / Windows, modified `build.rs` hash didn't change in some runs. Deleting `target/debug/build/ffmpeg-sys-next-*` and `.fingerprint/ffmpeg-sys-next-*` was needed to force rebuild.

3. **rdkafka API change**: `Delivery` changed from tuple struct (`.0`, `.1`) to named fields (`.partition`, `.offset`) in rdkafka 0.39.

4. **aws-credential-types API change**: `Credentials::from_keys()` removed; use `Credentials::new(ak, sk, None, None, "provider_name")`.

5. **yuvutils-rs**: The crate provides `yuv420_to_rgb()` function with `YuvPlanarImage` struct. SIMD dispatch is automatic.

## Performance Baseline

(To be measured in Phase 2 with real video sources and metrics.)

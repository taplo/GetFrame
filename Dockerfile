# Stage 1: Web UI build
FROM node:20-alpine AS web-builder
WORKDIR /web
COPY web/package.json web/ ./
RUN npm ci && npm run build

# Stage 2: Rust binary (glibc, Debian FFmpeg)
FROM rust:1.91-slim-bookworm AS rust-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libavcodec-dev libavformat-dev libavutil-dev libswscale-dev \
    libavdevice-dev libavfilter-dev libswresample-dev libpostproc-dev \
    cmake make g++ libssl-dev libcurl4-openssl-dev clang libclang-dev curl libzstd-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY src/ src/
COPY migrations/ migrations/
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV LIBCLANG_PATH=/usr/lib/x86_64-linux-gnu
RUN ln -s /usr/include/x86_64-linux-gnu/libswscale /usr/include/libswscale && \
    ln -s /usr/include/x86_64-linux-gnu/libavcodec /usr/include/libavcodec && \
    ln -s /usr/include/x86_64-linux-gnu/libavformat /usr/include/libavformat && \
    ln -s /usr/include/x86_64-linux-gnu/libavutil /usr/include/libavutil && \
    ln -s /usr/include/x86_64-linux-gnu/libavdevice /usr/include/libavdevice && \
    ln -s /usr/include/x86_64-linux-gnu/libavfilter /usr/include/libavfilter && \
    ln -s /usr/include/x86_64-linux-gnu/libpostproc /usr/include/libpostproc && \
    ln -s /usr/include/x86_64-linux-gnu/libswresample /usr/include/libswresample
RUN cargo build --release --bin getframe-worker

# Stage 3: Minimal runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libavcodec59 libavformat59 libavutil57 libswscale6 \
    libavdevice59 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /app/target/release/getframe-worker /getframe-worker
COPY --from=web-builder /web/dist /web/dist
COPY config.example.yaml /etc/getframe/config.yaml
EXPOSE 8080
ENTRYPOINT ["/getframe-worker"]
CMD ["--config", "/etc/getframe/config.yaml"]

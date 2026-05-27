# Stage 1: Web UI build
FROM node:20-alpine AS web-builder
WORKDIR /web
COPY web/package.json web/ ./
RUN npm ci && npm run build

# Stage 2: Rust binary (musl, Alpine FFmpeg)
FROM rust:1.85-alpine3.20 AS rust-builder
RUN apk add --no-cache \
    ffmpeg-dev ffmpeg-libs \
    musl-dev pkgconfig cmake make g++ \
    openssl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY src/ src/
COPY migrations/ migrations/
ENV PKG_CONFIG_ALLOW_CROSS=1
RUN cargo build --release --bin getframe-worker

# Stage 3: Minimal runtime image
FROM alpine:3.20
RUN apk add --no-cache ca-certificates ffmpeg-libs
COPY --from=rust-builder /app/target/release/getframe-worker /getframe-worker
COPY --from=web-builder /web/dist /web/dist
COPY config.example.yaml /etc/getframe/config.yaml
EXPOSE 8080
ENTRYPOINT ["/getframe-worker"]
CMD ["--config", "/etc/getframe/config.yaml"]

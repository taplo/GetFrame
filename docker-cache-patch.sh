#!/bin/bash
set -e
cd /home/taplo/getframe
# Add BuildKit cache mount for cargo registry
sed -i 's|RUN cargo build --release --bin getframe-worker|RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/app/target cargo build --release --bin getframe-worker|g' Dockerfile
echo "Dockerfile patched with cache mounts"
head -50 Dockerfile

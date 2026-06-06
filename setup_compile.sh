#!/bin/sh
docker rm -f getframe-compile 2>/dev/null
docker run -d --name getframe-compile --network host \
  -v /home/taplo/getframe:/app -w /app \
  -e RUSTFLAGS='--cfg tokio_unstable' \
  rust:1.91-slim-bookworm \
  sh -c '
    sed -i "s|deb.debian.org|mirrors.tuna.tsinghua.edu.cn|g" /etc/apt/sources.list.d/debian.sources &&
    apt-get update -qq &&
    apt-get install -y -qq --no-install-recommends \
      nasm pkg-config libavcodec-dev libavformat-dev libavutil-dev \
      libswscale-dev libavdevice-dev libavfilter-dev cmake make g++ \
      libssl-dev libcurl4-openssl-dev clang libclang-dev &&
    tail -f /dev/null
  '

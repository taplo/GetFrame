#!/bin/bash
cat > /tmp/compile-entry.sh << 'INNER'
#!/bin/sh
sed -i 's|http://deb.debian.org/debian|http://mirrors.tuna.tsinghua.edu.cn/debian|g' /etc/apt/sources.list.d/debian.sources
apt-get update -qq
apt-get install -y -qq --no-install-recommends nasm pkg-config libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavdevice-dev libavfilter-dev cmake make g++ libssl-dev libcurl4-openssl-dev clang libclang-dev
echo "BUILD_DEPS_DONE"
INNER
chmod +x /tmp/compile-entry.sh
docker rm -f getframe-compile 2>/dev/null
docker run -d --name getframe-compile --network host \
  -v /home/taplo/getframe:/app -w /app \
  -e RUSTFLAGS='--cfg tokio_unstable' \
  rust:1.91-slim-bookworm \
  /tmp/compile-entry.sh tail -f /dev/null
echo "Container started"

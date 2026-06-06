#!/bin/sh
sed -i 's|http://deb.debian.org/debian|http://mirrors.tuna.tsinghua.edu.cn/debian|g' /etc/apt/sources.list.d/debian.sources
apt-get update -qq
apt-get install -y -qq --no-install-recommends nasm pkg-config libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavdevice-dev libavfilter-dev cmake make g++ libssl-dev libcurl4-openssl-dev clang libclang-dev
echo "BUILD_DEPS_DONE"
tail -f /dev/null

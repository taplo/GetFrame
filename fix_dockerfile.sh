#!/bin/bash
set -e
cd /home/taplo/getframe
cp Dockerfile Dockerfile.bak
# Patch both apt-get RUN commands to use Tsinghua mirror
sed -i 's@RUN apt-get update && apt-get install -y --no-install-recommends@RUN sed -i "s|http://deb.debian.org/debian|http://mirrors.tuna.tsinghua.edu.cn/debian|g" /etc/apt/sources.list.d/debian.sources \&\& apt-get update \&\& apt-get install -y --no-install-recommends@g' Dockerfile
echo "=== Dockerfile patched (first 40 lines) ==="
head -30 Dockerfile

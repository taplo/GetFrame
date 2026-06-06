#!/bin/bash
set -e
ssh taplo@192.168.3.122 'bash -s' << 'ENDSSH'
  # Kill old builds
  ps aux | grep "docker compose" | grep -v grep | awk '{print $2}' | xargs kill 2>/dev/null
  sleep 2
  
  # Clear build cache for clean build
  docker buildx prune -f 2>&1
  
  cd /home/taplo/getframe
  
  # Ensure SWAGGER_UI_DOWNLOAD is set properly
  grep -q "SWAGGER_UI_DOWNLOAD" Dockerfile && echo "ENV present" || echo "ENV missing"
  
  # Build
  docker compose build --progress=plain \
    --build-arg http_proxy=http://192.168.3.208:8787 \
    --build-arg https_proxy=http://192.168.3.208:8787 \
    worker 2>&1
ENDSSH

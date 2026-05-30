# Benchmark: Stream Processing Capacity

## Prerequisites

- Docker Compose v2
- Python 3.14+
- Proxy: `192.168.3.200:8787` (for Docker Hub pulls via apt docker.io)

## Quick Start

```bash
# 1. Build the worker Docker image (on another machine with Rust toolchain)
docker build -t getframe-worker:latest .

# 2. Sync benchmark files to server
scp -P 422 -r benchmark/ taplo@server:~/getframe/

# 3. Run benchmark
cd ~/getframe/benchmark
python3 run.py
```

## Output

Results saved to `results/benchmark-{fps}fps.csv`:

```csv
streams,target_fps,actual_fps,cpu_percent,mem_mb,errors,cpu_per_stream
1,5,4.98,6.2,45,0,6.2
```

## Architecture

See [design doc](../docs/superpowers/specs/2026-05-30-benchmark-design.md).

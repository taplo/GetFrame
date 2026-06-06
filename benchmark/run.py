#!/usr/bin/env python3
"""Benchmark orchestrator for getframe-worker stream processing capacity."""

import json
import subprocess
import time
import csv
import os
import sys
import urllib.request
import urllib.error
import argparse
from pathlib import Path

COMPOSE_DIR = Path(__file__).parent
COMPOSE_FILE = COMPOSE_DIR / "compose.yaml"
CONFIG_DIR = COMPOSE_DIR / "config"
RESULTS_DIR = COMPOSE_DIR / "results"

WORKER_API = "http://localhost:8080"
WORKER_METRICS = f"{WORKER_API}/metrics"
MEDIAMTX_HOST = "mediamtx"
MEDIAMTX_PORT = "8554"

STREAM_COUNTS = [1, 2, 4, 8, 12, 16, 24, 32]
SCALE_STREAM_COUNTS = [32, 48, 64, 96, 128, 200]
SCALE_THRESHOLD = 48
TARGET_FPS_VALUES = [1]
STABILIZE_SEC = 30
SAMPLE_COUNT = 6
SAMPLE_INTERVAL = 5

FFMPEG_IMAGE = "linuxserver/ffmpeg:latest"


def sh(cmd: str, **kwargs) -> str:
    """Run shell command, return stdout."""
    result = subprocess.run(
        cmd, shell=True, capture_output=True, text=True, **kwargs
    )
    if result.returncode != 0:
        print(f"  WARN: '{cmd}' exited {result.returncode}: {result.stderr.strip()}")
    return result.stdout.strip()


def wait_for_worker(timeout: int = 60) -> bool:
    """Wait until worker /api/health responds."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = urllib.request.urlopen(f"{WORKER_API}/health", timeout=5)
            if resp.status == 200:
                return True
        except (urllib.error.URLError, ConnectionError):
            pass
        time.sleep(2)
    return False


def get_docker_stats(container: str = "getframe-bench-worker") -> dict:
    """Return CPU% and memory MB for a container."""
    out = sh(
        "docker stats --no-stream --format "
        '"{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}" '
        f"{container}"
    )
    if not out:
        return {"cpu": 0.0, "mem_mb": 0.0}
    parts = out.split("\t")
    cpu_str = parts[1].replace("%", "") if len(parts) > 1 else "0"
    mem_str = parts[2].split("/")[0].strip() if len(parts) > 2 else "0M"
    mem_str = mem_str.strip()
    if mem_str.endswith("GiB"):
        mem_val = float(mem_str.replace("GiB", "")) * 1024
    elif mem_str.endswith("MiB"):
        mem_val = float(mem_str.replace("MiB", ""))
    elif mem_str.endswith("MB"):
        mem_val = float(mem_str.replace("MB", ""))
    else:
        mem_val = 0.0
    return {"cpu": float(cpu_str), "mem_mb": mem_val}


def start_ffmpeg(stream_id: int) -> None:
    """Start a single ffmpeg container pushing synthetic RTSP."""
    name = f"getframe-bench-ffmpeg-{stream_id}"
    stream_url = f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/stream-{stream_id}"
    sh(f"docker rm -f {name} 2>/dev/null")
    sh(
        f"docker run -d --name {name} --network benchmark_bench-net "
        f"--restart no --entrypoint ffmpeg {FFMPEG_IMAGE} "
        f"-re -f lavfi -i testsrc2=size=1920x1080:rate={TARGET_FPS} "
        f"-c:v libx264 -preset ultrafast -tune zerolatency -g 1 "
        f"-f rtsp {stream_url}"
    )


def stop_ffmpeg(stream_num: int) -> None:
    """Stop and remove a single ffmpeg container."""
    name = f"getframe-bench-ffmpeg-{stream_num}"
    sh(f"docker rm -f {name} 2>/dev/null")


def start_scale_source() -> None:
    """Start a single ffmpeg feeding the shared RTSP source for relay mode."""
    name = "getframe-bench-ffmpeg-scale"
    stream_url = f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/bench-source"
    sh(f"docker rm -f {name} 2>/dev/null")
    sh(
        f"docker run -d --name {name} --network benchmark_bench-net "
        f"--restart no --entrypoint ffmpeg {FFMPEG_IMAGE} "
        f"-re -f lavfi -i testsrc2=size=1920x1080:rate={TARGET_FPS} "
        f"-c:v libx264 -preset ultrafast -tune zerolatency -g 1 "
        f"-f rtsp {stream_url}"
    )
    time.sleep(3)  # wait for ffmpeg to connect and publish


def stop_scale_source() -> None:
    """Stop the shared scale test RTSP source."""
    sh("docker rm -f getframe-bench-ffmpeg-scale 2>/dev/null")


def register_stream(stream_id: int, source_url: str | None = None) -> str | None:
    """Register RTSP stream with worker via API. Returns stream ID on success."""
    url = f"{WORKER_API}/api/v1/streams"
    interval = 1.0 / TARGET_FPS
    if source_url is None:
        source_url = f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/stream-{stream_id}"
    payload = {
        "config": {
            "source_url": source_url,
            "source_type": "rtsp",
            "extract_interval_seconds": interval,
            "rtsp_transport": "tcp",
        }
    }
    req = urllib.request.Request(
        url, data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        if resp.status in (200, 201):
            body = json.loads(resp.read())
            return body.get("id")
        print(f"[{resp.status}]", end=" ", flush=True)
        return None
    except urllib.error.URLError as e:
        print(f"[ERR]", end=" ", flush=True)
        return None


def get_total_frames() -> int:
    """Get total frames_decoded from all streams via API."""
    try:
        data = json.loads(urllib.request.urlopen(
            f"{WORKER_API}/api/v1/streams", timeout=5).read())
        return sum(s.get("frames_decoded", 0) for s in data.get("streams", []))
    except Exception:
        return 0

def get_total_errors() -> int:
    """Get total error_count from all streams via API."""
    try:
        data = json.loads(urllib.request.urlopen(
            f"{WORKER_API}/api/v1/streams", timeout=5).read())
        return sum(s.get("error_count", 0) for s in data.get("streams", []))
    except Exception:
        return 0

def collect_metrics(target_fps: int, num_streams: int) -> list:
    """Collect SAMPLE_COUNT samples of metrics."""
    samples = []
    last_total = get_total_frames()
    for i in range(SAMPLE_COUNT):
        time.sleep(SAMPLE_INTERVAL)
        current_total = get_total_frames()
        errors = get_total_errors()
        actual_fps = (current_total - last_total) / SAMPLE_INTERVAL
        stats = get_docker_stats()
        samples.append({
            "streams": num_streams,
            "target_fps": target_fps,
            "actual_fps": round(actual_fps, 2),
            "cpu_percent": round(stats["cpu"], 2),
            "mem_mb": round(stats["mem_mb"], 1),
            "errors": int(errors),
            "cpu_per_stream": round(stats["cpu"] / num_streams, 2) if num_streams > 0 else 0,
        })
        last_total = current_total
        sys.stdout.write(".")
        sys.stdout.flush()
    sys.stdout.write("\n")
    return samples


CSV_FIELDS = [
    "streams", "target_fps", "actual_fps", "cpu_percent",
    "mem_mb", "errors", "cpu_per_stream"
]


def log(msg: str, end="\n"):
    ts = time.strftime("%H:%M:%S")
    print(f"[{ts}] {msg}", end=end, flush=True)


def write_csv(csv_path: str, samples: list, append: bool = False):
    mode = "a" if append else "w"
    with open(csv_path, mode, newline="") as f:
        w = csv.DictWriter(f, fieldnames=CSV_FIELDS)
        if not append:
            w.writeheader()
        w.writerows(samples)


def run_benchmark(scale: bool = False):
    """Main benchmark orchestrator."""
    os.makedirs(RESULTS_DIR, exist_ok=True)

    log("Cleaning up previous benchmark containers...")
    sh("docker rm -f $(docker ps -aq --filter name=getframe-bench- 2>/dev/null) 2>/dev/null; true")

    stream_counts = SCALE_STREAM_COUNTS if scale else STREAM_COUNTS
    label = "scale" if scale else "baseline"

    for target_fps in TARGET_FPS_VALUES:
        global TARGET_FPS
        TARGET_FPS = target_fps

        csv_path = RESULTS_DIR / f"benchmark-{target_fps}fps-{label}.csv"

        log(f"\n{'='*60}")
        log(f"Benchmark ({label}): target {target_fps}fps")
        log(f"{'='*60}")

        log("Starting mediamtx, minio, and worker...")
        sh(f"docker compose -f {COMPOSE_FILE} up -d --pull missing")
        if not wait_for_worker():
            log("ERROR: worker did not become ready")
            sys.exit(1)
        log("ready")

        for num_streams in stream_counts:
            log(f"\u2192 {num_streams} streams...", end=" ")

            # In scale (relay) mode, start one shared ffmpeg source
            use_relay = scale and num_streams >= SCALE_THRESHOLD
            if use_relay:
                log(f"[relay mode]", end=" ")
                start_scale_source()
            else:
                for sid in range(1, num_streams + 1):
                    start_ffmpeg(sid)

            log("waiting 5s for ffmpeg...", end=" ")
            time.sleep(5)

            # Register streams with worker
            registered_ids = []
            for sid in range(1, num_streams + 1):
                source_url = (
                    f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/bench-source"
                    if use_relay else None
                )
                wid = register_stream(sid, source_url=source_url)
                if wid:
                    registered_ids.append(wid)
            log(f"registered {len(registered_ids)}")

            # Wait for first frame to arrive, or timeout
            waited = 0
            max_wait = 300 if target_fps <= 1 else min(300, max(STABILIZE_SEC, num_streams * 10))
            log(f"waiting for frames (up to {max_wait}s)...", end=" ")
            while waited < max_wait:
                frames = get_total_frames()
                if frames > 0:
                    log(f"first frame at {waited}s", end=" ")
                    break
                time.sleep(5)
                waited += 5
            else:
                log(f"no frames after {max_wait}s", end=" ")
            time.sleep(STABILIZE_SEC)

            # Collect metrics
            log("sampling", end=" ")
            samples = collect_metrics(target_fps, num_streams)

            # Write incremental results
            write_csv(csv_path, samples, append=os.path.exists(csv_path))
            log(f" wrote {len(samples)} samples to {csv_path}")

            # Cleanup
            if use_relay:
                stop_scale_source()
            else:
                for sid in range(1, num_streams + 1):
                    stop_ffmpeg(sid)
            sh(f"docker compose -f {COMPOSE_FILE} down -v")

            if num_streams != stream_counts[-1]:
                sh(f"docker compose -f {COMPOSE_FILE} up -d --pull missing")
                if not wait_for_worker():
                    log("ERROR: worker failed to restart")
                    sys.exit(1)
                log("[clean]", end=" ")

        log(f"Results saved to {csv_path}")

    log("Cleaning up...")
    sh(f"docker compose -f {COMPOSE_FILE} down -v")
    log(f"Done. Results in {RESULTS_DIR}/")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="getframe-worker stream processing benchmark")
    parser.add_argument("--scale", action="store_true",
                        help="Run large-scale relay benchmark (32-200 streams)")
    args = parser.parse_args()

    print("Benchmark: getframe-worker stream processing capacity")
    print(f"Python: {sys.version}")
    mode = "SCALE (200+ relay)" if args.scale else "baseline (1-32 streams)"
    print(f"Mode: {mode}")
    run_benchmark(scale=args.scale)

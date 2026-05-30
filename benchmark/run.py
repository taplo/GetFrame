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
TARGET_FPS_VALUES = [5, 1]
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
            resp = urllib.request.urlopen(f"{WORKER_API}/api/health", timeout=5)
            if resp.status == 200:
                return True
        except (urllib.error.URLError, ConnectionError):
            pass
        time.sleep(2)
    return False


def get_metric_value(text: str, name: str) -> float:
    """Extract a Prometheus counter or gauge value from /metrics text."""
    for line in text.splitlines():
        if line.startswith(name) and not line.startswith("#"):
            parts = line.split()
            if len(parts) >= 2:
                try:
                    return float(parts[-1])
                except ValueError:
                    pass
    return 0.0


def get_docker_stats() -> dict:
    """Return CPU% and memory MB for worker container."""
    out = sh(
        "docker stats --no-stream --format "
        '"{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}" '
        "getframe-bench-worker"
    )
    if not out:
        return {"cpu": 0.0, "mem_mb": 0.0}
    parts = out.split("\t")
    cpu_str = parts[1].replace("%", "") if len(parts) > 1 else "0"
    mem_str = parts[2].split("/")[0].strip() if len(parts) > 2 else "0M"
    mem_val = float(mem_str.replace("MiB", "").replace("MB", "").strip())
    return {"cpu": float(cpu_str), "mem_mb": mem_val}


def start_ffmpeg(stream_id: int) -> None:
    """Start a single ffmpeg container pushing synthetic RTSP."""
    name = f"getframe-bench-ffmpeg-{stream_id}"
    stream_url = f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/stream-{stream_id}"
    sh(
        f"docker run -d --name {name} --network getframe-bench_bench-net "
        f"--restart no {FFMPEG_IMAGE} "
        f"ffmpeg -re -f lavfi -i testsrc2=size=1920x1080:rate={TARGET_FPS} "
        f"-c:v libx264 -preset ultrafast -tune zerolatency "
        f"-f rtsp {stream_url}"
    )


def stop_ffmpeg(stream_id: int) -> None:
    """Stop and remove a single ffmpeg container."""
    name = f"getframe-bench-ffmpeg-{stream_id}"
    sh(f"docker rm -f {name} 2>/dev/null")


def register_stream(stream_id: int) -> bool:
    """Register RTSP stream with worker via API."""
    url = f"{WORKER_API}/api/v1/streams"
    body = json.dumps({
        "source_url": f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/stream-{stream_id}",
        "source_type": "rtsp",
        "fps": TARGET_FPS,
        "status": "active",
    }).encode()
    req = urllib.request.Request(
        url, data=body,
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        return resp.status in (200, 201)
    except urllib.error.URLError:
        return False


def unregister_stream(stream_id: int) -> None:
    """Remove stream from worker via API."""
    try:
        req = urllib.request.Request(
            f"{WORKER_API}/api/v1/streams/{stream_id}",
            method="DELETE"
        )
        urllib.request.urlopen(req, timeout=5)
    except urllib.error.URLError:
        pass


def collect_metrics(target_fps: int, num_streams: int) -> list:
    """Collect SAMPLE_COUNT samples of metrics."""
    samples = []
    last_total = get_metric_value(
        urllib.request.urlopen(WORKER_METRICS).read().decode(),
        "getframe_frames_processed_total"
    )
    for i in range(SAMPLE_COUNT):
        time.sleep(SAMPLE_INTERVAL)
        text = urllib.request.urlopen(WORKER_METRICS).read().decode()
        current_total = get_metric_value(text, "getframe_frames_processed_total")
        errors = get_metric_value(text, "getframe_decode_errors_total")
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


def run_benchmark():
    """Main benchmark orchestrator."""
    os.makedirs(RESULTS_DIR, exist_ok=True)

    # Pull images
    print("Pulling Docker images...")
    sh(f"docker pull {FFMPEG_IMAGE}")
    sh("docker pull bluenviron/mediamtx:latest")

    for target_fps in TARGET_FPS_VALUES:
        global TARGET_FPS
        TARGET_FPS = target_fps

        csv_path = RESULTS_DIR / f"benchmark-{target_fps}fps.csv"
        all_samples = []

        print(f"\n{'='*60}")
        print(f"Benchmark: target {target_fps}fps")
        print(f"{'='*60}")

        for num_streams in STREAM_COUNTS:
            print(f"\n\u2192 {num_streams} streams...", end=" ", flush=True)

            # Start ffmpeg containers
            for sid in range(1, num_streams + 1):
                start_ffmpeg(sid)

            # Wait for ffmpeg to connect
            time.sleep(5)

            # Register streams with worker
            for sid in range(1, num_streams + 1):
                register_stream(sid)

            # Wait for stabilization
            print(f"stabilizing {STABILIZE_SEC}s...", end=" ", flush=True)
            time.sleep(STABILIZE_SEC)

            # Collect metrics
            print("sampling", end=" ", flush=True)
            samples = collect_metrics(target_fps, num_streams)

            # Clean up
            for sid in range(1, num_streams + 1):
                unregister_stream(sid)
            for sid in range(1, num_streams + 1):
                stop_ffmpeg(sid)

            all_samples.extend(samples)

        # Write CSV
        with open(csv_path, "w", newline="") as f:
            writer = csv.DictWriter(f, fieldnames=[
                "streams", "target_fps", "actual_fps", "cpu_percent",
                "mem_mb", "errors", "cpu_per_stream"
            ])
            writer.writeheader()
            writer.writerows(all_samples)

        print(f"\nResults saved to {csv_path}")

    # Summary
    print_summary()


def print_summary():
    """Print summary table from all CSV files."""
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    for target_fps in TARGET_FPS_VALUES:
        csv_path = RESULTS_DIR / f"benchmark-{target_fps}fps.csv"
        if not csv_path.exists():
            continue
        with open(csv_path) as f:
            reader = list(csv.DictReader(f))
        if not reader:
            continue
        max_row = max(reader, key=lambda r: int(r["streams"]) if float(r["actual_fps"]) / float(r["target_fps"]) >= 0.9 else 0)
        max_streams = int(max_row["streams"])
        num_positive = len([r for r in reader if int(r["streams"]) > 0])
        total_cpu_per_stream = sum(float(r["cpu_per_stream"]) for r in reader if int(r["streams"]) > 0)
        avg_cpu_per_stream = total_cpu_per_stream / max(1, num_positive)
        print(f"\n  Target {target_fps}fps:")
        print(f"    Max streams before saturation: {max_streams}")
        print(f"    Avg CPU per stream: {avg_cpu_per_stream:.1f}%")
        print(f"    Streams per core (est): {max_streams / 16:.1f}")


if __name__ == "__main__":
    print("Benchmark: getframe-worker stream processing capacity")
    print(f"Python: {sys.version}")
    run_benchmark()

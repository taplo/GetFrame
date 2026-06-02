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
    sh(f"docker rm -f {name} 2>/dev/null")  # remove any leftover
    sh(
        f"docker run -d --name {name} --network benchmark_bench-net "
        f"--restart no --entrypoint ffmpeg {FFMPEG_IMAGE} "
        f"-re -f lavfi -i testsrc2=size=1920x1080:rate={TARGET_FPS} "
        f"-c:v libx264 -preset ultrafast -tune zerolatency "
        f"-f rtsp {stream_url}"
    )


def stop_ffmpeg(stream_num: int) -> None:
    """Stop and remove a single ffmpeg container."""
    name = f"getframe-bench-ffmpeg-{stream_num}"
    sh(f"docker rm -f {name} 2>/dev/null")


def register_stream(stream_id: int) -> str | None:
    """Register RTSP stream with worker via API. Returns stream ID on success."""
    url = f"{WORKER_API}/api/v1/streams"
    interval = 1.0 / TARGET_FPS
    payload = {
        "config": {
            "source_url": f"rtsp://{MEDIAMTX_HOST}:{MEDIAMTX_PORT}/stream-{stream_id}",
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


def unregister_stream(stream_id: str) -> None:
    """Remove stream from worker via API."""
    try:
        req = urllib.request.Request(
            f"{WORKER_API}/api/v1/streams/{stream_id}",
            method="DELETE"
        )
        urllib.request.urlopen(req, timeout=10)
    except Exception:
        pass


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


def run_benchmark():
    """Main benchmark orchestrator."""
    os.makedirs(RESULTS_DIR, exist_ok=True)

    log("Cleaning up previous benchmark containers...")
    sh("docker rm -f $(docker ps -aq --filter name=getframe-bench- 2>/dev/null) 2>/dev/null; true")

    #log("Pulling Docker images...")
    #sh(f"docker pull {FFMPEG_IMAGE}")
    #sh("docker pull bluenviron/mediamtx:latest")

    for target_fps in TARGET_FPS_VALUES:
        global TARGET_FPS
        TARGET_FPS = target_fps

        csv_path = RESULTS_DIR / f"benchmark-{target_fps}fps.csv"

        log(f"\n{'='*60}")
        log(f"Benchmark: target {target_fps}fps")
        log(f"{'='*60}")

        log("Starting mediamtx, minio, and worker...")
        sh(f"docker compose -f {COMPOSE_FILE} up -d")
        if not wait_for_worker():
            log("ERROR: worker did not become ready")
            sys.exit(1)
        log("ready")

        for num_streams in STREAM_COUNTS:
            log(f"\u2192 {num_streams} streams...", end=" ")

            # Start ffmpeg containers
            for sid in range(1, num_streams + 1):
                start_ffmpeg(sid)

            log("waiting 5s for ffmpeg...", end=" ")
            time.sleep(5)

            # Register streams with worker
            registered_ids = []
            for sid in range(1, num_streams + 1):
                wid = register_stream(sid)
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
            # Extra stabilization after first frame
            time.sleep(STABILIZE_SEC)

            # Collect metrics
            log("sampling", end=" ")
            samples = collect_metrics(target_fps, num_streams)

            # Write incremental results so partial data survives
            write_csv(csv_path, samples, append=os.path.exists(csv_path))
            log(f" wrote {len(samples)} samples to {csv_path}")

            # Stop ffmpeg, restart compose for a clean worker next round
            for sid in range(1, num_streams + 1):
                stop_ffmpeg(sid)
            sh(f"docker compose -f {COMPOSE_FILE} down")

            # Restart compose for next iteration
            if num_streams != STREAM_COUNTS[-1]:
                sh(f"docker compose -f {COMPOSE_FILE} up -d")
                if not wait_for_worker():
                    log("ERROR: worker failed to restart")
                    sys.exit(1)
                log("[clean]", end=" ")

        log(f"Results saved to {csv_path}")

    log("Cleaning up...")
    sh(f"docker compose -f {COMPOSE_FILE} down")
    log(f"Done. Results in {RESULTS_DIR}/")


if __name__ == "__main__":
    print("Benchmark: getframe-worker stream processing capacity")
    print(f"Python: {sys.version}")
    run_benchmark()

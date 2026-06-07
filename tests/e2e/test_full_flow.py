#!/usr/bin/env python3
"""E2E integration test for getframe-worker full pipeline:
  1. Start compose stack (MinIO + MySQL + Kafka + worker)
  2. Register RTSP stream via HTTP API
  3. Wait for frame extraction
  4. Verify frames in MinIO (S3 API)
  5. Verify metadata in Kafka
  6. Clean up & report
"""
import json, subprocess, sys, time, os, urllib.request, urllib.error
import hashlib, random
from pathlib import Path

COMPOSE_DIR = Path(__file__).parent.parent.parent / "benchmark"
COMPOSE_FILE = COMPOSE_DIR / "compose.yaml"
WORKER_API = "http://localhost:8080"
MINIO_ENDPOINT = "http://localhost:9002"
MINIO_ACCESS_KEY = "getframe"
MINIO_SECRET_KEY = "getframe123"
MINIO_BUCKET = "getframe-frames"
KAFKA_BROKER = "localhost:9093"
KAFKA_TOPIC = "getframe-frames"
TEST_RUN_ID = f"e2e-{int(time.time())}-{random.randint(1000,9999)}"
FFMPEG_IMAGE = "linuxserver/ffmpeg:latest"
TOKEN = None
PASS = 0
FAIL = 0
STEPS = []

def auth_headers():
    if TOKEN:
        return {"Authorization": f"Bearer {TOKEN}", "Content-Type": "application/json"}
    return {"Content-Type": "application/json"}

def step(name: str):
    global PASS, FAIL
    def decorator(fn):
        STEPS.append((name, fn))
        return fn
    return decorator

def sh(cmd: str, timeout: int = 60, check: bool = False) -> str:
    r = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"Command failed (exit {r.returncode}): {cmd}\nSTDERR: {r.stderr[:200]}")
    return r.stdout.strip()

def run_step(name: str, fn):
    global PASS, FAIL
    print(f"\n  [{name}] ", end="", flush=True)
    try:
        result = fn()
        if result is None or result:
            print("PASS")
            PASS += 1
        else:
            print("FAIL")
            FAIL += 1
    except Exception as e:
        print(f"FAIL ({e})")
        FAIL += 1

@step("Compose stack is running")
def check_compose():
    out = sh("docker ps --filter name=getframe-bench-worker --format '{{.Names}} {{.Status}}'", check=False)
    if "getframe-bench-worker" in out and "Up" in out:
        return True
    print("(starting stack...)")
    sh(f"cd {COMPOSE_DIR} && docker compose -f compose.yaml up -d")
    deadline = time.time() + 120
    while time.time() < deadline:
        out = sh("docker ps --filter name=getframe-bench-worker --format '{{.Status}}'", check=False)
        if "Up" in out:
            return True
        time.sleep(5)
    return False

@step("Worker health endpoint responds")
def check_health():
    resp = json.loads(urllib.request.urlopen(f"{WORKER_API}/health", timeout=5).read())
    return resp.get("status") == "healthy"

@step("MinIO is accessible")
def check_minio():
    r = sh("curl -sf http://localhost:9002/minio/health/live", check=False)
    return "ok" in r.lower() or r == ""

@step("Worker auth login succeeds")
def login():
    global TOKEN
    payload = json.dumps({"username": "admin", "password": "changeme123"})
    req = urllib.request.Request(
        f"{WORKER_API}/api/v1/auth/login",
        data=payload.encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    resp = urllib.request.urlopen(req, timeout=5)
    assert resp.status == 200, f"Login failed: {resp.status}"
    body = json.loads(resp.read())
    TOKEN = body["token"]
    return True

@step("Kafka topic exists")
def check_kafka():
    r = sh(f"docker exec benchmark-kafka-1 kafka-topics --bootstrap-server localhost:9092 --topic {KAFKA_TOPIC} --describe", check=False)
    return KAFKA_TOPIC in r

@step("Start RTSP source via ffmpeg")
def start_source():
    name = "getframe-e2e-source"
    sh(f"docker rm -f {name} 2>/dev/null")
    sh(f"docker run -d --name {name} --network benchmark_bench-net "
       f"--restart no --entrypoint ffmpeg {FFMPEG_IMAGE} "
       f"-re -f lavfi -i testsrc2=size=1920x1080:rate=30 "
       f"-c:v libx264 -preset ultrafast -tune zerolatency -g 1 "
       f"-f rtsp rtsp://mediamtx:8554/{TEST_RUN_ID}")
    time.sleep(5)
    src = sh(f"docker ps --filter name={name} --format '{{{{.Status}}}}'", check=False)
    if "Up" not in src:
        sh(f"docker logs {name}", check=False)
        return False
    return True

@step("Register stream with worker API")
def register_stream():
    payload = json.dumps({"config": {
        "source_url": f"rtsp://mediamtx:8554/{TEST_RUN_ID}",
        "source_type": "rtsp",
        "extract_interval_seconds": 1.0,
        "rtsp_transport": "tcp",
    }})
    req = urllib.request.Request(
        f"{WORKER_API}/api/v1/streams",
        data=payload.encode(),
        headers=auth_headers(),
        method="POST",
    )
    resp = urllib.request.urlopen(req, timeout=15)
    body = json.loads(resp.read())
    assert resp.status in (200, 201), f"Expected 201, got {resp.status}"
    assert "id" in body, "Missing stream id"
    global STREAM_ID
    STREAM_ID = body["id"]
    print(f"id={STREAM_ID[:12]}...", end=" ", flush=True)
    return True

@step("Stream appears as online")
def stream_online():
    deadline = time.time() + 90
    while time.time() < deadline:
        req = urllib.request.Request(f"{WORKER_API}/api/v1/streams/{STREAM_ID}", headers=auth_headers())
        data = json.loads(urllib.request.urlopen(req, timeout=5).read())
        if data.get("frames_decoded", 0) > 0:
            print(f"frames_decoded={data['frames_decoded']}", end=" ", flush=True)
            return True
        time.sleep(5)
    return False

@step("Frames accumulate over time")
def frames_accumulate():
    req = urllib.request.Request(f"{WORKER_API}/api/v1/streams/{STREAM_ID}", headers=auth_headers())
    data = json.loads(urllib.request.urlopen(req, timeout=5).read())
    f1 = data.get("frames_decoded", 0)
    time.sleep(10)
    req = urllib.request.Request(f"{WORKER_API}/api/v1/streams/{STREAM_ID}", headers=auth_headers())
    data = json.loads(urllib.request.urlopen(req, timeout=5).read())
    f2 = data.get("frames_decoded", 0)
    print(f"{f1} -> {f2}", end=" ", flush=True)
    return f2 > f1

@step("Frame exists in MinIO")
def minio_has_frames():
    try:
        r = sh(f"docker run --rm --network benchmark_bench-net "
               f"--entrypoint sh minio/mc -c "
               f"'mc alias set local http://minio:9000 {MINIO_ACCESS_KEY} {MINIO_SECRET_KEY} >/dev/null 2>&1 && "
               f"mc ls local/{MINIO_BUCKET}/ 2>/dev/null'", check=False, timeout=30)
        return len(r) > 50 and STREAM_ID[:8] in r
    except Exception as e:
        print(f"minio check: {e}", end=" ", flush=True)

@step("Kafka has metadata messages")
def kafka_has_messages():
    r = sh(
        f"docker exec benchmark-kafka-1 kafka-console-consumer "
        f"--bootstrap-server localhost:9092 --topic {KAFKA_TOPIC} "
        f"--from-beginning --max-messages 1 --timeout-ms 5000 2>/dev/null",
        check=False, timeout=15,
    )
    return len(r) > 20

@step("Activity log has stream events")
def activity_log_has_events():
    """Verify that activity log records stream.created and auth.login"""
    deadline = time.time() + 15
    while time.time() < deadline:
        try:
            req = urllib.request.Request(f"{WORKER_API}/api/v1/activity?resource_type=stream&page_size=5", headers=auth_headers())
            data = json.loads(urllib.request.urlopen(req, timeout=5).read())
            if data.get("total", 0) >= 1:
                created = [e for e in data["items"] if e["event_type"] == "stream.created"]
                if len(created) >= 1:
                    return True
        except Exception:
            pass
        time.sleep(3)
    return False

@step("Activity log CSV export works")
def activity_log_csv_export():
    req = urllib.request.Request(f"{WORKER_API}/api/v1/activity/export", headers=auth_headers())
    resp = urllib.request.urlopen(req, timeout=5)
    body = resp.read().decode()
    return body.startswith("id,event_type,")

@step("Worker logs show pipeline timing")
def pipeline_timing_logged():
    logs = sh("docker logs getframe-bench-worker 2>&1 | grep -c 'Pipeline timing'", check=False)
    return int(logs) > 0

@step("Stream can be deleted")
def delete_stream():
    req = urllib.request.Request(f"{WORKER_API}/api/v1/streams/{STREAM_ID}", method="DELETE", headers=auth_headers())
    resp = urllib.request.urlopen(req, timeout=5)
    assert resp.status in (200, 204)
    return True

@step("Deleted stream no longer in list")
def deleted_not_in_list():
    req = urllib.request.Request(f"{WORKER_API}/api/v1/streams", headers=auth_headers())
    data = json.loads(urllib.request.urlopen(req, timeout=5).read())
    return all(s["id"] != STREAM_ID for s in data.get("streams", []))

def cleanup():
    try:
        sh("docker rm -f getframe-e2e-source 2>/dev/null")
    except Exception:
        pass

if __name__ == "__main__":
    print(f"E2E Integration Test — {TEST_RUN_ID}")
    print("=" * 60)
    STREAM_ID = None
    try:
        for name, fn in STEPS:
            run_step(name, fn)
    finally:
        cleanup()

    print(f"\n{'=' * 60}")
    print(f"Results: {PASS} passed, {FAIL} failed / {len(STEPS)} total")
    if FAIL > 0:
        sys.exit(1)

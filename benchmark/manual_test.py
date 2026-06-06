import json, urllib.request, time, subprocess

# Start ffmpeg
subprocess.run(["docker", "run", "-d", "--name", "mt-ffmpeg-1", "--network", "benchmark_bench-net",
    "--entrypoint", "ffmpeg", "linuxserver/ffmpeg:latest",
    "-re", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=1",
    "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency",
    "-f", "rtsp", "rtsp://mediamtx:8554/mt-stream-1"],
    capture_output=True)
time.sleep(5)

# Register stream
payload = json.dumps({"config": {"source_url": "rtsp://mediamtx:8554/mt-stream-1",
    "source_type": "rtsp", "extract_interval_seconds": 1.0, "rtsp_transport": "tcp"}}).encode()
req = urllib.request.Request("http://localhost:8080/api/v1/streams",
    data=payload, headers={"Content-Type": "application/json"}, method="POST")
try:
    resp = urllib.request.urlopen(req, timeout=30)
    print("Created:", resp.status)
    body = json.loads(resp.read())
    print("ID:", body.get("id"))
except Exception as e:
    print("FAIL:", e)
    exit(1)

# Wait and check metrics
time.sleep(10)
try:
    m = urllib.request.urlopen("http://localhost:8080/metrics", timeout=5).read().decode()
    for line in m.split("\n"):
        if "getframe_" in line and not line.startswith("#"):
            print("  ", line)
except Exception as e:
    print("METRICS FAIL:", e)

# Check streams
try:
    s = json.loads(urllib.request.urlopen("http://localhost:8080/api/v1/streams", timeout=5).read())
    for stream in s["streams"]:
        print(f"  Stream: status={stream['status']}, decoded={stream['frames_decoded']}, extracted={stream['frames_extracted']}")
except Exception as e:
    print("STREAMS FAIL:", e)

import urllib.request, json, time

# 1. Start ffmpeg
import subprocess
subprocess.run(["docker", "run", "-d", "--rm", "--name", "qt-ffmpeg", "--network", "benchmark_bench-net", "--entrypoint", "ffmpeg", "linuxserver/ffmpeg:latest",
    "-re", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=5",
    "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency",
    "-f", "rtsp", "rtsp://mediamtx:8554/qt-stream"],
    capture_output=True)
time.sleep(5)

# 2. Register stream
payload = json.dumps({"config": {"source_url": "rtsp://mediamtx:8554/qt-stream", "source_type": "rtsp", "extract_interval_seconds": 0.2, "rtsp_transport": "tcp"}}).encode()
req = urllib.request.Request("http://localhost:8080/api/v1/streams", data=payload, headers={"Content-Type": "application/json"}, method="POST")
resp = urllib.request.urlopen(req, timeout=30)
print("Created:", resp.status, resp.read().decode())

# 3. Wait for decoding
print("Waiting 15s for decoding...")
time.sleep(15)

# 4. Check metrics
metrics = urllib.request.urlopen("http://localhost:8080/metrics").read().decode()
for line in metrics.split("\n"):
    if "getframe_" in line and not line.startswith("#"):
        print(line)

# 5. Check stream health
streams = json.loads(urllib.request.urlopen("http://localhost:8080/api/v1/streams").read())
for s in streams["streams"]:
    print(f"Stream: status={s['status']}, frames_decoded={s['frames_decoded']}, frames_extracted={s['frames_extracted']}")

# Cleanup
wid = streams["streams"][0]["id"]
req = urllib.request.Request(f"http://localhost:8080/api/v1/streams/{wid}", method="DELETE")
urllib.request.urlopen(req, timeout=10)
print("Deleted stream")
subprocess.run(["docker", "rm", "-f", "qt-ffmpeg"], capture_output=True)
print("Done")

import urllib.request, json
req = urllib.request.Request(
    "http://localhost:8080/api/v1/streams",
    data=json.dumps({"config": {"source_url": "rtsp://mediamtx:8554/test-ffmpeg", "source_type": "rtsp", "extract_interval_seconds": 1.0}}).encode(),
    headers={"Content-Type": "application/json"},
    method="POST"
)
try:
    resp = urllib.request.urlopen(req, timeout=15)
    print("OK:", resp.status, resp.read().decode())
except urllib.error.HTTPError as e:
    print("HTTP:", e.code, e.read().decode())
except Exception as e:
    print("ERR:", e)

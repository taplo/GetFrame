import urllib.request, json
payload = json.dumps({"config": {"source_url": "rtsp://mediamtx:8554/test-ffmpeg", "source_type": "rtsp", "extract_interval_seconds": 1.0, "rtsp_transport": "tcp"}}).encode()
req = urllib.request.Request(
    "http://localhost:8080/api/v1/streams",
    data=payload,
    headers={"Content-Type": "application/json"},
    method="POST"
)
try:
    resp = urllib.request.urlopen(req, timeout=30)
    print("OK:", resp.status, resp.read().decode())
except urllib.error.HTTPError as e:
    print("HTTP:", e.code, e.read().decode())
except urllib.error.URLError as e:
    print("URLERR:", e.reason)
except Exception as e:
    print("ERR:", type(e).__name__, e)

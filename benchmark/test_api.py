import json
import urllib.request

data = json.loads(urllib.request.urlopen("http://localhost:8080/api/v1/streams", timeout=5).read())
total = sum(s.get("frames_decoded", 0) for s in data.get("streams", []))
print("Total frames:", total)
print("Streams:", len(data.get("streams", [])))
for s in data.get("streams", []):
    print(f"  {s['source_url']}: decoded={s['frames_decoded']}, extracted={s['frames_extracted']}, status={s['status']}")

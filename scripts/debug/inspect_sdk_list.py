import json
import urllib.request

url = "https://repo.harmonyos.com/sdkmanager/v5/ohos/getSdkList"
body = json.dumps({
    "osType": "darwin",
    "osArch": "arm64",
    "supportVersion": "26.0-ohos-single-1"
}).encode('utf-8')

req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
with urllib.request.urlopen(req) as resp:
    data = json.loads(resp.read().decode('utf-8'))

print(f"Total items: {len(data)}")
for i, item in enumerate(data):
    print(f"[{i}] path={item.get('path')}, apiVersion={item.get('apiVersion')}, version={item.get('version')}, displayName={item.get('displayName')}, releaseType={item.get('releaseType')}")
    print(f"    url={item.get('archive', {}).get('url')}")

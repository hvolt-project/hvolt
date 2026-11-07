import json
import urllib.request

url = "https://devecostudio-drcn.deveco.dbankcloud.com/sdkmanager/v7/hos/getSdkList"
body = json.dumps({
    "osType": "mac",
    "osArch": "arm64",
    "supportVersion": "5.1-hos-single-1",
    "deviceName": "DevEcoStudio"
}).encode('utf-8')

headers = {
    "Content-Type": "application/json",
    "AppId": "DevEcoStudio_Lib",
    "Version": "0.0.0.1"
}

req = urllib.request.Request(url, data=body, headers=headers)
try:
    with urllib.request.urlopen(req) as resp:
        content = resp.read().decode('utf-8')
        print(f"Status: {resp.status}")
        data = json.loads(content)
        print(f"Total items: {len(data) if isinstance(data, list) else data}")
        if isinstance(data, list):
            for i, item in enumerate(data[:15]):
                print(f"[{i}] path={item.get('path')}, apiVersion={item.get('apiVersion')}, version={item.get('version')}, displayName={item.get('displayName')}, releaseType={item.get('releaseType')}")
                print(f"    archive={item.get('archive')}")
        else:
            print(data)
except urllib.error.HTTPError as e:
    print(f"HTTP Error: {e.code} {e.read().decode('utf-8', errors='replace')}")
except Exception as e:
    print(f"Error: {e}")

#!/usr/bin/env python3
"""Report whether a newer HarmonyOS SDK or hvigor than the pinned one exists.

The pins live in the source (`src/sdk.rs`, `src/tools.rs`); this script reads them and
asks the same endpoints hvolt uses. It only reports — nothing is bumped without a test
run, because a pin that moves without the gates having seen the new SDK would claim
support that was never exercised.
"""

import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SDK_ENDPOINT = "https://repo.harmonyos.com/sdkmanager/v5/ohos/getSdkList"
# @ohos lives on HarmonyOS's own registry, not on npmjs: registry.npmjs.org answers 404
# for it and repo.harmonyos.com/npm answers 200 (measured 2026-10-19).
NPM = "https://repo.harmonyos.com/npm/@ohos%2Fhvigor"


def read_pin(pattern: str, path: str) -> str:
    match = re.search(pattern, (ROOT / path).read_text(encoding="utf-8"))
    if not match:
        raise SystemExit(f"pin not found: {pattern} in {path}")
    return match.group(1)


def post_json(url: str, body: dict) -> dict:
    request = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode())


def get_json(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=30) as response:
        return json.loads(response.read().decode())


def main() -> int:
    support_version = read_pin(r'SDK_SUPPORT_VERSION: &str = "([^"]+)"', "src/sdk.rs")
    hvigor_pin = read_pin(r'HVIGOR_VERSION: &str = "([^"]+)"', "src/tools.rs")
    ohpm_pin = read_pin(r'OHPM_VERSION: &str = "([^"]+)"', "src/tools.rs")

    payload = post_json(
        SDK_ENDPOINT,
        {"osType": "darwin", "osArch": "arm64", "supportVersion": support_version},
    )
    # The endpoint returns a bare array for this query (measured 2026-10-19); keep the
    # wrapped shapes as a fallback because the manager UI uses them.
    if isinstance(payload, list):
        items = payload
    else:
        items = payload.get("data") or payload.get("body", {}).get("data") or []
    releases = [
        item for item in items if str(item.get("releaseType", "")).lower() == "release"
    ]
    # Dotted versions compare as numbers, not as strings: "6.1.0.32" sorts above
    # "26.0.0.38" lexicographically, which is wrong.
    def numeric(version: str) -> tuple:
        return tuple(int(part) for part in re.findall(r"\d+", version))

    newest = sorted(
        {(item.get("version", ""), item.get("platformVersion", "")) for item in releases},
        key=lambda pair: numeric(pair[0]),
        reverse=True,
    )

    hvigor_latest = get_json(NPM)["dist-tags"]["latest"]

    crate_version = read_pin(r'(?m)^version\s*=\s*"([^"]+)"', "Cargo.toml")
    pinned_platform = crate_version.split("-")[0]

    print(f"supportVersion pin : {support_version}")
    print(f"hvolt version      : {crate_version}  (platform {pinned_platform})")
    print(f"SDK (release)      : {len(newest)} versions, newest {newest[0] if newest else '?'}")
    if newest:
        print(f"  platform {newest[0][1]} / build {newest[0][0]}")
        for version, platform in newest[:3]:
            print(f"    platform {platform} → build {version}")
    print(f"hvigor pin         : {hvigor_pin}   npm latest: {hvigor_latest}")
    print(f"ohpm pin           : {ohpm_pin}   (pinned on purpose: newer builds need a Huawei account)")

    drift = []
    if newest:
        newest_platform = ".".join(str(part) for part in numeric(newest[0][0])[:3])
        print(f"newest platform    : {newest_platform}   pinned: {pinned_platform}")
        if newest_platform != pinned_platform:
            drift.append(f"SDK platform {pinned_platform} → {newest_platform} (needs an hvolt minor)")
    if hvigor_latest != hvigor_pin:
        drift.append(f"hvigor {hvigor_pin} → {hvigor_latest}")
    if not newest:
        drift.append("SDK list returned nothing (endpoint or supportVersion changed)")
    print("DRIFT: " + ("; ".join(drift) if drift else "none"))
    return 0


if __name__ == "__main__":
    sys.exit(main())

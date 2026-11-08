#!/usr/bin/env python3
"""Remove finished subagents from this session's catalog and delete their archives.

Three steps per child, per the agent-cleanup skill:

  1. `subagents/removeChild`  — drops the catalog projection (`list_agents` / sidebar)
  2. `workspace/archiveSession` — archives the session store
  3. `workspace/deleteSession`  — deletes it (hard, no recycle bin)

Run this as a *background* job: calling the host's control RPC from inside a synchronous tool
call deadlocks the event loop that is waiting for that tool call.

Usage: cleanup-agents.py <child-id> [<child-id> ...]
"""

import base64
import hashlib
import hmac
import json
import os
import sys
import time
import urllib.request
import urllib.error

DSH_HOME = os.environ.get("DSH_HOME", os.path.expanduser("~/.dsh"))
BASE = os.environ.get("DSH_WEB_URL", "http://127.0.0.1:49465")
PARENT = os.environ["DSH_SESSION_ID"]

# Ids this script must never touch: the session itself, and children the user may still
# continue.  Keep this list in the file that actually runs (skill §5).
KEEP = {
    PARENT,
    "fd26f04f-98ca-452b-9abe-9a630bcaf8a4",
    "aab96d2d-763e-42c3-8ef3-598d6b0483b0",
    "125ae2aa-4b7b-4c77-987e-d28dfd981df3",
    "3f58b227-306e-457a-b363-2bd542345f85",
    "42d707b1-76ce-4871-ad2a-1a945acd34ad",
    "68d5e26b-a414-4c02-906a-9eea6cc8290f",
    "af9956e4-d4f1-4495-bfda-a7f9e83e2d4e",
}


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode().rstrip("=")


def stack_value(text: str, keys: list[str]):
    """Read `key:` blocks from the YAML subset the credentials file uses."""
    wanted = list(keys)
    current = None
    values = {}
    for raw in text.splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip())
        line = raw.strip()
        key, _, rest = line.partition(":")
        rest = rest.strip()
        path = (indent, key)
        if current is not None and indent <= current[0]:
            current = None
        if current is not None and indent == current[0] + 2 and key in wanted[1:]:
            current = ((indent, key), key)
            continue
        if key == wanted[0] and not rest:
            current = ((indent, key), "root")
            continue
        if current is not None and rest:
            values.setdefault(id(current), {})[key] = rest
    return values


def read_secret() -> bytes:
    text = open(os.path.join(DSH_HOME, ".credentials.yaml")).read()
    # Find the browser-session record block, then its `secret:` line inside it.
    block = None
    for raw in text.splitlines():
        if raw.strip().startswith("client-connection/browser-session:"):
            block = []
            continue
        if block is not None:
            if raw and not raw.startswith(" "):
                break
            block.append(raw)
    if block is None:
        raise SystemExit("no client-connection/browser-session record")
    for raw in block:
        key, _, value = raw.strip().partition(":")
        if key == "secret":
            return base64.urlsafe_b64decode(value.strip() + "=" * (-len(value.strip()) % 4))
    raise SystemExit("browser-session record has no secret")


def cookie() -> str:
    secret = read_secret()
    authority = BASE.split("://", 1)[1]
    name = "dsh-auth-" + b64url(hashlib.sha256(authority.encode()).digest())
    now = int(time.time() * 1000)
    payload = {
        "version": 1,
        "authority": authority,
        "issuedAt": now,
        "expiresAt": now + 3600 * 1000,
    }
    body = b64url(json.dumps(payload, separators=(",", ":")).encode())
    mac = b64url(hmac.new(secret, body.encode(), hashlib.sha256).digest())
    return f"{name}=v1.{body}.{mac}"


def call(namespace: str, method: str, args: dict, rid: str) -> tuple[bool, str]:
    url = f"{BASE}/api/{namespace}/{method}"
    body = json.dumps(
        {
            "type": "client-request",
            "rpcId": rid,
            "method": f"{namespace}/{method}",
            "payload": {"args": {"request": args}},
        }
    ).encode()
    request = urllib.request.Request(
        url,
        data=body,
        headers={"content-type": "application/json", "cookie": cookie()},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            text = response.read().decode()
            return True, text[:200]
    except urllib.error.HTTPError as error:
        return False, f"HTTP {error.code}: {error.read().decode()[:200]}"
    except Exception as error:  # noqa: BLE001 - report and continue with the next id
        return False, f"{type(error).__name__}: {error}"


def main() -> int:
    children = [c for c in sys.argv[1:] if c]
    overlap = [c for c in children if c in KEEP]
    if overlap:
        print(f"refusing: {overlap} is in the keep set")
        return 2
    if not children:
        print("usage: cleanup-agents.py <child-id> ...")
        return 2
    print(f"parent: {PARENT}")
    print(f"to remove: {len(children)}")
    failures = 0
    for child in children:
        ok, detail = call(
            "subagents",
            "removeChild",
            {"parentSessionId": PARENT, "childSessionId": child},
            f"c-rm-{child[:8]}",
        )
        print(f"  removeChild {child[:8]}: {'ok' if ok else 'FAIL'} {detail}")
        if not ok and "not-found" not in detail:
            failures += 1
        ok, detail = call(
            "workspace", "archiveSession", {"sessionId": child}, f"c-ar-{child[:8]}"
        )
        print(f"  archive     {child[:8]}: {'ok' if ok else 'FAIL'} {detail}")
        if not ok:
            failures += 1
            continue
        ok, detail = call(
            "workspace", "deleteSession", {"sessionId": child}, f"c-dl-{child[:8]}"
        )
        print(f"  delete      {child[:8]}: {'ok' if ok else 'FAIL'} {detail}")
        if not ok:
            failures += 1
    print(f"done, {failures} failure(s)")
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
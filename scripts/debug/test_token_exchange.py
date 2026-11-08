#!/usr/bin/env python3
"""Test OAuth2 token exchange with Huawei's endpoint."""

import sys
import urllib.request
import urllib.parse
import json
import hashlib
import base64
import secrets

CLIENT_ID = "6099200"
TOKEN_URL = "https://oauth-login.cloud.huawei.com/oauth2/v2/token"
REDIRECT_URI = "https://developer.huawei.com/aui2/handleLoginV2.html"
PROXY = "http://127.0.0.1:10808"


def pkce_challenge(verifier: str) -> str:
    digest = hashlib.sha256(verifier.encode()).digest()
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode()


def _do(opener, url, body):
    data = urllib.parse.urlencode(body).encode()
    req = urllib.request.Request(url, data=data,
        headers={"Content-Type": "application/x-www-form-urlencoded"})
    try:
        with opener.open(req, timeout=15) as resp:
            result = json.loads(resp.read().decode())
            print(f"  Status: {resp.status}")
            print(f"  Response: {json.dumps(result, indent=2, ensure_ascii=False)}")
            if "access_token" in result:
                print("  >>> GOT access_token!")
                print(f"  access_token: {result['access_token'][:50]}...")
    except urllib.error.HTTPError as e:
        print(f"  Status: {e.code}")
        print(f"  Body: {e.read().decode()[:500]}")


def test(code, verifier=None):
    proxy_handler = urllib.request.ProxyHandler({"http": PROXY, "https": PROXY})
    opener = urllib.request.build_opener(proxy_handler)

    tests = [
        ("no PKCE, no secret", {
            "grant_type": "authorization_code", "code": code,
            "client_id": CLIENT_ID, "redirect_uri": REDIRECT_URI}),
        ("with PKCE, no secret", {
            "grant_type": "authorization_code", "code": code,
            "client_id": CLIENT_ID, "redirect_uri": REDIRECT_URI,
            "code_verifier": verifier} if verifier else None),
        ("empty client_secret", {
            "grant_type": "authorization_code", "code": code,
            "client_id": CLIENT_ID, "client_secret": "",
            "redirect_uri": REDIRECT_URI}),
        ("oauth-api URL", {
            "grant_type": "authorization_code", "code": code,
            "client_id": CLIENT_ID, "redirect_uri": REDIRECT_URI}),
    ]

    for name, body in tests:
        if body is None:
            continue
        print(f"\n{'='*50}")
        print(f"Test: {name}")
        url = TOKEN_URL
        if "oauth-api" in name:
            url = "https://oauth-api.cloud.huawei.com/oauth2/v2/token"
        _do(opener, url, body)

    # Basic auth test
    print(f"\n{'='*50}")
    print("Test: HTTP Basic auth (client_id:empty)")
    creds = base64.b64encode(f"{CLIENT_ID}:".encode()).decode()
    body = {"grant_type": "authorization_code", "code": code, "redirect_uri": REDIRECT_URI}
    data = urllib.parse.urlencode(body).encode()
    req = urllib.request.Request(TOKEN_URL, data=data,
        headers={"Content-Type": "application/x-www-form-urlencoded",
                 "Authorization": f"Basic {creds}"})
    try:
        with opener.open(req, timeout=15) as resp:
            result = json.loads(resp.read().decode())
            print(f"  Status: {resp.status}")
            print(f"  Response: {json.dumps(result, indent=2, ensure_ascii=False)}")
    except urllib.error.HTTPError as e:
        print(f"  Status: {e.code}")
        print(f"  Body: {e.read().decode()[:500]}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        verifier = secrets.token_urlsafe(32)[:43]
        challenge = pkce_challenge(verifier)
        auth_url = (
            f"https://oauth-login.cloud.huawei.com/oauth2/v2/authorize?"
            f"client_id={CLIENT_ID}&response_type=code&"
            f"redirect_uri={urllib.parse.quote(REDIRECT_URI, safe='')}&"
            f"scope=openid+profile&state=hvolt_test&access_type=offline&"
            f"code_challenge={challenge}&code_challenge_method=S256"
        )
        print(f"Login URL:\n  {auth_url}")
        print(f"code_verifier: {verifier}")
        sys.exit(0)

    code = sys.argv[1]
    verifier = sys.argv[2] if len(sys.argv) > 2 else None
    test(code, verifier)

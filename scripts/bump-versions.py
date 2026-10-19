#!/usr/bin/env python3
"""Apply a mechanical version bump: a tool pin plus the release counter.

Called by `.github/workflows/sdk-refresh.yml` after `check-versions.py` decided the drift is
mechanical (same tool major, same SDK platform). Everything else — an SDK platform change, a
tool major — is deliberately not handled here.

    python3 scripts/bump-versions.py summary.json            # plan only
    python3 scripts/bump-versions.py summary.json --apply    # write
"""

import datetime
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def plan(summary: dict) -> list[str]:
    changes = []
    tools = ROOT / "src/tools.rs"
    text = tools.read_text(encoding="utf-8")
    pinned, latest = summary["hvigor_pinned"], summary["hvigor_latest"]
    if f'HVIGOR_VERSION: &str = "{pinned}"' not in text:
        raise SystemExit(f"hvigor pin {pinned} not found in src/tools.rs")
    changes.append(f"src/tools.rs: HVIGOR_VERSION {pinned} → {latest}")

    cargo_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    current = re.search(r'(?m)^version = "([^"]+)"', cargo_text).group(1)
    base, counter = current.rsplit("-", 1)
    if not counter.isdigit():
        raise SystemExit(f"version {current} does not end in a release counter")
    nxt = f"{base}-{int(counter) + 1}"
    changes.append(f"Cargo.toml, npm/package.json: version {current} → {nxt}")
    changes.append("CHANGELOG.md: entry for " + nxt)
    return changes, nxt, pinned, latest, current


def main() -> int:
    if len(sys.argv) < 2:
        raise SystemExit("usage: bump-versions.py <summary.json> [--apply]")
    summary = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
    apply = "--apply" in sys.argv

    if not summary.get("tool_bump_only"):
        raise SystemExit("this drift is not mechanical; nothing to bump")
    changes, nxt, pinned, latest, current = plan(summary)
    for change in changes:
        print(change)
    if not apply:
        print("(plan only; pass --apply to write)")
        return 0

    tools = ROOT / "src/tools.rs"
    tools.write_text(
        tools.read_text(encoding="utf-8").replace(
            f'HVIGOR_VERSION: &str = "{pinned}"', f'HVIGOR_VERSION: &str = "{latest}"', 1
        ),
        encoding="utf-8",
    )
    cargo = ROOT / "Cargo.toml"
    cargo.write_text(
        re.sub(r'(?m)^version = "([^"]+)"', f'version = "{nxt}"', cargo.read_text(encoding="utf-8"), count=1),
        encoding="utf-8",
    )
    npm = ROOT / "npm/package.json"
    package = json.loads(npm.read_text(encoding="utf-8"))
    package["version"] = nxt
    npm.write_text(json.dumps(package, indent=2) + "\n", encoding="utf-8")

    changelog = ROOT / "CHANGELOG.md"
    text = changelog.read_text(encoding="utf-8")
    entry = (
        f"## [{nxt}] — {datetime.date.today().isoformat()}\n\n"
        "### Changed\n\n"
        f"- The hvigor pin follows upstream within the same major version ({pinned} → {latest}).\n"
        "  Upstream patch releases are compatible by their own policy, so no behaviour change is\n"
        "  claimed beyond the pin. This entry was written by the refresh workflow.\n\n"
    )
    text = text.replace("## [Unreleased]\n\n", "## [Unreleased]\n\n" + entry, 1)
    changelog.write_text(text, encoding="utf-8")
    print(f"applied; version {current} → {nxt}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

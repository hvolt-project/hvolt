#!/bin/bash
# The extra emitted files a fixture asks to be compared.
#
# `fixtures/<name>/extra-files.json` declares sources beyond the page:
#
#   { "files": ["common/ObservedModel.ets"] }
#
# Each entry is a source path relative to the module root, the way the fixture's project
# writes it (`project/src/common/ObservedModel.ets`). It is printed here as the emitted
# `.ts` path, relative to the module root, which is the shape both harvests and the
# comparison address a file by.
#
# Usage: extra-files.sh <fixture-dir>
#
# A fixture without the declaration prints nothing, which keeps every existing fixture on the
# page-only path it has always had.

set -euo pipefail

FIXTURE="${1:?usage: extra-files.sh <fixture-dir>}"
DECL="$FIXTURE/extra-files.json"
[ -f "$DECL" ] || exit 0

python3 - "$DECL" <<'PY'
import json
import sys

declaration = json.load(open(sys.argv[1], encoding="utf-8"))
for source in declaration.get("files", []):
    # `.ets` is four characters, `.ts` three: slicing a fixed four turns `A.ts` into `A.t`.
    if source.endswith(".ets"):
        stem = source[:-4]
    elif source.endswith(".ts"):
        stem = source[:-3]
    else:
        stem = source
    print(f"{stem}.ts")
PY

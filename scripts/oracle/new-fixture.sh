#!/bin/bash
# Materialise one fixture as a hvolt project: `fixtures/<name>/project`.
#
# The construct under test lives in `fixtures/<name>/input.ets`; the project around it comes
# from `fixtures/_hvolt-template`, so the repository only ever holds hvolt projects. Existing
# files are overwritten in place; nothing is removed.
#
# Usage: scripts/oracle/new-fixture.sh <name>

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAME="${1:?usage: new-fixture.sh <name>}"
FIXTURE="$ROOT/fixtures/$NAME"
[ -f "$FIXTURE/input.ets" ] || { echo "no input.ets in $FIXTURE" >&2; exit 2; }

mkdir -p "$FIXTURE/project"
rsync -a "$ROOT/fixtures/_hvolt-template/" "$FIXTURE/project/"
mkdir -p "$FIXTURE/project/src/pages"
cp "$FIXTURE/input.ets" "$FIXTURE/project/src/pages/Index.ets"
echo "project: $FIXTURE/project"

#!/bin/bash
# Build one fixture with hvigor and harvest its artifacts as the golden set.
#
# The fixture is a hvolt project (`fixtures/<name>/project`); the equivalent standard layout is
# written into a temporary directory by `emit-layout.py`, so the repository never contains a
# hvigor layout. Only the artifacts under `fixtures/<name>/golden` are kept.
#
# Usage: scripts/oracle/run-hvigor.sh <fixture-name>

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAME="${1:?usage: run-hvigor.sh <fixture-name>}"
FIXTURE="$ROOT/fixtures/$NAME"
PROJECT="$FIXTURE/project"
[ -d "$PROJECT" ] || "$ROOT/scripts/oracle/new-fixture.sh" "$NAME" >/dev/null

HVIGORW=/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw
NODE_HOME=/Applications/DevEco-Studio.app/Contents/tools/node
export NODE_HOME
export PATH="$NODE_HOME/bin:$PATH"
export DEVECO_SDK_HOME=/Applications/DevEco-Studio.app/Contents/sdk

WORK="$(mktemp -d "${TMPDIR:-/tmp}/oracle-$NAME-XXXXXX")"
python3 "$ROOT/scripts/oracle/emit-layout.py" "$PROJECT" "$WORK/layout" >/dev/null

cd "$WORK/layout"
export NODE_V8_COVERAGE="$WORK/v8cov"
mkdir -p "$NODE_V8_COVERAGE"
# `assembleHap` fails late on this machine because packaging needs a JRE, but the ArkTS compile
# and the resource stage run first; judge success by the artifacts, not by the exit code.
timeout 900 "$HVIGORW" --mode module -p module=entry@default -p product=default assembleHap --no-daemon \
  > "$WORK/hvigor.log" 2>&1 || true

mkdir -p "$FIXTURE/golden"
EMITTED="$(find "$WORK/layout/entry/build" -path "*pages/Index.ts" | head -1)"
[ -n "$EMITTED" ] || { echo "no emitted Index.ts for $NAME; log: $WORK/hvigor.log" >&2; tail -20 "$WORK/hvigor.log" >&2; exit 1; }
cp "$EMITTED" "$FIXTURE/golden/Index.ts"
# `compare.js` reads the AST-level golden next to the fixture, not inside `golden/`.
cp "$EMITTED" "$FIXTURE/golden.ts"
# Files beyond the page that the fixture declares (`extra-files.json`). The golden tree keeps
# one directory level per declared path, so `golden/common/ObservedModel.ts` mirrors the
# module's own `src/common/ObservedModel.ets`; the comparison reads the same relative path on
# both sides.
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  extra="$(find "$WORK/layout/entry/build" -path "*/$rel" -type f 2>/dev/null | head -1)"
  if [ -n "$extra" ]; then
    mkdir -p "$FIXTURE/golden/$(dirname "$rel")"
    cp "$extra" "$FIXTURE/golden/$rel"
  else
    echo "declared file not emitted by the reference: $rel" >&2
  fi
done < <(bash "$ROOT/scripts/oracle/extra-files.sh" "$FIXTURE")

harvest() {
  local pattern="$1" target="$2"
  local file
  file="$(find "$WORK/layout/entry/build" -name "$pattern" | head -1)"
  if [ -n "$file" ]; then cp "$file" "$FIXTURE/golden/$target"; fi
}
harvest "module.json" module.json
harvest "resources.index" resources.index
harvest "pack.info" pack.info
harvest "pkgContextInfo.json" pkgContextInfo.json
harvest "modules.abc" modules.abc
harvest "insight_intent.json" insight_intent.json
# The reference writes one `sourceMaps.map` per build in four places; the packaged copy under
# `outputs/default/mapping/` is the artifact the packer carries, so that is the one kept.
# hvolt does not produce a source map yet; the comparison below reports the missing file rather
# than treating the absence as agreement.
SOURCE_MAP="$(find "$WORK/layout/entry/build/default/outputs/default/mapping" -name "sourceMaps.map" 2>/dev/null | head -1)"
if [ -n "$SOURCE_MAP" ]; then cp "$SOURCE_MAP" "$FIXTURE/golden/sourceMaps.map"; fi

# Record what this build actually produced. `compare-artifacts.py` reads this list so a file
# left in `golden/` by an older harvest is not compared as if it were current.
GOLDEN_MANIFEST="$FIXTURE/golden/manifest.txt"
: > "$GOLDEN_MANIFEST"
for artifact in module.json resources.index pack.info pkgContextInfo.json modules.abc \
                insight_intent.json sourceMaps.map; do
  if [ -f "$FIXTURE/golden/$artifact" ]; then echo "$artifact" >> "$GOLDEN_MANIFEST"; fi
done

node "$ROOT/scripts/oracle/covers-from-coverage.js" "$NODE_V8_COVERAGE" "$FIXTURE/covers.json" 2>/dev/null \
  || echo "covers: none collected for $NAME"
echo "golden: $(ls "$FIXTURE/golden" | tr '\n' ' ')"

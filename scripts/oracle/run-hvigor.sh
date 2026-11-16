#!/bin/bash
# Build a fixture with hvigor and harvest its artifacts as the golden set.
#
# The fixture is a hvolt project (`fixtures/<name>/project`); the equivalent standard layout is
# written into a temporary directory by `emit-layout.py`, so the repository never contains a
# hvigor layout. Only the artifacts under `fixtures/<name>/golden` are kept.
#
# Two modes:
#
#   run-hvigor.sh <fixture-name>
#       Golden harvest for the repository's fixtures (entry module only, as before).
#
#   run-hvigor.sh --truth <hvolt-project-dir> <out-dir>
#       Truth harvest for a project of any shape: every member of `[app].members` is built with
#       the task that packages it (`assembleHap` for an entry module, `assembleHsp` for a shared
#       one), and each produced package is unpacked into `<out-dir>/<module>/`, one directory per
#       package. Nothing is written outside `<out-dir>`.
#
# Environment:
#   ORACLE_NO_JAVA_HOME=1   leave JAVA_HOME unset (records the without-JRE behaviour).
#                           Default: JAVA_HOME points at the JRE DevEco ships, without which the
#                           packaging stage (`assembleHap`/`assembleHsp`) fails on this machine
#                           while the ArkTS compile and resource stages have already succeeded.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

HVIGORW=/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw
NODE_HOME=/Applications/DevEco-Studio.app/Contents/tools/node
DEVECO_JRE=/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home
export NODE_HOME
export PATH="$NODE_HOME/bin:$PATH"
export DEVECO_SDK_HOME=/Applications/DevEco-Studio.app/Contents/sdk

# Packaging runs a JVM; without `JAVA_HOME` the build reaches the packaging stage and dies there.
if [ "${ORACLE_NO_JAVA_HOME:-0}" = "1" ]; then
  unset JAVA_HOME || true
else
  export JAVA_HOME="$DEVECO_JRE"
  export PATH="$JAVA_HOME/bin:$PATH"
fi

run_hvigor() {  # <layout-dir> <module> <task> <log> <label>
  local layout="$1" module="$2" task="$3" log="$4" label="$5"
  echo "== hvigor $label: $module ($task)" | tee -a "$log"
  echo "   JAVA_HOME=${JAVA_HOME:-<unset>}" | tee -a "$log"
  ( cd "$layout" && timeout 900 "$HVIGORW" --mode module -p module="$module@default" \
      -p product=default "$task" --no-daemon ) >> "$log" 2>&1 || true
}

# --- truth mode -----------------------------------------------------------------------------
if [ "${1:-}" = "--truth" ]; then
  PROJECT="${2:?usage: run-hvigor.sh --truth <hvolt-project-dir> <out-dir>}"
  OUT="${3:?usage: run-hvigor.sh --truth <hvolt-project-dir> <out-dir>}"
  PROJECT="$(cd "$PROJECT" && pwd)"
  mkdir -p "$OUT"
  OUT="$(cd "$OUT" && pwd)"

  WORK="$(mktemp -d "${TMPDIR:-/tmp}/oracle-truth-XXXXXX")"
  python3 "$ROOT/scripts/oracle/emit-layout.py" "$PROJECT" "$WORK/layout"
  MODULES="$WORK/layout.modules.json"
  cp "$MODULES" "$OUT/modules.json"

  {
    echo "project: $PROJECT"
    echo "layout:  $WORK/layout"
    echo "date:    $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "JAVA_HOME: ${JAVA_HOME:-<unset>}"
    echo "hvigorw: $HVIGORW"
    echo "sdk:     $DEVECO_SDK_HOME"
    echo "hvigorw --version: $(cd "$WORK/layout" && "$HVIGORW" --version --no-daemon 2>&1 | head -3 | tr '\n' ' ')"
  } > "$OUT/env.txt"

  # One hvigor invocation per module: a module's task packages that module and nothing else,
  # which is what makes "which file belongs to which package" answerable at all.
  while IFS=$'\t' read -r name dir kind task ext; do
    log="$OUT/hvigor-$name.log"
    : > "$log"
    run_hvigor "$WORK/layout" "$name" "$task" "$log" "$kind"
    pkg_dir="$OUT/$name"
    mkdir -p "$pkg_dir"

    # The package body first: the unpacked tree is the authority for "what is inside the package".
    unsigned="$(find "$WORK/layout/$dir/build" -name "*-unsigned.$ext" 2>/dev/null | head -1)"
    if [ -n "$unsigned" ]; then
      cp "$unsigned" "$pkg_dir/$(basename "$unsigned")"
      ( cd "$pkg_dir" && unzip -o -q "$(basename "$unsigned")" -d package ) || true
      ( cd "$pkg_dir" && unzip -l "$(basename "$unsigned")" > contents.txt ) || true
    else
      echo "no *-unsigned.$ext produced for $name (see hvigor-$name.log)" >&2
    fi

    # Harvested files prefer the unpacked copy — that is the packed form the device receives —
    # and fall back to the build tree so a failed pack still yields what was compiled.
    harvest() {
      local file="$1" target="$2"
      if [ -f "$pkg_dir/package/$target" ]; then
        mkdir -p "$pkg_dir/$(dirname "$target")"
        cp "$pkg_dir/package/$target" "$pkg_dir/$target"
        return 0
      fi
      file="$(find "$WORK/layout/$dir/build" -path "*/$target" 2>/dev/null | head -1)"
      if [ -n "$file" ]; then
        mkdir -p "$pkg_dir/$(dirname "$target")"
        cp "$file" "$pkg_dir/$target"
        echo "  $target: from build tree (not inside the package)" >> "$log"
      else
        echo "  $target: 未取到" >> "$log"
      fi
    }
    harvest "" module.json
    harvest "" pack.info
    harvest "" pkgContextInfo.json
    harvest "" resources.index
    harvest "" ets/modules.abc
    # Some builds carry the abc at the module root instead of under `ets/`.
    if [ ! -f "$pkg_dir/ets/modules.abc" ]; then
      harvest "" modules.abc
    fi
  done < <(python3 - "$MODULES" <<'PY'
import json, sys
for m in json.load(open(sys.argv[1]))["modules"]:
    print("\t".join([m["name"], m["dir"], m["kind"], m["task"], m["extension"]]))
PY
)

  echo "truth: $OUT"
  ls -1 "$OUT"
  exit 0
fi

# --- golden mode (fixtures) -----------------------------------------------------------------
NAME="${1:?usage: run-hvigor.sh <fixture-name> | --truth <project-dir> <out-dir>}"
FIXTURE="$ROOT/fixtures/$NAME"
PROJECT="$FIXTURE/project"
[ -d "$PROJECT" ] || "$ROOT/scripts/oracle/new-fixture.sh" "$NAME" >/dev/null

WORK="$(mktemp -d "${TMPDIR:-/tmp}/oracle-$NAME-XXXXXX")"
python3 "$ROOT/scripts/oracle/emit-layout.py" "$PROJECT" "$WORK/layout" >/dev/null
# The first member of the project is the module the golden set describes.
read -r MODULE MODDIR MODTASK <<<"$(python3 - "$WORK/layout.modules.json" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))["modules"][0]
print(m["name"], m["dir"], m["task"])
PY
)"

cd "$WORK/layout"
export NODE_V8_COVERAGE="$WORK/v8cov"
mkdir -p "$NODE_V8_COVERAGE"
# `assembleHap` fails late on this machine because packaging needs a JRE, but the ArkTS compile
# and the resource stage run first; judge success by the artifacts, not by the exit code.
# `JAVA_HOME` is exported above so the packaged artifacts exist too; set `ORACLE_NO_JAVA_HOME=1`
# to reproduce the pre-JRE behaviour (compile-stage artifacts only).
timeout 900 "$HVIGORW" --mode module -p module="$MODULE@default" -p product=default "$MODTASK" --no-daemon \
  > "$WORK/hvigor.log" 2>&1 || true

mkdir -p "$FIXTURE/golden"
EMITTED="$(find "$WORK/layout/$MODDIR/build" -path "*pages/Index.ts" | head -1)"
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
  extra="$(find "$WORK/layout/$MODDIR/build" -path "*/$rel" -type f 2>/dev/null | head -1)"
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
  file="$(find "$WORK/layout/$MODDIR/build" -name "$pattern" | head -1)"
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
SOURCE_MAP="$(find "$WORK/layout/$MODDIR/build/default/outputs/default/mapping" -name "sourceMaps.map" 2>/dev/null | head -1)"
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
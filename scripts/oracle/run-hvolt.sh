#!/bin/bash
# Build one fixture with hvolt and harvest the artifacts it produces.
#
# The same hvolt project the oracle layout was generated from is built here, so both sides
# describe one project; only the produced artifacts are compared.
#
# Usage: scripts/oracle/run-hvolt.sh <fixture-name>

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAME="${1:?usage: run-hvolt.sh <fixture-name>}"
FIXTURE="$ROOT/fixtures/$NAME"
PROJECT="$FIXTURE/project"
[ -d "$PROJECT" ] || { echo "no project for $NAME; run new-fixture.sh" >&2; exit 2; }
HVOLT="$ROOT/target/debug/hvolt"

# Where this run's outputs land. Running this script by hand still writes beside the fixture, as
# it always has. `all.sh` points this at a per-run directory instead, so that two oracles
# building the same fixture at the same time can never write, or read, one `actual.ts` between
# them: every comparison reads exactly what its own run produced.
RUN_DIR="${HVOLT_RUN_DIR:-$FIXTURE}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/hvolt-$NAME-XXXXXX")"
rsync -a "$PROJECT/" "$WORK/project/"
# hvolt stages its emitted `.ts` tree under `std::env::temp_dir()` as `hvolt-ets-<pid>`. Point
# TMPDIR at this run's own directory: the pid is not unique across concurrent builds, and
# picking a staging tree out of the shared temp dir by name order reads another fixture's
# sources (measured 2026-11-06: component-v2 was built from lazy-forms' tree).
mkdir -p "$WORK/tmp"
cd "$WORK/project"
TMPDIR="$WORK/tmp" HVOLT_KEEP_TS=1 "$HVOLT" build > "$WORK/hvolt.log" 2>&1 || {
  echo "hvolt failed for $NAME; log: $WORK/hvolt.log" >&2
  tail -20 "$WORK/hvolt.log" >&2
  exit 1
}

mkdir -p "$RUN_DIR/actual"
# Every run records which artifacts it produced. The comparison only looks at the files in this
# list, so a file left behind by an earlier build cannot be compared as if it were current —
# and nothing has to be deleted to achieve that.
MANIFEST="$RUN_DIR/actual/manifest.txt"
: > "$MANIFEST"
stage() {
  [ -f "$1" ] || return 0
  cp "$1" "$RUN_DIR/actual/$2"
  echo "$2" >> "$MANIFEST"
}
OUT="$WORK/project/build/hvolt-out"
for artifact in module.json pack.info pkgContextInfo.json resources.index; do
  stage "$OUT/$artifact" "$artifact"
done
ABC="$(find "$WORK/project/build" -name 'modules.abc' | head -1)"
[ -n "$ABC" ] && stage "$ABC" modules.abc
# The intent manifest lives in the module's profile resources, not at the output root.
PROFILE_JSON="$(find "$OUT" -path "*base/profile/insight_intent.json" -type f 2>/dev/null | head -1)"
[ -n "$PROFILE_JSON" ] && stage "$PROFILE_JSON" insight_intent.json
# A source map is a build artifact, not part of `modules.abc`: stage it when hvolt emits one so
# the comparison has something to judge. An absent map is recorded as absent — `stage` only
# writes the manifest entry for a file that exists.
SOURCE_MAP="$(find "$WORK/project/build" -name 'sourceMaps.map' -type f 2>/dev/null | head -1)"
[ -n "$SOURCE_MAP" ] && stage "$SOURCE_MAP" sourceMaps.map
STAGED="$(find "$WORK/tmp" -maxdepth 1 -name 'hvolt-ets-*' -type d 2>/dev/null | sort | head -1)"
if [ -n "$STAGED" ]; then
  TS="$(find "$STAGED" -name 'Index.ts' -path '*pages*' | sort | head -1)"
  if [ -n "$TS" ]; then
    stage "$TS" Index.ts
    # The AST comparison reads this one beside `actual/`, not inside it.
    cp "$TS" "$RUN_DIR/actual.ts"
  fi
  # The files beyond the page the fixture declares (`extra-files.json`), harvested the same
  # way the golden side harvests them so the two trees line up by relative path.
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    extra="$(find "$STAGED" -path "*/$rel" -type f 2>/dev/null | head -1)"
    if [ -n "$extra" ]; then
      mkdir -p "$RUN_DIR/actual/$(dirname "$rel")"
      cp "$extra" "$RUN_DIR/actual/$rel"
    fi
  done < <(bash "$ROOT/scripts/oracle/extra-files.sh" "$FIXTURE")
fi
echo "actual: $(ls "$RUN_DIR/actual" | tr '\n' ' ')"

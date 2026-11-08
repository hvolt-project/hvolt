#!/bin/bash
# Regenerate every fixture's golden (hvigor) and actual (hvolt), then compare them.
#
# Usage: scripts/oracle/all.sh [fixture ...]

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Where the hvolt side of each comparison lands. Two oracles can run at once (concurrent agents,
# or a `verify.sh` next to a hand-run `all.sh`), and they must not share a fixture's `actual.ts`:
# one would read the other's output, or a half-written file. Each run therefore gets its own
# directory, and the comparison addresses a *view* of the fixture built inside it — the golden
# side is linked in (shared and read-only), the actual side is this run's alone.
#
# `verify.sh` passes a root it owns because it reads the same directories afterwards for the
# artifact comparison; when nobody passes one, this script makes its own and removes it on exit.
RUN_ROOT="${ORACLE_RUN_ROOT:-}"
if [ -z "$RUN_ROOT" ]; then
  RUN_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/oracle-runs-XXXXXX")"
  trap 'rm -rf "${RUN_ROOT:?}"' EXIT
fi
export RUN_ROOT

# A golden is the reference implementation's output for one fixture's input, so it only has to
# be regenerated when that input changes. Rebuilding it on every run made the quick loop pay for
# thirteen DevEco builds; `--regen` forces the slow path when the reference itself moved.
REGEN=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --regen) REGEN=1 ;;
    *) ARGS+=("$arg") ;;
  esac
done
set -- ${ARGS[@]+"${ARGS[@]}"}

needs_golden() {
  local fixture="$1"
  [ "$REGEN" -eq 1 ] && return 0
  [ -f "$fixture/golden/Index.ts" ] || return 0
  [ "$fixture/golden.ts" -nt "$fixture/golden/Index.ts" ] && return 0
  [ "$fixture/input.ets" -nt "$fixture/golden/Index.ts" ] && return 0
  # The manifest records which artifacts this harvest produced. A golden set generated before
  # the manifest existed is incomplete (it may be missing artifacts the reference does emit),
  # so it is rebuilt once.
  [ -f "$fixture/golden/manifest.txt" ] || return 0
  # A fixture that declares extra files declares *which* files are part of its golden: a
  # declaration newer than the golden means the set may have changed.
  if [ -f "$fixture/extra-files.json" ] && [ "$fixture/extra-files.json" -nt "$fixture/golden/Index.ts" ]; then
    return 0
  fi
  return 1
}

FIXTURES=()
if [ "$#" -gt 0 ]; then FIXTURES=("$@"); fi
if [ "${#FIXTURES[@]}" -eq 0 ]; then
  # macOS ships bash 3.2, which has no `mapfile`.
  while IFS= read -r name; do FIXTURES+=("$name"); done < <(
    find "$ROOT/fixtures" -mindepth 1 -maxdepth 1 -type d ! -name '_*' \
      -exec basename {} \; | sort
  )
fi

fail=0
# Goldens stay serial: hvigor takes a per-user cache lock, so two `run-hvigor.sh` at once
# collide. Only the fixtures whose input actually moved reach here, so this loop is usually
# empty and costs nothing.
for name in "${FIXTURES[@]}"; do
  if needs_golden "$ROOT/fixtures/$name"; then
    "$ROOT/scripts/oracle/run-hvigor.sh" "$name" > /dev/null || { echo "GOLDEN FAIL $name"; fail=1; }
  fi
done

# The comparison half is what actually dominated the wall time: one `hvolt` process and one
# `node` process per fixture, mostly process startup rather than work. Each fixture stages its
# own tree under its own directory, so they run concurrently.
check_one() {
  local name="$1"
  local fixture="$ROOT/fixtures/$name"
  local run="$RUN_ROOT/$name"
  mkdir -p "$run"
  # `compare.js` addresses both sides by fixed names inside one directory, so the run directory
  # is a view of the fixture: golden linked (shared, never written), actual belonging to this run.
  ln -sf "$fixture/golden" "$run/golden"
  ln -sf "$fixture/golden.ts" "$run/golden.ts"
  # `compare-artifacts.py` resolves the source names in `sourceMaps.map` under
  # `<dir>/project/src`, so the project comes along by link as well; `verify.sh` compares the
  # same view once the oracle has finished.
  ln -sf "$fixture/project" "$run/project"
  if [ -f "$fixture/extra-files.json" ]; then
    ln -sf "$fixture/extra-files.json" "$run/extra-files.json"
  fi
  HVOLT_RUN_DIR="$run" "$ROOT/scripts/oracle/run-hvolt.sh" "$name" > /dev/null || {
    echo "ACTUAL FAIL $name"
    return 1
  }
  # The verdict is `compare.js`'s; the only thing rewritten here is the directory it names, so a
  # failure still points at the fixture rather than at this run's temporary tree.
  local report
  if report="$(node "$ROOT/scripts/oracle/compare.js" "$run" 2>&1)"; then
    printf '%s\n' "$report" | sed "s|$run|$fixture|g"
    return 0
  fi
  printf '%s\n' "$report" | sed "s|$run|$fixture|g" >&2
  return 1
}
export ROOT
export -f check_one
jobs="${ORACLE_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"
printf '%s\n' "${FIXTURES[@]}" | xargs -P "$jobs" -I{} bash -c 'check_one "$@"' _ {} || fail=1
exit $fail

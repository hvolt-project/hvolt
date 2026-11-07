#!/bin/bash
# Report oracle gaps that span every fixture once.
#
# `compare-artifacts.py` judges artifacts both sides produce: a difference there is a per-fixture
# defect. A file the reference produces for every fixture and hvolt never produces is one gap,
# not one difference per fixture; mixing it into the per-fixture verdict marks every fixture DIFF
# and hides real differences. Each gap is printed here once, and the script exits non-zero while
# one is open.
#
# Usage: scripts/oracle/gaps.sh [fixture ...]

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURES=()
if [ "$#" -gt 0 ]; then
  FIXTURES=("$@")
else
  # macOS ships bash 3.2, which has no `mapfile`.
  while IFS= read -r name; do FIXTURES+=("$name"); done < <(
    find "$ROOT/fixtures" -mindepth 1 -maxdepth 1 -type d ! -name '_*' \
      -exec basename {} \; | sort
  )
fi

# One `gap <artifact>` call per artifact that only one side produces.
gap() {
  local artifact="$1"
  local expected=0 produced=0
  local name dir
  for name in "${FIXTURES[@]}"; do
    dir="$ROOT/fixtures/$name"
    [ -f "$dir/golden/$artifact" ] || continue
    expected=$((expected + 1))
    if [ -f "$dir/actual/manifest.txt" ] && grep -qx "$artifact" "$dir/actual/manifest.txt"; then
      produced=$((produced + 1))
    fi
  done
  if [ "$expected" -gt 0 ] && [ "$produced" -lt "$expected" ]; then
    echo "GAP $artifact: golden has it in $expected/$expected fixture(s), hvolt produced $produced/$expected"
    return 1
  fi
  echo "ok  $artifact: golden has it in $expected/$expected fixture(s), hvolt produced $produced/$expected"
  return 0
}

fail=0
# The reference writes `sourceMaps.map` for every build; hvolt has no source-map emission yet.
gap "sourceMaps.map" || fail=1
exit $fail
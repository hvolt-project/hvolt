#!/bin/bash
# One gate for the whole tree: format, lint, tests, then the oracle.
#
# The oracle compares hvolt's output against the reference implementation's, so it is the only
# evidence that matters; a project that merely builds proves nothing.
#
# Usage: scripts/verify.sh [--regen]   (--regen rebuilds every golden with hvigor)

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_HOME="$ROOT/.cargo-home"
cd "$ROOT"

fail=0
say() { printf '%-22s %s\n' "$1" "$2"; }

if cargo fmt --check >/dev/null 2>&1; then say "fmt" "ok"; else say "fmt" "FAIL"; fail=1; fi
lint="$(cargo clippy --offline --all-targets -- -D warnings 2>&1 | grep -cE '^error')"
if [ "$lint" = "0" ]; then say "clippy" "ok"; else say "clippy" "$lint errors"; fail=1; fi
tests="$(cargo test --offline 2>&1 | grep -oE '[0-9]+ passed; [0-9]+ failed' | tail -1)"
case "$tests" in
  *"0 failed") say "tests" "$tests" ;;
  *) say "tests" "$tests"; fail=1 ;;
esac

oracle_args=()
[ "${1:-}" = "--regen" ] && oracle_args+=(--regen)
# A per-run log: a fixed name is shared by every concurrent verify, so two runs overwrite each
# other and the counts come out as half a suite (seen once: 44 passed while all.sh was 86).
# The template must end in its Xs — `mktemp` only substitutes a *trailing* run of them (BSD and
# GNU agree). With a `.log` suffix it created the literal name instead, so a second concurrent
# run died on `File exists`, left `oracle_log` empty, and this script went on to print
# `oracle  passed,  failed` with both counts blank.
oracle_log="$(mktemp "${TMPDIR:-/tmp}/verify-oracle.XXXXXX")"
# `all.sh` puts each fixture's hvolt side in a per-run directory, so two oracles cannot read each
# other's `actual.ts`. This script owns that root because the artifact pass below looks at the
# same directories once the oracle has finished.
ORACLE_RUN_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/verify-runs-XXXXXX")"
export ORACLE_RUN_ROOT
bash scripts/oracle/all.sh "${oracle_args[@]+${oracle_args[@]}}" > "$oracle_log" 2>&1
passed="$(grep -c '^PASS' "$oracle_log")"
failed="$(grep -c '^FAIL' "$oracle_log")"
say "oracle" "$passed passed, $failed failed"
[ "$failed" != "0" ] && { grep '^FAIL' "$oracle_log" | head -5; fail=1; }
rm -f "$oracle_log"

# The artifact comparison is a separate judgement from the AST one: it looks at the packed
# modules.abc records, so it can disagree while every `.ts` matches. It exits non-zero on any
# difference, which is what decides the count.
#
# One `python3` process per fixture dominated this stage, so the checks run concurrently and
# report one line each; the counts are then folded here.
artifact_one() {
  local dir="$1"
  if python3 "$ROOT/scripts/oracle/compare-artifacts.py" "$dir" >/dev/null 2>&1; then
    echo same
  else
    echo different
  fi
}
export ROOT
export -f artifact_one
artifact_dirs=()
# The directories the oracle actually wrote: each one is a view of a fixture (golden linked,
# actual belonging to that run), which is where `compare-artifacts.py` expects to look.
for dir in "$ORACLE_RUN_ROOT"/*/; do
  name="$(basename "$dir")"
  case "$name" in _*|project) continue ;; esac
  [ -f "$dir/golden/modules.abc" ] || continue
  artifact_dirs+=("$dir")
done
artifact_jobs="${VERIFY_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"
same=0
different=0
if [ "${#artifact_dirs[@]}" -gt 0 ]; then
  while IFS= read -r verdict; do
    case "$verdict" in
      same) same=$((same + 1)) ;;
      different) different=$((different + 1)) ;;
    esac
  done < <(printf '%s\n' "${artifact_dirs[@]}" | xargs -P "$artifact_jobs" -I{} bash -c 'artifact_one "$@"' _ {})
fi
say "artifacts" "$same same, $different differing"

rm -rf "${ORACLE_RUN_ROOT:?}"

exit $fail

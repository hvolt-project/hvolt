#!/bin/bash
# One gate for the whole tree: format, lint, tests, then the oracle.
#
# The oracle compares hvolt's output against the reference implementation's, so it is the only
# evidence that matters; a project that merely builds proves nothing.
#
# Usage: scripts/verify.sh [--regen]   (--regen rebuilds every golden with hvigor)

set -uo pipefail

# 这台机器是多人共享的 32 核：编译与测试线程都压到 3，避免把 CPU 打满。
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-3}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# 仓库内的 `.cargo-home` 只有在解析得开时才用于离线构建；它可能只下载了部分
# crate（实测：`fluent-uri v0.4.1` 缺失时 clippy 报成一条编译错误）。用离线
# `cargo metadata` 判它能不能用，不能就用调用者自己的 `~/.cargo`。
if CARGO_HOME="$ROOT/.cargo-home" cargo metadata --offline --format-version 1 >/dev/null 2>&1; then
  export CARGO_HOME="$ROOT/.cargo-home"
else
  export CARGO_HOME="$HOME/.cargo"
fi
cd "$ROOT"

# The container ships its toolchain under `/opt/software/<name>/bin` (`software.env` names the
# same tools at `/srv/software`, the host-side mount). `compare.js` needs `node`; without it the
# oracle stage can only report that it could not judge anything.
if [ -d /opt/software ]; then
  for dir in /opt/software/*/bin; do
    [ -d "$dir" ] && PATH="$PATH:$dir"
  done
  export PATH
fi

fail=0
say() { printf '%-22s %s\n' "$1" "$2"; }

if cargo fmt --check >/dev/null 2>&1; then say "fmt" "ok"; else say "fmt" "FAIL"; fail=1; fi
lint="$(cargo clippy --offline --all-targets -- -D warnings 2>&1 | grep -cE '^error')"
if [ "$lint" = "0" ]; then say "clippy" "ok"; else say "clippy" "$lint errors"; fail=1; fi
# `cargo test` prints one summary per target; the last one is the doc-tests target, which
# usually runs nothing, so taking the last line reported "0 passed; 0 failed" while the unit
# tests had run. Sum every summary instead.
tests="$(cargo test --offline 2>&1 | grep -oE '[0-9]+ passed; [0-9]+ failed' \
  | awk '{passed += $1; failed += $3} END {printf "%d passed; %d failed", passed, failed}')"
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
# `compare.js` is what decides a fixture, and it needs `node`. Without it every fixture falls
# through to a stderr line that is neither `PASS` nor `FAIL`, which used to print as
# `0 passed, 0 failed` — a green-looking line for a stage that never judged anything.
# `all.sh` still runs without `node`: the hvolt side of every fixture is what the artifact pass
# below compares, and that half does not need it. Only the verdict is unavailable.
bash scripts/oracle/all.sh "${oracle_args[@]+${oracle_args[@]}}" > "$oracle_log" 2>&1
if command -v node >/dev/null 2>&1; then
  passed="$(grep -c '^PASS' "$oracle_log")"
  failed="$(grep -c '^FAIL' "$oracle_log")"
  # A fixture whose golden is missing is rebuilt by hvigor before it is compared. That step can
  # only run where `hvigorw` exists; anywhere else it prints `GOLDEN FAIL` and the fixture is
  # never judged — counting only `^FAIL` reported those as green (`139 GOLDEN FAIL` next to
  # `0 failed`).
  unbuilt="$(grep -c '^GOLDEN FAIL' "$oracle_log")"
  # `ACTUAL FAIL` is a fixture whose hvolt side produced nothing to compare. It was never
  # counted, so a build failure in the compiler under test looked like a clean run.
  noactual="$(grep -c '^ACTUAL FAIL' "$oracle_log")"
  say "oracle" "$passed passed, $failed failed, $unbuilt not compared, $noactual without actual"
  if [ "$failed" != "0" ] || [ "$unbuilt" != "0" ] || [ "$noactual" != "0" ]; then
    grep -E '^(FAIL|GOLDEN FAIL|ACTUAL FAIL)' "$oracle_log" | head -8
    fail=1
  fi
else
  say "oracle" "unavailable (node not found)"
  fail=1
fi
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
# 并发上限固定 3：这台机器是多人共享的 32 核，默认按 nproc 会把 CPU 打满
# （实测一分钟 load 冲到 14.95）。需要更多时显式给 VERIFY_JOBS。
artifact_jobs="${VERIFY_JOBS:-3}"
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

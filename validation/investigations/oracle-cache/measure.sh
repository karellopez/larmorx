#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The oracle cache's measurements (docs/validation/oracle-cache.md, "The proof, and the gain"):
# the smoke tier of every suite and one standard suite per family, before (the cache off) and
# after (cold: an empty cache; warm: the same again), wall times and original runs, and whether
# the records are the same. Run from a clean commit (the records name it).
# Usage: measure.sh <checkout> <out> [--jobs N] [--skip-smoke] [--skip-standard]
#   <out>/smoke-{off,cold,warm}/, <out>/std-{off,cold,warm}/ (records, logs, timings.tsv),
#   <out>/pytest-{off,warm}.log, <out>/steps.tsv (one line per step: name, seconds), the caches
#   <out>/cache-smoke and <out>/cache-standard, and same-*.txt (same_records.py).
R=$1; OUT=$2; shift 2
JOBS=6; SMOKE=1; STANDARD=1
while [ $# -gt 0 ]; do
  case $1 in
    --jobs) JOBS=$2; shift 2;;
    --skip-smoke) SMOKE=0; shift;;
    --skip-standard) STANDARD=0; shift;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
H=$(cd "$(dirname "$0")" && pwd)
D=$(cd "$H/../../../scripts/orchestration" && pwd)
mkdir -p "$OUT"
STEPS=$OUT/steps.tsv; [ -f "$STEPS" ] || printf "step\tseconds\n" > "$STEPS"
step() {  # step <name> <command...>: run it, append its wall time
  local name=$1; shift
  local start; start=$(date +%s.%N)
  "$@"
  local code=$?
  LC_ALL=C printf "%s\t%.1f\texit %s\n" "$name" "$(echo "$(date +%s.%N) - $start" | bc)" "$code" \
    | tee -a "$STEPS"
}
suites() { bash "$H/run_suites.sh" "$R" "$@"; }
confirm() { bash "$D/lxv.sh" "$R" oracle-cache confirm --jobs "$JOBS" "$@"; }
pytest_parity() {  # pytest_parity <mode> <log>
  (export PYTHONPATH=$R/python:$R/validation/src LARMORX_ORACLE_CACHE_MODE=$1; cd "$R" \
    && python -m pytest -q tests/parity) > "$2" 2>&1
}
same() { python -P "$H/same_records.py" "$@"; }
echo "commit $(git -C "$R" rev-parse --short=12 HEAD) dirty=[$(git -C "$R" status --porcelain --untracked-files=no | wc -l)] jobs $JOBS start $(date -Is)" | tee -a "$STEPS"

if [ $SMOKE = 1 ]; then
  export LARMORX_ORACLE_CACHE=$OUT/cache-smoke
  step smoke-off suites "$OUT/smoke-off" smoke off --jobs "$JOBS"
  step smoke-cold suites "$OUT/smoke-cold" smoke use --jobs "$JOBS"
  step smoke-confirm confirm
  step smoke-warm suites "$OUT/smoke-warm" smoke use --jobs "$JOBS"
  step pytest-off pytest_parity off "$OUT/pytest-off.log"
  step pytest-warm pytest_parity use "$OUT/pytest-warm.log"
  for d in records records-replica; do
    same "$OUT/smoke-off/$d" "$OUT/smoke-warm/$d" > "$OUT/same-smoke-off-warm-$d.txt"
    same "$OUT/smoke-off/$d" "$OUT/smoke-warm/$d" --addresses > "$OUT/same-smoke-off-warm-$d-addresses.txt"
    same "$OUT/smoke-off/$d" "$OUT/smoke-cold/$d" --addresses > "$OUT/same-smoke-off-cold-$d-addresses.txt"
  done
fi

if [ $STANDARD = 1 ]; then
  export LARMORX_ORACLE_CACHE=$OUT/cache-standard
  STD=(ants-registration ants-image-math afni-tshift afni-tshift:replica mri-hmc)
  step std-off suites "$OUT/std-off" standard off --jobs "$JOBS" "${STD[@]}"
  step std-cold suites "$OUT/std-cold" standard use --jobs "$JOBS" "${STD[@]}"
  step std-confirm confirm
  step std-warm suites "$OUT/std-warm" standard use --jobs "$JOBS" "${STD[@]}"
  for d in records records-replica; do
    same "$OUT/std-off/$d" "$OUT/std-warm/$d" > "$OUT/same-std-off-warm-$d.txt"
    same "$OUT/std-off/$d" "$OUT/std-warm/$d" --addresses > "$OUT/same-std-off-warm-$d-addresses.txt"
    same "$OUT/std-off/$d" "$OUT/std-cold/$d" --addresses > "$OUT/same-std-off-cold-$d-addresses.txt"
  done
fi
echo "ALLDONE $(date -Is)" | tee -a "$STEPS"

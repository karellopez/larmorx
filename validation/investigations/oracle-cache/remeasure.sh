#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The measurements made again after the fixes of 2026-10-25 (AFNI's key without Intel's KMP_
# variables; rewritten inputs replayed onto the run's input; stable names in the corrections
# suite): the parity smoke tier under pytest, warm, on measure.sh's smoke cache; the corrections
# suite's smoke tier cold and twice warm in a cache of its own.
# Usage: remeasure.sh <checkout> <measure.sh's out>
R=$1; OUT=$2
H=$(cd "$(dirname "$0")" && pwd)
STEPS=$OUT/steps-remeasure.tsv
printf "step\tseconds\n" > "$STEPS"
echo "commit $(git -C "$R" rev-parse --short=12 HEAD) dirty=[$(git -C "$R" status --porcelain --untracked-files=no | wc -l)] start $(date -Is)" | tee -a "$STEPS"
step() {
  local name=$1; shift
  local start; start=$(date +%s.%N)
  "$@"
  local code=$?
  LC_ALL=C printf "%s\t%.1f\texit %s\n" "$name" "$(echo "$(date +%s.%N) - $start" | bc)" "$code" \
    | tee -a "$STEPS"
}
pytest_parity() {
  (export PYTHONPATH=$R/python:$R/validation/src LARMORX_ORACLE_CACHE=$OUT/cache-smoke \
     LARMORX_ORACLE_CACHE_MODE=use; cd "$R" && python -m pytest -q --durations=40 tests/parity) > "$1" 2>&1
}
corrections() {
  LARMORX_ORACLE_CACHE=$OUT/cache-corrections bash "$H/run_suites.sh" "$R" "$1" smoke use --jobs 6 \
    ants-registration-corrections
}
step corrections-cold corrections "$OUT/corrections-cold"
step corrections-warm corrections "$OUT/corrections-warm"
step corrections-warm2 corrections "$OUT/corrections-warm2"
step pytest-warm2 pytest_parity "$OUT/pytest-warm2.log"
echo "ALLDONE $(date -Is)" | tee -a "$STEPS"

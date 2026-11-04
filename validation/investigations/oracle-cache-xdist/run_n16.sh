#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The validation package's own tests once on 16 pytest-xdist workers, in a checkout (one run;
# the failures differ between runs, so run it again to see others).
#   run_n16.sh <checkout> <log>
# The checkout's python/larmorx needs its _core (one test imports it; skipped without it).
set -uo pipefail
C=$(cd "$1" && pwd) || exit 1
LOG=$2
# shellcheck disable=SC1091
source "$C/scripts/orchestration/env.sh"
export VIRTUAL_ENV=$LARMORX_VENV PYTHONPATH="$C/python:$C/validation/src"
cd "$C" || exit 1
python -m pytest -q -p no:cacheprovider -n 16 validation/tests >"$LOG" 2>&1
status=$?
echo "exit $status" >>"$LOG"
grep -E '^FAILED|passed|failed' "$LOG"
exit $status

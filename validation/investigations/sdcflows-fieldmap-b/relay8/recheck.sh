#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The part's checks on one re-made fMRIPrep oracle run (the 2026-11-08 field-map runs, re-made
# with OPENBLAS_CORETYPE=Haswell), each boxed: check_bold_steps.py and check_bold_stage.py in both
# kept environments, check_bold_fmap.py's nodes, and check_pepolar.py's image section (PEPOLAR)
# or check_workflows.py's direct section (a direct field map).
#
# usage: LARMORX_WORKSPACE=<workspace> LARMORX_NC_BIN=<larmorx-nc> \
#        recheck.sh <full run name> [pepolar|direct|syn] > <workspace>/oracles/logs/<log> 2>&1
# The run counts only once <workspace>/oracles/logs/fmap-queue-haswell.log has "<run> exit 0".
# The log ends with "RECHECK-END <run>".
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
W=${LARMORX_WORKSPACE:?set LARMORX_WORKSPACE}
: "${LARMORX_NC_BIN:?set LARMORX_NC_BIN to the larmorx-nc program (the FSL replicas)}"
export LARMORX_NC_BIN LARMORX_WORKSPACE=$W
export OPENBLAS_CORETYPE=Haswell OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
export PYTHONPATH=$REPO/python:$REPO/validation/src
run=$1; kind=${2:-pepolar}
py() { if [ "$1" = shared ]; then echo "$W/.venv/bin/python"; else echo "$W/oracles/python-newest/bin/python"; fi; }
box() { bash "$REPO/scripts/orchestration/boxed.sh" 12 timeout 3600 "$@"; }
for env in shared newest; do
  echo "##### check_bold_steps $env"; box "$(py $env)" -P "$HERE/check_bold_steps.py" --runs "$run" --threads 6
  echo "##### check_bold_stage $env"; box "$(py $env)" -P "$HERE/check_bold_stage.py" --runs "$run" --threads 6
done
echo "##### check_bold_fmap nodes shared"
box "$(py shared)" -P "$HERE/check_bold_fmap.py" --sections nodes --runs "$run" --threads 6
if [ "$kind" = pepolar ]; then
  echo "##### check_pepolar image shared"
  box "$(py shared)" -P "$HERE/check_pepolar.py" --sections image --runs "$run" --threads 6
fi
if [ "$kind" = direct ]; then
  for env in shared newest; do
    echo "##### check_workflows direct $env"
    box "$(py $env)" -P "$HERE/check_workflows.py" --sections direct --threads 6
  done
fi
echo "RECHECK-END $run"

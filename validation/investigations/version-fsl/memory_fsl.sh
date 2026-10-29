#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The BET replicas' memory peaks against FSL's (tests/parity/test_memory_peak.py, the FSL calls),
# with this checkout's replica and python package, in a memory box; the FSL installation is the
# replicas' default (FSL 6.0.7.23) unless LARMORX_FSL_VERSION says otherwise.
# Usage: memory_fsl.sh <log>
set -u
log=$1
here=$(cd "$(dirname "$0")/../../.." && pwd)
export LARMORX_NC_BIN="$here/crates-nc/target/release/larmorx-nc"
export PYTHONPATH="$here/python:$here/validation/src"
export OPENBLAS_CORETYPE=Haswell
cd "$here" || exit 1
"$here/scripts/orchestration/boxed.sh" 6 "$HOME/VScode_projects/super_fmriprep/.venv/bin/python" \
  -m pytest -q tests/parity/test_memory_peak.py -k fsl -p no:cacheprovider >"$log" 2>&1
echo "MEMORY-EXIT $?" >>"$log"

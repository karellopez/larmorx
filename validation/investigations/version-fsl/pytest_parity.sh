#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The parity gate (`pytest tests/parity`, the smoke tier against the real oracles) of this
# checkout in a memory box, its last line "PARITY-EXIT <status>". Usage: pytest_parity.sh <log>
set -u
log=$1
here=$(cd "$(dirname "$0")/../../.." && pwd)
export PYTHONPATH="$here/python:$here/validation/src"
export OPENBLAS_CORETYPE=Haswell
export PATH="$HOME/VScode_projects/super_fmriprep/.venv/bin:/usr/bin:/bin"
cd "$here" || exit 1
"$here/scripts/orchestration/boxed.sh" 16 "$HOME/VScode_projects/super_fmriprep/.venv/bin/python" \
  -m pytest -q -p no:cacheprovider tests/parity >"$log" 2>&1
echo "PARITY-EXIT $?" >>"$log"

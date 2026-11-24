#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# seg2cc's recorded steps (STEPS, default the one of each recording) replayed on
# every recorded run (the classic and v8 streams of the six heads), three runs at a time, with
# the larmorx-freesurfer program in MODE (reproduce, fix), or FreeSurfer's own program with
# MODE=official; one JSON report and log per run in <work>/replay-<mode>/ (the replays' folders
# are removed after). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).
#
#   MODE=reproduce scripts/orchestration/boxed.sh 20 validation/probes/fs-scripts/replay.sh \
#       <larmorx-freesurfer> <work>
set -u
LXFS=$1; WORK=$2
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$REPO" || exit 1
source scripts/orchestration/env.sh >/dev/null 2>&1; unset FSLDIR
export PYTHONPATH=$REPO/python:$REPO/validation/src
RUNS=$(python -c "from larmorx_validation.oracle_cache.store import workspace; print(workspace())")/oracles/freesurfer-8.2.0/runs
mode=${MODE:-reproduce}; STEPS=${STEPS:-"seg2cc"}
mkdir -p "$WORK/replay-$mode"
one() {
  if [ "$mode" = official ]; then cmd=(); else cmd=(--command "$LXFS --upstream-defects $mode freesurfer"); fi
  # shellcheck disable=SC2086
  python -m larmorx_validation freesurfer-replay run "$1" $STEPS "${cmd[@]}" --work "$WORK/rw-$mode/$1" \
    --json "$WORK/replay-$mode/$1.json" > "$WORK/replay-$mode/$1.log" 2>&1
  echo "$1 rc=$?"
}
export -f one; export LXFS WORK mode STEPS
ls "$RUNS" | grep -E -- "-(classic|v8)$" | xargs -P 3 -I{} bash -c 'one {}'
echo "steps reproduced: $(cat "$WORK/replay-$mode"/*.log | grep -c ': REPRODUCED'), other: $(cat "$WORK/replay-$mode"/*.log | grep -c ': DIFFERENT\|: FAILED')"
rm -rf "${WORK:?}/rw-$mode"
echo REPLAY-END

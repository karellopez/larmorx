#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# numpy.linalg.lstsq against larmorx's lstsq in both Python environments: records tedana's own
# lstsq calls on fMRIPrep's three multi-echo t2smap calls (ds003592 sub-280; ds000210 sub-02's
# cuedSGT and rest runs, from the oracle runs' working directories), then runs check_lstsq.py
# (synthetic, edge and real cases) with each environment.
#   run_lstsq_check.sh <checker> <scratch dir>     (inside scripts/orchestration/boxed.sh)
set -u
[ -n "${RUN_LSTSQ_CHECK_GUARD:-}" ] && { echo "recursion guard" >&2; exit 3; }
export RUN_LSTSQ_CHECK_GUARD=1
export OPENBLAS_CORETYPE=Haswell
CHECKER=$1
SCR=$2
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
case "$REPO" in
  */.claude/worktrees/*) WS=${REPO%%/larmorx/.claude/worktrees/*} ;;
  *) WS=$(dirname "$REPO") ;;
esac
FP=$WS/oracles/fmriprep-25.2.6
declare -A CALL
W=$FP/ds003592-sub-280-noreconall/work/fmriprep_25_2_wf/sub_280_ses_1_wf/bold_ses_1_task_rest_echo_1_wf/bold_native_wf
CALL[ds003592]="-d $W/_echoidx_0/boldref_bold/sub-280_ses-1_task-rest_echo-1_bold_valid_tshift_xformresampled.nii.gz $W/_echoidx_1/boldref_bold/sub-280_ses-1_task-rest_echo-2_bold_valid_tshift_xformresampled.nii.gz $W/_echoidx_2/boldref_bold/sub-280_ses-1_task-rest_echo-3_bold_valid_tshift_xformresampled.nii.gz -e 14.0 29.96 45.92 --mask $W/bold_t2smap_wf/dilate_mask/dilated_mask.nii.gz"
for run in cuedSGT rest; do
  W=$FP/ds000210-sub-02-noreconall/work/fmriprep_25_2_wf/sub_02_wf/bold_task_${run}_run_01_echo_1_wf/bold_native_wf
  if [ "$run" = cuedSGT ]; then TE="13.0 27.0 43.0"; else TE="13.700000000000001 30.0 47.0"; fi
  CALL[ds000210-$run]="-d $W/_echoidx_0/boldref_bold/sub-02_task-${run}_run-01_echo-1_bold_tshift_xformresampled.nii.gz $W/_echoidx_1/boldref_bold/sub-02_task-${run}_run-01_echo-2_bold_tshift_xformresampled.nii.gz $W/_echoidx_2/boldref_bold/sub-02_task-${run}_run-01_echo-3_bold_tshift_xformresampled.nii.gz -e $TE --mask $W/bold_t2smap_wf/dilate_mask/dilated_mask.nii.gz"
done
for env in shared newest; do
  if [ $env = shared ]; then PY=$WS/.venv/bin/python; else PY=$WS/oracles/python-newest/bin/python; fi
  REAL=()
  for name in ds003592 ds000210-cuedSGT ds000210-rest; do
    D=$SCR/dump-$env/$name
    # shellcheck disable=SC2086
    "$PY" -P "$HERE/dump_lstsq_calls.py" --out "$D" --workdir "$SCR/t2smap-$env-$name" -- ${CALL[$name]} --fittype loglin > "$SCR/dump-$env-$name.log" 2>&1
    tail -1 "$SCR/dump-$env-$name.log"
    REAL+=(--real "$D")
  done
  echo "=== $env"
  "$PY" -P "$HERE/check_lstsq.py" --checker "$CHECKER" --work "$SCR/cases-$env" "${REAL[@]}"
done
echo RUN-LSTSQ-CHECK-DONE

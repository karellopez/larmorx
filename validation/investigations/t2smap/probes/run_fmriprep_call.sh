#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Probe (relay 1 inventory): tedana's t2smap on fMRIPrep's real call (ds003592), Haswell kernels.
# Usage: run_fmriprep_call.sh <python-env-bin-dir> <out-dir> <fittype> [extra t2smap args...]
set -u
[ -n "${T2SMAP_PROBE_GUARD:-}" ] && { echo "recursion guard" >&2; exit 3; }
export T2SMAP_PROBE_GUARD=1
export OPENBLAS_CORETYPE=Haswell
BIN=$1; OUT=$2; FIT=$3; shift 3
WS=$(cd "$(dirname "$0")/../../../../.." && pwd)  # the workspace
W=$WS/oracles/fmriprep-25.2.6/ds003592-sub-280-noreconall/work/fmriprep_25_2_wf/sub_280_ses_1_wf/bold_ses_1_task_rest_echo_1_wf/bold_native_wf
mkdir -p "$OUT"
cd "$OUT" || exit 4
/usr/bin/time -v "$BIN/t2smap" \
  -d "$W/_echoidx_0/boldref_bold/sub-280_ses-1_task-rest_echo-1_bold_valid_tshift_xformresampled.nii.gz" \
     "$W/_echoidx_1/boldref_bold/sub-280_ses-1_task-rest_echo-2_bold_valid_tshift_xformresampled.nii.gz" \
     "$W/_echoidx_2/boldref_bold/sub-280_ses-1_task-rest_echo-3_bold_valid_tshift_xformresampled.nii.gz" \
  -e 14.0 29.96 45.92 --mask "$W/bold_t2smap_wf/dilate_mask/dilated_mask.nii.gz" --fittype "$FIT" "$@"
echo "EXIT $?"

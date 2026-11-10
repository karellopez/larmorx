#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# brain_masker's random-walker intermediates of ds002603's magnitude node in the shared
# environment and in fMRIPrep 25.2.6's image (one thread, OpenBLAS's Haswell kernels), then the
# sums: the numpy 2.2 / 2.3 difference of whole-array sums.
# usage: LARMORX_WORKSPACE=<workspace> OUT=<scratch dir> bash probe_rw_steps.sh
set -u
W=$LARMORX_WORKSPACE
HERE=$(cd "$(dirname "$0")" && pwd)
R=$W/oracles/fmriprep-25.2.6
case=ds002603-sub-01-noreconall/work/fmriprep_25_2_wf/sub_01_wf/fmap_preproc_wf/wf_auto_00000/magnitude_wf/brainextraction_wf
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 OPENBLAS_CORETYPE=Haswell
mkdir -p "$OUT"
timeout 900 "$W/.venv/bin/python" -I "$HERE/probe_rw_steps.py" "$R/$case/clipper_post/clipped.nii.gz" "$OUT/shared.npz"
timeout 900 apptainer exec --cleanenv --env OMP_NUM_THREADS=1,OPENBLAS_NUM_THREADS=1,OPENBLAS_CORETYPE=Haswell \
  -B "$OUT":/o -B "$R":/r -B "$HERE":/h "$W/oracles/images/fmriprep-25.2.6.sif" \
  python -I /h/probe_rw_steps.py "/r/$case/clipper_post/clipped.nii.gz" /o/image.npz
"$W/.venv/bin/python" -I "$HERE/probe_sum.py" "$OUT/shared.npz"
timeout 300 apptainer exec --cleanenv -B "$OUT":/o -B "$HERE":/h "$W/oracles/images/fmriprep-25.2.6.sif" python -I /h/probe_sum.py /o/shared.npz

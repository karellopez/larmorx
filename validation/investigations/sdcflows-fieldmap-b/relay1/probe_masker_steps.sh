#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# probe_masker_steps.py on ds005040's PEPOLAR reference (fMRIPrep's masker input) in the shared and
# the newest environments.
S=${OUT:?set OUT to an output folder}
W=${LARMORX_WORKSPACE:?}
IN=$W/oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall/work/fmriprep_25_2_wf/sub_002_wf/fmap_preproc_wf/wf_auto_00000/brainextraction_wf/clipper_post/clipped.nii.gz
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 OPENBLAS_CORETYPE=Haswell
for py in "$W/.venv/bin/python" "$W/oracles/python-newest/bin/python"; do
  timeout 600 "$py" -I "$S/scripts/probe_masker_steps.py" "$IN" 2>&1 | grep -v -i warn
done

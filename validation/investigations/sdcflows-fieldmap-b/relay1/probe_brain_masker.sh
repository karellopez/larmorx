#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# brain_masker on fMRIPrep's masker inputs (ds002603: 128^3; ds005454: 112x112x70; ds005040: PEPOLAR
# reference), in the newest environment (one thread) and in fMRIPrep's image.
S=${OUT:?set OUT to an output folder}
W=${LARMORX_WORKSPACE:?}
R=$W/oracles/fmriprep-25.2.6
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 OPENBLAS_CORETYPE=Haswell
for case in \
  "ds002603-sub-01-noreconall/work/fmriprep_25_2_wf/sub_01_wf/fmap_preproc_wf/wf_auto_00000/magnitude_wf/brainextraction_wf" \
  "ds005454-sub-16-noreconall/work/fmriprep_25_2_wf/sub_16_wf/fmap_preproc_wf/wf_gre16/magnitude_wf/brainextraction_wf" \
  "ds005040-sub-002-noreconall/work/fmriprep_25_2_wf/sub_002_wf/fmap_preproc_wf/wf_auto_00000/brainextraction_wf"; do
  name=$(echo "$case" | cut -d/ -f1)
  echo "== $name (newest env)"
  timeout 600 "$W/oracles/python-newest/bin/python" -I "$S/scripts/probe_brain_masker.py" \
    "$R/$case/clipper_post/clipped.nii.gz" "$R/$case/masker" "$S/probe-masker/newest/$name"
  echo "== $name (fMRIPrep image)"
  timeout 600 apptainer exec --cleanenv --env OMP_NUM_THREADS=1,OPENBLAS_NUM_THREADS=1,OPENBLAS_CORETYPE=Haswell \
    -B "$S":/s -B "$R":/r "$W/oracles/images/fmriprep-25.2.6.sif" python -I /s/scripts/probe_brain_masker.py \
    "/r/$case/clipper_post/clipped.nii.gz" "/r/$case/masker" "/s/probe-masker/image/$name"
done

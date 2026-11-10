#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Print library versions inside the fMRIPrep 25.2.6 image.
S=${OUT:?set OUT to an output folder}
IMG=${LARMORX_WORKSPACE:?}/oracles/images/fmriprep-25.2.6.sif
timeout 300 apptainer exec --cleanenv -B "$S":/s "$IMG" python -I /s/versions.py 2>&1 | tail -25
timeout 300 apptainer exec --cleanenv "$IMG" bash -c 'python -c "import sdcflows,niworkflows,fmriprep;print(sdcflows.__file__, niworkflows.__file__, fmriprep.__file__)"; which 3dQwarp mri_robust_template antsRegistration topup 3dvolreg N4BiasFieldCorrection DenoiseImage antsApplyTransforms 3drefit 3dresample fslmerge 2>&1'

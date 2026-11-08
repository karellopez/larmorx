#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copy fMRIPrep 25.2.6's, niworkflows 1.15.2's, smriprep's and nipype's Python sources out of the
# fMRIPrep image into the scratch folder (read-only reference for the inventory).
S=${OUT:?set OUT to an output folder}
IMG=${LARMORX_WORKSPACE:?}/oracles/images/fmriprep-25.2.6.sif
mkdir -p "$S/image-src"
timeout 600 apptainer exec --cleanenv -B "$S/image-src":/out "$IMG" bash -c '
SP=/app/.pixi/envs/fmriprep/lib/python3.12/site-packages
for p in fmriprep niworkflows smriprep sdcflows; do
  cp -r $SP/$p /out/ 2>&1 | tail -2
done
mkdir -p /out/nipype_interfaces
cp -r $SP/nipype/interfaces/fsl/epi.py $SP/nipype/interfaces/afni/preprocess.py $SP/nipype/interfaces/ants /out/nipype_interfaces/ 2>&1 | tail -2
ls /out
'
find "$S/image-src" -name '*.pyc' -delete
du -sh "$S/image-src"

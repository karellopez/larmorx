#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copy scikit-image 0.25.2's Python sources (the fMRIPrep image's) into the scratch folder.
S=${OUT:?set OUT to an output folder}
IMG=${LARMORX_WORKSPACE:?}/oracles/images/fmriprep-25.2.6.sif
mkdir -p "$S/image-src/skimage-0.25.2" "$S/image-src/nipype-algorithms"
timeout 600 apptainer exec --cleanenv -B "$S/image-src":/out "$IMG" bash -c '
SP=/app/.pixi/envs/fmriprep/lib/python3.12/site-packages
cd $SP/skimage && find . -name "*.py" | while read f; do mkdir -p "/out/skimage-0.25.2/$(dirname $f)"; cp "$f" "/out/skimage-0.25.2/$f"; done
cp $SP/nipype/algorithms/confounds.py /out/nipype-algorithms/
ls $SP | grep -i -E "^(scipy|numpy|nilearn|nitransforms|nibabel|templateflow)-"
'

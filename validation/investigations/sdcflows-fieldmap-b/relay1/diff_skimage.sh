#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Diff the scikit-image functions part B reaches: 0.25.2 (fMRIPrep's image) against 0.26.0 (newest).
O=${OUT:?set OUT to an output folder}/image-src/skimage-0.25.2
N=${LARMORX_WORKSPACE:?}/tags/scikit-image-0.26.0/src/skimage
for f in morphology/footprints.py filters/thresholding.py segmentation/random_walker_segmentation.py exposure/histogram_matching.py exposure/exposure.py util/dtype.py _shared/compat.py _shared/utils.py; do
  echo "== $f"
  diff -u "$O/$f" "$N/$f" | grep '^[-+]' | grep -v '^[-+][-+]' | head -40
done

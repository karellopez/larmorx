# SPDX-License-Identifier: Apache-2.0
"""cmpmask.py <a.nii.gz> <b.nii.gz>...: voxels differing and Dice of each image against the first."""

import sys

import nibabel as nib
import numpy as np

ref = np.asanyarray(nib.load(sys.argv[1]).dataobj)
for p in sys.argv[2:]:
    b = np.asanyarray(nib.load(p).dataobj)
    diff = int(np.count_nonzero(ref != b))
    ra, rb = ref > 0, b > 0
    dice = 2 * np.count_nonzero(ra & rb) / max(1, np.count_nonzero(ra) + np.count_nonzero(rb))
    print(f"{p}: {diff} voxels differ, Dice {dice:.6f}")

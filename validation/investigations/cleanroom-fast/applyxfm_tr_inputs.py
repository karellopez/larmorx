# SPDX-License-Identifier: Apache-2.0
"""Two small 3D images for FLIRT's -applyxfm TR probe: in.nii.gz (pixdim[4] 2.5, pixdim[5..7]
0) and ref.nii.gz (pixdim[4] 1, pixdim[5..7] 1), a 4D input in4d.nii.gz (TR 3), the identity."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

out = Path(sys.argv[1])
rng = np.random.default_rng(7)
a = (rng.random((20, 18, 16)) * 100).astype(np.float32)
i = nib.Nifti1Image(a, np.diag([2.0, 2.0, 2.0, 1.0]))
i.header["pixdim"][4:8] = [2.5, 0, 0, 0]
nib.save(i, out / "in.nii.gz")
r = nib.Nifti1Image(
    (rng.random((24, 22, 20)) * 100).astype(np.float32), np.diag([1.7, 1.7, 1.7, 1.0])
)
r.header["pixdim"][4:8] = [1.0, 1.0, 1.0, 1.0]
nib.save(r, out / "ref.nii.gz")
f = nib.Nifti1Image(np.stack([a, a * 0.5], -1), np.diag([2.0, 2.0, 2.0, 1.0]))
f.header["pixdim"][4:8] = [3.0, 0, 0, 0]
nib.save(f, out / "in4d.nii.gz")
np.savetxt(out / "ident.mat", np.eye(4), fmt="%.6f")

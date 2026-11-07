# SPDX-License-Identifier: Apache-2.0
"""Variants of <folder>/s1.nii.gz (make_series.py) for batch.py: other orientations, types and
scalings, an oblique and a sagittal grid, a 3D base, a weight, bytes.

Usage: variants.py <folder>
"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

d = Path(sys.argv[1])
img = nib.load(d / "s1.nii.gz")
data = np.asarray(img.dataobj, dtype=np.float32)
aff = img.affine

# LPI (positive diagonal), int16 data
a2 = np.diag([3.0, 3.0, 3.5, 1.0])
a2[:3, 3] = [-60, -66, -40]
i2 = nib.Nifti1Image(np.round(data).astype(np.int16), a2)
i2.header.set_xyzt_units("mm", "sec")
i2.set_qform(a2, 1)
i2.set_sform(a2, 1)
i2.to_filename(d / "s2_short.nii.gz")

# int16 with a slope (scale factors)
i2b = nib.Nifti1Image(np.round(data * 4).astype(np.int16), a2)
i2b.header.set_slope_inter(0.25, 0)
i2b.set_qform(a2, 1)
i2b.set_sform(a2, 1)
i2b.to_filename(d / "s2_scaled.nii.gz")

# Oblique, odd sizes (crop to 39 x 43 x 23), a permuted axis order (z first)
c = data[:39, :43, :23, :]
th = np.deg2rad(8)
r = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
a3 = np.eye(4)
a3[:3, :3] = r @ np.diag([2.5, -3.0, 3.5])
a3[:3, 3] = [-50, 60, -30]
i3 = nib.Nifti1Image(c, a3)
i3.set_qform(a3, 1)
i3.set_sform(a3, 1)
i3.to_filename(d / "s3_oblique.nii.gz")

# Sagittal acquisition: axes permuted (i = A-P, j = S-I, k = L-R)
p = np.transpose(data, (1, 2, 0, 3))
a4 = np.zeros((4, 4))
a4[1, 0] = -3.0  # i -> -y (A to P)
a4[2, 1] = 3.5  # j -> +z
a4[0, 2] = 3.0  # k -> +x
a4[3, 3] = 1
a4[:3, 3] = [-60, 66, -40]
i4 = nib.Nifti1Image(np.ascontiguousarray(p), a4)
i4.set_qform(a4, 1)
i4.set_sform(a4, 1)
i4.to_filename(d / "s4_sagittal.nii.gz")

# A 3D external base (volume 2) and a weight dataset
nib.Nifti1Image(data[..., 2], aff).to_filename(d / "base3d.nii.gz")
w = np.abs(data[..., :2]).astype(np.float32)
nib.Nifti1Image(w, aff).to_filename(d / "weight.nii.gz")
# byte data
i5 = nib.Nifti1Image(np.clip(data / 6, 0, 255).astype(np.uint8), aff)
i5.to_filename(d / "s5_byte.nii.gz")
print("ok")

# SPDX-License-Identifier: Apache-2.0
"""A synthetic moving series for 3dvolreg (an ellipsoid with texture, rotated 0.6 degrees and
shifted per volume, with noise).

Usage: make_series.py <out.nii.gz> [nx ny nz nt [dtype]]
"""

import sys

import nibabel as nib
import numpy as np
from scipy import ndimage

out = sys.argv[1]
nx, ny, nz, nt = (int(v) for v in sys.argv[2:6]) if len(sys.argv) > 5 else (40, 44, 24, 6)
dtype = sys.argv[6] if len(sys.argv) > 6 else "float32"
rng = np.random.default_rng(7)
x, y, z = np.meshgrid(
    np.arange(nx) - nx / 2, np.arange(ny) - ny / 2, np.arange(nz) - nz / 2, indexing="ij"
)
r = (x / (nx * 0.35)) ** 2 + (y / (ny * 0.38)) ** 2 + (z / (nz * 0.4)) ** 2
base = np.where(r < 1, 800 + 300 * np.cos(x / 3.0) * np.sin(y / 4.0) + 50 * z, 0.0)
base += rng.normal(0, 10, base.shape) * (r < 1.2)
base = ndimage.gaussian_filter(base, 0.7)
vols = []
for t in range(nt):
    ang = np.deg2rad(0.6 * t) if t else 0.0
    c, s = np.cos(ang), np.sin(ang)
    m = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1.0]])
    centre = np.array([nx, ny, nz]) / 2
    shift = np.array([0.3 * t, -0.2 * t, 0.15 * t])
    off = centre - m @ centre + shift
    v = ndimage.affine_transform(base, m, offset=off, order=3)
    vols.append(v + rng.normal(0, 5, v.shape))
data = np.stack(vols, axis=-1).astype(dtype)
aff = np.diag([-3.0, -3.0, 3.5, 1.0])
aff[:3, 3] = [60, 66, -40]
img = nib.Nifti1Image(data, aff)
img.header.set_xyzt_units("mm", "sec")
img.header["pixdim"][4] = 2.0
img.set_qform(aff, 1)
img.set_sform(aff, 1)
img.to_filename(out)
print(out, data.shape, data.dtype)

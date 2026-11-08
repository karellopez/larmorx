# SPDX-License-Identifier: Apache-2.0
"""``np.asanyarray(img.dataobj)`` applies a NIfTI's scl_slope/inter (nibabel's ArrayProxy), so
BSplineApprox's mask ``dataobj > 1e-4`` uses scaled values; numpy's sum of -0.0 starts from
+0; median and mean dtypes.  Run: ``python -P probe_mask_scaling.py``."""

import os
import tempfile

import nibabel as nb
import numpy as np

d = tempfile.mkdtemp()
a = np.zeros((4, 4, 4), np.int16)
a[1, 1, 1] = 1
a[2, 2, 2] = 3
img = nb.Nifti1Image(a, np.eye(4))
img.header.set_slope_inter(1e-5, 0)
p = os.path.join(d, "m.nii")
img.to_filename(p)
m = nb.load(p)
print(
    "dataobj scaled:",
    np.asanyarray(m.dataobj)[1, 1, 1],
    np.asanyarray(m.dataobj).dtype,
    "mask count",
    (np.asanyarray(m.dataobj) > 1e-4).sum(),
)
print("sum(-0.0):", np.array([-0.0], np.float32).sum(), np.array([-0.0, -0.0], np.float32).sum())
print(
    "median",
    np.median(np.array([1, 2, 3, 4], np.float32)).dtype,
    np.median(np.array([], np.float32)),
)
print("mean dtype", np.mean(np.arange(10, dtype=np.float32)).dtype)

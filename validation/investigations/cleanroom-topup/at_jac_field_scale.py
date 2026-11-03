# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage
d = Path(sys.argv[1]); name = sys.argv[2]
want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)
img = nib.load(str(d/name)); f = np.asarray(img.dataobj, np.float64)
vox = np.abs(np.asarray(img.header.get_zooms()[:3], float))
h = np.asarray(nib.load(str(d/"tu_field.nii.gz")).dataobj, np.float64)
t = 0.0137951
n = f.shape
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), 0)
def eval_deriv(co, axis):
    out = co
    for a in range(3):
        k = np.array([-0.5, 0, 0.5]) if a == axis else np.array([1/6, 4/6, 1/6])
        out = ndimage.correlate1d(out, k, axis=a, mode="constant", cval=0.0)
    return out
for ps in (1.0, vox[0]/vox[1]):
  for js in (1.0, vox[0]/vox[1]):
    P = X.copy(); P[1] += h*t*ps
    v = ndimage.map_coordinates(f, P, order=1, mode="grid-wrap")
    J = 1 + eval_deriv(ndimage.spline_filter(h*t*js, order=3, mode="mirror"), 1)
    g = v*J
    e = np.abs(g-want)/(np.abs(want)+1)
    print("pos scale", ps, "jac scale", js, "p50 %.3g p99 %.3g max %.3g" % tuple(np.percentile(e,[50,99,100])))

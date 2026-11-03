# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage

d = Path(sys.argv[1])
name = sys.argv[2]
want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
img = nib.load(str(d / name))
f = np.asarray(img.dataobj, np.float64)
vox = np.abs(np.asarray(img.header.get_zooms()[:3], float))
h = np.asarray(nib.load(str(d / "tu_field.nii.gz")).dataobj, np.float64)
t = 0.0137951
n = f.shape
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), 0)
P = X.copy()
P[1] += h * t
v = ndimage.map_coordinates(f, P, order=1, mode="grid-wrap")
ok = np.abs(v) > 50
Jf = np.full(n, np.nan)
Jf[ok] = want[ok] / v[ok]


def eval_deriv(co, axis, em):
    k_v = np.array([1 / 6, 4 / 6, 1 / 6])
    k_d = np.array([-0.5, 0, 0.5])
    out = co
    for a in range(3):
        k = k_d if a == axis else k_v
        out = ndimage.correlate1d(out, k, axis=a, mode=em, cval=0.0)
    return out


for scale in ("vox", "mmy", "mmx"):
    s = {"vox": 1, "mmy": vox[1], "mmx": vox[0]}[scale]
    D = h * t * s
    for pm in ("mirror", "grid-wrap", "nearest", "grid-constant"):
        co = ndimage.spline_filter(D, order=3, mode=pm)
        for em in ("constant", "wrap", "mirror", "nearest"):
            for div in (1, vox[0], vox[1]):
                J = 1 + eval_deriv(co, 1, em) / div
                e = np.abs(J - Jf)[ok]
                if np.percentile(e, 99) < 1e-3:
                    print(
                        scale,
                        pm,
                        em,
                        div,
                        "p50 %.3g p99 %.3g max %.3g" % tuple(np.percentile(e, [50, 99, 100])),
                    )

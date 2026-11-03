# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage
sys.path.insert(0, str(Path(__file__).parent))
from at_field import field
d = Path(sys.argv[1])
want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)
f = np.asarray(nib.load(str(d/"half_up_64x16.nii.gz")).dataobj, np.float64)
c = nib.load(str(d/"tu_fieldcoef.nii.gz")); h = field(c); dh = field(c, 1)
t = 0.0137951
n = f.shape; m = (32, 32, 20)
re = lambda a, s: a.reshape(-1, order="F").reshape(s, order="F")
# hypothesis A: output = jac of the image on its own grid using the field laid (h laid onto 64x16), then mask laid?
# hypothesis B: the image re-laid on the field grid (64x16 -> 32x32), corrected there, laid back
f32 = re(f, m)
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in m], indexing="ij"), 0)
P = X.copy(); P[1] += h*t
for order in (1, 3):
    g = re(ndimage.map_coordinates(f32, P, order=order, mode="grid-wrap")*(1 + t*dh), n)
    mm = want != 0
    e = np.abs(g-want)/(np.abs(want)+1)
    print("B", order, "on FSL nonzero p50 %.3g p99 %.3g" % tuple(np.percentile(e[mm],[50,99])))
# hypothesis C: interpolation on image's own grid, using laid field; positions: y + d where image y has 16 rows; with x>=32 invalid?
X2 = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), 0)
for order in (1,3):
    for fsrc in ("laid", "eval"):
        H = re(h, n) if fsrc == "laid" else field(c, None, n=list(n))
        DH = re(dh, n) if fsrc == "laid" else field(c, 1, n=list(n))
        P = X2.copy(); P[1] += H*t
        g = ndimage.map_coordinates(f, P, order=order, mode="grid-wrap")*(1+t*DH)
        mm = want != 0
        e = np.abs(g-want)/(np.abs(want)+1)
        print("C", order, fsrc, "p50 %.3g p99 %.3g" % tuple(np.percentile(e[mm],[50,99])))

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, name):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{name}").dataobj, float)


def basis_o(n, k, off, nc):
    xs = np.arange(n).astype(float)
    return tp.bspl((xs[:, None] - (off + np.arange(nc)[None, :] * k)) / k)


def rf_B(B, lam=0.005):
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


p = sys.argv[1]
k2 = int(sys.argv[2])
f1 = load(p, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load(p, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], 2)) for a in range(3)])
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], 2, 0, pos[a]) for a in range(3)])
B2 = [tp.basis(n2[a], k2) for a in range(3)]
M2 = [rf_B(b) for b in B2]
res = []
for off in (-4, -3.5, -3, -2.5, -2, -1.5, -1, 0):
    for extra in (-1, 0, 1, 2):
        Bs = []
        for a in range(3):
            nc = tp.ncoef(n2[a], 3) + extra
            Bs.append(basis_o(n2[a], 3, off, nc))
        f = tp.apply(tp.apply(fs, [rf_B(b) for b in Bs]), Bs)
        f = tp.apply(tp.apply(f, M2), B2)
        res.append((float(np.sqrt(((f - f2) ** 2).mean())), off, extra))
res.sort()
print(res[:3])
from scipy import ndimage as ndi

K1 = int(sys.argv[3]) if len(sys.argv) > 3 else 3
B1 = [tp.basis(n2[a], K1) for a in range(3)]
M1 = [rf_B(b) for b in B1]
grid = np.meshgrid(*pos, indexing="ij")
for nm, fsx in [("bspline", fs)] + [
    (f"img order{o} {m}", ndi.map_coordinates(f1, grid, order=o, mode=m))
    for o in (1, 3)
    for m in ("nearest", "mirror", "reflect", "grid-wrap")
]:
    f = tp.apply(tp.apply(fsx, M1), B1)
    if k2 != K1:
        f = tp.apply(tp.apply(f, M2), B2)
    print(nm, "rms", float(np.sqrt(((f - f2) ** 2).mean())))
# box average of the old spline over each new voxel's extent
sub = np.linspace(-0.25, 0.25, 11)
acc = 0
for dxs in sub:
    for dys in sub:
        for dzs in sub:
            pp = [-0.25 + np.arange(n2[a]) * 0.5 + d * 1.0 for a, d in enumerate((dxs, dys, dzs))]
            acc = acc + tp.apply(c1, [tp.basis(n1[a], 2, 0, pp[a]) for a in range(3)])
fsx = acc / len(sub) ** 3
f = tp.apply(tp.apply(fsx, M1), B1)
if k2 != K1:
    f = tp.apply(tp.apply(f, M2), B2)
print("box-average", "rms", float(np.sqrt(((f - f2) ** 2).mean())))

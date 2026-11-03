# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Transition hypotheses judged by both the in-grid rms and FSL's bending energy (probe scg)."""

import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]
f1 = np.asarray(
    nib.load(f"{R}/fsl-probe-scg/TopupDebugFieldlevel01_iter03_attempt01.nii.gz").dataobj, float
)
f2 = np.asarray(
    nib.load(f"{R}/fsl-probe-scg/TopupDebugFieldlevel02_iter00_attempt01.nii.gz").dataobj, float
)
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
vox = np.array([6.0, 6.0, 6.5999985])


def E(c, n, k):
    G = {}
    for a in range(3):
        nc = tp.ncoef(n[a], k)
        lo = tp.offset(k) - 2 * k
        hi = tp.offset(k) + (nc - 1) * k + 2 * k
        xs = np.arange(lo, hi + 1)
        for d in range(3):
            b = tp.basis(n[a], k, d, xs) / vox[a] ** d
            G[(a, d)] = b.T @ b
    terms = [
        ((2, 0, 0), 1),
        ((0, 2, 0), 1),
        ((0, 0, 2), 1),
        ((1, 1, 0), 2),
        ((1, 0, 1), 2),
        ((0, 1, 1), 2),
    ]
    return sum(
        wt * float((c * tp.apply(c, [G[(a, ds[a])] for a in range(3)])).sum()) for ds, wt in terms
    )


def fit(n, k, xs, lam=0.005):
    B = tp.basis(n, k, 0, xs)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], 2)) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(tp.basis(n2[a], 2)) for a in range(3)])
print("FSL E", E(c2, n2, 2))
B2 = [tp.basis(n2[a], 2) for a in range(3)]
# coefficients copied onto the new grid with twice the knot spacing
print("ncoef old", c1.shape, "new k=4", [tp.ncoef(n2[a], 4) for a in range(3)])
cc = c1
f = tp.apply(cc, [tp.basis(n2[a], 4) for a in range(3)])
print("copy k=4 field rms vs FSL", np.sqrt(((f - f2) ** 2).mean()))
for path in ((2,), (3, 2)):
    g = f
    for kk in path:
        c = tp.apply(g, [fit(n2[a], kk, np.arange(n2[a]).astype(float)) for a in range(3)])
        g = tp.apply(c, [tp.basis(n2[a], kk) for a in range(3)])
    print("copy then", path, "E %.1f" % E(c, n2, 2), "rms %.4f" % np.sqrt(((g - f2) ** 2).mean()))
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], 2, 0, pos[a]) for a in range(3)])
xs0 = [np.arange(n2[a]).astype(float) for a in range(3)]
c = tp.apply(fs, [fit(n2[a], 3, xs0[a]) for a in range(3)])
g = tp.apply(c, [tp.basis(n2[a], 3) for a in range(3)])
cm = tp.apply(g, [fit(n2[a], 2, xs0[a]) for a in range(3)])
np.set_printoptions(precision=2, suppress=True, linewidth=250)
for j in (5, 9):
    print("x-line y=%d z=6 FSL" % j, c2[:, j, 6])
    print("              mine", cm[:, j, 6])
print("z-line FSL", c2[9, 9, :])
print("z-line mine", cm[9, 9, :])
d = np.abs(c2 - cm)
print("coef |d| max by x index", np.round(d.max((1, 2)), 2))
print("by z", np.round(d.max((0, 1)), 2))
for K1 in ():
    for m in (0, 1, 2, 3, 4, 6, 8):
        xs = [np.arange(-m, n2[a] + m).astype(float) for a in range(3)]
        old = [-0.25 + x * 0.5 for x in xs]
        fs = tp.apply(c1, [tp.basis(n1[a], 2, 0, old[a]) for a in range(3)])
        c = tp.apply(fs, [fit(n2[a], K1, xs[a]) for a in range(3)])
        if K1 != 2:
            f = tp.apply(c, [tp.basis(n2[a], K1, 0, xs[a]) for a in range(3)])
            c = tp.apply(f, [fit(n2[a], 2, xs[a]) for a in range(3)])
        f = tp.apply(c, B2)
        print(
            "K1",
            K1,
            "extend",
            m,
            "E %.1f" % E(c, n2, 2),
            "rms %.4f" % np.sqrt(((f - f2) ** 2).mean()),
            "coef rms %.3f" % np.sqrt(((c - c2) ** 2).mean()),
        )

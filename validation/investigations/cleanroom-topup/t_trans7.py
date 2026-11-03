# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Bending energy of FSL's level-2 start field vs transition models; my Rust result's fieldcoef."""

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


def rf(n, k, lam=0.005):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


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


c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], 2)) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(tp.basis(n2[a], 2)) for a in range(3)])
print("FSL L2 start E", E(c2, n2, 2), "(Reg/(lam*SSD) =", 5.19953 / (1e-4 * 193.788), ")")
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], 2, 0, pos[a]) for a in range(3)])
B2 = [tp.basis(n2[a], 2) for a in range(3)]
for K1 in (2, 3):
    for l1 in (0.005, 0.05, 0.2, 0.5, 1.0):
        for l2 in (0.005, 0.05, 0.2, 0.5, 1.0) if K1 != 2 else (None,):
            c = tp.apply(fs, [rf(n2[a], K1, l1) for a in range(3)])
            if K1 != 2:
                f = tp.apply(c, [tp.basis(n2[a], K1) for a in range(3)])
                c = tp.apply(f, [rf(n2[a], 2, l2) for a in range(3)])
            f = tp.apply(c, B2)
            print(
                "K1",
                K1,
                "l1",
                l1,
                "l2",
                l2,
                "E %.1f" % E(c, n2, 2),
                "rms %.4f" % np.sqrt(((f - f2) ** 2).mean()),
            )

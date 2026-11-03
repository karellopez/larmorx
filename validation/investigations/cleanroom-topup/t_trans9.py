# SPDX-License-Identifier: Apache-2.0
"""Probe B (k 2 -> 1, same grid, voxels 12 mm): FSL's energy at level 2 vs the refit's."""

import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]
p = "B"
f1 = np.asarray(
    nib.load(f"{R}/fsl-probe-{p}/TopupDebugFieldlevel01_iter03_attempt01.nii.gz").dataobj, float
)
f2 = np.asarray(
    nib.load(f"{R}/fsl-probe-{p}/TopupDebugFieldlevel02_iter00_attempt01.nii.gz").dataobj, float
)
n = np.array(f1.shape)
vox = np.array([12.0, 12.0, 13.199997])


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


def fit(n, k, lam=0.005):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


c2 = tp.apply(f2, [np.linalg.inv(tp.basis(n[a], 1)) for a in range(3)])
print("B: FSL E", E(c2, n, 1), "Reg/(lam*ssd)", 0.452516 / (1e-4 * 75.6761))
for lam in (0.0, 0.005, 0.05):
    c = tp.apply(f1, [fit(n[a], 1, lam) for a in range(3)])
    print(
        "B: refit lam",
        lam,
        "E",
        E(c, n, 1),
        "rms",
        np.sqrt(((tp.apply(c, [tp.basis(n[a], 1) for a in range(3)]) - f2) ** 2).mean()),
    )

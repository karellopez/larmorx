# SPDX-License-Identifier: Apache-2.0
"""Refit hypotheses in which the samples cover the new coefficients' span (knots, or every voxel
of the span) — probe scg's subsampling change and probe B's knot change."""

import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


def E(c, n, k, vox):
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


def fit_at(n, k, xs, lam=0.005):
    B = tp.basis(n, k, 0, xs)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


def span(n, k, how):
    nc = tp.ncoef(n, k)
    if how == "knots":
        return tp.offset(k) + np.arange(nc) * k
    if how == "span":
        return np.arange(int(tp.offset(k)), int(tp.offset(k) + (nc - 1) * k) + 1).astype(float)
    return np.arange(n).astype(float)


def step(c_old, n_old, k_old, n_new, k_new, posmap, how, lam):
    xs = [span(n_new[a], k_new, how) for a in range(3)]
    vals = tp.apply(
        c_old,
        [tp.basis(n_old[a], k_old[a], 0, posmap(xs[a]) if posmap else xs[a]) for a in range(3)],
    )
    return tp.apply(vals, [fit_at(n_new[a], k_new, xs[a], lam) for a in range(3)])


# probe scg: 16 -> 32 (K1 = 3), then k 3 -> 2
f1 = load("scg", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("scg", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
vox = np.array([6.0, 6.0, 6.5999985])
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], 2)) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(tp.basis(n2[a], 2)) for a in range(3)])
print("scg FSL E", round(E(c2, n2, 2, vox), 2))
B2 = [tp.basis(n2[a], 2) for a in range(3)]
for h1 in ("voxels", "knots", "span"):
    for h2 in ("voxels", "knots", "span"):
        for lam in (0.005, 0.0):
            cm = step(c1, n1, [2, 2, 2], n2, 3, lambda x: -0.25 + 0.5 * x, h1, lam)
            cm = step(cm, n2, [3, 3, 3], n2, 2, None, h2, lam)
            f = tp.apply(cm, B2)
            print(
                f"  {h1:6s} {h2:6s} lam {lam}: E {E(cm, n2, 2, vox):8.2f} rms {np.sqrt(((f - f2) ** 2).mean()):.4f} coef rms {np.sqrt(((cm - c2) ** 2).mean()):.4f}"
            )
# probe B: k 2 -> 1 on 16
f1 = load("B", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("B", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n = np.array(f1.shape)
vox = np.array([12.0, 12.0, 13.199997])
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n[a], 2)) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.inv(tp.basis(n[a], 1)) for a in range(3)])
print("B FSL E", round(E(c2, n, 1, vox), 3))
for h in ("voxels", "knots", "span"):
    cm = step(c1, n, [2, 2, 2], n, 1, None, h, 0.005)
    f = tp.apply(cm, [tp.basis(n[a], 1) for a in range(3)])
    print(f"  {h}: E {E(cm, n, 1, vox):.3f} rms {np.sqrt(((f - f2) ** 2).mean()):.5f}")

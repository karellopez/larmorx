# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Refits regularised by the 1D bending energy over the support (probe scg and B)."""

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

exec(
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "t_trans10.py"))
    .read()
    .split("# probe scg")[0]
    .split("import tp\n", 1)[1]
)


def g2(n, k, d=2):
    nc = tp.ncoef(n, k)
    lo = tp.offset(k) - 2 * k
    hi = tp.offset(k) + (nc - 1) * k + 2 * k
    b = tp.basis(n, k, d, np.arange(lo, hi + 1))
    return b.T @ b


def fit_be(n, k, mu, d=2):
    B = tp.basis(n, k)
    return np.linalg.solve(B.T @ B + mu * g2(n, k, d), B.T)


f1 = load("scg", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("scg", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
vox = np.array([6.0, 6.0, 6.5999985])
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], 2)) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(tp.basis(n2[a], 2)) for a in range(3)])
B2 = [tp.basis(n2[a], 2) for a in range(3)]
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], 2, 0, pos[a]) for a in range(3)])
print("scg FSL E 268.31")


def g2k(n, k, pts):
    nc = tp.ncoef(n, k)
    lo = tp.offset(k) - 2 * k
    hi = tp.offset(k) + (nc - 1) * k + 2 * k
    xs = np.arange(lo, hi + 1) if pts == "voxels" else np.arange(lo, hi + 1, k)
    b = tp.basis(n, k, 2, xs) * k**2  # per knot
    return b.T @ b


def fit_k(n, k, mu, pts):
    B = tp.basis(n, k)
    return np.linalg.solve(B.T @ B + mu * g2k(n, k, pts), B.T)


for pts in ("voxels", "knots"):
    for mu in (2.5e-5, 1e-4):
        c = tp.apply(fs, [fit_k(n2[a], 3, mu, pts) for a in range(3)])
        f = tp.apply(c, [tp.basis(n2[a], 3) for a in range(3)])
        c = tp.apply(f, [fit_k(n2[a], 2, mu, pts) for a in range(3)])
        f = tp.apply(c, B2)
        print(
            f"knot units, {pts}, mu {mu:g}: E {E(c, n2, 2, vox):.2f} rms {np.sqrt(((f - f2) ** 2).mean()):.4f} coef rms {np.sqrt(((c - c2) ** 2).mean()):.4f}"
        )
for d in ():
    for mu in (2.5e-5, 1e-4, 1e-3, 1e-2, 0.1):
        c = tp.apply(fs, [fit_be(n2[a], 3, mu, d) for a in range(3)])
        f = tp.apply(c, [tp.basis(n2[a], 3) for a in range(3)])
        c = tp.apply(f, [fit_be(n2[a], 2, mu, d) for a in range(3)])
        f = tp.apply(c, B2)
        print(
            f"d{d} mu {mu:g}: E {E(c, n2, 2, vox):.2f} rms {np.sqrt(((f - f2) ** 2).mean()):.4f} coef rms {np.sqrt(((c - c2) ** 2).mean()):.4f}"
        )
f1 = load("B", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("B", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n = np.array(f1.shape)
vox = np.array([12.0, 12.0, 13.199997])
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n[a], 2)) for a in range(3)])
print("B FSL E 59.797")
for d in (2, 1):
    for mu in (2.5e-5, 1e-4, 1e-3, 1e-2):
        c = tp.apply(f1, [fit_be(n[a], 1, mu, d) for a in range(3)])
        f = tp.apply(c, [tp.basis(n[a], 1) for a in range(3)])
        print(f"d{d} mu {mu:g}: E {E(c, n, 1, vox):.3f} rms {np.sqrt(((f - f2) ** 2).mean()):.5f}")

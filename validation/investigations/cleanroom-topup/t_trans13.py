# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe E: warpres 20 -> 22 at the same knot spacing (2) and grid: FSL's refit vs models."""

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


def D(nc, p):
    rows = []
    for j in range(-p, nc - 2 + p):
        r = np.zeros(nc)
        for o, w in ((0, 1.0), (1, -2.0), (2, 1.0)):
            if 0 <= j + o < nc:
                r[j + o] = w
        rows.append(r)
    return np.array(rows)


def fitp(n, k, lam, p, xs=None):
    xs = np.arange(n).astype(float) if xs is None else xs
    B = tp.basis(n, k, 0, xs)
    Dm = D(B.shape[1], p)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


f1 = load("E", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("E", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n = np.array(f1.shape)
vox = np.array([12.0, 12.0, 13.199997])
B = [tp.basis(n[a], 2) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B[a]) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(B[a]) for a in range(3)])
print(
    "FSL: E1",
    E(c1, n, 2, vox),
    "E2",
    E(c2, n, 2, vox),
    "field change rms",
    np.sqrt(((f2 - f1) ** 2).mean()),
    "max",
    np.abs(f2 - f1).max(),
)
for p in (0, 1, 2):
    for lam in (0.001, 0.005, 0.01, 0.02, 0.05, 0.1):
        c = tp.apply(f1, [fitp(n[a], 2, lam, p) for a in range(3)])
        f = tp.apply(c, B)
        print(
            f"p {p} lam {lam}: E {E(c, n, 2, vox):.3f} rms vs FSL {np.sqrt(((f - f2) ** 2).mean()):.5f} change {np.sqrt(((f - f1) ** 2).mean()):.4f}"
        )
np.set_printoptions(precision=3, suppress=True, linewidth=250)
print("x line FSL c1", c1[:, 5, 4])
print("x line FSL c2", c2[:, 5, 4])
c = tp.apply(f1, [fitp(n[a], 2, 0.005, 1) for a in range(3)])
print("x line mine  ", c[:, 5, 4])

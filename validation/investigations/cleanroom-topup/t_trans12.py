# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Second-difference penalties with zero padding (p rows beyond each end), probes scg and B."""

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


def fitp(n, k, lam, p):
    B = tp.basis(n, k)
    Dm = D(B.shape[1], p)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


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
fB1 = load("B", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
fB2 = load("B", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
nB = np.array(fB1.shape)
voxB = np.array([12.0, 12.0, 13.199997])
print("FSL: scg E 268.31, B E 59.797")
for p in (0, 1, 2):
    for lam in (0.005, 0.01, 0.02, 0.05):
        c = tp.apply(fs, [fitp(n2[a], 3, lam, p) for a in range(3)])
        f = tp.apply(c, [tp.basis(n2[a], 3) for a in range(3)])
        c = tp.apply(f, [fitp(n2[a], 2, lam, p) for a in range(3)])
        f = tp.apply(c, B2)
        cB = tp.apply(fB1, [fitp(nB[a], 1, lam, p) for a in range(3)])
        gB = tp.apply(cB, [tp.basis(nB[a], 1) for a in range(3)])
        print(
            f"p {p} lam {lam}: scg E {E(c, n2, 2, vox):.2f} rms {np.sqrt(((f - f2) ** 2).mean()):.4f} coef {np.sqrt(((c - c2) ** 2).mean()):.3f} | B E {E(cB, nB, 1, voxB):.3f} rms {np.sqrt(((gB - fB2) ** 2).mean()):.5f}"
        )

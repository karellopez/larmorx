# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe D (s 2 -> 1, warpres 20 -> 14, level 2 with miter 0): FSL's exact level-2 start
coefficients (its _fieldcoef) against refit models applied to the level-1 field."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


f1 = load("D", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("D", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
cF = load("D", "pr_fieldcoef.nii.gz")
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
print("grids", n1, n2, "coef", cF.shape)
B1 = [tp.basis(n1[a], 2) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B1[a]) for a in range(3)])
B2 = [tp.basis(n2[a], 2) for a in range(3)]
print("layout check |B cF - f2| max", np.abs(tp.apply(cF, B2) - f2).max())
xs = [-0.25 + 0.5 * np.arange(n2[a]) for a in range(3)]
v = tp.apply(c1, [tp.basis(n1[a], 2, 0, xs[a]) for a in range(3)])


def D(nc, p):
    rows = []
    for j in range(-p, nc - 2 + p):
        r = np.zeros(nc)
        for o, w in ((0, 1.0), (1, -2.0), (2, 1.0)):
            if 0 <= j + o < nc:
                r[j + o] = w
        rows.append(r)
    return np.array(rows)


def sep_fit(vals, n, k, lam, p):
    ops = []
    for a in range(3):
        B = tp.basis(n[a], k)
        Dm = D(B.shape[1], p)
        ops.append(np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T))
    return tp.apply(vals, ops)


def report(name, c):
    f = tp.apply(c, B2)
    dc = c - cF
    print(
        f"{name}: field rms vs FSL {np.sqrt(((f - f2) ** 2).mean()):.5f} max {np.abs(f - f2).max():.4f}; coef rms diff {np.sqrt((dc**2).mean()):.4f} (FSL coef rms {np.sqrt((cF**2).mean()):.3f})"
    )
    return f


for p in (0, 1, 2):
    for lam in (0.001, 0.005, 0.02):
        ci = sep_fit(v, n2, 3, lam, p)
        vi = tp.apply(ci, [tp.basis(n2[a], 3) for a in range(3)])
        c = sep_fit(vi, n2, 2, lam, p)
        report(f"sep p{p} lam {lam}", c)
np.set_printoptions(precision=3, suppress=True, linewidth=250)
ci = sep_fit(v, n2, 3, 0.005, 1)
vi = tp.apply(ci, [tp.basis(n2[a], 3) for a in range(3)])
c = sep_fit(vi, n2, 2, 0.005, 1)
for a in range(3):
    ax = tuple(i for i in range(3) if i != a)
    print("axis", a, "rms (mine - FSL) per index", np.sqrt(((c - cF) ** 2).mean(axis=ax)))
    print("axis", a, "rms FSL per index        ", np.sqrt((cF**2).mean(axis=ax)))
np.save(os.path.join(R, "D_v.npy"), v)

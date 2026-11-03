# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: separable refit models with the curvature penalty on the FIELD's values (second
differences at the voxels: periodic, zero-continued, interior only) instead of the coefficients."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


f1 = load("G", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
cF = load("G", "pr_fieldcoef.nii.gz")
n = np.array(f1.shape)
B = [tp.basis(n[a], 3) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B[a]) for a in range(3)])


def L(nv, kind):
    if kind == "per":
        Lm = np.zeros((nv, nv))
        for j in range(nv):
            for o, w in ((-1, 1.0), (0, -2.0), (1, 1.0)):
                Lm[j, (j + o) % nv] += w
        return Lm
    if kind == "zero":
        Lm = np.zeros((nv, nv))
        for j in range(nv):
            for o, w in ((-1, 1.0), (0, -2.0), (1, 1.0)):
                if 0 <= j + o < nv:
                    Lm[j, j + o] += w
        return Lm
    Lm = np.zeros((nv - 2, nv))
    for j in range(nv - 2):
        Lm[j, j : j + 3] = (1, -2, 1)
    return Lm


def Dc(nc, kind):
    Lm = L(nc, kind)
    return Lm


print("identity: coef rms diff", np.sqrt(((c1 - cF) ** 2).mean()))
res = []
for space in ("field", "coef"):
    for kind in ("per", "zero", "int"):
        for lam in (0.001, 0.002, 0.005, 0.01, 0.02, 0.05):
            ops = []
            for a in range(3):
                Ba = B[a]
                Lm = L(n[a], kind) @ Ba if space == "field" else Dc(Ba.shape[1], kind)
                ops.append(np.linalg.solve(Ba.T @ Ba + lam**2 * Lm.T @ Lm, Ba.T @ Ba))
            c = tp.apply(c1, ops)
            res.append((np.sqrt(((c - cF) ** 2).mean()), f"{space} {kind} lam {lam}"))
for r, name in sorted(res)[:8]:
    print(f"{r:.5f} {name}")

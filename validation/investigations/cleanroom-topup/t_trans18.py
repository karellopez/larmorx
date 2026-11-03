# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: where FSL's refit (same basis) changes the field and the coefficients, in the
eigenbasis of the 3D normal matrix (B^T B per axis)."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


np.set_printoptions(precision=4, suppress=True, linewidth=250)
f1 = load("G", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("G", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
cF = load("G", "pr_fieldcoef.nii.gz")
n = np.array(f1.shape)
B = [tp.basis(n[a], 3) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B[a]) for a in range(3)])
df = f2 - f1
for a in range(3):
    ax = tuple(i for i in range(3) if i != a)
    print("axis", a, "field change rms per voxel", np.sqrt((df**2).mean(axis=ax)))
dc = cF - c1
for a in range(3):
    ax = tuple(i for i in range(3) if i != a)
    print("axis", a, "coef change rms per index", np.sqrt((dc**2).mean(axis=ax)))
# eigenbasis per axis
W, V = [], []
for a in range(3):
    w, v = np.linalg.eigh(B[a].T @ B[a])
    W.append(w)
    V.append(v)
    print("axis", a, "eig", w)
t1 = tp.apply(c1, [V[a].T for a in range(3)])
tF = tp.apply(cF, [V[a].T for a in range(3)])
lam3 = W[0][:, None, None] * W[1][None, :, None] * W[2][None, None, :]
order = np.argsort(lam3.ravel())
r = (tF - t1).ravel()[order]
base = t1.ravel()[order]
l = lam3.ravel()[order]
for q in range(0, len(order), len(order) // 12):
    sl = slice(q, q + len(order) // 12)
    print(
        f"eig {l[sl].min():.2e}..{l[sl].max():.2e}: rms change {np.sqrt((r[sl] ** 2).mean()):.4e}, rms coef {np.sqrt((base[sl] ** 2).mean()):.3e}, ratio {np.sqrt((r[sl] ** 2).mean()) / np.sqrt((base[sl] ** 2).mean()):.3e}"
    )
# shrinkage factor per component: tF/t1 for well-defined components
ratio = (tF / np.where(np.abs(t1) > 1e-3, t1, np.nan)).ravel()[order]
for q in range(0, len(order), len(order) // 12):
    sl = slice(q, q + len(order) // 12)
    print(f"eig {l[sl].min():.2e}..{l[sl].max():.2e}: median tF/t1 {np.nanmedian(ratio[sl]):.6f}")

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: is FSL's refit separable? Alternating least squares for per-axis matrices M_a with
c_FSL = (M_x, M_y, M_z) applied to c1; then the fitted M_a against penalty models."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


np.set_printoptions(precision=5, suppress=True, linewidth=250)
f1 = load("G", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
cF = load("G", "pr_fieldcoef.nii.gz")
n = np.array(f1.shape)
B = [tp.basis(n[a], 3) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B[a]) for a in range(3)])
nc = cF.shape
M = [np.eye(nc[a]) for a in range(3)]


def unfold(x, a):
    return np.moveaxis(x, a, 0).reshape(x.shape[a], -1)


for it in range(30):
    for a in range(3):
        ops = [M[b] if b != a else np.eye(nc[a]) for b in range(3)]
        Y = tp.apply(c1, ops)  # other axes applied
        X = unfold(Y, a)
        T = unfold(cF, a)
        M[a] = np.linalg.lstsq(X.T, T.T, rcond=None)[0].T
    # fix the scale ambiguity: normalise traces
    s = [np.trace(M[a]) / nc[a] for a in range(3)]
    g = np.prod(s) ** (1 / 3)
    M = [M[a] / s[a] * g for a in range(3)]
res = tp.apply(c1, M) - cF
print(
    "separable fit residual rms",
    np.sqrt((res**2).mean()),
    "vs change rms",
    np.sqrt(((cF - c1) ** 2).mean()),
)
for a in range(3):
    print("axis", a, "M - I diag", np.diag(M[a]) - 1)
    print("axis", a, "M - I row 0", (M[a] - np.eye(nc[a]))[0])
    print("axis", a, "M - I row -1", (M[a] - np.eye(nc[a]))[-1])
np.save(os.path.join(R, "G_M.npy"), np.array(M, dtype=object), allow_pickle=True)

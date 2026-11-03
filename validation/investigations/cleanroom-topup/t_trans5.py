# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import os
import sys

import nibabel as nib
import numpy as np
from scipy.optimize import minimize

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, name):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{name}").dataobj, float)


def rf(n, k, lam=0.005):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


p, k1, K1, k2 = sys.argv[1], 2, int(sys.argv[2]), int(sys.argv[3])
f1 = load(p, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load(p, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], k1)) for a in range(3)])
M1 = [rf(n2[a], K1) for a in range(3)]
B1 = [tp.basis(n2[a], K1) for a in range(3)]
M2 = [rf(n2[a], k2) for a in range(3)]
B2 = [tp.basis(n2[a], k2) for a in range(3)]


def model(q):
    pos = [q[2 * a] + np.arange(n2[a]) * q[2 * a + 1] for a in range(3)]
    fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
    f = tp.apply(tp.apply(fs, M1), B1)
    if k2 != K1:
        f = tp.apply(tp.apply(f, M2), B2)
    return f


def obj(q):
    return np.sqrt(((model(q) - f2) ** 2).mean())


q0 = np.array([-0.25, 0.5] * 3)
print("start", obj(q0))
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
from scipy import ndimage as ndi

f = tp.apply(tp.apply(tp.apply(tp.apply(fs, M1), B1), M2), B2)
r = f2 - f
lap = ndi.laplace(f, mode="nearest")
print(
    "corr(residual, laplacian(f))",
    np.corrcoef(r[2:-2, 2:-2, 2:-2].ravel(), lap[2:-2, 2:-2, 2:-2].ravel())[0, 1],
)
for ax in range(3):
    d2 = np.gradient(np.gradient(f, axis=ax), axis=ax)
    print(
        " corr with d2 along",
        ax,
        np.corrcoef(r[2:-2, 2:-2, 2:-2].ravel(), d2[2:-2, 2:-2, 2:-2].ravel())[0, 1],
    )
# compare FSL's L2 start with the k=3 field directly sampled/refit to k=2 without the intermediate regularisation
import itertools

out = []
for ks in itertools.product((1, 2, 3, 4), repeat=3):
    f = tp.apply(
        tp.apply(fs, [rf(n2[a], ks[a]) for a in range(3)]),
        [tp.basis(n2[a], ks[a]) for a in range(3)],
    )
    f = tp.apply(tp.apply(f, M2), B2)
    out.append((float(np.sqrt(((f - f2) ** 2).mean())), ks))
out.sort()
print(out[:8])
# no second refit at all: direct k=3
f = tp.apply(tp.apply(fs, M1), B1)
print("k=3 field only vs fsl (should be in k=2 space?)", np.sqrt(((f - f2) ** 2).mean()))
sys.exit()
r = minimize(
    obj, q0, method="Nelder-Mead", options={"maxiter": 3000, "xatol": 1e-7, "fatol": 1e-10}
)
print(np.round(r.x, 5), r.fun)
d = model(q0) - f2
print("per-x rms", np.round(np.sqrt((d**2).mean((1, 2))), 4).tolist())
print("per-z rms", np.round(np.sqrt((d**2).mean((0, 1))), 4).tolist())

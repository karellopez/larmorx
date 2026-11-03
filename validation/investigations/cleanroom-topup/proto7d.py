# SPDX-License-Identifier: Apache-2.0
"""Regulariser gradient (GradientWithReg - Gradient) vs candidate R c, per term (least squares)."""

import sys

import nibabel as nib
import numpy as np

D = sys.argv[1]
lam = 0.001
ssd = {0: 191.809, 1: 97.6978, 2: 81.8478, 3: 79.873}
vox = np.array([6.0, 6.0, 6.5999985])
nt = (32, 32, 20)
k = 2


def bspl(t, d=0):
    a = np.abs(t)
    s = np.sign(t)
    if d == 0:
        return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))
    if d == 1:
        return s * np.where(a < 1, -2 * a + 1.5 * a**2, np.where(a < 2, -0.5 * (2 - a) ** 2, 0.0))
    return np.where(a < 1, -2 + 3 * a, np.where(a < 2, (2 - a), 0.0))


nc = [int(np.ceil((nt[a_] + 1) / k)) + 2 for a_ in range(3)]


def B(ax, d, xs):
    return bspl((xs[:, None] - (np.arange(nc[ax])[None, :] - 1) * k) / k, d) / k**d


def apply(c, mats):
    out = c
    for ax in range(3):
        out = np.moveaxis(np.tensordot(mats[ax], np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


def gram(ax, d, lo, hi, per=64):
    xs = np.arange(lo, hi + 1).astype(float)
    b = B(ax, d, xs) / vox[ax] ** d
    return b.T @ b * vox[ax]


terms = [(2, 0, 0), (0, 2, 0), (0, 0, 2), (1, 1, 0), (1, 0, 1), (0, 1, 1)]
rng = sys.argv[2] if len(sys.argv) > 2 else "support"
rf = {
    "support": lambda n: (-3 * k, (int(np.ceil((n + 1) / k))) * k + 2 * k),
    "fov": lambda n: (-0.5, n - 0.5),
    "grid": lambda n: (0, n - 1),
    "ext1": lambda n: (-1, n),
    "ext2": lambda n: (-2, n + 1),
    "extk": lambda n: (-k, n + k - 1),
}[rng]
G = {(ax, d): gram(ax, d, *rf(nt[ax])) for ax in range(3) for d in range(3)}
for it in (1, 2):
    f = np.asarray(
        nib.load(f"{D}/TopupDebugFieldlevel01_iter0{it}_attempt01.nii.gz").dataobj, np.float64
    )
    pinv = [np.linalg.pinv(B(ax, 0, np.arange(nt[ax]).astype(float))) for ax in range(3)]
    c = apply(f, pinv)
    g = np.loadtxt(f"{D}/TopupDebugGradient_level01_iter0{it + 1}_attempt00.txt")
    gr = np.loadtxt(f"{D}/TopupDebugGradientWithReg_level01_iter0{it + 1}_attempt00.txt")
    rg = gr - g
    cols = []
    for ds in terms:
        cols.append(
            apply(c, [G[(ax, ds[ax])] for ax in range(3)]).flatten(order="F") * 2 * lam * ssd[it]
        )
    A = np.stack(cols, 1)
    sol, *_ = np.linalg.lstsq(A, rg, rcond=None)
    res = rg - A @ sol
    print(
        rng, it, "weights", np.round(sol, 6), "rel resid", np.linalg.norm(res) / np.linalg.norm(rg)
    )

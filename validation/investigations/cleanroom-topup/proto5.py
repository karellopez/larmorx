# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Bending energy as integrals of the B-spline field over several ranges."""

import sys

import nibabel as nib
import numpy as np

D = sys.argv[1]
lam = 0.001
reg = {1: 6.37415, 2: 11.0998, 3: 12.4782}
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
    # midpoint rule on a fine grid over [lo, hi] voxels
    m = int(round((hi - lo) * per))
    xs = lo + (np.arange(m) + 0.5) * (hi - lo) / m
    b = B(ax, d, xs) / vox[ax] ** d
    return b.T @ b * ((hi - lo) / m) * vox[ax]


coefs = {}
for it in (1, 2, 3):
    f = np.asarray(
        nib.load(f"{D}/TopupDebugFieldlevel01_iter0{it}_attempt01.nii.gz").dataobj, np.float64
    )
    pinv = [np.linalg.pinv(B(ax, 0, np.arange(nt[ax]).astype(float))) for ax in range(3)]
    coefs[it] = apply(f, pinv)

ranges = {
    "[-.5,n-.5]": lambda n: (-0.5, n - 0.5),
    "[0,n-1]": lambda n: (0, n - 1),
    "support": lambda n: (-2 * k - k, (int(np.ceil((n + 1) / k))) * k + 2 * k),
}
for rn, rf in ranges.items():
    G = {(ax, d): gram(ax, d, *rf(nt[ax])) for ax in range(3) for d in range(3)}
    for it in (1, 2, 3):
        c = coefs[it]

        def q(ds):
            mats = [G[(ax, ds[ax])] for ax in range(3)]
            return float((c * apply(c, mats)).sum())

        diag = q((2, 0, 0)) + q((0, 2, 0)) + q((0, 0, 2))
        cross = q((1, 1, 0)) + q((1, 0, 1)) + q((0, 1, 1))
        for nm, E in (("x2", diag + 2 * cross), ("x1", diag + cross)):
            print(
                rn,
                it,
                nm,
                f"E={E:.6g}",
                "Reg/(lam*ssd*E)",
                reg[it] / (lam * ssd[it] * E),
                "Reg/(lam*E)",
                reg[it] / (lam * E),
            )

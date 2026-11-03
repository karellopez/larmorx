# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Fit Reg = alpha * lam * ssd * (sum_d a_d * term_d) over the debug iterations."""

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
terms = [(2, 0, 0), (0, 2, 0), (0, 0, 2), (1, 1, 0), (1, 0, 1), (0, 1, 1)]
for rn, rf in {
    "support": lambda n: (-3 * k, (int(np.ceil((n + 1) / k))) * k + 2 * k),
    "[-.5,n-.5]": lambda n: (-0.5, n - 0.5),
}.items():
    G = {(ax, d): gram(ax, d, *rf(nt[ax])) for ax in range(3) for d in range(3)}
    rows = []
    for it in (1, 2, 3):
        c = coefs[it]
        q = [float((c * apply(c, [G[(ax, ds[ax])] for ax in range(3)])).sum()) for ds in terms]
        rows.append([lam * ssd[it] * x for x in q])
    A = np.array(rows)
    y = np.array([reg[i] for i in (1, 2, 3)])
    # fit alpha*(diag + w*cross): y = a*Dg + b*X
    Dg = A[:, :3].sum(1)
    X = A[:, 3:].sum(1)
    sol, res, *_ = np.linalg.lstsq(np.stack([Dg, X], 1), y, rcond=None)
    print(
        rn,
        "alpha",
        sol[0],
        "alpha*w",
        sol[1],
        "w",
        sol[1] / sol[0],
        "pred",
        np.stack([Dg, X], 1) @ sol,
        y,
    )
    print("  per-term values (it1)", A[0] / (lam * ssd[1]))

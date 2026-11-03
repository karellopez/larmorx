# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Bending energy convention: recover coefficients from FSL's debug field, compare E with Reg."""

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


def B(ax, d=0, x=None):
    xs = np.arange(nt[ax]) if x is None else x
    return bspl((xs[:, None] - (np.arange(nc[ax])[None, :] - 1) * k) / k, d) / k**d


def apply(c, mats):
    out = c
    for ax in range(3):
        out = np.moveaxis(np.tensordot(mats[ax], np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


for it in (1, 2, 3):
    f = np.asarray(
        nib.load(f"{D}/TopupDebugFieldlevel01_iter0{it}_attempt01.nii.gz").dataobj, np.float64
    )
    pinv = [np.linalg.pinv(B(ax)) for ax in range(3)]
    c = apply(f, pinv)
    print(
        it,
        "refit err",
        np.abs(apply(c, [B(ax) for ax in range(3)]) - f).max(),
        "field max",
        np.abs(f).max(),
    )

    # second derivatives on the voxel grid, per mm
    def d2(i, j):
        mats = []
        for ax in range(3):
            d = (ax == i) + (ax == j)
            mats.append(B(ax, d) / vox[ax] ** d)
        return apply(c, mats)

    sq = {(i, j): (d2(i, j) ** 2).sum() for i in range(3) for j in range(i, 3)}
    diag = sq[(0, 0)] + sq[(1, 1)] + sq[(2, 2)]
    cross = sq[(0, 1)] + sq[(0, 2)] + sq[(1, 2)]
    for name, E in [("sum vox, cross x2", diag + 2 * cross), ("sum vox, cross x1", diag + cross)]:
        for norm_name, norm in [("", 1), ("/nvox", np.prod(nt)), ("*voxvol", 1 / np.prod(vox))]:
            Ev = E / norm
            print(
                f"  {name}{norm_name}: E={Ev:.6g}  lam*E={lam * Ev:.6g} lam*ssd_cur*E={lam * ssd[it] * Ev:.6g} lam*ssd_prev*E={lam * ssd[it - 1] * Ev:.6g}  (Reg {reg[it]})"
            )

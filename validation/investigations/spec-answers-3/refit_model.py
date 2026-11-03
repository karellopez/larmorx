# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §4.4 (spec writer, 2026-11-03): the refit between levels, from the spec's text.

At a change of subsampling the field is sampled at the new grid's voxel centres
(`(step - 1) / 2 + j * step` old voxels) and refitted with the knot spacing the *previous*
level's --warpres gives on the new grid (round half away from zero, 1 for 0); then, when the
level's --warpres differs from the previous level's (as a number, whatever spacing it gives),
the field is sampled at the voxels and refitted with the new spacing. Each refit: along x, then
y, then z, the coefficients `c = (B^T B + 0.005^2 S^T S)^-1 B^T y` of the samples `y` (single
precision), `B` the B-spline basis at the voxels, `S` only the two second differences of the
coefficients that wrap around the ends, `c[n-1] - 2 c[0] + c[1]` and `c[n-2] - 2 c[n-1] + c[0]`.

This script applies that to FSL's level-1 field of the clean-room implementer's probes D to H
(`validation/investigations/cleanroom-topup/probes.py`: the half-resolution pair, two levels,
the second with --miter=0, so FSL's `pr_fieldcoef` holds its refitted coefficients) and compares
with FSL's coefficients and its level-2 start field. `--variant interior` adds the interior
second differences (rows (1, -2, 1) for every coefficient, continued periodically), `zero` the
differences continued by zeros beyond the ends (the spec's earlier reading), `none` no penalty:
each parts from FSL.

Usage: refit_model.py <runs dir holding fsl-probe-D ... fsl-probe-H> [--variant ...]
"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

F32 = np.float32


def bspline3(t):
    a = np.abs(t)
    return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))


def ncoef(n, k):
    return n if k == 1 else int(np.ceil((n + 1) / k)) + 2


def basis(n, k, xs=None):
    """The cubic basis at positions xs (default the voxels): coefficient i centred at voxel
    (i - 1) k (k > 1) or i (k = 1) (§4.2)."""
    xs = np.arange(n, dtype=float) if xs is None else np.asarray(xs, float)
    centres = (np.arange(ncoef(n, k)) - (1 if k > 1 else 0)) * k
    return bspline3((xs[:, None] - centres[None, :]) / k)


def penalty(nc, variant):
    rows = []
    if variant in ("wrap", "interior"):
        rows.append({nc - 1: 1, 0: -2, 1: 1})
        rows.append({nc - 2: 1, nc - 1: -2, 0: 1})
    if variant == "interior":
        rows += [{j - 1: 1, j: -2, j + 1: 1} for j in range(1, nc - 1)]
    if variant == "zero":
        rows += [{j - 1: 1, j: -2, j + 1: 1} for j in range(nc)]
    S = np.zeros((max(len(rows), 1), nc))
    for r, row in enumerate(rows):
        for j, w in row.items():
            if 0 <= j < nc:
                S[r, j] += w
    return S


def along(a, mats):
    out = a
    for ax in range(3):
        out = np.moveaxis(np.tensordot(mats[ax], np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


def refit(y, ks, variant):
    mats = []
    for ax in range(3):
        B = basis(y.shape[ax], ks[ax])
        S = penalty(B.shape[1], variant)
        lam = 0.0 if variant == "none" else 0.005
        mats.append(np.linalg.solve(B.T @ B + lam**2 * (S.T @ S), B.T))
    return along(y, mats)


def ksp(warpres, vox):
    return [max(int(warpres / v + 0.5), 1) for v in vox]


def main():
    runs = Path(sys.argv[1])
    variant = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else "wrap"
    # probe: the subsamplings and the warp resolutions of its two levels
    probes = {
        "D": (2, 1, 20, 14),
        "E": (2, 2, 20, 22),
        "F": (1, 1, 20, 14),
        "G": (1, 1, 18, 20),
        "H": (2, 1, 20, 20),
    }
    for name, (s1, s2, wr1, wr2) in probes.items():
        d = runs / f"fsl-probe-{name}"
        hdr = nib.load(str(d / "half.nii.gz")).header
        n0 = np.array(hdr.get_data_shape()[:3])
        vox = np.array(hdr.get_zooms()[:3], float).astype(F32).astype(float)
        n1, n2 = tuple(n0 // s1), tuple(n0 // s2)
        v1, v2 = vox * s1, vox * s2  # the target grids' voxel sizes (§4.4)
        f1 = np.asarray(
            nib.load(str(d / "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")).dataobj, float
        )
        start2 = np.asarray(
            nib.load(str(d / "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")).dataobj, float
        )
        k1 = ksp(wr1, v1)
        # the level-1 coefficients that give FSL's level-1 field at its voxels
        c = along(f1, [np.linalg.pinv(basis(n1[a], k1[a])) for a in range(3)])
        k = k1
        if n2 != n1:  # a change of subsampling: sample at the new voxels' centres
            step = s2 / s1
            xs = [(step - 1) / 2 + step * np.arange(n2[a]) for a in range(3)]
            y = along(c, [basis(n1[a], k1[a], xs[a]) for a in range(3)]).astype(F32)
            k = ksp(wr1, v2)  # the previous level's --warpres on the new grid
            c = refit(y.astype(float), k, variant)
        if wr2 != wr1:  # --warpres changed (as a number): sample at the voxels, refit
            y = along(c, [basis(n2[a], k[a]) for a in range(3)]).astype(F32)
            k = ksp(wr2, v2)
            c = refit(y.astype(float), k, variant)
        f2 = along(c, [basis(n2[a], k[a]) for a in range(3)])
        coef = np.asarray(nib.load(str(d / "pr_fieldcoef.nii.gz")).dataobj, float)
        # E's second level runs one iteration and is subsampled (its coefficients are refitted
        # again at the end): only its start field is compared
        dc = (
            min(np.abs(coef - c).max(), np.abs(coef[::-1] - c).max())
            if coef.shape == c.shape and name != "E"
            else float("nan")
        )
        change = (
            f"{np.sqrt(((start2 - f1) ** 2).mean()):.5f} Hz"
            if f1.shape == start2.shape
            else "(a new grid)"
        )
        print(
            f"probe {name}: FSL's change of the field rms {change}; "
            f"model - FSL: start field rms {np.sqrt(((f2 - start2) ** 2).mean()):.2e} max "
            f"{np.abs(f2 - start2).max():.2e} Hz, coefficients max {dc:.2e} (of "
            f"{np.abs(coef).max():.3g}); knot spacing {k}"
        )


if __name__ == "__main__":
    main()

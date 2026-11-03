# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §7.4 (spec writer, 2026-11-03): applytopup's least-squares restoration with
the resampling matrices as FSL builds them, from the spec's text: row by row (each observed
voxel's extent mapped back through the line's piecewise-linear mapping, its two ends found by
FSL's walk), the mapping at an index outside the line read as FSL-133 reads it.

For a line of `n` voxels and an image with `s = p t` (the phase-encoding component times the
read-out time), `phi(k) = k + s h_w(k)` with `w(k) = k mod n` for `k >= 0` and, for `k < 0`,
`w(k) = (k + 2^32) mod n` (FSL-133: the circular `k + n` only when `n` divides 2^32). Row `i`
of `K`: the ends `u1`, `u2` where `phi` crosses `i - 1/2` and `i + 1/2` (each found by the walk:
from `ceil(x)` up while `x > phi(k)`, else down while `x < phi(k - 1)`), and `K(i, j mod n)` =
the length of `[u1, u2]` inside `[j - 1/2, j + 1/2]`. The restoration `(K^T K + 0.01 S^T S)^-1
K^T y` with `S` the second differences of the line (first and last rows `(1, -1)`, `(-1, 1)`).

Usage: lsr_model.py <dir of probe directories> [--variant circular]
(each probe directory as `cleanroom-topup/wrap_probe.py` or `fold_probe.py` writes it, with
FSL's `hifi` output from `cleanroom-topup/at_case.py args`). `--variant circular`: `w(k) = k mod
n` for every `k`; `--variant overlap`: the matrices column by column, every part of a folded
mapping counted (the reading of §7.4 before 2026-11-03).
"""

import math
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def bspline3(t):
    a = np.abs(t)
    return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))


def field_line(coef: np.ndarray, x: int, z: int) -> np.ndarray:
    """The field along y at (x, z) of a coefficient image of knot spacing 1 (coefficient i at
    voxel i, §4.2)."""
    n = coef.shape
    out = np.zeros(n[1])
    for a in range(max(0, x - 2), min(n[0], x + 3)):
        for c in range(max(0, z - 2), min(n[2], z + 3)):
            w = bspline3(np.array(x - a)) * bspline3(np.array(z - c))
            if w == 0:
                continue
            for j in range(n[1]):
                b = bspline3(np.arange(n[1]) - j)
                out[j] += w * float(b @ coef[a, :, c])
    return out.astype(np.float32).astype(float)  # the field is held in single precision


def k_matrix(h: np.ndarray, s: float, variant: str) -> np.ndarray:
    n = len(h)

    def w(k):
        if k >= 0 or variant == "circular":
            return k % n
        return (k + 2**32) % n

    def phi(k):
        return k + s * h[w(k)]

    def find(x):
        k = int(math.ceil(x))
        if x > phi(k):
            k += 1
            while x > phi(k):
                k += 1
            return k - 1
        if x == phi(k):
            return k
        while x < phi(k - 1):
            k -= 1
        return k - 1

    K = np.zeros((n, n))
    for i in range(n):
        ends = []
        for x in (i - 0.5, i + 0.5):
            k = find(x)
            ends.append(k + (x - phi(k)) / (phi(k + 1) - phi(k)))
        u1, u2 = ends
        for j in range(int(math.floor(u1 + 0.5)), int(math.floor(u2 + 0.5)) + 1):
            ov = min(u2, j + 0.5) - max(u1, j - 0.5)
            if ov > 0:
                K[i, j % n] += ov
    return K


def overlap_matrix(h: np.ndarray, s: float, variant: str) -> np.ndarray:
    """`--variant overlap`: column by column, the length of every part of voxel j's extent
    that the (circular) mapping sends into observed voxel i's extent, folds counted."""
    n = len(h)
    K = np.zeros((n, n))
    for j in range(n):
        for u0, u1, a, b in ((j - 0.5, j, j - 1, j), (j, j + 0.5, j, j + 1)):
            pa, pb = a + s * h[a % n], b + s * h[b % n]
            v0, v1 = pa + (u0 - a) * (pb - pa), pa + (u1 - a) * (pb - pa)
            lo, hi = min(v0, v1), max(v0, v1)
            if hi == lo:
                continue
            for i in range(int(math.floor(lo + 0.5)), int(math.floor(hi + 0.5)) + 1):
                ov = min(hi, i + 0.5) - max(lo, i - 0.5)
                if ov > 0:
                    K[i % n, j] += ov * (u1 - u0) / (hi - lo)
    return K


def s_matrix(n):
    S = np.zeros((n, n))
    S[0, 0], S[0, 1] = 1, -1
    for i in range(1, n - 1):
        S[i, i - 1 : i + 2] = (-1, 2, -1)
    S[-1, -2], S[-1, -1] = -1, 1
    return S


def main():
    root = Path(sys.argv[1])
    variant = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else "fsl"
    for d in sorted(root.iterdir()):
        if not (d / "hifi.nii.gz").exists():
            continue
        _, x, y, z = d.name.split("_")
        x, y, z = int(x), int(y), int(z)
        want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, float)[x, :, z]
        coef = np.asarray(nib.load(str(d / "tu_fieldcoef.nii.gz")).dataobj, float)
        acq = np.loadtxt(d / "acqw.txt")
        h = field_line(coef, x, z)
        n = len(h)
        ys = [
            np.asarray(nib.load(str(d / f"{name}.nii.gz")).dataobj, float)[x, :, z]
            for name in ("pu", "pd")
        ]
        build = overlap_matrix if variant == "overlap" else k_matrix
        Ks = [build(h, acq[a, 1] * acq[a, 3], variant) for a in range(2)]
        K = np.vstack(Ks)
        S = s_matrix(n)
        f = np.linalg.solve(K.T @ K + 0.01 * S.T @ S, K.T @ np.concatenate(ys))
        err = np.abs(f - want)
        print(f"{root.name}/{d.name}: largest |model - FSL| {err.max():.2e} "
              f"(the line's largest {np.abs(want).max():.4g}); at voxels {np.flatnonzero(err > 1e-3)[:6]}")


if __name__ == "__main__":
    main()

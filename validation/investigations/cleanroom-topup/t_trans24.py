# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The wrap-around penalty (0.005^2 times the two second differences of the coefficients that
wrap around the ends) against FSL's level-2 start fields of probes D to H, and the zero-continued
penalty the original used before."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


def P_of(nc, kind):
    rows = []

    def r(pairs):
        v = np.zeros(nc)
        for j, w in pairs:
            if kind == "wrap":
                v[j % nc] += w
            elif 0 <= j < nc:
                v[j] += w
        rows.append(v)

    if kind == "wrap":
        r([(-1, 1), (0, -2), (1, 1)])
        r([(-2, 1), (-1, -2), (0, 1)])
    else:  # zero-continued: one row per coefficient
        for j in range(nc):
            r([(j - 1, 1), (j, -2), (j + 1, 1)])
    Rm = np.array(rows)
    return Rm.T @ Rm


def fit(n, k, xs, kind, lam=0.005):
    Bs = tp.basis(n, k, 0, xs if xs is not None else None)
    Bv = tp.basis(n, k)  # fit at the voxels: the samples are given at the voxels
    return np.linalg.solve(Bv.T @ Bv + lam**2 * P_of(Bv.shape[1], kind), Bv.T)


def run(probe, n1, k1, n2, step, kint, k2):
    f1 = load(probe, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
    f2 = load(probe, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
    B1 = [tp.basis(n1[a], k1[a]) for a in range(3)]
    c1 = tp.apply(f1, [np.linalg.pinv(B1[a]) for a in range(3)])
    out = {}
    for kind in ("zero", "wrap"):
        if step is not None:
            xs = [(step - 1) / 2 + step * np.arange(n2[a]) for a in range(3)]
            v = tp.apply(c1, [tp.basis(n1[a], k1[a], 0, xs[a]) for a in range(3)])
            c = tp.apply(v, [fit(n2[a], kint[a], None, kind) for a in range(3)])
            kc = kint
        else:
            c, kc = c1, k1
        if list(kc) != list(k2) or step is None:
            v = tp.apply(c, [tp.basis(n2[a], kc[a]) for a in range(3)])
            c = tp.apply(v, [fit(n2[a], k2[a], None, kind) for a in range(3)])
        f = tp.apply(c, [tp.basis(n2[a], k2[a]) for a in range(3)])
        out[kind] = (np.sqrt(((f - f2) ** 2).mean()), np.abs(f - f2).max())
    base = np.sqrt(((f2 - (f1 if f1.shape == f2.shape else f2)) ** 2).mean())
    print(
        f"probe {probe}: FSL change rms {base:.5f}; zero-continued rms {out['zero'][0]:.6f} max {out['zero'][1]:.5f}; wrap rms {out['wrap'][0]:.6f} max {out['wrap'][1]:.5f}"
    )


# probe, level-1 grid, k1, level-2 grid, step (None: same grid), intermediate k, k2
run("G", (32, 32, 20), (3, 3, 3), (32, 32, 20), None, None, (3, 3, 3))
run("F", (32, 32, 20), (3, 3, 3), (32, 32, 20), None, None, (2, 2, 2))
run("E", (16, 16, 10), (2, 2, 2), (16, 16, 10), None, None, (2, 2, 2))
run("H", (16, 16, 10), (2, 2, 2), (32, 32, 20), 0.5, (3, 3, 3), (3, 3, 3))
run("D", (16, 16, 10), (2, 2, 2), (32, 32, 20), 0.5, (3, 3, 3), (2, 2, 2))

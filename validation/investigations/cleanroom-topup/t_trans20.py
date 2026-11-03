# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: the fitted per-axis refit operators (t_trans19) against models with wrap-around:
circular second differences of the coefficients; periodic samples of the field."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]
np.set_printoptions(precision=5, suppress=True, linewidth=250)
M = list(np.load(os.path.join(R, "G_M.npy"), allow_pickle=True))
n = [32, 32, 20]


def Dc(nc):
    D = np.zeros((nc, nc))
    for j in range(nc):
        for o, w in ((-1, 1.0), (0, -2.0), (1, 1.0)):
            D[j, (j + o) % nc] += w
    return D


def model_circ(nv, k, mu):
    B = tp.basis(nv, k)
    D = Dc(B.shape[1])
    return np.linalg.solve(B.T @ B + mu * D.T @ D, B.T @ B)


def cmp(Mf, Mm):
    # scale-free: compare M - I after matching the median diagonal
    s = np.median(np.diag(Mf)) / np.median(np.diag(Mm))
    d = Mf - s * Mm
    return np.sqrt((d**2).mean()), np.sqrt(
        ((Mf - np.eye(len(Mf)) * np.median(np.diag(Mf))) ** 2).mean()
    )


for a in range(3):
    best = None
    for mu in np.geomspace(1e-7, 1e-3, 41):
        r, base = cmp(M[a], model_circ(n[a], 3, mu))
        if best is None or r < best[0]:
            best = (r, mu, base)
    print(
        f"axis {a}: circular D best mu {best[1]:.3e} (lam {np.sqrt(best[1]):.4f}) resid {best[0]:.2e} of {best[2]:.2e}"
    )
    Mm = model_circ(n[a], 3, best[1])
    print("  fitted row 0", (M[a] - np.eye(len(M[a])))[0])
    print("  model  row 0", (Mm - np.eye(len(Mm)))[0])
    print("  fitted row -1", (M[a] - np.eye(len(M[a])))[-1])
    print("  model  row -1", (Mm - np.eye(len(Mm)))[-1])

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: penalties on the edge coefficients only (rows that wrap around the ends) against
the fitted per-axis refit operators (t_trans19)."""

import sys, os, numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]
np.set_printoptions(precision=4, suppress=True, linewidth=250)
M = list(np.load(os.path.join(R, "G_M.npy"), allow_pickle=True))


def rows(nc, kind):
    out = []

    def r(pairs):
        v = np.zeros(nc)
        for j, w in pairs:
            v[j % nc] += w
        out.append(v)

    if kind in ("d2wrap", "d2wrap+d1"):
        r([(-1, 1), (0, -2), (1, 1)])
        r([(-2, 1), (-1, -2), (0, 1)])
    if kind in ("d1wrap", "d2wrap+d1"):
        r([(-1, 1), (0, -1)])
    if kind == "d3wrap":
        for j in (-2, -1, 0):
            r([(j - 1, -1), (j, 3), (j + 1, -3), (j + 2, 1)])
    if kind == "d2full":
        for j in range(nc):
            r([(j - 1, 1), (j, -2), (j + 1, 1)])
    return np.array(out)


def score(Mf, Mm):
    s = np.median(np.diag(Mf)) / np.median(np.diag(Mm))
    return np.sqrt(((Mf - s * Mm) ** 2).mean())


for a, n in ((0, 32), (2, 20)):
    B = tp.basis(n, 3)
    G = B.T @ B
    nc = G.shape[0]
    print(f"axis {a}: identity {score(M[a], np.eye(nc)):.2e}")
    for kind in ("d2wrap", "d1wrap", "d2wrap+d1", "d3wrap", "d2full"):
        Rm = rows(nc, kind)
        P = Rm.T @ Rm
        best = min(
            (score(M[a], np.linalg.solve(G + mu * P, G)), mu) for mu in np.geomspace(1e-7, 1e-2, 51)
        )
        Mm = np.linalg.solve(G + best[1] * P, G)
        print(
            f"   {kind}: {best[0]:.2e} at mu {best[1]:.2e}; row 0 {(Mm - np.eye(nc))[0][[0, 1, 2, -2, -1]]}"
        )
    print("   fitted row 0", (M[a] / np.median(np.diag(M[a])) - np.eye(nc))[0][[0, 1, 2, -2, -1]])

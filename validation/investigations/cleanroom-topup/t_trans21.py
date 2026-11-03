# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G: refit operators with wrap-around (periodic Gram, periodic basis, periodically extended
samples), with and without the small curvature penalty, against the fitted ones (t_trans19)."""

import sys, os, numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]
np.set_printoptions(precision=4, suppress=True, linewidth=250)
M = list(np.load(os.path.join(R, "G_M.npy"), allow_pickle=True))


def basis_at(n, k, xs):
    return tp.basis(n, k, 0, xs)


def Bper(n, k):  # periodic basis on n voxels: rows x=0..n-1, each basis wrapped
    nc = tp.ncoef(n, k)
    B = np.zeros((n, nc))
    for s in range(-3, 4):
        B += basis_at(n, k, np.arange(n) + s * n) * 0  # placeholder
    xs = np.arange(n)
    for s in (-2, -1, 0, 1, 2):
        B += tp.basis(n, k, 0, xs + s * n)
    return B


def D(nc, kind):
    if kind == "none":
        return np.zeros((1, nc))
    if kind == "circ":
        Dm = np.zeros((nc, nc))
        for j in range(nc):
            for o, w in ((-1, 1.0), (0, -2.0), (1, 1.0)):
                Dm[j, (j + o) % nc] += w
        return Dm
    p = int(kind[-1])
    rows = []
    for j in range(-p, nc - 2 + p):
        r = np.zeros(nc)
        for o, w in ((0, 1.0), (1, -2.0), (2, 1.0)):
            if 0 <= j + o < nc:
                r[j + o] = w
        rows.append(r)
    return np.array(rows)


def models(n, k):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Bp = Bper(n, k)
    lo, hi = -3 * k - 2, n + 3 * k + 2
    xe = np.arange(lo, hi)
    Be = tp.basis(n, k, 0, xe)
    W = np.zeros((len(xe), n))  # periodic extension of the samples
    W[np.arange(len(xe)), xe % n] = 1.0
    Bi = tp.basis(
        10 * n, k, 0, np.arange(-4 * n, 5 * n)
    )  # wide grid for an 'infinite' Toeplitz Gram
    out = {}
    for pen in ("none", "p1", "circ"):
        for lam in (0.0, 0.002, 0.005):
            if pen == "none" and lam > 0 or pen != "none" and lam == 0:
                continue
            P = lam**2 * D(nc, pen).T @ D(nc, pen)
            out[f"periodic Gram + B^T f, {pen} {lam}"] = np.linalg.solve(Bp.T @ Bp + P, B.T @ B)
            out[f"periodic basis both, {pen} {lam}"] = np.linalg.solve(Bp.T @ Bp + P, Bp.T @ B)
            out[f"extended periodic samples, {pen} {lam}"] = np.linalg.solve(
                Be.T @ Be + P, Be.T @ W @ B
            )
    return out


def score(Mf, Mm):
    s = np.median(np.diag(Mf)) / np.median(np.diag(Mm))
    return np.sqrt(((Mf - s * Mm) ** 2).mean())


for a, n in ((0, 32), (2, 20)):
    base = score(M[a], np.eye(len(M[a])))
    print(f"axis {a} (n {n}): identity {base:.2e}")
    res = sorted((score(M[a], Mm), name) for name, Mm in models(n, 3).items())
    for r, name in res[:6]:
        print(f"   {r:.2e}  {name}")

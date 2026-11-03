# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Refits solved approximately by conjugate gradients (3D, diagonal preconditioner, from 0,
|r|/|b| < tol): probes E (same grid, same k), B (k 2 -> 1) and scg (16 -> 32, k 3 -> 2)."""

import os
import sys

import numpy as np
import scipy.sparse as sp

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

exec(
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "t_trans10.py"))
    .read()
    .split("# probe scg")[0]
    .split("import tp\n", 1)[1]
)


def D(nc, p):
    rows = []
    for j in range(-p, nc - 2 + p):
        r = np.zeros(nc)
        for o, w in ((0, 1.0), (1, -2.0), (2, 1.0)):
            if 0 <= j + o < nc:
                r[j + o] = w
        rows.append(r)
    return np.array(rows)


def pcg(A, b, x0, tol, maxit=500):
    x = x0.copy()
    r = b - A @ x
    Minv = 1 / A.diagonal()
    z = Minv * r
    p = z.copy()
    rz = r @ z
    nb = np.linalg.norm(b)
    for it in range(maxit):
        if np.linalg.norm(r) / nb < tol:
            return x
        Ap = A @ p
        al = rz / (p @ Ap)
        x += al * p
        r -= al * Ap
        z = Minv * r
        rz2 = r @ z
        p = z + (rz2 / rz) * p
        rz = rz2
    return x


def refit3(f, n, k, lam, p, tol, x0=None, joint=True):
    Bs = [sp.csr_matrix(tp.basis(n[a], k)) for a in range(3)]
    B = sp.kron(sp.kron(Bs[2], Bs[1]), Bs[0]).tocsr()
    A = B.T @ B
    if lam:
        ncs = [Bs[a].shape[1] for a in range(3)]
        I = [sp.identity(ncs[a]) for a in range(3)]
        Ds = [sp.csr_matrix(D(ncs[a], p)) for a in range(3)]
        R = (
            sp.kron(sp.kron(I[2], I[1]), Ds[0].T @ Ds[0])
            + sp.kron(sp.kron(I[2], Ds[1].T @ Ds[1]), I[0])
            + sp.kron(sp.kron(Ds[2].T @ Ds[2], I[1]), I[0])
        )
        A = A + lam**2 * R
    b = B.T @ f.flatten(order="F")
    x0 = np.zeros(A.shape[0]) if x0 is None else x0
    x = pcg(A.tocsr(), b, x0, tol)
    return x.reshape([Bs[a].shape[1] for a in range(3)], order="F")


# probe E
f1 = load("E", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("E", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n = np.array(f1.shape)
vox = np.array([12.0, 12.0, 13.199997])
Bb = [tp.basis(n[a], 2) for a in range(3)]
print("E: FSL E2 23.66, change rms 0.0140")
for tol in (1e-2, 3e-3, 1e-3, 3e-4, 1e-4):
    for lam, p in ((0.005, 1), (0.0, 0)):
        c = refit3(f1, n, 2, lam, p, tol)
        f = tp.apply(c, Bb)
        print(
            f"  tol {tol:g} lam {lam} p {p}: E {E(c, n, 2, vox):.3f} rms vs FSL {np.sqrt(((f - f2) ** 2).mean()):.5f} change {np.sqrt(((f - f1) ** 2).mean()):.4f}"
        )

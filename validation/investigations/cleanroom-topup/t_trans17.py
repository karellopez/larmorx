# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe G (s 1, k 3 -> 3, warpres 18 -> 20, level 2 miter 0): FSL's exact refit of a field in
its own space (its _fieldcoef) against penalty models; probe F (k 3 -> 2) the same way."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


def D(nc, p):
    rows = []
    for j in range(-p, nc - 2 + p):
        r = np.zeros(nc)
        for o, w in ((0, 1.0), (1, -2.0), (2, 1.0)):
            if 0 <= j + o < nc:
                r[j + o] = w
        rows.append(r)
    return np.array(rows)


def kron3(mx, my, mz):  # x fastest: index = x + nx*(y + ny*z) -> np order (z,y,x)
    return np.kron(np.kron(mz, my), mx)


def vec(c):  # c[x,y,z] -> x fastest
    return c.transpose(2, 1, 0).ravel()


def unvec(v, nc):
    return v.reshape(nc[2], nc[1], nc[0]).transpose(2, 1, 0)


for probe, k1, k2 in (("G", 3, 3), ("F", 3, 2)):
    f1 = load(probe, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
    f2 = load(probe, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
    cF = load(probe, "pr_fieldcoef.nii.gz")
    n = np.array(f1.shape)
    B1 = [tp.basis(n[a], k1) for a in range(3)]
    B2 = [tp.basis(n[a], k2) for a in range(3)]
    c1 = tp.apply(f1, [np.linalg.pinv(B1[a]) for a in range(3)])
    print(
        f"== probe {probe}: grid {n} coef {cF.shape}; layout |B cF - f2| {np.abs(tp.apply(cF, B2) - f2).max():.2e}; |B c1 - f1| {np.abs(tp.apply(c1, B1) - f1).max():.2e}"
    )
    print(
        f"   FSL: field change rms {np.sqrt(((f2 - f1) ** 2).mean()):.6f} max {np.abs(f2 - f1).max():.5f}"
    )

    def rep(name, c):
        f = tp.apply(c, B2)
        print(
            f"   {name}: field rms vs FSL {np.sqrt(((f - f2) ** 2).mean()):.6f} max {np.abs(f - f2).max():.5f}; coef rms diff {np.sqrt(((c - cF) ** 2).mean()):.5f}"
        )

    rep("pure LS (no penalty)", tp.apply(f1, [np.linalg.pinv(B2[a]) for a in range(3)]))
    for p in (0, 1, 2):
        for lam in (0.002, 0.005, 0.01):
            ops = []
            for a in range(3):
                B = B2[a]
                Dm = D(B.shape[1], p)
                ops.append(np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T))
            rep(f"sep p{p} lam {lam}", tp.apply(f1, ops))
    # joint 3D: G^T G + lam^2 * sum_a D_a^T D_a (Kronecker sum)
    nc = np.array([B2[a].shape[1] for a in range(3)])
    G = kron3(B2[0], B2[1], B2[2])
    GtG = G.T @ G
    Gtf = G.T @ vec(f1)
    for p in (0, 1, 2):
        Ps = [D(nc[a], p).T @ D(nc[a], p) for a in range(3)]
        I = [np.eye(nc[a]) for a in range(3)]
        P = kron3(Ps[0], I[1], I[2]) + kron3(I[0], Ps[1], I[2]) + kron3(I[0], I[1], Ps[2])
        for lam in (0.002, 0.005, 0.01):
            rep(f"joint ksum p{p} lam {lam}", unvec(np.linalg.solve(GtG + lam**2 * P, Gtf), nc))
    # what FSL's change implies for the joint model: GtG (c_LS - cF) = lam^2 P cF
    np.save(os.path.join(R, f"{probe}_c1.npy"), c1)

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from vbm import *

S = os.environ["VB_DIR"] + "/p1/"
y = [a.ravel(order="F") for a in load(S + "vb2D", 2)]
n = 8
K = np.vstack([np.eye(n)] * 2)
KtK = K.T @ K
Y = np.concatenate(y)
lsr_fsl = out(S + "lsr").ravel(order="F")
SS = sopen(n).T @ sopen(n)
yeff = (KtK + 0.01 * SS) @ lsr_fsl
Ye = np.concatenate([y[0], yeff - y[0]])
Q1 = circ_line(n)
Q2 = lap_sq((1, 8, 1), 1)[:, :][:8, :8] if False else None


def run(Ye, Q, iters, a0=1e-10, b0=1e10):
    N = len(Ye)
    nn = Q.shape[0]
    Sig = np.linalg.inv(KtK + Q1)
    f = Sig @ (K.T @ Ye)
    phi = lam = 1.0
    res = [(lam / phi, phi, lam)]
    for it in range(iters):
        r = Ye - K @ f
        phi = (a0 + N // 2) / (1 / b0 + 0.5 * (r @ r + np.trace(KtK @ Sig)))
        lam = (a0 + nn // 2) / (1 / b0 + 0.5 * (f @ Q @ f + np.trace(Q @ Sig)))
        Sig = np.linalg.inv(phi * KtK + lam * Q)
        f = phi * Sig @ (K.T @ Ye)
        res.append((lam / phi, phi, lam))
    return res, f


for m, shape, it in [("vb2D", (1, 8), 50), ("vb3D", (1, 8, 1), 20)]:
    Q = lap_sq(shape, 1)
    fsl = parse(S + m + "/STDOUT.txt")
    for lab, YY in [("raw", Y), ("eff", Ye)]:
        mine, f = run(YY, Q, it)
        rel = [max(abs(a - b) / abs(b) for a, b in zip(x, z)) for x, z in zip(mine, fsl)]
        o = out(S + m).ravel(order="F")
        print(
            m,
            lab,
            len(fsl),
            len(mine),
            "max rel printed %.2g" % max(rel),
            "first >1e-5",
            next((i for i, v in enumerate(rel) if v > 1e-5), None),
            "out rel %.2g" % np.max(np.abs(f - o) / np.abs(o)),
        )

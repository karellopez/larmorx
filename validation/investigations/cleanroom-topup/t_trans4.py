# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, name):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{name}").dataobj, float)


def rf(n, k, lam=0.005, pad=None):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


cases = {"scg": ("scg", 2, 2), "C": ("C", 2, 4)}
for nm, (p, k1, k2) in cases.items():
    f1 = load(p, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
    f2 = load(p, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
    n1 = np.array(f1.shape)
    n2 = np.array(f2.shape)
    c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], k1)) for a in range(3)])
    pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
    fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
    res = []
    for K1 in (1, 2, 3, 4, 5, 6, 8):
        for lam in (0.0, 0.005):
            f = tp.apply(
                tp.apply(fs, [rf(n2[a], K1, lam) for a in range(3)]),
                [tp.basis(n2[a], K1) for a in range(3)],
            )
            if k2 != K1:
                f = tp.apply(
                    tp.apply(f, [rf(n2[a], k2, lam) for a in range(3)]),
                    [tp.basis(n2[a], k2) for a in range(3)],
                )
            res.append((np.sqrt(((f - f2) ** 2).mean()), K1, lam))
    res.sort()
    print(nm, [(round(r, 4), K, l) for r, K, l in res])

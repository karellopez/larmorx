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


def rf(n, k, lam=0.005):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


if len(sys.argv) > 1:
    f1 = load("B", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
    f2 = load("B", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
    n = np.array(f1.shape)
    c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n[a], 2)) for a in range(3)])
    for lam in (0.0, 0.005, 0.01, 0.02, 0.05, 0.1):
        f = tp.apply(
            tp.apply(f1, [rf(n[a], 1, lam) for a in range(3)]),
            [tp.basis(n[a], 1) for a in range(3)],
        )
        print("lam", lam, "max", np.abs(f - f2).max(), "rms", np.sqrt(((f - f2) ** 2).mean()))
    # 1D: along x only? compare the lines' difference structure
    d = f2 - f1
    np.set_printoptions(precision=4, suppress=True, linewidth=250)
    print("d line x", d[:, 8, 5])
    print("d line y", d[8, :, 5])
    print("d line z", d[8, 8, :])
    print("f1 line x", f1[:, 8, 5])
    sys.exit()
for p, k1, k2, step in (("A", 2, 2, 1), ("B", 2, 1, 1), ("C", 2, 4, 0.5)):
    print(open(f"{R}/fsl-probe-{p}/STDOUT.txt").read().strip().replace("\n", " | "))
    f1 = load(p, "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
    f2 = load(p, "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
    n1 = np.array(f1.shape)
    n2 = np.array(f2.shape)
    c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], k1)) for a in range(3)])
    pos = [(step - 1) / 2 + np.arange(n2[a]) * step for a in range(3)]
    fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
    print(p, "n1", n1, "n2", n2, "f2 - f1 sampled: max", np.abs(fs - f2).max())
    for kp in sorted({k1, k2, 2 * k1}):
        c = tp.apply(fs, [rf(n2[a], kp) for a in range(3)])
        f = tp.apply(c, [tp.basis(n2[a], kp) for a in range(3)])
        if kp != k2:
            f = tp.apply(
                tp.apply(f, [rf(n2[a], k2) for a in range(3)]),
                [tp.basis(n2[a], k2) for a in range(3)],
            )
        print(
            "   refit via k",
            kp,
            "-> k",
            k2,
            "max",
            np.abs(f - f2).max(),
            "rms",
            np.sqrt(((f - f2) ** 2).mean()),
        )

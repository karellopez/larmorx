# SPDX-License-Identifier: Apache-2.0
"""Level transition (subsampling 2 -> 1): sample + refit conventions vs FSL's level-2 start field."""

import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

D = sys.argv[1]
f1 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel01_iter03_attempt01.nii.gz").dataobj, float)
f2 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel02_iter00_attempt01.nii.gz").dataobj, float)
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
k1 = 2
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], k1)) for a in range(3)])
print("c1 refit err", np.abs(tp.apply(c1, [tp.basis(n1[a], k1) for a in range(3)]) - f1).max())


def refit1d(n, k, lam=0.005):
    B = tp.basis(n, k)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


def sample_at(c, n, k, pos):
    return tp.apply(c, [tp.basis(n[a], k, 0, pos[a]) for a in range(3)])


step = 0.5
pos = [(step - 1) / 2 + np.arange(n2[a]) * step for a in range(3)]
fs = sample_at(c1, n1, k1, pos)
for name, kprev, lam in (
    ("k=2 new voxels", 2, 0.005),
    ("k=4 new voxels then k=2", 4, 0.005),
    ("k=2 no reg", 2, 0.0),
):
    c = tp.apply(fs, [refit1d(n2[a], kprev, lam) for a in range(3)])
    if kprev != 2:
        f_mid = tp.apply(c, [tp.basis(n2[a], kprev) for a in range(3)])
        c = tp.apply(f_mid, [refit1d(n2[a], 2, lam) for a in range(3)])
    f = tp.apply(c, [tp.basis(n2[a], 2) for a in range(3)])
    print(name, "max |f - fsl|", np.abs(f - f2).max(), "max", np.abs(f2).max())
# alternative sampling: plain scaling j*step
pos2 = [np.arange(n2[a]) * step for a in range(3)]
fs2 = sample_at(c1, n1, k1, pos2)
c = tp.apply(fs2, [refit1d(n2[a], 2) for a in range(3)])
print("scaled sampling", np.abs(tp.apply(c, [tp.basis(n2[a], 2) for a in range(3)]) - f2).max())
c = tp.apply(fs, [refit1d(n2[a], 2) for a in range(3)])
f = tp.apply(c, [tp.basis(n2[a], 2) for a in range(3)])
d = np.abs(f - f2)
print("interior max", d[3:-3, 3:-3, 3:-3].max(), "per-x", np.round(d.max((1, 2)), 3).tolist())
print("per-y", np.round(d.max((0, 2)), 3).tolist())
print("per-z", np.round(d.max((0, 1)), 3).tolist())
print(
    "sampled-only diff interior",
    np.abs(fs - f2)[3:-3, 3:-3, 3:-3].max(),
    "all",
    np.abs(fs - f2).max(),
)
x_ = fs[3:-3, 3:-3, 3:-3].ravel()
y_ = f2[3:-3, 3:-3, 3:-3].ravel()
print("slope", (x_ @ y_) / (x_ @ x_), "corr", np.corrcoef(x_, y_)[0, 1])
r_ = f2 - fs
print(
    "diff stats",
    r_.mean(),
    r_.std(),
    "smoothness: diff of neighbours",
    np.abs(np.diff(r_, axis=0)).mean(),
)
np.set_printoptions(precision=2, suppress=True, linewidth=250)
print("line y=16 z=10: fsl", f2[:, 16, 10])
print("        mine", fs[:, 16, 10])
print("        diff", (f2 - fs)[:, 16, 10])
print("line x=16 z=10 diff", (f2 - fs)[16, :, 10])
print("line x=16 y=16 diff", (f2 - fs)[16, 16, :])
print("old field line", f1[:, 8, 5])
for lr in ():
    for kp in (2, 4):
        for o in (-0.25, 0.0):
            pos = [o + np.arange(n2[a]) * 0.5 for a in range(3)]
            fsx = sample_at(c1, n1, k1, pos)
            c = tp.apply(fsx, [refit1d(n2[a], kp, lr) for a in range(3)])
            f = tp.apply(c, [tp.basis(n2[a], kp) for a in range(3)])
            if kp != 2:
                c = tp.apply(f, [refit1d(n2[a], 2, 0.005) for a in range(3)])
                f = tp.apply(c, [tp.basis(n2[a], 2) for a in range(3)])
            print(
                "lr",
                lr,
                "kp",
                kp,
                "o",
                o,
                "max",
                np.abs(f - f2).max(),
                "rms",
                np.sqrt(((f - f2) ** 2).mean()),
            )
for o in []:
    for st in (0.5, 0.5 * 32 / 33, 0.5 * 31 / 32, 15 / 31):
        pos = [o + np.arange(n2[a]) * st for a in range(3)]
        fsx = sample_at(c1, n1, k1, pos)
        dd = np.abs(fsx - f2)[3:-3, 3:-3, 3:-3].max()
        if dd < 1.0:
            print("offset", o, "step", st, "interior max", dd)

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probe E (same grid, same knot spacing 2, warpres 20 -> 22): where FSL's refit changes the
coefficients, per index along each axis."""

import sys, os, numpy as np, nibabel as nib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

R = os.environ["TOPUP_RUNS"]


def load(p, n):
    return np.asarray(nib.load(f"{R}/fsl-probe-{p}/{n}").dataobj, float)


f1 = load("E", "TopupDebugFieldlevel01_iter03_attempt01.nii.gz")
f2 = load("E", "TopupDebugFieldlevel02_iter00_attempt01.nii.gz")
n = np.array(f1.shape)
print("grid", n)
B = [tp.basis(n[a], 2) for a in range(3)]
c1 = tp.apply(f1, [np.linalg.pinv(B[a]) for a in range(3)])
c2 = tp.apply(f2, [np.linalg.pinv(B[a]) for a in range(3)])
print("fit residuals", np.abs(tp.apply(c1, B) - f1).max(), np.abs(tp.apply(c2, B) - f2).max())
d = c2 - c1
np.set_printoptions(precision=4, suppress=True, linewidth=250)
for a in range(3):
    ax = tuple(i for i in range(3) if i != a)
    print("axis", a, "rms change per index", np.sqrt((d**2).mean(axis=ax)))
    print("axis", a, "rms coef per index  ", np.sqrt((c1**2).mean(axis=ax)))
# field change pattern on the grid
df = f2 - f1
for a in range(3):
    ax = tuple(i for i in range(3) if i != a)
    print("axis", a, "field change rms per voxel", np.sqrt((df**2).mean(axis=ax)))
np.save(os.path.join(R, "E_c1.npy"), c1)
np.save(os.path.join(R, "E_c2.npy"), c2)

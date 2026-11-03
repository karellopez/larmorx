# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# The Euler order and the matrix's centre from FSL's four-volume case (rotations up to 0.05 rad).
import itertools, sys
import numpy as np, nibabel as nib
from pathlib import Path

d = Path(sys.argv[1])
mp = np.loadtxt(d / "o_movpar.txt")
img = nib.load(str(d / "four.nii.gz"))
n = np.array(img.shape[:3])
dx = np.asarray(img.header.get_zooms()[:3], float)


def R1(a, ang):
    c, s = np.cos(ang), np.sin(ang)
    m = np.eye(3)
    i, j = [(1, 2), (0, 2), (0, 1)][a]
    sg = -1 if a == 1 else 1
    m[i, i] = c
    m[j, j] = c
    m[i, j] = sg * s
    m[j, i] = -sg * s
    return m


for order in itertools.permutations(range(3)):
    errs = []
    for v in (1, 2, 3):
        M = np.loadtxt(d / f"r_{v + 1:02d}.mat")
        r = mp[v, 3:]
        t = mp[v, :3]
        Rs = [R1(a, r[a]) for a in range(3)]
        R = Rs[order[0]] @ Rs[order[1]] @ Rs[order[2]]
        c = (n - 1) * dx / 2
        T = t + c - R @ c
        errs.append((np.abs(R - M[:3, :3]).max(), np.abs(T - M[:3, 3]).max()))
    print(order, np.array(errs).max(0))

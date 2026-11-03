# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Fit the rigid-body matrix convention from FSL's movpar and rbmout (debug/movements).
import itertools, sys
import numpy as np
from pathlib import Path

d = Path(sys.argv[1])
mp = np.loadtxt(d / "mov_movpar.txt")[1]
M = np.loadtxt(d / "mov_xfm_02.mat")
t, r = mp[:3], mp[3:]


def R1(a, ang, sgn):
    c, s = np.cos(ang), np.sin(ang)
    m = np.eye(3)
    i, j = [(1, 2), (0, 2), (0, 1)][a]
    m[i, i] = c
    m[j, j] = c
    m[i, j] = sgn * s
    m[j, i] = -sgn * s
    return m


best = []
for order in itertools.permutations(range(3)):
    for sg in itertools.product((1, -1), repeat=3):
        Rs = [R1(a, r[a], sg[a]) for a in range(3)]
        R = Rs[order[0]] @ Rs[order[1]] @ Rs[order[2]]
        err = np.abs(R - M[:3, :3]).max()
        best.append((err, order, sg, R))
best.sort(key=lambda x: x[0])
for e, o, s, R in best[:3]:
    print(f"{e:.3g} order {o} signs {s}")
R = best[0][3]
for name, c in {
    "full (n-1)v/2": np.array([63 * 3, 63 * 3, 39 * 3.3]) / 2,
    "sub (n-1)v/2": np.array([31 * 6, 31 * 6, 19 * 6.6]) / 2,
    "full n v/2": np.array([64 * 3, 64 * 3, 40 * 3.3]) / 2,
}.items():
    for sgn in (1, -1):
        T = sgn * t + c - R @ c
        print(name, sgn, np.abs(T - M[:3, 3]).max(), T)
print("residual (best)\n", best[0][3] - M[:3, :3])
print("residual (0,2,1)\n", best[1][3] - M[:3, :3])
c = np.array([63 * 3, 63 * 3, 39 * 3.2999992]) / 2
print("T residual", t + c - best[0][3] @ c - M[:3, 3])

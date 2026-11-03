# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from vbg import *

V = os.environ["VB_DIR"] + "/"
fl = lambda a: a.reshape(-1, order="F")
P, m = sys.argv[1], sys.argv[2]
d = V + P + "/" + m
ys = effective(V + P + "/vb3D", 2)
f = out(d)
fsl = parse(d + "/STDOUT.txt")
sh = f.shape
if m == "vb2D":
    z = int(sys.argv[3])
    rl = [l for l in open(d + "/STDOUT.txt") if l.startswith(f"slice = {z},")][0]
    rl = float(rl.split("relative lambda = ")[1].split(",")[0])
    ys = [y[:, :, z] for y in ys]
    f = f[:, :, z]
    sh = f.shape
else:
    rl = fsl[-1][0]
rhs = fl(ys[0] + ys[1] - 2 * f) / rl
ff = fl(f)
nd_ = len(sh)
basis = {"c": {(0,) * nd_: 1}}
for a in range(nd_):
    for k, s in [("1", 1), ("2", 2)]:
        o1 = [0] * nd_
        o1[a] = s
        o2 = [0] * nd_
        o2[a] = -s
        basis[f"{'xyz'[a]}{k}"] = {tuple(o1): 1, tuple(o2): 1}
for a in range(nd_):
    for b in range(a + 1, nd_):
        st = {}
        for s in (1, -1):
            for t in (1, -1):
                o = [0] * nd_
                o[a] = s
                o[b] = t
                st[tuple(o)] = 1
        basis["xyz"[a] + "xyz"[b]] = st
cols = []
names = []
for k, st in basis.items():
    v = stencil_Q(sh, 1, st) @ ff
    if np.any(v):
        cols.append(v)
        names.append(k)
A = np.array(cols).T
th, *_ = np.linalg.lstsq(A, rhs, rcond=None)
print(
    P,
    m,
    {n: float("%.5g" % t) for n, t in zip(names, th)},
    "resid rel %.2g" % (np.linalg.norm(A @ th - rhs) / np.linalg.norm(rhs)),
)

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from vbm import *

V = os.environ["VB_DIR"] + "/"
fl = lambda a: a.reshape(-1, order="F")
for P in ["p2", "p3"]:
    for m in ["vb3D"]:
        S = V + P + "/" + m + "/"
        y1, y2 = y2_effective(S)
        sh = y1.shape
        N = y1.size
        I = np.eye(N)
        for yz in [2.0, 1.0]:
            Q = stencil_Q(sh, 1, sten3(yz=yz))
            res, f = gs_vb([fl(y1), fl(y2)], [I, I], sh, 1, Q, 20)
            print(P, m, "yz", yz, compare(res, parse(S + "STDOUT.txt"), f, fl(out(S))))

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from vbm import *

V = os.environ["VB_DIR"] + "/"
fl = lambda a: a.reshape(-1, order="F")
for P in ["p4", "p6"]:
    S = V + P + "/vb3D/"
    y1, y2e = y2_effective(S) if P == "p4" else (None, None)
    if P == "p4":
        y2 = load(S, 2)[1]
        d = np.abs(y2e - y2)
        print(
            P,
            "dev max",
            d.max(),
            "p50",
            np.median(d),
            "where max",
            np.unravel_index(d.argmax(), d.shape),
        )
        sh = y1.shape
        N = y1.size
        I = np.eye(N)
        for z2 in [1.0, 0.5, 0.0]:
            Q = stencil_Q(sh, 1, sten3(yz=1.0, z2=z2))
            for m in ["vb3D", "vb4D"]:
                res, f = gs_vb([fl(y1), fl(y2e)], [I, I], sh, 1, Q, 20)
                print(
                    P,
                    m,
                    "z2",
                    z2,
                    compare(
                        res, parse(V + P + "/" + m + "/STDOUT.txt"), f, fl(out(V + P + "/" + m))
                    ),
                )

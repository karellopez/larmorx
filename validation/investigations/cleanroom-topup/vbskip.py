# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import vbg, vbt, numpy as np


def skip(shape):
    b = np.zeros(shape, bool)
    b[0] = True
    return b.reshape(-1, order="F")


vbg.SKIP = skip
for vc in [False, True]:
    vbg.VALIDCOUNT = vc
    print("valid count", vc)
    vbt.run("p11", "vb2D")
    vbt.run("p11", "vb3D")

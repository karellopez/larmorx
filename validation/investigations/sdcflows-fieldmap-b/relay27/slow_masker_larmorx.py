# SPDX-License-Identifier: Apache-2.0
"""larmorx's brain_masker on a difftest case whose random walker does not converge (sdcflows-
estimation, seed fmapb-try1, case 492): the time and conjugate-gradient iterations (2 x 162,120,
209 s at one thread on the main machine, 2026-11-09).

    python -P slow_masker_larmorx.py 492 1     (case number, threads)
"""

import sys
import time

import numpy as np
from larmorx_validation.difftest import core, inputs, tools

import larmorx.sdcflows.masks as m
from larmorx import _core

r = core.make_recipe(tools.get("sdcflows-estimation"), "fmapb-try1", int(sys.argv[1]))
prefix = inputs.register(r["inputs"])
src = str(inputs.path(inputs.key(prefix, "in")))
img = m._load(src, np.float32)
data = np.ascontiguousarray(img.data, dtype=np.float32)
zooms = [float(np.float32(z)) for z in img.header.zooms[:3]]
t0 = time.perf_counter()
prob, mask, th, iters, warns = _core.sdcflows_brain_masker(
    data, zooms, 5, np.__version__, "1.18.1", True, int(sys.argv[2])
)
print(
    "larmorx",
    time.perf_counter() - t0,
    "s; iterations",
    iters,
    "threshold",
    th,
    warns[:2],
    flush=True,
)

# SPDX-License-Identifier: Apache-2.0
"""Probe: digest of numpy's float32 log over every non-negative float32 bit pattern (incl. inf/NaN),
in chunks; run under different NPY_DISABLE_CPU_FEATURES to compare dispatch paths."""

import hashlib
import os

import numpy as np

h = hashlib.sha256()
step = 1 << 26
with np.errstate(all="ignore"):
    for start in range(0, 1 << 31, step):
        x = np.arange(start, start + step, dtype=np.uint32).view(np.float32)
        h.update(np.log(x).tobytes())
print(np.__version__, os.environ.get("NPY_DISABLE_CPU_FEATURES", "-"), "log_f32_all", h.hexdigest())

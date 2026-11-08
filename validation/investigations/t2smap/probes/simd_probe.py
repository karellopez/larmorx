# SPDX-License-Identifier: Apache-2.0
"""Probe: does numpy's float32 log / float64 exp depend on the SIMD path? Writes digests to compare."""

import hashlib
import os

import numpy as np

out = {}
# float32 log over every float32 in [1, 2^17) (log(|x|+1) domain of typical MRI magnitudes),
x = np.arange(
    np.float32(1).view(np.uint32), np.float32(131072).view(np.uint32), dtype=np.uint32
).view(np.float32)
out["log_f32"] = hashlib.sha256(np.log(x).tobytes()).hexdigest()
# float64 exp on a deterministic grid of arguments in [-20, 20]
rng = np.random.default_rng(0)
y = rng.uniform(-20, 20, 20_000_000)
out["exp_f64"] = hashlib.sha256(np.exp(y).tobytes()).hexdigest()
out["log_f64"] = hashlib.sha256(np.log(np.abs(y) + 1).tobytes()).hexdigest()
z = rng.uniform(-3, 0, 20_000_000)
out["exp_f64_neg"] = hashlib.sha256(np.exp(z).tobytes()).hexdigest()
print(np.__version__, os.environ.get("NPY_DISABLE_CPU_FEATURES", "-"), out)

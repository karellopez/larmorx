# SPDX-License-Identifier: Apache-2.0

"""Probe: do numpy's float32/float64 exp (and log) depend on the CPU dispatch (AVX512F vs AVX2 vs
baseline)? Writes the raw bits of np.exp over a fixed set of inputs to an .npy file named by the
environment's dispatch, so runs with different NPY_DISABLE_CPU_FEATURES can be compared.

    python -I probe_numpy_exp.py <out_dir>
"""

import hashlib
import os
import sys

import numpy as np

out = sys.argv[1]
rng = np.random.default_rng(0)
# The random walker's arguments: scale * gradient**2 with scale < 0: values in [-80, 0].
x32 = np.concatenate(
    [
        -rng.random(2_000_000, dtype=np.float32) * np.float32(80.0),
        np.linspace(-103.0, 88.0, 1_000_001, dtype=np.float32),
    ]
).astype(np.float32)
x64 = x32.astype(np.float64) * 1.000000123
res = {
    "exp32": np.exp(x32),
    "exp64": np.exp(x64),
    "log32": np.log(np.abs(x32) + np.float32(1e-3)),
    "log64": np.log(np.abs(x64) + 1e-3),
    "sqrt32": np.sqrt(np.abs(x32)),
}
feat = os.environ.get("NPY_DISABLE_CPU_FEATURES", "") or "all"
try:
    from numpy._core._multiarray_umath import __cpu_features__ as cf

    on = [k for k in ("AVX2", "FMA3", "AVX512F", "AVX512_SKX") if cf.get(k)]
except Exception:
    on = []
print("numpy", np.__version__, "disabled:", feat, "enabled:", on)
for k, v in res.items():
    h = hashlib.sha256(v.tobytes()).hexdigest()[:16]
    print(f"  {k}: {h}")
np.savez(os.path.join(out, f"exp_{np.__version__}_{feat.replace(' ', '-')}.npz"), **res)

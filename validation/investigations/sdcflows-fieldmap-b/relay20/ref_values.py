# SPDX-License-Identifier: Apache-2.0
"""SciPy's values behind ``tests/python/test_regrid_scipy_inv.py``'s derivative and complex tests
(float hex). Run in each environment with ``OPENBLAS_CORETYPE=Haswell``; the outputs are the
same. It re-runs itself with numpy's AVX-512 dispatch off (numpy's float64 ``exp`` is glibc's)."""

import os
import sys

NO_AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
if os.environ.get("NPY_DISABLE_CPU_FEATURES") != NO_AVX512:
    os.environ["NPY_DISABLE_CPU_FEATURES"] = NO_AVX512
    os.execv(sys.executable, [sys.executable, "-P", *sys.argv])

import numpy as np  # noqa: E402
from scipy import ndimage as snd  # noqa: E402
from scipy.ndimage._filters import _gaussian_kernel1d as kernel  # noqa: E402

print("k1", [float(v).hex() for v in kernel(1.5, 3, 2)])
print("k2", [float(v).hex() for v in kernel(0.8, 5, 3)])
print("k0", [float(v).hex() for v in kernel(1.0, 2, 0)])
c = snd.correlate1d(
    np.array([1 + 2j, 3 - 1j, -2 + 0.5j, 4j]),
    np.array([0.5 + 0.25j, 1 - 1j, 0.125j]),
    mode="constant",
    cval=1 - 1j,
)
print("c", [(float(v.real).hex(), float(v.imag).hex()) for v in c], c.dtype)
g = snd.gaussian_filter1d(np.arange(9.0) ** 2, 1.0, order=1)
print("g", [float(v).hex() for v in g])

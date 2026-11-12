# SPDX-License-Identifier: Apache-2.0
"""How often glibc's double atan2 and hypot (what SciPy's Rotation.as_euler calls through
libc) differ from the correctly rounded result, on arguments like the quaternion components
of small head rotations and on uniform ones."""

import ctypes
import ctypes.util
import sys

import mpmath
import numpy as np

mpmath.mp.prec = 300
libm = ctypes.CDLL(ctypes.util.find_library("m"))
libm.atan2.restype = ctypes.c_double
libm.atan2.argtypes = [ctypes.c_double, ctypes.c_double]
libm.hypot.restype = ctypes.c_double
libm.hypot.argtypes = [ctypes.c_double, ctypes.c_double]

rng = np.random.default_rng(0)
n = int(sys.argv[1]) if len(sys.argv) > 1 else 100000
sets = {
    "uniform[-1,1]": (rng.uniform(-1, 1, n), rng.uniform(-1, 1, n)),
    # quaternion-like: a ~ 1, b ~ small (rotations below 0.1 rad)
    "small/near-1": (rng.normal(0, 0.02, n), 1 + rng.normal(0, 1e-4, n)),
}
for name, (y, x) in sets.items():
    bad_atan2 = bad_hypot = 0
    for yi, xi in zip(y, x, strict=True):
        a = libm.atan2(yi, xi)
        h = libm.hypot(yi, xi)
        if a != float(mpmath.atan2(mpmath.mpf(yi), mpmath.mpf(xi))):
            bad_atan2 += 1
        if h != float(mpmath.sqrt(mpmath.mpf(yi) ** 2 + mpmath.mpf(xi) ** 2)):
            bad_hypot += 1
    print(f"{name}: {n} pairs; glibc atan2 != correctly rounded: {bad_atan2}; hypot: {bad_hypot}")

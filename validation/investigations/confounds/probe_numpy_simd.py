# SPDX-License-Identifier: Apache-2.0
"""Probe numpy's SIMD paths the confounds reach, in the running environment:

1. float64 ``np.cos`` on the DCT basis' arguments (``(pi / n) * (t + 0.5) * k``) against
   ``math.cos`` (the C library's) and, if mpmath is present, the correctly rounded value;
2. complex64 multiplication (nitime's ``IN1 *= fft(in2)``) against the plain float32 formula
   ``(ac - bd) + (ad + bc) i`` with every product rounded.

Run it once with the default dispatch and once with NPY_DISABLE_CPU_FEATURES set to the
AVX-512 features (or AVX2/FMA3 too) to see which path decides the bits.
"""

import math
import os
import sys

import numpy as np

print("numpy", np.__version__, "python", sys.version.split()[0])
print("NPY_DISABLE_CPU_FEATURES =", os.environ.get("NPY_DISABLE_CPU_FEATURES", ""))
try:
    from numpy._core._multiarray_umath import __cpu_features__ as feats
except Exception:  # numpy < 2
    from numpy.core._multiarray_umath import __cpu_features__ as feats
print("enabled:", " ".join(k for k, v in feats.items() if v and k.startswith(("AVX", "FMA"))))

try:
    import mpmath

    mpmath.mp.prec = 200
except Exception:
    mpmath = None

# 1. cos on the cosine-drift arguments of every series length 2..1200 and order up to 40
args = []
for n in range(2, 1201, 7):
    t = np.arange(n)
    for k in range(1, 41):
        args.append((np.pi / n) * (t + 0.5) * k)
x = np.concatenate(args)
y = np.cos(x)
libm = np.array([math.cos(v) for v in x])
print(f"cos: {x.size} arguments; numpy != libm: {int(np.sum(y != libm))}")
if mpmath is not None:
    sub = x[:: max(1, x.size // 200000)]
    cr = np.array([float(mpmath.cos(mpmath.mpf(float(v)))) for v in sub])
    ys = np.cos(sub)
    ls = np.array([math.cos(v) for v in sub])
    print(
        f"cos subsample {sub.size}: numpy != correctly rounded: {int(np.sum(ys != cr))};"
        f" libm != correctly rounded: {int(np.sum(ls != cr))}"
    )

# 2. complex64 multiply
rng = np.random.default_rng(0)
n = 1 << 20
a, b, c, d = (rng.standard_normal(n).astype(np.float32) * 100 for _ in range(4))
z = (a + 1j * b).astype(np.complex64) * (c + 1j * d).astype(np.complex64)
re_plain = a * c - b * d
im_plain = a * d + b * c
print(
    f"complex64 multiply: {n} pairs; real != plain: {int(np.sum(z.real != re_plain))},"
    f" imag != plain: {int(np.sum(z.imag != im_plain))}"
)
z2 = (a + 1j * b).astype(np.complex64)
z2 *= (c + 1j * d).astype(np.complex64)
print(
    f"complex64 in-place multiply: real != plain: {int(np.sum(z2.real != re_plain))},"
    f" imag != plain: {int(np.sum(z2.imag != im_plain))}"
)

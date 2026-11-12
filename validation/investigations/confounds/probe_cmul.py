# SPDX-License-Identifier: Apache-2.0
"""Which formula numpy's complex64 multiply follows (candidates with one fused multiply-add,
evaluated exactly with fractions and rounded once to float32)."""

import os
from fractions import Fraction

import numpy as np

print("numpy", np.__version__, "disabled:", os.environ.get("NPY_DISABLE_CPU_FEATURES", ""))
rng = np.random.default_rng(1)
n = 20000
a, b, c, d = (rng.standard_normal(n).astype(np.float32) * 100 for _ in range(4))
z = (a + 1j * b).astype(np.complex64)
z *= (c + 1j * d).astype(np.complex64)


def r32(q):
    """Round a Fraction to the nearest float32, ties to even, exactly."""
    x = np.float32(float(q))
    cands = [np.nextafter(x, np.float32(-np.inf)), x, np.nextafter(x, np.float32(np.inf))]

    def key(v):
        bits = int(np.frombuffer(np.float32(v).tobytes(), np.uint32)[0])
        return (abs(Fraction(float(v)) - q), bits & 1)

    return min(cands, key=key)


def F(v):
    return Fraction(float(v))


counts = {
    k: [0, 0] for k in ["plain", "fma(a,c,-bd)", "fma(-b,d,ac)", "fma(a,d,bc)", "fma(b,c,ad)"]
}
for i in range(n):
    A, B, C, D = a[i], b[i], c[i], d[i]
    ac, bd, ad, bc = np.float32(A * C), np.float32(B * D), np.float32(A * D), np.float32(B * C)
    re, im = z.real[i], z.imag[i]
    counts["plain"][0] += bool(re == np.float32(ac - bd))
    counts["plain"][1] += bool(im == np.float32(ad + bc))
    counts["fma(a,c,-bd)"][0] += bool(re == r32(F(A) * F(C) - F(bd)))
    counts["fma(-b,d,ac)"][0] += bool(re == r32(F(ac) - F(B) * F(D)))
    counts["fma(a,d,bc)"][1] += bool(im == r32(F(A) * F(D) + F(bc)))
    counts["fma(b,c,ad)"][1] += bool(im == r32(F(ad) + F(B) * F(C)))
for k, (r, i_) in counts.items():
    print(f"{k:14s} real {r}/{n}  imag {i_}/{n}")

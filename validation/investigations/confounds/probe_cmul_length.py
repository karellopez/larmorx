# SPDX-License-Identifier: Apache-2.0
"""Which formula numpy's complex64 product follows by array length, in place (``z *= b``, as
nitime multiplies) and into a new array (``a * b``).

Candidates, evaluated exactly with fractions and rounded once to float32: ``plain`` (every
product rounded: numpy's scalar loop), ``fma1`` (``fma(a.re, b.re, -(a.im b.im))``, ``fma(a.re,
b.im, a.im b.re)``: the SIMD loop's ``muladdsub``), ``fma2`` (the other product fused). Measured
2026-11-12 in numpy 2.5.4, 2.3.5 and 2.2.6 (fMRIPrep 25.2.6's image) on the main machine: the
one-element in-place product is ``plain`` (300 of 300; numpy's iterator gives the one-element
output a stride of 0, which sends the loop to its scalar branch), every other case ``fma1``.
Run it in each environment: ``python -P probe_cmul_length.py``.
"""

from fractions import Fraction

import numpy as np


def exact(v) -> Fraction:
    return Fraction(float(v))


def r32(q: Fraction) -> np.float32:
    """``q`` rounded to the nearest float32, ties to even."""
    x = np.float32(float(q))
    cands = [np.nextafter(x, np.float32(-np.inf)), x, np.nextafter(x, np.float32(np.inf))]
    return min(cands, key=lambda v: (abs(exact(v) - q), int(np.float32(v).view(np.uint32)) & 1))


def main() -> None:
    print("numpy", np.__version__)
    rng = np.random.default_rng(7)
    counts: dict[tuple[int, str, str], int] = {}
    for n in (1, 2, 3, 4, 5, 8):
        for _ in range(300):
            a = (rng.standard_normal(n) * 3e5 + 1j * rng.standard_normal(n) * 3e5).astype(
                np.complex64
            )
            b = (rng.standard_normal(n) * 3e5 + 1j * rng.standard_normal(n) * 3e5).astype(
                np.complex64
            )
            z = a.copy()
            z *= b
            new = a * b
            for i in range(n):
                ar, ai, br, bi = a.real[i], a.imag[i], b.real[i], b.imag[i]
                forms = {
                    "plain": (
                        np.float32(np.float32(ar * br) - np.float32(ai * bi)),
                        np.float32(np.float32(ar * bi) + np.float32(ai * br)),
                    ),
                    "fma1": (
                        r32(exact(ar) * exact(br) - exact(np.float32(ai * bi))),
                        r32(exact(ar) * exact(bi) + exact(np.float32(ai * br))),
                    ),
                    "fma2": (
                        r32(exact(np.float32(ar * br)) - exact(ai) * exact(bi)),
                        r32(exact(np.float32(ar * bi)) + exact(ai) * exact(br)),
                    ),
                }
                for k, (re, im) in forms.items():
                    for lab, arr in (("inplace", z), ("new", new)):
                        key = (n, lab, k)
                        counts[key] = counts.get(key, 0) + int(
                            arr.real[i] == re and arr.imag[i] == im
                        )
    for n in (1, 2, 3, 4, 5, 8):
        row = {f"{lab}/{k}": v for (m, lab, k), v in counts.items() if m == n}
        print(n, n * 300, row)


if __name__ == "__main__":
    main()

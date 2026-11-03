# SPDX-License-Identifier: Apache-2.0
"""Kiss FFT's twiddle factors (Armadillo 14.2.1, fslmaths' FFT convolution): for each transform
size, how many of ``cos(i k)`` and ``sin(i k)`` (``k = -+2 acos(-1) / N``) glibc 2.35's double
functions give other than the correctly rounded value (mpmath at 80 digits). The replica uses
correctly rounded ``cos`` and ``sin`` (glibc's are IBM's code, LGPL: GLIBC-07), so a size with
such a twiddle can move an FFT convolution's float output by a unit in the last place.

    python -P validation/investigations/fsl-maths/fft_twiddles.py 60 38 [N ...]

Found by random testing (2026-11-03, ``fsl-maths:fsl-maths-trial:61795``: ``-kernel gauss 2.25
-fmeanu`` of a 30 x 1 x 3 image, transforms of 60 and 38 points; 1 of 90 voxels one unit in the
last place from FSL's).
"""

from __future__ import annotations

import ctypes
import sys

import mpmath

_libm = ctypes.CDLL("libm.so.6")
for f in ("cos", "sin", "acos"):
    getattr(_libm, f).restype = ctypes.c_double
    getattr(_libm, f).argtypes = (ctypes.c_double,)
mpmath.mp.dps = 80


def main(sizes: list[int]) -> None:
    for n in sizes:
        for sign in (-2.0, 2.0):
            k = sign * _libm.acos(-1.0) / n
            differ = []
            for i in range(n):
                y = i * k
                if abs(y) <= 2.2250738585072014e-308:
                    continue
                for name in ("cos", "sin"):
                    cr = float(getattr(mpmath, name)(mpmath.mpf(y)))
                    if getattr(_libm, name)(y) != cr:
                        differ.append(f"{name}({i} k)")
            kind = "forward" if sign < 0 else "inverse"
            print(f"N={n} {kind}: {len(differ)} of {2 * n} differ {differ[:6]}")


if __name__ == "__main__":
    main([int(a) for a in sys.argv[1:]])

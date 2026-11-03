# SPDX-License-Identifier: Apache-2.0
"""Kiss FFT's twiddle factors (Armadillo 14.2.1, fslmaths' FFT convolution): for each transform
size, how many of ``cos(i k)`` and ``sin(i k)`` (``k = -+2 acos(-1) / N``) glibc 2.35 gives other
than the correctly rounded value (mpmath at 80 digits), through ``sincos`` (what FSL's library
takes: it computes each factor with ``cexp``, which calls ``sincos``) and through the separate
``cos`` and ``sin`` (on a CPU with FMA another build, which can differ from ``sincos``). The
replica uses correctly rounded ``cos`` and ``sin`` (glibc's are IBM's code, LGPL: GLIBC-07), so
a size with such a factor can move an FFT convolution's float output by a unit in the last place.

    python -P validation/investigations/fsl-maths/fft_twiddles.py 60 38 [N ...]

Found by random testing (2026-11-03, ``fsl-maths:fsl-maths-trial:61795``: ``-kernel gauss 2.25
-fmeanu`` of a 30 x 1 x 3 image, transforms of 60 and 26 points; 1 of 90 voxels one unit in the
last place from FSL's). The first version of this script compared only the separate ``cos`` and
``sin`` (the FMA build on the oracle machine), which agree with the correctly rounded value on
those sizes, and so missed the cause; ``validation/probes/fsl-avwutils-lib/check_fft.py`` compares
Armadillo's own factors.
"""

from __future__ import annotations

import ctypes
import sys

import mpmath

_libm = ctypes.CDLL("libm.so.6")
for f in ("cos", "sin", "acos"):
    getattr(_libm, f).restype = ctypes.c_double
    getattr(_libm, f).argtypes = (ctypes.c_double,)
_libm.sincos.restype = None
_libm.sincos.argtypes = (
    ctypes.c_double,
    ctypes.POINTER(ctypes.c_double),
    ctypes.POINTER(ctypes.c_double),
)
mpmath.mp.dps = 80


def _sincos(y: float) -> tuple[float, float]:
    s, c = ctypes.c_double(), ctypes.c_double()
    _libm.sincos(y, ctypes.byref(s), ctypes.byref(c))
    return s.value, c.value


def main(sizes: list[int]) -> None:
    for n in sizes:
        for sign in (-2.0, 2.0):
            k = sign * _libm.acos(-1.0) / n
            via_sincos, separate = [], []
            for i in range(n):
                y = i * k
                if abs(y) <= 2.2250738585072014e-308:
                    continue
                s, c = _sincos(y)
                for name, v in (("cos", c), ("sin", s)):
                    cr = float(getattr(mpmath, name)(mpmath.mpf(y)))
                    if v != cr:
                        via_sincos.append(f"{name}({i} k)")
                    if getattr(_libm, name)(y) != cr:
                        separate.append(f"{name}({i} k)")
            kind = "forward" if sign < 0 else "inverse"
            print(
                f"N={n} {kind}: sincos {len(via_sincos)} of {2 * n} differ {via_sincos[:6]}; "
                f"separate cos, sin {len(separate)} {separate[:6]}"
            )


if __name__ == "__main__":
    main([int(a) for a in sys.argv[1:]])

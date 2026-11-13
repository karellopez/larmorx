# SPDX-License-Identifier: Apache-2.0
"""The correctly rounded float arc cosine of the floats scan.c lists (those whose correctly
rounded double result is exactly halfway between two floats), at 200 bits (mpmath): prints each
input, the float the double rounds to (ties to even) and the correctly rounded float.

    python -I decide.py 328885a3 39826222
"""

import struct
import sys

import mpmath
import numpy as np

mpmath.mp.prec = 200
for h in sys.argv[1:]:
    x = struct.unpack(">f", bytes.fromhex(h))[0]
    a = mpmath.acos(mpmath.mpf(x))
    d = float(a)
    rounded = np.float32(d)
    cands = [rounded, np.nextafter(rounded, np.float32(4)), np.nextafter(rounded, np.float32(0))]
    best = min(cands, key=lambda c: abs(mpmath.mpf(float(c)) - a))
    print(
        h,
        f"double->float {int(rounded.view(np.uint32)):08x}",
        f"correct {int(best.view(np.uint32)):08x}",
    )

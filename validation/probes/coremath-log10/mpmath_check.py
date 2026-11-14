# SPDX-License-Identifier: Apache-2.0
"""CORE-MATH's C ``cr_log10`` (the shared library ``build.sh`` makes) against mpmath's log10 at
400 bits rounded to nearest, on every hard-to-round case of ``log10.wc`` and on seeded arguments
(random bit patterns of positive doubles, numbers next to 1, p-values). The Rust port equals the
C bit for bit (``coremath-log10``), so this checks the correct rounding of both independently of
CORE-MATH's own proof.

    python -I mpmath_check.py <libcrlog10.so> <log10.wc> [n]

Prints the counts and ``MPMATH-PASS`` or ``MPMATH-FAIL`` (exit status 1).
"""

from __future__ import annotations

import ctypes
import random
import struct
import sys

import mpmath


def bits(v: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", v))[0]


def double(u: int) -> float:
    return struct.unpack("<d", struct.pack("<Q", u))[0]


def main(argv: list[str]) -> int:
    lib = ctypes.CDLL(argv[0])
    cr = lib.cr_log10
    cr.restype = ctypes.c_double
    cr.argtypes = [ctypes.c_double]
    n = int(argv[2]) if len(argv) > 2 else 100_000
    mpmath.mp.prec = 400
    xs: list[tuple[str, float]] = []
    with open(argv[1], encoding="utf-8") as f:
        for line in f:
            t = line.strip()
            if t.startswith(("0x", "-0x")):
                x = float.fromhex(t)
                if x > 0:
                    xs.append(("log10.wc", x))
    rng = random.Random(20261114)
    for _ in range(n):
        # a positive finite double from random bits
        u = rng.getrandbits(63)
        if (u >> 52) == 0x7FF or u == 0:
            continue
        xs.append(("random bits", double(u)))
        xs.append(("near 1", 1.0 + (rng.random() * 2 - 1) * 2.0 ** -rng.randrange(1, 53)))
        xs.append(("p-values", max(rng.random(), 2.0**-53)))
    counts: dict[str, list[int]] = {}
    for fam, x in xs:
        want = float(mpmath.log10(mpmath.mpf(x)))
        got = cr(x)
        c = counts.setdefault(fam, [0, 0])
        c[0] += 1
        if bits(want) != bits(got):
            c[1] += 1
            if c[1] <= 4:
                print(f"  DIFF {fam} {x.hex()}: C {got.hex()}, mpmath {want.hex()}")
    bad = 0
    for fam, (k, d) in counts.items():
        print(f"{fam}: {k} arguments, the C differs from mpmath on {d}")
        bad += d
    print("MPMATH-PASS" if bad == 0 else "MPMATH-FAIL")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

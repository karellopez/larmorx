# SPDX-License-Identifier: Apache-2.0
"""Dimension numbers that pass MRIreorder's check only through 32-bit overflow (FS-843).

The check: the product and the sum of the three numbers' ``abs`` values, in 32-bit signed
arithmetic (wrapping; ``abs`` of -2**31 is itself), must both be 6. For an odd ``a`` the other two
absolute values are the roots of ``x**2 - (6 - a) x + 6 / a`` modulo 2**32, found here bit by bit;
the triples printed have every value below 2**31 and are not 1, 2 and 3.

    python3 -P validation/probes/fs-io/reorder_triples.py [count]
"""

import sys

M = 1 << 32


def i32(x: int) -> int:
    x %= M
    return x - M if x >= 1 << 31 else x


def cabs(x: int) -> int:
    return x if x >= 0 else i32(-x)


def passes(a: int, b: int, c: int) -> bool:
    return i32(cabs(a) * cabs(b) * cabs(c)) == 6 and i32(cabs(a) + cabs(b) + cabs(c)) == 6


def triples(count: int) -> list[tuple[int, int, int]]:
    out = []
    for a in range(5, 400001, 2):
        s, p = (6 - a) % M, (6 * pow(a, -1, M)) % M
        roots = [r for r in (0, 1) if (r * r - s * r + p) % 2 == 0]
        for k in range(2, 33):
            m = 1 << k
            roots = sorted(
                {t for r in roots for t in (r, r + m // 2) if (t * t - s * t + p) % m == 0}
            )[:64]
        for b in roots:
            c = (s - b) % M
            if b < 1 << 31 and c < 1 << 31 and b not in (1, 2, 3) and passes(a, b, c):
                out.append((a, b, c))
        if len(out) >= count:
            return out[:count]
    return out


if __name__ == "__main__":
    for t in triples(int(sys.argv[1]) if len(sys.argv) > 1 else 4):
        print(*t)

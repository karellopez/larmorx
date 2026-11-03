# SPDX-License-Identifier: Apache-2.0
"""Which of Armadillo's twiddle factors that are not correctly rounded a Kiss FFT butterfly reads.

Reads ``fft_twiddles.cc``'s output (``check_fft.py`` writes it as ``arma_twiddles.txt``), finds
the factors whose ``(cos, sin)`` is not the correctly rounded pair (mpmath at 60 digits), and
lists, per transform length, whether any butterfly of the engine reads one: the engine's factor
order (4, then 2, 3, 5, 7, ...) and the indices each butterfly (radix 2, 3, 4, 5, generic) reads,
as fft_engine_kissfft.hpp defines them. A length whose differing factors no butterfly reads
transforms exactly as the replica does.

    python -P validation/probes/fsl-avwutils-lib/fft_reads.py <work>/arma_twiddles.txt
"""

from __future__ import annotations

import struct
import sys
from collections import defaultdict
from pathlib import Path

import mpmath

DBL_MIN = 2.2250738585072014e-308


def stages(n: int) -> list[tuple[int, int]]:
    """``(radix, residue)`` per stage."""
    out, r = [], 4
    while n >= 2:
        while n % r:
            r = 3 if r == 2 else 2 if r == 4 else r + 2
            if r * r > n:
                r = n
        n //= r
        out.append((r, n))
    return out


def read_indices(n: int) -> set[int]:
    """The factor indices the engine's butterflies read for a length ``n``."""
    idx: set[int] = set()
    st = stages(n)

    def run(stage: int, stride: int) -> None:
        r, m = st[stage]
        if m != 1:
            for _ in range(r):
                run(stage + 1, stride * r)
        if r == 2:
            idx.update(i * stride for i in range(m))
        elif r == 3:
            idx.add(stride * m)
            idx.update(j * stride * k for j in range(m) for k in (1, 2))
        elif r == 4:
            idx.update(i * stride * k for i in range(m) for k in (1, 2, 3))
        elif r == 5:
            idx.update({stride * m, stride * 2 * m})
            idx.update(i * stride * k for i in range(m) for k in (1, 2, 3, 4))
        else:
            for u in range(m):
                k = u
                for _ in range(r):
                    j = 0
                    for _ in range(1, r):
                        j += stride * k
                        if j >= n:
                            j -= n
                        idx.add(j)
                    k += m

    if n >= 2:
        run(0, 1)
    return idx


def main(path: str) -> None:
    mpmath.mp.dps = 60

    def cr(f, y: float) -> str:
        return struct.pack(">d", float(f(mpmath.mpf(y)))).hex()

    differ: dict[int, set[int]] = defaultdict(set)
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        w = line.split()
        n, i = int(w[1]), int(w[3])
        y = struct.unpack(">d", bytes.fromhex(w[4]))[0]
        if abs(y) <= DBL_MIN:
            continue  # cexp's (1, y): exact
        if (w[5], w[6]) != (cr(mpmath.cos, y), cr(mpmath.sin, y)):
            differ[n].add(i)
    read = sorted(n for n in differ if differ[n] & read_indices(n))
    print(
        f"{len(differ)} lengths hold a factor that is not correctly rounded; a butterfly reads "
        f"one in {len(read)}"
    )
    print(f"  read: {' '.join(map(str, read))}")
    print(f"  never read: {' '.join(str(n) for n in sorted(set(differ) - set(read)))}")


if __name__ == "__main__":
    main(sys.argv[1])

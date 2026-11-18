# SPDX-License-Identifier: Apache-2.0
"""Where glibc's acos differs: the --list files of the black-box tool, summarised.

    python3 -P analyse.py <fma-list.txt> <sse2-list.txt> <core-math>/src/binary64/acos/acos.wc

Prints, for family 4, the differing arguments per centre (below and above it, negative ones) for
each build; families 4 to 6, where glibc's two builds (with FMA, without: GLIBC_TUNABLES) differ
from each other; whether glibc lies above or below the correctly rounded value; family 5's
differing arguments per binade, and how many are CORE-MATH's listed arguments themselves.
"""

from __future__ import annotations

import collections
import contextlib
import struct
import sys

CENTRES = [
    0, 0x3C88 << 48, 0x3E50 << 48, 0x3FC0 << 48, 0x3FD0 << 48, 0x3FE0 << 48, 0x3FE8 << 48,
    0x3FED8 << 44, 0x3FEE8 << 44, 0x3FEF0 << 44, 0x3FF0 << 48,
]  # fmt: skip
ABS = 0x7FFF_FFFF_FFFF_FFFF


def as_float(bits: int) -> float:
    return struct.unpack("<d", bits.to_bytes(8, "little"))[0]


def load(path: str) -> dict[tuple[int, int], tuple[int, int, int]]:
    out = {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("#"):
                continue
            f, i, x, g, c, _ = line.split()
            out[(int(f), int(i))] = (int(x, 16), int(g, 16), int(c, 16))
    return out


def main() -> None:
    fma, sse2, wc_path = sys.argv[1:4]
    builds = {"fma": load(fma), "sse2": load(sse2)}
    for name, d in builds.items():
        below, above, neg = collections.Counter(), collections.Counter(), collections.Counter()
        for (f, i), (x, _, _) in d.items():
            if f != 4:
                continue
            k = i >> 26
            (below if x & ABS < CENTRES[k] else above)[k] += 1
            neg[k] += x >> 63
        for k, c in enumerate(CENTRES):
            print(
                f"{name} centre {c:016x} ({as_float(c):.6g}): below {below[k]}, above {above[k]}"
                f" (negative arguments {neg[k]}) of {1 << 26}"
            )
        up = sum(1 for _, g, c in d.values() if g > c)
        print(f"{name}: glibc above the correctly rounded value {up}, below {len(d) - up}")
    a, b = builds["fma"], builds["sse2"]
    for fam in (4, 5, 6):
        ka = {k for k in a if k[0] == fam}
        kb = {k for k in b if k[0] == fam}
        both = ka & kb
        same = sum(1 for k in both if a[k][1] == b[k][1])
        print(
            f"family {fam}: only the FMA build differs {len(ka - kb)}, only the SSE2 build"
            f" {len(kb - ka)}, both {len(both)} (equal to each other on {same})"
        )
    listed = set()
    with open(wc_path, encoding="utf-8") as fh:
        for line in fh:
            t = line.strip()
            if t and not t.startswith("#"):
                with contextlib.suppress(ValueError):
                    listed.add(abs(float.fromhex(t.lstrip("+"))))
    per, exact, n = collections.Counter(), 0, 0
    for (f, _), (x, _, _) in a.items():
        if f != 5:
            continue
        per[((x & ABS) >> 52) - 1023] += 1
        exact += abs(as_float(x)) in listed
        n += 1
    print("family 5 (FMA build) differing per binade of |x|:", dict(sorted(per.items())))
    print(f"family 5: {exact} of {n} are listed arguments themselves (either sign)")


if __name__ == "__main__":
    main()

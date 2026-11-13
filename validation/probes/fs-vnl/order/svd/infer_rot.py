#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Each `srot` operation's first operand, per element, from `kern_rot`'s output.

Model of element i: `x' = A(M1(c, x), M2(s, y))`, `y' = M3(c, y) - M4(s, x)`; each of M1..M4 and
A passes its first operand's NaN when both are NaN. The pairs {c,x}, {s,y}, {c,s}, {c,y}, {s,x}
give the five choices (letters: `N` the source's left operand first, `S` the right one); every
other subset is then predicted and checked. Prints one line per distinct pattern with the
configurations (length, alignment, distance, direction) that show it, and `MODEL-OK` or
`MODEL-FAIL`.
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

NAMES = "csxy"


def subset(k):
    return {NAMES[b] for b in range(4) if k >> b & 1}


def predict(model, s):
    m1, m2, a, m3, m4 = model

    def op(order, left, right):  # the NaN passed on: letter or None
        lv, rv = left, right
        if lv and rv:
            return lv if order == "N" else rv
        return lv or rv

    nan = {v: v.upper() if v in s else None for v in NAMES}
    p1 = op(m1, nan["c"], nan["x"])
    p2 = op(m2, nan["s"], nan["y"])
    xo = op(a, p1, p2)
    p3 = op(m3, nan["c"], nan["y"])
    p4 = op(m4, nan["s"], nan["x"])
    yo = op("N", p3, p4)
    return (xo or "."), (yo or ".")


def main():
    pats = defaultdict(list)
    ok = True
    for line in Path(sys.argv[1]).read_text().splitlines():
        head, _, rest = line.partition(":")
        n = int(re.search(r"n=(\d+)", head).group(1))
        res = rest.split()
        assert len(res) == 15
        outs = {k + 1: r.split("/") for k, r in enumerate(res)}
        model_per_i = []
        for i in range(n):

            def g(k, w, outs=outs, i=i):
                return outs[k][w][i]

            m1 = "N" if g(5, 0) == "C" else "S"
            m2 = "N" if g(10, 0) == "S" else "S"
            a = "N" if g(3, 0) == "C" else "S"
            m3 = "N" if g(9, 1) == "C" else "S"
            m4 = "N" if g(6, 1) == "S" else "S"
            model = (m1, m2, a, m3, m4)
            for k in range(1, 16):
                if predict(model, subset(k)) != (g(k, 0), g(k, 1)):
                    ok = False
                    print(
                        "MISMATCH",
                        head,
                        "element",
                        i,
                        "subset",
                        sorted(subset(k)),
                        outs[k][0][i],
                        outs[k][1][i],
                    )
            model_per_i.append("".join(model))
        pats[" ".join(model_per_i)].append(head.strip())
    for p, heads in sorted(pats.items(), key=lambda kv: len(kv[0])):
        print(f"{len(heads):5d} configs: {p}   e.g. {heads[0]}")
    print("MODEL-OK" if ok else "MODEL-FAIL")


main()

# SPDX-License-Identifier: Apache-2.0
"""Reads kern.c's output; prints, per length, each dot-product operation's first operand.

`mul`: `x` or `y` first in each product; `add`: `N` (the running sum first) or `S` (the product
first) for the sums of elements 2 to n (the first sum adds to 0, where the order is not seen).
"""

import re
import sys


def main(path: str) -> None:
    section = None
    with open(path, encoding="utf-8") as f:
        lines = f.readlines()
    for line in lines:
        if line.startswith("=="):
            section = line.strip()
            if "dot" in section:
                print(section)
            continue
        if not section or "dot" not in section:
            continue
        m = re.match(r"n=\s*(\d+):(.*)", line)
        n = int(m.group(1))
        tags = [int(t) for t in m.group(2).split()]
        # products in the order their first operand comes out; x tags < 500, y tags 500 + i
        products, mul = [], {}
        for tag in tags:
            e = tag if tag < 500 else tag - 500
            if e not in mul:
                mul[e] = "x" if tag < 500 else "y"
                products.append(e)
        # a left-to-right chain: peel the last sum; its product is first (S) or last (N)
        rest, add = list(products), {}
        for e in range(n, 1, -1):
            add[e] = "S" if rest[0] == e else "N" if rest[-1] == e else "?"
            rest.remove(e)
        muls = "".join(mul[e] for e in range(1, n + 1))
        adds = "".join(add[e] for e in range(2, n + 1))
        print(f"n={n:3d} mul {muls} add -{adds}")


if __name__ == "__main__":
    main(sys.argv[1])

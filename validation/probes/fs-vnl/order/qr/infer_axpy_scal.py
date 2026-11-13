# SPDX-License-Identifier: Apache-2.0
"""Reads kern.c's output; prints, per length, `axpy`'s and `scal`'s operand orders.

`axpy`: `mul` `a` or `x` first in each product, `add` `p` (the product first) or `y`, and whether
the all-NaN case agrees; `scal`: which NaN (`A` the scalar, `X` the element) each element keeps.
"""

import re
import sys


def main(path: str) -> None:
    section = None
    with open(path, encoding="utf-8") as f:
        lines = f.readlines()
    for line in lines:
        if line.startswith("=="):
            section = line.split("(")[0].strip()
            print(section)
            continue
        m = re.match(r"n=\s*(\d+):(.*)", line)
        if not m or section is None:
            continue
        n, body = int(m.group(1)), m.group(2)
        if "axpy" in section:
            d = dict(re.findall(r"\[(...)\](\S+)", body))
            mul = "".join("a" if c == "A" else "x" for c in d["ax."])
            add = "".join("p" if c == "X" else "y" for c in d[".xy"])
            first = ["A" if mul[i] == "a" else "X" for i in range(n)]
            ok = all(d["axy"][i] == (first[i] if add[i] == "p" else "Y") for i in range(n))
            print(f"n={n:3d} mul {mul} add {add} {'ok' if ok else 'INCONSISTENT'}")
        elif "scal" in section:
            print(f"n={n:3d} {body.strip()}")


if __name__ == "__main__":
    main(sys.argv[1])

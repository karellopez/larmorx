# SPDX-License-Identifier: Apache-2.0
"""CORE-MATH's ``src/binary64/log10/log10.c`` fast-path tables transcribed mechanically into Rust
bit patterns (``larmorx_core::math::log10``): every hex-float literal of the C initialisers
``_INVERSE`` and ``_LOG_INV`` written as ``0x...`` with the literal in a comment, as
``math/log.rs`` holds log.c's. The accurate path's tables of ``dint.h`` (``_INVERSE_2``,
``_LOG_INV_2``, ``P_2``) and the polynomial ``P`` are the same as log.c's (``--check-shared``
compares them with log.c and log/dint.h and prints ``SHARED-SAME`` or the first difference).

    python -P coremath_tables.py <core-math>/src/binary64/log10/log10.c > tables.rs.txt
    python -P coremath_tables.py --check-shared <core-math>/src/binary64
"""

from __future__ import annotations

import re
import struct
import sys
from pathlib import Path

HEX = re.compile(r"-?0x[0-9a-fA-F]+(?:\.[0-9a-fA-F]*)?p[+-]?\d+")


def bits(lit: str) -> str:
    v = float.fromhex(lit)
    return f"0x{struct.unpack('<Q', struct.pack('<d', v))[0]:016x}"


def block(src: str, head: str) -> str:
    start = src.index(head)
    end = src.index("};", start)
    return src[src.index("{", start) + 1 : end]


def strip_comments(s: str) -> str:
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return re.sub(r"//[^\n]*", "", s)


def tables(src: str) -> tuple[list[str], list[list[str]]]:
    inv = HEX.findall(strip_comments(block(src, "static const double _INVERSE[363]")))
    pairs = HEX.findall(strip_comments(block(src, "static const double _LOG_INV[363][2]")))
    assert len(inv) == 363, len(inv)
    assert len(pairs) == 726, len(pairs)
    return inv, [pairs[k : k + 2] for k in range(0, 726, 2)]


def dint_block(src: str, name: str) -> str:
    m = re.search(r"static const dint64_t " + re.escape(name) + r"\[\] = \{(.*?)\n\};", src, re.S)
    if m is None:
        raise SystemExit(f"{name} not found")
    return re.sub(r"\s+", "", strip_comments(m.group(1)))


def check_shared(root: Path) -> int:
    log_c = (root / "log/log.c").read_text(encoding="utf-8")
    log10_c = (root / "log10/log10.c").read_text(encoding="utf-8")
    p_log = HEX.findall(strip_comments(block(log_c, "static const double P[6]")))
    p_log10 = HEX.findall(strip_comments(block(log10_c, "static const double P[6]")))
    if p_log != p_log10:
        print("P differs")
        return 1
    d_log = (root / "log/dint.h").read_text(encoding="utf-8")
    d_log10 = (root / "log10/dint.h").read_text(encoding="utf-8")
    for name in ("_INVERSE_2", "_LOG_INV_2", "P_2"):
        if dint_block(d_log, name) != dint_block(d_log10, name):
            print(f"{name} differs")
            return 1
    print("SHARED-SAME")
    return 0


def main(argv: list[str]) -> int:
    if argv and argv[0] == "--check-shared":
        return check_shared(Path(argv[1]))
    src = Path(argv[0]).read_text(encoding="utf-8")
    inv, log_inv = tables(src)
    print("static INVERSE: [f64; 363] = f64_table([")
    for lit in inv:
        print(f"    {bits(lit)}, // {lit}")
    print("]);")
    print("static LOG_INV: [[f64; 2]; 363] = f64_pair_table([")
    for a, b in log_inv:
        print(f"    [{bits(a)}, {bits(b)}], // {{{a}, {b}}}")
    print("]);")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

# SPDX-License-Identifier: Apache-2.0
"""CORE-MATH's ``src/binary64/tanh/tanh.c`` tables transcribed mechanically into Rust bit
patterns (``larmorx_core::math::tanh``): every hex-float literal of the C initialisers ``t0``,
``t1`` (and the small coefficient tables) written as ``f64::from_bits(0x...)``.

``python -P coremath_tables.py <core-math>/src/binary64/tanh/tanh.c`` prints the Rust tables.
"""

from __future__ import annotations

import re
import sys

HEX = re.compile(r"-?0x[0-9a-fA-F]+(?:\.[0-9a-fA-F]*)?p[+-]?\d+")


def bits(lit: str) -> str:
    import struct

    v = float.fromhex(lit)
    return f"0x{struct.unpack('<Q', struct.pack('<d', v))[0]:016x}"


def table(src: str, name: str) -> list[str]:
    m = re.search(r"static const double " + name + r"\[\]\[2\] = \{(.*?)\};", src, re.S)
    if m is None:
        raise SystemExit(f"{name} not found")
    return HEX.findall(m.group(1))


def main(argv: list[str]) -> int:
    from pathlib import Path

    src = Path(argv[0]).read_text(encoding="utf-8")
    for name in ("t0", "t1"):
        lits = table(src, name)
        assert len(lits) == 128, (name, len(lits))
        print(f"/// CORE-MATH's `{name}` (64 pairs `{{low, high}}`), as bits.")
        print(f"const {name.upper()}: [[u64; 2]; 64] = [")
        for k in range(0, 128, 2):
            print(f"    [{bits(lits[k])}, {bits(lits[k + 1])}],")
        print("];")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

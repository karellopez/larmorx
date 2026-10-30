# SPDX-License-Identifier: Apache-2.0
"""The double-precision constants of specs/cbrtf.md (the spec writer, 2026-10-30): each decimal
as the spec gives it, the double it rounds to (IEEE round to nearest), its bits and its exact
value, and the five scale factors (their quotients rounded once to double).

    python -P validation/investigations/specs-memory/cbrtf_constants.py
"""

from __future__ import annotations

import struct
from decimal import Decimal, getcontext

getcontext().prec = 60

DECIMALS = {
    "c0": "0.492659620528969547",
    "c1": "0.697570460207922770",
    "c2": "0.191502161678719066",
    "K1 (2^(1/3))": "1.2599210498948731648",
    "K2 (2^(2/3))": "1.5874010519681994748",
}


def bits(x: float) -> str:
    return f"0x{struct.unpack('<Q', struct.pack('<d', x))[0]:016x}"


def main() -> None:
    values = {}
    for name, text in DECIMALS.items():
        x = float(text)
        values[name] = x
        print(f"{name:14s} {text:24s} -> {bits(x)}  {x.hex():24s} exact {Decimal(x)}")
    k1, k2 = values["K1 (2^(1/3))"], values["K2 (2^(2/3))"]
    factors = [("1/K2", 1.0 / k2), ("1/K1", 1.0 / k1), ("1", 1.0), ("K1", k1), ("K2", k2)]
    for i, (name, x) in enumerate(factors):
        print(f"F[{i}] = {name:5s} {bits(x)}  {x.hex():24s} exact {Decimal(x)}")


if __name__ == "__main__":
    main()

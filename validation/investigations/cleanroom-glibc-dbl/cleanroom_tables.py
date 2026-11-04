# SPDX-License-Identifier: Apache-2.0
"""The numbers of `specs/glibc-dbl.md` from their definitions, for the clean-room implementation.

Written by the clean-room implementer from the spec's text alone (§3 and §5; no glibc file is
read, and the spec writer's `glibc-dbl/tables.py` was never opened). With `mpmath`:

- the 75 base-2^24 digits of the fraction of 2/π, ``D_j = floor((2/π) 2^(24 (j+1))) mod 2^24``
  (spec §3), checked against the three the spec quotes (``D_0``, ``D_1``, ``D_74``);
- table ``R`` (spec §5.1): for ``N = 0 … 109``, ``SH = fl(sin(N/128))``,
  ``SL = fl(sin(N/128) − SH)``, ``CH = fl(cos(N/128))``, ``CL = fl(cos(N/128) − CH)``, rounded
  to nearest, ties to even, from values computed with 400 bits (spec §5.2) and again with 1,000
  bits (the two must agree);
- the constants of spec §3 that have a definition (``H0``, ``H1``, ``G``, ``M1``, ``M2``, ``M3``,
  ``M4``, ``M2L``, ``T3``, ``V2``, ``τ``), recomputed and compared with the bits the spec gives
  (they are used as the spec's bits; this only checks them).

It writes the Rust constants to ``crates/larmorx-core/src/math/sincos_glibc_tables.rs``
(``--write``) or checks that file (default). Exact rounding: every high-precision value is
converted to an exact rational (``mpf`` is ``m 2^e``), and rounded with integer arithmetic.

Run (from the repository root): ``python -P validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py``
(prints ``TABLES-OK`` or the differences; ``--write`` regenerates the Rust file).
"""

from __future__ import annotations

import struct
import sys
from fractions import Fraction
from pathlib import Path

import mpmath

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "crates/larmorx-core/src/math/sincos_glibc_tables.rs"
COMMAND = "python -P validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py --write"


def exact(v: mpmath.mpf) -> Fraction:
    """The exact rational value of an mpmath number (``man 2^exp``)."""
    man, exp = mpmath.mpf(v).man_exp
    return Fraction(man) * Fraction(2) ** exp


def round_bits(v: Fraction, p: int) -> Fraction:
    """``v`` rounded to nearest with ``p`` significant bits, ties to even (no exponent range)."""
    if v == 0:
        return Fraction(0)
    sign = -1 if v < 0 else 1
    a = abs(v)
    e = a.numerator.bit_length() - a.denominator.bit_length()
    if Fraction(2) ** e > a:
        e -= 1
    while Fraction(2) ** (e + 1) <= a:
        e += 1
    # 2^e <= a < 2^(e+1); keep p bits: a * 2^(p-1-e) in [2^(p-1), 2^p)
    scale = Fraction(2) ** (p - 1 - e)
    t = a * scale
    q, r = divmod(t.numerator, t.denominator)
    twice = 2 * r
    if twice > t.denominator or (twice == t.denominator and q % 2 == 1):
        q += 1
    return sign * Fraction(q) / scale


def to_double(v: Fraction) -> float:
    """``fl(v)``: the nearest double, ties to even (normal range; ``float`` of a ``Fraction`` is
    correctly rounded in CPython, and must equal the 53-bit rounding here)."""
    r = round_bits(v, 53)
    f = float(r)
    assert Fraction(f) == r, (v, r, f)
    assert float(v) == f or v == 0
    return f


def bits(f: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", f))[0]


def frombits(b: int) -> float:
    return struct.unpack("<d", struct.pack("<Q", b))[0]


def digits(prec: int) -> list[int]:
    mpmath.mp.prec = prec
    two_over_pi = exact(2 / mpmath.pi)
    out = []
    for j in range(75):
        v = two_over_pi * (1 << (24 * (j + 1)))
        out.append((v.numerator // v.denominator) % (1 << 24))
    return out


def table(prec: int) -> list[tuple[int, int, int, int]]:
    mpmath.mp.prec = prec
    rows = []
    for n in range(110):
        a = mpmath.mpf(n) / 128
        s = exact(mpmath.sin(a))
        c = exact(mpmath.cos(a))
        sh = to_double(s)
        sl = to_double(s - Fraction(sh)) if s != Fraction(sh) else 0.0
        ch = to_double(c)
        cl = to_double(c - Fraction(ch)) if c != Fraction(ch) else 0.0
        rows.append((bits(sh), bits(sl), bits(ch), bits(cl)))
    return rows


def constants() -> list[tuple[str, int, int]]:
    """(name, from the definition, the spec's bits)."""
    mpmath.mp.prec = 1000
    half_pi = exact(mpmath.pi / 2)
    h0 = to_double(half_pi)
    h1 = to_double(half_pi - Fraction(h0))
    g = to_double(exact(2 / mpmath.pi))
    m1 = round_bits(half_pi, 26)
    m2 = round_bits(half_pi - m1, 27)
    m3 = round_bits(half_pi - m1 - m2, 26)
    m4 = to_double(half_pi - m1 - m2 - m3)
    m2l = round_bits(half_pi - m1, 23)
    tau = to_double(Fraction(126, 1000))
    return [
        ("H0", bits(h0), 0x3FF921FB54442D18),
        ("H1", bits(h1), 0x3C91A62633145C07),
        ("G", bits(g), 0x3FE45F306DC9C883),
        ("M1", bits(float(m1)), 0x3FF921FB58000000),
        ("M2", bits(float(m2)), 0xBE4DDE973C000000),
        ("M3", bits(float(m3)), 0xBC8CB3B398000000),
        ("M4", bits(m4), 0xBACD747F23E32ED7),
        ("M2L", bits(float(m2l)), 0xBE4DDE9740000000),
        ("T3", bits(to_double(Fraction(-1, 6))), 0xBFC5555555555555),
        ("V2", bits(0.5), 0x3FE0000000000000),
        ("tau", bits(tau), 0x3FC020C49BA5E354),
    ]


def render(d: list[int], r: list[tuple[int, int, int, int]]) -> str:
    lines = [
        "// SPDX-License-Identifier: Apache-2.0",
        "//! The numbers of `specs/glibc-dbl.md` that are defined mathematically, written by the",
        "//! clean-room generator from their definitions alone (no number taken from glibc):",
        f"//! `{COMMAND}`",
        "//! (mpmath; the generator checks them with 400 and 1,000 bits of precision). Do not edit:",
        "//! regenerate.",
        "",
        "/// `D_j = floor((2/π) 2^(24 (j+1))) mod 2^24`, `j = 0 … 74`: the base-2^24 digits of the",
        "/// fraction of 2/π (spec §3), each held exactly by a double.",
        "#[rustfmt::skip]",
        "pub(super) const D: [f64; 75] = [",
    ]
    for i in range(0, 75, 8):
        lines.append("    " + " ".join(f"{v}.0," for v in d[i : i + 8]))
    lines += [
        "];",
        "",
        "/// Table `R` (spec §5.1), as bits: row `N` (`N = 0 … 109`) is `SH`, `SL`, `CH`, `CL`, with",
        "/// `SH + SL` and `CH + CL` the correctly rounded double-doubles of `sin(N/128)` and",
        "/// `cos(N/128)` (`SH = fl(sin(N/128))`, `SL = fl(sin(N/128) − SH)`, the same for the cosine).",
        "#[rustfmt::skip]",
        "pub(super) const R: [[u64; 4]; 110] = [",
    ]
    for n, row in enumerate(r):
        lines.append(f"    [{', '.join(f'0x{v:016x}' for v in row)}], // {n}")
    lines += ["];", ""]
    return "\n".join(lines)


def main() -> int:
    ok = True
    d = digits(2400)
    if d != digits(3000):
        print("digits: precision-dependent")
        ok = False
    for j, want in ((0, 10680707), (1, 7228996), (74, 343717)):
        if d[j] != want:
            print(f"D_{j} = {d[j]}, the spec says {want}")
            ok = False
    r = table(400)
    if r != table(1000):
        print("table R: 400 and 1,000 bits disagree")
        ok = False
    if r[0] != (0, 0, bits(1.0), 0):
        print("table R: row 0 is not (0, 0, 1, 0)")
        ok = False
    for name, got, spec in constants():
        if got != spec:
            print(f"{name}: definition {got:016x}, spec {spec:016x}")
            ok = False
    text = render(d, r)
    if "--write" in sys.argv[1:]:
        OUT.write_text(text, encoding="utf-8")
        print(f"written {OUT.relative_to(ROOT)}")
    elif not OUT.is_file() or OUT.read_text(encoding="utf-8") != text:
        print(f"{OUT.relative_to(ROOT)} differs from the generator's output")
        ok = False
    print("TABLES-OK" if ok else "TABLES-FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

# SPDX-License-Identifier: Apache-2.0
"""The numbers of specs/glibc-dbl.md, regenerated from their mathematical definitions and compared,
bit for bit, with glibc 2.35's (the source text of its tables and, where the C library's debug
symbols are installed, the bytes of the system's libm.so.6).

Every definition here is mathematics (a correctly rounded value of a real number, the digits of a
constant); no number is copied from glibc. glibc's files are only read at run time, to compare.

    python -P tables.py [--glibc <workspace>/tags/glibc-2.35] [--libm /lib/x86_64-linux-gnu/libm.so.6]
                        [--debug <libm's .debug file>] [--print]

Prints one line per table or constant and a summary line `TABLES: ...`; exits 1 only when a
constant or 2/pi's digits disagree with their definitions. The survey of tan's, asin's, acos's
and atan's tables reports what their natural definitions reproduce (specs/glibc-dbl.md, "The
tables"): counts only, no number of glibc's.
"""

from __future__ import annotations

import argparse
import math
import re
import struct
import subprocess
import sys
from pathlib import Path

import mpmath
from mpmath import mp, mpf

mp.prec = 400


# ---------------------------------------------------------------------------------------------
# Rounding helpers (exact, on mpmath numbers)


def rn(v: mpf, bits: int = 53) -> float:
    """`v` rounded to the nearest number with `bits` significant bits, ties to even (normal range).

    Returned as a Python float, which holds it exactly for `bits` <= 53."""
    if v == 0:
        return 0.0
    m, e = mpmath.frexp(v)  # v = m * 2^e, 0.5 <= |m| < 1
    scaled = m * mpf(2) ** bits
    n = int(mpmath.nint(scaled))  # mpmath's nint rounds half to even
    return float(mpf(n) * mpf(2) ** (e - bits))


def bits_of(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def hexd(x: float) -> str:
    return f"{bits_of(x):016x}"


def rn_split(v: mpf) -> tuple[float, float]:
    """The correctly rounded hi part of `v` and the correctly rounded rest."""
    hi = rn(v)
    lo = rn(v - mpf(hi))
    return hi, lo


# ---------------------------------------------------------------------------------------------
# glibc's tables, read from its source text (big-endian blocks: high word first) and its binary


def words_of_block(text: str, start_pat: str) -> list[int]:
    """The 32-bit words of the first `{ ... }` initialiser after `start_pat` (hex numbers only)."""
    i = text.index(start_pat)
    j = text.index("{", text.index("=", i))
    depth, k = 0, j
    while True:
        c = text[k]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                break
        k += 1
    body = re.sub(r"/\*.*?\*/", " ", text[j : k + 1], flags=re.S)
    return [int(w, 16) for w in re.findall(r"0x([0-9a-fA-F]+)", body)]


def doubles_from_words_be(words: list[int]) -> list[float]:
    out = []
    for hi, lo in zip(words[0::2], words[1::2], strict=True):
        out.append(struct.unpack(">d", struct.pack(">II", hi, lo))[0])
    return out


class Binary:
    """The bytes of the system's libm.so.6 at a symbol's address (from the debug symbols)."""

    def __init__(self, libm: Path, debug: Path | None):
        self.data = libm.read_bytes()
        self.syms: dict[str, tuple[int, int]] = {}
        if debug is not None and debug.exists():
            out = subprocess.run(
                ["nm", "-S", str(debug)], capture_output=True, text=True, check=True
            ).stdout
            for line in out.splitlines():
                p = line.split()
                if len(p) == 4:
                    self.syms[p[3]] = (int(p[0], 16), int(p[1], 16))
        d = self.data
        phoff = struct.unpack_from("<Q", d, 0x20)[0]
        phentsize, phnum = struct.unpack_from("<HH", d, 0x36)
        self.loads = []
        for i in range(phnum):
            p_type, _f, off, vaddr, _pa, filesz = struct.unpack_from(
                "<IIQQQQ", d, phoff + i * phentsize
            )
            if p_type == 1:
                self.loads.append((vaddr, off, filesz))

    def doubles(self, name: str) -> list[float] | None:
        if name not in self.syms:
            return None
        addr, size = self.syms[name]
        for vaddr, off, filesz in self.loads:
            if vaddr <= addr < vaddr + filesz:
                b = self.data[off + addr - vaddr : off + addr - vaddr + size]
                return list(struct.unpack(f"<{size // 8}d", b))
        return None


# ---------------------------------------------------------------------------------------------
# The definitions


def sincos_table() -> list[float]:
    """110 entries, N = 0 ... 109: hi and lo of sin(N/128), then hi and lo of cos(N/128)."""
    out = []
    for n in range(110):
        t = mpf(n) / 128
        out += [*rn_split(mpmath.sin(t)), *rn_split(mpmath.cos(t))]
    return out


def two_over_pi_digits(count: int = 75) -> list[float]:
    """The first `count` base-2^24 digits of the fraction 2/pi (its integer part is 0)."""
    with mpmath.workprec(24 * count + 64):
        v = 2 / mpmath.pi
        digits = [
            int(mpmath.floor(v * mpf(2) ** (24 * i))) % (1 << 24) for i in range(1, count + 1)
        ]
    return [float(d) for d in digits]


def rn_bits(v: mpf, p: int) -> float:
    return rn(v, p)


def trunc_bits(v: mpf, p: int) -> float:
    m, e = mpmath.frexp(v)
    k = int(mpmath.floor(abs(m) * mpf(2) ** p))
    return float((k if m > 0 else -k) * mpf(2) ** (e - p))


def find_short(name: str, v: mpf, target: float) -> str:
    """The definition of a constant that is `v` cut to fewer bits: which rule and how many bits."""
    if bits_of(rn(v)) == bits_of(target):
        return f"{name} = RN(v)"
    for p in range(1, 53):
        if bits_of(rn_bits(v, p)) == bits_of(target):
            return f"{name} = v rounded to nearest with {p} significant bits"
        if bits_of(trunc_bits(v, p)) == bits_of(target):
            return f"{name} = v truncated to {p} significant bits"
    return ""


def parse_hexfloat(text: str, name: str) -> float:
    m = re.search(rf"\b{name}\s*=\s*(-?0x[0-9A-Fa-f.]+p[-+]?[0-9]+)", text)
    return float.fromhex(m.group(1))


def parse_words_be(text: str, name: str) -> float:
    i = text.index("#ifdef BIG_ENDI")
    m = re.search(rf"\b{name}\s*=\s*\{{\{{\s*0x([0-9A-Fa-f]+)\s*,\s*0x([0-9A-Fa-f]+)", text[i:])
    return struct.unpack(">d", struct.pack(">II", int(m.group(1), 16), int(m.group(2), 16)))[0]


def constants(dbl: Path) -> bool:
    """The constants of the sin/cos method whose value has a definition."""
    u = (dbl / "usncs.h").read_text()
    b = (dbl / "branred.h").read_text()
    pi2 = mpmath.pi / 2
    g = {
        k: parse_hexfloat(u, k)
        for k in ("hp0", "hp1", "mp1", "mp2", "pp3", "pp4", "hpinv", "toint", "big")
    }
    r = {
        k: parse_words_be(b, k)
        for k in ("t576", "tm600", "tm24", "big", "big1", "hp0", "hp1", "mp1", "mp2")
    }
    checks = [
        ("hp0", pi2, g["hp0"]),
        ("hp1", pi2 - mpf(g["hp0"]), g["hp1"]),
        ("hpinv", 2 / mpmath.pi, g["hpinv"]),
        ("mp1", pi2, g["mp1"]),
        ("mp2", pi2 - mpf(g["mp1"]), g["mp2"]),
        ("pp3", pi2 - mpf(g["mp1"]) - mpf(g["mp2"]), g["pp3"]),
        ("pp4", pi2 - mpf(g["mp1"]) - mpf(g["mp2"]) - mpf(g["pp3"]), g["pp4"]),
        ("reduction's mp1", pi2, r["mp1"]),
        ("reduction's mp2", pi2 - mpf(r["mp1"]), r["mp2"]),
        ("reduction's hp0", pi2, r["hp0"]),
        ("reduction's hp1", pi2 - mpf(r["hp0"]), r["hp1"]),
    ]
    exact = [
        ("toint", mpf(3) * mpf(2) ** 51, g["toint"]),
        ("big", mpf(3) * mpf(2) ** 44, g["big"]),
        ("reduction's big", mpf(3) * mpf(2) ** 51, r["big"]),
        ("reduction's big1", mpf(3) * mpf(2) ** 53, r["big1"]),
        ("t576", mpf(2) ** 576, r["t576"]),
        ("tm600", mpf(2) ** -600, r["tm600"]),
        ("tm24", mpf(2) ** -24, r["tm24"]),
    ]
    ok = True
    for name, v, target in checks:
        d = find_short(name, v, target)
        print(f"constant {name} = {target.hex()}: {d or 'NO DEFINITION FOUND'}")
        ok &= bool(d)
    for name, v, target in exact:
        same = bits_of(float(v)) == bits_of(target)
        print(f"constant {name} = {target.hex()}: {'exact' if same else 'DIFFERS'}")
        ok &= same
    return ok


def survey(dbl: Path) -> None:
    """tan's, asin's and acos's, and atan's tables against the natural definitions."""
    # tan: rows (x_i, F_i, G_i, FF_i), i = 16 ... 201
    d = doubles_from_words_be(words_of_block((dbl / "utan.tbl").read_text(), "#ifdef BIG_ENDI"))
    nf = nff = ng = 0
    run_min, off_max = 99.0, 0.0
    for j in range(len(d) // 4):
        x, f, gi, ff = d[4 * j : 4 * j + 4]
        t = mpmath.tan(mpf(x))
        hi, lo = rn_split(t)
        nf += bits_of(hi) == bits_of(f)
        nff += bits_of(lo) == bits_of(ff)
        ng += bits_of(rn(1 / t)) == bits_of(gi)
        r = abs(float((t - mpf(f)) / mpf(math.ulp(f))))
        run_min = min(run_min, -math.log2(r))
        off_max = max(off_max, abs((x - (j + 16) / 256) / math.ulp((j + 16) / 256)))
    n = len(d) // 4
    print(
        f"tan table: {n} rows; F = RN(tan x_i): {nf}, FF = RN(tan x_i - F): {nff}, G = RN(1/tan x_i): {ng}; "
        f"the points x_i: no definition (tan x_i within 2^-{run_min:.1f} of F_i's last place, "
        f"x_i up to {off_max:.3g} units of the last place from i/256)"
    )
    # asin, acos: per subinterval [x0, c1 ... cd, lo, hi, (two numbers for acos)]
    a = doubles_from_words_be(words_of_block((dbl / "asincos.tbl").read_text(), "#ifdef BIG_ENDI"))
    ranges = [
        (0, 11, range(0, 32), 6),
        (352, 11, range(0, 64), 6),
        (1056, 12, range(0, 64), 7),
        (992, 13, range(64, 108), 8),
        (884, 14, range(108, 116), 9),
        (768, 15, range(116, 120), 10),
    ]
    tot = c1 = c2 = cr_hi = 0
    worst_hi = 0.0
    for n0, st, js, deg in ranges:
        for j in js:
            k = n0 + st * j
            x0 = a[k]
            co = mpmath.taylor(mpmath.asin, mpf(x0), deg)
            tot += 1
            c1 += bits_of(rn(co[1])) == bits_of(a[k + 1])
            c2 += bits_of(rn(co[2])) == bits_of(a[k + 2])
            v = mpmath.asin(mpf(x0))
            hi, lo = a[k + deg + 2], a[k + deg + 1]
            cr_hi += bits_of(rn(v)) == bits_of(hi)
            worst_hi = max(worst_hi, abs(float((mpf(hi) + mpf(lo) - v) / v)))
    print(
        f"asin/acos table: {tot} subintervals; first-derivative coefficient = RN(asin'(x0)): {c1}, second: {c2}, "
        f"higher: 0; constant term hi = RN(asin x0): {cr_hi}; hi + lo approximates asin(x0) only to "
        f"2^{math.log2(worst_hi):.1f} (relative): fitted data, no definition"
    )
    # atan: rows (x0, c0 ... c5), 241 rows on (1/16, 1)
    c = doubles_from_words_be(
        words_of_block((dbl / "uatan.tbl").read_text().replace("0X", "0x"), "#ifdef BIG_ENDI")
    )
    ok0 = ok1 = ok2 = 0
    for i in range(len(c) // 7):
        e = c[7 * i : 7 * i + 7]
        co = mpmath.taylor(mpmath.atan, mpf(e[0]), 2)
        ok0 += bits_of(rn(co[0])) == bits_of(e[1])
        ok1 += bits_of(rn(co[1])) == bits_of(e[2])
        ok2 += bits_of(rn(co[2])) == bits_of(e[3])
    print(
        f"atan table: {len(c) // 7} rows; RN(atan x0): {ok0}, RN(atan'(x0)): {ok1}, RN(atan''(x0)/2): {ok2}, "
        f"higher: not Taylor; the points x0: no definition (searched points)"
    )


def search_sincos(src: list[float]) -> None:
    """The rules tried for the sin/cos table's low parts: the value cut to p bits (to nearest,
    toward zero, downward) or to d base-2^24 digits, then split into hi and the correctly rounded
    rest. Prints how many of the 440 numbers each rule misses (the best rules miss 17)."""
    exact = []
    for n in range(110):
        t = mpf(n) / 128
        exact += [mpmath.sin(t), mpmath.cos(t)]

    def cut(v: mpf, scale: mpf, mode: str) -> mpf:
        s = v / scale
        k = (
            mpmath.nint(s)
            if mode == "n"
            else (mpmath.floor(s) if mode == "f" or s > 0 else mpmath.ceil(s))
        )
        return k * scale

    def misses(rule) -> int:
        bad = 0
        for i, v in enumerate(exact):
            vp = rule(v) if v != 0 else v
            hi = rn(vp)
            lo = rn(vp - mpf(hi))
            n, which = divmod(i, 2)
            bad += bits_of(hi) != bits_of(src[4 * n + 2 * which]) or bits_of(lo) != bits_of(
                src[4 * n + 2 * which + 1]
            )
        return bad

    results = []
    for mode in "ntf":
        for p in range(100, 131):
            results.append(
                (
                    misses(
                        lambda v, p=p, mode=mode: cut(
                            v, mpf(2) ** (int(mpmath.frexp(v)[1]) - p), mode
                        )
                    ),
                    f"{p} bits, {mode}",
                )
            )
        for d in range(4, 10):
            results.append(
                (
                    misses(
                        lambda v, d=d, mode=mode: cut(
                            v,
                            mpf(2)
                            ** (24 * (int(mpmath.floor(mpmath.log(abs(v), 2) / 24)) + 1 - d)),
                            mode,
                        )
                    ),
                    f"{d} base-2^24 digits, {mode}",
                )
            )
    results.sort()
    print(
        "sin/cos table, rules tried (numbers missed of 440): "
        + ", ".join(f"{r[1]}: {r[0]}" for r in results[:6])
        + f"; {len(results)} rules, none misses fewer than {results[0][0]}"
    )


def compare(name: str, ours: list[float], theirs: list[float] | None, quiet: bool = False) -> bool:
    if theirs is None:
        print(f"{name}: not available (skipped)")
        return True
    if len(ours) != len(theirs):
        print(f"{name}: FAIL, {len(ours)} numbers against {len(theirs)}")
        return False
    bad = [i for i, (a, b) in enumerate(zip(ours, theirs, strict=True)) if bits_of(a) != bits_of(b)]
    if bad:
        print(
            f"{name}: {len(ours) - len(bad)} of {len(ours)} from the definition, {len(bad)} differ"
        )
        if not quiet:
            for i in bad:
                print(f"   [{i}] definition {hexd(ours[i])}  glibc {hexd(theirs[i])}")
        return False
    print(f"{name}: {len(ours)} numbers, 0 differ")
    return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--glibc", type=Path, default=None, help="glibc 2.35's source tree")
    ap.add_argument("--libm", type=Path, default=Path("/lib/x86_64-linux-gnu/libm.so.6"))
    ap.add_argument("--debug", type=Path, default=None)
    ap.add_argument("--print", action="store_true", help="list the table entries that differ")
    ap.add_argument(
        "--search", action="store_true", help="the other rules tried for the sin/cos table"
    )
    a = ap.parse_args()
    glibc = a.glibc
    if glibc is None:  # the workspace's tags/glibc-2.35, above this checkout
        glibc = next(
            (
                q / "tags/glibc-2.35"
                for q in Path(__file__).resolve().parents
                if (q / "tags/glibc-2.35").is_dir()
            ),
            None,
        )
        if glibc is None:
            sys.exit("glibc 2.35's source not found: pass --glibc")
    dbl = glibc / "sysdeps/ieee754/dbl-64"
    debug = a.debug
    if debug is None:
        note = subprocess.run(["readelf", "-n", str(a.libm)], capture_output=True, text=True).stdout
        m = re.search(r"Build ID: ([0-9a-f]+)", note)
        if m:
            bid = m.group(1)
            debug = Path(f"/usr/lib/debug/.build-id/{bid[:2]}/{bid[2:]}.debug")
    binary = Binary(a.libm, debug)
    ok = True

    # sin/cos: the table of sin and cos of N/128
    gen = sincos_table()
    src = doubles_from_words_be(
        words_of_block((dbl / "sincostab.c").read_text(), "#ifdef BIG_ENDI")
    )
    sincos_ok = compare("sin/cos table (source)", gen, src, quiet=not a.print)
    if a.search:
        search_sincos(src)
    compare("sin/cos table (libm.so.6)", gen, binary.doubles("__sincostab"), quiet=True)
    # the same table with glibc's numbers in place: the binary holds the source's table
    compare(
        "sin/cos table: libm.so.6 against the source",
        src,
        binary.doubles("__sincostab"),
        quiet=True,
    )

    # the large-argument reduction: the base-2^24 digits of 2/pi
    gen = two_over_pi_digits()
    text = (dbl / "branred.h").read_text()
    i = text.index("toverp[75]")
    body = text[text.index("{", i) : text.index("}", i)]
    src = [float(v) for v in re.findall(r"([0-9]+)\.0", body)]
    ok &= compare("2/pi digits (source)", gen, src)

    # the constants of sin and cos, from their definitions
    ok &= constants(dbl)

    # the other IBM tables: what a definition reproduces and what it does not (no failure: the
    # survey's counts are the finding; specs/glibc-dbl.md, "The tables")
    survey(dbl)

    print(
        "TABLES: 2/pi's digits and the sin/cos constants from their definitions: "
        + ("all" if ok else "NOT ALL")
        + "; the sin/cos table: "
        + ("all" if sincos_ok else "not all (its low parts, above)")
        + "; tan's points, asin/acos's and atan's tables: no definition (above)"
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

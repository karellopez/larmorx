# SPDX-License-Identifier: Apache-2.0
"""Statements of ``specs/freesurfer-maths.md`` checked on the prebuilt probe as models of its printed
numbers (black box: the probe's lines and the spec's text, nothing else).

    python -P models.py affine --probe P --scratch DIR
        §3's operand order: every ``affine`` line of the group ``affine`` (its seeded batch and the
        fixed batch ``affine-nan``) against AffineMV and AffineMM computed in float with x86-64's NaN
        rules (§7) in the order the spec gives: in each product the vector's number (for AffineMM the
        second matrix's number) is the left operand, in each sum the running sum
    python -P models.py scanf --scratch DIR [--seeds 1,2] [--n 3000]
        §18.1's errno at the end of the input: random inputs (numbers that overflow, underflow or
        not, words, white space) and formats (``%f``, ``%d``, ``%s`` with white space between and
        after) through the C library's ``sscanf`` (a small C program compiled with ``cc``: the C
        library as a black box), each call's errno against the rule the spec states

The probe runs through the oracle-output cache (one thread, the family ``freesurfer``) when the
validation package is importable (``PYTHONPATH=validation/src``), else directly. Exit status 1 when a
line differs from its model.
"""

from __future__ import annotations

import argparse
import importlib.util
import random
import subprocess
import sys
from decimal import Decimal
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent


def _compare():
    spec = importlib.util.spec_from_file_location("fs_maths_compare", HERE / "compare.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["fs_maths_compare"] = mod
    spec.loader.exec_module(mod)
    return mod


C = _compare()

DEFAULT_NAN = 0x7FC00000 | 0x80000000  # a NaN made from numbers: SSE's, the sign bit set


def isnan(b: int) -> bool:
    return (b & 0x7F800000) == 0x7F800000 and (b & 0x007FFFFF) != 0


def f32(b: int) -> np.float32:
    return np.array([b], dtype=np.uint32).view(np.float32)[0]


def sse(op, a: int, b: int) -> int:
    """One float operation with x86-64's NaN rules (§7): a NaN operand passes on quieted, the left
    one of two; a NaN made from numbers is the default NaN (the sign bit set)."""
    if isnan(a):
        return a | 0x00400000
    if isnan(b):
        return b | 0x00400000
    with np.errstate(all="ignore"):
        r = int(np.array([op(f32(a), f32(b))], dtype=np.float32).view(np.uint32)[0])
    return DEFAULT_NAN if isnan(r) else r


def fmul(a: int, b: int) -> int:
    return sse(lambda x, y: x * y, a, b)


def fadd(a: int, b: int) -> int:
    return sse(lambda x, y: x + y, a, b)


def to_affine(row_major: list[int]) -> list[int]:
    """§3: float i + 4j is the matrix's number at row-major place j + 4i."""
    return [row_major[(k // 4) + 4 * (k % 4)] for k in range(16)]


def column(a: list[int], v: list[int]) -> list[int]:
    out = []
    for i in range(4):
        acc = fmul(v[0], a[i])
        for j in (1, 2, 3):
            acc = fadd(acc, fmul(v[j], a[i + 4 * j]))
        out.append(acc)
    return out


def affine(runner, scratch: Path) -> int:
    batches = C.Gen(7).affine()
    good = bad = 0
    for b in batches:
        path = C.write_batch(scratch, b)
        out = C.parse(
            runner._run(
                [str(runner.probe), "batch", str(path)],
                dict(C.BASE_ENV),
                "",
                "freesurfer-maths-probe",
            )
        )
        for src, line in zip(b.lines, out, strict=True):
            tok = src.split()
            if tok[0] != "affine":
                continue
            a = to_affine([int(t, 16) for t in tok[4:20]])
            m2 = to_affine([int(t, 16) for t in tok[23:39]])
            vec = [int(t, 16) for t in tok[39:43]]
            res = line.split(b" | ")[0].decode().split()
            w = [int(t, 16) for t in res[4:8]]
            mm = [int(t, 16) for t in res[14:30]]
            want_mm = [x for j in range(4) for x in column(a, m2[4 * j : 4 * j + 4])]
            ok = w == column(a, vec) and mm == want_mm
            good += ok
            bad += not ok
            if not ok and bad <= 3:
                print(f"DIFFERS: {src[:120]}\n    probe {line[:200]!r}")
    print(f"affine: {good + bad} affine lines: as §3's order {good}, not {bad}")
    return 1 if bad else 0


# ---- §18.1: errno at the end of the input --------------------------------------------------------

SCANF_TOKENS = ["1", "-2", "1e40", "1e-50", "3.5", "1e", "x", "99999999999999999999", "ab"]


def scanf_case(r: random.Random) -> tuple[str, str, int]:
    ws = ["", " ", "  ", "\n"]
    inp = "".join(r.choice(ws) + r.choice(SCANF_TOKENS) for _ in range(r.randint(0, 3)))
    inp += r.choice(ws)
    fmt = "".join(
        r.choice(["", " ", "  "]) + r.choice(["%f", "%d", "%s"]) for _ in range(r.randint(1, 4))
    )
    fmt += r.choice(["", " ", "\n", "  "])
    return inp, fmt, r.choice([0, 7])


def out_of_range(text: str) -> bool:
    """ERANGE for a %f item (overflow, or a non-zero value below the smallest normal float; the
    tokens are far from the limits)."""
    mant, _, ex = text.lower().partition("e")
    m = abs(Decimal(mant))
    e = int(ex) if ex not in ("", "+", "-") else 0
    if m == 0:
        return False
    if abs(e) > 60:
        return True
    v = m * Decimal(10) ** e
    return v > Decimal("3.4028235677973366e38") or v < Decimal("1.1754943508222875e-38")


def scanf_rule(inp: str, fmt: str, before: int) -> int:
    """§18.1's rule: the first time the call meets the end of its input it keeps errno's value then
    (an item meets it before its own ERANGE), or 0 when that first meeting is a conversion's skip of
    the white space before its item; every later attempt of the call to read sets errno back to the
    kept value. White space in the format before a conversion reads nothing itself (the conversion
    skips it); white space at the format's end reads."""
    errno, pos, kept, n = before, 0, None, len(inp)

    def meet(first_value: int) -> None:
        nonlocal errno, kept
        if kept is None:
            kept = first_value
        errno = kept

    i = 0
    while i < len(fmt):
        if fmt[i].isspace():
            i += 1
            if i < len(fmt) and not fmt[i].isspace():
                continue  # the next conversion skips the white space
            if i < len(fmt):
                continue
            while pos < n and inp[pos].isspace():
                pos += 1
            if pos >= n:
                meet(errno)
            continue
        conv = fmt[i : i + 2]
        i += 2
        while pos < n and inp[pos].isspace():
            pos += 1
        if pos >= n:
            meet(0)
            return errno
        end = pos
        while end < n and not inp[end].isspace():
            end += 1
        tok = inp[pos:end]
        if conv == "%s":
            pos = end
            if pos >= n:
                meet(errno)
            continue
        j = 1 if tok[:1] in "+-" else 0
        d0 = j
        while j < len(tok) and tok[j].isdigit():
            j += 1
        if conv == "%f" and j < len(tok) and tok[j] == ".":
            j += 1
            while j < len(tok) and tok[j].isdigit():
                j += 1
        if j == d0:
            return errno  # a matching failure
        if conv == "%f" and j < len(tok) and tok[j] in "eE":
            j += 1
            if j < len(tok) and tok[j] in "+-":
                j += 1
            while j < len(tok) and tok[j].isdigit():
                j += 1
        pos += j
        if pos >= n:
            meet(errno)  # the item looked past its last character: before its own ERANGE
        text = tok[:j]
        if conv == "%d":
            if abs(int(text)) > 2**63 - 1:
                errno = 34
        elif out_of_range(text.rstrip("eE+-")):
            errno = 34
    return errno


def cstr(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"') + '"'


def scanf(scratch: Path, seeds: list[int], n: int) -> int:
    fixed = [
        ("1e40", "%f", 7),
        ("1e40", "%f ", 7),
        ("1e40", "%f %f", 7),
        ("1e40 ", "%f %f", 7),
        ("1 2 ", "%f %f %f", 7),
        ("1e-50 3", "%f %f %f", 7),
        ("-1877   1e40", "%f %f %f %f", 7),
        ("", "%f", 7),
    ]
    cases = list(fixed)
    for seed in seeds:
        r = random.Random(seed)
        cases += [scanf_case(r) for _ in range(n)]
    lines = ["#include <errno.h>", "#include <stdio.h>", "int main(void) {"]
    lines.append("  char s[8][64]; float f[8]; int d[8];")
    for inp, fmt, before in cases:
        args, nf, nd, ns = [], 0, 0, 0
        for k in range(len(fmt) - 1):
            if fmt[k] == "%":
                c = fmt[k + 1]
                if c == "f":
                    args.append(f"&f[{nf}]")
                    nf += 1
                elif c == "d":
                    args.append(f"&d[{nd}]")
                    nd += 1
                else:
                    args.append(f"s[{ns}]")
                    ns += 1
        call = f"sscanf({cstr(inp)}, {cstr(fmt)}{''.join(', ' + a for a in args)})"
        lines.append(f'  errno = {before}; {call}; printf("%d\\n", errno);')
    lines += ["  return 0;", "}"]
    src = scratch / "scanf_errno_cases.c"
    exe = scratch / "scanf_errno_cases"
    src.write_text("\n".join(lines) + "\n")
    subprocess.run(["cc", "-O0", "-w", "-o", str(exe), str(src)], check=True)
    got = [
        int(x)
        for x in subprocess.run(
            [str(exe)], capture_output=True, text=True, check=True
        ).stdout.split()
    ]
    bad = 0
    for (inp, fmt, before), g in zip(cases, got, strict=True):
        m = scanf_rule(inp, fmt, before)
        if m != g:
            bad += 1
            if bad <= 5:
                print(
                    f"DIFFERS: sscanf({inp!r}, {fmt!r}) errno {before} before: C library {g}, rule {m}"
                )
    print(
        f"scanf: {len(cases)} calls ({len(fixed)} fixed): as §18.1's rule {len(cases) - bad}, not {bad}"
    )
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["affine", "scanf"])
    ap.add_argument("--probe", type=Path)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--no-cache", action="store_true")
    ap.add_argument("--seeds", default="1,2")
    ap.add_argument("--n", type=int, default=3000)
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    if a.command == "scanf":
        return scanf(scratch, [int(x) for x in a.seeds.split(",") if x], a.n)
    if a.probe is None:
        ap.error("--probe is needed")
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    return affine(runner, scratch)


if __name__ == "__main__":
    sys.exit(main())

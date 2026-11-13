# SPDX-License-Identifier: Apache-2.0
"""glibc's ``sscanf`` at the end of the input: track E's 9,008 calls, for fs.core's ``cscan``.

The calls of track E's ``models.py scanf`` (``validation/investigations/freesurfer-maths-probe/``,
specs/freesurfer-maths.md §18.1: its eight fixed calls and, for seeds 1, 2 and 3, 3,000 random ones
of numbers, words and white space read with ``%f``, ``%d``, ``%s`` and white space, ``errno`` 0 or
7 before), compiled into one C program and run on this machine's C library (glibc 2.35, a black
box); writes the table ``<out>`` (input and format in hexadecimal, ``errno`` before, the return
value and ``errno`` after) that ``crates-freesurfer/larmorx-freesurfer-core/tests/scanf_errno_e.rs``
(ignored) runs through ``cscan::Scanf``:

    python -P validation/probes/fs-maths/scanf_errno_cases.py <out>
    ESCANF=<out> cargo test -p larmorx-freesurfer-core --test scanf_errno_e -- --ignored

2026-11-13 (track A relay 17): 9,008 calls, the return value and ``errno`` identical on every one.
"""

from __future__ import annotations

import random
import subprocess
import sys
import tempfile
from pathlib import Path

TOKENS = ["1", "-2", "1e40", "1e-50", "3.5", "1e", "x", "99999999999999999999", "ab"]
FIXED = [
    ("1e40", "%f", 7),
    ("1e40", "%f ", 7),
    ("1e40", "%f %f", 7),
    ("1e40 ", "%f %f", 7),
    ("1 2 ", "%f %f %f", 7),
    ("1e-50 3", "%f %f %f", 7),
    ("-1877   1e40", "%f %f %f %f", 7),
    ("", "%f", 7),
]


def case(r: random.Random) -> tuple[str, str, int]:
    """One call as ``models.py``'s ``scanf_case`` draws it."""
    ws = ["", " ", "  ", "\n"]
    inp = "".join(r.choice(ws) + r.choice(TOKENS) for _ in range(r.randint(0, 3)))
    inp += r.choice(ws)
    fmt = "".join(
        r.choice(["", " ", "  "]) + r.choice(["%f", "%d", "%s"]) for _ in range(r.randint(1, 4))
    )
    fmt += r.choice(["", " ", "\n", "  "])
    return inp, fmt, r.choice([0, 7])


def cstr(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"') + '"'


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        raise SystemExit("usage: scanf_errno_cases.py <out>")
    cases = list(FIXED)
    for seed in (1, 2, 3):
        r = random.Random(seed)
        cases += [case(r) for _ in range(3000)]
    lines = ["#include <errno.h>", "#include <stdio.h>", "int main(void) {"]
    lines.append("  char s[8][64]; float f[8]; int d[8]; int r;")
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
        lines.append(f'  errno = {before}; r = {call}; printf("%d %d\\n", r, errno);')
    lines += ["  return 0;", "}"]
    with tempfile.TemporaryDirectory(prefix="lx-scanf-") as d:
        src, exe = Path(d) / "c.c", Path(d) / "c"
        src.write_text("\n".join(lines) + "\n", encoding="utf-8")
        subprocess.run(["cc", "-O0", "-w", "-o", str(exe), str(src)], check=True)
        got = subprocess.run([str(exe)], capture_output=True, text=True, check=True).stdout
    rows = [
        f"{inp.encode().hex() or '-'} {fmt.encode().hex()} {before} {g}"
        for (inp, fmt, before), g in zip(cases, got.splitlines(), strict=True)
    ]
    Path(argv[0]).write_text("\n".join(rows) + "\n", encoding="utf-8")
    print(f"{len(rows)} calls")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

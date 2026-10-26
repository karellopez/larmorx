# SPDX-License-Identifier: Apache-2.0
"""Whether glibc 2.35's ``pow`` and ``exp`` are Arm's Optimized Routines v19.11 (MIT): the
data tables compared number by number, the code expression by expression.

larmorx-nc's ``glibc_double.rs`` is translated from Arm's MIT files (the FSL-licensed package
takes no LGPL code: decision log, 2026-10-24). This script shows that the result is glibc's
``pow`` and ``exp``: it downloads Arm's ``math/pow.c``, ``pow_log_data.c``, ``exp.c``,
``exp_data.c`` and ``math_config.h`` at the tag v19.11, preprocesses the data files with Arm's
own configuration for x86-64 (``gcc -E``: EXP_TABLE_BITS 7, EXP_POLY_ORDER 5, POW_LOG_TABLE_BITS
7, POW_LOG_POLY_ORDER 8), compares every number of ``__exp_data`` and ``__pow_log_data`` with
glibc 2.35's ``sysdeps/ieee754/dbl-64/e_exp_data.c`` and ``e_pow_log_data.c``, and prints the
code's differences (``diff`` of the files after their ``#include`` lines), which are macro
names only (``HAVE_FAST_FMA``/``__FP_FAST_FMA``, ``eval_as_double``/``math_narrow_eval``,
``unlikely``/``__glibc_unlikely``) and Arm's other configurations behind ``#if``.

    python -P arm_glibc_pow_exp.py <work dir> [--glibc <glibc-2.35 source>]

The glibc source defaults to the workspace's ``tags/glibc-2.35``. Needs ``gcc`` and the network.
Result on 2026-10-25: ``__exp_data``: 270 of 270 numbers identical; ``__pow_log_data``: 393 of
393; the code: macro names only.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ARM = "https://raw.githubusercontent.com/ARM-software/optimized-routines/v19.11/"
FILES = ["math/pow.c", "math/pow_log_data.c", "math/exp.c", "math/exp_data.c", "math/math_config.h"]
NUM = re.compile(r"-?0x[0-9a-fA-F.]+p[-+]?\d+|-?0x[0-9a-fA-F]+\b(?![.p])|\b\d+\.\d*(?:e[-+]?\d+)?")


def numbers(text: str, key: str) -> list[float | int]:
    """The number literals of the definition ``key`` onwards, comments removed."""
    text = text[text.index(key) :]
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    text = re.sub(r"//[^\n]*", " ", text)
    out: list[float | int] = []
    for x in NUM.findall(text):
        if "0x" in x.lower():
            out.append(float.fromhex(x) if "p" in x.lower() else int(x, 16))
        else:
            out.append(float(x))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("work", type=Path)
    here = Path(__file__).resolve()
    ws = next((p for p in here.parents if (p / "tags").is_dir()), None)
    default = ws / "tags" / "glibc-2.35" if ws else None
    ap.add_argument("--glibc", type=Path, default=default)
    a = ap.parse_args()
    if a.glibc is None or not a.glibc.is_dir():
        sys.exit("glibc 2.35's source not found (--glibc)")
    g = a.glibc / "sysdeps" / "ieee754" / "dbl-64"
    arm = a.work / "arm-v19.11"
    arm.mkdir(parents=True, exist_ok=True)
    for f in FILES:
        dest = arm / Path(f).name
        if not dest.exists():
            dest.write_bytes(urllib.request.urlopen(ARM + f, timeout=60).read())
    same = True
    for gfile, afile, key in (
        ("e_exp_data.c", "exp_data.c", "__exp_data ="),
        ("e_pow_log_data.c", "pow_log_data.c", "__pow_log_data ="),
    ):
        pre = subprocess.run(
            ["gcc", "-E", "-P", "-I", str(arm), str(arm / afile)],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        gn = numbers((g / gfile).read_text(encoding="utf-8"), key)
        an = numbers(pre, key)
        ok = gn == an
        same &= ok
        print(f"{key.split()[0]}: glibc {len(gn)} numbers, Arm {len(an)}; identical: {ok}")
    for gfile, afile in (("e_pow.c", "pow.c"), ("e_exp.c", "exp.c")):
        gt = (g / gfile).read_text(encoding="utf-8")
        at = (arm / afile).read_text(encoding="utf-8")
        gp, apath = a.work / f"glibc-{gfile}", a.work / f"arm-{afile}"
        gp.write_text(gt[gt.index("#include") :], encoding="utf-8")
        apath.write_text(at[at.index("#include") :], encoding="utf-8")
        d = subprocess.run(["diff", str(apath), str(gp)], capture_output=True, text=True).stdout
        print(f"--- the code: Arm's {afile} (<) and glibc's {gfile} (>)")
        print(d)
    return 0 if same else 1


if __name__ == "__main__":
    sys.exit(main())

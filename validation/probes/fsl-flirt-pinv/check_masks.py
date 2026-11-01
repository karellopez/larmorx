# SPDX-License-Identifier: Apache-2.0
"""Checks the closed form larmorx-nc-flirt uses for FLIRT's parameter-subset pseudo-inverse
against FSL 6.0.7.23's own (``pinv_probe``, Armadillo 14.2.1's ``pinv`` on FSL's LAPACK).

For a mask whose columns hold only 0 and 1 and never share a row, LAPACK's divide-and-conquer
SVD gives, for a column of ``k`` ones, ``(1 / sqrt(k))**2`` at each of its rows (``sqrt`` and the
reciprocal correctly rounded, then the product rounded; ``k = 3`` gives ``0x1.5555555555557p-2``,
two units above the rounded 1/3), and zero elsewhere. This script builds masks with every ``k``
from 1 to 12, in two columns, at shifted rows, runs the probe and compares.

Usage::

    python -P check_masks.py --probe <path to pinv_probe>
"""

from __future__ import annotations

import argparse
import math
import os
import subprocess
import sys


def masks() -> list[list[list[float]]]:
    out = []
    for k in range(1, 13):
        for shift in range(0, 12 - k + 1, 3):
            c1 = [0.0] * 12
            for r in range(shift, shift + k):
                c1[r] = 1.0
            rest = [r for r in range(12) if c1[r] == 0.0]
            cols = [c1]
            if rest:
                c2 = [0.0] * 12
                for r in rest[: max(1, len(rest) // 2)]:
                    c2[r] = 1.0
                cols.append(c2)
            out.append(cols)
    return out


def expected(col: list[float]) -> list[float]:
    k = sum(1 for v in col if v == 1.0)
    e = 1.0 if k == 1 else (1.0 / math.sqrt(k)) ** 2
    return [e if v == 1.0 else 0.0 for v in col]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True)
    args = ap.parse_args()
    ms = masks()
    text = "".join(
        f"{len(cols)} " + " ".join(repr(v) for col in cols for v in col) + "\n" for cols in ms
    )
    env = dict(os.environ, OPENBLAS_CORETYPE="Haswell")
    p = subprocess.run(
        [args.probe, "--stdin"], input=text, capture_output=True, text=True, env=env, check=True
    )
    lines = [ln for ln in p.stdout.splitlines() if ln.strip()]
    i = 0
    bad = 0
    for cols in ms:
        nr = int(lines[i].split()[1])
        rows = [[float.fromhex(t) for t in lines[i + 1 + r].split()] for r in range(nr)]
        i += 1 + nr
        for r, col in enumerate(cols):
            if rows[r] != expected(col):
                bad += 1
                print("mismatch", col, [v.hex() for v in rows[r]])
    print(f"{len(ms)} masks, {bad} rows differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

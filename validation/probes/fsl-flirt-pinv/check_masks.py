# SPDX-License-Identifier: Apache-2.0
"""Checks the closed form larmorx-nc-flirt uses for FLIRT's parameter-subset pseudo-inverse
against FSL 6.0.7.23's own (``pinv_probe``, Armadillo 14.2.1's ``pinv`` on FSL's LAPACK).

The replica computes ``pinv(parammask)`` in closed form for the masks it accepts, and refuses
the others (their pseudo-inverse is LAPACK's divide-and-conquer SVD, part 2 of the port):

* masks whose columns are single ones on distinct rows (FLIRT's 12-dof identity, the search's
  translations, the 2D schedule's ``rz, tx, ty``, every ``paramsubset`` of FSL's shipped
  schedules): the pseudo-inverse is the transpose;
* the search's 12 x 4 mask (one common scale on rows 7 to 9, then the three translations):
  ``(1 / sqrt(3))**2 = 0x1.5555555555557p-2`` on the scale's rows (two units above the rounded
  1/3) and ones on the translations'.

Both kinds are asserted here. Masks the replica refuses (columns of several ones beside other
columns, a single column of ``k`` ones, values other than 0 and 1) are only reported, with how
many of their rows the closed form would miss: they show why the replica refuses them.

Usage::

    python -P check_masks.py --probe <path to pinv_probe>
"""

from __future__ import annotations

import argparse
import itertools
import os
import subprocess
import sys

Mask = list[list[float]]


def _unit(r: int) -> list[float]:
    c = [0.0] * 12
    c[r] = 1.0
    return c


def accepted() -> list[Mask]:
    """Masks of single-one columns on distinct rows (every column count, several orders), and
    the search's 12 x 4 mask."""
    out: list[Mask] = [[_unit(r) for r in range(12)]]
    for n in range(1, 13):
        for start in range(0, 12, 5):
            rows = [(start + 7 * i) % 12 for i in range(n)]
            if len(set(rows)) == n:
                out.append([_unit(r) for r in rows])
    for rows in itertools.combinations(range(12), 2):
        out.append([_unit(r) for r in rows])
    scale = [0.0] * 12
    for r in (6, 7, 8):
        scale[r] = 1.0
    out.append([scale, _unit(3), _unit(4), _unit(5)])
    return out


def refused() -> list[Mask]:
    """Masks the replica refuses: one column of ``k`` ones, and a column of ``k`` ones beside a
    second column."""
    out: list[Mask] = []
    for k in range(2, 13):
        c1 = [1.0 if r < k else 0.0 for r in range(12)]
        out.append([c1])
        rest = [r for r in range(12) if c1[r] == 0.0]
        if rest:
            c2 = [1.0 if r in rest[: max(1, len(rest) // 2)] else 0.0 for r in range(12)]
            out.append([c1, c2])
    return out


def closed_form(cols: Mask) -> list[list[float]]:
    """The replica's closed form: a column of ``k`` ones gives ``(1 / sqrt(k))**2`` at its rows
    (1 for ``k = 1``), zero elsewhere."""
    rows = []
    for col in cols:
        k = sum(1 for v in col if v == 1.0)
        e = 1.0 if k == 1 else (1.0 / k**0.5) ** 2
        rows.append([e if v == 1.0 else 0.0 for v in col])
    return rows


def probe(path: str, masks: list[Mask]) -> list[list[list[float]]]:
    text = "".join(
        f"{len(cols)} " + " ".join(repr(v) for col in cols for v in col) + "\n" for cols in masks
    )
    env = dict(os.environ, OPENBLAS_CORETYPE="Haswell")
    p = subprocess.run(
        [path, "--stdin"], input=text, capture_output=True, text=True, env=env, check=True
    )
    lines = [ln for ln in p.stdout.splitlines() if ln.strip() and ln.startswith((" ", "stdin"))]
    out, i = [], 0
    for _ in masks:
        nr = int(lines[i].split()[1])
        out.append([[float.fromhex(t) for t in lines[i + 1 + r].split()] for r in range(nr)])
        i += 1 + nr
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True)
    args = ap.parse_args()
    ok = accepted()
    bad = 0
    for cols, got in zip(ok, probe(args.probe, ok), strict=True):
        if got != closed_form(cols):
            bad += 1
            print("accepted mask differs:", [[r for r in range(12) if c[r]] for c in cols])
    print(f"accepted: {len(ok)} masks, {bad} differ from the closed form")
    no = refused()
    missed = 0
    for cols, got in zip(no, probe(args.probe, no), strict=True):
        missed += sum(1 for a, b in zip(got, closed_form(cols), strict=True) if a != b)
    print(f"refused (reported only): {len(no)} masks, {missed} rows the closed form would miss")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

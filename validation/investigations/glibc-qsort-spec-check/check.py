# SPDX-License-Identifier: Apache-2.0
"""Checks specs/glibc-qsort.md: the spec writer's model against the C library (the open probe).

    python -P check.py <glibc_qsort_probe> [--quick]

The groups of the spec's §9 (1 small arrays, 2 around the stack limit, 3 large arrays, 4 the
fallback, 5 large elements and qsort_r). One line per group: cases, identical (of them, the cases
where both leave the array, §5, at the same call: the calls before it compared), the first
difference. --quick: a tenth of the cases.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import model

KINDS_INT = ("3way", "tie+", "tie-")
INT_SIZES = (1, 2, 4, 8, 12, 16, 24, 32)


def groups(quick: bool) -> dict[str, list[tuple]]:
    g: dict[str, list[tuple]] = {k: [] for k in ("1", "2", "3", "4", "5")}
    seeds = (11, 12)
    for n in range(65):
        for seed in seeds:
            for s in INT_SIZES:
                for kind in KINDS_INT:
                    for kr in (1, 2, 3, 1000):
                        if s == 1 and kr > 256:
                            continue
                        g["1"].append((s, n, kind, seed, kr, "qsort", 0))
            for s in (8, 12, 16, 24, 32):
                g["1"].append((s, n, "hash", seed, 3, "qsort", 0))
            for kind, s in (("f3", 4), ("f2", 4), ("d3", 8), ("d2", 8)):
                for kr in (3, 12):
                    g["1"].append((s, n, kind, seed, kr, "qsort", 0))
    for s, lo, hi in ((4, 250, 262), (8, 124, 132), (12, 82, 90), (32, 30, 34)):
        for n in range(lo, hi):
            for kind in ("3way", "tie+", "tie-", "hash"):
                if kind == "hash" and s < 8:
                    continue
                for fail in (0, 1):
                    g["2"].append((s, n, kind, 21, 3, "qsort", fail))
    for n in (1000, 4096, 10007, 100000):
        for kr in (3, 1000):
            for s, kind in (
                (4, "3way"),
                (12, "tie+"),
                (16, "tie-"),
                (16, "hash"),
                (4, "f3"),
                (8, "d2"),
            ):
                if kind[0] in "fd" and kr == 1000:
                    continue
                g["3"].append((s, n, kind, 31 + n, kr if kind[0] not in "fd" else 12, "qsort", 0))
                if n <= 10007:
                    g["4"].append(
                        (s, n, kind, 41 + n, kr if kind[0] not in "fd" else 12, "qsort", 1)
                    )
    for n in range(65):
        for seed in seeds:
            for s in (1024, 1040):
                for kind in (*KINDS_INT, "hash"):
                    g["4"].append((s, n, kind, seed, 3, "qsort", 1))
    for n in list(range(256, 300)):
        for kind in ("f3", "f2"):
            g["4"].append((4, n, kind, 51, 12, "qsort", 1))
    for n in list(range(128, 150)):
        for kind in ("d3", "d2"):
            g["4"].append((8, n, kind, 52, 12, "qsort", 1))
    for s in (33, 40, 72, 104, 160):
        for n in (*range(65), 1000):
            for kind in ("3way", "tie+", "hash"):
                g["5"].append((s, n, kind, 61, 3, "qsort", 0))
                if n % 7 == 0:
                    g["5"].append((s, n, kind, 61, 3, "qsort_r", 0))
                    g["5"].append((s, n, kind, 62, 3, "qsort_r", 1))
    if quick:
        g = {k: v[::10] for k, v in g.items()}
    return g


def probe_run(probe: str, cases: list[tuple]) -> list[str]:
    """The probe's lines for the cases; one process for all, or one per case if that one fails."""
    text = "".join(" ".join(map(str, c)) + "\n" for c in cases)
    out = subprocess.run([probe], input=text, capture_output=True, text=True)
    lines = out.stdout.splitlines()
    if out.returncode == 0 and len(lines) == len(cases):
        return lines
    res = []
    for c in cases:
        one = subprocess.run(
            [probe], input=" ".join(map(str, c)) + "\n", capture_output=True, text=True
        )
        res.append(one.stdout.strip() if one.returncode == 0 else f"probe exit {one.returncode}")
    return res


def main() -> int:
    probe = sys.argv[1]
    quick = "--quick" in sys.argv
    total_bad = 0
    for name, cases in groups(quick).items():
        lines = probe_run(probe, cases)
        same = outside = 0
        first = None
        for case, line in zip(cases, lines, strict=True):
            want = model.run_case(*case[:5], case[6])
            if line == want:
                same += 1
                outside += want.endswith("outside")
            elif first is None:
                first = f"{' '.join(map(str, case))}: probe '{line}', model '{want}'"
        bad = len(cases) - same
        total_bad += bad
        print(
            f"group {name}: {len(cases)} cases, {same} identical ({outside} of them leaving the array"
            " at the same call)" + (f", {bad} differ; first: {first}" if bad else "")
        )
    print("ALL IDENTICAL" if total_bad == 0 else f"DIFFERENCES: {total_bad}")
    return 0 if total_bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

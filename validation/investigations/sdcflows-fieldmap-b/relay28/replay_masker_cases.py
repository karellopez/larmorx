# SPDX-License-Identifier: Apache-2.0
"""Relay 28: the random-testing trial's ``brain_masker`` cases replayed through their runner
(``relay27/run_one.py``: sdcflows 2.17.1 in-process against larmorx's ``reproduce`` bit for
bit, then the D13 fix pass) after the random walker's change (its conjugate gradients stop once
the system is NaN): every case of a night that timed out or took more than a minute, and
``--others`` of the rest (drawn with a fixed seed), each in a process of its own.

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \\
    PYTHONPATH=<repo>/python:<repo>/validation/src LARMORX_WORKSPACE=<workspace> \\
        python -P replay_masker_cases.py oracles/difftest/2026-11-09-fieldmaps-b [--others 40] \\
        [--jobs 14]

The night with numpy's AVX-512 dispatch off (``2026-11-09-fieldmaps-b-noavx512``) is replayed
with ``NPY_DISABLE_CPU_FEATURES`` set as that night had it (``difftest.md``). Run inside a memory
box (``scripts/orchestration/boxed.sh``: 14 cases at once took about 3 GiB each). Prints one line
per case (seconds, checks, failures; the night's seconds and verdict) and ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import json
import random
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

RUN_ONE = Path(__file__).resolve().parents[1] / "relay27" / "run_one.py"


def select(night: Path, others: int) -> list[tuple[str, float, str]]:
    slow, rest = [], []
    with open(night / "results.jsonl", encoding="utf-8") as f:
        for line in f:
            r = json.loads(line)
            if r.get("tool") != "sdcflows-estimation":
                continue
            if r["features"].get("call") != "brain_masker":
                continue
            spec = f"sdcflows-estimation:{r['seed']}:{r['n']}"
            row = (spec, r.get("seconds") or 0.0, r["verdict"])
            (slow if r["verdict"] != "agree" or row[1] > 60 else rest).append(row)
    random.Random(28).shuffle(rest)
    return slow + rest[:others]


def replay(spec: str) -> tuple[float, str]:
    t0 = time.perf_counter()
    p = subprocess.run(
        [sys.executable, "-P", str(RUN_ONE), spec],
        capture_output=True,
        text=True,
        timeout=3000,
        check=False,
    )
    last = [ln for ln in p.stdout.splitlines() if ln.startswith(spec)]
    return time.perf_counter() - t0, last[-1] if last else f"{spec} exit {p.returncode}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night", type=Path)
    ap.add_argument("--others", type=int, default=40)
    ap.add_argument("--jobs", type=int, default=14)
    args = ap.parse_args()
    cases = select(args.night, args.others)
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda c: replay(c[0]), cases))
    bad = 0
    for (_spec, before, verdict), (seconds, line) in zip(cases, results, strict=True):
        ok = line.endswith(" 0 failed")
        bad += not ok
        print(f"{seconds:8.1f} s (the night: {before:7.1f} s, {verdict}) {line}")
    print(f"{len(cases)} cases")
    print("ALL OK" if not bad else f"FAIL {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

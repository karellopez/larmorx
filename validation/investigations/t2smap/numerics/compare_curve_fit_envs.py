# SPDX-License-Identifier: Apache-2.0
"""The library step of tedana's curve fits: SciPy 1.15.3/numpy 2.3.5 against 1.18.1/numpy 2.5.4.

Reads the folders ``run_curve_fit_check.sh`` wrote (``<scratch>/dump-shared/<run>`` and
``<scratch>/dump-newest/<run>``: tedana's own ``curve_fit`` calls and results in each environment,
on the same inputs) and prints, per run, the failures and how many fits' parameters, evaluation
counts or outcomes differ between the two environments (largest and median relative difference).

    python -P compare_curve_fit_envs.py <scratch dir>
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

import numpy as np

RUNS = ["ds003592", "ds000210-cuedSGT", "ds000210-rest", "ds000258"]


def fits(folder: Path) -> list[bytes]:
    blob = (folder / "expected.bin").read_bytes()
    kinds = (folder / "kinds.txt").read_text(encoding="utf-8").split()
    out = []
    at = 0
    for kind in kinds:
        (n,) = struct.unpack("<I", blob[at : at + 4])
        if kind == "fit":
            out.append(blob[at + 4 : at + 4 + n])
        at += 4 + n
    return out


def main() -> int:
    base = Path(sys.argv[1])
    for name in RUNS:
        a = fits(base / "dump-shared" / name)
        b = fits(base / "dump-newest" / name)
        failures: dict[str, int] = {}
        for x in a:
            if x[0] == 0:
                key = x[1:].decode()
                failures[key] = failures.get(key, 0) + 1
        diff = status = outcome = 0
        rel = []
        for x, y in zip(a, b, strict=True):
            if x == y:
                continue
            if x[0] != y[0]:
                outcome += 1
                continue
            if x[0] == 0:
                continue
            px = np.frombuffer(x[1:17], "<f8")
            py = np.frombuffer(y[1:17], "<f8")
            if px.tobytes() != py.tobytes():
                diff += 1
                rel.append(float(np.max(np.abs(px - py) / np.abs(px))))
            if x[50:58] != y[50:58]:
                status += 1
        print(
            f"{name}: {len(a)} fits; failures {failures}; parameters differ in {diff} "
            f"(largest relative {max(rel, default=0):.3g}, median {np.median(rel) if rel else 0:.3g}); "
            f"evaluations or status differ in {status}; outcome differs in {outcome}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

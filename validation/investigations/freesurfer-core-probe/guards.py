# SPDX-License-Identifier: Apache-2.0
"""Whether FreeSurfer's selections and median depend on memory outside the list on lists with NaNs
and infinities (specs/freesurfer-core.md §11): 3,000 seeded lists through the probe's ``kth``,
``qselect`` and ``median``, run with three guards (FS_CORE_PROBE_GUARD 7777, -7777 and 0, the values
placed just before and after each list); prints how many results or reorderings change with the guard.

Usage: python -P guards.py <fs_core_probe> <scratch folder>
A one-off observation of the spec writer (outside the oracle-output cache); suites use oracle_cache.run.
"""

from __future__ import annotations

import random
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

PROBE, OUT = sys.argv[1], Path(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)
rng = random.Random(5)
NAN = float("nan")


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


lines = []
for _ in range(3000):
    n = rng.randint(1, 15)
    vals = []
    for _ in range(n):
        u = rng.random()
        if u < 0.2:
            vals.append(NAN)
        elif u < 0.25:
            vals.append(float("inf") * rng.choice([1, -1]))
        else:
            vals.append(float(rng.randint(-5, 5)))
    cmd = rng.choice(["kth", "qselect", "median"])
    k = "" if cmd == "median" else f" {rng.randint(1, n)}"
    lines.append(f"{cmd}{k} " + " ".join(f32(v) for v in vals))
batch = OUT / "guards.txt"
batch.write_text("\n".join(lines) + "\n")
results = {}
for guard in ("7777", "-7777", "0"):
    p = subprocess.run(
        [PROBE, "batch", str(batch)],
        env={"FS_CORE_PROBE_GUARD": guard},
        capture_output=True,
        timeout=600,
        check=True,
    )
    results[guard] = p.stdout.decode().split("\n")
a, b, c = results["7777"], results["-7777"], results["0"]
changed = [i for i in range(len(lines)) if not (a[i] == b[i] == c[i])]
print(f"{len(changed)} of {len(lines)} lists give a result or an order that depends on the guard")
print(Counter(lines[i].split()[0] for i in changed))

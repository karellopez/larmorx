# SPDX-License-Identifier: Apache-2.0
"""bench_options.py <larmorx program> <larmorx-nc program> <out dir> [threads ...]: the wall
time of ``bet -S``, ``-B``, ``-A`` (the adult T1w of ds000005) and ``-A2`` (the 7.5-year-old's
T1w and T2w of ds000256), as the BET suite's real cases call them: FSL 6.0.7.23 (one thread; the
oracle cache's time of its run), the replica (``larmorx-nc``, one thread) and the clean-room
original at each thread count (default 1 and 28), each in a fresh directory, one run at a time
(the clean-room catch-up, relay 6, 2026-11-05). Reported, not gated (``specs/bet.md`` §11.1)."""

import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from larmorx_validation.oracle_cache import core
from larmorx_validation.parity import fsl_bet as b

LX, NC, OUT = sys.argv[1], sys.argv[2], Path(sys.argv[3])
#: ``--fsl-only``: FSL's times alone (served from the cache: no program runs)
FSL_ONLY = "--fsl-only" in sys.argv[4:]
THREADS = [int(t) for t in sys.argv[4:] if t != "--fsl-only"] or [1, 28]
OUT.mkdir(parents=True, exist_ok=True)
T1 = str(b.input_path("real:adult-ds000005"))
C1, C2 = str(b.input_path("real:child-7y")), str(b.input_path("real:t2:child-7y"))
CALLS = [
    ("-S -m", [T1, "out", "-S", "-m"]),
    ("-B -m", [T1, "out", "-B", "-m"]),
    ("-A", [T1, "out", "-A"]),
    ("-A2 -m", [C1, "out", "-A2", C2, "-m"]),
]


def timed(cmd: list[str], env: dict[str, str]) -> tuple[int, float]:
    d = Path(tempfile.mkdtemp(prefix="bench_", dir=OUT))
    t = time.perf_counter()
    p = subprocess.run(cmd, cwd=d, env=env, capture_output=True, check=False)
    return p.returncode, time.perf_counter() - t


env = {**os.environ, **b.environment()}
for name, args in CALLS:
    d = Path(tempfile.mkdtemp(prefix="fsl_", dir=OUT))
    # the cache's tally counts a served run's recorded time inside a case
    core.begin_case(f"bench/{name}")
    t0 = time.perf_counter()
    r = b.oracle_run("bet", args, env, d)
    ran = time.perf_counter() - t0
    tally = core.end_case()
    fsl = tally.saved_seconds if tally.served else ran
    row = [f"bet {name}", f"FSL {fsl:.1f} s (exit {r.code}{', cached' if tally.served else ''})"]
    if FSL_ONLY:
        print(" | ".join(row), flush=True)
        continue
    code, s = timed([NC, "fsl", "bet", *args], {**env, "OMP_NUM_THREADS": "1"})
    row.append(f"replica 1 thread {s:.1f} s ({code})")
    for n in THREADS:
        e = {
            **env,
            "OMP_NUM_THREADS": str(n),
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_NC_BIN": NC,
        }
        code, s = timed([LX, "fsl", "bet", *args], e)
        row.append(f"original {n} thread{'s' if n > 1 else ''} {s:.1f} s ({code})")
    print(" | ".join(row), flush=True)

# SPDX-License-Identifier: Apache-2.0
"""`mi_trace.py` over a spread of a random-testing run's `mri-hmc` findings of one cost: `n`
findings taken at a regular step through the sorted list (every fifth or sixth of the 2026-10-25
run's 91 `normmi` and 78 `mutualinfo` findings for n = 15).

    python -P trace_many.py <run> <work> <traced larmorx> <cost> <n> [--env K=V]...
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main(argv: list[str]) -> int:
    run, work, lx, cost, n = argv[0], argv[1], argv[2], argv[3], int(argv[4])
    env = []
    rest = argv[5:]
    while rest:
        if rest.pop(0) == "--env":
            env += ["--env", rest.pop(0)]
    picked = []
    for f in sorted((Path(run) / "findings" / "mri-hmc").glob("*-original.json")):
        j = json.loads(f.read_text(encoding="utf-8"))
        r = j.get("shrunk") or j["recipe"]
        words = [w for g in r["groups"] for w in g["args"]]
        if f"-cost {cost}" in " ".join(words):
            picked.append(str(f))
    picked = picked[:: max(1, len(picked) // n)][:n]
    args = [sys.executable, "-P", str(HERE / "mi_trace.py"), work, "--lx", lx, "--cost", cost, *env]
    for p in picked:
        args += ["--case", p]
    return subprocess.run(args).returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

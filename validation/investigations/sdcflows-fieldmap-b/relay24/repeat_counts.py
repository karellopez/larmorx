# SPDX-License-Identifier: Apache-2.0
"""Whether a check script reports the same thing on every run: runs it ``--runs`` times (each a
new process of this interpreter, ``python -P``) and compares what a parity record takes from
its output (every line but ``TIME``/``time`` lines and the progress lines ending in ``done``),
and its ``STEP`` lines when ``--step`` is given (``LARMORX_STEP_LINES`` set). Prints the
number of distinct outputs (1: the same on every run) and, otherwise, the lines that differ.

Written by relay 24 of the field maps' part B for check_ndimage's SCI-21 count, which could
differ by one between runs while a SciPy process that writes past its output's end crashed
before printing (now every call is one case: ``check_ndimage._scipy_in_process``).

Usage (the repository's ``python/`` and ``validation/src`` on ``PYTHONPATH``)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P repeat_counts.py --runs 5 \\
        [--step] -- check_ndimage.py --quick --sections arguments
"""

from __future__ import annotations

import argparse
import collections
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent


def kept(out: str) -> tuple[str, ...]:
    lines = []
    for raw in out.splitlines():
        s = raw.strip()
        if not s or s.startswith(("TIME ", "time ")) or s.endswith(" done"):
            continue
        lines.append(s)
    return tuple(lines)


def main() -> int:
    argv = sys.argv[1:]
    if "--" not in argv:
        raise SystemExit(__doc__)
    k = argv.index("--")
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--step", action="store_true")
    args = ap.parse_args(argv[:k])
    script = argv[k + 1 :]
    path = script[0] if os.path.isabs(script[0]) else str(HERE / script[0])
    env = dict(os.environ)
    if args.step:
        env["LARMORX_STEP_LINES"] = "1"
    outs = collections.Counter()
    codes = collections.Counter()
    for _ in range(args.runs):
        p = subprocess.run(
            [sys.executable, "-P", path, *script[1:]],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        outs[kept(p.stdout)] += 1
        codes[p.returncode] += 1
    print(f"{args.runs} runs: {len(outs)} distinct outputs; exit statuses {dict(codes)}")
    if len(outs) > 1:
        sets = [set(o) for o in outs]
        common = set.intersection(*sets)
        for i, o in enumerate(outs):
            print(f"output {i + 1} ({outs[o]} runs):")
            for line in o:
                if line not in common:
                    print(f"    {line[:200]}")
    return 0 if len(outs) == 1 and set(codes) == {0} else 1


if __name__ == "__main__":
    sys.exit(main())

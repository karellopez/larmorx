# SPDX-License-Identifier: Apache-2.0
"""Speed of BET's clean-room original against FSL 6.0.7.23 and the replica (``larmorx-nc``) on
real calls: ``bet2 <in> out -m`` (and fMRIPrep's ``bet <boldref> out -f 0.20 -m``), wall time,
the median of ``--repeat`` runs, each program in a clean environment (FSL off the ``PATH``),
outputs NIfTI-1 gzipped.

``python -P bench.py --larmorx <larmorx binary> --replica <larmorx-nc binary> --threads 1,24
--repeat 3 <name>=<image>...`` prints a Markdown table (seconds) and each run's command.
"""

from __future__ import annotations

import argparse
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import fslrun


def timed(cmd: list[str], env: dict[str, str], repeat: int) -> float:
    times = []
    for _ in range(repeat):
        with tempfile.TemporaryDirectory(prefix="lxbench_") as d:
            t = time.perf_counter()
            p = subprocess.run(cmd, cwd=d, env=env, capture_output=True, text=True, check=False)
            times.append(time.perf_counter() - t)
            if p.returncode != 0:
                raise RuntimeError(f"{cmd}: exit {p.returncode}: {p.stderr[-400:]}")
    return statistics.median(times)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--larmorx", required=True)
    ap.add_argument("--replica", default=None)
    ap.add_argument("--threads", default="1")
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--tool", default="bet2")
    ap.add_argument("--args", default="-m")
    ap.add_argument("images", nargs="+")
    a = ap.parse_args(argv)
    threads = [int(t) for t in a.threads.split(",")]
    extra = a.args.split()
    head = ["image", "FSL"]
    if a.replica:
        head += [f"replica ({t} thr)" for t in threads]
    head += [f"original ({t} thr)" for t in threads]
    print("| " + " | ".join(head) + " |")
    print("|" + "---|" * len(head))
    for item in a.images:
        name, path = item.split("=", 1)
        env = fslrun.environment()
        fsl = timed([str(fslrun.FSLDIR / "bin" / a.tool), path, "out", *extra], env, a.repeat)
        row = [name, f"{fsl:.2f}"]
        if a.replica:
            for t in threads:
                e = {**env, "OMP_NUM_THREADS": str(t)}
                row.append(
                    f"{timed([a.replica, 'fsl', a.tool, path, 'out', *extra], e, a.repeat):.2f}"
                )
        for t in threads:
            e = {**env, "OMP_NUM_THREADS": str(t), "LARMORX_IMPLEMENTATION": "original"}
            row.append(f"{timed([a.larmorx, 'fsl', a.tool, path, 'out', *extra], e, a.repeat):.2f}")
        print("| " + " | ".join(row) + " |", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

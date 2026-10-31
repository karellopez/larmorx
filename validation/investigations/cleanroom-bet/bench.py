# SPDX-License-Identifier: Apache-2.0
"""Speed of BET's clean-room original against FSL 6.0.7.23 and the replica (``larmorx-nc``) on
real calls: ``bet2 <in> out -m`` (and fMRIPrep's ``bet <boldref> out -f 0.20 -m``, or betsurf),
wall time, the median of ``--repeat`` runs, each program in a clean environment (FSL off the
``PATH``), outputs NIfTI-1 gzipped.

``python -P bench.py --larmorx <larmorx binary> --replica <larmorx-nc binary> --threads 1,24
--repeat 3 [--tool bet2 --args "-m"] [--json report.json] <name>=<inputs>...`` prints a
Markdown table (seconds) and, with ``--json``, adds the runs to a benchmark report in the form
of ``docs/benchmarks/*.json`` (its environment, one result per program, input and thread count).
An input with several files (betsurf's T1, mesh and matrix) is ``name=<file>,<file>,...``: they
replace the call's ``{0}``, ``{1}``, ... in ``--args`` (default: the file, then ``out``).
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import fslrun


def timed(cmd: list[str], env: dict[str, str], repeat: int) -> list[float]:
    times = []
    for _ in range(repeat):
        with tempfile.TemporaryDirectory(prefix="lxbench_") as d:
            t = time.perf_counter()
            p = subprocess.run(cmd, cwd=d, env=env, capture_output=True, text=True, check=False)
            times.append(time.perf_counter() - t)
            if p.returncode != 0:
                raise RuntimeError(f"{cmd}: exit {p.returncode}: {p.stderr[-400:]}")
    return times


def environment() -> dict:
    cpu = "?"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    commit = subprocess.run(
        ["git", "-C", str(Path(__file__).parent), "rev-parse", "--short=12", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", str(Path(__file__).parent), "status", "--porcelain", "--", "crates"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    return {
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "platform": f"{platform.system()} {platform.machine()}",
        "os": platform.platform(),
        "python": platform.python_version(),
        "larmorx_commit": commit + ("-dirty" if dirty else ""),
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--larmorx", required=True)
    ap.add_argument("--replica", default=None)
    ap.add_argument("--threads", default="1")
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--tool", default="bet2")
    ap.add_argument("--args", default="{0} out -m")
    ap.add_argument("--json", default=None)
    ap.add_argument("images", nargs="+")
    a = ap.parse_args(argv)
    threads = [int(t) for t in a.threads.split(",")]
    report = {"environment": environment(), "repeats": a.repeat, "results": []}
    if a.json and Path(a.json).is_file():
        report = json.loads(Path(a.json).read_text(encoding="utf-8"))
        report["environment"] = environment()
    report["load"] = " ".join(f"{x:.2f}" for x in os.getloadavg())
    head = ["image", "FSL"]
    if a.replica:
        head += [f"replica ({t} thr)" for t in threads]
    head += [f"original ({t} thr)" for t in threads]
    print("| " + " | ".join(head) + " |")
    print("|" + "---|" * len(head))

    def add(tool: str, name: str, t: int, times: list[float]) -> str:
        report["results"].append(
            {
                "tool": tool,
                "operation": f"{a.tool}, {t} thread" + ("s" if t > 1 else ""),
                "input": name,
                "median": statistics.median(times),
                "best": min(times),
                "seconds": times,
                "extra": {"threads": t, "call": a.args},
            }
        )
        return f"{statistics.median(times):.2f}"

    for item in a.images:
        name, files = item.split("=", 1)
        paths = files.split(",")
        args = [w.format(*paths) for w in a.args.split()]
        env = fslrun.environment()
        row = [name, add("FSL 6.0.7.23", name, 1, timed([str(fslrun.FSLDIR / "bin" / a.tool), *args], env, a.repeat))]  # fmt: skip
        if a.replica:
            for t in threads:
                e = {**env, "OMP_NUM_THREADS": str(t)}
                row.append(add("replica (larmorx-nc)", name, t, timed([a.replica, "fsl", a.tool, *args], e, a.repeat)))  # fmt: skip
        for t in threads:
            e = {**env, "OMP_NUM_THREADS": str(t), "LARMORX_IMPLEMENTATION": "original"}
            row.append(add("original (larmorx)", name, t, timed([a.larmorx, "fsl", a.tool, *args], e, a.repeat)))  # fmt: skip
        print("| " + " | ".join(row) + " |", flush=True)
    if a.json:
        Path(a.json).write_text(json.dumps(report, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

# SPDX-License-Identifier: Apache-2.0
"""Speed of FLIRT's clean-room original against FSL 6.0.7.23's `flirt` and the replica
(`larmorx-nc`): wall time of the whole command, one run each (informal), on the fsl-flirt suite's
standard tier's real cases: fMRIPrep's BBR call on the tier's three oracle runs (ds005040,
ds003592, ds000228), the default 12-dof registration and a 6-dof one of ds005040's BOLD
reference to its T1w (the suite's images), BBR with a field map and its mask (the suite's
`bbr/fieldmap-mask`), and `-applyxfm` of that pair. FSL and the replica on
one thread (FSL is single-threaded), the original on each thread count of `--threads`.

    PYTHONPATH=python:validation/src python -P bench.py --larmorx <larmorx> \\
        --replica <larmorx-nc> [--threads 1,14] [--json report.json]

prints a Markdown table (seconds) and, with --json, writes the runs in the form of
docs/benchmarks/*.json. Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep


def timed(argv: list[str], run: rep.Run | None, fp_args: list[str] | None, env: dict) -> float:
    with tempfile.TemporaryDirectory(prefix="lx-bench-") as d_:
        d = Path(d_)
        args = fp_args if fp_args is not None else rep._prepare(run, d)
        t = time.perf_counter()
        p = subprocess.run(
            [*argv, *args], cwd=d, env=env, capture_output=True, text=True, check=False
        )
        dt = time.perf_counter() - t
        if p.returncode != 0:
            raise RuntimeError(f"{argv[0]}: exit {p.returncode}: {p.stderr[-400:]}")
        return dt


def environment() -> dict:
    cpu = "?"
    for line in Path("/proc/cpuinfo").read_text().splitlines():
        if line.startswith("model name"):
            cpu = line.split(":", 1)[1].strip()
            break
    here = Path(__file__).parent
    commit = subprocess.run(
        ["git", "-C", str(here), "rev-parse", "--short=12", "HEAD"],
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
        "larmorx_commit": commit,
        "load_average_before": os.getloadavg()[0],
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--larmorx", required=True)
    ap.add_argument("--replica", required=True)
    ap.add_argument("--threads", default="1,14")
    ap.add_argument("--json", default=None)
    o = ap.parse_args(argv)
    threads = [int(t) for t in o.threads.split(",")]
    base = ("-in", "@epi.nii", "-ref", "@t1.nii")
    cases: list[tuple[str, rep.Run | None, list[str] | None]] = []
    calls = {r: n for r, n in reversed(rep.fmriprep_calls())}
    for r in rep.STANDARD_RUNS:
        if r in calls:
            cases.append(
                (f"fMRIPrep's BBR call, {r.split('-sub')[0]}", None, rep.fmriprep_args(r, calls[r]))
            )
    cases.append(
        (
            "default 12 dof (BOLD reference to T1w)",
            rep.Run("flirt", (*base, "-omat", "o.mat")),
            None,
        )
    )
    cases.append(("6 dof", rep.Run("flirt", (*base, "-dof", "6", "-omat", "o.mat")), None))
    fm = rep._all_cases().get("bbr/fieldmap-mask")
    if fm is not None:
        cases.append(("BBR with a field map and its mask (-pedir -2)", fm[2], None))
    cases.append(
        (
            "-applyxfm (trilinear)",
            rep.Run("flirt", (*base, "-applyxfm", "-init", "@init.mat", "-out", "o")),
            None,
        )
    )
    env = rep._environment({})
    report = {"environment": environment(), "repeats": 1, "results": []}
    print("| Case | FSL | replica (1) | " + " | ".join(f"original ({t})" for t in threads) + " |")
    print("|---|---|---|" + "---|" * len(threads))
    for name, run, fp in cases:
        row = []

        def add(
            tool: str, t: int, argv: list[str], extra_env: dict, name=name, run=run, fp=fp
        ) -> float:
            s = timed(argv, run, fp, {**env, **extra_env, "OMP_NUM_THREADS": str(t)})
            report["results"].append(
                {
                    "tool": tool,
                    "operation": f"{t} thread" + ("s" if t > 1 else ""),
                    "input": name,
                    "seconds": [s],
                    "median": s,
                }
            )
            return s

        fsl = add("FSL 6.0.7.23", 1, [str(rep.fsldir() / "bin" / "flirt")], {})
        row.append(fsl)
        row.append(add("replica (larmorx-nc)", 1, [o.replica, "fsl", "flirt"], {}))
        for t in threads:
            row.append(
                add(
                    "original (larmorx)",
                    t,
                    [o.larmorx, "fsl", "flirt"],
                    {"LARMORX_IMPLEMENTATION": "original"},
                )
            )
        cells = [f"{row[0]:.2f} s"] + [f"{x:.2f} s ({fsl / x:.1f}×)" for x in row[1:]]
        print(f"| {name} | " + " | ".join(cells) + " |", flush=True)
    if o.json:
        Path(o.json).write_text(json.dumps(report, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

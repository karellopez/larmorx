# SPDX-License-Identifier: Apache-2.0
"""An informal speed table of FLIRT's clean-room original against FSL 6.0.7.23 and the replica
(`larmorx-nc`): wall time of one run each, on the fsl-flirt suite's images (a default 12-dof
registration of the 3 mm BOLD reference to the T1w at half resolution, a 6-dof one) and on
fMRIPrep's boundary-based call of ds005040 (its full-size T1w). The original at 1 and 8 threads,
the replica and FSL at one (FSL is single-threaded). Not a benchmark report (part 2 writes one):
one run per program, on a machine that may be busy. Development tool of part 1 (2026-11-02).

    PYTHONPATH=python:validation/src LARMORX_NC_BIN=<larmorx-nc> \\
        python -P validation/investigations/cleanroom-flirt/speed.py

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import time
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep


def timed(argv: list[str], cwd: Path, env: dict[str, str]) -> float:
    t = time.perf_counter()
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, check=False)
    dt = time.perf_counter() - t
    if p.returncode != 0:
        print("  failed:", " ".join(argv[:3]), p.stderr[-300:])
    return dt


def main() -> None:
    nc = os.environ.get("LARMORX_NC_BIN")
    lx = os.environ.get("LARMORX_FLIRT_BIN")
    cases = {
        "default 12 dof": rep.Run(
            "flirt", ("-in", "@epi.nii", "-ref", "@t1.nii", "-omat", "o.mat")
        ),
        "6 dof with -out": rep.Run(
            "flirt",
            ("-in", "@epi.nii", "-ref", "@t1.nii", "-dof", "6", "-out", "o", "-omat", "o.mat"),
        ),
    }
    runs = rep.fmriprep_calls()
    fp = next((r for r in runs if r[0] == "ds005040-sub-002-noreconall"), None)
    rows = []
    for name, run in cases.items():
        with tempfile.TemporaryDirectory() as d_:
            d = Path(d_)
            args = rep._prepare(run, d)
            rows.append((name, args, d))
            env = rep._environment({})
            fsl = timed([str(rep.fsldir() / "bin" / "flirt"), *args], d, env)
            o1 = timed(
                [lx, "fsl", "flirt", *args],
                d,
                {**env, "OMP_NUM_THREADS": "1", "LARMORX_IMPLEMENTATION": "original"},
            )
            o8 = timed(
                [lx, "fsl", "flirt", *args],
                d,
                {**env, "OMP_NUM_THREADS": "8", "LARMORX_IMPLEMENTATION": "original"},
            )
            r1 = (
                timed([nc, "fsl", "flirt", *args], d, {**env, "OMP_NUM_THREADS": "1"})
                if nc
                else float("nan")
            )
            print(f"| {name} | {fsl:.2f} | {r1:.2f} | {o1:.2f} | {o8:.2f} |")
    if fp is not None:
        args = rep.fmriprep_args(*fp)
        with tempfile.TemporaryDirectory() as d_:
            d = Path(d_)
            env = rep._environment({})
            fsl = timed([str(rep.fsldir() / "bin" / "flirt"), *args], d, env)
            o1 = timed(
                [lx, "fsl", "flirt", *args],
                d,
                {**env, "OMP_NUM_THREADS": "1", "LARMORX_IMPLEMENTATION": "original"},
            )
            o8 = timed(
                [lx, "fsl", "flirt", *args],
                d,
                {**env, "OMP_NUM_THREADS": "8", "LARMORX_IMPLEMENTATION": "original"},
            )
            r1 = (
                timed([nc, "fsl", "flirt", *args], d, {**env, "OMP_NUM_THREADS": "1"})
                if nc
                else float("nan")
            )
            print(f"| fMRIPrep's BBR (ds005040) | {fsl:.2f} | {r1:.2f} | {o1:.2f} | {o8:.2f} |")


if __name__ == "__main__":
    main()

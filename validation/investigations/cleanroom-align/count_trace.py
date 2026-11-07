# SPDX-License-Identifier: Apache-2.0
"""The weighted correlation's count per point (`specs/mcflirt.md` §6.4, the spec writer's answer of
2026-11-06), evaluation by evaluation: FSL 6.0.7.23's mcflirt with `-verbose 20` (every evaluated
matrix; through the oracle cache) against two traced builds of larmorx's clean-room original
(`trace.patch` of this folder applied to a local build: one with the count's running total taking
each row's total, one taking each point's weight), on recorded runs of `mri-hmc`'s record, in
`reproduce` mode. For each run: the evaluations before the first matrix that differs from
mcflirt's, and how many of all are identical. Black box: only the programs' outputs are read.

    python -P count_trace.py <work> <label>=<traced larmorx> ... [--case <recorded run>]...
        (PYTHONPATH=<checkout>/validation/src)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

from larmorx_validation import oracle_cache

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
RUNS = WORKSPACE / "oracles" / "fsl-6.0.7.23" / "mcflirt-haswell" / "runs"
CASES = (
    "syn/geometry/single-volume/reffile",
    "syn/geometry/single-volume/default",
    "syn/orientation/ras-unflipped/reffile",
    "syn/orientation/ras-unflipped/default",
    "syn/content/ramp/reffile",
    "syn/motion/small/default",
    "syn/motion/small/reffile",
    "syn/motion/small/meanvol",
    "syn/motion/small/dof12",
)
MATRIX = re.compile(r"Cost::affmat = \s*\n((?:[^\n]*\n){4})")


def matrices(text: str) -> list[str]:
    return [" ".join(m.group(1).split()) for m in MATRIX.finditer(text)]


def argv_of(case: str) -> list[str]:
    run = json.loads((RUNS / case / "run.json").read_text(encoding="utf-8"))
    a = list(run["steps"][0]["argv"][1:])
    k = a.index("-out")
    a[k + 1] = "out_mcf.nii.gz"
    return a


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("builds", nargs="+")
    ap.add_argument("--case", action="append")
    a = ap.parse_args()
    builds = [b.split("=", 1) for b in a.builds]
    fsl = Path(os.environ.get("FSL_623", Path.home() / "fsl-6.0.7.23"))
    env = {
        "HOME": str(Path.home()),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsl),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "OPENBLAS_CORETYPE": "Haswell",
    }
    for case in a.case or CASES:
        args = argv_of(case)
        d = a.work / case.replace("/", "_")
        (d / "fsl").mkdir(parents=True, exist_ok=True)
        r = oracle_cache.run(
            [str(fsl / "bin" / "mcflirt"), *args, "-verbose", "20"],
            env=env,
            cwd=d / "fsl",
            outputs=[d / "fsl"],
            policy=oracle_cache.Policy(threads=1, family="fsl"),
            runner="count-trace",
            timeout=3600,
        )
        want = matrices(r.stdout + r.stderr)
        line = [f"{case}: mcflirt {len(want)} evaluations"]
        for label, prog in builds:
            w = d / label
            w.mkdir(exist_ok=True)
            trace = w / "trace.txt"
            trace.unlink(missing_ok=True)
            e = {
                **env,
                "LARMORX_HMC_TRACE": str(trace),
                "LARMORX_UPSTREAM_DEFECTS": "reproduce",
                "LARMORX_IMPLEMENTATION": "original",
                "OMP_NUM_THREADS": "1",
            }
            subprocess.run(
                [prog, "mri", "hmc", *args], cwd=w, env=e, capture_output=True, check=False
            )
            got = matrices(trace.read_text(encoding="utf-8")) if trace.exists() else []
            first = next(
                (i for i, (x, y) in enumerate(zip(want, got, strict=False)) if x != y),
                min(len(want), len(got)),
            )
            same = sum(x == y for x, y in zip(want, got, strict=False))
            line.append(
                f"{label}: {len(got)} evaluations, first {first} identical, {same} identical"
            )
        print("; ".join(line))
    return 0


if __name__ == "__main__":
    sys.exit(main())

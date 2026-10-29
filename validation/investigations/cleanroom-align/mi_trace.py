# SPDX-License-Identifier: Apache-2.0
"""The search path of the mutual-information costs, evaluation by evaluation: FSL 6.0.7.23's
mcflirt prints every matrix it evaluates with `-verbose 20` (`specs/mcflirt.md` §11); larmorx's
clean-room original writes the same into the file `$LARMORX_HMC_TRACE` with the trace patch of
this folder applied (`trace.patch`, a local build only: `git apply trace.patch`, `cargo build
--release -p larmorx-cli`, the binary copied aside, the patch reverted; a file, not standard error,
which the command line holds locked while its pool works). The two traces are compared matrix by
matrix: how many evaluations agree to the printed digits before the first difference, per
volume (with `-fudge -stages 1` every volume starts from the identity, so each volume's path is
independent of the others). Black-box: only the programs' outputs are read.

    python -P mi_trace.py <work> --lx <patched larmorx> [--cost normmi] [--case <finding.json>]...
           [--series <series.nii.gz>] [--extra "-bins 128"] [--env NAME=VALUE]...

Without `--case` or `--series`, the synthetic series of the recorded runs is used
(`larmorx-testdata` `synthetic/hmc/motion/small.nii.gz`).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
FSL = Path(os.environ.get("FSL_623", Path.home() / "fsl-6.0.7.23"))
SERIES = WORKSPACE / "larmorx-testdata" / "data" / "synthetic" / "hmc" / "motion" / "small.nii.gz"

MATRIX = re.compile(r"Cost::affmat = \s*\n((?:[^\n]*\n){4})")


def matrices(text: str) -> list[str]:
    """The evaluated matrices of a trace, each as its four printed lines."""
    return [" ".join(m.group(1).split()) for m in MATRIX.finditer(text)]


def run(argv: list[str], cwd: Path, extra: dict[str, str]) -> str:
    env = {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI_GZ", **extra}
    cwd.mkdir(parents=True, exist_ok=True)
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=3600)
    return p.stdout + p.stderr


def compare(
    series: Path, work: Path, lx: str, cost: str, extra: list[str], label: str, env: dict
) -> None:
    args = ["-in", str(series), "-out", "out", "-cost", cost, "-fudge", "-stages", "1", *extra]
    a = run(
        [str(FSL / "bin" / "mcflirt"), *args, "-verbose", "20"],
        work / label / "mcflirt",
        {"FSLDIR": str(FSL)},
    )
    trace = work / label / "lx-trace.txt"
    trace.unlink(missing_ok=True)
    run(
        [lx, "mri", "hmc", *args],
        work / label / "lx",
        {
            "LARMORX_UPSTREAM_DEFECTS": "reproduce",
            "OMP_NUM_THREADS": "1",
            "LARMORX_HMC_TRACE": str(trace),
            **env,
        },
    )
    b = trace.read_text() if trace.exists() else ""
    ma, mb = matrices(a), matrices(b)
    same = 0
    while same < min(len(ma), len(mb)) and ma[same] == mb[same]:
        same += 1
    print(
        f"{label}: mcflirt {len(ma)} evaluations, larmorx {len(mb)}; the first {same} identical"
        + (f"; first difference:\n  mcflirt {ma[same]}\n  larmorx {mb[same]}" if same < min(len(ma), len(mb)) else "")
    )


def main(argv: list[str]) -> int:
    work = Path(argv.pop(0)).resolve()
    keys = ("--lx", "--cost", "--case", "--series", "--extra", "--env")
    opts: dict[str, list[str]] = {k: [] for k in keys}
    while argv:
        k = argv.pop(0)
        opts[k].append(argv.pop(0))
    cost = (opts["--cost"] or ["normmi"])[0]
    extra = (opts["--extra"] or [""])[0].split()
    lx = opts["--lx"][0]
    series = [Path(s) for s in opts["--series"]]
    if opts["--case"]:
        from larmorx_validation.difftest import inputs

        for f in opts["--case"]:
            finding = json.loads(Path(f).read_text(encoding="utf-8"))
            recipe = finding.get("shrunk") or finding["recipe"]
            spec = recipe["inputs"]["in"]
            q = work / "inputs" / (Path(f).stem + inputs.suffix_of(spec))
            q.parent.mkdir(parents=True, exist_ok=True)
            inputs.materialize(spec, q)
            series.append(q)
    env = dict(e.split("=", 1) for e in opts["--env"])
    for s in series or [SERIES]:
        compare(s, work, lx, cost, extra, s.name.split(".")[0], env)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

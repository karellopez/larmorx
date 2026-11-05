# SPDX-License-Identifier: Apache-2.0
"""centre_moves.py <work> <case> [<case> ...] --build NAME=PYTHONPATH [--build ...]: how far a
change of the head-motion correction moves its outputs below the record's printed precision.

For each recorded mcflirt run of the `mri-hmc` suite (``oracles/fsl-6.0.7.23/mcflirt-haswell/runs/
<case>/run.json``: its command line, with ``-out`` in ``<work>`` and ``-plots`` added), ``larmorx
mri hmc`` is run once per build (each a ``python/`` directory holding a built ``larmorx._core``,
in its own process, ``reproduce`` as the suite compares, 4 threads), and the ``.mat`` files are
compared as text with mcflirt's and between the builds (how many differ, the largest element
difference), and the ``.par`` lines between the builds. The clean-room catch-up, relay 7
(2026-11-05): main's build, the blocked centres alone, and the centres with the correctly rounded
``atan2``."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

RUNS = Path.home() / "VScode_projects/super_fmriprep/oracles/fsl-6.0.7.23/mcflirt-haswell/runs"


def run_build(name: str, pythonpath: str, argv: list[str], env: dict[str, str], out: Path) -> int:
    code = "import sys; from larmorx.cli import run; sys.exit(run(['larmorx', 'mri', 'hmc', *sys.argv[1:]]))"
    e = dict(
        os.environ,
        **env,
        PYTHONPATH=pythonpath,
        OMP_NUM_THREADS="4",
        LARMORX_UPSTREAM_DEFECTS="reproduce",
    )
    p = subprocess.run(
        [sys.executable, "-c", code, *argv],
        env=e,
        cwd=out,
        capture_output=True,
        text=True,
        check=False,
    )
    (out / "stderr.txt").write_text(p.stderr, encoding="utf-8")
    return p.returncode


def mats(d: Path) -> dict[str, list[float]]:
    out = {}
    if d.is_dir():
        for f in sorted(d.glob("MAT_*")):
            out[f.name] = [float(x) for x in f.read_text(encoding="utf-8").split()]
    return out


def texts(d: Path) -> dict[str, str]:
    return (
        {f.name: f.read_text(encoding="utf-8") for f in sorted(d.glob("MAT_*"))}
        if d.is_dir()
        else {}
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("work")
    ap.add_argument("cases", nargs="+")
    ap.add_argument("--build", action="append", default=[])
    a = ap.parse_args(argv)
    builds = [b.split("=", 1) for b in a.build]
    work = Path(a.work)
    for case in a.cases:
        run = json.loads((RUNS / case / "run.json").read_text(encoding="utf-8"))
        step = run["steps"][0]
        args = list(step["argv"][1:])
        k = args.index("-out")
        mcflirt_out = Path(args[k + 1])
        if "-plots" not in args:
            args.append("-plots")
        env = {"FSLOUTPUTTYPE": run["env"].get("FSLOUTPUTTYPE", "NIFTI_GZ")}
        row: dict = {"case": case}
        fsl_text = texts(Path(str(mcflirt_out) + ".mat"))
        results = {}
        for name, pp in builds:
            d = work / case.replace("/", "_") / name
            d.mkdir(parents=True, exist_ok=True)
            here = list(args)
            here[k + 1] = str(d / mcflirt_out.name)
            code = run_build(name, pp, here, env, d)
            mat_dir = d / (mcflirt_out.name + ".mat")
            t = texts(mat_dir)
            par = (
                (d / (mcflirt_out.name + ".par")).read_text(encoding="utf-8").splitlines()
                if (d / (mcflirt_out.name + ".par")).exists()
                else []
            )
            results[name] = (mats(mat_dir), t, par)
            row[name] = {
                "exit": code,
                "matrices identical to mcflirt's as text": f"{sum(1 for m in t if fsl_text.get(m) == t[m])}/{len(t)}",
            }
        names = [n for n, _ in builds]
        for i in range(1, len(names)):
            a0, a1 = results[names[i - 1]], results[names[i]]
            diff_text = sum(1 for m in a1[1] if a0[1].get(m) != a1[1][m])
            max_el = max(
                (
                    abs(x - y)
                    for m in a1[0]
                    for x, y in zip(a0[0].get(m, []), a1[0][m], strict=False)
                ),
                default=0.0,
            )
            par_lines = sum(1 for x, y in zip(a0[2], a1[2], strict=False) if x != y)
            par_max = max(
                (
                    abs(float(u) - float(v))
                    for x, y in zip(a0[2], a1[2], strict=False)
                    for u, v in zip(x.split(), y.split(), strict=False)
                ),
                default=0.0,
            )
            row[f"{names[i - 1]} -> {names[i]}"] = {
                "matrices differing as text": f"{diff_text}/{len(a1[1])}",
                "largest element difference": max_el,
                ".par lines differing": f"{par_lines}/{len(a1[2])}",
                "largest .par difference": par_max,
            }
        print(json.dumps(row), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

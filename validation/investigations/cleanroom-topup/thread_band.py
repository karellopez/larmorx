# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL's own band on random-testing findings: FSL at `--nthr=1` and at `--nthr=4` (both through
the oracle cache, `use` mode) and the original (reproduce mode, one thread) on each finding's
drawn case, with `-v`; prints the three final costs and the p99 of |d| inside FSL's corrected
image's brain (`--iout`, relative to FSL's) between FSL at 4 threads and FSL, and between the
original and FSL. Usage:
  thread_band.py <trial dir> <out dir> <finding number>...
"""

import dataclasses
import json
import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as fo
from larmorx_validation.parity import fsl_topup_replica as rep


def last_cost(stdout):
    c = [float(ln.split("Cost = ")[1]) for ln in stdout.splitlines() if "Cost = " in ln]
    return c[-1] if c else float("nan")


def p99(x, y, m):
    d = np.abs(x - y)[m] / max(np.abs(x[m]).mean(), 1e-30)
    return float(np.percentile(d, 99)) if d.size else float("nan")


def main():
    trial, out = sys.argv[1], Path(sys.argv[2])
    for n in map(int, sys.argv[3:]):
        j = json.loads(
            Path(
                f"{trial}/findings/fsl-topup-original/topup-trial-{n:05d}-original.json"
            ).read_text()
        )
        run, _ = D.TOOL.case(j["recipe"], "original").payload[1]
        base = [
            a
            for a in run.args
            if not a.startswith(("--nthr", "-v", "--verbose", "--debug", "--iout"))
        ]
        res = {}
        for who, extra in (("fsl1", ["--nthr=1"]), ("fsl4", ["--nthr=4"]), ("lx", ["--nthr=1"])):
            d = out / str(n) / who
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
            r2 = dataclasses.replace(
                run,
                args=tuple(base + extra + ["--iout=iout_band", "-v"]),
                threads=4 if who == "fsl4" else 1,
            )
            args = rep._prepare(r2, d, None)
            if who.startswith("fsl"):
                p = rep.run_oracle(args, cwd=d, settings=dict(run.settings))
            else:
                p = fo.run_original(
                    args, cwd=d, settings=dict(run.settings), mode=defects.REPRODUCE, threads=1
                )
            f = next(d.glob("iout_band.nii*"), None)
            res[who] = (last_cost(p.stdout), np.asarray(nib.load(f).dataobj, float) if f else None)
        a = res["fsl1"][1]
        line = f"{n}: final cost FSL {res['fsl1'][0]:.6g}, FSL --nthr=4 {res['fsl4'][0]:.6g}, original {res['lx'][0]:.6g}"
        if a is not None and res["fsl4"][1] is not None and res["lx"][1] is not None:
            m = a > 0.1 * a.max()
            line += f"; --iout p99 |d| / mean: FSL 4 threads {p99(a, res['fsl4'][1], m):.3g}, original {p99(a, res['lx'][1], m):.3g}"
        print(line, flush=True)


main()

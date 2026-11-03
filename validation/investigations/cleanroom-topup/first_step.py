# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The first Levenberg-Marquardt step of one random-testing finding: FSL (oracle cache, use mode)
and the original, on the finding's images and on subsets of its volumes, with `--miter=1
--debug=1`; compares the first gradient (`TopupDebugGradient_level01_iter01_attempt00.txt`) and
the first two costs. Usage:
  first_step.py <trial dir> <finding number> <out dir> [volume subsets, e.g. 0,1 2,3 0,1,2,3]
FIRST_STEP_ACQ="r1;r2;..." replaces the acquisition rows (e.g. oblique phase encodings);
FIRST_STEP_MODE=fix runs the original in its default mode (else reproduce);
FIRST_STEP_MITER=own keeps the case's own --miter (every iteration's cost compared);
LX_PROBE_* variables are passed to the original (a temporary probe build's switches).
FIRST_STEP_DEBUG=3 asks for the Hessian's weight images too; every debug image both programs
wrote is then compared (max |FSL - original| over max |FSL|).
"""

import json
import os
import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as fo
from larmorx_validation.parity import fsl_topup_replica as rep

GRAD = "TopupDebugGradient_level01_iter01_attempt00.txt"


def costs(stdout):
    return [float(ln.split("Cost = ")[1]) for ln in stdout.splitlines() if "Cost = " in ln]


def main():
    trial, n, out = sys.argv[1], int(sys.argv[2]), Path(sys.argv[3])
    subsets = [tuple(map(int, s.split(","))) for s in sys.argv[4:]] or [None]
    j = json.loads(
        Path(f"{trial}/findings/fsl-topup-original/topup-trial-{n:05d}-original.json").read_text()
    )
    run, _ = D.TOOL.case(j["recipe"], "original").payload[1]
    src = out / "src"
    shutil.rmtree(src, ignore_errors=True)
    src.mkdir(parents=True)
    args = rep._prepare(run, src, None)
    own = os.environ.get("FIRST_STEP_MITER") == "own"
    keep = {"--debug", "-v", "--verbose"} | (set() if own else {"--miter"})
    args = [a for a in args if a.split("=")[0] not in keep] + ([] if own else ["--miter=1"])
    args += [f"--debug={os.environ.get('FIRST_STEP_DEBUG', '1')}", "-v"]
    imain = next(a.split("=", 1)[1] for a in args if a.startswith("--imain="))
    acqf = next(a.split("=", 1)[1] for a in args if a.startswith(("--datain=", "--acqp=")))
    img = nib.load(next(src.glob(imain + "*")))
    acq = [ln for ln in (src / acqf).read_text().splitlines() if ln.strip()]
    print("args:", " ".join(args))
    print("shape", img.shape, "voxel", img.header.get_zooms()[:3], "acq:", " | ".join(acq))
    for sub in subsets:
        name = "all" if sub is None else "v" + "".join(map(str, sub))
        g = {}
        c = {}
        for who in ("fsl", "lx"):
            d = out / name / who
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
            data = np.asarray(img.dataobj)
            rows = acq
            if sub is not None:
                data = data[..., list(sub)]
                rows = [acq[i] for i in sub]
            if os.environ.get("FIRST_STEP_ACQ"):
                rows = os.environ["FIRST_STEP_ACQ"].split(";")
            nib.save(nib.Nifti1Image(data, img.affine, img.header), d / "pair.nii")
            (d / "acq.txt").write_text("\n".join(rows) + "\n")
            a = [x for x in args if not x.startswith(("--imain=", "--datain=", "--acqp="))]
            a = ["--imain=pair.nii", "--datain=acq.txt"] + a
            if who == "fsl":
                p = rep.run_oracle(a, cwd=d, settings=dict(run.settings))
            else:
                extra = {k: v for k, v in os.environ.items() if k.startswith("LX_PROBE")}
                p = fo.run_original(
                    a,
                    cwd=d,
                    settings={**dict(run.settings), **extra},
                    mode=defects.FIX
                    if os.environ.get("FIRST_STEP_MODE") == "fix"
                    else defects.REPRODUCE,
                    threads=1,
                )
            c[who] = costs(p.stdout)
            f = d / GRAD
            g[who] = np.loadtxt(f) if f.exists() else None
        a, b = g["fsl"], g["lx"]
        line = f"{name}: costs FSL {c['fsl']}, original {c['lx']}"
        if a is not None and b is not None and a.shape == b.shape:
            line += (
                f"; gradient max|FSL| {np.abs(a).max():.4g}, max|d| {np.abs(a - b).max():.4g},"
                f" FSL/original least-squares scale {(a @ b) / (b @ b):.6f}"
            )
        print(line)
        if os.environ.get("FIRST_STEP_DEBUG", "1") != "1":
            for f in sorted((out / name / "fsl").glob("TopupDebug*.nii*")):
                q = out / name / "lx" / f.name
                if q.exists():
                    x = np.asarray(nib.load(f).dataobj, float)
                    y = np.asarray(nib.load(q).dataobj, float)
                    if x.shape == y.shape:
                        r = np.abs(x - y).max() / max(np.abs(x).max(), 1e-30)
                        print(f"  {f.name}: {x.shape}, relative max difference {r:.3g}")
                    else:
                        print(f"  {f.name}: shapes {x.shape} {y.shape}")
                else:
                    print(f"  {f.name}: only FSL's")


main()

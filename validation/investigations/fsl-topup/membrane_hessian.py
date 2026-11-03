# SPDX-License-Identifier: Apache-2.0
"""FSL-117 (rule D, measured): topup's Levenberg-Marquardt steps with the membrane energy's
Hessian in mm (FSL's, ``reproduce``) against the Hessian in the cost's own units (voxels,
larmorx's correction, ``fix``), on the same cost: the final cost each reaches.

Runs the replica only (``larmorx-nc fsl topup``; in ``reproduce`` mode it is FSL 6.0.7.23's
``topup`` bit for bit: the suite's ``opt/membrane-lm`` case), on the suite's derived inputs
(``oracles/inputs/fsl-topup-replica/``: the half-resolution pair, 6 x 6 x 6.6 mm, and the
full-resolution pair, 3 x 3 x 3.3 mm), each in its own directory. ``-v`` prints the cost of
every iteration; the last line is the cost of the result.

    python -P membrane_hessian.py <larmorx-nc> <inputs dir> <work dir>

Run it inside a memory box (``scripts/orchestration/boxed.sh 8``).
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

CASES = {
    "half-1level": ("half.nii.gz", ["--warpres=20", "--miter=5", "--lambda=0.005", "--estmov=0"]),
    "half-movements": ("half.nii.gz", ["--warpres=20", "--miter=5", "--lambda=0.005"]),
    "half-strong": ("half.nii.gz", ["--warpres=14", "--miter=5", "--lambda=0.5", "--estmov=0"]),
    "half-levels": (
        "half.nii.gz",
        [
            "--warpres=20,14,10",
            "--subsamp=2,1,1",
            "--fwhm=8,4,2",
            "--miter=5,5,5",
            "--lambda=0.005,0.001,0.0001",
            "--estmov=1,1,0",
        ],
    ),
    "pair-1level": (
        "pair.nii.gz",
        ["--warpres=14", "--subsamp=2", "--fwhm=6", "--miter=5", "--lambda=0.005", "--estmov=0"],
    ),
}
_COST = re.compile(r"Cost = (\S+)")


def run(exe: str, inputs: Path, work: Path, name: str, mode: str) -> tuple[int, float, int]:
    image, args = CASES[name]
    d = work / f"{name}-{mode}"
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    shutil.copy(inputs / image, d / image)
    shutil.copy(inputs / "acq.txt", d / "acq.txt")
    env = dict(os.environ, FSLOUTPUTTYPE="NIFTI_GZ", LARMORX_UPSTREAM_DEFECTS=mode)
    p = subprocess.run(
        [
            exe,
            "fsl",
            "topup",
            f"--imain={image}",
            "--datain=acq.txt",
            "--out=o",
            "--regmod=membrane_energy",
            "-v",
            *args,
        ],
        cwd=d,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    costs = [float(c) for c in _COST.findall(p.stdout)]
    return p.returncode, costs[-1] if costs else float("nan"), len(costs)


def main(argv: list[str]) -> int:
    exe, inputs, work = argv[1], Path(argv[2]), Path(argv[3])
    print("case | FSL's Hessian (mm): final cost | in the cost's units: final cost | change")
    for name in CASES:
        c1, r, n1 = run(exe, inputs, work, name, "reproduce")
        c2, f, n2 = run(exe, inputs, work, name, "fix")
        print(
            f"{name} | {r:.6g} ({n1} evaluations, exit {c1}) | {f:.6g} ({n2}, exit {c2}) "
            f"| {100 * (f - r) / r:+.2f} %"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

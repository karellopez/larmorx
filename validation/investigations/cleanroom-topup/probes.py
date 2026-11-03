# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL probe runs (through the oracle cache, use mode) named in PROBES; usage: probes.py <name>..."""

import os
import sys
from pathlib import Path

from larmorx_validation.parity import fsl_topup_replica as T

RUNS = Path(os.environ["TOPUP_RUNS"])
HALF = [
    "--imain=@half.nii.gz",
    "--datain=@acq.txt",
    "--out=pr",
    "--scale=1",
    "--debug=1",
    "-v",
    "--estmov=0",
]
PROBES = {
    "A": HALF
    + [
        "--warpres=20,20",
        "--subsamp=2,2",
        "--fwhm=8,4",
        "--miter=3,1",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
    ],
    "B": HALF
    + [
        "--warpres=20,12",
        "--subsamp=2,2",
        "--fwhm=8,4",
        "--miter=3,1",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
    ],
    "C": HALF
    + [
        "--warpres=20,24",
        "--subsamp=2,1",
        "--fwhm=8,4",
        "--miter=3,1",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
    ],
    "E": HALF
    + [
        "--warpres=20,22",
        "--subsamp=2,2",
        "--fwhm=8,8",
        "--miter=3,1",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
    ],
    "D": HALF
    + [
        "--warpres=20,14",
        "--subsamp=2,1",
        "--fwhm=8,4",
        "--miter=3,0",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
        "--fout=pr_field",
    ],
    "F": HALF
    + [
        "--warpres=20,14",
        "--subsamp=1,1",
        "--fwhm=8,8",
        "--miter=3,0",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
        "--fout=pr_field",
    ],
    "G": HALF
    + [
        "--warpres=18,20",
        "--subsamp=1,1",
        "--fwhm=8,8",
        "--miter=3,0",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
        "--fout=pr_field",
    ],
    "H": HALF
    + [
        "--warpres=20,20",
        "--subsamp=2,1",
        "--fwhm=8,8",
        "--miter=3,0",
        "--lambda=0.005,0.0001",
        "--minmet=0,0",
        "--fout=pr_field",
    ],
}


def main():
    for name in sys.argv[1:]:
        out = RUNS / f"fsl-probe-{name}"
        out.mkdir(parents=True, exist_ok=True)
        run = T.Run(tuple(PROBES[name]))
        args = T._prepare(run, out, T._source_node())
        p = T._oracle(run, args, out, {})
        (out / "ARGS.txt").write_text(" ".join(args) + "\n")
        (out / "STDOUT.txt").write_text(p.stdout)
        (out / "STDERR.txt").write_text(p.stderr)
        print(name, "exit", p.code)


main()

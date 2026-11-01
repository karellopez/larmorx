# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of makerot."""

import os
from pathlib import Path

#: the parity suite's derived inputs (``oracles/inputs/fsl-flirt-replica`` in the workspace)
INP = Path(
    os.environ.get(
        "LARMORX_FLIRT_INPUTS",
        Path(__file__).resolve().parents[4] / "oracles/inputs/fsl-flirt-replica",
    )
)
FSL = Path(os.environ.get("LARMORX_FSL_623", Path.home() / "fsl-6.0.7.23"))

PROGRAM = "makerot"
FILES = {"t1.nii": INP / "t1.nii", "epi.nii": INP / "epi.nii", "coords.txt": "1 2 3\n"}
CASES = [
    [],
    ["-h"],
    ["--help"],
    ["-t", "30"],
    ["-t", "30", "-a", "1,1,0", "-c", "10,20,30"],
    ["--theta=30", "--axis=0,0,4"],
    ["-t", "30", "-a", "0,0,4"],
    ["-t", "-45.5", "-a", "1,2,3"],
    ["-t", "0", "-a", "1,2,3"],
    ["-t", "180", "-a", "0,1,0"],
    ["-t", "1e-7", "-a", "1,0,0"],
    ["-a", "1,2,3"],
    ["-t", "10", "-a", "0,0,0"],
    ["-t", "30", "-a", "5"],
    ["-t", "30", "-a", "1,2"],
    ["-t", "30", "-a", "1,2,3", "-a", "7"],
    ["-t", "30", "-a", "1,2,3,4"],
    ["-t", "30", "-a", "1:2:3"],
    ["-t", "30", "-a", "1,,3"],
    ["-t", "30", "-a", "x,1,2"],
    ["-t", "30", "-a", "1e-30,0,0"],
    ["-t", "30", "-c", "5"],
    ["-t", "30", "--cov", "t1.nii"],
    ["-t", "30", "--cov=t1.nii"],
    ["-t", "30", "--cov=epi.nii", "-a", "0,1,1"],
    ["-t", "30", "--cov=missing.nii"],
    ["-t", "30", "--cov=t1.nii", "-c", "1,2,3"],
    ["-t", "30", "--cov=coords.txt"],
    ["-t", "30", "-a", "1,2,3,4,5,6,7", "-a", "9"],
    ["-t", "30", "-a", "1", "-a", "2"],
    ["-t", "30", "-c", "1,2", "-a", "0,1,0"],
    ["-t", "30", "-a", "1,2,3", "-c", "4,5,6", "-o", "out.mat", "-v"],
    ["-t", "abc"],
    ["-t", "30abc"],
    ["-t"],
    ["-t", "30", "extra"],
    ["-t", "30", "-x"],
    ["-t", "30", "--bogus=1"],
    ["-t", "30", "-v"],
    ["-vt", "30"],
    ["-t", "30", "-o", "out.mat"],
    ["-t", "30", "--out=out.mat", "-a", "0,0,1"],
    ["-t", "30", "-o", "nodir/out.mat"],
    ["--theta", "30"],
    ["-t", "30", "-t", "60"],
    ["-t", "nan"],
    ["-t", "inf", "-a", "1,0,0"],
    ["-t", "30", "-a", "nan,0,1"],
    ["-t", "30", "-a", "1e30,1e30,0"],
    ["-t", "1e30", "-a", "1,0,0"],
]

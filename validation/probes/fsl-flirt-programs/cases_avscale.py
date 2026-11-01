# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of avscale."""

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

PROGRAM = "avscale"
FILES = {
    "init.mat": INP / "init.mat",
    "epi.nii": INP / "epi.nii",
    "sing.mat": "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n",
    "short.mat": "1 2 3\n",
    "flip.mat": "-1 0 0 10\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
    "aff.mat": "1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n",
    "three.mat": "1 0 0\n0 1 0\n0 0 1\n",
    "five.mat": "1 0 0 0 0\n0 1 0 0 0\n0 0 1 0 0\n0 0 0 1 0\n0 0 0 0 1\n",
    "nan.mat": "nan 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
}
CASES = [
    [],
    ["init.mat"],
    ["--allparams", "init.mat"],
    ["--inverteddies", "aff.mat"],
    ["nothere.mat"],
    ["short.mat"],
    ["sing.mat"],
    ["flip.mat"],
    ["--allparams", "aff.mat", "epi.nii"],
    ["init.mat", "nothere.nii.gz"],
    ["init.mat", ""],
    ["--frob", "init.mat"],
    ["three.mat"],
    ["five.mat"],
    ["nan.mat"],
    [""],
    ["--allparams"],
]

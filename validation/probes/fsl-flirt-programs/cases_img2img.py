# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of img2img."""

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

PROGRAM = "img2imgcoord"
FILES = {
    "init.mat": INP / "init.mat",
    "epi.nii": INP / "epi.nii",  # neurological? (BOLD reference)
    "t1.nii": INP / "t1.nii",
    "coords.txt": "10 20 30\n0 0 0\n-5.5 3.25 100\n1e3 2 3\n",
    "bad.txt": "1 2 x 4\n5 6 7\n",
    "short.mat": "1 2 3\n",
}
CASES = [
    [],
    ["-help"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-xfm", "init.mat", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-xfm", "init.mat", "-mm", "coords.txt"],
    ["-src", "t1.nii", "-dest", "epi.nii", "coords.txt"],
    ["-src", "t1.nii", "-dest", "epi.nii", "-mm", "-v", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-xfm", "init.mat", "-v", "-vox", "coords.txt"],
    ["-src", "nothere.nii", "-dest", "t1.nii", "coords.txt"],
    ["-src", "epi.nii", "-dest", "nothere.nii", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-xfm", "nothere.mat", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-xfm", "short.mat", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "nothere.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "bad.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-frob", "x", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "coords.txt", "-xfm"],
    ["-dest", "t1.nii", "coords.txt"],
    ["-src", "epi.nii", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-flirt", "-premat", "init.mat", "coords.txt"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-"],
    ["-src", "epi.nii", "-dest", "t1.nii"],
    ["-src", "epi.nii", "-dest", "t1.nii", "-v"],
]

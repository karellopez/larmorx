# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of std."""

import os
from pathlib import Path

import nibabel as nib
import numpy as np

#: the parity suite's derived inputs (``oracles/inputs/fsl-flirt-replica`` in the workspace)
INP = Path(
    os.environ.get(
        "LARMORX_FLIRT_INPUTS",
        Path(__file__).resolve().parents[4] / "oracles/inputs/fsl-flirt-replica",
    )
)
FSL = Path(os.environ.get("LARMORX_FSL_623", Path.home() / "fsl-6.0.7.23"))


STD = FSL / "data/standard/MNI152_T1_2mm_brain_mask.nii.gz"
here = Path(os.environ.get("PROBE_TMP", "/tmp"))
# an image without sform or qform
nos = here / "noform.nii"
if not nos.exists():
    img = nib.Nifti1Image(np.zeros((5, 6, 7), np.float32), np.diag([2.0, 3.0, 4.0, 1.0]))
    img.header.set_sform(None, code=0)
    img.header.set_qform(None, code=0)
    nib.save(img, str(nos))
PROGRAM = os.environ.get("PROG", "std2imgcoord")
FILES = {
    "init.mat": INP / "init.mat",
    "epi.nii": INP / "epi.nii",
    "t1.nii": INP / "t1.nii",
    "std.nii.gz": STD,
    "noform.nii": nos,
    "coords.txt": "10 20 30\n0 0 0\n-5.5 3.25 100\n1e3 2 3\n",
    "sing.mat": "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n",
    "four3.mat": "1 0 0\n0 1 0\n0 0 1\n0 0 0\n",
}
CASES = [
    [],
    ["-help"],
    ["-img", "epi.nii", "-std", "std.nii.gz", "-xfm", "init.mat", "coords.txt"],
    ["-img", "epi.nii", "-std", "std.nii.gz", "-xfm", "init.mat", "-vox", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-mm", "-v", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-verbose", "-xfm", "init.mat", "coords.txt"],
    ["-img", "t1.nii", "coords.txt"],
    ["-img", "t1.nii", "-std", "noform.nii", "coords.txt"],
    ["-img", "noform.nii", "-std", "t1.nii", "-vox", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-xfm", "sing.mat", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-xfm", "four3.mat", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-flirt", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-warp", "x.nii.gz", "coords.txt"],
    ["-std", "std.nii.gz", "coords.txt"],
    ["-img", "t1.nii", "-std", "nothere.nii", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "nothere.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "-frob", "x", "coords.txt"],
    ["-img", "t1.nii", "-std", "std.nii.gz", "coords.txt", "-xfm"],
    (["-img", "t1.nii", "-std", "std.nii.gz", "-"], "1 2 3\n4 5 6\n4 5 6\n7 8 9\n"),
    (["-img", "t1.nii", "-std", "std.nii.gz", "-v", "-"], "1 2 3\n"),
    (["-img", "epi.nii", "-std", "std.nii.gz"], "10 20 30\n"),
]

# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of rmsdiff."""

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


PROGRAM = "rmsdiff"
FILES = {
    "init.mat": INP / "init.mat",
    "epi.nii": INP / "epi.nii",
    "t1.nii": INP / "t1.nii",
    "wm.nii": INP / "wm.nii",
    "bold4.nii": INP / "bold4.nii",
    "id.mat": "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
    "small.mat": "0.1 0 0 0\n0 0.5 0 0\n0 0 1 0\n0 0 0 1\n",
    "sing.mat": "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n",
    "short.mat": "1 2\n",
    "empty.nii": None,
}

e = Path(os.environ.get("TMPDIR", "/tmp")) / "larmorx-probe-emptymask.nii"
if not e.exists():
    nib.save(
        nib.Nifti1Image(np.zeros((96, 144, 144), np.float32), nib.load(str(INP / "t1.nii")).affine),
        str(e),
    )
FILES["empty.nii"] = e
rng = np.random.default_rng(7)
for i in range(30):
    m = np.eye(4)
    m[:3, :3] += rng.normal(0, 0.05, (3, 3))
    m[:3, 3] = rng.normal(0, 5, 3)
    FILES[f"r{i}.mat"] = "\n".join("  ".join(f"{v:.10g}" for v in row) for row in m) + "\n"
CASES = (
    [
        [],
        ["init.mat", "id.mat"],
        ["init.mat", "id.mat", "epi.nii"],
        ["init.mat", "id.mat", "t1.nii", "wm.nii"],
        ["small.mat", "id.mat", "epi.nii"],
        ["id.mat", "small.mat", "epi.nii", "wm.nii"],
        ["sing.mat", "id.mat", "epi.nii"],
        ["id.mat", "sing.mat", "epi.nii"],
        ["nothere.mat", "id.mat", "epi.nii"],
        ["init.mat", "short.mat", "epi.nii"],
        ["init.mat", "id.mat", "nothere.nii"],
        ["init.mat", "id.mat", "epi.nii", "nothere.nii"],
        ["init.mat", "id.mat", "bold4.nii"],
        ["init.mat", "id.mat", "t1.nii", "empty.nii"],
        ["init.mat", "id.mat", "epi.nii", "wm.nii", "extra"],
    ]
    + [[f"r{i}.mat", f"r{i + 1}.mat", "epi.nii"] for i in range(0, 29, 2)]
    + [[f"r{i}.mat", "init.mat", "t1.nii", "wm.nii"] for i in range(1, 30, 3)]
)

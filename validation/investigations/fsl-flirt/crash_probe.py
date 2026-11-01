# SPDX-License-Identifier: Apache-2.0
"""Which inputs make FSL's flirt crash (finding 28's class): constant or one-voxel images, at
several sizes, with the default options; FSL and the replica's exit status side by side."""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

FSL = Path.home() / "fsl-6.0.7.23"
BIN = sys.argv[1]  # the larmorx-nc program


def save(p, data, zooms):
    aff = np.diag([*zooms, 1.0])
    img = nib.Nifti1Image(data.astype(np.float32), aff)
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    nib.save(img, str(p))


rng = np.random.default_rng(1)
cases = {
    "in1-const_ref8x1x1-const": ((1, 1, 1), "const", (8, 1, 1), "const"),
    "in1_ref8x8x8-noise": ((1, 1, 1), "const", (8, 8, 8), "noise"),
    "in8-const_ref8-noise": ((8, 8, 8), "const", (8, 8, 8), "noise"),
    "in8-noise_ref8-const": ((8, 8, 8), "noise", (8, 8, 8), "const"),
    "in8-const_ref8-const": ((8, 8, 8), "const", (8, 8, 8), "const"),
    "in2x2x2-noise_ref8-noise": ((2, 2, 2), "noise", (8, 8, 8), "noise"),
    "in4x1x1-noise_ref8-noise": ((4, 1, 1), "noise", (8, 8, 8), "noise"),
    "in8-noise_ref1-const": ((8, 8, 8), "noise", (1, 1, 1), "const"),
}
env = dict(
    os.environ,
    FSLDIR=str(FSL),
    FSLOUTPUTTYPE="NIFTI_GZ",
    OPENBLAS_CORETYPE="Haswell",
    LARMORX_UPSTREAM_DEFECTS="reproduce",
)
for name, (si, ci, sr, cr) in cases.items():
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        mk = lambda s, c: np.full(s, 100.0) if c == "const" else rng.random(s) * 100  # noqa: E731
        save(d / "in.nii.gz", mk(si, ci), (1, 1, 1))
        save(d / "ref.nii.gz", mk(sr, cr), (1, 1, 1))
        args = ["-in", "in.nii.gz", "-ref", "ref.nii.gz", "-omat", "o.mat"]
        a = subprocess.run(
            [str(FSL / "bin/flirt"), *args],
            cwd=d,
            env=env,
            capture_output=True,
            text=True,
            timeout=600,
        )
        b = subprocess.run(
            [BIN, "fsl", "flirt", *args],
            cwd=d,
            env=env,
            capture_output=True,
            text=True,
            timeout=600,
        )
        print(
            f"{name:30s} FSL {a.returncode:4d} replica {b.returncode:4d}  FSL: "
            f"{' | '.join(a.stderr.splitlines()[-2:])[:100]}"
        )

# SPDX-License-Identifier: Apache-2.0
"""Probe (FSL-270 candidate): what NaN phase values inside the mask do to prelude's result.

Runs the replica (reproduce mode: FSL's bits, as the suite showed) on the suite's s_ph and
s_nanph inputs and compares the unwrapped outputs outside the NaN voxels, and with -s."""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

exe, inputs = sys.argv[1], Path(sys.argv[2])
env = dict(os.environ, FSLOUTPUTTYPE="NIFTI_GZ", LARMORX_UPSTREAM_DEFECTS="reproduce")
for extra in ([], ["-s"], ["--labelslices"]):
    outs = {}
    with tempfile.TemporaryDirectory() as d:
        for ph in ("s_ph", "s_nanph"):
            r = subprocess.run(
                [
                    exe,
                    "fsl",
                    "prelude",
                    "-p",
                    str(inputs / f"{ph}.nii.gz"),
                    "-a",
                    str(inputs / "s_mag.nii.gz"),
                    "-o",
                    f"{d}/{ph}",
                    *extra,
                ],
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            outs[ph] = np.asanyarray(nib.load(f"{d}/{ph}.nii.gz").dataobj)
            print(extra, ph, "exit", r.returncode, r.stderr.strip()[:200])
    a, b = outs["s_ph"], outs["s_nanph"]
    nan = np.isnan(b)
    diff = (a != b) & ~nan & ~np.isnan(a)
    k = np.round((b - a)[diff] / (2 * np.pi))
    print(
        extra,
        "NaN outputs",
        int(nan.sum()),
        "other voxels changed",
        int(diff.sum()),
        "of",
        int((a != 0).sum()),
        "nonzero; shifts in 2pi",
        np.unique(k)[:10],
    )

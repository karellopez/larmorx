# SPDX-License-Identifier: Apache-2.0
"""Probe (FSL-269): FSL's unwrapped phase (the replica in reproduce mode, bit-identical in the
suite) against the corrected one for a NaN phase inside a region of the first phase bin."""

import os
import subprocess
import sys
import tempfile

import nibabel as nib
import numpy as np

exe, inputs = sys.argv[1], sys.argv[2]
out = {}
with tempfile.TemporaryDirectory() as d:
    for mode in ("reproduce", "fix"):
        env = dict(os.environ, FSLOUTPUTTYPE="NIFTI_GZ", LARMORX_UPSTREAM_DEFECTS=mode)
        r = subprocess.run(
            [
                exe,
                "fsl",
                "prelude",
                "-p",
                f"{inputs}/s_nanph_bin1.nii.gz",
                "-a",
                f"{inputs}/s_mag.nii.gz",
                "-m",
                f"{inputs}/s_mask.nii.gz",
                "-o",
                f"{d}/{mode}",
            ],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        print(mode, r.returncode, r.stderr.strip()[-300:])
        out[mode] = np.asanyarray(nib.load(f"{d}/{mode}.nii.gz").dataobj).astype(np.float64)
a, b = out["reproduce"], out["fix"]
ok = ~np.isnan(a) & ~np.isnan(b)
d = np.abs(a - b)[ok]
print(
    "voxels differing",
    int((d > 0).sum()),
    "max |FSL|",
    float(np.nanmax(np.abs(a))),
    "max |fixed|",
    float(np.nanmax(np.abs(b))),
    "max diff / 2pi",
    float(d.max() / (2 * np.pi)),
)

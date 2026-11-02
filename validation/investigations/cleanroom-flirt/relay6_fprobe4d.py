# SPDX-License-Identifier: Apache-2.0
"""fprobe4d.py <outdir>: applyxfm4D with per-volume matrices and a formless reference (FSL
through the oracle cache, use mode; the original with reproduce)."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=True)
rng = np.random.default_rng(8)
data = (rng.random((12, 10, 8, 3)) * 100).astype(np.float32)
c, s = np.cos(0.2), np.sin(0.2)
oblique = np.array(
    [
        [c * 1.5, -s * 2.0, 0.1, -20.0],
        [s * 1.5, c * 2.0, 0.05, 10.0],
        [0.0, 0.1, 2.5, -5.0],
        [0, 0, 0, 1],
    ]
)
mats = [
    "1 0 0 1\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
    "0.99 -0.1 0 2\n0.1 0.99 0 0\n0 0 1 0\n0 0 0 1\n",
    "1 0 0 0\n0 1 0 3\n0 0 1 0\n0 0 0 1\n",
]
for who in ("fsl", "rep"):
    d = root / who
    d.mkdir(parents=True, exist_ok=True)
    img = nib.Nifti1Image(data, oblique)
    img.header.set_zooms((1.5, 2.0, 2.5, 1.0))
    img.set_sform(oblique, 1)
    img.set_qform(oblique, 1)
    nib.save(img, str(d / "in.nii"))
    ref = nib.Nifti1Image(np.zeros((14, 9, 7), np.float32), None)
    ref.header.set_zooms((2.0, 2.0, 2.0))
    ref.set_sform(np.diag([2.0, 2, 2, 1]), 0)
    ref.set_qform(np.diag([2.0, 2, 2, 1]), 0)
    nib.save(ref, str(d / "ref.nii"))
    (d / "mats").mkdir(exist_ok=True)
    for i, t in enumerate(mats):
        (d / "mats" / f"MAT_{i:04d}").write_text(t)
    args = ["in.nii", "ref.nii", "out", "mats", "-fourdigit", "-interp", "trilinear"]
    st = {"FSLOUTPUTTYPE": "NIFTI"}
    r = (
        rep.run_oracle("applyxfm4D", args, cwd=d, settings=st)
        if who == "fsl"
        else orig.run_original("applyxfm4D", args, cwd=d, settings=st, mode=defects.REPRODUCE)
    )
    h = nib.load(str(d / "out.nii")).header
    print(
        who,
        r.code,
        int(h["qform_code"]),
        int(h["sform_code"]),
        np.round(np.array([h["srow_x"], h["srow_y"], h["srow_z"]]), 5).tolist(),
    )
    print("   data", np.asarray(nib.load(str(d / "out.nii")).dataobj).sum())

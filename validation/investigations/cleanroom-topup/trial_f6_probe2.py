# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: finding 6's 1 x 1 x 3 pair tiled (x and y to 4 voxels; z to 24 = 8 periods): which
short axis makes FSL's smoothed --featout magnitude differ from the original's."""

import shutil, sys
from pathlib import Path
import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as T

src = Path(sys.argv[1])
base = Path(sys.argv[2])
img = nib.load(src / "pair.nii.gz")
x = np.asarray(img.dataobj)
tiles = {"xy4": (4, 4, 1, 1), "z24": (1, 1, 8, 1), "x4": (4, 1, 1, 1)}
for name, reps in tiles.items():
    data = np.tile(x, reps)
    for who in ("fsl", "lx"):
        d = base / name / who
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        nib.save(nib.Nifti1Image(data, img.affine, img.header), d / "pair.nii.gz")
        shutil.copy(src / "acq.txt", d / "acq.txt")
        args = ["--imain=pair.nii.gz", "--datain=acq.txt", "--out=o", "--featout=feat"]
        run = T.Run(tuple(args))
        if who == "fsl":
            p = T._oracle(run, list(args), d, {"FSLOUTPUTTYPE": "NIFTI"})
        else:
            p = orig.run_original(
                list(args), cwd=d, settings={"FSLOUTPUTTYPE": "NIFTI"}, mode=defects.REPRODUCE
            )
        mag = np.asarray(nib.load(d / "feat_fieldmap_mag.nii").dataobj)
        print(
            f"{name:5s} {who}: exit {p.code} mag[0,0,:3] {mag[0, 0, :3]} max-min {float(mag.max() - mag.min()):.3g}"
        )

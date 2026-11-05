# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: finding 6's shrunk case (1 x 1 x 3 voxels, 3 volumes): FSL (oracle cache, use) and the
original on variants (default, --fwhm=0, --estmov=0), each with --iout and --featout."""

import shutil, sys
from pathlib import Path
import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as T

src = Path(sys.argv[1])
base = Path(sys.argv[2])
variants = {
    "default": [],
    "fwhm0": ["--fwhm=0"],
    "estmov0": ["--estmov=0"],
    "both0": ["--fwhm=0", "--estmov=0"],
}
for name, extra in variants.items():
    for who in ("fsl", "lx"):
        d = base / name / who
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        shutil.copy(src / "pair.nii.gz", d / "pair.nii.gz")
        shutil.copy(src / "acq.txt", d / "acq.txt")
        args = [
            "--imain=pair.nii.gz",
            "--datain=acq.txt",
            "--out=o",
            "--featout=feat",
            "--iout=u",
            *extra,
        ]
        run = T.Run(tuple(args))
        if who == "fsl":
            p = T._oracle(run, list(args), d, {"FSLOUTPUTTYPE": "NIFTI"})
        else:
            p = orig.run_original(
                list(args), cwd=d, settings={"FSLOUTPUTTYPE": "NIFTI"}, mode=defects.REPRODUCE
            )
        mag = np.asarray(nib.load(d / "feat_fieldmap_mag.nii").dataobj).ravel()
        u = np.asarray(nib.load(d / "u.nii").dataobj)[0, 0]
        um = np.asarray(nib.load(next(d.glob("u*_mean*"))).dataobj).ravel()
        mov = (d / "o_movpar.txt").read_text().split()
        print(f"{name:8s} {who}: exit {p.code} mag {mag} iout_mean {um}")
        print(f"          iout (z, t) {u.ravel()} movs-nonzero {[m for m in mov if m != '0']}")

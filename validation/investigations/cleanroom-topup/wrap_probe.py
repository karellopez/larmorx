# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL-133 probes (applytopup --method=lsr on lines of 30 voxels): synthetic inputs written into
<outdir>: two images of 6 x 30 x 5 voxels (2 mm, radiological), image 1 zero but for one voxel
of 1000 at (x, y, z), image 2 zero; coefficients with knot spacing 1 and every coefficient
<hz> Hz (intent 2016), movements of zeros; acquisition rows (0, +-1, 0, 0.05).
Usage: wrap_probe.py <outdir> <x> <y> <z> <hz> [<ny>, default 30]   (then at_case.py args <outdir> - -- ...)"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def save(path, data, aff, intent=None):
    img = nib.Nifti1Image(np.asarray(data, np.float32), aff)
    img.header.set_xyzt_units("mm", "sec")
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    if intent is not None:
        img.header["intent_code"], p = intent
        img.header["intent_p1"], img.header["intent_p2"], img.header["intent_p3"] = p
    nib.save(img, str(path))


def main():
    out, x, y, z, hz = Path(sys.argv[1]), *map(int, sys.argv[2:5]), float(sys.argv[5])
    ny = int(sys.argv[6]) if len(sys.argv) > 6 else 30
    out.mkdir(parents=True, exist_ok=True)
    n = (6, ny, 5)
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    a = np.zeros(n)
    a[x, y, z] = 1000.0
    save(out / "pu.nii.gz", a, aff)
    save(out / "pd.nii.gz", np.zeros(n), aff)
    # the coefficients: knot spacing 1 (voxel sizes 1), the field's size as the qform's
    # translation, its voxel sizes as the intent parameters
    caff = np.eye(4)
    caff[:3, 3] = n
    save(out / "tu_fieldcoef.nii.gz", np.full(n, hz), caff, (2016, (2.0, 2.0, 2.0)))
    img = nib.load(str(out / "tu_fieldcoef.nii.gz"))
    img.set_sform(None, 0)
    nib.save(img, str(out / "tu_fieldcoef.nii.gz"))
    (out / "tu_movpar.txt").write_text("0 0 0 0 0 0\n0 0 0 0 0 0\n")
    (out / "acqw.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n")


main()

# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Probes for applytopup --method=lsr where the line's mapping folds (specs/topup.md §7.4,
spec writer, 2026-11-03): two images of 6 x n x 5 voxels (2 mm, radiological), lines along y,
image 1 a smooth profile with one bright voxel, image 2 a shifted copy; coefficients of knot
spacing 1 rising linearly along y (`slope` Hz per voxel from 0), so the field's two ends differ
and the mapping `phi(k) = k + s h(k)` steps back across the line's wrap-around; movements of
zeros; acquisition rows (0, +-1, 0, t). Then `cleanroom-topup/at_case.py args <dir> - --
--imain=pu.nii.gz,pd.nii.gz --inindex=1,2 --datain=acqw.txt --topup=tu --out=hifi` (FSL through
the oracle cache) and `lsr_model.py <parent dir>` (its directory name `p_2_<y>_2`).

Usage: fold_probe.py <outdir> <y bright> <slope Hz/voxel> <t> [<n>, default 16]"""

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
    out, y, slope, t = Path(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])
    ny = int(sys.argv[5]) if len(sys.argv) > 5 else 16
    out.mkdir(parents=True, exist_ok=True)
    n = (6, ny, 5)
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    j = np.arange(ny, dtype=float)
    profile = 200.0 + 10.0 * j - 0.5 * j * j  # plain arithmetic: identical everywhere
    a = np.broadcast_to(profile[None, :, None], n).copy()
    a[2, y, 2] += 1000.0
    b = np.roll(a, 1, axis=1) * 0.9
    save(out / "pu.nii.gz", a, aff)
    save(out / "pd.nii.gz", b, aff)
    caff = np.eye(4)
    caff[:3, 3] = n
    coef = np.broadcast_to((slope * j)[None, :, None], n).copy()
    save(out / "tu_fieldcoef.nii.gz", coef, caff, (2016, (2.0, 2.0, 2.0)))
    img = nib.load(str(out / "tu_fieldcoef.nii.gz"))
    img.set_sform(None, 0)
    nib.save(img, str(out / "tu_fieldcoef.nii.gz"))
    (out / "tu_movpar.txt").write_text("0 0 0 0 0 0\n0 0 0 0 0 0\n")
    (out / "acqw.txt").write_text(f"0 1 0 {t}\n0 -1 0 {t}\n")


main()

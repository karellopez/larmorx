# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""applytopup --method=vb* probes: synthetic inputs written into <outdir>: <nimg> images of
<dims> voxels (`--vox`, default 2 mm, radiological), seeded uniform integers 0..999 (`--nvol` volumes), the
coefficients of a constant field of <hz> Hz (knot spacing 1, intent 2016; `--ramp` adds
<ramp> Hz per voxel along y to the coefficients), movements of zeros (or `--mov "tx ty tz rx ry rz;..."`), and the acquisition
rows given (default (0, 1, 0, 0.05) and (0, -1, 0, 0.05)).
Usage: vb_probe.py <outdir> <nx,ny,nz> <hz> [--seed s] [--nimg m] [--nvol v] [--ramp r]
       [--acq "px py pz t;..."]       (then at_case.py args <outdir> - -- ...)"""

import argparse
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
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("dims")
    ap.add_argument("hz", type=float)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--nimg", type=int, default=2)
    ap.add_argument("--nvol", type=int, default=1)
    ap.add_argument("--ramp", type=float, default=0.0)
    ap.add_argument("--acq", default="0 1 0 0.05;0 -1 0 0.05")
    ap.add_argument("--vox", default="2,2,2")
    ap.add_argument("--mov", default="")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    n = tuple(int(v) for v in a.dims.split(","))
    vox = [float(v) for v in a.vox.split(",")]
    aff = np.diag([-vox[0], vox[1], vox[2], 1.0])
    rng = np.random.default_rng(a.seed)
    for i in range(a.nimg):
        shape = n if a.nvol == 1 else (*n, a.nvol)
        save(out / f"p{i}.nii.gz", rng.integers(0, 1000, shape).astype(np.float32), aff)
    caff = np.eye(4)
    caff[:3, 3] = n
    coef = np.full(n, a.hz) + a.ramp * np.arange(n[1])[None, :, None]
    save(out / "tu_fieldcoef.nii.gz", coef, caff, (2016, tuple(vox)))
    img = nib.load(str(out / "tu_fieldcoef.nii.gz"))
    img.set_sform(None, 0)
    nib.save(img, str(out / "tu_fieldcoef.nii.gz"))
    rows = [r.strip() for r in a.acq.split(";")]
    mov = [r.strip() for r in a.mov.split(";")] if a.mov else ["0 0 0 0 0 0"] * len(rows)
    (out / "tu_movpar.txt").write_text("\n".join(mov) + "\n")
    (out / "acqw.txt").write_text("\n".join(rows) + "\n")


main()

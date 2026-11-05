# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""catchup8_int_nn.py <outdir>: how FSL's applyxfm4D and flirt -applyxfm convert the values of an
int32 image beyond float precision (the clean-room catch-up's trial, 2026-11-05: finding
20261103-03732, an image of 2147483647 everywhere, FSL writes 2147483647 where the original writes
-2147483648). An int32 input of 8 x 8 x 6 voxels (blocks of 16777217, 2147483647, -2147483647,
123456789 and 7), a float reference on the same grid, the identity matrix; FSL through the oracle
cache (use mode), the original in reproduce and in its default; prints each output's values at the
blocks' centres and its background."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

VALUES = (16777217, 2147483647, -2147483647, 123456789, 7)
root = Path(sys.argv[1])


def inputs(d: Path) -> None:
    v = np.zeros((8, 8, 6), np.int32)
    for k, x in enumerate(VALUES):
        v[:, :, k] = x
    v[:, :, 5] = VALUES[1]
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    im = nib.Nifti1Image(v, aff)
    im.header.set_xyzt_units("mm", "sec")
    nib.save(im, str(d / "in.nii.gz"))
    ref = nib.Nifti1Image(np.ones((8, 8, 6), np.float32), aff)
    ref.header.set_xyzt_units("mm", "sec")
    nib.save(ref, str(d / "ref.nii.gz"))
    (d / "id.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n")
    (d / "shift.mat").write_text("1 0 0 8\n0 1 0 0\n0 0 1 0\n0 0 0 1\n")
    b = np.full((8, 8, 6), VALUES[1], np.int32)
    b[2:6, 2:6, 1:5] = 7
    im = nib.Nifti1Image(b, aff)
    im.header.set_xyzt_units("mm", "sec")
    nib.save(im, str(d / "border.nii.gz"))


CALLS = {
    "applyxfm4D-nn": (
        "applyxfm4D",
        [
            "in.nii.gz",
            "ref.nii.gz",
            "out",
            "id.mat",
            "-singlematrix",
            "-interp",
            "nearestneighbour",
        ],
    ),
    "applyxfm4D-tri": (
        "applyxfm4D",
        ["in.nii.gz", "ref.nii.gz", "out", "id.mat", "-singlematrix", "-interp", "trilinear"],
    ),
    "flirt-nn": (
        "flirt",
        [
            "-in",
            "in.nii.gz",
            "-ref",
            "ref.nii.gz",
            "-out",
            "out",
            "-applyxfm",
            "-init",
            "id.mat",
            "-interp",
            "nearestneighbour",
        ],
    ),
    "applyxfm4D-nn-shift": (
        "applyxfm4D",
        [
            "border.nii.gz",
            "ref.nii.gz",
            "out",
            "shift.mat",
            "-singlematrix",
            "-interp",
            "nearestneighbour",
        ],
    ),
    "applyxfm4D-tri-shift": (
        "applyxfm4D",
        [
            "border.nii.gz",
            "ref.nii.gz",
            "out",
            "shift.mat",
            "-singlematrix",
            "-interp",
            "trilinear",
        ],
    ),
    "flirt-nn-shift": (
        "flirt",
        [
            "-in",
            "border.nii.gz",
            "-ref",
            "ref.nii.gz",
            "-out",
            "out",
            "-applyxfm",
            "-init",
            "shift.mat",
            "-interp",
            "nearestneighbour",
        ],
    ),
    "flirt-tri": (
        "flirt",
        [
            "-in",
            "in.nii.gz",
            "-ref",
            "ref.nii.gz",
            "-out",
            "out",
            "-applyxfm",
            "-init",
            "id.mat",
            "-interp",
            "trilinear",
        ],
    ),
}

for name, (program, args) in CALLS.items():
    out = root / name
    rows = []
    for who in ("fsl", defects.REPRODUCE, defects.FIX):
        d = out / who
        d.mkdir(parents=True, exist_ok=True)
        inputs(d)
        env = {**rep._environment({}), "FSLOUTPUTTYPE": "NIFTI_GZ"}
        if who == "fsl":
            p = oracle_cache.run(
                [str(rep.fsldir() / "bin" / program), *args],
                env=env,
                cwd=d,
                outputs=[d],
                policy=oracle_cache.Policy(threads=1, family="fsl"),
                runner="catchup8-int-nn",
                inputs=[x for x in sorted(d.iterdir()) if x.is_file()],
                timeout=600,
            )
            code = p.returncode
        else:
            code = orig.run_original(
                program, args, cwd=d, settings={"FSLOUTPUTTYPE": "NIFTI_GZ"}, mode=who
            ).code
        o = nib.load(str(d / "out.nii.gz"))
        x = np.asarray(o.dataobj)
        rows.append(
            f"  {who:9s} exit {code} {o.get_data_dtype()} centres {[int(x[4, 4, k]) for k in range(6)]} corner {int(x[0, 0, 0])} x-row {[int(v) for v in x[:, 4, 2]]}"
        )
    print(name)
    print("\n".join(rows))

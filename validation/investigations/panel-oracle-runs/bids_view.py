# SPDX-License-Identifier: Apache-2.0
"""The BIDS input of a data-panel fMRIPrep oracle run: one dataset's panel files, hard-linked
from the test-data cache (no copies, the bytes the catalog verified), in their BIDS layout.

For ds004884 (the Aphasia Recovery Cohort) the view also holds the subject's lesion mask where
fMRIPrep 25.2.6 looks for it (``anat/sub-<label>[_ses-<label>]_label-lesion_roi.nii.gz``, in
the T1w's space; niworkflows' query ``{'datatype': 'anat', 'suffix': 'roi'}``). The authors drew
the mask on the session's T2w and stored it on that grid, and sMRIPrep subtracts the mask from
a mask on the T1w's grid voxel by voxel (niworkflows' ``create_cfm``), so the view carries the
mask resampled onto the T1w's grid: nearest neighbour through both files' scanner coordinates,
no registration (the T2w and the T1w of this session align by their headers to about 1 mm,
measured with a rigid registration: 0.38 degrees and 0.4 mm, at most 1.1 mm at the brain's
edge). The mask file is the only file of a view that is not upstream's; its voxels are
written by :func:`mask_on_grid` and its SHA-256 printed.

usage: bids_view.py <dataset> <out dir>   e.g. bids_view.py ds004884 <workspace>/oracles/bids-views/ds004884
"""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

import numpy as np

#: ds004884's lesion mask: the upstream mask (T2w grid) and the T1w it is put onto.
LESION = {
    "ds004884": (
        "openneuro/ds004884/derivatives/lesion_masks/sub-M2079/ses-217/anat/"
        "sub-M2079_ses-217_acq-spc3_run-4_T2w_desc-lesion_mask.nii.gz",
        "openneuro/ds004884/sub-M2079/ses-217/anat/sub-M2079_ses-217_acq-tfl3p2_run-3_T1w.nii.gz",
        "sub-M2079/ses-217/anat/sub-M2079_ses-217_label-lesion_roi.nii.gz",
    )
}


def mask_on_grid(mask_img, ref_img):
    """``mask_img``'s values on ``ref_img``'s grid: for each voxel centre of the reference,
    the mask's voxel nearest to the same scanner coordinates (0 outside the mask's grid)."""
    import nibabel as nib

    ref_shape = ref_img.shape[:3]
    ijk = np.indices(ref_shape, dtype=np.float64).reshape(3, -1)
    world = ref_img.affine @ np.vstack([ijk, np.ones(ijk.shape[1])])
    src = np.linalg.inv(mask_img.affine) @ world
    idx = np.floor(src[:3] + 0.5).astype(np.int64)
    data = np.asanyarray(mask_img.dataobj)
    inside = np.all((idx >= 0) & (idx < np.array(data.shape[:3])[:, None]), axis=0)
    out = np.zeros(ijk.shape[1], dtype=np.uint8)
    out[inside] = (data[idx[0, inside], idx[1, inside], idx[2, inside]] != 0).astype(np.uint8)
    img = nib.Nifti1Image(out.reshape(ref_shape), ref_img.affine)
    img.set_qform(ref_img.affine, int(ref_img.header["qform_code"]))
    img.set_sform(ref_img.affine, int(ref_img.header["sform_code"]))
    img.header.set_xyzt_units("mm")
    return img


def main() -> None:
    import larmorx_testdata as td
    import nibabel as nib

    ds, out = sys.argv[1], Path(sys.argv[2])
    entries = [e for e in td.select(tier="full", tags={"panel"}) if e.path.split("/")[1] == ds]
    if not entries:
        raise SystemExit(f"no panel files for {ds}")
    for e in entries:
        rel = e.path.split("/", 2)[2]
        if rel.startswith("derivatives/"):
            continue
        dest = out / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        if not dest.exists():
            os.link(td.get(e), dest)
    if ds in LESION:
        mask_path, t1w_path, roi = LESION[ds]
        img = mask_on_grid(nib.load(str(td.get(mask_path))), nib.load(str(td.get(t1w_path))))
        dest = out / roi
        nib.save(img, dest)
        data = np.asanyarray(img.dataobj)
        vox = float(np.prod(img.header.get_zooms()[:3]))
        sha = hashlib.sha256(dest.read_bytes()).hexdigest()
        print(f"{roi}: {int(data.sum())} voxels ({data.sum() * vox / 1000:.2f} mL), sha256 {sha}")
    n = sum(1 for p in out.rglob("*") if p.is_file())
    print(f"{out}: {n} files")


if __name__ == "__main__":
    main()

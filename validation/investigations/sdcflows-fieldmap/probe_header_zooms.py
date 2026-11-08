# SPDX-License-Identifier: Apache-2.0
"""Do field maps' header zooms agree with their affines? (SDC-f's second half.)

sdcflows' ``bspline_grid`` takes the voxel sizes from the header (``get_zooms()``, every
dimension) and divides the affine's columns by them to get direction cosines: when the header's
pixdim disagree with the affine's column lengths (a header whose sform scales differently),
the control grid's spacing is off by their ratio; a 4D header (``(x, y, z, 1)``) fails.

For every field map and magnitude/phase image of the fMRIPrep oracle runs (the BIDS inputs
and the field maps fMRIPrep's ``bs_filter`` fitted), this prints the number of dimensions and
the largest relative difference between the header zooms and the affine's voxel sizes.

    python -P probe_header_zooms.py   (LARMORX_WORKSPACE=<workspace>)
"""

import os
from pathlib import Path

import nibabel as nb
import numpy as np

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
PATTERNS = (
    "*_fieldmap.nii*",
    "*_phasediff.nii*",
    "*_phase1.nii*",
    "*_phase2.nii*",
    "*_magnitude*.nii*",
    "*_epi.nii*",
    "*_fmap.nii.gz",
)

seen = 0
worst = 0.0
four_d = []
for run in sorted(p for p in R.iterdir() if p.is_dir()):
    files = set()
    for pat in PATTERNS:
        files.update(run.rglob(pat))
    for f in sorted(files):
        if "/fmap/" not in str(f) and "compfmap" not in str(f):
            continue
        try:
            img = nb.load(f)
        except Exception:  # not an image
            continue
        seen += 1
        z = np.array(img.header.get_zooms()[:3], dtype=float)
        v = nb.affines.voxel_sizes(img.affine)
        rel = float(np.max(np.abs(z - v) / v))
        worst = max(worst, rel)
        if len(img.header.get_zooms()) != 3:
            four_d.append((f.relative_to(R), img.shape))
        if rel > 1e-5:
            print(f"pixdim differ from the affine ({rel:.2e}): {f.relative_to(R)}")
print(f"{seen} images; largest relative zoom difference {worst:.2e}")
print(f"{len(four_d)} with other than 3 header zooms:")
for f, s in four_d:
    print(f"  {f} {s}")

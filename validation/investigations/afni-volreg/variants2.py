# SPDX-License-Identifier: Apache-2.0
"""Relay 2's inputs, from <folder>/s1.nii.gz (make_series.py), for batch.py: slice timing for
-tshift, -gridparent grids, a shifted base for -twodup, and -rotparent datasets made by AFNI's
own 3dvolreg (which writes the VOLREG_* attributes into its NIfTI extension).

Usage: variants2.py <folder> <AFNI's 3dvolreg>
"""

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

d = Path(sys.argv[1])
afni = sys.argv[2]
img = nib.load(d / "s1.nii.gz")
data = np.asarray(img.dataobj, dtype=np.float32)
aff = img.affine


def save(arr, affine, name, tr=None, slice_code=0, slope=None):
    im = nib.Nifti1Image(arr, affine)
    im.set_qform(affine, 1)
    im.set_sform(affine, 1)
    h = im.header
    h.set_xyzt_units("mm", "sec")
    if tr is not None:
        h["pixdim"][4] = tr
    if slice_code:
        h.set_dim_info(slice=2)
        h["slice_code"] = slice_code
        h["slice_start"] = 0
        h["slice_end"] = arr.shape[2] - 1
        h["slice_duration"] = tr / arr.shape[2]
    if slope is not None:
        h.set_slope_inter(slope, 0)
    im.to_filename(d / name)


# Slice timing: sequential ascending, TR 2 s, 24 slices (float32).
save(data, aff, "s6_st.nii.gz", tr=2.0, slice_code=1)
# int16 with a negative slope and alternating slices (AFNI-01 on -tshift's path).
save(
    np.round(-data * 4).astype(np.int16), aff, "s6_stneg.nii.gz", tr=2.0, slice_code=3, slope=-0.25
)
# Slice timing on a 4-volume series (-tshift's "ignore is too large").
save(data[..., :4], aff, "s6_st4.nii.gz", tr=2.0, slice_code=1)

# -gridparent: 27 slices (padding 1 and 2), the same corner; 24 slices; 22 slices (too few);
# 41 x 44 (x, y mismatch); another spacing.
zero = np.zeros((40, 44, 27), np.float32)
save(zero, aff, "gp27.nii.gz")
save(np.zeros((40, 44, 24), np.float32), aff, "gp24.nii.gz")
save(np.zeros((40, 44, 22), np.float32), aff, "gp22.nii.gz")
save(np.zeros((41, 44, 27), np.float32), aff, "gpx.nii.gz")
a_sp = aff.copy()
a_sp[2, 2] = 4.0
save(zero, a_sp, "gpsp.nii.gz")
# 27 slices with the origin moved, slice timing kept (time offsets padded and shifted).
a_gm = aff.copy()
a_gm[:3, 3] += [1.5, -2.0, -3.5]
save(np.zeros((40, 44, 27, 2), np.float32), a_gm, "gp27m.nii.gz")

# -twodup: the base (volume 2) with its origin moved.
a_b = aff.copy()
a_b[:3, 3] += [3.0, -2.0, 1.75]
save(data[..., 2], a_b, "base_shift.nii.gz")

# AFNI-89 and AFNI-93: a constant series, one slice, four slices.
save(np.full((40, 44, 24, 3), 100.0, np.float32), aff, "s7_const.nii.gz")
save(np.ascontiguousarray(data[:, :, 12:13, :4]), aff, "s7_nz1.nii.gz")
save(np.ascontiguousarray(data[:, :, 10:14, :4]), aff, "s7_nz4.nii.gz")

# -rotparent: AFNI's own 3dvolreg outputs (registered to sub-brick 3, so sub-brick 0 moved).
env = {"PATH": "/usr/bin:/bin", "HOME": str(d), "OMP_NUM_THREADS": "1", "AFNI_QUIET_STARTUP": "YES"}
for src, out in (
    ("s1.nii.gz", "rp.nii.gz"),
    ("s6_st.nii.gz", "rp_st.nii.gz"),
    ("s3_oblique.nii.gz", "rp_obl.nii.gz"),
):
    if (d / out).exists():
        os.remove(d / out)
    subprocess.run(
        [afni, "-base", "3", "-prefix", out, src], cwd=d, env=env, check=True, capture_output=True
    )
print("ok")

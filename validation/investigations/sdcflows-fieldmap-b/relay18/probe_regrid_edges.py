# SPDX-License-Identifier: Apache-2.0
"""What niworkflows' ``resample_by_spacing`` does with voxel sizes (``pixdim``) that are not
positive normal numbers, and with the smoothing sigma of ``smooth=True`` (PIPE-35, PIPE-36).

    OPENBLAS_CORETYPE=Haswell python -P probe_regrid_edges.py

Prints, for each edge case, the result's shape and whether it is finite, or the exception.
"""

from __future__ import annotations

import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from niworkflows.utils.images import resample_by_spacing


def case(name, pixdim, zooms=(2.0, 2.0, 2.0), smooth=False):
    rng = np.random.default_rng(1)
    data = rng.normal(100, 20, (6, 7, 5)).astype(np.float32)
    img = nb.Nifti1Image(data, np.diag([1.0, 1.0, 1.0, 1.0]))
    img.header["pixdim"][1:4] = pixdim
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "in.nii.gz"
        img.to_filename(p)
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            try:
                out = resample_by_spacing(str(p), zooms, smooth=smooth)
                arr = np.asarray(out.dataobj)
                msg = f"shape {arr.shape}, finite {bool(np.isfinite(arr).all())}"
            except Exception as e:
                msg = f"{type(e).__name__}: {e}"
        warns = sorted({str(x.message)[:60] for x in w})
    print(f"{name}: {msg}; warnings {warns}")


for name, pd in [
    ("normal", (1.0, 1.0, 1.0)),
    ("zero", (1.0, 0.0, 1.0)),
    ("negative", (1.0, -1.0, 1.0)),
    ("subnormal", (1.0, 1e-40, 1.0)),
    ("nan", (1.0, np.nan, 1.0)),
    ("inf", (1.0, np.inf, 1.0)),
]:
    case(name, pd)
# The smoothing sigma: (pre_zooms / zooms - 1) / 2, positive only when the grid gets finer.
for pre, z in [((1.0, 1.0, 1.0), (4.0, 4.0, 4.0)), ((3.0, 3.0, 3.0), (2.0, 2.0, 2.0))]:
    sig = np.maximum(0, (np.array(pre, dtype=np.float32) / np.array(z) - 1) / 2)
    print(f"smooth=True from {pre} mm to {z} mm: sigma {sig}")

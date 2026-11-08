# SPDX-License-Identifier: Apache-2.0
"""Probe: compare two t2smap outputs (data only): identical voxels, max abs/rel difference."""

import sys

import nibabel as nb
import numpy as np

for a_path, b_path in zip(sys.argv[1::2], sys.argv[2::2], strict=True):
    a = np.asanyarray(nb.load(a_path).dataobj)
    b = np.asanyarray(nb.load(b_path).dataobj)
    same = (a == b) | (np.isnan(a) & np.isnan(b))
    d = np.abs(a.astype(np.float64) - b.astype(np.float64))
    fin = np.isfinite(d)
    rel = d[fin] / np.maximum(np.abs(b.astype(np.float64))[fin], 1e-30)
    print(
        a_path.split("/")[-1],
        a.dtype,
        b.dtype,
        a.shape,
        "identical",
        int(same.sum()),
        "/",
        a.size,
        "max abs",
        float(d[fin].max()) if fin.any() else None,
        "p99 rel",
        float(np.percentile(rel, 99)) if rel.size else None,
        "n rel>1e-3",
        int((rel > 1e-3).sum()),
    )

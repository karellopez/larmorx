# SPDX-License-Identifier: Apache-2.0
"""Probe: summarise every NIfTI output of a t2smap run (dtype, shape, header fields, values)."""

import glob
import os
import sys

import nibabel as nb
import numpy as np

d = sys.argv[1]
ref = sys.argv[2] if len(sys.argv) > 2 else None
fields = [
    "datatype",
    "bitpix",
    "dim",
    "pixdim",
    "xyzt_units",
    "qform_code",
    "sform_code",
    "scl_slope",
    "scl_inter",
    "cal_min",
    "cal_max",
    "descrip",
    "intent_code",
    "slice_code",
    "toffset",
    "quatern_b",
    "quatern_c",
    "quatern_d",
    "qoffset_x",
    "vox_offset",
    "dim_info",
]
if ref:
    h = nb.load(ref).header
    print(
        "REF",
        os.path.basename(ref),
        {f: h[f].tolist() if hasattr(h[f], "tolist") else h[f] for f in fields},
    )
for f in sorted(glob.glob(os.path.join(d, "*.nii*"))):
    img = nb.load(f)
    h = img.header
    a = np.asanyarray(img.dataobj)
    with open(f, "rb") as fh:
        gz = fh.read(10)
    print(
        "==",
        os.path.basename(f),
        a.dtype,
        a.shape,
        "gzip header flags/mtime/xfl/os:",
        gz[3],
        int.from_bytes(gz[4:8], "little"),
        gz[8],
        gz[9],
    )
    print("   ", {k: (h[k].tolist() if hasattr(h[k], "tolist") else h[k]) for k in fields})
    fin = np.isfinite(a) if a.dtype.kind == "f" else np.ones(a.shape, bool)
    print(
        "    nonzero",
        int(np.count_nonzero(a)),
        "nan",
        int(np.isnan(a).sum()) if a.dtype.kind == "f" else 0,
        "min/max",
        (a[fin].min(), a[fin].max()) if fin.any() else None,
    )
    print("    extensions", len(h.extensions))

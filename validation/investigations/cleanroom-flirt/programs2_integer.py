# SPDX-License-Identifier: Apache-2.0
"""applyxfm4D black-box probes on generated inputs (an int16 series, a uint16 one, a blur
question): FSL (oracle cache) against the original in both modes. Run boxed."""

import shutil
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

inp = rep.inputs()
epi = nib.load(str(inp / "epi.nii"))
d = np.asanyarray(epi.dataobj).astype(np.float64)
rot = "0.996 -0.087 0 3\n0.087 0.996 0 -2\n0 0 1 1.5\n0 0 0 1\n"


def series(path, dtype, scale):
    a = np.stack([d * scale, d * scale * 0.9 + 3], axis=-1).astype(dtype)
    img = nib.Nifti1Image(a, epi.affine, epi.header)
    img.set_data_dtype(dtype)
    img.header["scl_slope"] = 1
    img.header["scl_inter"] = 0
    nib.save(img, str(path))


cases = [
    ("i16-spline", "int16", 0.05, ["-singlematrix", "-interp", "spline"]),
    ("i16-spline-3d", "int16", 0.05, ["-3D", "-interp", "spline"]),
    ("i16-sinc", "int16", 0.05, ["-singlematrix"]),
    ("i16-tri", "int16", 0.05, ["-singlematrix", "-interp", "trilinear"]),
    ("u16-tri", "uint16", 0.05, ["-singlematrix", "-interp", "trilinear"]),
    ("u8-nn-v", "uint8", 0.002, ["-singlematrix", "-interp", "nn", "-v"]),
    ("blur-t1", None, 0, ["-singlematrix", "-interp", "trilinear"]),
]
for name, dt, sc, extra in cases:
    with (
        tempfile.TemporaryDirectory() as a_,
        tempfile.TemporaryDirectory() as b_,
        tempfile.TemporaryDirectory() as c_,
    ):
        dirs = [Path(a_), Path(b_), Path(c_)]
        for x in dirs:
            if dt is None:
                shutil.copy(inp / "t1.nii", x / "in.nii")
            else:
                series(x / "in.nii", dt, sc)
            shutil.copy(inp / "epi.nii", x / "ref.nii")
            (x / "m.mat").write_text(rot)
        args = ["in.nii", "ref.nii", "out.nii.gz", "m.mat", *extra]
        f = rep.run_oracle("applyxfm4D", args, cwd=dirs[0], settings={})
        res = []
        for mode, x in ((defects.REPRODUCE, dirs[1]), (defects.FIX, dirs[2])):
            o = orig.run_original("applyxfm4D", args, cwd=x, settings={}, mode=mode)
            res.append(o)
        fa = nib.load(str(dirs[0] / "out.nii.gz")) if (dirs[0] / "out.nii.gz").exists() else None
        print(
            f"== {name}: exit FSL {f.code}, original {res[0].code}/{res[1].code}; FSL out {f.stdout!r} err {f.stderr[:120]!r}; orig out {orig._own(res[0].stdout)!r}"
        )
        print(
            "   notices reproduce:",
            defects.notices_in(res[0].stderr),
            "fix:",
            defects.notices_in(res[1].stderr),
        )
        if fa is None:
            continue
        va = np.asanyarray(fa.dataobj).astype(float)
        for label, x in (("reproduce", dirs[1]), ("fix", dirs[2])):
            fb = nib.load(str(x / "out.nii.gz"))
            vb = np.asanyarray(fb.dataobj).astype(float)
            diff = va != vb
            ok = np.isfinite(va) & np.isfinite(vb)
            same = orig._same_streams(dirs[0] / "out.nii.gz", x / "out.nii.gz", blank_descrip=True)
            print(
                f"   {label}: dtype {fa.get_data_dtype()}/{fb.get_data_dtype()} bit-identical {same}, differ {int(diff.sum())}/{diff.size} max {np.abs(va - vb).max():.4g} NCC {orig._ncc(np.where(ok, va, 0), np.where(ok, vb, 0), np.ones(va.shape, bool)):.8f}"
            )

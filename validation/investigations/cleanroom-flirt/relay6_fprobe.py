# SPDX-License-Identifier: Apache-2.0
"""fprobe.py <outdir> [names...]: formless-reference probes (FSL through the oracle cache, use mode;
the original with reproduce): prints FSL's and the original's output codes, sform, quaternion."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=True)
rng = np.random.default_rng(7)
shape = (12, 10, 8)
data = (rng.random(shape) * 100).astype(np.float32)
data[3:9, 3:7, 2:6] += 200
c, s = np.cos(0.2), np.sin(0.2)
oblique = np.array(
    [
        [c * 1.5, -s * 2.0, 0.1, -20.0],
        [s * 1.5, c * 2.0, 0.05, 10.0],
        [0.0, 0.1, 2.5, -5.0],
        [0, 0, 0, 1],
    ]
)
qmat = np.array([[1.5, 0, 0, -18.0], [0, 2.0, 0, 11.0], [0, 0, 2.5, -4.0], [0, 0, 0, 1]])
neuro = oblique.copy()
neuro[:, 0] *= -1
neuro[0, 3] = 15.0
A = "0.99 -0.1 0.02 1.5\n0.1 0.98 -0.03 -2.0\n-0.02 0.03 1.01 0.5\n0 0 0 1\n"
R = "-1 0 0 10\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"
inputs = {
    "s2q1": (oblique, 2, qmat, 1),
    "s0q1": (None, 0, oblique, 1),
    "s1q0": (oblique, 1, None, 0),
    "neuro": (neuro, 1, None, 0),
    "none": (None, 0, None, 0),
}


def write(path, sform, scode, qform, qcode, pixdim=(1.5, 2.0, 2.5)):
    img = nib.Nifti1Image(data, None)
    h = img.header
    h.set_zooms(pixdim)
    if sform is not None:
        img.set_sform(sform, scode)
    else:
        img.set_sform(np.diag([*list(pixdim), 1]), 0)
    if qform is not None:
        img.set_qform(qform, qcode)
    else:
        img.set_qform(np.diag([*list(pixdim), 1]), 0)
    nib.save(img, str(path))


programs = {
    "a4d": (
        "applyxfm4D",
        ["in.nii", "ref.nii", "out", "m.mat", "-singlematrix", "-3D", "-interp", "trilinear"],
    ),
    "apply": (
        "flirt",
        [
            "-in",
            "in.nii",
            "-ref",
            "ref.nii",
            "-applyxfm",
            "-init",
            "m.mat",
            "-out",
            "out",
            "-interp",
            "trilinear",
        ],
    ),
    "reg": (
        "flirt",
        [
            "-in",
            "in.nii",
            "-ref",
            "ref.nii",
            "-init",
            "m.mat",
            "-nosearch",
            "-dof",
            "6",
            "-out",
            "out",
            "-omat",
            "o.mat",
        ],
    ),
}
sel = sys.argv[2:]
for iname, (sf, sc, qf, qc) in inputs.items():
    for mname, mtext in (("A", A), ("R", R)):
        for pname, (prog, args) in programs.items():
            name = f"{pname}-{iname}-{mname}"
            if sel and not any(x in name for x in sel):
                continue
            if mname == "R" and pname == "reg":
                continue
            res = {}
            for who in ("fsl", "rep"):
                d = root / name / who
                d.mkdir(parents=True, exist_ok=True)
                write(d / "in.nii", sf, sc, qf, qc)
                ref = nib.Nifti1Image(np.zeros((14, 9, 7), np.float32), None)
                ref.header.set_zooms((2.0, 2.0, 2.0))
                ref.set_sform(np.diag([2.0, 2, 2, 1]), 0)
                ref.set_qform(np.diag([2.0, 2, 2, 1]), 0)
                nib.save(ref, str(d / "ref.nii"))
                (d / "m.mat").write_text(mtext)
                st = {"FSLOUTPUTTYPE": "NIFTI"}
                r = (
                    rep.run_oracle(prog, args, cwd=d, settings=st)
                    if who == "fsl"
                    else orig.run_original(prog, args, cwd=d, settings=st, mode=defects.REPRODUCE)
                )
                if r.code != 0:
                    res[who] = f"exit {r.code} {r.stderr[:150]}"
                    continue
                o = nib.load(str(d / "out.nii"))
                h = o.header
                sr = np.array([h["srow_x"], h["srow_y"], h["srow_z"]], dtype=np.float32)
                q = np.array(
                    [
                        h["quatern_b"],
                        h["quatern_c"],
                        h["quatern_d"],
                        h["qoffset_x"],
                        h["qoffset_y"],
                        h["qoffset_z"],
                        h["pixdim"][0],
                    ],
                    dtype=np.float32,
                )
                res[who] = (
                    int(h["qform_code"]),
                    int(h["sform_code"]),
                    sr,
                    q,
                    np.asarray(o.dataobj),
                )
            f, g = res.get("fsl"), res.get("rep")
            if isinstance(f, str) or isinstance(g, str):
                print(
                    name,
                    "FSL:",
                    f if isinstance(f, str) else "ok",
                    "| orig:",
                    g if isinstance(g, str) else "ok",
                )
                continue
            same_s = np.array_equal(f[2].view(np.uint32), g[2].view(np.uint32))
            same_q = np.array_equal(f[3].view(np.uint32), g[3].view(np.uint32))
            same_d = np.array_equal(f[4], g[4])
            print(
                f"{name}: codes FSL q{f[0]} s{f[1]} orig q{g[0]} s{g[1]}; sform same {same_s}; quatern same {same_q}; data same {same_d}"
            )
            if not (same_s and same_q):
                print("   FSL sform", np.round(f[2], 4).tolist(), "q", f[3].tolist())
                print("   orig sform", np.round(g[2], 4).tolist(), "q", g[3].tolist())

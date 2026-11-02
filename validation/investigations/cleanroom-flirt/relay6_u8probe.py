# SPDX-License-Identifier: Apache-2.0
"""u8probe.py <outdir>: uint8 black-box probes of applyxfm4D and flirt -applyxfm (FSL through the
oracle cache, use mode); prints FSL's and the original's (reproduce, default) output rows."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=True)
aff = np.diag([1.0, 1.0, 1.0, 1.0])


def image(vals, dtype=np.uint8):
    a = np.zeros((10, 6, 6), np.float64)
    for x, v in enumerate(vals):
        a[x] = v
    return a


def write(d, name, a, dtype=np.uint8):
    img = nib.Nifti1Image(a.astype(dtype), aff)
    img.header.set_data_dtype(dtype)
    img.header.set_sform(aff, 1)
    img.header.set_qform(aff, 1)
    nib.save(img, str(d / name))


shift = "1 0 0 0.4\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"
probes = []
# x profile: border values first two and last two
profiles = {
    "lo": [10, 10, 20, 40, 60, 80, 100, 120, 10, 10],
    "hi": [10, 10, 100, 140, 180, 220, 250, 120, 10, 10],
    "bg255": [255, 255, 100, 140, 180, 220, 250, 120, 255, 255],
}
profiles["i16"] = [0, 0, 30000, 32000, 32767, 0, -32768, -30000, 0, 0]
profiles["i16b"] = [0, 0, 3000, 32767, 32767, 32767, 0, 0, 0, 0]
for pname, prof in profiles.items():
    for interp in ("nearestneighbour", "trilinear", "sinc", "spline"):
        probes.append(
            (
                f"a4d-{pname}-{interp}",
                "applyxfm4D",
                ["in.nii", "ref.nii", "out", "m.mat", "-singlematrix", "-3D", "-interp", interp],
                prof,
            )
        )
        probes.append(
            (
                f"flirt-{pname}-{interp}",
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
                    interp,
                    "-noresampblur",
                ],
                prof,
            )
        )
sel = sys.argv[2:] or None
for name, prog, args, prof in probes:
    if sel and not any(s in name for s in sel):
        continue
    rows = {}
    for who in ("fsl", "rep", "fix"):
        d = root / name / who
        d.mkdir(parents=True, exist_ok=True)
        a = image(prof)
        dt = np.int16 if "-i16" in name else np.uint8
        write(d, "in.nii", a, dt)
        write(d, "ref.nii", np.zeros_like(a), dt)
        (d / "m.mat").write_text(shift)
        st = {"FSLOUTPUTTYPE": "NIFTI"}
        if who == "fsl":
            r = rep.run_oracle(prog, args, cwd=d, settings=st)
        else:
            r = orig.run_original(
                prog,
                args,
                cwd=d,
                settings=st,
                mode=defects.REPRODUCE if who == "rep" else defects.FIX,
            )
        if r.code != 0:
            print(name, who, "exit", r.code, r.stderr[:200])
            continue
        o = nib.load(str(d / "out.nii"))
        rows[who] = (np.asarray(o.dataobj)[:, 3, 3], o.get_data_dtype())
    print(f"{name}: in {prof}")
    for who, (row, odt) in rows.items():
        print(f"   {who:4s} {odt}: {row.tolist()}")

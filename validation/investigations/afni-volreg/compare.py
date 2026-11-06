# SPDX-License-Identifier: Apache-2.0
"""Runs AFNI's 3dvolreg and larmorx's replica on one case and compares them.

Usage: compare.py <input.nii.gz> <mode fix|reproduce> [3dvolreg options ...]
Each run happens in its own folder under <input's folder>/cmp/<name>/ with the input linked;
the options may name the outputs (-prefix out.nii.gz -1Dfile p.1D ...). Prints the exit
statuses, the messages that differ, the text files that differ, and how the NIfTI outputs
compare (bit-identical or the largest difference).
"""

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

REPO = Path(__file__).resolve().parents[3]
WS = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_VOLREG", WS / "oracles/afni-26.3.05/bin/3dvolreg"))
LARMORX = Path(os.environ.get("LARMORX_BIN", REPO / "target/release/larmorx"))

inp = Path(sys.argv[1]).resolve()
mode = sys.argv[2]
opts = sys.argv[3:]
root = inp.parent / "cmp"
res = {}
for name, cmd in (("afni", [str(AFNI)]), ("lx", [str(LARMORX), "afni", "3dvolreg"])):
    d = root / name
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    for other in inp.parent.iterdir():
        if other.is_file() and other.name.endswith((".nii", ".nii.gz")):
            (d / other.name).symlink_to(other)
    env = {
        "HOME": str(d),
        "PATH": "/usr/bin:/bin",
        "LARMORX_IMPLEMENTATION": "replica",
        "LARMORX_UPSTREAM_DEFECTS": mode,
        "OMP_NUM_THREADS": "1",
    }
    p = subprocess.run([*cmd, *opts, inp.name], cwd=d, env=env, capture_output=True)
    res[name] = (
        p.returncode,
        p.stdout.decode(errors="replace"),
        p.stderr.decode(errors="replace"),
        d,
    )

(ca, oa, ea, da), (cl, ol, el, dl) = res["afni"], res["lx"]
print(f"exit: afni {ca}, larmorx {cl}")

ansi = re.compile(r"\x1b\[[0-9;]*m")
ea = ansi.sub("", ea)
el = ansi.sub("", el)
skip = (
    "** Program compile date",
    "++ 3dvolreg: AFNI version",
    "*+ WARNING: upstream defect",
    "++ implementation:",
    "++ upstream defects:",
)
la = [x for x in ea.splitlines() if not x.startswith(skip)]
ll = [x for x in el.splitlines() if not x.startswith(skip)]
if la != ll:
    print("stderr differs:\n--- afni\n" + "\n".join(la) + "\n--- larmorx\n" + "\n".join(ll))
if oa != ol:
    print("stdout differs:\n--- afni\n" + oa + "\n--- larmorx\n" + ol)
for f in sorted(da.iterdir()):
    if f.name == ".afni.log" or (inp.parent / f.name).exists():
        continue
    g = dl / f.name
    if not g.exists():
        print(f"missing in larmorx: {f.name}")
        continue
    if f.name.endswith((".nii", ".nii.gz")):
        a = nib.load(f)
        b = nib.load(g)
        x = np.asanyarray(a.dataobj)
        y = np.asanyarray(b.dataobj)
        if x.dtype != y.dtype or x.shape != y.shape:
            print(f"{f.name}: dtype/shape {x.dtype}{x.shape} vs {y.dtype}{y.shape}")
            continue
        if x.tobytes() == y.tobytes():
            print(f"{f.name}: data bit-identical {x.dtype}{x.shape}")
        else:
            dd = np.abs(x.astype(np.float64) - y.astype(np.float64))
            nd = int((x != y).sum())
            print(f"{f.name}: {nd} of {x.size} voxels differ, max |d| {dd.max():.3g}")
        if not np.allclose(a.affine, b.affine, atol=0, rtol=0):
            print(f"{f.name}: affine differs\n{a.affine}\n{b.affine}")
        ha, hb = a.header, b.header
        fields = [
            "dim", "pixdim", "xyzt_units", "datatype", "slice_code", "slice_start",
            "slice_end", "slice_duration", "toffset", "qform_code", "sform_code",
            "quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z",
            "srow_x", "srow_y", "srow_z", "scl_slope", "scl_inter", "dim_info",
        ]
        hd = [k for k in fields if np.asarray(ha[k]).tobytes() != np.asarray(hb[k]).tobytes()
              and not (np.all(np.isnan(ha[k])) and np.all(np.isnan(hb[k])))]
        if hd:
            print(f"{f.name}: header differs in " + ", ".join(
                f"{k} ({ha[k]} vs {hb[k]})" for k in hd))
    else:
        if f.read_bytes() != g.read_bytes():
            print(f"{f.name} differs:\n--- afni\n{f.read_text()}--- larmorx\n{g.read_text()}")
        else:
            print(f"{f.name}: identical")
for g in sorted(dl.iterdir()):
    if not (da / g.name).exists() and g.name != ".afni.log":
        print(f"only in larmorx: {g.name}")

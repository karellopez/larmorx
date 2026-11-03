# SPDX-License-Identifier: Apache-2.0
"""Runs one case of the fsl-maths-replica suite into ``<out>/{fsl,rep}`` (FSL through the oracle
cache) and prints the header fields that differ and the first differing values.

Usage: ``python -P one_case.py <case> [fix|reproduce] [--out DIR]`` (default DIR: a ``case``
directory beside this script's working directory), boxed: ``scripts/orchestration/boxed.sh 4``."""

import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths_replica as fm

argv = sys.argv[1:]
out_dir = Path("case")
if "--out" in argv:
    k = argv.index("--out")
    out_dir = Path(argv[k + 1])
    del argv[k : k + 2]
name = argv[0]
mode = argv[1] if len(argv) > 1 else defects.REPRODUCE
run = fm._cases()[name][2]
base = out_dir
shutil.rmtree(base, ignore_errors=True)
a, b = base / "fsl", base / "rep"
a.mkdir(parents=True)
b.mkdir(parents=True)
args = fm._prepare(run, a)
fm._prepare(run, b)
o = fm.run_oracle(args, cwd=a, settings=dict(run.settings))
r = fm.run_replica("fslmaths", args, cwd=b, settings=dict(run.settings), mode=mode)
print("exit", o.code, r.code)
print("FSL out:", o.stdout[-300:], "| err:", o.stderr[-300:])
print("REP out:", r.stdout[-300:], "| err:", r.stderr[-300:])
for f in sorted(a.glob("out*")):
    g = b / f.name
    if not g.exists():
        print("missing", f.name)
        continue
    if f.name.endswith((".nii.gz", ".nii", ".hdr")):
        ha, hb = nib.load(str(f)).header, nib.load(str(g)).header
        for k in ha:
            va, vb = np.asarray(ha[k]), np.asarray(hb[k])
            if va.tobytes() != vb.tobytes() and k != "descrip":
                print("header", k, va, vb)
        da = np.asarray(nib.load(str(f)).dataobj)
        db = np.asarray(nib.load(str(g)).dataobj)
        if da.tobytes() != db.tobytes():
            fa, fb = np.ascontiguousarray(da).reshape(-1), np.ascontiguousarray(db).reshape(-1)
            ua = fa.view(f"u{fa.itemsize}")
            ub = fb.view(f"u{fb.itemsize}")
            k = np.nonzero(ua != ub)[0]
            print("data differ:", da.dtype, len(k), "of", fa.size)
            for j in k[:6]:
                print("  ", j, fa[j], fb[j], hex(int(ua[j])), hex(int(ub[j])))

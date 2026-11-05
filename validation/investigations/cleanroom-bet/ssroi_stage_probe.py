# SPDX-License-Identifier: Apache-2.0
"""ssroi_stage_probe.py <outdir> <case id>: the BET suite's stage-by-stage run of a
standard_space_roi case, kept for inspection: FSL's matrix (from its ``-d`` run, through the
oracle cache) given to FSL and to the clean-room original (``-d -applyxfm -init``, reproduce),
then every image's header fields that differ (NIfTI-1 fields by name) and its values (catch-up
relay 5, 2026-11-05)."""

import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import fsl_bet as b

b.IMPLEMENTATION["name"] = "original"
root = Path(sys.argv[1])
cid = sys.argv[2]
s = next(sc for c, _cat, _d, sc in b._scenarios() if c == cid)
args = b._arguments(s, [b.input_path(n) for n in s.inputs])
env = b.environment(s.output_type, s.env)
with_d = args if "-d" in args[2:] else [args[0], args[1], "-d", *args[2:]]
d_dir = root / "fsl-d"
b._prepare(d_dir, s)
if not (d_dir / "out_tmp_to_std.mat").is_file():
    b.oracle_run(s.tool, with_d, env, d_dir, never=b.case_never(s))
stage = [*with_d, "-applyxfm", "-init", "fsl-matrix.mat"]
dirs = {"fsl": root / "fsl-stage", "larmorx": root / "stage"}
for d in dirs.values():
    if d.exists():
        shutil.rmtree(d)
    b._prepare(d, s)
    shutil.copyfile(d_dir / "out_tmp_to_std.mat", d / "fsl-matrix.mat")
f = b.oracle_run(s.tool, stage, env, dirs["fsl"], never=b.case_never(s))
r = b.run_larmorx(s.tool, stage, "reproduce", env, dirs["larmorx"])
print(f"FSL {f.code}, larmorx {r.code}")
for p in sorted(dirs["fsl"].glob("out*.nii*")):
    q = dirs["larmorx"] / p.name
    ha, hb = nib.load(str(p)).header, nib.load(str(q)).header
    diffs = []
    for k in ha:
        if k == "descrip":
            continue
        va, vb = np.asarray(ha[k]), np.asarray(hb[k])
        same = va.shape == vb.shape and (
            np.array_equal(va, vb, equal_nan=True)
            if va.dtype.kind == "f"
            else np.array_equal(va, vb)
        )
        if not same:
            diffs.append(f"{k}: FSL {va.tolist()} larmorx {vb.tolist()}")
    da = np.asanyarray(nib.load(str(p)).dataobj)
    db = np.asanyarray(nib.load(str(q)).dataobj)
    n = int(np.count_nonzero(da != db)) if da.shape == db.shape else -1
    print(f"{p.name}: {n} values differ; header: {'; '.join(diffs) or 'the same'}")

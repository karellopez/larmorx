# SPDX-License-Identifier: Apache-2.0
"""dcmp.py <seed> <n> [outdir]: an original difftest case run by hand (FSL with the cache off,
the original in reproduce), both directories kept in <outdir>/<n>/{fsl,orig}; prints the
arguments, the exit statuses, the first differing stream lines, and per image the header fields
that differ (values) and the voxels that differ."""

import os
import sys
from pathlib import Path

import numpy as np

os.environ["LARMORX_ORACLE_CACHE_MODE"] = "off"
from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

seed, n = sys.argv[1], int(sys.argv[2])
out = Path(sys.argv[3] if len(sys.argv) > 3 else "/tmp/dcmp") / str(n)
recipe = core.make_recipe(dt.ORIGINAL, seed, n)
run = dt.case(recipe, "original").payload[1]
a, b = out / "fsl", out / "orig"
for d in (a, b):
    d.mkdir(parents=True, exist_ok=True)
args = rep._prepare(run, a)
rep._prepare(run, b)
settings = dict(run.settings)
print(run.program, " ".join(args), settings.get("FSLOUTPUTTYPE"))
fo = rep.run_oracle(run.program, args, cwd=a, settings=settings)
lo = orig.run_original(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
print("exit", fo.code, lo.code)
for nm, r in (("fsl", fo), ("orig", lo)):
    (out / f"{nm}.stdout").write_text(r.stdout)
    (out / f"{nm}.stderr").write_text(r.stderr)
for s in ("stdout", "stderr"):
    x, y = getattr(fo, s).splitlines(), orig._own(getattr(lo, s)).splitlines()
    if x != y:
        i = next((k for k, (p, q) in enumerate(zip(x, y)) if p != q), min(len(x), len(y)))
        print(f"{s} differs at line {i + 1} ({len(x)} vs {len(y)} lines):")
        print("   FSL :", x[i] if i < len(x) else "<end>")
        print("   orig:", y[i] if i < len(y) else "<end>")
for f in sorted(a.iterdir()):
    g = b / f.name
    if not g.exists() or f.suffix in (".mat", ".txt") or f.name in {Path(x).name for x in args}:
        if f.suffix == ".mat" and g.exists() and f.read_text() != g.read_text():
            print(f.name, "FSL:\n" + f.read_text() + "orig:\n" + g.read_text())
        continue
    if f.name.endswith((".nii", ".nii.gz", ".hdr")):
        import nibabel as nib

        try:
            ha, hb = nib.load(str(f)).header, nib.load(str(g)).header
        except Exception as e:
            print(f.name, "unreadable", e)
            continue
        for k in ha.keys():
            va, vb = np.asarray(ha[k]), np.asarray(hb[k])
            if va.tobytes() != vb.tobytes() and k != "descrip":
                print(f"  {f.name} {k}: FSL {va.tolist()!r} orig {vb.tolist()!r}")
        da, db = np.asarray(nib.load(str(f)).dataobj), np.asarray(nib.load(str(g)).dataobj)
        if da.shape == db.shape:
            nd = int(np.sum(da != db))
            print(f"  {f.name} voxels differing: {nd} of {da.size}")

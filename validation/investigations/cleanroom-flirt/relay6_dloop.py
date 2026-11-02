# SPDX-License-Identifier: Apache-2.0
"""dloop.py <seed> <outdir> <n>...: original difftest cases run by hand (FSL through the oracle
cache, use mode; the original with reproduce); one line per case: exits, streams, images."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

seed, root = sys.argv[1], Path(sys.argv[2])
for n in map(int, sys.argv[3:]):
    recipe = core.make_recipe(dt.ORIGINAL, seed, n)
    run = dt.case(recipe, "original").payload[1]
    out = root / str(n)
    a, b = out / "fsl", out / "orig"
    for d in (a, b):
        d.mkdir(parents=True, exist_ok=True)
    args = rep._prepare(run, a)
    rep._prepare(run, b)
    settings = dict(run.settings)
    fo = rep.run_oracle(run.program, args, cwd=a, settings=settings)
    lo = orig.run_original(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
    parts = [f"exit {fo.code}/{lo.code}"]
    for nm, rr in (("fsl", fo), ("orig", lo)):
        (out / f"{nm}.stdout").write_text(rr.stdout)
        (out / f"{nm}.stderr").write_text(rr.stderr)
    for s in ("stdout", "stderr"):
        x, y = getattr(fo, s), orig._own(getattr(lo, s))
        if x.strip() != y.strip():
            parts.append(f"{s} differs")
    for f in sorted(a.iterdir()):
        g = b / f.name
        if not f.is_file():
            continue
        if not g.exists():
            parts.append(f"{f.name} only FSL")
            continue
        if f.name.endswith((".nii", ".nii.gz", ".hdr")):
            da, db = np.asarray(nib.load(str(f)).dataobj), np.asarray(nib.load(str(g)).dataobj)
            ha, hb = nib.load(str(f)).header, nib.load(str(g)).header
            hd = [
                k
                for k in ha
                if k != "descrip" and np.asarray(ha[k]).tobytes() != np.asarray(hb[k]).tobytes()
            ]
            if da.shape != db.shape or da.dtype != db.dtype:
                parts.append(f"{f.name} shape/type {da.shape}{da.dtype} vs {db.shape}{db.dtype}")
            else:
                nd = int(np.sum(da != db))
                mx = float(np.max(np.abs(da.astype(float) - db.astype(float)))) if nd else 0
                parts.append(
                    f"{f.name} {nd}/{da.size} differ (max {mx:g}) {da.dtype}"
                    + (f" hdr {hd}" if hd else "")
                )
        elif f.read_bytes() != g.read_bytes():
            parts.append(f"{f.name} bytes differ")
    print(n, run.program, " ".join(args)[:90], "|", "; ".join(parts), flush=True)

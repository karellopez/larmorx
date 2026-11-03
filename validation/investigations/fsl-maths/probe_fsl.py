# SPDX-License-Identifier: Apache-2.0
"""Runs FSL's fslmaths through the oracle-output cache (use mode) on a list of argument lines
(``@name`` names an input of the suite) and prints each exit status, FSL's messages and a summary
of each output (type, shape, NaN count, range, sum).

Usage: ``python -P probe_fsl.py probe_filters.txt``."""

import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import fsl_maths_replica as fm

CASES = [line.split() for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]

for args in CASES:
    run = fm._r(*args)
    with tempfile.TemporaryDirectory(prefix="lx-probe-") as t_:
        t = Path(t_)
        a = fm._prepare(run, t)
        o = fm.run_oracle(a, cwd=t, settings={})
        print("==", " ".join(args), "-> exit", o.code)
        for s in (o.stdout.strip(), o.stderr.strip()):
            if s:
                print("   |", s[-400:].replace("\n", "\n   | "))
        for f in sorted(t.glob("out*.nii.gz")):
            d = np.asarray(nib.load(str(f)).dataobj)
            fin = d[np.isfinite(d)] if d.dtype.kind == "f" else d
            nan = int(np.isnan(d).sum()) if d.dtype.kind == "f" else 0
            mn = fin.min() if fin.size else None
            mx = fin.max() if fin.size else None
            print(f"   {f.name}: {d.dtype} {d.shape} nan={nan} min={mn} max={mx} sum={fin.sum()}")

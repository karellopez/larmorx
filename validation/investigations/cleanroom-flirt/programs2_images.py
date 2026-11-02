# SPDX-License-Identifier: Apache-2.0
"""Each line of a command file (py/fsl.py's format): FSL (oracle cache) and the original
(reproduce)
for each image written, the header fields that differ and the values."""

import shlex
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip() or line.startswith("#"):
        continue
    words = shlex.split(line)
    files, args = [], []
    for w in words[1:]:
        if w.startswith("file:"):
            n, t = w[5:].split("=", 1)
            files.append((n, t.replace("\\n", "\n")))
        else:
            args.append(w)
    run = rep.Run(words[0], tuple(args), files=tuple(files))
    with tempfile.TemporaryDirectory() as a_, tempfile.TemporaryDirectory() as b_:
        a, b = Path(a_), Path(b_)
        x = rep._prepare(run, a)
        rep._prepare(run, b)
        f = rep.run_oracle(run.program, x, cwd=a, settings={})
        o = orig.run_original(run.program, x, cwd=b, settings={}, mode=defects.REPRODUCE)
        print(f"### {run.program} {' '.join(x)}: exit {f.code} {o.code}")
        for p in sorted(a.glob("out*")):
            q = b / p.name
            if not q.exists():
                print("  missing", p.name)
                continue
            ia, ib = nib.load(str(p)), nib.load(str(q))
            for k in ia.header:
                if k != "descrip" and orig._differs(ia.header[k], ib.header[k]):
                    print(f"  {k}: FSL {ia.header[k]} original {ib.header[k]}")
            da, db = (
                np.asanyarray(ia.dataobj).astype(float),
                np.asanyarray(ib.dataobj).astype(float),
            )
            if da.shape != db.shape:
                print("  shapes", da.shape, db.shape)
                continue
            d = ~((da == db) | (np.isnan(da) & np.isnan(db)))
            print(
                f"  values differ {int(d.sum())}/{d.size} same-bytes {orig._same_streams(p, q, blank_descrip=True)}"
            )

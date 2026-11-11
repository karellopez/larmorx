# SPDX-License-Identifier: Apache-2.0
"""The log-linear slopes numpy's lstsq gives (tedana's batched call) at the voxels whose
observations are all equal, per replayed case, written as JSON: {case: [[group, t, voxel, b1], ...]}.
Run under several environments and compare."""

import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model

out = {}
for case in sorted(Path(sys.argv[2]).glob("f*")):
    tout = case / "tedana/out"
    if not case.is_dir() or not (tout / "t2smap_call.sh").exists():
        continue
    a = model.parser().parse_args(check.call_words(tout))
    a.data = [
        str((case / "tedana" / p).resolve()) if not Path(p).is_absolute() else p for p in a.data
    ]
    try:
        c = model.prepare(a)
    except Exception:
        continue
    x, xu, count, tes = c["x"], c["x_used"], c["count"], c["tes"]
    ts = a.fitmode == "ts"
    rows = []
    for t in range(x.shape[2]) if ts else [None]:
        xx = x[:, :, t : t + 1] if ts else xu
        for n, members in model.groups_of(count):
            xg = xx[members, :n, :]
            nv, _, tu = xg.shape
            y = np.log(np.abs(xg) + 1).reshape(nv, n * tu)
            const = np.all(y == y[:, :1], axis=1)
            if not const.any():
                continue
            design = np.column_stack([np.ones(n * tu), -np.repeat(tes[:n], tu)])
            with np.errstate(all="ignore"):
                B = np.linalg.lstsq(design, y.T, rcond=None)[0]
            for k in np.flatnonzero(const):
                rows.append(
                    [
                        int(n),
                        -1 if t is None else int(t),
                        int(members[k]),
                        float(B[1, k]),
                        float(y[k, 0]),
                    ]
                )
    out[case.name] = rows
Path(sys.argv[3]).write_text(json.dumps(out))
print(sys.argv[3], sum(len(v) for v in out.values()), "voxels")

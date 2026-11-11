# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's calls: voxels whose log-linear observations are all equal, and the smallest
|slope| relative to the noise scale (numpy's lstsq, tedana's batches)."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model

for out in sys.argv[2:]:
    out = Path(out)
    a = model.parser().parse_args(check.call_words(out))
    c = model.prepare(a)
    xu, count, tes = c["x_used"], c["count"], c["tes"]
    n_const = 0
    small = []
    for n, members in model.groups_of(count):
        xg = xu[members, :n, :]
        nv, _, tu = xg.shape
        y = np.log(np.abs(xg) + 1).reshape(nv, n * tu)
        n_const += int(np.all(y == y[:, :1], axis=1).sum())
        design = np.column_stack([np.ones(n * tu), -np.repeat(tes[:n], tu)])
        B = np.linalg.lstsq(design, y.T, rcond=None)[0]
        scale = np.abs(y).max(axis=1) * 2.0**-52 / np.ptp(tes[:n])
        small.append(np.min(np.abs(B[1]) / np.maximum(scale, 1e-300)))
    print(
        out.parent.parent.name,
        "fit voxels",
        int((count >= 1).sum()),
        "equal observations",
        n_const,
        "min |b1| / (eps max|y| / TE range)",
        min(small),
    )

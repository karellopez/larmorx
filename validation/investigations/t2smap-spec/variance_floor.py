# SPDX-License-Identifier: Apache-2.0
"""For the variance maps' failing values: the difference against the rounding floor of a variance,
P_kk * m * (2^-24 max|y|)^2 / (m - 2), P = (J^T J)^-1 at the reference's written fit."""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model

for line in Path(sys.argv[2]).read_text(encoding="utf-8").splitlines():
    n, ref, oth = line.split()
    case = Path(sys.argv[3]) / n
    ro, oo = case / ref / "out", case / oth / "out"
    a = model.parser().parse_args(check.call_words(ro))
    c = model.prepare(a)
    xu, count, tes = c["x_used"], c["count"], c["tes"]
    idx = np.flatnonzero(c["mask"].ravel())
    pre = a.prefix if (a.prefix == "" or a.prefix.endswith("_")) else a.prefix + "_"
    fn = lambda d: model.name(d, a.convention, pre)  # noqa: E731, B023
    on = lambda arr: arr.reshape((-1, *arr.shape[3:]))[idx]  # noqa: E731, B023
    get = lambda o, d: on(np.asanyarray(nib.load(o / fn(d)).dataobj)).astype(np.float64)  # noqa: E731, B023
    t2 = get(ro, "t2star img") * 1000
    s0 = get(ro, "s0 img")
    worst = 0.0
    nbad = 0
    for desc, k in (
        ("t2star variance img", 0),
        ("s0 variance img", 1),
        ("t2star-s0 covariance img", 2),
    ):
        va, vb = get(ro, desc), get(oo, desc)
        bad = np.flatnonzero(
            (va != vb) & ~(np.abs(va - vb) <= 1e-4 * np.abs(va)) & np.isfinite(va) & np.isfinite(vb)
        )
        for v in bad:
            if a.fitmode == "ts":
                continue
            ncnt = max(int(count[v]), 2)
            y = xu[v, :ncnt, :].reshape(-1).astype(np.float64)
            te = np.repeat(tes[:ncnt], xu.shape[2])
            m = y.size
            if m <= 2:
                continue
            e = np.exp(-te / t2[v])
            J = np.column_stack([e, s0[v] * e * te / t2[v] ** 2])
            P = np.linalg.pinv(J.T @ J)
            floor = m * (2.0**-24 * np.abs(y).max()) ** 2 / (m - 2)
            # tedana: variance image 0 = P[0,0] (S0's), 1 = P[1,1] (T2*'s): TED-07; reproduce keeps it
            pk = [P[0, 0], P[1, 1], abs(P[0, 1])][k]
            ratio = abs(va[v] - vb[v]) / (pk * floor) if pk * floor > 0 else np.inf
            worst = max(worst, ratio)
            nbad += 1
    print(f"{n} {oth}: {nbad} failing values, largest diff / (P_kk floor) {worst:.3g}")

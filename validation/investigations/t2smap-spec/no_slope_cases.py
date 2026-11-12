# SPDX-License-Identifier: Apache-2.0
"""Per replayed case: the log-linear fit's voxels whose exact slope is 0 (rational arithmetic on
the float32 observations and float64 design), tedana's batched lstsq there, each voxel alone,
and the written T2* of tedana and of the original (reproduce, default)."""

import json
import sys
from fractions import Fraction
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model


def words(out):
    return check.call_words(out)


tot = dict(cases=0, exact0=0, const=0, pos=0, neg=0, zero=0, mism=0, alone_diff_sign=0, bad_repro=0)
rows = []
for case in sorted(Path(sys.argv[2]).glob("f*")):
    if not case.is_dir() or not (case / "tedana/out/t2smap_call.sh").exists():
        continue
    tout = case / "tedana/out"
    if not any(tout.glob("*.json")):
        rows.append(f"{case.name}: tedana did not complete")
        continue
    w = words(tout)
    a = model.parser().parse_args(w)
    a.data = [
        str((case / "tedana" / p).resolve()) if not Path(p).is_absolute() else p for p in a.data
    ]
    try:
        c = model.prepare(a)
    except Exception as e:
        rows.append(f"{case.name}: model {type(e).__name__}: {e}")
        continue
    x, xu, count, tes = c["x"], c["x_used"], c["count"], c["tes"]
    idx = np.flatnonzero(c["mask"].ravel())
    pre = a.prefix if (a.prefix == "" or a.prefix.endswith("_")) else a.prefix + "_"
    fn = lambda d: model.name(d, a.convention, pre)  # noqa: E731, B023
    on = lambda arr: arr.reshape((-1, *arr.shape[3:]))[idx]  # noqa: E731, B023

    def get(run, d, case=case, fn=fn, on=on):
        path = case / run / "out" / fn(d)
        return on(np.asanyarray(nib.load(path).dataobj)) if path.exists() else None

    ted, rep, dflt = (get(r, "t2star img") for r in ("tedana", "reproduce", "default"))
    ts = a.fitmode == "ts"
    cols = range(x.shape[2]) if ts else [None]
    n0 = nc = npos = nneg = nz = mism = ads = 0
    for t in cols:
        xx = x[:, :, t : t + 1] if ts else xu
        for n, members in model.groups_of(count):
            xg = xx[members, :n, :]
            nv, _, tu = xg.shape
            y = np.log(np.abs(xg) + 1).reshape(nv, n * tu)
            te = -np.repeat(tes[:n], tu)
            design = np.column_stack([np.ones(n * tu), te])
            with np.errstate(all="ignore"):
                B = np.linalg.lstsq(design, y.T, rcond=None)[0]
            tt = [Fraction(float(v)) for v in te]
            st = sum(tt)
            m = len(tt)
            for k in range(nv):
                yy = [Fraction(float(v)) for v in y[k]]
                if not all(np.isfinite(y[k])):
                    continue
                if m * sum(p * q for p, q in zip(tt, yy, strict=True)) - st * sum(yy) != 0:
                    continue
                n0 += 1
                const = bool(np.all(y[k] == y[k][0]))
                nc += const
                b1 = B[1, k]
                npos += b1 > 0
                nneg += b1 < 0
                nz += b1 == 0
                with np.errstate(all="ignore"):
                    alone = np.linalg.lstsq(design, y[k][:, None], rcond=None)[0][1, 0]
                ads += np.sign(alone) != np.sign(b1)
                v = members[k]
                kk = (v, t) if ts else v
                # tedana's written T2* from b1 (the floors: +inf -> 500 ms, <= 0 -> 1 ms; seconds)
                with np.errstate(all="ignore"):
                    t2 = 1 / b1
                exp_ms = 500.0 if t2 == np.inf else (1.0 if t2 <= 0 else t2)
                if (
                    count[v] >= 1
                    and abs(float(ted[kk]) - np.float32(exp_ms / 1000)) > 0
                    and not (exp_ms > 1e6)
                ):
                    mism += 1
    tot["cases"] += 1
    for k_, v_ in (
        ("exact0", n0),
        ("const", nc),
        ("pos", npos),
        ("neg", nneg),
        ("zero", nz),
        ("mism", mism),
        ("alone_diff_sign", ads),
    ):
        tot[k_] += v_
    rows.append(
        f"{case.name}: {a.fittype} {a.fitmode} groups {[n for n, _ in model.groups_of(count)]}: exact-zero slopes {n0} (equal observations {nc}); tedana's batched slope there + {npos} / 0 {nz} / - {nneg}; alone of another sign {ads}; written T2* not as the slope says {mism}"
    )
print("\n".join(rows))
print(json.dumps(tot))

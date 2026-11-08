# SPDX-License-Identifier: Apache-2.0
"""``specs/t2smap.md`` §17.3 between two ``t2smap`` output directories: tedana's (the reference)
and another run of the same command (an original, the model, or tedana in another setup).

Written from the spec only (a clean-room implementer may read and run it)::

    python -I check.py <reference out dir> <other out dir> [--cwd DIR] [--json FILE]

The command is read from the reference's ``t2smap_call.sh`` (relative paths resolve against
``--cwd``, by default the out dir's parent); the inputs are read again with ``model.py``'s
readers, for the curve fit's objective, the RMSE's predictions and the combination's weights.
The report gives, per output, the values compared, how many are identical, how many pass by each
rule, how many fail, and the largest difference as a fraction of its allowance ("use"): a check
passes when every value is within its allowance; the margins of §17.3 are the largest uses
measured on tedana's own spread. Exit status 0 when every check passes, 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import shlex
import sys
import warnings
from dataclasses import dataclass, field
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import model

F32, F64 = np.float32, np.float64
TOL_LOGLIN_REL = 2.0**-22  # two float32 units in the last place
TOL_RATE_ABS = 1e-9  # s^-1
TOL_CURVE_REL = 1e-6
TOL_OBJECTIVE = 1e-6
TOL_OBJECTIVE_ABS = 1e-12
TOL_SHARE_BC = 0.03  # at most this share of the fit voxels pass by (b) or (c) (measured 0.31 %)
TOL_VAR_REL = 1e-4  # measured 8.1e-6 where (a) holds (ds003592 with --verbose)
TOL_PRED_REL = 2.0**-21  # the predictions' float32 rounding from each run's own maps
SUM_UNIT = 2.0**-24  # a float32 sum of n terms in another order: SUM_UNIT * (n + SUM_SLACK) of it
SUM_SLACK = 32
TOL_TABLE_REL = 1e-6  # the percentiles' interpolation
TOL_COMB_REL = 2.0**-20  # eight float32 units (relay 8b: tedana's own spread used 0.99 of one)
TINY = 2.0**-149


@dataclass
class Result:
    name: str
    n: int = 0
    identical: int = 0
    by: dict[str, int] = field(default_factory=dict)
    failed: int = 0
    use: float = 0.0  # largest |difference| / allowance among the non-identical values
    notes: list[str] = field(default_factory=list)
    examples: list[str] = field(default_factory=list)  # the first failures (--details)

    def add(self, rule: str, k: int) -> None:
        if k:
            self.by[rule] = self.by.get(rule, 0) + int(k)

    @property
    def ok(self) -> bool:
        return self.failed == 0

    def line(self) -> str:
        by = ", ".join(f"{k} {v}" for k, v in sorted(self.by.items()))
        status = "PASS" if self.ok else "FAIL"
        extra = f"; {'; '.join(self.notes)}" if self.notes else ""
        return (
            f"{status} {self.name}: {self.n} values, {self.identical} identical"
            f"{', ' + by if by else ''}, {self.failed} failed, use {self.use:.3g}{extra}"
        )


def same_bits(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    if a.dtype.kind == "f":
        return (a == b) & (np.signbit(a) == np.signbit(b)) | (np.isnan(a) & np.isnan(b))
    return a == b


def load(path: Path) -> tuple[nib.Nifti1Image, np.ndarray]:
    img = nib.load(str(path))
    return img, np.asanyarray(img.dataobj)


def call_words(out: Path) -> list[str]:
    words = shlex.split((out / "t2smap_call.sh").read_text(encoding="utf-8"))[1:]
    out_words, skip = [], False
    for i, w in enumerate(words):
        if skip:
            skip = False
            continue
        if w == "--n-threads":
            skip = True
            continue
        if w.startswith("--n-threads="):
            continue
        out_words.append(w)
        del i
    return out_words


def rate(t2s_s: np.ndarray) -> np.ndarray:
    with np.errstate(divide="ignore"):
        return 1.0 / t2s_s.astype(F64)


def compare_maps(
    res: Result, a, b, tol_rel, rate_domain: bool, ok_extra=None, use_where=None
) -> np.ndarray:
    """Per value: identical, or within tol_rel (the rate domain for T2*: also 1e-9 s^-1);
    returns the mask of values that pass by the tolerance or are identical. The use is taken
    over ``use_where`` (all values by default)."""
    same = same_bits(a, b)
    res.n += a.size
    res.identical += int(same.sum())
    with np.errstate(all="ignore"):
        if rate_domain:
            ra, rb = rate(a), rate(b)
            allow = tol_rel * np.abs(ra) + TOL_RATE_ABS
            diff = np.abs(ra - rb)
        else:
            allow = tol_rel * np.abs(a.astype(F64))
            diff = np.abs(a.astype(F64) - b.astype(F64))
        within = ~same & (diff <= allow)
        with np.errstate(invalid="ignore", divide="ignore"):
            use = np.where(~same & np.isfinite(diff), diff / allow, 0.0)
    if use_where is not None:
        use = np.where(use_where, use, 0.0)
    if use.size:
        res.use = max(res.use, float(np.nanmax(np.where(within, use, 0.0))))
    passed = same | within
    if ok_extra is not None:
        extra = ~passed & ok_extra
        res.add("other rule", int(extra.sum()))
        passed = passed | extra
    res.add("tolerance", int(within.sum()))
    res.failed += int((~passed).sum())
    for i in np.flatnonzero(~passed.reshape(-1))[:8]:
        res.examples.append(
            f"[{i}] ref {a.reshape(-1)[i]!r} other {b.reshape(-1)[i]!r} "
            f"diff {diff.reshape(-1)[i]:.3g} allow {allow.reshape(-1)[i]:.3g}"
        )
    return passed


def cap_rule(res: Result, la, lb, fa, fb, count, tol, alt) -> np.ndarray:
    """§8's cap on the limited T2* map: c is an order statistic (the 99.5th percentile), so the
    two runs' caps differ by at most the largest difference of the values below them; a value
    capped on one side only lies near ten caps. Returns the mask of values accepted so."""
    two = count >= 2
    if la.ndim == 2:
        two = np.repeat(two[:, None], la.shape[1], axis=1)
    cap_a = two & ~same_bits(la, fa)
    cap_b = two & ~same_bits(lb, fb)
    ok = np.zeros(la.shape, dtype=bool)
    if not (cap_a.any() or cap_b.any()):
        return ok
    ua, ub = np.unique(la[cap_a]), np.unique(lb[cap_b])
    if ua.size != 1 or ub.size != 1:
        res.notes.append(f"caps not single values: {ua[:3]} / {ub[:3]}")
        return ok
    ca, cb = float(ua[0]), float(ub[0])
    rest = two & ~cap_a & ~cap_b
    lip = float(np.max(np.abs(la[rest].astype(F64) - lb[rest].astype(F64)), initial=0.0))
    allow = lip + tol * abs(ca)
    use = abs(ca - cb) / allow if allow > 0 else (0.0 if ca == cb else np.inf)
    res.notes.append(f"cap {ca!r} / {cb!r} s, allowance {allow:.3g}, use {use:.3g}")
    res.use = max(res.use, float(use) if np.isfinite(use) else 0.0)
    if use <= 1:
        ok |= cap_a & cap_b
        # capped on one side only: the other side's value near the cap's threshold, ten caps
        one = cap_a ^ cap_b
        v = np.where(cap_a, lb, la).astype(F64)
        near = np.abs(v - 10 * np.where(cap_a, ca, cb)) <= 10 * abs(ca - cb) + 10 * tol * abs(ca)
        ok |= one & (near | alt)
    return ok


def objective(x_v: np.ndarray, te: np.ndarray, s0: float, t2s_ms: float) -> float:
    with np.errstate(all="ignore"):
        r = s0 * np.exp(-te / t2s_ms) - x_v
    return float(np.sum(r * r))


def check(ref: Path, other: Path, cwd: Path) -> list[Result]:
    results: list[Result] = []
    words = call_words(ref)
    a = model.parser().parse_args(words)
    resolve = lambda p: str((cwd / p).resolve()) if p and not Path(p).is_absolute() else p  # noqa: E731
    a.data = [resolve(p) for p in a.data]
    a.mask = resolve(a.mask)
    c = model.prepare(a)
    x, x_used, count, tes, mask = c["x"], c["x_used"], c["count"], c["tes"], c["mask"]
    idx = np.flatnonzero(mask.ravel(order="C"))
    prefix = a.prefix if (a.prefix == "" or a.prefix.endswith("_")) else a.prefix + "_"

    def fname(desc: str) -> str:
        return model.name(desc, a.convention, prefix)

    # ---- files, the call, the registry, the JSON
    res = Result("files")
    fa = sorted(p.relative_to(ref).as_posix() for p in ref.rglob("*"))
    fb = sorted(p.relative_to(other).as_posix() for p in other.rglob("*"))
    res.n = len(fa)
    res.identical = len(set(fa) & set(fb))
    if fa != fb:
        res.failed = len(set(fa) ^ set(fb))
        res.notes.append(f"only in one: {sorted(set(fa) ^ set(fb))}")
    results.append(res)
    res = Result("call and JSON")
    res.n = 3
    if call_words(ref) == call_words(other):
        res.identical += 1
    else:
        res.failed += 1
        res.notes.append("t2smap_call.sh differs")
    reg = fname("registry json")
    if (ref / reg).read_bytes() == (other / reg).read_bytes():
        res.identical += 1
    else:
        res.failed += 1
        res.notes.append("registry differs")
    dd = fname("data description json")
    ja = json.loads((ref / dd).read_text(encoding="utf-8"))
    jb = json.loads((other / dd).read_text(encoding="utf-8"))
    for j in (ja, jb):
        for k in ("Node", "Python", "Python_Libraries", "Command"):
            j["GeneratedBy"][0].pop(k, None)
    if ja == jb and (ref / dd).read_bytes()[-1:] == (other / dd).read_bytes()[-1:]:
        res.identical += 1
    else:
        res.failed += 1
        res.notes.append("dataset_description differs")
    results.append(res)

    # ---- headers
    res = Result("image headers")
    images = sorted(p.name for p in ref.glob("*.nii.gz"))
    for n in images:
        if not (other / n).exists():
            continue
        ia, ib = nib.load(str(ref / n)), nib.load(str(other / n))
        res.n += 1
        if ia.header.binaryblock == ib.header.binaryblock:
            res.identical += 1
        else:
            res.failed += 1
            res.notes.append(f"{n} header differs")
    results.append(res)

    # ---- the adaptive mask
    res = Result("adaptive mask")
    _, ma = load(ref / fname("adaptive mask img"))
    _, mb = load(other / fname("adaptive mask img"))
    same = same_bits(ma, mb)
    res.n, res.identical, res.failed = ma.size, int(same.sum()), int((~same).sum())
    results.append(res)
    if res.failed:
        return results  # every later quantity depends on it

    on = lambda arr: arr.reshape((-1, *arr.shape[3:]))[idx]  # noqa: E731
    fv = count >= 1
    curve = a.fittype == "curvefit"
    ts = a.fitmode == "ts"

    t2a_s = on(load(ref / fname("t2star img"))[1]).astype(F64)  # written in seconds
    t2b_s = on(load(other / fname("t2star img"))[1]).astype(F64)
    t2a, t2b = t2a_s * 1000, t2b_s * 1000  # ms, as the echo times
    s0a = on(load(ref / fname("s0 img"))[1]).astype(F64)
    s0b = on(load(other / fname("s0 img"))[1]).astype(F64)

    # ---- the curve fit's alternatives (b), (c) per fit voxel (and volume in ts)
    alt = np.zeros(t2a.shape, dtype=bool)
    if curve:
        res_bc = Result("curvefit (b)/(c) share")
        t2_ll = np.zeros(t2a.shape)
        s0_ll = np.zeros(t2a.shape)
        cols = range(x.shape[2]) if ts else [None]
        for t in cols:
            xu = x[:, :, t : t + 1] if ts else x_used
            for n, members in model.groups_of(count):
                g_t2, g_s0 = model.loglin_group(xu[members, :n, :], tes, "exact")
                if ts:
                    t2_ll[members, t], s0_ll[members, t] = g_t2, g_s0
                else:
                    t2_ll[members], s0_ll[members] = g_t2, g_s0
        ra, rb = rate(t2a_s), rate(t2b_s)
        n_b = n_c = 0
        detail_b: list[float] = []
        for n, members in model.groups_of(count):
            for v in members:
                for t in cols:
                    k = (v, t) if ts else (v,)
                    if same_bits(t2a[k], t2b[k]) and same_bits(s0a[k], s0b[k]):
                        continue
                    da = abs(ra[k] - rb[k]) <= TOL_CURVE_REL * abs(ra[k]) + TOL_RATE_ABS
                    ds = abs(s0a[k] - s0b[k]) <= TOL_CURVE_REL * abs(s0a[k])
                    if da and ds:
                        continue
                    xv = (x[v, :n, t] if ts else x_used[v, :n, :]).reshape(-1).astype(F64)
                    te = np.repeat(tes[:n], 1 if ts else x_used.shape[2])
                    fa_ = objective(xv, te, s0a[k], t2a[k])
                    fb_ = objective(xv, te, s0b[k], t2b[k])
                    detail_b.append((fb_ / fa_ - 1) if fa_ > 0 else np.inf)
                    if fb_ <= fa_ * (1 + TOL_OBJECTIVE) + TOL_OBJECTIVE_ABS * float(xv @ xv):
                        alt[k] = True
                        n_b += 1
                        continue
                    with np.errstate(divide="ignore"):
                        r_start = 1000.0 / t2_ll[k]
                    near = abs(r_start) <= TOL_RATE_ABS or abs(s0_ll[k] - xv.min()) <= 1e-6 * abs(
                        xv.min()
                    )
                    if near:
                        alt[k] = True
                        n_c += 1
        n_fit = int(fv.sum()) * (x.shape[2] if ts else 1)
        res_bc.n = n_fit
        res_bc.identical = n_fit - n_b - n_c
        res_bc.add("(b) objective", n_b)
        res_bc.add("(c) start", n_c)
        share = (n_b + n_c) / max(n_fit, 1)
        res_bc.use = share / TOL_SHARE_BC
        res_bc.failed = int(share > TOL_SHARE_BC)
        res_bc.notes.append(f"share {share:.3%} of the fit voxels")
        if detail_b:
            q = np.quantile(detail_b, [0, 0.5, 1])
            res_bc.notes.append(
                f"F'/F - 1 of the {len(detail_b)} voxels outside (a): min {q[0]:.3g}, "
                f"median {q[1]:.3g}, max {q[2]:.3g}"
            )
        results.append(res_bc)

    # ---- the maps
    tol = TOL_CURVE_REL if curve else TOL_LOGLIN_REL
    for desc, rate_dom in (
        ("t2star img", True),
        ("limited t2star img", True),
        ("s0 img", False),
        ("limited s0 img", False),
    ):
        res = Result(desc)
        va = on(load(ref / fname(desc))[1])
        vb = on(load(other / fname(desc))[1])
        floored = np.zeros(va.shape, dtype=bool)
        if rate_dom:
            floors = np.array([1.0, 500.0, tes.max() / 36.04365338911715]) / 1000
            fa_ = np.isin(va.astype(F64), floors.astype(F32).astype(F64))
            fb_ = np.isin(vb.astype(F64), floors.astype(F32).astype(F64))
            ra, rb = rate(va), rate(vb)
            near = np.zeros(va.shape, dtype=bool)
            for fr in [0.0, *list(1.0 / floors)]:
                near |= (np.abs(ra - fr) <= TOL_RATE_ABS) | (np.abs(rb - fr) <= TOL_RATE_ABS)
            floored = (fa_ ^ fb_) & near
        cap_ok = np.zeros(va.shape, dtype=bool)
        if desc == "limited t2star img":
            cap_ok = cap_rule(
                res,
                va,
                vb,
                on(load(ref / fname("t2star img"))[1]),
                on(load(other / fname("t2star img"))[1]),
                count,
                tol,
                alt,
            )
        compare_maps(res, va, vb, tol, rate_dom, ok_extra=floored | alt | cap_ok)
        results.append(res)

    # ---- failures and variances
    if curve:
        res = Result("fit failures")
        fa_ = on(load(ref / fname("fit failures img"))[1])
        fb_ = on(load(other / fname("fit failures img"))[1])
        same = fa_ == fb_
        res.n, res.identical = fa_.size, int(same.sum())
        res.add("(b)/(c)", int((~same & alt).sum()))
        res.failed = int((~same & ~alt).sum())
        results.append(res)
        if a.verbose:
            ra, rb = rate(t2a_s), rate(t2b_s)
            with np.errstate(invalid="ignore"):
                exact_ok = (np.abs(ra - rb) <= TOL_CURVE_REL * np.abs(ra) + TOL_RATE_ABS) & (
                    np.abs(s0a - s0b) <= TOL_CURVE_REL * np.abs(s0a)
                )
            for desc in ("t2star variance img", "s0 variance img", "t2star-s0 covariance img"):
                res = Result(desc)
                va = on(load(ref / fname(desc))[1])
                vb = on(load(other / fname(desc))[1])
                passed = compare_maps(
                    res, va, vb, TOL_VAR_REL, False, ok_extra=~exact_ok, use_where=exact_ok
                )
                del passed
                results.append(res)

    # ---- the RMSE map and the table: allowances from the two runs' written limited maps
    lt_a = on(load(ref / fname("limited t2star img"))[1]).astype(F64) * 1000
    lt_b = on(load(other / fname("limited t2star img"))[1]).astype(F64) * 1000
    ls_a = on(load(ref / fname("limited s0 img"))[1]).astype(F64)
    ls_b = on(load(other / fname("limited s0 img"))[1]).astype(F64)
    n_vox, _, n_t = x.shape
    bound = np.zeros((n_vox, n_t))
    pmax = np.zeros((n_vox, n_t))
    for n in range(2, x.shape[1] + 1):
        v = np.flatnonzero(count == n)
        if not v.size:
            continue
        with np.errstate(all="ignore"):
            if ts:
                pa = (
                    ls_a[v][:, None, :] * np.exp(-tes[None, :n, None] / lt_a[v][:, None, :])
                ).astype(F32)
                pb = (
                    ls_b[v][:, None, :] * np.exp(-tes[None, :n, None] / lt_b[v][:, None, :])
                ).astype(F32)
            else:
                pa = (ls_a[v][:, None] * np.exp(-tes[None, :n] / lt_a[v][:, None])).astype(F32)[
                    ..., None
                ]
                pb = (ls_b[v][:, None] * np.exp(-tes[None, :n] / lt_b[v][:, None])).astype(F32)[
                    ..., None
                ]
            d = np.where(same_bits(pa, pb), 0.0, pa.astype(F64) - pb.astype(F64))
            d = np.where(np.isnan(d), np.inf, d)  # NaN on one side only: no allowance
            bound[v] = np.sqrt(np.mean(d * d, axis=1)) * np.ones(n_t)
            pmax[v] = np.nanmax(np.abs(pa.astype(F64)), axis=1) * np.ones(n_t)
    res = Result("rmse img")
    ra_ = on(load(ref / fname("rmse img"))[1]).astype(F64)
    rb_ = on(load(other / fname("rmse img"))[1]).astype(F64)
    same = same_bits(ra_, rb_)
    n_e = x.shape[1]
    maps_part = (bound + TOL_PRED_REL * np.nan_to_num(pmax)).max(axis=1)
    allow = maps_part + SUM_UNIT * (n_t + n_e + 2 * SUM_SLACK) * np.abs(ra_)
    with np.errstate(invalid="ignore"):
        diff = np.abs(ra_ - rb_)
        within = ~same & (diff <= allow)
        use = np.where(within, diff / np.where(allow > 0, allow, 1), 0)
    res.n, res.identical = ra_.size, int(same.sum())
    res.add("tolerance", int(within.sum()))
    res.failed = int((~same & ~within).sum())
    res.use = float(use.max()) if use.size else 0.0
    for i in np.flatnonzero(~same & ~within)[:8]:
        res.examples.append(
            f"[{i}] ref {ra_[i]!r} other {rb_[i]!r} diff {diff[i]:.3g} allow {allow[i]:.3g} "
            f"maps' part {maps_part[i]:.3g} count {count[i]}"
        )
    results.append(res)

    res = Result("confounds table")
    tsv = fname("confounds tsv")
    la = (ref / tsv).read_text(encoding="utf-8").split("\n")
    lb = (other / tsv).read_text(encoding="utf-8").split("\n")
    if la[0] != lb[0] or len(la) != len(lb):
        res.failed += 1
        res.notes.append("header or row count differs")
    else:
        r_a = model.rmse_series(x, count, lt_a, ls_a, tes, "exact")
        with np.errstate(invalid="ignore"):
            allow_vt = bound + TOL_PRED_REL * np.nan_to_num(pmax)
            allow_vt = allow_vt + SUM_UNIT * (n_e + SUM_SLACK) * np.nan_to_num(np.abs(r_a))
            per_t = np.where(np.isnan(r_a), 0.0, allow_vt).max(axis=0)
            n_valid = (~np.isnan(r_a)).sum(axis=0)
        for t, (x_a, x_b) in enumerate(zip(la[1:], lb[1:], strict=True)):
            if not x_a:
                continue
            mean_t = abs(float(x_a.split("\t")[0])) if x_a.split("\t")[0] != "n/a" else 0.0
            for col, (u, w) in enumerate(zip(x_a.split("\t"), x_b.split("\t"), strict=True)):
                res.n += 1
                if u == w:
                    res.identical += 1
                    continue
                if "n/a" in (u, w):
                    res.failed += 1
                    continue
                fu, fw = float(u), float(w)
                if col < 2:  # the mean and the deviation: sums over the voxels
                    allow = per_t[t] + SUM_UNIT * (n_valid[t] + SUM_SLACK) * max(abs(fu), mean_t)
                else:  # order statistics and their interpolation
                    allow = per_t[t] + TOL_TABLE_REL * abs(fu)
                if abs(fu - fw) <= allow:
                    res.add("tolerance", 1)
                    res.use = max(res.use, abs(fu - fw) / allow)
                else:
                    res.failed += 1
    results.append(res)

    # ---- the combined series
    res = Result("combined img")
    ca = on(load(ref / fname("combined img"))[1]).astype(F64)
    cb = on(load(other / fname("combined img"))[1]).astype(F64)
    allow = TOL_COMB_REL * np.abs(ca) + TINY
    if a.combmode == "t2s":
        for n, members in model.groups_of(count):
            xs = np.abs(x[members, :n, :].astype(F64))
            with np.errstate(all="ignore"):
                if ts:
                    wa = tes[None, :n, None] * np.exp(
                        -tes[None, :n, None] / t2a[members][:, None, :]
                    )
                    wb = tes[None, :n, None] * np.exp(
                        -tes[None, :n, None] / t2b[members][:, None, :]
                    )
                else:
                    wa = (tes[None, :n] * np.exp(-tes[None, :n] / t2a[members][:, None]))[..., None]
                    wb = (tes[None, :n] * np.exp(-tes[None, :n] / t2b[members][:, None]))[..., None]
                wa = wa / wa.sum(axis=1, keepdims=True)
                wb = wb / wb.sum(axis=1, keepdims=True)
                allow[members] += np.nansum(xs * np.abs(wa - wb), axis=1)
                # each run's T2* within half a float32 unit of its written value: the
                # combination's derivative in ln T2* times 2 x 2^-24
                xg = x[members, :n, :].astype(F64)
                te_ = tes[None, :n, None]
                tbar = np.sum(wa * te_, axis=1, keepdims=True)
                t2 = t2a[members][:, None, :] if ts else t2a[members][:, None, None]
                slope = np.abs(np.sum(wa * xg * (te_ - tbar), axis=1)) / t2[:, 0, :]
                allow[members] += np.nan_to_num(slope) * TOL_COMB_REL
    same = same_bits(ca, cb)
    with np.errstate(invalid="ignore"):
        diff = np.abs(ca - cb)
        within = ~same & (diff <= allow)
        use = np.where(within, diff / allow, 0)
    res.n, res.identical = ca.size, int(same.sum())
    res.add("tolerance", int(within.sum()))
    res.failed = int((~same & ~within).sum())
    res.use = float(use.max()) if use.size else 0.0
    for i in np.flatnonzero((~same & ~within).reshape(-1))[:8]:
        v, t = divmod(int(i), ca.shape[1])
        res.examples.append(
            f"[{v}, {t}] ref {ca[v, t]!r} other {cb[v, t]!r} diff {diff[v, t]:.3g} allow "
            f"{allow[v, t]:.3g} T2* {t2a[v]!r} / {t2b[v]!r} ms count {count[v]}"
        )
    results.append(res)
    return results


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("reference", type=Path)
    p.add_argument("other", type=Path)
    p.add_argument("--cwd", type=Path, default=None)
    p.add_argument("--json", type=Path, default=None)
    p.add_argument("--details", action="store_true", help="print the first failures")
    a = p.parse_args(argv)
    warnings.simplefilter("ignore", RuntimeWarning)
    cwd = a.cwd or a.reference.resolve().parent
    results = check(a.reference.resolve(), a.other.resolve(), cwd)
    for r in results:
        print(r.line())
        if a.details:
            for e in r.examples:
                print(f"    {e}")
    if a.json:
        a.json.write_text(
            json.dumps([r.__dict__ for r in results], indent=1, default=str), encoding="utf-8"
        )
    return 0 if all(r.ok for r in results) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

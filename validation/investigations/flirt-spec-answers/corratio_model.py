# SPDX-License-Identifier: Apache-2.0
"""A model of FLIRT's ``corratio`` and ``woods`` costs written only from the specs (``specs/flirt.md``
§6.1, §6.2 and §8.1 to §8.3 with the spec writer's text of 2026-11-02, ``specs/mcflirt.md`` §4.4,
§6.1 to §6.3, §7.2, §7.3 and appendix A), checked against FSL 6.0.7.23's ``-verbose 7`` trace of
the suite's ``real/dof6`` (``fsl_runs.py``'s ``dof6-trace``): the search's first coarse grid point
at 8 mm, its first sweep of line searches (common scale, then the three translations) on the
8 mm pair that ``-debug`` saved (``dof6-debug``: ``refvol_8``, ``testvol``). Each evaluation's
printed cost and parameters (6 decimals) are compared with FSL's line by line; the model also runs
with the variants the clean-room part tried (``--variant``), to show which detail decides FSL's
9th and 10th evaluations.

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \
        python -P validation/investigations/flirt-spec-answers/corratio_model.py <runs dir> \
        [--variant exact|single-final|double-sums|appendix-a] [--evals N]

numpy's float32 operations are IEEE single precision, each rounded; ``np.cumsum`` of a float32
array is a sequential single-precision running sum; ``math`` is the platform's libm.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

import nibabel as nib
import numpy as np

F32, F64 = np.float32, np.float64
TRIG = "single"


def read(path: Path) -> tuple[np.ndarray, np.ndarray]:
    img = nib.load(str(path))
    a = np.asanyarray(img.dataobj).astype(F32)
    if np.linalg.det(img.affine[:3, :3]) > 0:
        a = a[::-1].copy()
    return a, np.array(img.header["pixdim"][1:4], dtype=F32)


def cog(v: np.ndarray, d: np.ndarray) -> np.ndarray:
    """``specs/flirt.md`` §8.1's centre (the spec writer's text of 2026-11-02): ``v − v_min`` in
    single precision, then double running sums in z, y, x order, flushed into the totals after
    every ``max(1000, trunc(√N)) + 1`` voxels; voxel coordinates divided by the total, times the
    voxel sizes."""
    vm = F32(v.min())
    flat = (v.transpose(2, 1, 0).ravel() - vm).astype(F32).astype(F64)  # z outer, x inner
    nx, ny, nz = v.shape
    zz, yy, xx = np.meshgrid(np.arange(nz), np.arange(ny), np.arange(nx), indexing="ij")
    coords = [xx.ravel().astype(F64), yy.ravel().astype(F64), zz.ravel().astype(F64)]
    nlim = max(1000, int(math.sqrt(float(v.size))))
    total, cog_ = 0.0, [0.0, 0.0, 0.0]
    for s in range(0, len(flat), nlim + 1):
        blk = flat[s : s + nlim + 1]
        t = 0.0
        for x in blk:
            t += x
        total += t
        for k in range(3):
            acc = 0.0
            for x, q in zip(blk, coords[k][s : s + nlim + 1], strict=True):
                acc += x * q
            cog_[k] += acc
    if abs(total) < 1e-5:
        total = 1.0
    return np.array([F64(d[k]) * (cog_[k] / total) for k in range(3)])


def rot_axis(k: int, a: float, c: np.ndarray) -> np.ndarray:
    """A rotation about axis ``k`` by angle ``a`` about the centre ``c``: ``θ = |a|`` in single
    precision, its sine and cosine in single precision; the rows of ``specs/mcflirt.md`` §4.4."""
    th = F32(abs(a))
    m = np.eye(4)
    if th < 1e-8:
        return m
    if TRIG == "double":  # a variant: the angle, sine and cosine in double
        s, co = math.sin(abs(a)) * (1.0 if a >= 0 else -1.0), math.cos(abs(a))
    else:
        s = F64(F32(math.sin(F64(th)))) * (1.0 if a >= 0 else -1.0)
        co = F64(F32(math.cos(F64(th))))
    i, j = [(1, 2), (0, 2), (0, 1)][k]
    sg = -1.0 if k == 1 else 1.0  # Ry's sine sits the other way round
    m[i, i] = co
    m[j, j] = co
    m[i, j] = sg * s
    m[j, i] = -sg * s
    m[:3, 3] = (np.eye(3) - m[:3, :3]) @ c
    return m


def compose(p: np.ndarray, c: np.ndarray) -> np.ndarray:
    """12 parameters about the centre: ``Rx·Ry·Rz`` (each about ``c``), the translations added,
    then ``·K·S`` with the skew and scale each kept about ``c``."""
    m = rot_axis(0, p[0], c) @ rot_axis(1, p[1], c) @ rot_axis(2, p[2], c)
    m[:3, 3] += p[3:6]
    s = np.diag([p[6], p[7], p[8], 1.0])
    s[:3, 3] = c - s[:3, :3] @ c
    k = np.eye(4)
    k[0, 1], k[0, 2], k[1, 2] = p[9], p[10], p[11]
    k[:3, 3] = c - k[:3, :3] @ c
    return m @ k @ s


class Pair:
    def __init__(self, ref, dref, test, dtest, bins: int, smooth: float):
        self.ref, self.test = ref, test
        self.dref, self.dtest = dref, dtest
        self.B = bins
        rmin, rmax = F32(ref.min()), F32(ref.max())
        if rmax - rmin == 0:
            rmax = F32(rmax + 1)
        a1 = F32(F32(bins) / F32(rmax - rmin))
        a0 = F32(F32(-rmin * F32(bins)) / F32(rmax - rmin))
        b = np.trunc((ref * a1).astype(F32) + a0).astype(np.int64)
        self.bins = np.clip(b, 0, bins - 1)
        self.sm = [F32(F32(smooth) / dtest[k]) for k in range(3)]

    def samples(self, aff: np.ndarray):
        """The contributing points in sampling order: their reference bins, interpolated values and
        edge weights (``specs/mcflirt.md`` §6.1 to §6.3)."""
        nt = self.test.shape
        bmat = np.diag([1 / F64(d) for d in self.dtest] + [1.0]) @ np.linalg.inv(aff)
        bmat = bmat @ np.diag([F64(d) for d in self.dref] + [1.0])
        a = bmat.astype(F32)
        a11, a21, a31 = a[0, 0], a[1, 0], a[2, 0]
        xb2 = [F32(F32(n) - F32(1.0001)) for n in nt]
        nx, ny, nz = self.ref.shape
        bins_out, vals, wts = [], [], []
        for z in range(nz):
            for y in range(ny):
                o = [
                    F32(F32(F32(F32(y) * a[r, 1]) + F32(F32(z) * a[r, 2])) + a[r, 3])
                    for r in range(3)
                ]
                lo, hi = F32(0.0), F32(nx - 1)
                for r, step in ((0, a11), (1, a21), (2, a31)):
                    if abs(step) < 1e-8:
                        x1, x2 = (
                            (F32(-1e8), F32(1e8)) if 0.0 <= o[r] <= xb2[r] else (F32(-1e8),) * 2
                        )
                    else:
                        x1, x2 = F32(-o[r] / step), F32(F32(xb2[r] - o[r]) / step)
                    lo, hi = max(lo, min(x1, x2)), min(hi, max(x1, x2))
                if hi < lo:
                    continue
                xmin, xmax = math.ceil(lo), math.floor(hi)
                # the stepping check with the same float additions
                p = [F32(o[r] + F32(F32(xmin) * a[r, 0])) for r in range(3)]
                xs = xmin
                for xc in range(xs, xmax + 1):
                    inb = all(F32(0.0) <= p[r] <= xb2[r] for r in range(3))
                    if xc == xmin and not inb:
                        xmin += 1
                    elif not inb:
                        xmax = xc - 1
                        break
                    p = [F32(p[r] + a[r, 0]) for r in range(3)]
                if xmax < xmin:
                    continue
                n = xmax - xmin + 1
                pos = []
                for r in range(3):
                    seq = np.empty(n, dtype=F32)
                    seq[0] = F32(o[r] + F32(F32(xmin) * a[r, 0]))
                    seq[1:] = a[r, 0]
                    pos.append(np.cumsum(seq, dtype=F32))
                pos = np.stack(pos, axis=1)
                keep = np.ones(n, dtype=bool)
                for e in (0, n - 1):
                    ix = pos[e].astype(np.int64)
                    keep[e] = all(ix[r] >= 0 and ix[r] + 1 <= nt[r] - 1 for r in range(3))
                pos = pos[keep]
                xi = np.arange(xmin, xmax + 1)[keep]
                bins_out.append(self.bins[xi, y, z])
                vals.append(self.trilinear(pos))
                wts.append(self.weight(pos, xb2))
        return np.concatenate(bins_out), np.concatenate(vals), np.concatenate(wts)

    def trilinear(self, p):
        v = self.test
        i = p.astype(np.int64)
        f = (p - i.astype(F32)).astype(F32)
        g = lambda a, b, c: v[i[:, 0] + a, i[:, 1] + b, i[:, 2] + c]  # noqa: E731
        fx, fy, fz = f[:, 0], f[:, 1], f[:, 2]
        e00 = ((g(1, 0, 0) - g(0, 0, 0)) * fx + g(0, 0, 0)).astype(F32)
        e01 = ((g(1, 0, 1) - g(0, 0, 1)) * fx + g(0, 0, 1)).astype(F32)
        e10 = ((g(1, 1, 0) - g(0, 1, 0)) * fx + g(0, 1, 0)).astype(F32)
        e11 = ((g(1, 1, 1) - g(0, 1, 1)) * fx + g(0, 1, 1)).astype(F32)
        h0 = ((e10 - e00) * fy + e00).astype(F32)
        h1 = ((e11 - e01) * fy + e01).astype(F32)
        return ((h1 - h0) * fz + h0).astype(F32)

    @np.errstate(divide="ignore", invalid="ignore")  # smoothing 0: both branches computed
    def weight(self, p, xb2):
        w = np.ones(len(p), dtype=F32)
        for r in range(3):
            o, s = p[:, r], self.sm[r]
            near = (o / s).astype(F32)
            far = ((xb2[r] - o).astype(F32) / s).astype(F32)
            w = np.where(
                o < s, (w * near).astype(F32), np.where((xb2[r] - o) < s, (w * far).astype(F32), w)
            )
        return np.maximum(w, F32(0.0)).astype(F32)

    def corratio(self, aff, variant="exact"):
        b, val, w = self.samples(aff)
        B = self.B
        num = np.zeros(B, dtype=F32)
        sy = np.zeros(B, dtype=F32)
        sy2 = np.zeros(B, dtype=F32)
        wy = (w * val).astype(F32)
        wyy = (wy * val).astype(F32)
        for k in range(B):
            sel = b == k
            if not sel.any():
                continue
            if variant == "double-sums":
                num[k], sy[k], sy2[k] = (F32(np.sum(x[sel].astype(F64))) for x in (w, wy, wyy))
            else:
                num[k], sy[k], sy2[k] = (np.cumsum(x[sel], dtype=F32)[-1] for x in (w, wy, wyy))
        cr, var, ts, ts2, nt = F32(0), F32(0), F32(0), F32(0), F32(0)
        for k in range(B):
            if variant == "appendix-a":  # the sparse bins left out of the bins' sum only
                nt, ts, ts2 = F32(nt + num[k]), F32(ts + sy[k]), F32(ts2 + sy2[k])
            if num[k] > 2.0:
                if variant != "appendix-a":
                    nt = F32(nt + num[k])
                    ts = F32(ts + sy[k])
                    ts2 = F32(ts2 + sy2[k])
                var = F32(F32(sy2[k] - F32(F32(sy[k] * sy[k]) / num[k])) / F32(num[k] - F32(1.0)))
                cr = F32(cr + F32(var * num[k]))
        if nt > 0:
            cr = F32(cr / nt)
        if nt > 1:
            var = F32(F32(ts2 - F32(F32(ts * ts) / nt)) / F32(nt - F32(1.0)))
        if var > 0:
            cr = F32(cr / var)
        eta = F32(0.0) if nt <= 1 or var <= 0 else F32(1.0 - F64(cr))
        if variant == "single-final":
            return cr if (nt > 1 and var > 0) else F32(1.0)
        return F32(1.0 - F64(eta))

    def woods(self, aff, variant="exact"):
        b, val, _ = self.samples(aff)
        tot, n_all = F32(0.0), 0
        for k in range(self.B):
            sel = b == k
            n = int(sel.sum())
            if n <= 2:
                continue
            n_all += n
            s = np.cumsum(val[sel], dtype=F32)[-1]
            s2 = np.cumsum((val[sel] * val[sel]).astype(F32), dtype=F32)[-1]
            var = F32(F32(s2 - F32(F32(s * s) / F32(n))) / F32(n - 1))
            sd = F32(math.sqrt(F64(var))) if var > 0 else F32(0.0)
            nn = F32(F32(n) * F32(n))
            if variant == "n-sigma-mu":
                mu = F32(s / F32(n))
                tot = F32(tot + (F32(F32(F32(n) * sd) / mu) if mu > 0 else F32(F32(n) * sd)))
            else:
                tot = F32(tot + (F32(F32(nn * sd) / s) if s > 0 else F32(nn * sd)))
        return F32(tot / F32(n_all)) if n_all > 0 else F32(1e10)


def parabola(a1, y1, am, ym, a2, y2):
    """``specs/mcflirt.md`` §7.3 in single precision: the vertex, or None."""
    P = F32(F32(F32(am - a2) * F32(ym - y1)) - F32(F32(am - a1) * F32(ym - y2)))
    Q = F32(F32(-F32(F32(F32(am * am) - F32(a2 * a2)) * F32(ym - y1)))
            + F32(F32(F32(am * am) - F32(a1 * a1)) * F32(ym - y2)))  # fmt: skip
    H = F32(F32(F32(am - a2) * F32(a2 - a1)) * F32(a1 - am))
    if abs(H) > 1e-15 and F32(P / H) < 0:
        return None
    if abs(P) > 1e-15:
        return F32(-Q / F32(F32(2.0) * P))
    return None


def golden(a1, am, a2):
    far = a1 if abs(a1 - am) >= abs(a2 - am) else a2
    return F32(F32(F32(0.381966) * far) + F32(F32(0.618034) * am))


def line_search(phi, u, ym, bound):
    """``specs/mcflirt.md`` §7.2 (bracketing with the first step ``bound·u``, refinement)."""
    am, a1 = F32(0.0), F32(F32(bound) * u)
    y1 = phi(a1)
    if y1 < ym:
        am, ym, a1, y1 = a1, y1, am, ym
    a2 = F32(am + F32(F32(1.6) * F32(am - a1)))
    y2 = phi(a2)
    dl = 1.0 if am >= a1 else -1.0
    while ym > y2:
        limit = F32(am + F32(F32(3.2) * F32(a2 - am)))
        a = parabola(a1, y1, am, ym, a2, y2)
        if a is None or (a - a1) * dl < 0 or (a - limit) * dl > 0:
            a = F32(am + F32(F32(1.6) * F32(a2 - a1)))
        y = phi(a)
        if min(a1, am) < a < max(a1, am):
            if y < ym:
                a2, y2, am, ym = am, ym, a, y
                break
            a1, y1 = a, y
        else:
            if y > ym:
                a2, y2 = a, y
                break
            if (a - a2) * dl < 0:
                a1, y1, am, ym = am, ym, a, y
            else:
                a1, y1, am, ym, a2, y2 = am, ym, a2, y2, a, y
    dmin = F32(F32(0.1) * u)
    for _ in range(100):
        if not F32(abs(F32(a2 - a1)) / u) > 1:
            break
        a = parabola(a1, y1, am, ym, a2, y2)
        if a is None or not (min(a1, a2) <= a <= max(a1, a2)):
            a = golden(a1, am, a2)
        eps = F32(1.0) if a2 >= a1 else F32(-1.0)
        if abs(F32(a - a1)) < dmin:
            a = F32(a1 + F32(eps * dmin))
        if abs(F32(a - a2)) < dmin:
            a = F32(a2 - F32(eps * dmin))
        if abs(F32(a - am)) < dmin:
            a = golden(a1, am, a2)
        if abs(F32(am - a1)) < F32(F32(0.4) * u):
            a = F32(am + F32(F32(eps * F32(0.5)) * u))
        if abs(F32(am - a2)) < F32(F32(0.4) * u):
            a = F32(am - F32(F32(eps * F32(0.5)) * u))
        y = phi(a)
        if (a - am) * (a2 - am) > 0:
            a1, y1, a2, y2 = a2, y2, a1, y1
        if y < ym:
            a2, y2, am, ym = am, ym, a, y
        else:
            a1, y1 = a, y
    return am, ym


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("runs")
    ap.add_argument("--variant", default="exact")
    ap.add_argument("--evals", type=int, default=0)
    ap.add_argument(
        "--woods", default="exact", help="exact | n-sigma-mu (n_b sigma_b / mu_b, n_b sigma_b)"
    )
    ap.add_argument("--trig", default="single", help="single | double (the rotations' sines)")
    o = ap.parse_args()
    global TRIG
    TRIG = o.trig
    runs = Path(o.runs)
    ref, dref = read(runs / "dof6-debug" / "refvol_8.nii.gz")
    test, dtest = read(runs / "dof6-debug" / "testvol.nii.gz")
    pair = Pair(ref, dref, test, dtest, bins=256 // 8, smooth=8.0)
    c_in, c_ref = cog(test, dtest), cog(ref, dref)
    ang = F64(F32(-math.pi / 2))
    x0 = np.array([ang, ang, ang, *(c_ref - c_in), 1, 1, 1, 0, 0, 0], dtype=F64)
    trace = [
        ln for ln in (runs / "dof6-trace" / "stdout.txt").read_text().splitlines() if " :: " in ln
    ]
    printed: list[str] = []

    def cost(x):
        c = pair.corratio(compose(x, c_in), o.variant)
        printed.append(f"{len(printed) + 1} : {c:.6f} :: " + " ".join(f"{v:.6f}" for v in x) + " ")
        return c

    # the subset: a common scale (ones on sx, sy, sz, pinv entries (1/√3)²) and tx, ty, tz
    p3 = F64(1.0 / math.sqrt(3.0)) ** 2
    tol12 = np.array([0.005] * 3 + [0.2] * 3 + [0.002] * 3 + [0.001] * 3) * 8
    tol_s = sum(p3 * ((tol12[k] + x0[k]) - x0[k]) for k in (6, 7, 8))
    tols = [F32(tol_s)] + [F32((tol12[k] + x0[k]) - x0[k]) for k in (3, 4, 5)]
    y = np.zeros(4)

    def to12(yv):
        x = x0.copy()
        x[6:9] = x0[6:9] + yv[0]
        x[3:6] = x0[3:6] + yv[1:4]
        return x

    ym = cost(to12(y))
    for k in range(4):

        def phi(a, k=k):
            yy = y.copy()
            yy[k] += F64(a)
            return cost(to12(yy))

        am, ym = line_search(phi, tols[k], ym, 10.0)
        y[k] += F64(am)
    n = o.evals or len(printed)
    same = 0
    for i in range(min(n, len(printed))):
        if printed[i] == trace[i]:
            same += 1
        else:
            print(f"evaluation {i + 1} differs:\n  FSL   {trace[i]}\n  model {printed[i]}")
            break
    print(f"variant {o.variant}: {same} of {len(printed)} evaluations of the first sweep as FSL's "
          f"(cost and parameters as printed)")  # fmt: skip
    for run in ("corratio-mc", "woods-neg"):
        if (runs / run / "stdout.txt").is_file():
            check_costs(runs, run, o.variant, o.woods)


def check_costs(runs: Path, run: str, variant: str, woods: str) -> None:
    """``corratio_costs.sch``: corratio with smoothing 8 and 0, and woods, at the identity and a
    translation (FLIRT's measurecost decomposes and recomposes the row, which keeps both exact),
    on the run's own 8 mm pair (its ``-debug`` volumes) or ``dof6-debug``'s."""
    d = runs / run if (runs / run / "testvol.nii.gz").is_file() else runs / "dof6-debug"
    ref, dref = read(d / "refvol_8.nii.gz")
    test, dtest = read(d / "testvol.nii.gz")
    ub = np.array([[1, 0, 0, 2.5], [0, 1, 0, -1.5], [0, 0, 1, 3.25], [0, 0, 0, 1.0]])
    pair = Pair(ref, dref, test, dtest, bins=256 // 8, smooth=8.0)
    pair0 = Pair(ref, dref, test, dtest, bins=256 // 8, smooth=0.0)
    mine = [pair.corratio(m, variant) for m in (np.eye(4), ub)]
    mine += [pair0.corratio(m, variant) for m in (np.eye(4), ub)]
    mine += [pair.woods(m, woods) for m in (np.eye(4), ub)]
    mine_s = [f"{c:.6f}" for c in mine]
    rows = [ln.split() for ln in (runs / run / "stdout.txt").read_text().splitlines()]
    fsl = [r[0] for r in rows if len(r) == 17][:6]
    print(f"{run}: corratio (smoothing 8, 0) and woods at two matrices: FSL {' '.join(fsl)}; "
          f"model {' '.join(mine_s)}: {'same' if fsl == mine_s else 'DIFFER'}")  # fmt: skip


if __name__ == "__main__":
    main()

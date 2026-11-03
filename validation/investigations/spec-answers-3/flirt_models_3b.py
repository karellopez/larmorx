# SPDX-License-Identifier: Apache-2.0
"""Models of FLIRT part 2's spec answers to questions 22 to 24 and 38 (relay 3b, 2026-11-03),
written from ``specs/flirt.md``'s new text alone, against FSL 6.0.7.23's outputs (every FSL run
through the oracle cache, ``use``):

- ``sinc`` (question 22, §9.2): the windowed sinc's table with glibc's single-precision sine
  against a correctly rounded one, on every voxel of ``real/interp-sinc`` (the earlier part's
  ``fsl_runs.py sinc`` run) and on applyxfm4D's sinc of float images (four trial findings);
- ``spline`` (question 22, §9.3, §15.7): applyxfm4D's spline of float and double images (five
  trial findings), every interpolated voxel;
- ``leastsq`` (question 23, §6.2): six ``leastsq`` costs at 8 mm (a schedule of ``measurecost``
  rows, smoothing 8 and 0) from the three-level sums;
- ``ties`` (question 24, §8.2): the line searches with every cost equal, the parameters of every
  evaluation of the 8 mm stage's optimisations in FSL's ``-verbose 7`` traces of the FSL-91 and
  FSL-92 cases;
- ``debug`` (question 38, §9.4): the ``-debug`` levels' sform and quaternion fields and
  ``global_refweight1``'s header (two trial findings).

The trial findings are rebuilt from the part-2 trial's recipes (``difftest`` seed 20261103 of
``fsl-flirt-original``). ``original`` runs the clean-room original as a black box (``reproduce``)
on the same inputs, so that the checks also count where it differs from FSL and from the model.

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \\
        python -P validation/investigations/spec-answers-3/flirt_models_3b.py runs <dir>
    ... flirt_models_3b.py original <dir> <larmorx binary>        (optional)
    ... flirt_models_3b.py check <dir> [sinc spline leastsq ties debug]
"""

from __future__ import annotations

import ast
import importlib.util
import math
import os
import re
import shutil
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
EARLIER = HERE.parent / "flirt-spec-answers"
F32 = np.float32


def _load(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


rm = _load(EARLIER / "resample_model.py", "resample_model")  # §9.2's positions and sinc sums
cm = _load(EARLIER / "corratio_model.py", "corratio_model")  # §6.1's sampling and edge weights
mm = _load(EARLIER / "matrices_model.py", "matrices_model")  # §3, §4.1 and §12's matrices

SEED = "20261103"
SPLINE_CASES = (368, 3494, 4718, 6140, 6013)
SINC_CASES = (681, 2358, 6389, 7134)
DEBUG_CASES = (170, 2424)
LEASTSQ_SCH = """setscale 8
setoption smoothing 8
setoption costfunction leastsq
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
setrow UB 1 0 0 0  0 1 0 0  0 0 1 0  2.5 -1.5 3.25 1
setrow UC 1 0 0 0  0 1 0 0  0 0 1 0  -7.75 4.125 -2.5 1
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
measurecost 12 UB 0 0 0 0 0 0 rel
measurecost 12 UC 0 0 0 0 0 0 rel
print U
setoption smoothing 0
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
measurecost 12 UB 0 0 0 0 0 0 rel
measurecost 12 UC 0 0 0 0 0 0 rel
print U
"""
LEASTSQ_MATS = ((0.0, 0.0, 0.0), (2.5, -1.5, 3.25), (-7.75, 4.125, -2.5))
TRACES = {
    "empty-refweight": ("-refweight", "@zero:t1.nii:zeroweight.nii"),
    "empty-inweight": ("-inweight", "@zero:epi.nii:zeroinweight.nii"),
    "onevox-in": (),
    "dof6": ("-dof", "6"),
}


# --- the runs ------------------------------------------------------------------------------------


def _trace_args(name: str) -> tuple[str, ...]:
    inp = "@const:1x1x1:2x2x2:one.nii" if name == "onevox-in" else "@epi.nii"
    return ("-in", inp, "-ref", "@t1.nii", *TRACES[name], "-omat", "out.mat", "-verbose", "7")


def runs(out: Path) -> None:
    """FSL's runs through the oracle cache (``use``: a run already computed is served)."""
    from larmorx_validation.difftest import core
    from larmorx_validation.difftest.tools import fsl_flirt as dt
    from larmorx_validation.parity import fsl_flirt_replica as rep

    fr = _load(EARLIER / "fsl_runs.py", "fsl_runs")
    fr.run(out, "sinc")  # real/interp-sinc's call
    # real/cost-leastsq's pair at 8 mm (``-debug`` saves it): six costs of ``LEASTSQ_SCH``
    fr.RUNS["leastsq-costs"] = (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-omat",
            "o.mat",
            "-schedule",
            "leastsq_costs.sch",
            "-debug",
        ),
        (),
    )
    fr.FILES["leastsq-costs"] = (("leastsq_costs.sch", LEASTSQ_SCH),)
    fr.run(out, "leastsq-costs")
    for n in (*SPLINE_CASES, *SINC_CASES, *DEBUG_CASES):
        recipe = core.make_recipe(dt.ORIGINAL, SEED, n)
        run = dt.case(recipe, "original").payload[1]
        d = out / "trial" / str(n)
        shutil.rmtree(d, ignore_errors=True)
        (d / "fsl").mkdir(parents=True)
        args = rep._prepare(run, d / "fsl")
        settings = dict(run.settings)
        (d / "args.txt").write_text(
            run.program + " " + " ".join(args) + "\n" + repr(settings) + "\n", encoding="utf-8"
        )
        p = rep.oracle_run(run.program, args, cwd=d / "fsl", settings=settings)
        (d / "fsl.stdout").write_text(p.stdout, encoding="utf-8")
        (d / "fsl.stderr").write_text(p.stderr, encoding="utf-8")
        print(f"trial {n}: {run.program}, exit {p.returncode}, cached {p.cached}")
    for name in TRACES:
        d = out / "traces" / name
        shutil.rmtree(d, ignore_errors=True)
        (d / "fsl").mkdir(parents=True)
        run = rep.Run("flirt", _trace_args(name))
        args = rep._prepare(run, d / "fsl")
        (d / "argv").write_text("flirt " + " ".join(args) + "\n", encoding="utf-8")
        p = rep.oracle_run("flirt", args, cwd=d / "fsl", settings={})
        (d / "fsl" / "stdout.txt").write_text(p.stdout, encoding="utf-8")
        print(f"trace {name}: exit {p.returncode}, cached {p.cached}")


def original(out: Path, binary: str) -> None:
    """The clean-room original (``reproduce``, one thread) on the same inputs, as a black box."""
    from larmorx_validation.parity import defects
    from larmorx_validation.parity import fsl_flirt as orig

    os.environ["LARMORX_FLIRT_BIN"] = binary
    dirs = [d for d in sorted((out / "trial").iterdir())] + [
        d for d in sorted((out / "traces").iterdir())
    ]
    for d in dirs:
        if (d / "args.txt").is_file():
            lines = (d / "args.txt").read_text(encoding="utf-8").split("\n")
            words, settings = lines[0].split(), ast.literal_eval(lines[1])
        else:
            words, settings = (d / "argv").read_text(encoding="utf-8").split(), {}
        program, args = words[0], words[1:]
        b = d / "orig"
        shutil.rmtree(b, ignore_errors=True)
        b.mkdir()
        for w in args:
            src = d / "fsl" / w
            if not w.startswith("/") and src.is_file():
                shutil.copy(src, b / w)
                if src.suffix == ".hdr":
                    shutil.copy(src.with_suffix(".img"), b / src.with_suffix(".img").name)
        r = orig.run_original(
            program, args, cwd=b, settings=settings, mode=defects.REPRODUCE, threads=1
        )
        (b / "stdout.txt").write_text(r.stdout, encoding="utf-8")
        (b / "stderr.txt").write_text(r.stderr, encoding="utf-8")
        print(f"original {d.name}: exit {r.code}")
    traces_compared(out)


def _own(text: str) -> list[str]:
    return [ln for ln in text.splitlines() if "(larmorx" not in ln and not ln.startswith("larmorx")]


def traces_compared(out: Path) -> None:
    """Where the original's ``-verbose 7`` trace first parts from FSL's (a sign of zero aside)."""
    zero = re.compile(r"-(0\.0+)(?![0-9])")
    for name in TRACES:
        d = out / "traces" / name
        if not (d / "orig" / "stdout.txt").is_file():
            continue
        f = _own((d / "fsl" / "stdout.txt").read_text(encoding="utf-8"))
        o = _own((d / "orig" / "stdout.txt").read_text(encoding="utf-8"))
        o = [ln for ln in o if "transformed sform from the input" not in ln or ln in f]
        first = next((i for i, (a, b) in enumerate(zip(f, o, strict=False)) if a != b), None)
        real = next(
            (
                i
                for i, (a, b) in enumerate(zip(f, o, strict=False))
                if zero.sub(r"\1", a) != zero.sub(r"\1", b)
            ),
            None,
        )
        show = lambda i: "none" if i is None else str(i + 1)  # noqa: E731
        print(
            f"trace {name}: {len(f)} lines; the original first differs at line {show(first)}, "
            f"beyond a sign of zero at line {show(real)}"
        )


# --- question 22: the sinc -----------------------------------------------------------------------


def _crsin(y: F32) -> F32:
    import mpmath

    mpmath.mp.prec = 200
    return F32(float(mpmath.sin(mpmath.mpf(float(y)))))


def sinc_table(width: int, window: str, sine: str) -> np.ndarray:
    """§9.2's table (1201 samples), the sine glibc's (the platform's ``sinf``) or correctly
    rounded."""
    hw = (width - 1) // 2
    m = F32((1201 - 1.0) / 2.0)
    ker = np.zeros(1201)
    for k in range(1, 1202):
        x = F32(F32(F32(F32(k) - m) - F32(1.0)) / m) * F32(hw)
        if abs(float(x)) < 1e-7:
            s = F32(1.0 - abs(float(x)))
        else:
            y = F32(math.pi * float(x))
            sv = F32(rm._libm.sinf(float(y))) if sine == "glibc" else _crsin(y)
            s = F32(sv / y)
        c1 = rm._libm.cos(math.pi * float(x) / hw)
        if abs(float(x)) > hw:
            w = F32(0.0)
        elif window == "hanning":
            w = F32(0.5 + 0.5 * c1)
        elif window == "blackman":
            w = F32(0.42 + 0.5 * c1 + 0.08 * rm._libm.cos(2.0 * math.pi * float(x) / hw))
        else:
            w = F32(1.0)
        ker[k - 1] = float(F32(s * w))
    return ker


def _image(p: Path):
    import nibabel as nib

    img = nib.load(str(p))
    return img, np.asanyarray(img.dataobj), bool(np.linalg.det(img.affine[:3, :3]) > 0)


def _case(d: Path):
    """A trial case's arrays as FLIRT holds them (neurological files reversed along x): the input
    values (single, or double for a double image), FSL's output, the original's if present, and
    the positions of §9.2 with the mask of the interpolated voxels."""
    words = (d / "args.txt").read_text(encoding="utf-8").split("\n")[0].split()
    inn, ref, outn, mat = words[1:5]
    f = d / "fsl"
    im, a, ni = _image(f / inn)
    rim, r, nr = _image(f / ref)
    outf = next(p for p in f.iterdir() if p.name.startswith(outn + "."))
    o = np.asanyarray(_image(outf)[0].dataobj)
    q = np.asanyarray(_image(d / "orig" / outf.name)[0].dataobj) if (d / "orig").is_dir() else None
    dbl = a.dtype == np.float64
    vals = np.asanyarray(im.dataobj.get_unscaled()).astype(np.float64 if dbl else F32)
    if vals.ndim == 3:
        vals = vals[..., None]
    if "-3D" in words:
        vals = vals[..., :1]
    o = o[..., None] if o.ndim == 3 else o
    q = q[..., None] if q is not None and q.ndim == 3 else q
    if ni:
        vals = vals[::-1]
    if nr:
        o = o[::-1]
        q = q[::-1] if q is not None else None
    d_in = [abs(float(v)) for v in im.header.get_zooms()[:3]]
    d_ref = [abs(float(v)) for v in rim.header.get_zooms()[:3]]
    px, py, pz = rm.positions(rm.mm.read_matrix(f / mat), d_in, d_ref, r.shape[:3])
    n = vals.shape[:3]
    inside = (px >= 0) & (px <= n[0] - 1) & (py >= 0) & (py <= n[1] - 1) & (pz >= 0)
    inside &= pz <= n[2] - 1
    return words, a.dtype, vals, o, q, (px, py, pz), inside


def _same(got: np.ndarray, want: np.ndarray | None) -> int:
    if want is None:
        return -1
    return int(np.sum(got.astype(np.float64) == want.astype(np.float64)))


def check_sinc(out: Path) -> None:
    import nibabel as nib

    d = out / "sinc"
    epi, t1 = nib.load(str(d / "epi.nii")), nib.load(str(d / "t1.nii"))
    fsl = np.asanyarray(nib.load(str(d / "out.nii.gz")).dataobj).astype(F32)[::-1]
    img = np.asanyarray(epi.dataobj).astype(F32)
    d_in = [float(v) for v in epi.header.get_zooms()[:3]]
    d_out = [float(v) for v in t1.header.get_zooms()[:3]]
    px, py, pz = rm.positions(rm.mm.read_matrix(d / "init.mat"), d_in, d_out, t1.shape[:3])
    n = img.shape
    inside = (px >= 0) & (px <= n[0] - 1) & (py >= 0) & (py <= n[1] - 1) & (pz >= 0)
    inside &= pz <= n[2] - 1
    idx = np.nonzero(inside)
    tables = {s: sinc_table(7, "hanning", s) for s in ("glibc", "correctly rounded")}
    k = np.nonzero(tables["glibc"] != tables["correctly rounded"])[0] + 1
    print(f"sinc table (width 7): {len(k)} of 1201 samples differ between the sines: {k.tolist()}")
    for sine, ker in tables.items():
        same = 0
        for s in range(0, len(idx[0]), 200000):
            sel = tuple(a[s : s + 200000] for a in idx)
            got = rm.sinc_values(img, px[sel], py[sel], pz[sel], ker, 3)
            same += int(np.sum(got.view(np.uint32) == fsl[sel].view(np.uint32)))
        rest = np.unique(fsl[~inside])
        print(
            f"real/interp-sinc, {sine} sine: {same} of {len(idx[0])} interpolated voxels FSL's; "
            f"the other {int((~inside).sum())} one value ({len(rest)} distinct)"
        )
    for n_ in SINC_CASES:
        _, dtype, vals, o, q, (px, py, pz), inside = _case(out / "trial" / str(n_))
        res = []
        for sine in ("glibc", "correctly rounded"):
            ker = sinc_table(7, "blackman", sine)
            sf = so = 0
            for t in range(vals.shape[3]):
                got = rm.sinc_values(vals[..., t], px[inside], py[inside], pz[inside], ker, 3)
                sf += _same(got, o[..., t][inside])
                so += _same(got, q[..., t][inside]) if q is not None else 0
            res.append(f"{sine} sine: FSL {sf}" + (f", original {so}" if q is not None else ""))
        print(
            f"applyxfm4D sinc {n_} ({dtype}, {int(inside.sum()) * vals.shape[3]}): "
            + "; ".join(res)
        )


# --- question 22: the spline ---------------------------------------------------------------------


def spline_coefficients(v: np.ndarray, store) -> np.ndarray:
    """§9.3's prefilter along x, y, z; each pass's coefficients stored as ``store``."""
    zp = math.sqrt(3.0) - 2.0
    out = v.astype(store)
    for axis in range(3):
        size = out.shape[axis]
        if size <= 1:
            continue
        col = np.moveaxis(out, axis, 0).astype(np.float64).copy()
        nterm = min(int(math.log(1e-8) / math.log(abs(zp)) + 1.5), size)
        last, start, zi = col[size - 1].copy(), col[0].copy(), zp
        for i in range(1, nterm):
            start = start + zi * col[i]
            zi *= zp
        col[0] = start
        for i in range(1, size):
            col[i] = col[i] + zp * col[i - 1]
        col[size - 1] = (-zp / (1.0 - zp * zp)) * (2.0 * col[size - 1] - last)
        for i in range(size - 2, -1, -1):
            col[i] = zp * (col[i + 1] - col[i])
        out = np.moveaxis((col * 6.0).astype(store), 0, axis)
    return out


def check_spline(out: Path) -> None:
    tot = {"model": 0, "fsl": 0, "orig": 0, "single": 0}
    for n_ in SPLINE_CASES:
        _, dtype, vals, o, q, (px, py, pz), inside = _case(out / "trial" / str(n_))
        dbl = dtype == np.float64
        sf = so = ss = 0
        for t in range(vals.shape[3]):
            got = rm.spline_values(
                spline_coefficients(vals[..., t], np.float64 if dbl else F32),
                px[inside],
                py[inside],
                pz[inside],
                zeros=False,
            )
            sf += _same(got, o[..., t][inside])
            so += _same(got, q[..., t][inside]) if q is not None else 0
            if dbl:  # the reading the spec corrects: single-precision coefficients
                g1 = rm.spline_values(
                    spline_coefficients(vals[..., t].astype(F32), F32),
                    px[inside],
                    py[inside],
                    pz[inside],
                    zeros=False,
                )
                ss += _same(g1, o[..., t][inside])
        m = int(inside.sum()) * vals.shape[3]
        tot["model"] += m
        tot["fsl"] += sf
        tot["orig"] += so
        extra = f"; single-precision coefficients: FSL {ss}" if dbl else ""
        orig = f", the original {so}" if q is not None else ""
        print(f"applyxfm4D spline {n_} ({dtype}): {sf} of {m} FSL's{orig}{extra}")
    print(
        f"spline: the model gives FSL's value at {tot['fsl']} of {tot['model']} interpolated voxels"
        + (f"; the original's at {tot['orig']}" if tot["orig"] else "")
    )


# --- question 23: leastsq ------------------------------------------------------------------------


def _rows(pair, aff):
    """§6.1's contributing points row by row: (z, reference values, interpolated values, edge
    weights): the earlier model's sampling (``corratio_model.Pair.samples``, which concatenates
    its rows), its loop replayed to keep the rows apart."""
    rows = []
    nx, ny, nz = pair.ref.shape
    nt = pair.test.shape
    bmat = np.diag([1 / float(v) for v in pair.dtest] + [1.0]) @ np.linalg.inv(aff)
    a = (bmat @ np.diag([float(v) for v in pair.dref] + [1.0])).astype(F32)
    xb2 = [F32(F32(n) - F32(1.0001)) for n in nt]
    for z in range(nz):
        for y in range(ny):
            o = [
                F32(F32(F32(F32(y) * a[r, 1]) + F32(F32(z) * a[r, 2])) + a[r, 3]) for r in range(3)
            ]
            lo, hi = F32(0.0), F32(nx - 1)
            for r in range(3):
                step = a[r, 0]
                if abs(step) < 1e-8:
                    x1, x2 = (F32(-1e8), F32(1e8)) if 0.0 <= o[r] <= xb2[r] else (F32(-1e8),) * 2
                else:
                    x1, x2 = F32(-o[r] / step), F32(F32(xb2[r] - o[r]) / step)
                lo, hi = max(lo, min(x1, x2)), min(hi, max(x1, x2))
            if hi < lo:
                continue
            xmin, xmax = math.ceil(lo), math.floor(hi)
            p = [F32(o[r] + F32(F32(xmin) * a[r, 0])) for r in range(3)]
            for xc in range(xmin, xmax + 1):
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
            pos, xi = pos[keep], np.arange(xmin, xmax + 1)[keep]
            rows.append((z, pair.ref[xi, y, z], pair.trilinear(pos), pair.weight(pos, xb2)))
    return rows


def leastsq(pair, aff, smooth: bool, levels: str = "three") -> F32:
    """§6.2's ``leastsq``: three-level single-precision sums (row, slice, total), or (variants)
    one level or double sums."""
    if levels == "double":
        s = n = 0.0
        for _, r, v, w in _rows(pair, aff):
            dd = r.astype(np.float64) - v.astype(np.float64)
            s += float(np.sum(w.astype(np.float64) * dd * dd if smooth else dd * dd))
            n += float(np.sum(w.astype(np.float64))) if smooth else len(dd)
        return F32(s / n)
    total, weight, count, zcur = [F32(0)] * 2, [F32(0)] * 2, 0, None
    for z, r, v, w in _rows(pair, aff):
        if levels == "three" and zcur is not None and z != zcur:
            total, weight = [F32(total[0] + total[1]), F32(0)], [F32(weight[0] + weight[1]), F32(0)]
        zcur = z
        d = (r - v).astype(F32)
        t = ((w * d).astype(F32) * d).astype(F32) if smooth else (d * d).astype(F32)
        if levels == "three":
            srow = np.cumsum(t, dtype=F32)[-1] if len(t) else F32(0)
            wrow = np.cumsum(w, dtype=F32)[-1] if len(w) else F32(0)
            total[1], weight[1] = F32(total[1] + srow), F32(weight[1] + wrow)
        else:
            for x in t:
                total[0] = F32(total[0] + x)
            for x in w:
                weight[0] = F32(weight[0] + x)
        count += len(d)
    s, wsum = F32(total[0] + total[1]), F32(weight[0] + weight[1])
    num = wsum if smooth else F32(count)
    if (smooth and num > 1.0) or (not smooth and count > 1):
        return F32(s / num)
    m = F32(max(pair.ref.max(), pair.test.max()) - min(pair.ref.min(), pair.test.min()))
    return F32(m * m)


def check_leastsq(out: Path) -> None:
    d = out / "leastsq-costs"
    ref, dref = cm.read(d / "refvol_8.nii.gz")
    test, dtest = cm.read(d / "testvol.nii.gz")
    fsl = [
        r.split()[0] for r in (d / "stdout.txt").read_text().splitlines() if len(r.split()) == 17
    ]
    mats = []
    for t in LEASTSQ_MATS:
        m = np.eye(4)
        m[:3, 3] = t
        mats.append(m)
    for levels in ("three", "one", "double"):
        mine = []
        for smooth in (8.0, 0.0):
            pair = cm.Pair(ref, dref, test, dtest, bins=32, smooth=smooth)
            mine += [f"{leastsq(pair, m, smooth > 0, levels):.6f}" for m in mats]
        same = sum(a == b for a, b in zip(fsl, mine, strict=True))
        print(f"leastsq, {levels}-level sums: {same} of {len(fsl)} costs FSL's ({' '.join(mine)})")
    print(f"leastsq, FSL: {' '.join(fsl)}")


# --- question 24: equal costs --------------------------------------------------------------------


def _parabola(a1, y1, am, ym, a2, y2):
    """``specs/mcflirt.md`` §7.3, single precision; ``None`` when there is no vertex."""
    p = F32(F32(F32(am - a2) * F32(ym - y1)) - F32(F32(am - a1) * F32(ym - y2)))
    h = F32(F32(F32(am - a2) * F32(a2 - a1)) * F32(a1 - am))
    if abs(h) > 1e-15 and F32(p / h) < 0:
        return None
    if abs(p) > 1e-15:
        q = F32(
            F32(-F32(F32(am * am) - F32(a2 * a2)) * F32(ym - y1))
            + F32(F32(F32(am * am) - F32(a1 * a1)) * F32(ym - y2))
        )
        return F32(-q / F32(2 * p))
    return None


def _golden(a1, am, a2):
    far = a1 if abs(a1 - am) >= abs(a2 - am) else a2
    return F32(F32(F32(0.3819660) * far) + F32(F32(0.6180340) * am))


def line_search(phi, u, bound, ym=None):
    """``specs/mcflirt.md`` §7.2 with §8.2's bound guess; every comparison strict."""
    am = F32(0)
    ym = phi(am) if ym is None else ym
    a1 = F32(bound * u)
    y1 = phi(a1)
    if y1 < ym:
        a1, y1, am, ym = am, ym, a1, y1
    a2 = F32(am + F32(F32(1.6) * F32(am - a1)))
    y2 = phi(a2)
    dl = 1 if am >= a1 else -1
    while ym > y2:
        lim = F32(am + F32(F32(3.2) * F32(a2 - am)))
        a = _parabola(a1, y1, am, ym, a2, y2)
        if a is None or (a - a1) * dl < 0 or (a - lim) * dl > 0:
            a = F32(am + F32(F32(1.6) * F32(a2 - a1)))
        y = phi(a)
        if (a - a1) * (a - am) < 0:
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
        if not F32(abs(a2 - a1) / u) > 1:
            break
        a = _parabola(a1, y1, am, ym, a2, y2)
        if a is None or not (min(a1, a2) <= a <= max(a1, a2)):
            a = _golden(a1, am, a2)
        e = 1 if a2 >= a1 else -1
        if abs(a - a1) < dmin:
            a = F32(a1 + e * dmin)
        if abs(a - a2) < dmin:
            a = F32(a2 - e * dmin)
        if abs(a - am) < dmin:
            a = _golden(a1, am, a2)
        if abs(am - a1) < F32(F32(0.4) * u):
            a = F32(am + e * F32(F32(0.5) * u))
        if abs(am - a2) < F32(F32(0.4) * u):
            a = F32(am - e * F32(F32(0.5) * u))
        y = phi(a)
        if (a - am) * (a2 - am) > 0:
            a1, y1, a2, y2 = a2, y2, a1, y1
        if y < ym:
            a2, y2, am, ym = am, ym, a, y
        else:
            a1, y1 = a, y
    return am, ym


def _blocks(lines: list[str]):
    """The optimisations of a trace: their printed start and tolerances, the schedule's dof and
    bound guess in force, and their evaluations' parameters."""
    out, dof, bound = [], 7, 10.0
    for i, ln in enumerate(lines):
        m0 = re.match(r"^ >>\s+optimise\s+(\d+)", ln)
        if m0:
            dof = min(12, max(6, int(m0.group(1))))
        if re.match(r"^ >>\s+search", ln):
            dof = 7
        m1 = re.match(r"^setoptions: boundguess (\S+)", ln)
        if m1:
            bound = float(m1.group(1))
        nxt = lines[i + 1] if i + 1 < len(lines) else ""
        if "Starting with " in ln and "Starting with :" not in ln and "and tolerance" in nxt:
            tol = [float(v) for v in nxt.split("tolerance", 1)[1].split()]
            if len(tol) != 12:
                continue
            start = [float(v) for v in ln.split("with", 1)[1].split()]
            evs = []
            for l2 in lines[i + 2 :]:
                m = re.match(r"^\d+ : (\S+) :: (.*)$", l2)
                if m:
                    evs.append([float(v) for v in m.group(2).split()])
                elif evs and (":::" in l2 or not l2[:1].isdigit()):
                    break
            out.append((start, tol, evs, dof, bound))
    return out


def check_ties(out: Path) -> None:
    for name in TRACES:
        if name == "dof6":
            continue
        lines = (out / "traces" / name / "fsl" / "stdout.txt").read_text().splitlines()
        blocks = _blocks(lines)
        same = total = 0
        for start, tol, evs, dof, bound in blocks:
            printed: list[list[float]] = []
            for cur in range(dof):

                def phi(a, cur=cur, start=start, dof=dof, printed=printed):
                    x = list(start)
                    x[cur] = start[cur] + float(a)
                    if dof == 7 and cur == 6:  # one scale for the three axes
                        x[7] = x[8] = x[6]
                    printed.append(x)
                    return F32(1.0)  # every cost equal

                ym = None if cur == 0 else F32(1.0)
                line_search(phi, F32(1.0 / F32(1.0 / F32(tol[cur]))), bound, ym)
            for p, q in zip(printed, evs, strict=False):
                total += 1
                same += all(abs(a - b) <= 1.5e-6 for a, b in zip(p, q, strict=False))
            total += abs(len(printed) - len(evs))
        print(
            f"ties, {name}: {len(blocks)} optimisations, {same} of {total} evaluations' "
            "parameters FSL's (printed digits)"
        )


# --- question 38: the -debug levels --------------------------------------------------------------


def _forms(p: Path):
    import nibabel as nib

    h = nib.load(str(p)).header
    srow = np.eye(4)
    srow[:3] = np.array([h["srow_x"], h["srow_y"], h["srow_z"]], dtype=float)
    keys = ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z")
    qfac = float(h["pixdim"][0]) if h["pixdim"][0] != 0 else 1.0
    q = mm.quatern_to_mat44(*(float(h[k]) for k in keys), *map(float, h["pixdim"][1:4]), qfac)
    return h, srow, q


def debug_levels(ref: Path, msamp: float) -> dict:
    """§9.4's levels: the isotropic resampling to the minimum sampling and the subsamplings by
    2, composing only set forms, written as §4.1 writes (a neurological reference's forms, set or
    not, swapped back with each level's grid)."""
    h, s, q = _forms(ref)
    shape, d = [int(v) for v in h["dim"][1:4]], [float(v) for v in h["pixdim"][1:4]]
    sset, qset = int(h["sform_code"]) != 0, int(h["qform_code"]) != 0
    neuro = np.linalg.det((s if sset else q)[:3, :3]) > 0
    if neuro:
        s, q = mm.swap_lr(s, shape[0], d), mm.swap_lr(q, shape[0], d)
    st = [mm.f32(F32(msamp) / F32(v)) for v in d]
    n = [int(max(1.0, float(F32(a) / F32(b)))) for a, b in zip(shape, st, strict=True)]
    s = mm.mul(s, np.diag([*st, 1.0])) if sset else s
    q = mm.mul(q, np.diag([*st, 1.0])) if qset else q
    cur = (n, msamp, s, q)
    levels = {"refvol": cur}
    for name, limit in (("refvol_2", 1.9), ("refvol_4", 3.9), ("refvol_8", 7.9)):
        if msamp < limit:
            nn, sc, s2, q2 = cur
            two = np.diag([2.0, 2.0, 2.0, 1.0])
            cur = (
                [(v + 1) // 2 for v in nn],
                2 * sc,
                mm.mul(s2, two) if sset else s2,
                mm.mul(q2, two) if qset else q2,
            )
        levels[name] = cur
    res = {}
    for name, (nn, sc, s2, q2) in levels.items():
        if neuro:
            s2, q2 = mm.swap_lr(s2, nn[0], (sc, sc, sc)), mm.swap_lr(q2, nn[0], (sc, sc, sc))
        qf = np.array([[mm.f32(v) for v in row] for row in q2])
        quat = [mm.f32(v) for v in mm.mat44_to_quatern(qf)]
        res[name] = (nn, [mm.f32(v) for v in s2[:3].ravel()], quat)
    return res


def check_debug(out: Path) -> None:
    import nibabel as nib

    good = total = 0
    for n_ in DEBUG_CASES:
        f = out / "trial" / str(n_) / "fsl"
        ref = next(p for p in f.iterdir() if p.name.startswith("ref."))
        ext = ".nii.gz" if (f / "refvol.nii.gz").exists() else ".hdr"
        msamp = float(nib.load(str(f / ("refvol" + ext))).header["pixdim"][1])
        for name, (nn, srow, quat) in debug_levels(ref, msamp).items():
            h = nib.load(str(f / (name + ext))).header
            ws = [float(v) for v in np.array([h["srow_x"], h["srow_y"], h["srow_z"]]).ravel()]
            keys = ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z")
            wq = [float(F32(h[k])) for k in keys]
            ok = nn == [int(v) for v in h["dim"][1:4]] and ws == srow and wq == quat
            good += ok
            total += 1
            if not ok:
                print(f"debug {n_} {name}: DIFFERS sform {srow} / {ws}; quaternion {quat} / {wq}")
        g = nib.load(str(f / ("global_refweight1" + ext))).header
        print(
            f"debug {n_}: global_refweight1 dim {g['dim'][:4].tolist()}, pixdim "
            f"{g['pixdim'].tolist()}, codes {int(g['qform_code'])}/{int(g['sform_code'])}"
        )
    print(f"debug: {good} of {total} levels' sform rows and quaternion fields FSL's")


CHECKS = {
    "sinc": check_sinc,
    "spline": check_spline,
    "leastsq": check_leastsq,
    "ties": check_ties,
    "debug": check_debug,
}


def main() -> None:
    cmd, out = sys.argv[1], Path(sys.argv[2])
    if cmd == "runs":
        runs(out)
    elif cmd == "original":
        original(out, sys.argv[3])
    elif cmd == "check":
        for name in sys.argv[3:] or list(CHECKS):
            CHECKS[name](out)
        traces_compared(out)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()

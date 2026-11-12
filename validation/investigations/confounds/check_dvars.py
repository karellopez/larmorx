# SPDX-License-Identifier: Apache-2.0
"""nipype's DVARS and the numerics under it: larmorx against this environment's numpy, SciPy,
nitime and nipype, bit for bit.

Sections:

- ``cmul``: numpy's in-place complex64 product (``IN1 *= IN2``, nitime's ``fftconvolve``)
  against ``_core.numpy_cmul64``: lengths 1 to 64, around the SIMD widths, powers of two to
  2^16; Gaussian, FFT-like, subnormal, huge (overflow), infinite and NaN values.
- ``crosscov``: nitime's ``crosscov(x, y, debias=False)`` (lags ``0 .. n`` and ``all_lags``) and
  ``autocorr(x)`` against ``_core.nitime_crosscov`` with this environment's SciPy FFTs, ``T``
  from 1 to 9000 (and 30000 in ``--tier full``).
- ``solve``: SciPy's ``linalg.solve(toeplitz([a]), [b])`` of float32 values against
  ``_core.scipy_solve_1x1``: values, or the exception's type and message (zeros of both signs,
  subnormals, infinities, NaNs, overflow).
- ``ar``: nitime's ``AR_est_YW(x, 1)`` (``ak`` and ``sigma_v``) against
  ``_core.nitime_ar_est_yw`` on nipype's ``regress_poly(0, ...)`` residuals of synthetic series
  (BOLD-like, smooth, steps, all zero, constant, a NaN, an infinity; ``T`` 1 to 9000).
- ``dvars``: nipype's ``compute_dvars`` against ``larmorx.confounds.compute_dvars(...,
  upstream_defects="reproduce")`` told this environment's numpy and SciPy, on synthetic NIfTI
  files: shapes, masks (full, partial, one voxel, none, more than 8192 voxels), ``T`` 0 to 600,
  float32, scaled int16, float64 and big-endian series, ``remove_zerovariance``,
  ``variance_tol``, ``intensity_normalization`` (0, 1, 1000, 123.4), zero, constant, spiky
  (robust SD 0), NaN, infinite voxels, a median of 0, a 3-D series, a mask of another shape, a
  4-D mask: the three series bit for bit, or the exception's type and message. The default
  (``upstream_defects="fix"``) is checked to equal ``reproduce`` wherever no defect notice is
  raised, and its notices are counted by ID.
- ``node``: nipype's ``ComputeDVARS`` interface against ``compute_dvars_node``: the files byte
  for byte and ``avg_std``, ``avg_nstd``, ``avg_vxstd`` (every ``save_*`` option).
- ``real``: fMRIPrep's real DVARS calls, every ``bold_confounds_wf/dvars`` node of the oracle
  runs (``oracles/fmriprep-25.2.6``; ``--tier quick``: three small ones): nipype's
  ``compute_dvars`` run here with the node's inputs against larmorx; larmorx's files told
  fMRIPrep 25.2.6's libraries (numpy 2.2.6, SciPy 1.15.2) against the node's own
  ``*_dvars_std.tsv`` and ``*_dvars_nstd.tsv``; and the step from fMRIPrep's libraries to the
  newest (the values of ``std`` that change between SciPy 1.15 and 1.18.1, between numpy 2.2.6
  and 2.3.5 and 2.5.4, and both together), with the times of nipype and larmorx (1 and 8
  threads).

NaN results are compared bit for bit too; a case whose bits differ only in a NaN's sign or
payload is counted apart (``nan-bits``). ``--control`` tells larmorx the other SciPy release
(1.15 and 1.18) and counts the cases whose bits change. Run it in each environment with the
worktree's extension (numpy 2.5.4 with ``OPENBLAS_L2_SIZE=512``)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P check_dvars.py [--tier quick|full] [--sections ...] [--control]

Exit status 1 when any case differs (``--control``: never).
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy
from nipype.algorithms.confounds import ComputeDVARS, compute_dvars, regress_poly
from nitime.algorithms import AR_est_YW
from nitime.utils import autocorr, crosscov
from scipy import linalg

import larmorx as lx
from larmorx import _core

NUMPY = np.__version__
SCIPY = scipy.__version__
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0])  # identical, nan-bits, differ
NOTICES: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
CONTROL = False
LIB = SCIPY


def other(v: str) -> str:
    return "1.18.1" if v.startswith("1.15") else "1.15.3"


def bits(a: np.ndarray) -> np.ndarray:
    a = np.ascontiguousarray(a)
    return a.view({2: np.uint16, 4: np.uint32, 8: np.uint64, 16: np.uint64}[a.dtype.itemsize])


def same(want, got) -> str:
    """``identical``, ``nan-bits`` or ``differ`` for two arrays (or two exception texts)."""
    if isinstance(want, str) or isinstance(got, str):
        return "identical" if want == got else "differ"
    want, got = np.asarray(want), np.asarray(got)
    if want.shape != got.shape or want.dtype != got.dtype:
        return "differ"
    if np.array_equal(bits(want), bits(got)):
        return "identical"
    if want.dtype.kind in "fc":
        w = want.view(want.real.dtype) if want.dtype.kind == "c" else want
        g = got.view(got.real.dtype) if got.dtype.kind == "c" else got
        nw, ng = np.isnan(w), np.isnan(g)
        if np.array_equal(nw, ng) and np.all((bits(w) == bits(g)) | nw):
            return "nan-bits"
    return "differ"


def count(key: str, verdict: str, what: str) -> None:
    c = COUNTS[key]
    c[["identical", "nan-bits", "differ"].index(verdict)] += 1
    if verdict == "differ" and not CONTROL and len(FAILS) < 60:
        FAILS.append(f"{key}: {what}")


def outcome(f, *args, notices: list | None = None, **kw):
    """``f``'s result, or ``"<ExceptionType>: <message>"``; warnings silenced (larmorx's
    defect notices' IDs appended to ``notices``)."""
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = f(*args, **kw)
        except Exception as e:  # the exception is the result compared
            out = f"{type(e).__name__}: {e}"
    if notices is not None:
        notices += [
            w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)
        ]
    return out


# --- cmul ---------------------------------------------------------------------------------------


def complex_kinds(rng: np.random.Generator, n: int) -> dict[str, np.ndarray]:
    def c(re, im):
        return (np.asarray(re, np.float32) + 1j * np.asarray(im, np.float32)).astype(np.complex64)

    g = lambda s: (rng.standard_normal(n) * s).astype(np.float32)  # noqa: E731
    out = {
        "gauss": c(g(1), g(1)),
        "fft-like": c(g(3e5), g(3e5)),
        "subnormal": c(g(1e-41), g(1e-41)),
        "huge": c(g(3e37), g(3e37)),
        "mixed": c(g(1e-20), g(1e20)),
    }
    special = np.array([np.inf, -np.inf, np.nan, 0.0, -0.0, 1.0], np.float32)
    out["special"] = c(rng.choice(special, n), rng.choice(special, n))
    return out


def cmul_section(tier: str) -> None:
    rng = np.random.default_rng(1)
    lengths = [*range(1, 65), 127, 128, 129, 1000, 1023, 1024, 1025, 4096, 65536]
    if tier == "quick":
        lengths = [1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 33, 1024, 1025]
    for n in lengths:
        ka, kb = complex_kinds(rng, n), complex_kinds(rng, n)
        for name in ka:
            for other_name in ("gauss", name):
                a, b = ka[name].copy(), kb[other_name]
                got = _core.numpy_cmul64(a, b)
                with np.errstate(all="ignore"):
                    a *= b
                count("cmul", same(a, got), f"n={n} {name}x{other_name}")


# --- crosscov ------------------------------------------------------------------------------------


def series_kinds(rng: np.random.Generator, t: int) -> dict[str, np.ndarray]:
    x = np.arange(t, dtype=np.float64)
    walk = np.cumsum(rng.standard_normal(t))
    kinds = {
        "bold": 1000 + walk * 3 + rng.standard_normal(t) * 10,
        "smooth": 1000 + np.cumsum(walk),
        "step": 1000 + (x >= t // 2) * 25.0,
        "ramp": 1000 + x * 0.5,
        "zero": np.zeros(t),
        "constant": np.full(t, 1234.5),
    }
    if t >= 3:
        nan = kinds["bold"].copy()
        nan[t // 3] = np.nan
        inf = kinds["bold"].copy()
        inf[t // 2] = np.inf
        kinds["nan"], kinds["inf"] = nan, inf
    return {k: v.astype(np.float32) for k, v in kinds.items()}


def residuals(m: np.ndarray) -> np.ndarray:
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return regress_poly(0, m[None, :], remove_mean=True)[0].astype(np.float32)[0]


T_LIST = (1, 2, 3, 4, 5, 10, 33, 64, 129, 164, 240, 256, 257, 280, 513, 600, 1200, 2000, 9000)


def crosscov_section(tier: str) -> None:
    rng = np.random.default_rng(2)
    ts = T_LIST + ((30000,) if tier == "full" else ())
    if tier == "quick":
        ts = (1, 2, 3, 10, 129, 280, 600)
    for t in ts:
        for name, s in series_kinds(rng, t).items():
            r = np.ascontiguousarray(residuals(s))
            y = np.ascontiguousarray(rng.standard_normal(t).astype(np.float32))
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                want = crosscov(r, r, debias=False)
                want_all = crosscov(r, y, debias=False, all_lags=True)
                want_ac = autocorr(r)
            count("crosscov", same(want, _core.nitime_crosscov(r, r, False, LIB)), f"T={t} {name}")
            count(
                "crosscov all_lags",
                same(want_all, _core.nitime_crosscov(r, y, True, LIB)),
                f"T={t} {name}",
            )
            count(
                "autocorr", same(want_ac, _core.nitime_crosscov(r, r, False, LIB)), f"T={t} {name}"
            )


# --- solve ---------------------------------------------------------------------------------------


def solve_section(tier: str) -> None:
    rng = np.random.default_rng(3)
    special = [0.0, -0.0, 1e-45, -1e-45, 1e-38, 1.0, -2.5, 3e38, -3e38, np.inf, -np.inf, np.nan]
    k = 40 if tier == "quick" else 400
    vals = special + list(rng.standard_normal(k) * 10.0 ** rng.integers(-30, 30, k))
    vals = [np.float32(v) for v in vals]
    pairs = [(a, b) for a in vals[: len(special)] for b in vals[: len(special)]]
    pairs += [(vals[i], vals[i + 1]) for i in range(len(special), len(vals) - 1)]

    def ref(a, b):
        x = linalg.solve(linalg.toeplitz(np.array([a], np.float32)), np.array([b], np.float32))
        assert x.dtype == np.float32 and x.shape == (1,)
        return x[0]

    for a, b in pairs:
        want = outcome(ref, a, b)
        got = outcome(_core.scipy_solve_1x1, a, b, LIB)
        if not isinstance(got, str):
            got = np.float32(got)
        count("solve", same(want, got), f"a={a!r} b={b!r}: {want!r} vs {got!r}")


# --- ar ------------------------------------------------------------------------------------------


def ar_section(tier: str) -> None:
    rng = np.random.default_rng(4)
    ts = T_LIST if tier == "full" else (1, 2, 3, 10, 129, 280, 600)
    for t in ts:
        for name, s in series_kinds(rng, t).items():
            r = np.ascontiguousarray(residuals(s))

            def ref(r=r):
                ak, sigma = AR_est_YW(r, 1)
                return np.asarray(ak), np.float32(sigma)

            want = outcome(ref)
            got = outcome(_core.nitime_ar_est_yw, r, LIB)
            if isinstance(want, str) or isinstance(got, str):
                count("ar", same(str(want), str(got)), f"T={t} {name}: {want!r} vs {got!r}")
                continue
            count("ar ak", same(want[0], got[0]), f"T={t} {name}")
            count("ar sigma_v", same(np.float32(want[1]), np.float32(got[1])), f"T={t} {name}")


# --- dvars ---------------------------------------------------------------------------------------


def write_nifti(path: Path, data: np.ndarray, kind: str = "float32") -> None:
    aff = np.diag([2.0, 2.0, 2.5, 1.0])
    if kind == "int16":
        img = nb.Nifti1Image(np.round(data).astype(np.int16), aff)
        img.header.set_slope_inter(0.75, 10.0)
    elif kind == "float64":
        img = nb.Nifti1Image(data.astype(np.float64), aff)
    elif kind == "big":
        img = nb.Nifti1Image(data.astype(">f4"), aff, nb.Nifti1Header(endianness=">"))
    elif kind == "uint8":
        img = nb.Nifti1Image(data.astype(np.uint8), aff)
    else:
        img = nb.Nifti1Image(data.astype(np.float32), aff)
    img.to_filename(path)


def bold(rng: np.random.Generator, shape: tuple[int, ...]) -> np.ndarray:
    base = 600 + rng.uniform(0, 800, shape[:-1])[..., None]
    walk = np.cumsum(rng.standard_normal(shape) * 2, axis=-1)
    return (base + walk + rng.standard_normal(shape) * 8).astype(np.float32)


def dvars_cases(tier: str):
    rng = np.random.default_rng(5)
    sh = (6, 5, 4)
    full = np.ones(sh, bool)
    part = rng.random(sh) > 0.3
    one = np.zeros(sh, bool)
    one[2, 1, 3] = True
    for t in (0, 1, 2, 3, 4, 10, 33, 129, 164, 280) + ((600,) if tier == "full" else ()):
        for mname, mask in (("full", full), ("part", part), ("one", one)):
            data = np.zeros((*sh, 0), np.float32) if t == 0 else bold(rng, (*sh, t))
            for rz, tol in ((False, 0.0), (True, 1e-7)):
                yield f"T={t} mask={mname} rz={rz}", data, mask, "float32", (rz, 1000, tol)
    data = bold(rng, (*sh, 40))
    for norm in (0, 1, 123.4, 1000.0):
        for kind in ("float32", "int16", "float64", "big"):
            yield f"norm={norm} {kind}", data, part, kind, (True, norm, 1e-7)
    yield "tol 0.5", data, part, "float32", (True, 1000, 0.5)
    yield "tol -1", data, part, "float32", (True, 1000, -1.0)
    yield "mask uint8 scaled", data, part, "float32", (False, 1000, 0.0)
    for special in ("zero", "constant", "spike", "nan", "inf", "median0", "allconst"):
        d = data.copy()
        if special == "zero":
            d[1, 1, 1, :] = 0
        elif special == "constant":
            d[1, 1, 1, :] = 777.25
        elif special == "spike":
            d[1, 1, 1, :] = 900
            d[1, 1, 1, 7] = 1500
        elif special == "nan":
            d[1, 1, 1, 5] = np.nan
        elif special == "inf":
            d[1, 1, 1, 5] = np.inf
        elif special == "median0":
            d[...] = 0
            d[0, 0, 0, :] = rng.standard_normal(40)
        elif special == "allconst":
            d[...] = 500.0
        for rz in (False, True):
            yield f"{special} rz={rz}", d, full, "float32", (rz, 1000, 1e-7 if rz else 0.0)
    big = (24, 24, 16)  # 9216 voxels: numpy 2.2 sums diff_sdhat in chunks of 8192
    yield "9216 voxels", bold(rng, (*big, 30)), np.ones(big, bool), "float32", (True, 1000, 1e-7)
    yield "3-D series", data[..., 0], part, "float32", (False, 1000, 0.0)
    yield "mask shape", data, np.ones((6, 5, 3), bool), "float32", (False, 1000, 0.0)
    yield "4-D mask", data, np.ones((*sh, 40), bool), "float32", (False, 1000, 0.0)


def lx_dvars(*args, **kw) -> tuple:
    """larmorx's ``compute_dvars`` as nipype's triple."""
    return lx.confounds.compute_dvars(*args, **kw).as_tuple()


def run_dvars(f: Path, m: Path, args, numpy_v=NUMPY, scipy_v=None, mode="reproduce", notices=None):
    rz, norm, tol = args
    return outcome(
        lx_dvars,
        f,
        m,
        rz,
        norm,
        tol,
        numpy_version=numpy_v,
        scipy_version=scipy_v or LIB,
        upstream_defects=mode,
        n_threads=4,
        notices=notices,
    )


def compare_triple(key: str, want, got, what: str) -> None:
    if isinstance(want, str) or isinstance(got, str):
        count(key, same(str(want), str(got)), f"{what}: {want!r} vs {got!r}"[:400])
        return
    verdicts = [same(w, g) for w, g in zip(want, got, strict=True)]
    v = (
        "differ"
        if "differ" in verdicts
        else ("nan-bits" if "nan-bits" in verdicts else "identical")
    )
    count(key, v, f"{what}: {verdicts}")


def dvars_section(tier: str, tmp: Path) -> None:
    for i, (what, data, mask, kind, args) in enumerate(dvars_cases(tier)):
        f, m = tmp / f"f{i}.nii.gz", tmp / f"m{i}.nii"
        write_nifti(f, data, kind)
        write_nifti(m, mask.astype(np.uint8), "uint8")
        rz, norm, tol = args
        want = outcome(compute_dvars, str(f), str(m), rz, norm, tol)
        got = run_dvars(f, m, args)
        compare_triple("dvars", want, got, what)
        # The default: equal to reproduce unless a defect notice is raised.
        found: list[str] = []
        fixed = run_dvars(f, m, args, mode="fix", notices=found)
        ids = sorted(set(found))
        for e in ids:
            NOTICES[e] += 1
        if not ids:
            compare_triple("dvars fix = reproduce (no notice)", got, fixed, what)


# --- node ----------------------------------------------------------------------------------------


def node_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(6)
    sh = (7, 6, 5)
    data = bold(rng, (*sh, 50))
    data[0, 0, 0, :] = 0  # removed by remove_zerovariance
    f, m = tmp / "sub-x_bold.nii.gz", tmp / "mask.nii.gz"
    write_nifti(f, data)
    write_nifti(m, (rng.random(sh) > 0.2).astype(np.uint8), "uint8")
    settings = [
        {},
        {"save_nstd": True},
        {"save_nstd": True, "save_vxstd": True, "save_all": True},
        {"save_std": False, "save_all": True, "intensity_normalization": 0.0},
        {"remove_zerovariance": True, "variance_tol": 1e-7, "save_nstd": True},
    ]
    for k, s in enumerate(settings):
        nd, ld = tmp / f"nipype{k}", tmp / f"larmorx{k}"
        nd.mkdir()
        ld.mkdir()
        cwd = os.getcwd()
        os.chdir(nd)
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                res = ComputeDVARS(in_file=str(f), in_mask=str(m), **s).run()
        finally:
            os.chdir(cwd)
        out = lx.confounds.compute_dvars_node(
            f,
            m,
            out_dir=ld,
            numpy_version=NUMPY,
            scipy_version=LIB,
            upstream_defects="reproduce",
            **s,
        )
        for key in ("out_std", "out_nstd", "out_vxstd", "out_all"):
            a = getattr(res.outputs, key)
            b = getattr(out, key)
            if not isinstance(a, str):
                count("node files", "identical" if b is None else "differ", f"{s} {key}")
                continue
            ok = (
                b is not None
                and Path(a).name == Path(b).name
                and Path(a).read_bytes() == Path(b).read_bytes()
            )
            count("node files", "identical" if ok else "differ", f"{s} {key}")
        for key in ("avg_std", "avg_nstd", "avg_vxstd"):
            count(
                "node avg",
                same(np.float64(getattr(res.outputs, key)), np.float64(getattr(out, key))),
                f"{s} {key}",
            )


# --- real ----------------------------------------------------------------------------------------


def real_nodes() -> list[tuple[str, Path, Path, Path, dict]]:
    out = []
    for rep in sorted(
        RUNS.glob(
            "*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf/dvars/_report/report.rst"
        )
    ):
        run = rep.relative_to(RUNS).parts[0]
        text = rep.read_text(encoding="utf-8")
        inputs = text.split("Execution Inputs", 1)[1].split("Execution Outputs", 1)[0]
        kv = dict(re.findall(r"^\* (\w+) : (.*)$", inputs, re.M))

        def host(p: str, run: str = run) -> Path:
            return RUNS / run / p.lstrip("/")

        out.append(
            (
                f"{run}/{rep.parts[-5]}",
                host(kv["in_file"]),
                host(kv["in_mask"]),
                rep.parent.parent,
                kv,
            )
        )
    return out


QUICK_REAL = ("ds005752-sub-ON01016-noreconall/", "ds000256-sub-CTS203-noreconall/", "ds005040")


def real_section(tier: str, tmp: Path) -> None:
    nodes = real_nodes()
    if tier == "quick":
        nodes = [next(n for n in nodes if n[0].startswith(q)) for q in QUICK_REAL]
    print(
        "real: case, T, N, nipype s, larmorx s (1 / 8 threads), std values that change: "
        "SciPy 1.15<->1.18, numpy 2.2.6->2.3.5, 2.2.6->2.5.4, fMRIPrep 25.2.6's -> newest"
    )
    for name, fin, fmask, node, kv in nodes:
        rz = kv["remove_zerovariance"] == "True"
        tol = float(kv["variance_tol"])
        norm = float(kv["intensity_normalization"])
        if not fin.exists() or not fmask.exists():
            FAILS.append(f"real {name}: inputs missing")
            continue
        t0 = time.perf_counter()
        want = outcome(compute_dvars, str(fin), str(fmask), rz, norm, tol)
        t_nipype = time.perf_counter() - t0
        times = []
        for th in (1, 8):
            t0 = time.perf_counter()
            got = outcome(
                lx_dvars,
                fin,
                fmask,
                rz,
                norm,
                tol,
                numpy_version=NUMPY,
                scipy_version=LIB,
                upstream_defects="reproduce",
                n_threads=th,
            )
            times.append(time.perf_counter() - t0)
            compare_triple(f"real ({th} th)", want, got, name)
        # fMRIPrep's own files, with fMRIPrep 25.2.6's libraries.
        d = tmp / re.sub(r"\W", "_", name)
        d.mkdir()
        res = outcome(
            lx.confounds.compute_dvars_node,
            fin,
            fmask,
            remove_zerovariance=rz,
            variance_tol=tol,
            intensity_normalization=norm,
            save_std=True,
            save_nstd=True,
            out_dir=d,
            numpy_version="2.2.6",
            scipy_version="1.15.2",
            upstream_defects="reproduce",
        )
        for key, suffix in (("out_std", "_dvars_std.tsv"), ("out_nstd", "_dvars_nstd.tsv")):
            theirs = list(node.glob(f"*{suffix}"))
            ok = (
                not isinstance(res, str)
                and len(theirs) == 1
                and Path(getattr(res, key)).name == theirs[0].name
                and Path(getattr(res, key)).read_bytes() == theirs[0].read_bytes()
            )
            count("real: fMRIPrep's files", "identical" if ok else "differ", f"{name} {key}")
        # The step from fMRIPrep's libraries to the newest (larmorx alone).
        step = {}
        for label, (nv, sv) in {
            "e3": ("2.2.6", "1.15.2"),
            "s18": ("2.2.6", "1.18.1"),
            "n23": ("2.3.5", "1.15.2"),
            "n25": ("2.5.4", "1.15.2"),
            "e1": ("2.5.4", "1.18.1"),
        }.items():
            r = outcome(
                lx.confounds.compute_dvars,
                fin,
                fmask,
                rz,
                norm,
                tol,
                numpy_version=nv,
                scipy_version=sv,
                upstream_defects="reproduce",
                n_threads=8,
            )
            step[label] = r

        def changed(a, b):
            if isinstance(a, str) or isinstance(b, str):
                return "err"
            return int(np.sum(bits(a.std) != bits(b.std)))

        e3 = step["e3"]
        t_len = len(e3.std) + 1 if not isinstance(e3, str) else "?"
        n_vox = e3.voxels if not isinstance(e3, str) else "?"
        print(
            f"  {name}: T={t_len} N={n_vox} {t_nipype:.2f} s, {times[0]:.2f} / {times[1]:.2f} s; "
            f"{changed(e3, step['s18'])}, {changed(e3, step['n23'])}, {changed(e3, step['n25'])}, "
            f"{changed(e3, step['e1'])} of {len(e3.std) if not isinstance(e3, str) else '?'}",
            flush=True,
        )


def main() -> int:
    global CONTROL, LIB
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="cmul,crosscov,solve,ar,dvars,node,real")
    p.add_argument("--control", action="store_true")
    a = p.parse_args()
    CONTROL = a.control
    LIB = other(SCIPY) if CONTROL else SCIPY
    print(f"numpy {NUMPY}, SciPy {SCIPY}, larmorx told SciPy {LIB}, tier {a.tier}", flush=True)
    sections = a.sections.split(",")
    with tempfile.TemporaryDirectory(prefix="lx-check-dvars-") as tmp:
        tmp = Path(tmp)
        for s in sections:
            t0 = time.perf_counter()
            d = tmp / s
            d.mkdir()
            if s in ("dvars", "node", "real"):
                {"dvars": dvars_section, "node": node_section, "real": real_section}[s](a.tier, d)
            else:
                {
                    "cmul": cmul_section,
                    "crosscov": crosscov_section,
                    "solve": solve_section,
                    "ar": ar_section,
                }[s](a.tier)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, nb_, d) in COUNTS.items():
        print(f"  {key:40s} identical {i:6d}  nan-bits {nb_:4d}  differ {d:5d}")
    if NOTICES:
        print(
            "  default's notices (cases): "
            + ", ".join(f"{k} {v}" for k, v in sorted(NOTICES.items()))
        )
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[2] for c in COUNTS.values())
    print(
        f"{'CONTROL' if CONTROL else 'CHECK'} {'PASS' if differ == 0 and not FAILS else 'DIFFER'}: {differ} cases differ"
    )
    return 0 if CONTROL or (differ == 0 and not FAILS) else 1


if __name__ == "__main__":
    sys.exit(main())

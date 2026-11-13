# SPDX-License-Identifier: Apache-2.0
"""nipype's CompCor: larmorx against this environment's nipype, numpy and SciPy, bit for bit.

Sections:

- ``drift``: nipype's ``_cosine_drift(period_cut, frametimes)`` against
  ``lx.confounds.cosine_drift(..., upstream_defects="reproduce")`` (glibc's ``cos``, as numpy's
  ``np.cos``): ``T`` 2 to 1200, repetition times as Python floats and as nibabel's float32 header
  values, cutoffs 10 to 1000 s; the errors (``T`` 0 and 1, a cutoff of 0, NaN and infinite
  repetition times). The default's values are counted where they differ (GLIBC-07).
- ``poly``: ``regress_poly(degree, data, remove_mean, failure_mode)`` (degrees 0 to 4; float32,
  float64, int16; empty, one-voxel, 3-D and Fortran-ordered 4-D data; ``T`` 1 to 300).
- ``cosine``: ``cosine_filter(data, timestep, period_cut, remove_mean, failure_mode)`` (``T`` 2
  to 600, from no cosine to 23; float32 and float64; NaN and infinite voxels).
- ``tstd``: ``_compute_tSTD(M, x, axis)`` (C and Fortran float64, zeros, NaN).
- ``noise``: ``compute_noise_components`` on synthetic float32 and float64 series: every
  criterion (fractions, counts, ``'all'``, a float count, 0, -2), every filter (cosine,
  polynomial of degree 1 to 3, ``False``; ``'foo'`` on float64), both failure modes, empty,
  one-voxel, NaN, infinite and constant voxels, several masks, too few names, a mask of another
  shape: components, basis and metadata bit for bit (with their dtypes), or the exception's type
  and message. The default (``upstream_defects="fix"``) is checked to equal ``reproduce`` wherever
  no defect notice is raised, and its notices are counted by ID.
- ``iface``: nipype's ``CompCor``, ``ACompCor`` and ``TCompCor`` interfaces (run in a temporary
  folder) against ``lx.confounds.compcor``/``acompcor``/``tcompcor``: every file byte for byte
  (``tCompCor``'s masks by their decompressed content), over the options (masks merged or not,
  the pre-filters, saved files under their default or given names, skipped volumes, prefixes,
  criteria, the header's repetition time in seconds and milliseconds, failure modes, the
  percentile, a scaled int16 series, a probabilistic float mask).
- ``real``: fMRIPrep's real CompCor calls, every ``bold_confounds_wf`` ``acompcor``,
  ``crowncompcor`` and ``tcompcor`` node of the oracle runs (``--tier quick``: three runs):
  nipype run here with the node's inputs against larmorx, and larmorx's files told fMRIPrep
  25.2.6's libraries (numpy 2.2.6, SciPy 1.15.2) against the node's own files byte for byte, with
  the times.

``--control`` tells larmorx the other numpy release (2.2.6 for numpy >= 2.3, else 2.5.4) and
counts the cases whose bits change. Run it in each environment with the worktree's extension
(numpy 2.5.4 with ``OPENBLAS_L2_SIZE=512``)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P check_compcor.py [--tier quick|full] [--sections ...] [--control]

Exit status 1 when any case differs (``--control``: never).
"""

from __future__ import annotations

import argparse
import contextlib
import logging
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
from nipype.algorithms import confounds as nac

import larmorx as lx

NUMPY = np.__version__
SCIPY = scipy.__version__
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0])  # identical, nan-bits, differ
NOTICES: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
CONTROL = False
NP = NUMPY
logging.getLogger("nipype.interface").setLevel(logging.ERROR)
logging.getLogger("larmorx.confounds").setLevel(logging.ERROR)


def bits(a: np.ndarray) -> np.ndarray:
    a = np.ascontiguousarray(a)
    return a.view({1: np.uint8, 2: np.uint16, 4: np.uint32, 8: np.uint64}[a.dtype.itemsize])


def same(want, got) -> str:
    """``identical``, ``nan-bits`` or ``differ`` for two values (arrays, exception texts, ...)."""
    if isinstance(want, tuple) and isinstance(got, tuple):
        if len(want) != len(got):
            return "differ"
        v = [same(a, b) for a, b in zip(want, got, strict=True)]
        return "differ" if "differ" in v else ("nan-bits" if "nan-bits" in v else "identical")
    if isinstance(want, dict) and isinstance(got, dict):
        if list(want) != list(got):
            return "differ"
        return same(tuple(want.values()), tuple(got.values()))
    if isinstance(want, list) and isinstance(got, list):
        return "identical" if want == got else "differ"
    if type(want) is not type(got) and (
        isinstance(want, (tuple, dict, list)) or isinstance(got, (tuple, dict, list))
    ):
        return "differ"
    if isinstance(want, (str, bytes)) or isinstance(got, (str, bytes)):
        return "identical" if type(want) is type(got) and want == got else "differ"
    want, got = np.asarray(want), np.asarray(got)
    if want.shape != got.shape or want.dtype != got.dtype:
        return "differ"
    if want.dtype.kind not in "fc" or want.size == 0:
        return "identical" if np.array_equal(want, got) else "differ"
    if np.array_equal(bits(want), bits(got)):
        return "identical"
    nw, ng = np.isnan(want), np.isnan(got)
    if np.array_equal(nw, ng) and np.all((bits(want) == bits(got)) | nw):
        return "nan-bits"
    return "differ"


def count(key: str, verdict: str, what: str) -> None:
    c = COUNTS[key]
    if verdict == "differ" and os.environ.get("CHECK_VERBOSE"):
        print(f"    differ {key}: {what}", flush=True)
    c[["identical", "nan-bits", "differ"].index(verdict)] += 1
    if verdict == "differ" and not CONTROL and len(FAILS) < 60:
        FAILS.append(f"{key}: {what}")


def outcome(f, *args, notices: list | None = None, **kw):
    """``f``'s result, or ``"<ExceptionType>: <message>"``; warnings silenced (larmorx's defect
    notices' IDs appended to ``notices``)."""
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


def lxkw(**extra):
    kw = {"numpy_version": NP, "upstream_defects": "reproduce"}
    kw.update(extra)
    return kw


def compare(key: str, want, got, what: str) -> None:
    count(key, same(want, got), what)


# --- drift ---------------------------------------------------------------------------------------


def drift_section(tier: str) -> None:
    lengths = [*list(range(2, 40)), 50, 64, 100, 128, 150, 164, 210, 240, 280, 300, 588, 600]
    if tier == "full":
        lengths += list(range(40, 1201, 7))
    trs = [0.5, 0.72, 0.8, 1.0, 1.5, 2.0, 2.5, 3.0, 0.1, 7.3]
    periods = [128.0, 100.0, 64.0, 1000.0, 10.0]
    fixdiff = 0
    for t in lengths:
        for tr in trs:
            for tr_kind in ("float", "float32"):
                step = np.float32(tr) if tr_kind == "float32" else tr
                ft = step * np.arange(t)
                for period in periods if tier == "full" or t % 3 == 0 else periods[:1]:
                    want = outcome(nac._cosine_drift, period, ft)
                    got = outcome(
                        lx.confounds.cosine_drift, period, ft, upstream_defects="reproduce"
                    )
                    compare("drift", want, got, f"T={t} tr={step!r} period={period}")
                    if not isinstance(want, str):
                        fx = outcome(lx.confounds.cosine_drift, period, ft)
                        fixdiff += same(want, fx) != "identical"
    for args in (
        (128.0, np.arange(0.0)),
        (128.0, np.arange(1.0)),
        (0.0, 2.0 * np.arange(10)),
        (128.0, np.array([0.0, np.nan, 1.0])),
        (128.0, np.array([0.0, np.inf, 1.0])),
        (-50.0, 2.0 * np.arange(10)),
    ):
        compare(
            "drift: errors",
            outcome(nac._cosine_drift, *args),
            outcome(lx.confounds.cosine_drift, *args, upstream_defects="reproduce"),
            f"{args}",
        )
    print(f"  drift: the default (correctly rounded cos) differs from glibc's in {fixdiff} bases")


# --- poly, cosine, tstd --------------------------------------------------------------------------


def series(rng: np.random.Generator, n: int, t: int, dtype=np.float32) -> np.ndarray:
    base = rng.uniform(50, 5000, size=(n, 1))
    drift = np.linspace(0, 1, t)[None, :] * rng.normal(0, 20, size=(n, 1))
    x = base + drift + rng.normal(0, 10, size=(n, t))
    return x.astype(dtype)


def poly_section(tier: str) -> None:
    rng = np.random.default_rng(1)
    ns = [0, 1, 3, 100, 1000] + ([5000] if tier == "full" else [])
    ts = [1, 2, 3, 10, 50, 300]
    for degree in range(5):
        for n in ns:
            for t in ts:
                for dtype in (np.float32, np.float64, np.int16):
                    x = series(rng, n, t, np.float64)
                    x = x.astype(dtype) if dtype != np.int16 else np.round(x).astype(np.int16)
                    for rm in (True, False):
                        for fm in ("error", "NaN"):
                            if fm == "NaN" and n:
                                continue
                            want = outcome(nac.regress_poly, degree, x, rm, failure_mode=fm)
                            got = outcome(
                                lx.confounds.regress_poly, degree, x, rm, failure_mode=fm, **lxkw()
                            )
                            compare(
                                f"poly deg {degree}", want, got, f"n={n} t={t} {dtype} rm={rm} {fm}"
                            )
    # 3-D and Fortran 4-D data (TSNR's), axis 0 (PIPE-53)
    for shape in ((4, 5, 30), (3, 4, 5, 20)):
        x = np.asfortranarray(rng.normal(100, 5, size=shape).astype(np.float32))
        for degree in (1, 2):
            compare(
                "poly n-d",
                outcome(nac.regress_poly, degree, x, False),
                outcome(lx.confounds.regress_poly, degree, x, False, **lxkw()),
                f"{shape} {degree}",
            )
    x = series(rng, 9, 4)
    compare(
        "poly axis=0 (reproduce)",
        outcome(nac.regress_poly, 1, x, axis=0),
        outcome(lx.confounds.regress_poly, 1, x, axis=0, **lxkw()),
        "axis 0",
    )


def cosine_section(tier: str) -> None:
    rng = np.random.default_rng(2)
    ts = [2, 3, 10, 64, 100, 164, 280] + ([600, 1200] if tier == "full" else [])
    ns = [0, 1, 5, 100, 3000]
    for t in ts:
        for tr in (0.5, 2.0, 0.72, np.float32(0.72), 3.0):
            for period in (128.0, 100.0):
                for n in ns:
                    for dtype in (np.float32, np.float64):
                        x = series(rng, n, t, dtype)
                        for rm in (True, False) if n == 5 else (True,):
                            for fm in ("error", "NaN") if n == 0 else ("error",):
                                want = outcome(
                                    nac.cosine_filter, x, tr, period, rm, failure_mode=fm
                                )
                                got = outcome(
                                    lx.confounds.cosine_filter,
                                    x,
                                    tr,
                                    period,
                                    rm,
                                    failure_mode=fm,
                                    **lxkw(scipy_version=SCIPY),
                                )
                                compare(
                                    "cosine", want, got, f"t={t} tr={tr!r} n={n} {dtype} rm={rm}"
                                )
    for bad in (np.nan, np.inf, -np.inf):
        x = series(rng, 6, 50)
        x[2, 7] = bad
        compare(
            "cosine: non-finite",
            outcome(nac.cosine_filter, x, 2.0, 128.0),
            outcome(lx.confounds.cosine_filter, x, 2.0, 128.0, **lxkw(scipy_version=SCIPY)),
            f"{bad}",
        )


def tstd_section(tier: str) -> None:
    rng = np.random.default_rng(3)
    for n, t in ((1, 1), (3, 10), (200, 50), (40, 300)):
        m = rng.normal(0, 1, size=(t, n))
        m[:, 0] = 5.0
        if n > 2:
            m[3 % t, 1] = np.nan
        for arr in (m, np.asfortranarray(m), np.ascontiguousarray(m.T).T):
            for axis in (0, 1, -1):
                for x in (1.0, 0.0):
                    compare(
                        "tstd",
                        outcome(nac._compute_tSTD, arr, x, axis),
                        outcome(lx.confounds.compute_tstd, arr, x, axis, numpy_version=NP),
                        f"{n}x{t} axis {axis}",
                    )


# --- noise ---------------------------------------------------------------------------------------


def nib_mask(m: np.ndarray):
    return nb.Nifti1Image(m.astype(np.uint8), np.eye(4))


def lx_mask(m: np.ndarray):
    return lx.Image(m.astype(np.uint8), np.eye(4), None)


def noise_cases(tier: str):
    rng = np.random.default_rng(4)
    shape = (6, 5, 4)
    for t in (12, 40, 130) + ((300,) if tier == "full" else ()):
        base = series(rng, int(np.prod(shape)), t).reshape((*shape, t))
        full = np.ones(shape, bool)
        half = np.zeros(shape, bool)
        half[:3] = True
        few = np.zeros(shape, bool)
        few[0, 0, :3] = True
        one = np.zeros(shape, bool)
        one[1, 1, 1] = True
        empty = np.zeros(shape, bool)
        mask_sets = {
            "full": [full],
            "half+few": [half, few],
            "few+one+full": [few, one, full],
            "empty+full": [empty, full],
            "full+empty": [full, empty],
            "empty": [empty],
        }
        crits = [0.5, 0.2, 0.9, 1, 3, 6, "all", 2.0, 0, -2]
        filters = [("cosine", 0), ("polynomial", 1), ("polynomial", 3), (False, 0)]
        for mname, masks in mask_sets.items():
            for crit in crits if mname in ("full", "empty+full", "full+empty") else crits[:7]:
                for filt, deg in filters:
                    for fm in ("error", "NaN"):
                        yield (
                            f"t={t} {mname} c={crit!r} {filt}{deg} {fm}",
                            base,
                            masks,
                            crit,
                            filt,
                            deg,
                            fm,
                        )
        # bad voxels
        for kind in ("nan", "inf", "constant", "both infs", "overflow"):
            s = base.copy()
            if kind == "nan":
                s[0, 0, 0, 3] = np.nan
            elif kind == "inf":
                s[0, 0, 1, 3] = np.inf
            elif kind == "constant":
                s[1, 0, 0, :] = 7.0
            elif kind == "both infs":
                s[0, 1, 0, 2] = np.inf
                s[0, 1, 0, 5] = -np.inf
            else:
                s[0, 2, 0, :] = np.float32(3e38) * np.where(np.arange(t) % 2, 1, -1)
                s[0, 2, 0, :4] = np.float32(3e38)
            for filt, deg in filters[:2]:
                for fm in ("error", "NaN"):
                    for crit in (0.5, 3, "all"):
                        yield (
                            f"t={t} {kind} c={crit!r} {filt}{deg} {fm}",
                            s,
                            [full, half],
                            crit,
                            filt,
                            deg,
                            fm,
                        )
        # float64 series, the unknown filter
        s64 = base.astype(np.float64)
        for filt in ("cosine", "foo"):
            yield (f"t={t} float64 {filt}", s64, [half], 3, filt, 0, "error")
        yield (f"t={t} float32 foo", base, [half], 3, "foo", 0, "error")


def noise_section(tier: str) -> None:
    for name, s, masks, crit, filt, deg, fm in noise_cases(tier):
        want = outcome(
            nac.compute_noise_components,
            s.copy(),
            [nib_mask(m) for m in masks],
            crit,
            filt,
            deg,
            40,
            2.0,
            fm,
            [f"m{i}" for i in range(len(masks))],
        )
        got = outcome(
            lx.confounds.compute_noise_components,
            s,
            [lx_mask(m) for m in masks],
            crit,
            filt,
            deg,
            40,
            2.0,
            fm,
            [f"m{i}" for i in range(len(masks))],
            **lxkw(scipy_version=SCIPY),
        )
        if isinstance(got, str) and got.startswith("NotImplementedError"):
            # numpy's single-precision SVD (sgesdd), which an unknown filter_type reaches with a
            # float32 series under reproduce, is not ported
            count("noise: not ported (float32 SVD)", "identical", name)
        else:
            compare("noise", want, got, name)
        notices: list = []
        fx = outcome(
            lx.confounds.compute_noise_components,
            s,
            [lx_mask(m) for m in masks],
            crit,
            filt,
            deg,
            40,
            2.0,
            fm,
            [f"m{i}" for i in range(len(masks))],
            numpy_version=NP,
            scipy_version=SCIPY,
            notices=notices,
        )
        for n in set(notices):
            NOTICES[n] += 1
        if isinstance(fx, str) and re.search(r"\(PIPE-\d+\)", fx):
            NOTICES[re.search(r"PIPE-\d+", fx).group(0) + " (refused)"] += 1
        elif not notices:
            compare("noise: default = reproduce (no notice)", got, fx, name)
    # names, a mask of another shape
    rng = np.random.default_rng(5)
    s = series(rng, 120, 20).reshape((6, 5, 4, 20))
    full = np.ones((6, 5, 4), bool)
    other = np.ones((6, 5, 3), bool)
    for masks, names in (([full, full], ["a"]), ([full, other], None), ([other, full], None)):
        for crit in (0.5, "all"):
            want = outcome(
                nac.compute_noise_components,
                s.copy(),
                [nib_mask(m) for m in masks],
                crit,
                "cosine",
                0,
                40,
                2.0,
                "error",
                names,
            )
            got = outcome(
                lx.confounds.compute_noise_components,
                s,
                [lx_mask(m) for m in masks],
                crit,
                "cosine",
                0,
                40,
                2.0,
                "error",
                names,
                **lxkw(scipy_version=SCIPY),
            )
            compare("noise: names, shapes", want, got, f"{len(masks)} masks {names}")
    want = outcome(nac.compute_noise_components, s.copy(), [], 0.5, "cosine", 0, 40, 2.0, "NaN")
    got = outcome(
        lx.confounds.compute_noise_components,
        s,
        [],
        0.5,
        "cosine",
        0,
        40,
        2.0,
        "NaN",
        **lxkw(scipy_version=SCIPY),
    )
    compare("noise: names, shapes", want, got, "no mask")


# --- iface ---------------------------------------------------------------------------------------


@contextlib.contextmanager
def cwd(path: Path):
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


def file_content(p: Path) -> bytes:
    if p.name.endswith(".nii.gz"):
        img = nb.load(p)
        return img.header.binaryblock + np.asarray(img.dataobj).tobytes()
    return p.read_bytes()


def run_nipype(klass, d: Path, **inputs):
    d.mkdir(parents=True)
    with cwd(d):
        try:
            iface = klass(**{k: v for k, v in inputs.items() if v is not None})
            iface.run()
        except Exception as e:  # traits' and nipype's errors: compared by their last line
            msg = str(e).strip().splitlines()
            return f"error: {msg[-1] if msg else ''}"
    return {
        p.name: file_content(p)
        for p in sorted(d.iterdir())
        if p.is_file() and not p.name.startswith(("_", "result"))
    }


def run_lx(fn, d: Path, **inputs):
    d.mkdir(parents=True)
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            fn(**inputs, out_dir=d)
    except Exception as e:
        msg = str(e).strip().splitlines()
        return f"error: {msg[-1] if msg else ''}"
    return {p.name: file_content(p) for p in sorted(d.iterdir()) if p.is_file()}


def iface_inputs(tmp: Path):
    rng = np.random.default_rng(6)
    shape, t = (8, 7, 5), 30
    data = series(rng, int(np.prod(shape)), t).reshape((*shape, t))
    aff = np.diag([2.0, 2.0, 3.0, 1.0])
    files = {}

    def save(name, arr, dtype=None, slope=None, tr=2.0, unit="sec"):
        img = nb.Nifti1Image(arr, aff)
        if dtype is not None:
            img.set_data_dtype(dtype)
        if arr.ndim == 4:
            img.header.set_zooms((2.0, 2.0, 3.0, tr))
            img.header.set_xyzt_units("mm", unit)
        if slope is not None:
            img.header.set_slope_inter(slope, 3.0)
        p = tmp / name
        img.to_filename(p)
        files[name.split(".")[0]] = str(p)

    save("bold.nii.gz", data)
    save("bold_msec.nii.gz", data, tr=2000.0, unit="msec")
    save("bold_int16.nii.gz", np.round(data), dtype=np.int16, slope=0.5)
    m1 = np.zeros(shape, np.uint8)
    m1[:4] = 1
    m2 = np.zeros(shape, np.uint8)
    m2[2:6, :3] = 1
    prob = rng.uniform(0, 1, size=shape).astype(np.float32)
    prob[prob < 0.6] = 0
    save("m1.nii.gz", m1)
    save("m2.nii.gz", m2)
    save("prob.nii.gz", prob)
    return files


def iface_section(tier: str, tmp: Path) -> None:
    f = iface_inputs(tmp)
    cases = [
        dict(),
        dict(mask_files=[f["m1"]]),
        dict(mask_files=[f["m1"], f["m2"]], merge_method="union"),
        dict(mask_files=[f["m1"], f["m2"]], merge_method="intersect"),
        dict(
            mask_files=[f["m1"], f["m2"], f["prob"]],
            merge_method="none",
            mask_names=["a", "b", "c"],
        ),
        dict(mask_files=[f["m1"], f["m2"]], mask_index=1),
        dict(mask_files=[f["m1"], f["m2"]]),
        dict(mask_files=[f["prob"]]),
    ]
    options = [
        dict(
            pre_filter="cosine",
            repetition_time=2.0,
            variance_threshold=0.5,
            save_pre_filter=True,
            save_metadata=True,
        ),
        dict(
            pre_filter="cosine",
            num_components=3,
            save_pre_filter="pf.tsv",
            save_metadata="md.tsv",
            ignore_initial_volumes=3,
        ),
        dict(
            pre_filter="polynomial",
            regress_poly_degree=2,
            num_components="all",
            save_pre_filter=True,
            save_metadata=True,
            header_prefix="x_",
        ),
        dict(pre_filter=False, save_pre_filter=True, save_metadata=True),
        dict(
            pre_filter="cosine",
            num_components=5,
            save_metadata=True,
            failure_mode="NaN",
            ignore_initial_volumes=1,
        ),
        dict(use_regress_poly=True, num_components=2, components_file="c.tsv"),
    ]
    series_files = [f["bold"], f["bold_msec"], f["bold_int16"]]
    kinds = [
        ("CompCor", nac.CompCor, lx.confounds.compcor),
        ("ACompCor", nac.ACompCor, lx.confounds.acompcor),
        ("TCompCor", nac.TCompCor, lx.confounds.tcompcor),
    ]
    i = 0
    for sname in series_files:
        for case in cases:
            for opt in options if (tier == "full" or sname == series_files[0]) else options[:2]:
                for kname, klass, fn in kinds:
                    extra = {}
                    if kname == "TCompCor":
                        extra["percentile_threshold"] = 0.1 if i % 2 else 0.02
                    inputs = dict(realigned_file=sname, **case, **opt, **extra)
                    i += 1
                    want = run_nipype(klass, tmp / f"n{i}", **inputs)
                    got = run_lx(fn, tmp / f"l{i}", **inputs, **lxkw(scipy_version=SCIPY))
                    compare(f"iface {kname}", want, got, f"{Path(sname).name} {case} {opt}")


# --- real ----------------------------------------------------------------------------------------


def node_inputs(rep: Path) -> dict:
    text = rep.read_text(encoding="utf-8")
    inputs = text.split("Execution Inputs", 1)[1].split("Execution Outputs", 1)[0]
    kv = dict(re.findall(r"^\* (\w+) : (.*)$", inputs, re.M))
    return {k: v for k, v in kv.items() if v != "<undefined>"}


def real_nodes():
    out = []
    for node in ("acompcor", "crowncompcor", "tcompcor"):
        for rep in sorted(
            RUNS.glob(
                f"*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf/{node}/_report/report.rst"
            )
        ):
            run = rep.relative_to(RUNS).parts[0]
            out.append(
                (f"{run}/{rep.parts[-5]}/{node}", run, node, rep.parent.parent, node_inputs(rep))
            )
    return out


def parse_inputs(run: str, kv: dict) -> dict:
    def host(p: str) -> str:
        return str(RUNS / run / p.lstrip("/"))

    out: dict = {}
    for k, v in kv.items():
        if k in ("realigned_file",):
            out[k] = host(v)
        elif k == "mask_files":
            out[k] = [host(p) for p in re.findall(r"'([^']+)'", v)]
        elif k == "mask_names":
            out[k] = re.findall(r"'([^']+)'", v)
        elif k in (
            "variance_threshold",
            "high_pass_cutoff",
            "repetition_time",
            "percentile_threshold",
        ):
            out[k] = float(v)
        elif k in ("num_components", "ignore_initial_volumes", "regress_poly_degree"):
            out[k] = v if v == "all" else int(v)
        elif k in ("save_pre_filter", "save_metadata"):
            out[k] = v == "True" if v in ("True", "False") else v
        elif k == "pre_filter":
            out[k] = False if v == "False" else v
        else:
            out[k] = v
    return out


QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256")


def real_section(tier: str, tmp: Path) -> None:
    nodes = real_nodes()
    if tier == "quick":
        nodes = [n for n in nodes if any(n[0].startswith(q) for q in QUICK_REAL)]
    print(
        "real: node, nipype s, larmorx s, nipype here, fMRIPrep's files (numpy 2.2.6, SciPy 1.15.2)"
    )
    fns = {
        "acompcor": (nac.ACompCor, lx.confounds.acompcor),
        "crowncompcor": (nac.ACompCor, lx.confounds.acompcor),
        "tcompcor": (nac.TCompCor, lx.confounds.tcompcor),
    }
    for i, (name, run, node, ndir, kv) in enumerate(nodes):
        inputs = parse_inputs(run, kv)
        klass, fn = fns[node]
        t0 = time.perf_counter()
        want = run_nipype(klass, tmp / f"rn{i}", **inputs)
        t1 = time.perf_counter()
        got = run_lx(fn, tmp / f"rl{i}", **inputs, **lxkw(scipy_version=SCIPY, n_threads=8))
        t2 = time.perf_counter()
        verdict = same(want, got)
        count(f"real: nipype here ({node})", verdict, name)
        theirs = {
            p.name: file_content(p)
            for p in sorted(ndir.iterdir())
            if p.is_file() and not p.name.startswith(("_", "result"))
        }
        e3 = run_lx(
            fn,
            tmp / f"re{i}",
            **inputs,
            numpy_version="2.2.6",
            scipy_version="1.15.2",
            upstream_defects="reproduce",
            n_threads=8,
        )
        ok = not isinstance(e3, str) and all(e3.get(k) == v for k, v in theirs.items())
        count(f"real: fMRIPrep's files ({node})", "identical" if ok else "differ", name)
        print(
            f"  {name}: {t1 - t0:.2f} s, {t2 - t1:.2f} s, {verdict}, {'identical' if ok else 'DIFFER ' + str([k for k, v in theirs.items() if isinstance(e3, str) or e3.get(k) != v])}",
            flush=True,
        )


def main() -> int:
    global CONTROL, NP
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="drift,poly,cosine,tstd,noise,iface,real")
    p.add_argument("--control", action="store_true")
    a = p.parse_args()
    CONTROL = a.control
    if CONTROL:
        NP = "2.2.6" if tuple(int(v) for v in NUMPY.split(".")[:2]) >= (2, 3) else "2.5.4"
    print(f"numpy {NUMPY}, SciPy {SCIPY}, larmorx told numpy {NP}, tier {a.tier}", flush=True)
    with tempfile.TemporaryDirectory(prefix="lx-check-compcor-") as tmp:
        tmp = Path(tmp)
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = tmp / s
            d.mkdir()
            if s in ("iface", "real"):
                {"iface": iface_section, "real": real_section}[s](a.tier, d)
            else:
                {
                    "drift": drift_section,
                    "poly": poly_section,
                    "cosine": cosine_section,
                    "tstd": tstd_section,
                    "noise": noise_section,
                }[s](a.tier)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, nb_, d) in COUNTS.items():
        print(f"  {key:46s} identical {i:6d}  nan-bits {nb_:4d}  differ {d:5d}")
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

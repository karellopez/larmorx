# SPDX-License-Identifier: Apache-2.0
"""numpy's reductions and order statistics at the confounds' call sites: larmorx against numpy.

Each section rebuilds the operands of one call site the way nipype 1.12.0
(``algorithms/confounds.py``), niworkflows 1.15.4 and fMRIPrep 25.2.7 build them (images written
and loaded with nibabel, so the layouts are numpy's own: ``probe_reduction_layouts.py``), runs the
original expression with this environment's numpy, and compares its bits with larmorx's
(``larmorx._core``'s ``numpy_*`` bindings over ``larmorx_core::numpy``, told this numpy's
release). Sections:

- ``dvars``: ``compute_dvars``' ``np.median(mfunc)``, ``np.percentile(mfunc, 75/25, axis=1,
  method="lower")``, ``diff_sdhat.mean()``, ``np.diff(mfunc, axis=1)``,
  ``np.square(func_diff).mean(axis=0)``, the voxelwise ``mean(axis=0)`` in float64, and
  ``ComputeDVARS``' ``np.mean(dvars, axis=1)``;
- ``fd``: nipype's ``FramewiseDisplacement``: ``np.abs(diff).sum(axis=1)``, ``fd_res.mean()``;
- ``noise``: ``compute_noise_components``' ``np.sum(voxel_timecourses, axis=1)``,
  ``_compute_tSTD(M, 1.0)`` (``np.std(M, axis=0)`` of the Fortran-ordered ``M``), ``(s**2) /
  np.sum(s**2)``, ``np.cumsum``, ``np.searchsorted``, ``_full_rank``'s ``s.max()``/``s.min()``;
- ``tcompcor``: ``_compute_tSTD(regress_poly(2, timeseries[mask, :])[0], 0, axis=-1)``, the
  threshold ``np.percentile(tSTD, np.int64(...))``, ``regress_poly``'s
  ``Legendre.basis(i)(np.linspace(-1, 1, T))``;
- ``tsnr``: ``np.nan_to_num``, ``np.mean``/``np.std(data, axis=3)`` with and without
  ``regress_poly``;
- ``nss``: nipype's ``NonSteadyStateDetector``: ``dataobj[..., :50].mean(axis=0)`` three times
  (float32, int16, scaled int16 and float64 files) and ``is_outlier`` against
  ``lx.niworkflows.bold.is_outlier``;
- ``signals``: niworkflows' ``SignalExtraction``: ``data[mask, :].mean(axis=0)``;
- ``mask2vf``: fMRIPrep's ``np.percentile(data[data > 0], 99)`` and ``np.clip(data / max_data,
  0, 1)``; ``motion``: fMRIPrep 25.2.7's ``data.min()``; ``deriv``: niworkflows'
  ``np.diff(data[variables], n=o, axis=0)``;
- ``general``: the rules beyond the call sites: every axis of C- and Fortran-ordered arrays of
  one to four axes, both types, lengths around 8, 128 and 8192, special values; the rank
  percentiles, ``min``/``max`` (the ``-0.0``/``+0.0`` ties counted apart: numpy's sign there
  depends on the CPU's SIMD kernel), ``cumsum``, ``searchsorted`` (sorted and unsorted),
  ``nan_to_num``, ``diff``, ``linspace``, the Legendre basis to degree 8.

``--control`` tells larmorx the other numpy release (2.2 against 2.3 and later, 2.3 against
2.5) and counts the cases whose bits change: where the releases differ. ``--tier full`` adds
the real-size cases (CompCor's crowns, ds005454's brain mask, a 8999-volume series). Run it in
each environment with the worktree's extension (``PYTHONPATH=<worktree>/python``, plus
``<worktree>/python`` for ``lx.niworkflows``)::

    python -P check_numpy_reductions.py [--tier quick|full] [--control]

Exit status 1 when any case differs (``--control``: never).
"""

from __future__ import annotations

import argparse
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from pathlib import Path

import nibabel as nb
import numpy as np
from numpy.polynomial import Legendre

from larmorx import _core

NPV = np.__version__
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])
FAILS: list[str] = []
NOTES: list[str] = []
CONTROL_KINDS: dict[str, int] = defaultdict(int)
CONTROL = False
OTHER = NPV


def other_release(v: str) -> str:
    major, minor = (int(x) for x in v.split(".")[:2])
    return "2.5.4" if (major, minor) <= (2, 3) else "2.3.5"


def ver() -> str:
    return OTHER if CONTROL else NPV


def ver22() -> str:
    """The release for the sum orders and legval (2.2 against the others)."""
    if not CONTROL:
        return NPV
    major, minor = (int(x) for x in NPV.split(".")[:2])
    return "2.3.5" if (major, minor) <= (2, 2) else "2.2.6"


def bits(x) -> tuple:
    a = np.asarray(x)
    if a.dtype.kind == "f":
        u = {2: np.uint16, 4: np.uint32, 8: np.uint64}[a.dtype.itemsize]
        return (a.dtype.str[1:], a.shape, a.view(u).tobytes())
    return (a.dtype.str[1:], a.shape, a.tobytes())


def check(section: str, label: str, want, got) -> None:
    c = COUNTS[section]
    c[1] += 1
    if bits(want) == bits(got):
        c[0] += 1
    elif CONTROL:
        CONTROL_KINDS[f"{section}: {label.split(' ')[0]}"] += 1
    elif len(FAILS) < 60:
        w, g = np.asarray(want), np.asarray(got)
        FAILS.append(
            f"[{section}] {label}: numpy {w.dtype}{w.shape} vs larmorx {g.dtype}{g.shape}"
            + (
                f", first differences {np.flatnonzero(w.ravel() != g.ravel())[:5]}"
                if w.shape == g.shape and w.dtype == g.dtype
                else ""
            )
        )


def check_raises(section: str, label: str, fn_np, fn_lx) -> None:
    def kind(fn):
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                fn()
        except Exception as e:  # the exception's type is the comparison
            return type(e).__name__
        return "no error"

    c = COUNTS[section]
    c[1] += 1
    a, b = kind(fn_np), kind(fn_lx)
    if a == b and a != "no error":
        c[0] += 1
    elif not CONTROL:
        FAILS.append(f"[{section}] {label}: numpy {a}, larmorx {b}")


def red(a, op, axis=None, ddof=0, v=None):
    return _core.numpy_reduce(a, op, axis=axis, numpy_version=v or ver22(), ddof=ddof)


# ---------------------------------------------------------------- inputs


def series(rng, shape, kind: str):
    """A BOLD-like series: a baseline per voxel, drifts, noise; some special voxels."""
    x, y, z, t = shape
    base = rng.uniform(200, 3000, size=(x, y, z, 1))
    drift = np.linspace(0, 1, t)[None, None, None, :] * rng.uniform(-30, 30, size=(x, y, z, 1))
    noise = rng.normal(0, 1, size=shape) * rng.uniform(1, 40, size=(x, y, z, 1))
    d = base + drift + noise
    d[0, 0, 0, :] = 1234.5  # a constant voxel
    d[-1, -1, -1, :] = 0.0  # a zero voxel
    if kind == "i2":
        return np.clip(np.round(d), -32768, 32767).astype("<i2")
    return d.astype("<f4" if kind == "f4" else ">f4" if kind == ">f4" else "<f8")


def write_image(tmp: Path, name: str, data, scaled: bool = False) -> Path:
    img = nb.Nifti1Image(data, np.diag([2.0, 2.0, 2.4, 1.0]))
    img.header.set_data_dtype(data.dtype)
    if scaled:
        img.header.set_slope_inter(0.37, 11.0)
    fn = tmp / f"{name}.nii.gz"
    img.to_filename(fn)
    return fn


def mask_of(rng, shape, frac=0.6):
    m = rng.random(shape) < frac
    m[0, 0, 0] = True
    m[-1, -1, -1] = True
    return m


# ---------------------------------------------------------------- sections


def sec_dvars(rng, tmp, tier) -> None:
    cases = [
        ((9, 8, 7, 40), "f4"),
        ((8, 7, 6, 25), ">f4"),
        ((10, 9, 8, 33), "i2"),
        ((6, 5, 4, 2), "f4"),
        ((5, 4, 3, 3), "f4"),
    ]
    if tier == "full":
        cases += [((48, 48, 36, 280), "f4"), ((12, 10, 8, 600), "f4")]
    for shape, kind in cases:
        data = series(rng, shape, kind)
        fn = write_image(tmp, f"dv_{kind}_{shape[3]}", data, scaled=(kind == "i2"))
        mfn = write_image(tmp, "dv_mask", mask_of(rng, shape[:3]).astype("uint8"))
        # nipype compute_dvars, verbatim
        func = np.float32(nb.load(fn).dataobj)
        mask = np.bool_(nb.load(mfn).dataobj)
        mfunc = func[mask]
        lab = f"{shape} {kind}"
        med = np.median(mfunc)
        check("dvars", f"median {lab}", med, _core.numpy_median(mfunc))
        mfunc = (mfunc / med) * 1000
        for q in (75, 25):
            want = np.percentile(mfunc, q, axis=1, method="lower")
            got = _core.numpy_percentile_rank(mfunc, q, axis=1, method="lower", numpy_version=ver())
            check("dvars", f"percentile {q} lower axis 1 {lab}", want, got)
        func_sd = (
            np.percentile(mfunc, 75, axis=1, method="lower")
            - np.percentile(mfunc, 25, axis=1, method="lower")
        ) / 1.349
        keep = func_sd > 1e-7
        mfunc = mfunc[keep, :]
        func_sd = func_sd[keep]
        # diff_sdhat: float64 from the AR(1) estimates (synthetic here: 2e ports AR_est_YW)
        ar1 = rng.uniform(-0.2, 0.95, size=(mfunc.shape[0], 1)).astype(np.float32)
        diff_sdhat = np.squeeze(np.sqrt(((1 - ar1) * 2).tolist())) * func_sd
        check("dvars", f"diff_sdhat.mean() {lab}", diff_sdhat.mean(), red(diff_sdhat, "mean"))
        func_diff = np.diff(mfunc, axis=1)
        check("dvars", f"np.diff(mfunc, axis=1) {lab}", func_diff, _core.numpy_diff(mfunc, 1, 1))
        sq = np.square(func_diff)
        dvars_nstd = np.sqrt(sq.mean(axis=0))
        check(
            "dvars",
            f"square(func_diff).mean(axis=0) {lab}",
            sq.mean(axis=0),
            red(sq, "mean", axis=0),
        )
        dvars_stdz = dvars_nstd / diff_sdhat.mean()
        with np.errstate(all="ignore"):
            vx = np.square(func_diff / np.array([diff_sdhat] * func_diff.shape[-1]).T)
        check("dvars", f"voxelwise mean(axis=0) {lab}", vx.mean(axis=0), red(vx, "mean", axis=0))
        dvars_vx = np.sqrt(vx.mean(axis=0))
        dv = (dvars_stdz, dvars_nstd, dvars_vx)
        check(
            "dvars",
            f"np.mean(dvars, axis=1) {lab}",
            np.mean(dv, axis=1),
            red(np.asarray(dv), "mean", axis=1),
        )
    # a NaN voxel: the median and every percentile become NaN
    data = series(rng, (6, 5, 4, 20), "f4")
    data[2, 2, 2, 5] = np.nan
    m = data.reshape(-1, 20)
    check("dvars", "median with a NaN", np.median(m), _core.numpy_median(m))
    check(
        "dvars",
        "percentile lower with a NaN row",
        np.percentile(m, 75, axis=1, method="lower"),
        _core.numpy_percentile_rank(m, 75, axis=1, method="lower", numpy_version=ver()),
    )
    # ComputeDVARS over a long series (numpy 2.2 sums rows of more than 8192 in chunks)
    for t in [9000] if tier == "full" else [8200]:
        dv = (rng.random(t - 1) * 3, (rng.random(t - 1) * 50).astype(np.float32), rng.random(t - 1))
        check(
            "dvars",
            f"np.mean(dvars, axis=1) T={t}",
            np.mean(dv, axis=1),
            red(np.asarray(dv), "mean", axis=1),
        )
        d = rng.random(20000) * 2
        check("dvars", "diff_sdhat.mean() 20000 voxels", d.mean(), red(d, "mean"))


def sec_fd(rng, tmp, tier) -> None:
    for t in (2, 3, 10, 150, 600) + ((9001,) if tier == "full" else ()):
        par = rng.normal(0, 0.01, size=(t, 6)) * np.array([1, 1, 1, 0.01, 0.01, 0.01])
        fn = tmp / "fd.par"
        np.savetxt(fn, par)
        mpars = np.loadtxt(fn, ndmin=2)
        diff = mpars[:-1, :6] - mpars[1:, :6]
        diff[:, 3:6] *= 50.0
        fd_res = np.abs(diff).sum(axis=1)
        check("fd", f"abs(diff).sum(axis=1) T={t}", fd_res, red(np.abs(diff), "sum", axis=1))
        check("fd", f"fd_res.mean() T={t}", fd_res.mean(), red(fd_res, "mean"))
    fd_res = np.array([np.nan, 0.1, 0.2])
    check("fd", "mean with NaN", fd_res.mean(), red(fd_res, "mean"))


def sec_noise(rng, tmp, tier) -> None:
    from nipype.algorithms.confounds import _compute_tSTD

    shapes = [((9, 8, 7, 40), "f4"), ((7, 6, 5, 12), ">f4"), ((4, 3, 3, 30), "f4")]
    if tier == "full":
        shapes += [((60, 60, 40, 280), "f4")]
    for shape, kind in shapes:
        data = series(rng, shape, kind)
        data[1, 1, 1, 3] = np.nan if data.dtype.kind == "f" else data[1, 1, 1, 3]
        fn = write_image(tmp, f"nc_{kind}_{shape[3]}", data)
        imgseries = nb.load(fn).get_fdata(dtype=np.float32)
        mask = mask_of(rng, shape[:3])
        vt = imgseries[mask, :]
        lab = f"{shape} {kind}"
        check("noise", f"np.sum(vt, axis=1) {lab}", np.sum(vt, axis=1), red(vt, "sum", axis=1))
        vt[np.isnan(np.sum(vt, axis=1)), :] = 0
        T = vt.shape[1]
        X = np.ones((T, 3))
        X[:, 0] = np.cos(np.pi / T * (np.arange(T) + 0.5))
        X[:, 1] = np.cos(2 * np.pi / T * (np.arange(T) + 0.5))
        betas = np.linalg.lstsq(X, vt.T, rcond=None)[0]
        res = (vt - X.dot(betas).T).reshape(vt.shape)
        M = res.T
        want = _compute_tSTD(M, 1.0)
        got = np.array(red(M, "std", axis=0))
        got[got == 0] = 1.0
        got[np.isnan(got)] = 1.0
        check("noise", f"_compute_tSTD(M, 1.0) {lab}", want, got)
        M = M / want
        s = np.linalg.svd(M, full_matrices=False, compute_uv=False)
        sq = s**2
        check("noise", f"np.sum(s**2) {lab}", np.sum(sq), red(sq, "sum"))
        ve = sq / np.sum(sq)
        cv = np.cumsum(ve)
        check("noise", f"np.cumsum(ve) {lab}", cv, _core.numpy_cumsum(ve))
        for crit in (0.5, 0.25, 0.9, 0.999999, 1.0, 1e-9):
            want = np.searchsorted(cv, crit) + 1
            got = _core.numpy_searchsorted(cv, crit, "left", ver()) + 1
            check("noise", f"searchsorted({crit}) {lab}", np.intp(want), np.intp(got))
        check("noise", f"s.max() {lab}", s.max(), np.float64(_core.numpy_extreme(s, "max")))
        check("noise", f"s.min() {lab}", s.min(), np.float64(_core.numpy_extreme(s, "min")))
    # the SVD failure's NaN fill (float32): every searchsorted gives 0
    s = np.full(12, np.nan, dtype=np.float32)
    sq = s**2
    check("noise", "np.sum(NaN float32)", np.sum(sq), red(sq, "sum"))
    cv = np.cumsum(sq / np.sum(sq))
    check("noise", "np.cumsum(NaN float32)", cv, _core.numpy_cumsum(sq / np.sum(sq)))
    check(
        "noise",
        "searchsorted(NaN, 0.5)",
        np.intp(np.searchsorted(cv, 0.5)),
        np.intp(_core.numpy_searchsorted(cv.astype(np.float64), 0.5, "left", ver())),
    )


def sec_tcompcor(rng, tmp, tier) -> None:
    from nipype.algorithms.confounds import _compute_tSTD, regress_poly

    cases = [((9, 8, 7, 40), "f4"), ((8, 7, 6, 25), "i2"), ((7, 6, 5, 12), ">f4")]
    if tier == "full":
        cases += [((64, 64, 40, 280), "f4")]
    for shape, kind in cases:
        data = series(rng, shape, kind)
        fn = write_image(tmp, f"tc_{kind}_{shape[3]}", data)
        timeseries = np.asanyarray(nb.load(fn).dataobj)
        mask = mask_of(rng, shape[:3])
        imgseries = timeseries[mask, :]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            imgseries = regress_poly(2, imgseries)[0]
        lab = f"{shape} {kind}"
        tstd = _compute_tSTD(imgseries, 0, axis=-1)
        got = np.array(red(imgseries, "std", axis=imgseries.ndim - 1))
        got[got == 0] = 0
        got[np.isnan(got)] = 0
        check("tcompcor", f"_compute_tSTD(imgseries, 0, axis=-1) {lab}", tstd, got)
        for pt in (0.02, 0.05, 0.5, 0.01, 0.0, 1.0):
            q = np.round(100.0 * (1.0 - pt)).astype(int)
            want = np.percentile(tstd, q)
            got = np.float64(_core.numpy_percentile(tstd, [float(q)])[0])
            check("tcompcor", f"percentile(tSTD, int64 {q}) {lab}", want, got)
    for T in (1, 2, 3, 4, 5, 7, 10, 25, 40, 128, 129, 280, 600, 1201):
        x = np.linspace(-1, 1, T)
        check("tcompcor", f"linspace(-1, 1, {T})", x, _core.numpy_linspace(-1.0, 1.0, T))
        for deg in (1, 2):
            want = Legendre.basis(deg)(x)
            got = _core.numpy_legendre_basis(deg, x, ver22())
            check("tcompcor", f"Legendre.basis({deg}) T={T}", want, got)


def sec_tsnr(rng, tmp, tier) -> None:
    from nipype.algorithms.confounds import regress_poly

    cases = [(9, 8, 7, 30), (5, 4, 3, 2)] + ([(64, 64, 40, 200)] if tier == "full" else [])
    for shape in cases:
        d = series(rng, shape, "f4")
        d[1, 2, 1, 0] = np.nan
        d[2, 1, 1, 1] = np.inf
        d[1, 1, 2, 1] = -np.inf
        d[0, 1, 1, 0] = -0.0
        fn = write_image(tmp, "tsnr", d)
        vol = nb.load(fn)
        data = np.concatenate(
            [vol.get_fdata(dtype=np.float32).reshape((*vol.shape[:3], -1))], axis=3
        )
        n2n = np.nan_to_num(data)
        check("tsnr", f"nan_to_num {shape}", n2n, _core.numpy_nan_to_num(data))
        data = n2n
        with np.errstate(all="ignore"):
            check("tsnr", f"mean axis 3 {shape}", np.mean(data, axis=3), red(data, "mean", axis=3))
            check("tsnr", f"std axis 3 {shape}", np.std(data, axis=3), red(data, "std", axis=3))
            rp = regress_poly(2, data, remove_mean=False)[0]
            check(
                "tsnr",
                f"mean axis 3 after regress_poly {shape}",
                np.mean(rp, axis=3),
                red(rp, "mean", axis=3),
            )
            check(
                "tsnr",
                f"std axis 3 after regress_poly {shape}",
                np.std(rp, axis=3),
                red(rp, "std", axis=3),
            )


def sec_nss(rng, tmp, tier) -> None:
    from nipype.algorithms.confounds import is_outlier

    from larmorx.niworkflows.bold import is_outlier as lx_is_outlier

    cases = [
        ((9, 8, 7, 60), "f4", False),
        ((10, 9, 8, 40), "i2", False),
        ((10, 9, 8, 40), "i2", True),
        ((8, 7, 6, 55), "f8", False),
        ((7, 6, 5, 30), ">f4", False),
    ]
    if tier == "full":
        cases += [((96, 96, 60, 60), "f4", False), ((104, 104, 72, 50), "i2", True)]
    for shape, kind, scaled in cases:
        data = series(rng, shape, kind)
        data[..., :3] = (data[..., :3] * 1.6).astype(data.dtype)  # non-steady volumes
        fn = write_image(tmp, f"nss_{kind}", data, scaled=scaled)
        in_nii = nb.load(fn)
        g = in_nii.dataobj[:, :, :, :50]
        lab = f"{shape} {kind}{' scaled' if scaled else ''}"
        m0, m1, m2 = (
            g.mean(axis=0),
            g.mean(axis=0).mean(axis=0),
            g.mean(axis=0).mean(axis=0).mean(axis=0),
        )
        # numpy sums integers in float64: exact, in any order
        gf = g.astype(np.float64, order="K") if g.dtype.kind == "i" else g
        l0 = red(gf, "mean", axis=0)
        check("nss", f"mean(axis=0) {lab}", m0, l0)
        l1 = red(np.asarray(l0, order="K"), "mean", axis=0)
        check("nss", f".mean(axis=0).mean(axis=0) {lab}", m1, l1)
        l2 = red(np.asarray(l1, order="K"), "mean", axis=0)
        check("nss", f"global signal {lab}", m2, l2)
        check(
            "nss",
            f"is_outlier {lab}",
            np.intp(is_outlier(m2)),
            np.intp(lx_is_outlier(np.asarray(l2), upstream_defects="reproduce")),
        )


def sec_signals(rng, tmp, tier) -> None:
    cases = [(9, 8, 7, 30), (6, 5, 4, 3)] + ([(64, 64, 48, 120)] if tier == "full" else [])
    for shape in cases:
        d = series(rng, shape, "f4")
        img_fn = write_image(tmp, "sig", d)
        probs = [rng.random(shape[:3]).astype(np.float32) for _ in range(3)]
        mfns = [write_image(tmp, f"sigm{i}", p) for i, p in enumerate(probs)]
        img = nb.load(img_fn)
        masks = [np.asanyarray(nb.load(f).dataobj) >= 0.5 for f in mfns]
        data = img.get_fdata()
        for j, mask in enumerate(masks):
            want = data[mask, :].mean(axis=0)
            got = red(data[mask, :], "mean", axis=0)
            check("signals", f"data[mask, :].mean(axis=0) {shape} mask {j}", want, got)


def sec_mask2vf(rng, tmp, tier) -> None:
    from scipy.ndimage import gaussian_filter

    for shape in [(20, 18, 16), (9, 8, 7)] + ([(97, 115, 97)] if tier == "full" else []):
        m = (rng.random(shape) > 0.7).astype(np.float32)
        data = gaussian_filter(m, sigma=[0.6, 0.7, 0.5])
        pos = data[data > 0]
        want = np.percentile(pos, 99)
        got = np.float32(_core.numpy_percentile_weak(pos, 99, ver()))
        check("mask2vf", f"percentile(data[data > 0], 99) {shape}", want, got)
        dv = data / want
        check(
            "mask2vf",
            f"clip(data / max, 0, 1) {shape}",
            np.clip(dv, a_min=0, a_max=1),
            _core.numpy_clip_f32(dv, 0.0, 1.0),
        )


def sec_motion(rng, tmp, tier) -> None:
    for shape in [(20, 18, 16), (9, 8, 7)]:
        d = series(rng, (*shape, 1), "f4")[..., 0]
        d[-1, -1, -1] = -12.5  # a negative minimum (a +0/-0 tie: see general's extremes)
        fn = write_image(tmp, "ref", d)
        data = nb.load(fn).get_fdata()
        data = data[::-1]
        check(
            "motion",
            f"data.min() {shape}",
            data.min(),
            np.float64(_core.numpy_extreme(np.ascontiguousarray(data), "min")),
        )


def sec_deriv(rng, tmp, tier) -> None:
    import pandas as pd

    for t in (1, 2, 3, 30):
        df = pd.DataFrame(rng.normal(size=(t, 6)), columns=list("abcdef"))
        if t > 2:
            df.iloc[1, 2] = np.nan
        for o in (1, 2, 3):
            if o >= t:
                continue
            want = np.diff(df[list("abcdef")], n=o, axis=0)
            got = _core.numpy_diff(np.ascontiguousarray(np.asarray(df[list("abcdef")])), o, 0)
            check("deriv", f"np.diff(df, n={o}, axis=0) T={t}", want, got)


def special(rng, n, dtype, kind):
    a = (rng.random(n) * 1000 - 300).astype(dtype)
    if kind == "nan" and n:
        a[rng.integers(n)] = np.nan
    elif kind == "inf" and n:
        a[rng.integers(n)] = np.inf
    elif kind == "infs" and n > 1:
        a[0], a[-1] = np.inf, -np.inf
    elif kind == "zeros":
        a[: n // 2] = -0.0
    elif kind == "ties":  # +0 and -0 at the minimum (and, negated, at the maximum)
        a = np.abs(a)
        a[: max(n // 3, 1)] = np.where(rng.random(max(n // 3, 1)) < 0.5, 0.0, -0.0).astype(dtype)
        if rng.random() < 0.5:
            a = -a
    elif kind == "huge":
        a *= np.asarray(1e35 if dtype == np.float32 else 1e305, dtype)
    elif kind == "cancel":
        a = (rng.random(n) * 2 - 1).astype(dtype) * np.asarray(1e7, dtype)
    return a


def sec_general(rng, tmp, tier) -> None:
    lengths = [0, 1, 2, 7, 8, 9, 16, 127, 128, 129, 130, 1000, 8192, 8193]
    if tier == "full":
        lengths += [16384, 20000, 50001]
    kinds = ["plain", "nan", "inf", "infs", "zeros", "huge", "cancel"]
    sec = "general"
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for dtype in (np.float32, np.float64):
            # 1-D whole reductions
            for n in lengths:
                for kind in kinds:
                    a = special(rng, n, dtype, kind)
                    for op in ("sum", "mean", "var", "std"):
                        check(
                            sec,
                            f"{op} 1-D n={n} {kind} {dtype.__name__}",
                            getattr(np, op)(a),
                            red(a, op),
                        )
                    if n > 1:
                        check(
                            sec,
                            f"std ddof=1 n={n} {kind}",
                            np.std(a, ddof=1),
                            red(a, "std", ddof=1),
                        )
            # 2-D to 4-D, C and F, every axis and None
            shapes = [
                (3, 9),
                (9, 3),
                (130, 4),
                (4, 130),
                (2, 8193),
                (5, 1, 7),
                (1, 9, 1),
                (3, 4, 5),
                (6, 5, 4, 3),
                (17, 1, 9, 2),
            ]
            for shp in shapes:
                for order in ("C", "F"):
                    for kind in ("plain", "nan", "zeros", "cancel"):
                        a = np.asarray(
                            special(rng, int(np.prod(shp)), dtype, kind).reshape(shp), order=order
                        )
                        for op in ("sum", "mean", "var", "std"):
                            check(
                                sec, f"{op} {shp} {order} None {kind}", getattr(a, op)(), red(a, op)
                            )
                            for ax in range(len(shp)):
                                check(
                                    sec,
                                    f"{op} {shp} {order} axis {ax} {kind}",
                                    getattr(a, op)(axis=ax),
                                    red(a, op, axis=ax),
                                )
            # rank percentiles
            for shp in [(1,), (2,), (7,), (40,), (5, 1), (3, 41), (41, 3), (6, 5, 4)]:
                for order in ("C", "F"):
                    for kind in ("plain", "nan", "zeros"):
                        a = np.asarray(
                            special(rng, int(np.prod(shp)), dtype, kind).reshape(shp), order=order
                        )
                        for q in (0, 25, 33.3, 50, 75, 99.9, 100, 12.5):
                            for method in ("lower", "higher"):
                                axes = [None, *range(len(shp))]
                                for ax in axes:
                                    want = np.percentile(a, q, axis=ax, method=method)
                                    got = _core.numpy_percentile_rank(
                                        a, q, axis=ax, method=method, numpy_version=ver()
                                    )
                                    want = np.asarray(want)
                                    if kind == "zeros":
                                        # the order statistic is selected, not sorted: the sign
                                        # of a zero rank is the partition's (module docs)
                                        want, got = np.abs(want), np.abs(np.asarray(got))
                                    check(
                                        sec,
                                        f"percentile {method} {q} {shp} {order} {ax} {kind}",
                                        want,
                                        got,
                                    )
            for bad in (-1, 100.5, 1e9):
                a = special(rng, 9, dtype, "plain")
                check_raises(
                    sec,
                    f"percentile q={bad}",
                    lambda a=a, bad=bad: np.percentile(a, bad, method="lower"),
                    lambda a=a, bad=bad: _core.numpy_percentile_rank(a, bad, numpy_version=ver()),
                )
            e = np.zeros(0, dtype)
            check_raises(
                sec,
                "percentile of empty",
                lambda e=e: np.percentile(e, 50, method="lower"),
                lambda e=e: _core.numpy_percentile_rank(e, 50, numpy_version=ver()),
            )
            # extremes
            ties = [0, 0]
            for n in [1, 2, 3, 7, 8, 9, 33, 64, 65, 1000]:
                for kind in kinds + ["ties"] * 4:
                    a = special(rng, n, dtype, kind)
                    for which in ("min", "max"):
                        want = getattr(a, which)()
                        got = np.asarray(_core.numpy_extreme(a, which), dtype)
                        tie = (
                            (want == 0)
                            and np.sum(a == 0) > 1
                            and np.any(np.signbit(a[a == 0]))
                            and not np.all(np.signbit(a[a == 0]))
                        )
                        if tie:
                            ties[0] += 1
                            ties[1] += int(bits(want) == bits(got))
                            continue
                        check(sec, f"{which} n={n} {kind}", want, got)
            NOTES.append(
                f"{dtype.__name__} min/max with -0/+0 ties: {ties[1]} of {ties[0]} have "
                "numpy's sign of zero on this CPU (not compared: CPU-dependent)"
            )
            check_raises(
                sec, "min of empty", lambda e=e: e.min(), lambda e=e: _core.numpy_extreme(e, "min")
            )
            # cumsum
            for n in (0, 1, 2, 9, 1000):
                for kind in kinds:
                    a = special(rng, n, dtype, kind)
                    if n:
                        a[0] = -0.0 if kind == "zeros" else a[0]
                    check(sec, f"cumsum n={n} {kind}", np.cumsum(a), _core.numpy_cumsum(a))
            # nan_to_num and diff
            for kind in kinds:
                a = np.asfortranarray(special(rng, 60, dtype, kind).reshape(3, 4, 5))
                check(sec, f"nan_to_num {kind}", np.nan_to_num(a), _core.numpy_nan_to_num(a))
                for order in ("C", "F"):
                    b = np.asarray(a, order=order)
                    for ax in range(3):
                        for nn in (1, 2, 3):
                            check(
                                sec,
                                f"diff n={nn} axis {ax} {order} {kind}",
                                np.diff(b, n=nn, axis=ax),
                                _core.numpy_diff(b, nn, ax),
                            )
        # searchsorted: sorted with ties and NaN tails, and unsorted arrays
        for n in (0, 1, 2, 3, 8, 31, 100, 1001):
            srt = np.sort(np.round(rng.random(n) * 20) / 20)
            for tail in (0, 1, 3):
                a = np.concatenate([srt, np.full(tail, np.nan)])
                uns = rng.permutation(a)
                for arr, name in ((a, "sorted"), (uns, "unsorted")):
                    for v in (-1.0, 0.0, 0.05, 0.5, 0.55, 1.0, 2.0, np.nan):
                        for side in ("left", "right"):
                            check(
                                sec,
                                f"searchsorted {name} n={n} tail {tail} {v} {side}",
                                np.intp(np.searchsorted(arr, v, side=side)),
                                np.intp(_core.numpy_searchsorted(arr, v, side, ver())),
                            )
        # linspace and the Legendre basis
        for start, stop in ((-1.0, 1.0), (0.0, 1.0), (-3.5, 7.25), (1.0, 1.0)):
            for num in (0, 1, 2, 3, 10, 99, 1000):
                check(
                    sec,
                    f"linspace({start}, {stop}, {num})",
                    np.linspace(start, stop, num),
                    _core.numpy_linspace(start, stop, num),
                )
        for T in (2, 3, 10, 57, 280):
            x = np.linspace(-1, 1, T)
            xs = np.concatenate([x, [-0.0, 0.3, np.nan, 1e300]])
            for deg in range(9):
                check(
                    sec,
                    f"Legendre.basis({deg}) T={T}",
                    Legendre.basis(deg)(xs),
                    _core.numpy_legendre_basis(deg, xs, ver22()),
                )


SECTIONS = {
    "dvars": sec_dvars,
    "fd": sec_fd,
    "noise": sec_noise,
    "tcompcor": sec_tcompcor,
    "tsnr": sec_tsnr,
    "nss": sec_nss,
    "signals": sec_signals,
    "mask2vf": sec_mask2vf,
    "motion": sec_motion,
    "deriv": sec_deriv,
    "general": sec_general,
}


def main() -> int:
    global CONTROL, OTHER
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--tier", choices=("quick", "full"), default="quick")
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument("--control", action="store_true")
    ap.add_argument("--seed", type=int, default=20261111)
    args = ap.parse_args()
    CONTROL = args.control
    OTHER = other_release(NPV)
    print(
        f"numpy {NPV}, tier {args.tier}{', control (larmorx told ' + OTHER + ' / ' + ver22() + ')' if CONTROL else ''}"
    )
    t0 = time.time()
    with tempfile.TemporaryDirectory(prefix="lx-npred-") as td:
        for name in args.sections.split(","):
            rng = np.random.default_rng([args.seed, list(SECTIONS).index(name)])
            t = time.time()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", RuntimeWarning)
                SECTIONS[name](rng, Path(td), args.tier)
            ok, n = COUNTS[name]
            word = "differ" if CONTROL else "identical"
            shown = n - ok if CONTROL else ok
            print(f"  {name:9s} {shown:6d} of {n:6d} {word}  ({time.time() - t:.1f} s)")
    for kind, k in sorted(CONTROL_KINDS.items()):
        print(f"  control: {kind} {k} cases differ")
    for note in NOTES:
        print(f"  note: {note}")
    for f in FAILS:
        print(f"  FAIL {f}")
    total_ok = sum(c[0] for c in COUNTS.values())
    total = sum(c[1] for c in COUNTS.values())
    print(
        f"TOTAL {total_ok} of {total} identical ({time.time() - t0:.1f} s)"
        + (" [control: differences expected]" if CONTROL else "")
    )
    return 0 if CONTROL or total_ok == total else 1


if __name__ == "__main__":
    sys.exit(main())

# SPDX-License-Identifier: Apache-2.0
"""A model of tedana 26.0.3's ``t2smap`` written from ``specs/t2smap.md`` (§§3–11) only.

Never from tedana's source: a clean-room implementer may read and run it. It calls numpy, SciPy
and nibabel (permissive libraries) where the spec names them, and writes out the orders of the
sums the spec describes (§13.2) and its percentile rules (§13.6), so that a bit-for-bit agreement
with tedana's files checks the spec's text, not only the libraries::

    python -I model.py [--variant exact|fast] <t2smap's options>

``exact`` (the default) is the spec as written, tedana's results (``reproduce``) in the reference
setup (§17.1: run it with the same environment as tedana: ``OPENBLAS_CORETYPE=Haswell``,
``NPY_DISABLE_CPU_FEATURES`` with numpy's AVX-512 kernels off, one BLAS thread). ``fast`` is the
kind of change §16 allows, to measure §17.3's margins: the log-linear fit solved per voxel in
closed form (a centred 2 × 2 least squares), the curve fit with an analytic Jacobian, and other
orders of the sums in the RMSE, the table and the combination.

Scope: every image form the spec's §4.1 reads (NIfTI-1 and NIfTI-2 single files and pairs,
ANALYZE 7.5, MGH/MGZ, as nibabel loads them), a user mask of any of them (a 4D one of one volume
through nilearn's masking, §4.4) or the computed mask; the defects' corrections (larmorx's
default) are not modelled: the model is tedana's behaviour. Messages are not written
(``blackbox.py`` covers them); only the files.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
import warnings
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy.optimize import curve_fit

F32 = np.float32
F64 = np.float64

NAMES = {  # §11.1: description -> (bids, orig)
    "adaptive mask img": ("desc-adaptiveGoodSignal_mask", "adaptive_mask"),
    "fit failures img": ("desc-decayFitFailures_mask", "fit_failures"),
    "t2star variance img": ("stat-variance_desc-t2star_statmap", "t2s_var"),
    "s0 variance img": ("stat-variance_desc-s0_statmap", "s0_var"),
    "t2star-s0 covariance img": ("stat-covariance_desc-t2star+s0_statmap", "t2s_s0_covar"),
    "s0 img": ("S0map", "s0vG"),
    "rmse img": ("desc-rmse_statmap", "rmse"),
    "confounds tsv": ("desc-confounds_timeseries", "confounds"),
    "limited s0 img": ("desc-limited_S0map", "s0v"),
    "limited t2star img": ("desc-limited_T2starmap", "t2sv"),
    "t2star img": ("T2starmap", "t2svG"),
    "combined img": ("desc-optcom_bold", "ts_OC"),
    "data description json": ("dataset_description", "dataset_description"),
    "registry json": ("desc-tedana_registry", "registry"),
}
TABLE_COLUMNS = (
    "rmse_mean",
    "rmse_std",
    "rmse_min",
    "rmse_percentile02",
    "rmse_percentile25",
    "rmse_median",
    "rmse_percentile75",
    "rmse_percentile98",
    "rmse_max",
)
PERCENTILES = (0, 2, 25, 50, 75, 98, 100)


# ---------------------------------------------------------------- §13.2: numpy's sums and means
def _pairwise(a: np.ndarray) -> np.ndarray:
    """numpy's pairwise sum P along the last axis (§13.2), elementwise over the other axes."""
    n = a.shape[-1]
    if n < 8:
        res = np.full(a.shape[:-1], -0.0, dtype=a.dtype)
        for i in range(n):
            res = res + a[..., i]
        return res
    if n <= 128:
        r = [a[..., k].copy() for k in range(8)]
        m = n - n % 8
        for i in range(8, m, 8):
            for k in range(8):
                r[k] = r[k] + a[..., i + k]
        res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))
        for i in range(m, n):
            res = res + a[..., i]
        return res
    n2 = n // 2
    n2 -= n2 % 8
    return _pairwise(a[..., :n2]) + _pairwise(a[..., n2:])


def sum_inner(a: np.ndarray) -> np.ndarray:
    """A sum along the innermost axis: +0.0 plus the pairwise sum (§13.2)."""
    return a.dtype.type(0.0) + _pairwise(a)


def sum_outer(a: np.ndarray) -> np.ndarray:
    """A sum along an outer axis (the last axis here): a plain sum in index order from +0.0."""
    res = np.zeros(a.shape[:-1], dtype=a.dtype)
    for i in range(a.shape[-1]):
        res = res + a[..., i]
    return res


def divide_count(total: np.ndarray, count) -> np.ndarray:
    """A mean's division: by the count in float64, rounded to the sum's type (§13.2)."""
    with np.errstate(invalid="ignore", divide="ignore"):
        return (total.astype(F64) / np.asarray(count, dtype=F64)).astype(total.dtype)


# ---------------------------------------------------------------- §13.6: percentiles
def percentile_higher(values: np.ndarray, q: float) -> np.floating:
    n = values.size
    if n == 0:
        raise IndexError("index -1 is out of bounds for axis 0 with size 0")
    if np.isnan(values).any():
        return values.dtype.type(np.nan)
    idx = int(np.ceil(F64(n - 1) * (F64(q) / F64(100))))
    return np.sort(values)[idx]


def percentile_lower(values: np.ndarray, per: float) -> np.floating:
    n = values.size
    idx = int(np.floor(F64(per) / F64(100) * F64(n - 1)))
    return np.sort(values)[idx]  # NaN sorts last


def nanpercentile_linear(values: np.ndarray, q: float) -> F64:
    """numpy's linear percentile of the non-NaN float32 values (§13.6), float64."""
    v = np.sort(values[~np.isnan(values)])
    n = v.size
    g = F64(n - 1) * (F64(q) / F64(100))
    lo = int(np.floor(g))
    frac = g - F64(lo)
    hi = min(lo + 1, n - 1)
    a, b = v[lo], v[hi]
    diff = F64(b - a)  # the difference in float32, then widened
    if frac >= 0.5:
        return F64(b) - diff * (F64(1) - frac)
    return F64(a) + diff * frac


# ---------------------------------------------------------------- §4: the inputs
def as_nifti1(img, dtype=None, max_dim: int | None = None):
    """§4.1: a NIfTI-1 single-file image as it is (a NIfTI-2 one too: in nibabel a NIfTI-2
    image is a kind of NIfTI-1 image); any other image nibabel reads (a NIfTI-1 or NIfTI-2
    pair, ANALYZE 7.5, MGH/MGZ) becomes an in-memory NIfTI-1 image of nibabel's scaled array,
    in nibabel's own type for it (``dtype`` given: that type), with the image's affine (§4.5)
    and voxel sizes. ``max_dim`` (the user mask, §4.3): such an image with more dimensions is
    first squeezed of its trailing axes of length 1, and refused if it still has more."""
    if isinstance(img, nib.Nifti1Image):
        return img
    data = np.asanyarray(img.dataobj) if dtype is None else np.asarray(img.dataobj, dtype=dtype)
    new = nib.Nifti1Image(data, img.affine)
    new.header.set_zooms(img.header.get_zooms())
    if max_dim and new.ndim > max_dim:
        new = nib.funcs.squeeze_image(new)
        if new.ndim > max_dim:
            raise ValueError(f"{img.get_filename()} has more than {max_dim} dimensions")
    return new


def load_inputs(paths: list[str], n_echoes: int, mask_path: str | None):
    """The reference image, the mask (grid booleans), its affine, the outputs' image class and
    the echoes' float32 data on the mask's voxels, ``x[v, e, t]`` (§4): every image form of
    §4.1, a user mask of any form (§4.3), and nilearn's masking (§4.4) where an echo's grid is
    not the mask's shape (the shapes of the grid: a 4D mask of one volume) or an echo is not 4D;
    nilearn's masking refuses a mask of another grid, as tedana's run does."""
    imgs = [nib.load(p) for p in paths]
    if len(paths) == 1:
        img = as_nifti1(imgs[0])
        nz = img.shape[2] // n_echoes
        full = np.asarray(img.dataobj, dtype=F32)
        if full.ndim != 4:
            raise ValueError("Expected 4D z-concatenated image")
        slabs = [full[:, :, e * nz : (e + 1) * nz, :] for e in range(n_echoes)]
        ref = nib.Nifti1Image(img.slicer[:, :, :nz, :].get_fdata(), img.affine)
        ref.header.set_zooms(img.header.get_zooms())
    else:
        slabs = None
        ref = as_nifti1(imgs[0])
    if mask_path is not None:
        mimg = as_nifti1(nib.load(mask_path), max_dim=3)
        mask = mimg.get_fdata() > 0
        mask_img = nib.Nifti1Image(mask, mimg.affine, mimg.header)
        affine, cls = mimg.affine, nib.Nifti1Image
    else:
        from nilearn.masking import compute_epi_mask

        mask_img = compute_epi_mask(ref)
        mask = np.asanyarray(mask_img.dataobj) > 0
        affine, cls = mask_img.affine, type(mask_img)
    idx = np.flatnonzero(mask.ravel(order="C"))
    if slabs is not None:
        if slabs[0].shape[:3] != mask.shape:
            raise ValueError("Z-cat echo slice/mask shape mismatch")
        x = np.stack([s.reshape(-1, s.shape[3])[idx] for s in slabs], axis=1)
        return ref, mask, affine, cls, np.ascontiguousarray(x, dtype=F32)
    try:  # §4.4: each echo on the mask's grid, read as float32
        cols = []
        for img in imgs:
            conv = as_nifti1(img)
            if conv.shape[:3] != mask.shape:
                raise ValueError("Image/mask shape mismatch")
            arr = np.asarray(conv.dataobj, dtype=F32)
            if arr.ndim != 4:
                raise ValueError("Expected 4D image")
            cols.append(arr.reshape(-1, arr.shape[3])[idx])
    except ValueError:  # §4.4: nilearn's masking of every echo (non-finite values set to 0)
        from nilearn import masking

        cols = []
        for img in imgs:
            m = masking.apply_mask(as_nifti1(img, dtype=F32), mask_img)
            if m.ndim != 2:
                raise ValueError("Expected 4D image") from None
            cols.append(m.T.astype(F32, copy=False))
    x = np.stack(cols, axis=1)
    return ref, mask, affine, cls, np.ascontiguousarray(x, dtype=F32)


def parse_exclude(text: str | None) -> list[int]:
    if not text:
        return []
    out: set[int] = set()
    for item in text.split(","):
        item = item.strip()
        if item.count(":") > 1:
            raise ValueError(
                f"Invalid volume indices string ({text}). Step sizes are not supported."
            )
        if ":" in item:
            a, b = item.split(":")
            if a.strip() == "" or b.strip() == "":
                raise ValueError(
                    f"Invalid volume indices string ({text}). Open-ended ranges are not supported."
                )
            lo, hi = int(a), int(b)
            if lo < 0 or hi < 0:
                raise ValueError(
                    f"Invalid volume indices string ({text}). Negative indices are not supported."
                )
            out.update(range(lo, hi))
        else:
            i = int(item)
            if i < 0:
                raise ValueError(
                    f"Invalid volume indices string ({text}). Negative indices are not supported."
                )
            out.add(i)
    return sorted(out)


# ---------------------------------------------------------------- §6: the adaptive mask
def temporal_means(xu: np.ndarray, volumes_outer: bool) -> np.ndarray:
    """m[v, e] (float32): pairwise along the volumes, or a plain sum with ``--exclude``."""
    total = sum_outer(xu) if volumes_outer else sum_inner(xu)
    return divide_count(total, xu.shape[-1])


def adaptive_mask(xu: np.ndarray, methods: list[str], volumes_outer: bool) -> np.ndarray:
    _, n_echo, _ = xu.shape
    with np.errstate(invalid="ignore"):
        good = ~np.any(np.isnan(xu) | (xu <= 0), axis=2)
    base = np.cumprod(good, axis=1).sum(axis=1)
    counts = [base]
    methods = [m for m in methods if m != "none"]
    if methods:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            m = temporal_means(xu, volumes_outer)
    if "dropout" in methods:
        first = m[:, 0]
        perc = percentile_higher(first[first != 0], 33)
        ex = np.flatnonzero(first == perc)
        if ex.size == 0:
            raise ValueError("attempt to get argmax of an empty sequence")
        thirds = m[ex] / 3  # each exemplar's candidates (float32)
        # several: the candidates with the largest sum (of the thirds, along the echoes)
        thr = thirds[0] if ex.size == 1 else thirds[int(np.argmax(sum_inner(thirds)))]
        above = np.abs(m) > thr[None, :]
        last = n_echo - np.argmax(above[:, ::-1], axis=1)
        counts.append(np.where(above.any(axis=1), last, 0))
    if "decay" in methods:
        with np.errstate(invalid="ignore"):
            rising = (m[:, 1:] - m[:, :-1]) >= 0
        first_rise = np.argmax(rising, axis=1) + 1
        counts.append(np.where(rising.any(axis=1), first_rise, n_echo))
    count = np.min(np.stack(counts), axis=0)
    count[count < 1] = 0
    return count.astype(np.int64)


# ---------------------------------------------------------------- §7: the fits
def groups_of(count: np.ndarray) -> list[tuple[int, np.ndarray]]:
    present = {int(c) for c in np.unique(count[count >= 1])}
    if 1 in present:
        present.add(2)
    out = []
    for n in sorted(c for c in present if c >= 2):
        members = (count == 1) | (count == 2) if n == 2 else count == n
        out.append((n, np.flatnonzero(members)))
    return out


def loglin_group(xg: np.ndarray, tes: np.ndarray, variant: str) -> tuple[np.ndarray, np.ndarray]:
    """T2* (ms) and S0 of the voxels ``xg[v, e, t]`` (n echoes, T_u volumes): §7.2."""
    n_vox, n, tu = xg.shape
    with np.errstate(divide="ignore", invalid="ignore"):
        y = np.log(np.abs(xg) + 1).reshape(n_vox, n * tu)  # float32, echo-major
    x_te = -np.repeat(tes[:n], tu)
    if variant == "fast" and n_vox and np.isfinite(y).all() and np.ptp(x_te) > 0:
        yd = y.astype(F64)
        xbar = x_te.mean()
        xc = x_te - xbar
        ybar = yd.mean(axis=1)
        b1 = (yd - ybar[:, None]) @ xc / (xc @ xc)
        b0 = ybar - b1 * xbar
    else:
        design = np.column_stack([np.ones(n * tu), x_te])
        with np.errstate(all="ignore"):
            beta = np.linalg.lstsq(design, y.T, rcond=None)[0]
        b0, b1 = beta[0], beta[1]
    with np.errstate(divide="ignore", invalid="ignore"):
        t2s = 1 / b1
        s0 = np.exp(b0)
    return t2s, s0


def _model(te, s0, t2s):
    return s0 * np.exp(-te / t2s)


def _jac(te, s0, t2s):
    e = np.exp(-te / t2s)
    return np.column_stack([e, s0 * e * te / (t2s * t2s)])


def curvefit_voxel(y: np.ndarray, te: np.ndarray, p0: tuple[float, float], variant: str):
    """One voxel's bounded curve fit (§7.3): (S0, T2*, failed, P) with P the covariance."""
    try:
        lb = float(np.min(y))
        kwargs = dict(
            p0=p0,
            bounds=((lb, 0.0), (np.inf, np.inf)),
            method="trf",
            jac=_jac if variant == "fast" else "2-point",
            x_scale=1.0,
            tr_solver="exact",
            ftol=1e-8,
            xtol=1e-8,
            gtol=1e-8,
            max_nfev=200,
        )
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            popt, pcov = curve_fit(_model, te, y.astype(F64), **kwargs)
    except (RuntimeError, ValueError):
        return None
    return popt[0], popt[1], pcov


def fit(x_used: np.ndarray, count: np.ndarray, tes: np.ndarray, fittype: str, variant: str):
    """The full maps over the S voxels (zeros outside the fit voxels), §7: T2* (ms), S0,
    failures, the three variance values (tedana's written order: P[0,0], P[1,1], P[0,1])."""
    n_vox = x_used.shape[0]
    t2s = np.zeros(n_vox)
    s0 = np.zeros(n_vox)
    failed = np.zeros(n_vox, dtype=bool)
    var = np.zeros((3, n_vox))
    for n, members in groups_of(count):
        xg = x_used[members, :n, :]
        g_t2s, g_s0 = loglin_group(xg, tes, variant)
        t2s[members] = g_t2s
        s0[members] = g_s0
        if fittype != "curvefit":
            continue
        tu = xg.shape[2]
        te = np.repeat(tes[:n], tu)
        for j, v in enumerate(members):
            res = curvefit_voxel(xg[j].reshape(-1), te, (g_s0[j], g_t2s[j]), variant)
            one = count[v] == 1
            if res is None:
                failed[v] = not one
                continue
            s0[v], t2s[v], pcov = res
            if not one:
                var[:, v] = (pcov[0, 0], pcov[1, 1], pcov[0, 1])
    return t2s, s0, failed, var


# ---------------------------------------------------------------- §8: floors, limited, cap
def floors(t2s: np.ndarray, s0: np.ndarray, tes: np.ndarray) -> None:
    """In place, on the fit voxels' values (any shape)."""
    t2s[np.isposinf(t2s)] = 500
    with np.errstate(invalid="ignore"):
        t2s[t2s <= 0] = 1
    flat = t2s.reshape(-1)
    nz = np.flatnonzero(flat != 0)
    with np.errstate(divide="ignore", invalid="ignore", over="ignore", under="ignore"):
        dec = np.exp(-tes[None, :] / flat[nz, None])
    under = nz[np.any(dec == 0, axis=1)]
    with np.errstate(divide="ignore"):
        flat[under] = np.min(-tes) / np.log(np.finfo(F64).eps)
    s0[np.isnan(s0)] = 0


def cap(limited: np.ndarray) -> None:
    c = percentile_lower(limited.reshape(-1), 99.5)
    limited[limited > 10 * c] = c


# ---------------------------------------------------------------- §9: RMSE and the table
def rmse_series(x: np.ndarray, count: np.ndarray, t2s: np.ndarray, s0: np.ndarray, tes, variant):
    """rmse[v, t] (float32) over all volumes from the limited maps (ms): NaN for counts < 2."""
    n_vox, n_echo, n_t = x.shape
    out = np.full((n_vox, n_t), np.nan, dtype=F32)
    ts = t2s.ndim == 2
    for n in range(2, n_echo + 1):
        v = np.flatnonzero(count == n)
        if v.size == 0:
            continue
        with np.errstate(all="ignore"):
            if ts:  # (V, n, T)
                p = (s0[v][:, None, :] * np.exp(-tes[None, :n, None] / t2s[v][:, None, :])).astype(
                    F32
                )
            else:
                p = (s0[v][:, None] * np.exp(-tes[None, :n] / t2s[v][:, None])).astype(F32)[
                    ..., None
                ]
            sq = (x[v, :n, :] - p) ** 2  # float32
            sq = np.moveaxis(sq, 1, -1)  # (V, T, n)
            if variant == "fast":
                mean = (sq.astype(F64)[..., ::-1].sum(axis=-1) / n).astype(F32)
            else:
                mean = divide_count(sum_outer(sq), n)
            out[v] = np.sqrt(mean)
    return out


def rmse_map(r: np.ndarray, variant: str) -> np.ndarray:
    """§9 step 2; a voxel without a value is numpy's ``nan``, the positive quiet NaN (§13.1)."""
    nan = np.isnan(r)
    filled = np.where(nan, F32(0), r)
    total = sum_outer(filled) if variant == "fast" else sum_inner(filled)
    cnt = (~nan).sum(axis=1)
    return np.where(cnt == 0, F32(np.nan), divide_count(total, cnt))


def table(r: np.ndarray, variant: str) -> list[list[float]]:
    nan = np.isnan(r)
    filled = np.where(nan, F32(0), r).T  # (T, S): voxels last, an outer axis in tedana's array
    cnt = (~nan).sum(axis=0)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        # the voxels an outer axis: a plain sum, but pairwise with one volume (§13.2)
        if variant == "fast" or r.shape[1] == 1:
            mean = divide_count(sum_inner(filled), cnt)
        else:
            mean = divide_count(sum_outer(filled), cnt)
        std = np.nanstd(r, axis=0)
    rows = []
    first_empty = cnt.size > 0 and cnt[0] == 0
    for t in range(r.shape[1]):
        col = r[:, t]
        if cnt[t] == 0:
            pct = [np.nan] * len(PERCENTILES)
        else:
            pct = [nanpercentile_linear(col, q) for q in PERCENTILES]
        if first_empty:
            pct = [float(F32(p)) for p in pct]
        rows.append([float(mean[t]), float(std[t]), *[float(p) for p in pct]])
    return rows


# ---------------------------------------------------------------- §10: the combination
def combine(x: np.ndarray, count: np.ndarray, t2s_full: np.ndarray, tes, combmode: str, variant):
    n_vox, _, n_t = x.shape
    out = np.zeros((n_vox, n_t))
    ts = t2s_full.ndim == 2
    for n, members in groups_of(count):
        xg = x[members, :n, :].astype(F64)  # (V, n, T)
        with np.errstate(all="ignore"):
            if combmode == "paid":
                xs = x[members, :n, :]
                mu = divide_count(sum_inner(xs), n_t)
                dev = xs - mu[..., None]
                sd = np.sqrt(divide_count(sum_inner(dev * dev), n_t))
                snr = np.where(sd != 0, mu / np.where(sd != 0, sd, F32(1)), F32(0))
                w = (snr * tes[None, :n])[..., None] * np.ones(n_t)
            elif ts:
                w = tes[None, :n, None] * np.exp(
                    -tes[None, :n, None] / t2s_full[members][:, None, :]
                )
                zero = np.all(w == 0, axis=1)
                w = np.where(zero[:, None, :], 1.0, w)
            else:
                w = (tes[None, :n] * np.exp(-tes[None, :n] / t2s_full[members][:, None]))[..., None]
                w = w * np.ones(n_t)
            wm = np.moveaxis(w, 1, -1)  # (V, T, n)
            xm = np.moveaxis(xg, 1, -1)
            if variant == "fast":
                scl = wm[..., ::-1].sum(axis=-1)
                num = ((wm / scl[..., None]) * xm)[..., ::-1].sum(axis=-1)
                out[members] = num
                continue
            scl = sum_inner(np.ascontiguousarray(wm)) if ts else sum_outer(wm)
            if np.any(scl == 0):
                raise ZeroDivisionError("Weights sum to zero, can't be normalized")
            out[members] = sum_outer(wm * xm) / scl
    return out


# ---------------------------------------------------------------- §11: the files
def name(desc: str, convention: str, prefix: str) -> str:
    base = NAMES[desc][0 if convention == "bids" else 1]
    ext = ".tsv" if desc.endswith("tsv") else ".json" if desc.endswith("json") else ".nii.gz"
    return f"{prefix}{base}{ext}"


def save_image(vals, mask, affine, ref, dtype, path: Path, cls=nib.Nifti1Image) -> None:
    """§11.3: a new image of the mask image's class (NIfTI-2 for a mask computed from a NIfTI-2
    first echo) on the mask's three spatial axes."""
    vals = np.asarray(vals)
    shape = mask.shape[:3] + vals.shape[1:]
    data = np.zeros(shape, dtype=dtype)
    flat = data.reshape((-1, *vals.shape[1:]))
    flat[np.flatnonzero(mask.ravel(order="C"))] = vals.astype(dtype)
    img = cls(data, affine)
    if data.ndim == 4:
        img.header.set_zooms(ref.header.get_zooms()[:4])
    img.to_filename(str(path))


def float_text(v: float) -> str:
    return "n/a" if np.isnan(v) else repr(float(v))


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="model.py")
    p.add_argument("--variant", choices=("exact", "fast"), default="exact")
    p.add_argument("-d", dest="data", nargs="+", required=True)
    p.add_argument("-e", dest="tes", nargs="+", type=float, required=True)
    p.add_argument("--out-dir", default=".")
    p.add_argument("--prefix", default="")
    p.add_argument("--convention", choices=("orig", "bids"), default="bids")
    p.add_argument("--verbose", action="store_true")
    p.add_argument("--overwrite", "-f", action="store_true")
    p.add_argument("--mask", default=None)
    p.add_argument("--masktype", nargs="+", default=["dropout"])
    p.add_argument("--dummy-scans", type=int, default=0)
    p.add_argument("--exclude", default=None)
    p.add_argument("--fittype", choices=("loglin", "curvefit"), default="loglin")
    p.add_argument("--fitmode", choices=("all", "ts"), default="all")
    p.add_argument("--combmode", choices=("t2s", "paid"), default="t2s")
    p.add_argument("--n-independent-echos", type=int, default=None)
    p.add_argument("--n-threads", type=int, default=1)
    p.add_argument("--debug", action="store_true")
    p.add_argument("--quiet", action="store_true")
    return p


def echo_times_ms(values) -> np.ndarray:
    tes = np.asarray(values, dtype=F64)
    if np.all((tes > 0) & (tes < 1)):
        return tes * 1000
    if np.all(tes >= 1):
        return tes
    raise ValueError("TE values must be positive and either all in seconds or all in ms.")


def prepare(a: argparse.Namespace) -> dict:
    """The inputs as §§4–6 make them: the data of every volume (after the dummy scans), the
    used volumes, the adaptive mask's counts."""
    exclude = parse_exclude(a.exclude)
    if a.fitmode == "ts" and exclude:
        raise ValueError("Excluding volumes is not supported for fitmode='ts'.")
    tes = echo_times_ms(a.tes)
    ref, mask, affine, cls, x = load_inputs(a.data, len(tes), a.mask)
    if a.dummy_scans > 0:
        x = np.ascontiguousarray(x[:, :, a.dummy_scans :])
    n_t = x.shape[2]
    used = np.arange(n_t)
    if exclude:
        ex = [i - a.dummy_scans for i in exclude if i >= a.dummy_scans]
        if ex and max(ex) > n_t:
            raise ValueError("The maximum exclude index is greater than the number of timepoints")
        exclude = ex
        used = np.setdiff1d(np.arange(n_t), ex)
    x_used = x[:, :, used] if exclude else x
    count = adaptive_mask(x_used, a.masktype, volumes_outer=bool(exclude))
    if x.shape[1] != len(tes):
        raise ValueError("Second dimension of data does not match number of echoes provided")
    return dict(
        ref=ref,
        mask=mask,
        affine=affine,
        cls=cls,
        x=x,
        x_used=x_used,
        used=used,
        tes=tes,
        count=count,
        n_t=n_t,
    )


def run(argv: list[str]) -> int:
    a = parser().parse_args(argv)
    variant = a.variant
    call_args = list(argv)
    for i, w in enumerate(argv):
        if w == "--variant":
            call_args = argv[:i] + argv[i + 2 :]
        elif w.startswith("--variant="):
            call_args = argv[:i] + argv[i + 1 :]
    out = Path(os.path.abspath(a.out_dir))
    out.mkdir(exist_ok=True)
    parse_exclude(a.exclude)  # its errors come before any file but the directory
    call = "t2smap " + " ".join(call_args)
    (out / "t2smap_call.sh").write_text(call, encoding="utf-8")
    echo_times_ms(a.tes)
    prefix = a.prefix if (a.prefix == "" or a.prefix.endswith("_")) else a.prefix + "_"
    (out / "figures").mkdir(exist_ok=True)
    c = prepare(a)
    ref, mask, affine, x, x_used = c["ref"], c["mask"], c["affine"], c["x"], c["x_used"]
    tes, count, n_t = c["tes"], c["count"], c["n_t"]

    written: dict[str, str] = {}

    def img(desc, vals, dtype):
        fname = name(desc, a.convention, prefix)
        save_image(vals, mask, affine, ref, dtype, out / fname, c["cls"])
        written[desc] = fname

    img("adaptive mask img", count, np.int32)

    if a.fitmode == "ts":
        cols = [fit(x[:, :, t : t + 1], count, tes, a.fittype, variant) for t in range(n_t)]
        t2s = np.stack([c[0] for c in cols], axis=1)
        s0 = np.stack([c[1] for c in cols], axis=1)
        failed = np.stack([c[2] for c in cols], axis=1)
        var = np.stack([c[3] for c in cols], axis=2)
    else:
        t2s, s0, failed, var = fit(x_used, count, tes, a.fittype, variant)
    fv = count >= 1
    if a.fittype == "curvefit":
        img("fit failures img", failed, np.uint8)
        if a.verbose:
            img("t2star variance img", var[0], np.float32)
            img("s0 variance img", var[1], np.float32)
            img("t2star-s0 covariance img", var[2], np.float32)
    t2s_f, s0_f = t2s[fv], s0[fv]
    floors(t2s_f, s0_f, tes)
    one = count[fv] == 1
    t2s_l, s0_l = t2s_f.copy(), s0_f.copy()
    t2s_l[one] = 0
    s0_l[one] = 0
    cap(t2s_l)

    def on_s(vals):
        full = np.zeros((count.size, *vals.shape[1:]))
        full[fv] = vals
        return full

    t2s_full, s0_full, t2s_lim, s0_lim = on_s(t2s_f), on_s(s0_f), on_s(t2s_l), on_s(s0_l)
    img("s0 img", s0_full, np.float32)
    r = rmse_series(x, count, t2s_lim, s0_lim, tes, variant)
    img("rmse img", rmse_map(r, variant), np.float32)
    fname = name("confounds tsv", a.convention, prefix)
    lines = ["\t".join(TABLE_COLUMNS)]
    lines += ["\t".join(float_text(v) for v in row) for row in table(r, variant)]
    (out / fname).write_text("\n".join(lines) + "\n", encoding="utf-8")
    written["confounds tsv"] = fname
    img("limited s0 img", s0_lim, np.float32)
    img("limited t2star img", t2s_lim / 1000, np.float32)
    comb = combine(x, count, t2s_full, tes, a.combmode, variant)
    img("t2star img", t2s_full / 1000, np.float32)
    img("combined img", comb, np.float32)

    u = platform.uname()
    desc = {
        "BIDSVersion": "1.5.0",
        "DatasetType": "derivative",
        "GeneratedBy": [
            {
                "CodeURL": "https://github.com/ME-ICA/tedana",
                "Command": call,
                "Description": "A pipeline estimating T2* from multi-echo fMRI data and "
                "combining data across echoes.",
                "Name": "t2smap",
                "Node": {
                    "Machine": u.machine,
                    "Name": u.node,
                    "Processor": u.processor,
                    "Release": u.release,
                    "System": u.system,
                    "Version": u.version,
                },
                "Python": f"the spec's model (validation/investigations/t2smap-spec), {sys.version}",
                "Python_Libraries": {"numpy": np.__version__},
                "Version": "26.0.3",
            }
        ],
        "Name": "t2smap Outputs",
    }
    fname = name("data description json", a.convention, prefix)
    with open(out / fname, "w", encoding="utf-8") as f:
        json.dump(desc, f, indent=4, sort_keys=True)
    written["data description json"] = fname
    with open(out / name("registry json", a.convention, prefix), "w", encoding="utf-8") as f:
        json.dump(written, f, indent=4, sort_keys=True)
    return 0


if __name__ == "__main__":
    sys.exit(run(sys.argv[1:]))

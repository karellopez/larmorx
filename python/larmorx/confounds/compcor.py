# SPDX-License-Identifier: Apache-2.0
"""nipype's CompCor: ``compute_noise_components``, its pre-filters and helpers, and the
``CompCor``, ``ACompCor`` and ``TCompCor`` interfaces' files.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``CompCor``, ``ACompCor``, ``TCompCor``, ``combine_mask_files``,
``compute_noise_components``, ``regress_poly``, ``cosine_filter``, ``_cosine_drift``,
``_full_rank``, ``_compute_tSTD``, ``fallback_svd``), with the arithmetic of the libraries under
it, bit for bit (``larmorx_confounds::compcor``): numpy's reductions, Legendre polynomials,
``pinv``, ``lstsq`` and ``svd`` (``numpy_version``: 2.5.4, 2.3.5 or 2.2.6, fMRIPrep 25.2.6's),
SciPy's ``gesvd`` for ``fallback_svd`` (``scipy_version``: 1.18.1 or 1.15), and the C library's
``cos`` of numpy's ``np.cos`` (glibc 2.35's with ``upstream_defects="reproduce"``, the correctly
rounded one by default: GLIBC-07). The images are read as nibabel reads them
(``get_fdata(dtype=np.float32)``, ``dataobj``).

Corrected by default (``upstream_defects="reproduce"`` gives nipype's result with the release's
libraries; each correction names its ID in a :class:`larmorx.UpstreamDefectWarning`):

- PIPE-44: a mask that gives no component (an empty mask with ``'all'``) ends nipype's loop over
  the masks: the later masks are skipped. larmorx goes on with them, and with
  ``failure_mode='NaN'`` returns no component instead of nipype's ``ValueError: need at least one
  array to concatenate``.
- PIPE-45: nipype zeroes only the voxels whose float sum over time is NaN; a voxel holding an
  infinity makes the mask's SVD fail (every component NaN with ``failure_mode='NaN'``). larmorx
  zeroes exactly the voxels that hold a non-finite value.
- PIPE-52: ``regress_poly``'s non-constant regressors (CompCor's Legendre pre-filter file) are
  nipype's ``X[:, :-1]``, the constant and every polynomial but the highest; larmorx returns the
  polynomials of degree 1 to ``degree``.
- PIPE-53: ``regress_poly`` and ``cosine_filter`` with an ``axis`` other than the last reshape the
  data in memory order to rows of that axis' length; larmorx refuses it (``ValueError``).
- PIPE-54: a ``components_criterion`` of 0, or negative other than -1 (``'all'``), is refused
  (``ValueError``); a count given as a float is taken by its integer part where nipype raises
  ``TypeError`` (a failed SVD with ``failure_mode='NaN'``).
- PIPE-55: an empty last mask with ``failure_mode='NaN'`` loses nipype's pre-filter basis;
  larmorx returns it.
- PIPE-56: a repetition time read from a header in microseconds is converted to seconds (nipype
  converts milliseconds only).
- PIPE-57: an unknown ``filter_type`` is refused (``ValueError``); nipype filters nothing.
- PIPE-58: ``mask_names`` shorter than the masks is refused (``ValueError``); nipype drops the
  masks without a name.
- PIPE-59: a failed SVD with ``failure_mode='NaN'`` is filled at the decomposition's size
  ``min(T, N)`` (nipype: ``T`` singular values, and one component for ``'all'``).

nipype's ``fallback_svd`` with ``full_matrices=True`` and ``compute_uv=False`` (no caller of the
module uses them) is not ported (``NotImplementedError``).
"""

from __future__ import annotations

import logging
import os
from collections import OrderedDict
from collections.abc import Sequence
from dataclasses import dataclass
from itertools import chain
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "CompCorResult",
    "acompcor",
    "combine_mask_files",
    "compcor",
    "compute_noise_components",
    "compute_tstd",
    "cosine_drift",
    "cosine_filter",
    "fallback_svd",
    "full_rank",
    "regress_poly",
    "tcompcor",
]

NUMPY_NEWEST = "2.5.4"
SCIPY_NEWEST = "1.18.1"

logger = logging.getLogger("larmorx.confounds")


def _numpy_lapack(numpy_version: str) -> str:
    """The LAPACK release of numpy ``numpy_version``'s OpenBLAS."""
    major, minor = (int(v) for v in str(numpy_version).split(".")[:2])
    return "3.12.1" if (major, minor) >= (2, 4) else "3.12.0"


def _scipy_library(scipy_version: str) -> str:
    """The SciPy library ``_core``'s ``fallback_svd`` follows (1.15.x: 1.15.3's wheel)."""
    v = str(scipy_version)
    if v.startswith("1.18"):
        return "1.18.1"
    if v.startswith("1.15"):
        return "1.15.3"
    raise ValueError(f"scipy must be a 1.15 or 1.18 release, got {v!r}")


# --- the numerics --------------------------------------------------------------------------------


def cosine_drift(
    period_cut: float, frametimes: Any, *, upstream_defects: UpstreamDefectsName = "fix"
) -> np.ndarray:
    """nipype's ``_cosine_drift(period_cut, frametimes)``: the ``T × order`` discrete cosine
    basis with periods of at least ``period_cut`` seconds and a constant last column (float64).
    The cosine is correctly rounded by default, glibc's with ``upstream_defects="reproduce"``
    (numpy's ``np.cos``; GLIBC-07)."""
    _defects.check(upstream_defects)
    ft = np.ascontiguousarray(np.asarray(frametimes), dtype=np.float64)
    out, notices = _core.nipype_cosine_drift(float(period_cut), ft, upstream_defects)
    _defects.warn(notices)
    return out


def full_rank(
    X: Any,
    cmax: float = 1e15,
    *,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    n_threads: int = 1,
) -> tuple[np.ndarray, float]:
    """nipype's ``_full_rank(X, cmax)``: ``X`` (float64), regularized by a multiple of the
    identity added to its singular values when its condition number is ``cmax`` or more (nipype
    then logs "Matrix is singular at working precision, regularizing..."), and the condition
    number (or ``cmax``). The SVD is nipype's ``fallback_svd`` with numpy ``numpy_version``'s
    and SciPy ``scipy_version``'s libraries."""
    x = np.asarray(X, dtype=np.float64)
    out, c, regularized = _core.nipype_full_rank(
        x,
        float(cmax),
        lapack=_numpy_lapack(numpy_version),
        scipy=_scipy_library(scipy_version),
        n_threads=n_threads,
    )
    if regularized:
        logger.warning("Matrix is singular at working precision, regularizing...")
        return out, float(c)
    return X if isinstance(X, np.ndarray) else x, float(c)


def fallback_svd(
    a: Any,
    full_matrices: bool = True,
    compute_uv: bool = True,
    *,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """nipype's ``fallback_svd(a, full_matrices, compute_uv)``: ``numpy.linalg.svd``, and on its
    ``LinAlgError`` ``scipy.linalg.svd(..., lapack_driver='gesvd')``. Only ``full_matrices=False``
    with ``compute_uv=True`` (what nipype's callers use) is ported."""
    u, s, vt, _ = _core.nipype_fallback_svd(
        np.asarray(a, dtype=np.float64),
        full_matrices=bool(full_matrices),
        compute_uv=bool(compute_uv),
        lapack=_numpy_lapack(numpy_version),
        scipy=_scipy_library(scipy_version),
        n_threads=n_threads,
    )
    return u, s, vt


def compute_tstd(
    M: Any, x: float, axis: int = 0, *, numpy_version: str = NUMPY_NEWEST
) -> np.ndarray:
    """nipype's ``_compute_tSTD(M, x, axis)``: ``np.std(M, axis=axis)`` (numpy
    ``numpy_version``'s summation order) with 0 and NaN replaced by ``x``."""
    m = np.asarray(M)
    if m.dtype not in (np.float32, np.float64):
        m = m.astype(np.float64)
    if m.ndim == 2 and not (m.flags.c_contiguous or m.flags.f_contiguous):
        m = np.ascontiguousarray(m)
    axis = int(axis)
    if m.ndim and -m.ndim <= axis < 0:
        axis += m.ndim
    std = np.asarray(_core.numpy_reduce(m, "std", axis, str(numpy_version)))
    std = np.array(std, copy=True)
    std[std == 0] = x
    std[np.isnan(std)] = x
    return std


def _check_axis(data: np.ndarray, axis: int, upstream_defects: str, what: str) -> None:
    """PIPE-53: an ``axis`` other than the last."""
    if data.ndim == 0 or (int(axis) % data.ndim) == data.ndim - 1:
        return
    if upstream_defects == "fix":
        raise ValueError(
            f"{what}: axis must be the last axis (the time axis); nipype reshapes the data in "
            f"memory order to rows of axis {axis}'s length (PIPE-53)"
        )
    _defects.warn(
        [
            (
                "PIPE-53",
                "nipype regress_poly, cosine_filter",
                upstream_defects,
                f"axis {axis} is not the last: the data are reshaped in memory order to rows of "
                "its length (the series regressed are not the voxels' series)",
            )
        ]
    )


def _rows(data: np.ndarray, timepoints: int) -> np.ndarray:
    """``data.reshape((-1, timepoints))`` as float32 or float64 (numpy's casts are exact)."""
    flat = data.reshape((-1, timepoints))
    if flat.dtype.kind == "c":
        raise NotImplementedError("complex series are not ported")
    if flat.dtype == np.float16:
        flat = flat.astype(np.float32)
    elif flat.dtype not in (np.float32, np.float64):
        flat = flat.astype(np.float64)
    return np.ascontiguousarray(flat)


def regress_poly(
    degree: int,
    data: Any,
    remove_mean: bool = True,
    axis: int = -1,
    failure_mode: str = "error",
    *,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray]:
    """nipype's ``regress_poly(degree, data, remove_mean, axis, failure_mode)``: ``data`` with
    the Legendre polynomials of degree 0 (unless ``remove_mean=False``) to ``degree`` regressed
    out of each series along the last axis (float64), and the non-constant regressors (PIPE-52:
    the polynomials of degree 1 to ``degree``; nipype's ``X[:, :-1]`` with ``reproduce``), or
    ``np.array([])`` for degree 0. An empty ``data`` with a ``failure_mode`` other than
    ``"error"`` is returned as it is. PIPE-53: ``axis`` must be the last axis."""
    _defects.check(upstream_defects)
    data = np.asarray(data)
    datashape = data.shape
    timepoints = datashape[axis]
    if datashape[0] == 0 and failure_mode != "error":
        return data, np.array([])
    _check_axis(data, axis, upstream_defects, "regress_poly")
    k = len(range(degree))  # nipype's range(degree): a float raises TypeError
    flat = _rows(data, timepoints)
    res, basis, notices = _core.nipype_regress_poly(
        k, flat, bool(remove_mean), str(numpy_version), upstream_defects, n_threads
    )
    _defects.warn(notices)
    return res.reshape(datashape), (basis if basis is not None else np.array([]))


def cosine_filter(
    data: Any,
    timestep: float,
    period_cut: float,
    remove_mean: bool = True,
    axis: int = -1,
    failure_mode: str = "error",
    *,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray]:
    """nipype's ``cosine_filter(data, timestep, period_cut, remove_mean, axis, failure_mode)``:
    ``data`` with the discrete cosine basis of periods of at least ``period_cut`` seconds (and
    the constant, unless ``remove_mean=False``) regressed out of each series along the last axis
    by least squares (float64), and the cosines, or ``np.array([])`` when there are none. An empty
    ``data`` with a ``failure_mode`` other than ``"error"`` is returned as it is. PIPE-53:
    ``axis`` must be the last axis."""
    _defects.check(upstream_defects)
    data = np.asarray(data)
    datashape = data.shape
    timepoints = datashape[axis]
    if datashape[0] == 0 and failure_mode != "error":
        return data, np.array([])
    _check_axis(data, axis, upstream_defects, "cosine_filter")
    flat = _rows(data, timepoints)
    timestep * np.arange(timepoints)  # nipype's frametimes: its TypeError for a non-number
    res, basis, notices = _core.nipype_cosine_filter(
        flat,
        float(timestep),
        float(period_cut),
        bool(remove_mean),
        str(numpy_version),
        str(scipy_version),
        upstream_defects,
        n_threads,
    )
    _defects.warn(notices)
    return res.reshape(datashape), (basis if basis is not None else np.array([]))


# --- masks ---------------------------------------------------------------------------------------


def _load(img: Any, n_threads: int = 1):
    """An :class:`larmorx.Image` as nibabel loads it (``nb.load``), from a path or an image."""
    from larmorx.image import Image
    from larmorx.io.nibabel_header import load_nibabel

    if isinstance(img, Image):
        return img
    return load_nibabel(os.fspath(img), n_threads=n_threads)


def _squeezed_data(img: Any) -> np.ndarray:
    """``np.asanyarray(nb.squeeze_image(img).dataobj)``: the trailing axes of length 1 after the
    third removed."""
    data = np.asanyarray(img.data)
    shape = data.shape
    slen = len(shape)
    for bdim in shape[3:][::-1]:
        if bdim == 1:
            slen -= 1
        else:
            break
    return data.reshape(shape[:slen]) if slen != len(shape) else data


def _as_int32_positive(data: np.ndarray) -> np.ndarray:
    """``data.astype("int32") > 0`` as x86-64 computes it (an out-of-range or NaN float becomes
    ``INT32_MIN``; integers wrap)."""
    if data.dtype.kind == "f":
        with np.errstate(invalid="ignore"):
            return (data >= 1.0) & (data < 2147483648.0)
    if data.dtype.kind == "b":
        return data.copy()
    return data.astype(np.int32) > 0


def combine_mask_files(
    mask_files: Sequence[Any] | Any,
    mask_method: str | None = None,
    mask_index: int | None = None,
    *,
    n_threads: int = 1,
) -> list:
    """nipype's ``combine_mask_files(mask_files, mask_method, mask_index)``: the masks as images
    (:class:`larmorx.Image`, nibabel's ``nb.load``): the ``mask_index``-th (the only one when
    neither argument is given), all of them (``"none"``), or their union or intersection
    (``"union"``, ``"intersect"``: ``astype("int32") > 0`` of each, with the last file's affine
    and header). nipype's ``ValueError``s for several files without a method or an index, and
    for an index past the list."""
    from larmorx.image import Image

    if isinstance(mask_files, (str, os.PathLike)) or not isinstance(mask_files, Sequence):
        mask_files = [mask_files]
    if mask_index is not None or mask_method is None:
        if mask_index is None:
            if len(mask_files) == 1:
                mask_index = 0
            else:
                raise ValueError(
                    "When more than one mask file is provided, one of merge_method or mask_index "
                    "must be set"
                )
        if mask_index < len(mask_files):
            return [_load(mask_files[mask_index], n_threads)]
        raise ValueError(
            f"mask_index {mask_index} must be less than number of mask files {len(mask_files)}"
        )
    if mask_method == "none":
        return [_load(f, n_threads) for f in mask_files]
    if mask_method in ("union", "intersect"):
        mask = None
        img = None
        for f in mask_files:
            img = _load(f, n_threads)
            as_mask = _as_int32_positive(np.asanyarray(img.data))
            if mask is None:
                mask = as_mask
            if mask_method == "union":
                np.logical_or(mask, as_mask, mask)
            else:
                np.logical_and(mask, as_mask, mask)
        return [Image(mask, img.affine, img.header)]
    return None


# --- compute_noise_components --------------------------------------------------------------------


def _criterion(c: Any) -> tuple[Any, bool]:
    if isinstance(c, str):
        if c != "all":
            raise ValueError(f"components_criterion must be 'all' or a number, not {c!r}")
        return "all", False
    return float(c), isinstance(c, (int, np.integer))


def compute_noise_components(
    imgseries: Any,
    mask_images: Sequence[Any],
    components_criterion: float | int | str = 0.5,
    filter_type: str | bool = False,
    degree: int = 0,
    period_cut: float = 128,
    repetition_time: float | None = None,
    failure_mode: str = "error",
    mask_names: Sequence[Any] | None = None,
    *,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray, OrderedDict]:
    """nipype's ``compute_noise_components``: the CompCor components of the 4-D array
    ``imgseries`` within each mask (images or paths; module documentation): ``(components,
    basis, metadata)``, the components ``T × n`` (the masks' in order), the pre-filter's
    non-constant regressors (the last mask's) or ``np.array([])``, and the metadata (``mask``,
    ``singular_value``, ``variance_explained``, ``cumulative_variance_explained``,
    ``retained``), with nipype's dtypes."""
    _defects.check(upstream_defects)
    imgseries = np.asarray(imgseries)
    crit, crit_is_int = _criterion(components_criterion)
    names = list(mask_names or range(len(mask_images)))
    masks = list(mask_images)
    if len(names) < len(masks):
        if upstream_defects == "fix":
            raise ValueError(
                f"mask_names has {len(names)} names for {len(masks)} masks; nipype drops the "
                "masks without a name (PIPE-58)"
            )
        _defects.warn(
            [
                (
                    "PIPE-58",
                    "nipype compute_noise_components",
                    upstream_defects,
                    f"{len(masks) - len(names)} masks without a name are dropped",
                )
            ]
        )
        masks = masks[: len(names)]
    if filter_type == "cosine":
        ftype = "cosine"
    elif filter_type == "polynomial":
        ftype = "polynomial"
    elif filter_type is False:
        ftype = "false"
    else:
        ftype = "other"
    # The masks' rows; a mask of another shape raises when nipype's loop reaches it.
    rows, mismatch = [], None
    series = imgseries
    if series.dtype.kind == "c":
        raise NotImplementedError("complex series are not ported")
    if series.dtype == np.float16:
        series = series.astype(np.float32)
    elif series.dtype not in (np.float32, np.float64):
        series = series.astype(np.float64)
    for img in masks:
        mask = _squeezed_data(_load(img, n_threads)).astype(bool)
        if imgseries.shape[:3] != mask.shape:
            mismatch = (
                "Inputs for CompCor, timeseries and mask, do not have matching spatial "
                f"dimensions ({imgseries.shape[:3]} and {mask.shape}, respectively)"
            )
            break
        rows.append(np.ascontiguousarray(series[mask, :]))
    t = imgseries.shape[3] if imgseries.ndim > 3 else 0
    period = float(period_cut)
    comps, comp_f32, basis, md, notices = _core.nipype_compute_noise_components(
        rows,
        t,
        crit,
        crit_is_int,
        ftype,
        max(int(degree), 0),
        period,
        None if repetition_time is None else float(repetition_time),
        "error" if failure_mode == "error" else "NaN",
        str(numpy_version),
        str(scipy_version),
        upstream_defects,
        n_threads,
        mismatch,
    )
    _defects.warn(notices)
    components = comps.astype(np.float32) if comp_f32 else comps
    if md:
        sv = np.hstack([s.astype(np.float32) if f32 else s for (_, s, _, _, _, f32) in md])
        var = np.hstack([v.astype(np.float32) if f32 else v for (_, _, v, _, _, f32) in md])
        cum = np.hstack([c.astype(np.float32) if f32 else c for (_, _, _, c, _, f32) in md])
    else:
        sv = var = cum = np.array([])
    metadata = OrderedDict(
        [
            ("mask", list(chain(*[[names[i]] * len(s) for (i, s, _, _, _, _) in md]))),
            ("singular_value", sv),
            ("variance_explained", var),
            ("cumulative_variance_explained", cum),
            (
                "retained",
                list(chain(*[[j < r for j in range(len(s))] for (_, s, _, _, r, _) in md])),
            ),
        ]
    )
    return components, (basis if basis is not None else np.array([])), metadata


# --- the interfaces ------------------------------------------------------------------------------


@dataclass(frozen=True)
class CompCorResult:
    """The CompCor interfaces' outputs: the files written (``None`` when not asked for) and, for
    ``TCompCor``, the high-variance masks; with the arrays ``compute_noise_components`` gave."""

    components_file: str
    pre_filter_file: str | None
    metadata_file: str | None
    high_variance_masks: tuple[str, ...]
    components: np.ndarray
    basis: np.ndarray
    metadata: OrderedDict


def _repetition_time(img: Any, header_name: str, upstream_defects: str) -> np.float32 | float:
    """``CompCor``'s TR from the header: ``get_zooms()[3]`` (float32), divided by 1000 for
    ``msec`` (and, PIPE-56, by 1000000 for ``usec``); 0 when the header has none."""
    hdr = img.header
    if hdr is None or hdr.magic.rstrip(b"\x00") not in (b"n+1", b"ni1", b"n+2", b"ni2"):
        tr = 0
    else:
        tr = np.float32(hdr.pixdim[4])
        unit = hdr.xyzt_units - hdr.xyzt_units % 8  # nibabel's get_xyzt_units
        if unit == 16:
            tr /= 1000
        elif unit == 24:
            if upstream_defects == "fix":
                tr /= 1000000
            _defects.warn(
                [
                    (
                        "PIPE-56",
                        "nipype CompCor",
                        upstream_defects,
                        "the header's repetition time is in microseconds: "
                        + (
                            "converted to seconds"
                            if upstream_defects == "fix"
                            else "taken as seconds, as nipype takes it"
                        ),
                    )
                ]
            )
    if tr == 0:
        raise ValueError(
            f"{header_name} cannot detect repetition time from image - Set the repetition_time "
            "input"
        )
    return tr


#: The input specification nipype's deprecation message names for each header (``ACompCor``
#: shares ``CompCor``'s).
_CLASS = {"CompCor": "CompCor", "aCompCor": "CompCor", "tCompCor": "TCompCor"}


def _savetxt(path: str, data: np.ndarray, header: str) -> None:
    """nipype's ``np.savetxt(path, data, fmt="%.10f", delimiter="\\t", header=header,
    comments="")``."""
    np.savetxt(path, data, fmt="%.10f", delimiter="\t", header=header, comments="")


def _compcor(
    header_name: str,
    realigned_file: Any,
    mask_files: Sequence[Any] | Any | None,
    *,
    merge_method: str | None,
    mask_index: int | None,
    mask_names: Sequence[Any] | None,
    components_file: str,
    num_components: int | str | None,
    variance_threshold: float | None,
    pre_filter: str | bool,
    use_regress_poly: bool | None,
    regress_poly_degree: int,
    header_prefix: str | None,
    high_pass_cutoff: float,
    repetition_time: float | None,
    save_pre_filter: bool | str,
    save_metadata: bool | str,
    ignore_initial_volumes: int,
    failure_mode: str,
    percentile_threshold: float | None,
    out_dir: Any,
    numpy_version: str,
    scipy_version: str,
    upstream_defects: str,
    n_threads: int,
) -> CompCorResult:
    from larmorx.image import Image
    from larmorx.io.nibabel_header import load_nibabel
    from larmorx.io.nibabel_writer import save_nibabel

    _defects.check(upstream_defects)
    # The interface's traits (use_regress_poly is deprecated: nipype's traits refuse it).
    if use_regress_poly is not None:
        raise ValueError(
            f"Input use_regress_poly in interface {_CLASS[header_name]} is deprecated. Will be "
            "removed or "
            "raise an error as of release 0.15.0 It has been replaced by pre_filter."
        )
    if pre_filter not in ("polynomial", "cosine", False):
        raise ValueError(f"pre_filter must be 'polynomial', 'cosine' or False, not {pre_filter!r}")
    if num_components is not None and variance_threshold is not None:
        raise ValueError("num_components and variance_threshold are mutually exclusive")
    if num_components is not None and num_components != "all" and int(num_components) < 1:
        raise ValueError("num_components must be 'all' or at least 1")
    if variance_threshold is not None and not 0.0 < variance_threshold < 1.0:
        raise ValueError("variance_threshold must be between 0 and 1, exclusive")
    if merge_method is not None and mask_index is not None:
        raise ValueError("merge_method and mask_index are mutually exclusive")
    if merge_method is not None and merge_method not in ("union", "intersect", "none"):
        raise ValueError(
            f"merge_method must be 'union', 'intersect' or 'none', not {merge_method!r}"
        )
    if (merge_method is not None or mask_index is not None) and not mask_files:
        raise ValueError("merge_method and mask_index require mask_files")
    if int(regress_poly_degree) < 1 or int(ignore_initial_volumes) < 0:
        raise ValueError(
            "regress_poly_degree must be at least 1, ignore_initial_volumes at least 0"
        )
    if failure_mode not in ("error", "NaN"):
        raise ValueError(f"failure_mode must be 'error' or 'NaN', not {failure_mode!r}")
    if percentile_threshold is not None and not 0.0 < percentile_threshold < 1.0:
        raise ValueError("percentile_threshold must be between 0 and 1, exclusive")
    out_dir = os.fspath(out_dir)
    os.makedirs(out_dir, exist_ok=True)

    mask_images = []
    if mask_files:
        mask_images = combine_mask_files(mask_files, merge_method, mask_index, n_threads=n_threads)
    if use_regress_poly:
        pre_filter = "polynomial"
    degree = int(regress_poly_degree) if pre_filter == "polynomial" else 0
    path = os.fspath(realigned_file)
    raw = load_nibabel(path, n_threads=n_threads)
    shape = np.asarray(raw.data).shape
    if len(shape) != 4:
        raise ValueError(
            f"{header_name} expected a 4-D nifti file. Input {path} has {len(shape)} dimensions "
            f"(shape {shape})"
        )
    if len(mask_images) == 0:
        mask_images = [Image(np.ones(shape[:3], dtype=bool), raw.affine, raw.header)]
    skip_vols = int(ignore_initial_volumes)
    dataobj = np.asarray(raw.data)[..., skip_vols:] if skip_vols else np.asarray(raw.data)

    high_variance = []
    if percentile_threshold is not None:
        # TCompCor._process_masks: the series' values (nibabel's dataobj) in each mask.
        out_images = []
        for i, img in enumerate(mask_images):
            mask = np.asanyarray(img.data).astype(bool)
            imgseries = dataobj[mask, :]
            if imgseries.dtype.kind == "c":
                raise NotImplementedError("complex series are not ported")
            hv = _core.nipype_tcompcor_mask(
                np.ascontiguousarray(imgseries, dtype=np.float64),
                float(percentile_threshold),
                str(numpy_version),
                n_threads,
            )
            mask_data = np.zeros_like(mask)
            mask_data[mask != 0] = hv
            out_image = Image(mask_data, img.affine, img.header)
            mask_file = os.path.abspath(os.path.join(out_dir, f"mask_{i:03d}.nii.gz"))
            save_nibabel(out_image, mask_file, n_threads=n_threads)
            high_variance.append(mask_file)
            out_images.append(out_image)
        mask_images = out_images

    tr: Any = 0
    if pre_filter == "cosine":
        if repetition_time is not None:
            tr = repetition_time
        else:
            tr = _repetition_time(raw, header_name, upstream_defects)

    if variance_threshold is not None:
        criterion: Any = variance_threshold
    elif num_components is not None:
        criterion = num_components
    else:
        criterion = 6
        logger.warning(
            "`num_components` and `variance_threshold` are not defined. Setting number of "
            "components to 6 for backward compatibility. Please set either `num_components` or "
            "`variance_threshold`, as this feature may be deprecated in the future."
        )

    # imgseries.get_fdata(dtype=np.float32) (of the sliced dataobj when volumes are skipped).
    if skip_vols:
        series = dataobj.astype(np.float32)
    elif dataobj.dtype == np.float32:
        series = dataobj
    else:
        series = np.asarray(load_nibabel(path, dtype=np.float32, n_threads=n_threads).data)
    del raw, dataobj
    components, filter_basis, metadata = compute_noise_components(
        series,
        mask_images,
        criterion,
        pre_filter,
        degree,
        high_pass_cutoff,
        tr if pre_filter == "cosine" else 0,
        failure_mode,
        mask_names,
        numpy_version=numpy_version,
        scipy_version=scipy_version,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )
    del series
    nrows = 0
    if skip_vols:
        old_comp = components
        nrows = skip_vols + components.shape[0]
        components = np.zeros((nrows, components.shape[1]), dtype=components.dtype)
        components[skip_vols:] = old_comp

    prefix = header_prefix if header_prefix is not None else header_name
    components_header = [f"{prefix}{i:02d}" for i in range(components.shape[1])]
    comp_path = os.path.join(out_dir, components_file)
    _savetxt(comp_path, components, "\t".join(components_header))

    pre_filter_path = None
    if pre_filter in ("polynomial", "cosine") and save_pre_filter:
        pre_filter_path = (
            os.path.join(out_dir, "pre_filter.tsv")
            if save_pre_filter is True
            else os.path.join(out_dir, os.fspath(save_pre_filter))
        )
        ftype = {"polynomial": "Legendre", "cosine": "Cosine"}[pre_filter]
        ncols = filter_basis.shape[1] if filter_basis.size > 0 else 0
        header = [f"{ftype}{i:02d}" for i in range(ncols)]
        if skip_vols:
            old_basis = filter_basis
            filter_basis = np.zeros((nrows, ncols + skip_vols), dtype=filter_basis.dtype)
            if old_basis.size > 0:
                filter_basis[skip_vols:, :ncols] = old_basis
            filter_basis[:skip_vols, -skip_vols:] = np.eye(skip_vols)
            header.extend([f"NonSteadyStateOutlier{i:02d}" for i in range(skip_vols)])
        _savetxt(pre_filter_path, filter_basis, "\t".join(header))

    metadata_path = None
    if save_metadata:
        metadata_path = (
            os.path.join(out_dir, "component_metadata.tsv")
            if save_metadata is True
            else os.path.join(out_dir, os.fspath(save_metadata))
        )
        components_names = np.empty(len(metadata["mask"]), dtype="object_")
        retained = np.where(metadata["retained"])
        not_retained = np.where(np.logical_not(metadata["retained"]))
        components_names[retained] = components_header
        components_names[not_retained] = [f"dropped{i}" for i in range(len(not_retained[0]))]
        with open(metadata_path, "w", encoding="utf-8") as f:
            f.write("\t".join(["component", *list(metadata.keys())]) + "\n")
            for i in zip(components_names, *metadata.values(), strict=False):
                f.write(f"{i[0]}\t{i[1]}\t{i[2]:.10f}\t{i[3]:.10f}\t{i[4]:.10f}\t{i[5]}\n")

    return CompCorResult(
        components_file=os.path.abspath(comp_path),
        pre_filter_file=None if pre_filter_path is None else os.path.abspath(pre_filter_path),
        metadata_file=None if metadata_path is None else os.path.abspath(metadata_path),
        high_variance_masks=tuple(high_variance),
        components=components,
        basis=filter_basis,
        metadata=metadata,
    )


_DOC_OPTIONS = """
    Options (nipype's inputs, their defaults): ``mask_files`` (one or more 3-D masks; none: the
    whole volume), ``merge_method`` (``"union"``, ``"intersect"``, ``"none"``) or
    ``mask_index``, ``mask_names``, ``components_file`` (``"components_file.txt"``),
    ``num_components`` (a count or ``"all"``) or ``variance_threshold`` (a fraction; neither: 6
    components, with nipype's warning), ``pre_filter`` (``"polynomial"``, ``"cosine"``,
    ``False``), ``use_regress_poly`` (deprecated: ``pre_filter="polynomial"``),
    ``regress_poly_degree`` (1), ``header_prefix``, ``high_pass_cutoff`` (128 s),
    ``repetition_time`` (default: the header's), ``save_pre_filter`` and ``save_metadata``
    (``True``: ``pre_filter.tsv``, ``component_metadata.tsv``; or a file name),
    ``ignore_initial_volumes`` (0), ``failure_mode`` (``"error"``, ``"NaN"``). The files are
    written in ``out_dir`` (nipype's node directory).
"""


def compcor(
    realigned_file: Any,
    mask_files: Sequence[Any] | Any | None = None,
    *,
    merge_method: str | None = None,
    mask_index: int | None = None,
    mask_names: Sequence[Any] | None = None,
    components_file: str = "components_file.txt",
    num_components: int | str | None = None,
    variance_threshold: float | None = None,
    pre_filter: str | bool = "polynomial",
    use_regress_poly: bool | None = None,
    regress_poly_degree: int = 1,
    header_prefix: str | None = None,
    high_pass_cutoff: float = 128.0,
    repetition_time: float | None = None,
    save_pre_filter: bool | str = False,
    save_metadata: bool | str = False,
    ignore_initial_volumes: int = 0,
    failure_mode: str = "error",
    out_dir: Any = ".",
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> CompCorResult:
    return _compcor("CompCor", **_kw(locals()))


def acompcor(
    realigned_file: Any,
    mask_files: Sequence[Any] | Any | None = None,
    *,
    merge_method: str | None = None,
    mask_index: int | None = None,
    mask_names: Sequence[Any] | None = None,
    components_file: str = "components_file.txt",
    num_components: int | str | None = None,
    variance_threshold: float | None = None,
    pre_filter: str | bool = "polynomial",
    use_regress_poly: bool | None = None,
    regress_poly_degree: int = 1,
    header_prefix: str | None = None,
    high_pass_cutoff: float = 128.0,
    repetition_time: float | None = None,
    save_pre_filter: bool | str = False,
    save_metadata: bool | str = False,
    ignore_initial_volumes: int = 0,
    failure_mode: str = "error",
    out_dir: Any = ".",
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> CompCorResult:
    return _compcor("aCompCor", **_kw(locals()))


def tcompcor(
    realigned_file: Any,
    mask_files: Sequence[Any] | Any | None = None,
    *,
    percentile_threshold: float = 0.02,
    merge_method: str | None = None,
    mask_index: int | None = None,
    mask_names: Sequence[Any] | None = None,
    components_file: str = "components_file.txt",
    num_components: int | str | None = None,
    variance_threshold: float | None = None,
    pre_filter: str | bool = "polynomial",
    use_regress_poly: bool | None = None,
    regress_poly_degree: int = 1,
    header_prefix: str | None = None,
    high_pass_cutoff: float = 128.0,
    repetition_time: float | None = None,
    save_pre_filter: bool | str = False,
    save_metadata: bool | str = False,
    ignore_initial_volumes: int = 0,
    failure_mode: str = "error",
    out_dir: Any = ".",
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> CompCorResult:
    return _compcor("tCompCor", **_kw(locals()))


def _kw(local: dict) -> dict:
    kw = dict(local)
    kw.setdefault("percentile_threshold", None)
    return kw


compcor.__doc__ = (
    """nipype's ``CompCor`` interface (header ``CompCor``): ``compute_noise_components`` of
    ``realigned_file`` (a 4-D NIfTI file, read with ``get_fdata(dtype=np.float32)``) within the
    masks, and nipype's files: the components (``%.10f``, tab-separated, a header of
    ``<prefix>NN``; zeros for the ignored initial volumes), the pre-filter's basis
    (``Legendre``/``Cosine`` columns, then ``NonSteadyStateOutlierNN`` columns) and the
    metadata (``component``, ``mask``, ``singular_value``, ``variance_explained``,
    ``cumulative_variance_explained``, ``retained``; the components not retained named
    ``droppedN``)."""
    + _DOC_OPTIONS
)
acompcor.__doc__ = (
    """nipype's ``ACompCor`` interface: :func:`compcor` with the header ``aCompCor``."""
    + _DOC_OPTIONS
)
tcompcor.__doc__ = (
    """nipype's ``TCompCor`` interface: :func:`compcor` with the header ``tCompCor``, on the
    high-variance voxels of each mask (its series, as nibabel's ``dataobj`` gives it, detrended
    by ``regress_poly(2, ...)``; the voxels whose SD is at or above the ``round(100 (1 -
    percentile_threshold))``th percentile), each written as ``mask_NNN.nii.gz`` with the mask's
    header (``high_variance_masks``)."""
    + _DOC_OPTIONS
)

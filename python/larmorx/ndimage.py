# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""SciPy-compatible spline interpolation, filters, morphology and labelling: ``lx.ndimage``.

The filters (``correlate1d``, ``gaussian_filter1d``, ``gaussian_filter``, minimum, maximum,
rank, median, percentile), grey and binary morphology,
``label``, ``sum_labels``, ``center_of_mass`` and ``watershed_ift`` are SciPy 1.18.1's, bit for
bit, from ``larmorx._ndimage_filters`` (``docs/api/ndimage-filters.md``). The rest of this text is about the interpolation.

A replica of ``scipy.ndimage``'s ``map_coordinates``, ``affine_transform`` (3-D arrays),
``spline_filter`` and ``spline_filter1d`` (SciPy 1.18.1; first ported from 1.15.2), in Rust. Results are
bit-identical to SciPy's on x86-64
Linux for every order (0-5) and boundary mode, and the same on every platform. The work runs
on ``n_threads`` threads with the GIL released; the result does not depend on the thread
count.

Differences from SciPy: complex inputs and ``output`` arrays are not supported (pass a dtype),
and arrays have 1 to 4 dimensions. See ``docs/api/resample-series.md``.

The spline prefilter's ``reflect`` boundary (modes ``reflect``, ``grid-mirror`` and
``nearest``) is SciPy 1.18.1's: SciPy 1.15 read a coefficient it had already updated, so its
splines did not pass through the samples on short axes (SCI-03, ``docs/api/upstream-defects.md``);
SciPy 1.18 fixed it, and larmorx follows the fix in both modes of ``upstream_defects``.

The argument checks and their messages follow ``scipy/ndimage/_interpolation.py`` (SciPy
1.18.1, unchanged there from 1.15.2; BSD-3-Clause; the notice is in the NOTICE file of
larmorx).
"""

from __future__ import annotations

from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx._ndimage_filters import (
    binary_closing,
    binary_dilation,
    binary_erosion,
    binary_fill_holes,
    binary_hit_or_miss,
    binary_opening,
    binary_propagation,
    center_of_mass,
    correlate1d,
    gaussian_filter,
    gaussian_filter1d,
    generate_binary_structure,
    grey_closing,
    grey_dilation,
    grey_erosion,
    grey_opening,
    iterate_structure,
    label,
    maximum_filter,
    maximum_filter1d,
    median_filter,
    minimum_filter,
    minimum_filter1d,
    percentile_filter,
    rank_filter,
    sum,
    sum_labels,
    watershed_ift,
)

__all__ = [
    "MODES",
    "affine_transform",
    "binary_closing",
    "binary_dilation",
    "binary_erosion",
    "binary_fill_holes",
    "binary_hit_or_miss",
    "binary_opening",
    "binary_propagation",
    "center_of_mass",
    "correlate1d",
    "gaussian_filter",
    "gaussian_filter1d",
    "generate_binary_structure",
    "grey_closing",
    "grey_dilation",
    "grey_erosion",
    "grey_opening",
    "iterate_structure",
    "label",
    "map_coordinates",
    "maximum_filter",
    "maximum_filter1d",
    "median_filter",
    "minimum_filter",
    "minimum_filter1d",
    "percentile_filter",
    "rank_filter",
    "spline_filter",
    "spline_filter1d",
    "sum",
    "sum_labels",
    "watershed_ift",
]

#: SciPy's boundary modes.
MODES = (
    "reflect",
    "grid-mirror",
    "constant",
    "grid-constant",
    "nearest",
    "mirror",
    "grid-wrap",
    "wrap",
)

_INPUT_TYPES = (
    np.float32,
    np.float64,
    np.int16,
    np.uint8,
    np.int8,
    np.uint16,
    np.int32,
    np.uint32,
    np.int64,
    np.uint64,
)


def _check_mode(mode: str) -> str:
    if mode not in MODES:
        raise RuntimeError("boundary mode not supported")
    return mode


def map_coordinates(
    input: Any,
    coordinates: Any,
    output: Any = None,
    order: int = 3,
    mode: str = "constant",
    cval: float = 0.0,
    prefilter: bool = True,
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """Map ``input`` to new coordinates by spline interpolation (``scipy.ndimage``).

    ``coordinates[d]`` holds the index along axis ``d`` of each output point; the output has
    the shape ``coordinates.shape[1:]``. ``output`` is a dtype (default: the input's), not an
    array. With ``prefilter`` and ``order > 1`` the input is padded (``nearest`` and
    ``grid-constant``) and spline-filtered in float64 first, as SciPy does.
    The ``reflect`` boundary is SciPy 1.18.1's in both modes of ``upstream_defects`` (SCI-03,
    fixed upstream).
    """
    _defects.check(upstream_defects)
    if order < 0 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input)
    if data.dtype.kind == "c":
        raise TypeError("complex input is not supported by larmorx")
    if data.dtype == np.bool_ or data.dtype.type not in _INPUT_TYPES or not data.dtype.isnative:
        data = data.astype(np.float64)
    coords = np.asarray(coordinates)
    if np.iscomplexobj(coords):
        raise TypeError("Complex type not supported")
    out_shape = coords.shape[1:]
    if data.ndim < 1 or len(out_shape) < 1:
        raise RuntimeError("input and output rank must be > 0")
    if coords.shape[0] != data.ndim:
        raise RuntimeError("invalid shape for coordinate array")
    out_dtype = np.dtype(data.dtype if output is None else output)
    flat = np.ascontiguousarray(coords.reshape(data.ndim, -1), dtype=np.float64)
    values, notices = _core.ndimage_map_coordinates(
        data,
        flat,
        order=int(order),
        mode=_check_mode(mode),
        cval=float(cval),
        prefilter=bool(prefilter),
        output=out_dtype.name,
        n_threads=int(n_threads),
        upstream_defects=upstream_defects,
    )
    _defects.warn(notices)
    return values.reshape(out_shape)


def affine_transform(
    input: Any,
    matrix: Any,
    offset: Any = 0.0,
    output_shape: Any = None,
    output: Any = None,
    order: int = 3,
    mode: str = "constant",
    cval: float = 0.0,
    prefilter: bool = True,
    *,
    scipy_version: str = "1.18.1",
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """Apply an affine transformation (``scipy.ndimage.affine_transform``) to a 3-D ``input``.

    The value at output index ``o`` is ``input`` interpolated at ``matrix @ o + offset`` (a 3 x 3
    matrix, or 4 x 4 / 3 x 4 homogeneous, whose last column is the offset; numbers added in
    SciPy's order) or, for a vector ``matrix`` (a diagonal), at ``(o + offset / matrix) *
    matrix`` (SciPy's ``zoom_shift`` path, whose roundings differ). Interpolation, boundary modes
    and the conversion to ``output`` (a dtype) are :func:`map_coordinates`'. 3-D inputs only.
    SciPy 1.16 changed the order of a matrix's sums (the offset first; before, last):
    ``scipy_version`` (``"1.15.3"`` for the older order) selects it; the default is the newest.
    """
    _defects.check(upstream_defects)
    if order < 0 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input)
    if output_shape is None:
        output_shape = data.shape
    output_shape = tuple(int(n) for n in output_shape)
    if data.ndim < 1 or len(output_shape) < 1:
        raise RuntimeError("input and output rank must be > 0")
    if data.dtype.kind == "c":
        raise TypeError("complex input is not supported by larmorx")
    if data.ndim != 3 or len(output_shape) != 3:
        raise NotImplementedError("larmorx's affine_transform takes 3-D inputs and outputs")
    if data.dtype == np.bool_ or data.dtype.type not in _INPUT_TYPES or not data.dtype.isnative:
        data = data.astype(np.float64)
    matrix = np.asarray(matrix, dtype=np.float64)
    if matrix.ndim not in (1, 2) or matrix.shape[0] < 1:
        raise RuntimeError("no proper affine matrix provided")
    if (
        matrix.ndim == 2
        and matrix.shape[1] == data.ndim + 1
        and matrix.shape[0] in (data.ndim, data.ndim + 1)
    ):
        if matrix.shape[0] == data.ndim + 1:
            exptd = [0] * data.ndim + [1]
            if not np.all(matrix[data.ndim] == exptd):
                raise ValueError(
                    f"Expected homogeneous transformation matrix with shape {matrix.shape} "
                    f"for image shape {data.shape}, but bottom row was not equal to {exptd}"
                )
        offset = matrix[: data.ndim, data.ndim]
        matrix = matrix[: data.ndim, : data.ndim]
    if matrix.shape[0] != data.ndim:
        raise RuntimeError("affine matrix has wrong number of rows")
    if matrix.ndim == 2 and matrix.shape[1] != len(output_shape):
        raise RuntimeError("affine matrix has wrong number of columns")
    off = np.asarray(offset, dtype=np.float64)
    if off.ndim == 0:
        off = np.broadcast_to(off, (data.ndim,))
    elif off.ndim == 1 and off.shape[0] != data.ndim:
        # _ni_support._normalize_sequence
        raise RuntimeError("sequence argument must have length equal to input rank")
    if off.ndim != 1 or off.shape[0] < 1:
        raise RuntimeError("no proper offset provided")
    out_dtype = np.dtype(data.dtype if output is None else output)
    values, notices = _core.ndimage_affine_transform3(
        data,
        [float(v) for v in matrix.reshape(-1)],
        [float(v) for v in off],
        list(output_shape),
        order=int(order),
        mode=_check_mode(mode),
        cval=float(cval),
        prefilter=bool(prefilter),
        output=out_dtype.name,
        n_threads=int(n_threads),
        upstream_defects=upstream_defects,
        scipy_version=str(scipy_version),
    )
    _defects.warn(notices)
    return np.asarray(values).reshape(output_shape, order="F")


def spline_filter1d(
    input: Any,
    order: int = 3,
    axis: int = -1,
    mode: str = "mirror",
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The 1D B-spline prefilter along ``axis`` (float64 result), as ``scipy.ndimage``;
    ``upstream_defects`` as for :func:`map_coordinates`."""
    _defects.check(upstream_defects)
    if order < 0 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input, dtype=np.float64)
    out, notices = _core.ndimage_spline_filter1d(
        data,
        order=int(order),
        axis=int(axis),
        mode=_check_mode(mode),
        n_threads=int(n_threads),
        upstream_defects=upstream_defects,
    )
    _defects.warn(notices)
    return out


def spline_filter(
    input: Any,
    order: int = 3,
    mode: str = "mirror",
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The multidimensional B-spline prefilter (float64 result), as ``scipy.ndimage``;
    ``upstream_defects`` as for :func:`map_coordinates`."""
    _defects.check(upstream_defects)
    if order < 2 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input, dtype=np.float64)
    out, notices = _core.ndimage_spline_filter(
        data,
        order=int(order),
        mode=_check_mode(mode),
        n_threads=int(n_threads),
        upstream_defects=upstream_defects,
    )
    _defects.warn(notices)
    return out

# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""SciPy-compatible spline interpolation: ``lx.ndimage``.

A replica of ``scipy.ndimage``'s ``map_coordinates``, ``spline_filter`` and
``spline_filter1d`` (SciPy 1.15.2), in Rust. Results are bit-identical to SciPy's on x86-64
Linux for every order (0-5) and boundary mode, and the same on every platform. The work runs
on ``n_threads`` threads with the GIL released; the result does not depend on the thread
count.

Differences from SciPy: complex inputs and ``output`` arrays are not supported (pass a dtype),
and arrays have 1 to 4 dimensions. See ``docs/api/resample-series.md``.

The argument checks and their messages follow ``scipy/ndimage/_interpolation.py`` (SciPy
1.15.2, BSD-3-Clause; the notice is in the NOTICE file of larmorx).
"""

from __future__ import annotations

from typing import Any

import numpy as np

from larmorx import _core

__all__ = ["MODES", "map_coordinates", "spline_filter", "spline_filter1d"]

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
) -> np.ndarray:
    """Map ``input`` to new coordinates by spline interpolation (``scipy.ndimage``).

    ``coordinates[d]`` holds the index along axis ``d`` of each output point; the output has
    the shape ``coordinates.shape[1:]``. ``output`` is a dtype (default: the input's), not an
    array. With ``prefilter`` and ``order > 1`` the input is padded (``nearest`` and
    ``grid-constant``) and spline-filtered in float64 first, as SciPy does.
    """
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
    values = _core.ndimage_map_coordinates(
        data,
        flat,
        order=int(order),
        mode=_check_mode(mode),
        cval=float(cval),
        prefilter=bool(prefilter),
        output=out_dtype.name,
        n_threads=int(n_threads),
    )
    return values.reshape(out_shape)


def spline_filter1d(
    input: Any,
    order: int = 3,
    axis: int = -1,
    mode: str = "mirror",
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """The 1D B-spline prefilter along ``axis`` (float64 result), as ``scipy.ndimage``."""
    if order < 0 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input, dtype=np.float64)
    return _core.ndimage_spline_filter1d(
        data, order=int(order), axis=int(axis), mode=_check_mode(mode), n_threads=int(n_threads)
    )


def spline_filter(
    input: Any,
    order: int = 3,
    mode: str = "mirror",
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """The multidimensional B-spline prefilter (float64 result), as ``scipy.ndimage``."""
    if order < 2 or order > 5:
        raise RuntimeError("spline order not supported")
    data = np.asarray(input, dtype=np.float64)
    return _core.ndimage_spline_filter(
        data, order=int(order), mode=_check_mode(mode), n_threads=int(n_threads)
    )

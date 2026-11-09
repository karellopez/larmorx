# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""SciPy-compatible filters, morphology and labelling for ``lx.ndimage`` (imported there).

A replica of ``scipy.ndimage``'s ``correlate1d``, ``gaussian_filter1d`` and ``gaussian_filter``
(order 0; the Gaussian kernel with numpy's float64 ``exp`` on its path without AVX-512, glibc's,
and numpy's sum), minimum, maximum, rank, median and percentile filters, grey
and binary morphology, ``label``, ``sum_labels``, ``center_of_mass``, ``watershed_ift``,
``generate_binary_structure`` and ``iterate_structure`` (SciPy 1.18.1; the C and Cython code
of these functions is the same in SciPy 1.15.3), computed in Rust (``larmorx_ndimage``) with SciPy's results bit for bit: the
boundary modes, the order a footprint is visited in, the comparisons (NaNs and signed zeros
travel as in SciPy), the additions of a non-flat structuring element in the input's type,
SciPy's quickselect for the rank filters and C's casts. The argument handling and messages
follow ``scipy/ndimage/_filters.py``, ``_morphology.py``, ``_measurements.py`` and
``_ni_support.py`` (BSD-3-Clause; SciPy's notice is in larmorx's NOTICE file).

Differences from SciPy: complex inputs are not supported; ``output`` may be a dtype or an array
(the result is computed in the input's type and cast, which is SciPy's value except for
integers beyond 2^53 cast to floats); the n-dimensional ``reflect`` boundary maps a coordinate
two or more whole periods before an axis to index 0 where SciPy reads before the array (a
footprint more than eight times the axis); a binary erosion or dilation iterated until nothing
changes that alternates for ever raises instead of looping; ``watershed_ift`` corrects SciPy's
neighbours across the array's edges, its queue that keeps an element alone in it and its 32-bit
markers by default (``upstream_defects``; ``docs/api/ndimage-filters.md``).
The work runs on ``n_threads`` threads with the GIL released; the result does not depend on
the thread count.
"""

from __future__ import annotations

import numbers
import operator
import warnings
from collections.abc import Iterable
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "binary_closing",
    "correlate1d",
    "gaussian_filter",
    "gaussian_filter1d",
    "binary_dilation",
    "binary_erosion",
    "binary_fill_holes",
    "binary_hit_or_miss",
    "binary_opening",
    "binary_propagation",
    "center_of_mass",
    "generate_binary_structure",
    "grey_closing",
    "grey_dilation",
    "grey_erosion",
    "grey_opening",
    "iterate_structure",
    "label",
    "maximum_filter",
    "maximum_filter1d",
    "median_filter",
    "minimum_filter",
    "minimum_filter1d",
    "percentile_filter",
    "rank_filter",
    "sum",
    "sum_labels",
    "watershed_ift",
]

_FILTER_MODES = (
    "nearest",
    "wrap",
    "reflect",
    "grid-mirror",
    "mirror",
    "constant",
    "grid-wrap",
    "grid-constant",
)

_TYPES = (
    np.float32,
    np.float64,
    np.uint8,
    np.int8,
    np.uint16,
    np.int16,
    np.uint32,
    np.int32,
    np.uint64,
    np.int64,
)


#: The newest numpy release (the order of ``center_of_mass``'s sums by default).
NUMPY_NEWEST = "2.5.4"

# --- _ni_support ---------------------------------------------------------------------------


def _mode(mode: str) -> str:
    if mode not in _FILTER_MODES:
        raise RuntimeError("boundary mode not supported")
    return mode


def _normalize_sequence(input: Any, rank: int) -> list:
    is_str = isinstance(input, str)
    if not is_str and np.iterable(input):
        normalized = list(input)
        if len(normalized) != rank:
            raise RuntimeError("sequence argument must have length equal to input rank")
    else:
        normalized = [input] * rank
    return normalized


def _check_axes(axes: Any, ndim: int) -> tuple[int, ...]:
    if axes is None:
        return tuple(range(ndim))
    elif np.isscalar(axes):
        axes = (operator.index(axes),)
    elif isinstance(axes, Iterable):
        for ax in axes:
            axes = tuple(operator.index(ax) for ax in axes)
            if ax < -ndim or ax > ndim - 1:
                raise ValueError(f"specified axis: {ax} is out of range")
        axes = tuple(ax % ndim if ax < 0 else ax for ax in axes)
    else:
        raise ValueError("axes must be an integer, iterable of integers, or None")
    if len(tuple(set(axes))) != len(axes):
        raise ValueError("axes must be unique")
    return axes


def _expand_origin(ndim_image: int, axes: tuple[int, ...], origin: Any) -> list:
    num_axes = len(axes)
    origins = _normalize_sequence(origin, num_axes)
    if num_axes < ndim_image:
        origins_temp = [0] * ndim_image
        for o, ax in zip(origins, axes, strict=False):
            origins_temp[ax] = o
        origins = origins_temp
    return origins


def _expand_footprint(
    ndim_image: int, axes: tuple[int, ...], footprint: np.ndarray, name: str = "footprint"
) -> np.ndarray:
    num_axes = len(axes)
    if num_axes < ndim_image:
        if footprint.ndim != num_axes:
            raise RuntimeError(f"{name}.ndim ({footprint.ndim}) must match len(axes) ({num_axes})")
        footprint = np.expand_dims(
            footprint, tuple(ax for ax in range(ndim_image) if ax not in axes)
        )
    return footprint


def _expand_mode(ndim_image: int, axes: tuple[int, ...], mode: Any) -> Any:
    num_axes = len(axes)
    if not isinstance(mode, str) and isinstance(mode, Iterable):
        modes = _normalize_sequence(mode, num_axes)
        modes_temp = ["constant"] * ndim_image
        for m, ax in zip(modes, axes, strict=False):
            modes_temp[ax] = m
        mode = modes_temp
    return mode


def _input(input: Any) -> tuple[np.ndarray, np.dtype]:
    """The input as an array larmorx computes on (bool as uint8) and its own dtype."""
    a = np.asarray(input)
    if np.iscomplexobj(a):
        raise TypeError("Complex type not supported")
    dtype = a.dtype
    if a.dtype == np.bool_:
        return a.view(np.uint8), dtype
    if a.dtype.type not in _TYPES:
        raise RuntimeError("array type not supported")
    if not a.dtype.isnative:
        a = a.astype(a.dtype.newbyteorder("="))
    return a, dtype


def _out_dtype(output: Any, dtype: np.dtype, shape: tuple[int, ...]) -> np.dtype:
    """``_get_output``'s element type: the input's, the given dtype's, or the given array's."""
    if output is None:
        return dtype
    if isinstance(output, type | np.dtype | str):
        target = np.dtype(output)
    else:
        arr = np.asarray(output)
        if arr.shape != shape:
            raise RuntimeError("output shape not correct")
        target = arr.dtype
    if target.kind == "c" or target.type not in (*_TYPES, np.bool_):
        raise RuntimeError("array type not supported")
    return target


def _result(out: np.ndarray, target: np.dtype, output: Any) -> np.ndarray:
    """The result (computed in ``target`` by the Rust code) as SciPy returns it: a new array,
    or written into the ``output`` array."""
    res = out.view(np.bool_) if target == np.bool_ else out
    if output is None or isinstance(output, type | np.dtype | str):
        return res
    np.copyto(output, res, casting="unsafe")
    return output


# --- correlate1d and the Gaussian filters -----------------------------------------------------


def correlate1d(
    input: Any,
    weights: Any,
    axis: int = -1,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: int = 0,
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.correlate1d``: the correlation of the lines along ``axis`` with
    ``weights`` (``NI_Correlate1D``: a symmetric or anti-symmetric kernel adds the mirrored pairs
    first; C's cast to the output type)."""
    a = np.asarray(input)
    w = np.asarray(weights)
    if a.dtype.kind == "c" or w.dtype.kind == "c":
        raise NotImplementedError("complex inputs and weights are not supported")
    a, dtype = _input(a)
    target = _out_dtype(output, dtype, a.shape)
    w = np.asarray(w, dtype=np.float64)
    if w.ndim != 1 or w.shape[0] < 1:
        raise RuntimeError("no filter weights given")
    axis = operator.index(axis)
    if not -a.ndim <= axis < a.ndim:
        raise np.exceptions.AxisError(axis, a.ndim)
    axis = axis % a.ndim
    lw = len(w)
    if (origin < -(lw // 2)) or (origin > (lw - 1) // 2):
        raise ValueError(
            "Invalid origin; origin must satisfy -(len(weights) // 2) <= origin <= "
            "(len(weights)-1) // 2"
        )
    out = _core.ndimage_correlate1d(
        a,
        [float(v) for v in w],
        axis,
        _mode(mode),
        float(cval),
        int(origin),
        int(n_threads),
        target.name,
    )
    return _result(out, target, output)


def _gaussian_kernel1d(sigma: Any, order: int, radius: int, numpy_version: str) -> np.ndarray:
    """SciPy's ``_gaussian_kernel1d`` (order 0): ``exp(-0.5 / sigma**2 * x**2)`` for ``x`` from
    ``-radius`` to ``radius``, divided by its sum, with numpy's float64 ``exp`` (glibc's, the
    path without AVX-512) and its sum (``numpy_version``'s order)."""
    if order < 0:
        raise ValueError("order must be non-negative")
    if order != 0:
        raise NotImplementedError("Gaussian derivatives (order > 0) are not ported")
    sigma2 = sigma * sigma
    x = np.arange(-radius, radius + 1)
    arg = np.asarray(-0.5 / sigma2 * x**2)
    if arg.dtype != np.float64:
        raise NotImplementedError(f"a Gaussian kernel computed in {arg.dtype} is not ported")
    phi_x = _core.numpy_exp_f64(np.ascontiguousarray(arg))
    return phi_x / _core.numpy_add_reduce(phi_x, numpy_version)


def gaussian_filter1d(
    input: Any,
    sigma: float,
    axis: int = -1,
    order: int = 0,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    truncate: float = 4.0,
    *,
    radius: int | None = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.gaussian_filter1d`` (order 0): :func:`correlate1d` with the reversed
    Gaussian kernel of ``int(truncate * sigma + 0.5)`` (or ``radius``) elements each side."""
    sd = float(sigma)
    lw = int(truncate * sd + 0.5)
    if radius is not None:
        lw = radius
    if not isinstance(lw, numbers.Integral) or lw < 0:
        raise ValueError("Radius must be a nonnegative integer.")
    weights = _gaussian_kernel1d(sigma, order, lw, numpy_version)[::-1]
    return correlate1d(input, weights, axis, output, mode, cval, 0, n_threads=n_threads)


def gaussian_filter(
    input: Any,
    sigma: Any,
    order: Any = 0,
    output: Any = None,
    mode: Any = "reflect",
    cval: float = 0.0,
    truncate: float = 4.0,
    *,
    radius: Any = None,
    axes: Any = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.gaussian_filter`` (order 0): :func:`gaussian_filter1d` along each axis
    whose ``sigma`` exceeds 1e-15, in turn, each pass written in the output's type; a copy in
    that type when none does."""
    a = np.asarray(input)
    if a.dtype.kind == "c":
        raise NotImplementedError("complex inputs are not supported")
    target = _out_dtype(output, a.dtype, a.shape)
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    orders = _normalize_sequence(order, num_axes)
    sigmas = _normalize_sequence(sigma, num_axes)
    modes = _normalize_sequence(mode, num_axes)
    radiuses = _normalize_sequence(radius, num_axes)
    passes = [
        (axes[ii], sigmas[ii], orders[ii], modes[ii], radiuses[ii])
        for ii in range(num_axes)
        if sigmas[ii] > 1e-15
    ]
    if not passes:
        out = np.zeros(a.shape, dtype=target)
        out[...] = a[...]
        return _result(out, target, output)
    cur = a
    for ax, sg, od, md, rd in passes:
        cur = gaussian_filter1d(
            cur,
            sg,
            ax,
            od,
            target,
            md,
            cval,
            truncate,
            radius=rd,
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
    return _result(cur, target, output)


# --- the minimum, maximum and rank filters ---------------------------------------------------


def _min_or_max_filter1d(
    input, size, axis, output, mode, cval, origin, minimum, n_threads
) -> np.ndarray:
    a, dtype = _input(input)
    axis = operator.index(axis)
    if not -a.ndim <= axis < max(a.ndim, 1):
        raise np.exceptions.AxisError(axis, a.ndim)
    axis = axis % max(a.ndim, 1)
    if size < 1:
        raise RuntimeError("incorrect filter size")
    if (size // 2 + origin < 0) or (size // 2 + origin >= size):
        raise ValueError("invalid origin")
    target = _out_dtype(output, dtype, a.shape)
    out = _core.ndimage_min_or_max_filter1d(
        a,
        int(size),
        axis,
        _mode(mode),
        float(cval),
        int(origin),
        bool(minimum),
        int(n_threads),
        target.name,
    )
    return _result(out, target, output)


def minimum_filter1d(
    input: Any,
    size: int,
    axis: int = -1,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: int = 0,
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.minimum_filter1d``: the minimum over ``size`` elements along ``axis``."""
    return _min_or_max_filter1d(input, size, axis, output, mode, cval, origin, True, n_threads)


def maximum_filter1d(
    input: Any,
    size: int,
    axis: int = -1,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: int = 0,
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.maximum_filter1d``: the maximum over ``size`` elements along ``axis``."""
    return _min_or_max_filter1d(input, size, axis, output, mode, cval, origin, False, n_threads)


def _min_or_max_filter(
    input, size, footprint, structure, output, mode, cval, origin, minimum, axes, n_threads
) -> np.ndarray:
    if (size is not None) and (footprint is not None):
        warnings.warn("ignoring size because footprint is set", UserWarning, stacklevel=3)
    if structure is None:
        if footprint is None:
            if size is None:
                raise RuntimeError("no footprint provided")
            separable = True
        else:
            footprint = np.asarray(footprint, dtype=bool)
            if not footprint.any():
                raise ValueError("All-zero footprint is not supported.")
            if footprint.all():
                size = footprint.shape
                footprint = None
                separable = True
            else:
                separable = False
    else:
        structure = np.asarray(structure, dtype=np.float64)
        separable = False
        if footprint is None:
            footprint = np.ones(structure.shape, bool)
        else:
            footprint = np.asarray(footprint, dtype=bool)
    a, dtype = _input(input)
    target = _out_dtype(output, dtype, a.shape)
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    if separable:
        origins = _normalize_sequence(origin, num_axes)
        sizes = _normalize_sequence(size, num_axes)
        modes = _normalize_sequence(mode, num_axes)
        todo = [
            (axes[ii], sizes[ii], origins[ii], modes[ii])
            for ii in range(len(axes))
            if sizes[ii] > 1
        ]
        cur = a
        if len(todo) > 0:
            # the first axis filters into the output's type, the others the output in place
            for axis, sz, org, md in todo:
                sz = int(sz)
                if (sz // 2 + org < 0) or (sz // 2 + org >= sz):
                    raise ValueError("invalid origin")
                cur = _core.ndimage_min_or_max_filter1d(
                    cur,
                    sz,
                    int(axis),
                    _mode(md),
                    float(cval),
                    int(org),
                    bool(minimum),
                    int(n_threads),
                    target.name,
                )
        else:
            cur = np.ascontiguousarray(a).astype(np.uint8 if target == np.bool_ else target)
        return _result(cur, target, output)
    footprint = _expand_footprint(a.ndim, axes, footprint)
    origins = _expand_origin(a.ndim, axes, origin)
    fshape = [ii for ii in footprint.shape if ii > 0]
    if len(fshape) != a.ndim:
        raise RuntimeError(f"footprint.ndim ({footprint.ndim}) must match len(axes) ({len(axes)})")
    for org, lenf in zip(origins, fshape, strict=False):
        if (lenf // 2 + org < 0) or (lenf // 2 + org >= lenf):
            raise ValueError("invalid origin")
    footprint = np.ascontiguousarray(footprint)
    if structure is not None:
        if len(structure.shape) != num_axes:
            raise RuntimeError("structure array has incorrect shape")
        if num_axes != structure.ndim:
            structure = np.expand_dims(
                structure, tuple(ax for ax in range(structure.ndim) if ax not in axes)
            )
        structure = np.ascontiguousarray(structure)
        if structure.shape != footprint.shape:
            raise RuntimeError("structure array has incorrect shape")
    if not isinstance(mode, str) and isinstance(mode, Iterable):
        raise RuntimeError("A sequence of modes is not supported for non-separable footprints")
    out = _core.ndimage_min_or_max_filter(
        a,
        footprint,
        structure,
        _mode(mode),
        float(cval),
        [int(o) for o in origins],
        bool(minimum),
        int(n_threads),
        target.name,
    )
    return _result(out, target, output)


def minimum_filter(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: Any = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.minimum_filter``: the minimum over a footprint (or a box of ``size``)."""
    return _min_or_max_filter(
        input, size, footprint, None, output, mode, cval, origin, True, axes, n_threads
    )


def maximum_filter(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: Any = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.maximum_filter``: the maximum over a footprint (or a box of ``size``)."""
    return _min_or_max_filter(
        input, size, footprint, None, output, mode, cval, origin, False, axes, n_threads
    )


def _rank_filter(
    input,
    rank,
    size,
    footprint,
    output,
    mode,
    cval,
    origin,
    operation,
    axes,
    n_threads,
    upstream_defects="fix",
) -> np.ndarray:
    _defects.check(upstream_defects)
    if (size is not None) and (footprint is not None):
        warnings.warn("ignoring size because footprint is set", UserWarning, stacklevel=3)
    a, dtype = _input(input)
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    if footprint is None:
        if size is None:
            raise RuntimeError("no footprint or filter size provided")
        sizes = _normalize_sequence(size, num_axes)
        footprint = np.ones(sizes, dtype=bool)
    else:
        footprint = np.asarray(footprint, dtype=bool)
    footprint = _expand_footprint(a.ndim, axes, footprint)
    origins = _expand_origin(a.ndim, axes, origin)
    mode = _expand_mode(a.ndim, axes, mode)
    fshape = [ii for ii in footprint.shape if ii > 0]
    if len(fshape) != a.ndim:
        raise RuntimeError(f"footprint.ndim ({footprint.ndim}) must match len(axes) ({len(axes)})")
    # (SciPy's loop rebinds `origin`: the 1-D test below reads the last axis' origin)
    for origin, lenf in zip(origins, fshape, strict=False):
        if (lenf // 2 + origin < 0) or (lenf // 2 + origin >= lenf):
            raise ValueError("invalid origin")
    footprint = np.ascontiguousarray(footprint)
    filter_size = int(np.where(footprint, 1, 0).sum())
    if operation == "median":
        rank = filter_size // 2
    elif operation == "percentile":
        percentile = rank
        if percentile < 0.0:
            percentile += 100.0
        if percentile < 0 or percentile > 100:
            raise RuntimeError("invalid percentile")
        if percentile == 100.0:
            rank = filter_size - 1
        else:
            rank = int(float(filter_size) * percentile / 100.0)
    if rank < 0:
        rank += filter_size
    if rank < 0 or rank >= filter_size:
        raise RuntimeError("rank not within filter footprint size")
    if rank == 0:
        return minimum_filter(
            input, None, footprint, output, mode, cval, origins, axes=None, n_threads=n_threads
        )
    elif rank == filter_size - 1:
        return maximum_filter(
            input, None, footprint, output, mode, cval, origins, axes=None, n_threads=n_threads
        )
    if not isinstance(mode, str) and isinstance(mode, Iterable):
        raise RuntimeError("A sequence of modes is not supported by non-separable rank filters")
    target = _out_dtype(output, dtype, a.shape)
    mode = _mode(mode)
    # SciPy 1.18.1 sends 1-D inputs to its running-rank code unless the footprint is too large
    # for the input (gh-23293); it passes the footprint's size, its false elements included
    lim2 = a.size - ((footprint.size - 1) // 2 - origin)

    def nd():
        return _core.ndimage_rank_filter(
            a,
            int(rank),
            footprint,
            mode,
            float(cval),
            [int(o) for o in origins],
            int(n_threads),
            target.name,
        )

    if a.ndim == 1 and ((lim2 >= 0) or (a.size == 1)):
        x = a if dtype in (np.int64, np.float64, np.float32) else a.astype("int64")
        cv = x.dtype.type(cval)
        code = {"grid-wrap": "wrap", "grid-mirror": "reflect", "grid-constant": "constant"}.get(
            mode, mode
        )
        x_out = _core.ndimage_rank_filter_1d(
            x, int(rank), int(footprint.size), code, float(cv), int(origin)
        )
        res = np.empty(a.shape, dtype=target)
        np.copyto(res, x_out, casting="unsafe")
        res = res.view(np.uint8) if target == np.bool_ else res
        # SCI-09: SciPy's running rank takes the whole window, the footprint's false elements
        # included; corrected, a footprint with holes is filtered by the n-dimensional filter
        if not footprint.all():
            fixed = nd()
            if not np.array_equal(fixed, res, equal_nan=res.dtype.kind == "f"):
                _defects.warn(
                    [
                        (
                            "SCI-09",
                            "SciPy ndimage",
                            upstream_defects,
                            f"the 1-D {operation} filter's footprint has false elements, which "
                            "SciPy's running rank counts in its window",
                        )
                    ],
                    stacklevel=4,
                )
            if upstream_defects == "fix":
                return _result(fixed, target, output)
        return _result(res, target, output)
    return _result(nd(), target, output)


def rank_filter(
    input: Any,
    rank: int,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.rank_filter``: the ``rank``-th smallest value over a footprint.

    SCI-09 (rule A; ``upstream_defects="reproduce"`` gives SciPy's result), also of
    :func:`median_filter` and :func:`percentile_filter`: on a 1-D input SciPy's running rank
    takes the footprint's whole window, its false elements included (the rank is counted from
    the true ones), so a 1-D footprint with holes filters another neighbourhood; larmorx filters
    it with the n-dimensional filter, with a warning when that changes the result."""
    rank = operator.index(rank)
    return _rank_filter(
        input,
        rank,
        size,
        footprint,
        output,
        mode,
        cval,
        origin,
        "rank",
        axes,
        n_threads,
        upstream_defects,
    )


def median_filter(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.median_filter``: the median over a footprint (rank ``size // 2``)."""
    return _rank_filter(
        input,
        0,
        size,
        footprint,
        output,
        mode,
        cval,
        origin,
        "median",
        axes,
        n_threads,
        upstream_defects,
    )


def percentile_filter(
    input: Any,
    percentile: float,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.percentile_filter``: the ``percentile``-th value over a footprint."""
    return _rank_filter(
        input,
        percentile,
        size,
        footprint,
        output,
        mode,
        cval,
        origin,
        "percentile",
        axes,
        n_threads,
        upstream_defects,
    )


# --- grey morphology -------------------------------------------------------------------------


def grey_erosion(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    structure: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.grey_erosion``: the minimum over a footprint, minus a structure."""
    if size is None and footprint is None and structure is None:
        raise ValueError("size, footprint, or structure must be specified")
    return _min_or_max_filter(
        input, size, footprint, structure, output, mode, cval, origin, True, axes, n_threads
    )


def grey_dilation(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    structure: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.grey_dilation``: the maximum over the reversed footprint, plus the
    reversed structure."""
    if size is None and footprint is None and structure is None:
        raise ValueError("size, footprint, or structure must be specified")
    if structure is not None:
        structure = np.asarray(structure)
        structure = structure[tuple([slice(None, None, -1)] * structure.ndim)]
    if footprint is not None:
        footprint = np.asarray(footprint)
        footprint = footprint[tuple([slice(None, None, -1)] * footprint.ndim)]
    a = np.asarray(input)
    axes = _check_axes(axes, a.ndim)
    origin = _normalize_sequence(origin, len(axes))
    for ii in range(len(origin)):
        origin[ii] = -origin[ii]
        if footprint is not None:
            sz = footprint.shape[ii]
        elif structure is not None:
            sz = structure.shape[ii]
        elif np.isscalar(size):
            sz = size
        else:
            sz = size[ii]
        if not sz & 1:
            origin[ii] -= 1
    return _min_or_max_filter(
        a, size, footprint, structure, output, mode, cval, origin, False, axes, n_threads
    )


def grey_opening(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    structure: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.grey_opening``: a grey erosion, then a grey dilation."""
    if (size is not None) and (footprint is not None):
        warnings.warn("ignoring size because footprint is set", UserWarning, stacklevel=2)
    tmp = grey_erosion(
        input, size, footprint, structure, None, mode, cval, origin, axes=axes, n_threads=n_threads
    )
    return grey_dilation(
        tmp, size, footprint, structure, output, mode, cval, origin, axes=axes, n_threads=n_threads
    )


def grey_closing(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    structure: Any = None,
    output: Any = None,
    mode: str = "reflect",
    cval: float = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.grey_closing``: a grey dilation, then a grey erosion."""
    if (size is not None) and (footprint is not None):
        warnings.warn("ignoring size because footprint is set", UserWarning, stacklevel=2)
    tmp = grey_dilation(
        input, size, footprint, structure, None, mode, cval, origin, axes=axes, n_threads=n_threads
    )
    return grey_erosion(
        tmp, size, footprint, structure, output, mode, cval, origin, axes=axes, n_threads=n_threads
    )


# --- binary morphology -----------------------------------------------------------------------


def generate_binary_structure(rank: int, connectivity: int) -> np.ndarray:
    """``scipy.ndimage.generate_binary_structure``: the ``3^rank`` structure of the elements
    within ``connectivity`` city-block steps of the centre."""
    if connectivity < 1:
        connectivity = 1
    if rank < 1:
        return np.array(True, dtype=bool)
    output = np.fabs(np.indices([3] * rank) - 1)
    output = np.add.reduce(output, 0)
    return output <= connectivity


def _binary_erosion(
    input,
    structure,
    iterations,
    mask,
    output,
    border_value,
    origin,
    invert,
    brute_force,
    axes,
    n_threads,
    upstream_defects="fix",
) -> np.ndarray:
    _defects.check(upstream_defects)
    try:
        iterations = operator.index(iterations)
    except TypeError as e:
        raise TypeError("iterations parameter should be an integer") from e
    a = np.asarray(input)
    if not a.flags.c_contiguous and not a.flags.f_contiguous:
        a = np.ascontiguousarray(a)
    ndim = a.ndim
    if np.iscomplexobj(a):
        raise TypeError("Complex type not supported")
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    if structure is None:
        structure = generate_binary_structure(num_axes, 1)
    else:
        structure = np.asarray(structure, dtype=bool)
    if ndim > num_axes:
        structure = _expand_footprint(ndim, axes, structure, "structure")
    if structure.ndim != a.ndim:
        raise RuntimeError("structure and input must have same dimensionality")
    structure = np.ascontiguousarray(structure)
    if structure.size < 1:
        raise RuntimeError("structure must not be empty")
    if mask is not None:
        mask = np.asarray(mask)
        if mask.shape != a.shape:
            raise RuntimeError("mask and input must have equal sizes")
        if mask.dtype == np.bool_:
            mask = mask.view(np.uint8)
        if not mask.dtype.isnative:
            mask = mask.astype(mask.dtype.newbyteorder("="))
    origin = _normalize_sequence(origin, num_axes)
    origin = _expand_origin(ndim, axes, origin)
    if isinstance(output, np.ndarray) and np.iscomplexobj(output):
        raise TypeError("Complex output type not supported")
    data, _ = _input(a)

    def run(m):
        return _core.ndimage_binary_erosion(
            data,
            structure,
            [int(o) for o in origin],
            int(iterations),
            bool(border_value),
            bool(invert),
            bool(brute_force),
            m,
            int(n_threads),
        )

    # SCI-10: SciPy's passes after the first read the mask cast to int8 (0.5 and 256 are false
    # there, true in the first pass); corrected, every pass reads the mask's truth
    truth = None
    if mask is not None and iterations != 1 and mask.dtype not in (np.uint8, np.int8):
        t = mask != 0
        if not np.array_equal(t, mask.astype(np.int8) != 0):
            truth = t.view(np.uint8)
    out = run(mask if truth is None or upstream_defects == "reproduce" else truth)
    if truth is not None:
        other = run(truth if upstream_defects == "reproduce" else mask)
        if not np.array_equal(out, other):
            _defects.warn(
                [
                    (
                        "SCI-10",
                        "SciPy ndimage",
                        upstream_defects,
                        "the passes after the first read the mask cast to int8 (values such as "
                        "0.5 or 256 false), the first its truth",
                    )
                ],
                stacklevel=3,
            )
    res = out.view(np.bool_)
    if output is None:
        return res
    if isinstance(output, np.ndarray):
        if output.shape != a.shape:
            raise RuntimeError("output shape not correct")
        np.copyto(output, out, casting="unsafe")
        return output
    return out.astype(np.dtype(output))


def binary_erosion(
    input: Any,
    structure: Any = None,
    iterations: int = 1,
    mask: Any = None,
    output: Any = None,
    border_value: int = 0,
    origin: Any = 0,
    brute_force: bool = False,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.binary_erosion`` (SCI-10, ``upstream_defects``: :func:`binary_propagation`)."""
    return _binary_erosion(
        input,
        structure,
        iterations,
        mask,
        output,
        border_value,
        origin,
        0,
        brute_force,
        axes,
        n_threads,
        upstream_defects,
    )


def binary_dilation(
    input: Any,
    structure: Any = None,
    iterations: int = 1,
    mask: Any = None,
    output: Any = None,
    border_value: int = 0,
    origin: Any = 0,
    brute_force: bool = False,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.binary_dilation``: the erosion of the complement with the reversed
    structure."""
    a = np.asarray(input)
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    if structure is None:
        structure = generate_binary_structure(num_axes, 1)
    origin = _normalize_sequence(origin, num_axes)
    structure = np.asarray(structure)
    structure = structure[tuple([slice(None, None, -1)] * structure.ndim)]
    for ii in range(len(origin)):
        origin[ii] = -origin[ii]
        if not structure.shape[ii] & 1:
            origin[ii] -= 1
    return _binary_erosion(
        a,
        structure,
        iterations,
        mask,
        output,
        border_value,
        origin,
        1,
        brute_force,
        axes,
        n_threads,
        upstream_defects,
    )


def binary_opening(
    input: Any,
    structure: Any = None,
    iterations: int = 1,
    output: Any = None,
    origin: Any = 0,
    mask: Any = None,
    border_value: int = 0,
    brute_force: bool = False,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.binary_opening``: an erosion, then a dilation."""
    a = np.asarray(input)
    axes = _check_axes(axes, a.ndim)
    if structure is None:
        structure = generate_binary_structure(len(axes), 1)
    tmp = binary_erosion(
        a,
        structure,
        iterations,
        mask,
        None,
        border_value,
        origin,
        brute_force,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    return binary_dilation(
        tmp,
        structure,
        iterations,
        mask,
        output,
        border_value,
        origin,
        brute_force,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )


def binary_closing(
    input: Any,
    structure: Any = None,
    iterations: int = 1,
    output: Any = None,
    origin: Any = 0,
    mask: Any = None,
    border_value: int = 0,
    brute_force: bool = False,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.binary_closing``: a dilation, then an erosion."""
    a = np.asarray(input)
    axes = _check_axes(axes, a.ndim)
    if structure is None:
        structure = generate_binary_structure(len(axes), 1)
    tmp = binary_dilation(
        a,
        structure,
        iterations,
        mask,
        None,
        border_value,
        origin,
        brute_force,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    return binary_erosion(
        tmp,
        structure,
        iterations,
        mask,
        output,
        border_value,
        origin,
        brute_force,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )


def binary_hit_or_miss(
    input: Any,
    structure1: Any = None,
    structure2: Any = None,
    output: Any = None,
    origin1: Any = 0,
    origin2: Any = None,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.binary_hit_or_miss``."""
    a = np.asarray(input)
    axes = _check_axes(axes, a.ndim)
    num_axes = len(axes)
    if structure1 is None:
        structure1 = generate_binary_structure(num_axes, 1)
    else:
        structure1 = np.asarray(structure1)
    if structure2 is None:
        structure2 = np.logical_not(structure1)
    origin1 = _normalize_sequence(origin1, num_axes)
    origin2 = origin1 if origin2 is None else _normalize_sequence(origin2, num_axes)
    tmp1 = _binary_erosion(a, structure1, 1, None, None, 0, origin1, 0, False, axes, n_threads)
    inplace = isinstance(output, np.ndarray)
    result = _binary_erosion(a, structure2, 1, None, output, 0, origin2, 1, False, axes, n_threads)
    if inplace:
        np.logical_not(output, output)
        np.logical_and(tmp1, output, output)
        return output
    result = np.logical_not(result)
    return np.logical_and(tmp1, result)


def binary_propagation(
    input: Any,
    structure: Any = None,
    mask: Any = None,
    output: Any = None,
    border_value: int = 0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.binary_propagation``: a dilation repeated until nothing changes.

    SCI-10 (rule A; ``upstream_defects="reproduce"`` gives SciPy's result): with a ``mask`` of a
    type other than bool and 8-bit integers and more than one pass (``iterations`` other than 1,
    as here), SciPy's first pass reads the mask's truth and the later ones the mask cast to
    ``int8``, so a value such as 0.5 or 256 is true in the first pass and false afterwards;
    larmorx reads its truth in every pass, with a warning when that changes the result."""
    return binary_dilation(
        input,
        structure,
        -1,
        mask,
        output,
        border_value,
        origin,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )


def binary_fill_holes(
    input: Any,
    structure: Any = None,
    output: Any = None,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.binary_fill_holes``."""
    a = np.asarray(input)
    mask = np.logical_not(a)
    tmp = np.zeros(mask.shape, bool)
    inplace = isinstance(output, np.ndarray)
    if inplace:
        binary_dilation(tmp, structure, -1, mask, output, 1, origin, axes=axes, n_threads=n_threads)
        np.logical_not(output, output)
        return output
    out = binary_dilation(tmp, structure, -1, mask, None, 1, origin, axes=axes, n_threads=n_threads)
    return np.logical_not(out)


def iterate_structure(structure: Any, iterations: int, origin: Any = None) -> Any:
    """``scipy.ndimage.iterate_structure``: the structure dilated with itself ``iterations - 1``
    times (with the origin scaled, when one is given)."""
    structure = np.asarray(structure)
    if iterations < 2:
        return structure.copy()
    ni = iterations - 1
    shape = [ii + ni * (ii - 1) for ii in structure.shape]
    pos = [ni * (structure.shape[ii] // 2) for ii in range(len(shape))]
    slc = tuple(slice(pos[ii], pos[ii] + structure.shape[ii], None) for ii in range(len(shape)))
    out = np.zeros(shape, bool)
    out[slc] = structure != 0
    out = binary_dilation(out, structure, iterations=ni)
    if origin is None:
        return out
    origin = _normalize_sequence(origin, structure.ndim)
    origin = [iterations * o for o in origin]
    return out, origin


# --- measurements ----------------------------------------------------------------------------


def label(input: Any, structure: Any = None, output: Any = None) -> Any:
    """``scipy.ndimage.label``: the connected components of the non-zero elements, numbered
    from 1 by their first element in C order; ``(labels, number)``, or ``number`` when an
    ``output`` array is given."""
    a = np.asarray(input)
    if np.iscomplexobj(a):
        raise TypeError("Complex type not supported")
    if structure is None:
        structure = generate_binary_structure(a.ndim, 1)
    structure = np.asarray(structure, dtype=bool)
    if structure.ndim != a.ndim:
        raise RuntimeError("structure and input must have equal rank")
    for ii in structure.shape:
        if ii != 3:
            raise ValueError("structure dimensions must be equal to 3")
    need_64bits = a.size >= (2**31 - 2)
    caller_provided_output = isinstance(output, np.ndarray)
    if caller_provided_output and output.shape != a.shape:
        raise ValueError("output shape not correct")
    if a.ndim == 0 or a.size == 0:
        maxlabel = (1 if (a != 0) else 0) if a.ndim == 0 else 0
        if caller_provided_output:
            output[...] = maxlabel
            return maxlabel
        out = np.empty(a.shape, np.intp if need_64bits else np.int32 if output is None else output)
        out[...] = maxlabel
        return out, maxlabel
    if not np.all(structure == structure[(np.s_[::-1],) * structure.ndim]):
        raise AssertionError("Structuring element is not symmetric")
    data, _ = _input(a)
    labels, n = _core.ndimage_label(data, np.ascontiguousarray(structure))
    if caller_provided_output:
        output[...] = labels
        if not np.all(output == labels):
            raise RuntimeError("insufficient bit-depth in requested output type")
        return n
    if output is not None:
        res = labels.astype(output)
        if not np.all(res == labels):
            raise RuntimeError("insufficient bit-depth in requested output type")
        return res, n
    return labels, n


def sum_labels(input: Any, labels: Any = None, index: Any = None) -> Any:
    """``scipy.ndimage.sum_labels``: the sum of the input over each label of ``index``; over
    the positive labels (``index`` None) or all of it (``labels`` None)."""
    a = np.asarray(input)
    if labels is None:
        return a.sum()
    lab = np.asarray(labels)
    a, lab = np.broadcast_arrays(a, lab)
    if index is None:
        return a[lab > 0].sum()
    if np.isscalar(index):
        return a[lab == index].sum()
    if a.dtype.kind not in "biuf" or lab.dtype.kind not in "biu":
        # SciPy's unique path for labels of other types: the same sums by value
        uniq, inv = np.unique(lab, return_inverse=True)
        sums = np.bincount(inv.reshape(-1), weights=a.ravel())
        idxs = np.searchsorted(uniq, index)
        idxs[idxs >= uniq.size] = 0
        found = uniq[idxs] == index
        out = sums[idxs]
        out[~found] = 0
        return out
    data, _ = _input(np.ascontiguousarray(a))
    lab = np.ascontiguousarray(lab)
    if lab.dtype == np.bool_:
        lab = lab.view(np.uint8)
    idx = np.asanyarray(index)
    flat = [int(v) for v in idx.ravel()]
    res = np.asarray(_core.ndimage_sum_labels(data, lab, flat), dtype=np.float64)
    return res.reshape(idx.shape)


def _sum_in_order(a: np.ndarray, numpy_version: str) -> Any:
    """``a.sum()`` as numpy ``numpy_version`` adds it: float32 and float64 arrays contiguous in
    C or Fortran order (their elements in memory order) by larmorx's port of numpy's sums
    (pairwise from numpy 2.3, chunks of 8192 before); others by the installed numpy."""
    if (
        a.dtype in (np.float32, np.float64)
        and a.dtype.isnative
        and (a.flags.c_contiguous or a.flags.f_contiguous)
    ):
        flat = a.ravel(order="K")
        return a.dtype.type(_core.numpy_add_reduce(flat, numpy_version))
    return a.sum()


def _sum_labels_in_order(a: np.ndarray, labels: Any, index: Any, numpy_version: str) -> Any:
    """``sum_labels(a, labels, index)`` with the whole-array and selected sums in numpy
    ``numpy_version``'s order (``_stats``' ``single_group``)."""
    if labels is None:
        return _sum_in_order(a, numpy_version)
    lab = np.asarray(labels)
    a, lab = np.broadcast_arrays(a, lab)
    if index is None:
        return _sum_in_order(a[lab > 0], numpy_version)
    if np.isscalar(index):
        return _sum_in_order(a[lab == index], numpy_version)
    return sum_labels(a, lab, index)


def center_of_mass(
    input: Any,
    labels: Any = None,
    index: Any = None,
    *,
    numpy_version: str = NUMPY_NEWEST,
) -> Any:
    """``scipy.ndimage.center_of_mass``: the centre of mass of the values (of each label of
    ``index``, of the positive labels, or of the whole array), a tuple per centre; each
    coordinate is ``sum_labels(input * grid) / sum_labels(input)`` in float64.

    numpy's sums depend on its release (pairwise from numpy 2.3, chunks of 8192 before): the
    whole-array and selected sums of float32 and float64 values follow ``numpy_version`` (the
    newest release by default) for arrays contiguous in C or Fortran order, the installed
    numpy's for other layouts and types; the sums per label of a sequence ``index`` are
    numpy's ``bincount`` (in C order, the same in every release)."""
    input = np.asarray(input)
    normalizer = _sum_labels_in_order(input, labels, index, numpy_version)
    grids = np.ogrid[[slice(0, i) for i in input.shape]]
    results = [
        _sum_labels_in_order(input * grids[dir].astype(float), labels, index, numpy_version)
        / normalizer
        for dir in range(input.ndim)
    ]
    if np.isscalar(results[0]):
        return tuple(results)
    return [tuple(v) for v in np.array(results).T]


#: The register's entries for ``watershed_ift``'s events (provisional IDs: the part registers
#: them, ``docs/findings/scipy-ndimage-filters.md``, candidates 8 to 11).
_WATERSHED_DEFECTS = (
    ("SCI-13", "SciPy links elements across the array's edges (it checks only the C-order index)"),
    ("SCI-14", "SciPy left an element alone in its queue there when its cost dropped"),
    ("SCI-15", "SciPy reads markers beyond 32 bits as C ints"),
    (
        "SCI-16",
        "SciPy's int byte offsets overflow above 2 GiB (it reads and writes outside its "
        "arrays; never reproduced)",
    ),
)


def watershed_ift(
    input: Any,
    markers: Any,
    structure: Any = None,
    output: Any = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """``scipy.ndimage.watershed_ift``: the watershed of a ``uint8`` or ``uint16`` image from
    integer ``markers`` (positive: objects; negative: background, processed after them) by the
    image foresting transform, with the ``structure``'s connectivity (``3`` along each axis; the
    faces by default). The labels have the markers' type, or the integer ``output`` array's
    (a non-array ``output`` is ignored, as SciPy ignores it).

    Corrected by default (``upstream_defects="reproduce"`` gives SciPy's labels, bit for bit,
    in 1.15.3 and 1.18.1): SciPy's neighbours across the array's edges (the last element of a
    row neighbours the first of the next; an axis of length 1 has neighbours along it), its
    queue that keeps an element alone in it when the element's cost drops, and its markers
    read as 32-bit ``int``; an ``UpstreamDefectWarning`` says which, when the labels differ.
    Arrays over 2 GiB are labelled with 64-bit offsets in both modes (SciPy's ``int`` offsets
    read and write outside its arrays there)."""
    _defects.check(upstream_defects)
    input = np.asarray(input)
    if input.dtype.type not in [np.uint8, np.uint16]:
        raise TypeError("only 8 and 16 unsigned inputs are supported")

    if structure is None:
        structure = generate_binary_structure(input.ndim, 1)
    structure = np.asarray(structure, dtype=bool)
    if structure.ndim != input.ndim:
        raise RuntimeError("structure and input must have equal rank")
    for ii in structure.shape:
        if ii != 3:
            raise RuntimeError("structure dimensions must be equal to 3")

    if not structure.flags.contiguous:
        structure = structure.copy()
    markers = np.asarray(markers)
    if input.shape != markers.shape:
        raise RuntimeError("input and markers must have equal shape")

    integral_types = [np.int8, np.int16, np.int32, np.int64, np.intc, np.intp]

    if markers.dtype.type not in integral_types:
        raise RuntimeError("marker should be of integer type")

    given = isinstance(output, np.ndarray)
    if given:
        if output.dtype.type not in integral_types:
            raise RuntimeError("output should be of integer type")
        if output.shape != input.shape:
            raise RuntimeError("output shape not correct")
        out_dtype = output.dtype
    else:
        out_dtype = markers.dtype
    if input.ndim > 7:
        raise RuntimeError("too many dimensions")
    if not input.dtype.isnative:
        input = input.astype(input.dtype.newbyteorder("="))
    if not markers.dtype.isnative:
        markers = markers.astype(markers.dtype.newbyteorder("="))
    labels, events, differ = _core.ndimage_watershed_ift(
        input, markers, structure, 8 * out_dtype.itemsize, upstream_defects == "reproduce"
    )
    if differ:
        _defects.warn(
            [
                (
                    entry,
                    "SciPy watershed_ift",
                    upstream_defects,
                    f"the labels of {differ} of {labels.size} elements differ ({what})",
                )
                for (entry, what), happened in zip(_WATERSHED_DEFECTS, events, strict=True)
                if happened
            ],
            stacklevel=2,
        )
    elif events[3]:
        entry, what = _WATERSHED_DEFECTS[3]
        _defects.warn([(entry, "SciPy watershed_ift", upstream_defects, what)], stacklevel=2)
    if given:
        output[...] = labels
        return output
    if out_dtype == labels.dtype:
        return labels
    return labels.astype(out_dtype)


def sum(input: Any, labels: Any = None, index: Any = None) -> Any:
    """``scipy.ndimage.sum``: an alias of :func:`sum_labels`."""
    return sum_labels(input, labels, index)

# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""SciPy-compatible filters, morphology and labelling for ``lx.ndimage`` (imported there).

A replica of ``scipy.ndimage``'s ``correlate1d``, ``gaussian_filter1d`` and ``gaussian_filter``
(complex inputs and weights through their parts, as SciPy's; the Gaussian kernel with numpy's
float64 ``exp`` on its path without AVX-512, glibc's, and numpy's sum; its derivatives' polynomial
with numpy's ``dot``, OpenBLAS's Haswell ``dgemv``), minimum, maximum, rank, median and percentile
filters, grey
and binary morphology, ``label``, ``sum_labels``, ``center_of_mass``, ``watershed_ift``,
``generate_binary_structure`` and ``iterate_structure`` (SciPy 1.18.1; the C and Cython code
of these functions is the same in SciPy 1.15.3), computed in Rust (``larmorx_ndimage``) with SciPy's results bit for bit: the
boundary modes, the order a footprint is visited in, the comparisons (NaNs and signed zeros
travel as in SciPy), the additions of a non-flat structuring element in the input's type,
SciPy's quickselect for the rank filters and C's casts. The argument handling and messages
follow ``scipy/ndimage/_filters.py``, ``_morphology.py``, ``_measurements.py`` and
``_ni_support.py`` (BSD-3-Clause; SciPy's notice is in larmorx's NOTICE file).

Differences from SciPy: ``output`` may be a dtype or an array
(the result is computed in the input's type and cast, which is SciPy's value except for
integers beyond 2^53 cast to floats); the n-dimensional ``reflect`` boundary maps a coordinate
two or more whole periods before an axis to index 0 where SciPy reads before the array (a
footprint more than eight times the axis); a binary erosion or dilation iterated until nothing
changes that alternates for ever raises instead of looping; ``watershed_ift`` corrects SciPy's
neighbours across the array's edges, its queue that keeps an element alone in it and its 32-bit
markers by default; the 1-D rank filters correct SciPy's writes of the input's type into an
output of another type (SCI-21), its refusal of big-endian floats and ``longlong`` (SCI-22) and
its write-back warning (SCI-23) by default (``upstream_defects``; ``docs/api/ndimage-filters.md``);
a ``numpy.longdouble`` sigma's Gaussian kernel is computed from the sigma as float64. Binary
morphology, ``label``, the sums and ``watershed_ift`` take their arguments as SciPy (relay 22) and
correct by default an iterated erosion into a float output that erodes outside its mask (SCI-25),
the erosion's write-back warning (SCI-26), ``label``'s refusal of big-endian arrays (SCI-27), its
writes into a read-only output (SCI-28) and its label bytes in a bool output (SCI-29), and the
sums' failures on a 0-d index array or an empty input (SCI-30).
The work runs on ``n_threads`` threads with the GIL released; the result does not depend on
the thread count.
"""

from __future__ import annotations

import math
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


class _OutputOf:
    """An output array of ``dtype`` and the input's shape, not yet allocated: SciPy's
    ``gaussian_filter`` hands one output array to every pass; larmorx gives each pass a new one
    (the same values: the lines of a pass are disjoint)."""

    def __init__(self, dtype: Any) -> None:
        self.dtype = np.dtype(dtype)


def _get_output(
    output: Any, input: np.ndarray, complex_output: bool = False
) -> tuple[np.dtype, np.ndarray | None]:
    """SciPy's ``_ni_support._get_output``: the output's element type and the array the result
    is written into (``None``: a new array of that type)."""
    shape = input.shape
    if output is None:
        if not complex_output:
            return np.dtype(input.dtype.name), None
        return np.promote_types(input.dtype, np.complex64), None
    if isinstance(output, _OutputOf):
        if complex_output and output.dtype.kind != "c":
            raise RuntimeError("output must have complex dtype")
        return output.dtype, None
    if isinstance(output, type | np.dtype):
        if complex_output and np.dtype(output).kind != "c":
            warnings.warn("promoting specified output dtype to complex", stacklevel=3)
            output = np.promote_types(output, np.complex64)
        return np.zeros((), dtype=output).dtype, None
    if isinstance(output, str):
        dt = np.dtype(output)
        if complex_output and dt.kind != "c":
            raise RuntimeError("output must have complex dtype")
        elif not issubclass(dt.type, np.number):
            raise RuntimeError("output must have numeric dtype")
        return dt, None
    arr = np.asarray(output)
    if arr.shape != shape:
        raise RuntimeError("output shape not correct")
    elif complex_output and arr.dtype.kind != "c":
        raise RuntimeError("output must have complex dtype")
    return arr.dtype, arr


def _c_double(v: Any) -> float:
    """``PyArg_ParseTuple``'s ``"d"``: CPython's ``PyFloat_AsDouble`` (the C slots ``nb_float``,
    then ``nb_index``, else ``TypeError("must be real number, not ...")``), which ``math.ldexp``
    applies to its first argument; ``ldexp(x, 0)`` is ``x`` exactly."""
    return math.ldexp(v, 0)


def _normalize_axis_index(axis: Any, ndim: int) -> Any:
    """SciPy's ``normalize_axis_index`` (``scipy/_lib/_util.py``)."""
    if axis < -ndim or axis >= ndim:
        msg = f"axis {axis} is out of bounds for array of dimension {ndim}"
        raise np.exceptions.AxisError(msg)
    if axis < 0:
        axis = axis + ndim
    return axis


def _line_type(dtype: np.dtype) -> np.dtype:
    """``NI_InitLineBuffer``'s type check (the 1-D filters' input and output): the canonical
    type larmorx computes in (bool as uint8), or SciPy's ``RuntimeError`` naming the dtype."""
    if dtype == np.bool_:
        return np.dtype(np.uint8)
    if (dtype.kind in "iu" and dtype.itemsize in (1, 2, 4, 8)) or (
        dtype.kind == "f" and dtype.itemsize in (4, 8)
    ):
        return np.dtype(f"{dtype.kind}{dtype.itemsize}")
    raise RuntimeError(f"array type {dtype.newbyteorder('=')!r} not supported")


def _write(out: np.ndarray, target: np.dtype, out_arr: np.ndarray | None) -> np.ndarray:
    """The result computed in ``target`` as SciPy returns it: a new array, or written into the
    output array."""
    res = out.view(np.bool_) if target == np.bool_ else out
    if out_arr is None:
        return res if res.dtype == target else res.astype(target)
    np.copyto(out_arr, res, casting="unsafe")
    return out_arr


def _complex_via_real_components(
    func: Any, input: np.ndarray, weights: np.ndarray, output: np.ndarray, cval: Any, **kwargs: Any
) -> np.ndarray:
    """SciPy's ``_complex_via_real_components``: a complex correlation as real correlations of
    the parts, combined with numpy's subtraction and addition in the output's part type."""
    complex_input = input.dtype.kind == "c"
    complex_weights = weights.dtype.kind == "c"
    if complex_input and complex_weights:
        func(input.real, weights.real, output=output.real, cval=np.real(cval), **kwargs)
        output.real -= func(input.imag, weights.imag, output=None, cval=np.imag(cval), **kwargs)
        func(input.real, weights.imag, output=output.imag, cval=np.real(cval), **kwargs)
        output.imag += func(input.imag, weights.real, output=None, cval=np.imag(cval), **kwargs)
    elif complex_input:
        func(input.real, weights, output=output.real, cval=np.real(cval), **kwargs)
        func(input.imag, weights, output=output.imag, cval=np.imag(cval), **kwargs)
    else:
        if np.iscomplexobj(cval):
            raise ValueError("Cannot provide a complex-valued cval when the input is real.")
        func(input, weights.real, output=output.real, cval=cval, **kwargs)
        func(input, weights.imag, output=output.imag, cval=cval, **kwargs)
    return output


def correlate1d(
    input: Any,
    weights: Any,
    axis: int = -1,
    output: Any = None,
    mode: str = "reflect",
    cval: Any = 0.0,
    origin: int = 0,
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.correlate1d``: the correlation of the lines along ``axis`` with
    ``weights`` (``NI_Correlate1D``: a symmetric or anti-symmetric kernel adds the mirrored pairs
    first; C's cast to the output type). A complex input or complex weights (conjugated, as
    SciPy's) are correlated through their real and imaginary parts."""
    input = np.asarray(input)
    weights = np.asarray(weights)
    complex_input = input.dtype.kind == "c"
    complex_weights = weights.dtype.kind == "c"
    if complex_input or complex_weights:
        if complex_weights:
            weights = weights.conj()
            weights = weights.astype(np.complex128, copy=False)
        kwargs = dict(axis=axis, mode=mode, origin=origin, n_threads=n_threads)
        target, out_arr = _get_output(output, input, complex_output=True)
        if out_arr is None:
            out_arr = np.zeros(input.shape, dtype=target)
        return _complex_via_real_components(correlate1d, input, weights, out_arr, cval, **kwargs)

    target, out_arr = _get_output(output, input)
    weights = np.asarray(weights, dtype=np.float64)
    if weights.ndim != 1 or weights.shape[0] < 1:
        raise RuntimeError("no filter weights given")
    axis = _normalize_axis_index(axis, input.ndim)
    lw = len(weights)
    if (origin < -(lw // 2)) or (origin > (lw - 1) // 2):
        raise ValueError(
            "Invalid origin; origin must satisfy -(len(weights) // 2) <= origin <= "
            "(len(weights)-1) // 2"
        )
    mode = _mode(mode)
    # _nd_image.correlate1d's arguments ("O&O&iO&idn"), in order
    axis = operator.index(axis)
    if out_arr is not None and not out_arr.flags.writeable:
        raise ValueError("output array is read-only.")
    cval = _c_double(cval)
    origin = operator.index(origin)
    # NI_Correlate1D's line buffers: the input's type, then the output's
    in_type = _line_type(input.dtype)
    out_type = _line_type(target)
    a = input
    if not a.dtype.isnative:
        a = a.astype(a.dtype.newbyteorder("="))
    if a.dtype != in_type:
        a = a.view(in_type)
    out = _core.ndimage_correlate1d(
        a,
        [float(v) for v in weights],
        axis,
        mode,
        cval,
        origin,
        int(n_threads),
        "bool" if target == np.bool_ else out_type.name,
    )
    return _write(out, target, out_arr)


def _dot(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """``a.dot(b)`` of a 2-D and a 1-D array as numpy computes it in float64
    (``_core.numpy_matvec_f64``: OpenBLAS's Haswell ``dgemv_t``)."""
    return _core.numpy_matvec_f64(
        np.ascontiguousarray(a, dtype=np.float64), np.ascontiguousarray(b, dtype=np.float64)
    )


def _longdouble_kind() -> str:
    """numpy's long double on this platform, in words (its significand's width)."""
    nmant = np.finfo(np.longdouble).nmant
    names = {63: "x86-64's 80-bit extended precision", 112: "IEEE quadruple precision"}
    return names.get(nmant, f"a {nmant + 1}-bit significand")


def _gaussian_kernel1d(sigma: Any, order: Any, radius: int, numpy_version: str) -> np.ndarray:
    """SciPy's ``_gaussian_kernel1d``: ``exp(-0.5 / sigma**2 * x**2)`` for ``x`` from
    ``-radius`` to ``radius``, divided by its sum, with numpy's float64 ``exp`` (glibc's, the
    path without AVX-512) and its sum (``numpy_version``'s order); for ``order > 0`` times the
    derivative's polynomial, its coefficients ``q`` from ``order`` products with the matrix
    ``D + P`` (``q' + q·p'``) and its values ``x**k · q``, numpy's ``dot`` (OpenBLAS's ``dgemv``)
    both.

    A ``numpy.longdouble`` sigma is taken as float64: SciPy computes that kernel in numpy's long
    double, a different type per platform (x86-64's 80-bit extended precision on Linux and
    macOS Intel, IEEE quadruple precision on Linux aarch64, float64 itself on Windows and Apple
    silicon), so its weights differ between platforms in their last bits; larmorx gives the
    float64 kernel everywhere, with a warning where the long double is wider than float64."""
    if order < 0:
        raise ValueError("order must be non-negative")
    if np.asarray(sigma).dtype.char == "g":
        if np.dtype(np.longdouble).itemsize > 8:
            warnings.warn(
                f"a {np.dtype(np.longdouble).name} sigma ({_longdouble_kind()} on this "
                "platform): larmorx computes the Gaussian kernel from the sigma as float64, "
                "where SciPy computes it in numpy's long double (docs/api/ndimage-filters.md)",
                UserWarning,
                stacklevel=4,
            )
        sigma = float(sigma)
    exponent_range = np.arange(order + 1)
    sigma2 = sigma * sigma
    x = np.arange(-radius, radius + 1)
    arg = np.asarray(-0.5 / sigma2 * x**2)
    if arg.dtype != np.float64:
        raise NotImplementedError(f"a Gaussian kernel computed in {arg.dtype} is not ported")
    phi_x = _core.numpy_exp_f64(np.ascontiguousarray(arg))
    phi_x = phi_x / _core.numpy_add_reduce(phi_x, numpy_version)
    if order == 0:
        return phi_x
    q = np.zeros(order + 1)
    q[0] = 1
    D = np.diag(exponent_range[1:], 1)
    P = np.diag(np.ones(order) / -sigma2, -1)
    Q_deriv = D + P
    if Q_deriv.dtype != np.float64:
        raise NotImplementedError(
            f"a Gaussian derivative computed in {Q_deriv.dtype} is not ported"
        )
    for _ in range(order):
        q = _dot(Q_deriv, q)
    q = _dot(x[:, None] ** exponent_range, q)
    return q * phi_x


def gaussian_filter1d(
    input: Any,
    sigma: float,
    axis: int = -1,
    order: int = 0,
    output: Any = None,
    mode: str = "reflect",
    cval: Any = 0.0,
    truncate: float = 4.0,
    *,
    radius: int | None = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.gaussian_filter1d``: :func:`correlate1d` with the reversed Gaussian kernel
    (or its ``order``-th derivative) of ``int(truncate * sigma + 0.5)`` (or ``radius``) elements
    each side."""
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
    cval: Any = 0.0,
    truncate: float = 4.0,
    *,
    radius: Any = None,
    axes: Any = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.gaussian_filter``: :func:`gaussian_filter1d` (each axis's ``order``)
    along each axis whose ``sigma`` exceeds 1e-15, in turn, each pass written in the output's
    type; a copy in that type when none does."""
    input = np.asarray(input)
    target, out_arr = _get_output(output, input)
    axes = _check_axes(axes, input.ndim)
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
        out = np.zeros(input.shape, dtype=target) if out_arr is None else out_arr
        out[...] = input[...]
        return out
    cur = input
    for ax, sg, od, md, rd in passes:
        cur = gaussian_filter1d(
            cur,
            sg,
            ax,
            od,
            _OutputOf(target),
            md,
            cval,
            truncate,
            radius=rd,
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
    if out_arr is None:
        return cur
    np.copyto(out_arr, cur, casting="unsafe")
    return out_arr


# --- the minimum, maximum and rank filters ---------------------------------------------------


def _intp(v: Any) -> int:
    """numpy's ``PyArray_PyIntAsIntp``, which ``PyArray_IntpConverter`` applies to each origin
    of the n-dimensional filters: a bool (Python's or numpy's) is refused with "an integer is
    required", anything else is converted by ``__index__``."""
    if isinstance(v, bool | np.bool_):
        raise TypeError("an integer is required")
    return operator.index(v)


def _nd_type(dtype: np.dtype) -> np.dtype:
    """The n-dimensional filters' type switch (``NI_MinOrMaxFilter``, ``NI_RankFilter``): the
    canonical type larmorx computes in (bool as uint8), or SciPy's ``RuntimeError``."""
    if dtype == np.bool_:
        return np.dtype(np.uint8)
    if (dtype.kind in "iu" and dtype.itemsize in (1, 2, 4, 8)) or (
        dtype.kind == "f" and dtype.itemsize in (4, 8)
    ):
        return np.dtype(f"{dtype.kind}{dtype.itemsize}")
    raise RuntimeError("array type not supported")


def _native(a: np.ndarray, canonical: np.dtype) -> np.ndarray:
    """``a`` in native byte order, viewed as ``canonical`` (bool as uint8, ``longlong`` as int64)."""
    if not a.dtype.isnative:
        a = a.astype(a.dtype.newbyteorder("="))
    if a.dtype != canonical:
        a = a.view(canonical)
    return a


def _c_output(out_arr: np.ndarray | None) -> None:
    """``NI_ObjectToOutputArray``: a read-only output array is refused."""
    if out_arr is not None and not out_arr.flags.writeable:
        raise ValueError("output array is read-only.")


def _min_or_max_1d_c(
    input, size, axis, target, out_arr, check_out, mode, cval, origin, minimum, n_threads
) -> np.ndarray:
    """``_nd_image.min_or_max_filter1d``: its arguments (``"O&niO&idni"``: the size and the
    origin by ``__index__``, the axis likewise, the output's write access, ``cval`` as a C
    double) and ``NI_InitLineBuffer``'s types (the input's, then the output's), then the
    filter in Rust."""
    size = operator.index(size)
    axis = operator.index(axis)
    if check_out:
        _c_output(out_arr)
    cval = _c_double(cval)
    origin = operator.index(origin)
    in_type = _line_type(input.dtype)
    out_type = _line_type(target)
    return _core.ndimage_min_or_max_filter1d(
        _native(input, in_type),
        size,
        axis,
        mode,
        cval,
        origin,
        bool(minimum),
        int(n_threads),
        "bool" if target == np.bool_ else out_type.name,
    )


def _min_or_max_filter1d(
    input, size, axis, output, mode, cval, origin, minimum, n_threads
) -> np.ndarray:
    """SciPy's ``minimum_filter1d`` and ``maximum_filter1d``, check for check."""
    input = np.asarray(input)
    if np.iscomplexobj(input):
        raise TypeError("Complex type not supported")
    axis = _normalize_axis_index(axis, input.ndim)
    if size < 1:
        raise RuntimeError("incorrect filter size")
    target, out_arr = _get_output(output, input)
    if (size // 2 + origin < 0) or (size // 2 + origin >= size):
        raise ValueError("invalid origin")
    mode = _mode(mode)
    out = _min_or_max_1d_c(
        input, size, axis, target, out_arr, True, mode, cval, origin, minimum, n_threads
    )
    return _write(out, target, out_arr)


def minimum_filter1d(
    input: Any,
    size: int,
    axis: int = -1,
    output: Any = None,
    mode: str = "reflect",
    cval: Any = 0.0,
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
    cval: Any = 0.0,
    origin: int = 0,
    *,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.maximum_filter1d``: the maximum over ``size`` elements along ``axis``."""
    return _min_or_max_filter1d(input, size, axis, output, mode, cval, origin, False, n_threads)


def _empty_result(input: np.ndarray, target: np.dtype, out_arr: np.ndarray | None) -> np.ndarray:
    """The n-dimensional filters of an empty input: SciPy's loop over its elements never reaches
    the type switch, so the output (zeros of its type, or the given array) is returned as is."""
    return np.zeros(input.shape, dtype=target) if out_arr is None else out_arr


def _min_or_max_filter(
    input, size, footprint, structure, output, mode, cval, origin, minimum, axes, n_threads
) -> np.ndarray:
    """SciPy's ``_min_or_max_filter`` (the minimum and maximum filters and grey morphology),
    check for check: a box (``size``, or a footprint of ones) as 1-D filters along each axis in
    turn, each into the output's type; any other footprint, or a structure, by
    ``NI_MinOrMaxFilter``."""
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
    input = np.asarray(input)
    if np.iscomplexobj(input):
        raise TypeError("Complex type not supported")
    target, out_arr = _get_output(output, input)
    # SciPy filters into a new array when the output shares memory with the input; larmorx's
    # filters always do, and only skip the output's write check (that new array is writable)
    check_out = not (out_arr is not None and np.may_share_memory(input, out_arr))
    axes = _check_axes(axes, input.ndim)
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
        if not todo:
            out = np.zeros(input.shape, dtype=target) if out_arr is None else out_arr
            out[...] = input[...]
            return out
        # each axis is minimum_filter1d's (or maximum_filter1d's) call into the output array:
        # the first from the input, the others from the output's values
        cur = input
        for k, (axis, sz, org, md) in enumerate(todo):
            sz = int(sz)
            axis = _normalize_axis_index(axis, cur.ndim)
            if (sz // 2 + org < 0) or (sz // 2 + org >= sz):
                raise ValueError("invalid origin")
            md = _mode(md)
            cur = _min_or_max_1d_c(
                cur if k == 0 else _write(cur, target, None),
                sz,
                axis,
                target,
                out_arr,
                check_out,
                md,
                cval,
                org,
                minimum,
                n_threads,
            )
        return _write(cur, target, out_arr)
    footprint = _expand_footprint(input.ndim, axes, footprint)
    origins = _expand_origin(input.ndim, axes, origin)
    fshape = [ii for ii in footprint.shape if ii > 0]
    if len(fshape) != input.ndim:
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
    mode = _mode(mode)
    # _nd_image.min_or_max_filter's arguments ("O&O&O&O&idO&i"), in order
    if check_out:
        _c_output(out_arr)
    cval = _c_double(cval)
    origins = [_intp(o) for o in origins]
    if input.size == 0:
        return _empty_result(input, target, out_arr)
    in_type = _nd_type(input.dtype)
    out_type = _nd_type(target)
    out = _core.ndimage_min_or_max_filter(
        _native(input, in_type),
        footprint,
        structure,
        mode,
        cval,
        origins,
        bool(minimum),
        int(n_threads),
        "bool" if target == np.bool_ else out_type.name,
    )
    return _write(out, target, out_arr)


def minimum_filter(
    input: Any,
    size: Any = None,
    footprint: Any = None,
    output: Any = None,
    mode: Any = "reflect",
    cval: Any = 0.0,
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
    cval: Any = 0.0,
    origin: Any = 0,
    *,
    axes: Any = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.maximum_filter``: the maximum over a footprint (or a box of ``size``)."""
    return _min_or_max_filter(
        input, size, footprint, None, output, mode, cval, origin, False, axes, n_threads
    )


#: numpy's warning when an array with a pending write-back copy is freed (SCI-23), per release.
_WRITEBACK_WARNING = (
    "WRITEBACKIFCOPY detected in {where}.  Required call to PyArray_ResolveWritebackIfCopy or "
    "PyArray_DiscardWritebackIfCopy is missing."
)


def _writeback_warning() -> None:
    """SCI-23 reproduced: numpy's ``RuntimeWarning`` for the write-back copy SciPy's 1-D rank
    filter never resolves ("array_dealloc" up to numpy 2.3, "clearing of array" since)."""
    major, minor = (int(p) for p in np.__version__.split(".")[:2])
    where = "array_dealloc" if (major, minor) <= (2, 3) else "clearing of array"
    warnings.warn(_WRITEBACK_WARNING.format(where=where), RuntimeWarning, stacklevel=5)


def _rank_1d(
    input: np.ndarray,
    rank: int,
    footprint: np.ndarray,
    target: np.dtype,
    out_arr: np.ndarray | None,
    check_out: bool,
    mode: str,
    cval: Any,
    origin: Any,
    operation: str,
    upstream_defects: str,
    nd: Any,
) -> np.ndarray:
    """SciPy 1.18.1's running rank of a 1-D input (``_rank_filter_1d``), its type dispatch and
    arguments (``"OiiOiOi"``) included, with SCI-09, SCI-21, SCI-22 and SCI-23."""
    notices: list[tuple[str, str, str, str]] = []
    dt = input.dtype
    # its own types are compared by equality: big-endian float32 and float64 are refused (SCI-22)
    direct = dt in (np.int64, np.float64, np.float32)
    if direct:
        x = input
    elif dt == np.float16:
        x = input.astype("float32")
    elif np.result_type(input, np.int64) == np.int64 or dt.kind in "biu":
        x = input.astype("int64")
    elif dt.kind == "f" and dt.itemsize in (4, 8) and upstream_defects == "fix":
        x = input.astype(dt.newbyteorder("="))
        notices.append(
            (
                "SCI-22",
                "SciPy ndimage",
                upstream_defects,
                f"the 1-D {operation} filter refuses a {dt} input",
            )
        )
    else:
        raise RuntimeError("Unsupported array type")
    cv = x.dtype.type(cval)
    origin = operator.index(origin)
    # the array SciPy's C++ writes into: the output itself for its three types (x_out = output)
    if direct:
        own = out_arr is not None and check_out
        dest = out_arr if own else np.zeros(input.shape, dtype=target)
        if own and not dest.flags.writeable:
            raise ValueError("WRITEBACKIFCOPY base is read-only")
    # the C++ switches on the input's type number: numpy's longlong is not its int64, nor its long
    # double its float64 where both have 8 bytes (Windows, macOS on Apple silicon) (SCI-22)
    long_int = x.dtype.kind == "i" and x.dtype.num != np.dtype(np.int64).num
    long_float = x.dtype.kind == "f" and x.dtype.num not in (
        np.dtype(np.float32).num,
        np.dtype(np.float64).num,
    )
    if long_int or long_float:
        if upstream_defects != "fix":
            raise TypeError("Unsupported array type")
        name = "longlong" if long_int else "long double"
        notices.append(
            (
                "SCI-22",
                "SciPy ndimage",
                upstream_defects,
                f"the 1-D {operation} filter refuses a {x.dtype.char!r} ({name}) input",
            )
        )
    xc = _native(x, np.dtype(f"{x.dtype.kind}{x.dtype.itemsize}"))
    code = {"grid-wrap": "wrap", "grid-mirror": "reflect", "grid-constant": "constant"}.get(
        mode, mode
    )
    # the C++ reads cval by the input's type: PyLong_AsLongLong (by __index__) or PyFloat_AsDouble
    cv_c = float(operator.index(cv)) if x.dtype.kind == "i" else _c_double(cv)
    res = _core.ndimage_rank_filter_1d(xc, int(rank), int(footprint.size), code, cv_c, origin)
    # SCI-09: SciPy's running rank takes the whole window, the footprint's false elements
    # included; corrected, a footprint with holes is filtered by the n-dimensional filter
    if not footprint.all():
        fixed = nd(xc)
        if not np.array_equal(fixed, res, equal_nan=res.dtype.kind == "f"):
            notices.append(
                (
                    "SCI-09",
                    "SciPy ndimage",
                    upstream_defects,
                    f"the 1-D {operation} filter's footprint has false elements, which "
                    "SciPy's running rank counts in its window",
                )
            )
        if upstream_defects == "fix":
            res = fixed
    if not direct:
        out = np.zeros(input.shape, dtype=target) if out_arr is None else out_arr
        np.copyto(out, res, casting="unsafe")
        _defects.warn(notices, stacklevel=4)
        return out
    contiguous = dest.flags.c_contiguous and dest.flags.aligned
    if dest.dtype == res.dtype:
        np.copyto(dest, res)
        if not contiguous and upstream_defects != "fix":
            _writeback_warning()
    elif upstream_defects == "fix":
        np.copyto(dest, res, casting="unsafe")
        notices.append(
            (
                "SCI-21",
                "SciPy ndimage",
                upstream_defects,
                f"the 1-D {operation} filter of a {res.dtype} input into a {dest.dtype} output",
            )
        )
    else:
        # SciPy's C++ writes the input type's values into the output's (contiguous) buffer
        buf = dest if contiguous else np.ascontiguousarray(dest)
        raw = np.ascontiguousarray(res).reshape(-1).view(np.uint8)
        flat = buf.reshape(-1).view(np.uint8)
        n = min(raw.size, flat.size)
        flat[:n] = raw[:n]
        detail = f"the 1-D {operation} filter of a {res.dtype} input into a {dest.dtype} output"
        if raw.size > flat.size:
            detail += (
                f": SciPy writes {raw.size - flat.size} bytes past the output's end, which "
                "larmorx does not write"
            )
        notices.append(("SCI-21", "SciPy ndimage", upstream_defects, detail))
        if buf is not dest:
            dest[...] = buf
            _writeback_warning()
    _defects.warn(notices, stacklevel=4)
    if out_arr is not None and not check_out:  # SciPy's temporary output, copied into the given one
        out_arr[...] = dest
        return out_arr
    return dest


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
    """SciPy's ``_rank_filter`` (the rank, median and percentile filters), check for check:
    the smallest and largest ranks as the minimum and maximum filters, a 1-D input by SciPy's
    running rank, any other by ``NI_RankFilter``."""
    _defects.check(upstream_defects)
    if (size is not None) and (footprint is not None):
        warnings.warn("ignoring size because footprint is set", UserWarning, stacklevel=3)
    input = np.asarray(input)
    if np.iscomplexobj(input):
        raise TypeError("Complex type not supported")
    axes = _check_axes(axes, input.ndim)
    num_axes = len(axes)
    if footprint is None:
        if size is None:
            raise RuntimeError("no footprint or filter size provided")
        sizes = _normalize_sequence(size, num_axes)
        footprint = np.ones(sizes, dtype=bool)
    else:
        footprint = np.asarray(footprint, dtype=bool)
    footprint = _expand_footprint(input.ndim, axes, footprint)
    origins = _expand_origin(input.ndim, axes, origin)
    mode = _expand_mode(input.ndim, axes, mode)
    fshape = [ii for ii in footprint.shape if ii > 0]
    if len(fshape) != input.ndim:
        raise RuntimeError(f"footprint.ndim ({footprint.ndim}) must match len(axes) ({len(axes)})")
    # (SciPy's loop rebinds `origin`: the 1-D path below reads the last axis' origin)
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
    target, out_arr = _get_output(output, input)
    check_out = not (out_arr is not None and np.may_share_memory(input, out_arr))
    if not isinstance(mode, str) and isinstance(mode, Iterable):
        raise RuntimeError("A sequence of modes is not supported by non-separable rank filters")
    mode = _mode(mode)

    # SciPy 1.18.1 sends 1-D inputs to its running-rank code unless the footprint is too large
    # for the input (gh-23293); it passes the footprint's size, its false elements included
    lim2 = input.size - ((footprint.size - 1) // 2 - origin)
    if input.ndim == 1 and ((lim2 >= 0) or (input.size == 1)):

        def nd_1d(xc: np.ndarray) -> np.ndarray:
            # SCI-09's correction: the n-dimensional filter of the running rank's own input,
            # in its type (the values a rank filter returns are its input's)
            return _core.ndimage_rank_filter(
                xc,
                int(rank),
                footprint,
                mode,
                float(xc.dtype.type(cval)),
                [int(o) for o in origins],
                int(n_threads),
                xc.dtype.name,
            )

        return _rank_1d(
            input,
            rank,
            footprint,
            target,
            out_arr,
            check_out,
            mode,
            cval,
            origin,
            operation,
            upstream_defects,
            nd_1d,
        )
    # _nd_image.rank_filter's arguments ("O&iO&O&idO&"), in order
    if check_out:
        _c_output(out_arr)
    cval = _c_double(cval)
    origins = [_intp(o) for o in origins]
    if input.size == 0:
        return _empty_result(input, target, out_arr)
    in_type = _nd_type(input.dtype)
    out_type = _nd_type(target)
    out = _core.ndimage_rank_filter(
        _native(input, in_type),
        int(rank),
        footprint,
        mode,
        cval,
        origins,
        int(n_threads),
        "bool" if target == np.bool_ else out_type.name,
    )
    return _write(out, target, out_arr)


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
    it with the n-dimensional filter, with a warning when that changes the result.

    SCI-21 to SCI-23 (rule A; the same switch), of SciPy's running rank of a 1-D input: a
    float32, float64 or int64 input written into an output of another type or byte order gets
    the input type's bytes (past the output's end when its items are smaller), which larmorx
    corrects to the values cast into the output; big-endian float32 and float64 inputs and
    numpy's ``longlong`` are refused by SciPy, filtered by larmorx; a non-contiguous output of
    the input's type makes numpy warn about SciPy's unresolved write-back copy, which larmorx
    does not (``reproduce``: SciPy's bytes where they are the output's own, its errors, numpy's
    warning)."""
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


def _morph_type(dtype: np.dtype) -> np.dtype:
    """``NI_BinaryErosion``'s type switches (the mask's, the input's and the output's, in that
    order at the first element): the canonical type larmorx computes in (bool as uint8,
    ``longlong`` as int64), or SciPy's ``RuntimeError``. A byte-swapped array is a native copy
    by then (``NI_ObjectToInputArray``; the output's write-back copy)."""
    if dtype == np.bool_:
        return np.dtype(np.uint8)
    if (dtype.kind in "iu" and dtype.itemsize in (1, 2, 4, 8)) or (
        dtype.kind == "f" and dtype.itemsize in (4, 8)
    ):
        return np.dtype(f"{dtype.kind}{dtype.itemsize}")
    raise RuntimeError("data type not supported")


def _c_int(v: Any) -> int:
    """``PyArg_ParseTuple``'s ``"i"``: CPython's ``PyLong_AsLong`` (``__index__``, so a float, a
    string or numpy's bool is a ``TypeError``; beyond a C ``long``, 64-bit on Linux and macOS:
    "Python int too large to convert to C long"), then the C ``int`` range."""
    v = operator.index(v)
    if v > 2**63 - 1 or v < -(2**63):
        raise OverflowError("Python int too large to convert to C long")
    if v > 2**31 - 1:
        raise OverflowError("signed integer is greater than maximum")
    if v < -(2**31):
        raise OverflowError("signed integer is less than minimum")
    return v


def _center_is_true(structure: np.ndarray, origin: list) -> bool:
    """SciPy's ``_center_is_true``: the structure's element at its centre moved by the origins
    (numpy's indexing: a float origin is an ``IndexError``, a negative index counts from the
    end)."""
    structure = np.asarray(structure)
    coor = tuple([oo + ss // 2 for ss, oo in zip(structure.shape, origin, strict=False)])
    return bool(structure[coor])


def _needs_writeback(out_arr: np.ndarray, shares: bool) -> bool:
    """Whether ``NI_ObjectToInputOutputArray`` (``NPY_ARRAY_BEHAVED_NS``) gives
    ``NI_BinaryErosion2`` a write-back copy of the output: a byte-swapped or misaligned array
    (SciPy's temporary of the output's type, when the output shares the input's memory)."""
    if not out_arr.dtype.isnative:
        return True
    return not shares and not out_arr.flags.aligned


#: The register's text for the binary erosion's events (``docs/api/upstream-defects.md``).
_SCI10 = (
    "the passes after the first read the mask cast to int8 (values such as 0.5 or 256 false), "
    "the first its truth"
)
_SCI25 = (
    "into a float32 or float64 output array the later passes erode elements outside the mask "
    "(SciPy marks them by the first byte of each element, 0 for 0.0 and 1.0)"
)
_SCI31 = (
    "the passes after the first use an int8 mask array as their scratch (SciPy's cast to int8 "
    "does not copy it): the caller's mask is left -1 where it was true and the first pass's "
    "output where it was false (binary_closing's erosion then reads that mask)"
)


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
    """SciPy's ``_binary_erosion``, check for check: its Python checks, then
    ``Py_BinaryErosion``'s parser (the output's write access, ``border_value`` as a C ``int``,
    the origins by numpy's ``PyArray_IntpConverter``) and ``NI_BinaryErosion``'s type switches
    (none for an empty input); for the propagation strategy (``iterations`` other than 1, the
    centre true, no ``brute_force``) the mask cast to ``int8`` and ``iterations - 1`` as a C
    ``int`` (SciPy's second C call); then the passes in Rust.

    Defects (``upstream_defects``): SCI-10 (the later passes read the mask cast to ``int8``),
    SCI-25 (into a float output array the later passes erode outside the mask), SCI-26 (numpy's
    write-back warning for a byte-swapped or misaligned output array; ``reproduce`` only),
    SCI-31 (the later passes overwrite the caller's ``int8`` mask array: ``reproduce`` writes
    SciPy's values into it, the default leaves it as given)."""
    _defects.check(upstream_defects)
    try:
        iterations = operator.index(iterations)
    except TypeError as e:
        raise TypeError("iterations parameter should be an integer") from e
    input = np.asarray(input)
    # (SciPy 1.18.1; 1.15.3 reads a broadcast input as it is, with the same result)
    if not input.flags.c_contiguous and not input.flags.f_contiguous:
        input = np.ascontiguousarray(input)
    ndim = input.ndim
    if np.iscomplexobj(input):
        raise TypeError("Complex type not supported")
    axes = _check_axes(axes, input.ndim)
    num_axes = len(axes)
    if structure is None:
        structure = generate_binary_structure(num_axes, 1)
    else:
        structure = np.asarray(structure, dtype=bool)
    if ndim > num_axes:
        structure = _expand_footprint(ndim, axes, structure, "structure")
    if structure.ndim != input.ndim:
        raise RuntimeError("structure and input must have same dimensionality")
    structure = np.ascontiguousarray(structure)
    if structure.size < 1:
        raise RuntimeError("structure must not be empty")
    # SCI-31: the caller's own int8 array (numpy's cast to int8 returns it, or a view of it)
    caller_int8 = isinstance(mask, np.ndarray) and mask.dtype == np.int8
    if mask is not None:
        mask = np.asarray(mask)
        if mask.shape != input.shape:
            raise RuntimeError("mask and input must have equal sizes")
    origin = _normalize_sequence(origin, num_axes)
    origin = _expand_origin(ndim, axes, origin)
    cit = _center_is_true(structure, origin)
    out_arr = None
    if isinstance(output, np.ndarray):
        if np.iscomplexobj(output):
            raise TypeError("Complex output type not supported")
        # (any other output is ignored: SciPy's output is then a new bool array)
        if output.shape != input.shape:
            raise RuntimeError("output shape not correct")
        out_arr = output
    # Py_BinaryErosion's parser ("O&O&O&O&iO&iii")
    _c_output(out_arr)
    border = _c_int(border_value)
    origins = [_intp(o) for o in origin]
    out_dtype = np.dtype(bool) if out_arr is None else out_arr.dtype
    # NI_BinaryErosion's type switches, at the first element
    if input.size:
        if mask is not None:
            _morph_type(mask.dtype)
        _morph_type(input.dtype)
        _morph_type(out_dtype)
    shares = out_arr is not None and np.may_share_memory(input, out_arr)
    lists = iterations != 1 and bool(cit) and not brute_force
    reproduce = upstream_defects == "reproduce"
    if input.size == 0:
        out = np.zeros(input.shape, np.uint8)
        if lists:
            if mask is not None and reproduce:
                np.asarray(mask, dtype=np.int8)
            _c_int(iterations - 1)
    else:
        data = _native(input, _morph_type(input.dtype))
        st = structure
        mdata = None if mask is None else _native(mask, _morph_type(mask.dtype))
        if ndim == 0:
            # a 0-d array: a single element, its own neighbourhood
            data, st, origins = data.reshape(1), st.reshape(1), [0]
            mdata = None if mdata is None else mdata.reshape(1)

        def run(m, its=iterations, protect=True):
            return _core.ndimage_binary_erosion(
                data,
                st,
                origins,
                int(its),
                border != 0,
                bool(invert),
                bool(brute_force),
                m,
                int(n_threads),
                protect,
            )

        if lists:
            # SciPy's steps before its second C call: the mask cast to int8 (numpy's warnings
            # for NaN and values out of int8's range: SCI-10's cast, with reproduce), then
            # iterations - 1 as a C int, which fails after the first pass wrote the output
            if mask is not None and reproduce:
                np.asarray(mask, dtype=np.int8)
            try:
                _c_int(iterations - 1)
            except OverflowError:
                if out_arr is not None and not shares:
                    first = run(mdata, 1).reshape(input.shape)
                    np.copyto(out_arr, first, casting="unsafe")
                raise
        # SCI-10: the int8 cast of a mask of another type than bool and 8-bit integers
        truth = None
        if lists and mask is not None and mask.dtype not in (np.bool_, np.uint8, np.int8):
            truth = (mask != 0).view(np.uint8)
            truth = truth.reshape(1) if ndim == 0 else truth
        # SCI-25: an erosion into a float output array
        sci25 = lists and mask is not None and not invert and out_dtype.kind == "f"
        m_fix = mdata if truth is None else truth
        out = run(mdata, protect=not sci25) if reproduce else run(m_fix)
        notices = []
        if truth is not None:
            other = run(m_fix, protect=not sci25) if reproduce else run(mdata)
            if not np.array_equal(out, other):
                notices.append(("SCI-10", "SciPy ndimage", upstream_defects, _SCI10))
        if sci25:
            other = run(mdata) if reproduce else run(m_fix, protect=False)
            if not np.array_equal(out, other):
                notices.append(("SCI-25", "SciPy ndimage", upstream_defects, _SCI25))
        scratch = None
        if lists and caller_int8:
            # SCI-31: NI_BinaryErosion2 marks the mask's elements (-1 where true; where false,
            # the first byte of the output element after the first pass, which it then sets
            # false) and restores only the output; the mask stays so
            first = run(mdata, 1).reshape(input.shape)
            odt = out_dtype.newbyteorder("=")
            b0 = np.ascontiguousarray(first.astype(odt)).view(np.uint8)
            b0 = b0.reshape(first.size, odt.itemsize)[:, 0].view(np.int8).reshape(input.shape)
            scratch = np.where(mask != 0, np.int8(-1), b0)
            if np.array_equal(scratch, mask):
                scratch = None
            else:
                notices.append(("SCI-31", "SciPy ndimage", upstream_defects, _SCI31))
        if notices:
            _defects.warn(notices, stacklevel=3)
        if scratch is not None and reproduce:
            _write_scratch(mask, scratch)
        out = out.reshape(input.shape)
    if lists and reproduce and out_arr is not None and _needs_writeback(out_arr, shares):
        # SCI-26 reproduced: NI_BinaryErosion2's write-back copy is never resolved
        _writeback_warning()
    if out_arr is None:
        return out.view(np.bool_)
    np.copyto(out_arr, out, casting="unsafe")
    return out_arr


def _write_scratch(mask: np.ndarray, scratch: np.ndarray) -> None:
    """SCI-31 reproduced: SciPy's values written into the caller's mask (a read-only one too,
    as SciPy's C writes through its pointer, where numpy lets the flag be lifted)."""
    lifted = False
    if not mask.flags.writeable:
        try:
            mask.flags.writeable = True
            lifted = True
        except ValueError:
            return  # memory numpy does not let be written: never written
    try:
        mask[...] = scratch
    finally:
        if lifted:
            mask.flags.writeable = False


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
    """``scipy.ndimage.binary_erosion`` (SCI-10, SCI-25 and SCI-26: ``upstream_defects``,
    ``docs/api/ndimage-filters.md``). An ``output`` that is not an array is ignored, as SciPy
    ignores it (the result is a new ``bool`` array)."""
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
    input = np.asarray(input)
    axes = _check_axes(axes, input.ndim)
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
        input,
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
    input = np.asarray(input)
    axes = _check_axes(axes, input.ndim)
    num_axes = len(axes)
    if structure is None:
        structure = generate_binary_structure(num_axes, 1)

    def run(mode: str, out: Any, m: Any) -> Any:
        tmp = binary_erosion(
            input, structure, iterations, m, None, border_value, origin, brute_force,
            axes=axes, n_threads=n_threads, upstream_defects=mode,
        )  # fmt: skip
        return binary_dilation(
            tmp, structure, iterations, m, out, border_value, origin, brute_force,
            axes=axes, n_threads=n_threads, upstream_defects=mode,
        )  # fmt: skip

    return _composite(run, output, mask, upstream_defects)


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
    input = np.asarray(input)
    axes = _check_axes(axes, input.ndim)
    num_axes = len(axes)
    if structure is None:
        structure = generate_binary_structure(num_axes, 1)

    def run(mode: str, out: Any, m: Any) -> Any:
        tmp = binary_dilation(
            input, structure, iterations, m, None, border_value, origin, brute_force,
            axes=axes, n_threads=n_threads, upstream_defects=mode,
        )  # fmt: skip
        return binary_erosion(
            tmp, structure, iterations, m, out, border_value, origin, brute_force,
            axes=axes, n_threads=n_threads, upstream_defects=mode,
        )  # fmt: skip

    return _composite(run, output, mask, upstream_defects)


def _composite(run: Any, output: Any, mask: Any, upstream_defects: str) -> Any:
    """An opening or a closing in ``upstream_defects``' mode, its steps' notices given only
    where they change the composite's result or the caller's mask (SCI-31): a step's change
    (SCI-10, SCI-25) can vanish in the other step."""
    import warnings as _w

    # only an int8 mask array is written into (SCI-31, reproduced): its values before
    before = mask.copy() if isinstance(mask, np.ndarray) and mask.dtype == np.int8 else mask
    with _w.catch_warnings(record=True) as caught:
        _w.simplefilter("always")
        result = run(upstream_defects, output, mask)
    notices = [w for w in caught if isinstance(w.message, _defects.UpstreamDefectWarning)]
    if notices:
        m2 = before.copy() if before is not mask else mask
        o2 = np.empty_like(output) if isinstance(output, np.ndarray) else output
        try:
            with _w.catch_warnings():
                _w.simplefilter("ignore")
                other = run("fix" if upstream_defects == "reproduce" else "reproduce", o2, m2)
        except Exception:
            other = None
        if other is not None:
            a, b = np.asarray(result), np.asarray(other)
            same = a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()
            if same and (m2 is mask or np.array_equal(np.asarray(mask), np.asarray(m2))):
                notices = []
    for w in caught:
        if w in notices or not isinstance(w.message, _defects.UpstreamDefectWarning):
            _w.warn(w.message, stacklevel=3)
    return result


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
) -> np.ndarray | None:
    """``scipy.ndimage.binary_hit_or_miss``: the elements where ``structure1`` fits the input
    and ``structure2`` its complement; ``None`` when ``output`` is an array (the result is
    written there), as SciPy returns."""
    input = np.asarray(input)
    axes = _check_axes(axes, input.ndim)
    num_axes = len(axes)
    if structure1 is None:
        structure1 = generate_binary_structure(num_axes, 1)
    else:
        structure1 = np.asarray(structure1)
    if structure2 is None:
        structure2 = np.logical_not(structure1)
    origin1 = _normalize_sequence(origin1, num_axes)
    origin2 = origin1 if origin2 is None else _normalize_sequence(origin2, num_axes)
    tmp1 = _binary_erosion(input, structure1, 1, None, None, 0, origin1, 0, False, axes, n_threads)
    inplace = isinstance(output, np.ndarray)
    result = _binary_erosion(
        input, structure2, 1, None, output, 0, origin2, 1, False, axes, n_threads
    )
    if inplace:
        np.logical_not(output, output)
        np.logical_and(tmp1, output, output)
        return None
    np.logical_not(result, result)
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
    larmorx reads its truth in every pass, with a warning when that changes the result. SCI-26:
    a byte-swapped or misaligned ``output`` array gets numpy's write-back warning with
    ``reproduce`` only."""
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
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray | None:
    """``scipy.ndimage.binary_fill_holes``: the complement of the background reached from the
    array's edges; ``None`` when ``output`` is an array (the result is written there), as
    SciPy returns (SCI-26: :func:`binary_propagation`)."""
    input = np.asarray(input)
    mask = np.logical_not(input)
    tmp = np.zeros(mask.shape, bool)
    inplace = isinstance(output, np.ndarray)
    if inplace:
        binary_dilation(
            tmp,
            structure,
            -1,
            mask,
            output,
            1,
            origin,
            axes=axes,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )
        np.logical_not(output, output)
        return None
    output = binary_dilation(
        tmp,
        structure,
        -1,
        mask,
        None,
        1,
        origin,
        axes=axes,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    np.logical_not(output, output)
    return output


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


def _label_type(dtype: np.dtype) -> np.dtype:
    """``_ni_label``'s fused type ``data_t`` (Cython's dispatch by kind and size): the canonical
    type, or Cython's ``TypeError``."""
    if (dtype.kind in "iu" and dtype.itemsize in (1, 2, 4, 8)) or (
        dtype.kind == "f" and dtype.itemsize in (4, 8)
    ):
        return np.dtype(f"{dtype.kind}{dtype.itemsize}")
    raise TypeError("No matching signature found")


_BIG_ENDIAN = "Big-endian buffer not supported on little-endian compiler"
_SCI27 = "SciPy's label refuses a big-endian {what} ({error}); labelled as its native copy"
_SCI28 = "SciPy's label writes the labels into a read-only output array"
_SCI29 = (
    "SciPy's label writes labels above 1 into a bool output as their bytes ({n} labels: a bool "
    "array holding bytes other than 0 and 1)"
)


def _iter_axis(a: np.ndarray) -> int:
    """numpy's ``PyArray_IterAllButAxis`` with ``axis = -1``, the axis ``_ni_label`` runs its
    lines along (the output's): the axis of the smallest positive stride, starting from the
    first non-zero stride counted as axis 0's."""
    minaxis, minstride, i = 0, 0, 0
    while minstride == 0 and i < a.ndim:
        minstride = a.strides[i]
        i += 1
    for i in range(1, a.ndim):
        if 0 < a.strides[i] < minstride:
            minaxis, minstride = i, a.strides[i]
    return minaxis


def _provisional_labels(fg: np.ndarray, structure: np.ndarray) -> int:
    """The number of working labels ``_ni_label`` gives out (its lines along the last axis):
    the foreground elements none of whose earlier neighbours (the structure's elements before
    its centre, in C order) is foreground. Its working labels run from 2 up."""
    has = np.zeros(fg.shape, bool)
    for lin in range((structure.size - 1) // 2):
        if not structure.flat[lin]:
            continue
        d = [int(k) - 1 for k in np.unravel_index(lin, structure.shape)]
        src = tuple(slice(max(0, di), n + min(0, di)) for di, n in zip(d, fg.shape, strict=True))
        dst = tuple(slice(max(0, -di), n - max(0, di)) for di, n in zip(d, fg.shape, strict=True))
        has[dst] |= fg[src]
    return int(np.count_nonzero(fg & ~has))


def _write_labels(output: np.ndarray, labels: np.ndarray) -> bool:
    """``output[...] = labels`` (numpy's cast) and whether every label fits (SciPy's check after
    its ``NeedMoreBits`` retry)."""
    output[...] = labels
    return bool(np.all(output == labels))


def label(
    input: Any,
    structure: Any = None,
    output: Any = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """``scipy.ndimage.label``: the connected components of the non-zero elements, numbered
    from 1 by their first element in C order; ``(labels, number)``, or ``number`` when an
    ``output`` array is given.

    SciPy's checks in its order (the output made from a dtype first, scalars and empty arrays,
    ``_ni_label``'s symmetry check, then the input's and the output's types). Corrected by
    default (``upstream_defects="reproduce"`` gives SciPy's behaviour): SCI-27, a big-endian
    input or output is labelled as its native copy (SciPy: ``ValueError``); SCI-28, a read-only
    output array is refused (SciPy writes into it); SCI-29, labels above 1 into a ``bool``
    output are refused as labels that do not fit any type (SciPy writes their bytes into the
    bool array, up to 255)."""
    _defects.check(upstream_defects)
    reproduce = upstream_defects == "reproduce"
    input = np.asarray(input)
    if np.iscomplexobj(input):
        raise TypeError("Complex type not supported")
    if structure is None:
        structure = generate_binary_structure(input.ndim, 1)
    structure = np.asarray(structure, dtype=bool)
    if structure.ndim != input.ndim:
        raise RuntimeError("structure and input must have equal rank")
    for ii in structure.shape:
        if ii != 3:
            raise ValueError("structure dimensions must be equal to 3")
    need_64bits = input.size >= (2**31 - 2)
    if isinstance(output, np.ndarray):
        if output.shape != input.shape:
            raise ValueError("output shape not correct")
        caller_provided_output = True
    else:
        caller_provided_output = False
        if output is None:
            output = np.empty(input.shape, np.intp if need_64bits else np.int32)
        else:
            output = np.empty(input.shape, output)
    if input.ndim == 0 or input.size == 0:
        if input.ndim == 0:
            maxlabel = 1 if (input != 0) else 0
            output[...] = maxlabel
        else:
            maxlabel = 0
        if caller_provided_output:
            return maxlabel
        return output, maxlabel
    # _ni_label._label
    if not np.all(structure == structure[(np.s_[::-1],) * structure.ndim]):
        raise AssertionError("Structuring element is not symmetric")
    notices = []
    in_dtype = np.dtype(np.uint8) if input.dtype == np.bool_ else input.dtype
    in_t = _label_type(in_dtype)
    if not in_dtype.isnative:
        if reproduce:
            raise ValueError(_BIG_ENDIAN)
        notices.append(("SCI-27", "SciPy ndimage", upstream_defects, "input"))
    out_dtype = np.dtype(np.uint8) if output.dtype == np.bool_ else output.dtype
    _label_type(out_dtype)
    if not out_dtype.isnative:
        if reproduce:
            raise ValueError(_BIG_ENDIAN)
        notices.append(("SCI-27", "SciPy ndimage", upstream_defects, "output"))
    if notices:
        what = " and ".join(n[3] for n in notices)
        _defects.warn(
            [
                (
                    "SCI-27",
                    "SciPy ndimage",
                    upstream_defects,
                    _SCI27.format(what=what, error=f"ValueError: {_BIG_ENDIAN}"),
                )
            ],
            stacklevel=2,
        )
    lifted = False
    if not output.flags.writeable:
        if not reproduce:
            raise ValueError(
                "output array is read-only. (SCI-28: SciPy's label writes into it; "
                "upstream_defects='reproduce' does)"
            )
        try:
            output.flags.writeable = True
            lifted = True
        except ValueError:
            raise ValueError(
                "output array is read-only. (SCI-28: numpy does not let this array be "
                "written, so SciPy's write into it is not reproduced)"
            ) from None
        _defects.warn([("SCI-28", "SciPy ndimage", upstream_defects, _SCI28)], stacklevel=2)
    try:
        # the lines run along the output's axis of the smallest stride: the components are
        # numbered by their first element in C order with that axis last
        axis = _iter_axis(output.view(np.uint8) if output.dtype == np.bool_ else output)
        x = _native(input, in_t)
        st = structure
        last = axis == input.ndim - 1
        if not last:
            x, st = np.moveaxis(x, axis, -1), np.moveaxis(st, axis, -1)
        st = np.ascontiguousarray(st)
        labels, n = _core.ndimage_label(x, st)
        # SciPy views a bool output as uint8: when its working labels fit in a byte
        # (NeedMoreBits otherwise: the labels cast, refused), the labels are written as their
        # bytes
        if output.dtype == np.bool_ and n > 1 and _provisional_labels(x != 0, st) + 1 <= 255:
            _defects.warn(
                [("SCI-29", "SciPy ndimage", upstream_defects, _SCI29.format(n=n))],
                stacklevel=2,
            )
            if reproduce:
                output.view(np.uint8)[...] = labels if last else np.moveaxis(labels, -1, axis)
                return n if caller_provided_output else (output, n)
        if not last:
            labels = np.moveaxis(labels, -1, axis)
        if not _write_labels(output, labels):
            raise RuntimeError("insufficient bit-depth in requested output type")
    finally:
        if lifted:
            output.flags.writeable = False
    if caller_provided_output:
        return n
    return output, n


def _safely_castable_to_int(dt: np.dtype) -> bool:
    """SciPy's ``_safely_castable_to_int``: labels ``bincount`` takes as they are."""
    int_size = np.dtype(int).itemsize
    return bool(
        (np.issubdtype(dt, np.signedinteger) and dt.itemsize <= int_size)
        or (np.issubdtype(dt, np.unsignedinteger) and dt.itemsize < int_size)
    )


_SCI30 = "SciPy's sums per label fail on {what}; corrected: {fix}"


def _stats(input, labels, index, single, mode: str):
    """SciPy's ``_stats`` (not centred): ``(counts, sums)`` of the whole input, of the positive
    labels, of one label or per label of a sequence ``index`` (``bincount``, through
    ``unique`` for labels it cannot take); ``single(values)`` sums a selection. SCI-30 (rule A):
    a 0-d index array and an empty input with a sequence index, where SciPy fails, give the
    sums per label (a scalar for the 0-d index; zeros for the empty input)."""

    def single_group(vals):
        return vals.size, single(vals)

    input = np.asarray(input)
    if labels is None:
        return single_group(input)
    labels = np.asarray(labels)
    input, labels = np.broadcast_arrays(input, labels)
    if index is None:
        return single_group(input[labels > 0])
    if np.isscalar(index):
        return single_group(input[labels == index])
    if mode != "reproduce":
        if isinstance(index, np.ndarray) and index.ndim == 0:
            what = "a 0-d index array (TypeError)"
            if input.size:
                counts, sums = _stats(input, labels, index.reshape(1), single, "reproduce")
                fix = "the sums per label of its one label"
                counts, sums = counts.reshape(())[()], sums.reshape(())[()]
            else:
                counts, sums = np.intp(0), np.float64(0.0)
                fix = "zeros"
            _defects.warn(
                [("SCI-30", "SciPy ndimage", mode, _SCI30.format(what=what, fix=fix))],
                stacklevel=4,
            )
            return counts, sums
        if input.size == 0:
            shape = np.shape(index)
            _defects.warn(
                [
                    (
                        "SCI-30",
                        "SciPy ndimage",
                        mode,
                        _SCI30.format(
                            what="an empty input with a sequence index (ValueError or IndexError)",
                            fix="zero counts and sums",
                        ),
                    )
                ],
                stacklevel=4,
            )
            return np.zeros(shape, np.intp), np.zeros(shape, np.float64)
    if not _safely_castable_to_int(labels.dtype) or labels.min() < 0 or labels.max() > labels.size:
        unique_labels, new_labels = np.unique(labels, return_inverse=True)
        new_labels = np.reshape(new_labels, (-1,))
        counts = np.bincount(new_labels)
        sums = np.bincount(new_labels, weights=input.ravel())
        idxs = np.searchsorted(unique_labels, index)
        idxs[idxs >= unique_labels.size] = 0
        found = unique_labels[idxs] == index
    else:
        counts = np.bincount(labels.ravel())
        sums = np.bincount(labels.ravel(), weights=input.ravel())
        idxs = np.asanyarray(index, np.int_).copy()
        found = (idxs >= 0) & (idxs < counts.size)
        idxs[~found] = 0
    counts = counts[idxs]
    counts[~found] = 0
    sums = sums[idxs]
    sums[~found] = 0
    return counts, sums


def sum_labels(
    input: Any,
    labels: Any = None,
    index: Any = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """``scipy.ndimage.sum_labels``: the sum of the input over each label of ``index`` (float64,
    numpy's ``bincount``), over the positive labels (``index`` None), one label (a scalar
    ``index``) or all of it (``labels`` None), numpy's sums of the selection. SCI-30
    (``upstream_defects``): a 0-d ``index`` array and an empty input with a sequence ``index``
    give the sums where SciPy fails."""
    _defects.check(upstream_defects)
    return _stats(input, labels, index, lambda v: v.sum(), upstream_defects)[1]


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


def center_of_mass(
    input: Any,
    labels: Any = None,
    index: Any = None,
    *,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """``scipy.ndimage.center_of_mass``: the centre of mass of the values (of each label of
    ``index``, of the positive labels, or of the whole array), a tuple per centre; each
    coordinate is ``sum_labels(input * grid) / sum_labels(input)`` in float64.

    numpy's sums depend on its release (pairwise from numpy 2.3, chunks of 8192 before): the
    whole-array and selected sums of float32 and float64 values follow ``numpy_version`` (the
    newest release by default) for arrays contiguous in C or Fortran order, the installed
    numpy's for other layouts and types; the sums per label of a sequence ``index`` are
    numpy's ``bincount`` (in C order, the same in every release). SCI-30: :func:`sum_labels`
    (one notice per call)."""
    _defects.check(upstream_defects)

    def single(v):
        return _sum_in_order(v, numpy_version)

    input = np.asarray(input)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        normalizer = _stats(input, labels, index, single, upstream_defects)[1]
        grids = np.ogrid[[slice(0, i) for i in input.shape]]
        results = [
            _stats(input * grids[dir].astype(float), labels, index, single, upstream_defects)[1]
            / normalizer
            for dir in range(input.ndim)
        ]
    seen = set()
    for w in caught:
        if isinstance(w.message, _defects.UpstreamDefectWarning):
            if w.message.entry in seen:
                continue
            seen.add(w.message.entry)
            warnings.warn(w.message, stacklevel=2)
        else:
            warnings.warn_explicit(w.message, w.category, w.filename, w.lineno)
    if np.isscalar(results[0]):
        return tuple(results)
    return [tuple(v) for v in np.array(results).T]


#: The register's entries for ``watershed_ift``'s events (``docs/api/upstream-defects.md``).
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
    # Py_WatershedIFT's parser ("O&O&O&O&": the output's write access), then NI_WatershedIFT
    _c_output(output if given else None)
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


def sum(
    input: Any,
    labels: Any = None,
    index: Any = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Any:
    """``scipy.ndimage.sum``: an alias of :func:`sum_labels`."""
    return sum_labels(input, labels, index, upstream_defects=upstream_defects)

# SPDX-License-Identifier: Apache-2.0 AND MIT
"""nibabel's NIfTI writer with its scaling: an image stored in its header's data type.

nibabel writes an in-memory image (``img.to_filename``) in the type its header names, not the
array's: float data written to an integer type are scaled (``scl_slope``, ``scl_inter``), as the
outputs of Python programs that keep an input's integer header are (sdcflows' ``BSplineApprox``
error map, ``B0FieldTransform.apply``'s corrected series and ``CheckB0Units``' field map;
fMRIPrep's own steps). ``lx.save`` stores the array's own type; :func:`save_nibabel` stores
what nibabel stores.

Ported from nibabel 5.4.2 (MIT, Copyright the NiBabel developers):
``arraywriters.py`` (``make_array_writer``, ``ArrayWriter``, ``SlopeArrayWriter``,
``SlopeInterArrayWriter``: ``scaling_needed``, ``calc_scale``, ``_do_scaling``, ``_iu2iu``,
``_range_scale``, ``_writing_range``, ``get_slope_inter``), ``volumeutils.py``
(``array_to_file``, ``_write_data``, ``finite_range``, ``best_write_scale_ftype``,
``_ftype4scaled_finite``, ``working_type``, ``_dt_min_max``, ``better_float_of``,
``_matching_float``), ``casting.py`` (``shared_range``, ``floor_exact``, ``ceil_exact``,
``floor_log2``, ``int_abs``) and ``analyze.py`` (``AnalyzeImage.to_file_map``: the scaling
is computed when the header's ``scl_slope`` and ``scl_inter`` are unset, as they always are
for an image built in memory). The arithmetic is numpy's own, so it is nibabel's by
construction, with one exception: nibabel computes the slope and intercept of a float-to-integer
scaling in ``np.longdouble`` (``best_float()``), which is x87's 80-bit extended type on x86-64
Linux and float64 elsewhere. Those few scalar operations are made here in exact rational
arithmetic rounded to a 64-bit significand (:class:`_Extended`), so every platform gives
x86-64 Linux's slope and intercept bits.

The voxel data and header are written by larmorx's NIfTI writer (``larmorx-io``, through
``_core.nifti_header_for_image``, the header nibabel's ``update_header`` makes, and
``_core.nifti_write``).
"""

from __future__ import annotations

import math
import os
from fractions import Fraction
from typing import Any

import numpy as np

from larmorx import _core

__all__ = ["HeaderDataError", "ScalingError", "WriterError", "nibabel_stored", "save_nibabel"]


class WriterError(Exception):
    """nibabel's ``WriterError``: the array cannot be written in the requested type."""


class ScalingError(WriterError):
    """nibabel's ``ScalingError``: no finite slope and intercept exist."""


class HeaderDataError(Exception):
    """nibabel's ``HeaderDataError``: the header cannot hold the scaling (a slope of 0)."""


# ------------------------------------------------------------------------------------------------
# x87 extended precision (nibabel's best_float() on x86-64 Linux), exactly


class _Extended:
    """A value of x87's extended type (64-bit significand, IEEE round to nearest even), with
    the sign of zero. Only the operations nibabel's ``_range_scale`` makes."""

    __slots__ = ("negzero", "q")

    def __init__(self, q: Fraction, negzero: bool = False) -> None:
        self.q = q
        self.negzero = bool(negzero and q == 0)

    @classmethod
    def of(cls, v: Any) -> _Extended:
        if isinstance(v, (int, np.integer)):
            # numpy's longdouble(int): exact up to 64 bits (every int64 and uint64)
            return cls(_round(Fraction(int(v)), 64, None, None))
        f = float(v)  # float32 and float64 values convert exactly
        if not math.isfinite(f):
            raise ValueError("an extended value must be finite here")
        return cls(Fraction(f), f == 0 and math.copysign(1.0, f) < 0)

    def __neg__(self) -> _Extended:
        return _Extended(-self.q, self.q == 0 and not self.negzero)

    def __add__(self, other: _Extended) -> _Extended:
        s = self.q + other.q
        if s == 0:
            both_neg = (self.q == 0 and self.negzero) and (other.q == 0 and other.negzero)
            return _Extended(Fraction(0), both_neg)
        return _Extended(_round(s, 64, None, None))

    def __sub__(self, other: _Extended) -> _Extended:
        return self + (-other)

    def __mul__(self, other: _Extended) -> _Extended:
        p = self.q * other.q
        if p == 0:
            return _Extended(Fraction(0), self._signbit() != other._signbit())
        return _Extended(_round(p, 64, None, None))

    def __truediv__(self, other: _Extended) -> _Extended:
        if other.q == 0:
            raise ZeroDivisionError("division by zero in the scaling")
        r = self.q / other.q
        if r == 0:
            return _Extended(Fraction(0), self._signbit() != other._signbit())
        return _Extended(_round(r, 64, None, None))

    def _signbit(self) -> bool:
        return self.q < 0 or (self.q == 0 and self.negzero)

    def abs_lt(self, other: _Extended) -> bool:
        return abs(self.q) < abs(other.q)

    def to_float32(self) -> np.float32:
        """``np.float32(longdouble)``: one rounding to binary32 (subnormals, overflow)."""
        if self.q == 0:
            return np.float32(-0.0 if self.negzero else 0.0)
        r = _round(self.q, 24, -126, 127)
        if r is None:
            return np.float32(np.inf if self.q > 0 else -np.inf)
        if r == 0:  # underflow keeps the sign
            return np.float32(-0.0 if self.q < 0 else 0.0)
        return np.float32(float(r))


def _round(q: Fraction, p: int, emin: int | None, emax: int | None) -> Fraction | None:
    """``q`` rounded to ``p`` significant bits, ties to even; with ``emin`` the quantum stays
    at least ``2**(emin - p + 1)`` (subnormals); with ``emax`` a result of ``2**(emax + 1)`` or
    more is ``None`` (overflow)."""
    if q == 0:
        return q
    sign = -1 if q < 0 else 1
    a = abs(q)
    e = a.numerator.bit_length() - a.denominator.bit_length()
    if Fraction(2) ** e > a:
        e -= 1
    if emin is not None:
        e = max(e, emin)
    quantum = Fraction(2) ** (e - p + 1)
    n = a / quantum
    whole = n.numerator // n.denominator
    rest = n - whole
    if rest > Fraction(1, 2) or (rest == Fraction(1, 2) and whole % 2 == 1):
        whole += 1
    r = whole * quantum
    if emax is not None and r >= Fraction(2) ** (emax + 1):
        return None
    return sign * r


# ------------------------------------------------------------------------------------------------
# nibabel/casting.py and volumeutils.py helpers

# nibabel's TRUNC_UINT64 on x86-64 (float64(2**63 + 2**11) converts to uint64 exactly there)
_TRUNC_UINT64 = False
_SHARED_RANGES: dict[tuple[type, type], tuple[Any, Any]] = {}
# OK_FLOATS from float32 on (float16 sorts first; x86-64 Linux has an extended longdouble)
_OK_FLOATS_FROM_F32 = (np.float32, np.float64, "longdouble")


def _floor_log2(x: int) -> int | None:
    ip = 0
    rem = abs(x)
    if rem > 1:
        while rem >= 2:
            ip += 1
            rem //= 2
        return ip
    if rem == 0:
        return None
    while rem < 1:
        ip -= 1
        rem *= 2
    return ip


def _floor_exact(val: Any, flt_type: Any) -> Any:
    val = int(val)
    flt_type = np.dtype(flt_type).type
    sign = 1 if val > 0 else -1
    try:
        fval = flt_type(val)
    except OverflowError:
        return sign * np.inf
    if not np.isfinite(fval):
        return fval
    nmant = np.finfo(flt_type).nmant
    diff = val - int(fval)
    if diff >= 0:
        return fval
    biggest_gap = 2 ** (_floor_log2(val) - nmant)
    if not biggest_gap > 1:
        raise AssertionError
    fval -= flt_type(biggest_gap)
    return fval


def _ceil_exact(val: Any, flt_type: Any) -> Any:
    return -_floor_exact(-val, flt_type)


def _shared_range(flt_type: Any, int_type: Any) -> tuple[Any, Any]:
    flt_type = np.dtype(flt_type).type
    int_type = np.dtype(int_type).type
    key = (flt_type, int_type)
    if key in _SHARED_RANGES:
        return _SHARED_RANGES[key]
    ii = np.iinfo(int_type)
    fi = np.finfo(flt_type)
    mn = _ceil_exact(ii.min, flt_type)
    if mn == -np.inf:
        mn = fi.min
    mx = _floor_exact(ii.max, flt_type)
    if mx == np.inf:
        mx = fi.max
    elif _TRUNC_UINT64 and int_type == np.uint64:
        mx = min(mx, flt_type(2**63))
    _SHARED_RANGES[key] = (mn, mx)
    return mn, mx


def _int_abs(arr: Any) -> np.ndarray:
    arr = np.asarray(arr)
    dt = arr.dtype
    if dt.kind == "u":
        return arr
    if dt.kind != "i":
        return np.absolute(arr)
    out = arr.astype(np.dtype(dt.str.replace("i", "u")))
    return np.choose(arr < 0, (arr, arr * -1), out=out)


def _finite_range(arr: Any) -> tuple[Any, Any, bool]:
    """``finite_range(arr, check_nan=True)``."""
    arr = np.asarray(arr)
    if arr.size == 0:
        return (np.inf, -np.inf, False)
    stride_order = np.argsort(arr.strides)[::-1]
    sarr = arr.transpose(stride_order)
    kind = sarr.dtype.kind
    if kind in "iu":
        return np.min(sarr), np.max(sarr), False
    if kind not in "cf":
        raise TypeError("Can only handle numeric types")
    sarr = np.atleast_2d(sarr)
    has_nan = False
    n_slices = sarr.shape[0]
    maxes = np.zeros(n_slices, dtype=sarr.dtype) - np.inf
    mins = np.zeros(n_slices, dtype=sarr.dtype) + np.inf
    for s in range(n_slices):
        this_slice = sarr[s]
        if not has_nan:
            maxes[s] = np.max(this_slice)
            max_good = False
            if np.isnan(maxes[s]):
                has_nan = True
            elif maxes[s] != np.inf:
                max_good = True
                mins[s] = np.min(this_slice)
                if mins[s] != -np.inf:
                    continue
        tmp = this_slice[np.isfinite(this_slice)]
        if tmp.size == 0:
            maxes[s] = -np.inf
            mins[s] = np.inf
            continue
        if not max_good:
            maxes[s] = np.max(tmp)
        mins[s] = np.min(tmp)
    return np.nanmin(mins), np.nanmax(maxes), has_nan


def _dt_min_max(dtype_like: Any, mn: Any = None, mx: Any = None) -> tuple[Any, Any]:
    dt = np.dtype(dtype_like)
    if dt.kind in "fc":
        dt_mn, dt_mx = (-np.inf, np.inf)
    elif dt.kind in "iu":
        info = np.iinfo(dt)
        dt_mn, dt_mx = (info.min, info.max)
    else:
        raise ValueError("unknown dtype")
    return dt_mn if mn is None else mn, dt_mx if mx is None else mx


def _better_float_of(first: Any, second: Any, default: Any = np.float32) -> Any:
    first = np.dtype(first)
    second = np.dtype(second)
    default = np.dtype(default).type
    if issubclass(first.type, np.floating):
        if issubclass(second.type, np.floating) and first.itemsize < second.itemsize:
            return second.type
        return first.type
    if issubclass(second.type, np.floating):
        return second.type
    return default


def _matching_float(np_type: Any) -> Any:
    dtype = np.dtype(np_type)
    if dtype.kind not in "cf":
        raise ValueError("Expecting float or complex type as input")
    if issubclass(dtype.type, np.floating):
        return dtype.type
    return {8: np.float32, 16: np.float64}[dtype.itemsize]


def _working_type(in_type: Any, slope: Any = 1.0, inter: Any = 0.0) -> Any:
    val = np.array([1], dtype=in_type)
    if inter != 0:
        val = val + np.array([0], dtype=np.array(inter).dtype)
    if slope != 1:
        val = val / np.array([1], dtype=np.array(slope).dtype)
    return val.dtype.type


def _ftype4scaled_finite(tst_arr: np.ndarray, slope: Any, inter: Any, default: Any) -> Any:
    """``_ftype4scaled_finite(..., 'write', default)``; ``"longdouble"`` stands for nibabel's
    last choice (never reached: a finite float32 slope and intercept keep float64 finite)."""
    order = list(_OK_FLOATS_FROM_F32)
    start = order.index(default) if default in order else 0
    tst_arr = np.atleast_1d(tst_arr)
    slope = np.atleast_1d(slope)
    inter = np.atleast_1d(inter)
    for ftype in order[start:]:
        if ftype == "longdouble":
            return ftype
        tst_trans = tst_arr.copy()
        s = slope.astype(ftype)
        i = inter.astype(ftype)
        with np.errstate(over="raise"):
            try:
                if i != 0.0:
                    tst_trans = tst_trans - i
                if s != 1.0:
                    tst_trans = tst_trans / s
            except FloatingPointError:
                continue
        if np.all(np.isfinite(tst_trans)):
            return ftype
    raise ValueError("Overflow using highest floating point type")


def _best_write_scale_ftype(arr: np.ndarray, slope: Any, inter: Any, default: Any) -> Any:
    default = _better_float_of(arr.dtype.type, default)
    if not np.all(np.isfinite(arr)):
        return default
    try:
        return _ftype4scaled_finite(arr, slope, inter, default)
    except ValueError:
        return "longdouble"


# ------------------------------------------------------------------------------------------------
# nibabel/arraywriters.py: SlopeInterArrayWriter (NIfTI-1 and NIfTI-2 have slope and inter)


class _SlopeInterWriter:
    """``make_array_writer(data, out_type, True, True)``: the scaling nibabel chooses."""

    def __init__(self, array: np.ndarray, out_dtype: Any, nan2zero: bool = True) -> None:
        self.array = np.asanyarray(array)
        self.out_dtype = np.dtype(out_dtype)
        self.scaler = np.dtype(np.float32)
        self.nan2zero = nan2zero
        self._range: tuple[Any, Any] | None = None
        self._has_nan: bool | None = None
        self.slope = np.float32(1.0)
        self.inter = np.float32(0.0)
        if self._scaling_needed():
            self._do_scaling()

    # ArrayWriter / SlopeArrayWriter --------------------------------------------------------
    def finite_range(self) -> tuple[Any, Any]:
        if self._range is None:
            mn, mx, has_nan = _finite_range(self.array)
            self._range = (mn, mx)
            self._has_nan = has_nan
        return self._range

    @property
    def has_nan(self) -> bool:
        if self._has_nan is None:
            if self.array.dtype.kind in "fc":
                self.finite_range()
            else:
                self._has_nan = False
        return bool(self._has_nan)

    def _set_slope(self, val: Any) -> None:
        self.slope = self._scaler_value(val)

    def _set_inter(self, val: Any) -> None:
        self.inter = self._scaler_value(val)

    def _scaler_value(self, val: Any) -> np.float32:
        if isinstance(val, _Extended):
            return val.to_float32()
        return np.squeeze(self.scaler.type(val))

    def _scaling_needed(self) -> bool:
        data = self.array
        arr_dtype = data.dtype
        out_dtype = self.out_dtype
        if "V" in (arr_dtype.kind, out_dtype.kind):
            if arr_dtype == out_dtype:
                return False
            raise WriterError("Cannot cast to or from non-numeric types")
        if np.can_cast(arr_dtype, out_dtype):
            return False
        if out_dtype.kind == "c":
            return False
        if arr_dtype.kind == "c":
            raise WriterError("Cannot cast complex types to non-complex")
        if out_dtype.kind == "f":
            return False
        if data.size == 0:
            return False
        mn, mx = self.finite_range()
        if (mn, mx) == (0, 0):
            return False
        if arr_dtype.kind == "f":
            needed = True
        else:
            info = np.iinfo(out_dtype)
            needed = not (int(mn) >= int(info.min) and int(mx) <= int(info.max))
        # SlopeArrayWriter: no scaling without a finite value
        return needed and (mn, mx) != (np.inf, -np.inf)

    def _do_scaling(self) -> None:
        arr = self.array
        out_dtype = self.out_dtype
        if out_dtype.kind not in "iu":
            raise AssertionError
        mn, mx = self.finite_range()
        if arr.dtype.kind == "f":
            if self.nan2zero and self.has_nan:
                mn = min(mn, 0)
                mx = max(mx, 0)
            self._range_scale(mn, mx)
            return
        info = np.iinfo(out_dtype)
        if int(mx) <= int(info.max) and int(mn) >= int(info.min):
            return
        self._iu2iu()

    def _iu2iu(self) -> None:
        # SlopeInterArrayWriter._iu2iu
        mn, mx = (int(v) for v in self.finite_range())
        out_dtype = self.out_dtype
        o_min, o_max = (int(v) for v in _shared_range(self.scaler, out_dtype))
        type_range = o_max - o_min
        mn2mx = mx - mn
        if mn2mx <= type_range:
            if o_min == 0:
                inter = _floor_exact(mn - o_min, self.scaler)
            else:
                midpoint = mn + int(np.ceil(mn2mx / 2.0))
                inter = _floor_exact(midpoint, self.scaler)
            int_inter = int(inter)
            if not mn - int_inter >= o_min:
                raise AssertionError
            if mx - int_inter <= o_max:
                self._set_inter(inter)
                return
        # SlopeArrayWriter._iu2iu
        fmn, fmx = self.finite_range()
        if out_dtype.kind == "u":
            _, s_max = _shared_range(self.scaler, out_dtype)
            if fmx <= 0 and _int_abs(fmn) <= int(s_max):
                self._set_slope(-1.0)
                return
        self._range_scale(fmn, fmx)

    def _range_scale(self, in_min: Any, in_max: Any) -> None:
        # SlopeInterArrayWriter._range_scale, best_float() = x87 extended (see _Extended)
        if in_max == in_min:
            self._set_slope(1.0)
            self._set_inter(in_min)
            return
        in_dtype = self.array.dtype
        out_dtype = self.out_dtype
        if in_dtype.kind == "f":
            x_min, x_max = _Extended.of(in_min), _Extended.of(in_max)
            in_range = x_max - x_min
        else:
            i_min, i_max = int(in_min), int(in_max)
            in_range = _Extended.of(i_max - i_min)
            x_min, x_max = _Extended.of(i_min), _Extended.of(i_max)
        o_min, o_max = _shared_range(self.scaler, out_dtype)
        out_min, out_max = _Extended.of(o_min), _Extended.of(o_max)
        out_range = out_max - out_min
        slope = in_range / out_range
        if out_min.q == 0 and x_max.abs_lt(x_min):
            inter = x_max + out_min * slope
            slope = -slope
        else:
            inter = x_min - out_min * slope
        self._set_inter(inter)
        self._set_slope(slope)
        if not np.all(np.isfinite([self.slope, self.inter])):
            raise ScalingError("Slope / inter not both finite")
        if not ((x_min.q == 0 or x_max.q == 0) and self.nan2zero and self.has_nan):
            return
        nan_fill_f = -self.inter / self.slope
        nan_fill_i = np.rint(nan_fill_f)
        info = np.iinfo(out_dtype)
        # nibabel: the value survives the cast to the stored type (compared exactly: a Python
        # int beside a float32 would be rounded to float32 first)
        if int(info.min) <= float(nan_fill_i) <= int(info.max):
            return
        clipped = min(max(Fraction(float(nan_fill_f)), out_min.q), out_max.q)
        neg = _Extended(-clipped, clipped == 0 and not math.copysign(1.0, float(nan_fill_f)) < 0)
        self._set_inter(neg * _Extended.of(self.slope))
        nan_fill_i = np.rint(-self.inter / self.slope)
        if not int(info.min) <= float(nan_fill_i) <= int(info.max):
            raise AssertionError

    def _writing_range(self) -> tuple[Any, Any]:
        if self.out_dtype.kind in "iu" and self.array.dtype.kind == "f":
            mn, mx = self.finite_range()
            if (mn, mx) == (np.inf, -np.inf):
                mn, mx = 0, 0
            return mn, mx
        return None, None

    def needs_nan2zero(self) -> bool:
        return (
            self.nan2zero
            and self.array.dtype.kind in "fc"
            and self.out_dtype.kind in "iu"
            and self.has_nan
        )


# ------------------------------------------------------------------------------------------------
# nibabel/volumeutils.py: array_to_file's arithmetic (the stored values, not the file)


def _array_to_stored(
    data: np.ndarray,
    out_dtype: np.dtype,
    intercept: Any,
    divslope: Any,
    mn: Any,
    mx: Any,
    nan2zero: bool,
) -> np.ndarray:
    if not np.isfinite(np.array((intercept, 1.0 if divslope is None else divslope))).all():
        raise ValueError("divslope and intercept must be finite")
    if divslope == 0:
        raise ValueError("divslope cannot be zero")
    in_dtype = data.dtype
    if divslope is None or (mn, mx) == (0, 0) or ((mn is not None and mx is not None) and mx < mn):
        return np.zeros(data.shape, dtype=out_dtype)
    pre_clips = None if (mn is None and mx is None) else (mn, mx)
    null_scaling = intercept == 0 and divslope == 1
    if in_dtype.type == np.void:
        raise WriterError("Cannot write non-numeric types")
    if pre_clips is not None:
        pre_clips = _dt_min_max(in_dtype, *pre_clips)
    if null_scaling and np.can_cast(in_dtype, out_dtype):
        return _write_data(data, out_dtype, pre_clips=pre_clips)
    slope, inter = (np.atleast_1d(v) for v in (divslope, intercept))
    if slope.dtype.kind in "iu":
        slope = slope.astype(float)
    if inter.dtype.kind in "iu":
        inter = inter.astype(float)
    in_kind = in_dtype.kind
    out_kind = out_dtype.kind
    if out_kind in "fc":
        return _write_data(data, out_dtype, slope=slope, inter=inter, pre_clips=pre_clips)
    if in_kind in "iu":
        if null_scaling:
            lo, hi = _dt_min_max(in_dtype, mn, mx)
            mn_out, mx_out = _dt_min_max(out_dtype)
            return _write_data(data, out_dtype, pre_clips=(max(lo, mn_out), min(hi, mx_out)))
        nan2zero = False
    slope, inter = (v.astype(_matching_float(v.dtype)) for v in (slope, inter))
    cast_in_dtype = in_dtype
    if in_kind == "c":
        cast_in_dtype = np.dtype(_matching_float(in_dtype))
    elif in_kind == "f" and in_dtype.itemsize == 2:
        cast_in_dtype = np.dtype(np.float32)
    w_type = _working_type(cast_in_dtype, slope, inter)
    dt_mnmx = _dt_min_max(cast_in_dtype, mn, mx)
    extremes = np.array(dt_mnmx, dtype=cast_in_dtype)
    w_type = _best_write_scale_ftype(extremes, slope, inter, w_type)
    if w_type == "longdouble":
        raise NotImplementedError(
            "nibabel would scale these values in long double precision; not ported"
        )
    slope, inter = (v.astype(w_type) for v in (slope, inter))
    specials = np.array((*dt_mnmx, 0), dtype=w_type)
    if inter != 0.0:
        specials = specials - inter
    if slope != 1.0:
        specials = specials / slope
    post_mn, post_mx, nan_fill = np.rint(specials)
    if post_mn > post_mx:
        post_mn, post_mx = post_mx, post_mn
    both_mn, both_mx = _shared_range(w_type, out_dtype)
    if nan2zero and not both_mn <= nan_fill <= both_mx:
        est_err = np.round(2 * np.finfo(w_type).eps * abs(inter / slope))
        if (nan_fill < both_mn and abs(nan_fill - both_mn) < est_err) or (
            nan_fill > both_mx and abs(nan_fill - both_mx) < est_err
        ):
            nan_fill = np.clip(nan_fill, both_mn, both_mx)
        else:
            raise ValueError(
                f"nan_fill == {nan_fill}, outside safe int range "
                f"({int(both_mn)}-{int(both_mx)}); "
                "change scaling or set nan2zero=False?"
            )
    post_mn = np.max([post_mn, both_mn])
    post_mx = np.min([post_mx, both_mx])
    in_cast = None if cast_in_dtype == in_dtype else cast_in_dtype
    return _write_data(
        data,
        out_dtype,
        in_cast=in_cast,
        inter=inter,
        slope=slope,
        post_clips=(post_mn, post_mx),
        nan_fill=nan_fill if nan2zero else None,
    )


def _write_data(
    data: np.ndarray,
    out_dtype: np.dtype,
    in_cast: Any = None,
    pre_clips: Any = None,
    inter: Any = 0.0,
    slope: Any = 1.0,
    post_clips: Any = None,
    nan_fill: Any = None,
) -> np.ndarray:
    """``_write_data``'s values, on the whole array (nibabel's slices are only a memory
    measure: every element is computed alone)."""
    d = data
    if pre_clips is not None:
        d = np.clip(d, *pre_clips)
    if in_cast is not None:
        d = d.astype(in_cast)
    if inter != 0.0:
        d = d - inter
    if slope != 1.0:
        d = d / slope
    if post_clips is not None:
        d = np.clip(np.rint(d), *post_clips)
    if nan_fill is not None:
        nans = np.isnan(d)
        if np.any(nans):
            if d is data:
                d = d.copy()
            d[nans] = nan_fill
    if d.dtype != out_dtype:
        d = d.astype(out_dtype)
    return d


def nibabel_stored(data: Any, out_dtype: Any) -> tuple[np.ndarray, float, float]:
    """The values nibabel stores for ``data`` written as ``out_dtype``, with the
    ``scl_slope`` and ``scl_inter`` it writes (float32 values, as nibabel's writer keeps
    them): ``(stored, slope, inter)``. Raises :class:`WriterError` (or ``ScalingError``) and
    :class:`HeaderDataError` where nibabel does."""
    arr = np.asarray(data)
    out = np.dtype(out_dtype)
    writer = _SlopeInterWriter(arr, out)
    # to_file_map sets the header's scaling (Nifti1Header.set_slope_inter) before the data
    if writer.slope in (0, np.inf, -np.inf):
        raise HeaderDataError("Slope cannot be 0 or infinite")
    if writer.inter in (np.inf, -np.inf):
        raise HeaderDataError("Intercept cannot be infinite")
    mn, mx = writer._writing_range()
    stored = _array_to_stored(
        writer.array, out, writer.inter, writer.slope, mn, mx, writer.needs_nan2zero()
    )
    return stored, float(writer.slope), float(writer.inter)


def save_nibabel(
    image: Any,
    path: str | os.PathLike[str],
    *,
    dtype: Any = None,
    version: int | None = None,
    compression_level: int = 2,
    n_threads: int = 1,
) -> None:
    """Write ``image`` as nibabel's ``img.to_filename(path)`` writes it: the voxel data in the
    header's data type (``dtype`` overrides it; without a header, the array's type), scaled
    by nibabel's rules when the values do not fit it, with the ``scl_slope``/``scl_inter``
    nibabel writes, and the header nibabel's ``update_header`` makes for the image's affine
    and shape."""
    from larmorx.image import as_image

    img = as_image(image)
    data = np.asanyarray(img.data)
    if data.dtype == np.bool_:
        data = data.astype(np.uint8)
    header = img.header
    if dtype is not None:
        out = np.dtype(dtype)
    elif header is not None and header.data_dtype is not None:
        out = np.dtype(header.data_dtype)
    else:
        out = data.dtype
    stored, slope, inter = nibabel_stored(data, out.newbyteorder("="))
    template = header.to_dict() if header is not None else None
    fields = _core.nifti_header_for_image(
        list(stored.shape),
        stored.dtype.name,
        np.asarray(img.affine, dtype=np.float64),
        template,
        version,
    )
    fields["scl_slope"] = slope
    fields["scl_inter"] = inter
    _core.nifti_write(os.fspath(path), stored, fields, int(compression_level), int(n_threads))

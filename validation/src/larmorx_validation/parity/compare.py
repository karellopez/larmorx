# SPDX-License-Identifier: Apache-2.0
"""Array comparisons for parity checks.

``identical`` is the strictest test: same shape, same dtype, and the same value in every
element, where NaN equals NaN (any payload) and -0.0 differs from +0.0.
"""

from __future__ import annotations

import numpy as np

from larmorx_validation.parity.harness import Check


def _canonical_bits(a: np.ndarray) -> np.ndarray:
    """The bit patterns of a float/complex array with every NaN replaced by one canonical NaN."""
    if a.dtype.kind == "c":
        part = np.dtype(f"f{a.dtype.itemsize // 2}")
        a = a.view(part)
    a = np.where(np.isnan(a), np.array(np.nan, dtype=a.dtype), a)
    return a.view(f"u{a.dtype.itemsize}")


def _native(a: np.ndarray) -> np.ndarray:
    return a if a.dtype.isnative or a.dtype.kind == "V" else a.astype(a.dtype.newbyteorder("="))


def identical(name: str, actual, expected, *, byte_order: bool = False) -> Check:
    """Same shape, dtype and values (NaN == NaN; -0.0 != +0.0). The byte order of the dtypes is
    ignored unless ``byte_order`` is set (the values are still compared exactly)."""
    a, e = np.asarray(actual), np.asarray(expected)
    if not byte_order:
        a, e = _native(a), _native(e)
    if a.shape != e.shape:
        return Check(name, False, detail=f"shape {a.shape} != {e.shape}")
    if a.dtype != e.dtype:
        return Check(name, False, detail=f"dtype {a.dtype} != {e.dtype}")
    if a.dtype.kind in "fc":
        diff = _canonical_bits(np.ascontiguousarray(a)) != _canonical_bits(np.ascontiguousarray(e))
    elif a.dtype.kind == "V":
        diff = a.view(np.uint8) != e.view(np.uint8)
    else:
        diff = a != e
    n = int(np.count_nonzero(diff))
    if n == 0:
        return Check(name, True, metric="identical", value=0, threshold=0)
    detail = f"{n} of {a.size} elements differ"
    if a.dtype.kind in "iuf":
        detail += f"; max |diff| = {max_abs_diff(a, e):.3g}"
    return Check(name, False, metric="elements differing", value=n, threshold=0, detail=detail)


def max_abs_diff(actual, expected) -> float:
    """Largest absolute difference, ignoring positions where both are NaN."""
    a = (
        np.asarray(actual, dtype=np.float64)
        if np.asarray(actual).dtype.kind != "c"
        else np.asarray(actual)
    )
    e = (
        np.asarray(expected, dtype=np.float64)
        if np.asarray(expected).dtype.kind != "c"
        else np.asarray(expected)
    )
    both_nan = np.isnan(a) & np.isnan(e)
    with np.errstate(invalid="ignore"):
        d = np.abs(a - e)
    d = np.where(both_nan, 0.0, d)
    d = np.where(np.isnan(d), np.inf, d)  # NaN on one side only, or inf - inf
    return float(d.max()) if d.size else 0.0


def close(name: str, actual, expected, atol: float) -> Check:
    """Element-wise ``|actual - expected| <= atol`` (NaN matches NaN)."""
    a, e = np.asarray(actual), np.asarray(expected)
    if a.shape != e.shape:
        return Check(name, False, detail=f"shape {a.shape} != {e.shape}")
    d = max_abs_diff(a, e)
    return Check(name, d <= atol, metric="max |diff|", value=d, threshold=atol)


def equal(name: str, actual, expected) -> Check:
    """Plain equality for scalars, tuples and bytes (NaN == NaN for floats)."""

    def norm(v):
        if isinstance(v, float) and v != v:
            return "nan"
        if isinstance(v, (tuple, list)):
            return tuple(norm(x) for x in v)
        if isinstance(v, np.generic):
            return norm(v.item())
        return v

    ok = norm(actual) == norm(expected)
    return Check(name, ok, detail="" if ok else f"{actual!r} != {expected!r}")

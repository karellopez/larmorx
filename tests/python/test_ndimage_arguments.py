# SPDX-License-Identifier: Apache-2.0
"""``lx.ndimage``'s minimum, maximum and rank filters take their arguments as SciPy 1.18.1 does
(relay 21 of the field maps' part B): its checks in its order, its C parsers' conversions and
messages, the 1-D running rank's own type dispatch, and SciPy's defects SCI-21 to SCI-23 of that
path in both modes; a ``numpy.longdouble`` sigma for the Gaussian filters. Every expected text
and value is SciPy's (``check_ndimage.py --sections arguments``, ``check_regrid.py``)."""

from __future__ import annotations

import warnings

import numpy as np
import pytest

import larmorx as lx
from larmorx import ndimage as nd

LINE = np.array([3.0, 1, 4, 1, 5, 9, 2, 6, 5])
MEDIAN = [3.0, 3, 1, 4, 5, 5, 6, 5, 5]
BASE = np.arange(12.0).reshape(3, 4) % 5


def outcome(fn, *args, **kwargs):
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        out = fn(*args, **kwargs)
    ids = [x.message.entry for x in w if isinstance(x.message, lx.UpstreamDefectWarning)]
    other = [(x.category, str(x.message)) for x in w if x.category is not lx.UpstreamDefectWarning]
    return out, ids, other


@pytest.mark.parametrize(
    ("call", "error", "message"),
    [
        # NI_InitLineBuffer (the 1-D filters, and the separable boxes through them)
        (lambda: nd.minimum_filter1d(BASE.astype(np.float16), 3), RuntimeError,
         "array type dtype('float16') not supported"),
        (lambda: nd.maximum_filter(BASE.astype(object), 3), RuntimeError,
         "array type dtype('O') not supported"),
        # the n-dimensional C filters' type switch
        (lambda: nd.maximum_filter(BASE.astype(np.float16), footprint=[[1, 0, 1]]), RuntimeError,
         "array type not supported"),
        # _get_output
        (lambda: nd.median_filter(BASE, 3, output="bool"), RuntimeError,
         "output must have numeric dtype"),
        (lambda: nd.minimum_filter1d(BASE.astype("U3"), 3), TypeError,
         "data type 'str96' not understood"),
        # NI_ObjectToOutputArray
        (lambda: nd.grey_erosion(BASE, size=2, output=_read_only(BASE.shape)), ValueError,
         "output array is read-only."),
        # PyArg_ParseTuple's "d" (cval) and "n"/"i" (sizes, axes, origins)
        (lambda: nd.rank_filter(BASE, 1, 3, mode="constant", cval="x"), TypeError,
         "must be real number, not str"),
        (lambda: nd.minimum_filter(BASE, 3, mode="constant", cval=np.str_("1")), TypeError,
         "must be real number, not numpy.str_"),
        (lambda: nd.minimum_filter1d(LINE, 3.0), TypeError,
         "'float' object cannot be interpreted as an integer"),
        (lambda: nd.minimum_filter(BASE, 3, origin=0.0), TypeError,
         "'float' object cannot be interpreted as an integer"),
        # numpy's PyArray_IntpConverter (the n-dimensional filters' origins)
        (lambda: nd.median_filter(BASE, 3, origin=np.True_), TypeError, "an integer is required"),
        # the 1-D running rank
        (lambda: nd.median_filter(LINE, 3, output=_read_only(LINE.shape)), ValueError,
         "WRITEBACKIFCOPY base is read-only"),
        (lambda: nd.median_filter(LINE.astype(np.int8), 3, mode="constant", cval=np.array([2.0])),
         TypeError, "only integer scalar arrays can be converted to a scalar index"),
    ],
)  # fmt: skip
def test_scipy_refusals(call, error, message):
    with pytest.raises(error) as e:
        call()
    assert str(e.value) == message


def _read_only(shape):
    a = np.zeros(shape)
    a.flags.writeable = False
    return a


def test_scipy_acceptances():
    """What SciPy takes that larmorx refused before: a float16 1-D running rank (computed in
    float32), the separable path with nothing to filter (numpy's assignment of any type), bool
    origins of the 1-D C parsers, a list as the output."""
    out = nd.median_filter(LINE.astype(np.float16), 3)
    assert out.dtype == np.float16 and out.tolist() == MEDIAN
    obj = nd.minimum_filter(BASE.astype(object), 1)
    assert obj.dtype == object and obj.tolist() == BASE.tolist()
    assert nd.minimum_filter(BASE, 1, cval="x").tolist() == BASE.tolist()
    assert nd.minimum_filter1d(LINE, 3, origin=True).tolist() == [1.0] * 6 + [2.0] * 3
    listed = nd.median_filter(LINE, 3, output=[0.0] * 9)
    assert isinstance(listed, np.ndarray) and listed.tolist() == MEDIAN
    assert nd.median_filter(np.zeros(0, np.float16), 3).dtype == np.float16
    empty = nd.minimum_filter(np.zeros((0, 3), np.float16), footprint=[[1, 0, 1]])
    assert empty.dtype == np.float16 and empty.shape == (0, 3)


def test_sci21_both_modes():
    """A float32 input into a float64 output: SciPy writes the float32 bytes into the first half
    of the output; corrected, the values cast."""
    x = LINE.astype(np.float32)
    fixed, ids, _ = outcome(nd.median_filter, x, 3, output=np.float64)
    assert ids == ["SCI-21"] and fixed.dtype == np.float64 and fixed.tolist() == MEDIAN
    raw, ids, _ = outcome(nd.median_filter, x, 3, output=np.float64, upstream_defects="reproduce")
    assert ids == ["SCI-21"]
    want = np.zeros(9)
    want.view(np.uint8)[:36] = np.array(MEDIAN, np.float32).view(np.uint8)
    assert raw.tobytes() == want.tobytes()
    # float64 into float32: SciPy writes 36 bytes past the output's end; larmorx only its own
    small, ids, _ = outcome(nd.rank_filter, LINE, 1, 3, output=np.float32, upstream_defects="reproduce")  # fmt: skip
    assert ids == ["SCI-21"]
    assert small.tobytes() == np.array(MEDIAN, np.float64).tobytes()[:36]
    # the same type: no notice
    assert outcome(nd.median_filter, LINE, 3, output=np.float64)[1] == []


def test_sci22_both_modes():
    for dt in (">f8", ">f4", np.longlong):
        x = LINE.astype(dt)
        fixed, ids, _ = outcome(nd.median_filter, x, 3)
        assert ids == ["SCI-22"] and fixed.tolist() == MEDIAN
        error = TypeError if dt is np.longlong and np.dtype(np.longlong).num != np.dtype(np.int64).num else RuntimeError  # fmt: skip
        if dt is np.longlong and np.dtype(np.longlong).num == np.dtype(np.int64).num:
            continue  # numpy's longlong is its int64 here (Windows): SciPy takes it
        with pytest.raises(error, match=r"^Unsupported array type$"):
            nd.median_filter(x, 3, upstream_defects="reproduce")


def test_sci22_long_double():
    """numpy's long double: where it has 8 bytes (Windows, macOS on Apple silicon) SciPy's Python
    takes it as float64 and its C++ refuses its type number (rule A: filtered, with a notice);
    where it is wider (x86-64: 80 bits; Linux aarch64: 128) SciPy's Python refuses it, in both
    modes."""
    x = LINE.astype(np.longdouble)
    if np.dtype(np.longdouble).itemsize == 8:
        fixed, ids, _ = outcome(nd.median_filter, x, 3)
        assert ids == ["SCI-22"] and fixed.tolist() == MEDIAN
        with pytest.raises(TypeError, match=r"^Unsupported array type$"):
            nd.median_filter(x, 3, upstream_defects="reproduce")
    else:
        for mode in ("fix", "reproduce"):
            with pytest.raises(RuntimeError, match=r"^Unsupported array type$"):
                nd.median_filter(x, 3, upstream_defects=mode)


def test_sci23_both_modes():
    o1, o2 = np.zeros((9, 2))[:, 0], np.zeros((9, 2))[:, 0]
    _, ids, other = outcome(nd.median_filter, LINE, 3, output=o1)
    assert ids == [] and other == [] and o1.tolist() == MEDIAN
    _, ids, other = outcome(nd.median_filter, LINE, 3, output=o2, upstream_defects="reproduce")
    assert o2.tolist() == MEDIAN and len(other) == 1
    category, text = other[0]
    assert category is RuntimeWarning and text.startswith("WRITEBACKIFCOPY detected in ")
    assert text.endswith(
        ".  Required call to PyArray_ResolveWritebackIfCopy or PyArray_DiscardWritebackIfCopy "
        "is missing."
    )


def test_longdouble_sigma():
    """The kernel from the sigma as float64 on every platform, with a warning where numpy's
    long double is wider than float64 (x86-64: 80-bit; Linux aarch64: 128-bit)."""
    a = (np.arange(60.0).reshape(5, 12) % 7) / 3
    want = nd.gaussian_filter(a, 1.3, (0, 1))
    got, _, other = outcome(nd.gaussian_filter, a, np.longdouble(1.3), (0, 1))
    assert got.tobytes() == want.tobytes()
    wide = np.dtype(np.longdouble).itemsize > 8
    assert len(other) == (2 if wide else 0)  # one per pass (each axis's kernel)
    assert all(c is UserWarning and "as float64" in t for c, t in other)

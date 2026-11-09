# SPDX-License-Identifier: Apache-2.0
"""``lx.ndimage``'s binary morphology, ``label``, ``sum_labels``, ``center_of_mass`` and
``watershed_ift`` take their arguments as SciPy 1.18.1 does (relay 22 of the field maps' part
B): its checks in its order, its C parsers' conversions and messages, ``label``'s numbering
along its output's axis of the smallest stride, and SciPy's defects SCI-25 to SCI-30 in both
modes. Every expected text and value is SciPy's (``check_ndimage.py --sections arguments``,
``relay22/morph_cases.py``)."""

from __future__ import annotations

import warnings

import numpy as np
import pytest

import larmorx as lx
from larmorx import ndimage as nd

A = np.zeros((5, 6), bool)
A[1:5, 1:6] = True
A[2, 3] = False
A[0, 0] = True
A[4, 1] = False
EROSION = [[0] * 6, [0] * 6, [0] * 6, [0, 0, 1, 0, 1, 0], [0] * 6]
LABELS = [[1, 0, 0, 0, 0, 0], [0, 2, 2, 2, 2, 2], [0, 2, 2, 0, 2, 2], [0, 2, 2, 2, 2, 2],
          [0, 0, 2, 2, 2, 2]]  # fmt: skip
GROUPS = np.arange(30).reshape(5, 6) % 3
VALUES = np.arange(30.0).reshape(5, 6) * 0.25


def outcome(fn, *args, **kwargs):
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        out = fn(*args, **kwargs)
    ids = [x.message.entry for x in w if isinstance(x.message, lx.UpstreamDefectWarning)]
    other = [(x.category, str(x.message)) for x in w if x.category is not lx.UpstreamDefectWarning]
    return out, ids, other


def _read_only(shape, dtype=bool):
    a = np.zeros(shape, dtype)
    a.flags.writeable = False
    return a


@pytest.mark.parametrize(
    ("call", "error", "message"),
    [
        # NI_BinaryErosion's type switches (the input's, the mask's, the output's)
        (lambda: nd.binary_erosion(A.astype(np.float16)), RuntimeError, "data type not supported"),
        (lambda: nd.binary_erosion(A, mask=A.astype(object)), RuntimeError,
         "data type not supported"),
        (lambda: nd.binary_dilation(A, output=np.zeros(A.shape, np.float16)), RuntimeError,
         "data type not supported"),
        # Py_BinaryErosion's parser: the output's write access, border_value as a C int
        (lambda: nd.binary_erosion(A, output=_read_only(A.shape)), ValueError,
         "output array is read-only."),
        (lambda: nd.binary_erosion(A, border_value=2.5), TypeError,
         "'float' object cannot be interpreted as an integer"),
        (lambda: nd.binary_erosion(A, border_value=2**31), OverflowError,
         "signed integer is greater than maximum"),
        # _center_is_true's numpy indexing, then PyArray_IntpConverter
        (lambda: nd.binary_erosion(A, origin=0.0), IndexError, "only integers, slices"),
        (lambda: nd.binary_erosion(A, origin=True), TypeError, "an integer is required"),
        # iterations - 1 as a C int (the propagation strategy's second call)
        (lambda: nd.binary_erosion(A, iterations=-(2**31)), OverflowError,
         "signed integer is less than minimum"),
        # _ni_label's fused types
        (lambda: nd.label(A.astype(np.float16)), TypeError, "No matching signature found"),
        (lambda: nd.label(A, output=np.complex64), TypeError, "No matching signature found"),
        # numpy's bincount of a long double input
        (lambda: nd.sum_labels(VALUES.astype(np.longdouble), GROUPS, [1]), TypeError,
         "Cannot cast array data from dtype('float128') to dtype('float64')"),
        # Py_WatershedIFT's parser
        (lambda: nd.watershed_ift(A.astype(np.uint8), A.astype(np.int32),
                                  output=_read_only(A.shape, np.int32)),
         ValueError, "output array is read-only."),
    ],
)  # fmt: skip
def test_scipy_refusals(call, error, message):
    with pytest.raises(error) as e:
        call()
    assert str(e.value).startswith(message)


def test_scipy_acceptances():
    # an output that is not an array is ignored: a new bool array
    out = nd.binary_erosion(A, output=np.float32)
    assert out.dtype == np.bool_ and out.astype(int).tolist() == EROSION
    assert nd.binary_erosion(A, output="x").dtype == np.bool_
    # longlong inputs, a 0-d input, an empty input of a type the C code does not take
    assert nd.binary_erosion(A.astype(np.longlong)).astype(int).tolist() == EROSION
    assert nd.binary_erosion(np.array(True)).tolist() is True
    assert nd.binary_erosion(np.zeros((0, 3), np.float16)).shape == (0, 3)
    # into an output array: hit-or-miss and hole filling return None, as SciPy
    o = np.zeros(A.shape, np.int32)
    assert nd.binary_fill_holes(A, output=o) is None
    assert o[2, 3] == 1 and o.sum() == A.sum() + 1
    assert nd.binary_hit_or_miss(A, output=np.zeros(A.shape, bool)) is None
    # label: longlong, a 0-d input, an empty input into a read-only output (nothing written)
    assert nd.label(A.astype(np.longlong))[0].tolist() == LABELS
    assert nd.label(np.array(3)) == (np.array(1), 1)
    assert nd.label(np.zeros((0, 3)), output=_read_only((0, 3), np.int16)) == 0


def test_label_numbers_along_the_output_axis():
    # SciPy's lines run along the output's axis of the smallest stride (axis 0 of a Fortran
    # array): the components are numbered by their first element in that order
    x = np.array([[0, 1], [1, 0]])
    assert nd.label(x)[0].tolist() == [[0, 1], [2, 0]]
    o = np.zeros((2, 2), np.int32, order="F")
    assert nd.label(x, output=o) == 2 and o.tolist() == [[0, 2], [1, 0]]


def test_sci25_both_modes():
    x, m = np.ones(5, bool), np.array([1, 1, 1, 0, 0], bool)
    o = np.zeros(5)
    _, ids, _ = outcome(nd.binary_erosion, x, iterations=4, mask=m, output=o)
    assert ids == ["SCI-25"] and o.tolist() == [0, 0, 0, 1, 1]
    o = np.zeros(5)
    _, ids, _ = outcome(
        nd.binary_erosion, x, iterations=4, mask=m, output=o, upstream_defects="reproduce"
    )
    assert ids == ["SCI-25"] and o.tolist() == [0, 0, 0, 0, 1]
    # into a bool output nothing changes: no notice
    out, ids, _ = outcome(nd.binary_erosion, x, iterations=4, mask=m)
    assert ids == [] and out.astype(int).tolist() == [0, 0, 0, 1, 1]


def test_sci26_both_modes():
    want = nd.binary_propagation(A)
    o = np.zeros(A.shape, ">i4")
    _, ids, other = outcome(nd.binary_propagation, A, output=o)
    assert not ids and not other and o.astype(bool).tolist() == want.tolist()
    o = np.zeros(A.shape, ">i4")
    _, ids, other = outcome(nd.binary_propagation, A, output=o, upstream_defects="reproduce")
    assert not ids and o.astype(bool).tolist() == want.tolist()
    assert [c for c, _ in other] == [RuntimeWarning] and "WRITEBACKIFCOPY" in other[0][1]


def test_sci27_both_modes():
    (labels, n), ids, _ = outcome(nd.label, A.astype(">f8"))
    assert ids == ["SCI-27"] and n == 2 and labels.tolist() == LABELS
    with pytest.raises(ValueError, match="Big-endian buffer not supported"):
        nd.label(A.astype(">f8"), upstream_defects="reproduce")


def test_sci28_both_modes():
    o = _read_only(A.shape, np.int32)
    with pytest.raises(ValueError, match="output array is read-only"):
        nd.label(A, output=o)
    n, ids, _ = outcome(nd.label, A, output=o, upstream_defects="reproduce")
    assert ids == ["SCI-28"] and n == 2 and o.tolist() == LABELS and not o.flags.writeable


def test_sci29_both_modes():
    o = np.zeros(A.shape, bool)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        with pytest.raises(RuntimeError, match="insufficient bit-depth"):
            nd.label(A, output=o)
    assert [x.message.entry for x in w] == ["SCI-29"]
    o = np.zeros(A.shape, bool)
    n, ids, _ = outcome(nd.label, A, output=o, upstream_defects="reproduce")
    assert ids == ["SCI-29"] and n == 2 and o.view(np.uint8).tolist() == LABELS


def test_sci30_both_modes():
    total, ids, _ = outcome(nd.sum_labels, VALUES, GROUPS, np.array(2))
    assert ids == ["SCI-30"] and total == 38.75 and np.isscalar(total)
    with pytest.raises(TypeError, match="does not support item assignment"):
        nd.sum_labels(VALUES, GROUPS, np.array(2), upstream_defects="reproduce")
    empty, lab = np.zeros((0, 3)), np.zeros((0, 3), int)
    sums, ids, _ = outcome(nd.sum_labels, empty, lab, [1, 2])
    assert ids == ["SCI-30"] and sums.tolist() == [0.0, 0.0]
    with pytest.raises(ValueError, match="zero-size array"):
        nd.sum_labels(empty, lab, [1, 2], upstream_defects="reproduce")
    # center_of_mass: one notice per call
    _, ids, _ = outcome(nd.center_of_mass, VALUES, GROUPS, np.array(2))
    assert ids == ["SCI-30"]

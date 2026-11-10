# SPDX-License-Identifier: Apache-2.0
"""SciPy's watershed_ift and center_of_mass as lx.ndimage's (their defects' two modes, the
wrapper's errors) and sdcflows' epi_mask as lx.sdcflows.epi_mask (its outputs, errors and
SDC-28) on small generated images; SciPy's and sdcflows' own functions against them:
validation/investigations/sdcflows-fieldmap-b/check_epi_mask.py."""

from __future__ import annotations

import os
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx._defects import UpstreamDefectWarning
from larmorx.sdcflows.masks import _uint16_levels


def _entries(caught: list[warnings.WarningMessage]) -> set[str]:
    return {w.message.entry for w in caught if isinstance(w.message, UpstreamDefectWarning)}


def test_watershed_across_an_edge():
    a = np.zeros((3, 4), np.uint8)
    a[:, 1] = 200
    m = np.zeros((3, 4), np.int64)
    m[0, 0], m[2, 3] = 1, 2
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        scipy = lx.ndimage.watershed_ift(a, m, upstream_defects="reproduce")
        fixed = lx.ndimage.watershed_ift(a, m)
    assert scipy.tolist() == [[1, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2]]
    assert fixed.tolist() == [[1, 2, 2, 2], [1, 2, 2, 2], [1, 2, 2, 2]]
    assert fixed.dtype == np.int64
    assert _entries(caught) == {"SCI-13"}


def test_watershed_queue_and_wide_markers():
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        line = np.array([1, 0, 0, 1, 1, 2], np.uint8)
        lm = np.array([-1, 0, -1, 0, 0, 0])
        assert lx.ndimage.watershed_ift(line, lm, upstream_defects="reproduce").tolist() == [
            -1, -1, -1, -1, -1, 0,
        ]  # fmt: skip
        assert lx.ndimage.watershed_ift(line, lm).tolist() == [-1] * 6
        assert _entries(caught) == {"SCI-14"}
        big = np.array([[2**32 + 5, 0], [0, 2**31]], np.int64)
        flat = np.zeros((2, 2), np.uint8)
        assert lx.ndimage.watershed_ift(flat, big, upstream_defects="reproduce").tolist() == [
            [5, 5], [5, -(2**31)],
        ]  # fmt: skip
        assert lx.ndimage.watershed_ift(flat, big).tolist() == [[2**32 + 5, 2**31], [2**31] * 2]


def test_watershed_outputs_and_errors():
    a = np.array([[0, 5], [9, 0]], np.uint8)
    m = np.array([[300, 0], [0, 128]], np.int32)
    out = np.zeros((2, 2), np.int8)
    res = lx.ndimage.watershed_ift(a, m, output=out)
    assert res is out and out.tolist() == [[44, -128], [-128, -128]]
    # a non-array output is ignored (SciPy's wrapper), the markers' type kept
    assert lx.ndimage.watershed_ift(a, m.astype(np.int16), output=np.int8).dtype == np.int16
    assert lx.ndimage.watershed_ift(np.array(3, np.uint8), np.array(-4)).tolist() == -4
    assert lx.ndimage.watershed_ift(np.zeros((0, 3), np.uint8), np.zeros((0, 3), int)).shape == (
        0, 3,
    )  # fmt: skip
    with pytest.raises(TypeError, match="only 8 and 16 unsigned"):
        lx.ndimage.watershed_ift(a.astype(np.int16), m)
    with pytest.raises(RuntimeError, match="marker should be of integer type"):
        lx.ndimage.watershed_ift(a, m.astype(np.uint8))
    with pytest.raises(RuntimeError, match="input and markers must have equal shape"):
        lx.ndimage.watershed_ift(a, np.zeros((2, 3), int))
    with pytest.raises(RuntimeError, match="structure dimensions must be equal to 3"):
        lx.ndimage.watershed_ift(a, m, structure=np.ones((3, 5), bool))
    with pytest.raises(RuntimeError, match="output should be of integer type"):
        lx.ndimage.watershed_ift(a, m, output=np.zeros((2, 2), np.uint16))
    with pytest.raises(RuntimeError, match="too many dimensions"):
        lx.ndimage.watershed_ift(np.zeros((2,) * 8, np.uint8), np.zeros((2,) * 8, int))


def test_center_of_mass():
    a = np.array([[0, 0, 0, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0]])
    assert lx.ndimage.center_of_mass(a) == (2.0, 1.5)
    b = np.array([[0, 1, 1, 0], [0, 1, 0, 0], [0, 0, 0, 0], [0, 0, 1, 1], [0, 0, 1, 1]])
    lbl = lx.ndimage.label(b)[0]
    got = lx.ndimage.center_of_mass(b, lbl, [1, 2])
    assert np.allclose(got, [(1 / 3, 4 / 3), (3.5, 2.5)], rtol=0, atol=1e-15)
    c = np.array([[-1, 0, 0, 0], [0, -1, -1, 0], [0, 1, -1, 0], [0, 1, 1, 0]])
    assert lx.ndimage.center_of_mass(c) == (-4.0, 1.0)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        assert lx.ndimage.center_of_mass(np.array([-1.0, 1.0])) == (np.inf,)
    w = (np.indices((9, 8, 7)).sum(0) % 5).astype(np.float32)
    for ver in ("2.2.6", "2.3.5", "2.5.4"):
        cm = lx.ndimage.center_of_mass(w, numpy_version=ver)
        assert all(isinstance(x, np.float64) for x in cm)


def _head(tmp: Path, name: str = "epi.nii.gz", shape=(26, 28, 20)) -> str:
    i, j, k = np.indices(shape)
    r2 = ((i - 13) / 9.0) ** 2 + ((j - 14) / 11.0) ** 2 + ((k - 10) / 7.0) ** 2
    head = np.where(r2 < 1, 600 + 40 * ((i * 7 + j * 3 + k * 5) % 13), 0) + 25 * (
        (i + 2 * j + k) % 3
    )
    path = str(tmp / name)
    lx.save((head.astype(np.float32), np.diag([-3.0, 3.0, 3.5, 1.0])), path)
    return path


def test_epi_mask(tmp_path, monkeypatch):
    epi = _head(tmp_path)
    monkeypatch.chdir(tmp_path)
    res = lx.sdcflows.epi_mask(epi, numpy_version="2.3.5")
    assert res.out_file == os.path.abspath("mask.nii.gz")
    mask = lx.io.load(res.out_file)
    assert mask.data.dtype == np.uint8 and mask.data.shape == (26, 28, 20)
    assert res.centre == (13, 14, 10)
    assert 0 < int(mask.data.sum()) < mask.data.size
    assert np.array_equal(np.asarray(mask.affine), np.diag([-3.0, 3.0, 3.5, 1.0]))
    other = lx.sdcflows.epi_mask(epi, tmp_path / "m.nii", write=False)
    assert other.out_file is None and np.array_equal(other.mask.data, res.mask.data)


def test_epi_mask_errors(tmp_path):
    four = str(tmp_path / "four.nii.gz")
    lx.save((np.ones((8, 8, 8, 2), np.float32), np.eye(4)), four)
    with pytest.raises(RuntimeError):
        lx.sdcflows.epi_mask(four, write=False)
    flat = str(tmp_path / "flat.nii.gz")
    lx.save((np.full((8, 8, 8), 7.0, np.float32), np.eye(4)), flat)
    with pytest.raises(IndexError, match="index -1 is out of bounds for axis 0 with size 0"):
        lx.sdcflows.epi_mask(flat, write=False)


def test_epi_mask_levels_sdc28():
    w = np.zeros((3, 3, 3), np.float32)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        fixed = _uint16_levels(w, "fix")
        scipy = _uint16_levels(w, "reproduce")
    assert fixed.dtype == np.uint16 and not fixed.any() and not scipy.any()
    assert _entries(caught) == {"SDC-28"}
    big = np.array([0.0, 3.0e38, 1.0e38], np.float32)  # 1e3 * x overflows: inf / max = inf
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        assert _uint16_levels(big, "fix").tolist() == [0, 65535, 65535]
        assert _uint16_levels(big, "reproduce").tolist() == [0, 0, 0]

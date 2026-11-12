# SPDX-License-Identifier: Apache-2.0
"""The evidence's comparison of two images (``larmorx_validation.evidence.core.compare``) and the
viewer's windows when an image holds NaN, a voxel without a value (tedana's RMSE map where fewer
than two echoes are good, 2026-11-12): NaN against NaN is no difference, the sizes come from the
voxels where both hold a number, and images without NaN keep exactly their earlier metrics."""

from __future__ import annotations

import math

import numpy as np
from larmorx_validation.evidence import core
from larmorx_validation.evidence import example as exm


def test_nan_against_nan_is_no_difference():
    a = np.array([[1.0, np.nan], [3.0, 4.0]], dtype=np.float32)
    b = np.array([[1.0, np.nan], [3.5, 4.0]], dtype=np.float32)
    m = core.compare(a, b)
    assert m["differ"] == 1
    assert m["differ_nan"] == 0
    assert m["nan"] == 1
    assert m["max_abs"] == 0.5
    assert m["at"] == [1, 0]
    assert (m["a_at"], m["b_at"]) == (3.0, 3.5)
    assert math.isfinite(m["rms"]) and math.isfinite(m["one_minus_ncc"])
    assert m["range"] == 3.0


def test_nan_against_a_number_counts_without_a_size():
    a = np.array([1.0, np.nan, 2.0])
    b = np.array([1.0, 5.0, 2.0])
    m = core.compare(a, b)
    assert (m["differ"], m["differ_nan"], m["max_abs"]) == (1, 1, 0.0)
    assert m["at"] == [1]


def test_identical_with_nan():
    a = np.array([np.nan, 1.0], dtype=np.float32)
    m = core.compare(a, a.copy())
    assert m["identical"] and m["differ"] == 0 and m["max_abs"] == 0.0


def test_without_nan_the_metrics_are_unchanged():
    rng = np.random.default_rng(7)
    a = rng.normal(size=(5, 6, 7)).astype(np.float32)
    b = a + rng.normal(scale=1e-3, size=a.shape).astype(np.float32)
    m = core.compare(a, b)
    assert "differ_nan" not in m
    x, y = a.astype(np.float64), b.astype(np.float64)
    d = np.abs(x - y)
    assert m["max_abs"] == float(d.max())
    assert m["rms"] == math.sqrt(float(np.mean((x - y) ** 2)))


def test_windows_leave_nan_out():
    w = exm.windows(np.array([np.nan, 0.0, 1.0, 2.0, np.nan]))
    assert all(math.isfinite(x["lo"]) and math.isfinite(x["hi"]) for x in w)
    assert w[1]["lo"] == 0.0 and w[1]["hi"] == 2.0

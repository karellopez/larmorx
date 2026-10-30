# SPDX-License-Identifier: Apache-2.0
"""The originals' own step of the Python suites (``parity.python_versions``): NaN values counted
alike in its digests, one digest per pass of the shared releases, and a comparison whose original
varies between passes reported as varying (SCI-05's kind), never as changed or identical."""

from __future__ import annotations

import numpy as np
import pytest

pv = pytest.importorskip("larmorx_validation.parity.python_versions")


def test_digest_counts_every_nan_alike():
    a = np.array([1.0, np.nan, 2.0], dtype=np.float32)
    b = a.copy()
    b.view(np.uint32)[1] = 0xFFD7FF97  # a NaN with another sign and payload (SCI-05's bits)
    assert pv.digest(a) == pv.digest(b)
    c = a.copy()
    c[2] = 3.0
    assert pv.digest(a) != pv.digest(c)
    # The type and the shape count.
    assert pv.digest(a) != pv.digest(a.astype(np.float64))
    assert pv.digest(np.zeros((2, 3))) != pv.digest(np.zeros((3, 2)))


def test_record_keeps_one_digest_per_pass(tmp_path, monkeypatch):
    monkeypatch.setenv(pv.STEP_RECORD, str(tmp_path))
    with pv.case("ndimage/nearest-order5"):
        pv.record("m", np.array([1.0, 2.0]))
        pv.record("m", np.array([1.0, 2.5]))
    monkeypatch.delenv(pv.STEP_RECORD)
    monkeypatch.setenv(pv.STEP_FROM, str(tmp_path))
    with pv.case("ndimage/nearest-order5"):
        olds = pv.previous("m")
        assert olds is not None and len(olds) == 2 and len(set(olds)) == 2
        assert pv.previous("other") is None
    with pv.case("another-case"):
        assert pv.previous("m") is None


def test_step_reports_a_varying_original(tmp_path, monkeypatch):
    from larmorx_validation.parity import resample_series as rs
    from larmorx_validation.parity.harness import CheckList

    monkeypatch.setenv(pv.STEP_RECORD, str(tmp_path))
    with pv.case("c"):
        pv.record("varying", np.array([1.0]))
        pv.record("varying", np.array([2.0]))
        pv.record("steady", np.array([1.0]))
        pv.record("steady", np.array([1.0]))
    monkeypatch.delenv(pv.STEP_RECORD)
    monkeypatch.setenv(pv.STEP_FROM, str(tmp_path))
    checks = CheckList()
    with pv.case("c"):
        rs._step(checks, "varying", np.array([1.0]))
        rs._step(checks, "steady", np.array([1.0]))
    by = {c.name.rsplit(": ", 1)[1]: c for c in checks.checks}
    assert by["varying"].value == pv.VARIES and by["varying"].passed
    assert "SCI-05" in by["varying"].detail
    assert by["steady"].value == "identical" and by["steady"].passed

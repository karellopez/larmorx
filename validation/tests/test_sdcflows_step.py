# SPDX-License-Identifier: Apache-2.0
"""The originals' own step of ``sdcflows-fieldmap``: a change of the NIfTI quaternions alone
(nibabel's, moved by numpy's LAPACK 3.12.0 -> 3.12.1) has its known reason; any other change
fails its check."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

pv = pytest.importorskip("larmorx_validation.parity.python_versions")
sf = pytest.importorskip("larmorx_validation.parity.sdcflows_fieldmap")


def _stepdigest():
    path = Path(sf.SCRIPTS) / "stepdigest.py"
    spec = importlib.util.spec_from_file_location("_test_stepdigest", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["_test_stepdigest"] = module
    spec.loader.exec_module(module)
    return module


def _header(quatern_b: float, cal_max: float = 0.0) -> bytes:
    nib = pytest.importorskip("nibabel")
    h = nib.Nifti1Header()
    h["quatern_b"] = quatern_b
    h["cal_max"] = cal_max
    return h.binaryblock


def test_masked_digest_ignores_only_the_quaternions():
    sd = _stepdigest()
    a, b, c = _header(0.25), _header(0.2500001), _header(0.25, cal_max=1.0)
    assert sd.digest(a) != sd.digest(b)
    assert sd.digest(a, mask=True) == sd.digest(b, mask=True)
    assert sd.digest(a, mask=True) != sd.digest(c, mask=True)
    assert sd.mask_quaternions(b"not a header" * 40) == b"not a header" * 40
    sd.reset()
    sd.oracle(a)
    full = sd.take()
    assert full is not None and sd.last_masked() is not None
    assert sd.take() is None and sd.last_masked() is None


def _record(tmp_path, monkeypatch, name, digest, masked):
    monkeypatch.setenv(pv.STEP_RECORD, str(tmp_path))
    with pv.case("c"):
        for _ in range(pv.STEP_PASSES):
            sf._step(sf.CheckList(), name, digest, masked)
    monkeypatch.delenv(pv.STEP_RECORD)


def test_step_names_the_lapack_change_of_the_quaternions(tmp_path, monkeypatch):
    sd = _stepdigest()
    old, new, other = _header(0.25), _header(0.2500001), _header(0.25, cal_max=1.0)
    for name in ("quaternions", "data", "same"):
        _record(tmp_path, monkeypatch, name, sd.digest(old), sd.digest(old, mask=True))
    monkeypatch.setenv(pv.STEP_FROM, str(tmp_path))
    checks = sf.CheckList()
    with pv.case("c"):
        for name, now in (("quaternions", new), ("data", other), ("same", old)):
            sf._step(checks, name, sd.digest(now), sd.digest(now, mask=True))
    by = {c.name.rsplit(": ", 1)[1]: c for c in checks.checks}
    assert by["quaternions"].value == "changed" and by["quaternions"].passed
    assert "LAPACK 3.12.1" in by["quaternions"].detail
    assert by["data"].value == "changed" and not by["data"].passed
    assert "no known reason" in by["data"].detail
    assert by["same"].value == "identical" and by["same"].passed

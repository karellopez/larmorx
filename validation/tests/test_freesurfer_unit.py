# SPDX-License-Identifier: Apache-2.0
"""larmorx_validation.freesurfer_unit: the unit oracle's Python side.

The value conversions and the matrix text form are checked everywhere; the calls themselves only
where the harness is built (``validation/investigations/freesurfer-unit-oracle/build.py``: the
main machine, never CI), each through the oracle-output cache in a temporary cache.
"""

from __future__ import annotations

import struct

import pytest

from larmorx_validation import freesurfer_unit as fu


def test_value_reads_hex_exactly():
    x = 0.1 + 0.2
    assert fu.value({"hex": x.hex(), "dec": repr(x)}) == x
    assert fu.value(7) == 7
    assert fu.values([{"hex": "0x1p-1"}, [{"hex": "-0x1.8p+1"}]]) == [0.5, [-3.0]]


def test_f32_rounds_to_float32():
    assert fu.f32(0.1) == struct.unpack("<f", struct.pack("<f", 0.1))[0]
    assert fu.f32(0.1) != 0.1
    assert fu.f32(1.5) == 1.5


def test_matrix_text_form(tmp_path):
    p = tmp_path / "m.txt"
    fu._write_matrix(p, [[1.0, 0.1], [2.0, -3.25]])
    lines = p.read_text(encoding="utf-8").splitlines()
    assert lines[0] == "2 2"
    assert [float.fromhex(v) for v in lines[1].split()] == [1.0, fu.f32(0.1)]


def test_unknown_vnl_method_is_refused():
    with pytest.raises(ValueError):
        fu.vnl_random(1, 1, "uniform")


needs_harness = pytest.mark.skipif(not fu.available(), reason="the unit oracle is not built here")


@pytest.fixture
def private_cache(tmp_path, monkeypatch):
    monkeypatch.setenv("LARMORX_ORACLE_CACHE", str(tmp_path / "cache"))
    return tmp_path


@needs_harness
def test_open_ran1_is_drand64_as_float(private_cache):
    seed = 1234
    draws = fu.vnl_random(seed, 4, "drand64")
    assert fu.open_ran1(seed, 4) == [fu.f32(x) for x in draws]


@needs_harness
def test_matrix_inverse_of_a_diagonal(private_cache):
    inv = fu.matrix_inverse([[2.0, 0, 0, 0], [0, 4.0, 0, 0], [0, 0, 0.5, 0], [0, 0, 0, 1.0]])
    assert [inv[i][i] for i in range(4)] == [0.5, 0.25, 2.0, 1.0]
    assert fu.matrix_inverse([[1.0, 2.0], [2.0, 4.0]]) is None

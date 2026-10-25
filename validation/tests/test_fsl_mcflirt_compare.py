# SPDX-License-Identifier: Apache-2.0
"""The mcflirt replica suite's file comparison, which streams large series: the same verdicts
and counts as comparing whole decompressed files and whole arrays."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest

nib = pytest.importorskip("nibabel")
pytest.importorskip("larmorx_validation.parity.fsl_mcflirt_replica")

from larmorx_validation.parity import fsl_mcflirt_replica as m  # noqa: E402


def _save(path: Path, data: np.ndarray, descrip: bytes) -> Path:
    img = nib.Nifti1Image(data, np.eye(4))
    img.header["descrip"] = descrip
    nib.save(img, path)
    return path


@pytest.fixture
def series() -> np.ndarray:
    rng = np.random.default_rng(7)
    return rng.integers(0, 1000, size=(9, 8, 7, 5)).astype(np.float32)


@pytest.mark.parametrize("suffix", [".nii", ".nii.gz"])
def test_descrip_is_ignored_and_values_counted(
    tmp_path: Path, series: np.ndarray, suffix: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(m, "_CHUNK", 64)  # many chunks, and chunks that split values
    a = _save(tmp_path / f"a{suffix}", series, b"FSL 6.0.7")
    b = _save(tmp_path / f"b{suffix}", series, b"larmorx-nc")
    assert m._same_streams(a, b, blank_descrip=True)
    assert not m._same_streams(a, b, blank_descrip=False)
    changed = series.copy()
    changed[1, 2, 3, 4] += 7.0
    changed[8, 7, 6, 0] = np.nan
    c = _save(tmp_path / f"c{suffix}", changed, b"larmorx-nc")
    assert not m._same_streams(a, c, blank_descrip=True)
    assert m._differing_values(a, c) == f"2 of {series.size} values differ (max |diff| 7)"


@pytest.mark.parametrize("dtype", [np.float32, np.int16, np.uint16])
def test_fix_pass_change_in_chunks_equals_whole_arrays(
    tmp_path: Path, series: np.ndarray, dtype: type, monkeypatch: pytest.MonkeyPatch
) -> None:
    from larmorx_validation.parity import defects

    monkeypatch.setattr(m, "_CHUNK", 40)
    a = series.astype(dtype)
    b = a.copy()
    b[0, 0, 0, 0] += 3
    b[4, 4, 4, 2] += 1
    pa = tmp_path / "a.nii.gz"
    pb = tmp_path / "b.nii.gz"
    nib.save(nib.Nifti1Image(a, np.eye(4)), pa)
    nib.save(nib.Nifti1Image(b, np.eye(4)), pb)
    assert m._streamable(pb, pa) is not None
    assert m._change(pb, pa) == defects.Change.of(m._image_values(pb), m._image_values(pa))
    assert m._change(pb, pa).differing == 2


def test_scaled_images_fall_back_to_whole_arrays(tmp_path: Path, series: np.ndarray) -> None:
    from larmorx_validation.parity import defects

    img = nib.Nifti1Image(series.astype(np.int16), np.eye(4))
    img.header.set_slope_inter(0.5, 2.0)
    pa = tmp_path / "scaled.nii"
    nib.save(img, pa)
    pb = _save(tmp_path / "plain.nii", series.astype(np.int16), b"")
    assert m._streamable(pa, pb) is None
    assert m._change(pa, pb) == defects.Change.of(m._image_values(pa), m._image_values(pb))


def test_other_files_compare_their_stored_bytes(tmp_path: Path) -> None:
    (tmp_path / "x.par").write_bytes(b"0 0 0\n" * 1000)
    (tmp_path / "y.par").write_bytes(b"0 0 0\n" * 1000)
    (tmp_path / "z.par").write_bytes(b"0 0 0\n" * 999 + b"0 0 1\n")
    assert m._same_bytes(tmp_path / "x.par", tmp_path / "y.par")
    assert not m._same_bytes(tmp_path / "x.par", tmp_path / "z.par")

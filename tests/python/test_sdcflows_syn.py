# SPDX-License-Identifier: Apache-2.0
"""sdcflows' SyN preprocessing helpers and niworkflows' steps under it: the defects' switches,
the errors and the short paths. The nodes are checked against fMRIPrep's files and against
the originals by ``validation/investigations/sdcflows-fieldmap-b/check_syn_preprocessing.py``,
and on every platform by the golden cases ``py.sdcflows.syn_preprocessing``,
``py.niworkflows.epi_reference`` and ``py.ndimage.affine_transform``."""

import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx


def test_is_outlier_counts_the_leading_outliers() -> None:
    pts = np.asarray([9.0, 8.5, 1.0, 1.2, 0.9, 1.1, 1.0], dtype=np.float32)
    assert lx.niworkflows.is_outlier(pts) == 2
    assert lx.niworkflows.is_outlier(pts[2:]) == 0


def test_is_outlier_zero_spread_pipe23() -> None:
    flat = np.asarray([5.0, 5.0, 5.0, 9.0, 5.0], dtype=np.float32)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        assert lx.niworkflows.is_outlier(flat, upstream_defects="reproduce") == 5
    with pytest.warns(lx.UpstreamDefectWarning, match="PIPE-23"):
        assert lx.niworkflows.is_outlier(flat) == 0
    lead = np.asarray([9.0, 5.0, 5.0, 5.0], dtype=np.float32)
    with pytest.warns(lx.UpstreamDefectWarning, match="PIPE-23"):
        assert lx.niworkflows.is_outlier(lead) == 1


def _write(path: Path, data: np.ndarray, aff: np.ndarray) -> str:
    """A file with matching qform and sform, both of code 1 (valid for ValidateImage)."""
    nb = pytest.importorskip("nibabel")
    img = nb.Nifti1Image(data, aff)
    img.header.set_qform(aff, 1)
    img.header.set_sform(aff, 1)
    img.to_filename(path)
    return str(path)


def test_short_paths(tmp_path: Path) -> None:
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    data = (np.arange(6 * 5 * 4).reshape(6, 5, 4) % 7).astype(np.int16)
    p = _write(tmp_path / "a.nii.gz", data, aff)
    assert lx.sdcflows.deoblique(p, out_dir=tmp_path / "d") == p
    assert lx.sdcflows.reoblique(p, p, p, out_dir=tmp_path / "r") == (p, p, None)
    assert lx.sdcflows.set_dtype(p, "int16") == p
    assert lx.sdcflows.merge_meta("ref.nii.gz", [{"a": 1}, {"a": 2}]) == ("ref.nii.gz", {"a": 1})
    v = lx.niworkflows.validate_image(p, out_dir=tmp_path / "v")
    assert v.out_file == p and v.warning is None and Path(v.out_report).read_text() == ""
    nss = lx.niworkflows.nonsteady_states_detector(p)
    assert nss.t_mask == (True,) and nss.n_dummy == 1
    with pytest.raises(ValueError, match="n_volumes"):
        lx.niworkflows.nonsteady_states_detector(p, n_volumes=5)


def test_errors(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    p = _write(tmp_path / "a.nii.gz", np.ones((6, 5, 4), dtype=np.int16), aff)
    monkeypatch.setenv("LARMORX_SDCFLOWS_ATLAS", p)
    with pytest.raises(ValueError, match="SHA-256"):
        lx.sdcflows.fmap_atlas()
    assert Path(lx.sdcflows.fmap_atlas_affine()).stat().st_size == 193
    with pytest.raises(NotImplementedError, match="RegridToZooms"):
        lx.sdcflows.syn_preprocessing_workflow(p, [{}], p, p, p, out_dir=tmp_path / "s", debug=True)
    with pytest.raises(NotImplementedError, match="mri_robust_template"):
        lx.niworkflows.epi_reference_workflow([p, p], out_dir=tmp_path / "e")
    with pytest.raises(ValueError, match="xform_code"):
        lx.niworkflows.generate_sampling_reference(p, p, xform_code=1)
    assert lx.niworkflows.generate_sampling_reference(p, p, keep_native=False) == p


def test_affine_transform_checks() -> None:
    data = np.arange(60, dtype=np.float64).reshape(5, 4, 3)
    with pytest.raises(RuntimeError, match="spline order"):
        lx.ndimage.affine_transform(data, np.eye(3), order=6)
    with pytest.raises(RuntimeError, match="rows"):
        lx.ndimage.affine_transform(data, np.eye(2))
    with pytest.raises(RuntimeError, match="input rank"):
        lx.ndimage.affine_transform(data, np.eye(3), offset=[0.0, 1.0])
    with pytest.raises(ValueError, match="bottom row"):
        bad = np.eye(4)
        bad[3, 0] = 1.0
        lx.ndimage.affine_transform(data, bad)
    with pytest.raises(NotImplementedError, match="3-D"):
        lx.ndimage.affine_transform(data[..., 0], np.eye(2))
    same = lx.ndimage.affine_transform(data, np.eye(3), order=0)
    assert np.array_equal(same, data)
    assert np.array_equal(lx.ndimage.affine_transform(data, np.ones(3), order=1), data)


def _plain(path: Path, shape: tuple[int, ...], zooms: tuple[float, ...], rot: float = 0.0) -> str:
    """A file of ``shape`` with voxel sizes ``zooms``, rotated by ``rot`` radians about z."""
    c, s = np.cos(rot), np.sin(rot)
    aff = np.eye(4)
    aff[:3, :3] = np.array([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]) @ np.diag(zooms)
    data = (np.arange(int(np.prod(shape))).reshape(shape) % 11).astype(np.float32)
    return _write(path, data, aff)


def test_syn_sdc_helpers(tmp_path: Path, caplog: pytest.LogCaptureFixture) -> None:
    anat = _plain(tmp_path / "anat.nii.gz", (8, 9, 7), (1.0, 1.25, 1.5))
    epi = _plain(tmp_path / "epi.nii.gz", (6, 7, 5), (3.0, 3.0, 3.3))
    # 0.5 * (1.0 + 3.0) = 2.0 (float32); the bounds as Python floats
    z = lx.sdcflows.adjust_zooms(anat, epi)
    assert z == (2.0, 2.0, 2.0) and isinstance(z[0], np.float32)
    assert lx.sdcflows.adjust_zooms(anat, anat) == (1.8, 1.8, 1.8)
    assert isinstance(lx.sdcflows.adjust_zooms(anat, anat)[0], float)
    assert lx.sdcflows.adjust_zooms(epi, epi) == (2.2, 2.2, 2.2)
    assert lx.sdcflows.warp_dir(epi, anat, "j-") == [[0.1, 1.0, 0.1], [0.1, 1.0, 0.1]]
    assert lx.sdcflows.warp_dir(epi, anat, "k", nlevels=1) == [[0.1, 0.1, 1.0]]
    with pytest.raises(ValueError):
        lx.sdcflows.warp_dir(epi, anat, "x")
    oblique = _plain(tmp_path / "obl.nii.gz", (8, 9, 7), (1.0, 1.25, 1.5), rot=0.1)
    with caplog.at_level("WARNING", logger="larmorx.sdcflows"):
        lx.sdcflows.warp_dir(epi, oblique, "i")
    assert "oblique dataset" in caplog.text
    cfg = {"transform_parameters": [[0.8, 6.0, 10.0], [0.8, 2.0, 0.5]]}
    vp = lx.sdcflows.mm2vox(epi, anat, "j", cfg)
    assert vp == [[0.8, 6.0, 8.0], [0.8, 2.0, 0.4]]


def test_norm_lap_sdc14(tmp_path: Path) -> None:
    data = np.zeros((6, 5, 4), dtype=np.float32)
    data[2:4, 2:4, 1:3] = 5.0
    data[0, 0, 0] = -3.0
    data[5, 4, 3] = 7.0
    p = _write(tmp_path / "lap.nii.gz", data, np.eye(4))
    out = lx.sdcflows.norm_lap(p, out_dir=tmp_path / "n")
    v = np.asarray(lx.io.load_nibabel(out).data)
    assert v.dtype == np.float32 and v.min() == -1.0 and v.max() == 1.0
    flat = _write(tmp_path / "flat.nii.gz", np.full((4, 4, 4), 2.0, np.float32), np.eye(4))
    with pytest.raises(ValueError, match="SDC-14"):
        lx.sdcflows.norm_lap(flat, out_dir=tmp_path / "f")
    with pytest.raises(IndexError):
        lx.sdcflows.norm_lap(flat, out_dir=tmp_path / "f", upstream_defects="reproduce")

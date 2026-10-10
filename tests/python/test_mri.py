# SPDX-License-Identifier: Apache-2.0
"""``lx.mri.hmc`` and ``larmorx mri hmc``: behaviour that does not need FSL (parity with
mcflirt itself is in ``tests/parity`` and ``validation/``)."""

from __future__ import annotations

import contextlib
import io
import os

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

SHAPE = (24, 26, 18)
ZOOMS = (3.5, 3.5, 4.0)


def las_affine(shape=SHAPE, zooms=ZOOMS) -> np.ndarray:
    a = np.diag([-zooms[0], zooms[1], zooms[2], 1.0])
    a[:3, 3] = -a[:3, :3] @ ((np.array(shape) - 1) / 2.0)
    return a


def phantom(shifts_mm, shape=SHAPE, zooms=ZOOMS, seed=0) -> np.ndarray:
    """Volumes of a smooth asymmetric blob, each moved by ``shifts_mm`` along the voxel axes
    (in mm: index times voxel size, which is FSL-mm for this LAS image)."""
    rng = np.random.default_rng(seed)
    grid = np.stack(np.meshgrid(*[np.arange(n) for n in shape], indexing="ij"), axis=-1)
    pts = grid * np.array(zooms)
    centre = (np.array(shape) - 1) / 2 * np.array(zooms)
    vols = []
    for s in shifts_mm:
        q = pts - centre - np.asarray(s)
        r2 = (q**2 / np.array([30.0, 36.0, 24.0]) ** 2).sum(-1)
        v = 800 * np.exp(-r2) + 300 * np.exp(-((q - [8, -5, 4]) ** 2).sum(-1) / 60.0)
        v += 200 * np.exp(-((q - [-9, 10, -3]) ** 2).sum(-1) / 40.0)
        vols.append(v + rng.normal(0, 2, shape))
    return np.stack(vols, axis=-1).astype(np.float32)


SHIFTS = [(0, 0, 0), (1.0, 0, 0), (0, -1.5, 0.5), (0.5, 0.5, -1.0), (0, 0, 0)]


@pytest.fixture(scope="module")
def series(tmp_path_factory) -> str:
    path = tmp_path_factory.mktemp("hmc") / "bold.nii.gz"
    nib.Nifti1Image(phantom(SHIFTS), las_affine()).to_filename(path)
    return str(path)


def test_recovers_known_translations(series):
    res = lx.mri.hmc(series, ref_index=0, n_threads=2)
    assert res.matrices.shape == (5, 4, 4)
    assert res.reference_index == 0
    np.testing.assert_array_equal(res.matrices[0], np.eye(4))
    # A volume whose head moved by s (FSL-mm) is mapped back by -s.
    for t, s in enumerate(SHIFTS):
        np.testing.assert_allclose(res.matrices[t][:3, 3], -np.asarray(s), atol=0.25)
    assert res.params.shape == (5, 6)
    assert res.rms_abs.shape == (5,) and res.rms_rel.shape == (4,)
    assert res.fd[0] == 0.0 and res.fd[1] > 0.5
    assert res.image is not None and res.image.shape == (*SHAPE, 5)
    assert res.image.dtype == np.float32
    np.testing.assert_allclose(res.image.affine, las_affine())


def test_results_do_not_depend_on_threads_or_input_form(series):
    a = lx.mri.hmc(series, n_threads=1, resample=False)
    b = lx.mri.hmc(series, n_threads=3, resample=False)
    np.testing.assert_array_equal(a.matrices, b.matrices)
    assert a.image is None
    # The same data in memory give the same matrices.
    img = nib.load(series)
    c = lx.mri.hmc((np.asarray(img.dataobj), img.affine), resample=False)
    np.testing.assert_array_equal(a.matrices, c.matrices)


def test_world_transforms_map_reference_points(series):
    res = lx.mri.hmc(series, ref_index=0)
    # Volume 1 shows the head moved by +1 mm along the voxel x axis, which points to the left
    # (LAS): the transform from the reference to the volume moves points by -1 mm in RAS x,
    # +1 mm in LPS x.
    np.testing.assert_allclose(res.affines[1][:3, 3], [-1.0, 0.0, 0.0], atol=0.1)
    np.testing.assert_allclose(res.itk[1][:3, 3], [1.0, 0.0, 0.0], atol=0.1)
    assert len(res.affine_series) == 5


@pytest.mark.parametrize("ras_storage", [False, True])
def test_world_transforms_match_nitransforms(series, tmp_path, ras_storage):
    """The RAS transforms are what nitransforms makes of the FSL matrices (fMRIPrep's
    MCFLIRT2ITK reads them with the HMC reference as both images)."""
    fsl = pytest.importorskip("nitransforms.io.fsl")
    img = nib.load(series)
    data = np.asarray(img.dataobj)
    affine = img.affine
    if ras_storage:  # the same head stored with x reversed (positive determinant)
        flip = np.eye(4)
        flip[0, 0], flip[0, 3] = -1.0, data.shape[0] - 1
        data, affine = data[::-1], affine @ flip
    path = tmp_path / "bold.nii.gz"
    nib.Nifti1Image(data, affine).to_filename(path)
    ref = tmp_path / "ref.nii.gz"
    nib.Nifti1Image(data[..., 0], affine).to_filename(ref)
    res = lx.mri.hmc(path, reference=ref, resample=False)
    res.save_mats(tmp_path / "mats")
    ref_img = nib.load(ref)
    for t in range(len(res.matrices)):
        xfm = fsl.FSLLinearTransform.from_filename(tmp_path / "mats" / f"MAT_{t:04d}")
        expected = xfm.to_ras(reference=ref_img, moving=ref_img)
        # The .mat files hold 6 decimals: 5e-7 per entry, times ~100 mm for the translations.
        np.testing.assert_allclose(res.affines[t], expected, atol=2e-4)


def test_separate_reference_and_file_formats(series, tmp_path):
    img = nib.load(series)
    ref = tmp_path / "ref.nii.gz"
    nib.Nifti1Image(np.asarray(img.dataobj)[..., 0], img.affine).to_filename(ref)
    res = lx.mri.hmc(series, reference=ref)
    assert res.reference_index is None
    paths = res.save_mats(tmp_path / "out.mat")
    assert [p.name for p in paths] == [f"MAT_{t:04d}" for t in range(5)]
    first = paths[1].read_text(encoding="utf-8").splitlines()[0]
    assert first.endswith(" ") and len(first.split()) == 4
    np.testing.assert_allclose(lx.mri.read_mats(tmp_path / "out.mat"), res.matrices, atol=5e-7)
    res.save_par(tmp_path / "out.par")
    assert len((tmp_path / "out.par").read_text(encoding="utf-8").splitlines()) == 5
    res.save_itk(tmp_path / "mat2itk.txt")
    back = lx.transforms.load_itk_linear(tmp_path / "mat2itk.txt")
    np.testing.assert_allclose(back.matrices, res.affines, atol=1e-9)


def test_options(series):
    for kwargs in [
        dict(stages=1),
        dict(stages=4),
        dict(final="sinc"),
        dict(final="spline"),
        dict(final="nearest"),
        dict(cost="leastsquares"),
        dict(cost="corratio"),
        dict(dof=7),
        dict(mean=True),
        dict(smooth=0.0),
        dict(fudge=True),
        dict(in_plane="always"),
    ]:
        res = lx.mri.hmc(series, **kwargs)
        assert np.isfinite(res.matrices).all(), kwargs
    with pytest.raises(ValueError, match="unknown cost"):
        lx.mri.hmc(series, cost="bogus")  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="stages"):
        lx.mri.hmc(series, stages=0)
    with pytest.raises(ValueError):
        lx.mri.hmc(series, ref_index=99)


def cli(*args, env=None) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    old = dict(os.environ)
    try:
        if env:
            os.environ.update(env)
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(out):
            code = cli_run(["larmorx", "mri", "hmc", *map(str, args)])
    finally:
        os.environ.clear()
        os.environ.update(old)
    return code, out.getvalue(), err.getvalue()


def test_command_line_writes_mcflirt_files(series, tmp_path):
    out = tmp_path / "x_mcf.nii.gz"
    code, _, err = cli("-in", series, "-out", out, "-mats", "-plots", "-rmsrel", "-rmsabs")
    assert code == 0, err
    for name in [
        "x_mcf.nii.gz",
        "x_mcf.nii.gz.par",
        "x_mcf.nii.gz_abs.rms",
        "x_mcf.nii.gz_rel.rms",
    ]:
        assert (tmp_path / name).exists(), name
    mats = sorted(p.name for p in (tmp_path / "x_mcf.nii.gz.mat").iterdir())
    assert mats == [f"MAT_{t:04d}" for t in range(5)]
    # The reference volume (N // 2 = 2) has the identity, printed as mcflirt prints it.
    text = (tmp_path / "x_mcf.nii.gz.mat" / "MAT_0002").read_text(encoding="utf-8")
    assert text.splitlines()[0] == "1.000000 0.000000 0.000000 0.000000 "
    assert (tmp_path / "x_mcf.nii.gz.par").read_text(encoding="utf-8").splitlines()[2] == (
        "0  -0  0  0  0  0  "
    )
    # The Python API gives the same matrices.
    res = lx.mri.hmc(series)
    np.testing.assert_allclose(
        lx.mri.read_mats(tmp_path / "x_mcf.nii.gz.mat"), res.matrices, atol=5e-7
    )
    # A second run with the same -out writes into x_mcf.nii.gz.mat+ (as mcflirt does).
    code, _, _ = cli("-in", series, "-out", out, "-mats")
    assert code == 0 and (tmp_path / "x_mcf.nii.gz.mat+").is_dir()


def test_command_line_errors(series):
    assert cli()[0] == 1
    code, out, _ = cli("-in", series, "-help")
    assert code == 0 and out.startswith("Usage: larmorx mri hmc")
    code, out, err = cli("-out", "x", "-mats")
    assert code == 2 and "Input filename not found" in err
    code, _, err = cli("-in", series, "-edge", "-out", "x")
    assert code == 255 and err == "Unrecognised option -edge\n"
    code, _, err = cli("-in", series, "-refvol", "99", "-out", "x")
    assert code == 1 and "-refvol 99" in err

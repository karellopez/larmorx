# SPDX-License-Identifier: Apache-2.0
"""D13 for the ANTs and ITK ports: each defect corrected by default, reproduced with
``upstream_defects="reproduce"`` (``LARMORX_UPSTREAM_DEFECTS=reproduce`` on the command line),
and named by a warning (a notice line) exactly where it changes the result
(``docs/api/upstream-defects.md``, ``docs/findings/upstream-defects.md``)."""

from __future__ import annotations

import contextlib
import io
import os
import warnings

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx import _defects
from larmorx.cli import run as cli_run

FIX, REPRODUCE = "fix", "reproduce"
AFFINE = np.diag([2.0, 2.0, 3.0, 1.0])


def image(data, affine=AFFINE):
    return lx.Image(np.asfortranarray(np.asarray(data, dtype=np.float32)), affine)


def entries(fn, *args, **kwargs):
    """``fn``'s result and the IDs of the ``UpstreamDefectWarning``s it raised."""
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        result = fn(*args, **kwargs)
    found = [w.message.entry for w in record if isinstance(w.message, lx.UpstreamDefectWarning)]
    return result, found


def ants(*args, env=FIX):
    """``larmorx ants <args>`` in mode ``env``: ``(code, stdout, stderr)``."""
    out, err = io.StringIO(), io.StringIO()
    old = os.environ.get(_defects.ENV)
    try:
        os.environ[_defects.ENV] = env
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = cli_run(["larmorx", "ants", *map(str, args)])
    finally:
        if old is None:
            os.environ.pop(_defects.ENV, None)
        else:
            os.environ[_defects.ENV] = old
    return code, out.getvalue(), err.getvalue()


def save(img, path, dtype=np.float32):
    nib.Nifti1Image(np.asarray(img, dtype=dtype), AFFINE).to_filename(path)
    return path


def phantom(shape=(12, 10, 8), seed=1):
    rng = np.random.default_rng(seed)
    data = 100.0 + 50.0 * rng.random(shape)
    data[3:8, 3:7, 2:6] += 300.0
    return data.astype(np.float32)


def test_the_registry_lists_the_ants_entries():
    ids = {d.id for d in _defects.defects()}
    for entry in (
        "ANTS-03",
        "ANTS-06",
        "ANTS-08",
        "ANTS-20",
        "ANTS-30",
        "ANTS-38",
        "ANTS-43",
        "ANTS-47",
        "ANTS-49",
        "ANTS-50",
        "ANTS-51",
        "ANTS-52",
        "ANTS-53",
        "ANTS-55",
        "ANTS-56",
        "ITK-01",
        "ITK-02",
        "ITK-08",
        "ITK-10",
        "ITK-18",
        "ITK-19",
        "ITK-20",
        "SCI-02",
    ):
        assert entry in ids, entry


# --- ImageMath -----------------------------------------------------------------------------------


def test_arithmetic_defects():
    a = image([[[6.0, 8.0, 10.0, 12.0]]])
    b = image([[[2.0, 0.0, -1.0, 4.0]]])
    # ANTS-08
    fixed, fw = entries(lx.ants.image_arithmetic, a, "divide", b)
    ants_, aw = entries(lx.ants.image_arithmetic, a, "divide", b, upstream_defects=REPRODUCE)
    assert fixed.data.ravel().tolist() == [3, 0, -10, 3]
    assert ants_.data.ravel().tolist() == [3, 3, 3, 3]
    assert fw == aw == ["ANTS-08"]
    # No difference, no warning.
    _, w = entries(lx.ants.image_arithmetic, a, "divide", 2.0)
    assert w == []
    # ANTS-09
    fixed, fw = entries(lx.ants.image_arithmetic, a, "mean", 1.0)
    ants_, aw = entries(lx.ants.image_arithmetic, a, "mean", 1.0, upstream_defects=REPRODUCE)
    assert fixed.data.ravel().tolist() == [9, 9, 9, 9]
    assert ants_.data.ravel().tolist() == [6, 14, 24, 36]
    assert fw == aw == ["ANTS-09"]
    # ANTS-07: a smaller second image.
    small = image([[[1.0, 2.0]]])
    with pytest.raises(ValueError, match="ANTS-07"):
        lx.ants.image_arithmetic(a, "multiply", small)
    out, w = entries(lx.ants.image_arithmetic, a, "multiply", small, upstream_defects=REPRODUCE)
    assert out.data.ravel().tolist() == [6, 16, 0, 0]
    assert w == ["ANTS-07"]
    with pytest.raises(ValueError, match="ANTS-07"):
        lx.ants.multiply_images(a, small)
    # ANTS-11
    increasing = image([[[1.0, 2.0, 3.0]]])
    fixed, fw = entries(lx.ants.negative_image, increasing)
    ants_, aw = entries(lx.ants.negative_image, increasing, upstream_defects=REPRODUCE)
    assert fixed.data.ravel().tolist() == [2, 1, 0]
    assert ants_.data.ravel().tolist() != [2, 1, 0]
    assert fw == aw == ["ANTS-11"]


def test_intensity_defects():
    # ANTS-12: the brightest voxel first.
    data = np.arange(1, 1001, dtype=np.float32).reshape(10, 10, 10, order="F")
    data[0, 0, 0] = 5000.0
    fixed, fw = entries(lx.ants.truncate_image_intensity, image(data), 0.0, 1.0, 10)
    ants_, aw = entries(
        lx.ants.truncate_image_intensity, image(data), 0.0, 1.0, 10, upstream_defects=REPRODUCE
    )
    assert fw == aw == ["ANTS-12"]
    assert fixed.data.max() > ants_.data.max()
    # ANTS-13: all negative.
    neg = image([[[-4.0, -2.0]]])
    fixed, fw = entries(lx.ants.normalize_image, neg)
    ants_, aw = entries(lx.ants.normalize_image, neg, upstream_defects=REPRODUCE)
    assert fixed.data.ravel().tolist() == [0, 1]
    assert ants_.data.ravel().tolist() == [0, 0.5]
    assert fw == aw == ["ANTS-13"]


def test_gaussian_defects(tmp_path):
    data = phantom()
    img = image(data, np.eye(4))
    # ANTS-14: a variance of 900 voxels².
    fixed, fw = entries(lx.ants.discrete_gaussian, img, 30.0)
    ants_, aw = entries(lx.ants.discrete_gaussian, img, 30.0, upstream_defects=REPRODUCE)
    assert np.isfinite(fixed.data).all() and np.isnan(ants_.data).all()
    assert fw == aw == ["ANTS-14"]
    # ANTS-15: two sigmas in 3D.
    src = save(data, tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    code, _, err = ants("ImageMath", 3, out, "G", src, "1x2", env=FIX)
    assert code == 1 and "ANTS-15" in err
    code, _, err = ants("ImageMath", 3, out, "G", src, "1x2", env=REPRODUCE)
    assert code == 0 and "ANTS-15" in err
    np.testing.assert_array_equal(nib.load(out).get_fdata(dtype=np.float32), data)
    code, _, err = ants("SmoothImage", 3, src, "1x2", out, env=FIX)
    assert code == 1 and "ANTS-15" in err
    code, _, err = ants("SmoothImage", 3, src, "1x2", out, env=REPRODUCE)
    assert code == 0 and "ANTS-15" in err
    # ANTS-28: SmoothImage's median with a wrong sigma count leaves the radius uninitialised.
    code, _, err = ants("SmoothImage", 3, src, "1x2", out, 0, 1, env=REPRODUCE)
    assert code == 0 and "ANTS-28" in err
    np.testing.assert_array_equal(nib.load(out).get_fdata(dtype=np.float32), data)
    # ANTS-16: Laplacian's flag.
    code, _, err = ants("ImageMath", 3, out, "Laplacian", src, "1.5", "0", env=FIX)
    assert code == 0 and "ANTS-16" in err
    lap = nib.load(out).get_fdata(dtype=np.float32)
    assert lap.min() < 0
    code, _, err = ants("ImageMath", 3, out, "Laplacian", src, "1.5", "0", env=REPRODUCE)
    assert code == 0 and "ANTS-16" in err
    assert nib.load(out).get_fdata(dtype=np.float32).min() == 0.0


def test_operands_and_short_names(tmp_path):
    src = save(phantom(), tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    # ANTS-05: "7 " is a number here, a file name in ANTs.
    code, _, err = ants("ImageMath", 3, out, "m", src, "7 ", env=FIX)
    assert code == 0 and "ANTS-05" in err
    np.testing.assert_allclose(nib.load(out).get_fdata(), 7 * phantom(), rtol=1e-6)
    code, _, err = ants("ImageMath", 3, out, "m", src, "7 ", env=REPRODUCE)
    assert code == 1 and "ANTS-05" in err
    # "1e": ANTs reads 0, larmorx refuses it; no operand: ANTs multiplies by 1.
    code, _, err = ants("ImageMath", 3, out, "m", src, "1e", env=FIX)
    assert code == 1 and "not a number" in err
    code, _, _ = ants("ImageMath", 3, out, "m", src, "1e", env=REPRODUCE)
    assert code == 0 and (nib.load(out).get_fdata() == 0).all()
    code, _, err = ants("ImageMath", 3, out, "m", src, env=FIX)
    assert code == 1 and "ANTS-05" in err
    code, _, _ = ants("ImageMath", 3, out, "abs", src, env=FIX)
    assert code == 0
    # ANTS-10: vtotal.
    code, _, err = ants("ImageMath", 3, out, "vtotal", src, 1, env=FIX)
    assert code == 1 and "ANTS-10" in err
    code, _, err = ants("ImageMath", 3, out, "vtotal", src, 1, env=REPRODUCE)
    assert code == 0 and "ANTS-10" in err and (nib.load(out).get_fdata() == 0).all()
    # ANTS-03: an output name of 2 characters.
    cwd = os.getcwd()
    try:
        os.chdir(tmp_path)
        code, _, err = ants("ImageMath", 3, "ab", "m", src, 2, env=REPRODUCE)
        assert code == 0 and "ANTS-03" in err and not (tmp_path / "ab").exists()
        code, _, err = ants("ImageMath", 3, "ab", "m", src, 2, env=FIX)
        assert code == 1 and "ANTS-03" in err
    finally:
        os.chdir(cwd)


def test_components_and_holes():
    point = np.zeros((6, 6, 6), np.float32)
    point[2, 2, 2] = 1
    # ANTS-20
    fixed, fw = entries(lx.ants.largest_component, image(point))
    ants_, aw = entries(lx.ants.largest_component, image(point), upstream_defects=REPRODUCE)
    assert (fixed.data == 0).all() and (ants_.data == 1).all()
    assert fw == aw == ["ANTS-20"]
    # ANTS-19: a hole on the first slice.
    data = np.zeros((7, 7, 3), np.float32)
    data[1:6, 1:6, :] = 1
    data[3, 3, 0] = 0
    fixed, fw = entries(lx.ants.fill_holes, image(data), 0.9)
    ants_, aw = entries(lx.ants.fill_holes, image(data), 0.9, upstream_defects=REPRODUCE)
    assert fixed.data[3, 3, 0] == 1 and ants_.data[3, 3, 0] == 0
    assert fw == aw == ["ANTS-19"]
    # ANTS-06: a negative label.
    labels = np.zeros((4, 4, 4), np.float32)
    labels[1:3, 1:3, 1:3] = -2
    fixed, fw = entries(lx.ants.extract_contours, image(labels))
    ants_, aw = entries(lx.ants.extract_contours, image(labels), upstream_defects=REPRODUCE)
    assert (fixed.data == 0).all() and ants_.data.max() > 1e19
    assert fw == aw == ["ANTS-06"]


# --- ThresholdImage, MultiplyImages -------------------------------------------------------------


def test_threshold_and_multiply_defects(tmp_path):
    data = -10.0 + np.arange(64, dtype=np.float32).reshape(4, 4, 4) / 10
    # ANTS-25: Otsu with a mask over negative values.
    mask = np.ones((4, 4, 4))
    fixed, fw = entries(lx.ants.otsu_threshold, image(data), 1, mask=mask)
    ants_, aw = entries(
        lx.ants.otsu_threshold, image(data), 1, mask=mask, upstream_defects=REPRODUCE
    )
    assert fw == aw == ["ANTS-25"]
    assert fixed.thresholds != ants_.thresholds
    src = save(phantom(), tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    # ANTS-24: "otsu" is not a mode.
    code, _, err = ants("ThresholdImage", 3, src, out, "otsu", 2, env=FIX)
    assert code == 1 and "ANTS-24" in err
    code, _, err = ants("ThresholdImage", 3, src, out, "otsu", 2, env=REPRODUCE)
    assert code == 0 and "ANTS-24" in err
    # ANTS-27: a missing second image.
    missing = tmp_path / "missing.nii.gz"
    code, _, err = ants("MultiplyImages", 3, src, missing, out, env=FIX)
    assert code == 1 and "ANTS-27" in err
    code, _, err = ants("MultiplyImages", 3, src, missing, out, env=REPRODUCE)
    assert code == 0 and "ANTS-27" in err and (nib.load(out).get_fdata() == 0).all()


# --- Resampling ---------------------------------------------------------------------------------


def test_resample_defects(tmp_path):
    data = phantom()
    src = save(data, tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    # ANTS-29: size 1.
    with pytest.raises(ValueError, match="ANTS-29"):
        lx.ants.resample_image(src, size=[1, 10, 8])
    one, w = entries(lx.ants.resample_image, src, size=[1, 10, 8], upstream_defects=REPRODUCE)
    assert one.shape == (1, 10, 8) and (one.data == 0).all() and w == ["ANTS-29"]
    # ANTS-30: B-spline order 5 is also pixel type 5 (unsigned int) in ANTs.
    code, _, err = ants("ResampleImage", 3, src, out, "1.5", 0, 4, 5, env=FIX)
    assert code == 0 and "ANTS-30" in err and nib.load(out).get_data_dtype() == np.float32
    code, _, err = ants("ResampleImage", 3, src, out, "1.5", 0, 4, 5, env=REPRODUCE)
    assert code == 0 and "ANTS-30" in err and nib.load(out).get_data_dtype() == np.uint32
    code, _, err = ants("ResampleImage", 3, src, out, "1.5", 0, 3, "l", env=FIX)
    assert code == 0, err
    # ANTS-06: unsigned int pixels of negative values.
    neg = save(-phantom(), tmp_path / "neg.nii.gz")
    code, _, err = ants("ResampleImage", 3, neg, out, "2", 0, 0, 5, env=FIX)
    assert code == 0 and "ANTS-06" in err and nib.load(out).get_fdata().max() == 0
    code, _, err = ants("ResampleImage", 3, neg, out, "2", 0, 0, 5, env=REPRODUCE)
    assert code == 0 and "ANTS-06" in err and nib.load(out).get_fdata().max() > 4e9
    # ANTS-28: smoothing an axis of 3 voxels.
    thin = save(phantom((8, 7, 3)), tmp_path / "thin.nii.gz")
    with pytest.raises(ValueError, match="less than 4"):
        lx.ants.resample_image_by_spacing(thin, [2.0, 2.0, 6.0])
    r, w = entries(
        lx.ants.resample_image_by_spacing, thin, [2.0, 2.0, 6.0], upstream_defects=REPRODUCE
    )
    assert w == ["ANTS-28"] and not r.data.any()
    code, stdout, err = ants("ResampleImageBySpacing", 3, thin, out, 2, 2, 6, env=REPRODUCE)
    assert code == 0 and "Exception catched !" in stdout and "ANTS-28" in err
    # ANTS-31: 2D reads nn from addvox in ANTs.
    plane = tmp_path / "plane.nii.gz"
    nib.Nifti1Image(phantom((12, 10, 1))[:, :, 0], AFFINE).to_filename(plane)
    code, _, err = ants("ResampleImageBySpacing", 2, plane, out, 1, 1, 0, 1, 0, env=FIX)
    assert code == 0 and "ANTS-31" in err
    code, _, err = ants("ResampleImageBySpacing", 5, plane, out, 1, 1, env=FIX)
    assert code == 1 and "ANTS-31" in err
    code, _, err = ants("ResampleImageBySpacing", 5, plane, out, 1, 1, env=REPRODUCE)
    assert code == 0 and "ANTS-31" in err


def test_apply_transforms_defects(tmp_path):
    data = phantom() * 3
    src = save(data, tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    # ANTS-33: an unknown -u type.
    code, _, err = ants("antsApplyTransforms", "-i", src, "-r", src, "-o", out, "-u", "uint8")
    assert code == 1 and "ANTS-33" in err
    code, _, err = ants(
        "antsApplyTransforms", "-i", src, "-r", src, "-o", out, "-u", "uint8", env=REPRODUCE
    )
    assert code == 0 and "ANTS-33" in err and nib.load(out).get_data_dtype() == np.float64
    # ANTS-06: -u uchar of values above 255.
    code, _, err = ants("antsApplyTransforms", "-i", src, "-r", src, "-o", out, "-u", "uchar")
    assert code == 0 and "ANTS-06" in err and nib.load(out).get_fdata().max() == 255
    code, _, err = ants(
        "antsApplyTransforms", "-i", src, "-r", src, "-o", out, "-u", "uchar", env=REPRODUCE
    )
    assert code == 0 and "ANTS-06" in err
    wrapped = np.asarray(nib.load(out).dataobj)
    assert (wrapped != np.clip(np.trunc(data), 0, 255)).any()
    fixed, fw = entries(lx.ants.apply_transforms, src, src, dtype=np.uint8)
    ants_, aw = entries(
        lx.ants.apply_transforms, src, src, dtype=np.uint8, upstream_defects=REPRODUCE
    )
    assert fw == aw == ["ANTS-06"]
    np.testing.assert_array_equal(ants_.data, wrapped)
    assert fixed.data.max() == 255
    # ANTS-33: MultiLabel's alpha, GenericLabel's interpolator.
    labels = save(np.round(phantom() / 100), tmp_path / "labels.nii.gz")
    for env in (FIX, REPRODUCE):
        code, _, err = ants(
            "antsApplyTransforms", "-i", labels, "-r", labels, "-o", out,
            "-n", "MultiLabel[2,2]", env=env,
        )  # fmt: skip
        assert code == 0 and "ANTS-33" in err
    code, _, err = ants(
        "antsApplyTransforms", "-i", labels, "-r", labels, "-o", out,
        "-n", "GenericLabel[NearestNeighbor]",
    )  # fmt: skip
    assert code == 0 and "ANTS-33" in err
    code, _, err = ants(
        "antsApplyTransforms", "-i", labels, "-r", labels, "-o", out, "-n", "GenericLabel[Foo]"
    )
    assert code == 1 and "ANTS-33" in err


def test_float_versor(tmp_path):
    # ITK-10: a versor axis that ITK fails to load in float (parameters from the register).
    src = save(phantom(), tmp_path / "in.nii.gz")
    xfm = tmp_path / "versor.txt"
    xfm.write_text(
        "#Insight Transform File V1.0\n#Transform 0\n"
        "Transform: VersorRigid3DTransform_double_3_3\n"
        "Parameters: -0.7041559 0.63925344 0.3665738 1 2 3\n"
        "FixedParameters: 0 0 0\n",
        encoding="utf-8",
    )
    out, w = entries(lx.ants.apply_transforms, src, src, [xfm], precision="float32")
    assert w == ["ITK-10"] and out.data.dtype == np.float32
    with pytest.raises(ValueError, match="ITK-10"):
        lx.ants.apply_transforms(src, src, [xfm], precision="float32", upstream_defects=REPRODUCE)


# --- N4, DenoiseImage ---------------------------------------------------------------------------


def test_n4_defects(tmp_path):
    data = phantom((20, 18, 12))
    data[0, 0, 0] = 1.0  # the first voxel is the unique minimum (ANTS-38)
    img = image(data)
    kw = {"shrink_factor": 1, "convergence": ((3,), 0.0)}
    fixed, fw = entries(lx.ants.n4_bias_field_correction, img, **kw)
    ants_, aw = entries(lx.ants.n4_bias_field_correction, img, upstream_defects=REPRODUCE, **kw)
    assert fw == aw == ["ANTS-38"]
    assert not np.array_equal(fixed.corrected.data, ants_.corrected.data)
    # ANTS-39: 256 bins padded to 512 or 1024 points. The size changes only the FFTs'
    # rounding, so most inputs give the same output either way, and no warning.
    kw256 = {"shrink_factor": 1, "convergence": ((2,), 0.0)}
    kw256["histogram_sharpening"] = (0.15, 0.01, 256)
    _, fw = entries(lx.ants.n4_bias_field_correction, image(phantom((20, 18, 12))), **kw256)
    assert fw == []
    rng = np.random.default_rng(17)  # an input where one voxel's sharpened value changes
    skewed = image(np.exp(rng.normal(5, 3.0, (8, 8, 8))).astype(np.float32))
    fixed, fw = entries(lx.ants.n4_bias_field_correction, skewed, **kw256)
    ants_, aw = entries(
        lx.ants.n4_bias_field_correction, skewed, upstream_defects=REPRODUCE, **kw256
    )
    assert fw == aw == ["ANTS-39"]
    assert not np.array_equal(fixed.corrected.data, ants_.corrected.data)
    # ANTS-35, ANTS-36 on the command line.
    src = save(data, tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    base = ("N4BiasFieldCorrection", "-d", 3, "-i", src, "-o", out, "-s", 2)
    code, _, err = ants(*base, "-c", "3", env=FIX)
    assert code == 0 and "ANTS-35" in err
    code, _, err = ants(*base, "-c", "3", env=REPRODUCE)
    assert code == 0 and "ANTS-35" in err
    code, _, err = ants(*base, "-c", "[3]", "-x", tmp_path / "nomask.nii.gz", env=FIX)
    assert code == 1 and "ANTS-36" in err
    code, _, err = ants(*base, "-c", "[3]", "-x", tmp_path / "nomask.nii.gz", env=REPRODUCE)
    assert code == 0 and "ANTS-36" in err
    code, _, err = ants(*base, "-c", "[3]", "-r", 1, env=FIX)
    assert code == 0 and "ANTS-36" in err


def test_denoise_defects(tmp_path):
    rng = np.random.default_rng(3)
    data = (1000.0 + rng.random((10, 9, 8)) * 5).astype(np.float32)
    data[2:7, 2:6, 2:6] += 500.0
    img = image(data)
    # ANTS-43: the SNR is high.
    fixed, fw = entries(lx.ants.denoise_image, img)
    ants_, aw = entries(lx.ants.denoise_image, img, upstream_defects=REPRODUCE)
    assert fw == aw == ["ANTS-43"]
    assert (fixed.denoised.data <= ants_.denoised.data).all()
    # ANTS-47: an unreadable mask.
    src = save(data, tmp_path / "in.nii.gz")
    out = tmp_path / "out.nii.gz"
    args = ("DenoiseImage", "-d", 3, "-i", src, "-x", tmp_path / "nomask.nii.gz", "-o", out)
    code, _, err = ants(*args, env=FIX)
    assert code == 1 and "ANTS-47" in err
    code, _, err = ants(*args, env=REPRODUCE)
    assert code == 0 and "ANTS-47" in err


# --- ITK's reader -------------------------------------------------------------------------------


def test_itk_reader_defects(tmp_path):
    out = tmp_path / "out.nii.gz"
    # ITK-08: asked for x.nii.gz, ITK reads the voxels of x.nii.
    nib.Nifti1Image(np.full((4, 4, 4), 1.0, np.float32), AFFINE).to_filename(tmp_path / "x.nii")
    nib.Nifti1Image(np.full((4, 4, 4), 2.0, np.float32), AFFINE).to_filename(tmp_path / "x.nii.gz")
    code, _, err = ants("ImageMath", 3, out, "m", tmp_path / "x.nii.gz", 1, env=FIX)
    assert code == 0 and "ITK-08" in err and (nib.load(out).get_fdata() == 2).all()
    code, _, err = ants("ImageMath", 3, out, "m", tmp_path / "x.nii.gz", 1, env=REPRODUCE)
    assert code == 0 and "ITK-08" in err and (nib.load(out).get_fdata() == 1).all()
    # ITK-02: the first volume of an oblique 4D file read in 3D.
    theta = np.deg2rad(20)
    oblique = np.eye(4)
    oblique[:2, :2] = [[np.cos(theta), -np.sin(theta)], [np.sin(theta), np.cos(theta)]]
    bold = tmp_path / "bold.nii.gz"
    nib.Nifti1Image(np.ones((4, 4, 4, 3), np.float32), oblique).to_filename(bold)
    code, _, err = ants("ImageMath", 3, out, "m", bold, 2, env=FIX)
    assert code == 0 and "ITK-02" in err
    np.testing.assert_allclose(nib.load(out).affine[:3, :3], oblique[:3, :3], atol=1e-6)
    code, _, err = ants("ImageMath", 3, out, "m", bold, 2, env=REPRODUCE)
    assert code == 0 and "ITK-02" in err
    # ITK's identity direction is LPS: in RAS, x and y flip.
    np.testing.assert_allclose(nib.load(out).affine[:3, :3], np.diag([-1, -1, 1]), atol=1e-6)


# --- antsAI and antsAffineInitializer ----------------------------------------------------------


def _pair(tmp_path, dims=3):
    """A phantom and a copy shifted one voxel, as files."""
    if dims == 3:
        data = phantom((14, 12, 10))
        moved = np.roll(data, 1, axis=0)
        affine = AFFINE
    else:
        data = phantom((14, 12, 10))[:, :, 5]
        moved = np.roll(data, 1, axis=1)
        affine = np.diag([2.0, 2.0, 1.0, 1.0])
    a, b = tmp_path / f"a{dims}.nii.gz", tmp_path / f"b{dims}.nii.gz"
    nib.Nifti1Image(data, affine).to_filename(a)
    nib.Nifti1Image(moved.astype(np.float32), affine).to_filename(b)
    return a, b


def test_ai_alignment_with_iterations(tmp_path):
    a, b = _pair(tmp_path)
    # ANTS-53: ANTs runs a search with no start point and writes an empty file; corrected,
    # the alignment is written as with 0 iterations.
    plain = lx.ants.ai(a, b, transform="AlignGeometricCenters")
    fixed, fw = entries(lx.ants.ai, a, b, transform="AlignGeometricCenters", iterations=2)
    assert fw == ["ANTS-53"]
    np.testing.assert_array_equal(fixed.transform.parameters, plain.transform.parameters)
    with pytest.raises(lx.ants.AIError, match="ANTS-53"), pytest.warns(lx.UpstreamDefectWarning):
        lx.ants.ai(
            a, b, transform="AlignGeometricCenters", iterations=2, upstream_defects=REPRODUCE
        )
    out = tmp_path / "empty.mat"
    code, _, _ = ants(
        "antsAI", "-d", 3, "-m", f"Mattes[{a},{b}]", "-t", "AlignGeometricCenters", "-c", 2,
        "-o", out, env=REPRODUCE,
    )  # fmt: skip
    assert code == 0 and out.stat().st_size == 0


def test_ai_translation_grid(tmp_path):
    a, b = _pair(tmp_path, dims=2)
    kw = {"transform": "Rigid", "search_factor": 90, "arc_fraction": 0.5, "iterations": 1}
    grid = {"translation_search_grid": (2.0, (2.0, 0.0))}
    # ANTS-52: in 2D the rigid start points ignore the grid (corrected, it is named when the
    # best start point is a translated one).
    fixed, fw = entries(lx.ants.ai, a, b, **kw, **grid)
    ants_, aw = entries(lx.ants.ai, a, b, **kw, **grid, upstream_defects=REPRODUCE)
    without = lx.ants.ai(a, b, **kw)
    assert aw == ["ANTS-52"] and fw in ([], ["ANTS-52"])
    assert (fw == []) == np.array_equal(fixed.transform.parameters, ants_.transform.parameters)
    assert fixed.start_points == ants_.start_points == 3 * without.start_points
    np.testing.assert_array_equal(ants_.transform.parameters, without.transform.parameters)
    # ANTS-55: a 3D grid of one extent is read past the end of its vector.
    a3, b3 = _pair(tmp_path)
    short = {"translation_search_grid": (2.0, (2.0,))}
    with pytest.raises(lx.ants.AIError, match="ANTS-55"), pytest.warns(lx.UpstreamDefectWarning):
        lx.ants.ai(a3, b3, **kw, **short)
    _, w = entries(lx.ants.ai, a3, b3, **kw, **short, upstream_defects=REPRODUCE)
    assert w == ["ANTS-55"]


def test_ai_wrapper(tmp_path):
    a, b = _pair(tmp_path)
    res = lx.ants.ai(a, b, transform="Affine", search_factor=90, arc_fraction=0.5, iterations=2)
    assert res.transform.name == "AffineTransform_double_3_3"
    assert res.start_points == 27 and res.best_value is not None and res.values
    # In memory, the same result.
    img = lx.load(b)
    again = lx.ants.ai(a, img, transform="Affine", search_factor=90, arc_fraction=0.5, iterations=2)
    np.testing.assert_array_equal(again.transform.parameters, res.transform.parameters)
    with pytest.raises(lx.ants.AIError, match="does not exist"):
        lx.ants.ai(a, tmp_path / "missing.nii.gz")
    with pytest.raises(ValueError, match="fixed mask"):
        lx.ants.ai(a, b, moving_mask=a)


def test_affine_initializer(tmp_path):
    a, b = _pair(tmp_path)
    t = lx.ants.affine_initializer(a, b, search_factor=45, principal_axes=True)
    assert t.name == "AffineTransform_double_3_3" and t.parameters.size == 12
    # ANTS-54 (questionable, reproduced): the local search's iterations are not read.
    other = lx.ants.affine_initializer(
        a, b, search_factor=45, principal_axes=True, local_search_iterations=3
    )
    np.testing.assert_array_equal(other.parameters, t.parameters)
    # ANTS-56: a dimension other than 2, 3 or 4 runs as 2D; corrected, an error.
    a2, b2 = _pair(tmp_path, dims=2)
    out = tmp_path / "t.mat"
    code, _, err = ants("antsAffineInitializer", 7, a2, b2, out, env=FIX)
    assert code == 1 and "ANTS-56" in err
    code, _, err = ants("antsAffineInitializer", 7, a2, b2, out, env=REPRODUCE)
    assert code == 0 and "ANTS-56" in err and out.exists()
    with pytest.raises(lx.ants.AffineInitializerError, match="zero image2 error"):
        lx.ants.affine_initializer(a, lx.load(a).with_data(np.zeros((14, 12, 10), np.float32)))


# --- SciPy --------------------------------------------------------------------------------------


def test_scipy_integer_outputs():
    # SCI-02: NaN into an int32 output.
    data = 1.0 + np.arange(27, dtype=np.float64).reshape(3, 3, 3)
    data[2, 2, 2] = np.nan
    coords = np.array([[0.0, 2.0], [0.0, 2.0], [0.0, 2.0]])
    kw = {"output": np.int32, "order": 1, "mode": "nearest"}
    fixed, fw = entries(lx.ndimage.map_coordinates, data, coords, **kw)
    ants_, aw = entries(lx.ndimage.map_coordinates, data, coords, upstream_defects=REPRODUCE, **kw)
    assert fw == aw == ["SCI-02"]
    assert fixed.tolist() == [1, 0]
    assert ants_.tolist() == [1, np.iinfo(np.int32).min]
    # Without a NaN, nothing to say.
    _, w = entries(lx.ndimage.map_coordinates, data[:1], coords[:, :1] * 0, output=np.int32)
    assert w == []

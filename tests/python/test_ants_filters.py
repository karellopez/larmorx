# SPDX-License-Identifier: Apache-2.0
"""``lx.ants`` image programs (ImageMath, ThresholdImage, MultiplyImages): behaviour that does
not need ANTs (parity with ANTs is in ``tests/parity`` and ``validation/``)."""

from __future__ import annotations

import contextlib
import io

import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

AFFINE = np.diag([2.0, 2.0, 3.0, 1.0])


def image(data, affine=AFFINE):
    return lx.Image(np.asfortranarray(np.asarray(data, dtype=np.float32)), affine)


def ramp(shape=(6, 5, 4)):
    i, j, k = np.meshgrid(*(np.arange(n, dtype=np.float32) for n in shape), indexing="ij")
    return image(1.0 + 2.0 * i - 0.5 * j + 3.0 * k)


def run(argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "ants", *argv])
    return code, out.getvalue(), err.getvalue()


# --- ThresholdImage -----------------------------------------------------------------------------


def test_threshold_is_inclusive_float32_and_keeps_geometry():
    img = image([[[0.0, 0.5, 0.75, 1.0, 1.5]]])
    out = lx.ants.threshold_image(img, 0.5, 1.0)
    assert out.data.dtype == np.float32
    np.testing.assert_array_equal(out.data.ravel(), [0, 1, 1, 1, 0])
    np.testing.assert_array_equal(out.affine, AFFINE)
    out = lx.ants.threshold_image(img, 0.5, 1.0, inside=7, outside=-2)
    np.testing.assert_array_equal(out.data.ravel(), [-2, 7, 7, 7, -2])
    with pytest.raises(ValueError, match="Lower threshold cannot be greater"):
        lx.ants.threshold_image(img, 2.0, 1.0)


def test_otsu_labels_and_mask():
    rng = np.random.default_rng(0)
    data = np.concatenate([rng.normal(10, 1, 500), rng.normal(50, 2, 500)]).reshape(10, 10, 10)
    r = lx.ants.otsu_threshold(image(data), 1)
    # Every threshold between the clusters ties; the first (lowest) bin wins.
    assert len(r.thresholds) == 1 and 12 < r.thresholds[0] < 46
    assert set(np.unique(r.image.data)) == {0.0, 1.0}
    mask = (np.arange(1000).reshape(10, 10, 10) % 2).astype(np.int16)
    r = lx.ants.otsu_threshold(image(data), 1, mask=mask)
    assert set(np.unique(r.image.data[mask == 1])) <= {1.0, 2.0}
    assert not r.image.data[mask == 0].any()
    with pytest.raises(ValueError, match="thresholds"):
        lx.ants.otsu_threshold(image(data), 0)


# --- ImageMath ----------------------------------------------------------------------------------


def test_generic_image_math_equals_the_typed_functions():
    img = ramp()
    for op, operand in [("m", 2.5), ("+", 1), ("abs", None), ("exp", 0.01)]:
        operands = [] if operand is None else [operand]
        generic = lx.ants.image_math(op, img, *operands)
        typed = lx.ants.image_arithmetic(img, op, 1.0 if operand is None else operand)
        np.testing.assert_array_equal(generic.data, typed.data, err_msg=op)
        np.testing.assert_allclose(generic.affine, img.affine, atol=1e-12)
    t1 = lx.ants.image_math("TruncateImageIntensity", img, 0.05, 0.95, 32)
    t2 = lx.ants.truncate_image_intensity(img, 0.05, 0.95, 32)
    np.testing.assert_array_equal(t1.data, t2.data)


def test_division_keeps_the_previous_value_where_the_divisor_is_not_positive():
    a = image([[[6.0, 8.0, 10.0, 12.0]]])
    b = image([[[2.0, 0.0, -1.0, 4.0]]])
    out = lx.ants.image_arithmetic(a, "divide", b)
    np.testing.assert_array_equal(out.data.ravel(), [3, 3, 3, 3])


def test_add_to_zero_and_running_total():
    a = image([[[0.0, 1.0, 0.0, 2.0]]])
    b = image([[[5.0, 5.0, 5.0, 5.0]]])
    np.testing.assert_array_equal(lx.ants.add_to_zero(a, b).data.ravel(), [5, 1, 5, 2])
    total = lx.ants.image_arithmetic(a, "total", 1.0)
    np.testing.assert_array_equal(total.data.ravel(), [0, 1, 1, 3])


def test_truncation_normalize_rescale():
    data = np.arange(1, 1001, dtype=np.float32).reshape(10, 10, 10)
    out = lx.ants.truncate_image_intensity(image(data), 0.01, 0.99, 100)
    assert out.data.min() > 1 and out.data.max() < 1000
    assert lx.ants.normalize_image(image(data)).data.max() == 1.0
    np.testing.assert_allclose(
        lx.ants.normalize_image(image(data), by_mean=True).data.mean(), 1.0, rtol=1e-5
    )
    r = lx.ants.rescale_image(image(data), -1, 1)
    assert (r.data.min(), r.data.max()) == (-1.0, 1.0)


def test_multiply_images():
    img = ramp()
    np.testing.assert_array_equal(lx.ants.multiply_images(img, 2).data, img.data * 2)
    np.testing.assert_array_equal(lx.ants.multiply_images(img, img).data, img.data * img.data)
    with pytest.raises(FileNotFoundError):
        lx.ants.multiply_images(img, "does-not-exist.nii.gz")


def test_operations_and_errors():
    ops = lx.ants.image_math_operations()
    assert {"m", "addtozero", "TruncateImageIntensity", "Normalize"} <= set(ops)
    with pytest.raises(lx.ants.ImageMathError, match="not found"):
        lx.ants.image_math("Bogus", ramp())
    with pytest.raises(lx.ants.ImageMathError, match="not supported yet"):
        lx.ants.image_math("Canny", ramp(), 1, 0, 1)


# --- Gaussian group -----------------------------------------------------------------------------


def textured(shape=(9, 8, 7), affine=AFFINE, seed=0):
    rng = np.random.default_rng(seed)
    return image(rng.uniform(0, 100, shape), affine)


def test_typed_gaussian_functions_equal_image_math():
    img = textured()
    pairs = [
        # ANTs reads Laplacian's normalize flag from the sigma argument: stoi("1.5") is 1.
        (lx.ants.laplacian(img, 1.5, normalize=True), ("Laplacian", 1.5, 0)),
        (lx.ants.laplacian(img, 0.8), ("Laplacian", 0.8)),
        (lx.ants.gradient_magnitude(img, 1.2), ("Grad", 1.2)),
        (lx.ants.gradient_magnitude(img, 2.0, normalize=True), ("Grad", 2.0, 1)),
        (lx.ants.discrete_gaussian(img, 1.5), ("G", 1.5)),
        (lx.ants.discrete_gaussian(img, [1.0, 2.0, 0.5]), ("G", "1x2x0.5")),
        (lx.ants.unsharp_mask(img, 0.7, 1.5, 2.0), ("UnsharpMask", 0.7, 1.5, 2.0)),
    ]
    for typed, (op, *operands) in pairs:
        generic = lx.ants.image_math(op, img, *operands)
        np.testing.assert_array_equal(typed.data, generic.data, err_msg=op)
        assert typed.data.dtype == np.float32
    lap = lx.ants.laplacian(img, 1.5, normalize=True).data
    assert (lap.min(), lap.max()) == (0.0, 1.0)


def test_smooth_image_modes_threads_and_dimensions():
    img = textured()
    a = lx.ants.smooth_image(img, 1.0, n_threads=1)
    b = lx.ants.smooth_image(img, [1.0, 1.0, 1.0], n_threads=4)
    np.testing.assert_array_equal(a.data, b.data)
    assert np.abs(a.data - img.data).max() > 1  # it smooths
    # One voxel is 2 × 2 × 3 mm.
    physical = lx.ants.smooth_image(img, [2.0, 2.0, 3.0], sigma_in_physical_units=True)
    np.testing.assert_allclose(physical.data, a.data, atol=1e-4)
    med = lx.ants.smooth_image(img, 1, median=True)
    assert set(np.unique(med.data)) <= set(np.unique(img.data))
    # 2D and 4D are filtered in their own dimensions; a 4D image along time too.
    two = lx.ants.smooth_image(image(img.data[:, :, 0]), 1.0)
    assert two.shape == (9, 8)
    series = image(np.random.default_rng(1).uniform(0, 1, (6, 5, 4, 8)))
    four = lx.ants.smooth_image(series, 1.0)
    assert four.shape == series.shape
    assert not np.allclose(
        four.data[..., 0], lx.ants.smooth_image(image(series.data[..., 0]), 1).data
    )
    with pytest.raises(ValueError, match="sigmas"):
        lx.ants.smooth_image(img, [1.0, 2.0])
    with pytest.raises(ValueError, match="less than 4"):
        lx.ants.smooth_image(image(np.ones((9, 8, 3))), 1.0)


def test_resample_image_by_spacing_grid_and_command_line(tmp_path):
    img = textured((10, 9, 8))
    out = lx.ants.resample_image_by_spacing(img, [4.0, 4.0, 3.0])
    assert out.shape == (5, 4, 8)
    np.testing.assert_allclose(out.affine[:3, :3], np.diag([4.0, 4.0, 3.0]), atol=1e-12)
    np.testing.assert_allclose(out.affine[:3, 3], img.affine[:3, 3], atol=1e-12)
    src, dst = tmp_path / "in.nii.gz", tmp_path / "out.nii.gz"
    lx.save(img, src)
    code, stdout, _ = run(["ResampleImageBySpacing", "3", str(src), str(dst), "4", "4", "3"])
    assert code == 0 and " output size [5, 4, 8]" in stdout
    np.testing.assert_array_equal(lx.load(dst).data, out.data)
    from_path = lx.ants.resample_image_by_spacing(src, [4.0, 4.0, 3.0], n_threads=3)
    np.testing.assert_array_equal(from_path.data, out.data)
    nn = lx.ants.resample_image_by_spacing(img, [1.0, 1.0, 3.0], smooth=False, nearest=True)
    assert nn.shape == (20, 18, 8)
    assert set(np.unique(nn.data)) <= set(np.unique(img.data))
    code, _, _ = run(["SmoothImage", "3", str(src), "1.5", str(dst)])
    assert code == 0
    np.testing.assert_array_equal(lx.load(dst).data, lx.ants.smooth_image(src, 1.5).data)


# --- command lines ------------------------------------------------------------------------------


def test_command_lines_on_files(tmp_path):
    src = tmp_path / "in.nii.gz"
    lx.save(ramp(), src)
    out = tmp_path / "out.nii.gz"
    code, stdout, _ = run(["ImageMath", "3", str(out), "total", str(src), "1"])
    assert code == 0
    assert stdout.startswith("total: ")
    assert lx.load(out).data.dtype == np.float32
    code, _, _ = run(["ThresholdImage", "3", str(src), str(out), "10", "20"])
    assert code == 0
    assert set(np.unique(lx.load(out).data)) <= {0.0, 1.0}
    code, _, _ = run(["MultiplyImages", "3", str(src), "0.5", str(out)])
    assert code == 0
    np.testing.assert_array_equal(lx.load(out).data, ramp().data * 0.5)


def test_command_line_errors_follow_ants(tmp_path):
    code, stdout, _ = run(["ImageMath", "3", "o.nii.gz", "m"])
    assert code == 1 and "Usage: ImageMath" in stdout
    code, stdout, _ = run(["ImageMath", "--help"])
    assert code == 0
    code, stdout, _ = run(["ImageMath", "7", "o.nii.gz", "m", "a.nii.gz"])
    assert (code, stdout) == (1, " Dimension 7 is not supported \n")
    missing = str(tmp_path / "missing.nii.gz")
    code, _, err = run(["ImageMath", "3", str(tmp_path / "o.nii.gz"), "m", missing, "2"])
    assert code == 1 and "does not exist" in err
    code, stdout, _ = run(["ThresholdImage", "9", "a", "b", "1", "2"])
    assert (code, stdout) == (1, "Unsupported dimension\n")

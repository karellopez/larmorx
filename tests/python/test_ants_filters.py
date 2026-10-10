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


def test_resample_image_types_interpolators_and_command_line(tmp_path):
    img = textured((10, 9, 8))
    src, dst = tmp_path / "in.nii.gz", tmp_path / "out.nii.gz"
    lx.save(img, src)
    out = lx.ants.resample_image(img, 1.5)
    assert out.shape == (13, 12, 16) and out.data.dtype == np.float32
    np.testing.assert_allclose(np.diag(out.affine)[:3], [1.5, 1.5, 1.5], atol=1e-12)
    for args, kw in [
        (["1.5"], {}),
        (["1.5", "0", "1"], {"interpolation": "nearest"}),
        (["0.8", "0", "4", "5"], {"interpolation": "bspline", "order": 5, "pixel_type": "uint"}),
        (["1.2", "0", "3"], {"interpolation": "sinc"}),
        (["2", "0", "2"], {"interpolation": "gaussian"}),
        (["20x18x16", "1"], {"size": [20, 18, 16]}),
        (["1", "0", "0", "2"], {"pixel_type": "short"}),
    ]:
        code, _, err = run(["ResampleImage", "3", str(src), str(dst), *args])
        assert code == 0, err
        spacing = None if "size" in kw else float(args[0])
        typed = lx.ants.resample_image(src, spacing, **kw)
        written = lx.load(dst)
        assert written.data.dtype == typed.data.dtype, args
        np.testing.assert_array_equal(written.data, typed.data, err_msg=str(args))
    by_size = lx.ants.resample_image(img, size=[19, 17, 15])
    np.testing.assert_allclose(np.diag(by_size.affine)[:3], [1.0, 1.0, 1.5], atol=1e-12)
    two = lx.ants.resample_image(image(img.data[:, :, 0]), 0.5, interpolation="nearest")
    assert two.shape == (40, 36)
    with pytest.raises(ValueError, match="3D"):
        lx.ants.resample_image(image(img.data[:, :, 0]), 0.5, interpolation="bspline")
    with pytest.raises(ValueError, match="either"):
        lx.ants.resample_image(img)
    code, stdout, _ = run(["ResampleImage", "3", str(src), str(dst), "1", "0", "0", "9"])
    assert (code, stdout) == (1, "Unsupported pixel type\n")


# --- Morphology group ---------------------------------------------------------------------------


def cube_mask(shape=(11, 11, 11)):
    d = np.zeros(shape, np.float32)
    d[2:9, 2:9, 2:9] = 1.0
    d[5, 5, 5] = 0.0  # a one-voxel hole
    d[0, 0, 0] = 1.0  # a one-voxel object
    return image(d)


def test_typed_morphology_equals_image_math():
    mask = cube_mask()
    img = textured((9, 8, 7))
    pairs = [
        (lx.ants.morphological_dilate(mask, 2), ("MD", mask, 2)),
        (lx.ants.morphological_erode(mask, 1), ("ME", mask, 1)),
        (lx.ants.morphological_open(mask, 1), ("MO", mask, 1)),
        (lx.ants.morphological_close(mask, 1), ("MC", mask, 1)),
        (lx.ants.morphological_dilate(mask, 1, value=0.0), ("MD", mask, 1, 0)),
        (lx.ants.grayscale_dilate(img, 2), ("GD", img, 2)),
        (lx.ants.grayscale_erode(img, 1), ("GE", img, 1)),
        (lx.ants.grayscale_open(img, 1), ("GO", img, 1)),
        (lx.ants.grayscale_close(img, 2), ("GC", img, 2)),
        (lx.ants.fill_holes(mask), ("FillHoles", mask, 2)),
        (lx.ants.fill_holes(mask, 0.5), ("FillHoles", mask, 0.5)),
    ]
    for typed, (op, *operands) in pairs:
        generic = lx.ants.image_math(op, *operands)
        np.testing.assert_array_equal(typed.data, generic.data, err_msg=op)
        assert typed.data.dtype == np.float32
    # The ball of radius 1 in 3D is the 3 x 3 x 3 cube without its corners.
    point = np.zeros((5, 5, 5), np.float32)
    point[2, 2, 2] = 1
    assert lx.ants.morphological_dilate(image(point), 1).data.sum() == 19
    assert lx.ants.morphological_dilate(image(point[:, :, 2]), 1).data.sum() == 9
    closed = lx.ants.morphological_close(mask, 1).data
    assert closed[5, 5, 5] == 1 and lx.ants.fill_holes(mask).data[5, 5, 5] == 1
    assert lx.ants.morphological_open(mask, 1).data[0, 0, 0] == 0
    # ME's output is 0/1, and voxels above 0.5 that are not the foreground stay 1.
    labels = mask.data * 2
    eroded = lx.ants.morphological_erode(image(labels), 1).data
    np.testing.assert_array_equal(eroded, (labels > 0.5).astype(np.float32))
    for n in (1, 4):
        np.testing.assert_array_equal(
            lx.ants.grayscale_close(img, 2, n_threads=n).data, pairs[8][0].data
        )
    with pytest.raises(ValueError, match="radius"):
        lx.ants.morphological_dilate(mask, -1)
    with pytest.raises(ValueError, match="unknown operation"):
        lx.ants.morphology(mask, "XX")


def test_pad_image_moves_the_origin(tmp_path):
    img = textured((6, 5, 4))
    padded = lx.ants.pad_image(img, 3, value=-1)
    assert padded.shape == (12, 11, 10)
    np.testing.assert_array_equal(padded.data[3:9, 3:8, 3:7], img.data)
    assert (padded.data[:3] == -1).all()
    # Voxel (3, 3, 3) of the padded image is where voxel (0, 0, 0) was.
    np.testing.assert_allclose(padded.affine @ [3, 3, 3, 1], img.affine @ [0, 0, 0, 1])
    back = lx.ants.pad_image(padded, -3)
    np.testing.assert_array_equal(back.data, img.data)
    np.testing.assert_allclose(back.affine, img.affine)
    generic = lx.ants.image_math("PadImage", img, 3, -1)
    np.testing.assert_array_equal(generic.data, padded.data)
    np.testing.assert_allclose(generic.affine, padded.affine)
    src = tmp_path / "in.nii.gz"
    lx.save(img, src)
    from_path = lx.ants.pad_image(src, 2.5)
    assert from_path.shape == (11, 10, 9)
    np.testing.assert_array_equal(from_path.data[2:8, 2:7, 2:6], img.data)
    with pytest.raises(ValueError, match="voxels"):
        lx.ants.pad_image(img, -2)


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

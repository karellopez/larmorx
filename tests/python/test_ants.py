# SPDX-License-Identifier: Apache-2.0
"""``lx.transforms`` and ``lx.ants``: behaviour that does not need ANTs (parity with ANTs is in
``tests/parity`` and ``validation/``)."""

from __future__ import annotations

import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run
from larmorx.transforms import ItkTransform

LPS = np.diag([-1.0, -1.0, 1.0, 1.0])


def affine_transform(matrix=None, translation=(0.0, 0.0, 0.0), center=(0.0, 0.0, 0.0)):
    matrix = np.eye(3) if matrix is None else np.asarray(matrix, float)
    params = np.concatenate([matrix.ravel(), np.asarray(translation, float)])
    return ItkTransform("AffineTransform_double_3_3", params, np.asarray(center, float))


def ramp_image(shape=(12, 10, 8), affine=None):
    i, j, k = np.meshgrid(*(np.arange(n, dtype=float) for n in shape), indexing="ij")
    data = 1.0 + 2.0 * i - 0.5 * j + 3.0 * k
    return lx.Image(np.asfortranarray(data), np.eye(4) if affine is None else affine)


# --- transform files ----------------------------------------------------------------------------


@pytest.mark.parametrize("suffix", [".txt", ".tfm", ".mat", ".h5"])
def test_linear_transforms_round_trip(tmp_path, suffix):
    t = affine_transform(np.diag([1.1, 0.9, 1.0]), (1.0, -2.0, 0.5), (3.0, 4.0, 5.0))
    path = tmp_path / f"xfm{suffix}"
    lx.transforms.write(path, t)
    (back,) = lx.transforms.read(path)
    assert back.name == t.name
    np.testing.assert_array_equal(back.parameters, t.parameters)
    np.testing.assert_array_equal(back.fixed_parameters, t.fixed_parameters)


def test_h5_composites_keep_float32_fields(tmp_path):
    size = (4, 3, 2)
    fixed = np.array([*size, 0, 0, 0, 1, 1, 1, *np.eye(3).ravel()], float)
    field = np.arange(3 * np.prod(size), dtype=np.float32) / 10
    parts = [
        ItkTransform("CompositeTransform_float_3_3", np.zeros(0), np.zeros(0)),
        affine_transform(translation=(1.0, 0.0, 0.0)),
        ItkTransform("DisplacementFieldTransform_float_3_3", field, fixed),
    ]
    path = tmp_path / "composite.h5"
    lx.transforms.write(path, parts)
    back = lx.transforms.read(path)
    assert [p.name for p in back] == [p.name for p in parts]
    assert back[2].parameters.dtype == np.float32
    np.testing.assert_array_equal(back[2].parameters, field)


def test_unknown_formats_are_rejected(tmp_path):
    with pytest.raises(ValueError, match="unknown transform file type"):
        lx.transforms.read(tmp_path / "x.xfm")
    with pytest.raises(ValueError, match=r"\.mat file holds exactly one"):
        lx.transforms.write(tmp_path / "x.mat", [affine_transform(), affine_transform()])


# --- apply_transforms ---------------------------------------------------------------------------


def test_identity_reproduces_the_image():
    img = ramp_image()
    for interpolation in ["linear", "nearest", "bspline", "lanczos", "GenericLabel"]:
        out = lx.ants.apply_transforms(img, img, interpolation=interpolation)
        np.testing.assert_allclose(out.data, img.data, rtol=0, atol=1e-9, err_msg=interpolation)
        np.testing.assert_array_equal(out.affine, img.affine)


def test_translation_moves_the_image_and_fills_outside():
    img = ramp_image()
    # Output point x reads the input at x + 1 voxel along LPS x, i.e. RAS -x: index i - 1.
    shift = affine_transform(translation=(1.0, 0.0, 0.0))
    out = lx.ants.apply_transforms(img, img, [shift], default_value=-7.0)
    np.testing.assert_allclose(out.data[1:], img.data[:-1])
    assert np.all(out.data[0] == -7.0)


def test_chain_order_and_inversion_on_points():
    scale = affine_transform(np.diag([2.0, 2.0, 2.0]))
    move = affine_transform(translation=(1.0, 0.0, 0.0))
    p = np.array([[3.0, 0.0, 0.0]])
    # The first transform acts first on points: 2·3 + 1.
    np.testing.assert_allclose(lx.ants.apply_transforms_to_points(p, [scale, move]), [[7, 0, 0]])
    np.testing.assert_allclose(
        lx.ants.apply_transforms_to_points(p, [scale, move], invert=[True, False]),
        [[2.5, 0, 0]],
    )
    # RAS coordinates: x and y flip on the way in and out.
    np.testing.assert_allclose(
        lx.ants.apply_transforms_to_points([[-3.0, 1.0, 2.0, 9.0]], [move], coordinates="ras"),
        [[-4.0, 1.0, 2.0, 9.0]],
    )


def test_time_series_and_output_types():
    img = ramp_image()
    series = lx.Image(np.stack([img.data, -img.data], axis=-1), img.affine)
    out = lx.ants.apply_transforms(series, img, dtype="int16")
    assert out.shape == (*img.shape, 2) and out.dtype == np.int16
    np.testing.assert_array_equal(out.data[..., 1], np.trunc(-img.data).astype(np.int16))
    with pytest.raises(ValueError, match="time_series=True"):
        lx.ants.apply_transforms(series, img, time_series=False)
    single = lx.ants.apply_transforms(img, img, time_series=True)
    assert single.shape == (*img.shape, 1)


def test_reference_grid_and_files(tmp_path):
    img = ramp_image(affine=np.diag([2.0, 2.0, 2.0, 1.0]))
    img.save(tmp_path / "in.nii.gz")
    ref = lx.Image(np.zeros((6, 5, 4), np.float32), np.diag([4.0, 4.0, 4.0, 1.0]))
    ref.save(tmp_path / "ref.nii")
    from_files = lx.ants.apply_transforms(tmp_path / "in.nii.gz", tmp_path / "ref.nii")
    in_memory = lx.ants.apply_transforms(img, ref)
    np.testing.assert_array_equal(from_files.data, in_memory.data)
    np.testing.assert_allclose(from_files.affine, ref.affine)
    # Voxel (1, 1, 1) of the reference is at 4 mm: voxel (2, 2, 2) of the input.
    assert from_files.data[1, 1, 1] == img.data[2, 2, 2]


def test_bad_arguments():
    img = ramp_image()
    with pytest.raises(ValueError, match="unknown interpolation"):
        lx.ants.apply_transforms(img, img, interpolation="cubic")
    with pytest.raises(ValueError, match="invert flags"):
        lx.ants.apply_transforms(img, img, [affine_transform()], invert=[True, False])
    field = ItkTransform(
        "DisplacementFieldTransform_double_3_3",
        np.zeros(3 * 8),
        np.array([2, 2, 2, 0, 0, 0, 1, 1, 1, *np.eye(3).ravel()], float),
    )
    with pytest.raises(ValueError, match="cannot be inverted"):
        lx.ants.apply_transforms(img, img, [field], invert=[True])


# --- command line -------------------------------------------------------------------------------


def test_cli_matches_the_python_api(tmp_path, capsys):
    img = ramp_image(affine=np.diag([2.0, 2.0, 2.0, 1.0]))
    img.save(tmp_path / "in.nii.gz")
    lx.transforms.write(tmp_path / "shift.mat", affine_transform(translation=(0.5, -1.0, 2.0)))
    args = ["-d", "3", "-i", str(tmp_path / "in.nii.gz"), "-r", str(tmp_path / "in.nii.gz")]
    args += ["-t", f"[{tmp_path / 'shift.mat'},1]", "-n", "BSpline[5]", "-u", "float"]
    code = cli_run(["lx", "ants", "antsApplyTransforms", *args, "-o", str(tmp_path / "out.nii.gz")])
    assert code == 0, capsys.readouterr().err
    written = lx.load(tmp_path / "out.nii.gz")
    expected = lx.ants.apply_transforms(
        tmp_path / "in.nii.gz",
        tmp_path / "in.nii.gz",
        [tmp_path / "shift.mat"],
        invert=[True],
        interpolation="bspline",
        order=5,
        dtype=np.float32,
    )
    assert written.dtype == np.float32
    np.testing.assert_array_equal(written.data, expected.data)
    header = lx.io.read_header(tmp_path / "out.nii.gz")
    assert (header.qform_code, header.sform_code) == (1, 1)


@pytest.mark.parametrize(
    ("args", "message"),
    [
        (["-e", "1", "-i", "a", "-r", "b", "-o", "c"], "not supported yet"),
        (["-d", "2", "-i", "a", "-r", "b", "-o", "c"], "only 3D"),
        (["-i", "a", "-r", "b", "-o", "Linear[x.mat]"], "not supported yet"),
        (["--nope"], "Invalid flag"),
        (["-r", "b", "-o", "c"], "An input image is required"),
    ],
)
def test_cli_errors(args, message, capsys):
    assert cli_run(["lx", "ants", "antsApplyTransforms", *args]) == 1
    assert message in capsys.readouterr().err

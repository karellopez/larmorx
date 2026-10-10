# SPDX-License-Identifier: Apache-2.0
"""``lx.ndimage``, ``lx.transforms`` chains and ``lx.transforms.resample_series``, without test
data. Bit-parity with SciPy, nitransforms and fMRIPrep is checked by the ``resample-series``
validation suite; the comparisons with SciPy here run only where SciPy is installed."""

from __future__ import annotations

import numpy as np
import pytest

import larmorx as lx


def _phantom(shape, seed=0):
    rng = np.random.default_rng(seed)
    idx = np.indices(shape, dtype=np.float64)
    smooth = sum(np.sin(0.4 * (i + 1) * g) for i, g in enumerate(idx))
    return (100 + 20 * smooth + rng.normal(0, 1, size=shape)).astype(np.float32)


# ------------------------------------------------------------------------------------------------
# lx.ndimage


def test_map_coordinates_reproduces_samples_at_integer_coordinates():
    data = _phantom((7, 6, 5))
    coords = np.indices(data.shape).reshape(3, -1).astype(np.float64)
    for order in range(6):
        out = lx.ndimage.map_coordinates(
            data, coords, order=order, mode="mirror", output=np.float64
        )
        np.testing.assert_allclose(out, data.ravel(), rtol=1e-9, atol=1e-9)


def test_map_coordinates_output_shape_and_dtype():
    data = _phantom((5, 5))
    coords = np.zeros((2, 3, 4)) + 1.5
    out = lx.ndimage.map_coordinates(data, coords, order=1)
    assert out.shape == (3, 4) and out.dtype == np.float32
    out = lx.ndimage.map_coordinates(data, coords, order=1, output=np.int16)
    assert out.dtype == np.int16


def test_map_coordinates_errors():
    data = _phantom((4, 4))
    with pytest.raises(RuntimeError, match="spline order"):
        lx.ndimage.map_coordinates(data, np.zeros((2, 1)), order=6)
    with pytest.raises(RuntimeError, match="boundary mode"):
        lx.ndimage.map_coordinates(data, np.zeros((2, 1)), mode="bogus")
    with pytest.raises(RuntimeError, match="invalid shape"):
        lx.ndimage.map_coordinates(data, np.zeros((3, 1)))
    with pytest.raises(TypeError):
        lx.ndimage.map_coordinates(data.astype(np.complex64), np.zeros((2, 1)))


def test_thread_count_does_not_change_results():
    data = _phantom((19, 17, 13))
    rng = np.random.default_rng(3)
    coords = rng.uniform(-3, 20, size=(3, 5000))
    a = lx.ndimage.map_coordinates(data, coords, mode="grid-constant", n_threads=1)
    b = lx.ndimage.map_coordinates(data, coords, mode="grid-constant", n_threads=4)
    assert a.tobytes() == b.tobytes()
    f1 = lx.ndimage.spline_filter(data, 3, "nearest", n_threads=1)
    f4 = lx.ndimage.spline_filter(data, 3, "nearest", n_threads=4)
    assert f1.tobytes() == f4.tobytes()


@pytest.mark.parametrize("mode", lx.ndimage.MODES)
@pytest.mark.parametrize("order", range(6))
def test_bit_identical_to_scipy(mode, order):
    ndi = pytest.importorskip("scipy.ndimage")
    rng = np.random.default_rng(order)
    data = (rng.normal(size=(8, 7, 5)) * 50).astype(np.float32)
    coords = rng.uniform(-4, 11, size=(3, 800))
    coords[:, :50] = np.round(coords[:, :50]) + 0.5
    coords[:, 50] = np.nan
    ref = ndi.map_coordinates(data, coords, order=order, mode=mode, cval=-1.5)
    got = lx.ndimage.map_coordinates(data, coords, order=order, mode=mode, cval=-1.5)
    assert np.array_equal(ref, got, equal_nan=True)
    if order >= 2:
        d = data.astype(np.float64)
        assert np.array_equal(
            ndi.spline_filter(d, order, mode=mode), lx.ndimage.spline_filter(d, order, mode)
        )


# ------------------------------------------------------------------------------------------------
# Transform chains


def _itk_text(path, mats, centres):
    lps = np.diag([-1.0, -1.0, 1.0, 1.0])
    lines = ["#Insight Transform File V1.0"]
    for i, (m, c) in enumerate(zip(mats, centres, strict=True)):
        p = lps @ m @ lps
        params = [*p[:3, :3].ravel(), *p[:3, 3]]
        lines += [
            f"#Transform {i}",
            "Transform: MatrixOffsetTransformBase_double_3_3",
            "Parameters: " + " ".join(f"{v:.17g}" for v in params),
            "FixedParameters: " + " ".join(f"{v:.17g}" for v in c),
        ]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _shift(x, y, z):
    m = np.eye(4)
    m[:3, 3] = (x, y, z)
    return m


def test_load_transforms_reads_itk_text_as_float32(tmp_path):
    _itk_text(tmp_path / "one.txt", [_shift(0.1, 2, -3)], [[0, 0, 0]])
    _itk_text(tmp_path / "many.txt", [_shift(1, 0, 0), _shift(0, 1, 0)], [[0, 0, 0]] * 2)
    one = lx.transforms.load_transforms([tmp_path / "one.txt"])
    assert isinstance(one, lx.transforms.Affine)
    # nitransforms parses the text as float32: 0.1 becomes float32(0.1).
    assert one.matrix[0, 3] == np.float64(np.float32(0.1))
    many = lx.transforms.load_transforms([tmp_path / "many.txt"])
    assert isinstance(many, lx.transforms.AffineSeries) and len(many) == 2
    chain = lx.transforms.load_transforms([tmp_path / "many.txt", tmp_path / "one.txt"])
    assert isinstance(chain, lx.transforms.TransformChain)
    assert isinstance(chain[-1], lx.transforms.AffineSeries)  # head motion last
    inv = lx.transforms.load_transforms([tmp_path / "one.txt"], inverse=True)
    np.testing.assert_allclose(inv.matrix @ one.matrix, np.eye(4), atol=1e-12)
    ident = lx.transforms.load_transforms([])
    assert np.array_equal(ident.matrix, np.eye(4))
    with pytest.raises(ValueError, match="Mismatched"):
        lx.transforms.load_transforms([tmp_path / "one.txt"], inverse=[True, False])


def test_chain_maps_points_through_affine_and_field():
    shift = lx.transforms.Affine(_shift(1.0, -2.0, 0.5))
    pts = np.array([[0.0, 0.0, 0.0], [10.0, 5.0, -3.0]])
    np.testing.assert_array_equal(shift.map(pts), pts + np.array([1.0, -2.0, 0.5]))
    deltas = np.zeros((5, 5, 5, 3))
    deltas[..., 0] = 0.25
    field = lx.transforms.DenseField(deltas, np.eye(4))
    chain = lx.transforms.TransformChain((field, shift))
    on_grid = np.array([[1.0, 2.0, 3.0]])
    np.testing.assert_allclose(chain.map(on_grid), [[2.25, 0.0, 3.5]])
    outside = np.array([[1.5, 2.0, 30.0]])  # off the grid and outside the field: not displaced
    np.testing.assert_allclose(chain.map(outside), [[2.5, 0.0, 30.5]])


# ------------------------------------------------------------------------------------------------
# resample_series


def _series(shape=(14, 12, 10), n=5):
    data = np.stack([_phantom(shape, seed=t) for t in range(n)], -1)
    affine = np.diag([2.0, 2.0, 2.5, 1.0])
    affine[:3, 3] = (-13, -11, -11)
    return data, affine


def test_identity_resampling_returns_the_series():
    data, affine = _series()
    out = lx.transforms.resample_series((data, affine), (data.shape[:3], affine), n_threads=2)
    assert out.shape == data.shape and out.dtype == np.float32
    np.testing.assert_allclose(out.data, data, rtol=1e-5)
    assert np.array_equal(out.affine, affine)


def test_head_motion_and_thread_invariance():
    data, affine = _series()
    hmc = lx.transforms.AffineSeries(np.stack([_shift(0.3 * t, -0.2 * t, 0.1) for t in range(5)]))
    fmap = np.linspace(-50, 50, np.prod(data.shape[:3]), dtype=np.float32).reshape(data.shape[:3])
    kw = dict(fieldmap=fmap, pe_dir="j-", ro_time=0.04)
    results = [
        lx.transforms.resample_series(
            (data, affine), (data.shape[:3], affine), hmc, n_threads=t, **kw
        ).data
        for t in (1, 2, 5, 0)
    ]
    for r in results[1:]:
        assert r.tobytes() == results[0].tobytes()
    assert not np.allclose(results[0], data)


def test_3d_source_and_target_header(tmp_path):
    data, affine = _series(n=1)
    vol = data[..., 0]
    out = lx.transforms.resample_series((vol, affine), (vol.shape, affine), order=1)
    assert out.shape == vol.shape
    np.testing.assert_allclose(out.data, vol, rtol=1e-6)
    lx.save(lx.Image(data, affine), tmp_path / "bold.nii.gz")
    lx.save(lx.Image(vol, affine), tmp_path / "ref.nii.gz")
    img = lx.transforms.resample_series(tmp_path / "bold.nii.gz", tmp_path / "ref.nii.gz")
    assert img.header is not None and img.header.data_dtype == np.float32
    assert img.header.zooms[:3] == pytest.approx((2.0, 2.0, 2.5))


def test_resample_series_errors():
    data, affine = _series()
    target = (data.shape[:3], affine)
    with pytest.raises(ValueError, match="pe_dir"):
        lx.transforms.resample_series((data, affine), target, pe_dir="x", ro_time=0.03)
    with pytest.raises(ValueError, match="field map"):
        lx.transforms.resample_series(
            (data, affine),
            target,
            fieldmap=np.zeros((3, 3, 3), np.float32),
            pe_dir="i",
            ro_time=0.03,
        )
    hmc = lx.transforms.AffineSeries(np.stack([np.eye(4)] * 5))
    chain = lx.transforms.TransformChain((hmc, lx.transforms.Affine(np.eye(4))))
    with pytest.raises(ValueError, match="must come last"):
        lx.transforms.resample_series((data, affine), target, chain)
    short = lx.transforms.AffineSeries(np.stack([np.eye(4)] * 2))
    with pytest.raises(IndexError):
        lx.transforms.resample_series((data, affine), target, short)


def test_ensure_positive_cosines_flips_las():
    data = np.arange(24, dtype=np.float32).reshape(2, 3, 4)
    affine = np.diag([-2.0, 2.0, 2.0, 1.0])
    new, new_affine, codes = lx.transforms.ensure_positive_cosines(data, affine)
    assert codes == ("L", "A", "S")
    assert np.array_equal(new, data[::-1])
    assert new_affine[0, 0] == 2.0 and new_affine[0, 3] == -2.0

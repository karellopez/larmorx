# SPDX-License-Identifier: Apache-2.0
"""lx.sdcflows without sdcflows: the control grid, the weights' structure, the warning."""

import warnings

import numpy as np
import pytest

import larmorx as lx


def _image(shape, zooms):
    aff = np.diag([*zooms, 1.0])
    aff[:3, 3] = -0.5 * np.asarray(zooms) * (np.asarray(shape) - 1)
    return lx.Image(np.zeros(shape, np.float32), aff)


def test_bspline_grid_covers_and_centres_the_image():
    img = _image((64, 64, 44), (3.75, 3.75, 4.0))
    grid = lx.sdcflows.bspline_grid(img, lx.sdcflows.DEFAULT_HF_ZOOMS_MM)
    assert grid.shape == (17, 17, 20)
    assert grid.dtype == np.float32
    np.testing.assert_array_equal(np.diag(grid.affine)[:3], [16.0, 16.0, 10.0])
    centre = grid.affine @ np.r_[0.5 * (np.asarray(grid.shape) - 1), 1.0]
    np.testing.assert_allclose(centre[:3], 0.0, atol=1e-12)


def test_weights_form_a_partition_of_unity_inside_the_knots():
    img = _image((20, 18, 7), (2.0, 2.5, 3.0))
    grid = lx.sdcflows.bspline_grid(img, lx.sdcflows.DEFAULT_HF_ZOOMS_MM)
    w = lx.sdcflows.grid_bspline_weights(img, grid)
    assert w.shape == (20 * 18 * 7, int(np.prod(grid.shape)))
    assert w.dtype == np.float32 and w.indices.dtype == np.int32
    # rows sorted, explicit zeros stored for the axes scipy's kron keeps dense (sdcflows' matrix)
    for r in range(0, w.shape[0], 97):
        assert np.all(np.diff(w.indices[w.indptr[r] : w.indptr[r + 1]]) > 0)
    assert w.nnz == 191_520 and int((w.data == 0).sum()) == 71_136
    # a partition of unity where four knots surround a sample on every axis (the centre); less
    # near the edges, where the grid has no knot beyond the outer ones
    sums = w.toarray().sum(axis=1)
    assert np.all(sums <= 1.0 + 1e-6) and np.all(sums > 0.5)
    centre = np.ravel_multi_index((10, 9, 3), (20, 18, 7))
    np.testing.assert_allclose(sums[centre], 1.0, rtol=1e-6)


def test_rotated_target_warns_and_two_levels_stack():
    img = _image((12, 10, 6), (3.0, 3.0, 3.0))
    grid = lx.sdcflows.bspline_grid(img, (16.0, 16.0, 10.0))
    c, s = np.cos(0.2), np.sin(0.2)
    rot = np.array([[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]])
    with pytest.warns(UserWarning, match="grids are not aligned"):
        lx.sdcflows.grid_bspline_weights(
            lx.Image(img.data, rot @ img.affine), grid, upstream_defects="reproduce"
        )
    coarse = lx.sdcflows.bspline_grid(img, lx.sdcflows.DEFAULT_ZOOMS_MM)
    both = lx.sdcflows.collocation_matrix(img, [coarse, grid])
    assert both.shape[1] == coarse.data.size + grid.data.size
    with pytest.raises(ValueError, match="float32 and float64"):
        lx.sdcflows.grid_bspline_weights(img, grid, dtype="float16")


def _beta3(u):
    a = np.abs(u)
    return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))


def test_sdc01_weights_beyond_the_grid_are_the_bsplines():
    img = _image((12, 10, 6), (3.0, 3.0, 3.0))
    grid = lx.sdcflows.bspline_grid(img, (16.0, 16.0, 10.0))
    larger = _image((24, 22, 12), (3.0, 3.0, 3.0))  # reaches beyond the control points
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-01"):
        fixed = lx.sdcflows.grid_bspline_weights(larger, grid, dtype="float64")
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-01"):
        sdc = lx.sdcflows.grid_bspline_weights(
            larger, grid, dtype="float64", upstream_defects="reproduce"
        )
    to_grid = np.linalg.inv(grid.affine) @ larger.affine
    ijk = np.indices(larger.shape).reshape(3, -1).T
    q = ijk @ to_grid[:3, :3].T + to_grid[:3, 3]
    dense = fixed.toarray()
    kk = np.indices(grid.shape).reshape(3, -1).T
    for r in (0, 517, 1300, dense.shape[0] - 1):
        want = np.prod(_beta3(q[r][None, :] - kk), axis=1)
        np.testing.assert_allclose(dense[r], want, atol=1e-14)
    # the rows inside the grid are the same bits in both modes; the others differ
    inside = np.all((q >= 0) & (q <= np.array(grid.shape) - 1), axis=1)
    sd = sdc.toarray()
    assert np.array_equal(dense[inside], sd[inside])
    assert not np.allclose(dense[~inside], sd[~inside])
    # aligned and inside: no notice, the same matrix
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        a = lx.sdcflows.grid_bspline_weights(img, grid)
    b = lx.sdcflows.grid_bspline_weights(img, grid, upstream_defects="reproduce")
    assert a.data.tobytes() == b.data.tobytes() and a.indices.tobytes() == b.indices.tobytes()


def test_sdc02_a_rotated_grid_gets_the_weights_at_every_voxel():
    img = _image((12, 10, 6), (3.0, 3.0, 3.0))
    grid = lx.sdcflows.bspline_grid(img, (16.0, 16.0, 10.0))
    c, s = np.cos(0.3), np.sin(0.3)
    rot = np.array([[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]])
    target = lx.Image(img.data, rot @ img.affine)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-02") as rec:
        w = lx.sdcflows.grid_bspline_weights(target, grid, dtype="float64")
    assert not any(str(x.message) == "grids are not aligned." for x in rec)
    to_grid = np.linalg.inv(grid.affine) @ target.affine
    q = np.indices(img.shape).reshape(3, -1).T @ to_grid[:3, :3].T + to_grid[:3, 3]
    kk = np.indices(grid.shape).reshape(3, -1).T
    dense = w.toarray()
    for r in (0, 100, 359, dense.shape[0] - 1):
        np.testing.assert_allclose(
            dense[r], np.prod(_beta3(q[r][None, :] - kk), axis=1), atol=1e-14
        )
    # the field of smooth coefficients: sdcflows' separable product is far from it
    coef = np.linspace(-50, 80, w.shape[1])
    with pytest.warns(UserWarning):
        sep = lx.sdcflows.grid_bspline_weights(
            target, grid, dtype="float64", upstream_defects="reproduce"
        ).toarray()
    assert np.abs(sep @ coef - dense @ coef).max() > 1.0


def test_sdc06_a_4d_header_of_one_volume():
    img = _image((12, 10, 6), (3.0, 3.0, 3.0))
    four = lx.Image(np.zeros((12, 10, 6, 1), np.float32), img.affine)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-06"):
        g4 = lx.sdcflows.bspline_grid(four, (16.0, 16.0, 10.0))
    g3 = lx.sdcflows.bspline_grid(img, (16.0, 16.0, 10.0))
    assert g4.shape == g3.shape and np.array_equal(g4.affine, g3.affine)
    with pytest.raises(ValueError, match="could not be broadcast"):
        lx.sdcflows.bspline_grid(four, (16.0, 16.0, 10.0), upstream_defects="reproduce")
    rng = np.random.default_rng(3)
    field = (rng.random((12, 10, 6)) * 40 - 20).astype(np.float32)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-06"):
        r4 = lx.sdcflows.bspline_approx(lx.Image(field[..., None], img.affine))
    r3 = lx.sdcflows.bspline_approx(lx.Image(field, img.affine))
    assert r4.field.shape == (12, 10, 6, 1)
    assert np.asarray(r4.field.data).tobytes() == np.asarray(r3.field.data).tobytes()
    with pytest.raises(ValueError, match="2 volumes"):
        lx.sdcflows.bspline_approx(lx.Image(np.stack([field, field], -1), img.affine))
    with pytest.raises(ValueError, match="could not be broadcast"):
        lx.sdcflows.bspline_approx(
            lx.Image(field[..., None], img.affine), upstream_defects="reproduce"
        )


def test_sdc10_the_approximation_follows_the_transform():
    img = _image((24, 20, 12), (3.0, 3.0, 3.0))
    grid = lx.sdcflows.bspline_grid(img, (16.0, 16.0, 10.0))
    coef = np.linspace(-60, 90, grid.data.size, dtype=np.float32).reshape(grid.shape)
    coeffs = lx.Image(coef, grid.affine)
    c, s = np.cos(0.1), np.sin(0.1)
    rot = np.array([[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]])
    target = lx.Image(np.zeros(img.shape, np.float32), rot @ img.affine)
    xfm = np.eye(4)
    xfm[:3, 3] = (4.0, -2.0, 1.0)
    projected = lx.Image(target.data, xfm @ target.affine)
    exact = lx.sdcflows.B0FieldTransform(coeffs=coeffs)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-02"):
        exact.fit(projected)  # no transform: evaluated at every voxel (SDC-02 corrected)
    approx = lx.sdcflows.B0FieldTransform(coeffs=coeffs)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-10"):
        approx.fit(target, xfm_data2fmap=xfm)
    sdc = lx.sdcflows.B0FieldTransform(coeffs=coeffs)
    with pytest.warns(lx.UpstreamDefectWarning, match="SDC-10"):
        sdc.fit(target, xfm_data2fmap=xfm, upstream_defects="reproduce")
    inner = (slice(4, -4), slice(4, -4), slice(3, -3))
    e = np.asarray(exact.mapped.data)[inner]
    assert np.abs(np.asarray(approx.mapped.data)[inner] - e).max() < 0.5
    assert np.abs(np.asarray(sdc.mapped.data)[inner] - e).max() > 2.0


def _csr(dense):
    dense = np.asarray(dense, np.float32)
    rows, cols = np.nonzero(dense)
    indptr = np.searchsorted(rows, np.arange(dense.shape[0] + 1))
    return lx.sdcflows.CsrMatrix(
        dense[rows, cols], cols.astype(np.int32), indptr.astype(np.int32), dense.shape
    )


def test_ridge_solves_both_systems():
    rng = np.random.default_rng(0)
    tall = rng.standard_normal((40, 6)) * (rng.random((40, 6)) < 0.5)
    y = rng.standard_normal(40).astype(np.float32)
    fit = lx.sdcflows.ridge_sparse_cg(_csr(tall), y, alpha=1e-3, tol=1e-8)
    a = tall.astype(np.float64)
    want = np.linalg.solve(a.T @ a + 1e-3 * np.eye(6), a.T @ y)
    np.testing.assert_allclose(fit.coef, want, rtol=1e-3, atol=1e-4)
    assert (fit.system, fit.info, fit.intercept, fit.coef.dtype) == ("normal", 0, 0.0, np.float32)
    wide = rng.standard_normal((5, 12))
    fit = lx.sdcflows.ridge_sparse_cg(_csr(wide), y[:5], alpha=1e-2, tol=1e-8)
    b = wide.astype(np.float64)
    want = b.T @ np.linalg.solve(b @ b.T + 1e-2 * np.eye(5), y[:5])
    np.testing.assert_allclose(fit.coef, want, rtol=1e-3, atol=1e-4)
    assert fit.system == "kernel"


def test_ridge_messages_are_scikit_learns():
    eye = _csr(np.eye(3))
    with pytest.raises(
        ValueError,
        match=r"The 'alpha' parameter of Ridge must be a float in the range \[0\.0, inf\) or an array-like\. Got -1\.0 instead\.",
    ):
        lx.sdcflows.ridge_sparse_cg(eye, np.ones(3), alpha=-1.0)
    with pytest.raises(ValueError, match=r"^Input y contains NaN\.$"):
        lx.sdcflows.ridge_sparse_cg(eye, np.array([1.0, np.nan, 1.0]))
    with pytest.raises(ValueError, match=r"inconsistent numbers of samples: \[3, 4\]"):
        lx.sdcflows.ridge_sparse_cg(eye, np.ones(4))
    with pytest.raises(ValueError, match="only float32"):
        lx.sdcflows.ridge_sparse_cg(
            lx.sdcflows.CsrMatrix(
                *[np.asarray(a) for a in (np.ones(3), [0, 1, 2], [0, 1, 2, 3])], (3, 3)
            ),
            np.ones(3),
        )


def test_unwarp_with_a_zero_field_is_the_identity_and_conversions_invert():
    shape = (12, 10, 6)
    img = _image(shape, (3.0, 3.0, 4.0))
    rng = np.random.default_rng(1)
    data = rng.uniform(10, 100, shape).astype(np.float32)
    grid = lx.sdcflows.bspline_grid(img, lx.sdcflows.DEFAULT_HF_ZOOMS_MM)
    t = lx.sdcflows.B0FieldTransform(coeffs=[grid])
    out = t.apply(lx.Image(data, img.affine), "j", 0.05, order=1)
    assert out.shape == (*shape, 1)
    np.testing.assert_array_equal(out.data[..., 0], data)
    field = lx.Image(rng.normal(0, 30, shape).astype(np.float32), img.affine)
    disp = lx.sdcflows.fmap_to_disp(field, 0.04, "i-")
    assert disp.shape == (*shape, 1, 3)
    back = lx.sdcflows.disp_to_fmap(disp, field, 0.04, "i-")
    np.testing.assert_allclose(back.data, field.data, rtol=1e-5, atol=1e-4)


def test_phase_maps_to_radians_and_hz():
    shape = (6, 5, 4)
    aff = np.diag([3.0, 3.0, 3.0, 1.0])
    ph = lx.Image(np.arange(120, dtype=np.int16).reshape(shape) - 60, aff)
    rads = lx.sdcflows.au2rads(ph).image
    assert rads.data.dtype == np.float32
    assert float(rads.data.min()) == 0.0 and float(rads.data.max()) <= np.float32(2 * np.pi)
    with pytest.raises(ValueError, match="SDC-04"):
        lx.sdcflows.au2rads(lx.Image(np.zeros(shape, np.int16), aff))
    assert lx.sdcflows.delta_te({"EchoTime1": 0.005, "EchoTime2": 0.0075}) == pytest.approx(0.0025)
    with pytest.raises(ValueError, match="no echo-times information"):
        lx.sdcflows.delta_te({})
    fmap = lx.sdcflows.phdiff2fmap(rads, 0.0025).image
    np.testing.assert_allclose(fmap.data, rads.data / (2 * np.pi * 0.0025), rtol=1e-6)


# --- relay 7: the random-testing trial's findings ----------------------------------------------


def _translated(shape, zooms, offset):
    aff = np.diag([*zooms, 1.0])
    aff[:3, 3] = offset
    return lx.Image(np.zeros(shape, np.float32), aff)


def test_field_is_float64_where_scipys_stacked_matrix_is():
    """scipy's kron of an axis with no weights is an empty float64 matrix, and the stack of the
    levels then float64: B0FieldTransform's field is float64, summed in float64."""
    fmap = _image((10, 10, 8), (2.0, 2.0, 2.5))
    coarse = lx.sdcflows.bspline_grid(fmap, (100.0, 100.0, 40.0))
    fine = lx.sdcflows.bspline_grid(fmap, (16.0, 16.0, 10.0))
    rng = np.random.default_rng(7)
    levels = [
        lx.Image(rng.uniform(-50, 50, g.shape).astype(np.float32), g.affine) for g in (coarse, fine)
    ]
    near = _translated((3, 3, 3), (2.0, 2.0, 2.0), (-2.0, -2.0, -2.0))
    t = lx.sdcflows.B0FieldTransform(coeffs=levels)
    t.fit(near)
    assert t.mapped.data.dtype == np.float32
    # beyond the fine grid's support, within the coarse grid's
    far = _translated((3, 3, 3), (2.0, 2.0, 2.0), (150.0, 0.0, 0.0))
    t = lx.sdcflows.B0FieldTransform(coeffs=levels)
    t.fit(far)
    assert t.mapped.data.dtype == np.float64
    w = lx.sdcflows.collocation_matrix(far, [coarse])
    assert lx.sdcflows.collocation_matrix(far, [fine]).nnz == 0 and w.nnz > 0
    coef = np.asarray(levels[0].data, np.float32).ravel()
    want = np.zeros(w.shape[0])
    for r in range(w.shape[0]):
        s = 0.0
        for jj in range(w.indptr[r], w.indptr[r + 1]):
            s += float(w.data[jj]) * float(coef[w.indices[jj]])
        want[r] = s
    assert np.asarray(t.mapped.data).ravel().tobytes() == want.tobytes()
    # no weights at all: float64 zeros
    t = lx.sdcflows.B0FieldTransform(coeffs=levels[1:])
    t.fit(far)
    assert t.mapped.data.dtype == np.float64 and not np.asarray(t.mapped.data).any()


def test_median_of_an_empty_mask_is_numpys_x86_nan(tmp_path):
    """numpy's median (and mean) of no values is 0/0's NaN on x86, its sign bit set; after a
    resize the error map is that NaN everywhere."""
    rng = np.random.default_rng(3)
    data = rng.uniform(-20, 20, (8, 6, 5)).astype(np.float32)
    fmap = lx.Image(data, np.diag([0.5, 1.0, 1.0, 1.0]))
    mask = lx.Image(np.zeros((8, 6, 5), np.uint8), fmap.affine)
    for recenter in ("median", "mean"):
        r = lx.sdcflows.bspline_approx(fmap, mask, recenter=recenter, zooms_min=1.0)
        err = np.asarray(r.error.data)
        assert np.isnan(err).all() and np.signbit(err).all()


def test_notices_only_where_a_value_changes(tmp_path):
    from larmorx._defects import UpstreamDefectWarning

    # SDC-02 on a single voxel: the separable product is the exact one
    fmap = _image((10, 10, 8), (2.0, 2.0, 2.5))
    grid = lx.sdcflows.bspline_grid(fmap, (40.0, 40.0, 20.0))
    coeff = lx.Image(np.ones(grid.shape, np.float32), grid.affine)
    rot = np.eye(4)
    c, s = np.cos(0.3), np.sin(0.3)
    rot[:2, :2] = [[c, -s], [s, c]]
    one = lx.Image(np.zeros((1, 1, 1), np.float32), rot)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        lx.sdcflows.B0FieldTransform(coeffs=[coeff]).fit(one)
    assert not [x for x in w if issubclass(x.category, UpstreamDefectWarning)]
    # ... and named on a rotated grid of several voxels
    many = lx.Image(np.zeros((4, 4, 3), np.float32), rot)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        lx.sdcflows.B0FieldTransform(coeffs=[coeff]).fit(many)
    assert "SDC-02" in " ".join(str(x.message) for x in w)


def test_sdc07_is_not_named_when_the_shape_is_not_topups(tmp_path):
    from larmorx._defects import UpstreamDefectWarning

    ref = lx.Image(np.zeros((8, 6, 4), np.float32), np.diag([-2.0, 2.0, 2.0, 1.0]))
    bad = lx.Image(np.ones((9, 9, 9), np.float32), np.diag([2.0, 2.0, 2.0, 1.0]))
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        with pytest.raises(ValueError, match="does not meet the expected shape"):
            lx.sdcflows.topup_coeff_reorient([bad], ref, "i")
    assert not [x for x in w if issubclass(x.category, UpstreamDefectWarning)]


def test_stored_and_low_memory_fits_give_the_same_bits():
    rng = np.random.default_rng(11)
    shape = (14, 12, 9)
    data = rng.normal(0, 30, shape).astype(np.float32)
    mask = (rng.uniform(size=shape) > 0.3).astype(np.uint8)
    fmap = lx.Image(data, np.diag([3.0, 3.0, 3.5, 1.0]))
    m = lx.Image(mask, fmap.affine)
    spacing = [(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)]
    a = lx.sdcflows.bspline_approx(fmap, m, bs_spacing=spacing, n_threads=1)
    b = lx.sdcflows.bspline_approx(fmap, m, bs_spacing=spacing, n_threads=1, low_memory=True)
    c = lx.sdcflows.bspline_approx(fmap, m, bs_spacing=spacing, n_threads=4)
    for x in (b, c):
        for ca, cb in zip(a.coefficients, x.coefficients, strict=True):
            assert np.asarray(ca.data).tobytes() == np.asarray(cb.data).tobytes()
        assert np.asarray(a.field.data).tobytes() == np.asarray(x.field.data).tobytes()


def test_infinity_message_names_numpys_dtype(tmp_path):
    data = np.zeros((4, 4, 4), np.float32)
    data[1, 2, 3] = np.inf
    p = tmp_path / "fmap.nii"
    lx.save(lx.Image(data, np.eye(4)), p)
    with pytest.raises(ValueError, match=r"dtype\('<f4'\)\.$"):
        lx.sdcflows.bspline_approx(p, recenter=False)
    with pytest.raises(ValueError, match=r"dtype\('float32'\)\.$"):
        lx.sdcflows.bspline_approx(lx.Image(data, np.eye(4)), recenter=False)


def test_a_3d_mask_fits_a_4d_field_map_of_one_volume():
    rng = np.random.default_rng(5)
    data = rng.normal(0, 30, (10, 9, 8)).astype(np.float32)
    aff = np.diag([3.0, 3.0, 3.0, 1.0])
    mask = lx.Image((rng.uniform(size=(10, 9, 8)) > 0.2).astype(np.uint8), aff)
    r3 = lx.sdcflows.bspline_approx(lx.Image(data, aff), mask)
    r4 = lx.sdcflows.bspline_approx(lx.Image(data[..., None], aff), mask)
    assert r4.field.shape == (10, 9, 8, 1)
    assert np.asarray(r3.field.data).tobytes() == np.asarray(r4.field.data).tobytes()


def test_resize_onto_a_single_voxel_sdc11():
    from larmorx._defects import UpstreamDefectWarning

    rng = np.random.default_rng(2)
    img = lx.Image(rng.uniform(-5, 5, (4, 4, 4)).astype(np.float32), np.eye(4))
    with pytest.raises(AssertionError):
        lx.sdcflows.resample_to_zooms(img, (4.0, 4.0, 4.0), upstream_defects="reproduce")
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        out = lx.sdcflows.resample_to_zooms(img, (4.0, 4.0, 4.0))
    assert out.shape == (1, 1, 1)
    assert any(
        issubclass(x.category, UpstreamDefectWarning) and "SDC-11" in str(x.message) for x in w
    )
    r = lx.sdcflows.bspline_approx(img, zooms_min=4.0)
    assert r.field.shape == (4, 4, 4) and np.isfinite(np.asarray(r.field.data)).all()


def test_error_map_in_fortran_order_for_numpys_max():
    """sdcflows' error map is a Fortran-ordered array (nibabel's), and numpy's max of it, which
    becomes cal_max, depends on that order (which NaN, which zero sign)."""
    rng = np.random.default_rng(9)
    fmap = lx.Image(rng.normal(0, 10, (9, 8, 7)).astype(np.float32), np.diag([2.0, 2.0, 2.0, 1]))
    r = lx.sdcflows.bspline_approx(fmap)
    assert np.asarray(r.error.data).flags.f_contiguous


def test_topup_configurations_fsldir_then_larmorx_ncs_copy(tmp_path, monkeypatch):
    """FSL's ``b02b0_{1,2,4}.cnf`` (D9 and the user's decision of 2026-11-09): from ``fsldir``,
    else from larmorx-nc's copy beside its program, else the name, which larmorx's topup finds
    among its own settings (clean-room, ``specs/topup.md`` §2.5); sdcflows' own files always
    from larmorx."""
    import pathlib

    from larmorx.sdcflows import pepolar

    prefix = tmp_path / "nc ~1"
    (prefix / "bin").mkdir(parents=True)
    program = prefix / "bin" / "larmorx-nc"
    program.write_text("")
    copy = prefix / "share" / "larmorx-nc" / "fsl-data" / "etc" / "flirtsch"
    copy.mkdir(parents=True)
    fsldir = tmp_path / "fsl"
    (fsldir / "etc" / "flirtsch").mkdir(parents=True)
    for d, names in ((copy, pepolar.FSL_CONFIGS), (fsldir / "etc" / "flirtsch", ["b02b0_2.cnf"])):
        for n in names:
            (d / n).write_text("--miter=1\n")
    monkeypatch.setattr(pepolar._replica, "find", lambda family, tool: str(program))
    monkeypatch.setattr(pepolar.sys, "prefix", str(tmp_path / "elsewhere"))
    got = pepolar.topup_config_file("b02b0_2.cnf", fsldir)
    assert got == str((fsldir / "etc" / "flirtsch" / "b02b0_2.cnf").resolve())
    got = pepolar.topup_config_file("b02b0_4.cnf", fsldir)
    assert got == str((copy / "b02b0_4.cnf").resolve())
    assert pepolar.topup_config_file("b02b0_1.cnf") == str((copy / "b02b0_1.cnf").resolve())
    own = pathlib.Path(pepolar.topup_config_file("b02b0.cnf")).parts[-4:]
    assert own == ("sdcflows", "data", "flirtsch", "b02b0.cnf")
    # neither: the name itself, which larmorx's topup finds among its own settings
    monkeypatch.setattr(pepolar._replica, "find", lambda family, tool: None)
    assert pepolar.topup_config_file("b02b0_1.cnf", fsldir) == "b02b0_1.cnf"
    fsls = fsldir / "etc" / "flirtsch" / "b02b0_2.cnf"
    assert pepolar.topup_config_file("b02b0_2.cnf", fsldir) == str(fsls.resolve())
    assert pepolar.topup_config_file("b02b0_4.cnf") == "b02b0_4.cnf"
    with pytest.raises(ValueError, match="not a topup configuration of sdcflows"):
        pepolar.topup_config_file("b02b0_7T.cnf")

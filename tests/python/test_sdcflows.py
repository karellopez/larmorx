# SPDX-License-Identifier: Apache-2.0
"""lx.sdcflows without sdcflows: the control grid, the weights' structure, the warning."""

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
        lx.sdcflows.grid_bspline_weights(lx.Image(img.data, rot @ img.affine), grid)
    coarse = lx.sdcflows.bspline_grid(img, lx.sdcflows.DEFAULT_ZOOMS_MM)
    both = lx.sdcflows.collocation_matrix(img, [coarse, grid])
    assert both.shape[1] == coarse.data.size + grid.data.size
    with pytest.raises(ValueError, match="float32 and float64"):
        lx.sdcflows.grid_bspline_weights(img, grid, dtype="float16")

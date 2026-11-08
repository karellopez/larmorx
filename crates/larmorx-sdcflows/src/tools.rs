// SPDX-License-Identifier: Apache-2.0 AND MIT
//! sdcflows' grid helpers (`sdcflows/utils/tools.py`, sdcflows 2.17.1; Apache-2.0) with
//! nibabel's affine arithmetic (nibabel 5.4.2 `nibabel/affines.py`: `rescale_affine`,
//! `voxel_sizes`, `apply_affine`; MIT, notice below) and nitransforms' identity resampling
//! (nitransforms 25.1.0 `nitransforms/resampling.py`, `base.py`; MIT).
//!
//! - [`rescale_affine`]: nibabel's `rescale_affine(affine, shape, zooms, new_shape)`, the
//!   affine of a grid with new voxel sizes and the same centre voxel position.
//! - [`resample_to_zooms_grid`]: the grid `resample_to_zooms` resamples onto
//!   (`np.rint(shape * pre_zooms / zooms)` voxels, [`rescale_affine`]).
//! - [`identity_coordinates`]: the voxel coordinates in a moving image of every voxel of a
//!   reference grid, as `nitransforms.linear.Affine(reference=ref).apply(moving)` computes them
//!   for the identity transform (the coordinates `scipy.ndimage.map_coordinates` is given).
//!
//! The products go through numpy's BLAS calls in OpenBLAS's Haswell operation order
//! (`larmorx_transform::openblas`): nibabel's `apply_affine` of one point and `matrix @ vector`
//! are `dgemv` ([`openblas::rows3_single`]); nitransforms' `affine.dot(points)` is `dgemm`
//! (`larmorx_transform::nitransforms`).
//!
//! nibabel: Copyright (c) 2009-2019 Matthew Brett, Michael Hanke, Eric Larson, Chris Markiewicz
//! and the NiBabel developers (MIT License). nitransforms: Copyright (c) 2021 The NiPy
//! developers (MIT License). Permission is hereby granted, free of charge, to any person
//! obtaining a copy of this software and associated documentation files (the "Software"), to
//! deal in the Software without restriction, including without limitation the rights to use,
//! copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software,
//! and to permit persons to whom the Software is furnished to do so, subject to the following
//! conditions: The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT
//! WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN
//! AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
//! SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use larmorx_core::linalg::Mat4;
use larmorx_transform::{nitransforms, openblas};

use crate::{SdcError, npy};

/// nibabel's `voxel_sizes(affine)`: `sqrt(sum(affine[:3, :3] ** 2, axis=0))`, the column sums
/// taken row by row.
pub fn voxel_sizes(a: &Mat4) -> [f64; 3] {
    std::array::from_fn(|c| ((a[0][c] * a[0][c] + a[1][c] * a[1][c]) + a[2][c] * a[2][c]).sqrt())
}

/// nibabel's `rescale_affine(affine, shape, zooms, new_shape)`: the columns of `affine[:3, :3]`
/// scaled to `zooms` (`affine[:3, :3] * zooms / voxel_sizes`), and the translation that keeps
/// the voxel `(shape - 1) // 2` where the new grid's voxel `(new_shape - 1) // 2` is.
/// `new_shape` is float64, as `resample_to_zooms` passes it (the result of `np.rint`).
pub fn rescale_affine(
    affine: &Mat4,
    shape: [usize; 3],
    zooms: [f64; 3],
    new_shape: [f64; 3],
) -> Mat4 {
    let s = voxel_sizes(affine);
    let mut out: Mat4 = [[0.0; 4]; 4];
    for (r, row) in out.iter_mut().take(3).enumerate() {
        for (c, v) in row.iter_mut().take(3).enumerate() {
            *v = affine[r][c] * zooms[c] / s[c];
        }
    }
    out[3][3] = 1.0;
    // apply_affine(affine, (shape - 1) // 2): one point (int64, cast to float64), `pts @ rzs.T
    // + trans`, a dgemv.
    let half = shape.map(|n| ((n as i64 - 1).div_euclid(2)) as f64);
    let q = openblas::rows3_single(affine, half);
    let centroid: [f64; 3] = std::array::from_fn(|r| q[r] + affine[r][3]);
    // rzs_out @ ((new_shape - 1) // 2): matrix @ vector, a dgemv.
    let new_half = new_shape.map(|n| npy::floor_divide(n - 1.0, 2.0));
    let v = openblas::rows3_single(&out, new_half);
    for r in 0..3 {
        out[r][3] = centroid[r] - v[r];
    }
    out
}

/// The grid of sdcflows' `resample_to_zooms(img, zooms)`: `np.rint(shape * pre_zooms / zooms)`
/// voxels (float64, as sdcflows passes it on) and [`rescale_affine`]'s affine. `pre_zooms`
/// are the header's first three zooms; `zooms` the target zooms (float32 or float64 values).
pub fn resample_to_zooms_grid(
    shape: [usize; 3],
    affine: &Mat4,
    pre_zooms: [f64; 3],
    zooms: [f64; 3],
) -> ([f64; 3], Mat4) {
    let new_shape: [f64; 3] =
        std::array::from_fn(|i| ((shape[i] as f64 * pre_zooms[i]) / zooms[i]).round_ties_even());
    let aff = rescale_affine(affine, shape, zooms, new_shape);
    (new_shape, aff)
}

/// The coordinates `Affine(matrix, reference=ref).apply(moving)` (nitransforms 25.1.0) hands
/// to `map_coordinates`: for every voxel of the reference grid (`ref_shape`, `ref_affine`;
/// Fortran order), its RAS+ position (`ImageGrid.ndcoords`: indices through float32, `dgemm`),
/// mapped by the transform (`Affine.map`: rounded to float32, times `matrix`, `dgemm`) and into
/// the moving image's voxels (`ImageGrid.index`: rounded to float32, times
/// `np.linalg.inv(moving_affine)`, `dgemm`). [`SdcError::Singular`] when the moving affine is
/// singular (numpy's "Singular matrix").
pub fn affine_coordinates(
    ref_shape: [usize; 3],
    ref_affine: &Mat4,
    matrix: &Mat4,
    moving_affine: &Mat4,
    n_threads: usize,
) -> Result<Vec<[f64; 3]>, SdcError> {
    let inverse = openblas::inv4(moving_affine).ok_or(SdcError::Singular)?;
    let points = larmorx_core::parallel::with_threads(n_threads, || {
        let mut pts = nitransforms::ndcoords(ref_shape, ref_affine);
        nitransforms::affine_map(matrix, &mut pts);
        nitransforms::affine_map(&inverse, &mut pts);
        pts
    })?;
    Ok(points)
}

/// [`affine_coordinates`] for the identity transform, `Affine(reference=ref).apply(moving)`
/// (sdcflows' `resample_to_zooms`).
pub fn identity_coordinates(
    ref_shape: [usize; 3],
    ref_affine: &Mat4,
    moving_affine: &Mat4,
    n_threads: usize,
) -> Result<Vec<[f64; 3]>, SdcError> {
    affine_coordinates(ref_shape, ref_affine, &IDENTITY, moving_affine, n_threads)
}

/// The 4×4 identity.
pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// nibabel's `apply_affine(aff, pts)` for `(n, 3)` points: `pts @ aff[:3, :3].T + aff[:3, 3]`,
/// the product as numpy's BLAS computes it (`dgemm` for two or more points, `dgemv` for one).
pub fn apply_affine(aff: &Mat4, points: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let single = points.len() == 1;
    points
        .iter()
        .map(|&p| {
            let q = if single {
                openblas::rows3_single(aff, p)
            } else {
                openblas::rows3(aff, p)
            };
            std::array::from_fn(|r| q[r] + aff[r][3])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescale_keeps_the_centre_voxel() {
        let a: Mat4 = [
            [-2.0, 0.0, 0.0, 60.0],
            [0.0, 2.0, 0.0, -70.0],
            [0.0, 0.0, 2.5, -30.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (shape, b) = resample_to_zooms_grid([65, 64, 31], &a, [2.0, 2.0, 2.5], [4.0, 4.0, 4.0]);
        assert_eq!(shape, [32.0, 32.0, 19.0]);
        assert_eq!(b[0][0], -4.0);
        assert_eq!(b[2][2], 4.0);
        // voxel 32 of the old x axis (60 - 64 = -4) is voxel 15 of the new one
        assert_eq!(b[0][3] + b[0][0] * 15.0, -4.0);
    }

    #[test]
    fn identity_coordinates_of_the_same_grid() {
        // power-of-two voxel sizes: the inverse is exact
        let a: Mat4 = [
            [2.0, 0.0, 0.0, -10.0],
            [0.0, 2.0, 0.0, -12.0],
            [0.0, 0.0, 4.0, 5.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let c = identity_coordinates([3, 2, 2], &a, &a, 1).unwrap();
        assert_eq!(c.len(), 12);
        assert_eq!(c[1], [1.0, 0.0, 0.0]);
        assert_eq!(c[11], [2.0, 1.0, 1.0]);
    }
}

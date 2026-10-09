//! Sampling grids in ITK's physical space (LPS mm).
//!
//! A grid is what ITK calls an image's geometry: size, spacing, origin and direction cosines.
//! Tools ported from ITK/ANTs work on grids in LPS coordinates; [`Grid3::from_ras_affine`] and
//! [`Grid3::ras_affine`] convert to and from the RAS+ affines used everywhere else.

use crate::affine::Affine;
use crate::linalg::{self, Mat3};

/// A 3D sampling grid: voxel `index` ↦ `origin + direction · diag(spacing) · index` (LPS mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid3 {
    pub size: [usize; 3],
    pub spacing: [f64; 3],
    pub origin: [f64; 3],
    /// Direction cosines: column `j` is the LPS direction of index axis `j`.
    pub direction: Mat3,
    index_to_point: Mat3,
    point_to_index: Mat3,
}

impl Grid3 {
    /// A grid from ITK's geometry fields. Fails if `direction · diag(spacing)` is singular.
    pub fn new(
        size: [usize; 3],
        spacing: [f64; 3],
        origin: [f64; 3],
        direction: Mat3,
    ) -> Option<Self> {
        let index_to_point: Mat3 =
            std::array::from_fn(|i| std::array::from_fn(|j| direction[i][j] * spacing[j]));
        let point_to_index = linalg::inverse3_itk(&index_to_point)?;
        Some(Grid3 {
            size,
            spacing,
            origin,
            direction,
            index_to_point,
            point_to_index,
        })
    }

    /// The grid of an image with `shape` (first three dimensions) and RAS+ `affine`: spacing
    /// from the column lengths, direction from the normalised columns, both converted to LPS.
    pub fn from_ras_affine(shape: [usize; 3], affine: &Affine) -> Option<Self> {
        let lps = affine.ras_to_lps();
        let linear = lps.linear();
        let spacing = lps.voxel_sizes();
        let direction: Mat3 =
            std::array::from_fn(|i| std::array::from_fn(|j| linear[i][j] / spacing[j]));
        Self::new(shape, spacing, lps.translation(), direction)
    }

    /// The voxel-to-world affine of this grid in RAS+.
    pub fn ras_affine(&self) -> Affine {
        Affine::from_linear(self.index_to_point, self.origin).lps_to_ras()
    }

    pub fn len(&self) -> usize {
        self.size.iter().product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The physical point (LPS) of a (continuous) index, as ITK's
    /// `TransformContinuousIndexToPhysicalPoint`.
    pub fn index_to_point(&self, index: [f64; 3]) -> [f64; 3] {
        let m = &self.index_to_point;
        std::array::from_fn(|i| {
            self.origin[i] + (m[i][0] * index[0] + m[i][1] * index[1] + m[i][2] * index[2])
        })
    }

    /// The physical point (LPS) of voxel `index`, as ITK's `TransformIndexToPhysicalPoint` for an
    /// integer index (which sums from the origin, so it rounds differently from
    /// [`Grid3::index_to_point`]).
    pub fn voxel_to_point(&self, index: [usize; 3]) -> [f64; 3] {
        let m = &self.index_to_point;
        let idx = index.map(|i| i as f64);
        std::array::from_fn(|i| {
            let mut p = self.origin[i];
            for (j, &x) in idx.iter().enumerate() {
                p += m[i][j] * x;
            }
            p
        })
    }

    /// The continuous index of a physical point (LPS), as ITK's
    /// `TransformPhysicalPointToContinuousIndex`.
    pub fn point_to_index(&self, point: [f64; 3]) -> [f64; 3] {
        let v = [
            point[0] - self.origin[0],
            point[1] - self.origin[1],
            point[2] - self.origin[2],
        ];
        let m = &self.point_to_index;
        std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
    }

    /// Whether a continuous index lies inside the buffer, i.e. within half a voxel of the grid
    /// on every axis (`-0.5 <= index < size - 0.5`), as ITK's `ImageFunction::IsInsideBuffer`.
    /// NaN is outside.
    pub fn is_inside(&self, index: [f64; 3]) -> bool {
        (0..3).all(|j| index[j] >= -0.5 && index[j] < self.size[j] as f64 - 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_point_round_trip_and_ras_conversion() {
        let (c, s) = (0.3f64.cos(), 0.3f64.sin());
        let ras = Affine::from_linear(
            [
                [2.0 * c, 0.0, -3.0 * s],
                [0.0, 2.5, 0.0],
                [2.0 * s, 0.0, 3.0 * c],
            ],
            [-90.0, 120.0, -60.0],
        );
        let g = Grid3::from_ras_affine([10, 20, 30], &ras).unwrap();
        assert!(g.ras_affine().allclose(&ras, 0.0, 1e-12));
        let idx = [1.5, -0.25, 7.0];
        let back = g.point_to_index(g.index_to_point(idx));
        assert!(idx.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-12));
        // LPS: RAS x and y change sign.
        let p_ras = ras.transform_point(idx);
        let p_lps = g.index_to_point(idx);
        assert!((p_lps[0] + p_ras[0]).abs() < 1e-12 && (p_lps[1] + p_ras[1]).abs() < 1e-12);
        assert!((p_lps[2] - p_ras[2]).abs() < 1e-12);
    }

    #[test]
    fn inside_test_follows_itk() {
        let g = Grid3::new([4, 4, 4], [1.0; 3], [0.0; 3], linalg::IDENTITY3).unwrap();
        assert!(g.is_inside([-0.5, 0.0, 3.4999]));
        assert!(!g.is_inside([-0.5001, 0.0, 0.0]));
        assert!(!g.is_inside([0.0, 3.5, 0.0]));
        assert!(!g.is_inside([f64::NAN, 0.0, 0.0]));
        assert!(Grid3::new([4, 4, 4], [0.0, 1.0, 1.0], [0.0; 3], linalg::IDENTITY3).is_none());
    }
}

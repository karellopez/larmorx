// SPDX-License-Identifier: Apache-2.0
//! An ITK image grid: size, **start index**, spacing, origin and direction in `D` dimensions,
//! with ITK's index ↔ physical point conversions.
//!
//! Images read from files start at index 0, but filters such as `ConstantPadImageFilter` and
//! `ShrinkImageFilter` produce regions that start elsewhere (padding by 5 voxels gives a
//! region starting at −5 with the same origin), and later filters compute with those indices.
//! The conversions are `itk::ImageBase`'s (ITK v5.4.5, `itkImageBase.h`):
//! - `TransformIndexToPhysicalPoint` starts from the origin and adds `M[i][j]·index[j]` term
//!   by term (`M = direction · diag(spacing)`);
//! - `TransformContinuousIndexToPhysicalPoint` sums `M[i][j]·index[j]` from 0, then adds the
//!   origin;
//! - `TransformPhysicalPointToIndex` multiplies `point − origin` by vnl's SVD inverse of `M`
//!   ([`larmorx_core::vnl_svd::vnl_inverse`]) and rounds half up
//!   (`Math::RoundHalfIntegerUp`).

use larmorx_core::vnl_svd::vnl_inverse;

use crate::FilterError;

/// A `D`-dimensional ITK image grid.
#[derive(Clone, Debug, PartialEq)]
pub struct ItkGrid {
    /// Voxels along each axis.
    pub size: Vec<usize>,
    /// The index of the first voxel (ITK's largest possible region index).
    pub start: Vec<i64>,
    pub spacing: Vec<f64>,
    pub origin: Vec<f64>,
    /// Row `i`, column `j`: the direction cosines (column `j` is axis `j`).
    pub direction: Vec<Vec<f64>>,
}

impl ItkGrid {
    /// A grid starting at index 0.
    pub fn new(size: &[usize], spacing: &[f64], origin: &[f64], direction: &[Vec<f64>]) -> Self {
        let d = size.len();
        assert!(
            spacing.len() == d && origin.len() == d && direction.len() == d,
            "grid dimensions differ"
        );
        ItkGrid {
            size: size.to_vec(),
            start: vec![0; d],
            spacing: spacing.to_vec(),
            origin: origin.to_vec(),
            direction: direction.to_vec(),
        }
    }

    /// The number of dimensions.
    pub fn dim(&self) -> usize {
        self.size.len()
    }

    /// The number of voxels.
    pub fn len(&self) -> usize {
        self.size.iter().product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `M = direction · diag(spacing)`, row-major (`ComputeIndexToPhysicalPointMatrices`).
    pub fn index_to_point_matrix(&self) -> Vec<f64> {
        let d = self.dim();
        let mut m = vec![0.0; d * d];
        for i in 0..d {
            for j in 0..d {
                m[i * d + j] = self.direction[i][j] * self.spacing[j];
            }
        }
        m
    }

    /// `TransformIndexToPhysicalPoint` for an integer index, in double precision.
    pub fn index_to_point(&self, index: &[i64]) -> Vec<f64> {
        let d = self.dim();
        let m = self.index_to_point_matrix();
        (0..d)
            .map(|i| {
                let mut p = self.origin[i];
                for j in 0..d {
                    p += m[i * d + j] * index[j] as f64;
                }
                p
            })
            .collect()
    }

    /// `TransformContinuousIndexToPhysicalPoint` (double precision).
    pub fn continuous_index_to_point(&self, index: &[f64]) -> Vec<f64> {
        let d = self.dim();
        let m = self.index_to_point_matrix();
        (0..d)
            .map(|i| {
                let mut sum = 0.0;
                for j in 0..d {
                    sum += m[i * d + j] * index[j];
                }
                sum + self.origin[i]
            })
            .collect()
    }

    /// `TransformPhysicalPointToIndex` (double precision point): `M⁻¹ · (point − origin)`,
    /// each row summed from 0, rounded half up. Fails if `M` is singular.
    pub fn point_to_index(&self, point: &[f64]) -> Result<Vec<i64>, FilterError> {
        let d = self.dim();
        let inv = vnl_inverse(&self.index_to_point_matrix(), d)
            .ok_or_else(|| FilterError::invalid("the image direction or spacing is singular"))?;
        Ok((0..d)
            .map(|i| {
                let mut sum = 0.0;
                for j in 0..d {
                    sum += inv[i * d + j] * (point[j] - self.origin[j]);
                }
                round_half_integer_up(sum)
            })
            .collect())
    }
}

/// `itk::Math::RoundHalfIntegerUp<int64>(double)` on x86-64 (SSE2):
/// `round_half_to_even(2x + 0.5) >> 1`.
pub fn round_half_integer_up(x: f64) -> i64 {
    cvtsd2si64(2.0 * x + 0.5) >> 1
}

/// `itk::Math::Floor<int64>(double)` on x86-64 (SSE2): `round_half_to_even(2x − 0.5) >> 1`.
/// It is the true floor except for negative values in `[−2⁻⁵⁵, 0)`, whose floor is 0 (as
/// [`vnl_floor_f32`] for floats).
pub fn itk_floor_f64(x: f64) -> i64 {
    cvtsd2si64(2.0 * x - 0.5) >> 1
}

/// x86-64's `cvtsd2si` with a 64-bit destination: round to nearest (ties to even), with NaN
/// and values out of range giving `i64::MIN`.
fn cvtsd2si64(x: f64) -> i64 {
    let r = x.round_ties_even();
    let limit = -(i64::MIN as f64); // 2^63
    if r.is_nan() || !(-limit..limit).contains(&r) {
        i64::MIN
    } else {
        r as i64
    }
}

/// `vnl_math::floor(float)` as ITK v5.4.5 is built on x86-64 (`VNL_CONFIG_ENABLE_SSE2_ROUNDING`):
/// `_mm_cvtss_si32(2x − 0.5f) >> 1`, with the subtraction in float (ITK's
/// `Math::Floor<long>(float)` is the same with `_mm_cvtss_si64`). It is the true floor for
/// `|x| < 2^22` **except for negative values in `[−2⁻²⁶, 0)`**, where `2x − 0.5f` rounds to
/// `−0.5`, which rounds to 0: their floor is 0, not −1. It gives `i32::MIN >> 1` for NaN and
/// values out of range.
pub fn vnl_floor_f32(x: f32) -> i32 {
    let y = 2.0 * x - 0.5;
    let r = y.round_ties_even();
    let i = if r.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&r) {
        i32::MIN
    } else {
        r as i32
    };
    i >> 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnl_floor_of_tiny_negative_floats_is_zero() {
        assert_eq!(vnl_floor_f32(-1e-8), 0);
        assert_eq!(vnl_floor_f32(-(2.0f32).powi(-26)), 0);
        assert_eq!(vnl_floor_f32(-(2.0f32).powi(-25)), -1);
        assert_eq!(vnl_floor_f32(-0.25), -1);
        assert_eq!(vnl_floor_f32(4.999_999_5), 4);
        assert_eq!(vnl_floor_f32(5.0), 5);
        assert_eq!(itk_floor_f64(-1e-17), 0);
        assert_eq!(itk_floor_f64(-1e-15), -1);
        assert_eq!(itk_floor_f64(2.999_999_999_999), 2);
        assert_eq!(itk_floor_f64(-3.5), -4);
    }

    #[test]
    fn rounding_matches_itk() {
        assert_eq!(round_half_integer_up(1.5), 2);
        assert_eq!(round_half_integer_up(-1.5), -1);
        assert_eq!(round_half_integer_up(2.5), 3);
        assert_eq!(round_half_integer_up(-0.2), 0);
        assert_eq!(round_half_integer_up(-0.7), -1);
        for x in [
            0.0f32, 0.25, 0.5, 0.99, 1.0, 198.999, 199.0, -0.3, -1.0, -1.5,
        ] {
            assert_eq!(vnl_floor_f32(x), x.floor() as i32, "{x}");
        }
        assert_eq!(vnl_floor_f32(f32::NAN), i32::MIN >> 1);
    }

    #[test]
    fn points_and_indices() {
        let g = ItkGrid {
            size: vec![4, 5],
            start: vec![-2, 0],
            spacing: vec![2.0, 0.5],
            origin: vec![10.0, -3.0],
            direction: vec![vec![0.0, -1.0], vec![1.0, 0.0]],
        };
        let p = g.index_to_point(&[1, 2]);
        assert_eq!(p, [10.0 - 1.0, -3.0 + 2.0]);
        assert_eq!(g.point_to_index(&p).unwrap(), [1, 2]);
        assert_eq!(g.continuous_index_to_point(&[1.0, 2.0]), p);
    }
}

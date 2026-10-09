//! 4×4 affine transforms between voxel indices and world coordinates.

use std::fmt;
use std::ops::Mul;

use crate::linalg::{self, Mat3, Mat4};

/// A 4×4 affine matrix in f64, stored row-major.
///
/// For an image, the affine maps voxel indices `(i, j, k)` to world coordinates in **RAS+
/// millimetres**: x increases to the subject's Right, y to Anterior, z to Superior. This is
/// the convention of NIfTI and nibabel. ITK and ANTs use LPS+; convert with
/// [`Affine::ras_to_lps`] at their API boundary.
#[derive(Clone, Copy, PartialEq)]
pub struct Affine {
    m: Mat4,
}

/// The matrix is not invertible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the affine is singular")]
pub struct SingularAffine;

impl Affine {
    /// The identity transform.
    pub const IDENTITY: Affine = Affine {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// An affine from its four rows.
    pub const fn from_rows(rows: Mat4) -> Self {
        Affine { m: rows }
    }

    /// An affine from its 3×3 linear part (rotations, zooms, shears) and its translation.
    pub fn from_linear(linear: Mat3, translation: [f64; 3]) -> Self {
        let mut m = Self::IDENTITY.m;
        for i in 0..3 {
            m[i][..3].copy_from_slice(&linear[i]);
            m[i][3] = translation[i];
        }
        Affine { m }
    }

    /// A pure scaling by `zooms`, with the given translation.
    pub fn from_zooms(zooms: [f64; 3], translation: [f64; 3]) -> Self {
        let mut linear = [[0.0; 3]; 3];
        for (i, z) in zooms.into_iter().enumerate() {
            linear[i][i] = z;
        }
        Self::from_linear(linear, translation)
    }

    /// The four rows.
    pub const fn rows(&self) -> &Mat4 {
        &self.m
    }

    /// The 3×3 linear part (upper-left block).
    pub fn linear(&self) -> Mat3 {
        [0, 1, 2].map(|i| [self.m[i][0], self.m[i][1], self.m[i][2]])
    }

    /// The translation (last column).
    pub fn translation(&self) -> [f64; 3] {
        [self.m[0][3], self.m[1][3], self.m[2][3]]
    }

    /// Whether the last row is exactly `[0, 0, 0, 1]`.
    pub fn is_affine(&self) -> bool {
        self.m[3] == [0.0, 0.0, 0.0, 1.0]
    }

    /// Whether every entry is finite.
    pub fn is_finite(&self) -> bool {
        self.m.iter().flatten().all(|v| v.is_finite())
    }

    /// The composition `self · rhs`: apply `rhs` first, then `self`.
    pub fn compose(&self, rhs: &Affine) -> Affine {
        Affine {
            m: linalg::mul4(&self.m, &rhs.m),
        }
    }

    /// The inverse transform.
    pub fn inverse(&self) -> Result<Affine, SingularAffine> {
        linalg::inverse4(&self.m)
            .map(|m| Affine { m })
            .ok_or(SingularAffine)
    }

    /// Maps a point (e.g. voxel indices to world millimetres).
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.m;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
    }

    /// Lengths of the columns of the linear part: the voxel sizes of an image with this
    /// affine (as `nibabel.affines.voxel_sizes`).
    pub fn voxel_sizes(&self) -> [f64; 3] {
        let m = &self.m;
        [0, 1, 2].map(|j| (m[0][j] * m[0][j] + m[1][j] * m[1][j] + m[2][j] * m[2][j]).sqrt())
    }

    /// Determinant of the linear part. Negative means the voxel axes are left-handed with
    /// respect to RAS+ (as in "radiological" storage).
    pub fn determinant(&self) -> f64 {
        linalg::det3(&self.linear())
    }

    /// For each world axis, the angle in radians between it and the closest voxel axis
    /// (as `nibabel.affines.obliquity`). All zeros for an image aligned with the world axes.
    pub fn obliquity(&self) -> [f64; 3] {
        let zooms = self.voxel_sizes();
        [0, 1, 2].map(|i| {
            let best = (0..3)
                .map(|j| (self.m[i][j] / zooms[j]).abs())
                .fold(0.0f64, f64::max);
            libm::acos(best.min(1.0))
        })
    }

    /// The same transform expressed for LPS+ world coordinates (ITK/ANTs convention): the
    /// first two rows change sign. The conversion is its own inverse.
    pub fn ras_to_lps(&self) -> Affine {
        let mut m = self.m;
        for row in m.iter_mut().take(2) {
            for v in row.iter_mut() {
                *v = -*v;
            }
        }
        Affine { m }
    }

    /// Inverse of [`Affine::ras_to_lps`].
    pub fn lps_to_ras(&self) -> Affine {
        self.ras_to_lps()
    }

    /// Element-wise closeness, with numpy's `allclose` rule `|a - b| <= atol + rtol·|b|`.
    pub fn allclose(&self, other: &Affine, rtol: f64, atol: f64) -> bool {
        self.m
            .iter()
            .flatten()
            .zip(other.m.iter().flatten())
            .all(|(a, b)| (a - b).abs() <= atol + rtol * b.abs())
    }
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Affine {
    type Output = Affine;
    fn mul(self, rhs: Affine) -> Affine {
        self.compose(&rhs)
    }
}

impl From<Mat4> for Affine {
    fn from(rows: Mat4) -> Self {
        Affine::from_rows(rows)
    }
}

impl From<Affine> for Mat4 {
    fn from(a: Affine) -> Self {
        a.m
    }
}

impl fmt::Debug for Affine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Affine[")?;
        for (i, row) in self.m.iter().enumerate() {
            let sep = if i == 0 { "" } else { ", " };
            write!(f, "{sep}{row:?}")?;
        }
        f.write_str("]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oblique() -> Affine {
        let (c, s) = (0.2f64.cos(), 0.2f64.sin());
        Affine::from_linear(
            [
                [2.0 * c, 0.0, -3.0 * s],
                [0.0, 2.5, 0.0],
                [2.0 * s, 0.0, 3.0 * c],
            ],
            [-90.0, 126.0, -72.0],
        )
    }

    #[test]
    fn compose_and_inverse_round_trip() {
        let a = oblique();
        let back = a.compose(&a.inverse().unwrap());
        assert!(back.allclose(&Affine::IDENTITY, 0.0, 1e-14), "{back:?}");
        assert_eq!(a * Affine::IDENTITY, a);
        let p = [3.0, 4.5, -2.0];
        let q = a.inverse().unwrap().transform_point(a.transform_point(p));
        assert!(p.iter().zip(q).all(|(x, y)| (x - y).abs() < 1e-12));
        assert_eq!(
            Affine::from_zooms([0.0, 1.0, 1.0], [0.0; 3]).inverse(),
            Err(SingularAffine)
        );
    }

    #[test]
    fn voxel_sizes_obliquity_and_handedness() {
        let a = oblique();
        let z = a.voxel_sizes();
        assert!(
            (z[0] - 2.0).abs() < 1e-15 && (z[1] - 2.5).abs() < 1e-15 && (z[2] - 3.0).abs() < 1e-15
        );
        let ob = a.obliquity();
        assert!(
            (ob[0] - 0.2).abs() < 1e-12 && ob[1] == 0.0 && (ob[2] - 0.2).abs() < 1e-12,
            "{ob:?}"
        );
        assert_eq!(Affine::IDENTITY.obliquity(), [0.0; 3]);
        assert!(a.determinant() > 0.0);
        assert!(Affine::from_zooms([-1.0, 1.0, 1.0], [0.0; 3]).determinant() < 0.0);
    }

    #[test]
    fn lps_conversion_flips_x_and_y() {
        let a = oblique();
        let lps = a.ras_to_lps();
        let (p_ras, p_lps) = (
            a.transform_point([1.0, 2.0, 3.0]),
            lps.transform_point([1.0, 2.0, 3.0]),
        );
        assert_eq!(p_lps, [-p_ras[0], -p_ras[1], p_ras[2]]);
        assert_eq!(lps.lps_to_ras(), a);
    }

    #[test]
    fn allclose_follows_numpy() {
        let a = Affine::from_zooms([1.0, 1.0, 1.0], [100.0, 0.0, 0.0]);
        let b = Affine::from_zooms([1.0, 1.0, 1.0], [100.0005, 0.0, 0.0]);
        assert!(a.allclose(&b, 1e-5, 1e-8));
        assert!(!a.allclose(&b, 1e-6, 1e-8));
    }
}

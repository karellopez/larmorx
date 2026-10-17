// SPDX-License-Identifier: Apache-2.0
//! An image's geometry and ITK's conversions between indices and physical points
//! (`itk::ImageBase`, ITK v5.4.5 `Modules/Core/Common/include/itkImageBase.h/.hxx`).
//!
//! The geometry is held in double (`SpacePrecisionType`), whatever the pixel type. The
//! matrices are ITK's: `IndexToPhysicalPoint = Direction · diag(Spacing)` and
//! `PhysicalPointToIndex` its inverse by vnl's SVD (`itk::Matrix::GetInverse`). Each
//! conversion follows ITK's loop and types: a continuous index is computed in double and
//! cast to the interpolator's coordinate type `R`.

use larmorx_core::vnl_svd::vnl_inverse;

use larmorx_core::ItkReal as _;

use crate::{ItkException, Matrix, Real};

/// An image's region and geometry: `size` voxels from index `start`, and the physical
/// placement.
#[derive(Clone, Debug, PartialEq)]
pub struct Space<const D: usize> {
    pub size: [usize; D],
    /// The region's first index (0 for an image read from a file; a shrunk image may start
    /// elsewhere).
    pub start: [i64; D],
    pub spacing: [f64; D],
    pub origin: [f64; D],
    /// Direction cosines, row by row: column `j` is the direction of index axis `j`.
    pub direction: Matrix<f64, D>,
    /// `m_IndexToPhysicalPoint`.
    pub index_to_point: Matrix<f64, D>,
    /// `m_PhysicalPointToIndex`.
    pub point_to_index: Matrix<f64, D>,
    /// `m_InverseDirection`.
    pub inverse_direction: Matrix<f64, D>,
}

/// ITK's `Matrix::GetInverse` (vnl's SVD pseudo-inverse), or its exception for a singular
/// matrix.
pub fn itk_inverse<const D: usize>(m: &Matrix<f64, D>) -> Result<Matrix<f64, D>, ItkException> {
    let singular = || {
        ItkException::new(
            "Modules/Core/Common/include/itkMatrix.h",
            256,
            "",
            "Singular matrix. Determinant is 0.",
        )
    };
    if determinant(m) == 0.0 {
        return Err(singular());
    }
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    let inv = vnl_inverse(&flat, D).ok_or_else(singular)?;
    Ok(std::array::from_fn(|i| {
        std::array::from_fn(|j| inv[i * D + j])
    }))
}

/// `vnl_determinant` of a small matrix (the explicit formulas of `vnl_det` for 1 to 4
/// rows); only its comparison with 0 is used.
pub fn determinant<const D: usize>(m: &Matrix<f64, D>) -> f64 {
    let mut a: Vec<f64> = m.iter().flatten().copied().collect();
    // Gaussian elimination with partial pivoting: exact zero for exactly singular matrices
    // with a zero row or column, as the explicit formulas give.
    let n = D;
    let mut det = 1.0;
    for c in 0..n {
        let mut p = c;
        for r in c + 1..n {
            if a[r * n + c].abs() > a[p * n + c].abs() {
                p = r;
            }
        }
        if a[p * n + c] == 0.0 {
            return 0.0;
        }
        if p != c {
            for k in 0..n {
                a.swap(c * n + k, p * n + k);
            }
            det = -det;
        }
        det *= a[c * n + c];
        for r in c + 1..n {
            let f = a[r * n + c] / a[c * n + c];
            for k in c..n {
                a[r * n + k] -= f * a[c * n + k];
            }
        }
    }
    det
}

/// `vnl_matrix_fixed` product `a · b`: each entry `a[i][0]·b[0][j]`, then `+= a[i][k]·b[k][j]`.
pub fn mat_mat<const D: usize>(a: &Matrix<f64, D>, b: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut s = a[i][0] * b[0][j];
            for k in 1..D {
                s += a[i][k] * b[k][j];
            }
            s
        })
    })
}

/// `itk::Matrix · itk::Vector`: each row summed from 0.
pub fn mat_vec<const D: usize>(m: &Matrix<f64, D>, v: &[f64; D]) -> [f64; D] {
    std::array::from_fn(|r| {
        let mut s = 0.0;
        for c in 0..D {
            s += m[r][c] * v[c];
        }
        s
    })
}

impl<const D: usize> Space<D> {
    /// A geometry; fails where ITK's `ComputeIndexToPhysicalPointMatrices` throws (a
    /// singular direction or a zero spacing).
    pub fn new(
        size: [usize; D],
        start: [i64; D],
        spacing: [f64; D],
        origin: [f64; D],
        direction: Matrix<f64, D>,
    ) -> Result<Self, ItkException> {
        let scale: Matrix<f64, D> =
            std::array::from_fn(|i| std::array::from_fn(|j| if i == j { spacing[i] } else { 0.0 }));
        let index_to_point = mat_mat(&direction, &scale);
        let point_to_index = itk_inverse(&index_to_point)?;
        let inverse_direction = itk_inverse(&direction)?;
        Ok(Space {
            size,
            start,
            spacing,
            origin,
            direction,
            index_to_point,
            point_to_index,
            inverse_direction,
        })
    }

    /// The number of voxels.
    pub fn len(&self) -> usize {
        self.size.iter().product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Strides of the voxel buffer (x fastest).
    pub fn strides(&self) -> [usize; D] {
        let mut s = [1usize; D];
        for k in 1..D {
            s[k] = s[k - 1] * self.size[k - 1];
        }
        s
    }

    /// The buffer offset of index `idx` (inside the region).
    #[inline]
    pub fn offset(&self, idx: &[i64; D]) -> usize {
        let mut o = 0usize;
        let mut stride = 1usize;
        for k in 0..D {
            o += (idx[k] - self.start[k]) as usize * stride;
            stride *= self.size[k];
        }
        o
    }

    /// The index of buffer offset `o`.
    #[inline]
    pub fn index_of(&self, mut o: usize) -> [i64; D] {
        let mut idx = [0i64; D];
        for k in 0..D {
            idx[k] = (o % self.size[k]) as i64 + self.start[k];
            o /= self.size[k];
        }
        idx
    }

    /// `TransformIndexToPhysicalPoint<double>`: `point[i] = origin[i]`, then
    /// `+= M[i][j]·index[j]`.
    #[inline]
    pub fn index_to_point(&self, idx: &[i64; D]) -> [f64; D] {
        std::array::from_fn(|i| {
            let mut p = self.origin[i];
            for j in 0..D {
                p += self.index_to_point[i][j] * idx[j] as f64;
            }
            p
        })
    }

    /// `TransformContinuousIndexToPhysicalPoint<double, double>`: `sum` from 0 of
    /// `M[r][c]·index[c]`, plus the origin.
    #[inline]
    pub fn cindex_to_point(&self, c: &[f64; D]) -> [f64; D] {
        std::array::from_fn(|r| {
            let mut s = 0.0;
            for k in 0..D {
                s += self.index_to_point[r][k] * c[k];
            }
            s + self.origin[r]
        })
    }

    /// `TransformPhysicalPointToContinuousIndex<R>` of a point (given as double; a float
    /// point is exact in double): the difference to the origin and the matrix product in
    /// double, each component cast to `R`.
    #[inline]
    pub fn point_to_cindex<R: Real>(&self, p: &[f64; D]) -> [R; D] {
        let v: [f64; D] = std::array::from_fn(|k| p[k] - self.origin[k]);
        let c = mat_vec(&self.point_to_index, &v);
        std::array::from_fn(|i| R::from_f64(c[i]))
    }

    /// `TransformPhysicalPointToIndex<double>`: `sum += M[i][j]·(p[j] − origin[j])` from 0,
    /// rounded half up (`Math::RoundHalfIntegerUp`).
    #[inline]
    pub fn point_to_index(&self, p: &[f64; D]) -> [i64; D] {
        std::array::from_fn(|i| {
            let mut s = 0.0f64;
            for j in 0..D {
                s += self.point_to_index[i][j] * (p[j] - self.origin[j]);
            }
            s.itk_round_half_up()
        })
    }

    /// Whether `idx` lies in the region (`ImageRegion::IsInside`).
    #[inline]
    pub fn contains(&self, idx: &[i64; D]) -> bool {
        (0..D).all(|k| idx[k] >= self.start[k] && idx[k] < self.start[k] + self.size[k] as i64)
    }

    /// `ImageFunction::IsInsideBuffer(ContinuousIndex)` for an interpolator of coordinate
    /// type `R`: `start − 0.5 ≤ c < end + 0.5` on every axis (false for NaN).
    #[inline]
    pub fn is_inside_buffer<R: Real>(&self, c: &[R; D]) -> bool {
        (0..D).all(|j| {
            let lo = R::from_f64(self.start[j] as f64 - 0.5);
            let hi = R::from_f64((self.start[j] + self.size[j] as i64 - 1) as f64 + 0.5);
            c[j] >= lo && c[j] < hi
        })
    }

    /// `TransformLocalVectorToPhysicalVector`: `Direction · v`, each row summed from 0.
    #[inline]
    pub fn local_to_physical_vector(&self, v: &[f64; D]) -> [f64; D] {
        mat_vec(&self.direction, v)
    }
}

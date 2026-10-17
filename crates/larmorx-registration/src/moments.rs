// SPDX-License-Identifier: Apache-2.0
//! `ImageMomentsCalculator` (ITK v5.4.5,
//! `Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx`): the total mass,
//! the first and second moments in index coordinates, the centre of gravity and the central
//! second moments in physical coordinates, and the principal moments and axes, as ANTs'
//! `antsAI` and `antsAffineInitializer` use them.
//!
//! Every sum runs over the voxels in index order (x fastest), each value converted to double,
//! in the expressions' order (`value · p[i] · p[j]` is `(value · p[i]) · p[j]`). The principal
//! axes are the eigenvectors of the central moments (vnl's symmetric eigensystem, ascending
//! eigenvalues), one per row; the last row is then multiplied by the real part of the product
//! of the eigenvalues of that matrix of axes (vnl's real eigensystem), which turns a
//! reflection into a rotation.

use larmorx_core::vnl_eigen::{real_eigenvalue_product, symmetric_eigensystem};

use crate::{Image, ItkException, Matrix, Real};

const FILE: &str = "Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx";
const CLASS: &str = "ImageMomentsCalculator";

/// The moments `ImageMomentsCalculator::Compute` leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Moments<const D: usize> {
    /// `GetTotalMass()`: the sum of the values.
    pub total_mass: f64,
    /// `GetFirstMoments()`: the mean index.
    pub first: [f64; D],
    /// `GetSecondMoments()`: the second moments of the index about the mean.
    pub second: Matrix<f64, D>,
    /// `GetCenterOfGravity()`: the mean physical point.
    pub center_of_gravity: [f64; D],
    /// `GetCentralMoments()`: the second moments of the physical point about the centre.
    pub central: Matrix<f64, D>,
    /// `GetPrincipalMoments()`: the eigenvalues of the central moments (ascending) times the
    /// total mass.
    pub principal_moments: [f64; D],
    /// `GetPrincipalAxes()`: the eigenvectors, one per row (the last row's sign makes the
    /// matrix a rotation).
    pub principal_axes: Matrix<f64, D>,
}

/// Why the moments cannot be computed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MomentsError {
    /// ITK's exception: a total mass of 0.
    #[error("{0}")]
    ZeroMass(ItkException),
    /// The moments are not finite (NaN or infinite values): vnl's eigensystems are then
    /// undefined (EISPACK reads past its arrays, `vnl_symmetric_eigensystem` leaves its
    /// eigenvectors unset). larmorx refuses.
    #[error(
        "the image moments are not finite (NaN or infinite values); ITK's principal axes are undefined for them"
    )]
    NotFinite,
}

/// `Compute()` over the whole image. Fails as ITK does for a total mass of 0, and where the
/// moments are not finite ([`MomentsError::NotFinite`]).
pub fn compute<R: Real, const D: usize>(image: &Image<R, D>) -> Result<Moments<D>, MomentsError> {
    let space = &image.space;
    let mut m0 = 0.0f64;
    let mut m1 = [0.0f64; D];
    let mut m2 = [[0.0f64; D]; D];
    let mut cg = [0.0f64; D];
    let mut cm = [[0.0f64; D]; D];
    for (o, &v) in image.data.iter().enumerate() {
        let value = v.to_f64();
        let idx = space.index_of(o);
        let p = space.index_to_point(&idx);
        m0 += value;
        for i in 0..D {
            m1[i] += idx[i] as f64 * value;
            for j in 0..D {
                let weight = value * idx[i] as f64 * idx[j] as f64;
                m2[i][j] += weight;
            }
        }
        for i in 0..D {
            cg[i] += p[i] * value;
            for j in 0..D {
                let weight = value * p[i] * p[j];
                cm[i][j] += weight;
            }
        }
    }
    if m0 == 0.0 {
        return Err(MomentsError::ZeroMass(ItkException::new(
            FILE,
            172,
            CLASS,
            "Compute(): Total Mass of the image was zero. Aborting here to prevent division by zero later on.",
        )));
    }
    for i in 0..D {
        cg[i] /= m0;
        m1[i] /= m0;
        for j in 0..D {
            m2[i][j] /= m0;
            cm[i][j] /= m0;
        }
    }
    for i in 0..D {
        for j in 0..D {
            m2[i][j] -= m1[i] * m1[j];
            cm[i][j] -= cg[i] * cg[j];
        }
    }
    let not_finite = || MomentsError::NotFinite;
    if !cm.iter().flatten().all(|v| v.is_finite()) {
        return Err(not_finite());
    }
    let flat: Vec<f64> = cm.iter().flatten().copied().collect();
    let eigen = symmetric_eigensystem(&flat, D);
    if eigen.ierr != 0 {
        return Err(not_finite());
    }
    let pm: [f64; D] = std::array::from_fn(|i| eigen.values[i] * m0);
    // m_Pa = eigen.V.transpose()
    let mut pa: Matrix<f64, D> =
        std::array::from_fn(|i| std::array::from_fn(|j| eigen.vectors[j * D + i]));
    let flat: Vec<f64> = pa.iter().flatten().copied().collect();
    let (det, _) = real_eigenvalue_product(&flat, D).ok_or_else(not_finite)?;
    for i in 0..D {
        pa[D - 1][i] *= det;
    }
    Ok(Moments {
        total_mass: m0,
        first: m1,
        second: m2,
        center_of_gravity: cg,
        central: cm,
        principal_moments: pm,
        principal_axes: pa,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Space;

    #[test]
    fn moments_of_a_bar() {
        // A bar along y: its principal axis of largest moment is y.
        let space = Space::new(
            [5, 9],
            [0, 0],
            [1.0, 2.0],
            [-2.0, 3.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap();
        let data: Vec<f64> = (0..45)
            .map(|o| if (1..4).contains(&(o % 5)) { 2.0 } else { 0.0 })
            .collect();
        let m = compute(&Image::new(space, data)).unwrap();
        assert_eq!(m.total_mass, 54.0);
        assert_eq!(m.center_of_gravity, [0.0, 11.0]);
        assert!(m.principal_moments[1] > m.principal_moments[0]);
        assert!((m.principal_axes[1][1].abs() - 1.0).abs() < 1e-15);
        let zero = Image::new(
            Space::new(
                [2, 2],
                [0, 0],
                [1.0, 1.0],
                [0.0, 0.0],
                [[1.0, 0.0], [0.0, 1.0]],
            )
            .unwrap(),
            vec![0.0f64; 4],
        );
        assert!(compute(&zero).is_err());
    }
}

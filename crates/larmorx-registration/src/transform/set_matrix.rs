// SPDX-License-Identifier: Apache-2.0
//! `SetMatrix` of the parameterised transforms (ITK v5.4.5, `Modules/Core/Transform/include`):
//! the matrix checked as each class checks it, then its parameters computed from it
//! (`ComputeMatrixParameters`), as `antsAI` sets the matrix of its rigid and similarity search
//! transforms.
//!
//! - `Euler3DTransform` (`itkRigid3DTransform.hxx:53-85`, `itkEuler3DTransform.hxx:176-215`):
//!   `M·Mᵀ` within 1e-10 of the identity, else "Attempting to set a non-orthogonal rotation
//!   matrix"; the offset from the given matrix, the angles from `asin` and `atan2`, then the
//!   matrix recomputed from the angles.
//! - `Euler2DTransform` / `Rigid2DTransform` (`itkRigid2DTransform.hxx:62-111`): the same test,
//!   else "Attempt to set a Non-Orthogonal matrix"; the angle `acos` of the orthogonal part of
//!   the matrix (`U·Vᵀ` of vnl's SVD); the matrix kept.
//! - `Similarity2DTransform` (`itkSimilarity2DTransform.hxx:146-170`, 281-308): `M·Mᵀ` divided by
//!   its first entry; the scale and angle from the first row and column.
//! - `Similarity3DTransform` (`itkSimilarity3DTransform.hxx:57-118`, 286-297): the cube root of
//!   the determinant as the scale, `M/s` orthogonal, then the versor of `M/s`
//!   (`Versor::Set(matrix)`, `itkVersor.hxx:320-400`).
//!
//! Every expression in double, in ITK's order; the transcendental functions correctly rounded
//! (`larmorx_core::math`).

use larmorx_core::math;
use larmorx_core::vnl_fixed;

use super::linear::{Kind, LinearXfm};
use crate::{ItkException, Matrix};

const TOLERANCE: f64 = 1e-10;

/// `vnl_matrix_fixed` product (each entry starts from its first product).
fn mul_fixed<const D: usize>(a: &Matrix<f64, D>, b: &Matrix<f64, D>) -> Matrix<f64, D> {
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

/// `vnl_matrix` product (each entry summed from 0).
fn mul_dyn<const D: usize>(a: &Matrix<f64, D>, b: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut s = 0.0;
            for k in 0..D {
                s += a[i][k] * b[k][j];
            }
            s
        })
    })
}

fn transpose<const D: usize>(m: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]))
}

/// `vnl_matrix_fixed::is_identity(tol)`.
fn is_identity<const D: usize>(m: &Matrix<f64, D>, tol: f64) -> bool {
    for i in 0..D {
        for j in 0..D {
            let xm = m[i][j];
            let absdev = if i == j { (xm - 1.0).abs() } else { xm.abs() };
            if absdev > tol {
                return false;
            }
        }
    }
    true
}

/// `Rigid3DTransform::MatrixIsOrthogonal(matrix, tolerance)`.
fn matrix_is_orthogonal<const D: usize>(m: &Matrix<f64, D>, tol: f64) -> bool {
    is_identity(&mul_fixed(m, &transpose(m)), tol)
}

const RIGID3D: &str = "Modules/Core/Transform/include/itkRigid3DTransform.hxx";
const RIGID2D: &str = "Modules/Core/Transform/include/itkRigid2DTransform.hxx";
const SIM2D: &str = "Modules/Core/Transform/include/itkSimilarity2DTransform.hxx";
const SIM3D: &str = "Modules/Core/Transform/include/itkSimilarity3DTransform.hxx";

impl<const D: usize> LinearXfm<f64, D> {
    /// `SetMatrix(matrix)` of this transform's class; `warnings` receives ITK's warning text
    /// (`Rigid2DTransform`'s "Bad Rotation Matrix"). For `AffineTransform` and
    /// `MatrixOffsetTransformBase` it is [`LinearXfm::set_matrix`].
    pub fn set_matrix_checked(
        &mut self,
        m: Matrix<f64, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(), ItkException> {
        match self.kind {
            Kind::Affine | Kind::MatrixOffset => {
                self.set_matrix(m);
                Ok(())
            }
            Kind::Euler3D { zyx } => {
                if !matrix_is_orthogonal(&m, TOLERANCE) {
                    return Err(ItkException::new(
                        RIGID3D,
                        81,
                        "Euler3DTransform",
                        "Attempting to set a non-orthogonal rotation matrix",
                    ));
                }
                self.matrix = m;
                self.compute_offset();
                self.euler3d_angles(zyx);
                Ok(())
            }
            Kind::Euler2D | Kind::Rigid2D => {
                if !is_identity(&mul_fixed(&m, &transpose(&m)), TOLERANCE) {
                    return Err(ItkException::located(
                        "void itk::Rigid2DTransform<TParametersValueType>::SetMatrix(const MatrixType&, TParametersValueType) [with TParametersValueType = double; MatrixType = itk::Matrix<double, 2, 2>]",
                        "Attempt to set a Non-Orthogonal matrix",
                    ));
                }
                self.matrix = m;
                self.compute_offset();
                self.rigid2d_angle(warnings);
                Ok(())
            }
            Kind::Similarity2D => {
                let mut test = mul_fixed(&m, &transpose(&m));
                let first = test[0][0];
                for row in test.iter_mut() {
                    for v in row.iter_mut() {
                        *v /= first;
                    }
                }
                if !is_identity(&test, TOLERANCE) {
                    return Err(ItkException::located(
                        "void itk::Similarity2DTransform<TParametersValueType>::SetMatrix(const MatrixType&, TParametersValueType) [with TParametersValueType = double; MatrixType = itk::Matrix<double, 2, 2>]",
                        "Attempt to set a Non-Orthogonal matrix",
                    ));
                }
                self.matrix = m;
                self.compute_offset();
                self.similarity2d_parameters()
            }
            Kind::Similarity3D => {
                let m3: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j]));
                let det = vnl_fixed::det3(&m3);
                if det == 0.0 {
                    return Err(ItkException::new(
                        SIM3D,
                        89,
                        "Similarity3DTransform",
                        "Attempting to set a matrix with a zero determinant",
                    ));
                }
                let s = math::cbrt(det);
                if s <= 0.0 {
                    return Err(ItkException::new(
                        SIM3D,
                        105,
                        "Similarity3DTransform",
                        "Attempting to set a matrix with a negative trace",
                    ));
                }
                let test: Matrix<f64, D> =
                    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] / s));
                if !matrix_is_orthogonal(&test, TOLERANCE) {
                    return Err(ItkException::new(
                        SIM3D,
                        113,
                        "Similarity3DTransform",
                        "Attempting to set a non-orthogonal matrix (after removing scaling)",
                    ));
                }
                self.matrix = m;
                self.compute_offset();
                // ComputeMatrixParameters: the scale, then the versor of M / scale.
                self.scale = math::cbrt(vnl_fixed::det3(&m3));
                let unscaled: [[f64; 3]; 3] =
                    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] / self.scale));
                self.versor = versor_from_matrix(&unscaled)?;
                Ok(())
            }
            _ => {
                self.set_matrix(m);
                Ok(())
            }
        }
    }

    /// `Euler3DTransform::ComputeMatrixParameters`: the angles from the matrix, then
    /// `ComputeMatrix`.
    fn euler3d_angles(&mut self, zyx: bool) {
        let m = &self.matrix;
        let (ax, ay, az);
        if zyx {
            ay = -math::asin(m[2][0]);
            let c = math::cos(ay);
            if c.abs() > 0.00005 {
                let x = m[2][2] / c;
                let y = m[2][1] / c;
                ax = math::atan2(y, x);
                let x = m[0][0] / c;
                let y = m[1][0] / c;
                az = math::atan2(y, x);
            } else {
                ax = 0.0;
                let x = m[1][1];
                let y = -m[0][1];
                az = math::atan2(y, x);
            }
        } else {
            ax = math::asin(m[2][1]);
            let a = math::cos(ax);
            if a.abs() > 0.00005 {
                let x = m[2][2] / a;
                let y = -m[2][0] / a;
                ay = math::atan2(y, x);
                let x = m[1][1] / a;
                let y = -m[0][1] / a;
                az = math::atan2(y, x);
            } else {
                az = 0.0;
                let x = m[0][0];
                let y = m[1][0];
                ay = math::atan2(y, x);
            }
        }
        self.angles = [ax, ay, az];
        self.compute_matrix();
    }

    /// `Rigid2DTransform::ComputeMatrixParameters`: the angle of the orthogonal part `U·Vᵀ`
    /// of the matrix (vnl's SVD; `vnl_matrix` product), the sign from its second row.
    fn rigid2d_angle(&mut self, warnings: &mut Vec<String>) {
        let flat: Vec<f64> = self.matrix.iter().flatten().copied().collect();
        let Some((u, _w, v)) = larmorx_core::vnl_svd::vnl_svd(&flat, D, D) else {
            return;
        };
        // U and V column-major: U(i, j) = u[i + D·j].
        let um: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| u[i + D * j]));
        let vt: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| v[j + D * i]));
        let r = mul_dyn(&um, &vt);
        let mut angle = math::acos(r[0][0]);
        if r[1][0] < 0.0 {
            angle = -angle;
        }
        self.angles[0] = angle;
        if r[1][0] - math::sin(angle) > 0.000001 {
            warnings.push(format!(
                "WARNING: In {RIGID2D}, line 109\nEuler2DTransform (0x0): Bad Rotation Matrix {}\n\n",
                crate::format::itk_matrix(&self.matrix)
            ));
        }
    }

    /// `Similarity2DTransform::ComputeMatrixParameters`.
    fn similarity2d_parameters(&mut self) -> Result<(), ItkException> {
        let m = &self.matrix;
        let scale = (m[0][0] * m[0][0] + m[0][1] * m[0][1]).sqrt();
        if scale < f64::MIN_POSITIVE {
            return Err(ItkException::new(
                SIM2D,
                155,
                "Similarity2DTransform",
                format!(
                    "Bad Rotation Matrix. Scale cannot be zero.\nm_Scale : {}",
                    crate::format::OStream::default().float(scale)
                ),
            ));
        }
        self.scale = scale;
        let mut angle = math::acos(m[0][0] / scale);
        if m[1][0] < 0.0 {
            angle = -angle;
        }
        self.angles[0] = angle;
        if (m[1][0] / scale) - math::sin(angle) > 0.000001 {
            return Err(ItkException::new(
                SIM2D,
                168,
                "Similarity2DTransform",
                "Bad Rotation Matrix",
            ));
        }
        Ok(())
    }

    /// `SetScale(scale)` of `Similarity2DTransform` and `Similarity3DTransform`: the scale,
    /// then `ComputeMatrix` and `ComputeOffset`.
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
        self.compute_matrix();
        self.compute_offset();
    }

    /// `Rigid3DTransform::Translate(offset)` (and `Rigid2DTransform`'s): the offset moved by
    /// `offset` (`SetOffset`, which computes the translation), then `ComputeTranslation`.
    pub fn translate_offset(&mut self, offset: &[f64; D]) {
        let o: [f64; D] = std::array::from_fn(|i| self.offset[i] + offset[i]);
        self.set_offset(o);
        self.compute_translation();
    }
}

/// `Versor<double>::Set(const MatrixType & mat)`: the rotation test with `Epsilon()` = 1e-10
/// (`m·mᵀ` as a `vnl_matrix` product), the quaternion by the largest of the trace and the
/// diagonal entries, then `Normalize()`.
fn versor_from_matrix(m: &[[f64; 3]; 3]) -> Result<[f64; 4], ItkException> {
    let epsilon = 1e-10;
    let i = mul_dyn(m, &transpose(m));
    let det_i = vnl_fixed::det3(&i);
    if i[0][1].abs() > epsilon
        || i[0][2].abs() > epsilon
        || i[1][0].abs() > epsilon
        || i[1][2].abs() > epsilon
        || i[2][0].abs() > epsilon
        || i[2][1].abs() > epsilon
        || (i[0][0] - 1.0).abs() > epsilon
        || (i[1][1] - 1.0).abs() > epsilon
        || (i[2][2] - 1.0).abs() > epsilon
        || det_i < 0.0
    {
        let os = crate::format::OStream::default();
        return Err(ItkException::new(
            "Modules/Core/Common/include/itkVersor.hxx",
            350,
            "",
            format!(
                "The following matrix does not represent rotation to within an epsion of {}.\n{}\ndet(m * m transpose) is: {}\nm * m transpose is:\n{}\n",
                os.float(epsilon),
                crate::format::itk_matrix(m),
                os.float(det_i),
                crate::format::itk_matrix(&i)
            ),
        ));
    }
    let trace = m[0][0] + m[1][1] + m[2][2] + 1.0;
    let (mut x, mut y, mut z, mut w);
    if trace > epsilon {
        let s = 0.5 / trace.sqrt();
        w = 0.25 / s;
        x = (m[2][1] - m[1][2]) * s;
        y = (m[0][2] - m[2][0]) * s;
        z = (m[1][0] - m[0][1]) * s;
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = 2.0 * (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt();
        x = 0.25 * s;
        y = (m[0][1] + m[1][0]) / s;
        z = (m[0][2] + m[2][0]) / s;
        w = (m[1][2] - m[2][1]) / s;
    } else if m[1][1] > m[2][2] {
        let s = 2.0 * (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt();
        x = (m[0][1] + m[1][0]) / s;
        y = 0.25 * s;
        z = (m[1][2] + m[2][1]) / s;
        w = (m[0][2] - m[2][0]) / s;
    } else {
        let s = 2.0 * (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt();
        x = (m[0][2] + m[2][0]) / s;
        y = (m[1][2] + m[2][1]) / s;
        z = 0.25 * s;
        w = (m[0][1] - m[1][0]) / s;
    }
    // Normalize.
    let tensor = (x * x + y * y + z * z + w * w).sqrt();
    if tensor.abs() < 1e-20 {
        return Err(ItkException::located(
            "Modules/Core/Common/include/itkVersor.hxx",
            "Attempt to normalize a itk::Versor with zero tensor",
        ));
    }
    x /= tensor;
    y /= tensor;
    z /= tensor;
    w /= tensor;
    Ok([x, y, z, w])
}

// SPDX-License-Identifier: Apache-2.0
//! ITK's linear transforms (the `MatrixOffsetTransformBase` family).
//!
//! Every type maps a point as `y = M·x + offset`, with `offset = t + c − M·c` for the
//! translation `t` and centre of rotation `c`. Only the way `M` is built from the parameters
//! differs. Ported from ITK v5.4.5 (`Modules/Core/Transform`, `Modules/Core/Common/itkVersor`).

use larmorx_core::linalg::{self, Mat3};
use larmorx_core::math;

use crate::TransformError;

/// The kinds of linear transform larmorx reads, by their ITK class names (without the
/// `_double_3_3`/`_float_3_3` suffix).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearKind {
    /// `AffineTransform`, `MatrixOffsetTransformBase`, `Rigid3DTransform`: 9 matrix entries
    /// (row-major) and 3 translations.
    Affine,
    /// `Euler3DTransform`: angles about x, y, z (radians) and 3 translations.
    Euler3D,
    /// `VersorRigid3DTransform`: versor (vector part of a unit quaternion) and 3 translations.
    VersorRigid3D,
    /// `Similarity3DTransform`: versor, 3 translations and an isotropic scale.
    Similarity3D,
    /// `TranslationTransform`: 3 translations.
    Translation,
    /// `IdentityTransform`: no parameters.
    Identity,
}

impl LinearKind {
    /// The kind of an ITK class name such as `AffineTransform_float_3_3`.
    pub fn from_itk_name(name: &str) -> Option<LinearKind> {
        let base = name.split('_').next().unwrap_or(name);
        Some(match base {
            "AffineTransform" | "MatrixOffsetTransformBase" | "Rigid3DTransform" => {
                LinearKind::Affine
            }
            "Euler3DTransform" => LinearKind::Euler3D,
            "VersorRigid3DTransform" => LinearKind::VersorRigid3D,
            "Similarity3DTransform" => LinearKind::Similarity3D,
            "TranslationTransform" => LinearKind::Translation,
            "IdentityTransform" => LinearKind::Identity,
            _ => return None,
        })
    }

    fn n_parameters(self) -> usize {
        match self {
            LinearKind::Affine => 12,
            LinearKind::Euler3D | LinearKind::VersorRigid3D => 6,
            LinearKind::Similarity3D => 7,
            LinearKind::Translation => 3,
            LinearKind::Identity => 0,
        }
    }
}

/// A linear (affine) transform in ITK's representation, mapping LPS points.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearTransform {
    /// The ITK class name it was read as (kept for writing it back).
    pub itk_name: String,
    pub kind: LinearKind,
    pub parameters: Vec<f64>,
    pub fixed_parameters: Vec<f64>,
    pub matrix: Mat3,
    pub offset: [f64; 3],
}

/// The rotation matrix of a versor (ITK's `Versor::GetMatrix`).
fn versor_matrix(x: f64, y: f64, z: f64, w: f64) -> Mat3 {
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, xw) = (x * y, x * z, x * w);
    let (yz, yw, zw) = (y * z, y * w, z * w);
    [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - zw), 2.0 * (xz + yw)],
        [2.0 * (xy + zw), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - xw)],
        [2.0 * (xz - yw), 2.0 * (yz + xw), 1.0 - 2.0 * (xx + yy)],
    ]
}

/// The versor of ITK's versor-based transforms: the vector part, scaled down slightly if its
/// norm reaches 1, and `w = √(1 − |v|²)`.
fn versor(p: &[f64]) -> Result<(f64, f64, f64, f64), TransformError> {
    let mut v = [p[0], p[1], p[2]];
    let norm = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    const EPSILON: f64 = 1e-10;
    if norm >= 1.0 - EPSILON {
        v = v.map(|c| c / (norm + EPSILON * norm));
    }
    let sin2 = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if sin2 > 1.0 {
        return Err(TransformError::Invalid(
            "versor with magnitude greater than 1".into(),
        ));
    }
    Ok((v[0], v[1], v[2], (1.0 - sin2 * sin2).sqrt()))
}

impl LinearTransform {
    /// A transform from an ITK class name, parameters and fixed parameters (the centre).
    pub fn from_itk(
        itk_name: &str,
        parameters: &[f64],
        fixed_parameters: &[f64],
    ) -> Result<Self, TransformError> {
        let kind = LinearKind::from_itk_name(itk_name)
            .ok_or_else(|| TransformError::Unsupported(itk_name.to_owned()))?;
        if parameters.len() != kind.n_parameters() {
            return Err(TransformError::Invalid(format!(
                "{itk_name} needs {} parameters, got {}",
                kind.n_parameters(),
                parameters.len()
            )));
        }
        let center = match kind {
            LinearKind::Translation | LinearKind::Identity => [0.0; 3],
            _ if fixed_parameters.len() >= 3 => [
                fixed_parameters[0],
                fixed_parameters[1],
                fixed_parameters[2],
            ],
            _ if fixed_parameters.is_empty() => [0.0; 3],
            _ => {
                return Err(TransformError::Invalid(format!(
                    "{itk_name} needs 3 fixed parameters (the centre)"
                )));
            }
        };
        let p = parameters;
        let (matrix, translation): (Mat3, [f64; 3]) = match kind {
            LinearKind::Affine => (
                [[p[0], p[1], p[2]], [p[3], p[4], p[5]], [p[6], p[7], p[8]]],
                [p[9], p[10], p[11]],
            ),
            LinearKind::Euler3D => {
                let (cx, sx) = (math::cos(p[0]), math::sin(p[0]));
                let (cy, sy) = (math::cos(p[1]), math::sin(p[1]));
                let (cz, sz) = (math::cos(p[2]), math::sin(p[2]));
                let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
                let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
                let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
                let zyx = fixed_parameters.get(3).is_some_and(|&f| f != 0.0);
                let m = if zyx {
                    linalg::mul3(&linalg::mul3(&rz, &ry), &rx)
                } else {
                    linalg::mul3(&linalg::mul3(&rz, &rx), &ry)
                };
                (m, [p[3], p[4], p[5]])
            }
            LinearKind::VersorRigid3D => {
                let (x, y, z, w) = versor(p)?;
                (versor_matrix(x, y, z, w), [p[3], p[4], p[5]])
            }
            LinearKind::Similarity3D => {
                let (x, y, z, w) = versor(p)?;
                let r = versor_matrix(x, y, z, w);
                (r.map(|row| row.map(|v| v * p[6])), [p[3], p[4], p[5]])
            }
            LinearKind::Translation => (linalg::IDENTITY3, [p[0], p[1], p[2]]),
            LinearKind::Identity => (linalg::IDENTITY3, [0.0; 3]),
        };
        // ITK's ComputeOffset subtracts the terms of M·c one at a time.
        let offset = std::array::from_fn(|i| {
            let mut o = translation[i] + center[i];
            for (m, c) in matrix[i].iter().zip(center) {
                o -= m * c;
            }
            o
        });
        Ok(LinearTransform {
            itk_name: itk_name.to_owned(),
            kind,
            parameters: parameters.to_vec(),
            fixed_parameters: fixed_parameters.to_vec(),
            matrix,
            offset,
        })
    }

    /// An affine transform `y = matrix·x + offset` (LPS), stored as ITK's
    /// `AffineTransform_double_3_3` with centre 0.
    pub fn from_matrix(matrix: Mat3, offset: [f64; 3]) -> Self {
        let mut parameters: Vec<f64> = matrix.iter().flatten().copied().collect();
        parameters.extend_from_slice(&offset);
        LinearTransform {
            itk_name: "AffineTransform_double_3_3".into(),
            kind: LinearKind::Affine,
            parameters,
            fixed_parameters: vec![0.0; 3],
            matrix,
            offset,
        }
    }

    /// Maps a point: `M·x + offset` (ITK's `MatrixOffsetTransformBase::TransformPoint`).
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.matrix;
        std::array::from_fn(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + self.offset[i])
    }

    /// The inverse transform (ITK's `GetInverse`): `M⁻¹`, offset `−M⁻¹·offset`.
    pub fn inverse(&self) -> Result<LinearTransform, TransformError> {
        let inv = linalg::inverse3_itk(&self.matrix)
            .ok_or_else(|| TransformError::Invalid("singular linear transform".into()))?;
        let offset = std::array::from_fn(|i| {
            -(inv[i][0] * self.offset[0] + inv[i][1] * self.offset[1] + inv[i][2] * self.offset[2])
        });
        Ok(LinearTransform::from_matrix(inv, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
    }

    #[test]
    fn affine_with_center() {
        // 90° about z with centre (1, 0, 0) and translation (0, 0, 5).
        let t = LinearTransform::from_itk(
            "AffineTransform_float_3_3",
            &[0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 5.0],
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        assert!(close(
            t.transform_point([1.0, 0.0, 0.0]),
            [1.0, 0.0, 5.0],
            1e-15
        ));
        assert!(close(
            t.transform_point([2.0, 0.0, 0.0]),
            [1.0, 1.0, 5.0],
            1e-15
        ));
        let inv = t.inverse().unwrap();
        assert!(close(
            inv.transform_point(t.transform_point([3.0, -2.0, 7.0])),
            [3.0, -2.0, 7.0],
            1e-12
        ));
    }

    #[test]
    fn rotation_parameterisations_agree() {
        // A rotation of 0.4 rad about z, as Euler angles and as a versor.
        let e = LinearTransform::from_itk(
            "Euler3DTransform_double_3_3",
            &[0.0, 0.0, 0.4, 1.0, 2.0, 3.0],
            &[0.0; 3],
        )
        .unwrap();
        let s = (0.2f64).sin();
        let v = LinearTransform::from_itk(
            "VersorRigid3DTransform_double_3_3",
            &[0.0, 0.0, s, 1.0, 2.0, 3.0],
            &[0.0; 3],
        )
        .unwrap();
        let p = [3.0, -1.0, 2.0];
        assert!(close(e.transform_point(p), v.transform_point(p), 1e-12));
        let sim = LinearTransform::from_itk(
            "Similarity3DTransform_double_3_3",
            &[0.0, 0.0, s, 1.0, 2.0, 3.0, 2.0],
            &[0.0; 3],
        )
        .unwrap();
        let q = sim.transform_point(p);
        assert!(close(
            q,
            std::array::from_fn(
                |i| 2.0 * (v.transform_point(p)[i] - [1.0, 2.0, 3.0][i]) + [1.0, 2.0, 3.0][i]
            ),
            1e-12
        ));
        // ZYX order differs from the default ZXY when rotating about two axes.
        let zxy = LinearTransform::from_itk(
            "Euler3DTransform_double_3_3",
            &[0.3, 0.2, 0.0, 0.0, 0.0, 0.0],
            &[0.0; 3],
        )
        .unwrap();
        let zyx = LinearTransform::from_itk(
            "Euler3DTransform_double_3_3",
            &[0.3, 0.2, 0.0, 0.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0],
        )
        .unwrap();
        assert!(!close(zxy.transform_point(p), zyx.transform_point(p), 1e-6));
    }

    #[test]
    fn errors() {
        assert!(matches!(
            LinearTransform::from_itk("BSplineTransform_double_3_3", &[], &[]),
            Err(TransformError::Unsupported(_))
        ));
        assert!(
            LinearTransform::from_itk("AffineTransform_double_3_3", &[1.0; 3], &[0.0; 3]).is_err()
        );
        assert!(
            LinearTransform::from_itk("TranslationTransform_double_3_3", &[1.0, 2.0, 3.0], &[])
                .is_ok()
        );
    }
}

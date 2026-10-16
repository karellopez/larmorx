// SPDX-License-Identifier: Apache-2.0
//! ITK's linear transforms (the `MatrixOffsetTransformBase` family).
//!
//! Every type maps a point as `y = M·x + offset`, with `offset = t + c − M·c` for the
//! translation `t` and centre of rotation `c`. Only the way `M` is built from the parameters
//! differs. Ported from ITK v5.4.5 (`Modules/Core/Transform`, `Modules/Core/Common/itkVersor`).

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::linalg::Mat3;
use larmorx_core::{ItkReal, Precision};

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

/// A transform of ITK's matrix-offset family instantiated with the computation type `R`
/// (`MatrixOffsetTransformBase<R, 3, 3>` and its subclasses): `y = M·x + offset`, every
/// operation in `R`. `R = f64` is ANTs' default; `R = f32` is `antsApplyTransforms --float 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatrixOffset<R> {
    pub matrix: [[R; 3]; 3],
    pub offset: [R; 3],
    /// `TranslationTransform`: it maps `x + offset`, with no matrix product, and inverts to
    /// `−offset`.
    pub translation_only: bool,
}

/// The matrix of a versor (`Versor<R>::GetMatrix`): each product of two components in `R`,
/// kept as double (`RealType`), the entries computed in double and stored as `R`.
fn versor_matrix<R: ItkReal>(x: R, y: R, z: R, w: R) -> [[R; 3]; 3] {
    let d = |a: R, b: R| (a * b).to_f64();
    let (xx, yy, zz) = (d(x, x), d(y, y), d(z, z));
    let (xy, xz, xw) = (d(x, y), d(x, z), d(x, w));
    let (yz, yw, zw) = (d(y, z), d(y, w), d(z, w));
    let r = R::from_f64;
    [
        [
            r(1.0 - 2.0 * (yy + zz)),
            r(2.0 * (xy - zw)),
            r(2.0 * (xz + yw)),
        ],
        [
            r(2.0 * (xy + zw)),
            r(1.0 - 2.0 * (xx + zz)),
            r(2.0 * (yz - xw)),
        ],
        [
            r(2.0 * (xz - yw)),
            r(2.0 * (yz + xw)),
            r(1.0 - 2.0 * (xx + yy)),
        ],
    ]
}

/// A versor's `(x, y, z, w)`.
type VersorParts<R> = (R, R, R, R);

/// The versor of ITK's versor-based transforms from the axis parameters `p` and their norm
/// (computed by the caller as its class does): the axis scaled down slightly when the norm
/// reaches 1 (`axis / (norm + ε·norm)`, a double division stored as `R`), then
/// `Versor<R>::Set(axis)`: `sin = |axis|` (summed and rooted in double, stored as `R`) and
/// `w = √(1 − sin²)` (in double, stored as `R`).
///
/// In float the scaled axis, rounded to float, can again be longer than 1, and ITK throws
/// ("Trying to initialize a Versor with a vector whose magnitude is greater than 1"; defect
/// ITK-10). With [`UpstreamDefects::Reproduce`] that is the error here too; with
/// [`UpstreamDefects::Fix`] the versor is computed as ITK computes it in double, then stored as
/// `R` (the second value says whether that happened).
fn versor<R: ItkReal>(
    p: [R; 3],
    norm: f64,
    mode: UpstreamDefects,
) -> Result<(VersorParts<R>, bool), TransformError> {
    const EPSILON: f64 = 1e-10;
    let mut v = p;
    if norm >= 1.0 - EPSILON {
        v = v.map(|c| R::from_f64(c.to_f64() / (norm + EPSILON * norm)));
    }
    let mut squared = 0.0f64;
    for c in v {
        squared += c.to_f64() * c.to_f64();
    }
    let sin = R::from_f64(squared.sqrt());
    if sin.to_f64() > 1.0 {
        if R::PRECISION == Precision::Double {
            return Err(TransformError::Invalid(
                "versor with magnitude greater than 1".into(),
            ));
        }
        if mode.reproduce() {
            return Err(TransformError::Invalid(
                "versor with magnitude greater than 1 in single precision, as ITK fails to load \
                 it (ITK-10; corrected, larmorx loads it)"
                    .into(),
            ));
        }
        let pd = p.map(|c| c.to_f64());
        let nd = (pd[0] * pd[0] + pd[1] * pd[1] + pd[2] * pd[2]).sqrt();
        let ((x, y, z, w), _) = versor::<f64>(pd, nd, UpstreamDefects::Reproduce)?;
        return Ok((
            (
                R::from_f64(x),
                R::from_f64(y),
                R::from_f64(z),
                R::from_f64(w),
            ),
            true,
        ));
    }
    let cos = R::from_f64((1.0 - sin.to_f64() * sin.to_f64()).sqrt());
    Ok(((v[0], v[1], v[2], cos), false))
}

/// `vnl_matrix_fixed` product `a · b` in `R`: each entry starts from the first product.
fn mul3<R: ItkReal>(a: &[[R; 3]; 3], b: &[[R; 3]; 3]) -> [[R; 3]; 3] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut acc = a[i][0] * b[0][j];
            for k in 1..3 {
                acc = acc + a[i][k] * b[k][j];
            }
            acc
        })
    })
}

impl<R: ItkReal> MatrixOffset<R> {
    /// The transform ITK builds from a class's parameters and fixed parameters, as
    /// `TransformFileReaderTemplate<R>` reads it: `parameters` are cast to `R`
    /// (`static_cast`), the fixed parameters stay double until the centre is stored as `R`
    /// (`SetFixedParameters`), and the matrix and offset are computed in `R` by the class's
    /// `SetParameters`, `ComputeMatrix` and `ComputeOffset`. The caller has checked the number
    /// of parameters.
    pub fn from_itk(
        kind: LinearKind,
        parameters: &[f64],
        fixed_parameters: &[f64],
    ) -> Result<Self, TransformError> {
        Self::from_itk_mode(
            kind,
            parameters,
            fixed_parameters,
            UpstreamDefects::Reproduce,
        )
        .map(|(m, _)| m)
    }

    /// [`MatrixOffset::from_itk`] with ITK-10 corrected ([`UpstreamDefects::Fix`]) or
    /// reproduced; the second value says whether the correction changed the transform.
    pub fn from_itk_mode(
        kind: LinearKind,
        parameters: &[f64],
        fixed_parameters: &[f64],
        mode: UpstreamDefects,
    ) -> Result<(Self, bool), TransformError> {
        let mut corrected = false;
        let p: Vec<R> = parameters.iter().map(|&v| R::from_f64(v)).collect();
        let (zero, one) = (R::ZERO, R::ONE);
        let identity = [[one, zero, zero], [zero, one, zero], [zero, zero, one]];
        let center: [R; 3] = match kind {
            LinearKind::Translation | LinearKind::Identity => [zero; 3],
            _ if fixed_parameters.len() >= 3 => {
                std::array::from_fn(|i| R::from_f64(fixed_parameters[i]))
            }
            _ => [zero; 3],
        };
        let (matrix, translation): ([[R; 3]; 3], [R; 3]) = match kind {
            LinearKind::Affine => (
                [[p[0], p[1], p[2]], [p[3], p[4], p[5]], [p[6], p[7], p[8]]],
                [p[9], p[10], p[11]],
            ),
            LinearKind::Euler3D => {
                // Euler3DTransform<R>::ComputeMatrix: std::cos and std::sin of R.
                let (cx, sx) = (p[0].cos(), p[0].sin());
                let (cy, sy) = (p[1].cos(), p[1].sin());
                let (cz, sz) = (p[2].cos(), p[2].sin());
                let rx = [[one, zero, zero], [zero, cx, -sx], [zero, sx, cx]];
                let ry = [[cy, zero, sy], [zero, one, zero], [-sy, zero, cy]];
                let rz = [[cz, -sz, zero], [sz, cz, zero], [zero, zero, one]];
                // The fourth fixed parameter (ComputeZYX) counts only when there are exactly
                // four (Euler3DTransform::SetFixedParameters).
                let zyx = fixed_parameters.len() == 4 && fixed_parameters[3] != 0.0;
                let m = if zyx {
                    mul3(&mul3(&rz, &ry), &rx)
                } else {
                    mul3(&mul3(&rz, &rx), &ry)
                };
                (m, [p[3], p[4], p[5]])
            }
            LinearKind::VersorRigid3D => {
                // VersorRigid3DTransform::SetParameters: the norm of the parameters, summed
                // and rooted in R.
                let norm = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().to_f64();
                let ((x, y, z, w), c) = versor([p[0], p[1], p[2]], norm, mode)?;
                corrected |= c;
                (versor_matrix(x, y, z, w), [p[3], p[4], p[5]])
            }
            LinearKind::Similarity3D => {
                // Similarity3DTransform::SetParameters: the squares summed in double.
                let mut norm = (p[0] * p[0]).to_f64();
                norm += (p[1] * p[1]).to_f64();
                norm += (p[2] * p[2]).to_f64();
                if norm > 0.0 {
                    norm = norm.sqrt();
                }
                let ((x, y, z, w), c) = versor([p[0], p[1], p[2]], norm, mode)?;
                corrected |= c;
                let r = versor_matrix(x, y, z, w);
                (r.map(|row| row.map(|v| v * p[6])), [p[3], p[4], p[5]])
            }
            LinearKind::Translation => (identity, [p[0], p[1], p[2]]),
            LinearKind::Identity => (identity, [zero; 3]),
        };
        // ComputeOffset subtracts the terms of M·c one at a time.
        let offset = std::array::from_fn(|i| {
            let mut o = translation[i] + center[i];
            for (&m, &c) in matrix[i].iter().zip(&center) {
                o = o - m * c;
            }
            o
        });
        Ok((
            MatrixOffset {
                matrix,
                offset,
                translation_only: kind == LinearKind::Translation,
            },
            corrected,
        ))
    }

    /// Maps a point (`TransformPoint`): each row of `M·x` summed from 0 in `R`, then
    /// `+ offset`; a translation adds its offset only.
    #[inline]
    pub fn transform_point(&self, p: [R; 3]) -> [R; 3] {
        if self.translation_only {
            return std::array::from_fn(|i| p[i] + self.offset[i]);
        }
        let m = &self.matrix;
        std::array::from_fn(|i| {
            let mut sum = R::ZERO;
            for (&mij, &pj) in m[i].iter().zip(&p) {
                sum = sum + mij * pj;
            }
            sum + self.offset[i]
        })
    }

    /// The inverse (`GetInverseTransform`): for the matrix-offset classes a
    /// `MatrixOffsetTransformBase` with `M⁻¹` (`itk::Matrix<R>::GetInverse`) and offset
    /// `−(M⁻¹·offset)`; for a translation `−offset`. `None` where ITK finds the matrix
    /// singular.
    pub fn inverse(&self) -> Option<Self> {
        if self.translation_only {
            return Some(MatrixOffset {
                matrix: self.matrix,
                offset: self.offset.map(|o| -o),
                translation_only: true,
            });
        }
        let inv = R::itk_inverse3(&self.matrix)?;
        let offset = std::array::from_fn(|i| {
            let mut sum = R::ZERO;
            for (&m, &o) in inv[i].iter().zip(&self.offset) {
                sum = sum + m * o;
            }
            -sum
        });
        Some(MatrixOffset {
            matrix: inv,
            offset,
            translation_only: false,
        })
    }
}

impl LinearTransform {
    /// A transform from an ITK class name, parameters and fixed parameters (the centre).
    pub fn from_itk(
        itk_name: &str,
        parameters: &[f64],
        fixed_parameters: &[f64],
    ) -> Result<Self, TransformError> {
        // ITK's names end with the input and output dimensions (`_3_3`). A transform of another
        // dimension inside a 3-D composite is cast blindly by ITK (ITK-17); larmorx refuses it.
        let mut dims = itk_name.rsplit('_').take(2);
        if let (Some(out), Some(input)) = (dims.next(), dims.next())
            && out.parse::<u32>().is_ok()
            && (out != "3" || input != "3")
        {
            return Err(TransformError::Invalid(format!(
                "{itk_name}: a transform of dimension {input}→{out}; 3-D transforms are read"
            )));
        }
        let kind = LinearKind::from_itk_name(itk_name)
            .ok_or_else(|| TransformError::Unsupported(itk_name.to_owned()))?;
        if parameters.len() != kind.n_parameters() {
            return Err(TransformError::Invalid(format!(
                "{itk_name} needs {} parameters, got {}",
                kind.n_parameters(),
                parameters.len()
            )));
        }
        if !matches!(kind, LinearKind::Translation | LinearKind::Identity)
            && !fixed_parameters.is_empty()
            && fixed_parameters.len() < 3
        {
            return Err(TransformError::Invalid(format!(
                "{itk_name} needs 3 fixed parameters (the centre)"
            )));
        }
        let m = MatrixOffset::<f64>::from_itk(kind, parameters, fixed_parameters)?;
        Ok(LinearTransform {
            itk_name: itk_name.to_owned(),
            kind,
            parameters: parameters.to_vec(),
            fixed_parameters: fixed_parameters.to_vec(),
            matrix: m.matrix,
            offset: m.offset,
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

    /// This transform in double precision: its own matrix and offset.
    fn as_f64(&self) -> MatrixOffset<f64> {
        MatrixOffset {
            matrix: self.matrix,
            offset: self.offset,
            translation_only: self.kind == LinearKind::Translation,
        }
    }

    /// This transform instantiated with the computation type `R`, from its stored parameters,
    /// as ITK reads a transform file into the reader's precision. For `R = f64` it is this
    /// transform's own matrix and offset.
    pub fn with_precision<R: ItkReal>(&self) -> Result<MatrixOffset<R>, TransformError> {
        MatrixOffset::<R>::from_itk(self.kind, &self.parameters, &self.fixed_parameters)
    }

    /// [`LinearTransform::with_precision`] with ITK-10 corrected or reproduced
    /// ([`MatrixOffset::from_itk_mode`]).
    pub fn with_precision_mode<R: ItkReal>(
        &self,
        mode: UpstreamDefects,
    ) -> Result<(MatrixOffset<R>, bool), TransformError> {
        MatrixOffset::<R>::from_itk_mode(self.kind, &self.parameters, &self.fixed_parameters, mode)
    }

    /// Maps a point: `M·x + offset` (ITK's `MatrixOffsetTransformBase::TransformPoint`), or
    /// `x + offset` for a translation.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        self.as_f64().transform_point(p)
    }

    /// The inverse transform (ITK's `GetInverseTransform`): `M⁻¹`, offset `−M⁻¹·offset`; a
    /// translation's inverse is the translation by `−offset`.
    pub fn inverse(&self) -> Result<LinearTransform, TransformError> {
        let inv = self
            .as_f64()
            .inverse()
            .ok_or_else(|| TransformError::Invalid("singular linear transform".into()))?;
        if self.kind == LinearKind::Translation {
            return Ok(LinearTransform {
                itk_name: self.itk_name.clone(),
                kind: LinearKind::Translation,
                parameters: inv.offset.to_vec(),
                fixed_parameters: self.fixed_parameters.clone(),
                matrix: inv.matrix,
                offset: inv.offset,
            });
        }
        Ok(LinearTransform::from_matrix(inv.matrix, inv.offset))
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
    /// The double-precision code before the precision became a type parameter (2026-10-11),
    /// kept to prove that `MatrixOffset::<f64>` computes the same bits.
    mod legacy {
        use larmorx_core::linalg::{self, Mat3};
        use larmorx_core::math;

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

        fn versor(p: &[f64]) -> (f64, f64, f64, f64) {
            let mut v = [p[0], p[1], p[2]];
            let norm = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            const EPSILON: f64 = 1e-10;
            if norm >= 1.0 - EPSILON {
                v = v.map(|c| c / (norm + EPSILON * norm));
            }
            let sin2 = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            (v[0], v[1], v[2], (1.0 - sin2 * sin2).sqrt())
        }

        /// `(matrix, offset, inverse matrix, inverse offset)` as the old `from_itk` and
        /// `inverse` computed them.
        pub fn matrix_offset(
            kind: &str,
            p: &[f64],
            c: [f64; 3],
            zyx: bool,
        ) -> (Mat3, [f64; 3], Mat3, [f64; 3]) {
            let (matrix, translation): (Mat3, [f64; 3]) = match kind {
                "affine" => (
                    [[p[0], p[1], p[2]], [p[3], p[4], p[5]], [p[6], p[7], p[8]]],
                    [p[9], p[10], p[11]],
                ),
                "euler" => {
                    let (cx, sx) = (math::cos(p[0]), math::sin(p[0]));
                    let (cy, sy) = (math::cos(p[1]), math::sin(p[1]));
                    let (cz, sz) = (math::cos(p[2]), math::sin(p[2]));
                    let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
                    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
                    let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
                    let m = if zyx {
                        linalg::mul3(&linalg::mul3(&rz, &ry), &rx)
                    } else {
                        linalg::mul3(&linalg::mul3(&rz, &rx), &ry)
                    };
                    (m, [p[3], p[4], p[5]])
                }
                "versor" => {
                    let (x, y, z, w) = versor(p);
                    (versor_matrix(x, y, z, w), [p[3], p[4], p[5]])
                }
                _ => {
                    let (x, y, z, w) = versor(p);
                    let r = versor_matrix(x, y, z, w);
                    (r.map(|row| row.map(|v| v * p[6])), [p[3], p[4], p[5]])
                }
            };
            let offset: [f64; 3] = std::array::from_fn(|i| {
                let mut o = translation[i] + c[i];
                for (m, c) in matrix[i].iter().zip(c) {
                    o -= m * c;
                }
                o
            });
            let inv = linalg::inverse3_itk(&matrix).unwrap();
            let inv_offset: [f64; 3] = std::array::from_fn(|i| {
                -(inv[i][0] * offset[0] + inv[i][1] * offset[1] + inv[i][2] * offset[2])
            });
            (matrix, offset, inv, inv_offset)
        }
    }

    /// `MatrixOffset::<f64>` gives the bits of the double-precision code it replaced, for every
    /// class, centred, with and without ZYX, and their inverses.
    #[test]
    fn the_generic_double_path_equals_the_old_code() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        for _ in 0..2000 {
            let c = [next() * 50.0, next() * 50.0, next() * 50.0];
            let fixed = [c[0], c[1], c[2]];
            for (kind, name, n) in [
                ("affine", "AffineTransform_double_3_3", 12),
                ("euler", "Euler3DTransform_double_3_3", 6),
                ("versor", "VersorRigid3DTransform_double_3_3", 6),
                ("similarity", "Similarity3DTransform_double_3_3", 7),
            ] {
                let mut p: Vec<f64> = (0..n).map(|_| next()).collect();
                if kind == "affine" {
                    for d in 0..3 {
                        p[4 * d] += 1.0;
                    }
                }
                if kind == "similarity" {
                    p[6] = 1.0 + 0.2 * p[6];
                }
                let t0 = if kind == "affine" { 9 } else { 3 };
                for t in p.iter_mut().skip(t0).take(3) {
                    *t *= 20.0;
                }
                for zyx in [false, true] {
                    if zyx && kind != "euler" {
                        continue;
                    }
                    let f: Vec<f64> = if zyx {
                        vec![c[0], c[1], c[2], 1.0]
                    } else {
                        fixed.to_vec()
                    };
                    let old = legacy::matrix_offset(kind, &p, c, zyx);
                    let new = LinearTransform::from_itk(name, &p, &f).unwrap();
                    assert_eq!(new.matrix, old.0, "{kind} matrix");
                    assert_eq!(new.offset, old.1, "{kind} offset");
                    let inv = new.inverse().unwrap();
                    assert_eq!(inv.matrix, old.2, "{kind} inverse");
                    assert_eq!(inv.offset, old.3, "{kind} inverse offset");
                    // The float instantiation is close to it, except for versors whose axis
                    // reaches norm 1: there w = √(1 − sin²) is ill-conditioned, and in float
                    // the scaled axis can even round back above 1, which ITK rejects.
                    let axis_norm = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                    let versor_kind = kind == "versor" || kind == "similarity";
                    if versor_kind && axis_norm > 0.9 {
                        continue;
                    }
                    if let Ok(single) = new.with_precision::<f32>() {
                        for (row, exact) in single.matrix.iter().zip(&new.matrix) {
                            for (&a, &b) in row.iter().zip(exact) {
                                assert!((f64::from(a) - b).abs() <= 1e-6 * b.abs().max(1.0));
                            }
                        }
                    }
                }
            }
        }
    }
}

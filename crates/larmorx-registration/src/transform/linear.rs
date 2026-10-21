// SPDX-License-Identifier: Apache-2.0
//! The matrix-offset family (`MatrixOffsetTransformBase` and its subclasses), ITK v5.4.5
//! `itkMatrixOffsetTransformBase.hxx`, `itkAffineTransform.hxx`, `itkEuler3DTransform.hxx`,
//! `itkRigid2DTransform.hxx` (`Euler2DTransform`), `itkSimilarity2DTransform.hxx`.

use crate::{ItkException, Matrix, Real};

/// Which class a matrix-offset transform is, and so how its parameters are read.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `AffineTransform`: the matrix row by row, then the translation.
    Affine,
    /// `MatrixOffsetTransformBase`: as `AffineTransform`.
    MatrixOffset,
    /// `Euler3DTransform`: three angles (rotation about x, y, z; composed `Z·X·Y`, or `Z·Y·X`
    /// with `zyx`), then the translation.
    Euler3D { zyx: bool },
    /// `Euler2DTransform` (`Rigid2DTransform`): the angle, then the translation.
    Euler2D,
    /// `Rigid2DTransform` read from a file: `Euler2DTransform`'s parent, the same parameters
    /// and computations under its own name.
    Rigid2D,
    /// `Similarity2DTransform`: the scale, the angle, then the translation.
    Similarity2D,
    /// `Similarity3DTransform`: the versor's vector part, the translation, the scale.
    Similarity3D,
    /// ANTs' `ANTSAffine3DTransform`: the quaternion (x, y, z, r), the scales S1-S3, the
    /// shears K1-K3, the translation.
    AntsAffine3D,
    /// ANTs' `ANTSCenteredAffine2DTransform`: the angle, the scales S1 and S2, the shear K,
    /// the centre, the translation.
    AntsCenteredAffine2D,
    /// Another linear class, read from a file and never optimised: its own name, parameters
    /// and fixed parameters as stored.
    Other {
        class: String,
        parameters: Vec<f64>,
        fixed: Vec<f64>,
    },
}

impl Kind {
    /// `GetNameOfClass()`.
    pub fn class(&self) -> &'static str {
        match self {
            Kind::Affine => "AffineTransform",
            Kind::MatrixOffset => "MatrixOffsetTransformBase",
            Kind::Euler3D { .. } => "Euler3DTransform",
            Kind::Euler2D => "Euler2DTransform",
            Kind::Rigid2D => "Rigid2DTransform",
            Kind::Similarity2D => "Similarity2DTransform",
            Kind::Similarity3D => "Similarity3DTransform",
            Kind::AntsAffine3D => "ANTSAffine3DTransform",
            Kind::AntsCenteredAffine2D => "ANTSCenteredAffine2DTransform",
            Kind::Other { class, .. } => {
                // The class names a file can hold are a closed set; keep them static.
                match class.as_str() {
                    "VersorRigid3DTransform" => "VersorRigid3DTransform",
                    "Similarity3DTransform" => "Similarity3DTransform",
                    "Rigid3DTransform" => "Rigid3DTransform",
                    "ScaleSkewVersor3DTransform" => "ScaleSkewVersor3DTransform",
                    "ScaleVersor3DTransform" => "ScaleVersor3DTransform",
                    "QuaternionRigidTransform" => "QuaternionRigidTransform",
                    "CenteredAffineTransform" => "CenteredAffineTransform",
                    "Rigid2DTransform" => "Rigid2DTransform",
                    "CenteredRigid2DTransform" => "CenteredRigid2DTransform",
                    "CenteredEuler3DTransform" => "CenteredEuler3DTransform",
                    "ScalableAffineTransform" => "ScalableAffineTransform",
                    "ScaleTransform" => "ScaleTransform",
                    "ScaleLogarithmicTransform" => "ScaleLogarithmicTransform",
                    _ => "MatrixOffsetTransformBase",
                }
            }
        }
    }

    /// `GetNumberOfParameters()` in dimension `d`.
    pub fn number_of_parameters(&self, d: usize) -> usize {
        match self {
            Kind::Affine | Kind::MatrixOffset => d * d + d,
            Kind::Euler3D { .. } => 6,
            Kind::Euler2D | Kind::Rigid2D => 3,
            Kind::Similarity2D => 4,
            Kind::Similarity3D => 7,
            Kind::AntsAffine3D => 13,
            Kind::AntsCenteredAffine2D => 8,
            Kind::Other { parameters, .. } => parameters.len(),
        }
    }
}

/// `itk::Math::FloatAlmostEqual<R>(x, 0.0)` with its defaults (4 ulps, an absolute
/// difference of `0.1·ε`).
fn float_almost_zero<R: Real>(x: R) -> bool {
    let max_abs = R::from_f64(0.1 * R::EPSILON.to_f64());
    if x.abs() <= max_abs {
        return true;
    }
    if x.to_f64().is_sign_negative() {
        return false;
    }
    // Same sign as +0: the ulp distance is the bit pattern itself.
    match R::PRECISION {
        larmorx_core::Precision::Double => x.to_f64().to_bits() <= 4,
        larmorx_core::Precision::Float => (x.to_f64() as f32).to_bits() <= 4,
    }
}

/// `Vector<R, 3>::GetNorm()`: the squares summed in double, `sqrt` in double.
fn vector_norm<R: Real>(v: &[R; 3]) -> f64 {
    let mut sum = 0.0f64;
    for &c in v {
        let value = c.to_f64();
        sum += value * value;
    }
    sum.sqrt()
}

/// `Versor<R>::Set(const VectorType & axis)`: the vector part as given, `w = sqrt(1 − |v|²)`;
/// ITK throws for a vector longer than 1.
fn versor_from_vector<R: Real>(axis: [R; 3]) -> Result<[R; 4], ItkException> {
    let sinangle2 = R::from_f64(vector_norm(&axis));
    if sinangle2 > R::ONE {
        return Err(ItkException::located(
            "itk::Versor::Set( const VectorType )",
            "Trying to initialize a Versor with a vector whose magnitude is greater than 1",
        ));
    }
    let cosangle2 = R::from_f64((1.0 - (sinangle2 * sinangle2).to_f64()).sqrt());
    Ok([axis[0], axis[1], axis[2], cosangle2])
}

/// `Versor<R>::Set(const VectorType & axis, ValueType angle)`.
fn versor_from_axis_angle<R: Real>(axis: [R; 3], angle: R) -> Result<[R; 4], ItkException> {
    let vector_norm = vector_norm(&axis);
    if float_almost_zero(R::from_f64(vector_norm)) {
        // ITK sets the location to `__FILE__`, the header's path where ITK was built.
        return Err(ItkException::located(
            "Modules/Core/Common/include/itkVersor.hxx",
            "Attempt to set rotation axis with zero norm",
        ));
    }
    let half = angle.to_f64() / 2.0;
    let cosangle2 = larmorx_core::math::cos(half);
    let sinangle2 = larmorx_core::math::sin(half);
    let factor = sinangle2 / vector_norm;
    Ok([
        R::from_f64(axis[0].to_f64() * factor),
        R::from_f64(axis[1].to_f64() * factor),
        R::from_f64(axis[2].to_f64() * factor),
        R::from_f64(cosangle2),
    ])
}

/// `Versor<R>::operator*`: each product in `R`, summed left to right.
fn versor_product<R: Real>(a: &[R; 4], b: &[R; 4]) -> [R; 4] {
    let [ax, ay, az, aw] = *a;
    let [bx, by, bz, bw] = *b;
    [
        aw * bx - az * by + ay * bz + ax * bw,
        az * bx + aw * by - ax * bz + ay * bw,
        -ay * bx + ax * by + aw * bz + az * bw,
        -ax * bx - ay * by - az * bz + aw * bw,
    ]
}

/// `Versor<R>::GetMatrix()`: the products in `R`, the entries in double.
fn versor_matrix<R: Real>(v: &[R; 4]) -> [[R; 3]; 3] {
    let [x, y, z, w] = *v;
    let f = |a: R, b: R| (a * b).to_f64();
    let (xx, yy, zz) = (f(x, x), f(y, y), f(z, z));
    let (xy, xz, xw) = (f(x, y), f(x, z), f(x, w));
    let (yz, yw, zw) = (f(y, z), f(y, w), f(z, w));
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

/// `vnl_quaternion<R>::rotation_matrix_transpose()` of the conjugate of `(x, y, z, r)`, in
/// `R`.
fn quaternion_conjugate_matrix<R: Real>(q: &[R; 4]) -> [[R; 3]; 3] {
    let (x, y, z, r) = (-q[0], -q[1], -q[2], q[3]);
    let (x2, xy, rx) = (x * x, x * y, r * x);
    let (y2, yz, ry) = (y * y, y * z, r * y);
    let (z2, zx, rz) = (z * z, z * x, r * z);
    let r2 = r * r;
    let two = R::from_f64(2.0);
    [
        [r2 + x2 - y2 - z2, two * (xy + rz), two * (zx - ry)],
        [two * (xy - rz), r2 - x2 + y2 - z2, two * (yz + rx)],
        [two * (zx + ry), two * (yz - rx), r2 - x2 - y2 + z2],
    ]
}

/// A matrix-offset transform in the computation type `R`.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearXfm<R, const D: usize> {
    pub kind: Kind,
    pub matrix: Matrix<R, D>,
    pub offset: [R; D],
    pub center: [R; D],
    pub translation: [R; D],
    /// Euler angles (`Euler3DTransform`: x, y, z; the 2D classes: the angle first).
    pub angles: [R; 3],
    /// The similarity transforms' scale.
    pub scale: R,
    /// `Similarity3DTransform`'s versor (x, y, z, w); `ANTSAffine3DTransform`'s quaternion
    /// (x, y, z, r).
    pub versor: [R; 4],
    /// The ANTs affine classes' scales (S1, S2, S3) and shears (K1, K2, K3; 2D: K).
    pub scales: [R; 3],
    pub shears: [R; 3],
    /// Fixed parameters after the centre that `m_FixedParameters` keeps: an inverse made by
    /// `GetInverse` takes its source's fixed parameters (an `Euler3DTransform`'s four, the
    /// ZYX flag last), and `GetFixedParameters` overwrites only the centre.
    pub extra_fixed: Vec<f64>,
}

/// `vnl_matrix_fixed` product in `R`: each entry starts from the first product.
pub fn mat_mat_r<R: Real, const D: usize>(a: &Matrix<R, D>, b: &Matrix<R, D>) -> Matrix<R, D> {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut s = a[i][0] * b[0][j];
            for k in 1..D {
                s = s + a[i][k] * b[k][j];
            }
            s
        })
    })
}

fn identity<R: Real, const D: usize>() -> Matrix<R, D> {
    std::array::from_fn(|i| std::array::from_fn(|j| if i == j { R::ONE } else { R::ZERO }))
}

impl<R: Real, const D: usize> LinearXfm<R, D> {
    /// A new transform of `kind` with its default parameters (identity matrix, zero centre
    /// and translation, scale 1).
    pub fn new(kind: Kind) -> Self {
        LinearXfm {
            kind,
            matrix: identity(),
            offset: [R::ZERO; D],
            center: [R::ZERO; D],
            translation: [R::ZERO; D],
            angles: [R::ZERO; 3],
            scale: R::ONE,
            versor: [R::ZERO, R::ZERO, R::ZERO, R::ONE],
            scales: [R::ONE; 3],
            shears: [R::ZERO; 3],
            extra_fixed: Vec::new(),
        }
    }

    /// A transform of another class from its matrix and offset as ITK holds them.
    pub fn frozen(
        class: &str,
        parameters: Vec<f64>,
        fixed: Vec<f64>,
        matrix: Matrix<R, D>,
        offset: [R; D],
        center: [R; D],
    ) -> Self {
        let kind = match class {
            "AffineTransform" => Kind::Affine,
            "MatrixOffsetTransformBase" => Kind::MatrixOffset,
            _ => Kind::Other {
                class: class.to_owned(),
                parameters,
                fixed,
            },
        };
        let mut t = LinearXfm::new(kind);
        t.matrix = matrix;
        t.offset = offset;
        t.center = center;
        t.compute_translation();
        t
    }

    /// `ComputeOffset`: `offset = translation + center`, then `−= M[i][j]·center[j]`.
    pub fn compute_offset(&mut self) {
        for i in 0..D {
            let mut o = self.translation[i] + self.center[i];
            for j in 0..D {
                o = o - self.matrix[i][j] * self.center[j];
            }
            self.offset[i] = o;
        }
    }

    /// `ComputeTranslation`: `translation = offset − center`, then `+= M[i][j]·center[j]`.
    pub fn compute_translation(&mut self) {
        for i in 0..D {
            let mut t = self.offset[i] - self.center[i];
            for j in 0..D {
                t = t + self.matrix[i][j] * self.center[j];
            }
            self.translation[i] = t;
        }
    }

    /// `SetMatrix` of an `AffineTransform` (`ComputeMatrixParameters` does nothing).
    pub fn set_matrix(&mut self, m: Matrix<R, D>) {
        self.matrix = m;
        self.compute_offset();
    }

    /// `SetOffset`.
    pub fn set_offset(&mut self, o: [R; D]) {
        self.offset = o;
        self.compute_translation();
    }

    /// `Compose(other, pre = true)`: `offset = M·other.offset + offset` (each row summed from
    /// 0 in `R`), `M = M·other.M`, then `ComputeTranslation`.
    pub fn compose_pre(&mut self, other: &Self) {
        let mv: [R; D] = std::array::from_fn(|r| {
            let mut s = R::ZERO;
            for c in 0..D {
                s = s + self.matrix[r][c] * other.offset[c];
            }
            s
        });
        self.offset = std::array::from_fn(|i| mv[i] + self.offset[i]);
        self.matrix = mat_mat_r(&self.matrix, &other.matrix);
        self.compute_translation();
    }

    /// `GetInverseTransform()`. `AffineTransform` and `MatrixOffsetTransformBase`: `M⁻¹` by
    /// vnl's SVD in `R`, `offset = −(M⁻¹·offset)`, `ComputeTranslation`, the centre kept. The
    /// 2D rotations (`Rigid2DTransform`, `Euler2DTransform`, `Similarity2DTransform`) invert
    /// to their own class through their parameters ([`LinearXfm::inverse_2d`]). Other classes
    /// invert to a `MatrixOffsetTransformBase` (ITK's `InvertTransform` with the base type).
    /// `None` for a singular matrix.
    pub fn inverse(&self) -> Option<Self> {
        if matches!(
            self.kind,
            Kind::Euler2D | Kind::Rigid2D | Kind::Similarity2D
        ) {
            return self.inverse_2d();
        }
        let inv = inverse_r(&self.matrix)?;
        let mv: [R; D] = std::array::from_fn(|r| {
            let mut s = R::ZERO;
            for c in 0..D {
                s = s + inv[r][c] * self.offset[c];
            }
            s
        });
        let kind = match &self.kind {
            Kind::Affine => Kind::Affine,
            _ => Kind::MatrixOffset,
        };
        let mut out = LinearXfm::new(kind);
        out.extra_fixed = self
            .fixed_parameters()
            .get(D..)
            .unwrap_or_default()
            .to_vec();
        out.center = self.center;
        out.matrix = inv;
        out.offset = std::array::from_fn(|i| -mv[i]);
        out.compute_translation();
        Some(out)
    }

    /// `Rigid2DTransform::GetInverse` (and `Euler2DTransform`'s, the same) and
    /// `Similarity2DTransform::GetInverse`: the same fixed parameters and centre, the scale
    /// `1.0 / scale` (in double, stored in `R`), the angle negated, the translation
    /// `−(M⁻¹·translation)` with this transform's inverse matrix (vnl's, in `R`; each row
    /// summed from 0); the matrix and offset recomputed. `None` for a singular matrix.
    fn inverse_2d(&self) -> Option<Self> {
        let inv = inverse_r(&self.matrix)?;
        let mut out = LinearXfm::new(self.kind.clone());
        out.extra_fixed = self.extra_fixed.clone();
        out.center = self.center;
        if self.kind == Kind::Similarity2D {
            out.scale = R::from_f64(1.0 / self.scale.to_f64());
        }
        out.angles[0] = -self.angles[0];
        out.translation = std::array::from_fn(|r| {
            let mut s = R::ZERO;
            for c in 0..D {
                s = s + inv[r][c] * self.translation[c];
            }
            -s
        });
        out.compute_matrix();
        out.compute_offset();
        Some(out)
    }

    /// `SetCenter`.
    pub fn set_center(&mut self, c: [R; D]) {
        self.center = c;
        self.compute_offset();
    }

    /// `SetIdentity()`: the identity matrix, a zero offset and translation; the centre is
    /// kept (ITK 5.4: it is a fixed parameter); the classes' own parameters reset (angles 0,
    /// scale 1, the identity versor).
    pub fn set_identity(&mut self) {
        self.matrix = identity();
        self.offset = [R::ZERO; D];
        self.translation = [R::ZERO; D];
        self.angles = [R::ZERO; 3];
        self.scale = R::ONE;
        self.versor = [R::ZERO, R::ZERO, R::ZERO, R::ONE];
        self.scales = [R::ONE; 3];
        self.shears = [R::ZERO; 3];
    }

    /// `itk::Matrix · itk::Vector` in `R`: each row summed from 0.
    fn matrix_vector(m: &Matrix<R, D>, v: &[R; D]) -> [R; D] {
        std::array::from_fn(|r| {
            let mut s = R::ZERO;
            for c in 0..D {
                s = s + m[r][c] * v[c];
            }
            s
        })
    }

    /// `AffineTransform::Translate(trans, pre)`: the translation moved by `M·trans` (`pre`)
    /// or by `trans`, then `ComputeOffset`.
    pub fn translate(&mut self, trans: &[R; D], pre: bool) {
        let add = if pre {
            Self::matrix_vector(&self.matrix, trans)
        } else {
            *trans
        };
        for i in 0..D {
            self.translation[i] = self.translation[i] + add[i];
        }
        self.compute_offset();
    }

    /// `AffineTransform::Scale(factor, pre)`: every matrix entry times `factor` (and, unless
    /// `pre`, the translation), then `ComputeOffset`.
    pub fn scale_uniform(&mut self, factor: R, pre: bool) {
        for row in self.matrix.iter_mut() {
            for v in row.iter_mut() {
                *v = *v * factor;
            }
        }
        if !pre {
            for t in self.translation.iter_mut() {
                *t = *t * factor;
            }
        }
        self.compute_offset();
    }

    /// Composes the matrix with `trans`: `M·trans` (`pre`), or `trans·M` with the translation
    /// `trans·t`; then `ComputeOffset`.
    fn compose_matrix(&mut self, trans: &Matrix<R, D>, pre: bool) {
        if pre {
            self.matrix = mat_mat_r(&self.matrix, trans);
        } else {
            self.matrix = mat_mat_r(trans, &self.matrix);
            self.translation = Self::matrix_vector(trans, &self.translation);
        }
        self.compute_offset();
    }

    /// `AffineTransform::Rotate2D(angle, pre)` (2D): the rotation
    /// `[[cos, −sin], [sin, cos]]` of `std::cos`/`std::sin` in `R`.
    pub fn rotate_2d(&mut self, angle: R, pre: bool) {
        assert_eq!(D, 2, "Rotate2D needs a 2D transform");
        let mut trans: Matrix<R, D> = [[R::ZERO; D]; D];
        trans[0][0] = angle.cos();
        trans[0][1] = -angle.sin();
        trans[1][0] = angle.sin();
        trans[1][1] = angle.cos();
        self.compose_matrix(&trans, pre);
    }

    /// `AffineTransform::Rotate3D(axis, angle, pre)` (3D): the rotation of the unit quaternion
    /// `(cos(θ/2), axis/|axis|·sin(θ/2))`; `θ/2` and its cosine and sine in double, the rest
    /// in `R` (each `2.0 · (…)` in double).
    pub fn rotate_3d(&mut self, axis: &[R; D], angle: R, pre: bool) {
        assert_eq!(D, 3, "Rotate3D needs a 3D transform");
        let r = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        let x1 = axis[0] / r;
        let x2 = axis[1] / r;
        let x3 = axis[2] / r;
        let half = angle.to_f64() / 2.0;
        let c = larmorx_core::math::cos(half);
        let s = larmorx_core::math::sin(half);
        let q0 = R::from_f64(c);
        let q1 = R::from_f64(x1.to_f64() * s);
        let q2 = R::from_f64(x2.to_f64() * s);
        let q3 = R::from_f64(x3.to_f64() * s);
        let two = |v: R| R::from_f64(2.0 * v.to_f64());
        let mut trans: Matrix<R, D> = [[R::ZERO; D]; D];
        trans[0][0] = q0 * q0 + q1 * q1 - q2 * q2 - q3 * q3;
        trans[0][1] = two(q1 * q2 - q0 * q3);
        trans[0][2] = two(q1 * q3 + q0 * q2);
        trans[1][0] = two(q1 * q2 + q0 * q3);
        trans[1][1] = q0 * q0 + q2 * q2 - q1 * q1 - q3 * q3;
        trans[1][2] = two(q2 * q3 - q0 * q1);
        trans[2][0] = two(q1 * q3 - q0 * q2);
        trans[2][1] = two(q2 * q3 + q0 * q1);
        trans[2][2] = q0 * q0 + q3 * q3 - q1 * q1 - q2 * q2;
        self.compose_matrix(&trans, pre);
    }

    /// `SetTranslation`.
    pub fn set_translation(&mut self, t: [R; D]) {
        self.translation = t;
        self.compute_offset();
    }

    /// `ComputeMatrix` of the parameterised classes.
    pub(crate) fn compute_matrix(&mut self) {
        match self.kind {
            Kind::Euler3D { zyx } => {
                let [ax, ay, az] = self.angles;
                let (cx, sx) = (ax.cos(), ax.sin());
                let (cy, sy) = (ay.cos(), ay.sin());
                let (cz, sz) = (az.cos(), az.sin());
                let (o, z) = (R::ONE, R::ZERO);
                let rx = [[o, z, z], [z, cx, -sx], [z, sx, cx]];
                let ry = [[cy, z, sy], [z, o, z], [-sy, z, cy]];
                let rz = [[cz, -sz, z], [sz, cz, z], [z, z, o]];
                let m = if zyx {
                    mat_mat_r(&mat_mat_r(&rz, &ry), &rx)
                } else {
                    mat_mat_r(&mat_mat_r(&rz, &rx), &ry)
                };
                for i in 0..3 {
                    for j in 0..3 {
                        self.matrix[i][j] = m[i][j];
                    }
                }
            }
            Kind::Euler2D | Kind::Rigid2D => {
                let ca = self.angles[0].cos();
                let sa = self.angles[0].sin();
                self.matrix[0][0] = ca;
                self.matrix[0][1] = -sa;
                self.matrix[1][0] = sa;
                self.matrix[1][1] = ca;
            }
            Kind::Similarity2D => {
                let angle = self.angles[0].to_f64();
                let cc = larmorx_core::math::cos(angle);
                let ss = larmorx_core::math::sin(angle);
                let s = self.scale.to_f64();
                let ca = R::from_f64(cc * s);
                let sa = R::from_f64(ss * s);
                self.matrix[0][0] = ca;
                self.matrix[0][1] = -sa;
                self.matrix[1][0] = sa;
                self.matrix[1][1] = ca;
            }
            Kind::Similarity3D => {
                // VersorTransform::ComputeMatrix, then `matrix *= m_Scale` in R.
                let m = versor_matrix(&self.versor);
                for i in 0..3 {
                    for j in 0..3 {
                        self.matrix[i][j] = m[i][j] * self.scale;
                    }
                }
            }
            Kind::AntsAffine3D => {
                // R·S·K: the conjugate's transposed rotation matrix, the scales, the shears.
                let r = quaternion_conjugate_matrix(&self.versor);
                let (o, z) = (R::ONE, R::ZERO);
                let [s1, s2, s3] = self.scales;
                let [k1, k2, k3] = self.shears;
                let s = [[s1, z, z], [z, s2, z], [z, z, s3]];
                let k = [[o, k1, k2], [z, o, k3], [z, z, o]];
                let m = mat_mat_r(&mat_mat_r(&r, &s), &k);
                for i in 0..3 {
                    for j in 0..3 {
                        self.matrix[i][j] = m[i][j];
                    }
                }
            }
            Kind::AntsCenteredAffine2D => {
                // The cosine and sine of the angle in R (`std::cos(m_Angle)`), then
                // rotation·scale·shear in R.
                let ca = self.angles[0].cos();
                let sa = self.angles[0].sin();
                let (o, z) = (R::ONE, R::ZERO);
                let rot = [[ca, -sa], [sa, ca]];
                let scale = [[self.scales[0], z], [z, self.scales[1]]];
                let shear = [[o, self.shears[0]], [z, o]];
                let m = mat_mat_r(&mat_mat_r(&rot, &scale), &shear);
                for i in 0..2 {
                    for j in 0..2 {
                        self.matrix[i][j] = m[i][j];
                    }
                }
            }
            _ => {}
        }
    }

    /// `GetParameters()`.
    pub fn parameters(&self) -> Vec<R> {
        match &self.kind {
            Kind::Affine | Kind::MatrixOffset => {
                let mut p: Vec<R> = self.matrix.iter().flatten().copied().collect();
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Euler3D { .. } => {
                let mut p = self.angles.to_vec();
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Euler2D | Kind::Rigid2D => {
                let mut p = vec![self.angles[0]];
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Similarity2D => {
                let mut p = vec![self.scale, self.angles[0]];
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Similarity3D => {
                let mut p = self.versor[..3].to_vec();
                p.extend_from_slice(&self.translation);
                p.push(self.scale);
                p
            }
            Kind::AntsAffine3D => {
                let mut p = self.versor.to_vec();
                p.extend_from_slice(&self.scales);
                p.extend_from_slice(&self.shears);
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::AntsCenteredAffine2D => {
                let mut p = vec![
                    self.angles[0],
                    self.scales[0],
                    self.scales[1],
                    self.shears[0],
                ];
                p.extend_from_slice(&self.center);
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Other { parameters, .. } => parameters.iter().map(|&v| R::from_f64(v)).collect(),
        }
    }

    /// `SetParameters`. `Similarity3DTransform`'s throws where ITK's versor does (a vector
    /// part longer than 1 after its normalisation, possible in float).
    pub fn set_parameters(&mut self, p: &[R]) -> Result<(), ItkException> {
        match &self.kind {
            Kind::Affine | Kind::MatrixOffset => {
                let mut k = 0;
                for r in 0..D {
                    for c in 0..D {
                        self.matrix[r][c] = p[k];
                        k += 1;
                    }
                }
                for i in 0..D {
                    self.translation[i] = p[k];
                    k += 1;
                }
                self.compute_offset();
            }
            Kind::Euler3D { .. } => {
                self.angles = [p[0], p[1], p[2]];
                self.compute_matrix();
                for i in 0..D {
                    self.translation[i] = p[3 + i];
                }
                self.compute_offset();
            }
            Kind::Euler2D | Kind::Rigid2D => {
                self.angles[0] = p[0];
                for i in 0..D {
                    self.translation[i] = p[1 + i];
                }
                self.compute_matrix();
                self.compute_offset();
            }
            Kind::Similarity2D => {
                self.scale = p[0];
                self.angles[0] = p[1];
                for i in 0..D {
                    self.translation[i] = p[2 + i];
                }
                self.compute_matrix();
                self.compute_offset();
            }
            Kind::Similarity3D => {
                // The squares in R, their sum in double; a vector part of norm 1 or more is
                // scaled by 1/(norm·(1 + 1e-10)) in double.
                let mut axis = [p[0], p[1], p[2]];
                let mut norm = (p[0] * p[0]).to_f64();
                norm += (p[1] * p[1]).to_f64();
                norm += (p[2] * p[2]).to_f64();
                if norm > 0.0 {
                    norm = norm.sqrt();
                }
                let epsilon = 1e-10;
                if norm >= 1.0 - epsilon {
                    let divisor = norm + epsilon * norm;
                    for a in &mut axis {
                        *a = R::from_f64(a.to_f64() / divisor);
                    }
                }
                self.versor = versor_from_vector(axis)?;
                self.scale = p[6];
                self.compute_matrix();
                for i in 0..D {
                    self.translation[i] = p[3 + i];
                }
                self.compute_offset();
            }
            Kind::AntsAffine3D => {
                self.versor = [p[0], p[1], p[2], p[3]];
                self.scales = [p[4], p[5], p[6]];
                self.shears = [p[7], p[8], p[9]];
                self.compute_matrix();
                for i in 0..D {
                    self.translation[i] = p[10 + i];
                }
                self.compute_offset();
            }
            Kind::AntsCenteredAffine2D => {
                self.angles[0] = p[0];
                self.scales[0] = p[1];
                self.scales[1] = p[2];
                self.shears[0] = p[3];
                for i in 0..D {
                    self.center[i] = p[4 + i];
                    self.translation[i] = p[6 + i];
                }
                self.compute_matrix();
                self.compute_offset();
            }
            Kind::Other { .. } => {}
        }
        Ok(())
    }

    /// `GetFixedParameters()`: the centre (`Euler3DTransform`: and its ZYX flag), or the
    /// stored ones of another class.
    pub fn fixed_parameters(&self) -> Vec<f64> {
        match &self.kind {
            Kind::Other { fixed, .. } => fixed.clone(),
            Kind::Euler3D { zyx } => {
                let mut f: Vec<f64> = self.center.iter().map(|v| v.to_f64()).collect();
                f.push(if *zyx { 1.0 } else { 0.0 });
                f
            }
            _ => {
                let mut f: Vec<f64> = self.center.iter().map(|v| v.to_f64()).collect();
                f.extend_from_slice(&self.extra_fixed);
                f
            }
        }
    }

    /// `UpdateTransformParameters`: `Transform`'s (`GetParameters`, `+= update·factor`,
    /// `SetParameters`), or `VersorRigid3DTransform`'s for `Similarity3DTransform` (the
    /// rotation composed with the update's rotation, the rest added).
    pub fn update(&mut self, update: &[R], factor: R) -> Result<(), ItkException> {
        let n = self.kind.number_of_parameters(D);
        if update.len() != n {
            let (file, line) = match self.kind {
                Kind::Similarity3D => (
                    "Modules/Core/Transform/include/itkVersorRigid3DTransform.hxx",
                    131,
                ),
                _ => ("Modules/Core/Transform/include/itkTransform.hxx", 76),
            };
            return Err(ItkException::new(
                file,
                line,
                self.kind.class(),
                format!(
                    "Parameter update size, {}, must  be same as transform parameter size, {n}\n",
                    update.len()
                ),
            ));
        }
        let mut p = self.parameters();
        if self.kind == Kind::Similarity3D {
            let current = versor_from_vector([p[0], p[1], p[2]])?;
            let mut axis = [update[0], update[1], update[2]];
            let norm = R::from_f64(vector_norm(&axis));
            let gradient = if float_almost_zero(norm) {
                axis[2] = R::ONE;
                versor_from_axis_angle(axis, R::ZERO)?
            } else {
                versor_from_axis_angle(axis, factor * norm)?
            };
            let rotation = versor_product(&current, &gradient);
            p[..3].copy_from_slice(&rotation[..3]);
            for k in 3..n {
                p[k] = p[k] + update[k] * factor;
            }
            return self.set_parameters(&p);
        }
        if factor == R::ONE {
            for k in 0..n {
                p[k] = p[k] + update[k];
            }
        } else {
            for k in 0..n {
                p[k] = p[k] + update[k] * factor;
            }
        }
        self.set_parameters(&p)
    }

    /// `ComputeJacobianWithRespectToParameters`, `D` rows of `n` values.
    pub fn jacobian_parameters(&self, p: &[R; D], out: &mut [R]) {
        out.fill(R::ZERO);
        let n = self.kind.number_of_parameters(D);
        let j = |r: usize, c: usize| r * n + c;
        match &self.kind {
            Kind::Similarity3D => {
                // `Similarity3DTransform::ComputeJacobianWithRespectToParameters`: the versor's
                // products in R widened, the rest in double; the scale column is
                // `(M·(p − c)) / s` in R.
                let [vx, vy, vz, vw] = self.versor;
                let pp: [R; 3] = std::array::from_fn(|i| p[i] - self.center[i]);
                let (px, py, pz) = (pp[0].to_f64(), pp[1].to_f64(), pp[2].to_f64());
                let f = |a: R, b: R| (a * b).to_f64();
                let (vxx, vyy, vzz, vww) = (f(vx, vx), f(vy, vy), f(vz, vz), f(vw, vw));
                let (vxy, vxz, vxw) = (f(vx, vy), f(vx, vz), f(vx, vw));
                let (vyz, vyw, vzw) = (f(vy, vz), f(vy, vw), f(vz, vw));
                let s2 = self.scale.to_f64() * 2.0;
                let w = vw.to_f64();
                let r = R::from_f64;
                out[j(0, 0)] = r(s2 * ((vyw + vxz) * py + (vzw - vxy) * pz) / w);
                out[j(1, 0)] = r(s2 * ((vyw - vxz) * px - 2.0 * vxw * py + (vxx - vww) * pz) / w);
                out[j(2, 0)] = r(s2 * ((vzw + vxy) * px + (vww - vxx) * py - 2.0 * vxw * pz) / w);
                out[j(0, 1)] = r(s2 * (-2.0 * vyw * px + (vxw + vyz) * py + (vww - vyy) * pz) / w);
                out[j(1, 1)] = r(s2 * ((vxw - vyz) * px + (vzw + vxy) * pz) / w);
                out[j(2, 1)] = r(s2 * ((vyy - vww) * px + (vzw - vxy) * py - 2.0 * vyw * pz) / w);
                out[j(0, 2)] = r(s2 * (-2.0 * vzw * px + (vzz - vww) * py + (vxw - vyz) * pz) / w);
                out[j(1, 2)] = r(s2 * ((vww - vzz) * px - 2.0 * vzw * py + (vyw + vxz) * pz) / w);
                out[j(2, 2)] = r(s2 * ((vxw + vyz) * px + (vyw - vxz) * py) / w);
                out[j(0, 3)] = R::ONE;
                out[j(1, 4)] = R::ONE;
                out[j(2, 5)] = R::ONE;
                for row in 0..3 {
                    let mut sum = R::ZERO;
                    for c in 0..3 {
                        sum = sum + self.matrix[row][c] * pp[c];
                    }
                    out[j(row, 6)] = sum / self.scale;
                }
            }
            Kind::AntsAffine3D => {
                // `ANTSAffine3DTransform::ComputeJacobianWithRespectToParameters`, all in R.
                let [c1, c2, c3] = [self.center[0], self.center[1], self.center[2]];
                let [s1, s2, s3] = self.scales;
                let [k1, k2, k3] = self.shears;
                let [x1, x2, x3] = [p[0], p[1], p[2]];
                let w1 = (x1 - c1) + k1 * (x2 - c2) + k2 * (x3 - c3);
                let w2 = (x2 - c2) + k3 * (x3 - c3);
                let w3 = x3 - c3;
                let (z1, z2, z3) = (s1 * w1, s2 * w2, s3 * w3);
                let [qx, qy, qz, qr] = self.versor;
                let two = R::from_f64(2.0);
                let j00 = two * (qx * z1 + qy * z2 + qz * z3);
                let j01 = two * (-qy * z1 + qx * z2 + qr * z3);
                let j02 = two * (-qz * z1 - qr * z2 + qx * z3);
                let j03 = -two * (-qr * z1 + qz * z2 - qy * z3);
                out[j(0, 0)] = j00;
                out[j(0, 1)] = j01;
                out[j(0, 2)] = j02;
                out[j(0, 3)] = j03;
                out[j(1, 0)] = -j01;
                out[j(1, 1)] = j00;
                out[j(1, 2)] = j03;
                out[j(1, 3)] = -j02;
                out[j(2, 0)] = -j02;
                out[j(2, 1)] = -j03;
                out[j(2, 2)] = j00;
                out[j(2, 3)] = j01;
                let m = quaternion_conjugate_matrix(&self.versor);
                let w = [w1, w2, w3];
                for row in 0..3 {
                    for col in 0..3 {
                        out[j(row, 4 + col)] = m[row][col] * w[col];
                    }
                    out[j(row, 7)] = m[row][0] * s1 * (x2 - c2);
                    out[j(row, 8)] = m[row][0] * s1 * (x3 - c3);
                    out[j(row, 9)] = m[row][1] * s2 * (x3 - c3);
                    out[j(row, 10 + row)] = R::ONE;
                }
            }
            Kind::AntsCenteredAffine2D => {
                // `ANTSCenteredAffine2DTransform::ComputeJacobianWithRespectToParameters`: the
                // cosine and sine in R, everything else in double.
                let ca = self.angles[0].cos().to_f64();
                let sa = self.angles[0].sin().to_f64();
                let s1 = self.scales[0].to_f64();
                let s2 = self.scales[1].to_f64();
                let k = self.shears[0].to_f64();
                let cx = self.center[0].to_f64();
                let cy = self.center[1].to_f64();
                let pxoff = (p[0].to_f64() - cx) + k * (p[1].to_f64() - cy);
                let pyoff = p[1].to_f64() - cy;
                let r = R::from_f64;
                out[j(0, 0)] = r(s1 * pxoff * (-sa) + s2 * pyoff * (-ca));
                out[j(1, 0)] = r(s1 * pxoff * ca + s2 * pyoff * (-sa));
                out[j(0, 1)] = r(ca * pxoff);
                out[j(0, 2)] = r(-sa * pyoff);
                out[j(1, 1)] = r(sa * pxoff);
                out[j(1, 2)] = r(ca * pyoff);
                out[j(0, 3)] = r(ca * s1 * pyoff);
                out[j(1, 3)] = r(sa * s1 * pyoff);
                out[j(0, 4)] = r(-s1 * ca + 1.0);
                out[j(0, 5)] = r(-(k * s1 * ca - s2 * sa));
                out[j(1, 4)] = r(-s1 * sa);
                out[j(1, 5)] = r(-(k * s1 * sa + s2 * ca) + 1.0);
                out[j(0, 6)] = R::ONE;
                out[j(1, 7)] = R::ONE;
            }
            Kind::Affine | Kind::MatrixOffset | Kind::Other { .. } => {
                // Blocks of diagonal matrices: row `block`, columns `block·D + dim`, hold
                // `v[dim]`, `v = p − center` in R; the translation block is the identity.
                let v: [R; D] = std::array::from_fn(|i| p[i] - self.center[i]);
                let mut block_offset = 0;
                for block in 0..D {
                    for dim in 0..D {
                        out[block * n + block_offset + dim] = v[dim];
                    }
                    block_offset += D;
                }
                for dim in 0..D {
                    out[dim * n + block_offset + dim] = R::ONE;
                }
            }
            Kind::Euler3D { zyx } => {
                let f = |a: R| (a.cos().to_f64(), a.sin().to_f64());
                let (cx, sx) = f(self.angles[0]);
                let (cy, sy) = f(self.angles[1]);
                let (cz, sz) = f(self.angles[2]);
                let px = (p[0] - self.center[0]).to_f64();
                let py = (p[1] - self.center[1]).to_f64();
                let pz = (p[2] - self.center[2]).to_f64();
                let j = |r: usize, c: usize| r * n + c;
                let s = R::from_f64;
                if *zyx {
                    out[j(0, 0)] =
                        s((cz * sy * cx + sz * sx) * py + (-cz * sy * sx + sz * cx) * pz);
                    out[j(1, 0)] =
                        s((sz * sy * cx - cz * sx) * py + (-sz * sy * sx - cz * cx) * pz);
                    out[j(2, 0)] = s((cy * cx) * py + (-cy * sx) * pz);
                    out[j(0, 1)] = s((-cz * sy) * px + (cz * cy * sx) * py + (cz * cy * cx) * pz);
                    out[j(1, 1)] = s((-sz * sy) * px + (sz * cy * sx) * py + (sz * cy * cx) * pz);
                    out[j(2, 1)] = s((-cy) * px + (-sy * sx) * py + (-sy * cx) * pz);
                    out[j(0, 2)] = s((-sz * cy) * px
                        + (-sz * sy * sx - cz * cx) * py
                        + (-sz * sy * cx + cz * sx) * pz);
                    out[j(1, 2)] = s((cz * cy) * px
                        + (cz * sy * sx - sz * cx) * py
                        + (cz * sy * cx + sz * sx) * pz);
                    out[j(2, 2)] = R::ZERO;
                } else {
                    out[j(0, 0)] = s((-sz * cx * sy) * px + (sz * sx) * py + (sz * cx * cy) * pz);
                    out[j(1, 0)] = s((cz * cx * sy) * px + (-cz * sx) * py + (-cz * cx * cy) * pz);
                    out[j(2, 0)] = s((sx * sy) * px + (cx) * py + (-sx * cy) * pz);
                    out[j(0, 1)] =
                        s((-cz * sy - sz * sx * cy) * px + (cz * cy - sz * sx * sy) * pz);
                    out[j(1, 1)] =
                        s((-sz * sy + cz * sx * cy) * px + (sz * cy + cz * sx * sy) * pz);
                    out[j(2, 1)] = s((-cx * cy) * px + (-cx * sy) * pz);
                    out[j(0, 2)] = s((-sz * cy - cz * sx * sy) * px
                        + (-cz * cx) * py
                        + (-sz * sy + cz * sx * cy) * pz);
                    out[j(1, 2)] = s((cz * cy - sz * sx * sy) * px
                        + (-sz * cx) * py
                        + (cz * sy + sz * sx * cy) * pz);
                    out[j(2, 2)] = R::ZERO;
                }
                for dim in 0..3 {
                    out[j(dim, 3 + dim)] = R::ONE;
                }
            }
            Kind::Euler2D | Kind::Rigid2D => {
                let ca = self.angles[0].cos().to_f64();
                let sa = self.angles[0].sin().to_f64();
                let cx = self.center[0].to_f64();
                let cy = self.center[1].to_f64();
                let (x, y) = (p[0].to_f64() - cx, p[1].to_f64() - cy);
                out[0] = R::from_f64(-sa * x - ca * y);
                out[n] = R::from_f64(ca * x - sa * y);
                out[1] = R::ONE;
                out[n + 2] = R::ONE;
            }
            Kind::Similarity2D => {
                let angle = self.angles[0].to_f64();
                let ca = larmorx_core::math::cos(angle);
                let sa = larmorx_core::math::sin(angle);
                let cx = self.center[0].to_f64();
                let cy = self.center[1].to_f64();
                let (x, y) = (p[0].to_f64() - cx, p[1].to_f64() - cy);
                let s = self.scale.to_f64();
                out[0] = R::from_f64(ca * x - sa * y);
                out[n] = R::from_f64(sa * x + ca * y);
                out[1] = R::from_f64((-sa * x - ca * y) * s);
                out[n + 1] = R::from_f64((ca * x - sa * y) * s);
                out[2] = R::ONE;
                out[n + 3] = R::ONE;
            }
        }
    }
}

/// `itk::Matrix<R, D, D>::GetInverse`: `None` where vnl's determinant is 0 (ITK throws, and
/// `MatrixOffsetTransformBase` marks the transform singular), else vnl's SVD inverse
/// computed in `R` (`vnl_matrix_inverse<R>`).
pub fn inverse_r<R: Real, const D: usize>(m: &Matrix<R, D>) -> Option<Matrix<R, D>> {
    let as64: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j].to_f64()));
    if crate::space::determinant(&as64) == 0.0 || as64.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    match R::PRECISION {
        larmorx_core::Precision::Double => {
            let flat: Vec<f64> = as64.iter().flatten().copied().collect();
            let inv = larmorx_core::vnl_svd::vnl_inverse(&flat, D)?;
            Some(std::array::from_fn(|i| {
                std::array::from_fn(|j| R::from_f64(inv[i * D + j]))
            }))
        }
        larmorx_core::Precision::Float => {
            use larmorx_core::vnl_svd_f32::{matmul_f32, vnl_svd_f32};
            let flat: Vec<f32> = as64.iter().flatten().map(|&v| v as f32).collect();
            let (u, w, v) = vnl_svd_f32(&flat, D, D)?;
            let mut vr = vec![0.0f32; D * D];
            let mut ut = vec![0.0f32; D * D];
            let mut winv = vec![0.0f32; D * D];
            for i in 0..D {
                for j in 0..D {
                    vr[i * D + j] = v[i + D * j];
                    ut[i * D + j] = u[j + D * i];
                }
                winv[i * (D + 1)] = if w[i].abs() <= 0.0 { 0.0 } else { 1.0 / w[i] };
            }
            let vw = matmul_f32(&vr, D, D, &winv, D);
            let out = matmul_f32(&vw, D, D, &ut, D);
            Some(std::array::from_fn(|i| {
                std::array::from_fn(|j| R::from_f64(f64::from(out[i * D + j])))
            }))
        }
    }
}

/// `CollapseLinearTransforms` (ANTs' helper): an `AffineTransform` centred where the last
/// matrix-offset transform of the queue is, composed (`Compose(·, pre = true)`) with each
/// transform of the queue in order, each as an `AffineTransform` built from its centre,
/// matrix and translation (a `TranslationTransform` from its offset). `None` for an
/// `IdentityTransform` in the queue (ANTs dereferences a null pointer there).
pub fn collapse_linear<R: Real, const D: usize>(
    queue: &[super::Xfm<R, D>],
) -> Option<LinearXfm<R, D>> {
    use super::Xfm;
    let mut total = LinearXfm::<R, D>::new(Kind::Affine);
    if let Some(c) = queue.iter().rev().find_map(|t| match t {
        Xfm::Linear(l) => Some(l.center),
        _ => None,
    }) {
        total.set_center(c);
    }
    for t in queue {
        let mut nth = LinearXfm::<R, D>::new(Kind::Affine);
        match t {
            Xfm::Translation { offset } => nth.set_offset(*offset),
            Xfm::Linear(l) => {
                nth.set_center(l.center);
                nth.set_matrix(l.matrix);
                nth.set_translation(l.translation);
            }
            Xfm::Identity | Xfm::Field(_) | Xfm::BSpline(_) => return None,
        }
        total.compose_pre(&nth);
    }
    Some(total)
}

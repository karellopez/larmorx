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
    /// `Similarity2DTransform`: the scale, the angle, then the translation.
    Similarity2D,
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
            Kind::Similarity2D => "Similarity2DTransform",
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
            Kind::Euler2D => 3,
            Kind::Similarity2D => 4,
            Kind::Other { parameters, .. } => parameters.len(),
        }
    }
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
    /// `Similarity2DTransform`'s scale.
    pub scale: R,
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

    /// `GetInverse` into a transform of the same parameterisation where ITK keeps the class
    /// (`AffineTransform`, `MatrixOffsetTransformBase`): `M⁻¹` by vnl's SVD in `R`,
    /// `offset = −(M⁻¹·offset)`, `ComputeTranslation`; the centre is kept. Other classes
    /// invert to a `MatrixOffsetTransformBase` (ITK's `InvertTransform` with the base type).
    /// `None` for a singular matrix.
    pub fn inverse(&self) -> Option<Self> {
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
        out.center = self.center;
        out.matrix = inv;
        out.offset = std::array::from_fn(|i| -mv[i]);
        out.compute_translation();
        Some(out)
    }

    /// `SetCenter`.
    pub fn set_center(&mut self, c: [R; D]) {
        self.center = c;
        self.compute_offset();
    }

    /// `SetTranslation`.
    pub fn set_translation(&mut self, t: [R; D]) {
        self.translation = t;
        self.compute_offset();
    }

    /// `ComputeMatrix` of the parameterised classes.
    fn compute_matrix(&mut self) {
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
            Kind::Euler2D => {
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
            Kind::Euler2D => {
                let mut p = vec![self.angles[0]];
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Similarity2D => {
                let mut p = vec![self.scale, self.angles[0]];
                p.extend_from_slice(&self.translation);
                p
            }
            Kind::Other { parameters, .. } => parameters.iter().map(|&v| R::from_f64(v)).collect(),
        }
    }

    /// `SetParameters`.
    pub fn set_parameters(&mut self, p: &[R]) {
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
            Kind::Euler2D => {
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
            Kind::Other { .. } => {}
        }
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
            _ => self.center.iter().map(|v| v.to_f64()).collect(),
        }
    }

    /// `UpdateTransformParameters` (`Transform`'s: `GetParameters`, `+= update·factor`,
    /// `SetParameters`).
    pub fn update(&mut self, update: &[R], factor: R) -> Result<(), ItkException> {
        let n = self.kind.number_of_parameters(D);
        if update.len() != n {
            return Err(ItkException::new(
                "Modules/Core/Transform/include/itkTransform.hxx",
                76,
                self.kind.class(),
                format!(
                    "Parameter update size, {}, must  be same as transform parameter size, {n}\n",
                    update.len()
                ),
            ));
        }
        let mut p = self.parameters();
        if factor == R::ONE {
            for k in 0..n {
                p[k] = p[k] + update[k];
            }
        } else {
            for k in 0..n {
                p[k] = p[k] + update[k] * factor;
            }
        }
        self.set_parameters(&p);
        Ok(())
    }

    /// `ComputeJacobianWithRespectToParameters`, `D` rows of `n` values.
    pub fn jacobian_parameters(&self, p: &[R; D], out: &mut [R]) {
        out.fill(R::ZERO);
        let n = self.kind.number_of_parameters(D);
        match &self.kind {
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
            Kind::Euler2D => {
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
            Xfm::Identity => return None,
        }
        total.compose_pre(&nth);
    }
    Some(total)
}

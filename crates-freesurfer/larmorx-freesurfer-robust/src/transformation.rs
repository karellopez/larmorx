// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The robust registration's transformation models (queue 3.2-03), translated from FreeSurfer
//! 8.2.0's `mri_robust_register/Transformation.h`: fifteen models, 2-D and 3-D, each with its
//! parameters, its 4 x 4 matrix ([`Transformation::matrix`]), the image gradient times the
//! model's derivative for the robust step's system ([`Transformation::gradient`]) and Powell's
//! initial steps ([`Transformation::steps`]). The registration's own models (`setTransformation`:
//! translation, rigid, isotropic scale, affine, identity) and Powell's (the `*2` models: angles,
//! scales and shears, whose gradient FreeSurfer never implemented) are [`Kind`].
//!
//! The gradients are FreeSurfer's expressions: the image derivatives are `float`, the voxel
//! positions `unsigned int`, so most products are single precision (`fx * x` converts `x` to
//! `float`), stored as `double`; the 2-D affine model multiplies in `double`.
//!
//! **FS-1063:** `setParameters` from a `vnl_vector<double>` of another length than the model's
//! (the registration step in `double` with an intensity scale, `--doubleprec --iscale`, passes one
//! more) resizes FreeSurfer's parameter vector after the copy, which leaves it unwritten (memory
//! contents); from a shorter `std::vector<double>` or `vnl_vector<float>` FreeSurfer reads past it.
//! Both modes take the vector's first entries (the shorter vector's missing ones 0), noted.

use std::io::Write;

use larmorx_core::defects::FS_1063;
use larmorx_core::vnl_matrix::VnlMatrix;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add, add64, mul, mul64, sub};

use crate::mymatrix::{Mat4, get_vnl_matrix};
use crate::quaternion::Quaternion;
use crate::real::Real;

/// The fifteen models, in `Transformation.h`'s order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Affine2d,
    Affine2d2,
    Isoscale2d,
    Isoscale2d2,
    Rigid2d,
    Rigid2d2,
    Translate2d,
    Affine3d,
    Affine3d2,
    Isoscale3d,
    Isoscale3d2,
    Rigid3d,
    Rigid3d2,
    Translate3d,
    Identity3d,
}

impl Kind {
    /// All models, by number (the probes' order).
    pub const ALL: [Kind; 15] = [
        Kind::Affine2d,
        Kind::Affine2d2,
        Kind::Isoscale2d,
        Kind::Isoscale2d2,
        Kind::Rigid2d,
        Kind::Rigid2d2,
        Kind::Translate2d,
        Kind::Affine3d,
        Kind::Affine3d2,
        Kind::Isoscale3d,
        Kind::Isoscale3d2,
        Kind::Rigid3d,
        Kind::Rigid3d2,
        Kind::Translate3d,
        Kind::Identity3d,
    ];

    /// `getDOF()`.
    pub fn dof(self) -> usize {
        match self {
            Kind::Affine2d | Kind::Affine2d2 => 6,
            Kind::Isoscale2d | Kind::Isoscale2d2 => 4,
            Kind::Rigid2d | Kind::Rigid2d2 => 3,
            Kind::Translate2d => 2,
            Kind::Affine3d | Kind::Affine3d2 => 12,
            Kind::Isoscale3d | Kind::Isoscale3d2 => 7,
            Kind::Rigid3d | Kind::Rigid3d2 => 6,
            Kind::Translate3d => 3,
            Kind::Identity3d => 0,
        }
    }

    /// The message FreeSurfer prints (then `exit(1)`) where a model's gradient (or, for the 3-D
    /// isotropic scale, its matrix too) is not implemented.
    fn not_implemented(self) -> Option<&'static str> {
        match self {
            Kind::Affine2d2 => Some("Gradient for affine 2d (second type) not implemented!"),
            Kind::Isoscale2d2 => Some("Gradient for isoscale 2d (second type) not implemented!"),
            Kind::Rigid2d2 => Some("Gradient for rigid 2d (second type) not implemented!"),
            Kind::Affine3d2 => Some(" Affine in 3D (type 2): gradient not implemented!"),
            Kind::Isoscale3d => Some(" Isoscale in 3D not implemented yet, use ridig or affine"),
            Kind::Isoscale3d2 => Some(" Isoscale in 3D (type 2): gradient not implemented!"),
            Kind::Rigid3d2 => Some("ERROR rigid in 3D (type 2): gradient not implemented !"),
            _ => None,
        }
    }
}

/// A transformation model and its parameters (`Transformation*`).
#[derive(Clone, Debug, PartialEq)]
pub struct Transformation {
    pub kind: Kind,
    /// `getParameters()`.
    pub parameters: Vec<f64>,
}

fn identity4() -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// `MyMatrix::getVNLMatrix(q.getRotMatrix3d(), 3)`.
fn rot(q: &Quaternion) -> VnlMatrix {
    get_vnl_matrix(&q.rot_matrix3d(), 3)
}

impl Transformation {
    /// The model's default constructor: its parameters sized and set to the identity (the 3-D
    /// identity model has none).
    pub fn new(kind: Kind) -> Transformation {
        let mut t = Transformation {
            kind,
            parameters: vec![0.0; kind.dof()],
        };
        t.set_identity();
        t
    }

    /// `getDOF()`.
    pub fn dof(&self) -> usize {
        self.kind.dof()
    }

    /// `setIdentity()`: the parameters 0, the scales 1 (the `*2` models' and the 3-D isotropic
    /// scale's).
    pub fn set_identity(&mut self) {
        let p = &mut self.parameters;
        p.fill(0.0);
        match self.kind {
            Kind::Affine2d2 => {
                p[3] = 1.0;
                p[4] = 1.0;
            }
            Kind::Isoscale2d2 => p[3] = 1.0,
            Kind::Affine3d2 => {
                p[6] = 1.0;
                p[7] = 1.0;
                p[8] = 1.0;
            }
            Kind::Isoscale3d | Kind::Isoscale3d2 => p[6] = 1.0,
            _ => {}
        }
    }

    /// `setParameters(v)` from a `std::vector<double>` or a `vnl_vector<float>` (`exact`: the
    /// first `getDOF()` entries), or from a `vnl_vector<double>` (`exact == false`: FreeSurfer
    /// copies the vector, then resizes it to `getDOF()`, which leaves it unwritten when the
    /// lengths differ). FS-1063 for a vector of another length than FreeSurfer reads correctly.
    pub fn set_parameters<T: Real>(&mut self, g: &mut Globals, v: &[T], exact: bool) {
        let dof = self.dof();
        if v.len() < dof || (!exact && v.len() != dof) {
            g.notices.add(
                &FS_1063,
                format!(
                    "a transformation of {dof} parameters set from a vector of {}: its first entries taken (FreeSurfer's are memory contents)",
                    v.len()
                ),
            );
        }
        for i in 0..dof {
            self.parameters[i] = v.get(i).map_or(0.0, |x| x.wide());
        }
    }

    /// `getSteps()`: Powell's initial steps.
    pub fn steps(&self) -> Vec<f64> {
        let mut v = vec![0.001; self.dof()];
        match self.kind {
            Kind::Affine2d => {
                v[2] = 0.02;
                v[5] = 0.02;
            }
            Kind::Affine2d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
                v[3] = 0.01;
                v[4] = 0.01;
            }
            Kind::Isoscale2d | Kind::Rigid2d | Kind::Rigid2d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
            }
            Kind::Isoscale2d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
                v[3] = 0.01;
            }
            Kind::Translate2d | Kind::Translate3d => v.fill(0.02),
            Kind::Affine3d => {
                v[3] = 0.02;
                v[7] = 0.02;
                v[11] = 0.02;
            }
            Kind::Affine3d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
                v[2] = 0.02;
                v[6] = 0.01;
                v[7] = 0.01;
                v[8] = 0.01;
            }
            Kind::Isoscale3d | Kind::Isoscale3d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
                v[2] = 0.02;
                v[6] = 0.01;
            }
            Kind::Rigid3d | Kind::Rigid3d2 => {
                v[0] = 0.02;
                v[1] = 0.02;
                v[2] = 0.02;
            }
            Kind::Identity3d => {}
        }
        v
    }

    /// `getMatrix()`: the 4 x 4 matrix of the parameters. The 3-D isotropic scale model (not
    /// implemented in FreeSurfer) prints ` Isoscale in 3D not implemented yet, use ridig or
    /// affine` on standard error and ends the program (status 1).
    pub fn matrix(&self, g: &mut Globals, err: &mut dyn Write) -> Result<Mat4, Fatal> {
        let p = &self.parameters;
        let mut ret = identity4();
        match self.kind {
            Kind::Affine2d => {
                ret[0][0] = add64(ret[0][0], p[0]);
                ret[0][1] = p[1];
                ret[0][3] = p[2];
                ret[1][0] = p[3];
                ret[1][1] = add64(ret[1][1], p[4]);
                ret[1][3] = p[5];
            }
            Kind::Affine2d2 => {
                let q = Quaternion::import_zyx_angles(g, p[2].negate(), 0.0, 0.0);
                let rmat = rot(&q);
                let mut smat = VnlMatrix::zeros(3, 3);
                *smat.at_mut(0, 0) = p[3];
                *smat.at_mut(1, 1) = p[4];
                *smat.at_mut(2, 2) = 1.0;
                let mut zmat = VnlMatrix::identity(3);
                *zmat.at_mut(0, 1) = p[5];
                let m3 = zmat.mul(&smat).mul(&rmat);
                let t = [p[0], p[1], 0.0];
                copy3(&mut ret, &m3, &t);
            }
            Kind::Isoscale2d => {
                ret[0][0] = add64(ret[0][0], p[2]);
                ret[0][1] = p[3].negate();
                ret[0][3] = p[0];
                ret[1][0] = p[3];
                ret[1][1] = add64(ret[1][1], p[2]);
                ret[1][3] = p[1];
            }
            Kind::Isoscale2d2 => {
                let q = Quaternion::import_zyx_angles(g, p[2].negate(), 0.0, 0.0);
                let rmat = rot(&q);
                ret[0][0] = mul64(p[3], rmat.at(0, 0));
                ret[0][1] = mul64(p[3], rmat.at(0, 1));
                ret[1][0] = mul64(p[3], rmat.at(1, 0));
                ret[1][1] = mul64(p[3], rmat.at(1, 1));
                ret[0][3] = p[0];
                ret[1][3] = p[1];
            }
            Kind::Rigid2d => {
                let q = Quaternion::import_rot_vec(g, 0.0, 0.0, p[2]);
                let rmat = rot(&q);
                copy3(&mut ret, &rmat, &[0.0; 3]);
                ret[0][3] = p[0];
                ret[1][3] = p[1];
            }
            Kind::Rigid2d2 => {
                let q = Quaternion::import_zyx_angles(g, p[2].negate(), 0.0, 0.0);
                let rmat = rot(&q);
                copy3(&mut ret, &rmat, &[p[0], p[1], 0.0]);
            }
            Kind::Translate2d => {
                ret[0][3] = p[0];
                ret[1][3] = p[1];
            }
            Kind::Affine3d => {
                let mut count = 0;
                for row in ret.iter_mut().take(3) {
                    for x in row.iter_mut() {
                        *x = add64(*x, p[count]);
                        count += 1;
                    }
                }
            }
            Kind::Affine3d2 => {
                let q = Quaternion::import_zyx_angles(g, p[5].negate(), p[4], p[3].negate());
                let rmat = rot(&q);
                let mut smat = VnlMatrix::zeros(3, 3);
                *smat.at_mut(0, 0) = p[6];
                *smat.at_mut(1, 1) = p[7];
                *smat.at_mut(2, 2) = p[8];
                let mut zmat = VnlMatrix::identity(3);
                *zmat.at_mut(0, 1) = p[9];
                *zmat.at_mut(0, 2) = p[10];
                *zmat.at_mut(1, 2) = p[11];
                let m3 = zmat.mul(&smat).mul(&rmat);
                copy3(&mut ret, &m3, &[p[0], p[1], p[2]]);
            }
            Kind::Isoscale3d => {
                let _ = writeln!(
                    err,
                    "{}",
                    Kind::Isoscale3d.not_implemented().expect("a message")
                );
                return Err(Fatal::Exit(1));
            }
            Kind::Isoscale3d2 => {
                let q = Quaternion::import_zyx_angles(g, p[5].negate(), p[4], p[3].negate());
                let mut rmat = rot(&q);
                for x in rmat.data.iter_mut() {
                    *x = mul64(*x, p[6]);
                }
                block3(&mut ret, &rmat);
                ret[0][3] = p[0];
                ret[1][3] = p[1];
                ret[2][3] = p[2];
            }
            Kind::Rigid3d | Kind::Rigid3d2 => {
                let q = if self.kind == Kind::Rigid3d {
                    Quaternion::import_rot_vec(g, p[3], p[4], p[5])
                } else {
                    Quaternion::import_zyx_angles(g, p[5].negate(), p[4], p[3].negate())
                };
                let rmat = rot(&q);
                block3(&mut ret, &rmat);
                ret[0][3] = p[0];
                ret[1][3] = p[1];
                ret[2][3] = p[2];
            }
            Kind::Translate3d => {
                ret[0][3] = p[0];
                ret[1][3] = p[1];
                ret[2][3] = p[2];
            }
            Kind::Identity3d => {}
        }
        Ok(ret)
    }

    /// `getGradient(x, fx, y, fy, z, fz)`: the row of the robust step's system for the voxel at
    /// `(x, y, z)` with image derivatives `(fx, fy, fz)` (FreeSurfer's `float` expressions). The
    /// `*2` models and the 3-D isotropic scale print FreeSurfer's message on standard error and
    /// end the program (status 1).
    #[allow(clippy::too_many_arguments)]
    pub fn gradient(
        &self,
        err: &mut dyn Write,
        x: u32,
        fx: f32,
        y: u32,
        fy: f32,
        z: u32,
        fz: f32,
    ) -> Result<Vec<f64>, Fatal> {
        let mut g = [0.0f64; 12];
        let n = self.gradient_into(err, x, fx, y, fy, z, fz, &mut g)?;
        Ok(g[..n].to_vec())
    }

    /// [`Transformation::gradient`] into `out` (the model's degrees of freedom: their count).
    #[allow(clippy::too_many_arguments)]
    pub fn gradient_into(
        &self,
        err: &mut dyn Write,
        x: u32,
        fx: f32,
        y: u32,
        fy: f32,
        z: u32,
        fz: f32,
        out: &mut [f64; 12],
    ) -> Result<usize, Fatal> {
        if let Some(m) = self.kind.not_implemented() {
            let _ = writeln!(err, "{m}");
            return Err(Fatal::Exit(1));
        }
        let (xf, yf, zf) = (x as f32, y as f32, z as f32);
        let w = |v: f32| v.wide();
        let mut put = |vals: &[f64]| {
            out[..vals.len()].copy_from_slice(vals);
            vals.len()
        };
        Ok(match self.kind {
            Kind::Affine2d => {
                let (xd, yd) = (f64::from(x), f64::from(y));
                put(&[
                    mul64(xd, w(fx)),
                    mul64(yd, w(fx)),
                    w(fx),
                    mul64(xd, w(fy)),
                    mul64(yd, w(fy)),
                    w(fy),
                ])
            }
            Kind::Isoscale2d => put(&[
                w(fx),
                w(fy),
                w(add(mul(fx, xf), mul(fy, yf))),
                // `-fx * y + fy * x` compiled as `fy * x - fx * y` (exact)
                w(sub(mul(fy, xf), mul(fx, yf))),
            ]),
            Kind::Rigid2d => put(&[w(fx), w(fy), w(sub(mul(fy, xf), mul(fx, yf)))]),
            Kind::Translate2d => put(&[w(fx), w(fy)]),
            Kind::Affine3d => put(&[
                w(mul(fx, xf)),
                w(mul(fx, yf)),
                w(mul(fx, zf)),
                w(fx),
                w(mul(fy, xf)),
                w(mul(fy, yf)),
                w(mul(fy, zf)),
                w(fy),
                w(mul(fz, xf)),
                w(mul(fz, yf)),
                w(mul(fz, zf)),
                w(fz),
            ]),
            Kind::Rigid3d => put(&[
                w(fx),
                w(fy),
                w(fz),
                w(sub(mul(fz, yf), mul(fy, zf))),
                w(sub(mul(fx, zf), mul(fz, xf))),
                w(sub(mul(fy, xf), mul(fx, yf))),
            ]),
            Kind::Translate3d => put(&[w(fx), w(fy), w(fz)]),
            Kind::Identity3d => 0,
            _ => unreachable!("the models without a gradient end above"),
        })
    }
}

/// The upper 3 x 3 block of `ret` from `m` (`ret[rr][cc] = m[rr][cc]`), the rest of `ret` kept.
fn block3(ret: &mut Mat4, m: &VnlMatrix) {
    for (rr, row) in ret.iter_mut().enumerate().take(3) {
        for (cc, x) in row.iter_mut().enumerate().take(3) {
            *x = m.at(rr, cc);
        }
    }
}

/// `ret` from a 3 x 3 block and a translation, the last row `(0, 0, 0, 1)`.
fn copy3(ret: &mut Mat4, m: &VnlMatrix, t: &[f64; 3]) {
    for (rr, &tr) in t.iter().enumerate() {
        for (cc, x) in ret[rr].iter_mut().enumerate().take(3) {
            *x = m.at(rr, cc);
        }
        ret[rr][3] = tr;
        ret[3][rr] = 0.0;
    }
    ret[3][3] = 1.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// Every model's identity is the identity matrix (but the 3-D isotropic scale's, which
    /// FreeSurfer never implemented).
    #[test]
    fn identities() {
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let mut e = Vec::new();
        for kind in Kind::ALL {
            let t = Transformation::new(kind);
            assert_eq!(t.parameters.len(), kind.dof());
            assert_eq!(t.steps().len(), kind.dof());
            match t.matrix(&mut g, &mut e) {
                Ok(m) => assert_eq!(m, identity4(), "{kind:?}"),
                Err(_) => assert_eq!(kind, Kind::Isoscale3d),
            }
        }
    }

    #[test]
    fn a_rigid_quarter_turn_and_its_rows() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let mut t = Transformation::new(Kind::Rigid3d);
        let half_pi = std::f64::consts::FRAC_PI_2;
        t.set_parameters(&mut g, &[1.0f64, 2.0, 3.0, 0.0, 0.0, half_pi], false);
        let m = t.matrix(&mut g, &mut e).unwrap();
        assert!((m[0][1] + 1.0).abs() < 1e-15 && (m[1][0] - 1.0).abs() < 1e-15);
        assert_eq!([m[0][3], m[1][3], m[2][3]], [1.0, 2.0, 3.0]);
        let r = t.gradient(&mut e, 2, 1.0, 3, 0.5, 4, 0.25).unwrap();
        assert_eq!(r, vec![1.0, 0.5, 0.25, -1.25, 3.5, -2.0]);
        // a vector of another length (FS-1063): its first parameters
        t.set_parameters(&mut g, &[0.5f64; 7], false);
        assert_eq!(t.parameters, vec![0.5; 6]);
        assert_eq!(g.notices.as_slice()[0].defect.id, "FS-1063");
    }
}

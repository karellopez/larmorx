// SPDX-License-Identifier: Apache-2.0
//! Rotations and affine parameters (spec §64): `MRIangles2RotMat`, `TranformAffineParams2Matrix`
//! and `TranformExtractAffineParams` (FreeSurfer's spelling), `LTAmat2RotMat`, `RMSregDiffMJ`,
//! `Trns_ExtractScaleMatrix` and `Trns_ExtractRotationMatrix` (`mri_cc`'s rotation).

use std::f64::consts::PI;

use larmorx_core::math;

use super::ltachain::LtaInto;
use super::sample::FLT_EPSILON;
use crate::maths::elements::Region as MRegion;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::maths::{Output, Returned};
use crate::mri::Lta;
use crate::process::{Process, Stop};
use crate::sse;

/// The 4 x 4 identity.
fn identity4() -> Matrix {
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for k in 1..=4 {
        m.set(k, k, 1.0);
    }
    m
}

/// A NaN quieted (its sign and payload kept).
fn quiet64(x: f64) -> f64 {
    f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
}

impl Process {
    /// The C library's `cos` or `sin` of a double (§64): a NaN comes back quieted, an infinity
    /// gives SSE's default NaN; else glibc's with `reproduce`, correctly rounded by default.
    fn rot_trig(&mut self, sine: bool, x: f64) -> f64 {
        if x.is_nan() {
            return quiet64(x);
        }
        if sine {
            self.c_trig("sin", x, math::sin, math::sin_glibc)
        } else {
            self.c_trig("cos", x, math::cos, math::cos_glibc)
        }
    }

    /// `atan2f(y, x)` (§64): glibc's with `reproduce`, correctly rounded by default (GLIBC-07
    /// where they differ); a NaN argument gives `x + y`.
    fn rot_atan2f(&mut self, y: f32, x: f32) -> f32 {
        if y.is_nan() || x.is_nan() {
            return sse::add32(x, y);
        }
        let cr = sse::narrow(math::atan2(f64::from(y), f64::from(x)));
        let g = math::atan2f_glibc(y, x);
        if cr.to_bits() != g.to_bits() {
            self.note_glibc07("atan2f");
        }
        if self.mode().reproduce() { g } else { cr }
    }

    /// `atan2(y, x)` of doubles (§64): correctly rounded in both modes; a NaN gives `x + y`.
    fn rot_atan2(y: f64, x: f64) -> f64 {
        if y.is_nan() || x.is_nan() {
            return sse::add64(x, y);
        }
        math::atan2(y, x)
    }

    /// `MRIangles2RotMat(angles)` (§64): `(Rz Ry) Rx` (`float` products) of the rotations about
    /// x by `angles[0]`, y by `angles[1]` and z by `angles[2]` (radians; the cosines and sines in
    /// `double`, stored as `float`), in a 4 x 4 matrix of zeros with 1 at (4, 4).
    pub fn mri_angles2_rot_mat(&mut self, angles: [f64; 3]) -> Result<Matrix, Stop> {
        let mut cs = [(0.0f32, 0.0f32); 3];
        for (k, a) in angles.into_iter().enumerate() {
            let c = sse::narrow(self.rot_trig(false, a));
            let s = sse::narrow(self.rot_trig(true, a));
            cs[k] = (c, s);
        }
        let [(cg, sg), (cb, sb), (ca, sa)] = cs;
        let m3 = |v: [f32; 9]| {
            let mut m = Matrix::zeros(3, 3, MATRIX_REAL);
            for (i, x) in v.into_iter().enumerate() {
                m.set(i as i32 / 3 + 1, i as i32 % 3 + 1, x);
            }
            m
        };
        let rx = m3([1.0, 0.0, 0.0, 0.0, cg, -sg, 0.0, sg, cg]);
        let ry = m3([cb, 0.0, sb, 0.0, 1.0, 0.0, -sb, 0.0, cb]);
        let rz = m3([ca, -sa, 0.0, sa, ca, 0.0, 0.0, 0.0, 1.0]);
        let zy = match self.matrix_multiply(Some(&rz), Some(&ry), None)? {
            Returned::New(m) => m,
            _ => Matrix::zeros(3, 3, MATRIX_REAL),
        };
        let r = match self.matrix_multiply(Some(&zy), Some(&rx), None)? {
            Returned::New(m) => m,
            _ => Matrix::zeros(3, 3, MATRIX_REAL),
        };
        let mut out = Matrix::zeros(4, 4, MATRIX_REAL);
        for i in 1..=3 {
            for j in 1..=3 {
                out.set(i, j, r.at(i, j));
            }
        }
        out.set(4, 4, 1.0);
        Ok(out)
    }

    /// `TranformAffineParams2Matrix(p, M)` (§64): `((T R) Z) S` with `R = (R1 R2) R3` by the
    /// double-accumulating product (`MatrixMultiplyD`); the first product into `M` when given
    /// (another size: its messages and nothing, then the next product's end of the program).
    pub fn tranform_affine_params2_matrix(
        &mut self,
        p: &[f64; 12],
        m: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let mut t = identity4();
        for (k, &v) in p.iter().enumerate().take(3) {
            t.set(k as i32 + 1, 4, sse::narrow(v));
        }
        let mut cs = [(0.0f32, 0.0f32); 3];
        for (k, v) in cs.iter_mut().enumerate() {
            let a = sse::div64(sse::mul64(p[3 + k], PI), 180.0);
            let c = sse::narrow(self.rot_trig(false, a));
            let s = sse::narrow(self.rot_trig(true, a));
            *v = (c, s);
        }
        let [(c3, s3), (c4, s4), (c5, s5)] = cs;
        let mut r1 = identity4();
        r1.set(2, 2, c3);
        r1.set(2, 3, s3);
        r1.set(3, 2, -s3);
        r1.set(3, 3, c3);
        let mut r2 = identity4();
        r2.set(1, 1, c4);
        r2.set(1, 3, s4);
        r2.set(3, 1, -s4);
        r2.set(3, 3, c4);
        let mut r3 = identity4();
        r3.set(1, 1, c5);
        r3.set(1, 2, s5);
        r3.set(2, 1, -s5);
        r3.set(2, 2, c5);
        let mut z = identity4();
        for k in 0..3 {
            z.set(k as i32 + 1, k as i32 + 1, sse::narrow(p[6 + k]));
        }
        let mut s = identity4();
        s.set(1, 2, sse::narrow(p[9]));
        s.set(1, 3, sse::narrow(p[10]));
        s.set(2, 3, sse::narrow(p[11]));
        let r12 = self.new_product_d(Some(&r1), Some(&r2))?;
        let r = self.new_product_d(r12.as_ref(), Some(&r3))?;
        // the first product into the given matrix
        match m {
            Some(m) => {
                let first = self.matrix_multiply_d(Some(&t), r.as_ref(), Some(m))?;
                let ok = !first.is_nothing();
                let cur = if ok { Some(m.clone()) } else { None };
                let tz = self.matrix_multiply_d(cur.as_ref(), Some(&z), Some(m))?;
                if tz.is_nothing() {
                    return Ok(Returned::Nothing);
                }
                let cur = m.clone();
                let ts = self.matrix_multiply_d(Some(&cur), Some(&s), Some(m))?;
                Ok(if ts.is_nothing() {
                    Returned::Nothing
                } else {
                    Returned::Output
                })
            }
            None => {
                let tr = self.new_product_d(Some(&t), r.as_ref())?;
                let Some(mut acc) = tr else {
                    return Ok(Returned::Nothing);
                };
                for f in [&z, &s] {
                    let cur = acc.clone();
                    if self
                        .matrix_multiply_d(Some(&cur), Some(f), Some(&mut acc))?
                        .is_nothing()
                    {
                        return Ok(Returned::Nothing);
                    }
                }
                Ok(Returned::New(acc))
            }
        }
    }

    /// `MatrixMultiplyD(a, b, NULL)`: the new product, or none.
    fn new_product_d(
        &mut self,
        a: Option<&Matrix>,
        b: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        Ok(match self.matrix_multiply_d(a, b, None)? {
            Returned::New(m) => Some(m),
            _ => None,
        })
    }

    /// `MatrixMultiply(a, b, NULL)`: the new product, or none.
    fn new_product(
        &mut self,
        a: Option<&Matrix>,
        b: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        Ok(match self.matrix_multiply(a, b, None)? {
            Returned::New(m) => Some(m),
            _ => None,
        })
    }

    /// `TranformExtractAffineParams(M, p)` (§64): the translation, the three angles (degrees)
    /// of `Q` and the scales and shears of `R` from the QR decomposition with the signs of `R`'s
    /// diagonal moved into `Q` (the first negated for a negative determinant). `false` (`NULL`)
    /// without a matrix, after its line, `p` untouched.
    pub fn tranform_extract_affine_params(
        &mut self,
        m: Option<&Matrix>,
        p: &mut [f64; 12],
    ) -> Result<bool, Stop> {
        let Some(m) = m else {
            self.put_out(b"ERROR: TranformExtractAffineParams() input matrix is NULL\n");
            return Ok(false);
        };
        for (k, v) in p.iter_mut().enumerate().take(3) {
            *v = sse::widen(m.at(k as i32 + 1, 4));
        }
        let mut q = Matrix::zeros(4, 4, MATRIX_REAL);
        let mut r = Matrix::zeros(4, 4, MATRIX_REAL);
        self.matrix_qr_decomposition(Some(m), Some(&mut q), Some(&mut r));
        let det = self.matrix_determinant(m);
        let mut pm = Matrix::zeros(4, 4, MATRIX_REAL);
        for k in 1..=4 {
            pm.set(k, k, if r.at(k, k) < 0.0 { -1.0 } else { 1.0 });
        }
        if det < 0.0 {
            pm.set(1, 1, -pm.at(1, 1));
        }
        let q = self
            .new_product(Some(&q), Some(&pm))?
            .unwrap_or_else(|| Matrix::zeros(4, 4, MATRIX_REAL));
        let r = self
            .new_product(Some(&pm), Some(&r))?
            .unwrap_or_else(|| Matrix::zeros(4, 4, MATRIX_REAL));
        let deg = |a: f32| sse::div64(sse::widen(sse::mul32(a, 180.0)), PI);
        let a3 = self.rot_atan2f(q.at(2, 3), q.at(3, 3));
        p[3] = deg(a3);
        let (q23, q33) = (sse::widen(q.at(2, 3)), sse::widen(q.at(3, 3)));
        let h = sse::sqrt64(sse::add64(self.libm_pow(q23, 2.0), self.libm_pow(q33, 2.0)));
        let a4 = Self::rot_atan2(sse::widen(q.at(1, 3)), h);
        p[4] = sse::div64(sse::mul64(a4, 180.0), PI);
        let a5 = self.rot_atan2f(q.at(1, 2), q.at(1, 1));
        p[5] = deg(a5);
        for k in 0..3 {
            p[6 + k] = sse::widen(r.at(k as i32 + 1, k as i32 + 1));
        }
        p[9] = sse::widen(sse::div32(r.at(1, 2), r.at(1, 1)));
        p[10] = sse::widen(sse::div32(r.at(1, 3), r.at(1, 1)));
        p[11] = sse::widen(sse::div32(r.at(2, 3), r.at(2, 2)));
        Ok(true)
    }

    /// `LTAmat2RotMat(lta)` (§64): the array inverted in place, its first matrix replaced by the
    /// rotation of its affine parameters (translation 0, scales 1, shears 0), inverted again (the
    /// parameters extracted once more, unused). Returns 0. No first transform, or one without a
    /// matrix: FS-504.
    pub fn lta_mat2_rot_mat(&mut self, lta: &mut Lta) -> Result<i32, Stop> {
        self.lta_invert(lta, LtaInto::Itself)?;
        if lta.xforms.first().and_then(|t| t.m_l.as_ref()).is_none() {
            self.lta_fs504(
                "LTAmat2RotMat",
                "an array without a first transform's matrix",
            )?;
            return Ok(0);
        }
        let mut p = [0.0f64; 12];
        let m = lta.xforms[0].m_l.clone();
        self.tranform_extract_affine_params(m.as_ref(), &mut p)?;
        for k in 0..3 {
            p[k] = 0.0;
            p[6 + k] = 1.0;
            p[9 + k] = 0.0;
        }
        let rot = match self.tranform_affine_params2_matrix(&p, None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        let dst = lta.xforms[0]
            .m_l
            .as_mut()
            .map(Output::Given)
            .unwrap_or(Output::New);
        self.matrix_copy(rot.as_ref(), dst)?;
        self.lta_invert(lta, LtaInto::Itself)?;
        let m = lta.xforms.first().and_then(|t| t.m_l.clone());
        self.tranform_extract_affine_params(m.as_ref(), &mut p)?;
        Ok(0)
    }

    /// `RMSregDiffMJ(T1, T2, radius)` (§64): `sqrt(((r r) tr) / 5 + tt)` in `double`, `tr` the
    /// trace of `Mᵀ M` and `tt` the 1 x 1 `tᵀ t` of `T1 - T2`'s upper-left 3 x 3 `M` and first
    /// three rows of its fourth column `t` (double-accumulating products).
    pub fn rms_reg_diff_mj(&mut self, t1: &Matrix, t2: &Matrix, radius: f64) -> Result<f64, Stop> {
        let q = match self.matrix_add_sub(t1, t2, None, true)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(4, 4, MATRIX_REAL),
        };
        let m = match self.matrix_copy_region(MRegion::Whole, &q, None, 1, 1, 3, 3, 1, 1)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(3, 3, MATRIX_REAL),
        };
        let t = match self.matrix_copy_region(MRegion::Whole, &q, None, 1, 4, 3, 1, 1, 1)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(3, 1, MATRIX_REAL),
        };
        let mt = match self.matrix_transpose(&m, None)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(3, 3, MATRIX_REAL),
        };
        let mtm = self
            .new_product_d(Some(&mt), Some(&m))?
            .unwrap_or_else(|| Matrix::zeros(3, 3, MATRIX_REAL));
        let tr = self.matrix_trace(&mtm);
        let tt_ = match self.matrix_transpose(&t, None)? {
            Returned::New(x) => x,
            _ => Matrix::zeros(1, 3, MATRIX_REAL),
        };
        let tt = self
            .new_product_d(Some(&tt_), Some(&t))?
            .unwrap_or_else(|| Matrix::zeros(1, 1, MATRIX_REAL));
        let a = sse::div64(sse::mul64(sse::mul64(radius, radius), sse::widen(tr)), 5.0);
        Ok(sse::sqrt64(sse::add64(a, sse::widen(tt.at(1, 1)))))
    }
}

/// `Trns_tErr_NoErr`.
pub const TRNS_ERR_NO_ERR: i32 = 0;
/// `Trns_tErr_InvalidParameter`.
pub const TRNS_ERR_INVALID_PARAMETER: i32 = 3;

impl Process {
    /// `T0`: a copy of the transform with its fourth column's first three elements 0.
    fn trns_t0(t: &Matrix) -> Matrix {
        let mut t0 = t.clone();
        for r in 1..=3 {
            t0.set(r, 4, 0.0);
        }
        t0
    }

    /// The three scale factors of `T0`: the `float` lengths of `T0 e_k` (§64).
    fn trns_scales(&mut self, t0: &Matrix) -> Result<[f32; 3], Stop> {
        let mut f = [0.0f32; 3];
        for (k, fk) in f.iter_mut().enumerate() {
            let mut e = Matrix::zeros(4, 1, MATRIX_REAL);
            e.set(k as i32 + 1, 1, 1.0);
            let input = e.clone();
            self.matrix_multiply(Some(t0), Some(&input), Some(&mut e))?;
            *fk = self.vector_len(&e);
        }
        Ok(f)
    }

    /// The 4 x 4 identity with `d` on the first three diagonal places, into `m`.
    fn trns_diag(m: &mut Matrix, d: [f32; 3]) {
        for r in 1..=4 {
            for c in 1..=4 {
                m.set(r, c, if r == c { 1.0 } else { 0.0 });
            }
        }
        for (k, &v) in d.iter().enumerate() {
            m.set(k as i32 + 1, k as i32 + 1, v);
        }
    }

    /// `Trns_ExtractScaleMatrix(T, S)` (§64): `S` the identity with the lengths of the
    /// transform's first three columns on its diagonal; 3 without either matrix.
    pub fn trns_extract_scale_matrix(
        &mut self,
        t: Option<&Matrix>,
        s: Option<&mut Matrix>,
    ) -> Result<i32, Stop> {
        let (Some(t), Some(s)) = (t, s) else {
            return Ok(TRNS_ERR_INVALID_PARAMETER);
        };
        let t0 = Self::trns_t0(t);
        let f = self.trns_scales(&t0)?;
        Self::trns_diag(s, f);
        Ok(TRNS_ERR_NO_ERR)
    }

    /// `Trns_ExtractRotationMatrix(T, R)` (§64): `R = T0 x` the inverse scales (1 where a scale
    /// is below `FLT_EPSILON` in magnitude); 3 without either matrix.
    pub fn trns_extract_rotation_matrix(
        &mut self,
        t: Option<&Matrix>,
        r: Option<&mut Matrix>,
    ) -> Result<i32, Stop> {
        let (Some(t), Some(r)) = (t, r) else {
            return Ok(TRNS_ERR_INVALID_PARAMETER);
        };
        let t0 = Self::trns_t0(t);
        let f = self.trns_scales(&t0)?;
        let inv = f.map(|x| {
            if x.abs() < FLT_EPSILON as f32 {
                1.0
            } else {
                sse::narrow(sse::div64(1.0, sse::widen(x)))
            }
        });
        let mut si = Matrix::zeros(4, 4, MATRIX_REAL);
        Self::trns_diag(&mut si, inv);
        self.matrix_multiply(Some(&t0), Some(&si), Some(r))?;
        Ok(TRNS_ERR_NO_ERR)
    }
}

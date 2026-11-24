// SPDX-License-Identifier: Apache-2.0
//! Rotations and affine parameters (spec §64): `Trns_ExtractScaleMatrix` and
//! `Trns_ExtractRotationMatrix` (`mri_cc`'s rotation).

use super::sample::FLT_EPSILON;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop};
use crate::sse;

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

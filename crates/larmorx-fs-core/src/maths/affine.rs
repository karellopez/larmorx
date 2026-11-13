// SPDX-License-Identifier: Apache-2.0
//! The 4 x 4 affine helpers (spec §3): affine vectors of 4 floats and matrices of 16 floats
//! column by column, their conversions from and to float matrices, and their products.

use super::matrix::Matrix;
use crate::process::{Process, Stop};
use crate::sse;

/// An affine vector (x, y, z, w).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AffineVector(pub [f32; 4]);

/// An affine matrix: float `i + 4j` is row i, column j (from 0). A new one is all zeros.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AffineMatrix(pub [f32; 16]);

impl AffineVector {
    /// `SetAffineVector(v, x, y, z)`: (x, y, z, 1).
    pub fn new(x: f32, y: f32, z: f32) -> AffineVector {
        AffineVector([x, y, z, 1.0])
    }

    /// `GetAffineVector`: its first three floats.
    pub fn get(&self) -> (f32, f32, f32) {
        (self.0[0], self.0[1], self.0[2])
    }
}

impl Process {
    /// `GetFloorAffineVector`: `(int) floor(v)` of each of the first three floats (FS-17).
    pub fn affine_vector_floor(&mut self, v: &AffineVector) -> (i32, i32, i32) {
        let f = |p: &mut Process, x: f32| p.to_int(f64::from(x.floor()));
        (f(self, v.0[0]), f(self, v.0[1]), f(self, v.0[2]))
    }

    /// `SetAffineMatrix(A, m)`: float `i + 4j` = the number at row-major position `j + 4i`; a
    /// matrix that is not 4 x 4 aborts the program (status 134) after its line.
    pub fn set_affine_matrix(&mut self, m: &Matrix) -> Result<AffineMatrix, Stop> {
        if m.rows != 4 || m.cols != 4 {
            self.put_err(b"SetAffineMatrix: Bad matrix size\n");
            self.flush();
            return Err(Stop::Exit(134));
        }
        let mut a = [0.0f32; 16];
        for i in 0..4 {
            for j in 0..4 {
                a[i + 4 * j] = m.number(j + 4 * i);
            }
        }
        Ok(AffineMatrix(a))
    }

    /// `GetAffineMatrix(m, A)`: the reverse of [`Process::set_affine_matrix`].
    pub fn get_affine_matrix(&mut self, m: &mut Matrix, a: &AffineMatrix) -> Result<(), Stop> {
        if m.rows != 4 || m.cols != 4 {
            self.put_err(b"GetAffineMatrix: Bad matrix size\n");
            self.flush();
            return Err(Stop::Exit(134));
        }
        for i in 0..4 {
            for j in 0..4 {
                let place = Some(2 + (j + 4 * i) as i64);
                m.write(place, a.0[i + 4 * j]);
            }
        }
        Ok(())
    }
}

/// `AffineMV(w, A, v)`: element i = `((x0 A[i] + x1 A[4 + i]) + x2 A[8 + i]) + x3 A[12 + i]` in
/// float, from the first product. NaN operands (spec §3): in each product the vector's number is
/// the left operand, in each sum the running sum.
pub fn affine_mv(a: &AffineMatrix, v: &AffineVector) -> AffineVector {
    let (m, x) = (&a.0, &v.0);
    let mut w = [0.0f32; 4];
    for (i, wi) in w.iter_mut().enumerate() {
        let mut s = sse::mul32(x[0], m[i]);
        for k in 1..4 {
            s = sse::add32(s, sse::mul32(x[k], m[4 * k + i]));
        }
        *wi = s;
    }
    AffineVector(w)
}

/// `AffineMM(M, A, B)`: element (i, j) the float sum, from the first product, of
/// `B[k + 4j] × A[i + 4k]` for k = 0..3 (spec §3: the second matrix's number the left operand of
/// each product, the running sum the left operand of each sum).
pub fn affine_mm(a: &AffineMatrix, b: &AffineMatrix) -> AffineMatrix {
    let mut m = [0.0f32; 16];
    for j in 0..4 {
        for i in 0..4 {
            let p = |k: usize| sse::mul32(b.0[k + 4 * j], a.0[i + 4 * k]);
            let mut s = p(0);
            for k in 1..4 {
                s = sse::add32(s, p(k));
            }
            m[i + 4 * j] = s;
        }
    }
    AffineMatrix(m)
}

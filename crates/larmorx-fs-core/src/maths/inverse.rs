// SPDX-License-Identifier: Apache-2.0
//! The inverse, determinant and QR factors of float matrices (spec §8.8), on ITK 4.13.2's VNL in
//! float (larmorx-core's ports `vnl_fixed` and `vnl_qr`), FS-33.

use larmorx_core::defects::FS_33;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};

use super::Returned;
use super::matrix::Matrix;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// A matrix's first `rows × cols` numbers, row by row (§8.8: VNL is handed the numbers as they
/// lie in the array, whatever the type).
pub(crate) fn numbers_rm(m: &Matrix) -> Vec<f32> {
    let n = (m.rows.max(0) as usize) * (m.cols.max(0) as usize);
    (0..n).map(|i| m.number(i)).collect()
}

/// Writes `v` into a matrix's numbers from the first (beyond the array: dropped).
pub(crate) fn write_numbers(m: &mut Matrix, v: &[f32]) {
    for (i, &x) in v.iter().enumerate() {
        m.write(Some(2 + i as i64), x);
    }
}

impl Process {
    /// `MatrixDeterminant(m)` (§8.8; FS-33 for 0 x 0).
    pub fn matrix_determinant(&mut self, m: &Matrix) -> f32 {
        if m.rows != m.cols {
            return 0.0;
        }
        let n = m.rows.max(0) as usize;
        if n == 0 {
            if self.mode().reproduce() {
                self.note(
                    &FS_33,
                    "the determinant of a 0 x 0 matrix read from memory never allocated (0)",
                );
                return 0.0;
            }
            self.note(
                &FS_33,
                "the determinant of a 0 x 0 matrix is the empty product, 1",
            );
            return 1.0;
        }
        vnl_fixed::determinant::<f32>(&numbers_rm(m), n)
    }

    /// `MatrixQRdecomposition(m, Q, R)` (§8.8): 0, or 1 after its line.
    pub fn matrix_qr_decomposition(
        &mut self,
        m: Option<&Matrix>,
        q: Option<&mut Matrix>,
        r: Option<&mut Matrix>,
    ) -> i32 {
        let fail = |p: &mut Process, what: &str| {
            p.put_out(format!("QRdecomposition(): {what}\n").as_bytes());
            1
        };
        let Some(m) = m else {
            return fail(self, "input matrix is NULL");
        };
        let Some(q) = q else {
            return fail(self, "output Q matrix is NULL");
        };
        let Some(r) = r else {
            return fail(self, "output R matrix is NULL");
        };
        if q.rows != m.rows {
            return fail(self, "Q rows != input Matrix rows");
        }
        if q.cols != q.rows {
            return fail(self, "Q cols != Q rows");
        }
        if r.rows != m.rows {
            return fail(self, "R rows != input Matrix rows");
        }
        if r.cols != m.cols {
            return fail(self, "R cols != input Matrix cols");
        }
        let (rows, cols) = (m.rows.max(0) as usize, m.cols.max(0) as usize);
        if cols == 0 || rows == 0 {
            for i in 1..=m.rows {
                for j in 1..=m.rows {
                    q.set(i, j, if i == j { 1.0 } else { 0.0 });
                }
            }
            return 0;
        }
        let f = Qr::<f32>::new(&numbers_rm(m), rows, cols);
        let qq = f.q();
        let rr = f.r();
        for i in 0..rows {
            for j in 0..rows {
                q.set(i as i32 + 1, j as i32 + 1, qq[i * rows + j]);
            }
            for j in 0..cols {
                r.set(i as i32 + 1, j as i32 + 1, rr[i * cols + j]);
            }
        }
        0
    }

    /// The inverse of a real `n × n` row-major array as `MatrixInverse` computes it, `None` where
    /// it returns nothing (a zero determinant).
    fn real_inverse(&mut self, a: &[f32], n: usize) -> Option<Vec<f32>> {
        match n {
            0 => Some(Vec::new()),
            1 => {
                if a[0] == 0.0 {
                    return None;
                }
                Some(vec![sse::narrow(sse::div64(1.0, sse::widen(a[0])))])
            }
            2..=4 => {
                if vnl_fixed::determinant::<f32>(a, n) == 0.0 {
                    return None;
                }
                Some(vnl_fixed::inverse::<f32>(a, n))
            }
            _ => {
                let f = Qr::<f32>::new(a, n, n);
                if f.determinant() == 0.0 {
                    return None;
                }
                let (inv, infos) = f.inverse();
                for k in infos {
                    if k != 0 {
                        self.put_err(format!("{RANK_DEFICIENT}{k}\n").as_bytes());
                    }
                }
                Some(inv)
            }
        }
    }

    /// `MatrixInverse(m, out)` (§8.8; FS-28 for an output of another size or type, FS-33).
    pub fn matrix_inverse(
        &mut self,
        m: Option<&Matrix>,
        out: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let Some(m) = m else {
            self.error_exit(codes::BAD_PARM, b"MatrixInverse: NULL input matrix!\n")?;
            return Ok(Returned::Nothing);
        };
        if m.rows != m.cols {
            let msg = format!(
                "MatrixInverse: matrix ({} x {}) is not square\n",
                m.rows, m.cols
            );
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Ok(Returned::Nothing);
        }
        let n = m.rows.max(0) as usize;
        if n == 0 {
            self.note(&FS_33, "the inverse of a 0 x 0 matrix is the empty matrix (FreeSurfer writes past its output)");
        }
        let src = m.clone();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.matrix_alloc(m.rows, m.cols, m.kind)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                let ok = o.same_size(m) && o.kind == m.kind;
                if !self.fits(
                    "MatrixInverse",
                    "the output is not of the matrix's size and type",
                    ok,
                ) {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        let finite;
        if src.is_complex() {
            // [[A, -C], [C, A]]
            let k = 2 * n;
            let mut big = vec![0.0f32; k * k];
            for r in 0..n {
                for c in 0..n {
                    let (a, ci) = src.pair(r as i32 + 1, c as i32 + 1);
                    big[r * k + c] = a;
                    big[r * k + c + n] = -ci;
                    big[(r + n) * k + c] = ci;
                    big[(r + n) * k + c + n] = a;
                }
            }
            let Some(inv) = self.real_inverse(&big, k) else {
                self.note(
                    &FS_33,
                    "a singular complex matrix: nothing returned (FreeSurfer crashes)",
                );
                return Ok(Returned::Nothing);
            };
            let mut ok = true;
            for r in 0..n {
                for c in 0..n {
                    let (re, im) = (inv[r * k + c], inv[(r + n) * k + c]);
                    ok &= re.is_finite() && im.is_finite();
                    o.set_pair(r as i32 + 1, c as i32 + 1, re, im);
                }
            }
            finite = ok;
        } else {
            let Some(inv) = self.real_inverse(&numbers_rm(&src), n) else {
                return Ok(Returned::Nothing);
            };
            let ok = inv.iter().all(|v| v.is_finite());
            write_numbers(o, &inv);
            finite = ok;
        }
        if !finite {
            return Ok(Returned::Nothing);
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }
}

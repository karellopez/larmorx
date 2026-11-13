// SPDX-License-Identifier: Apache-2.0
//! The singular value decomposition and what uses it (spec §17.4, §17.5), on ITK 4.13.2's
//! `vnl_svd<float>` (larmorx-core's `vnl_svd_f32`): FS-40, FS-43 to FS-46, FS-48.

use larmorx_core::defects::{FS_40, FS_43, FS_44, FS_45, FS_46, FS_48};
use larmorx_core::vnl_svd_f32;

use super::Returned;
use super::inverse::{numbers_rm, write_numbers};
use super::matrix::{MATRIX_REAL, Matrix};
use crate::clib::printf::sprintf_f64;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The official build's path of VNL's SVD source, which its messages name.
const SVD_HXX: &str = "/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_svd.hxx";

/// A decomposition: `U` (rows × cols), the singular values, `V` (cols × cols), row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Svd {
    pub u: Vec<f32>,
    pub w: Vec<f32>,
    pub v: Vec<f32>,
}

fn new_or(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

impl Process {
    /// `vnl_svd<float>` of a matrix's numbers (§17.4): `None` for fewer rows than columns, or
    /// after VNL's failure lines (LINPACK not converging; FS-48: any non-finite entry).
    pub fn svd_core(&mut self, m: &Matrix) -> Option<Svd> {
        let (rows, cols) = (m.rows.max(0) as usize, m.cols.max(0) as usize);
        if rows < cols || cols == 0 {
            return None;
        }
        let a = numbers_rm(m);
        let info = if a.iter().any(|x| !x.is_finite()) {
            self.note(
                &FS_48,
                "an SVD LINPACK may never return on (a non-finite entry) refused",
            );
            Some((rows + 1).min(cols))
        } else {
            let s = vnl_svd_f32::vnl_svd_f32_full(&a, rows, cols);
            if s.info == 0 {
                let u = (0..rows * cols)
                    .map(|k| s.u[(k % cols) * rows + k / cols])
                    .collect();
                let v = (0..cols * cols)
                    .map(|k| s.v[(k % cols) * cols + k / cols])
                    .collect();
                return Some(Svd {
                    u,
                    w: s.w[..cols].to_vec(),
                    v,
                });
            }
            Some(s.info)
        };
        let info = info.expect("a failure");
        let mut t = format!(
            "{SVD_HXX}: suspicious return value ({info}) from SVDC\n{SVD_HXX}: M is {rows}x{cols}\nM = [ ...\n"
        );
        for r in 0..rows {
            for c in 0..cols {
                let x = a[r * cols + c];
                if x == 0.0 {
                    t.push_str("       0 ");
                } else {
                    t.push_str(&String::from_utf8_lossy(&sprintf_f64(
                        b"%8.5f ",
                        sse::widen(x),
                    )));
                }
            }
            if r + 1 == rows {
                t.push_str(" ]");
            }
            t.push('\n');
        }
        self.put_err(t.as_bytes());
        None
    }

    /// `MatrixSVD(A, w, V)` (§17.5; FS-43): U into A's numbers, the values into w's, V's numbers
    /// (a new V when none); returns V.
    pub fn matrix_svd(
        &mut self,
        a: &mut Matrix,
        w: &mut Matrix,
        v: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixSVD", a) {
            return Ok(Returned::Nothing);
        }
        let reproduce = self.mode().reproduce();
        let (rows, cols) = (a.rows, a.cols);
        let mut fresh = None;
        let vm: &mut Matrix = match v {
            None => {
                let n = if reproduce { rows } else { cols };
                match self.matrix_alloc(n, n, MATRIX_REAL)? {
                    Some(x) => fresh.insert(x),
                    None => return Ok(Returned::Nothing),
                }
            }
            Some(v) => {
                if !reproduce && (v.rows != cols || v.cols != cols) {
                    if !self.fits("MatrixSVD", "V is not cols x cols", false) {
                        return Ok(Returned::Nothing);
                    }
                }
                v
            }
        };
        if rows != cols {
            self.note(
                &FS_43,
                if reproduce {
                    "V made rows x rows"
                } else {
                    "V made cols x cols"
                },
            );
        }
        match self.svd_core(a) {
            Some(s) => {
                write_numbers(a, &s.u);
                write_numbers(w, &s.w);
                write_numbers(vm, &s.v);
            }
            None => {
                if reproduce {
                    self.note(&FS_43, "a failed decomposition ignored (V unchanged)");
                } else {
                    self.note(&FS_43, "a failed decomposition reported");
                    let msg = format!(
                        "MatrixSVD: the decomposition failed (a {rows} x {cols} matrix: fewer rows than columns, or one LINPACK cannot decompose)\n"
                    );
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                    return Ok(Returned::Nothing);
                }
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// The singular values as `MatrixSVDEigenValues` reads them (FS-40: `rows` of them with
    /// `reproduce`, the ones past the decomposition's 0), sorted by decreasing magnitude, and
    /// the condition number.
    fn svd_values_cond(&mut self, m: &Matrix, s: Option<&Svd>) -> (Vec<f32>, f32) {
        let reproduce = self.mode().reproduce();
        let cols = m.cols.max(0) as usize;
        let n = if reproduce {
            m.rows.max(0) as usize
        } else {
            cols
        };
        if m.rows > m.cols {
            self.note(
                &FS_40,
                if reproduce {
                    "singular values read past the decomposition's (0)"
                } else {
                    "only the decomposition's singular values read"
                },
            );
        }
        let raw: Vec<f32> = (0..n)
            .map(|i| s.and_then(|s| s.w.get(i).copied()).unwrap_or(0.0))
            .collect();
        let order = super::eigen::sort_by_magnitude_pub(&raw);
        let sorted: Vec<f32> = order.iter().map(|&i| raw[i]).collect();
        let (mut lo, mut hi) = (
            sorted.first().copied().unwrap_or(0.0),
            sorted.first().copied().unwrap_or(0.0),
        );
        for &x in sorted.iter().skip(1) {
            let a = x.abs();
            if a < lo {
                lo = a;
            }
            if a > hi {
                hi = a;
            }
        }
        let cond = if lo < f32::EPSILON {
            1e8
        } else {
            sse::div32(hi, lo)
        };
        (sorted, cond)
    }

    /// `MatrixSVDEigenValues(m, values)` (§17.5): the condition number; the values written into
    /// `values` (its first `rows` cleared first).
    pub fn matrix_svd_eigen_values(&mut self, m: &Matrix, values: &mut [f32]) -> f32 {
        for v in values.iter_mut().take(m.rows.max(0) as usize) {
            *v = 0.0;
        }
        let Some(s) = self.svd_core(m) else {
            return self.gerror as f32;
        };
        let (sorted, cond) = self.svd_values_cond(m, Some(&s));
        for (v, x) in values.iter_mut().zip(sorted) {
            *v = x;
        }
        cond
    }

    /// `MatrixConditionNumber(m)` (§17.5).
    pub fn matrix_condition_number(&mut self, m: &Matrix) -> f32 {
        if !self.real_only("MatrixConditionNumber", m) {
            return 1e8;
        }
        if self.matrix_is_zero(m) == 1 {
            return 1e10;
        }
        let s = self.svd_core(m);
        self.svd_values_cond(m, s.as_ref()).1
    }

    /// `MatrixNSConditionNumber(m)`: of `mᵀm` (more rows) or `mmᵀ` (fewer).
    pub fn matrix_ns_condition_number(&mut self, m: &Matrix) -> Result<f32, Stop> {
        if m.rows == m.cols {
            return Ok(self.matrix_condition_number(m));
        }
        let Some(t) = new_or(self.matrix_transpose(m, None)?) else {
            return Ok(1e8);
        };
        let p = if m.rows > m.cols {
            self.matrix_multiply(Some(&t), Some(m), None)?
        } else {
            self.matrix_multiply(Some(m), Some(&t), None)?
        };
        match new_or(p) {
            Some(p) => Ok(self.matrix_condition_number(&p)),
            None => Ok(1e8),
        }
    }

    /// `V (W⁻¹ Uᵀ)` of a decomposition, the values below `(float)(1e-4 w_max)` inverted to 0.
    fn svd_inverse_of(
        &mut self,
        s: &Svd,
        rows: i32,
        cols: i32,
        out: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let mut wmax = 0.0f32;
        for &x in &s.w {
            if x > wmax {
                wmax = x;
            }
        }
        let thr = sse::narrow(sse::mul64(1e-4, sse::widen(wmax)));
        let mut winv = Matrix::zeros(cols, cols, MATRIX_REAL);
        for (i, &x) in s.w.iter().enumerate() {
            winv.set(
                i as i32 + 1,
                i as i32 + 1,
                if x < thr { 0.0 } else { sse::div32(1.0, x) },
            );
        }
        let mut ut = Matrix::zeros(cols, rows, MATRIX_REAL);
        let mut vm = Matrix::zeros(cols, cols, MATRIX_REAL);
        for r in 0..rows as usize {
            for c in 0..cols as usize {
                ut.set(c as i32 + 1, r as i32 + 1, s.u[r * cols as usize + c]);
            }
        }
        write_numbers(&mut vm, &s.v);
        let Some(wu) = new_or(self.matrix_multiply(Some(&winv), Some(&ut), None)?) else {
            return Ok(Returned::Nothing);
        };
        self.matrix_multiply(Some(&vm), Some(&wu), out)
    }

    /// `MatrixSVDInverse(m, out)` (§17.5; FS-44).
    pub fn matrix_svd_inverse(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        if self.matrix_is_zero(m) == 1 {
            return Ok(Returned::Nothing);
        }
        let Some(s) = self.svd_core(m) else {
            return Ok(Returned::Nothing);
        };
        if m.rows > m.cols {
            if self.mode().reproduce() {
                self.note(&FS_44, "the SVD inverse of a matrix with more rows than columns: nothing (FreeSurfer crashes)");
                return Ok(Returned::Nothing);
            }
            self.note(
                &FS_44,
                "the SVD inverse of a matrix with more rows than columns: its pseudo-inverse",
            );
        }
        self.svd_inverse_of(&s, m.rows, m.cols, out)
    }

    /// `MatrixPseudoInverse(m, out)` (§17.5; FS-45).
    pub fn matrix_pseudo_inverse(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        if m.rows < m.cols {
            let Some(t) = new_or(self.matrix_transpose(m, None)?) else {
                return Ok(Returned::Nothing);
            };
            if self.mode().reproduce() {
                self.note(
                    &FS_45,
                    "the caller's output passed to the inner pseudo-inverse",
                );
                return match out {
                    None => match new_or(self.matrix_pseudo_inverse(&t, None)?) {
                        Some(inner) => self.matrix_transpose(&inner, None),
                        None => Ok(Returned::Nothing),
                    },
                    Some(o) => {
                        let r = self.matrix_pseudo_inverse(&t, Some(o))?;
                        match r {
                            Returned::Output => {
                                let inner = o.clone();
                                self.matrix_transpose(&inner, None)
                            }
                            Returned::New(inner) => self.matrix_transpose(&inner, None),
                            Returned::Nothing => Ok(Returned::Nothing),
                        }
                    }
                };
            }
            self.note(
                &FS_45,
                "the inner pseudo-inverse made new, its transpose into the output",
            );
            let Some(inner) = new_or(self.matrix_pseudo_inverse(&t, None)?) else {
                return Ok(Returned::Nothing);
            };
            return match out {
                None => self.matrix_transpose(&inner, None),
                Some(o) => self.matrix_transpose(&inner, Some(o)),
            };
        }
        let Some(t) = new_or(self.matrix_transpose(m, None)?) else {
            return Ok(Returned::Nothing);
        };
        let Some(mtm) = new_or(self.matrix_multiply(Some(&t), Some(m), None)?) else {
            return Ok(Returned::Nothing);
        };
        let inv = match new_or(self.matrix_inverse(Some(&mtm), None)?) {
            Some(i) => i,
            None => match new_or(self.matrix_svd_inverse(&mtm, None)?) {
                Some(i) => i,
                None => return Ok(Returned::Nothing),
            },
        };
        self.matrix_multiply(Some(&inv), Some(&t), out)
    }

    /// `MatrixSVDPseudoInverse(m, out)` (§17.5; FS-46).
    pub fn matrix_svd_pseudo_inverse(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let reproduce = self.mode().reproduce();
        if m.rows < m.cols {
            let Some(t) = new_or(self.matrix_transpose(m, None)?) else {
                return Ok(Returned::Nothing);
            };
            let Some(inner) = new_or(self.matrix_svd_pseudo_inverse(&t, None)?) else {
                return Ok(Returned::Nothing);
            };
            return if reproduce {
                self.matrix_transpose(&inner, None)
            } else {
                self.matrix_transpose(&inner, out)
            };
        }
        if self.matrix_is_zero(m) == 1 {
            return Ok(Returned::Nothing);
        }
        let (rows, cols) = (m.rows.max(0) as usize, m.cols.max(0) as usize);
        let s = match self.svd_core(m) {
            Some(s) => s,
            None => {
                if !reproduce {
                    self.note(&FS_46, "a failed decomposition reported");
                    let msg = format!(
                        "MatrixSVDPseudoInverse: the decomposition failed (a {} x {} matrix LINPACK cannot decompose)\n",
                        m.rows, m.cols
                    );
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                    return Ok(Returned::Nothing);
                }
                self.note(
                    &FS_46,
                    "a failed decomposition ignored (its singular values 0)",
                );
                Svd {
                    u: vec![0.0; rows * cols],
                    w: vec![0.0; cols],
                    v: vec![0.0; cols * cols],
                }
            }
        };
        let mut r = cols;
        for k in 1..cols {
            if f64::from(sse::div32(s.w[k], s.w[0])) < 1e-4 {
                r = k;
                break;
            }
        }
        let (rr, ri) = (r as i32, rows as i32);
        let mut vr = Matrix::zeros(cols as i32, rr, MATRIX_REAL);
        let mut d = Matrix::zeros(rr, rr, MATRIX_REAL);
        let mut ut = Matrix::zeros(rr, ri, MATRIX_REAL);
        for i in 0..r {
            for k in 0..cols {
                vr.set(k as i32 + 1, i as i32 + 1, s.v[k * cols + i]);
            }
            d.set(i as i32 + 1, i as i32 + 1, sse::div32(1.0, s.w[i]));
            for k in 0..rows {
                ut.set(i as i32 + 1, k as i32 + 1, s.u[k * cols + i]);
            }
        }
        let Some(vd) = new_or(self.matrix_multiply(Some(&vr), Some(&d), None)?) else {
            return Ok(Returned::Nothing);
        };
        if reproduce {
            if out.is_some() {
                self.note(
                    &FS_46,
                    "a new matrix returned, the given output not written",
                );
            }
            return self.matrix_multiply(Some(&vd), Some(&ut), None);
        }
        self.matrix_multiply(Some(&vd), Some(&ut), out)
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Eigensystems (spec §17.1 to §17.3) on ITK 4.13.2's VNL (larmorx-core's `vnl_eigen`): the
//! symmetric and non-symmetric eigensystems of float arrays and of matrices, their eigenvalues
//! sorted by magnitude (FS-41, FS-42, FS-47, ITK-110 to ITK-112).

use larmorx_core::defects::{FS_41, FS_42, FS_47, ITK_112};
use larmorx_core::vnl_eigen;

use super::Returned;
use super::inverse::numbers_rm;
use super::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// What an eigensystem wrote: the values and vectors (row by row), `None` where not written.
#[derive(Clone, Debug, PartialEq)]
pub struct Eigen {
    /// 0 (success) or -4 (failure).
    pub status: i32,
    pub values: Option<Vec<f32>>,
    pub vectors: Option<Vec<f32>>,
}

/// A stable merge sort of the indices by decreasing magnitude: an element goes before another
/// unless its magnitude is smaller (§17.3).
fn sort_by_magnitude(v: &[f32]) -> Vec<usize> {
    fn merge_sort(idx: &mut [usize], v: &[f32]) {
        let n = idx.len();
        if n < 2 {
            return;
        }
        let mid = n / 2;
        merge_sort(&mut idx[..mid], v);
        merge_sort(&mut idx[mid..], v);
        let (a, b) = (idx[..mid].to_vec(), idx[mid..].to_vec());
        let (mut i, mut j, mut k) = (0, 0, 0);
        while i < a.len() && j < b.len() {
            if v[a[i]].abs() < v[b[j]].abs() {
                idx[k] = b[j];
                j += 1;
            } else {
                idx[k] = a[i];
                i += 1;
            }
            k += 1;
        }
        while i < a.len() {
            idx[k] = a[i];
            i += 1;
            k += 1;
        }
        while j < b.len() {
            idx[k] = b[j];
            j += 1;
            k += 1;
        }
    }
    let mut idx: Vec<usize> = (0..v.len()).collect();
    merge_sort(&mut idx, v);
    idx
}

/// [`sort_by_magnitude`] for the SVD's values.
pub(crate) fn sort_by_magnitude_pub(v: &[f32]) -> Vec<usize> {
    sort_by_magnitude(v)
}

impl Process {
    /// `OpenEigenSystem(a, n, values, vectors)` (§17.1; FS-42, FS-47).
    pub fn open_eigen_system(&mut self, a: &[f32], n: usize) -> Eigen {
        if n == 0 {
            return Eigen {
                status: 0,
                values: None,
                vectors: None,
            };
        }
        // the entries the tridiagonalisation reads: the upper triangle as stored
        let nonfinite = (0..n).any(|r| (r..n).any(|c| !a[r * n + c].is_finite()));
        let e = vnl_eigen::symmetric_eigensystem_f32_full(&a[..n * n], n);
        if n >= 2 && nonfinite {
            // FS-47: where VNL never returns, refused in both modes (FreeSurfer hangs); where it
            // returns, FreeSurfer's result with `reproduce`, refused by default.
            let never = e.never_returns();
            if never || self.mode().fix() {
                self.note(
                    &FS_47,
                    if never {
                        "a symmetric eigensystem VNL never returns on (an infinite or NaN entry) refused"
                    } else {
                        "a symmetric eigensystem of a matrix with an infinite or NaN entry refused"
                    },
                );
                self.error_printf(
                    codes::BAD_PARM,
                    b"OpenEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or NaN entry)\n",
                );
                return Eigen {
                    status: codes::BAD_PARM,
                    values: None,
                    vectors: None,
                };
            }
            self.note(
                &FS_47,
                "a symmetric eigensystem of a matrix with an infinite or NaN entry: VNL's result",
            );
        }
        if e.ierr != 0 {
            self.put_err(format!("vnl_symmetric_eigensystem: ierr = {}\n", e.ierr).as_bytes());
            if self.mode().reproduce() {
                self.note(
                    &FS_42,
                    "the eigensystem did not converge: success reported, the vectors memory (0)",
                );
                return Eigen {
                    status: 0,
                    values: Some(e.values),
                    vectors: Some(vec![0.0; n * n]),
                };
            }
            self.note(
                &FS_42,
                "the eigensystem did not converge: a failure, the vectors not written",
            );
            let msg = format!(
                "OpenEigenSystem: the eigensystem did not converge (ierr {})\n",
                e.ierr
            );
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Eigen {
                status: codes::BAD_PARM,
                values: Some(e.values),
                vectors: None,
            };
        }
        Eigen {
            status: 0,
            values: Some(e.values),
            vectors: Some(e.vectors),
        }
    }

    /// `OpenNonSymmetricEigenSystem(a, n, values, vectors)` (§17.2; ITK-110 to ITK-112, FS-41).
    pub fn open_non_symmetric_eigen_system(&mut self, a: &[f32], n: usize) -> Eigen {
        if n == 0 {
            return Eigen {
                status: 0,
                values: None,
                vectors: None,
            };
        }
        if n * n * 8 > 8 << 20 {
            self.note(&FS_41, "a non-symmetric eigensystem of more than 8 MiB computed (FreeSurfer's stack copy crashes)");
        }
        let ad: Vec<f64> = a[..n * n].iter().map(|&x| sse::widen(x)).collect();
        match vnl_eigen::real_eigensystem(&ad, n) {
            Err(_) => {
                self.note(&ITK_112, "a non-symmetric eigensystem VNL never returns on (an infinite or NaN entry) refused");
                self.error_printf(
                    codes::BAD_PARM,
                    b"OpenNonSymmetricEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or NaN entry)\n",
                );
                Eigen {
                    status: codes::BAD_PARM,
                    values: None,
                    vectors: None,
                }
            }
            Ok(e) => {
                if e.ierr != 0 {
                    self.put_err(
                        vnl_eigen::real_eigensystem_failure_message(&ad, n, e.ierr).as_bytes(),
                    );
                }
                for nt in &e.notices {
                    self.note(nt.defect, nt.detail.clone());
                }
                let values = e.d_re.iter().map(|&v| sse::narrow(v)).collect();
                let vectors = e.vreal.iter().map(|&v| sse::narrow(v)).collect();
                Eigen {
                    status: 0,
                    values: Some(values),
                    vectors: Some(vectors),
                }
            }
        }
    }

    /// `MatrixEigenSystem(m, values, vectors)` (§17.3): the eigensystem of a symmetric (exactly)
    /// or non-symmetric matrix, the values sorted by decreasing magnitude.
    pub fn matrix_eigen_system(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
    ) -> Result<(Returned<Matrix>, Option<Vec<f32>>), Stop> {
        let sym = m.rows == m.cols && {
            let mut s = true;
            for r in 1..=m.rows {
                for c in 1..=m.cols {
                    s &= m.at(r, c) == m.at(c, r);
                }
            }
            s
        };
        self.matrix_eigen_helper(m, out, sym)
    }

    /// `MatrixCalculateEigenSystemHelper(m, values, vectors, isSymmetric)` (§17.3).
    pub fn matrix_eigen_helper(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
        symmetric: bool,
    ) -> Result<(Returned<Matrix>, Option<Vec<f32>>), Stop> {
        if m.rows != m.cols {
            return Ok((Returned::Nothing, None));
        }
        if !self.real_only("MatrixEigenSystem", m) {
            return Ok((Returned::Nothing, None));
        }
        let n = m.rows.max(0) as usize;
        let a = numbers_rm(m);
        let e = if symmetric {
            self.open_eigen_system(&a, n)
        } else {
            self.open_non_symmetric_eigen_system(&a, n)
        };
        if e.status != 0 {
            return Ok((Returned::Nothing, None));
        }
        let vals = e.values.unwrap_or_default();
        let vecs = e.vectors.unwrap_or_else(|| vec![0.0; n * n]);
        let order = sort_by_magnitude(&vals);
        let sorted: Vec<f32> = order.iter().map(|&i| vals[i]).collect();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.matrix_alloc(n as i32, n as i32, MATRIX_REAL)? {
                Some(x) => fresh.insert(x),
                None => return Ok((Returned::Nothing, Some(sorted))),
            },
            Some(o) => {
                let ok = o.rows >= n as i32 && o.cols >= n as i32;
                if !self.fits(
                    "MatrixEigenSystem",
                    "the vectors' matrix is smaller than n x n",
                    ok,
                ) {
                    return Ok((Returned::Nothing, Some(sorted)));
                }
                o
            }
        };
        for (c, &src) in order.iter().enumerate() {
            for r in 0..n {
                o.set(r as i32 + 1, c as i32 + 1, vecs[r * n + src]);
            }
        }
        let r = match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        };
        Ok((r, Some(sorted)))
    }
}

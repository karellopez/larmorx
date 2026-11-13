// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's numerics wrappers over VXL (`fs.maths`, queue items 3.1-23 and 3.1-24),
//! translated from FreeSurfer 8.2.0's `utils/numerics.cpp`: `OpenLUMatrixInverse` (1091-1153),
//! `OpenQRdecomposition` (1155-1196), `OpenMatrixDeterminant` (1198-1219). The VXL routines they
//! call are the main package's ports of ITK 4.13.2's VNL in `float` (`larmorx_core::vnl_fixed`:
//! `vnl_det`, `vnl_inverse`, `vnl_determinant`; `larmorx_core::vnl_qr::Qr<f32>`: `vnl_qr<float>`),
//! queue item 3.1-07; this crate contains no VXL code.
//!
//! `numerics.cpp`'s GSL functions (`sc_*`) are queue item 3.1-53 (`larmorx-gpl`) and never come
//! here; Burkardt's spline (`SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`) is item 3.1-54.
//!
//! Defect (`larmorx_core::defects`; `docs/findings/freesurfer-maths.md`): **FS-33**, the 0 x 0
//! matrix (see [`open_lu_matrix_inverse`] and [`open_matrix_determinant`]).

use std::io::Write;

use larmorx_core::defects::FS_33;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};

use crate::globals::Globals;
use crate::log::{ERROR_BADPARM, NO_ERROR};
use crate::matrix::Matrix;

/// `vnl_matrix<float>(m->data, rows, cols)`: the first `rows * cols` floats of the numbers, row by
/// row (a real matrix's elements).
fn vnl_of(m: &Matrix) -> Vec<f32> {
    let n = m.rows.max(0) as usize * m.cols.max(0) as usize;
    m.data()[..n.min(m.data().len())].to_vec()
}

/// `copy_out(o->data)`: `v` into the first floats of `o`'s numbers (those past its array are
/// dropped: the callers check the sizes, FS-28).
fn copy_out(o: &mut Matrix, v: &[f32]) {
    let d = o.data_mut();
    let n = v.len().min(d.len());
    d[..n].copy_from_slice(&v[..n]);
}

/// `OpenLUMatrixInverse(iMatrix, oInverse)`: the inverse of the square `iMatrix` into `oInverse`'s
/// numbers (row by row), in `float`: of one element `1.0 / a` (in `double`); of 2 to 4 rows VNL's
/// closed forms (`vnl_det`, then `vnl_inverse`); of more, `vnl_qr<float>`'s `inverse()` (printing
/// VNL's `rank-deficient` line for each column whose solution met a zero diagonal). Returns
/// `NO_ERROR`, or `ERROR_BADPARM` (nothing written) when the element or the determinant is 0.
/// **FS-33:** a 0 x 0 matrix takes FreeSurfer's 4 x 4 branch on memory VNL never allocated and
/// writes 16 floats past `oInverse`; both modes write nothing and return `NO_ERROR` (the result
/// FreeSurfer gives unless that memory makes a zero determinant), FS-33 noted.
pub fn open_lu_matrix_inverse(
    g: &mut Globals,
    err: &mut dyn Write,
    i_matrix: &Matrix,
    o_inverse: &mut Matrix,
) -> i32 {
    let r = i_matrix.rows.max(0) as usize;
    let a = vnl_of(i_matrix);
    if r == 0 {
        g.notices.add(
            &FS_33,
            "OpenLUMatrixInverse of a 0 x 0 matrix: no inverse to write (FreeSurfer reads and writes memory it does not own)",
        );
        return NO_ERROR;
    }
    if r <= 4 && i_matrix.cols as usize == r {
        if r == 1 {
            if a[0] == 0.0 {
                return ERROR_BADPARM;
            }
            copy_out(o_inverse, &[(1.0f64 / f64::from(a[0])) as f32]);
            return NO_ERROR;
        }
        if vnl_fixed::determinant::<f32>(&a, r) == 0.0 {
            return ERROR_BADPARM;
        }
        copy_out(o_inverse, &vnl_fixed::inverse::<f32>(&a, r));
        return NO_ERROR;
    }
    let qr = Qr::<f32>::new(&a, r, i_matrix.cols as usize);
    if qr.determinant() == 0.0 {
        return ERROR_BADPARM;
    }
    let (inv, infos) = qr.inverse();
    for info in infos.into_iter().filter(|&i| i > 0) {
        let _ = writeln!(err, "{RANK_DEFICIENT}{info}");
    }
    copy_out(o_inverse, &inv);
    NO_ERROR
}

/// `OpenQRdecomposition(iMatrix, oQ, oR)`: `vnl_qr<float>`'s factors of `iMatrix` (rows x cols):
/// `Q` (rows x rows) into `oQ`'s numbers and `R` (rows x cols) into `oR`'s, returning 0. A missing
/// matrix or a size that does not match prints its line on standard output and returns 1:
/// `QRdecomposition(): input matrix is NULL`, `... output Q matrix is NULL`, `... output R matrix
/// is NULL`, `... Q rows != input Matrix rows`, `... Q cols != Q rows`, `... R rows != input Matrix
/// rows`, `... R cols != input Matrix cols`. A matrix without columns gives the identity `Q`, one
/// without rows nothing (VNL's empty factorisation).
pub fn open_qr_decomposition(
    out: &mut dyn Write,
    i_matrix: Option<&Matrix>,
    o_q: Option<&mut Matrix>,
    o_r: Option<&mut Matrix>,
) -> i32 {
    let Some(m) = i_matrix else {
        let _ = writeln!(out, "QRdecomposition(): input matrix is NULL");
        return 1;
    };
    let Some(q) = o_q else {
        let _ = writeln!(out, "QRdecomposition(): output Q matrix is NULL");
        return 1;
    };
    let Some(r) = o_r else {
        let _ = writeln!(out, "QRdecomposition(): output R matrix is NULL");
        return 1;
    };
    let checks = [
        (q.rows != m.rows, "Q rows != input Matrix rows"),
        (q.cols != q.rows, "Q cols != Q rows"),
        (r.rows != m.rows, "R rows != input Matrix rows"),
        (r.cols != m.cols, "R cols != input Matrix cols"),
    ];
    if let Some((_, what)) = checks.iter().find(|(bad, _)| *bad) {
        let _ = writeln!(out, "QRdecomposition(): {what}");
        return 1;
    }
    let (rows, cols) = (m.rows.max(0) as usize, m.cols.max(0) as usize);
    if rows == 0 {
        return 0;
    }
    if cols == 0 {
        let mut id = vec![0.0f32; rows * rows];
        for i in 0..rows {
            id[i * rows + i] = 1.0;
        }
        copy_out(q, &id);
        return 0;
    }
    let qr = Qr::<f32>::new(&vnl_of(m), rows, cols);
    copy_out(q, &qr.q());
    copy_out(r, &qr.r());
    0
}

/// `OpenMatrixDeterminant(iMatrix)`: 0 for a matrix that is not square; of one element the
/// element; of 2 to 4 rows `vnl_det` of the `float` closed form; of more `vnl_determinant<float>`
/// (the QR determinant). **FS-33:** a 0 x 0 matrix reads the empty QR factor's first element:
/// by default 1 (the empty product), with `reproduce` 0 (memory not owned read as 0), FS-33
/// noted.
pub fn open_matrix_determinant(g: &mut Globals, i_matrix: &Matrix) -> f32 {
    if i_matrix.rows != i_matrix.cols {
        return 0.0;
    }
    let n = i_matrix.rows.max(0) as usize;
    if n == 0 {
        let reproduce = g.notices.reproduce();
        g.notices.add(
            &FS_33,
            if reproduce {
                "OpenMatrixDeterminant of a 0 x 0 matrix: memory not owned read as 0"
            } else {
                "OpenMatrixDeterminant of a 0 x 0 matrix: 1, the empty product"
            },
        );
        return if reproduce { 0.0 } else { 1.0 };
    }
    let a = vnl_of(i_matrix);
    vnl_fixed::determinant::<f32>(&a, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn inverses_and_determinants() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let m = Matrix::from_rows(&[vec![2.0, 0.0], vec![0.0, 4.0]]);
        let mut inv = Matrix::real(2, 2);
        assert_eq!(
            open_lu_matrix_inverse(&mut g, &mut e, &m, &mut inv),
            NO_ERROR
        );
        assert_eq!(inv.data(), &[0.5, 0.0, 0.0, 0.25]);
        let s = Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 4.0]]);
        assert_eq!(
            open_lu_matrix_inverse(&mut g, &mut e, &s, &mut inv),
            ERROR_BADPARM
        );
        assert_eq!(open_matrix_determinant(&mut g, &m), 8.0);
        let big = Matrix::from_rows(
            &(0..5)
                .map(|i| (0..5).map(|j| if i == j { 2.0 } else { 0.0 }).collect())
                .collect::<Vec<_>>(),
        );
        let mut bi = Matrix::real(5, 5);
        assert_eq!(
            open_lu_matrix_inverse(&mut g, &mut e, &big, &mut bi),
            NO_ERROR
        );
        assert_eq!(bi.relt(3, 3), 0.5);
        assert_eq!(open_matrix_determinant(&mut g, &big).abs(), 32.0);
        let empty = Matrix::real(0, 0);
        assert_eq!(open_matrix_determinant(&mut g, &empty), 1.0);
        assert!(g.notices.contains(&FS_33));
        let mut o = Vec::new();
        assert_eq!(open_qr_decomposition(&mut o, Some(&m), None, None), 1);
        assert_eq!(o, b"QRdecomposition(): output Q matrix is NULL\n");
        let (mut q, mut r) = (Matrix::real(2, 2), Matrix::real(2, 2));
        assert_eq!(
            open_qr_decomposition(&mut o, Some(&m), Some(&mut q), Some(&mut r)),
            0
        );
        assert_eq!(r.relt(2, 1), 0.0);
    }
}

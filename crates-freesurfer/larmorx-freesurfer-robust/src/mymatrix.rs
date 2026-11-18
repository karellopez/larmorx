// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MyMatrix` (queue 3.2-02, `fs.robust` 1 of 2): the matrix helpers of Martin Reuter's robust
//! registration that both `mri_robust_register` and `mri_robust_template` reach, translated from
//! FreeSurfer 8.2.0's `mri_robust_register/MyMatrix.cpp`:
//!
//! - the conversions between VNL's `double` matrices and FreeSurfer's `float` `MATRIX`
//!   ([`convert_vnl2matrix`], [`convert_vnl_vec2matrix`], [`convert_matrix2vnl`]) and
//!   [`get_vnl_matrix`] (`getVNLMatrix`);
//! - [`polar_decomposition`] and [`polar2_decomposition`] (`A = R S`, `A = R S D`, from VNL's SVD);
//! - the distances between transforms: [`affine_trans_dist_sq`], [`rigid_trans_dist_sq`] (with
//!   [`rot_matrix_log_norm`]) and [`get_frobenius_diff`];
//! - [`is_diag`].
//!
//! VNL's matrices are [`VnlMatrix`] (`larmorx_core::vnl_matrix`, ITK 4.13.2's loops); its SVD is
//! `larmorx_core::vnl_svd`. The `MATRIX` overloads of the distances and the functions only
//! `mri_robust_template` calls (the matrix logarithm and exponential, the geometric and rotation
//! means, the LTA conversions) come with that program's items; the matrix square root
//! (`MatrixSqrt`, LAPACK's complex Schur decomposition) is listed in the crate's `PROVENANCE.md`.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::FS_968;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_matrix::VnlMatrix;
use larmorx_core::vnl_svd::{matlab_long, vnl_svd_full};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::numerics::vnl_svd_hxx;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};

use crate::mymri::wide;

/// `vnl_matrix_fixed<double, 4, 4>`, row by row.
pub type Mat4 = [[f64; 4]; 4];

/// `MyMatrix::convertVNL2MATRIX(m, NULL)`: a new `float` `MATRIX` of the matrix's size, each
/// element converted to `float`.
pub fn convert_vnl2matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &VnlMatrix,
) -> Result<Matrix, Fatal> {
    let mut o = matrix_alloc(g, out, err, m.rows as i32, m.cols as i32, MATRIX_REAL)?
        .expect("a matrix of the vnl matrix's size");
    for r in 0..m.rows {
        for c in 0..m.cols {
            o.set_relt(r as i32 + 1, c as i32 + 1, m.at(r, c) as f32);
        }
    }
    Ok(o)
}

/// [`convert_vnl2matrix`] of a `vnl_matrix_fixed<double, 4, 4>` (`m.as_matrix()`).
pub fn to_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Mat4,
) -> Result<Matrix, Fatal> {
    convert_vnl2matrix(g, out, err, &VnlMatrix::from_4x4(m))
}

/// `MyMatrix::convertVNL2MATRIX(v, NULL)` of a vector: a new `n x 1` `float` `MATRIX`.
pub fn convert_vnl_vec2matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: &[f64],
) -> Result<Matrix, Fatal> {
    let mut o = matrix_alloc(g, out, err, v.len() as i32, 1, MATRIX_REAL)?
        .expect("a vector of the vnl vector's size");
    for (r, &x) in v.iter().enumerate() {
        o.set_relt(r as i32 + 1, 1, x as f32);
    }
    Ok(o)
}

/// `MyMatrix::convertMATRIX2VNL(m)`: the `MATRIX`'s elements widened to `double`.
pub fn convert_matrix2vnl(m: &Matrix) -> VnlMatrix {
    let (rows, cols) = (m.rows.max(0) as usize, m.cols.max(0) as usize);
    let mut v = VnlMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            *v.at_mut(r, c) = wide(m.relt(r as i32 + 1, c as i32 + 1));
        }
    }
    v
}

/// `MyMatrix::getVNLMatrix(d, r)`: the numbers row by row into `r` rows of `d.len() / r`
/// (integer division) columns. Its callers pass a rotation's nine numbers with `r = 3`; `r` must
/// be positive (FreeSurfer divides by it).
pub fn get_vnl_matrix(d: &[f64], r: usize) -> VnlMatrix {
    let c = d.len() / r;
    let mut m = VnlMatrix::zeros(r, c);
    let mut count = 0;
    for rr in 0..r {
        for cc in 0..c {
            *m.at_mut(rr, cc) = d[count];
            count += 1;
        }
    }
    m
}

/// `MyMatrix::isDiag(A, eps)`: whether the sum of the squares of the elements off the diagonal
/// (`A[i][j]^2` then `A[j][i]^2`, for `i` from 1 and `j` from `i - 1` down to 0) is below `eps`.
pub fn is_diag(a: &VnlMatrix, eps: f64) -> bool {
    let n = a.rows;
    let mut sse = 0.0f64;
    for i in 1..n {
        for j in (0..i).rev() {
            sse = add64(sse, mul64(a.at(i, j), a.at(i, j)));
            sse = add64(sse, mul64(a.at(j, i), a.at(j, i)));
        }
    }
    sse < eps
}

/// What `vnl_svd<double>(A)` keeps: `U`, `V` (row-major) and `W` (`|w|`, zeroed where `|w| <=
/// 0`), whether LINPACK converged (its warning when it did not: the decomposition reached is kept,
/// as VNL keeps it), or `None` where VNL never returns (ITK-116, noted: FreeSurfer hangs).
struct Svd {
    u: VnlMatrix,
    w: Vec<f64>,
    v: VnlMatrix,
    warning: Option<String>,
}

fn svd(g: &mut Globals, a: &VnlMatrix) -> Option<Svd> {
    let (n, p) = (a.rows, a.cols);
    let full = vnl_svd_full(&a.data, n, p);
    if full.never_returns() {
        for x in &full.notices {
            g.notices.add(x.defect, x.detail.clone());
        }
        return None;
    }
    let mut um = VnlMatrix::zeros(n, p);
    for j in 0..p {
        for i in 0..n {
            *um.at_mut(i, j) = full.u[j * n + i];
        }
    }
    let mut vm = VnlMatrix::zeros(p, p);
    for j in 0..p {
        for i in 0..p {
            *vm.at_mut(i, j) = full.v[j * p + i];
        }
    }
    let w = full
        .w
        .iter()
        .map(|&x| if x.abs() <= 0.0 { 0.0 } else { x.abs() })
        .collect();
    let warning = (full.info != 0).then(|| suspicious_svdc(&a.data, n, p, full.info));
    Some(Svd {
        u: um,
        w,
        v: vm,
        warning,
    })
}

/// The error the port gives where VNL's SVD never returns (FreeSurfer hangs: ITK-116).
const SVD_HANGS: &str = "MyMatrix: VNL's singular value decomposition never returns on this matrix (an infinite or NaN entry)";

/// The lines `vnl_svd<double>` prints on standard error when LINPACK's `dsvdc` does not converge
/// (ITK 4.13.2's `vnl_svd.hxx`, the official build's path): `info`, the size, and the matrix by
/// `vnl_matlab_print(std::cerr, M, "M", vnl_matlab_print_format_long)`.
fn suspicious_svdc(m: &[f64], rows: usize, cols: usize, info: usize) -> String {
    let file = vnl_svd_hxx();
    let mut s = format!(
        "{file}: suspicious return value ({info}) from SVDC\n{file}: M is {rows}x{cols}\nM = [ ...\n"
    );
    for i in 0..rows {
        for j in 0..cols {
            s.push_str(&matlab_long(m[i * cols + j]));
        }
        if i + 1 == rows {
            s.push_str(" ]");
        }
        s.push('\n');
    }
    s
}

/// `MyMatrix::PolarDecomposition(A, R, S)`: from `vnl_svd<double>(A)`, `R = U V^T` and `S = V W
/// V^T` (VNL's products). When LINPACK does not converge: VNL's warning, then `MyMatrix
/// PolarDecomposition ERROR: SVD not possible?` on standard error and the end of the program
/// (`exit(1)`). Where VNL never returns (ITK-116: FreeSurfer hangs): an error and `exit(1)`.
pub fn polar_decomposition(
    g: &mut Globals,
    err: &mut dyn Write,
    a: &VnlMatrix,
) -> Result<(VnlMatrix, VnlMatrix), Fatal> {
    let Some(d) = svd(g, a) else {
        let _ = writeln!(err, "{SVD_HANGS}");
        return Err(Fatal::Exit(1));
    };
    if let Some(w) = d.warning {
        let _ = err.write_all(w.as_bytes());
        let _ = writeln!(err, "MyMatrix PolarDecomposition ERROR: SVD not possible?");
        return Err(Fatal::Exit(1));
    }
    let vt = d.v.transpose();
    let r = d.u.mul(&vt);
    let s = d.v.scale_columns(&d.w).mul(&vt);
    Ok((r, s))
}

/// `MyMatrix::Polar2Decomposition(A, R, S, D)`: [`polar_decomposition`], then `D[c] = S[c][c]`
/// and column `c` of `S` divided by it, for each column in order.
pub fn polar2_decomposition(
    g: &mut Globals,
    err: &mut dyn Write,
    a: &VnlMatrix,
) -> Result<(VnlMatrix, VnlMatrix, Vec<f64>), Fatal> {
    let (r, mut s) = polar_decomposition(g, err, a)?;
    let mut d = vec![0.0; a.cols];
    for (c, dc) in d.iter_mut().enumerate() {
        *dc = s.at(c, c);
        for i in 0..s.rows {
            *s.at_mut(i, c) = div64(s.at(i, c), *dc);
        }
    }
    Ok((r, s, d))
}

/// `MyMatrix::AffineTransDistSq(a, b, r)` (VNL): `d = a - b` (`a - I` for an empty `b`); `tdq`
/// the sum of the squares of its translation (`d[i][3]`, `i < 3`, each then zeroed with `d[3][i]`;
/// `d[3][3]` zeroed); `tr` the trace of the upper 3 x 3 block of `d^T d`; `(1/5) r r tr + tdq`.
pub fn affine_trans_dist_sq(a: &VnlMatrix, b: Option<&VnlMatrix>, r: f64) -> f64 {
    let mut d = match b {
        Some(b) if !b.is_empty() => a.sub(b),
        _ => a.sub(&VnlMatrix::identity(a.rows)),
    };
    let mut tdq = 0.0f64;
    for i in 0..3 {
        tdq = add64(tdq, mul64(d.at(i, 3), d.at(i, 3)));
        *d.at_mut(i, 3) = 0.0;
        *d.at_mut(3, i) = 0.0;
    }
    *d.at_mut(3, 3) = 0.0;
    let d = d.transpose().mul(&d);
    let mut tr = 0.0f64;
    for i in 0..3 {
        tr = add64(tr, d.at(i, i));
    }
    add64(mul64(mul64(mul64(1.0 / 5.0, r), r), tr), tdq)
}

/// `MyMatrix::RigidTransDistSq(a, b)` (VNL): `d = b a^-1` (`vnl_matrix_inverse`, the SVD's
/// pseudo-inverse; `a` itself for an empty `b`); `tdq` the sum of the squares of its translation;
/// `rd` = [`rot_matrix_log_norm`] of `d`; `rd rd + tdq`.
pub fn rigid_trans_dist_sq(
    g: &mut Globals,
    err: &mut dyn Write,
    a: &VnlMatrix,
    b: Option<&VnlMatrix>,
) -> Result<f64, Fatal> {
    let d = match b {
        Some(b) if !b.is_empty() => b.mul(&matrix_inverse(g, err, a)?),
        _ => a.clone(),
    };
    let mut tdq = 0.0f64;
    for r in 0..3 {
        tdq = add64(tdq, mul64(d.at(r, 3), d.at(r, 3)));
    }
    let m4 = d.to_4x4().expect("a 4 x 4 transform");
    let rd = rot_matrix_log_norm(err, &m4);
    Ok(add64(mul64(rd, rd), tdq))
}

/// `vnl_matrix_inverse<double>(a)` as a matrix: `vnl_svd<double>(a).pinverse()`, `V W^-1 U^T`
/// with `W^-1` a full matrix (the inverse of each singular value kept, 0 for a zero one), after
/// VNL's warning when LINPACK does not converge, then (FS-968, rule B) by default an error and the
/// end of the program; `reproduce` uses the decomposition reached, as VNL does (its NaN bits are not
/// yet the official build's: the crate's PROVENANCE.md).
/// Where VNL never returns (ITK-116: FreeSurfer hangs) an error and the end of the program here.
pub fn matrix_inverse(
    g: &mut Globals,
    err: &mut dyn Write,
    a: &VnlMatrix,
) -> Result<VnlMatrix, Fatal> {
    let Some(d) = svd(g, a) else {
        let _ = writeln!(err, "{SVD_HANGS}");
        return Err(Fatal::Exit(1));
    };
    if let Some(w) = &d.warning {
        let _ = err.write_all(w.as_bytes());
        g.notices.add(
            &FS_968,
            "the inverse of a matrix whose singular value decomposition did not converge: FreeSurfer's from the decomposition reached",
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                err,
                "MyMatrix: no inverse of a matrix whose singular value decomposition did not converge (an infinite or NaN entry)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let p = d.w.len();
    let mut winv = VnlMatrix::zeros(p, p);
    for (i, &x) in d.w.iter().enumerate() {
        if x != 0.0 {
            *winv.at_mut(i, i) = div64(1.0, x);
        }
    }
    Ok(d.v.mul(&winv).mul(&d.u.transpose()))
}

/// `MyMatrix::getFrobeniusDiff(m1, m2)` (VNL): `(m1 - m2).fro_norm()`.
pub fn get_frobenius_diff(m1: &VnlMatrix, m2: &VnlMatrix) -> f64 {
    m1.sub(m2).fro_norm()
}

/// `MyMatrix::RotMatrixLogNorm(m)` (VNL): `vnl_determinant(m)`; `There is streching! det: <det>`
/// on standard error when `|det - 1| > 0.001` (FreeSurfer's assertion after it is compiled out);
/// then `t = (trace of the upper 3 x 3 block - 1) / 2` clipped to `[-1, 1]` and `sqrt(2) acos(t)`
/// (the correctly rounded `acos` in both modes: the project's decision for glibc's `acos`,
/// GLIBC-07).
// C's two comparisons, as written (a NaN passes both)
#[allow(clippy::manual_clamp)]
pub fn rot_matrix_log_norm(err: &mut dyn Write, m: &Mat4) -> f64 {
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    let det = vnl_fixed::determinant(&flat, 4);
    if sub64(det, 1.0).abs() > 0.001 {
        let _ = writeln!(err, "There is streching! det: {}", cfmt::ostream(det));
    }
    let mut trace = 0.0f64;
    for (n, row) in m.iter().enumerate().take(3) {
        trace = add64(trace, row[n]);
    }
    trace = mul64(0.5, sub64(trace, 1.0));
    if trace > 1.0 {
        trace = 1.0;
    }
    if trace < -1.0 {
        trace = -1.0;
    }
    let theta = larmorx_core::math::acos(trace);
    mul64(std::f64::consts::SQRT_2, theta)
}

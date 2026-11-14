// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's float matrices' decompositions and what is built on them (`fs.maths`, queue item
//! 3.1-24), translated from FreeSurfer 8.2.0's `utils/matrix.cpp`: `compare_evalues`,
//! `MatrixCalculateEigenSystemHelper`, `MatrixEigenSystem` (1611-1697), `MatrixSVD`,
//! `MatrixSVDEigenValues`, `MatrixSVDInverse` (1779-1919), `MatrixConditionNumber`,
//! `MatrixNSConditionNumber` (2464-2524), `MatrixPseudoInverse` (2792-2825) and
//! `MatrixSVDPseudoInverse` (3613-3658), on `utils/numerics.cpp`'s VNL wrappers
//! ([`numerics::open_eigen_system`], [`numerics::open_non_symmetric_eigen_system`],
//! [`numerics::open_svdcmp`]).
//!
//! The eigenvalues are sorted by the C library's `qsort` with `compare_evalues`, which returns 1
//! when the first value is smaller in magnitude and -1 otherwise (never 0): equal magnitudes keep
//! their order (glibc 2.35's `qsort` is a merge sort for these sizes, stable for a comparator that
//! is consistent). [`sort_evalues`] is a top-down merge sort that takes the left element unless the
//! comparator says it goes after the right one; with NaN eigenvalues the comparator is not
//! consistent and the order depends on the sort's algorithm (measured equal to FreeSurfer's on the
//! probe's cases, `docs/findings/freesurfer-maths.md`).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-maths.md`):
//! - **FS-40** (corrected by default): [`matrix_svd_eigen_values`] and [`matrix_condition_number`]
//!   read the singular values to the row count; there are only as many as the columns. By default
//!   they read the `cols` values (the rest of the eigenvalues 0); `reproduce` reads past the
//!   vector (0 there: FS-16).
//! - **FS-43** (corrected by default): [`matrix_svd`] allocates `V` rows x rows and ignores a failed
//!   decomposition. By default `V` is cols x cols and a failure is an error (`NULL`).
//! - **FS-44** (corrected by default): [`matrix_svd_inverse`] of a matrix with more rows than columns
//!   (see its documentation).
//! - **FS-45** (corrected by default): [`matrix_pseudo_inverse`] of a matrix with fewer rows than
//!   columns and a given output, or a `NULL` inner pseudo-inverse.
//! - **FS-46** (corrected by default): [`matrix_svd_pseudo_inverse`]'s given output, failed
//!   decomposition and `NULL` inner result.
//! - **FS-450** (corrected by default): [`matrix_svd`] and [`matrix_svd_eigen_values`] of a matrix
//!   without columns (any row count) crash FreeSurfer. By default the decomposition is the empty
//!   one (nothing written: `V` 0 x 0, the values cleared, the condition number `1e8` as for a
//!   matrix of zeros); `reproduce` ends the program with status 139, as FreeSurfer's crash
//!   (signal 11).
//! - FS-25 and FS-28 as in [`crate::matrix_ops`]; FS-41, FS-42 and VNL's ITK-110 to ITK-112 in
//!   [`crate::numerics`].

use std::io::Write;

use larmorx_core::defects::{FS_40, FS_43, FS_44, FS_45, FS_46, FS_450};

use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use crate::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_alloc2, matrix_copy, matrix_multiply, read_relt,
    write_relt,
};
use crate::matrix_ops::{
    Region, done, fzero, matrix_copy_region, matrix_diag, matrix_inverse, matrix_is_symmetric,
    matrix_is_zero, matrix_transpose, output, real_only, unfit,
};
use crate::numerics;
use crate::ret::Ret;
use crate::sse;

/// `EIGEN_VALUE` (`EVALUE`): an eigenvalue and its index before the sort.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Evalue {
    eno: i32,
    evalue: f32,
}

/// `compare_evalues`: 1 when `|e1| < |e2|`, else -1.
fn compare_evalues(e1: &Evalue, e2: &Evalue) -> i32 {
    if e1.evalue.abs() < e2.evalue.abs() {
        1
    } else {
        -1
    }
}

/// `qsort(eigen_values, n, sizeof(EVALUE), compare_evalues)`: a top-down merge sort, the left half
/// `n / 2` long, the left element taken while the comparator gives 0 or less.
fn sort_evalues(v: &mut [Evalue]) {
    let n = v.len();
    if n <= 1 {
        return;
    }
    let n1 = n / 2;
    {
        let (a, b) = v.split_at_mut(n1);
        sort_evalues(a);
        sort_evalues(b);
    }
    let mut tmp = Vec::with_capacity(n);
    let (mut i, mut j) = (0, n1);
    while i < n1 && j < n {
        if compare_evalues(&v[i], &v[j]) <= 0 {
            tmp.push(v[i]);
            i += 1;
        } else {
            tmp.push(v[j]);
            j += 1;
        }
    }
    tmp.extend_from_slice(&v[i..n1]);
    tmp.extend_from_slice(&v[j..n]);
    v.copy_from_slice(&tmp);
}

fn reproduce(g: &Globals) -> bool {
    g.notices.reproduce()
}

/// A new real matrix (the sizes are valid: `MatrixAlloc` cannot return `NULL` for them).
fn real(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    buffered: bool,
) -> Result<Matrix, Fatal> {
    let m = if buffered {
        matrix_alloc2(g, out, err, rows, cols, MATRIX_REAL)?
    } else {
        matrix_alloc(g, out, err, rows, cols, MATRIX_REAL)?
    };
    Ok(m.expect("a real matrix"))
}

/// A function's new result that cannot be `NULL` for the inputs the caller passes.
fn new_of(r: Ret<Matrix>) -> Option<Matrix> {
    r.into_new()
}

/// `MatrixCalculateEigenSystemHelper(m, evalues, m_evectors, isSymmetric)`: the eigensystem of the
/// square `m` by [`numerics::open_eigen_system`] (`is_symmetric`) or
/// [`numerics::open_non_symmetric_eigen_system`], the eigenvalues sorted by decreasing magnitude
/// into `evalues` (which must hold `rows` floats) and the eigenvectors, in that order, as the columns
/// of `m_evectors` (a new `rows` x `cols` matrix when there is none). `Null` for a matrix that is not
/// square, and when the wrapper fails. FS-25, FS-28 (a given output smaller than the matrix).
#[allow(clippy::too_many_arguments)]
pub fn matrix_calculate_eigen_system_helper(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    evalues: &mut [f32],
    m_evectors: Option<&mut Matrix>,
    is_symmetric: bool,
) -> Result<Ret<Matrix>, Fatal> {
    if m.rows != m.cols {
        return Ok(Ret::Null);
    }
    let name = "MatrixEigenSystem";
    if real_only(g, err, name, &[m]) {
        return Ok(Ret::Null);
    }
    let n = m.rows;
    let nn = n.max(0) as usize;
    let given = m_evectors.is_some();
    let mut fresh = None;
    let Some(ev) = output(g, out, err, m_evectors, &mut fresh, n, n, MATRIX_REAL)? else {
        return Ok(Ret::Null);
    };
    if given {
        if real_only(g, err, name, &[ev]) {
            return Ok(Ret::Null);
        }
        if (ev.rows < n || ev.cols < n)
            && unfit(
                g,
                err,
                name,
                &format!(
                    "a {} x {} output for the eigenvectors of a {n} x {n} matrix",
                    ev.rows, ev.cols
                ),
            )
        {
            return Ok(Ret::Null);
        }
    }
    let mut tmp = real(g, out, err, n, n, false)?;
    let a = m.data()[..nn * nn].to_vec();
    let code = if is_symmetric {
        numerics::open_eigen_system(g, err, &a, nn, evalues, tmp.data_mut())
    } else {
        numerics::open_non_symmetric_eigen_system(g, err, &a, nn, evalues, tmp.data_mut())
    };
    if code != NO_ERROR {
        return Ok(Ret::Null);
    }
    let mut list: Vec<Evalue> = (0..nn)
        .map(|i| Evalue {
            eno: i as i32,
            evalue: evalues[i],
        })
        .collect();
    sort_evalues(&mut list);
    for (e, l) in evalues.iter_mut().zip(&list) {
        *e = l.evalue;
    }
    for col in 0..n {
        let src = list[col as usize].eno + 1;
        for row in 1..=n {
            let v = read_relt(g, &tmp, row, src);
            write_relt(g, ev, row, col + 1, v);
        }
    }
    Ok(done(fresh))
}

/// `MatrixEigenSystem(m, evalues, m_evectors)`: [`matrix_calculate_eigen_system_helper`] with
/// `MatrixIsSymmetric(m)` (every element equal to its mirror, exactly) choosing the wrapper.
pub fn matrix_eigen_system(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    evalues: &mut [f32],
    m_evectors: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if m.rows == m.cols && real_only(g, err, "MatrixEigenSystem", &[m]) {
        return Ok(Ret::Null);
    }
    let sym = matrix_is_symmetric(g, err, m) != 0;
    matrix_calculate_eigen_system_helper(g, out, err, m, evalues, m_evectors, sym)
}

/// FS-450: the decomposition of a matrix without columns, which FreeSurfer's crashes (signal 11):
/// `reproduce` ends the program with status 139; by default the empty decomposition follows.
fn no_columns(g: &mut Globals, func: &str, rows: i32) -> Result<(), Fatal> {
    let rep = reproduce(g);
    g.notices.add(
        &FS_450,
        if rep {
            format!("{func} of a {rows} x 0 matrix: FreeSurfer crashes (status 139)")
        } else {
            format!("{func} of a {rows} x 0 matrix: the empty decomposition (FreeSurfer crashes)")
        },
    );
    if rep {
        return Err(Fatal::Exit(139));
    }
    Ok(())
}

/// `MatrixSVD(mA, v_z, mV)`: [`numerics::open_svdcmp`] of `mA` (which becomes `U`), the singular
/// values into `v_z` and `V` into `mV`, which is returned (a new matrix when there is none).
///
/// **FS-43:** FreeSurfer allocates a new `mV` with `mA`'s row count for both sizes, where the
/// decomposition's `V` has its column count (for more rows than columns the numbers land in the
/// wrong places), and ignores the decomposition's failure (fewer rows than columns, or LINPACK not
/// converging): `mV` comes back unchanged, zeros for a new one, with no message. By default a new
/// `mV` is cols x cols, a given one of another size is refused (FS-28), and a failure gives the
/// error `MatrixSVD: the decomposition failed (...)` and `Null`; `reproduce` does what FreeSurfer
/// does. FS-25. FS-450 for a matrix without columns.
pub fn matrix_svd(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_a: &mut Matrix,
    v_z: &mut Matrix,
    m_v: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let name = "MatrixSVD";
    if real_only(g, err, name, &[m_a, v_z]) {
        return Ok(Ret::Null);
    }
    let rep = reproduce(g);
    let (rows, cols) = (m_a.rows, m_a.cols);
    let given = m_v.is_some();
    if !given && rows > cols {
        g.notices.add(
            &FS_43,
            if rep {
                format!("MatrixSVD of a {rows} x {cols} matrix: V allocated {rows} x {rows}, its numbers misplaced, as FreeSurfer")
            } else {
                format!("MatrixSVD of a {rows} x {cols} matrix: V allocated {cols} x {cols}")
            },
        );
    }
    let vn = if rep { rows } else { cols };
    let mut fresh = None;
    let Some(v) = output(g, out, err, m_v, &mut fresh, vn, vn, MATRIX_REAL)? else {
        return Ok(Ret::Null);
    };
    if given {
        if real_only(g, err, name, &[v]) {
            return Ok(Ret::Null);
        }
        if (v.rows != cols || v.cols != cols)
            && unfit(
                g,
                err,
                name,
                &format!(
                    "a {} x {} output for the V of a {rows} x {cols} matrix",
                    v.rows, v.cols
                ),
            )
        {
            return Ok(Ret::Null);
        }
    }
    if cols == 0 {
        no_columns(g, name, rows)?;
    }
    if numerics::open_svdcmp(g, err, m_a, v_z, v) != NO_ERROR {
        if rep {
            g.notices.add(
                &FS_43,
                "MatrixSVD: the decomposition's failure ignored, V returned unchanged, as FreeSurfer",
            );
            return Ok(done(fresh));
        }
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MatrixSVD: the decomposition failed (a {rows} x {cols} matrix: fewer rows than columns, or one LINPACK cannot decompose)\n"
            ),
        );
        g.notices
            .add(&FS_43, "MatrixSVD: the decomposition failed: NULL");
        return Ok(Ret::Null);
    }
    Ok(done(fresh))
}

/// How many of the singular values FS-40's loops read: `rows` with `reproduce` (past the `cols`
/// values for more rows than columns, FS-16 there), else the values there are.
fn singular_count(g: &mut Globals, func: &str, rows: i32, cols: i32) -> i32 {
    if rows <= cols {
        return rows;
    }
    if reproduce(g) {
        g.notices.add(
            &FS_40,
            format!(
                "{func} of a {rows} x {cols} matrix: {rows} singular values read, as FreeSurfer"
            ),
        );
        rows
    } else {
        g.notices.add(
            &FS_40,
            format!("{func} of a {rows} x {cols} matrix: its {cols} singular values read"),
        );
        cols
    }
}

/// The condition number of `MatrixSVDEigenValues` and `MatrixConditionNumber` from the first `nw`
/// singular values of the row vector `w`: their largest magnitude over their smallest (the first
/// value as it is, the others' magnitudes), `1e8` when the smallest is below `FLT_EPSILON`.
fn condition_from(g: &mut Globals, w: &Matrix, nw: i32) -> f32 {
    if nw < 1 {
        // no singular values (FS-450's default for a matrix without columns): as for zeros
        return 1e8;
    }
    let mut wmin = read_relt(g, w, 1, 1);
    let mut wmax = wmin;
    for row in 2..=nw {
        let wi = read_relt(g, w, 1, row).abs();
        if wi > wmax {
            wmax = wi;
        }
        if wi < wmin {
            wmin = wi;
        }
    }
    if fzero(f64::from(wmin)) {
        1e8
    } else {
        sse::div(wmax, wmin)
    }
}

/// `MatrixSVDEigenValues(m, evalues)`: the singular values of `m` sorted by decreasing magnitude
/// into `evalues` (which must hold `rows` floats: the first `rows` are cleared first) and the
/// condition number ([`condition_from`]). When the decomposition fails (fewer rows than columns, or
/// LINPACK not converging) it returns `Gerror` (FreeSurfer's last error code) as a float, the
/// eigenvalues cleared. FS-25, FS-40, FS-450.
pub fn matrix_svd_eigen_values(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    evalues: &mut [f32],
) -> Result<f32, Fatal> {
    let (rows, cols) = (m.rows, m.cols);
    if real_only(g, err, "MatrixSVDEigenValues", &[m]) {
        return Ok(g.gerror as f32);
    }
    let mut u = new_of(matrix_copy(g, out, err, Some(m), None)?).expect("a copy");
    let mut w = real(g, out, err, 1, cols, false)?;
    let mut v = real(g, out, err, cols, cols, false)?;
    let nev = rows.max(0) as usize;
    evalues[..nev].fill(0.0);
    if cols == 0 {
        no_columns(g, "MatrixSVDEigenValues", rows)?;
    }
    if numerics::open_svdcmp(g, err, &mut u, &mut w, &mut v) != NO_ERROR {
        return Ok(g.gerror as f32);
    }
    let nw = singular_count(g, "MatrixSVDEigenValues", rows, cols);
    let mut list: Vec<Evalue> = (0..nev as i32)
        .map(|i| Evalue {
            eno: i,
            evalue: if i < nw {
                read_relt(g, &w, 1, i + 1)
            } else {
                0.0
            },
        })
        .collect();
    sort_evalues(&mut list);
    for (e, l) in evalues.iter_mut().zip(&list) {
        *e = l.evalue;
    }
    Ok(condition_from(g, &w, nw))
}

/// `MatrixSVDInverse(m, m_inverse)`: the inverse of `m` from its singular value decomposition, the
/// singular values below `1e-4` times the largest dropped: `V diag(1 / w) Uᵀ` (each product in
/// `float`, [`matrix_multiply`]) into `m_inverse` (a new matrix when there is none). `Null` for a
/// matrix whose elements are all within `1e-11` of 0 (`MatrixIsZero`), when the decomposition fails
/// (fewer rows than columns, or LINPACK not converging), and after `MatrixMultiply`'s messages for
/// a given `m_inverse` of another size.
///
/// **FS-44:** for more rows than columns FreeSurfer allocates `Uᵀ` cols x cols, reads the singular
/// values and writes their inverses up to the row count, past their vectors: after a successful
/// decomposition it crashes on the row pointers of the cols x cols matrix beyond its last row. By default the sizes are the
/// decomposition's (the pseudo-inverse, cols x rows); `reproduce` gives `Null` (the crash is
/// never reproduced). FS-25.
pub fn matrix_svd_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    m_inverse: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, "MatrixSVDInverse", &[m]) {
        return Ok(Ret::Null);
    }
    if matrix_is_zero(g, err, m) != 0 {
        return Ok(Ret::Null);
    }
    let (rows, cols) = (m.rows, m.cols);
    let mut u = real(g, out, err, rows, cols, true)?;
    matrix_copy(g, out, err, Some(m), Some(&mut u))?;
    let mut v = real(g, out, err, cols, cols, true)?;
    let mut w = real(g, out, err, 1, cols, true)?;
    if numerics::open_svdcmp(g, err, &mut u, &mut w, &mut v) != NO_ERROR {
        return Ok(Ret::Null);
    }
    if rows > cols {
        if reproduce(g) {
            g.notices.add(
                &FS_44,
                format!("MatrixSVDInverse of a {rows} x {cols} matrix: NULL (FreeSurfer crashes)"),
            );
            return Ok(Ret::Null);
        }
        g.notices.add(
            &FS_44,
            format!(
                "MatrixSVDInverse of a {rows} x {cols} matrix: the pseudo-inverse with the decomposition's sizes"
            ),
        );
    }
    let mut mw = real(g, out, err, cols, cols, true)?;
    let nw = rows.min(cols);
    let mut wmax = 0.0f32;
    for row in 1..=nw {
        let a = read_relt(g, &w, 1, row).abs();
        if a > wmax {
            wmax = a;
        }
    }
    let wmin = (1e-4 * f64::from(wmax)) as f32;
    for row in 1..=nw {
        let x = read_relt(g, &w, 1, row);
        let inv = if x.abs() < wmin {
            0.0
        } else {
            sse::div(1.0, x)
        };
        write_relt(g, &mut mw, row, row, inv);
    }
    let mut ut = real(g, out, err, cols, rows, true)?;
    matrix_transpose(g, out, err, &u, Some(&mut ut))?;
    let mut tmp = real(g, out, err, cols, rows, true)?;
    if matrix_multiply(g, out, err, Some(&mw), Some(&ut), Some(&mut tmp))?.is_null() {
        return Ok(Ret::Null);
    }
    matrix_multiply(g, out, err, Some(&v), Some(&tmp), m_inverse)
}

/// `MatrixConditionNumber(m)`: `1e10` for a matrix whose elements are all within `1e-11` of 0; else
/// the condition number of its singular values ([`condition_from`]). The decomposition's failure is
/// not checked (fewer rows than columns, or LINPACK not converging): the unset singular values are
/// 0 and the condition number `1e8`, FreeSurfer's "something big" (kept: the callers compare it with
/// a limit, and [`matrix_ns_condition_number`] is the function for matrices that are not square).
/// FS-25 (`1e8` by default for a complex matrix, after the error), FS-40.
pub fn matrix_condition_number(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
) -> Result<f32, Fatal> {
    if real_only(g, err, "MatrixConditionNumber", &[m]) {
        return Ok(1e8);
    }
    if matrix_is_zero(g, err, m) != 0 {
        return Ok(1e10);
    }
    let (rows, cols) = (m.rows, m.cols);
    let mut u = new_of(matrix_copy(g, out, err, Some(m), None)?).expect("a copy");
    let mut w = real(g, out, err, 1, cols, false)?;
    let mut v = real(g, out, err, cols, cols, false)?;
    let _ = numerics::open_svdcmp(g, err, &mut u, &mut w, &mut v);
    let nw = singular_count(g, "MatrixConditionNumber", rows, cols);
    Ok(condition_from(g, &w, nw))
}

/// `MatrixNSConditionNumber(m)`: [`matrix_condition_number`] of a square `m`, else of `mᵀm` (more
/// rows than columns) or `mmᵀ` (fewer), each product in `float`. FS-25.
pub fn matrix_ns_condition_number(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
) -> Result<f32, Fatal> {
    if m.rows == m.cols {
        return matrix_condition_number(g, out, err, m);
    }
    if real_only(g, err, "MatrixNSConditionNumber", &[m]) {
        return Ok(1e8);
    }
    let mt = new_of(matrix_transpose(g, out, err, m, None)?).expect("a transpose");
    let p = if m.rows > m.cols {
        matrix_multiply(g, out, err, Some(&mt), Some(m), None)?
    } else {
        matrix_multiply(g, out, err, Some(m), Some(&mt), None)?
    };
    let p = new_of(p).expect("a product");
    matrix_condition_number(g, out, err, &p)
}

/// `MatrixPseudoInverse(m, m_pseudo_inv)`: `(mᵀm)⁻¹ mᵀ` into `m_pseudo_inv` (a new matrix when
/// there is none), the inverse by `MatrixInverse` or, where it fails, [`matrix_svd_inverse`]
/// (`Null` when both fail); for fewer rows than columns the transpose of the pseudo-inverse of
/// `mᵀ`. A given output of another size gives `MatrixMultiply`'s messages and `Null`.
///
/// **FS-45:** for fewer rows than columns FreeSurfer passes the caller's output on to the inner
/// pseudo-inverse, which has the other shape: the product cannot be written into it, and it
/// crashes transposing the `NULL` result (as it does when the inner pseudo-inverse is `NULL`, a
/// zero matrix); an output of the inner shape is filled, then freed, and a new matrix returned. By
/// default the inner pseudo-inverse is a new matrix, its transpose goes into the caller's output
/// (FS-28 when it does not fit), and a `NULL` inner result gives `Null`; `reproduce` fills an output
/// of the inner shape and returns a new transpose, and gives `Null` where FreeSurfer crashes. FS-25.
pub fn matrix_pseudo_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    m_pseudo_inv: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, "MatrixPseudoInverse", &[m]) {
        return Ok(Ret::Null);
    }
    let (rows, cols) = (m.rows, m.cols);
    if rows < cols {
        let mt = new_of(matrix_transpose(g, out, err, m, None)?).expect("a transpose");
        let crash = |g: &mut Globals| {
            g.notices.add(
                &FS_45,
                format!("MatrixPseudoInverse of a {rows} x {cols} matrix: NULL (FreeSurfer crashes transposing the NULL inner result)"),
            );
        };
        return match m_pseudo_inv {
            None => match matrix_pseudo_inverse(g, out, err, &mt, None)? {
                Ret::New(inner) => matrix_transpose(g, out, err, &inner, None),
                _ => {
                    crash(g);
                    Ok(Ret::Null)
                }
            },
            Some(p) if reproduce(g) => {
                match matrix_pseudo_inverse(g, out, err, &mt, Some(&mut *p))? {
                    Ret::Null => {
                        crash(g);
                        Ok(Ret::Null)
                    }
                    _ => {
                        g.notices.add(
                        &FS_45,
                        "MatrixPseudoInverse: the caller's output filled with the inner pseudo-inverse (FreeSurfer then frees it): a new matrix returned",
                    );
                        let inner = p.clone();
                        matrix_transpose(g, out, err, &inner, None)
                    }
                }
            }
            Some(p) => match matrix_pseudo_inverse(g, out, err, &mt, None)? {
                Ret::New(inner) => {
                    g.notices.add(
                        &FS_45,
                        "MatrixPseudoInverse of a matrix with fewer rows than columns: the result written into the caller's output",
                    );
                    matrix_transpose(g, out, err, &inner, Some(p))
                }
                _ => {
                    crash(g);
                    Ok(Ret::Null)
                }
            },
        };
    }
    let mt = new_of(matrix_transpose(g, out, err, m, None)?).expect("a transpose");
    let mtm = new_of(matrix_multiply(g, out, err, Some(&mt), Some(m), None)?).expect("a product");
    let inv = match matrix_inverse(g, out, err, Some(&mtm), None)? {
        Ret::New(x) => x,
        _ => match matrix_svd_inverse(g, out, err, &mtm, None)? {
            Ret::New(x) => x,
            _ => return Ok(Ret::Null),
        },
    };
    matrix_multiply(g, out, err, Some(&inv), Some(&mt), m_pseudo_inv)
}

/// `MatrixSVDPseudoInverse(m, m_pseudo_inv)`: the pseudo-inverse from the singular value
/// decomposition, the singular values from the first whose ratio to the first is below `1e-4`
/// dropped: `V_r diag(1 / w_r) U_rᵀ` (each product in `float`); for fewer rows than columns the
/// transpose of the pseudo-inverse of `mᵀ`. `Null` for a matrix whose elements are all within
/// `1e-11` of 0.
///
/// **FS-46:** FreeSurfer never writes a given output for at least as many rows as columns (it
/// returns a new matrix), ignores a failed decomposition (LINPACK not converging: a pseudo-inverse
/// of infinities and NaNs from the unset singular values), and for fewer rows than columns crashes
/// transposing a `NULL` inner result (a zero matrix). By default the result goes into a given
/// output, a failure is an error (`MatrixSVDPseudoInverse: the decomposition failed`, `Null`) and a
/// `NULL` inner result gives `Null`; `reproduce` returns a new matrix, goes on after a failure, and
/// gives `Null` where FreeSurfer crashes. FS-25.
pub fn matrix_svd_pseudo_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    mut m_pseudo_inv: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let name = "MatrixSVDPseudoInverse";
    if real_only(g, err, name, &[m]) {
        return Ok(Ret::Null);
    }
    let rep = reproduce(g);
    let (rows, cols) = (m.rows, m.cols);
    if rows < cols {
        let mt = new_of(matrix_transpose(g, out, err, m, None)?).expect("a transpose");
        let inner = if rep {
            matrix_svd_pseudo_inverse(g, out, err, &mt, m_pseudo_inv.as_deref_mut())?
        } else {
            matrix_svd_pseudo_inverse(g, out, err, &mt, None)?
        };
        let Some(inner) = new_of(inner) else {
            g.notices.add(
                &FS_46,
                format!("MatrixSVDPseudoInverse of a {rows} x {cols} matrix: NULL (FreeSurfer crashes transposing the NULL inner result)"),
            );
            return Ok(Ret::Null);
        };
        if rep {
            if m_pseudo_inv.is_some() {
                g.notices.add(
                    &FS_46,
                    "MatrixSVDPseudoInverse: the caller's output not written, a new matrix returned, as FreeSurfer",
                );
            }
            return matrix_transpose(g, out, err, &inner, None);
        }
        if m_pseudo_inv.is_some() {
            g.notices.add(
                &FS_46,
                "MatrixSVDPseudoInverse: the result written into the caller's output",
            );
        }
        return matrix_transpose(g, out, err, &inner, m_pseudo_inv);
    }
    let mut u = new_of(matrix_copy(g, out, err, Some(m), None)?).expect("a copy");
    let mut v = real(g, out, err, cols, cols, false)?;
    let mut s = real(g, out, err, cols, 1, false)?;
    if matrix_is_zero(g, err, m) != 0 {
        return Ok(Ret::Null);
    }
    if numerics::open_svdcmp(g, err, &mut u, &mut s, &mut v) != NO_ERROR {
        if !rep {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "{name}: the decomposition failed (a {rows} x {cols} matrix LINPACK cannot decompose)\n"
                ),
            );
            g.notices
                .add(&FS_46, format!("{name}: the decomposition failed: NULL"));
            return Ok(Ret::Null);
        }
        g.notices.add(
            &FS_46,
            format!("{name}: the decomposition's failure ignored, as FreeSurfer"),
        );
    }
    let s1 = read_relt(g, &s, 1, 1);
    let mut r = 1;
    while r <= s.rows {
        if f64::from(sse::div(read_relt(g, &s, r, 1), s1)) < 1e-4 {
            break;
        }
        r += 1;
    }
    r -= 1;
    let region = |g: &mut Globals,
                  out: &mut dyn Write,
                  err: &mut dyn Write,
                  src: &Matrix,
                  rows: i32,
                  cols: i32|
     -> Result<Matrix, Fatal> {
        Ok(new_of(matrix_copy_region(
            g,
            out,
            err,
            Region::All,
            src,
            None,
            1,
            1,
            rows,
            cols,
            1,
            1,
        )?)
        .expect("a region"))
    };
    let tmp = region(g, out, err, &u, u.rows, r)?;
    let ur = new_of(matrix_transpose(g, out, err, &tmp, None)?).expect("a transpose");
    let sm = new_of(matrix_diag(g, out, err, &s, None)?).expect("a diagonal");
    let mut sr = region(g, out, err, &sm, r, r)?;
    for c in 1..=sr.rows {
        let x = read_relt(g, &sr, c, c);
        write_relt(g, &mut sr, c, c, sse::div(1.0, x));
    }
    let vr = region(g, out, err, &v, v.rows, r)?;
    let tmp = new_of(matrix_multiply(g, out, err, Some(&vr), Some(&sr), None)?).expect("a product");
    match m_pseudo_inv {
        Some(p) if !rep => {
            g.notices.add(
                &FS_46,
                format!("{name}: the result written into the caller's output"),
            );
            matrix_multiply(g, out, err, Some(&tmp), Some(&ur), Some(p))
        }
        given => {
            if given.is_some() {
                g.notices.add(
                    &FS_46,
                    format!("{name}: the caller's output not written, a new matrix returned, as FreeSurfer"),
                );
            }
            matrix_multiply(g, out, err, Some(&tmp), Some(&ur), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn ev(v: &[f32]) -> Vec<Evalue> {
        v.iter()
            .enumerate()
            .map(|(i, &x)| Evalue {
                eno: i as i32,
                evalue: x,
            })
            .collect()
    }

    #[test]
    fn the_sort_is_by_decreasing_magnitude_and_keeps_ties_in_order() {
        let mut v = ev(&[1.0, -3.0, 2.0, 3.0, -1.0, 0.0]);
        sort_evalues(&mut v);
        let enos: Vec<i32> = v.iter().map(|e| e.eno).collect();
        assert_eq!(enos, [1, 3, 2, 0, 4, 5]);
    }

    #[test]
    fn a_symmetric_eigensystem_and_an_svd() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = Matrix::from_rows(&[vec![2.0, 0.0], vec![0.0, -5.0]]);
        let mut vals = [0.0f32; 2];
        let v = matrix_eigen_system(&mut g, &mut o, &mut e, &m, &mut vals, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(vals, [-5.0, 2.0]);
        assert_eq!(v.relt(2, 1).abs(), 1.0);
        let mut a = Matrix::from_rows(&[vec![3.0, 0.0], vec![0.0, 4.0], vec![0.0, 0.0]]);
        let mut z = Matrix::real(2, 1);
        let vv = matrix_svd(&mut g, &mut o, &mut e, &mut a, &mut z, None)
            .unwrap()
            .unwrap_new();
        assert_eq!((vv.rows, vv.cols), (2, 2));
        let mut w = [0.0f32; 2];
        z.data().iter().zip(w.iter_mut()).for_each(|(a, b)| *b = *a);
        w.sort_by(f32::total_cmp);
        assert_eq!(w, [3.0, 4.0]);
        assert!(g.notices.contains(&FS_43));
        let c = matrix_condition_number(
            &mut g,
            &mut o,
            &mut e,
            &Matrix::from_rows(&[vec![2.0, 0.0], vec![0.0, 8.0]]),
        )
        .unwrap();
        assert_eq!(c, 4.0);
    }

    #[test]
    fn decompositions_of_a_matrix_without_columns_fs450() {
        // FreeSurfer crashes (signal 11) on every row count (the open maths probe's msvd and
        // msvdvalues of 0 x 0, 1 x 0, 2 x 0 and 3 x 0 matrices, observed 2026-11-14).
        for rows in [0, 3] {
            let mut g = Globals::new(UpstreamDefects::Fix);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut a = Matrix::real(rows, 0);
            let mut z = Matrix::real(0, 1);
            let v = matrix_svd(&mut g, &mut o, &mut e, &mut a, &mut z, None)
                .unwrap()
                .unwrap_new();
            assert_eq!((v.rows, v.cols), (0, 0));
            let mut vals = [7.0f32; 4];
            let c = matrix_svd_eigen_values(&mut g, &mut o, &mut e, &a, &mut vals).unwrap();
            assert_eq!(c, 1e8);
            assert_eq!(vals[..rows as usize], [0.0; 3][..rows as usize]);
            assert!(g.notices.contains(&FS_450));
            assert!(!g.notices.contains(&larmorx_core::defects::FS_16));
            assert!(e.is_empty());

            let mut g = Globals::new(UpstreamDefects::Reproduce);
            let r = matrix_svd(&mut g, &mut o, &mut e, &mut a, &mut z, None);
            assert_eq!(r.err().map(Fatal::status), Some(139));
            let r = matrix_svd_eigen_values(&mut g, &mut o, &mut e, &a, &mut vals);
            assert_eq!(r.err().map(Fatal::status), Some(139));
            assert!(g.notices.contains(&FS_450));
        }
    }
}

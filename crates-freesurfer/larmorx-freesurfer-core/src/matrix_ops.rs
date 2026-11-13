// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's float matrices: the element-wise operations, regions, vectors, printing and the
//! Gaussian matrices (`fs.maths`, queue item 3.1-24), translated from FreeSurfer 8.2.0's
//! `utils/matrix.cpp` 844-3660 (the functions FreeSurfer's programs reach: `MatrixPrint`,
//! `MatrixPrintFmt`, `MatrixIdentity`, `MatrixTranspose`, `MatrixAdd`, `MatrixSubtract`,
//! `MatrixScalarMul`, `MatrixClear`, `MatrixSquareElts`, `MatrixDiag`, `MatrixCopyRegion`,
//! `MatrixCopyRealRegion`, `MatrixCopyImagRegion`, `MatrixRealToComplex`, `MatrixIsSymmetric`,
//! `MatrixReallocRotation`, `MatrixAllocRotation`, `VectorLen`, `VectorDot`, `VectorDistance`,
//! `MatrixIsZero`, `VectorCrossProduct`, `VectorTripleProduct`, `Vector3Angle`,
//! `XYZ_NORMALIZED_LOAD`, `XYZApproxAngle_knownLength`, `MatrixCheck`, `MatrixReshape`,
//! `MatrixTrace`, `MatrixConstVal`, `MatrixZero`, `MatrixSumElts`, `MatrixNormalizeColScale`,
//! `MatrixNormalizeCol`, `MatrixSimilarityTransform`, `GaussianMatrix2`, `GaussianMatrix`) and
//! the `MatrixClone` macro. The product (`MatrixMultiply`) is in [`crate::matrix`]; the eigensystems,
//! SVD, condition numbers and pseudo-inverses in [`crate::matrix_decomp`] (the determinant, inverse
//! and QR factors are here).
//!
//! Every expression keeps C's types: `float` sums stay `float` (each step rounded), `double`
//! ones `double`; elements are addressed as the C macros address them (`rptr[r][c]`), so where a
//! caller's sizes do not match, the floats FreeSurfer reads and writes inside the array are the
//! ones read and written here, and outside it FS-16 applies (a read gives 0, a write is dropped,
//! in both modes). A C library function of a `float` argument is C++'s `float` overload (`expf`,
//! `sinf`, `cosf`, `sqrtf`), as `<cmath>` declares them; of a `double`, the `double` one. `expf`,
//! `sinf` and `cosf` are larmorx's correctly rounded ones by default and glibc 2.35's with
//! `reproduce` (`larmorx_core::math::Libm`), GLIBC-07 noted in both modes where the two differ on
//! a value computed (`cstd::note_glibc07`); `acos` is the correctly rounded one in both modes
//! (glibc's `acos` is not ported yet: queue item 3.1-08).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-maths.md`):
//! - **FS-25** (an error by default; `MatrixClear` corrected): the operations written for real
//!   matrices address a complex matrix's floats through its real-element pointers. By default
//!   they refuse a complex operand (`<function>: a complex matrix (the function handles real
//!   matrices only)` as an error, then `NULL` or the function's error value), and `MatrixClear`
//!   clears every float of a complex matrix; `reproduce` addresses the floats FreeSurfer does.
//! - **FS-26** (corrected by default): `MatrixCopyRealRegion` into a complex destination copies
//!   the imaginary parts. The default copies the real parts.
//! - **FS-27** (corrected by default; an error for `MatrixCopyRegion`): the real- and
//!   imaginary-part region copies read the source through `MATRIX_CELT` whatever its type. By
//!   default a real source's real part is its value and its imaginary part 0; `MatrixCopyRegion`
//!   between a real and a complex matrix is refused.
//! - **FS-28** (an error by default): a caller's output that does not fit is filled anyway. By
//!   default the function refuses it (an error, `NULL`).
//! - **FS-31** (an error by default): the vector functions read past a vector shorter than the
//!   others or than three elements. By default they refuse it.

use std::io::Write;

use larmorx_core::defects::{FS_25, FS_26, FS_27, FS_28, FS_31, FS_33};
use larmorx_core::math::Libm;

use crate::cstd;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use crate::matrix::{
    self, MATRIX_COMPLEX, MATRIX_REAL, Matrix, celt_read, matrix_alloc, read_relt, rptr, write_at,
    write_relt,
};
use crate::numerics;
use crate::ret::Ret;
use crate::sse;

/// `X_ROTATION`, `Y_ROTATION`, `Z_ROTATION` (`include/matrix.h`).
pub const X_ROTATION: i32 = 0;
pub const Y_ROTATION: i32 = 1;
pub const Z_ROTATION: i32 = 2;

/// `FZERO(f)`: `fabs(f) < FLT_EPSILON`.
pub fn fzero(f: f64) -> bool {
    f.abs() < f64::from(f32::EPSILON)
}

/// FS-25 for a function written for real matrices: `true` (refused, after the error) by default
/// when an operand is complex; with `reproduce` it notes FS-25 and the caller goes on.
pub(crate) fn real_only(g: &mut Globals, err: &mut dyn Write, func: &str, ms: &[&Matrix]) -> bool {
    if !ms.iter().any(|m| m.mtype == MATRIX_COMPLEX) {
        return false;
    }
    if g.notices.reproduce() {
        g.notices.add(
            &FS_25,
            format!("{func}: a complex matrix addressed as if real, as FreeSurfer"),
        );
        return false;
    }
    log::error_printf(
        g,
        err,
        ERROR_BADPARM,
        &format!("{func}: a complex matrix (the function handles real matrices only)\n"),
    );
    g.notices
        .add(&FS_25, format!("{func}: a complex matrix refused"));
    true
}

/// FS-28 for an output that does not fit: `true` (refused, after the error) by default; with
/// `reproduce` it notes FS-28 and the caller fills it as FreeSurfer does.
pub(crate) fn unfit(g: &mut Globals, err: &mut dyn Write, func: &str, what: &str) -> bool {
    if g.notices.reproduce() {
        g.notices.add(
            &FS_28,
            format!("{func}: {what}, filled as FreeSurfer fills it"),
        );
        return false;
    }
    log::error_printf(g, err, ERROR_BADPARM, &format!("{func}: {what}\n"));
    g.notices.add(&FS_28, format!("{func}: {what}: refused"));
    true
}

/// FS-31 for a vector too short: `true` (refused, after the error) by default.
fn short_vector(g: &mut Globals, err: &mut dyn Write, func: &str, what: &str) -> bool {
    if g.notices.reproduce() {
        g.notices.add(
            &FS_31,
            format!("{func}: {what}: read past its end as FreeSurfer reads it"),
        );
        return false;
    }
    log::error_printf(g, err, ERROR_BADPARM, &format!("{func}: {what}\n"));
    g.notices.add(&FS_31, format!("{func}: {what}: refused"));
    true
}

/// The output of `T *F(..., MATRIX *mOut)`: the caller's, or a new matrix of `rows` x `cols` of
/// `mtype` (`None`: `MatrixAlloc` returned `NULL`, an unknown type).
#[allow(clippy::too_many_arguments)]
pub(crate) fn output<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    given: Option<&'a mut Matrix>,
    fresh: &'a mut Option<Matrix>,
    rows: i32,
    cols: i32,
    mtype: i16,
) -> Result<Option<&'a mut Matrix>, Fatal> {
    Ok(match given {
        Some(m) => Some(m),
        None => match matrix_alloc(g, out, err, rows, cols, mtype)? {
            Some(m) => Some(fresh.insert(m)),
            None => None,
        },
    })
}

pub(crate) fn done(fresh: Option<Matrix>) -> Ret<Matrix> {
    match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    }
}

/// `MATRIX_CELT_REAL(m, r, c)` and `MATRIX_CELT_IMAG(m, r, c)` as floats (FS-16 outside).
fn celt(g: &mut Globals, m: &Matrix, r: i32, c: i32) -> (f32, f32) {
    let (re, im) = celt_read(g, m, r, c);
    (re as f32, im as f32)
}

/// `MATRIX_CELT_REAL(m, r, c) = re; MATRIX_CELT_IMAG(m, r, c) = im` (FS-16 outside).
fn set_celt(g: &mut Globals, m: &mut Matrix, r: i32, c: i32, re: f32, im: f32) {
    let i = rptr(m, r) + 2 * c as isize;
    write_at(g, m, i, re);
    write_at(g, m, i + 1, im);
}

/// `MATRIX_CELT_IMAG(m, r, c) = im` alone.
fn set_celt_imag(g: &mut Globals, m: &mut Matrix, r: i32, c: i32, im: f32) {
    let i = rptr(m, r) + 2 * c as isize;
    write_at(g, m, i + 1, im);
}

/// `MATRIX_CELT_REAL(m, r, c) = re` alone.
fn set_celt_real(g: &mut Globals, m: &mut Matrix, r: i32, c: i32, re: f32) {
    let i = rptr(m, r) + 2 * c as isize;
    write_at(g, m, i, re);
}

/// The text of one `printf` conversion of a `double` for the formats FreeSurfer passes.
fn fmt(spec: &str, x: f64) -> String {
    cstd::format_double(spec, x).unwrap_or_else(|| panic!("format {spec:?} not supported"))
}

/// The shared body of `MatrixPrint` (`spec` `None`: `% 2.<p>f` and `;` after each row) and
/// `MatrixPrintFmt`.
fn print_body(
    g: &mut Globals,
    fp: &mut dyn Write,
    err: &mut dyn Write,
    mat: &Matrix,
    real_spec: &str,
    complex_specs: (&str, &str),
    row_end: &[u8],
) -> i32 {
    for row in 1..=mat.rows {
        for col in 1..=mat.cols {
            match mat.mtype {
                MATRIX_REAL => {
                    let v = read_relt(g, mat, row, col);
                    let _ = fp.write_all(fmt(real_spec, f64::from(v)).as_bytes());
                }
                MATRIX_COMPLEX => {
                    let (re, im) = celt(g, mat, row, col);
                    let _ = write!(
                        fp,
                        "{} + {} i",
                        fmt(complex_specs.0, f64::from(re)),
                        fmt(complex_specs.1, f64::from(im))
                    );
                }
                t => {
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!("MatrixPrint: unknown type {t}\n"),
                    );
                    return ERROR_BADPARM;
                }
            }
            if col < mat.cols {
                let _ = fp.write_all(b"  ");
            }
        }
        let _ = fp.write_all(row_end);
    }
    NO_ERROR
}

/// `MatrixPrint(fp, mat, precision_real)` (`precision_real` 5 by default in the C header): each
/// real element as `% 2.<precision_real>f`, a complex one as `% 2.3f + % 2.3f i`, two spaces
/// between, `;` and a new line after each row. `fp` `None` (`NULL`) prints on `out` after
/// `MatrixPrint: fp = NULL!` as an error; `mat` `None` gives the error `MatrixPrint: mat =
/// NULL!` and `ERROR_BADPARM`; a matrix of another type gives `MatrixPrint: unknown type <type>`
/// at its first element. Returns `NO_ERROR` or `ERROR_BADPARM`.
pub fn matrix_print(
    g: &mut Globals,
    fp: Option<&mut dyn Write>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mat: Option<&Matrix>,
    precision_real: i32,
) -> i32 {
    let fp: &mut dyn Write = match fp {
        Some(fp) => fp,
        None => {
            log::error_printf(g, err, ERROR_BADPARM, "MatrixPrint: fp = NULL!");
            out
        }
    };
    let Some(mat) = mat else {
        log::error_printf(g, err, ERROR_BADPARM, "MatrixPrint: mat = NULL!");
        return ERROR_BADPARM;
    };
    // a negative precision given to `*` is taken as if omitted: 6
    let p = if precision_real < 0 {
        6
    } else {
        precision_real
    };
    let spec = format!("% 2.{p}f");
    let r = print_body(g, fp, err, mat, &spec, ("% 2.3f", "% 2.3f"), b";\n");
    // fflush(stdout), not fp
    let _ = fp.flush();
    r
}

/// `MatrixPrintFmt(fp, fmt, mat)`: each element with the caller's `printf` format (one `%f`
/// conversion: FreeSurfer's callers pass `%10f`), a complex one as `<fmt> + <fmt> i`, two spaces
/// between, a new line after each row (no `;`). The messages and returns of [`matrix_print`].
pub fn matrix_print_fmt(
    g: &mut Globals,
    fp: Option<&mut dyn Write>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    spec: &str,
    mat: Option<&Matrix>,
) -> i32 {
    let fp: &mut dyn Write = match fp {
        Some(fp) => fp,
        None => {
            log::error_printf(g, err, ERROR_BADPARM, "MatrixPrint: fp = NULL!");
            out
        }
    };
    let Some(mat) = mat else {
        log::error_printf(g, err, ERROR_BADPARM, "MatrixPrint: mat = NULL!");
        return ERROR_BADPARM;
    };
    let r = print_body(g, fp, err, mat, spec, (spec, spec), b"\n");
    let _ = fp.flush();
    r
}

/// `MatrixClear(mat)`: `memset` of `cols + 1` floats from each row's pointer (the float before
/// the row's first element, then the row): a real matrix becomes zeros (and its unused float
/// before the numbers); of a complex matrix FreeSurfer clears only part (FS-25: by default every
/// float).
pub fn matrix_clear(g: &mut Globals, mat: &mut Matrix) {
    if mat.mtype == MATRIX_COMPLEX {
        if !g.notices.reproduce() {
            if mat.data().iter().any(|&v| v != 0.0) {
                g.notices.add(
                    &FS_25,
                    "MatrixClear of a complex matrix: every float cleared",
                );
            }
            for v in mat.data_mut() {
                *v = 0.0;
            }
            return;
        }
        g.notices.add(
            &FS_25,
            "MatrixClear of a complex matrix: the floats FreeSurfer clears",
        );
    }
    for row in 1..=mat.rows {
        let base = rptr(mat, row);
        for k in 0..=mat.cols as isize {
            write_at(g, mat, base + k, 0.0);
        }
    }
}

/// `MatrixIdentity(n, mat)`: a new `n` x `n` real identity, or `mat` cleared with ones at
/// `(i, i)` for `i` in `1..=n` (FS-28 when `mat` is smaller than `n`; FS-25 when complex).
pub fn matrix_identity(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    n: i32,
    mat: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m: &mut Matrix = match mat {
        None => match matrix_alloc(g, out, err, n, n, MATRIX_REAL)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
        Some(m) => {
            if real_only(g, err, "MatrixIdentity", &[m]) {
                return Ok(Ret::Null);
            }
            if (m.rows < n || m.cols < n)
                && unfit(
                    g,
                    err,
                    "MatrixIdentity",
                    &format!("a {} x {} matrix for an identity of {n}", m.rows, m.cols),
                )
            {
                return Ok(Ret::Null);
            }
            matrix_clear(g, m);
            m
        }
    };
    for i in 1..=n {
        write_relt(g, m, i, i, 1.0);
    }
    Ok(done(fresh))
}

/// `MatrixTranspose(mIn, mOut)`: `mOut[col][row] = mIn[row][col]`; a new output is `cols` x
/// `rows` of `mIn`'s type. For a transpose in place pass a copy of the matrix as `m_in`
/// (FreeSurfer copies it first). FS-25, FS-28 (a given output of another size).
pub fn matrix_transpose(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, "MatrixTranspose", &[m_in]) {
        return Ok(Ret::Null);
    }
    let given = m_out.is_some();
    let mut fresh = None;
    let Some(m) = output(
        g, out, err, m_out, &mut fresh, m_in.cols, m_in.rows, m_in.mtype,
    )?
    else {
        return Ok(Ret::Null);
    };
    if given {
        if real_only(g, err, "MatrixTranspose", &[m]) {
            return Ok(Ret::Null);
        }
        if (m.rows != m_in.cols || m.cols != m_in.rows)
            && unfit(
                g,
                err,
                "MatrixTranspose",
                &format!(
                    "a {} x {} output for the transpose of {} x {}",
                    m.rows, m.cols, m_in.rows, m_in.cols
                ),
            )
        {
            return Ok(Ret::Null);
        }
    }
    for row in 1..=m_in.rows {
        for col in 1..=m_in.cols {
            let v = read_relt(g, m_in, row, col);
            write_relt(g, m, col, row, v);
        }
    }
    Ok(done(fresh))
}

/// The shared body of `MatrixAdd` and `MatrixSubtract`.
fn add_sub(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &Matrix,
    m2: &Matrix,
    m_out: Option<&mut Matrix>,
    subtract: bool,
) -> Result<Ret<Matrix>, Fatal> {
    let (name, op) = if subtract {
        ("MatrixSubtract", "-")
    } else {
        ("MatrixAdd", "+")
    };
    let (rows, cols) = (m1.rows, m1.cols);
    if rows != m2.rows || cols != m2.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{name}: incompatable matrices {rows} x {cols} {op} {} x {}\n",
                m2.rows, m2.cols
            ),
        );
        return Ok(Ret::Null);
    }
    if real_only(g, err, name, &[m1, m2]) {
        return Ok(Ret::Null);
    }
    let given = m_out.is_some();
    let mut fresh = None;
    let Some(m) = output(g, out, err, m_out, &mut fresh, rows, cols, m1.mtype)? else {
        return Ok(Ret::Null);
    };
    if rows != m.rows || cols != m.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{name}: incompatable matrices {rows} x {cols} = {} x {}\n",
                m.rows, m.cols
            ),
        );
        return Ok(Ret::Null);
    }
    if given && real_only(g, err, name, &[m]) {
        return Ok(Ret::Null);
    }
    for row in 1..=rows {
        for col in 1..=cols {
            let a = read_relt(g, m1, row, col);
            let b = read_relt(g, m2, row, col);
            write_relt(
                g,
                m,
                row,
                col,
                if subtract {
                    sse::sub(a, b)
                } else {
                    sse::add(a, b)
                },
            );
        }
    }
    Ok(done(fresh))
}

/// `MatrixAdd(m1, m2, mOut)`: element by element in `float`; `Null` after `MatrixAdd:
/// incompatable matrices ...` for operands of other sizes or an output of another size. A sum
/// into an operand: pass a copy as the operand (each element is read before it is written, so
/// the result is the same). FS-25.
pub fn matrix_add(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &Matrix,
    m2: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    add_sub(g, out, err, m1, m2, m_out, false)
}

/// `MatrixSubtract(m1, m2, mOut)`: as [`matrix_add`], `m1 - m2`.
pub fn matrix_subtract(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &Matrix,
    m2: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    add_sub(g, out, err, m1, m2, m_out, true)
}

/// The element-wise map of `MatrixScalarMul` and `MatrixSquareElts`: a new output of `mIn`'s
/// size and type, or a given one of the same size (`<name>: incompatable matrices` otherwise).
#[allow(clippy::too_many_arguments)]
fn map_elts(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    name: &str,
    m_in: &Matrix,
    m_out: Option<&mut Matrix>,
    f: impl Fn(f32) -> f32,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, name, &[m_in]) {
        return Ok(Ret::Null);
    }
    let given = m_out.is_some();
    let mut fresh = None;
    let Some(m) = output(
        g, out, err, m_out, &mut fresh, m_in.rows, m_in.cols, m_in.mtype,
    )?
    else {
        return Ok(Ret::Null);
    };
    let (rows, cols) = (m_in.rows, m_in.cols);
    if rows != m.rows || cols != m.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{name}: incompatable matrices {rows} x {cols} != {} x {}\n",
                m.rows, m.cols
            ),
        );
        return Ok(Ret::Null);
    }
    if given && real_only(g, err, name, &[m]) {
        return Ok(Ret::Null);
    }
    for row in 1..=rows {
        for col in 1..=cols {
            let v = read_relt(g, m_in, row, col);
            write_relt(g, m, row, col, f(v));
        }
    }
    Ok(done(fresh))
}

/// `MatrixScalarMul(mIn, val, mOut)`: each element times `val` in `float`. In place: pass a copy
/// as `m_in` (or use [`matrix_scalar_mul_in_place`]). FS-25.
pub fn matrix_scalar_mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: &Matrix,
    val: f32,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    map_elts(g, out, err, "MatrixScalarMul", m_in, m_out, |v| {
        sse::mul(v, val)
    })
}

/// `MatrixScalarMul(m, val, m)`.
pub fn matrix_scalar_mul_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Matrix,
    val: f32,
) -> Result<Ret<Matrix>, Fatal> {
    let src = m.clone();
    matrix_scalar_mul(g, out, err, &src, val, Some(m))
}

/// `MatrixSquareElts(mIn, mOut)`: each element squared in `float`. FS-25.
pub fn matrix_square_elts(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    map_elts(g, out, err, "MatrixSquareElts", m_in, m_out, |v| {
        sse::mul(v, v)
    })
}

/// `MatrixDiag(mDiag, mOut)`: the square matrix of `max(rows, cols)` with the column vector's
/// elements (the first column when `mDiag` has more than one row) or the row vector's on its
/// diagonal. A given `mOut` is cleared first, then checked (`MatrixDiag: incompatable matrices
/// ...` and `Null` when it is not of that size: cleared all the same). FS-25.
pub fn matrix_diag(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_diag: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let (rows, cols) = (m_diag.rows, m_diag.cols);
    let nout = rows.max(cols);
    if real_only(g, err, "MatrixDiag", &[m_diag]) {
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let m: &mut Matrix = match m_out {
        None => match matrix_alloc(g, out, err, nout, nout, m_diag.mtype)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
        Some(m) => {
            if real_only(g, err, "MatrixDiag", &[m]) {
                return Ok(Ret::Null);
            }
            matrix_clear(g, m);
            m
        }
    };
    if nout != m.rows || nout != m.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MatrixDiag: incompatable matrices {nout} x {nout} != {} x {}\n",
                m.rows, m.cols
            ),
        );
        return Ok(Ret::Null);
    }
    if rows != 1 {
        for row in 1..=rows {
            let v = read_relt(g, m_diag, row, 1);
            write_relt(g, m, row, row, v);
        }
    } else {
        for col in 1..=cols {
            let v = read_relt(g, m_diag, 1, col);
            write_relt(g, m, col, col, v);
        }
    }
    Ok(done(fresh))
}

/// Which region copy: `MatrixCopyRegion`, `MatrixCopyRealRegion` or `MatrixCopyImagRegion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    All,
    Real,
    Imag,
}

impl Region {
    fn name(self) -> &'static str {
        match self {
            Region::All => "MatrixCopyRegion",
            Region::Real => "MatrixCopyRealRegion",
            Region::Imag => "MatrixCopyImagRegion",
        }
    }
}

/// `MatrixCopyRegion` (`which` [`Region::All`]), `MatrixCopyRealRegion` ([`Region::Real`]) and
/// `MatrixCopyImagRegion` ([`Region::Imag`]) `(mSrc, mDst, start_row, start_col, rows, cols,
/// dest_row, dest_col)`: the `rows` x `cols` region of `mSrc` from `(start_row, start_col)` into
/// `mDst` from `(dest_row, dest_col)` (a new `rows` x `cols` matrix of `mSrc`'s type when there is
/// none). `Null` after `<name>: bad destination (r,c)` for a destination before `(1, 1)`, `<name>:
/// bad source region (r,c) --> (r,c)` for a region outside the source, `<name>: destination
/// matrix not large enough (rxc)` for a destination with fewer rows or columns than the region.
/// What is copied depends on the destination's type: a real destination takes the source's
/// element (`All`), real part (`Real`) or imaginary part (`Imag`); a complex one the element
/// (`All`) or the imaginary part (`Real` and `Imag`: FS-26). FS-27, FS-28.
#[allow(clippy::too_many_arguments)]
pub fn matrix_copy_region(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    which: Region,
    m_src: &Matrix,
    m_dst: Option<&mut Matrix>,
    start_row: i32,
    start_col: i32,
    rows: i32,
    cols: i32,
    dest_row: i32,
    dest_col: i32,
) -> Result<Ret<Matrix>, Fatal> {
    let name = which.name();
    if dest_col < 1 || dest_row < 1 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{name}: bad destination ({dest_row},{dest_col})\n"),
        );
        return Ok(Ret::Null);
    }
    let (srows, scols) = (m_src.rows, m_src.cols);
    let end_row = start_row.wrapping_add(rows).wrapping_sub(1);
    let end_col = start_col.wrapping_add(cols).wrapping_sub(1);
    if start_row < 1
        || start_row > srows
        || start_col < 1
        || start_col > scols
        || end_row > srows
        || end_col > scols
    {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{name}: bad source region ({start_row},{start_col}) --> ({end_row},{end_col})\n"
            ),
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let Some(dst) = output(g, out, err, m_dst, &mut fresh, rows, cols, m_src.mtype)? else {
        // MatrixAlloc's NULL for an unknown type: FreeSurfer reads the NULL matrix's sizes and
        // crashes (never reproduced).
        return Ok(Ret::Null);
    };
    if rows > dst.rows || cols > dst.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{name}: destination matrix not large enough ({rows}x{cols})\n"),
        );
        return Ok(Ret::Null);
    }
    let nonempty = rows > 0 && cols > 0;
    let last_row = i64::from(dest_row) + i64::from(rows) - 1;
    let last_col = i64::from(dest_col) + i64::from(cols) - 1;
    if nonempty
        && (last_row > i64::from(dst.rows) || last_col > i64::from(dst.cols))
        && unfit(
            g,
            err,
            name,
            &format!(
                "the region ({dest_row},{dest_col}) --> ({last_row},{last_col}) outside the {} x {} destination",
                dst.rows, dst.cols
            ),
        )
    {
        return Ok(Ret::Null);
    }
    let fix = !g.notices.reproduce();
    let src_real = m_src.mtype == MATRIX_REAL;
    let dst_known = dst.mtype == MATRIX_REAL || dst.mtype == MATRIX_COMPLEX;
    let dst_complex = dst.mtype == MATRIX_COMPLEX;
    // FS-27: a source read through the other type's layout.
    let mixed = dst_known
        && match which {
            Region::All => m_src.mtype != dst.mtype,
            Region::Real | Region::Imag => src_real,
        };
    if mixed && nonempty {
        if which == Region::All && fix {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("{name}: a region between a real and a complex matrix\n"),
            );
            g.notices.add(
                &FS_27,
                format!("{name}: a region between a real and a complex matrix refused"),
            );
            return Ok(Ret::Null);
        }
        g.notices.add(
            &FS_27,
            if fix {
                format!("{name}: a real source's values read as its real parts")
            } else {
                format!("{name}: the source read through the other type's layout, as FreeSurfer")
            },
        );
    }
    if which == Region::Real && dst_complex && nonempty {
        g.notices.add(
            &FS_26,
            if fix {
                "MatrixCopyRealRegion into a complex matrix: the real parts copied"
            } else {
                "MatrixCopyRealRegion into a complex matrix: the imaginary parts copied, as FreeSurfer"
            },
        );
    }
    let real_src_fix = fix && src_real && which != Region::All;
    // the source's parts as FreeSurfer reads them (MATRIX_CELT), or (value, 0) of a real source
    // by default (FS-27)
    let parts = |g: &mut Globals, r: i32, c: i32| -> (f32, f32) {
        if real_src_fix {
            (read_relt(g, m_src, r, c), 0.0)
        } else {
            celt(g, m_src, r, c)
        }
    };
    let mut drow = dest_row;
    for srow in start_row..=end_row {
        let mut dcol = dest_col;
        for scol in start_col..=end_col {
            match (which, dst.mtype) {
                (Region::All, MATRIX_REAL) => {
                    let v = read_relt(g, m_src, srow, scol);
                    write_relt(g, dst, drow, dcol, v);
                }
                (Region::All, MATRIX_COMPLEX) => {
                    let (re, im) = celt(g, m_src, srow, scol);
                    set_celt(g, dst, drow, dcol, re, im);
                }
                (Region::Real, MATRIX_REAL) => {
                    let (re, _) = parts(g, srow, scol);
                    write_relt(g, dst, drow, dcol, re);
                }
                (Region::Imag, MATRIX_REAL) => {
                    let (_, im) = parts(g, srow, scol);
                    write_relt(g, dst, drow, dcol, im);
                }
                (Region::Real, MATRIX_COMPLEX) => {
                    let (re, im) = parts(g, srow, scol);
                    if fix {
                        set_celt_real(g, dst, drow, dcol, re);
                    } else {
                        set_celt_imag(g, dst, drow, dcol, im);
                    }
                }
                (Region::Imag, MATRIX_COMPLEX) => {
                    let (_, im) = parts(g, srow, scol);
                    set_celt_imag(g, dst, drow, dcol, im);
                }
                _ => {}
            }
            dcol = dcol.wrapping_add(1);
        }
        drow = drow.wrapping_add(1);
    }
    Ok(done(fresh))
}

/// `MatrixRealToComplex(mReal, mImag, mOut)`: the complex matrix of `mReal`'s size with real
/// parts `mReal` and imaginary parts `mImag` (a new one when there is no output). FS-25; FS-28:
/// an `mImag` or `mOut` smaller than `mReal`, or an output that is not complex.
pub fn matrix_real_to_complex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_real: &Matrix,
    m_imag: &Matrix,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let (rows, cols) = (m_real.rows, m_real.cols);
    if real_only(g, err, "MatrixRealToComplex", &[m_real, m_imag]) {
        return Ok(Ret::Null);
    }
    if (m_imag.rows < rows || m_imag.cols < cols)
        && unfit(
            g,
            err,
            "MatrixRealToComplex",
            &format!(
                "a {} x {} imaginary part for a {rows} x {cols} real part",
                m_imag.rows, m_imag.cols
            ),
        )
    {
        return Ok(Ret::Null);
    }
    let given = m_out.is_some();
    let mut fresh = None;
    let Some(m) = output(g, out, err, m_out, &mut fresh, rows, cols, MATRIX_COMPLEX)? else {
        return Ok(Ret::Null);
    };
    if given
        && (m.mtype != MATRIX_COMPLEX || m.rows < rows || m.cols < cols)
        && unfit(
            g,
            err,
            "MatrixRealToComplex",
            &format!(
                "a {} x {} {} output for a {rows} x {cols} complex matrix",
                m.rows,
                m.cols,
                if m.mtype == MATRIX_COMPLEX {
                    "complex"
                } else {
                    "real"
                }
            ),
        )
    {
        return Ok(Ret::Null);
    }
    for row in 1..=rows {
        for col in 1..=cols {
            let re = read_relt(g, m_real, row, col);
            set_celt_real(g, m, row, col, re);
            let im = read_relt(g, m_imag, row, col);
            set_celt_imag(g, m, row, col, im);
        }
    }
    Ok(done(fresh))
}

/// `MatrixIsSymmetric(matrix)`: 1 when square and `m[r][c] == m[c][r]` everywhere (`float`
/// comparisons: a NaN makes it 0), else 0. FS-25 (by default a complex matrix gives 0 after the
/// error).
pub fn matrix_is_symmetric(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> i32 {
    if m.rows != m.cols {
        return 0;
    }
    if real_only(g, err, "MatrixIsSymmetric", &[m]) {
        return 0;
    }
    let mut sym = 1;
    for row in 1..=m.rows {
        for col in 1..=m.cols {
            if read_relt(g, m, row, col) != read_relt(g, m, col, row) {
                sym = 0;
            }
        }
    }
    sym
}

/// `MatrixReallocRotation(n, angle, which, m)`: the rotation by `angle` (radians) about the x
/// ([`X_ROTATION`]), y or z axis written into `m` (a new `n` x `n` identity when there is none):
/// `c = cos(angle)`, `s = sin(angle)` (C++'s `float` overloads: `cosf`, `sinf`); another
/// `which` leaves it unchanged. Only the four elements of the rotation are written (a given `m`
/// keeps its other elements). FS-25.
pub fn matrix_realloc_rotation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    n: i32,
    angle: f32,
    which: i32,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m: &mut Matrix = match m {
        None => match matrix_identity(g, out, err, n, None)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(Ret::Null),
        },
        Some(m) => {
            if real_only(g, err, "MatrixReallocRotation", &[m]) {
                return Ok(Ret::Null);
            }
            m
        }
    };
    // C++'s float overloads: cosf and sinf
    let libm = Libm::new(g.mode());
    let c = libm.cosf(angle);
    let s = libm.sinf(angle);
    cstd::note_glibc07(g, &libm, "cosf and sinf");
    let cells: [(i32, i32, f32); 4] = match which {
        X_ROTATION => [(2, 2, c), (2, 3, s), (3, 2, -s), (3, 3, c)],
        Y_ROTATION => [(1, 1, c), (1, 3, -s), (3, 1, s), (3, 3, c)],
        Z_ROTATION => [(1, 1, c), (1, 2, s), (2, 1, -s), (2, 2, c)],
        _ => return Ok(done(fresh)),
    };
    for (r, col, v) in cells {
        write_relt(g, m, r, col, v);
    }
    Ok(done(fresh))
}

/// `MatrixAllocRotation(n, angle, which)`.
pub fn matrix_alloc_rotation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    n: i32,
    angle: f32,
    which: i32,
) -> Result<Ret<Matrix>, Fatal> {
    matrix_realloc_rotation(g, out, err, n, angle, which, None)
}

/// `VectorLen(v)`: `sqrt` of the `float` sum of the squares of the first column (rounded to
/// `float` at each step), as `float`. FS-25 (0 by default for a complex vector).
pub fn vector_len(g: &mut Globals, err: &mut dyn Write, v: &Matrix) -> f32 {
    if real_only(g, err, "VectorLen", &[v]) {
        return 0.0;
    }
    let mut len = 0.0f32;
    for i in 1..=v.rows {
        let vi = read_relt(g, v, i, 1);
        len = sse::add(len, sse::mul(vi, vi));
    }
    sse::sqrt(len)
}

/// `VectorDot(v1, v2)`: the `float` sum of the products of the first columns, over `v1`'s rows.
/// FS-25; FS-31 (by default 0 after the error for a `v2` with fewer rows).
pub fn vector_dot(g: &mut Globals, err: &mut dyn Write, v1: &Matrix, v2: &Matrix) -> f32 {
    if real_only(g, err, "VectorDot", &[v1, v2]) {
        return 0.0;
    }
    if v2.rows < v1.rows
        && short_vector(
            g,
            err,
            "VectorDot",
            &format!(
                "a {}-element vector with a {}-element one",
                v1.rows, v2.rows
            ),
        )
    {
        return 0.0;
    }
    let mut dot = 0.0f32;
    for i in 1..=v1.rows {
        let (a, b) = (read_relt(g, v1, i, 1), read_relt(g, v2, i, 1));
        dot = sse::add(dot, sse::mul(a, b));
    }
    dot
}

/// `VectorDistance(v1, v2)`: the `float` sum of the squared differences of the first columns
/// over `v1`'s rows: the *squared* Euclidean distance, whatever the name says. FS-25; FS-31.
pub fn vector_distance(g: &mut Globals, err: &mut dyn Write, v1: &Matrix, v2: &Matrix) -> f32 {
    if real_only(g, err, "VectorDistance", &[v1, v2]) {
        return 0.0;
    }
    if v2.rows < v1.rows
        && short_vector(
            g,
            err,
            "VectorDistance",
            &format!(
                "a {}-element vector with a {}-element one",
                v1.rows, v2.rows
            ),
        )
    {
        return 0.0;
    }
    let mut dist = 0.0f32;
    for row in 1..=v1.rows {
        let d = sse::sub(read_relt(g, v1, row, 1), read_relt(g, v2, row, 1));
        dist = sse::add(dist, sse::mul(d, d));
    }
    dist
}

/// `MatrixIsZero(m)`: 0 at the first element (row by row) whose magnitude exceeds `1e-11`, else
/// 1 (a NaN counts as zero). FS-25 (by default a complex matrix gives 0 after the error).
pub fn matrix_is_zero(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> i32 {
    if real_only(g, err, "MatrixIsZero", &[m]) {
        return 0;
    }
    for row in 1..=m.rows {
        for col in 1..=m.cols {
            let val = f64::from(read_relt(g, m, row, col)).abs();
            if val > 1e-11 {
                return 0;
            }
        }
    }
    1
}

/// The test `VectorCrossProduct` and `VectorTripleProduct` make (`v1->rows != 3 && v1->cols !=
/// 1` refuses), then FS-25's and FS-31's for the vectors FreeSurfer does not test: `true` to
/// stop.
fn three_vectors(
    g: &mut Globals,
    err: &mut dyn Write,
    func: &str,
    v1: &Matrix,
    others: &[&Matrix],
) -> bool {
    if v1.rows != 3 && v1.cols != 1 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "VectorCrossProduct: must be 3-vectors",
        );
        return true;
    }
    let all: Vec<&Matrix> = std::iter::once(v1).chain(others.iter().copied()).collect();
    if real_only(g, err, func, &all) {
        return true;
    }
    if let Some(v) = all.iter().find(|v| v.rows < 3 || v.cols < 1) {
        return short_vector(
            g,
            err,
            func,
            &format!("a {} x {} vector for a 3-vector", v.rows, v.cols),
        );
    }
    false
}

/// `VectorCrossProduct(v1, v2, vdst)`: `v1 x v2` in `float` into `vdst` (a zeroed copy of `v1`
/// when there is none: `VectorClone`); `Null` after `VectorCrossProduct: must be 3-vectors` when
/// `v1` has neither 3 rows nor 1 column. FS-25, FS-31.
pub fn vector_cross_product(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v1: &Matrix,
    v2: &Matrix,
    vdst: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if three_vectors(g, err, "VectorCrossProduct", v1, &[v2]) {
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let d: &mut Matrix = match vdst {
        Some(d) => {
            if real_only(g, err, "VectorCrossProduct", &[d]) {
                return Ok(Ret::Null);
            }
            if (d.rows < 3 || d.cols < 1)
                && short_vector(
                    g,
                    err,
                    "VectorCrossProduct",
                    &format!("a {} x {} output for a 3-vector", d.rows, d.cols),
                )
            {
                return Ok(Ret::Null);
            }
            d
        }
        None => match matrix_clone(g, out, err, v1)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    let e = |g: &mut Globals, v: &Matrix, i: i32| read_relt(g, v, i, 1);
    let (x1, y1, z1) = (e(g, v1, 1), e(g, v1, 2), e(g, v1, 3));
    let (x2, y2, z2) = (e(g, v2, 1), e(g, v2, 2), e(g, v2, 3));
    let cross = |a: f32, b: f32, c: f32, d: f32| sse::sub(sse::mul(a, b), sse::mul(c, d));
    write_relt(g, d, 1, 1, cross(y1, z2, z1, y2));
    write_relt(g, d, 2, 1, cross(z1, x2, x1, z2));
    write_relt(g, d, 3, 1, cross(x1, y2, y1, x2));
    Ok(done(fresh))
}

/// `VectorTripleProduct(v1, v2, v3)`: `(v1 x v2) . v3` in `float`, summed as FreeSurfer sums it
/// (`x3 (y1 z2 - z1 y2)`, then `+ y3 (...)`, then `+ z3 (...)`); 0 after `VectorCrossProduct:
/// must be 3-vectors` when `v1` has neither 3 rows nor 1 column. FS-25, FS-31.
pub fn vector_triple_product(
    g: &mut Globals,
    err: &mut dyn Write,
    v1: &Matrix,
    v2: &Matrix,
    v3: &Matrix,
) -> f32 {
    if three_vectors(g, err, "VectorTripleProduct", v1, &[v2, v3]) {
        return 0.0;
    }
    let e = |g: &mut Globals, v: &Matrix, i: i32| read_relt(g, v, i, 1);
    let (x1, y1, z1) = (e(g, v1, 1), e(g, v1, 2), e(g, v1, 3));
    let (x2, y2, z2) = (e(g, v2, 1), e(g, v2, 2), e(g, v2, 3));
    let (x3, y3, z3) = (e(g, v3, 1), e(g, v3, 2), e(g, v3, 3));
    let cross = |a: f32, b: f32, c: f32, d: f32| sse::sub(sse::mul(a, b), sse::mul(c, d));
    let mut total = sse::mul(x3, cross(y1, z2, z1, y2));
    total = sse::add(total, sse::mul(y3, cross(z1, x2, x1, z2)));
    total = sse::add(total, sse::mul(z3, cross(x1, y2, y1, x2)));
    total
}

/// `Vector3Angle(v1, v2)`: the angle between two 3-vectors in `double`: the lengths from the
/// `float` elements in `double`, the dot product in `float` (`V3_DOT`), `acos(dot / norm)` with
/// the norm raised to `|dot|` when that is larger; 0 when the product of the lengths is below
/// `FLT_EPSILON`. FS-25, FS-31.
pub fn vector3_angle(g: &mut Globals, err: &mut dyn Write, v1: &Matrix, v2: &Matrix) -> f64 {
    if real_only(g, err, "Vector3Angle", &[v1, v2]) {
        return 0.0;
    }
    if let Some(v) = [v1, v2].into_iter().find(|v| v.rows < 3 || v.cols < 1)
        && short_vector(
            g,
            err,
            "Vector3Angle",
            &format!("a {} x {} vector for a 3-vector", v.rows, v.cols),
        )
    {
        return 0.0;
    }
    let e = |g: &mut Globals, v: &Matrix, i: i32| read_relt(g, v, i, 1);
    let (a1, b1, c1) = (e(g, v1, 1), e(g, v1, 2), e(g, v1, 3));
    let (a2, b2, c2) = (e(g, v2, 1), e(g, v2, 2), e(g, v2, 3));
    let len = |x: f32, y: f32, z: f32| {
        let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
        sse::sqrt64(sse::add64(
            sse::add64(sse::mul64(x, x), sse::mul64(y, y)),
            sse::mul64(z, z),
        ))
    };
    let (l1, l2) = (len(a1, b1, c1), len(a2, b2, c2));
    let mut norm = sse::mul64(l1, l2);
    if fzero(norm) {
        return 0.0;
    }
    let dot = f64::from(sse::add(
        sse::add(sse::mul(a1, a2), sse::mul(b1, b2)),
        sse::mul(c1, c2),
    ));
    if dot.abs() > norm.abs() {
        norm = dot.abs();
    }
    if dot > norm {
        larmorx_core::math::acos(1.0)
    } else {
        larmorx_core::math::acos(sse::div64(dot, norm))
    }
}

/// FreeSurfer's `XYZ`: three floats.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Xyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `XYZ_NORMALIZED_LOAD(xyz, &len, x, y, z)`: `len = sqrt(x² + y² + z²)` (in `double`, rounded
/// to `float`) and the unit vector `(x, y, z) * (1 / len)` in `float`, or `(1, 0, 0)` when the
/// length is 0. Returns `(xyz, len)`.
pub fn xyz_normalized_load(x: f32, y: f32, z: f32) -> (Xyz, f32) {
    let (dx, dy, dz) = (f64::from(x), f64::from(y), f64::from(z));
    let len = sse::sqrt64(sse::add64(
        sse::add64(sse::mul64(dx, dx), sse::mul64(dy, dy)),
        sse::mul64(dz, dz),
    )) as f32;
    if len == 0.0 {
        return (
            Xyz {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            len,
        );
    }
    let inv = sse::div(1.0, len);
    (
        Xyz {
            x: sse::mul(x, inv),
            y: sse::mul(y, inv),
            z: sse::mul(z, inv),
        },
        len,
    )
}

/// `XYZApproxAngle_knownLength(n, x2, y2, z2, norm)`: the angle between the unit vector `n` and
/// `(x2, y2, z2)` of length `norm`: the dot product in `double` rounded to `float`, the norm
/// raised to `|dot|`, then `acos(dot / norm)` below 0.99 and `sqrt(2 (1 - dot / norm))` (the
/// series' first term) from 0.99, as `float`. The products in the official build's operand order
/// (the unit oracle's `matrix.cpp.o`): `n.x x2`, `n.y y2`, but `z2 n.z` (it matters where both are
/// NaNs).
pub fn xyz_approx_angle_known_length(n: &Xyz, x2: f32, y2: f32, z2: f32, norm: f32) -> f32 {
    let (x1, y1, z1) = (f64::from(n.x), f64::from(n.y), f64::from(n.z));
    let dot = sse::add64(
        sse::add64(sse::mul64(x1, f64::from(x2)), sse::mul64(y1, f64::from(y2))),
        sse::mul64(f64::from(z2), z1),
    ) as f32;
    let mut norm = norm;
    if dot.abs() > norm {
        norm = dot.abs();
    }
    let acos_input = f64::from(sse::div(dot, norm));
    if acos_input < f64::from(0.99f32) {
        larmorx_core::math::acos(acos_input) as f32
    } else {
        sse::sqrt64(sse::mul64(2.0, sse::sub64(1.0, acos_input))) as f32
    }
}

/// `MatrixCheck(m)`: `ERROR_BADPARM` at the first element that is not finite, else `NO_ERROR`.
/// FS-25.
pub fn matrix_check(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> i32 {
    if real_only(g, err, "MatrixCheck", &[m]) {
        return ERROR_BADPARM;
    }
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            if !read_relt(g, m, r, c).is_finite() {
                return ERROR_BADPARM;
            }
        }
    }
    NO_ERROR
}

/// `MatrixReshape(m_src, m_dst, rows, cols)`: `m_src`'s elements, row by row, into `m_dst`
/// (whose own sizes replace `rows` and `cols` when given; a new `rows` x `cols` matrix of
/// `m_src`'s type otherwise), row by row, until `rows` x `cols` are placed or `m_src` ends (the
/// rest of `m_dst` unchanged). FS-25.
pub fn matrix_reshape(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_src: &Matrix,
    m_dst: Option<&mut Matrix>,
    rows: i32,
    cols: i32,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, "MatrixReshape", &[m_src]) {
        return Ok(Ret::Null);
    }
    let (rows, cols) = match &m_dst {
        Some(d) => (d.rows, d.cols),
        None => (rows, cols),
    };
    let given = m_dst.is_some();
    let mut fresh = None;
    let Some(d) = output(g, out, err, m_dst, &mut fresh, rows, cols, m_src.mtype)? else {
        return Ok(Ret::Null);
    };
    if given && real_only(g, err, "MatrixReshape", &[d]) {
        return Ok(Ret::Null);
    }
    let (mut r2, mut c2) = (1, 1);
    'rows: for r1 in 1..=m_src.rows {
        for c1 in 1..=m_src.cols {
            let v = read_relt(g, m_src, r1, c1);
            write_relt(g, d, r2, c2, v);
            c2 += 1;
            if c2 > d.cols {
                c2 = 1;
                r2 += 1;
            }
            if r2 > rows {
                break 'rows;
            }
        }
    }
    Ok(done(fresh))
}

/// `MatrixTrace(M)`: the `float` sum of the diagonal over `min(rows, cols)`. FS-25.
pub fn matrix_trace(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> f32 {
    if real_only(g, err, "MatrixTrace", &[m]) {
        return 0.0;
    }
    let nmax = if m.rows >= m.cols { m.cols } else { m.rows };
    let mut trace = 0.0f32;
    for n in 1..=nmax {
        trace = sse::add(trace, read_relt(g, m, n, n));
    }
    trace
}

/// `MatrixConstVal(val, rows, cols, X)`: every element `val` (a new real `rows` x `cols` matrix
/// when `X` is `None`; `X`'s own sizes otherwise). FS-25.
pub fn matrix_const_val(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    val: f32,
    rows: i32,
    cols: i32,
    x: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let given = x.is_some();
    let mut fresh = None;
    let Some(m) = output(g, out, err, x, &mut fresh, rows, cols, MATRIX_REAL)? else {
        return Ok(Ret::Null);
    };
    if given && real_only(g, err, "MatrixConstVal", &[m]) {
        return Ok(Ret::Null);
    }
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            write_relt(g, m, r, c, val);
        }
    }
    Ok(done(fresh))
}

/// `MatrixZero(rows, cols, X)`: [`matrix_const_val`] of 0.
pub fn matrix_zero(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    x: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    matrix_const_val(g, out, err, 0.0, rows, cols, x)
}

/// `MatrixClone(mat)` (`MatrixZero(0, 0, MatrixCopy(mat, NULL))`): a matrix of zeros of `mat`'s
/// size and type (`VectorClone` is the same). `None` where `MatrixCopy` returns `NULL` (an
/// unknown type). A complex `mat` gives its zeros by default; with `reproduce` the copy's floats
/// that FreeSurfer's `MatrixZero` does not reach stay (FS-14 of the copy, FS-25 of the zeros).
pub fn matrix_clone(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mat: &Matrix,
) -> Result<Option<Matrix>, Fatal> {
    let Some(mut c) = matrix::matrix_copy(g, out, err, Some(mat), None)?.into_new() else {
        return Ok(None);
    };
    if c.mtype == MATRIX_COMPLEX {
        if !g.notices.reproduce() {
            for v in c.data_mut() {
                *v = 0.0;
            }
            return Ok(Some(c));
        }
        g.notices.add(
            &FS_25,
            "MatrixClone of a complex matrix: zeros written as if real, as FreeSurfer",
        );
    }
    for r in 1..=c.rows {
        for col in 1..=c.cols {
            write_relt(g, &mut c, r, col, 0.0);
        }
    }
    Ok(Some(c))
}

/// `MatrixSumElts(m)`: the `double` sum of the elements, row by row. FS-25.
pub fn matrix_sum_elts(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> f64 {
    if real_only(g, err, "MatrixSumElts", &[m]) {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            sum = sse::add64(sum, f64::from(read_relt(g, m, r, c)));
        }
    }
    sum
}

/// `MatrixNormalizeColScale(m, scale)`: the `1` x `cols` row of each column's length (`sqrt` of
/// the `double` sum of squares, rounded to `float`), into `scale` (a new row when there is none).
/// FS-25; FS-28 (a given `scale` with fewer columns).
pub fn matrix_normalize_col_scale(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    scale: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    if real_only(g, err, "MatrixNormalizeColScale", &[m]) {
        return Ok(Ret::Null);
    }
    let given = scale.is_some();
    let mut fresh = None;
    let Some(s) = output(g, out, err, scale, &mut fresh, 1, m.cols, MATRIX_REAL)? else {
        return Ok(Ret::Null);
    };
    if given {
        if real_only(g, err, "MatrixNormalizeColScale", &[s]) {
            return Ok(Ret::Null);
        }
        if (s.rows < 1 || s.cols < m.cols)
            && unfit(
                g,
                err,
                "MatrixNormalizeColScale",
                &format!("a {} x {} scale for {} columns", s.rows, s.cols, m.cols),
            )
        {
            return Ok(Ret::Null);
        }
    }
    for c in 1..=m.cols {
        let mut sum2 = 0.0f64;
        for r in 1..=m.rows {
            let v = f64::from(read_relt(g, m, r, c));
            sum2 = sse::add64(sum2, sse::mul64(v, v));
        }
        write_relt(g, s, 1, c, sse::sqrt64(sum2) as f32);
    }
    Ok(done(fresh))
}

/// `MatrixNormalizeCol(m, mcnorm, scale)`: `m` with each column divided by its length (in
/// `double`; a column of length 0 becomes zeros) into `mcnorm` (a new matrix when there is none;
/// `ERROR: MatrixNormalizeCol: dimension mismatch` on standard output and `Null` for one of
/// another size); the lengths are left in `scale` when one is given. FS-25.
pub fn matrix_normalize_col(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    mcnorm: Option<&mut Matrix>,
    scale: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let n: &mut Matrix = match mcnorm {
        None => match matrix_alloc(g, out, err, m.rows, m.cols, MATRIX_REAL)? {
            Some(x) => fresh.insert(x),
            None => {
                let _ = writeln!(out, "ERROR: MatrixNormalizeCol: could not alloc");
                return Ok(Ret::Null);
            }
        },
        Some(x) => {
            if x.rows != m.rows || x.cols != m.cols {
                let _ = writeln!(out, "ERROR: MatrixNormalizeCol: dimension mismatch");
                return Ok(Ret::Null);
            }
            x
        }
    };
    if real_only(g, err, "MatrixNormalizeCol", &[m, n]) {
        return Ok(Ret::Null);
    }
    let lengths: Matrix = match scale {
        Some(s) => match matrix_normalize_col_scale(g, out, err, m, Some(s))? {
            Ret::Null => return Ok(Ret::Null),
            _ => s.clone(),
        },
        None => match matrix_normalize_col_scale(g, out, err, m, None)? {
            Ret::New(s) => s,
            _ => return Ok(Ret::Null),
        },
    };
    for c in 1..=m.cols {
        let v = f64::from(read_relt(g, &lengths, 1, c));
        for r in 1..=m.rows {
            let x = if v != 0.0 {
                sse::div64(f64::from(read_relt(g, m, r, c)), v) as f32
            } else {
                0.0
            };
            write_relt(g, n, r, c, x);
        }
    }
    Ok(done(fresh))
}

/// `MatrixSimilarityTransform(m_src, m_mul, m_dst)`: `m_mul * (m_src * m_mul^T)` with
/// `MatrixMultiply`'s `float` products; a size mismatch gives `MatrixMultiply`'s messages (the
/// second product of a failed first one ends the program: `m2 is null`).
pub fn matrix_similarity_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_src: &Matrix,
    m_mul: &Matrix,
    m_dst: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let m_mul_t = matrix_transpose(g, out, err, m_mul, None)?.into_new();
    let m_tmp =
        matrix::matrix_multiply(g, out, err, Some(m_src), m_mul_t.as_ref(), None)?.into_new();
    matrix::matrix_multiply(g, out, err, Some(m_mul), m_tmp.as_ref(), m_dst)
}

/// `GaussianMatrix(len, std, norm, G)`: the `len` x `len` matrix whose row `r` is the Gaussian
/// `exp(-d² / (2 std²)) / (sqrt(2 pi) std)` of `d = c - r` (`float` arithmetic; C++'s `float`
/// overload of `exp`, `expf`); `norm` 1 scales it so that the centre row (`d = c - len / 2`) sums to 1 (FreeSurfer
/// says of its own code that it meant every row), `norm` 2 makes every row sum to 1 (in `float`).
/// A given `G` of another size: `ERROR: GaussianMatrix: dimension mismatch` on standard output and
/// `Null`.
pub fn gaussian_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    len: i32,
    std: f32,
    norm: i32,
    gm: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m: &mut Matrix = match gm {
        None => match matrix_alloc(g, out, err, len, len, MATRIX_REAL)? {
            Some(x) => fresh.insert(x),
            None => return Ok(Ret::Null),
        },
        Some(x) => {
            if x.rows != len || x.cols != len {
                let _ = writeln!(out, "ERROR: GaussianMatrix: dimension mismatch");
                return Ok(Ret::Null);
            }
            if real_only(g, err, "GaussianMatrix", &[x]) {
                return Ok(Ret::Null);
            }
            x
        }
    };
    let libm = Libm::new(g.mode());
    let var = sse::mul(std, std);
    let mut f = sse::mul64((2.0 * std::f64::consts::PI).sqrt(), f64::from(std)) as f32;
    // C++'s float overload of exp: expf of the float argument, then a float division
    let gauss =
        |d: f32, f: f32| -> f32 { sse::div(libm.expf(sse::div(-(d * d), sse::mul(2.0, var))), f) };
    if norm == 1 {
        let mut sum = 0.0f32;
        for c in 0..len {
            let d = (c - len / 2) as f32;
            sum = sse::add(sum, gauss(d, f));
        }
        f = sse::div(f, sum);
    }
    for r in 0..len {
        let mut sum = 0.0f32;
        for c in 0..len {
            let d = (c - r) as f32;
            let v = gauss(d, f);
            sum = sse::add(sum, v);
            write_relt(g, m, r + 1, c + 1, v);
        }
        if norm == 2 {
            for c in 0..len {
                let v = sse::div(read_relt(g, m, r + 1, c + 1), sum);
                write_relt(g, m, r + 1, c + 1, v);
            }
        }
    }
    cstd::note_glibc07(g, &libm, "expf");
    Ok(done(fresh))
}

/// `GaussianMatrix2(len, std1, std2, w1, norm, G)`: `w1 G1 + (1 - w1) G2` of the two
/// [`gaussian_matrix`]es (`float` products and sum), into `G` (`MatrixAdd`'s messages for one of
/// another size).
#[allow(clippy::too_many_arguments)]
pub fn gaussian_matrix2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    len: i32,
    std1: f32,
    std2: f32,
    w1: f32,
    norm: i32,
    gm: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let Some(mut g1) = gaussian_matrix(g, out, err, len, std1, norm, None)?.into_new() else {
        return Ok(Ret::Null);
    };
    matrix_scalar_mul_in_place(g, out, err, &mut g1, w1)?;
    let Some(mut g2) = gaussian_matrix(g, out, err, len, std2, norm, None)?.into_new() else {
        return Ok(Ret::Null);
    };
    matrix_scalar_mul_in_place(g, out, err, &mut g2, sse::sub(1.0, w1))?;
    matrix_add(g, out, err, &g1, &g2, gm)
}

/// `MatrixDeterminant(mIn)`: [`numerics::open_matrix_determinant`] (0 for a matrix that is not
/// square). FS-25, FS-33.
pub fn matrix_determinant(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> f32 {
    if real_only(g, err, "MatrixDeterminant", &[m]) {
        return 0.0;
    }
    numerics::open_matrix_determinant(g, m)
}

/// `MatrixQRdecomposition(iMatrix, oQ, oR)`: [`numerics::open_qr_decomposition`] (0, or 1 after
/// its line on standard output). FS-25.
pub fn matrix_qr_decomposition(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    i_matrix: Option<&Matrix>,
    o_q: Option<&mut Matrix>,
    o_r: Option<&mut Matrix>,
) -> i32 {
    let mut all: Vec<&Matrix> = Vec::new();
    all.extend(i_matrix);
    all.extend(o_q.as_deref());
    all.extend(o_r.as_deref());
    if real_only(g, err, "MatrixQRdecomposition", &all) {
        return 1;
    }
    numerics::open_qr_decomposition(out, i_matrix, o_q, o_r)
}

/// `MatrixInverse(mIn, mOut)`: the inverse of the square `mIn` into `mOut` (a new matrix of its
/// size and type when there is none). `mIn` absent ends the program (`MatrixInverse: NULL input
/// matrix!`, code -4); a matrix that is not square gives `MatrixInverse: matrix (r x c) is not
/// square` and `Null`. A real matrix: [`numerics::open_lu_matrix_inverse`] of a copy; its error
/// gives `Null`. A complex `A + iC`: the inverse of the real `2n` x `2n` matrix `[A -C; C A]`
/// (built with the region copies, FS-26 and FS-27 not reached), whose left blocks are the real and
/// imaginary parts. Then `Null` when an element of the result is not finite (a given `mOut` keeps
/// what was written). FS-28 (a given `mOut` of another size or type), FS-33.
pub fn matrix_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: Option<&Matrix>,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let Some(m_in) = m_in else {
        log::error_exit(g, err, ERROR_BADPARM, "MatrixInverse: NULL input matrix!\n")?;
        return Ok(Ret::Null);
    };
    if m_in.rows != m_in.cols {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MatrixInverse: matrix ({} x {}) is not square\n",
                m_in.rows, m_in.cols
            ),
        );
        return Ok(Ret::Null);
    }
    let (rows, cols) = (m_in.rows, m_in.cols);
    let given = m_out.is_some();
    let mut fresh = None;
    let Some(m) = output(g, out, err, m_out, &mut fresh, rows, cols, m_in.mtype)? else {
        return Ok(Ret::Null);
    };
    if given
        && (m.rows != rows || m.cols != cols || m.mtype != m_in.mtype)
        && unfit(
            g,
            err,
            "MatrixInverse",
            &format!(
                "a {} x {} output of type {} for the inverse of a {rows} x {cols} matrix of type {}",
                m.rows, m.cols, m.mtype, m_in.mtype
            ),
        )
    {
        return Ok(Ret::Null);
    }
    if m_in.mtype == MATRIX_COMPLEX {
        let (r, c) = (rows, cols);
        let mut tmp = Matrix::real(2 * r, 2 * c);
        matrix_copy_region(
            g,
            out,
            err,
            Region::Real,
            m_in,
            Some(&mut tmp),
            1,
            1,
            r,
            c,
            1,
            1,
        )?;
        matrix_copy_region(
            g,
            out,
            err,
            Region::Real,
            m_in,
            Some(&mut tmp),
            1,
            1,
            r,
            c,
            r + 1,
            c + 1,
        )?;
        let mut quad = Matrix::real(r, c);
        matrix_copy_region(
            g,
            out,
            err,
            Region::Imag,
            m_in,
            Some(&mut quad),
            1,
            1,
            r,
            c,
            1,
            1,
        )?;
        matrix_scalar_mul_in_place(g, out, err, &mut quad, -1.0)?;
        matrix_copy_region(
            g,
            out,
            err,
            Region::All,
            &quad,
            Some(&mut tmp),
            1,
            1,
            r,
            c,
            1,
            c + 1,
        )?;
        matrix_copy_region(
            g,
            out,
            err,
            Region::Imag,
            m_in,
            Some(&mut tmp),
            1,
            1,
            r,
            c,
            c + 1,
            1,
        )?;
        let mut re = Matrix::real(r, c);
        let mut im = Matrix::real(r, c);
        let Some(inv) = matrix_inverse(g, out, err, Some(&tmp), None)?.into_new() else {
            // FreeSurfer copies from the NULL inverse and crashes (never reproduced)
            g.notices.add(
                &FS_33,
                "MatrixInverse of a singular complex matrix: NULL (FreeSurfer crashes)",
            );
            return Ok(Ret::Null);
        };
        matrix_copy_region(
            g,
            out,
            err,
            Region::All,
            &inv,
            Some(&mut re),
            1,
            1,
            r,
            c,
            1,
            1,
        )?;
        matrix_copy_region(
            g,
            out,
            err,
            Region::All,
            &inv,
            Some(&mut im),
            r + 1,
            1,
            r,
            c,
            1,
            1,
        )?;
        matrix_real_to_complex(g, out, err, &re, &im, Some(&mut *m))?;
    } else {
        let tmp = m_in.clone();
        if numerics::open_lu_matrix_inverse(g, err, &tmp, m) < 0 {
            return Ok(Ret::Null);
        }
    }
    for j in 1..=rows {
        for i in 1..=rows {
            let finite = match m.mtype {
                MATRIX_REAL => read_relt(g, m, i, j).is_finite(),
                MATRIX_COMPLEX => {
                    let (re, im) = celt(g, m, i, j);
                    re.is_finite() && im.is_finite()
                }
                _ => true,
            };
            if !finite {
                return Ok(Ret::Null);
            }
        }
    }
    Ok(done(fresh))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn gl(mode: UpstreamDefects) -> Globals {
        Globals::new(mode)
    }

    #[test]
    fn print_formats() {
        let mut g = gl(UpstreamDefects::Fix);
        let m = Matrix::from_rows(&[vec![1.0, -2.5], vec![0.125, 3.0]]);
        let (mut o, mut e, mut f) = (Vec::new(), Vec::new(), Vec::new());
        assert_eq!(
            matrix_print(&mut g, Some(&mut f), &mut o, &mut e, Some(&m), 5),
            NO_ERROR
        );
        assert_eq!(
            String::from_utf8(f).unwrap(),
            " 1.00000  -2.50000;\n 0.12500   3.00000;\n"
        );
        let mut f = Vec::new();
        matrix_print_fmt(&mut g, Some(&mut f), &mut o, &mut e, "%10f", Some(&m));
        assert_eq!(
            String::from_utf8(f).unwrap(),
            "  1.000000   -2.500000\n  0.125000    3.000000\n"
        );
        assert_eq!(
            matrix_print(&mut g, None, &mut o, &mut e, None, 5),
            ERROR_BADPARM
        );
    }

    #[test]
    fn elementwise_and_complex_refusal() {
        let mut g = gl(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = Matrix::from_rows(&[vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        let t = matrix_transpose(&mut g, &mut o, &mut e, &a, None)
            .unwrap()
            .unwrap_new();
        assert_eq!((t.rows, t.cols, t.relt(3, 2)), (3, 2, 6.0));
        let s = matrix_add(&mut g, &mut o, &mut e, &a, &a, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(s.relt(2, 3), 12.0);
        let c = matrix_alloc(&mut g, &mut o, &mut e, 2, 2, MATRIX_COMPLEX)
            .unwrap()
            .unwrap();
        assert!(
            matrix_transpose(&mut g, &mut o, &mut e, &c, None)
                .unwrap()
                .is_null()
        );
        assert!(g.notices.contains(&FS_25));
        let mut g = gl(UpstreamDefects::Reproduce);
        assert!(
            !matrix_transpose(&mut g, &mut o, &mut e, &c, None)
                .unwrap()
                .is_null()
        );
    }

    #[test]
    fn regions_fs26_fs27_fs28() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = gl(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut c = matrix_alloc(&mut g, &mut o, &mut e, 2, 2, MATRIX_COMPLEX)
                .unwrap()
                .unwrap();
            c.set_celt(1, 1, 5.0, 7.0);
            let mut d = matrix_alloc(&mut g, &mut o, &mut e, 2, 2, MATRIX_COMPLEX)
                .unwrap()
                .unwrap();
            let r = matrix_copy_region(
                &mut g,
                &mut o,
                &mut e,
                Region::Real,
                &c,
                Some(&mut d),
                1,
                1,
                1,
                1,
                1,
                1,
            )
            .unwrap();
            assert_eq!(r, Ret::Given);
            assert!(g.notices.contains(&FS_26));
            if mode.reproduce() {
                assert_eq!(d.celt(1, 1), (0.0, 7.0));
            } else {
                assert_eq!(d.celt(1, 1), (5.0, 0.0));
            }
            let v = Matrix::from_rows(&[vec![1.0], vec![2.0], vec![3.0]]);
            let w = matrix_copy_region(
                &mut g,
                &mut o,
                &mut e,
                Region::Real,
                &v,
                None,
                1,
                1,
                3,
                1,
                1,
                1,
            )
            .unwrap()
            .unwrap_new();
            assert!(g.notices.contains(&FS_27));
            if mode.reproduce() {
                assert_eq!(w.data(), &[2.0, 3.0, 0.0]);
            } else {
                assert_eq!(w.data(), v.data());
            }
            let mut small = Matrix::real(2, 2);
            let r = matrix_copy_region(
                &mut g,
                &mut o,
                &mut e,
                Region::All,
                &v,
                Some(&mut small),
                1,
                1,
                2,
                1,
                2,
                1,
            )
            .unwrap();
            assert!(g.notices.contains(&FS_28));
            assert_eq!(r.is_null(), !mode.reproduce());
        }
    }

    #[test]
    fn vectors_and_angles() {
        let mut g = gl(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let x = Matrix::from_rows(&[vec![1.0], vec![0.0], vec![0.0]]);
        let y = Matrix::from_rows(&[vec![0.0], vec![1.0], vec![0.0]]);
        let z = vector_cross_product(&mut g, &mut o, &mut e, &x, &y, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(z.data(), &[0.0, 0.0, 1.0]);
        assert_eq!(vector_triple_product(&mut g, &mut e, &x, &y, &z), 1.0);
        let a = vector3_angle(&mut g, &mut e, &x, &y);
        assert_eq!(a, std::f64::consts::FRAC_PI_2);
        let two = Matrix::from_rows(&[vec![1.0], vec![2.0]]);
        assert!(
            vector_cross_product(&mut g, &mut o, &mut e, &two, &y, None)
                .unwrap()
                .is_null()
        );
        assert!(g.notices.contains(&FS_31));
        let (n, len) = xyz_normalized_load(3.0, 0.0, 4.0);
        assert_eq!(len, 5.0);
        assert_eq!(xyz_approx_angle_known_length(&n, 3.0, 0.0, 4.0, 5.0), 0.0);
        // two NaNs meet in the third product: z2's comes through (the official build's order)
        let nz = Xyz {
            x: 0.0,
            y: 0.0,
            z: f32::from_bits(0x7fc0_0001),
        };
        let a = xyz_approx_angle_known_length(&nz, 1.0, 0.0, f32::from_bits(0xffc0_0002), 1.0);
        assert_eq!(a.to_bits(), 0xffc0_0002);
    }

    #[test]
    fn gaussians_and_rotation() {
        let mut g = gl(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = gaussian_matrix(&mut g, &mut o, &mut e, 5, 1.0, 2, None)
            .unwrap()
            .unwrap_new();
        let s: f32 = (1..=5).map(|c| m.relt(3, c)).sum();
        assert!((s - 1.0).abs() < 1e-6);
        let r = matrix_alloc_rotation(&mut g, &mut o, &mut e, 4, 0.0, Z_ROTATION)
            .unwrap()
            .unwrap_new();
        assert_eq!(r.relt(1, 1), 1.0);
        assert_eq!(r.relt(1, 2), 0.0);
    }
}

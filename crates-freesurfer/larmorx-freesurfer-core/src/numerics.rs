// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's numerics wrappers over VXL (`fs.maths`, queue items 3.1-23 and 3.1-24),
//! translated from FreeSurfer 8.2.0's `utils/numerics.cpp`: the conversions between NR's 1-based
//! `float` arrays and VNL's `double` ones (69-110), `OpenDFPMin` (846-926), `OpenPowell`
//! (980-1019), `OpenPowell2` (1037-1089), `OpenLUMatrixInverse` (1091-1153),
//! `OpenQRdecomposition` (1155-1196), `OpenMatrixDeterminant` (1198-1219) and Numerical Recipes'
//! allocators `vector` and `matrix` (1685-1727: [`NrVector`], [`NrMatrix`]; `free_vector` and
//! `free_matrix` are their drop). The VXL routines they call are the main package's ports of ITK
//! 4.13.2's VNL (`larmorx_core::vnl_fixed`: `vnl_det`, `vnl_inverse`, `vnl_determinant`;
//! `larmorx_core::vnl_qr::Qr<f32>`: `vnl_qr<float>`; `larmorx_core::vnl_minimize` and
//! `vnl_lbfgs` under [`crate::fs_vnl`]), queue item 3.1-07; this crate contains no VXL code.
//!
//! `numerics.cpp`'s GSL functions (`sc_*`) are queue item 3.1-53 (`larmorx-gpl`) and never come
//! here; Burkardt's spline (`SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`) is item 3.1-54.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-maths.md`): **FS-33**, the 0 x 0
//! matrix (see [`open_lu_matrix_inverse`] and [`open_matrix_determinant`]); **FS-34**, the
//! directions written through `NULL` ([`open_powell`], [`open_powell2`]); **FS-35** and **FS-36**
//! (kept), the Powell wrappers' return codes and tolerance; **FS-38** (kept until measured) and
//! **FS-39**, [`open_dfp_min`]'s passes and its evaluation limit.

use std::io::Write;
use std::ops::{Index, IndexMut};

use larmorx_core::defects::{FS_33, FS_34, FS_39};
use larmorx_core::vnl_minimize::ReturnCode;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};

use crate::fs_vnl::{FsCostFunction, FsFunctions, FsLbfgs, FsLbfgsObserver, FsPowell};
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use crate::matrix::Matrix;

/// Numerical Recipes' `NR_END`: the one extra float `vector` and `matrix` allocate.
const NR_END: i64 = 1;

/// `float *vector(nl, nh)`: a `float` vector indexed `v[nl..=nh]` (its block has `NR_END` floats
/// more, at the start: `v[nl - 1]` is inside it). FreeSurfer's `malloc` leaves the values unset;
/// they are 0 here (every caller sets them before reading). `free_vector` is the drop.
#[derive(Clone, Debug, PartialEq)]
pub struct NrVector {
    nl: i64,
    data: Vec<f32>,
}

impl NrVector {
    /// `vector(nl, nh)` (allocation failure, FreeSurfer's `ErrorExit(ERROR_NOMEMORY, "could
    /// not allocate vector(%ld, %ld)")`, aborts the process here).
    pub fn new(nl: i64, nh: i64) -> NrVector {
        let len = (nh - nl + 1 + NR_END).max(0) as usize;
        NrVector {
            nl,
            data: vec![0.0; len],
        }
    }

    /// The block: `v[nl - NR_END ..= nh]`. For `nl = 1`, element `i` is `as_slice()[i]` (the
    /// 1-based arrays the minimizers take).
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// The block, writable.
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }
}

impl Index<i64> for NrVector {
    type Output = f32;
    fn index(&self, i: i64) -> &f32 {
        &self.data[(i - self.nl + NR_END) as usize]
    }
}

impl IndexMut<i64> for NrVector {
    fn index_mut(&mut self, i: i64) -> &mut f32 {
        &mut self.data[(i - self.nl + NR_END) as usize]
    }
}

/// `float **matrix(nrl, nrh, ncl, nch)`: a `float` matrix indexed `m[r][c]` for `r` in
/// `nrl..=nrh`, `c` in `ncl..=nch`, its rows one block (`NR_END` floats more, at the start), the
/// values 0 here (unset in FreeSurfer). Indexed by `(r, c)`. `free_matrix` is the drop.
#[derive(Clone, Debug, PartialEq)]
pub struct NrMatrix {
    nrl: i64,
    ncl: i64,
    ncol: i64,
    data: Vec<f32>,
}

impl NrMatrix {
    /// `matrix(nrl, nrh, ncl, nch)` (allocation failure, FreeSurfer's `ErrorExit`, aborts the
    /// process here).
    pub fn new(nrl: i64, nrh: i64, ncl: i64, nch: i64) -> NrMatrix {
        let (nrow, ncol) = (nrh - nrl + 1, nch - ncl + 1);
        let len = (nrow * ncol + NR_END).max(0) as usize;
        NrMatrix {
            nrl,
            ncl,
            ncol,
            data: vec![0.0; len],
        }
    }

    fn at(&self, r: i64, c: i64) -> usize {
        (NR_END + (r - self.nrl) * self.ncol + (c - self.ncl)) as usize
    }
}

impl Index<(i64, i64)> for NrMatrix {
    type Output = f32;
    fn index(&self, (r, c): (i64, i64)) -> &f32 {
        &self.data[self.at(r, c)]
    }
}

impl IndexMut<(i64, i64)> for NrMatrix {
    fn index_mut(&mut self, (r, c): (i64, i64)) -> &mut f32 {
        let i = self.at(r, c);
        &mut self.data[i]
    }
}

/// `ConvertFromFloatToVNLDouble(iaSource, oDestination, n)`: `iaSource[1..=n]` widened.
fn float_to_vnl(src: &[f32], n: usize) -> Vec<f64> {
    (0..n).map(|i| f64::from(src[i + 1])).collect()
}

/// `ConvertFromVNLDoubleToFloat(iSource, oaDestination, n)`: `oaDestination[i + 1] =
/// (float) iSource(i)` for `i < n`.
fn vnl_to_float(src: &[f64], dst: &mut [f32], n: usize) {
    for i in 0..n {
        dst[i + 1] = src[i] as f32;
    }
}

/// The matrix forms: `oDestination(row, col) = iaSource[row + 1][col + 1]` (row by row).
fn float_to_vnl_matrix(src: &NrMatrix, n: usize) -> Vec<f64> {
    let mut d = vec![0.0f64; n * n];
    for r in 0..n {
        for c in 0..n {
            d[r * n + c] = f64::from(src[(r as i64 + 1, c as i64 + 1)]);
        }
    }
    d
}

fn vnl_to_float_matrix(src: &[f64], dst: &mut NrMatrix, n: usize) {
    for r in 0..n {
        for c in 0..n {
            dst[(r as i64 + 1, c as i64 + 1)] = src[r * n + c] as f32;
        }
    }
}

/// The identity, row by row (`vnl_matrix_identity`).
fn identity(n: usize) -> Vec<f64> {
    let mut d = vec![0.0f64; n * n];
    for i in 0..n {
        d[i * n + i] = 1.0;
    }
    d
}

/// The final directions into the caller's matrix; **FS-34**: without one (`NULL`) FreeSurfer
/// writes through the null pointer and crashes; both modes write nothing.
fn put_directions(g: &mut Globals, src: &[f64], dst: Option<&mut NrMatrix>, n: usize, who: &str) {
    match dst {
        Some(d) => vnl_to_float_matrix(src, d, n),
        None => g.notices.add(
            &FS_34,
            format!(
                "{who} without initial directions: the final directions are not written (FreeSurfer writes them through the NULL pointer and crashes)"
            ),
        ),
    }
}

/// `OpenPowell(iaParams, ioInitialDirection, icParams, iTolerance, oIterations,
/// oFinalFunctionReturn, iFunction)`: [`FsPowell`] (VNL's defaults: `ftol` 1e-10, `maxfev` 2000,
/// `linmin_xtol` 1e-4) on `iFunction` from `iaParams[1..=icParams]` along the directions
/// `ioInitialDirection[1..=n][1..=n]` (the identity without them). **FS-36** (kept): the tolerance
/// sets VNL's `xtol`, which Powell never reads. A return code of too many iterations exits with
/// `powell exceeding maximum iterations.`, the error codes with `powell error.` (both
/// `ERROR_BADPARM`; **FS-35**: `fs_powell` returns neither, so they never happen); otherwise the
/// iteration count, the end value, the parameters and the directions are written back
/// (**FS-34** without directions).
#[allow(clippy::too_many_arguments)]
pub fn open_powell(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ia_params: &mut [f32],
    io_initial_direction: Option<&mut NrMatrix>,
    ic_params: i32,
    i_tolerance: f32,
    o_iterations: &mut i32,
    o_final_function_return: &mut f32,
    i_function: &mut dyn FsFunctions,
) -> Result<(), Fatal> {
    let n = ic_params.max(0) as usize;
    let mut cost = FsCostFunction::new(i_function);
    let mut minimizer = FsPowell::new();
    minimizer.base.set_x_tolerance(f64::from(i_tolerance));
    let mut final_parameters = float_to_vnl(ia_params, n);
    let mut initial_direction = match io_initial_direction.as_deref() {
        Some(d) => float_to_vnl_matrix(d, n),
        None => identity(n),
    };
    let code = minimizer.minimize(
        &mut cost,
        &mut final_parameters,
        &mut initial_direction,
        out,
        err,
    );
    match code {
        ReturnCode::FailedTooManyIterations => log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "powell exceeding maximum iterations.",
        ),
        ReturnCode::ErrorFailure
        | ReturnCode::ErrorDodgyInput
        | ReturnCode::FailedFtolTooSmall
        | ReturnCode::FailedXtolTooSmall
        | ReturnCode::FailedGtolTooSmall => log::error_exit(g, err, ERROR_BADPARM, "powell error."),
        _ => {
            *o_iterations = minimizer.base.num_iterations as i32;
            *o_final_function_return = minimizer.base.end_error as f32;
            vnl_to_float(&final_parameters, ia_params, n);
            put_directions(
                g,
                &initial_direction,
                io_initial_direction,
                n,
                "OpenPowell",
            );
            Ok(())
        }
    }
}

/// `OpenPowell2(iaParams, ioInitialDirection, icParams, iTolerance, iLinMinTol, MaxIterations,
/// oIterations, oFinalFunctionReturn, iFunction)`: [`FsPowell`] with `ftol = iTolerance`,
/// `linmin_xtol = iLinMinTol`, `maxfev = MaxIterations`, verbose and traced when `Gdiag_no > 0`;
/// writes the iteration count, the end value, the parameters and the directions (**FS-34**
/// without directions) whatever the result, then returns 1 after `powell exceeded maximum
/// iterations` or `powell error <code>` on standard output for those return codes (**FS-35**:
/// never), else 0.
#[allow(clippy::too_many_arguments)]
pub fn open_powell2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ia_params: &mut [f32],
    io_initial_direction: Option<&mut NrMatrix>,
    ic_params: i32,
    i_tolerance: f32,
    i_lin_min_tol: f32,
    max_iterations: i32,
    o_iterations: &mut i32,
    o_final_function_return: &mut f32,
    i_function: &mut dyn FsFunctions,
) -> i32 {
    let n = ic_params.max(0) as usize;
    let mut cost = FsCostFunction::new(i_function);
    let mut minimizer = FsPowell::new();
    let mut final_parameters = float_to_vnl(ia_params, n);
    let mut initial_direction = match io_initial_direction.as_deref() {
        Some(d) => float_to_vnl_matrix(d, n),
        None => identity(n),
    };
    if g.gdiag_no > 0 {
        minimizer.base.trace = true;
        minimizer.base.verbose = true;
    }
    minimizer.linmin_xtol = f64::from(i_lin_min_tol);
    minimizer.base.ftol = f64::from(i_tolerance);
    minimizer.base.maxfev = i64::from(max_iterations);
    let code = minimizer.minimize(
        &mut cost,
        &mut final_parameters,
        &mut initial_direction,
        out,
        err,
    );
    *o_iterations = minimizer.base.num_iterations as i32;
    *o_final_function_return = minimizer.base.end_error as f32;
    vnl_to_float(&final_parameters, ia_params, n);
    put_directions(
        g,
        &initial_direction,
        io_initial_direction,
        n,
        "OpenPowell2",
    );
    match code {
        ReturnCode::FailedTooManyIterations => {
            let _ = writeln!(out, "powell exceeded maximum iterations");
            1
        }
        ReturnCode::ErrorFailure
        | ReturnCode::ErrorDodgyInput
        | ReturnCode::FailedFtolTooSmall
        | ReturnCode::FailedXtolTooSmall
        | ReturnCode::FailedGtolTooSmall => {
            let _ = writeln!(out, "powell error {}", code.value());
            1
        }
        _ => 0,
    }
}

/// `OpenDFPMin(p, n, iTolerance, oIterations, oFinalFunctionReturn, iFunction,
/// iDerivativeFunction, iStepFunction, iStepFunctionParams, iUserCallBackFunction)`: quasi-Newton
/// minimization of `iFunction` (with its gradient, [`FsFunctions`]) from `p[1..=n]` by
/// [`FsLbfgs`] (`memory` 7, line search accuracy 0.1, `gtol = iTolerance`; the x and f tolerances
/// it sets are not used), repeated while the end value improved on `*oFinalFunctionReturn` (the
/// caller's value on entry) by more than `iTolerance` relative to it (**FS-38**, kept) and the
/// pass succeeded; each new best point goes to the step function and the user callback, and
/// their count is the iteration count. Returns 0 (and writes the count, the end value and `p`)
/// when the last pass succeeded; otherwise the minimizer's failure code: `ERROR_FAILURE` (-1)
/// writes them too, `ERROR_DODGY_INPUT` and the tolerance codes exit (`ERROR_BADPARM`; never
/// reached), and **FS-39** `FAILED_USER_REQUEST` (9, the evaluation limit) writes nothing in
/// FreeSurfer (by default the outputs are written).
#[allow(clippy::too_many_arguments)]
pub fn open_dfp_min(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: &mut [f32],
    n: i32,
    i_tolerance: f32,
    o_iterations: &mut i32,
    o_final_function_return: &mut f32,
    funcs: &mut dyn FsFunctions,
) -> Result<i32, Fatal> {
    let nu = n.max(0) as usize;
    let mut cost = FsCostFunction::with_gradient(funcs, n);
    let mut final_parameters = float_to_vnl(p, nu);
    let mut minimizer = FsLbfgs::new();
    let mut observer = FsLbfgsObserver::default();
    minimizer.memory = 7;
    minimizer.line_search_accuracy = 0.1;
    minimizer.base.set_x_tolerance(0.001);
    minimizer.base.ftol = 0.1;
    minimizer.base.gtol = f64::from(i_tolerance);
    let mut is_success = true;
    let previous_value = *o_final_function_return;
    let mut should_continue = true;
    while should_continue && is_success {
        is_success = minimizer.minimize(
            g,
            &mut cost,
            Some(&mut observer),
            &mut final_parameters,
            out,
            err,
        );
        let current_value = minimizer.base.end_error as f32;
        if (previous_value - current_value) / previous_value <= i_tolerance {
            should_continue = false;
        }
    }
    let mut write_outputs = |p: &mut [f32]| {
        *o_iterations = observer.number_of_optimal_updates;
        *o_final_function_return = minimizer.base.end_error as f32;
        vnl_to_float(&final_parameters, p, nu);
    };
    if is_success {
        write_outputs(p);
        return Ok(0);
    }
    let code = minimizer.base.failure_code;
    match code {
        ReturnCode::ErrorFailure => write_outputs(p),
        ReturnCode::ErrorDodgyInput => {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "quasi-Newton minimization (lbfsg) dodgy input",
            )?;
        }
        ReturnCode::FailedTooManyIterations
        | ReturnCode::FailedFtolTooSmall
        | ReturnCode::FailedXtolTooSmall
        | ReturnCode::FailedGtolTooSmall => {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "quasi-Newton minimization (lbfsg) tolerances too small",
            )?;
        }
        ReturnCode::FailedUserRequest => {
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_39,
                    "OpenDFPMin reached the evaluation limit: the parameters, the iteration count and the final value are not written (FreeSurfer's)",
                );
            } else {
                write_outputs(p);
                g.notices.add(
                    &FS_39,
                    "OpenDFPMin reached the evaluation limit: the parameters, the iteration count and the final value are written (FreeSurfer drops them)",
                );
            }
        }
        _ => {}
    }
    Ok(code.value())
}

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

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's numerics wrappers over VXL (`fs.maths`, queue items 3.1-23 and 3.1-24),
//! translated from FreeSurfer 8.2.0's `utils/numerics.cpp`: the conversions between NR's 1-based
//! `float` arrays and VNL's `double` ones (69-110), `OpenDFPMin` (846-926), `OpenPowell`
//! (980-1019), `OpenPowell2` (1037-1089), `OpenLUMatrixInverse` (1091-1153),
//! `OpenQRdecomposition` (1155-1196), `OpenMatrixDeterminant` (1198-1219), `OpenEigenSystem`
//! (930-943), `OpenNonSymmetricEigenSystem` (948-978), `OpenSvdcmp` (1225-1248) and Numerical
//! Recipes' allocators `vector` and `matrix` (1685-1727: [`NrVector`], [`NrMatrix`]; `free_vector`
//! and `free_matrix` are their drop). The VXL routines they call are the main package's ports of
//! ITK 4.13.2's VNL (`larmorx_core::vnl_fixed`: `vnl_det`, `vnl_inverse`, `vnl_determinant`;
//! `larmorx_core::vnl_qr::Qr<f32>`: `vnl_qr<float>`; `larmorx_core::vnl_eigen`:
//! `vnl_symmetric_eigensystem<float>`, `vnl_real_eigensystem`; `larmorx_core::vnl_svd_f32`:
//! `vnl_svd<float>`; `larmorx_core::vnl_minimize` and `vnl_lbfgs` under [`crate::fs_vnl`]), queue
//! item 3.1-07; this crate contains no VXL code.
//!
//! `numerics.cpp`'s GSL functions (`sc_*`) are queue item 3.1-53 (`larmorx-gpl`) and never come
//! here; Burkardt's spline (`SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`) is item 3.1-54.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-maths.md`): **FS-33**, the 0 x 0
//! matrix (see [`open_lu_matrix_inverse`] and [`open_matrix_determinant`]); **FS-34**, the
//! directions written through `NULL` ([`open_powell`], [`open_powell2`]); **FS-35** and **FS-36**
//! (kept), the Powell wrappers' return codes and tolerance; **FS-38** and **FS-39**,
//! [`open_dfp_min`]'s passes and its evaluation limit; **FS-41**, [`open_non_symmetric_eigen_system`]'s
//! copy on the stack; **FS-42**, [`open_eigen_system`]'s ignored failure.

use std::io::Write;
use std::ops::{Index, IndexMut};

use larmorx_core::defects::{FS_33, FS_34, FS_38, FS_39, FS_41, FS_42, FS_47, FS_48, ITK_112};
use larmorx_core::vnl_minimize::ReturnCode;
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};
use larmorx_core::{vnl_eigen, vnl_fixed, vnl_svd_f32};

use crate::fs_vnl::{FsCostFunction, FsFunctions, FsLbfgs, FsLbfgsObserver, FsPowell};
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use crate::matrix::{self, Matrix};

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
            put_directions(g, &initial_direction, io_initial_direction, n, "OpenPowell");
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
/// it sets are not used), repeated while the pass succeeded and its end value improved by more
/// than `iTolerance` relative to the previous pass's (the first pass: to `*oFinalFunctionReturn`,
/// the caller's value); **FS-38**: FreeSurfer compares every pass with the caller's value and
/// continues where the relative improvement is a NaN (`reproduce`); each new best point goes to the step function and the user callback, and
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
    let reproduce = g.notices.reproduce();
    let mut is_success = true;
    // FreeSurfer compares every pass with the caller's value (FS-38); the default with the
    // previous pass's, and stops unless the improvement is more than the tolerance.
    let first_value = *o_final_function_return;
    let mut previous_value = first_value;
    let mut should_continue = true;
    let mut pass = 0;
    while should_continue && is_success {
        is_success = minimizer.minimize(
            g,
            &mut cost,
            Some(&mut observer),
            &mut final_parameters,
            out,
            err,
        );
        pass += 1;
        if minimizer.crashed {
            // FreeSurfer crashes here (FS-37, noted): the starting point, as a success.
            break;
        }
        let current_value = minimizer.base.end_error as f32;
        let fs_stop = (first_value - current_value) / first_value <= i_tolerance;
        // a NaN improvement (0 / 0) is not one: stop
        let improvement = (previous_value - current_value) / previous_value;
        let fixed_stop = improvement.is_nan() || improvement <= i_tolerance;
        let stop = if reproduce { fs_stop } else { fixed_stop };
        if is_success && fs_stop != fixed_stop {
            let what = match (reproduce, fs_stop) {
                (false, true) => {
                    "continues: the pass improved on the previous one by more than the tolerance (FreeSurfer compares with its first value and stops)"
                }
                (false, false) => {
                    "stops: the pass did not improve on the previous one by more than the tolerance (FreeSurfer compares with its first value and continues)"
                }
                (true, true) => {
                    "stops (FreeSurfer's comparison with its first value; compared with the previous pass it would continue)"
                }
                (true, false) => {
                    "continues (FreeSurfer's comparison with its first value; compared with the previous pass it would stop)"
                }
            };
            g.notices
                .add(&FS_38, format!("OpenDFPMin after pass {pass} {what}"));
        }
        previous_value = current_value;
        if stop {
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

/// The root of the ITK 4.13.2 source tree the official FreeSurfer 8.2.0 binaries were built from,
/// as VNL's `__FILE__` prints it in its messages (`strings` of the official `mri_ca_register`).
pub const ITK_ROOT: &str = "/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK";

/// `vnl_svd.hxx`'s `__FILE__` in the official build.
pub fn vnl_svd_hxx() -> String {
    format!("{ITK_ROOT}/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_svd.hxx")
}

/// `vnl_matlab_print_scalar(float, buf, vnl_matlab_print_format_long)`: `"%8d "` of 0 for an exact
/// zero (either sign), else `"%8.5f "` of the float as a `double`, glibc's `nan`, `-nan`, `inf`
/// and `-inf` included.
pub fn matlab_long_f32(v: f32) -> String {
    if v == 0.0 {
        return format!("{:>8} ", 0);
    }
    format!("{} ", crate::cstd::fmt_f(f64::from(v), 8, 5))
}

/// The lines `vnl_svd<float>` prints on standard error when LINPACK's `ssvdc` does not converge
/// (ITK 4.13.2's `vnl_svd.hxx` 69-72): the return value `info`, the matrix's size, and the matrix
/// by `vnl_matlab_print(std::cerr, M, "M", vnl_matlab_print_format_long)` (`m` row-major).
pub fn suspicious_svdc_f32(m: &[f32], rows: usize, cols: usize, info: usize) -> String {
    let file = vnl_svd_hxx();
    let mut s = format!(
        "{file}: suspicious return value ({info}) from SVDC\n{file}: M is {rows}x{cols}\nM = [ ...\n"
    );
    if rows == 0 {
        s.push_str("];\n");
        return s;
    }
    for i in 0..rows {
        for j in 0..cols {
            s.push_str(&matlab_long_f32(m[i * cols + j]));
        }
        if i + 1 == rows {
            s.push_str(" ]");
        }
        s.push('\n');
    }
    s
}

/// `copy_out(o->data)` through the C pointer: `v` into `o`'s numbers from `data[0]`, a float past
/// the array dropped (FS-16: FreeSurfer writes memory it does not own there).
fn copy_out_checked(g: &mut Globals, o: &mut Matrix, v: &[f32]) {
    for (k, &x) in v.iter().enumerate() {
        matrix::write_at(g, o, 2 + k as isize, x);
    }
}

/// `OpenEigenSystem(iaData, icData, oEigenValues, oEigenVectors)`: the eigenvalues (ascending)
/// and eigenvectors (the columns of `oEigenVectors`, row by row) of the symmetric `n` x `n`
/// matrix `ia_data` (row by row), as `vnl_symmetric_eigensystem<float>` computes them (EISPACK's
/// `rs` in `double`, rounded to `float`). `o_values` must hold `n` floats and `o_vectors` `n * n`.
///
/// **FS-42:** when EISPACK's `tql2` does not converge (a matrix with NaN entries), VNL prints
/// `vnl_symmetric_eigensystem: ierr = <i>` on standard error, keeps the eigenvalues as `rs` left
/// them and never writes the eigenvectors; FreeSurfer copies them (memory contents) and returns
/// `NO_ERROR`. By default the eigenvalues are written, the eigenvectors are not, and the error
/// `OpenEigenSystem: the eigensystem did not converge (ierr <i>)` gives `ERROR_BADPARM`; with
/// `reproduce` the eigenvectors are 0 (the memory VNL never wrote, filled) and `NO_ERROR` is
/// returned. A matrix of no rows: nothing is written, `NO_ERROR`.
///
/// **FS-47:** a matrix of two rows or more with an infinite or NaN entry among those EISPACK's
/// `tred2` reads (the upper triangle as stored). Where VNL's eigensystem never returns on it
/// (EISPACK's `pythag` loops for ever on a NaN or two infinities: `larmorx_core`'s
/// `symmetric_eigensystem_f32_full` names those matrices, ITK-115), FreeSurfer hangs: both modes
/// give the error `OpenEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or
/// NaN entry)` and `ERROR_BADPARM`, nothing written. Where it returns, `reproduce` gives
/// FreeSurfer's result (VNL's values and vectors, FS-42 where `tql2` stops); the default gives the
/// same error, nothing written, FS-47 noted (rule B: such a matrix has no eigensystem).
pub fn open_eigen_system(
    g: &mut Globals,
    err: &mut dyn Write,
    ia_data: &[f32],
    n: usize,
    o_values: &mut [f32],
    o_vectors: &mut [f32],
) -> i32 {
    if n == 0 {
        return NO_ERROR;
    }
    let full = vnl_eigen::symmetric_eigensystem_f32_full(&ia_data[..n * n], n);
    if n >= 2 && (0..n).any(|i| (i..n).any(|j| !ia_data[i * n + j].is_finite())) {
        let hangs = full.never_returns();
        if hangs || !g.notices.reproduce() {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                "OpenEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or NaN entry)\n",
            );
            g.notices.add(
                &FS_47,
                if hangs {
                    "OpenEigenSystem: an infinite or NaN entry on which FreeSurfer hangs (EISPACK's pythag): an error"
                } else {
                    "OpenEigenSystem: an infinite or NaN entry (FreeSurfer computes on it): an error"
                },
            );
            return ERROR_BADPARM;
        }
        g.notices.add(
            &FS_47,
            "OpenEigenSystem: an infinite or NaN entry: FreeSurfer's eigensystem of it",
        );
    }
    let (values, vectors, ierr) = (full.values, full.vectors, full.ierr);
    if ierr != 0 {
        let _ = writeln!(err, "vnl_symmetric_eigensystem: ierr = {ierr}");
        o_values[..n].copy_from_slice(&values);
        if g.notices.reproduce() {
            g.notices.add(
                &FS_42,
                format!("OpenEigenSystem: tql2 failed (ierr {ierr}): the eigenvectors VNL never wrote are 0"),
            );
            o_vectors[..n * n].fill(0.0);
            return NO_ERROR;
        }
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("OpenEigenSystem: the eigensystem did not converge (ierr {ierr})\n"),
        );
        g.notices.add(
            &FS_42,
            format!("OpenEigenSystem: tql2 failed (ierr {ierr}): an error, no eigenvectors"),
        );
        return ERROR_BADPARM;
    }
    o_vectors[..n * n].copy_from_slice(&vectors);
    o_values[..n].copy_from_slice(&values);
    NO_ERROR
}

/// The stack FreeSurfer's programs run with by default (8 MiB): `OpenNonSymmetricEigenSystem`'s
/// copy of a larger matrix overflows it (FS-41).
const DEFAULT_STACK: usize = 8 << 20;

/// `OpenNonSymmetricEigenSystem(iaData, icData, oEigenValues, oEigenVectors)`: the `n` x `n`
/// matrix converted to `double`, `vnl_real_eigensystem` of it (EISPACK's `rg`): the eigenvalues'
/// real parts into `o_values` and `Vreal` (row by row) into `o_vectors`, in `float`. VNL's own
/// defects come through: the columns of a complex eigenvalue pair are 0 (ITK-110), the eigenvalues
/// `hqr2` did not find are 0 (ITK-111, after VNL's ` *** vnl_real_eigensystem: Failed on <i>th
/// eigenvalue` and the matrix on standard error). Returns `NO_ERROR`.
///
/// **ITK-112:** EISPACK's `balanc` never returns on a matrix with an infinite or NaN entry (or one
/// whose scaled sums overflow): FreeSurfer hangs. Both modes give the error
/// `OpenNonSymmetricEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or
/// NaN entry)` and `ERROR_BADPARM`, nothing written. **FS-41:** FreeSurfer copies the matrix into
/// a variable-length array on the stack; one of more than 8 MiB (about 1,024 rows) overflows the
/// default stack, a crash. Both modes compute it (FS-41 noted).
pub fn open_non_symmetric_eigen_system(
    g: &mut Globals,
    err: &mut dyn Write,
    ia_data: &[f32],
    n: usize,
    o_values: &mut [f32],
    o_vectors: &mut [f32],
) -> i32 {
    if n == 0 {
        return NO_ERROR;
    }
    if n * n * 8 >= DEFAULT_STACK {
        g.notices.add(
            &FS_41,
            format!("OpenNonSymmetricEigenSystem: a {n} x {n} matrix copied to the stack (FreeSurfer overflows its 8 MiB stack): computed"),
        );
    }
    let data: Vec<f64> = ia_data[..n * n].iter().map(|&v| f64::from(v)).collect();
    let es = match vnl_eigen::real_eigensystem(&data, n) {
        Ok(es) => es,
        Err(vnl_eigen::BalancNeverReturns) => {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                "OpenNonSymmetricEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or NaN entry)\n",
            );
            g.notices.add(
                &ITK_112,
                "OpenNonSymmetricEigenSystem: balanc would never return (FreeSurfer hangs): an error",
            );
            return ERROR_BADPARM;
        }
    };
    if es.ierr != 0 {
        let _ = err
            .write_all(vnl_eigen::real_eigensystem_failure_message(&data, n, es.ierr).as_bytes());
    }
    for nt in &es.notices {
        g.notices.add(nt.defect, nt.detail.clone());
    }
    for (o, &v) in o_vectors[..n * n].iter_mut().zip(&es.vreal) {
        *o = v as f32;
    }
    for (o, &v) in o_values[..n].iter_mut().zip(&es.d_re) {
        *o = v as f32;
    }
    NO_ERROR
}

/// `OpenSvdcmp(ioA, oW, oV)`: the singular value decomposition of `ioA` (rows x cols, rows >=
/// cols) by `vnl_svd<float>` (LINPACK's `ssvdc`, economy size): `U` (rows x cols) into `ioA`'s
/// numbers, the `cols` singular values into `oW`'s, `V` (cols x cols) into `oV`'s, each row by row
/// from the first number whatever the outputs' sizes (a float past an output's array: FS-16).
/// Returns `NO_ERROR`; `ERROR_BADPARM` with nothing written for fewer rows than columns, and when
/// `ssvdc` does not converge (a matrix with NaN or infinite entries), after VNL's lines on standard
/// error ([`suspicious_svdc_f32`]). A matrix without columns: nothing written, `NO_ERROR`.
///
/// **FS-48:** a matrix with an infinite or NaN entry. Where LINPACK's `ssvdc` never returns on it
/// (`larmorx_core`'s `vnl_svd_f32_full` names those matrices, ITK-116), FreeSurfer hangs: both
/// modes give VNL's lines with `info` min(rows + 1, cols) and `ERROR_BADPARM`, nothing written.
/// Where it returns, `reproduce` gives FreeSurfer's result (LINPACK's failure with its `info`, or
/// the decomposition); the default gives the lines with min(rows + 1, cols) without decomposing,
/// `ERROR_BADPARM`, FS-48 noted (rule B).
pub fn open_svdcmp(
    g: &mut Globals,
    err: &mut dyn Write,
    io_a: &mut Matrix,
    o_w: &mut Matrix,
    o_v: &mut Matrix,
) -> i32 {
    if io_a.rows < io_a.cols {
        return ERROR_BADPARM;
    }
    let (rows, cols) = (io_a.rows.max(0) as usize, io_a.cols.max(0) as usize);
    if cols == 0 {
        return NO_ERROR;
    }
    let a = vnl_of(io_a);
    let full = vnl_svd_f32::vnl_svd_f32_full(&a, rows, cols);
    if a.iter().any(|v| !v.is_finite()) {
        let hangs = full.never_returns();
        if hangs || !g.notices.reproduce() {
            let info = (rows + 1).min(cols);
            let _ = err.write_all(suspicious_svdc_f32(&a, rows, cols, info).as_bytes());
            g.notices.add(
                &FS_48,
                if hangs {
                    format!("OpenSvdcmp of a {rows} x {cols} matrix with an infinite or NaN entry on which FreeSurfer hangs: a failed decomposition")
                } else {
                    format!("OpenSvdcmp of a {rows} x {cols} matrix with an infinite or NaN entry: refused as a failed decomposition")
                },
            );
            return ERROR_BADPARM;
        }
        g.notices.add(
            &FS_48,
            format!("OpenSvdcmp of a {rows} x {cols} matrix with an infinite or NaN entry: FreeSurfer's result"),
        );
    }
    if full.info != 0 {
        // LINPACK's info (min(rows + 1, cols) on every failing finite case measured)
        let _ = err.write_all(suspicious_svdc_f32(&a, rows, cols, full.info).as_bytes());
        return ERROR_BADPARM;
    }
    let (u_cm, w, v_cm) = (full.u, full.w, full.v);
    let mut u = vec![0.0f32; rows * cols];
    for i in 0..rows {
        for j in 0..cols {
            u[i * cols + j] = u_cm[j * rows + i];
        }
    }
    let mut v = vec![0.0f32; cols * cols];
    for i in 0..cols {
        for j in 0..cols {
            v[i * cols + j] = v_cm[j * cols + i];
        }
    }
    copy_out(io_a, &u);
    copy_out_checked(g, o_w, &w);
    copy_out_checked(g, o_v, &v);
    NO_ERROR
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

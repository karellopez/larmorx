// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The LAPACK and BLAS routines behind SciPy 1.18.1's `linalg.inv` of a 4×4 float64 matrix
//! beyond its LU path (`scipy_linalg`): the condition estimates that decide its
//! `LinAlgWarning` (`dgecon`, `dtrcon`, `dpocon`, `dsycon`, with `dlacn2`, `dlatrs`, `drscl`,
//! `dlantr`, `dsytrs`), and the symmetric path (OpenBLAS's Cholesky `dpotrf` → `potf2_U` and
//! `dpotri` → `dtrtri` + `dlauum` → `lauu2_U`; when Cholesky fails, Bunch-Kaufman's `dsytrf` →
//! `dsytf2` and `dsytri`), as the scipy-openblas32 0.3.31.dev library of SciPy 1.18.1's wheel
//! computes them on its **Haswell** kernels (`OPENBLAS_CORETYPE=Haswell`,
//! `docs/findings/numpy-blas.md`).
//!
//! - The LAPACK routines are the reference Fortran of OpenBLAS's `lapack-netlib` (LAPACK 3.12.0
//!   in OpenBLAS 0.3.30), which gfortran compiled for the x86-64 baseline: every operation is
//!   rounded on its own, in the source's order (no fused multiply-add in their disassembly in
//!   SciPy 1.18.1's `libscipy_openblas`).
//! - `dpotrf`, `dtrtri`, `dlauum` are OpenBLAS's own (`interface/lapack/`): one thread for this
//!   size, the unblocked `potf2_U`, `trti2_UN` and `lauu2_U` (`n ≤ DTB_ENTRIES`); `dpotri` is
//!   LAPACK's Fortran calling them.
//! - The BLAS kernels on these lengths (at most 4 elements): `dasum.c` with its Haswell
//!   microkernel (`|x|` summed from +0 in order), `ddot.c` (unit strides: `dot += y·x` from +0;
//!   strided: blocks of four into two sums, the rest into the first, then their sum), `daxpy.c`
//!   (`y += a·x`), `dscal.c` (`x *= a`; an internal call with `a = 0` stores +0),
//!   `iamax_sse2.S` (`openblas`'s `iamax`), `dsymv_U.c` below 16 rows,
//!   `dgemv_n_4.c` and `dgemv_t_4.c` below 4 rows (`dgemv_t_4.c`'s tails with GCC's
//!   contractions: `fma(a0, x0, a1·x1)`, then `fma(a2, x2, ·)`, added to `y`; one row
//!   `fma(a, x, y)`), the generic `ger.c` and the `trsv_U.c` / `trsv_L.c` drivers (one block:
//!   `axpy` per column, or `dot` per row); the C kernels other than `dgemv_t_4.c` have no fused
//!   multiply-adds (checked in the disassembly); the interfaces' early returns (`alpha == 0` in
//!   `daxpy`, `dger`, `dsyr`, `dsymv`; `alpha == 1` in `dscal`; `beta == 0` clears `y` to +0 in
//!   `dsymv`; `n == 0` everywhere).
//!
//! Upstream files (BSD-3-Clause; notices in `numpy_linalg` and NOTICE): OpenBLAS 0.3.30
//! release tarball, `lapack-netlib/SRC/` (`dgecon.f`, `dtrcon.f`, `dpocon.f`, `dsycon.f`,
//! `dlacn2.f`, `dlatrs.f`, `drscl.f`, `dlantr.f`, `dlange.f`, `dpotri.f`, `dsytrf.f`, `dsytf2.f`,
//! `dsytri.f`, `dsytrs.f`, `INSTALL/dlamch.f`), `interface/` (`asum.c`, `dot.c`, `axpy.c`,
//! `scal.c`, `imax.c`, `trsv.c`, `symv.c`, `syr.c`, `ger.c`, `gemv.c`, `lapack/potrf.c`,
//! `lapack/trtri.c`, `lapack/lauum.c`), `lapack/potf2/potf2_U.c`, `lapack/lauu2/lauu2_U.c`,
//! `lapack/potrf/potrf_U_single.c`, `lapack/lauum/lauum_U_single.c`, `driver/level2/trsv_U.c`,
//! `trsv_L.c`, `kernel/x86_64/` (`dasum.c`, `dasum_microk_haswell-2.c`, `ddot.c`, `daxpy.c`,
//! `dscal.c`, `dsymv_U.c`, `dgemv_n_4.c`, `dgemv_t_4.c`, `iamax_sse2.S`, `KERNEL.HASWELL`),
//! `kernel/generic/ger.c`; SciPy 1.18.1 `scipy/linalg/src/_linalg_inv.hh`,
//! `_common_array_utils.hh`. The logic is re-expressed in Rust; no code was copied.

// The routines keep LAPACK's names and 1-based loops, so the port can be checked line by line
// against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::assign_op_pattern
)]

use crate::openblas;

/// A column-major 4×4 matrix with leading dimension 4: element (row `r`, column `c`) is
/// `a[r + 4 c]`.
pub(crate) type Cm = [f64; 16];

const N: usize = 4;
const LD: usize = 4;

/// `DLAMCH('Safe minimum')`: `TINY(0d0)` (`1 / HUGE` is smaller).
const SAFMIN: f64 = f64::MIN_POSITIVE;
/// `DLAMCH('Precision')`: `EPSILON(0d0) · 0.5 · RADIX` = 2⁻⁵².
const PREC: f64 = f64::EPSILON;
/// `DLAMCH('Overflow')`.
const HUGEVAL: f64 = f64::MAX;

/// 1-based `A(i, j)`.
#[inline]
fn ix(i: usize, j: usize) -> usize {
    (i - 1) + LD * (j - 1)
}

// ------------------------------------------------------------------------------------------
// BLAS kernels on the lengths these matrices give them

/// `ABS_K` of OpenBLAS's C kernels: `a > 0 ? a : -a`.
fn abs_k(a: f64) -> f64 {
    if a > 0.0 { a } else { -a }
}

/// `DASUM(n, x, 1)` for `n < 8` (`asum.c`: 0 for `n ≤ 0`; `dasum.c`'s Haswell tail).
fn dasum(x: &[f64]) -> f64 {
    debug_assert!(x.len() < 8);
    let mut s = 0.0f64;
    for &v in x {
        s += abs_k(v);
    }
    s
}

/// `IDAMAX(n, x, 1)`, 0-based (`iamax_sse2.S`; `n ≥ 1`).
fn idamax(x: &[f64]) -> usize {
    openblas::iamax(x)
}

/// `IDAMAX(n, A(i, j), LDA)` along a row, 0-based (the elements gathered, then the same rule as
/// the unit-stride search: the first element of largest magnitude; the kernel's strided loop
/// treats NaN alike for these lengths).
fn idamax_row(a: &Cm, i: usize, j: usize, n: usize) -> usize {
    let v: Vec<f64> = (0..n).map(|k| a[ix(i, j + k)]).collect();
    openblas::iamax(&v)
}

/// `DDOT`/`DOTU_K` with unit strides, `n < 16` (`ddot.c`'s tail: `dot += y·x` from +0).
fn ddot(x: &[f64], y: &[f64]) -> f64 {
    debug_assert!(x.len() < 16);
    let mut dot = 0.0f64;
    for (xi, yi) in x.iter().zip(y) {
        dot += yi * xi;
    }
    dot
}

/// `DOTU_K` with strides (`ddot.c`: four at a time into `temp1 += m1 + m3`, `temp2 += m2 + m4`,
/// the rest into `temp1`; then `temp1 + temp2`).
fn ddot_strided(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let (mut t1, mut t2) = (0.0f64, 0.0f64);
    let n1 = n & !3;
    let mut i = 0;
    while i < n1 {
        let m1 = y[i] * x[i];
        let m2 = y[i + 1] * x[i + 1];
        let m3 = y[i + 2] * x[i + 2];
        let m4 = y[i + 3] * x[i + 3];
        t1 += m1 + m3;
        t2 += m2 + m4;
        i += 4;
    }
    while i < n {
        t1 += y[i] * x[i];
        i += 1;
    }
    t1 + t2
}

/// `AXPYU_K`: `y += da·x`, each product and sum rounded (`daxpy.c`, `n < 16` or strided), no
/// test of `da`. `y` is written at `y[yo + k·yinc]`.
fn axpy_k(da: f64, x: &[f64], y: &mut [f64], yo: usize, yinc: usize) {
    for (k, &xk) in x.iter().enumerate() {
        y[yo + k * yinc] += da * xk;
    }
}

/// `DAXPY` from Fortran (`axpy.c`: nothing for `n ≤ 0` or `alpha == 0`).
fn daxpy(da: f64, x: &[f64], y: &mut [f64], yo: usize) {
    if x.is_empty() || da == 0.0 {
        return;
    }
    axpy_k(da, x, y, yo, 1);
}

/// `DSCAL` from Fortran (`scal.c`: nothing for `alpha == 1`; then `x *= alpha`, zero included:
/// the kernel's `dummy2 == 1` path).
fn dscal(da: f64, x: &mut [f64]) {
    if da == 1.0 {
        return;
    }
    for v in x.iter_mut() {
        *v *= da;
    }
}

/// `SCAL_K` called inside OpenBLAS (`dummy2 == 0`): `x *= da`, or +0 stored when `da == 0`.
fn scal_k(da: f64, a: &mut [f64], off: usize, inc: usize, n: usize) {
    for k in 0..n {
        let p = off + k * inc;
        if da == 0.0 {
            a[p] = 0.0;
        } else {
            a[p] *= da;
        }
    }
}

/// `dgemv_t_4.c` for `m < 4` rows (`m3`, the tails), `y` strided (`inc_y != 1`) or a single
/// output: `xtemp = x·alpha`, then `y += a0·xt0 (+ a1·xt1 (+ a2·xt2))` as GCC contracted it.
/// `a` is the column-major matrix at `aoff` with leading dimension `lda`; `x` holds the `m`
/// values; the outputs are `y[yo + j·yinc]`.
fn gemv_t_tail(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
    y: &mut [f64],
    yo: usize,
    yinc: usize,
) {
    if m < 1 || n < 1 {
        return;
    }
    debug_assert!(m < 4);
    let xt: Vec<f64> = x.iter().take(m).map(|v| v * alpha).collect();
    for j in 0..n {
        let c = aoff + j * lda;
        let p = yo + j * yinc;
        y[p] = match m {
            1 => a[c].mul_add(xt[0], y[p]),
            2 => a[c].mul_add(xt[0], a[c + 1] * xt[1]) + y[p],
            _ => {
                let t = a[c].mul_add(xt[0], a[c + 1] * xt[1]);
                a[c + 2].mul_add(xt[2], t) + y[p]
            }
        };
    }
}

/// `dgemv_n_4.c` for `m < 4` rows (the tails, `lda != m`): `temp += a·x` from +0 over the `n`
/// columns, then `y += alpha·temp`. `x[k]` is the k-th column's value.
fn gemv_n_tail(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
    y: &mut [f64],
    yo: usize,
) {
    if m < 1 || n < 1 {
        return;
    }
    debug_assert!(m < 4);
    for r in 0..m {
        let mut temp = 0.0f64;
        for (k, xk) in x.iter().take(n).enumerate() {
            temp += a[aoff + r + k * lda] * xk;
        }
        y[yo + r] += alpha * temp;
    }
}

/// `DTRSV(uplo, trans, diag, 4, A, 4, x, 1)` (`trsv.c`; one block of `trsv_U.c` or `trsv_L.c`).
fn dtrsv(upper: bool, trans: bool, unit: bool, a: &Cm, b: &mut [f64; 4]) {
    match (upper, trans) {
        // trsv_U.c without TRANSA (NUU, NUN): from the last row, divide, then axpy the column.
        (true, false) => {
            for idx in (0..N).rev() {
                if !unit {
                    b[idx] /= a[idx + LD * idx];
                }
                if idx > 0 {
                    let col: Vec<f64> = (0..idx).map(|k| a[k + LD * idx]).collect();
                    axpy_k(-b[idx], &col, b, 0, 1);
                }
            }
        }
        // trsv_U.c with TRANSA (TLU, TLN): from the last row, subtract the dot, then divide.
        (false, true) => {
            for idx in (0..N).rev() {
                if idx < N - 1 {
                    let col: Vec<f64> = (idx + 1..N).map(|k| a[k + LD * idx]).collect();
                    let d = ddot(&col, &b[idx + 1..]);
                    b[idx] -= d;
                }
                if !unit {
                    b[idx] /= a[idx + LD * idx];
                }
            }
        }
        // trsv_L.c without TRANSA (NLU, NLN): from the first row, divide, then axpy below.
        (false, false) => {
            for i in 0..N {
                if !unit {
                    b[i] /= a[i + LD * i];
                }
                if i < N - 1 {
                    let col: Vec<f64> = (i + 1..N).map(|k| a[k + LD * i]).collect();
                    axpy_k(-b[i], &col, b, i + 1, 1);
                }
            }
        }
        // trsv_L.c with TRANSA (TUU, TUN): from the first row, subtract the dot, then divide.
        (true, true) => {
            for i in 0..N {
                if i > 0 {
                    let col: Vec<f64> = (0..i).map(|k| a[k + LD * i]).collect();
                    let d = ddot(&col, &b[..i]);
                    b[i] -= d;
                }
                if !unit {
                    b[i] /= a[i + LD * i];
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------
// LAPACK auxiliaries

/// `DRSCL(4, SA, X, 1)`: `x / sa` as products by safe factors (`DLABAD` changes nothing for
/// double precision).
fn drscl(sa: f64, x: &mut [f64; 4]) {
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let (mut cden, mut cnum) = (sa, 1.0f64);
    loop {
        let cden1 = cden * smlnum;
        let cnum1 = cnum / bignum;
        let (mul, done);
        if cden1.abs() > cnum.abs() && cnum != 0.0 {
            mul = smlnum;
            done = false;
            cden = cden1;
        } else if cnum1.abs() > cden.abs() {
            mul = bignum;
            done = false;
            cnum = cnum1;
        } else {
            mul = cnum / cden;
            done = true;
        }
        dscal(mul, x);
        if done {
            break;
        }
    }
}

/// Fortran's `NINT` of ±1.
fn nint(v: f64) -> i32 {
    v.round() as i32
}

/// `DLACN2(4, V, X, ISGN, EST, KASE, ISAVE)`: Higham's 1-norm estimator, one reverse-
/// communication step (`isave` is 1-based as in the Fortran).
fn dlacn2(
    v: &mut [f64; 4],
    x: &mut [f64; 4],
    isgn: &mut [i32; 4],
    est: &mut f64,
    kase: &mut i32,
    isave: &mut [usize; 3],
) {
    const ITMAX: usize = 5;
    let n = N;
    if *kase == 0 {
        for xi in x.iter_mut() {
            *xi = 1.0 / n as f64;
        }
        *kase = 1;
        isave[0] = 1;
        return;
    }
    // The labels the Fortran jumps to.
    enum Go {
        L50,
        L120,
    }
    let go = match isave[0] {
        1 => {
            // 20 (N > 1)
            *est = dasum(x);
            for i in 0..n {
                x[i] = if x[i] >= 0.0 { 1.0 } else { -1.0 };
                isgn[i] = nint(x[i]);
            }
            *kase = 2;
            isave[0] = 2;
            return;
        }
        2 => {
            // 40
            isave[1] = idamax(x) + 1;
            isave[2] = 2;
            Go::L50
        }
        3 => {
            // 70
            v.copy_from_slice(x);
            let estold = *est;
            *est = dasum(v);
            let changed = (0..n).any(|i| {
                let xs = if x[i] >= 0.0 { 1.0 } else { -1.0 };
                nint(xs) != isgn[i]
            });
            if !changed || *est <= estold {
                Go::L120
            } else {
                // 90 .. 100
                for i in 0..n {
                    x[i] = if x[i] >= 0.0 { 1.0 } else { -1.0 };
                    isgn[i] = nint(x[i]);
                }
                *kase = 2;
                isave[0] = 4;
                return;
            }
        }
        4 => {
            // 110
            let jlast = isave[1];
            isave[1] = idamax(x) + 1;
            if x[jlast - 1] != x[isave[1] - 1].abs() && isave[2] < ITMAX {
                isave[2] += 1;
                Go::L50
            } else {
                Go::L120
            }
        }
        _ => {
            // 140
            let temp = 2.0 * (dasum(x) / (3 * n) as f64);
            if temp > *est {
                v.copy_from_slice(x);
                *est = temp;
            }
            // 150
            *kase = 0;
            return;
        }
    };
    match go {
        Go::L50 => {
            for xi in x.iter_mut() {
                *xi = 0.0;
            }
            x[isave[1] - 1] = 1.0;
            *kase = 1;
            isave[0] = 3;
        }
        Go::L120 => {
            let mut altsgn = 1.0f64;
            for i in 0..n {
                x[i] = altsgn * (1.0 + (i as f64) / ((n - 1) as f64));
                altsgn = -altsgn;
            }
            *kase = 1;
            isave[0] = 5;
        }
    }
}

/// The estimator's state between `dlacn2` calls.
struct Estimator {
    v: [f64; 4],
    x: [f64; 4],
    isgn: [i32; 4],
    est: f64,
    kase: i32,
    isave: [usize; 3],
}

impl Estimator {
    fn new() -> Self {
        Self {
            v: [0.0; 4],
            x: [0.0; 4],
            isgn: [0; 4],
            est: 0.0,
            kase: 0,
            isave: [0; 3],
        }
    }

    /// One `dlacn2` call; the `kase` it asks for (0: done).
    fn step(&mut self) -> i32 {
        dlacn2(
            &mut self.v,
            &mut self.x,
            &mut self.isgn,
            &mut self.est,
            &mut self.kase,
            &mut self.isave,
        );
        self.kase
    }
}

/// `DLANGE('M', m, 1, A(i0, j), ...)` of one column segment: the largest `|a|`, NaN kept.
fn dlange_max(vals: impl Iterator<Item = f64>) -> f64 {
    let mut value = 0.0f64;
    for v in vals {
        let temp = v.abs();
        if value < temp || temp.is_nan() {
            value = temp;
        }
    }
    value
}

/// `DLATRS(uplo, trans, diag, normin, 4, A, 4, X, SCALE, CNORM, INFO)`: solves the
/// triangular system with scaling against overflow; returns `SCALE`.
fn dlatrs(
    upper: bool,
    notran: bool,
    nounit: bool,
    normin_y: bool,
    a: &Cm,
    x: &mut [f64; 4],
    cnorm: &mut [f64; 4],
) -> f64 {
    let n = N;
    let mut scale = 1.0f64;
    let smlnum = SAFMIN / PREC;
    let bignum = 1.0 / smlnum;
    let at = |i: usize, j: usize| a[ix(i, j)];
    if !normin_y {
        if upper {
            for j in 1..=n {
                let col: Vec<f64> = (1..j).map(|i| at(i, j)).collect();
                cnorm[j - 1] = dasum(&col);
            }
        } else {
            for j in 1..n {
                let col: Vec<f64> = (j + 1..=n).map(|i| at(i, j)).collect();
                cnorm[j - 1] = dasum(&col);
            }
            cnorm[n - 1] = 0.0;
        }
    }
    let imax = idamax(cnorm);
    let mut tmax = cnorm[imax];
    let tscal;
    if tmax <= bignum {
        tscal = 1.0;
    } else if tmax <= HUGEVAL {
        tscal = 1.0 / (smlnum * tmax);
        dscal(tscal, cnorm);
    } else {
        tmax = 0.0;
        if upper {
            for j in 2..=n {
                tmax = dlange_max((1..j).map(|i| at(i, j))).max(tmax);
            }
        } else {
            for j in 1..n {
                tmax = dlange_max((j + 1..=n).map(|i| at(i, j))).max(tmax);
            }
        }
        if tmax <= HUGEVAL {
            tscal = 1.0 / (smlnum * tmax);
            for j in 1..=n {
                if cnorm[j - 1] <= HUGEVAL {
                    cnorm[j - 1] = cnorm[j - 1] * tscal;
                } else {
                    cnorm[j - 1] = 0.0;
                    let rows: Vec<usize> = if upper {
                        (1..j).collect()
                    } else {
                        (j + 1..=n).collect()
                    };
                    for i in rows {
                        cnorm[j - 1] = cnorm[j - 1] + tscal * at(i, j).abs();
                    }
                }
            }
        } else {
            dtrsv(upper, !notran, !nounit, a, x);
            return scale;
        }
    }

    let j = idamax(x);
    let mut xmax = x[j].abs();
    let mut xbnd = xmax;
    // The order of the columns (1-based).
    let forward: Vec<usize> = (1..=n).collect();
    let backward: Vec<usize> = (1..=n).rev().collect();
    let order = if notran == upper { &backward } else { &forward };
    let mut grow;
    if notran {
        if tscal != 1.0 {
            grow = 0.0;
        } else if nounit {
            grow = 1.0 / xbnd.max(smlnum);
            xbnd = grow;
            let mut early = false;
            for &j in order {
                if grow <= smlnum {
                    early = true;
                    break;
                }
                let tjj = at(j, j).abs();
                xbnd = xbnd.min(1.0f64.min(tjj) * grow);
                if tjj + cnorm[j - 1] >= smlnum {
                    grow = grow * (tjj / (tjj + cnorm[j - 1]));
                } else {
                    grow = 0.0;
                }
            }
            if !early {
                grow = xbnd;
            }
        } else {
            grow = 1.0f64.min(1.0 / xbnd.max(smlnum));
            for &j in order {
                if grow <= smlnum {
                    break;
                }
                grow = grow * (1.0 / (1.0 + cnorm[j - 1]));
            }
        }
    } else if tscal != 1.0 {
        grow = 0.0;
    } else if nounit {
        grow = 1.0 / xbnd.max(smlnum);
        xbnd = grow;
        let mut early = false;
        for &j in order {
            if grow <= smlnum {
                early = true;
                break;
            }
            let xj = 1.0 + cnorm[j - 1];
            grow = grow.min(xbnd / xj);
            let tjj = at(j, j).abs();
            if xj > tjj {
                xbnd = xbnd * (tjj / xj);
            }
        }
        if !early {
            grow = grow.min(xbnd);
        }
    } else {
        grow = 1.0f64.min(1.0 / xbnd.max(smlnum));
        for &j in order {
            if grow <= smlnum {
                break;
            }
            let xj = 1.0 + cnorm[j - 1];
            grow = grow / xj;
        }
    }

    if grow * tscal > smlnum {
        dtrsv(upper, !notran, !nounit, a, x);
    } else {
        if xmax > bignum {
            scale = bignum / xmax;
            dscal(scale, x);
            xmax = bignum;
        }
        if notran {
            for &j in order {
                let mut xj = x[j - 1].abs();
                let tjjs = if nounit { at(j, j) * tscal } else { tscal };
                if nounit || tscal != 1.0 {
                    let tjj = tjjs.abs();
                    if tjj > smlnum {
                        if tjj < 1.0 && xj > tjj * bignum {
                            let rec = 1.0 / xj;
                            dscal(rec, x);
                            scale = scale * rec;
                            xmax = xmax * rec;
                        }
                        x[j - 1] = x[j - 1] / tjjs;
                        xj = x[j - 1].abs();
                    } else if tjj > 0.0 {
                        if xj > tjj * bignum {
                            let mut rec = (tjj * bignum) / xj;
                            if cnorm[j - 1] > 1.0 {
                                rec = rec / cnorm[j - 1];
                            }
                            dscal(rec, x);
                            scale = scale * rec;
                            xmax = xmax * rec;
                        }
                        x[j - 1] = x[j - 1] / tjjs;
                        xj = x[j - 1].abs();
                    } else {
                        for xi in x.iter_mut() {
                            *xi = 0.0;
                        }
                        x[j - 1] = 1.0;
                        xj = 1.0;
                        scale = 0.0;
                        xmax = 0.0;
                    }
                }
                // 100
                if xj > 1.0 {
                    let mut rec = 1.0 / xj;
                    if cnorm[j - 1] > (bignum - xmax) * rec {
                        rec = rec * 0.5;
                        dscal(rec, x);
                        scale = scale * rec;
                    }
                } else if xj * cnorm[j - 1] > (bignum - xmax) {
                    dscal(0.5, x);
                    scale = scale * 0.5;
                }
                if upper {
                    if j > 1 {
                        let col: Vec<f64> = (1..j).map(|i| at(i, j)).collect();
                        daxpy(-x[j - 1] * tscal, &col, x, 0);
                        let i = idamax(&x[..j - 1]);
                        xmax = x[i].abs();
                    }
                } else if j < n {
                    let col: Vec<f64> = (j + 1..=n).map(|i| at(i, j)).collect();
                    daxpy(-x[j - 1] * tscal, &col, x, j);
                    let i = j + idamax(&x[j..]);
                    xmax = x[i].abs();
                }
            }
        } else {
            for &j in order {
                let xj = x[j - 1].abs();
                let mut uscal = tscal;
                let mut rec = 1.0 / xmax.max(1.0);
                let mut tjjs = 0.0f64;
                if cnorm[j - 1] > (bignum - xj) * rec {
                    rec = rec * 0.5;
                    tjjs = if nounit { at(j, j) * tscal } else { tscal };
                    let tjj = tjjs.abs();
                    if tjj > 1.0 {
                        rec = 1.0f64.min(rec * tjj);
                        uscal = uscal / tjjs;
                    }
                    if rec < 1.0 {
                        dscal(rec, x);
                        scale = scale * rec;
                        xmax = xmax * rec;
                    }
                }
                let mut sumj = 0.0f64;
                if uscal == 1.0 {
                    if upper {
                        let col: Vec<f64> = (1..j).map(|i| at(i, j)).collect();
                        sumj = if col.is_empty() {
                            0.0
                        } else {
                            ddot(&col, &x[..j - 1])
                        };
                    } else if j < n {
                        let col: Vec<f64> = (j + 1..=n).map(|i| at(i, j)).collect();
                        sumj = ddot(&col, &x[j..]);
                    }
                } else if upper {
                    for i in 1..j {
                        sumj = sumj + (at(i, j) * uscal) * x[i - 1];
                    }
                } else if j < n {
                    for i in j + 1..=n {
                        sumj = sumj + (at(i, j) * uscal) * x[i - 1];
                    }
                }
                if uscal == tscal {
                    x[j - 1] = x[j - 1] - sumj;
                    let xj = x[j - 1].abs();
                    tjjs = if nounit { at(j, j) * tscal } else { tscal };
                    if nounit || tscal != 1.0 {
                        let tjj = tjjs.abs();
                        if tjj > smlnum {
                            if tjj < 1.0 && xj > tjj * bignum {
                                let rec = 1.0 / xj;
                                dscal(rec, x);
                                scale = scale * rec;
                                xmax = xmax * rec;
                            }
                            x[j - 1] = x[j - 1] / tjjs;
                        } else if tjj > 0.0 {
                            if xj > tjj * bignum {
                                let rec = (tjj * bignum) / xj;
                                dscal(rec, x);
                                scale = scale * rec;
                                xmax = xmax * rec;
                            }
                            x[j - 1] = x[j - 1] / tjjs;
                        } else {
                            for xi in x.iter_mut() {
                                *xi = 0.0;
                            }
                            x[j - 1] = 1.0;
                            scale = 0.0;
                            xmax = 0.0;
                        }
                    }
                } else {
                    x[j - 1] = x[j - 1] / tjjs - sumj;
                }
                xmax = xmax.max(x[j - 1].abs());
            }
        }
        scale = scale / tscal;
    }
    if tscal != 1.0 {
        dscal(1.0 / tscal, cnorm);
    }
    scale
}

// ------------------------------------------------------------------------------------------
// The condition estimates

/// `DGECON('1', 4, LU, 4, ANORM, RCOND, WORK, IWORK, INFO)` on `getrf`'s factors: `(rcond,
/// info)`; `info = -5` without an estimate when `anorm` is NaN or overflows.
pub(crate) fn dgecon(lu: &Cm, anorm: f64) -> (f64, i32) {
    let mut rcond = 0.0f64;
    if anorm == 0.0 {
        return (rcond, 0);
    } else if anorm.is_nan() {
        return (anorm, -5);
    } else if anorm > HUGEVAL {
        return (rcond, -5);
    }
    let smlnum = SAFMIN;
    let mut e = Estimator::new();
    let mut normin = false;
    let (mut cn_l, mut cn_u) = ([0.0f64; 4], [0.0f64; 4]);
    while e.step() != 0 {
        let (sl, su);
        if e.kase == 1 {
            sl = dlatrs(false, true, false, normin, lu, &mut e.x, &mut cn_l);
            su = dlatrs(true, true, true, normin, lu, &mut e.x, &mut cn_u);
        } else {
            su = dlatrs(true, false, true, normin, lu, &mut e.x, &mut cn_u);
            sl = dlatrs(false, false, false, normin, lu, &mut e.x, &mut cn_l);
        }
        let scale = sl * su;
        normin = true;
        if scale != 1.0 {
            let i = idamax(&e.x);
            if scale < e.x[i].abs() * smlnum || scale == 0.0 {
                return (rcond, 0);
            }
            drscl(scale, &mut e.x);
        }
    }
    let ainvnm = e.est;
    if ainvnm != 0.0 {
        rcond = (1.0 / ainvnm) / anorm;
    } else {
        return (rcond, 1);
    }
    let info = i32::from(rcond.is_nan() || rcond > HUGEVAL);
    (rcond, info)
}

/// `DLANTR('1', uplo, 'N', 4, 4, A, 4, WORK)`: the largest column sum of the triangle.
fn dlantr_one(upper: bool, a: &Cm) -> f64 {
    let mut value = 0.0f64;
    for j in 1..=N {
        let rows: Vec<usize> = if upper {
            (1..=j).collect()
        } else {
            (j..=N).collect()
        };
        let mut sum = 0.0f64;
        for i in rows {
            sum = sum + a[ix(i, j)].abs();
        }
        if value < sum || sum.is_nan() {
            value = sum;
        }
    }
    value
}

/// `DTRCON('1', uplo, 'N', 4, A, 4, RCOND, WORK, IWORK, INFO)`.
pub(crate) fn dtrcon(upper: bool, a: &Cm) -> f64 {
    let mut rcond = 0.0f64;
    let smlnum = SAFMIN * N as f64;
    let anorm = dlantr_one(upper, a);
    if anorm > 0.0 {
        let mut e = Estimator::new();
        let mut normin = false;
        let mut cnorm = [0.0f64; 4];
        while e.step() != 0 {
            let scale = if e.kase == 1 {
                dlatrs(upper, true, true, normin, a, &mut e.x, &mut cnorm)
            } else {
                dlatrs(upper, false, true, normin, a, &mut e.x, &mut cnorm)
            };
            normin = true;
            if scale != 1.0 {
                let i = idamax(&e.x);
                let xnorm = e.x[i].abs();
                if scale < xnorm * smlnum || scale == 0.0 {
                    return rcond;
                }
                drscl(scale, &mut e.x);
            }
        }
        if e.est != 0.0 {
            rcond = (1.0 / anorm) / e.est;
        }
    }
    rcond
}

/// `DPOCON('U', 4, U, 4, ANORM, RCOND, WORK, IWORK, INFO)` on the Cholesky factor.
fn dpocon_upper(u: &Cm, anorm: f64) -> f64 {
    let mut rcond = 0.0f64;
    if anorm == 0.0 {
        return rcond;
    }
    let smlnum = SAFMIN;
    let mut e = Estimator::new();
    let mut normin = false;
    let mut cnorm = [0.0f64; 4];
    while e.step() != 0 {
        let scalel = dlatrs(true, false, true, normin, u, &mut e.x, &mut cnorm);
        normin = true;
        let scaleu = dlatrs(true, true, true, normin, u, &mut e.x, &mut cnorm);
        let scale = scalel * scaleu;
        if scale != 1.0 {
            let i = idamax(&e.x);
            if scale < e.x[i].abs() * smlnum || scale == 0.0 {
                return rcond;
            }
            drscl(scale, &mut e.x);
        }
    }
    if e.est != 0.0 {
        rcond = (1.0 / e.est) / anorm;
    }
    rcond
}

// ------------------------------------------------------------------------------------------
// Cholesky (OpenBLAS's own)

/// OpenBLAS's `potf2_U` (`dpotrf('U')` for this size): `0`, or the 1-based column whose pivot
/// is not positive (its value stored).
fn potf2_upper(a: &mut Cm) -> usize {
    for j in 0..N {
        let col: Vec<f64> = (0..j).map(|k| a[k + LD * j]).collect();
        let ajj = a[j + LD * j] - ddot(&col, &col);
        if ajj <= 0.0 {
            a[j + LD * j] = ajj;
            return j + 1;
        }
        let ajj = ajj.sqrt();
        a[j + LD * j] = ajj;
        let i = N - j - 1;
        if i > 0 {
            // GEMV_T(j, i, -1, A(0, j+1), lda, A(0..j, j), 1, A(j, j+1..), lda)
            let snapshot = *a;
            gemv_t_tail(
                j,
                i,
                -1.0,
                &snapshot,
                LD * (j + 1),
                LD,
                &col,
                a,
                j + LD * (j + 1),
                LD,
            );
            scal_k(1.0 / ajj, a, j + LD * (j + 1), LD, i);
        }
    }
    0
}

/// OpenBLAS's `lauu2_U` (`dlauum('U')` for this size): `U·Uᵀ` into the upper triangle.
fn lauu2_upper(a: &mut Cm) {
    for i in 0..N {
        let aii = a[i + LD * i];
        scal_k(aii, a, LD * i, 1, i + 1);
        if i < N - 1 {
            let row: Vec<f64> = (i + 1..N).map(|k| a[i + LD * k]).collect();
            let d = ddot_strided(&row, &row);
            a[i + LD * i] += d;
            let snapshot = *a;
            gemv_n_tail(
                i,
                N - i - 1,
                1.0,
                &snapshot,
                LD * (i + 1),
                LD,
                &row,
                a,
                LD * i,
            );
        }
    }
}

/// `dpotri('U')` (LAPACK's Fortran: OpenBLAS's `dtrtri` then `dlauum`): `false` when `dtrtri`
/// finds a zero diagonal element.
fn potri_upper(a: &mut Cm) -> bool {
    if (0..N).any(|j| a[j + LD * j] == 0.0) {
        return false;
    }
    let mut c: [[f64; 4]; 4] = std::array::from_fn(|j| std::array::from_fn(|r| a[r + LD * j]));
    crate::scipy_linalg::trti2_upper(&mut c);
    for (j, col) in c.iter().enumerate() {
        for (r, v) in col.iter().enumerate() {
            a[r + LD * j] = *v;
        }
    }
    lauu2_upper(a);
    true
}

// ------------------------------------------------------------------------------------------
// Bunch-Kaufman (LAPACK's Fortran)

/// `DSWAP(n, A(i0, j0), inc0, A(i1, j1), inc1)` within one matrix (strides in elements).
fn dswap_in(a: &mut Cm, n: usize, p0: usize, inc0: usize, p1: usize, inc1: usize) {
    for k in 0..n {
        a.swap(p0 + k * inc0, p1 + k * inc1);
    }
}

/// `DSYR('U', n, alpha, A(1, K), 1, A, 4)` (`syr.c`'s small path: per column `i` with
/// `x[i] != 0`, `AXPYU_K(i + 1, alpha·x[i], x, A(1, i))`).
fn dsyr_upper(n: usize, alpha: f64, a: &mut Cm, xcol: usize) {
    if n == 0 || alpha == 0.0 {
        return;
    }
    let x: Vec<f64> = (0..n).map(|k| a[k + LD * xcol]).collect();
    for i in 0..n {
        if x[i] != 0.0 {
            axpy_k(alpha * x[i], &x[..=i], a, LD * i, 1);
        }
    }
}

/// `DSYTF2('U', 4, A, 4, IPIV, INFO)` (`dsytrf` for this size: the block size 64 is at least
/// 4): `info` (0, or the 1-based first zero pivot).
fn dsytf2_upper(a: &mut Cm, ipiv: &mut [i32; 4]) -> usize {
    let alpha = (1.0 + 17.0f64.sqrt()) / 8.0;
    let mut info = 0usize;
    let mut k = N;
    while k >= 1 {
        let mut kstep = 1;
        let absakk = a[ix(k, k)].abs();
        let (imax, colmax) = if k > 1 {
            let col: Vec<f64> = (1..k).map(|i| a[ix(i, k)]).collect();
            let imax = idamax(&col) + 1;
            (imax, a[ix(imax, k)].abs())
        } else {
            (0, 0.0)
        };
        let kp;
        if absakk.max(colmax) == 0.0 || absakk.is_nan() {
            if info == 0 {
                info = k;
            }
            kp = k;
        } else {
            if absakk >= alpha * colmax {
                kp = k;
            } else {
                let jmax = imax + 1 + idamax_row(a, imax, imax + 1, k - imax);
                let mut rowmax = a[ix(imax, jmax)].abs();
                if imax > 1 {
                    let col: Vec<f64> = (1..imax).map(|i| a[ix(i, imax)]).collect();
                    let jmax = idamax(&col) + 1;
                    rowmax = rowmax.max(a[ix(jmax, imax)].abs());
                }
                if absakk >= alpha * colmax * (colmax / rowmax) {
                    kp = k;
                } else if a[ix(imax, imax)].abs() >= alpha * rowmax {
                    kp = imax;
                } else {
                    kp = imax;
                    kstep = 2;
                }
            }
            let kk = k - kstep + 1;
            if kp != kk {
                dswap_in(a, kp - 1, ix(1, kk), 1, ix(1, kp), 1);
                dswap_in(a, kk - kp - 1, ix(kp + 1, kk), 1, ix(kp, kp + 1), LD);
                a.swap(ix(kk, kk), ix(kp, kp));
                if kstep == 2 {
                    a.swap(ix(k - 1, k), ix(kp, k));
                }
            }
            if kstep == 1 {
                let r1 = 1.0 / a[ix(k, k)];
                dsyr_upper(k - 1, -r1, a, k - 1);
                let base = ix(1, k);
                dscal(r1, &mut a[base..base + (k - 1)]);
            } else if k > 2 {
                let mut d12 = a[ix(k - 1, k)];
                let d22 = a[ix(k - 1, k - 1)] / d12;
                let d11 = a[ix(k, k)] / d12;
                let t = 1.0 / (d11 * d22 - 1.0);
                d12 = t / d12;
                for j in (1..=k - 2).rev() {
                    let wkm1 = d12 * (d11 * a[ix(j, k - 1)] - a[ix(j, k)]);
                    let wk = d12 * (d22 * a[ix(j, k)] - a[ix(j, k - 1)]);
                    for i in (1..=j).rev() {
                        a[ix(i, j)] = a[ix(i, j)] - a[ix(i, k)] * wk - a[ix(i, k - 1)] * wkm1;
                    }
                    a[ix(j, k)] = wk;
                    a[ix(j, k - 1)] = wkm1;
                }
            }
        }
        if kstep == 1 {
            ipiv[k - 1] = kp as i32;
        } else {
            ipiv[k - 1] = -(kp as i32);
            ipiv[k - 2] = -(kp as i32);
        }
        if k <= kstep {
            break;
        }
        k -= kstep;
    }
    info
}

/// `DSYTRS('U', 4, 1, A, 4, IPIV, B, 4, INFO)` on `dsytf2`'s factors, for one right-hand side.
fn dsytrs_upper(a: &Cm, ipiv: &[i32; 4], b: &mut [f64; 4]) {
    // Solve U·D·x = b: from the last column.
    let mut k = N;
    while k >= 1 {
        if ipiv[k - 1] > 0 {
            let kp = ipiv[k - 1] as usize;
            if kp != k {
                b.swap(k - 1, kp - 1);
            }
            // DGER(K-1, 1, -1, A(1,K), 1, B(K), LDB, B, LDB): alpha·y = -B(K).
            if k > 1 {
                let col: Vec<f64> = (1..k).map(|i| a[ix(i, k)]).collect();
                axpy_k(-b[k - 1], &col, b, 0, 1);
            }
            dscal(1.0 / a[ix(k, k)], &mut b[k - 1..k]);
            if k == 1 {
                break;
            }
            k -= 1;
        } else {
            let kp = (-ipiv[k - 1]) as usize;
            if kp != k - 1 {
                b.swap(k - 2, kp - 1);
            }
            if k > 2 {
                let col: Vec<f64> = (1..k - 1).map(|i| a[ix(i, k)]).collect();
                axpy_k(-b[k - 1], &col, b, 0, 1);
                let col: Vec<f64> = (1..k - 1).map(|i| a[ix(i, k - 1)]).collect();
                axpy_k(-b[k - 2], &col, b, 0, 1);
            }
            let akm1k = a[ix(k - 1, k)];
            let akm1 = a[ix(k - 1, k - 1)] / akm1k;
            let ak = a[ix(k, k)] / akm1k;
            let denom = akm1 * ak - 1.0;
            let bkm1 = b[k - 2] / akm1k;
            let bk = b[k - 1] / akm1k;
            b[k - 2] = (ak * bkm1 - bk) / denom;
            b[k - 1] = (akm1 * bk - bkm1) / denom;
            if k <= 2 {
                break;
            }
            k -= 2;
        }
    }
    // Solve Uᵀ·x = b: from the first column, DGEMV('T', K-1, 1, -1, B, LDB, A(1,K), 1, 1,
    // B(K), LDB): the "matrix" is B, x is A's column (times alpha first).
    let mut k = 1;
    while k <= N {
        if ipiv[k - 1] > 0 {
            if k > 1 {
                let col: Vec<f64> = (1..k).map(|i| a[ix(i, k)]).collect();
                let bb = *b;
                gemv_t_tail(k - 1, 1, -1.0, &bb, 0, LD, &col, b, k - 1, LD);
            }
            let kp = ipiv[k - 1] as usize;
            if kp != k {
                b.swap(k - 1, kp - 1);
            }
            k += 1;
        } else {
            if k > 1 {
                let col: Vec<f64> = (1..k).map(|i| a[ix(i, k)]).collect();
                let bb = *b;
                gemv_t_tail(k - 1, 1, -1.0, &bb, 0, LD, &col, b, k - 1, LD);
                let col: Vec<f64> = (1..k).map(|i| a[ix(i, k + 1)]).collect();
                let bb = *b;
                gemv_t_tail(k - 1, 1, -1.0, &bb, 0, LD, &col, b, k, LD);
            }
            let kp = (-ipiv[k - 1]) as usize;
            if kp != k {
                b.swap(k - 1, kp - 1);
            }
            k += 2;
        }
    }
}

/// `DSYCON('U', 4, A, 4, IPIV, ANORM, RCOND, WORK, IWORK, INFO)`.
fn dsycon_upper(a: &Cm, ipiv: &[i32; 4], anorm: f64) -> f64 {
    let mut rcond = 0.0f64;
    if anorm <= 0.0 {
        return rcond;
    }
    for i in (1..=N).rev() {
        if ipiv[i - 1] > 0 && a[ix(i, i)] == 0.0 {
            return rcond;
        }
    }
    let mut e = Estimator::new();
    while e.step() != 0 {
        dsytrs_upper(a, ipiv, &mut e.x);
    }
    if e.est != 0.0 {
        rcond = (1.0 / e.est) / anorm;
    }
    rcond
}

/// `DSYMV('U', n, -1, A, 4, work, 1, 0, A(1, col), 1)` (`symv.c`: `y` cleared to +0;
/// `dsymv_U.c` below 16 rows).
fn dsymv_upper_into(n: usize, a: &mut Cm, work: &[f64], col: usize) {
    if n == 0 {
        return;
    }
    let alpha = -1.0f64;
    let mut y = vec![0.0f64; n];
    for j in 0..n {
        let temp1 = alpha * work[j];
        let mut temp2 = 0.0f64;
        for i in 0..j {
            let aij = a[i + LD * j];
            y[i] += temp1 * aij;
            temp2 += aij * work[i];
        }
        y[j] += temp1 * a[j + LD * j] + alpha * temp2;
    }
    for (i, v) in y.into_iter().enumerate() {
        a[i + LD * col] = v;
    }
}

/// `DSYTRI('U', 4, A, 4, IPIV, WORK, INFO)`: `info` (0, or the 1-based zero diagonal of D).
fn dsytri_upper(a: &mut Cm, ipiv: &[i32; 4]) -> usize {
    for info in (1..=N).rev() {
        if ipiv[info - 1] > 0 && a[ix(info, info)] == 0.0 {
            return info;
        }
    }
    let colvec =
        |a: &Cm, k: usize, len: usize| -> Vec<f64> { (1..=len).map(|i| a[ix(i, k)]).collect() };
    let mut k = 1;
    while k <= N {
        let kstep = if ipiv[k - 1] > 0 {
            a[ix(k, k)] = 1.0 / a[ix(k, k)];
            if k > 1 {
                let work = colvec(a, k, k - 1);
                dsymv_upper_into(k - 1, a, &work, k - 1);
                let d = ddot(&work, &colvec(a, k, k - 1));
                a[ix(k, k)] = a[ix(k, k)] - d;
            }
            1
        } else {
            let t = a[ix(k, k + 1)].abs();
            let ak = a[ix(k, k)] / t;
            let akp1 = a[ix(k + 1, k + 1)] / t;
            let akkp1 = a[ix(k, k + 1)] / t;
            let d = t * (ak * akp1 - 1.0);
            a[ix(k, k)] = akp1 / d;
            a[ix(k + 1, k + 1)] = ak / d;
            a[ix(k, k + 1)] = -(akkp1 / d);
            if k > 1 {
                let work = colvec(a, k, k - 1);
                dsymv_upper_into(k - 1, a, &work, k - 1);
                let d = ddot(&work, &colvec(a, k, k - 1));
                a[ix(k, k)] = a[ix(k, k)] - d;
                let d = ddot(&colvec(a, k, k - 1), &colvec(a, k + 1, k - 1));
                a[ix(k, k + 1)] = a[ix(k, k + 1)] - d;
                let work = colvec(a, k + 1, k - 1);
                dsymv_upper_into(k - 1, a, &work, k);
                let d = ddot(&work, &colvec(a, k + 1, k - 1));
                a[ix(k + 1, k + 1)] = a[ix(k + 1, k + 1)] - d;
            }
            2
        };
        let kp = ipiv[k - 1].unsigned_abs() as usize;
        if kp != k {
            dswap_in(a, kp - 1, ix(1, k), 1, ix(1, kp), 1);
            dswap_in(a, k - kp - 1, ix(kp + 1, k), 1, ix(kp, kp + 1), LD);
            a.swap(ix(k, k), ix(kp, kp));
            if kstep == 2 {
                a.swap(ix(k, k + 1), ix(kp, k + 1));
            }
        }
        k += kstep;
    }
    0
}

// ------------------------------------------------------------------------------------------
// SciPy 1.18's symmetric path

/// SciPy's `norm1_sym_herm('U', ...)` (`norm1_sym_herm_lower` on the column-major copy: the
/// upper triangle read, each element added to its row's and its column's sum, in SciPy's
/// order); the largest sum by `>` from 0.
fn norm1_symmetric_upper(a: &Cm) -> f64 {
    let mut rwork = [0.0f64; 4];
    for i in 0..N {
        rwork[i] += a[i + LD * i].abs();
        for j in 0..i {
            let temp = a[j + LD * i].abs();
            rwork[j] += temp;
            rwork[i] += temp;
        }
    }
    rwork.iter().fold(0.0, |t, &w| if w > t { w } else { t })
}

/// What SciPy 1.18's symmetric path gives.
pub(crate) enum Symmetric {
    /// The inverse (column-major) and SciPy's `rcond` (`dpocon`'s, or `dsycon`'s when Cholesky
    /// failed).
    Inverse { inverse: Cm, rcond: f64 },
    /// `dsytrf` or `dsytri` reports a zero pivot: `LinAlgError`.
    Singular,
}

/// SciPy 1.18's `inv` of a symmetric matrix (column-major): Cholesky (`potrf`, `pocon`,
/// `potri`, the lower triangle filled from the upper); when `potrf` fails, the original matrix
/// again through `sytrf`, `sycon`, `sytri`.
pub(crate) fn invert_symmetric(a: &Cm) -> Symmetric {
    let anorm = norm1_symmetric_upper(a);
    let mut c = *a;
    let cholesky = if potf2_upper(&mut c) == 0 {
        let rcond = dpocon_upper(&c, anorm);
        // dpotri's failure (impossible after a successful potrf: its diagonal is positive) is
        // not checked by SciPy either: `lapack_info` is potrf's 0.
        let _ = potri_upper(&mut c);
        Some(rcond)
    } else {
        None
    };
    let (mut inverse, rcond) = match cholesky {
        Some(rcond) => (c, rcond),
        None => {
            let mut c = *a;
            let mut ipiv = [0i32; 4];
            if dsytf2_upper(&mut c, &mut ipiv) > 0 {
                return Symmetric::Singular;
            }
            let rcond = dsycon_upper(&c, &ipiv, anorm);
            if dsytri_upper(&mut c, &ipiv) > 0 {
                return Symmetric::Singular;
            }
            (c, rcond)
        }
    };
    // fill_other_triangle('U'): the lower triangle from the upper.
    for i in 0..N {
        for j in i + 1..N {
            inverse[j + LD * i] = inverse[i + LD * j];
        }
    }
    Symmetric::Inverse { inverse, rcond }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cm(rows: [[f64; 4]; 4]) -> Cm {
        std::array::from_fn(|k| rows[k % 4][k / 4])
    }

    fn product_is_identity(a: &Cm, b: &Cm, tol: f64) -> bool {
        (0..4).all(|i| {
            (0..4).all(|j| {
                let s: f64 = (0..4).map(|k| a[i + 4 * k] * b[k + 4 * j]).sum();
                (s - if i == j { 1.0 } else { 0.0 }).abs() <= tol
            })
        })
    }

    #[test]
    fn cholesky_and_bunch_kaufman_invert() {
        let spd = cm([
            [4.0, 1.0, 0.5, 0.25],
            [1.0, 3.0, 0.2, 0.1],
            [0.5, 0.2, 2.0, 0.3],
            [0.25, 0.1, 0.3, 1.5],
        ]);
        assert_eq!(potf2_upper(&mut spd.clone()), 0);
        match invert_symmetric(&spd) {
            Symmetric::Inverse { inverse, rcond } => {
                assert!(product_is_identity(&spd, &inverse, 1e-14));
                assert!(rcond > 0.1 && rcond <= 1.0);
            }
            Symmetric::Singular => panic!("singular"),
        }
        // An axis swap with a zero origin: indefinite, so Bunch-Kaufman with a 2×2 pivot.
        let swap = cm([
            [0.0, 2.0, 0.0, 0.0],
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.5, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(potf2_upper(&mut swap.clone()), 1);
        match invert_symmetric(&swap) {
            Symmetric::Inverse { inverse, .. } => {
                assert!(product_is_identity(&swap, &inverse, 0.0));
            }
            Symmetric::Singular => panic!("singular"),
        }
        let mut singular = swap;
        singular[ix(3, 3)] = 0.0;
        assert!(matches!(invert_symmetric(&singular), Symmetric::Singular));
    }

    #[test]
    fn condition_estimates() {
        // The identity's factors: rcond 1 for every estimate.
        let eye = cm(std::array::from_fn(|i| {
            std::array::from_fn(|j| f64::from(u8::from(i == j)))
        }));
        assert_eq!(dgecon(&eye, 1.0), (1.0, 0));
        assert_eq!(dtrcon(true, &eye), 1.0);
        assert_eq!(dtrcon(false, &eye), 1.0);
        assert_eq!(dpocon_upper(&eye, 1.0), 1.0);
        // An overflowing norm: no estimate (SCI-18's path).
        assert_eq!(dgecon(&eye, f64::INFINITY), (0.0, -5));
        // A badly scaled triangle exercises dlatrs' careful solve.
        let mut t = eye;
        t[ix(1, 1)] = 1e-300;
        t[ix(1, 4)] = 1e300;
        let r = dtrcon(true, &t);
        assert!(r.is_finite() && r < f64::EPSILON);
    }
}

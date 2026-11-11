// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! LAPACK's QR and LQ factorizations of general size (`DGEQRF`, `DGELQF`, blocked, with
//! `DGEQR2`, `DGELQ2`), the block reflector routines under them (`DLARFT`, `DLARFB`) and the
//! single reflectors (`DLARF`, `DLARF1F`), as numpy's x86-64 wheels run them at one BLAS thread
//! on the Haswell kernels (`OPENBLAS_CORETYPE=Haswell`), both LAPACK releases, on every
//! platform: the first steps of `numpy.linalg.svd` (`DGESDD`'s paths for tall and wide
//! matrices), `pinv` and `lstsq` on the confounds' sizes (fMRIPrep's CompCor: up to 600 ×
//! 60,968).
//!
//! **The two releases** ([`LapackRelease`]; `lapack-netlib/SRC/` of OpenBLAS 0.3.30, numpy
//! 2.3.5's wheels, and of OpenBLAS v0.3.34-106-g446c436e, numpy 2.5.4's):
//!
//! - `DGEQR2` and `DGELQ2` apply each reflector with `DLARF` (3.12.0: the reflector's leading 1
//!   stored in `A(i, i)` for the call, `w = Cᵀv`, `C -= tau·v·wᵀ`) or with `DLARF1F` (3.12.1:
//!   the 1 implicit; `w = C(2:, :)ᵀv(2:)`, `w += C(1, :)`, `C(1, :) -= tau·w`, `C(2:, :) -=
//!   tau·v(2:)·wᵀ`, a single row `C(1, :) *= 1 - tau`); `DLARF` itself is the same in both;
//! - `DLARFT` is the recursive algorithm (Elmroth and Gustavson: two half-size `T`, the
//!   off-diagonal block by `DTRMM`, `DGEMM`, `DTRMM`, `DTRMM`) in both, but 3.12.1 hands
//!   every call with fewer than `ILAENV(3, 'DLARFT') = 64` reflectors (the recursion's own
//!   calls included) to `DLARFT_LVL2`, the earlier column-by-column algorithm (`DGEMV`, then
//!   `DTRMV` with the `T` built so far); 3.12.0's `ILAENV` has no such entry (0) and recurses
//!   down to single reflectors. The blocked factorizations call it with `NB = 32` reflectors,
//!   so 3.12.1 always takes `DLARFT_LVL2` there.
//! - `DGEQRF`, `DGELQF`, `DLARFB`, `DLARFG` and `ILAENV`'s `NB = 32`, `NBMIN = 2`, `NX = 128`
//!   for these routines are the same in both.
//!
//! **The BLAS calls** go to [`crate::openblas`]'s ports of the Haswell kernels (`dgemm`,
//! `dtrmm`, `dgemv`, `dger`, `dtrmv`, `daxpy`, `dscal`, `dnrm2`), with the depth block
//! `gemm_q` of [`Lapack`] ([`GEMM_Q`] = 256, the oracles' and a real Haswell's; 512 follows
//! numpy 2.5.4's library forced onto the Haswell kernels on an AMD Zen 4 or 5, for validation,
//! `docs/findings/confounds-blas.md`). gfortran compiled LAPACK itself for the x86-64 baseline
//! (no fused multiply-adds), so the routines' own arithmetic (`DLARFG`, the subtraction that
//! ends `DLARFB`, `-tau·v` in `DLARFT_LVL2`) is plain double arithmetic in the source's order.
//!
//! **Where the bits come from.** The kernels' results do not depend on the leading dimensions
//! of what they pack (`dgemm`, `dtrmm`), so a block that LAPACK passes in the same array as the
//! matrix it updates is copied out first where Rust's borrows need it (the `V` panel of
//! `DGELQF`'s `DLARFB`, a triangle of `T` that `DTRMM` multiplies into another part of `T`);
//! the level-2 calls keep the caller's leading dimensions and increments, which decide some of
//! their paths. Every product of `dgemm` and `dtrmm` is parallel over blocks of its output in
//! the current rayon pool; the bits never depend on the number of threads (the one-thread
//! results of OpenBLAS). The reflectors, the panels and the level-2 calls run in order.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dgeqrf.f`, `dgeqr2.f`, `dgelqf.f`,
//! `dgelq2.f`, `dlarft.f` (both), `dlarft_lvl2.f` (0.3.34), `dlarfb.f`, `dlarf.f`, `dlarf1f.f`
//! (0.3.34), `dlarfg.f`, `dlacpy.f`, `iladlc.f`, `iladlr.f`, `ilaenv.f`, `xerbla.f`); the BLAS
//! files of [`crate::openblas`]. The logic is re-expressed in Rust; no code was copied.

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked
// line by line against the Fortran.
#![allow(clippy::too_many_arguments, clippy::many_single_char_names)]

use super::{LapackRelease, dlarfg, iladlc, iladlr};
use crate::openblas::level1::{daxpy, dscal};
use crate::openblas::level2::{dgemv, dger, dtrmv};
use crate::openblas::level3::{GEMM_Q, Operand, Trans, dgemm_with};
use crate::openblas::trmm::{Diag, Side, Uplo, dtrmm_with};

/// `ILAENV(1, 'DGEQRF' | 'DGELQF', ...)`: the block size of both releases.
pub const NB: usize = 32;
/// `ILAENV(3, 'DGEQRF' | 'DGELQF', ...)`: the crossover to the unblocked code.
pub const NX: usize = 128;
/// `ILAENV(2, ...)`: the smallest block size the blocked code takes.
const NBMIN: usize = 2;
/// `ILAENV(3, 'DLARFT', ...)` of LAPACK 3.12.1: fewer reflectors go to `DLARFT_LVL2`.
pub const DLARFT_LVL2_BELOW: usize = 64;

/// The LAPACK release and the BLAS depth block a blocked routine follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lapack {
    /// LAPACK 3.12.1 (numpy 2.5.4) or 3.12.0 (numpy 2.3.5, fMRIPrep 25.2.6's image).
    pub release: LapackRelease,
    /// OpenBLAS's `GEMM_Q` ([`GEMM_Q`], or [`crate::openblas::level3::GEMM_Q_ZEN4`] to follow
    /// numpy 2.5.4 forced onto the Haswell kernels on an AMD Zen 4 or 5).
    pub gemm_q: usize,
}

impl Lapack {
    /// `release` with the default depth block ([`GEMM_Q`]).
    pub const fn new(release: LapackRelease) -> Self {
        Self {
            release,
            gemm_q: GEMM_Q,
        }
    }
}

impl Default for Lapack {
    fn default() -> Self {
        Self::new(LapackRelease::default())
    }
}

/// The order in which the elementary reflectors make the block (LAPACK's `DIRECT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direct {
    /// `'F'`: `H = H(1) H(2) … H(k)`.
    Forward,
    /// `'B'`: `H = H(k) … H(2) H(1)`.
    Backward,
}

/// How the reflectors' vectors are stored (LAPACK's `STOREV`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storev {
    /// `'C'`: one vector per column of `V`.
    Columnwise,
    /// `'R'`: one vector per row of `V`.
    Rowwise,
}

/// A parameter LAPACK's checks refuse (`INFO = -i`; `XERBLA` reports it and the routine returns
/// with nothing done).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("** On entry to {routine} parameter number {param} had an illegal value")]
pub struct IllegalValue {
    /// The routine's name as `XERBLA` prints it.
    pub routine: &'static str,
    /// The 1-based position of the parameter (`-INFO`).
    pub param: u32,
}

#[inline(always)]
fn ix(off: usize, ld: usize, i: usize, j: usize) -> usize {
    off + (i - 1) + (j - 1) * ld
}

// ------------------------------------------------------------------------------------------
// Single reflectors

/// The first part of `DLARF` and `DLARF1F`: the reflector's last nonzero entry (scanning down
/// to `stop`) and the last nonzero column (left) or row (right) of `C` it meets.
fn scan(
    left: bool,
    m: usize,
    n: usize,
    v: &[f64],
    voff: usize,
    incv: usize,
    tau: f64,
    c: &[f64],
    coff: usize,
    ldc: usize,
    stop: usize,
) -> (usize, usize) {
    if tau == 0.0 {
        return (stop, 0);
    }
    let mut lastv = if left { m } else { n };
    while lastv > stop && v[voff + (lastv - 1) * incv] == 0.0 {
        lastv -= 1;
    }
    if lastv == 0 {
        // `DLARF` with an all-zero `v`: nothing is applied (`ILADLC(0, …)`'s value is unused).
        return (0, 0);
    }
    let lastc = if left {
        iladlc(lastv, n, c, coff, ldc)
    } else {
        iladlr(m, lastv, c, coff, ldc)
    };
    (lastv, lastc)
}

/// `DLARF(SIDE, M, N, V, INCV, TAU, C, LDC, WORK)` with `v` in its own array (`None`: in `c`'s
/// array, as `DGEQR2` and `DGELQ2` pass it), `incv > 0`.
fn dlarf_in(
    side: Side,
    m: usize,
    n: usize,
    v: Option<&[f64]>,
    voff: usize,
    incv: usize,
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    let left = side == Side::Left;
    let (lastv, lastc, work, vg) = {
        let vs: &[f64] = v.unwrap_or(c);
        let (lastv, lastc) = scan(left, m, n, vs, voff, incv, tau, c, coff, ldc, 0);
        if lastv == 0 {
            return;
        }
        let mut work = vec![0.0f64; lastc];
        if left {
            // w(1:lastc) = C(1:lastv, 1:lastc)ᵀ v(1:lastv)
            dgemv(
                Trans::T,
                lastv,
                lastc,
                1.0,
                c,
                coff,
                ldc,
                vs,
                voff,
                incv,
                0.0,
                &mut work,
                0,
                1,
            );
        } else {
            // w(1:lastc) = C(1:lastc, 1:lastv) v(1:lastv)
            dgemv(
                Trans::N,
                lastc,
                lastv,
                1.0,
                c,
                coff,
                ldc,
                vs,
                voff,
                incv,
                0.0,
                &mut work,
                0,
                1,
            );
        }
        // `dger` buffers a strided `x` and reads `y` element by element: its arithmetic does not
        // depend on where the vectors are, so `v` is gathered before `C` is written.
        let vg: Vec<f64> = (0..lastv).map(|t| vs[voff + t * incv]).collect();
        (lastv, lastc, work, vg)
    };
    if left {
        // C(1:lastv, 1:lastc) -= tau · v · wᵀ
        dger(lastv, lastc, -tau, &vg, 0, 1, &work, 0, 1, c, coff, ldc);
    } else {
        // C(1:lastc, 1:lastv) -= tau · w · vᵀ
        dger(lastc, lastv, -tau, &work, 0, 1, &vg, 0, 1, c, coff, ldc);
    }
}

/// `DLARF(SIDE, M, N, V, INCV, TAU, C, LDC, WORK)`: `C ← H·C` (left) or `C·H` (right) with `H =
/// I - tau·v·vᵀ`, `v` as stored (its first element included), `C` of `m × n` at `c[coff + i +
/// j·ldc]`; the same in both releases.
///
/// # Panics
/// For `incv = 0` (this port takes positive increments, as every LAPACK caller passes) and on
/// the BLAS checks.
pub fn dlarf(
    side: Side,
    m: usize,
    n: usize,
    v: &[f64],
    voff: usize,
    incv: usize,
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    assert!(incv > 0, "DLARF: INCV must be positive here");
    dlarf_in(side, m, n, Some(v), voff, incv, tau, c, coff, ldc);
}

/// `DLARF1F` with `v` in its own array or in `c`'s (`None`), `incv > 0`.
fn dlarf1f_in(
    side: Side,
    m: usize,
    n: usize,
    v: Option<&[f64]>,
    voff: usize,
    incv: usize,
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    let left = side == Side::Left;
    if (left && m == 0) || (!left && n == 0) {
        return;
    }
    let (lastv, lastc, work, vg) = {
        let vs: &[f64] = v.unwrap_or(c);
        let (lastv, lastc) = scan(left, m, n, vs, voff, incv, tau, c, coff, ldc, 1);
        if lastc == 0 {
            return;
        }
        if lastv == 1 {
            (lastv, lastc, Vec::new(), Vec::new())
        } else {
            let mut work = vec![0.0f64; lastc];
            let v2 = voff + incv;
            if left {
                // w(1:lastc) = C(2:lastv, 1:lastc)ᵀ v(2:lastv)
                dgemv(
                    Trans::T,
                    lastv - 1,
                    lastc,
                    1.0,
                    c,
                    coff + 1,
                    ldc,
                    vs,
                    v2,
                    incv,
                    0.0,
                    &mut work,
                    0,
                    1,
                );
                // w += C(1, 1:lastc)ᵀ
                daxpy(lastc, 1.0, c, coff, ldc as isize, &mut work, 0, 1);
            } else {
                // w(1:lastc) = C(1:lastc, 2:lastv) v(2:lastv)
                dgemv(
                    Trans::N,
                    lastc,
                    lastv - 1,
                    1.0,
                    c,
                    coff + ldc,
                    ldc,
                    vs,
                    v2,
                    incv,
                    0.0,
                    &mut work,
                    0,
                    1,
                );
                // w += C(1:lastc, 1)
                daxpy(lastc, 1.0, c, coff, 1, &mut work, 0, 1);
            }
            let vg: Vec<f64> = (0..lastv - 1).map(|t| vs[v2 + t * incv]).collect();
            (lastv, lastc, work, vg)
        }
    };
    if left {
        if lastv == 1 {
            // C(1, 1:lastc) *= 1 - tau
            dscal(lastc, 1.0 - tau, c, coff, ldc as isize);
        } else {
            daxpy(lastc, -tau, &work, 0, 1, c, coff, ldc as isize);
            dger(
                lastv - 1,
                lastc,
                -tau,
                &vg,
                0,
                1,
                &work,
                0,
                1,
                c,
                coff + 1,
                ldc,
            );
        }
    } else if lastv == 1 {
        // C(1:lastc, 1) *= 1 - tau
        dscal(lastc, 1.0 - tau, c, coff, 1);
    } else {
        daxpy(lastc, -tau, &work, 0, 1, c, coff, 1);
        dger(
            lastc,
            lastv - 1,
            -tau,
            &work,
            0,
            1,
            &vg,
            0,
            1,
            c,
            coff + ldc,
            ldc,
        );
    }
}

/// `DLARF1F(SIDE, M, N, V, INCV, TAU, C, LDC, WORK)` (LAPACK 3.12.1): [`dlarf`] with the
/// reflector's first element taken as 1 and never read.
///
/// # Panics
/// As [`dlarf`].
pub fn dlarf1f(
    side: Side,
    m: usize,
    n: usize,
    v: &[f64],
    voff: usize,
    incv: usize,
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    assert!(incv > 0, "DLARF1F: INCV must be positive here");
    dlarf1f_in(side, m, n, Some(v), voff, incv, tau, c, coff, ldc);
}

/// One reflector of `DGEQR2`/`DGELQ2`, `v` at `voff` in `a` itself: 3.12.0 stores the 1 in
/// `v(1)` for the call (`DLARF`) and restores it, 3.12.1 calls `DLARF1F`.
fn apply_in_place(
    release: LapackRelease,
    side: Side,
    m: usize,
    n: usize,
    a: &mut [f64],
    voff: usize,
    incv: usize,
    tau: f64,
    coff: usize,
    ldc: usize,
) {
    match release {
        LapackRelease::V3_12_0 => {
            let aii = a[voff];
            a[voff] = 1.0;
            dlarf_in(side, m, n, None, voff, incv, tau, a, coff, ldc);
            a[voff] = aii;
        }
        LapackRelease::V3_12_1 => dlarf1f_in(side, m, n, None, voff, incv, tau, a, coff, ldc),
    }
}

// ------------------------------------------------------------------------------------------
// Unblocked factorizations

fn check_lda(routine: &'static str, m: usize, lda: usize) -> Result<(), IllegalValue> {
    if lda < m.max(1) {
        return Err(IllegalValue { routine, param: 4 });
    }
    Ok(())
}

/// `DGEQR2(M, N, A, LDA, TAU, WORK, INFO)`: `A = Q·R` column by column, `A` of `m × n` at
/// `a[aoff + i + j·lda]` overwritten with `R` and the reflectors below it, `tau` (`min(m, n)`
/// values) with their scalars.
///
/// # Errors
/// `LDA < max(1, M)` (parameter 4).
pub fn dgeqr2(
    release: LapackRelease,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &mut [f64],
) -> Result<(), IllegalValue> {
    check_lda("DGEQR2", m, lda)?;
    let k = m.min(n);
    for i in 1..=k {
        // H(i) annihilates A(i+1:m, i).
        let aii = ix(aoff, lda, i, i);
        let mut alpha = a[aii];
        tau[i - 1] = dlarfg(
            m - i + 1,
            &mut alpha,
            a,
            ix(aoff, lda, (i + 1).min(m), i),
            1,
        );
        a[aii] = alpha;
        if i < n {
            // A(i:m, i+1:n) ← H(i)·A(i:m, i+1:n)
            apply_in_place(
                release,
                Side::Left,
                m - i + 1,
                n - i,
                a,
                aii,
                1,
                tau[i - 1],
                ix(aoff, lda, i, i + 1),
                lda,
            );
        }
    }
    Ok(())
}

/// `DGELQ2(M, N, A, LDA, TAU, WORK, INFO)`: `A = L·Q` row by row (as [`dgeqr2`], the reflectors
/// stored in the rows right of `L`).
///
/// # Errors
/// `LDA < max(1, M)` (parameter 4).
pub fn dgelq2(
    release: LapackRelease,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &mut [f64],
) -> Result<(), IllegalValue> {
    check_lda("DGELQ2", m, lda)?;
    let k = m.min(n);
    for i in 1..=k {
        // H(i) annihilates A(i, i+1:n).
        let aii = ix(aoff, lda, i, i);
        let mut alpha = a[aii];
        tau[i - 1] = dlarfg(
            n - i + 1,
            &mut alpha,
            a,
            ix(aoff, lda, i, (i + 1).min(n)),
            lda,
        );
        a[aii] = alpha;
        if i < m {
            // A(i+1:m, i:n) ← A(i+1:m, i:n)·H(i)
            apply_in_place(
                release,
                Side::Right,
                m - i,
                n - i + 1,
                a,
                aii,
                lda,
                tau[i - 1],
                ix(aoff, lda, i + 1, i),
                lda,
            );
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------
// DLARFT

/// `DTRMM` whose triangle `A` lies in `t` beside the block `B` it multiplies (`DLARFT`'s last
/// calls): the triangle's square block is copied out first (`dtrmm` packs it; its leading
/// dimension does not change the bits).
fn trmm_in_t(
    lp: Lapack,
    side: Side,
    uplo: Uplo,
    transa: Trans,
    diag: Diag,
    m: usize,
    n: usize,
    alpha: f64,
    t: &mut [f64],
    aoff: usize,
    ldt: usize,
    boff: usize,
) {
    let na = if side == Side::Left { m } else { n };
    if m == 0 || n == 0 {
        return;
    }
    let mut ablk = vec![0.0f64; na * na];
    for j in 0..na {
        for i in 0..na {
            ablk[i + j * na] = t[aoff + i + j * ldt];
        }
    }
    dtrmm_with(
        side, uplo, transa, diag, m, n, alpha, &ablk, 0, na, t, boff, ldt, lp.gemm_q,
    );
}

/// `DLARFT(DIRECT, STOREV, N, K, V, LDV, TAU, T, LDT)`: the triangular factor `T` of the block
/// reflector `H = I - V·T·Vᵀ` of `k` reflectors of order `n` (`T` upper for [`Direct::Forward`],
/// lower for [`Direct::Backward`]; the other triangle of `t` is not touched), `V` at `v[voff +
/// i + j·ldv]`, `T` at `t[toff + i + j·ldt]`. LAPACK 3.12.0 recurses down to single
/// reflectors; 3.12.1 takes `DLARFT_LVL2` below [`DLARFT_LVL2_BELOW`] reflectors (module
/// documentation).
///
/// # Panics
/// When an array is too small, and on the BLAS checks.
pub fn dlarft(
    lp: Lapack,
    direct: Direct,
    storev: Storev,
    n: usize,
    k: usize,
    v: &[f64],
    voff: usize,
    ldv: usize,
    tau: &[f64],
    t: &mut [f64],
    toff: usize,
    ldt: usize,
) {
    if n == 0 || k == 0 {
        return;
    }
    if n == 1 || k == 1 {
        t[toff] = tau[0];
        return;
    }
    if lp.release == LapackRelease::V3_12_1 && k < DLARFT_LVL2_BELOW {
        dlarft_lvl2(direct, storev, n, k, v, voff, ldv, tau, t, toff, ldt);
        return;
    }
    let l = k / 2;
    let vi = |i: usize, j: usize| ix(voff, ldv, i, j);
    let ti = |i: usize, j: usize| ix(toff, ldt, i, j);
    let q = lp.gemm_q;
    match (direct, storev) {
        (Direct::Forward, Storev::Columnwise) => {
            dlarft(lp, direct, storev, n, l, v, voff, ldv, tau, t, toff, ldt);
            dlarft(
                lp,
                direct,
                storev,
                n - l,
                k - l,
                v,
                vi(l + 1, l + 1),
                ldv,
                &tau[l..],
                t,
                ti(l + 1, l + 1),
                ldt,
            );
            // T(1:l, l+1:k) = V(l+1:k, 1:l)ᵀ
            for j in 1..=l {
                for i in 1..=k - l {
                    t[ti(j, l + i)] = v[vi(l + i, j)];
                }
            }
            dtrmm_with(
                Side::Right,
                Uplo::Lower,
                Trans::N,
                Diag::Unit,
                l,
                k - l,
                1.0,
                v,
                vi(l + 1, l + 1),
                ldv,
                t,
                ti(1, l + 1),
                ldt,
                q,
            );
            dgemm_with(
                l,
                k - l,
                n - k,
                1.0,
                Operand {
                    data: v,
                    off: vi(k + 1, 1),
                    ld: ldv,
                    trans: Trans::T,
                },
                Operand {
                    data: v,
                    off: vi(k + 1, l + 1),
                    ld: ldv,
                    trans: Trans::N,
                },
                1.0,
                t,
                ti(1, l + 1),
                ldt,
                q,
            );
            trmm_in_t(
                lp,
                Side::Left,
                Uplo::Upper,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                -1.0,
                t,
                ti(1, 1),
                ldt,
                ti(1, l + 1),
            );
            trmm_in_t(
                lp,
                Side::Right,
                Uplo::Upper,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                1.0,
                t,
                ti(l + 1, l + 1),
                ldt,
                ti(1, l + 1),
            );
        }
        (Direct::Forward, Storev::Rowwise) => {
            dlarft(lp, direct, storev, n, l, v, voff, ldv, tau, t, toff, ldt);
            dlarft(
                lp,
                direct,
                storev,
                n - l,
                k - l,
                v,
                vi(l + 1, l + 1),
                ldv,
                &tau[l..],
                t,
                ti(l + 1, l + 1),
                ldt,
            );
            // DLACPY('All', l, k-l, V(1, l+1), LDV, T(1, l+1), LDT)
            for j in 1..=k - l {
                for i in 1..=l {
                    t[ti(i, l + j)] = v[vi(i, l + j)];
                }
            }
            dtrmm_with(
                Side::Right,
                Uplo::Upper,
                Trans::T,
                Diag::Unit,
                l,
                k - l,
                1.0,
                v,
                vi(l + 1, l + 1),
                ldv,
                t,
                ti(1, l + 1),
                ldt,
                q,
            );
            dgemm_with(
                l,
                k - l,
                n - k,
                1.0,
                Operand {
                    data: v,
                    off: vi(1, k + 1),
                    ld: ldv,
                    trans: Trans::N,
                },
                Operand {
                    data: v,
                    off: vi(l + 1, k + 1),
                    ld: ldv,
                    trans: Trans::T,
                },
                1.0,
                t,
                ti(1, l + 1),
                ldt,
                q,
            );
            trmm_in_t(
                lp,
                Side::Left,
                Uplo::Upper,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                -1.0,
                t,
                ti(1, 1),
                ldt,
                ti(1, l + 1),
            );
            trmm_in_t(
                lp,
                Side::Right,
                Uplo::Upper,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                1.0,
                t,
                ti(l + 1, l + 1),
                ldt,
                ti(1, l + 1),
            );
        }
        (Direct::Backward, Storev::Columnwise) => {
            dlarft(
                lp,
                direct,
                storev,
                n - l,
                k - l,
                v,
                voff,
                ldv,
                tau,
                t,
                toff,
                ldt,
            );
            dlarft(
                lp,
                direct,
                storev,
                n,
                l,
                v,
                vi(1, k - l + 1),
                ldv,
                &tau[k - l..],
                t,
                ti(k - l + 1, k - l + 1),
                ldt,
            );
            // T(k-l+1:k, 1:k-l) = V(n-k+1:n-l, k-l+1:k)ᵀ
            for j in 1..=k - l {
                for i in 1..=l {
                    t[ti(k - l + i, j)] = v[vi(n - k + j, k - l + i)];
                }
            }
            dtrmm_with(
                Side::Right,
                Uplo::Upper,
                Trans::N,
                Diag::Unit,
                l,
                k - l,
                1.0,
                v,
                vi(n - k + 1, 1),
                ldv,
                t,
                ti(k - l + 1, 1),
                ldt,
                q,
            );
            dgemm_with(
                l,
                k - l,
                n - k,
                1.0,
                Operand {
                    data: v,
                    off: vi(1, k - l + 1),
                    ld: ldv,
                    trans: Trans::T,
                },
                Operand {
                    data: v,
                    off: vi(1, 1),
                    ld: ldv,
                    trans: Trans::N,
                },
                1.0,
                t,
                ti(k - l + 1, 1),
                ldt,
                q,
            );
            trmm_in_t(
                lp,
                Side::Left,
                Uplo::Lower,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                -1.0,
                t,
                ti(k - l + 1, k - l + 1),
                ldt,
                ti(k - l + 1, 1),
            );
            trmm_in_t(
                lp,
                Side::Right,
                Uplo::Lower,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                1.0,
                t,
                ti(1, 1),
                ldt,
                ti(k - l + 1, 1),
            );
        }
        (Direct::Backward, Storev::Rowwise) => {
            dlarft(
                lp,
                direct,
                storev,
                n - l,
                k - l,
                v,
                voff,
                ldv,
                tau,
                t,
                toff,
                ldt,
            );
            dlarft(
                lp,
                direct,
                storev,
                n,
                l,
                v,
                vi(k - l + 1, 1),
                ldv,
                &tau[k - l..],
                t,
                ti(k - l + 1, k - l + 1),
                ldt,
            );
            // DLACPY('All', l, k-l, V(k-l+1, n-k+1), LDV, T(k-l+1, 1), LDT)
            for j in 1..=k - l {
                for i in 1..=l {
                    t[ti(k - l + i, j)] = v[vi(k - l + i, n - k + j)];
                }
            }
            dtrmm_with(
                Side::Right,
                Uplo::Lower,
                Trans::T,
                Diag::Unit,
                l,
                k - l,
                1.0,
                v,
                vi(1, n - k + 1),
                ldv,
                t,
                ti(k - l + 1, 1),
                ldt,
                q,
            );
            dgemm_with(
                l,
                k - l,
                n - k,
                1.0,
                Operand {
                    data: v,
                    off: vi(k - l + 1, 1),
                    ld: ldv,
                    trans: Trans::N,
                },
                Operand {
                    data: v,
                    off: vi(1, 1),
                    ld: ldv,
                    trans: Trans::T,
                },
                1.0,
                t,
                ti(k - l + 1, 1),
                ldt,
                q,
            );
            trmm_in_t(
                lp,
                Side::Left,
                Uplo::Lower,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                -1.0,
                t,
                ti(k - l + 1, k - l + 1),
                ldt,
                ti(k - l + 1, 1),
            );
            trmm_in_t(
                lp,
                Side::Right,
                Uplo::Lower,
                Trans::N,
                Diag::NonUnit,
                l,
                k - l,
                1.0,
                t,
                ti(1, 1),
                ldt,
                ti(k - l + 1, 1),
            );
        }
    }
}

/// `DTRMV(UPLO, 'N', 'N', len, T(a), LDT, T(x), 1)` with the triangle and the vector both in
/// `t`: the vector is taken out for the call (`dtrmv` buffers it anyway; no effect on the bits).
fn trmv_in_t(uplo: Uplo, len: usize, t: &mut [f64], aoff: usize, ldt: usize, xoff: usize) {
    if len == 0 {
        return;
    }
    let mut x: Vec<f64> = t[xoff..xoff + len].to_vec();
    dtrmv(
        uplo,
        Trans::N,
        Diag::NonUnit,
        len,
        t,
        aoff,
        ldt,
        &mut x,
        0,
        1,
    );
    t[xoff..xoff + len].copy_from_slice(&x);
}

/// `DLARFT_LVL2` (LAPACK 3.12.1): `T` column by column (forward) or backwards (backward), each
/// from a `DGEMV` over the reflectors' overlapping parts and a `DTRMV` with the `T` built so
/// far.
fn dlarft_lvl2(
    direct: Direct,
    storev: Storev,
    n: usize,
    k: usize,
    v: &[f64],
    voff: usize,
    ldv: usize,
    tau: &[f64],
    t: &mut [f64],
    toff: usize,
    ldt: usize,
) {
    if n == 0 {
        return;
    }
    let vat = |i: usize, j: usize| v[ix(voff, ldv, i, j)];
    let ti = |i: usize, j: usize| ix(toff, ldt, i, j);
    let col = storev == Storev::Columnwise;
    if direct == Direct::Forward {
        let mut prevlastv = n;
        for i in 1..=k {
            prevlastv = prevlastv.max(i);
            let taui = tau[i - 1];
            if taui == 0.0 {
                // H(i) = I
                for j in 1..=i {
                    t[ti(j, i)] = 0.0;
                }
                continue;
            }
            // The last nonzero entry of v(i) past its unit element (`i` when none).
            let mut lastv = i;
            for lv in (i + 1..=n).rev() {
                let x = if col { vat(lv, i) } else { vat(i, lv) };
                if x != 0.0 {
                    lastv = lv;
                    break;
                }
            }
            for j in 1..i {
                let x = if col { vat(i, j) } else { vat(j, i) };
                t[ti(j, i)] = -taui * x;
            }
            let jj = lastv.min(prevlastv);
            if col {
                // T(1:i-1, i) += -tau(i) · V(i+1:j, 1:i-1)ᵀ V(i+1:j, i)
                dgemv(
                    Trans::T,
                    jj - i,
                    i - 1,
                    -taui,
                    v,
                    ix(voff, ldv, i + 1, 1),
                    ldv,
                    v,
                    ix(voff, ldv, i + 1, i),
                    1,
                    1.0,
                    t,
                    ti(1, i),
                    1,
                );
            } else {
                // T(1:i-1, i) += -tau(i) · V(1:i-1, i+1:j) V(i, i+1:j)ᵀ
                dgemv(
                    Trans::N,
                    i - 1,
                    jj - i,
                    -taui,
                    v,
                    ix(voff, ldv, 1, i + 1),
                    ldv,
                    v,
                    ix(voff, ldv, i, i + 1),
                    ldv,
                    1.0,
                    t,
                    ti(1, i),
                    1,
                );
            }
            // T(1:i-1, i) ← T(1:i-1, 1:i-1) · T(1:i-1, i)
            trmv_in_t(Uplo::Upper, i - 1, t, ti(1, 1), ldt, ti(1, i));
            t[ti(i, i)] = taui;
            prevlastv = if i > 1 { prevlastv.max(lastv) } else { lastv };
        }
    } else {
        let mut prevlastv = 1;
        for i in (1..=k).rev() {
            let taui = tau[i - 1];
            if taui == 0.0 {
                for j in i..=k {
                    t[ti(j, i)] = 0.0;
                }
                continue;
            }
            if i < k {
                // The first nonzero entry of v(i) (`i` when none of the first i-1 is).
                let mut lastv = i;
                for lv in 1..i {
                    let x = if col { vat(lv, i) } else { vat(i, lv) };
                    if x != 0.0 {
                        lastv = lv;
                        break;
                    }
                }
                for j in i + 1..=k {
                    let x = if col {
                        vat(n - k + i, j)
                    } else {
                        vat(j, n - k + i)
                    };
                    t[ti(j, i)] = -taui * x;
                }
                let jj = lastv.max(prevlastv);
                if col {
                    // T(i+1:k, i) += -tau(i) · V(j:n-k+i, i+1:k)ᵀ V(j:n-k+i, i)
                    dgemv(
                        Trans::T,
                        n - k + i - jj,
                        k - i,
                        -taui,
                        v,
                        ix(voff, ldv, jj, i + 1),
                        ldv,
                        v,
                        ix(voff, ldv, jj, i),
                        1,
                        1.0,
                        t,
                        ti(i + 1, i),
                        1,
                    );
                } else {
                    // T(i+1:k, i) += -tau(i) · V(i+1:k, j:n-k+i) V(i, j:n-k+i)ᵀ
                    dgemv(
                        Trans::N,
                        k - i,
                        n - k + i - jj,
                        -taui,
                        v,
                        ix(voff, ldv, i + 1, jj),
                        ldv,
                        v,
                        ix(voff, ldv, i, jj),
                        ldv,
                        1.0,
                        t,
                        ti(i + 1, i),
                        1,
                    );
                }
                // T(i+1:k, i) ← T(i+1:k, i+1:k) · T(i+1:k, i)
                trmv_in_t(Uplo::Lower, k - i, t, ti(i + 1, i + 1), ldt, ti(i + 1, i));
                prevlastv = if i > 1 { prevlastv.min(lastv) } else { lastv };
            }
            t[ti(i, i)] = taui;
        }
    }
}

// ------------------------------------------------------------------------------------------
// DLARFB

/// `DLARFB(SIDE, TRANS, DIRECT, STOREV, M, N, K, V, LDV, T, LDT, C, LDC, WORK, LDWORK)`: `C ←
/// H·C`, `Hᵀ·C`, `C·H` or `C·Hᵀ` for the block reflector `H = I - V·T·Vᵀ` of `k` reflectors
/// ([`dlarft`]'s `T`), `C` of `m × n` at `c[coff + i + j·ldc]`, `V` and `T` in their own
/// arrays; the workspace (`n × k` from the left, `m × k` from the right) is allocated here. The
/// same in both releases; `gemm_q` is the BLAS depth block.
///
/// # Panics
/// When an array is too small, on the BLAS checks, and (the right-side `DTRMM` port's limit)
/// for `k > TRMM_RIGHT_MAX_N`.
pub fn dlarfb(
    gemm_q: usize,
    side: Side,
    trans: Trans,
    direct: Direct,
    storev: Storev,
    m: usize,
    n: usize,
    k: usize,
    v: &[f64],
    voff: usize,
    ldv: usize,
    t: &[f64],
    toff: usize,
    ldt: usize,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    if m == 0 || n == 0 {
        return;
    }
    let transt = match trans {
        Trans::N => Trans::T,
        Trans::T => Trans::N,
    };
    let q = gemm_q;
    let left = side == Side::Left;
    // WORK is `n × k` (left) or `m × k` (right), LDWORK its rows.
    let ldw = if left { n } else { m };
    let mut w = vec![0.0f64; ldw.max(1) * k];
    let vi = |i: usize, j: usize| ix(voff, ldv, i, j);
    let ci = |i: usize, j: usize| ix(coff, ldc, i, j);
    // `C1` is the part of `C` the reflectors' unit triangle meets (its first k rows or
    // columns forward, its last k backward), `C2` the rest; `V1` and `V2` likewise.
    let fwd = direct == Direct::Forward;
    let col = storev == Storev::Columnwise;
    // Rows (left) or columns (right) of C1, as 1-based starts, and of C2.
    let len = if left { m } else { n };
    assert!(k <= len, "DLARFB: K ({k}) exceeds the order of H ({len})");
    let (c1, c2) = if fwd { (1, k + 1) } else { (len - k + 1, 1) };
    // The unit triangle of V and its other block: (offset of V1, offset of V2).
    let (v1, v2) = if col {
        if fwd {
            (vi(1, 1), vi(k + 1, 1))
        } else {
            (vi(len - k + 1, 1), vi(1, 1))
        }
    } else if fwd {
        (vi(1, 1), vi(1, k + 1))
    } else {
        (vi(1, len - k + 1), vi(1, 1))
    };
    // V1's triangle: columnwise forward lower, backward upper; rowwise forward upper, backward
    // lower. `W·V1` uses V1 as stored for columnwise, transposed for rowwise; `W·V1ᵀ` the
    // other way round.
    let v1_uplo = match (col, fwd) {
        (true, true) | (false, false) => Uplo::Lower,
        (true, false) | (false, true) => Uplo::Upper,
    };
    let (v1_first, v1_last) = if col {
        (Trans::N, Trans::T)
    } else {
        (Trans::T, Trans::N)
    };
    // T's triangle: upper forward, lower backward.
    let t_uplo = if fwd { Uplo::Upper } else { Uplo::Lower };
    let rest = len - k;
    if left {
        // W = C1ᵀ (n × k)
        for j in 1..=k {
            for i in 1..=n {
                w[(i - 1) + (j - 1) * ldw] = c[ci(c1 + j - 1, i)];
            }
        }
        // W ← W · V1 (V1 as the columnwise layout needs it)
        dtrmm_with(
            Side::Right,
            v1_uplo,
            v1_first,
            Diag::Unit,
            n,
            k,
            1.0,
            v,
            v1,
            ldv,
            &mut w,
            0,
            ldw,
            q,
        );
        if m > k {
            // W ← W + C2ᵀ · V2 (columnwise) or C2ᵀ · V2ᵀ (rowwise)
            dgemm_with(
                n,
                k,
                rest,
                1.0,
                Operand {
                    data: c,
                    off: ci(c2, 1),
                    ld: ldc,
                    trans: Trans::T,
                },
                Operand {
                    data: v,
                    off: v2,
                    ld: ldv,
                    trans: if col { Trans::N } else { Trans::T },
                },
                1.0,
                &mut w,
                0,
                ldw,
                q,
            );
        }
        // W ← W · Tᵀ or W · T
        dtrmm_with(
            Side::Right,
            t_uplo,
            transt,
            Diag::NonUnit,
            n,
            k,
            1.0,
            t,
            toff,
            ldt,
            &mut w,
            0,
            ldw,
            q,
        );
        if m > k {
            // C2 ← C2 - V2 · Wᵀ (columnwise) or V2ᵀ · Wᵀ (rowwise)
            dgemm_with(
                rest,
                n,
                k,
                -1.0,
                Operand {
                    data: v,
                    off: v2,
                    ld: ldv,
                    trans: if col { Trans::N } else { Trans::T },
                },
                Operand {
                    data: &w,
                    off: 0,
                    ld: ldw,
                    trans: Trans::T,
                },
                1.0,
                c,
                ci(c2, 1),
                ldc,
                q,
            );
        }
        // W ← W · V1ᵀ
        dtrmm_with(
            Side::Right,
            v1_uplo,
            v1_last,
            Diag::Unit,
            n,
            k,
            1.0,
            v,
            v1,
            ldv,
            &mut w,
            0,
            ldw,
            q,
        );
        // C1 ← C1 - Wᵀ
        for j in 1..=k {
            for i in 1..=n {
                let p = ci(c1 + j - 1, i);
                c[p] -= w[(i - 1) + (j - 1) * ldw];
            }
        }
    } else {
        // W = C1 (m × k)
        for j in 1..=k {
            for i in 1..=m {
                w[(i - 1) + (j - 1) * ldw] = c[ci(i, c1 + j - 1)];
            }
        }
        // W ← W · V1
        dtrmm_with(
            Side::Right,
            v1_uplo,
            v1_first,
            Diag::Unit,
            m,
            k,
            1.0,
            v,
            v1,
            ldv,
            &mut w,
            0,
            ldw,
            q,
        );
        if n > k {
            // W ← W + C2 · V2 (columnwise) or C2 · V2ᵀ (rowwise)
            dgemm_with(
                m,
                k,
                rest,
                1.0,
                Operand {
                    data: c,
                    off: ci(1, c2),
                    ld: ldc,
                    trans: Trans::N,
                },
                Operand {
                    data: v,
                    off: v2,
                    ld: ldv,
                    trans: if col { Trans::N } else { Trans::T },
                },
                1.0,
                &mut w,
                0,
                ldw,
                q,
            );
        }
        // W ← W · T or W · Tᵀ
        dtrmm_with(
            Side::Right,
            t_uplo,
            trans,
            Diag::NonUnit,
            m,
            k,
            1.0,
            t,
            toff,
            ldt,
            &mut w,
            0,
            ldw,
            q,
        );
        if n > k {
            // C2 ← C2 - W · V2ᵀ (columnwise) or W · V2 (rowwise)
            dgemm_with(
                m,
                rest,
                k,
                -1.0,
                Operand {
                    data: &w,
                    off: 0,
                    ld: ldw,
                    trans: Trans::N,
                },
                Operand {
                    data: v,
                    off: v2,
                    ld: ldv,
                    trans: if col { Trans::T } else { Trans::N },
                },
                1.0,
                c,
                ci(1, c2),
                ldc,
                q,
            );
        }
        // W ← W · V1ᵀ
        dtrmm_with(
            Side::Right,
            v1_uplo,
            v1_last,
            Diag::Unit,
            m,
            k,
            1.0,
            v,
            v1,
            ldv,
            &mut w,
            0,
            ldw,
            q,
        );
        // C1 ← C1 - W
        for j in 1..=k {
            for i in 1..=m {
                let p = ci(i, c1 + j - 1);
                c[p] -= w[(i - 1) + (j - 1) * ldw];
            }
        }
    }
}

// ------------------------------------------------------------------------------------------
// Blocked factorizations

/// `DGEQRF`'s and `DGELQF`'s optimal workspace (the `LWORK = -1` query): `n·NB` (QR) or `m·NB`
/// (LQ), 1 for an empty matrix.
pub fn lwork_opt(m: usize, n: usize, lq: bool) -> usize {
    if m.min(n) == 0 {
        1
    } else if lq {
        m * NB
    } else {
        n * NB
    }
}

/// The blocking `DGEQRF`/`DGELQF` choose: `(nb, nx, iws)`; blocked when `nb >= NBMIN`, `nb < k`
/// and `nx < k`.
fn blocking(k: usize, ldwork: usize, lwork: usize) -> (usize, usize, usize) {
    let mut nb = NB;
    let mut nx = 0;
    let mut iws = ldwork;
    if nb > 1 && nb < k {
        nx = NX;
        if nx < k {
            iws = ldwork * nb;
            if lwork < iws {
                nb = lwork / ldwork;
            }
        }
    }
    (nb, nx, iws)
}

/// `DGEQRF(M, N, A, LDA, TAU, WORK, LWORK, INFO)`: `A = Q·R`, blocked by [`NB`] columns (each
/// panel by [`dgeqr2`], the rest of the matrix by [`dlarft`] and [`dlarfb`]) while more than
/// [`NX`] columns remain, as [`dgeqr2`] otherwise. `lwork` is the workspace the caller gives
/// (`None`: the optimal, [`lwork_opt`], as numpy's queries give it; a smaller one lowers the
/// block size as LAPACK does). Returns `WORK(1)`, the workspace the blocking used.
///
/// # Errors
/// `LDA < max(1, M)` (4), `LWORK < max(1, N)` with `M > 0`, or `LWORK = 0` (7).
pub fn dgeqrf(
    lp: Lapack,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &mut [f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    check_lda("DGEQRF", m, lda)?;
    let lwork = lwork.unwrap_or_else(|| lwork_opt(m, n, false));
    if lwork == 0 || (m > 0 && lwork < n.max(1)) {
        return Err(IllegalValue {
            routine: "DGEQRF",
            param: 7,
        });
    }
    let k = m.min(n);
    if k == 0 {
        return Ok(1);
    }
    let (nb, nx, iws) = blocking(k, n, lwork);
    let mut i = 1;
    if nb >= NBMIN && nb < k && nx < k {
        let mut t = vec![0.0f64; nb * nb];
        while i <= k - nx {
            let ib = (k - i + 1).min(nb);
            // The panel A(i:m, i:i+ib-1).
            dgeqr2(
                lp.release,
                m - i + 1,
                ib,
                a,
                ix(aoff, lda, i, i),
                lda,
                &mut tau[i - 1..],
            )?;
            if i + ib <= n {
                let (vpart, cpart) = a.split_at_mut(ix(aoff, lda, 1, i + ib));
                let voff = ix(aoff, lda, i, i);
                dlarft(
                    lp,
                    Direct::Forward,
                    Storev::Columnwise,
                    m - i + 1,
                    ib,
                    vpart,
                    voff,
                    lda,
                    &tau[i - 1..],
                    &mut t,
                    0,
                    ib,
                );
                // A(i:m, i+ib:n) ← Hᵀ · A(i:m, i+ib:n)
                dlarfb(
                    lp.gemm_q,
                    Side::Left,
                    Trans::T,
                    Direct::Forward,
                    Storev::Columnwise,
                    m - i + 1,
                    n - i - ib + 1,
                    ib,
                    vpart,
                    voff,
                    lda,
                    &t,
                    0,
                    ib,
                    cpart,
                    i - 1,
                    lda,
                );
            }
            i += nb;
        }
    }
    if i <= k {
        dgeqr2(
            lp.release,
            m - i + 1,
            n - i + 1,
            a,
            ix(aoff, lda, i, i),
            lda,
            &mut tau[i - 1..],
        )?;
    }
    Ok(iws)
}

/// `DGELQF(M, N, A, LDA, TAU, WORK, LWORK, INFO)`: `A = L·Q`, blocked by [`NB`] rows as
/// [`dgeqrf`] (each panel by [`dgelq2`], the rows below by [`dlarft`] and [`dlarfb`] from the
/// right; the panel's reflectors are copied out for `DLARFB`, which writes the rows below them
/// in the same columns). Returns `WORK(1)`.
///
/// # Errors
/// `LDA < max(1, M)` (4), `LWORK < max(1, M)` with `N > 0`, or `LWORK = 0` (7).
pub fn dgelqf(
    lp: Lapack,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &mut [f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    check_lda("DGELQF", m, lda)?;
    let lwork = lwork.unwrap_or_else(|| lwork_opt(m, n, true));
    if lwork == 0 || (n > 0 && lwork < m.max(1)) {
        return Err(IllegalValue {
            routine: "DGELQF",
            param: 7,
        });
    }
    let k = m.min(n);
    if k == 0 {
        return Ok(1);
    }
    let (nb, nx, iws) = blocking(k, m, lwork);
    let mut i = 1;
    if nb >= NBMIN && nb < k && nx < k {
        let mut t = vec![0.0f64; nb * nb];
        let mut panel = vec![0.0f64; nb * n];
        while i <= k - nx {
            let ib = (k - i + 1).min(nb);
            dgelq2(
                lp.release,
                ib,
                n - i + 1,
                a,
                ix(aoff, lda, i, i),
                lda,
                &mut tau[i - 1..],
            )?;
            if i + ib <= m {
                let ncols = n - i + 1;
                dlarft(
                    lp,
                    Direct::Forward,
                    Storev::Rowwise,
                    ncols,
                    ib,
                    a,
                    ix(aoff, lda, i, i),
                    lda,
                    &tau[i - 1..],
                    &mut t,
                    0,
                    ib,
                );
                // V = A(i:i+ib-1, i:n), copied (leading dimension ib).
                for j in 0..ncols {
                    for r in 0..ib {
                        panel[r + j * ib] = a[ix(aoff, lda, i + r, i + j)];
                    }
                }
                // A(i+ib:m, i:n) ← A(i+ib:m, i:n) · H
                dlarfb(
                    lp.gemm_q,
                    Side::Right,
                    Trans::N,
                    Direct::Forward,
                    Storev::Rowwise,
                    m - i - ib + 1,
                    ncols,
                    ib,
                    &panel,
                    0,
                    ib,
                    &t,
                    0,
                    ib,
                    a,
                    ix(aoff, lda, i + ib, i),
                    lda,
                );
            }
            i += nb;
        }
    }
    if i <= k {
        dgelq2(
            lp.release,
            m - i + 1,
            n - i + 1,
            a,
            ix(aoff, lda, i, i),
            lda,
            &mut tau[i - 1..],
        )?;
    }
    Ok(iws)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded matrix of plain arithmetic (no transcendental functions).
    fn matrix(m: usize, n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (0..m * n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64) - 0.5
            })
            .collect()
    }

    /// `Q·R` (or `L·Q`) rebuilt from the factorization, by applying the reflectors one at a
    /// time with [`dlarf`] to `R` (or `L`): close to `A`.
    fn residual(m: usize, n: usize, a0: &[f64], f: &[f64], tau: &[f64], lq: bool) -> f64 {
        let k = m.min(n);
        let mut r = vec![0.0f64; m * n];
        for j in 0..n {
            for i in 0..m {
                let keep = if lq { j <= i } else { i <= j };
                if keep {
                    r[i + j * m] = f[i + j * m];
                }
            }
        }
        for i in (0..k).rev() {
            if lq {
                let mut v = vec![0.0f64; n];
                v[i] = 1.0;
                for (j, vj) in v.iter_mut().enumerate().skip(i + 1) {
                    *vj = f[i + j * m];
                }
                dlarf(Side::Right, m, n, &v, 0, 1, tau[i], &mut r, 0, m);
            } else {
                let mut v = vec![0.0f64; m];
                v[i] = 1.0;
                for (t, vt) in v.iter_mut().enumerate().skip(i + 1) {
                    *vt = f[t + i * m];
                }
                dlarf(Side::Left, m, n, &v, 0, 1, tau[i], &mut r, 0, m);
            }
        }
        r.iter()
            .zip(a0)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn blocked_factorizations_rebuild_the_matrix() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            for &(m, n) in &[(300, 170), (170, 300), (40, 40), (200, 129)] {
                let a0 = matrix(m, n, (m * 7 + n) as u64);
                let k = m.min(n);
                for lq in [false, true] {
                    let mut a = a0.clone();
                    let mut tau = vec![0.0f64; k];
                    if lq {
                        dgelqf(lp, m, n, &mut a, 0, m, &mut tau, None).unwrap();
                    } else {
                        dgeqrf(lp, m, n, &mut a, 0, m, &mut tau, None).unwrap();
                    }
                    let res = residual(m, n, &a0, &a, &tau, lq);
                    assert!(res < 1e-12, "{release:?} {m}x{n} lq={lq}: {res}");
                }
            }
        }
    }

    #[test]
    fn blocked_equals_unblocked_up_to_rounding_and_threads_do_not_matter() {
        let (m, n) = (260, 140);
        let a0 = matrix(m, n, 3);
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            let run = |threads: usize| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let mut a = a0.clone();
                    let mut tau = vec![0.0f64; n];
                    dgeqrf(lp, m, n, &mut a, 0, m, &mut tau, None).unwrap();
                    (a, tau)
                })
            };
            let one = run(1);
            assert_eq!(one, run(5));
            let mut b = a0.clone();
            let mut tb = vec![0.0f64; n];
            dgeqr2(release, m, n, &mut b, 0, m, &mut tb).unwrap();
            let diff = one
                .0
                .iter()
                .zip(&b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f64::max);
            assert!(diff < 1e-11, "{diff}");
        }
    }

    #[test]
    fn checks_and_queries() {
        let lp = Lapack::default();
        let mut a = vec![0.0f64; 12];
        let mut tau = vec![0.0f64; 3];
        let e = dgeqrf(lp, 4, 3, &mut a, 0, 3, &mut tau, None).unwrap_err();
        assert_eq!(e.param, 4);
        let e = dgeqrf(lp, 4, 3, &mut a, 0, 4, &mut tau, Some(2)).unwrap_err();
        assert_eq!((e.routine, e.param), ("DGEQRF", 7));
        let e = dgelqf(lp, 3, 4, &mut a, 0, 3, &mut tau, Some(2)).unwrap_err();
        assert_eq!((e.routine, e.param), ("DGELQF", 7));
        assert_eq!(lwork_opt(600, 60_968, true), 600 * 32);
        assert_eq!(lwork_opt(0, 5, false), 1);
        assert_eq!(dgeqrf(lp, 0, 3, &mut a, 0, 1, &mut tau, None), Ok(1));
    }
}

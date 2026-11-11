// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! LAPACK's QR and LQ factorizations of general size (`DGEQRF`, `DGELQF`, blocked, with
//! `DGEQR2`, `DGELQ2`), the generation and application of their `Q` (`DORGQR`, `DORGLQ`,
//! `DORMQR`, `DORMLQ`, blocked, with `DORG2R`, `DORGL2`, `DORM2R`, `DORML2`), the reduction to
//! bidiagonal form (`DGEBRD`, blocked with `DLABRD`, and `DGEBD2`) with its `Q` and `P`
//! (`DORMBR`, `DORGBR`: identical in both releases, on the routines above), the block reflector routines
//! under them (`DLARFT`, `DLARFB`) and the single reflectors (`DLARF`, `DLARF1F`), as numpy's
//! x86-64 wheels run them at one BLAS thread on the Haswell kernels
//! (`OPENBLAS_CORETYPE=Haswell`), both LAPACK releases, on every platform: the first steps of
//! `numpy.linalg.svd` (`DGESDD`'s paths for tall and wide matrices), `pinv` and `lstsq` on the
//! confounds' sizes (fMRIPrep's CompCor: up to 600 × 60,968).
//!
//! **The two releases** ([`LapackRelease`]; `lapack-netlib/SRC/` of OpenBLAS 0.3.30, numpy
//! 2.3.5's wheels, and of OpenBLAS v0.3.34-106-g446c436e, numpy 2.5.4's):
//!
//! - `DGEQR2` and `DGELQ2` apply each reflector with `DLARF` (3.12.0: the reflector's leading 1
//!   stored in `A(i, i)` for the call, `w = Cᵀv`, `C -= tau·v·wᵀ`) or with `DLARF1F` (3.12.1:
//!   the 1 implicit; `w = C(2:, :)ᵀv(2:)`, `w += C(1, :)`, `C(1, :) -= tau·w`, `C(2:, :) -=
//!   tau·v(2:)·wᵀ`, a single row `C(1, :) *= 1 - tau`); `DLARF` itself is the same in both;
//!   `DORG2R`, `DORGL2`, `DORM2R`, `DORML2` and `DGEBD2` change the same way (3.12.0 stores the
//!   1 for `DLARF`; `DORM2R`/`DORML2` restore the entry afterwards, `DGEBD2` puts `d` or `e`
//!   back);
//! - `DORMQR` and `DORMLQ` keep `T` after `NW·NB` entries of the workspace: 3.12.0 reserves a
//!   fixed `TSIZE = 65·64` for it (optimal workspace `NW·NB + TSIZE`, a short one gives `NB =
//!   (LWORK - TSIZE)/NW`), 3.12.1 `65·NB` (`NW·NB + 65·NB`, `NB = LWORK/(NW + 65)`); with `NB
//!   = 32` both are blocked only for more than 32 reflectors;
//! - `DLARFT` is the recursive algorithm (Elmroth and Gustavson: two half-size `T`, the
//!   off-diagonal block by `DTRMM`, `DGEMM`, `DTRMM`, `DTRMM`) in both, but 3.12.1 hands
//!   every call with fewer than `ILAENV(3, 'DLARFT') = 64` reflectors (the recursion's own
//!   calls included) to `DLARFT_LVL2`, the earlier column-by-column algorithm (`DGEMV`, then
//!   `DTRMV` with the `T` built so far); 3.12.0's `ILAENV` has no such entry (0) and recurses
//!   down to single reflectors. The blocked factorizations call it with `NB = 32` reflectors,
//!   so 3.12.1 always takes `DLARFT_LVL2` there.
//! - `DGEQRF`, `DGELQF`, `DORGQR`, `DORGLQ`, `DGEBRD`, `DLABRD`, `DLARFB`, `DLARFG` and
//!   `ILAENV`'s `NB = 32`, `NBMIN = 2`, `NX = 128` (`DORMQR`/`DORMLQ`: no crossover) for these
//!   routines are the same in both.
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
//! `dgelq2.f`, `dorgqr.f`, `dorg2r.f`, `dorglq.f`, `dorgl2.f`, `dormqr.f`, `dorm2r.f`,
//! `dormlq.f`, `dorml2.f`, `dgebrd.f`, `dlabrd.f`, `dgebd2.f`, `dlarft.f` (both),
//! `dlarft_lvl2.f` (0.3.34), `dlarfb.f`, `dlarf.f`, `dlarf1f.f` (0.3.34), `dlarfg.f`,
//! `dlacpy.f`, `iladlc.f`, `iladlr.f`, `ilaenv.f`, `xerbla.f`); the BLAS files of
//! [`crate::openblas`]. The logic is re-expressed in Rust; no code was copied.

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

// ------------------------------------------------------------------------------------------
// Generating Q (DORGQR, DORGLQ) and applying it (DORMQR, DORMLQ)

/// `ILAENV(1, 'DORMQR' | 'DORMLQ', ...)` is capped by `NBMAX` = 64 in both routines.
pub const NBMAX_ORM: usize = 64;
/// The leading dimension of `T` in `DORMQR` and `DORMLQ` (`LDT = NBMAX + 1`).
pub const LDT_ORM: usize = NBMAX_ORM + 1;
/// LAPACK 3.12.0's fixed room for `T` in `DORMQR`'s and `DORMLQ`'s workspace (`TSIZE = LDT ·
/// NBMAX`); 3.12.1 reserves `LDT · NB` instead.
pub const TSIZE_ORM: usize = LDT_ORM * NBMAX_ORM;

/// `DORGQR`'s (`lq = false`) and `DORGLQ`'s (`lq = true`) optimal workspace, the `LWORK = -1`
/// query: `max(1, N)·NB` (QR) or `max(1, M)·NB` (LQ); the same in both releases.
pub fn org_lwork_opt(m: usize, n: usize, lq: bool) -> usize {
    (if lq { m } else { n }).max(1) * NB
}

/// `DORMQR`'s and `DORMLQ`'s optimal workspace, the `LWORK = -1` query: `NW·NB + TSIZE`
/// (3.12.0) or `NW·NB + LDT·NB` (3.12.1), `NW = max(1, N)` from the left and `max(1, M)` from the
/// right, `NB = min(NBMAX, 32)`, whatever `K` is.
pub fn orm_lwork_opt(release: LapackRelease, side: Side, m: usize, n: usize) -> usize {
    let nw = (if side == Side::Left { n } else { m }).max(1);
    let nb = NB.min(NBMAX_ORM);
    match release {
        LapackRelease::V3_12_0 => nw * nb + TSIZE_ORM,
        LapackRelease::V3_12_1 => nw * nb + LDT_ORM * nb,
    }
}

/// `DORG2R(M, N, K, A, LDA, TAU, WORK, INFO)`: the first `n` columns of `Q = H(1)·…·H(k)` from
/// [`dgeqrf`]'s reflectors, overwriting `A` (`m × n`, `n ≤ m`, `k ≤ n`), reflector by reflector
/// from the last (3.12.0 with `DLARF` and the 1 stored in `A(i, i)`, 3.12.1 with `DLARF1F`);
/// each column then `-tau·v` below `1 - tau` and zeros above.
///
/// # Errors
/// `N > M` (2), `K > N` (3), `LDA < max(1, M)` (5).
pub fn dorg2r(
    release: LapackRelease,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
) -> Result<(), IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORG2R",
            param,
        })
    };
    if n > m {
        return err(2);
    }
    if k > n {
        return err(3);
    }
    if lda < m.max(1) {
        return err(5);
    }
    if n == 0 {
        return Ok(());
    }
    // Columns k+1:n start as columns of the unit matrix.
    for j in k + 1..=n {
        for l in 1..=m {
            a[ix(aoff, lda, l, j)] = 0.0;
        }
        a[ix(aoff, lda, j, j)] = 1.0;
    }
    for i in (1..=k).rev() {
        let aii = ix(aoff, lda, i, i);
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
        if i < m {
            dscal(m - i, -tau[i - 1], a, aii + 1, 1);
        }
        a[aii] = 1.0 - tau[i - 1];
        // A(1:i-1, i) = 0
        for l in 1..i {
            a[ix(aoff, lda, l, i)] = 0.0;
        }
    }
    Ok(())
}

/// `DORGL2(M, N, K, A, LDA, TAU, WORK, INFO)`: the first `m` rows of `Q = H(k)·…·H(1)` from
/// [`dgelqf`]'s reflectors (`m ≤ n`, `k ≤ m`), as [`dorg2r`] row by row.
///
/// # Errors
/// `N < M` (2), `K > M` (3), `LDA < max(1, M)` (5).
pub fn dorgl2(
    release: LapackRelease,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
) -> Result<(), IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORGL2",
            param,
        })
    };
    if n < m {
        return err(2);
    }
    if k > m {
        return err(3);
    }
    if lda < m.max(1) {
        return err(5);
    }
    if m == 0 {
        return Ok(());
    }
    if k < m {
        // Rows k+1:m start as rows of the unit matrix.
        for j in 1..=n {
            for l in k + 1..=m {
                a[ix(aoff, lda, l, j)] = 0.0;
            }
            if j > k && j <= m {
                a[ix(aoff, lda, j, j)] = 1.0;
            }
        }
    }
    for i in (1..=k).rev() {
        let aii = ix(aoff, lda, i, i);
        if i < n {
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
            dscal(n - i, -tau[i - 1], a, aii + lda, lda as isize);
        }
        a[aii] = 1.0 - tau[i - 1];
        // A(i, 1:i-1) = 0
        for l in 1..i {
            a[ix(aoff, lda, i, l)] = 0.0;
        }
    }
    Ok(())
}

/// The blocking `DORGQR`/`DORGLQ` choose: `(nb, nbmin, nx, iws)`; blocked when `nb >= nbmin`,
/// `nb < k` and `nx < k`.
fn org_blocking(k: usize, ldwork: usize, lwork: usize) -> (usize, usize, usize, usize) {
    let mut nb = NB;
    let nbmin = NBMIN;
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
    (nb, nbmin, nx, iws)
}

/// `DORGQR(M, N, K, A, LDA, TAU, WORK, LWORK, INFO)`: the first `n` columns of `Q` from `k` of
/// [`dgeqrf`]'s reflectors, blocked by [`NB`] reflectors from the last block back (each block's
/// `T` by [`dlarft`], applied to the columns right of it by [`dlarfb`], its own columns by
/// [`dorg2r`]) when more than [`NX`] reflectors are given; [`dorg2r`] on the trailing part
/// first. `lwork` as in [`dgeqrf`] (`None`: [`org_lwork_opt`]). Returns `WORK(1)`; the same in
/// both releases but for [`dorg2r`]'s reflectors.
///
/// # Errors
/// `N > M` (2), `K > N` (3), `LDA < max(1, M)` (5), `LWORK < max(1, N)` (8).
pub fn dorgqr(
    lp: Lapack,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORGQR",
            param,
        })
    };
    let lwork = lwork.unwrap_or_else(|| org_lwork_opt(m, n, false));
    if n > m {
        return err(2);
    }
    if k > n {
        return err(3);
    }
    if lda < m.max(1) {
        return err(5);
    }
    if lwork < n.max(1) {
        return err(8);
    }
    if n == 0 {
        return Ok(1);
    }
    let ldwork = n;
    let (nb, nbmin, nx, iws) = org_blocking(k, ldwork, lwork);
    let (ki, kk) = if nb >= nbmin && nb < k && nx < k {
        // The last block starts at ki + 1; the zeros above the blocked part's columns.
        let ki = ((k - nx - 1) / nb) * nb;
        let kk = k.min(ki + nb);
        for j in kk + 1..=n {
            for i in 1..=kk {
                a[ix(aoff, lda, i, j)] = 0.0;
            }
        }
        (ki, kk)
    } else {
        (0, 0)
    };
    if kk < n {
        dorg2r(
            lp.release,
            m - kk,
            n - kk,
            k - kk,
            a,
            ix(aoff, lda, kk + 1, kk + 1),
            lda,
            &tau[kk..],
        )?;
    }
    if kk > 0 {
        // T in WORK with leading dimension LDWORK, as LAPACK keeps it.
        let mut t = vec![0.0f64; ldwork * nb];
        let mut i = ki + 1;
        loop {
            let ib = nb.min(k - i + 1);
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
                    ldwork,
                );
                // A(i:m, i+ib:n) ← H·A(i:m, i+ib:n)
                dlarfb(
                    lp.gemm_q,
                    Side::Left,
                    Trans::N,
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
                    ldwork,
                    cpart,
                    i - 1,
                    lda,
                );
            }
            // The block's own columns, A(i:m, i:i+ib-1).
            dorg2r(
                lp.release,
                m - i + 1,
                ib,
                ib,
                a,
                ix(aoff, lda, i, i),
                lda,
                &tau[i - 1..],
            )?;
            // A(1:i-1, i:i+ib-1) = 0
            for j in i..i + ib {
                for l in 1..i {
                    a[ix(aoff, lda, l, j)] = 0.0;
                }
            }
            if i <= nb {
                break;
            }
            i -= nb;
        }
    }
    Ok(iws)
}

/// `DORGLQ(M, N, K, A, LDA, TAU, WORK, LWORK, INFO)`: the first `m` rows of `Q` from `k` of
/// [`dgelqf`]'s reflectors, as [`dorgqr`] row by row ([`dlarfb`] from the right with `Hᵀ`; the
/// block's reflectors are copied out for it, as in [`dgelqf`]). Returns `WORK(1)`.
///
/// # Errors
/// `N < M` (2), `K > M` (3), `LDA < max(1, M)` (5), `LWORK < max(1, M)` (8).
pub fn dorglq(
    lp: Lapack,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORGLQ",
            param,
        })
    };
    let lwork = lwork.unwrap_or_else(|| org_lwork_opt(m, n, true));
    if n < m {
        return err(2);
    }
    if k > m {
        return err(3);
    }
    if lda < m.max(1) {
        return err(5);
    }
    if lwork < m.max(1) {
        return err(8);
    }
    if m == 0 {
        return Ok(1);
    }
    let ldwork = m;
    let (nb, nbmin, nx, iws) = org_blocking(k, ldwork, lwork);
    let (ki, kk) = if nb >= nbmin && nb < k && nx < k {
        let ki = ((k - nx - 1) / nb) * nb;
        let kk = k.min(ki + nb);
        for j in 1..=kk {
            for i in kk + 1..=m {
                a[ix(aoff, lda, i, j)] = 0.0;
            }
        }
        (ki, kk)
    } else {
        (0, 0)
    };
    if kk < m {
        dorgl2(
            lp.release,
            m - kk,
            n - kk,
            k - kk,
            a,
            ix(aoff, lda, kk + 1, kk + 1),
            lda,
            &tau[kk..],
        )?;
    }
    if kk > 0 {
        let mut t = vec![0.0f64; ldwork * nb];
        let mut panel = vec![0.0f64; nb * n];
        let mut i = ki + 1;
        loop {
            let ib = nb.min(k - i + 1);
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
                    ldwork,
                );
                // V = A(i:i+ib-1, i:n), copied (leading dimension ib).
                for j in 0..ncols {
                    for r in 0..ib {
                        panel[r + j * ib] = a[ix(aoff, lda, i + r, i + j)];
                    }
                }
                // A(i+ib:m, i:n) ← A(i+ib:m, i:n)·Hᵀ
                dlarfb(
                    lp.gemm_q,
                    Side::Right,
                    Trans::T,
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
                    ldwork,
                    a,
                    ix(aoff, lda, i + ib, i),
                    lda,
                );
            }
            // The block's own rows, A(i:i+ib-1, i:n).
            dorgl2(
                lp.release,
                ib,
                n - i + 1,
                ib,
                a,
                ix(aoff, lda, i, i),
                lda,
                &tau[i - 1..],
            )?;
            // A(i:i+ib-1, 1:i-1) = 0
            for j in 1..i {
                for l in i..i + ib {
                    a[ix(aoff, lda, l, j)] = 0.0;
                }
            }
            if i <= nb {
                break;
            }
            i -= nb;
        }
    }
    Ok(iws)
}

/// One reflector of `DORM2R`/`DORML2` (`v` at `voff` in `a`, `C` its own array): 3.12.0 stores
/// the 1 in `v(1)` for `DLARF` and restores the entry, 3.12.1 calls `DLARF1F`.
fn apply_from(
    release: LapackRelease,
    side: Side,
    m: usize,
    n: usize,
    a: &mut [f64],
    voff: usize,
    incv: usize,
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    match release {
        LapackRelease::V3_12_0 => {
            let aii = a[voff];
            a[voff] = 1.0;
            dlarf(side, m, n, a, voff, incv, tau, c, coff, ldc);
            a[voff] = aii;
        }
        LapackRelease::V3_12_1 => dlarf1f(side, m, n, a, voff, incv, tau, c, coff, ldc),
    }
}

/// The checks `DORM2R`, `DORML2`, `DORMQR` and `DORMLQ` share: `K > NQ` (5), `LDA < max(1, NQ)`
/// (QR) or `max(1, K)` (LQ) (7), `LDC < max(1, M)` (10). Returns `NQ`, the order of `Q`.
fn orm_checks(
    routine: &'static str,
    lq: bool,
    side: Side,
    m: usize,
    n: usize,
    k: usize,
    lda: usize,
    ldc: usize,
) -> Result<usize, IllegalValue> {
    let nq = if side == Side::Left { m } else { n };
    let err = |param| Err(IllegalValue { routine, param });
    if k > nq {
        return err(5);
    }
    if lda < (if lq { k } else { nq }).max(1) {
        return err(7);
    }
    if ldc < m.max(1) {
        return err(10);
    }
    Ok(nq)
}

/// `(left, the reflectors in increasing order)` of an `ORM` routine: `Q = H(1)·…·H(k)` (QR) is
/// applied as `H(1)` first for `Qᵀ·C` and `C·Q`; `Q = H(k)·…·H(1)` (LQ) for `Q·C` and `C·Qᵀ`.
fn orm_forward(lq: bool, side: Side, trans: Trans) -> (bool, bool) {
    let left = side == Side::Left;
    let notran = trans == Trans::N;
    let forward = if lq {
        (left && notran) || (!left && !notran)
    } else {
        (left && !notran) || (!left && notran)
    };
    (left, forward)
}

/// `DORM2R`/`DORML2` (`lq`): every reflector applied with [`apply_from`], in order.
fn orm2(
    release: LapackRelease,
    lq: bool,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    let (left, forward) = orm_forward(lq, side, trans);
    let incv = if lq { lda } else { 1 };
    let mut step = |i: usize| {
        let (mi, ni, ic, jc) = if left {
            (m - i + 1, n, i, 1)
        } else {
            (m, n - i + 1, 1, i)
        };
        apply_from(
            release,
            side,
            mi,
            ni,
            a,
            ix(aoff, lda, i, i),
            incv,
            tau[i - 1],
            c,
            ix(coff, ldc, ic, jc),
            ldc,
        );
    };
    if forward {
        (1..=k).for_each(&mut step);
    } else {
        (1..=k).rev().for_each(&mut step);
    }
}

/// `DORM2R(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, INFO)`: `C ← Q·C`, `Qᵀ·C`, `C·Q` or
/// `C·Qᵀ` with `Q = H(1)·…·H(k)` of [`dgeqrf`]'s reflectors in `A` (`NQ × k`, `NQ` = `m` from the
/// left, `n` from the right), one reflector at a time. `A` is changed during the call and
/// restored (3.12.0 stores each reflector's 1 in it).
///
/// # Errors
/// `K > NQ` (5), `LDA < max(1, NQ)` (7), `LDC < max(1, M)` (10).
pub fn dorm2r(
    release: LapackRelease,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) -> Result<(), IllegalValue> {
    orm_checks("DORM2R", false, side, m, n, k, lda, ldc)?;
    orm2(
        release, false, side, trans, m, n, k, a, aoff, lda, tau, c, coff, ldc,
    );
    Ok(())
}

/// `DORML2(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, INFO)`: as [`dorm2r`] with `Q =
/// H(k)·…·H(1)` of [`dgelqf`]'s reflectors in the rows of `A` (`k × NQ`).
///
/// # Errors
/// `K > NQ` (5), `LDA < max(1, K)` (7), `LDC < max(1, M)` (10).
pub fn dorml2(
    release: LapackRelease,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) -> Result<(), IllegalValue> {
    orm_checks("DORML2", true, side, m, n, k, lda, ldc)?;
    orm2(
        release, true, side, trans, m, n, k, a, aoff, lda, tau, c, coff, ldc,
    );
    Ok(())
}

/// `DORMQR`/`DORMLQ` (`lq`) after their checks.
fn orm_blocked(
    lp: Lapack,
    lq: bool,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let routine = if lq { "DORMLQ" } else { "DORMQR" };
    let nq = orm_checks(routine, lq, side, m, n, k, lda, ldc)?;
    let left = side == Side::Left;
    let nw = (if left { n } else { m }).max(1);
    let lwkopt = orm_lwork_opt(lp.release, side, m, n);
    let lwork = lwork.unwrap_or(lwkopt);
    if lwork < nw {
        return Err(IllegalValue { routine, param: 12 });
    }
    if m == 0 || n == 0 || k == 0 {
        return Ok(1);
    }
    // Fortran's integer division (towards zero) on possibly negative numerators.
    let mut nb = NB.min(NBMAX_ORM) as i64;
    let ldwork = nw as i64;
    if nb > 1 && nb < k as i64 && lwork < lwkopt {
        nb = match lp.release {
            LapackRelease::V3_12_0 => (lwork as i64 - TSIZE_ORM as i64) / ldwork,
            LapackRelease::V3_12_1 => lwork as i64 / (ldwork + LDT_ORM as i64),
        };
    }
    if nb < NBMIN as i64 || nb >= k as i64 {
        orm2(
            lp.release, lq, side, trans, m, n, k, a, aoff, lda, tau, c, coff, ldc,
        );
        return Ok(lwkopt);
    }
    let nb = nb as usize;
    let (_, forward) = orm_forward(lq, side, trans);
    // DORMLQ applies the block reflector transposed (`TRANST`).
    let tr = match (lq, trans) {
        (false, t) => t,
        (true, Trans::N) => Trans::T,
        (true, Trans::T) => Trans::N,
    };
    let storev = if lq {
        Storev::Rowwise
    } else {
        Storev::Columnwise
    };
    let starts: Vec<usize> = if forward {
        (1..=k).step_by(nb).collect()
    } else {
        let i1 = ((k - 1) / nb) * nb + 1;
        (0..=(i1 - 1) / nb).map(|s| i1 - s * nb).collect()
    };
    let mut t = vec![0.0f64; LDT_ORM * nb];
    for i in starts {
        let ib = nb.min(k - i + 1);
        let voff = ix(aoff, lda, i, i);
        dlarft(
            lp,
            Direct::Forward,
            storev,
            nq - i + 1,
            ib,
            a,
            voff,
            lda,
            &tau[i - 1..],
            &mut t,
            0,
            LDT_ORM,
        );
        let (mi, ni, ic, jc) = if left {
            (m - i + 1, n, i, 1)
        } else {
            (m, n - i + 1, 1, i)
        };
        dlarfb(
            lp.gemm_q,
            side,
            tr,
            Direct::Forward,
            storev,
            mi,
            ni,
            ib,
            a,
            voff,
            lda,
            &t,
            0,
            LDT_ORM,
            c,
            ix(coff, ldc, ic, jc),
            ldc,
        );
    }
    Ok(lwkopt)
}

/// `DORMQR(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)`: [`dorm2r`]'s product,
/// blocked by [`NB`] reflectors ([`dlarft`]'s `T` with leading dimension [`LDT_ORM`], then
/// [`dlarfb`] on the rows or columns of `C` the block meets) when `NB < K`. A workspace below
/// the optimal ([`orm_lwork_opt`]; `lwork = None` gives it) lowers `NB` as each release does
/// (3.12.0: `(LWORK - TSIZE)/LDWORK`, 3.12.1: `LWORK/(LDWORK + LDT)`); below 2, or `NB >= K`,
/// everything goes to [`dorm2r`]. Returns `WORK(1)` (the optimal workspace; 1 when `C` or `Q`
/// is empty).
///
/// # Errors
/// As [`dorm2r`], and `LWORK < NW` (12).
pub fn dormqr(
    lp: Lapack,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    orm_blocked(
        lp, false, side, trans, m, n, k, a, aoff, lda, tau, c, coff, ldc, lwork,
    )
}

/// `DORMLQ(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)`: [`dorml2`]'s product,
/// blocked as [`dormqr`] (rowwise reflectors; the block reflector applied transposed).
///
/// # Errors
/// As [`dorml2`], and `LWORK < NW` (12).
pub fn dormlq(
    lp: Lapack,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    orm_blocked(
        lp, true, side, trans, m, n, k, a, aoff, lda, tau, c, coff, ldc, lwork,
    )
}

// ------------------------------------------------------------------------------------------
// Bidiagonal reduction (DGEBRD, DLABRD, DGEBD2)

/// `DGEBD2(M, N, A, LDA, D, E, TAUQ, TAUP, WORK, INFO)`: `A = Q·B·Pᵀ` reflector by reflector,
/// `B` upper bidiagonal for `m >= n` (`d`: `n` values, `e`: `n - 1`), lower otherwise (`d`:
/// `m`, `e`: `m - 1`), the reflectors of `Q` and `P` stored below and right of `B` with their
/// scalars `tauq` and `taup` (`min(m, n)` values each; the last one of `taup` (`m >= n`) or
/// `tauq` (`m < n`) is 0). 3.12.0 applies each reflector with `DLARF` (the 1 stored and the
/// entry put back), 3.12.1 with `DLARF1F`.
///
/// # Errors
/// `LDA < max(1, M)` (parameter 4).
pub fn dgebd2(
    release: LapackRelease,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    d: &mut [f64],
    e: &mut [f64],
    tauq: &mut [f64],
    taup: &mut [f64],
) -> Result<(), IllegalValue> {
    if lda < m.max(1) {
        return Err(IllegalValue {
            routine: "DGEBD2",
            param: 4,
        });
    }
    let p = |i: usize, j: usize| ix(aoff, lda, i, j);
    if m >= n {
        for i in 1..=n {
            // H(i) annihilates A(i+1:m, i).
            let mut alpha = a[p(i, i)];
            tauq[i - 1] = dlarfg(m - i + 1, &mut alpha, a, p((i + 1).min(m), i), 1);
            a[p(i, i)] = alpha;
            d[i - 1] = alpha;
            if i < n {
                // A(i:m, i+1:n) ← H(i)·A(i:m, i+1:n)
                apply_in_place(
                    release,
                    Side::Left,
                    m - i + 1,
                    n - i,
                    a,
                    p(i, i),
                    1,
                    tauq[i - 1],
                    p(i, i + 1),
                    lda,
                );
                // G(i) annihilates A(i, i+2:n).
                let mut alpha = a[p(i, i + 1)];
                taup[i - 1] = dlarfg(n - i, &mut alpha, a, p(i, (i + 2).min(n)), lda);
                a[p(i, i + 1)] = alpha;
                e[i - 1] = alpha;
                // A(i+1:m, i+1:n) ← A(i+1:m, i+1:n)·G(i)
                apply_in_place(
                    release,
                    Side::Right,
                    m - i,
                    n - i,
                    a,
                    p(i, i + 1),
                    lda,
                    taup[i - 1],
                    p(i + 1, i + 1),
                    lda,
                );
            } else {
                taup[i - 1] = 0.0;
            }
        }
    } else {
        for i in 1..=m {
            // G(i) annihilates A(i, i+1:n).
            let mut alpha = a[p(i, i)];
            taup[i - 1] = dlarfg(n - i + 1, &mut alpha, a, p(i, (i + 1).min(n)), lda);
            a[p(i, i)] = alpha;
            d[i - 1] = alpha;
            if i < m {
                // A(i+1:m, i:n) ← A(i+1:m, i:n)·G(i)
                apply_in_place(
                    release,
                    Side::Right,
                    m - i,
                    n - i + 1,
                    a,
                    p(i, i),
                    lda,
                    taup[i - 1],
                    p(i + 1, i),
                    lda,
                );
                // H(i) annihilates A(i+2:m, i).
                let mut alpha = a[p(i + 1, i)];
                tauq[i - 1] = dlarfg(m - i, &mut alpha, a, p((i + 2).min(m), i), 1);
                a[p(i + 1, i)] = alpha;
                e[i - 1] = alpha;
                // A(i+1:m, i+1:n) ← H(i)·A(i+1:m, i+1:n)
                apply_in_place(
                    release,
                    Side::Left,
                    m - i,
                    n - i,
                    a,
                    p(i + 1, i),
                    1,
                    tauq[i - 1],
                    p(i + 1, i + 1),
                    lda,
                );
            } else {
                tauq[i - 1] = 0.0;
            }
        }
    }
    Ok(())
}

/// `DLABRD`'s three arrays (`A`, the workspaces `X` and `Y`), each with its offset and leading
/// dimension.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Brd {
    A,
    X,
    Y,
}

struct Labrd<'s> {
    a: &'s mut [f64],
    x: &'s mut [f64],
    y: &'s mut [f64],
    off: [usize; 3],
    ld: [usize; 3],
}

impl Labrd<'_> {
    /// The storage index of element `(i, j)` (1-based) of an array.
    fn at(&self, w: Brd, i: usize, j: usize) -> usize {
        ix(self.off[w as usize], self.ld[w as usize], i, j)
    }

    fn get(&self, w: Brd) -> &[f64] {
        match w {
            Brd::A => self.a,
            Brd::X => self.x,
            Brd::Y => self.y,
        }
    }

    fn get_mut(&mut self, w: Brd) -> &mut [f64] {
        match w {
            Brd::A => self.a,
            Brd::X => self.x,
            Brd::Y => self.y,
        }
    }

    /// `DGEMV(TRANS, M, N, ALPHA, mat(i, j), LD, vec(xi, xj), INCX, BETA, vec(yi, yj), INCY)`
    /// where the matrix and both vectors may lie in the same array (`DLABRD` passes a row or a
    /// column of `A`, `X` or `Y` next to a block of the same array). `x` and `y` go through
    /// buffers; `dgemv`'s arithmetic depends on the increments only as unit or not, which the
    /// buffers keep (a stride of 2 stands for any other), and on the matrix's leading dimension,
    /// which is the array's.
    fn gemv(
        &mut self,
        trans: Trans,
        m: usize,
        n: usize,
        alpha: f64,
        mat: (Brd, usize, usize),
        x: (Brd, usize, usize, usize),
        beta: f64,
        y: (Brd, usize, usize, usize),
    ) {
        if m == 0 || n == 0 {
            return;
        }
        let (lenx, leny) = if trans == Trans::N { (n, m) } else { (m, n) };
        let (xa, xi, xj, incx) = x;
        let (ya, yi, yj, incy) = y;
        let (sx, sy) = (if incx == 1 { 1 } else { 2 }, if incy == 1 { 1 } else { 2 });
        let (xo, yo) = (self.at(xa, xi, xj), self.at(ya, yi, yj));
        let mut xb = vec![0.0f64; (lenx - 1) * sx + 1];
        let src = self.get(xa);
        for t in 0..lenx {
            xb[t * sx] = src[xo + t * incx];
        }
        let mut yb = vec![0.0f64; (leny - 1) * sy + 1];
        let dst = self.get(ya);
        for t in 0..leny {
            yb[t * sy] = dst[yo + t * incy];
        }
        let ao = self.at(mat.0, mat.1, mat.2);
        let lda = self.ld[mat.0 as usize];
        dgemv(
            trans,
            m,
            n,
            alpha,
            self.get(mat.0),
            ao,
            lda,
            &xb,
            0,
            sx,
            beta,
            &mut yb,
            0,
            sy,
        );
        let dst = self.get_mut(ya);
        for t in 0..leny {
            dst[yo + t * incy] = yb[t * sy];
        }
    }

    /// `DLARFG(N, A(ai, aj), A(xi, xj), INC, TAU)`: returns `(beta, tau)`, `beta` stored.
    fn larfg(
        &mut self,
        n: usize,
        (ai, aj): (usize, usize),
        (xi, xj): (usize, usize),
        inc: usize,
    ) -> (f64, f64) {
        let pa = self.at(Brd::A, ai, aj);
        let px = self.at(Brd::A, xi, xj);
        let mut alpha = self.a[pa];
        let tau = dlarfg(n, &mut alpha, self.a, px, inc);
        self.a[pa] = alpha;
        (alpha, tau)
    }

    /// `DSCAL(N, ALPHA, w(i, j), 1)`.
    fn scal(&mut self, n: usize, alpha: f64, w: Brd, i: usize, j: usize) {
        let p = self.at(w, i, j);
        dscal(n, alpha, self.get_mut(w), p, 1);
    }
}

/// `DLABRD(M, N, NB, A, LDA, D, E, TAUQ, TAUP, X, LDX, Y, LDY)`: the first `nb` rows and columns
/// of `A` (`m × n`) reduced to bidiagonal form as [`dgebd2`] would (upper for `m >= n`, lower
/// otherwise), with `X` (`m × nb`) and `Y` (`n × nb`) such that the rest of `A` is updated by
/// `A - V·Yᵀ - X·Uᵀ` (by the caller); `A`'s diagonal and off-diagonal entries of the reduced
/// part are left as 1 where the reflectors' unit elements were (`DGEBRD` writes `d` and `e`
/// back). Every product is OpenBLAS's `dgemv`; the same in both releases.
///
/// # Panics
/// When an array is too small, `ldx < m` or `ldy < n`.
pub fn dlabrd(
    m: usize,
    n: usize,
    nb: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    d: &mut [f64],
    e: &mut [f64],
    tauq: &mut [f64],
    taup: &mut [f64],
    x: &mut [f64],
    xoff: usize,
    ldx: usize,
    y: &mut [f64],
    yoff: usize,
    ldy: usize,
) {
    use Brd::{A, X, Y};
    use Trans::{N as No, T as Tr};
    if m == 0 || n == 0 {
        return;
    }
    let mut s = Labrd {
        a,
        x,
        y,
        off: [aoff, xoff, yoff],
        ld: [lda, ldx, ldy],
    };
    if m >= n {
        for i in 1..=nb {
            // A(i:m, i) -= A(i:m, 1:i-1)·Y(i, 1:i-1)ᵀ + X(i:m, 1:i-1)·A(1:i-1, i)
            s.gemv(
                No,
                m - i + 1,
                i - 1,
                -1.0,
                (A, i, 1),
                (Y, i, 1, ldy),
                1.0,
                (A, i, i, 1),
            );
            s.gemv(
                No,
                m - i + 1,
                i - 1,
                -1.0,
                (X, i, 1),
                (A, 1, i, 1),
                1.0,
                (A, i, i, 1),
            );
            // H(i) annihilates A(i+1:m, i).
            let (beta, tq) = s.larfg(m - i + 1, (i, i), ((i + 1).min(m), i), 1);
            tauq[i - 1] = tq;
            d[i - 1] = beta;
            if i < n {
                let pii = s.at(A, i, i);
                s.a[pii] = 1.0;
                // Y(i+1:n, i)
                s.gemv(
                    Tr,
                    m - i + 1,
                    n - i,
                    1.0,
                    (A, i, i + 1),
                    (A, i, i, 1),
                    0.0,
                    (Y, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    m - i + 1,
                    i - 1,
                    1.0,
                    (A, i, 1),
                    (A, i, i, 1),
                    0.0,
                    (Y, 1, i, 1),
                );
                s.gemv(
                    No,
                    n - i,
                    i - 1,
                    -1.0,
                    (Y, i + 1, 1),
                    (Y, 1, i, 1),
                    1.0,
                    (Y, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    m - i + 1,
                    i - 1,
                    1.0,
                    (X, i, 1),
                    (A, i, i, 1),
                    0.0,
                    (Y, 1, i, 1),
                );
                s.gemv(
                    Tr,
                    i - 1,
                    n - i,
                    -1.0,
                    (A, 1, i + 1),
                    (Y, 1, i, 1),
                    1.0,
                    (Y, i + 1, i, 1),
                );
                s.scal(n - i, tq, Y, i + 1, i);
                // A(i, i+1:n) -= Y(i+1:n, 1:i)·A(i, 1:i)ᵀ + A(1:i-1, i+1:n)ᵀ·X(i, 1:i-1)ᵀ
                s.gemv(
                    No,
                    n - i,
                    i,
                    -1.0,
                    (Y, i + 1, 1),
                    (A, i, 1, lda),
                    1.0,
                    (A, i, i + 1, lda),
                );
                s.gemv(
                    Tr,
                    i - 1,
                    n - i,
                    -1.0,
                    (A, 1, i + 1),
                    (X, i, 1, ldx),
                    1.0,
                    (A, i, i + 1, lda),
                );
                // G(i) annihilates A(i, i+2:n).
                let (beta, tp) = s.larfg(n - i, (i, i + 1), (i, (i + 2).min(n)), lda);
                taup[i - 1] = tp;
                e[i - 1] = beta;
                let pij = s.at(A, i, i + 1);
                s.a[pij] = 1.0;
                // X(i+1:m, i)
                s.gemv(
                    No,
                    m - i,
                    n - i,
                    1.0,
                    (A, i + 1, i + 1),
                    (A, i, i + 1, lda),
                    0.0,
                    (X, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    n - i,
                    i,
                    1.0,
                    (Y, i + 1, 1),
                    (A, i, i + 1, lda),
                    0.0,
                    (X, 1, i, 1),
                );
                s.gemv(
                    No,
                    m - i,
                    i,
                    -1.0,
                    (A, i + 1, 1),
                    (X, 1, i, 1),
                    1.0,
                    (X, i + 1, i, 1),
                );
                s.gemv(
                    No,
                    i - 1,
                    n - i,
                    1.0,
                    (A, 1, i + 1),
                    (A, i, i + 1, lda),
                    0.0,
                    (X, 1, i, 1),
                );
                s.gemv(
                    No,
                    m - i,
                    i - 1,
                    -1.0,
                    (X, i + 1, 1),
                    (X, 1, i, 1),
                    1.0,
                    (X, i + 1, i, 1),
                );
                s.scal(m - i, tp, X, i + 1, i);
            }
        }
    } else {
        for i in 1..=nb {
            // A(i, i:n) -= Y(i:n, 1:i-1)·A(i, 1:i-1)ᵀ + A(1:i-1, i:n)ᵀ·X(i, 1:i-1)ᵀ
            s.gemv(
                No,
                n - i + 1,
                i - 1,
                -1.0,
                (Y, i, 1),
                (A, i, 1, lda),
                1.0,
                (A, i, i, lda),
            );
            s.gemv(
                Tr,
                i - 1,
                n - i + 1,
                -1.0,
                (A, 1, i),
                (X, i, 1, ldx),
                1.0,
                (A, i, i, lda),
            );
            // G(i) annihilates A(i, i+1:n).
            let (beta, tp) = s.larfg(n - i + 1, (i, i), (i, (i + 1).min(n)), lda);
            taup[i - 1] = tp;
            d[i - 1] = beta;
            if i < m {
                let pii = s.at(A, i, i);
                s.a[pii] = 1.0;
                // X(i+1:m, i)
                s.gemv(
                    No,
                    m - i,
                    n - i + 1,
                    1.0,
                    (A, i + 1, i),
                    (A, i, i, lda),
                    0.0,
                    (X, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    n - i + 1,
                    i - 1,
                    1.0,
                    (Y, i, 1),
                    (A, i, i, lda),
                    0.0,
                    (X, 1, i, 1),
                );
                s.gemv(
                    No,
                    m - i,
                    i - 1,
                    -1.0,
                    (A, i + 1, 1),
                    (X, 1, i, 1),
                    1.0,
                    (X, i + 1, i, 1),
                );
                s.gemv(
                    No,
                    i - 1,
                    n - i + 1,
                    1.0,
                    (A, 1, i),
                    (A, i, i, lda),
                    0.0,
                    (X, 1, i, 1),
                );
                s.gemv(
                    No,
                    m - i,
                    i - 1,
                    -1.0,
                    (X, i + 1, 1),
                    (X, 1, i, 1),
                    1.0,
                    (X, i + 1, i, 1),
                );
                s.scal(m - i, tp, X, i + 1, i);
                // A(i+1:m, i) -= A(i+1:m, 1:i-1)·Y(i, 1:i-1)ᵀ + X(i+1:m, 1:i)·A(1:i, i)
                s.gemv(
                    No,
                    m - i,
                    i - 1,
                    -1.0,
                    (A, i + 1, 1),
                    (Y, i, 1, ldy),
                    1.0,
                    (A, i + 1, i, 1),
                );
                s.gemv(
                    No,
                    m - i,
                    i,
                    -1.0,
                    (X, i + 1, 1),
                    (A, 1, i, 1),
                    1.0,
                    (A, i + 1, i, 1),
                );
                // H(i) annihilates A(i+2:m, i).
                let (beta, tq) = s.larfg(m - i, (i + 1, i), ((i + 2).min(m), i), 1);
                tauq[i - 1] = tq;
                e[i - 1] = beta;
                let pji = s.at(A, i + 1, i);
                s.a[pji] = 1.0;
                // Y(i+1:n, i)
                s.gemv(
                    Tr,
                    m - i,
                    n - i,
                    1.0,
                    (A, i + 1, i + 1),
                    (A, i + 1, i, 1),
                    0.0,
                    (Y, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    m - i,
                    i - 1,
                    1.0,
                    (A, i + 1, 1),
                    (A, i + 1, i, 1),
                    0.0,
                    (Y, 1, i, 1),
                );
                s.gemv(
                    No,
                    n - i,
                    i - 1,
                    -1.0,
                    (Y, i + 1, 1),
                    (Y, 1, i, 1),
                    1.0,
                    (Y, i + 1, i, 1),
                );
                s.gemv(
                    Tr,
                    m - i,
                    i,
                    1.0,
                    (X, i + 1, 1),
                    (A, i + 1, i, 1),
                    0.0,
                    (Y, 1, i, 1),
                );
                s.gemv(
                    Tr,
                    i,
                    n - i,
                    -1.0,
                    (A, 1, i + 1),
                    (Y, 1, i, 1),
                    1.0,
                    (Y, i + 1, i, 1),
                );
                s.scal(n - i, tq, Y, i + 1, i);
            }
        }
    }
}

/// `DGEBRD`'s optimal workspace (the `LWORK = -1` query): `(m + n)·NB`, 1 for an empty matrix.
pub fn gebrd_lwork_opt(m: usize, n: usize) -> usize {
    if m.min(n) == 0 { 1 } else { (m + n) * NB }
}

/// `DGEBRD(M, N, A, LDA, D, E, TAUQ, TAUP, WORK, LWORK, INFO)`: [`dgebd2`]'s reduction, blocked
/// by [`NB`] rows and columns ([`dlabrd`], then the rest of `A` updated by two `DGEMM` with
/// depth `NB`, the panel's `d` and `e` written back) while more than [`NX`] remain; a workspace
/// below the optimal ([`gebrd_lwork_opt`]; `lwork = None` gives it) lowers `NB` to `LWORK/(M +
/// N)`, or unblocks everything below `2·(M + N)`. Returns `WORK(1)`; the same in both releases
/// but for [`dgebd2`]'s reflectors.
///
/// # Errors
/// `LDA < max(1, M)` (4), `LWORK < max(1, M, N)` (10).
pub fn dgebrd(
    lp: Lapack,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    d: &mut [f64],
    e: &mut [f64],
    tauq: &mut [f64],
    taup: &mut [f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let minmn = m.min(n);
    let (lwkmin, lwkopt) = if minmn == 0 {
        (1, 1)
    } else {
        (m.max(n), (m + n) * NB)
    };
    let lwork = lwork.unwrap_or(lwkopt);
    let err = |param| {
        Err(IllegalValue {
            routine: "DGEBRD",
            param,
        })
    };
    if lda < m.max(1) {
        return err(4);
    }
    if lwork < lwkmin {
        return err(10);
    }
    if minmn == 0 {
        return Ok(1);
    }
    let mut ws = m.max(n);
    let (ldx, ldy) = (m, n);
    let mut nb = NB;
    let nx = if nb > 1 && nb < minmn {
        let mut nx = NB.max(NX);
        if nx < minmn {
            ws = lwkopt;
            if lwork < ws {
                if lwork >= (m + n) * NBMIN {
                    nb = lwork / (m + n);
                } else {
                    nb = 1;
                    nx = minmn;
                }
            }
        }
        nx
    } else {
        minmn
    };
    let mut i = 1;
    if minmn > nx {
        // X and Y as LAPACK keeps them in WORK (leading dimensions M and N).
        let mut x = vec![0.0f64; ldx * nb];
        let mut y = vec![0.0f64; ldy * nb];
        let mut panel = vec![0.0f64; nb * n];
        while i <= minmn - nx {
            dlabrd(
                m - i + 1,
                n - i + 1,
                nb,
                a,
                ix(aoff, lda, i, i),
                lda,
                &mut d[i - 1..],
                &mut e[i - 1..],
                &mut tauq[i - 1..],
                &mut taup[i - 1..],
                &mut x,
                0,
                ldx,
                &mut y,
                0,
                ldy,
            );
            let (mm, nn) = (m - i - nb + 1, n - i - nb + 1);
            {
                // A(i+nb:m, i+nb:n) -= A(i+nb:m, i:i+nb-1)·Y(nb+1:, 1:nb)ᵀ
                let (left, right) = a.split_at_mut(ix(aoff, lda, 1, i + nb));
                dgemm_with(
                    mm,
                    nn,
                    nb,
                    -1.0,
                    Operand {
                        data: left,
                        off: ix(aoff, lda, i + nb, i),
                        ld: lda,
                        trans: Trans::N,
                    },
                    Operand {
                        data: &y,
                        off: nb,
                        ld: ldy,
                        trans: Trans::T,
                    },
                    1.0,
                    right,
                    i + nb - 1,
                    lda,
                    lp.gemm_q,
                );
            }
            // A(i+nb:m, i+nb:n) -= X(nb+1:, 1:nb)·A(i:i+nb-1, i+nb:n); the rows above the
            // updated block, in the same columns, are copied out (leading dimension nb).
            for j in 0..nn {
                for r in 0..nb {
                    panel[r + j * nb] = a[ix(aoff, lda, i + r, i + nb + j)];
                }
            }
            dgemm_with(
                mm,
                nn,
                nb,
                -1.0,
                Operand {
                    data: &x,
                    off: nb,
                    ld: ldx,
                    trans: Trans::N,
                },
                Operand {
                    data: &panel,
                    off: 0,
                    ld: nb,
                    trans: Trans::N,
                },
                1.0,
                a,
                ix(aoff, lda, i + nb, i + nb),
                lda,
                lp.gemm_q,
            );
            // The bidiagonal entries back into A.
            for j in i..i + nb {
                a[ix(aoff, lda, j, j)] = d[j - 1];
                if m >= n {
                    a[ix(aoff, lda, j, j + 1)] = e[j - 1];
                } else {
                    a[ix(aoff, lda, j + 1, j)] = e[j - 1];
                }
            }
            i += nb;
        }
    }
    dgebd2(
        lp.release,
        m - i + 1,
        n - i + 1,
        a,
        ix(aoff, lda, i, i),
        lda,
        &mut d[i - 1..],
        &mut e[i - 1..],
        &mut tauq[i - 1..],
        &mut taup[i - 1..],
    )?;
    Ok(ws)
}

// ------------------------------------------------------------------------------------------
// The bidiagonal reduction's Q and P (DORMBR, DORGBR)

/// Which factor of [`dgebrd`]'s reduction `A = Q·B·Pᵀ` a routine applies or generates
/// (LAPACK's `VECT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vect {
    /// `'Q'`: the reflectors `H(i)` stored below the diagonal (with `tauq`).
    Q,
    /// `'P'`: the reflectors `G(i)` stored right of the diagonal (with `taup`).
    P,
}

/// `DORMBR`'s optimal workspace, the `LWORK = -1` query: `NW·NB` with `NW = max(1, N)` from the
/// left and `max(1, M)` from the right, `NB = ILAENV(1, 'DORMQR' | 'DORMLQ', ...) = 32` (both
/// releases; the workspace `DORMQR`/`DORMLQ` would ask for themselves is larger, so with this
/// one they take their short-workspace block size).
pub fn ormbr_lwork_opt(side: Side, m: usize, n: usize) -> usize {
    (if side == Side::Left { n } else { m }).max(1) * NB
}

/// `DORMBR(VECT, SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)`: `C` (`m × n` at
/// `coff`) overwritten by `Q·C`, `Qᵀ·C`, `C·Q`, `C·Qᵀ` (`vect = Q`) or the same with `P`
/// (`vect = P`), from [`dgebrd`]'s reflectors of an `nq × k` (`Q`) or `k × nq` (`P`) matrix (`nq =
/// m` from the left, `n` from the right): [`dormqr`] (or [`dormlq`] with the transpose flipped
/// for `P`) on all `k` reflectors when `nq >= k` (`Q`) or `nq > k` (`P`), else on the `nq - 1`
/// reflectors shifted by one row (`Q`) or column (`P`) with `C`'s first row (left) or column
/// (right) untouched. `lwork` (`None`: [`ormbr_lwork_opt`]) is handed to `DORMQR`/`DORMLQ`
/// as it is. Returns `WORK(1)`: [`ormbr_lwork_opt`], or 1 for an empty `C`.
///
/// # Errors
/// `LDA < max(1, nq)` (`Q`) or `< max(1, min(nq, k))` (`P`) (8), `LDC < max(1, M)` (11), `LWORK
/// < NW` (13).
pub fn dormbr(
    lp: Lapack,
    vect: Vect,
    side: Side,
    trans: Trans,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORMBR",
            param,
        })
    };
    let applyq = vect == Vect::Q;
    let left = side == Side::Left;
    let nq = if left { m } else { n };
    let nw = (if left { n } else { m }).max(1);
    let lwkopt = ormbr_lwork_opt(side, m, n);
    let lwork = lwork.unwrap_or(lwkopt);
    if (applyq && lda < nq.max(1)) || (!applyq && lda < nq.min(k).max(1)) {
        return err(8);
    }
    if ldc < m.max(1) {
        return err(11);
    }
    if lwork < nw {
        return err(13);
    }
    if m == 0 || n == 0 {
        return Ok(1);
    }
    let (mi, ni, i1, i2) = if left {
        (m.saturating_sub(1), n, 2, 1)
    } else {
        (m, n.saturating_sub(1), 1, 2)
    };
    let cshift = coff + (i1 - 1) + (i2 - 1) * ldc;
    if applyq {
        // Apply Q.
        if nq >= k {
            dormqr(
                lp,
                side,
                trans,
                m,
                n,
                k,
                a,
                aoff,
                lda,
                tau,
                c,
                coff,
                ldc,
                Some(lwork),
            )?;
        } else if nq > 1 {
            dormqr(
                lp,
                side,
                trans,
                mi,
                ni,
                nq - 1,
                a,
                aoff + 1,
                lda,
                tau,
                c,
                cshift,
                ldc,
                Some(lwork),
            )?;
        }
    } else {
        // Apply P.
        let transt = if trans == Trans::N {
            Trans::T
        } else {
            Trans::N
        };
        if nq > k {
            dormlq(
                lp,
                side,
                transt,
                m,
                n,
                k,
                a,
                aoff,
                lda,
                tau,
                c,
                coff,
                ldc,
                Some(lwork),
            )?;
        } else if nq > 1 {
            dormlq(
                lp,
                side,
                transt,
                mi,
                ni,
                nq - 1,
                a,
                aoff + lda,
                lda,
                tau,
                c,
                cshift,
                ldc,
                Some(lwork),
            )?;
        }
    }
    Ok(lwkopt)
}

/// `DORGBR`'s optimal workspace, the `LWORK = -1` query: `DORGQR`'s (`Q`: of `m × n × k` when
/// `m >= k`, else of the shifted `(m - 1)`-order one; 1 for `m = 1`) or `DORGLQ`'s (`P`: of `m ×
/// n × k` when `k < n`, else of the shifted `(n - 1)`-order one; 1 for `n = 1`), at least
/// `min(m, n)`.
pub fn orgbr_lwork_opt(vect: Vect, m: usize, n: usize, k: usize) -> usize {
    let inner = match vect {
        Vect::Q if m >= k => org_lwork_opt(m, n, false),
        Vect::Q if m > 1 => org_lwork_opt(m - 1, m - 1, false),
        Vect::P if k < n => org_lwork_opt(m, n, true),
        Vect::P if n > 1 => org_lwork_opt(n - 1, n - 1, true),
        _ => 1,
    };
    inner.max(m.min(n))
}

/// `DORGBR(VECT, M, N, K, A, LDA, TAU, WORK, LWORK, INFO)`: [`dgebrd`]'s `Q` (`m × n`, from the
/// reflectors of an `m × k` matrix: [`dorgqr`] when `m >= k`, else the reflectors shifted one
/// column right and `Q`'s first row and column those of the identity, then `DORGQR` of order
/// `m - 1`) or `Pᵀ` (`m × n`, from a `k × n` matrix: [`dorglq`] when `k < n`, else shifted one row
/// down, `DORGLQ` of order `n - 1`), overwriting `A`. `lwork` (`None`: [`orgbr_lwork_opt`]) is
/// handed on as it is. Returns `WORK(1)`: [`orgbr_lwork_opt`], or 1 for an empty `A`.
///
/// # Errors
/// `Q`: `N > M` or `N < min(M, K)`; `P`: `M > N` or `M < min(N, K)` (3); `LDA < max(1, M)` (6);
/// `LWORK < max(1, min(M, N))` (9).
pub fn dorgbr(
    lp: Lapack,
    vect: Vect,
    m: usize,
    n: usize,
    k: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
    tau: &[f64],
    lwork: Option<usize>,
) -> Result<usize, IllegalValue> {
    let err = |param| {
        Err(IllegalValue {
            routine: "DORGBR",
            param,
        })
    };
    let wantq = vect == Vect::Q;
    let mn = m.min(n);
    if (wantq && (n > m || n < m.min(k))) || (!wantq && (m > n || m < n.min(k))) {
        return err(3);
    }
    if lda < m.max(1) {
        return err(6);
    }
    let lwkopt = orgbr_lwork_opt(vect, m, n, k);
    let lwork = lwork.unwrap_or(lwkopt);
    if lwork < mn.max(1) {
        return err(9);
    }
    if m == 0 || n == 0 {
        return Ok(1);
    }
    let at = |i: usize, j: usize| ix(aoff, lda, i, j);
    if wantq {
        // Form Q, determined by a call to DGEBRD to reduce an m-by-k matrix.
        if m >= k {
            dorgqr(lp, m, n, k, a, aoff, lda, tau, Some(lwork))?;
        } else {
            // Shift the vectors which define the elementary reflectors one column to the
            // right, and set the first row and column of Q to those of the unit matrix.
            for j in (2..=m).rev() {
                a[at(1, j)] = 0.0;
                for i in j + 1..=m {
                    a[at(i, j)] = a[at(i, j - 1)];
                }
            }
            a[at(1, 1)] = 1.0;
            for i in 2..=m {
                a[at(i, 1)] = 0.0;
            }
            if m > 1 {
                dorgqr(lp, m - 1, m - 1, m - 1, a, at(2, 2), lda, tau, Some(lwork))?;
            }
        }
    } else {
        // Form P**T, determined by a call to DGEBRD to reduce a k-by-n matrix.
        if k < n {
            dorglq(lp, m, n, k, a, aoff, lda, tau, Some(lwork))?;
        } else {
            // Shift the vectors which define the elementary reflectors one row downward, and
            // set the first row and column of P**T to those of the unit matrix.
            a[at(1, 1)] = 1.0;
            for i in 2..=n {
                a[at(i, 1)] = 0.0;
            }
            for j in 2..=n {
                for i in (2..j).rev() {
                    a[at(i, j)] = a[at(i - 1, j)];
                }
                a[at(1, j)] = 0.0;
            }
            if n > 1 {
                dorglq(lp, n - 1, n - 1, n - 1, a, at(2, 2), lda, tau, Some(lwork))?;
            }
        }
    }
    Ok(lwkopt)
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

    /// `Q` from [`dorgqr`]/[`dorglq`] (blocked) against the unblocked [`dorg2r`]/[`dorgl2`]:
    /// orthonormal, equal up to rounding, rebuilding `A` with `R` (`L`), the same at 1 and 5
    /// threads.
    #[test]
    fn generated_q_is_orthonormal_and_rebuilds_the_matrix() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            for &(m, n) in &[(300, 170), (170, 300), (200, 161)] {
                for lq in [false, true] {
                    let (rows, cols) = if lq {
                        (m.min(n), n.max(m))
                    } else {
                        (m.max(n), m.min(n))
                    };
                    let a0 = matrix(rows, cols, (rows * 3 + cols) as u64);
                    let k = cols.min(rows);
                    let mut f = a0.clone();
                    let mut tau = vec![0.0f64; k];
                    if lq {
                        dgelqf(lp, rows, cols, &mut f, 0, rows, &mut tau, None).unwrap();
                    } else {
                        dgeqrf(lp, rows, cols, &mut f, 0, rows, &mut tau, None).unwrap();
                    }
                    let run = |threads: usize| {
                        let pool = rayon::ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .unwrap();
                        pool.install(|| {
                            let mut q = f.clone();
                            if lq {
                                dorglq(lp, rows, cols, k, &mut q, 0, rows, &tau, None).unwrap();
                            } else {
                                dorgqr(lp, rows, cols, k, &mut q, 0, rows, &tau, None).unwrap();
                            }
                            q
                        })
                    };
                    let q = run(1);
                    assert_eq!(q, run(5));
                    let mut q2 = f.clone();
                    if lq {
                        dorgl2(release, rows, cols, k, &mut q2, 0, rows, &tau).unwrap();
                    } else {
                        dorg2r(release, rows, cols, k, &mut q2, 0, rows, &tau).unwrap();
                    }
                    let d = q
                        .iter()
                        .zip(&q2)
                        .map(|(x, y)| (x - y).abs())
                        .fold(0.0, f64::max);
                    assert!(d < 1e-12, "{release:?} {rows}x{cols} lq={lq}: {d}");
                    // A = Q·R (or L·Q) with the triangle of the factorization.
                    let mut worst = 0.0f64;
                    for j in 0..cols {
                        for i in 0..rows {
                            let mut s = 0.0;
                            for p in 0..k {
                                s += if lq {
                                    if p <= i {
                                        f[i + p * rows] * q[p + j * rows]
                                    } else {
                                        0.0
                                    }
                                } else if p <= j {
                                    q[i + p * rows] * f[p + j * rows]
                                } else {
                                    0.0
                                };
                            }
                            worst = worst.max((s - a0[i + j * rows]).abs());
                        }
                    }
                    assert!(worst < 1e-12, "{release:?} {rows}x{cols} lq={lq}: {worst}");
                }
            }
        }
    }

    /// [`dormqr`]/[`dormlq`] (blocked, every side and transpose) against [`dorm2r`]/[`dorml2`],
    /// and `A` left as it was.
    #[test]
    fn applied_q_matches_the_unblocked_product() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            let (nq, k, other) = (190, 150, 37);
            for lq in [false, true] {
                let (rows, cols) = if lq { (k, nq) } else { (nq, k) };
                let mut f = matrix(rows, cols, 9);
                let mut tau = vec![0.0f64; k];
                if lq {
                    dgelqf(lp, rows, cols, &mut f, 0, rows, &mut tau, None).unwrap();
                } else {
                    dgeqrf(lp, rows, cols, &mut f, 0, rows, &mut tau, None).unwrap();
                }
                for side in [Side::Left, Side::Right] {
                    for trans in [Trans::N, Trans::T] {
                        let (m, n) = if side == Side::Left {
                            (nq, other)
                        } else {
                            (other, nq)
                        };
                        let c0 = matrix(m, n, 21);
                        let (mut a1, mut c1) = (f.clone(), c0.clone());
                        let (mut a2, mut c2) = (f.clone(), c0.clone());
                        if lq {
                            dormlq(
                                lp, side, trans, m, n, k, &mut a1, 0, rows, &tau, &mut c1, 0, m,
                                None,
                            )
                            .unwrap();
                            dorml2(
                                release, side, trans, m, n, k, &mut a2, 0, rows, &tau, &mut c2, 0,
                                m,
                            )
                            .unwrap();
                        } else {
                            dormqr(
                                lp, side, trans, m, n, k, &mut a1, 0, rows, &tau, &mut c1, 0, m,
                                None,
                            )
                            .unwrap();
                            dorm2r(
                                release, side, trans, m, n, k, &mut a2, 0, rows, &tau, &mut c2, 0,
                                m,
                            )
                            .unwrap();
                        }
                        assert_eq!(a1, f);
                        assert_eq!(a2, f);
                        let d = c1
                            .iter()
                            .zip(&c2)
                            .map(|(x, y)| (x - y).abs())
                            .fold(0.0, f64::max);
                        assert!(d < 1e-12, "{release:?} lq={lq} {side:?} {trans:?}: {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn orthogonal_checks_and_queries() {
        let lp = Lapack::default();
        let mut a = vec![0.0f64; 64];
        let mut c = vec![0.0f64; 64];
        let tau = [0.5f64; 8];
        let p = |e: IllegalValue| (e.routine, e.param);
        assert_eq!(
            p(dorgqr(lp, 3, 4, 2, &mut a, 0, 3, &tau, None).unwrap_err()),
            ("DORGQR", 2)
        );
        assert_eq!(
            p(dorgqr(lp, 4, 3, 4, &mut a, 0, 4, &tau, None).unwrap_err()),
            ("DORGQR", 3)
        );
        assert_eq!(
            p(dorgqr(lp, 4, 3, 2, &mut a, 0, 3, &tau, None).unwrap_err()),
            ("DORGQR", 5)
        );
        assert_eq!(
            p(dorgqr(lp, 4, 3, 2, &mut a, 0, 4, &tau, Some(2)).unwrap_err()),
            ("DORGQR", 8)
        );
        assert_eq!(
            p(dorglq(lp, 4, 3, 2, &mut a, 0, 4, &tau, None).unwrap_err()),
            ("DORGLQ", 2)
        );
        assert_eq!(
            p(dorgl2(LapackRelease::V3_12_0, 3, 4, 4, &mut a, 0, 3, &tau).unwrap_err()),
            ("DORGL2", 3)
        );
        let e = dormqr(
            lp,
            Side::Left,
            Trans::N,
            4,
            3,
            5,
            &mut a,
            0,
            4,
            &tau,
            &mut c,
            0,
            4,
            None,
        );
        assert_eq!(p(e.unwrap_err()), ("DORMQR", 5));
        let e = dormlq(
            lp,
            Side::Right,
            Trans::T,
            3,
            4,
            2,
            &mut a,
            0,
            1,
            &tau,
            &mut c,
            0,
            3,
            None,
        );
        assert_eq!(p(e.unwrap_err()), ("DORMLQ", 7));
        let e = dorm2r(
            LapackRelease::V3_12_1,
            Side::Left,
            Trans::T,
            4,
            3,
            2,
            &mut a,
            0,
            4,
            &tau,
            &mut c,
            0,
            3,
        );
        assert_eq!(p(e.unwrap_err()), ("DORM2R", 10));
        let e = dormqr(
            lp,
            Side::Right,
            Trans::N,
            4,
            3,
            2,
            &mut a,
            0,
            4,
            &tau,
            &mut c,
            0,
            4,
            Some(3),
        );
        assert_eq!(p(e.unwrap_err()), ("DORMQR", 12));
        assert_eq!(org_lwork_opt(600, 60_968, true), 600 * 32);
        assert_eq!(
            orm_lwork_opt(LapackRelease::V3_12_0, Side::Left, 600, 100),
            100 * 32 + 65 * 64
        );
        assert_eq!(
            orm_lwork_opt(LapackRelease::V3_12_1, Side::Right, 600, 100),
            600 * 32 + 65 * 32
        );
        let e = dormqr(
            lp,
            Side::Left,
            Trans::N,
            0,
            3,
            0,
            &mut a,
            0,
            1,
            &tau,
            &mut c,
            0,
            1,
            None,
        );
        assert_eq!(e, Ok(1));
        assert_eq!(dorgqr(lp, 3, 0, 0, &mut a, 0, 3, &tau, None), Ok(1));
    }

    /// [`dgebrd`] (blocked, both shapes) against [`dgebd2`]: equal up to rounding, `B` keeps
    /// `A`'s Frobenius norm, the same at 1 and 5 threads; checks and the query.
    #[test]
    fn bidiagonal_reduction() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            for &(m, n) in &[(300, 170), (170, 300), (200, 200)] {
                let a0 = matrix(m, n, (m + 5 * n) as u64);
                let k = m.min(n);
                let run = |threads: usize, blocked: bool| {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    pool.install(|| {
                        let mut a = a0.clone();
                        let (mut d, mut e) = (vec![0.0f64; k], vec![0.0f64; k - 1]);
                        let (mut tq, mut tp) = (vec![0.0f64; k], vec![0.0f64; k]);
                        if blocked {
                            dgebrd(
                                lp, m, n, &mut a, 0, m, &mut d, &mut e, &mut tq, &mut tp, None,
                            )
                            .unwrap();
                        } else {
                            dgebd2(
                                release, m, n, &mut a, 0, m, &mut d, &mut e, &mut tq, &mut tp,
                            )
                            .unwrap();
                        }
                        (a, d, e, tq, tp)
                    })
                };
                let one = run(1, true);
                assert_eq!(one, run(5, true));
                let two = run(1, false);
                let diff = one
                    .0
                    .iter()
                    .zip(&two.0)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0, f64::max);
                assert!(diff < 1e-10, "{release:?} {m}x{n}: {diff}");
                let fa: f64 = a0.iter().map(|v| v * v).sum();
                let fb: f64 = one.1.iter().chain(&one.2).map(|v| v * v).sum();
                assert!((fa - fb).abs() < 1e-9 * fa, "{fa} {fb}");
            }
        }
        let lp = Lapack::default();
        let (mut a, mut v) = (vec![0.0f64; 64], vec![0.0f64; 8]);
        let (mut d, mut e, mut t) = (v.clone(), v.clone(), v.clone());
        let r = dgebrd(lp, 4, 3, &mut a, 0, 3, &mut d, &mut e, &mut t, &mut v, None);
        assert_eq!(r.unwrap_err().param, 4);
        let r = dgebrd(
            lp,
            4,
            3,
            &mut a,
            0,
            4,
            &mut d,
            &mut e,
            &mut t,
            &mut v,
            Some(3),
        );
        assert_eq!(r.unwrap_err().param, 10);
        let r = dgebrd(lp, 0, 3, &mut a, 0, 1, &mut d, &mut e, &mut t, &mut v, None);
        assert_eq!(r, Ok(1));
        assert_eq!(gebrd_lwork_opt(600, 1_000), 1_600 * 32);
    }

    #[test]
    fn ormbr_and_orgbr_rebuild_the_reduction() {
        // A = Q·B·Pᵀ: Qᵀ·A·P (DORMBR from both sides) is the bidiagonal B; DORGBR's Q and Pᵀ
        // are orthonormal and give the same product.
        for &(m, n) in &[(7usize, 5usize), (5, 7), (40, 40), (150, 140), (140, 150)] {
            for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
                let lp = Lapack::new(release);
                let mut s = 0x9E37_79B9u64 ^ ((m * 1000 + n) as u64);
                let mut next = || {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
                };
                let a0: Vec<f64> = (0..m * n).map(|_| next()).collect();
                let mut a = a0.clone();
                let k = m.min(n);
                let (mut d, mut e) = (vec![0.0; k], vec![0.0; k]);
                let (mut tq, mut tp) = (vec![0.0; k], vec![0.0; k]);
                dgebrd(
                    lp, m, n, &mut a, 0, m, &mut d, &mut e, &mut tq, &mut tp, None,
                )
                .unwrap();
                // C = Qᵀ·A0·P.
                let mut c = a0.clone();
                dormbr(
                    lp,
                    Vect::Q,
                    Side::Left,
                    Trans::T,
                    m,
                    n,
                    n,
                    &mut a,
                    0,
                    m,
                    &tq,
                    &mut c,
                    0,
                    m,
                    None,
                )
                .unwrap();
                dormbr(
                    lp,
                    Vect::P,
                    Side::Right,
                    Trans::N,
                    m,
                    n,
                    m,
                    &mut a,
                    0,
                    m,
                    &tp,
                    &mut c,
                    0,
                    m,
                    None,
                )
                .unwrap();
                for j in 0..n {
                    for i in 0..m {
                        let want = if i == j {
                            d[i]
                        } else if m >= n && j == i + 1 {
                            e[i]
                        } else if m < n && i == j + 1 {
                            e[j]
                        } else {
                            0.0
                        };
                        assert!((c[i + j * m] - want).abs() < 1e-12, "{m}x{n} ({i},{j})");
                    }
                }
                // DORGBR: Q (m × k) and Pᵀ (k × n) rebuild A0 with B.
                let mut q = a.clone();
                q.truncate(m * n);
                let qcols = if m >= n { n } else { m };
                let mut qs = vec![0.0; m * qcols];
                qs.copy_from_slice(&q[..m * qcols]);
                dorgbr(lp, Vect::Q, m, qcols, n, &mut qs, 0, m, &tq, None).unwrap();
                let prow = if m >= n { n } else { m };
                let mut pt = vec![0.0; prow * n];
                for j in 0..n {
                    for i in 0..prow {
                        pt[i + j * prow] = a[i + j * m];
                    }
                }
                dorgbr(lp, Vect::P, prow, n, m, &mut pt, 0, prow, &tp, None).unwrap();
                for j in 0..n {
                    for i in 0..m {
                        let mut r = 0.0;
                        for t in 0..k {
                            let mut b_t = d[t] * pt[t + j * prow];
                            if m >= n && t + 1 < k {
                                b_t += e[t] * pt[t + 1 + j * prow];
                            }
                            if m < n && t >= 1 {
                                b_t += e[t - 1] * pt[t - 1 + j * prow];
                            }
                            r += qs[i + t * m] * b_t;
                        }
                        assert!(
                            (r - a0[i + j * m]).abs() < 1e-12,
                            "{m}x{n} rebuild ({i},{j})"
                        );
                    }
                }
            }
        }
    }
}

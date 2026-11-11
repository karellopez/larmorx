// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! OpenBLAS's level-3 BLAS as numpy's x86-64 wheels run it on the Haswell kernels
//! (`OPENBLAS_CORETYPE=Haswell`, one BLAS thread; OpenBLAS 0.3.34 for numpy 2.5.4, 0.3.30 for
//! numpy 2.3.5: the same files), on every platform: `dgemm` (here), the products LAPACK's
//! blocked routines make and numpy's `matmul` of two matrices ([`super::numpy_matmul`]).
//!
//! **What decides the bits** (`interface/gemm.c`, `driver/level3/level3.c`,
//! `kernel/x86_64/dgemm_kernel_4x8_haswell.S`, `dgemm_beta_skylakex.c`, `param.h`,
//! `kernel/setparam-ref.c`):
//!
//! - the interface returns at once for `m = 0` or `n = 0`; x86-64 has no small-matrix kernel
//!   for this core (the generic permit returns 0) and no forwarding of one-column products to
//!   `dgemv` (`GEMM_GEMV_FORWARD` is ARM's, RISC-V's and POWER's), so every call reaches the
//!   driver;
//! - the driver scales `C` by `beta` first when `beta ≠ 1` (`dgemm_beta`: `beta = 0` stores +0,
//!   whatever `C` held; otherwise `c·beta`), then returns for `k = 0` or `alpha = 0`;
//! - the depth is cut into blocks (`ls`, `min_l`): `GEMM_Q = 256` steps while at least
//!   `2·GEMM_Q` remain, then, if more than `GEMM_Q` remain, `⌈⌊rest/2⌋/4⌉·4`, then the rest
//!   ([`depth_blocks`]); each block's sum is added to `C` on its own: `c = alpha·acc + c`
//!   (`SAVE*`: `vmulpd` then `vaddpd`, two roundings);
//! - within a block the kernel takes `C`'s rows in groups of four, then two, then one, and its
//!   columns in groups of 24, 8, 4, then two, then one (the driver's cuts, `GEMM_P = 512` rows
//!   and the `3·GEMM_UNROLL_N` column chunks, are multiples of these, so each element's path
//!   depends only on its place among the last rows and columns of the whole product): a
//!   4-row group with 4 or more columns starts its accumulators with the first product when
//!   the block has 8 or more steps (`KERNEL4x*_I`, `vmulpd`), every other group from +0; every
//!   path then adds the products in order with fused multiply-adds, except four rows by the
//!   last single column (`KERNEL4x1`), which spreads the steps over four accumulators
//!   ([`larmorx_core::numpy::gemm`]).
//!
//! Each element of `C` follows its own fixed sequence of operations, so the product is split
//! into tiles computed in parallel with any number of threads, with the one-thread result of
//! OpenBLAS (several OpenBLAS threads cut the depth differently, `level3_thread.c`:
//! `(rest + 1)/2`, so numpy's own bits depend on its BLAS thread count; larmorx follows one
//! thread, the validation's `OPENBLAS_NUM_THREADS=1`).
//!
//! NaN results are NaN; their sign and payload follow the host's arithmetic, not promised to
//! be x86-64's (no NaN reaches these products on the ported paths: numpy 2.5.4's SVD and least
//! squares refuse non-finite input, nipype's CompCor zeroes non-finite rows first).
//!
//! Upstream files (BSD-3-Clause; the notice is in the parent module and NOTICE): OpenBLAS
//! 0.3.34 and 0.3.30 `interface/gemm.c`, `driver/level3/level3.c`, `level3_thread.c` (read for
//! the difference above), `kernel/x86_64/dgemm_kernel_4x8_haswell.S`, `dgemm_beta_skylakex.c`,
//! `KERNEL.HASWELL`, `param.h` (`HASWELL`), `kernel/setparam-ref.c`, `common_param.h`. The logic
//! is re-expressed in Rust; no code was copied.

use larmorx_core::numpy::gemm::{MR, NR, tile_4x1_split, tile_4x8};
use rayon::prelude::*;

/// `GEMM_Q` of the Haswell core on Linux and macOS (`DGEMM_DEFAULT_Q`; Windows' builds use 128):
/// the depth block of every CPU that runs these kernels natively (Intel's AVX2 CPUs, AMD Zen 1
/// to 3), of OpenBLAS 0.3.30 everywhere, and so of every oracle (fMRIPrep's image, the shared
/// environment's numpy 2.3.5). The default.
pub const GEMM_Q: usize = 256;
/// `GEMM_Q` that OpenBLAS 0.3.31 and later set for *every* core's table on AMD CPUs with AVX-512
/// (Zen 4 and 5: `kernel/setparam-ref.c`, the block after the per-core parameters, keyed on
/// the vendor, a 1 MiB L2 and the L3's size), so also for the Haswell kernels forced by
/// `OPENBLAS_CORETYPE=Haswell` there: numpy 2.5.4's library (OpenBLAS 0.3.34 + 106 commits) on
/// the main machine. With it the Haswell kernel's 12-column paths overflow their stack buffer
/// (`L_BUFFER_SIZE`, room for 298 steps) on depth blocks above 298 steps, so such products
/// crash or are undefined there (`docs/findings/confounds-blas.md`). For validation only.
pub const GEMM_Q_ZEN4: usize = 512;
/// `DGEMM_DEFAULT_UNROLL_M` of the Haswell core.
const UNROLL_M: usize = 4;

/// Whether an operand is used as stored or transposed (BLAS's `TRANSA`, `TRANSB`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trans {
    /// `'N'`: as stored.
    N,
    /// `'T'`: transposed.
    T,
}

/// The depth blocks `(ls, min_l)` of the one-thread driver (`level3.c`) for a depth `k` and a
/// `GEMM_Q` of `q` ([`GEMM_Q`] or [`GEMM_Q_ZEN4`]).
pub fn depth_blocks(k: usize, q: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut ls = 0;
    while ls < k {
        let mut min_l = k - ls;
        if min_l >= q * 2 {
            min_l = q;
        } else if min_l > q {
            min_l = (min_l / 2).div_ceil(UNROLL_M) * UNROLL_M;
        }
        out.push((ls, min_l));
        ls += min_l;
    }
    out
}

/// A column-major operand: `data[off + i + j·ld]` is element `(i, j)` of the stored matrix,
/// used as stored or transposed.
#[derive(Clone, Copy, Debug)]
pub struct Operand<'a> {
    /// The storage.
    pub data: &'a [f64],
    /// The offset of element `(0, 0)`.
    pub off: usize,
    /// The leading dimension.
    pub ld: usize,
    /// As stored or transposed.
    pub trans: Trans,
}

impl Operand<'_> {
    /// Element `(i, j)` of `op(X)`.
    #[inline(always)]
    fn at(&self, i: usize, j: usize) -> f64 {
        match self.trans {
            Trans::N => self.data[self.off + i + j * self.ld],
            Trans::T => self.data[self.off + j + i * self.ld],
        }
    }
}

/// The path of the kernel an element of a call of `mc × nc` takes (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Path {
    /// A 4-row group with four or more columns: the first product starts the chain when the
    /// block has 8 or more steps.
    Big,
    /// Any other group of rows and columns but the one below: a chain from +0.
    Zero,
    /// Four rows by the last single column: four accumulators.
    Split,
}

#[inline(always)]
fn path(i: usize, j: usize, mc: usize, nc: usize) -> Path {
    let row4 = i < mc - (mc & 3);
    if !row4 {
        return Path::Zero;
    }
    if j >= nc - (nc & 3) {
        if nc & 1 == 1 && j == nc - 1 {
            Path::Split
        } else {
            Path::Zero
        }
    } else {
        Path::Big
    }
}

/// One block of depth `l` of one kernel call of `mc × nc` (`GEMM_KERNEL_N`), on the part of
/// the call in rows `r0..r1` and columns `c0..c1` (`r0` a multiple of 4, `c0` of 8): `emit(i,
/// j, acc)` for each element, where `acc` sums `a(i, kk)·b(kk, j)` for `kk < l` along the
/// element's path (before `alpha` and `C`). `ap` and `bp` are scratch.
#[allow(clippy::too_many_arguments)]
fn kernel_block(
    mc: usize,
    nc: usize,
    l: usize,
    a: &impl Fn(usize, usize) -> f64,
    b: &impl Fn(usize, usize) -> f64,
    (r0, r1): (usize, usize),
    (c0, c1): (usize, usize),
    emit: &mut impl FnMut(usize, usize, f64),
    ap: &mut Vec<f64>,
    bp: &mut Vec<f64>,
) {
    let groups = (r1 - r0).div_ceil(MR);
    ap.clear();
    ap.resize(groups * l * MR, 0.0);
    for g in 0..groups {
        let panel = &mut ap[g * l * MR..(g + 1) * l * MR];
        for rr in 0..MR {
            let i = r0 + g * MR + rr;
            if i < r1 {
                for kk in 0..l {
                    panel[kk * MR + rr] = a(i, kk);
                }
            }
        }
    }
    bp.resize(l * NR, 0.0);
    let mut col = vec![0.0f64; l];
    for cg in (c0..c1).step_by(NR) {
        let width = (c1 - cg).min(NR);
        bp.iter_mut().for_each(|v| *v = 0.0);
        for cc in 0..width {
            for kk in 0..l {
                bp[kk * NR + cc] = b(kk, cg + cc);
            }
        }
        let split_col = (nc & 1 == 1 && cg <= nc - 1 && nc - 1 < cg + width).then_some(nc - 1);
        if let Some(j) = split_col {
            for (kk, v) in col.iter_mut().enumerate() {
                *v = bp[kk * NR + (j - cg)];
            }
        }
        for g in 0..groups {
            let panel = &ap[g * l * MR..(g + 1) * l * MR];
            let t = tile_4x8(panel, bp, l);
            let split = split_col.map(|_| tile_4x1_split(panel, &col, l));
            for (cc, tcol) in t.iter().enumerate().take(width) {
                let j = cg + cc;
                for (rr, &tv) in tcol.iter().enumerate() {
                    let i = r0 + g * MR + rr;
                    if i >= r1 {
                        break;
                    }
                    let acc = match path(i, j, mc, nc) {
                        Path::Zero => tv,
                        Path::Split => split.expect("split column")[rr],
                        Path::Big => {
                            if l >= 8
                                && tv.to_bits() == 0
                                && all_products_negative_zero(panel, bp, l, rr, cc)
                            {
                                -0.0
                            } else {
                                tv
                            }
                        }
                    };
                    emit(i, j, acc);
                }
            }
        }
    }
}

/// Whether every product of a chain is exactly -0 (then a chain started by its first product,
/// `vmulpd`, ends at -0 where one started from +0 ends at +0).
#[cold]
fn all_products_negative_zero(ap: &[f64], bp: &[f64], l: usize, rr: usize, cc: usize) -> bool {
    (0..l).all(|kk| (ap[kk * MR + rr] * bp[kk * NR + cc]).to_bits() == (-0.0f64).to_bits())
}

/// The largest tile one task computes (rows, columns): about 1 MB of `C` and panels of a few
/// hundred kilobytes, so the packing costs well under 1 % of the products.
const TILE_ROWS: usize = 256;
const TILE_COLS: usize = 256;

/// `C ← alpha·op(A)·op(B) + beta·C` for `C` of `m × n` (column-major, `c[coff + i + j·ldc]`),
/// `op(A)` of `m × k`, `op(B)` of `k × n`, as OpenBLAS's `dgemm` computes it on the Haswell
/// kernels at one thread (module documentation). Parallel over tiles of `C` in the current
/// rayon pool; the bits do not depend on the number of threads.
///
/// # Panics
/// When an operand is too small for its dimensions (BLAS's parameter errors).
#[allow(clippy::too_many_arguments)]
pub fn dgemm(
    m: usize,
    n: usize,
    k: usize,
    alpha: f64,
    a: Operand<'_>,
    b: Operand<'_>,
    beta: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    dgemm_with(m, n, k, alpha, a, b, beta, c, coff, ldc, GEMM_Q);
}

/// [`dgemm`] with the depth blocked by `gemm_q` ([`GEMM_Q`], or [`GEMM_Q_ZEN4`] to follow numpy
/// 2.5.4's library forced onto the Haswell kernels on an AMD Zen 4 or 5).
#[allow(clippy::too_many_arguments)]
pub fn dgemm_with(
    m: usize,
    n: usize,
    k: usize,
    alpha: f64,
    a: Operand<'_>,
    b: Operand<'_>,
    beta: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    gemm_q: usize,
) {
    if m == 0 || n == 0 {
        return;
    }
    let (nra, nca) = if a.trans == Trans::N { (m, k) } else { (k, m) };
    let (nrb, ncb) = if b.trans == Trans::N { (k, n) } else { (n, k) };
    assert!(ldc >= m, "DGEMM: LDC ({ldc}) < M ({m})");
    // OpenBLAS checks `lda < nrowa` (no `max(1, ·)`, unlike the reference BLAS).
    assert!(a.ld >= nra, "DGEMM: LDA ({}) < {nra}", a.ld);
    assert!(b.ld >= nrb, "DGEMM: LDB ({}) < {nrb}", b.ld);
    assert!(c.len() >= coff + (n - 1) * ldc + m, "DGEMM: C is too small");
    if nca > 0 {
        assert!(
            a.data.len() >= a.off + (nca - 1) * a.ld + nra,
            "DGEMM: A is too small"
        );
    }
    if ncb > 0 {
        assert!(
            b.data.len() >= b.off + (ncb - 1) * b.ld + nrb,
            "DGEMM: B is too small"
        );
    }
    if beta != 1.0 {
        for j in 0..n {
            let col = &mut c[coff + j * ldc..coff + j * ldc + m];
            if beta == 0.0 {
                col.iter_mut().for_each(|v| *v = 0.0);
            } else {
                col.iter_mut().for_each(|v| *v *= beta);
            }
        }
    }
    if k == 0 || alpha == 0.0 {
        return;
    }
    let blocks = depth_blocks(k, gemm_q);
    let tiles: Vec<(usize, usize)> = (0..m)
        .step_by(TILE_ROWS)
        .flat_map(|r0| (0..n).step_by(TILE_COLS).map(move |c0| (r0, c0)))
        .collect();
    let compute = |&(r0, c0): &(usize, usize), cin: Vec<f64>| -> Vec<f64> {
        let (r1, c1) = ((r0 + TILE_ROWS).min(m), (c0 + TILE_COLS).min(n));
        let ldt = r1 - r0;
        let mut ct = cin;
        let (mut ap, mut bp) = (Vec::new(), Vec::new());
        for &(ls, l) in &blocks {
            let fa = |i: usize, kk: usize| a.at(i, ls + kk);
            let fb = |kk: usize, j: usize| b.at(ls + kk, j);
            let mut emit = |i: usize, j: usize, acc: f64| {
                let slot = &mut ct[(i - r0) + (j - c0) * ldt];
                let scaled = alpha * acc;
                *slot = scaled + *slot;
            };
            kernel_block(
                m,
                n,
                l,
                &fa,
                &fb,
                (r0, r1),
                (c0, c1),
                &mut emit,
                &mut ap,
                &mut bp,
            );
        }
        ct
    };
    // Waves of tiles: each tile's part of C is copied out, updated through every depth block
    // and copied back, so the extra memory stays a few tiles per thread.
    let wave = rayon::current_num_threads().max(1) * 2;
    for chunk in tiles.chunks(wave) {
        let inputs: Vec<Vec<f64>> = chunk
            .iter()
            .map(|&(r0, c0)| {
                let (r1, c1) = ((r0 + TILE_ROWS).min(m), (c0 + TILE_COLS).min(n));
                let mut t = Vec::with_capacity((r1 - r0) * (c1 - c0));
                for j in c0..c1 {
                    t.extend_from_slice(&c[coff + r0 + j * ldc..coff + r1 + j * ldc]);
                }
                t
            })
            .collect();
        let outputs: Vec<Vec<f64>> = if chunk.len() == 1 {
            vec![compute(
                &chunk[0],
                inputs.into_iter().next().expect("one tile"),
            )]
        } else {
            chunk
                .par_iter()
                .zip(inputs.into_par_iter())
                .map(|(t, cin)| compute(t, cin))
                .collect()
        };
        for (&(r0, c0), t) in chunk.iter().zip(outputs) {
            let r1 = (r0 + TILE_ROWS).min(m);
            let ldt = r1 - r0;
            for (jj, col) in t.chunks_exact(ldt).enumerate() {
                let j = c0 + jj;
                c[coff + r0 + j * ldc..coff + r1 + j * ldc].copy_from_slice(col);
            }
        }
    }
}

/// `GEMM_UNROLL_MN` of the Haswell core (`MAX(GEMM_UNROLL_M, GEMM_UNROLL_N)`).
const UNROLL_MN: usize = 8;
/// `GEMM_P` of the Haswell core.
const GEMM_P: usize = 512;
/// The largest order [`dsyrk_lower`] takes: `dgemm_r` with the smallest buffer the wheels may
/// be built with (`BUFFERSIZE = 20`; 64,992 with the default 128 MiB); one column block of
/// the driver below it.
pub const SYRK_MAX_N: usize = 15_856;

/// The depth blocks of the one-thread `dsyrk` driver (`level3_syrk.c`): `GEMM_Q` while at least
/// `2·GEMM_Q` remain, then `⌈rest/2⌉` if more than `GEMM_Q` remain, then the rest.
pub fn syrk_depth_blocks(k: usize, q: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut ls = 0;
    while ls < k {
        let mut min_l = k - ls;
        if min_l >= q * 2 {
            min_l = q;
        } else if min_l > q {
            min_l = min_l.div_ceil(2);
        }
        out.push((ls, min_l));
        ls += min_l;
    }
    out
}

/// One `GEMM_KERNEL` call the `dsyrk` driver and its kernel make for one depth block: the part
/// of `C` (rows `r0..r0 + mc`, columns `c0..c0 + nc`) and whether it goes through the kernel's
/// diagonal buffer (`subbuffer`, cleared to +0, then its lower triangle added to `C`).
#[derive(Clone, Copy, Debug)]
struct SyrkCall {
    r0: usize,
    c0: usize,
    mc: usize,
    nc: usize,
    diagonal: bool,
}

/// The `GEMM_KERNEL` calls of `dsyrk`'s lower case for an order `n` (one column block), in
/// the driver's order (`level3_syrk.c`, `LOWER`; `syrk_kernel.c`): row blocks of `GEMM_P`
/// (halved on the `GEMM_UNROLL_MN` grid between `GEMM_P` and `2·GEMM_P`); in each, the part
/// left of the block's diagonal square in one call, then the square in tiles of
/// `GEMM_UNROLL_MN`: each diagonal tile through the buffer, the strip below it in one call.
fn syrk_lower_calls(n: usize) -> Vec<SyrkCall> {
    let mut calls = Vec::new();
    let rule = |rest: usize| {
        if rest >= GEMM_P * 2 {
            GEMM_P
        } else if rest > GEMM_P {
            (rest / 2).div_ceil(UNROLL_MN) * UNROLL_MN
        } else {
            rest
        }
    };
    let mut is = 0;
    while is < n {
        let mi = rule(n - is);
        if is > 0 {
            calls.push(SyrkCall {
                r0: is,
                c0: 0,
                mc: mi,
                nc: is,
                diagonal: false,
            });
        }
        let mut lp = 0;
        while lp < mi {
            let nn = UNROLL_MN.min(mi - lp);
            calls.push(SyrkCall {
                r0: is + lp,
                c0: is + lp,
                mc: nn,
                nc: nn,
                diagonal: true,
            });
            let below = mi - lp - nn;
            if below > 0 {
                calls.push(SyrkCall {
                    r0: is + lp + nn,
                    c0: is + lp,
                    mc: below,
                    nc: nn,
                    diagonal: false,
                });
            }
            lp += UNROLL_MN;
        }
        is += mi;
    }
    calls
}

/// `DSYRK('L', TRANS, N, K, ALPHA, A, LDA, BETA, C, LDC)`: the lower triangle of
/// `C ← alpha·op(A)·op(A)ᵀ + beta·C` with `op(A) = A` (`n × k`, `Trans::N`) or `Aᵀ` (`A` of
/// `k × n`, `Trans::T`), as OpenBLAS computes it at one thread: `beta` on the lower triangle
/// only (`syrk_beta`), the depth in [`syrk_depth_blocks`], each block through the kernel calls
/// of `syrk_lower_calls` (each element's path within its own call); the diagonal tiles'
/// sums go through a cleared buffer, `s = alpha·acc + 0`, then `c = c + s`. The upper triangle
/// is not touched. numpy's `x @ x.T` and `x.T @ x` are the two `TRANS` (CBLAS turns the
/// row-major upper call into this one). Parallel over the calls of each depth block.
///
/// # Panics
/// When `n > SYRK_MAX_N` or an operand is too small.
#[allow(clippy::too_many_arguments)]
pub fn dsyrk_lower(
    trans: Trans,
    n: usize,
    k: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    beta: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    dsyrk_lower_with(trans, n, k, alpha, a, aoff, lda, beta, c, coff, ldc, GEMM_Q);
}

/// [`dsyrk_lower`] with the depth blocked by `gemm_q` (as [`dgemm_with`]).
#[allow(clippy::too_many_arguments)]
pub fn dsyrk_lower_with(
    trans: Trans,
    n: usize,
    k: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    beta: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
    gemm_q: usize,
) {
    if n == 0 {
        return;
    }
    assert!(
        n <= SYRK_MAX_N,
        "DSYRK: order {n} above {SYRK_MAX_N} is not ported"
    );
    assert!(ldc >= n, "DSYRK: LDC ({ldc}) < N ({n})");
    let nra = if trans == Trans::N { n } else { k };
    assert!(lda >= nra, "DSYRK: LDA ({lda}) < {nra}");
    if beta != 1.0 {
        for j in 0..n {
            for v in &mut c[coff + j + j * ldc..coff + n + j * ldc] {
                *v = if beta == 0.0 { 0.0 } else { *v * beta };
            }
        }
    }
    if k == 0 || alpha == 0.0 {
        return;
    }
    let op = |i: usize, kk: usize| match trans {
        Trans::N => a[aoff + i + kk * lda],
        Trans::T => a[aoff + kk + i * lda],
    };
    let calls = syrk_lower_calls(n);
    for (ls, l) in syrk_depth_blocks(k, gemm_q) {
        let accs: Vec<Vec<f64>> = calls
            .par_iter()
            .map(|call| {
                let mut out = vec![0.0f64; call.mc * call.nc];
                let fa = |i: usize, kk: usize| op(call.r0 + i, ls + kk);
                let fb = |kk: usize, j: usize| op(call.c0 + j, ls + kk);
                let mut emit = |i: usize, j: usize, acc: f64| out[i + j * call.mc] = acc;
                let (mut ap, mut bp) = (Vec::new(), Vec::new());
                kernel_block(
                    call.mc,
                    call.nc,
                    l,
                    &fa,
                    &fb,
                    (0, call.mc),
                    (0, call.nc),
                    &mut emit,
                    &mut ap,
                    &mut bp,
                );
                out
            })
            .collect();
        for (call, acc) in calls.iter().zip(accs) {
            for j in 0..call.nc {
                let i0 = if call.diagonal { j } else { 0 };
                for i in i0..call.mc {
                    let slot = &mut c[coff + call.r0 + i + (call.c0 + j) * ldc];
                    let scaled = alpha * acc[i + j * call.mc];
                    if call.diagonal {
                        let s = scaled + 0.0;
                        *slot += s;
                    } else {
                        *slot = scaled + *slot;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syrk_calls_cover_the_lower_triangle_once() {
        for n in [1usize, 5, 8, 9, 17, 513, 1100, 2049] {
            let mut seen = vec![0u8; n * n];
            for c in syrk_lower_calls(n) {
                for j in 0..c.nc {
                    let i0 = if c.diagonal { j } else { 0 };
                    for i in i0..c.mc {
                        seen[(c.r0 + i) + (c.c0 + j) * n] += 1;
                    }
                }
            }
            for j in 0..n {
                for i in 0..n {
                    assert_eq!(seen[i + j * n], u8::from(i >= j), "n = {n} at ({i}, {j})");
                }
            }
        }
    }

    fn values(n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64) - 0.5
            })
            .collect()
    }

    #[test]
    fn depth_blocks_follow_level3() {
        let cut = |k| {
            depth_blocks(k, GEMM_Q)
                .iter()
                .map(|b| b.1)
                .collect::<Vec<_>>()
        };
        assert_eq!(cut(0), Vec::<usize>::new());
        assert_eq!(cut(256), vec![256]);
        assert_eq!(cut(257), vec![128, 129]);
        assert_eq!(cut(300), vec![152, 148]);
        assert_eq!(cut(511), vec![256, 255]);
        assert_eq!(cut(512), vec![256, 256]);
        assert_eq!(cut(700), vec![256, 224, 220]);
    }

    /// A direct model of the paths, element by element, against the tiled driver.
    #[test]
    fn tiles_match_the_element_model() {
        for &(m, n, k) in &[
            (1, 1, 1),
            (5, 7, 9),
            (13, 11, 300),
            (260, 9, 17),
            (6, 263, 8),
        ] {
            let av = values(m * k, 1);
            let bv = values(k * n, 2);
            let cv = values(m * n, 3);
            let a = Operand {
                data: &av,
                off: 0,
                ld: m,
                trans: Trans::N,
            };
            let b = Operand {
                data: &bv,
                off: 0,
                ld: k,
                trans: Trans::N,
            };
            let mut got = cv.clone();
            dgemm(m, n, k, 0.75, a, b, 1.5, &mut got, 0, m);
            for j in 0..n {
                for i in 0..m {
                    let mut c = cv[i + j * m] * 1.5;
                    for (ls, l) in depth_blocks(k, GEMM_Q) {
                        let p = |kk: usize| (av[i + (ls + kk) * m], bv[(ls + kk) + j * k]);
                        let acc = match path(i, j, m, n) {
                            Path::Big | Path::Zero => {
                                let mut s = 0.0f64;
                                for kk in 0..l {
                                    let (x, y) = p(kk);
                                    s = x.mul_add(y, s);
                                }
                                s
                            }
                            Path::Split => {
                                let mut lanes = [0.0f64; 4];
                                for kk in 0..l {
                                    let lane = if kk < (l & !7) { kk % 4 } else { 0 };
                                    let (x, y) = p(kk);
                                    lanes[lane] = x.mul_add(y, lanes[lane]);
                                }
                                (lanes[0] + lanes[1]) + (lanes[2] + lanes[3])
                            }
                        };
                        c += 0.75 * acc;
                    }
                    assert_eq!(
                        got[i + j * m].to_bits(),
                        c.to_bits(),
                        "({m},{n},{k}) at ({i},{j})"
                    );
                }
            }
        }
    }

    #[test]
    fn beta_zero_clears_and_signs_of_zero_follow_the_first_product() {
        // Every product -0 in a 4-row group, depth 8: vmulpd keeps -0, and beta = 1 with
        // C = -0 keeps it; a chain from +0 would give +0.
        let (m, n, k) = (4usize, 4usize, 8usize);
        let av = vec![-0.0f64; m * k];
        let bv = vec![1.0f64; k * n];
        let mut c = vec![-0.0f64; m * n];
        let a = Operand {
            data: &av,
            off: 0,
            ld: m,
            trans: Trans::N,
        };
        let b = Operand {
            data: &bv,
            off: 0,
            ld: k,
            trans: Trans::N,
        };
        dgemm(m, n, k, 1.0, a, b, 1.0, &mut c, 0, m);
        assert!(c.iter().all(|v| v.to_bits() == (-0.0f64).to_bits()));
        // Depth 7: the chain starts from +0.
        let mut c = vec![-0.0f64; m * n];
        dgemm(m, n, 7, 1.0, a, b, 1.0, &mut c, 0, m);
        assert!(c.iter().all(|v| v.to_bits() == 0));
        // beta = 0 stores +0 over a NaN.
        let mut c = vec![f64::NAN; m * n];
        dgemm(m, n, 0, 1.0, a, b, 0.0, &mut c, 0, m);
        assert!(c.iter().all(|v| v.to_bits() == 0));
    }
}

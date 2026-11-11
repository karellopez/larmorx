// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! OpenBLAS's `dtrmm` (`B ← alpha·op(A)·B` or `B ← alpha·B·op(A)`, `A` triangular) as numpy's
//! x86-64 wheels run it on the Haswell kernels at one BLAS thread (OpenBLAS 0.3.30 for numpy
//! 2.3.5, v0.3.34-106-g446c436e for numpy 2.5.4: the same files but for `volatile` in three
//! copy routines and the interface's other paths), every side, triangle, transpose and
//! diagonal, on every platform: the products LAPACK's blocked `DLARFB` and recursive `DLARFT`
//! make.
//!
//! **What decides the bits** (`interface/trsm.c` compiled as `TRMM`, `driver/level3/trmm_L.c`,
//! `trmm_R.c`, `kernel/x86_64/dtrmm_kernel_4x8_haswell.c`, the generic copies
//! `kernel/generic/trmm_{u,l}{n,t}copy_{4,8}.c`, `dgemm_kernel_4x8_haswell.S`):
//!
//! - the interface returns for `m = 0` or `n = 0` (after its argument checks); the driver
//!   scales `B` by `alpha` first when `alpha ≠ 1` (`dgemm_beta`: `alpha = 0` stores +0 and
//!   returns), then works with `alpha = 1`;
//! - the triangle's dimension is cut into blocks of `GEMM_Q` (256; 512 under OpenBLAS ≥ 0.3.31
//!   on an AMD Zen 4 or 5, [`super::level3::GEMM_Q_ZEN4`]); each element of `B` gets the product
//!   with its own diagonal block from the TRMM kernel, which **overwrites** it, and then the
//!   products with the other blocks from `GEMM_KERNEL` calls, each **added** (`c = 1·acc + c`),
//!   in the driver's order of blocks (forwards for an upper `op(A)` on the left and a lower one
//!   on the right, backwards otherwise); every product uses `B`'s values before the call (the
//!   driver packs each block before it overwrites it);
//! - the TRMM kernel takes its call's rows in groups of 4, 2, 1 and its columns in groups of 8,
//!   4, 2, 1; a 4 × 8 group is the assembly path (fused multiply-add chains from +0 in step
//!   order), every other group the C paths (`res += a·b` from 0, each product rounded: the
//!   Haswell build has no fused multiply-adds there); each group's steps run over the part of
//!   the block the triangle can reach for the group as a whole (`offset`, `kk`), so a row of a
//!   group past the diagonal also multiplies the copy's stored zeros (and its stored 1 for a
//!   unit diagonal): an infinity or NaN in `B` there gives NaN, as in OpenBLAS;
//! - the `GEMM_KERNEL` calls have the shapes the driver gives them (`GEMM_P = 512` rows on the
//!   left, rounded down to the 4 grid; the right side's row blocks of 512), with the kernel's
//!   paths and classes of [`super::level3`].
//!
//! Each row of `B` (right side) or column (left side) goes through its own sequence of
//! operations, so blocks of rows or columns are computed in parallel with the one-thread
//! result of OpenBLAS (the threaded interface splits the other dimension: the bits are the
//! same, but larmorx follows one thread).
//!
//! Upstream files (BSD-3-Clause; the notice is in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `interface/trsm.c`, `driver/level3/trmm_L.c`, `trmm_R.c`,
//! `kernel/x86_64/dtrmm_kernel_4x8_haswell.c`, `kernel/generic/trmm_uncopy_4.c`,
//! `trmm_lncopy_4.c`, `trmm_utcopy_4.c`, `trmm_ltcopy_4.c`, `trmm_uncopy_8.c`, `trmm_lncopy_8.c`,
//! `trmm_utcopy_8.c`, `trmm_ltcopy_8.c`, `kernel/Makefile.L3`, `KERNEL.HASWELL`. The logic is
//! re-expressed in Rust; no code was copied.

use super::level3::{GEMM_Q, Trans, kernel_block};
use larmorx_core::numpy::gemm::tile_4x8;
use rayon::prelude::*;

/// Which side the triangular matrix multiplies from (BLAS's `SIDE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// `'L'`: `B ← alpha·op(A)·B`.
    Left,
    /// `'R'`: `B ← alpha·B·op(A)`.
    Right,
}

/// Which triangle of `A` is stored (BLAS's `UPLO`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uplo {
    /// `'U'`.
    Upper,
    /// `'L'`.
    Lower,
}

/// Whether the diagonal is taken as 1 (BLAS's `DIAG`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diag {
    /// `'U'`: unit diagonal, not read.
    Unit,
    /// `'N'`: the stored diagonal.
    NonUnit,
}

/// `GEMM_P` of the Haswell core.
const GEMM_P: usize = 512;
/// `GEMM_UNROLL_M` of the Haswell core.
const UNROLL_M: usize = 4;
/// The largest `n` the right side takes: one column block of the driver (`GEMM_R`) with the
/// smallest buffer the wheels may be built with and `GEMM_Q = 512` (`BUFFERSIZE = 20`;
/// [`super::level3::SYRK_MAX_N`] for the 256 depth).
pub const TRMM_RIGHT_MAX_N: usize = 7_664;
/// Columns of `B` per task on the left side (a multiple of 8, so every kernel call keeps its
/// column groups).
const LEFT_PANEL: usize = 64;

/// `min_i` of `trmm_L.c`: at most `GEMM_P`, rounded down to the `GEMM_UNROLL_M` grid when
/// larger than it.
fn round_m(x: usize) -> usize {
    let x = x.min(GEMM_P);
    if x > UNROLL_M {
        x / UNROLL_M * UNROLL_M
    } else {
        x
    }
}

/// The four TRMM kernels (`LEFT`, `TRANSA` of `dtrmm_kernel_4x8_haswell.c`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kernel {
    /// Left, the triangle's steps from the row group on (`op(A)` upper).
    Ln,
    /// Left, the steps up to the row group's end (`op(A)` lower).
    Lt,
    /// Right, the steps up to the column group's end (`op(A)` upper).
    Rn,
    /// Right, the steps from the column group on (`op(A)` lower).
    Rt,
}

/// The groups `(start, width)` of `n` rows or columns: `n / first` groups of `first`, then one
/// of each smaller power of two that `n` holds.
fn groups(n: usize, first: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut s = 0;
    for _ in 0..n / first {
        out.push((s, first));
        s += first;
    }
    let mut w = first / 2;
    while w >= 1 {
        if n & w != 0 {
            out.push((s, w));
            s += w;
        }
        w /= 2;
    }
    out
}

/// One TRMM kernel call of `bm × bn` outputs over `bk` steps with the driver's `offset`:
/// `pa[i·bk + k]` the packed row `i`, `pb[j·bk + k]` the packed column `j`; `emit(i, j, v)`
/// stores (the kernel overwrites `C`).
#[allow(clippy::too_many_arguments)]
fn trmm_kernel(
    kind: Kernel,
    bm: usize,
    bn: usize,
    bk: usize,
    offset: isize,
    pa: &[f64],
    pb: &[f64],
    emit: &mut impl FnMut(usize, usize, f64),
) {
    let (mut ap, mut bp) = (Vec::new(), Vec::new());
    for (cg, nr) in groups(bn, 8) {
        for (rg, mr) in groups(bm, 4) {
            let (k0, k1): (isize, isize) = match kind {
                Kernel::Ln => (offset + rg as isize, bk as isize),
                Kernel::Lt => (0, offset + (rg + mr) as isize),
                Kernel::Rn => (0, (cg + nr) as isize - offset),
                Kernel::Rt => (cg as isize - offset, bk as isize),
            };
            assert!(
                0 <= k0 && k0 <= k1 && k1 <= bk as isize,
                "TRMM kernel range"
            );
            let (k0, k1) = (k0 as usize, k1 as usize);
            let l = k1 - k0;
            if mr == 4 && nr == 8 {
                // The assembly path: 32 fused chains from +0, times alpha = 1.
                ap.clear();
                bp.clear();
                for k in k0..k1 {
                    ap.extend((0..4).map(|r| pa[(rg + r) * bk + k]));
                    bp.extend((0..8).map(|c| pb[(cg + c) * bk + k]));
                }
                let t = tile_4x8(&ap, &bp, l);
                for (c, col) in t.iter().enumerate() {
                    for (r, &v) in col.iter().enumerate() {
                        emit(rg + r, cg + c, v);
                    }
                }
            } else {
                for c in cg..cg + nr {
                    let b = &pb[c * bk + k0..c * bk + k1];
                    for r in rg..rg + mr {
                        let a = &pa[r * bk + k0..r * bk + k1];
                        let mut s = 0.0f64;
                        for (&x, &y) in a.iter().zip(b) {
                            let p = x * y;
                            s += p;
                        }
                        emit(r, c, s);
                    }
                }
            }
        }
    }
}

/// One `GEMM_KERNEL` call of `mc × nc` over `l` steps with `alpha = 1`, added to
/// `out[row0 + i + (col0 + j)·ldo]`.
#[allow(clippy::too_many_arguments)]
fn gemm_add(
    mc: usize,
    nc: usize,
    l: usize,
    fa: impl Fn(usize, usize) -> f64,
    fb: impl Fn(usize, usize) -> f64,
    out: &mut [f64],
    ldo: usize,
    (row0, col0): (usize, usize),
) {
    if mc == 0 || nc == 0 || l == 0 {
        return;
    }
    let (mut ap, mut bp) = (Vec::new(), Vec::new());
    let mut emit = |i: usize, j: usize, acc: f64| {
        let slot = &mut out[row0 + i + (col0 + j) * ldo];
        let scaled = 1.0 * acc;
        *slot += scaled;
    };
    kernel_block(
        mc,
        nc,
        l,
        &fa,
        &fb,
        (0, mc),
        (0, nc),
        &mut emit,
        &mut ap,
        &mut bp,
    );
}

/// `op(A)` with its triangle: the copies' stored values (the zeros of the other triangle, the
/// unit diagonal's 1).
#[derive(Clone, Copy)]
struct Tri<'a> {
    a: &'a [f64],
    aoff: usize,
    lda: usize,
    trans: Trans,
    /// Whether `op(A)` is upper triangular.
    upper: bool,
    unit: bool,
}

impl Tri<'_> {
    /// Element `(i, k)` of `op(A)` as stored (either triangle).
    #[inline(always)]
    fn op(&self, i: usize, k: usize) -> f64 {
        match self.trans {
            Trans::N => self.a[self.aoff + i + k * self.lda],
            Trans::T => self.a[self.aoff + k + i * self.lda],
        }
    }

    /// Element `(i, k)` of the triangular `op(A)`.
    #[inline(always)]
    fn tri(&self, i: usize, k: usize) -> f64 {
        if i == k {
            if self.unit { 1.0 } else { self.op(i, i) }
        } else if (self.upper && k > i) || (!self.upper && k < i) {
            self.op(i, k)
        } else {
            0.0
        }
    }
}

/// The left side on `nc` columns of `B` (`old`, `m × nc`, leading dimension `m`), into `out`
/// (same shape): `trmm_L.c`.
fn left_panel(tri: Tri<'_>, m: usize, nc: usize, q: usize, old: &[f64], out: &mut [f64]) {
    let bv = |k: usize, j: usize| old[k + j * m];
    let pb_of = |k0: usize, l: usize| -> Vec<f64> {
        let mut pb = Vec::with_capacity(nc * l);
        for j in 0..nc {
            pb.extend((0..l).map(|k| bv(k0 + k, j)));
        }
        pb
    };
    // TRMM_KERNEL on rows r0..r1 of the block of `l` steps from k0.
    let trmm_rows = |kind: Kernel, k0: usize, l: usize, r0: usize, r1: usize, out: &mut [f64]| {
        let pb = pb_of(k0, l);
        let mut is = r0;
        while is < r1 {
            let mi = round_m(r1 - is);
            let mut pa = Vec::with_capacity(mi * l);
            for i in 0..mi {
                pa.extend((0..l).map(|k| tri.tri(is + i, k0 + k)));
            }
            let mut emit = |i: usize, j: usize, v: f64| out[is + i + j * m] = v;
            trmm_kernel(kind, mi, nc, l, (is - k0) as isize, &pa, &pb, &mut emit);
            is += mi;
        }
    };
    // GEMM_KERNEL on rows r0..r1 with the block of `l` steps from k0.
    let gemm_rows = |k0: usize, l: usize, r0: usize, r1: usize, out: &mut [f64]| {
        let mut is = r0;
        while is < r1 {
            let mi = round_m(r1 - is);
            gemm_add(
                mi,
                nc,
                l,
                |i, kk| tri.op(is + i, k0 + kk),
                |kk, j| bv(k0 + kk, j),
                out,
                m,
                (is, 0),
            );
            is += mi;
        }
    };
    if tri.upper {
        let min_l = m.min(q);
        trmm_rows(Kernel::Ln, 0, min_l, 0, min_l, out);
        let mut ls = min_l;
        while ls < m {
            let l = (m - ls).min(q);
            gemm_rows(ls, l, 0, ls, out);
            trmm_rows(Kernel::Ln, ls, l, ls, ls + l, out);
            ls += q;
        }
    } else {
        let min_l = m.min(q);
        trmm_rows(Kernel::Lt, m - min_l, min_l, m - min_l, m, out);
        let mut ls = m - min_l;
        while ls > 0 {
            let l = ls.min(q);
            let bs = ls - l;
            trmm_rows(Kernel::Lt, bs, l, bs, ls, out);
            gemm_rows(bs, l, ls, m, out);
            ls = ls.saturating_sub(q);
        }
    }
}

/// The right side on `pm` rows of `B` (`old`, `pm × n`, leading dimension `pm`), into `out`
/// (same shape): `trmm_R.c` with one column block.
fn right_panel(tri: Tri<'_>, pm: usize, n: usize, q: usize, old: &[f64], out: &mut [f64]) {
    let bv = |i: usize, k: usize| old[i + k * pm];
    let trmm_cols = |kind: Kernel, ls: usize, l: usize, out: &mut [f64]| {
        let mut pa = Vec::with_capacity(pm * l);
        for i in 0..pm {
            pa.extend((0..l).map(|k| bv(i, ls + k)));
        }
        let mut pb = Vec::with_capacity(l * l);
        for c in 0..l {
            pb.extend((0..l).map(|k| tri.tri(ls + k, ls + c)));
        }
        let mut emit = |i: usize, c: usize, v: f64| out[i + (ls + c) * pm] = v;
        trmm_kernel(kind, pm, l, l, 0, &pa, &pb, &mut emit);
    };
    if !tri.upper {
        let mut ls = 0;
        while ls < n {
            let l = (n - ls).min(q);
            gemm_add(
                pm,
                ls,
                l,
                |i, kk| bv(i, ls + kk),
                |kk, c| tri.op(ls + kk, c),
                out,
                pm,
                (0, 0),
            );
            trmm_cols(Kernel::Rt, ls, l, out);
            ls += q;
        }
    } else {
        let mut ls = 0;
        while ls + q < n {
            ls += q;
        }
        loop {
            let l = (n - ls).min(q);
            trmm_cols(Kernel::Rn, ls, l, out);
            gemm_add(
                pm,
                n - ls - l,
                l,
                |i, kk| bv(i, ls + kk),
                |kk, c| tri.op(ls + kk, ls + l + c),
                out,
                pm,
                (0, ls + l),
            );
            if ls == 0 {
                break;
            }
            ls -= q;
        }
    }
}

/// `DTRMM(SIDE, UPLO, TRANSA, DIAG, M, N, ALPHA, A, LDA, B, LDB)`: `B ← alpha·op(A)·B`
/// (`Left`, `A` of `m × m`) or `B ← alpha·B·op(A)` (`Right`, `A` of `n × n`), `B` of `m × n`
/// column-major at `b[boff + i + j·ldb]`, `A` at `a[aoff + i + j·lda]`, as OpenBLAS computes it
/// on the Haswell kernels at one thread (module documentation). Parallel over blocks of rows
/// (right) or columns (left) in the current rayon pool; the bits do not depend on the number
/// of threads.
///
/// # Panics
/// On BLAS's parameter errors (`LDA`, `LDB`, storage too small), and on the right side when
/// `n > TRMM_RIGHT_MAX_N`.
#[allow(clippy::too_many_arguments)]
pub fn dtrmm(
    side: Side,
    uplo: Uplo,
    transa: Trans,
    diag: Diag,
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    b: &mut [f64],
    boff: usize,
    ldb: usize,
) {
    dtrmm_with(
        side, uplo, transa, diag, m, n, alpha, a, aoff, lda, b, boff, ldb, GEMM_Q,
    );
}

/// [`dtrmm`] with the triangle blocked by `gemm_q` (as [`super::level3::dgemm_with`]).
#[allow(clippy::too_many_arguments)]
pub fn dtrmm_with(
    side: Side,
    uplo: Uplo,
    transa: Trans,
    diag: Diag,
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    b: &mut [f64],
    boff: usize,
    ldb: usize,
    gemm_q: usize,
) {
    let nrowa = if side == Side::Left { m } else { n };
    assert!(ldb >= m.max(1), "DTRMM: LDB ({ldb}) < max(1, M)");
    assert!(lda >= nrowa.max(1), "DTRMM: LDA ({lda}) < max(1, {nrowa})");
    if m == 0 || n == 0 {
        return;
    }
    assert!(
        a.len() >= aoff + (nrowa - 1) * lda + nrowa,
        "DTRMM: A is too small"
    );
    assert!(
        b.len() >= boff + (n - 1) * ldb + m,
        "DTRMM: B is too small"
    );
    assert!(
        side == Side::Left || n <= TRMM_RIGHT_MAX_N,
        "DTRMM: the right side above N = {TRMM_RIGHT_MAX_N} is not ported"
    );
    if alpha != 1.0 {
        for j in 0..n {
            let col = &mut b[boff + j * ldb..boff + j * ldb + m];
            if alpha == 0.0 {
                col.iter_mut().for_each(|v| *v = 0.0);
            } else {
                col.iter_mut().for_each(|v| *v *= alpha);
            }
        }
        if alpha == 0.0 {
            return;
        }
    }
    let tri = Tri {
        a,
        aoff,
        lda,
        trans: transa,
        upper: (uplo == Uplo::Upper) != (transa == Trans::T),
        unit: diag == Diag::Unit,
    };
    // Panels: the driver's row blocks of GEMM_P on the right, column blocks on the left.
    let (count, width) = match side {
        Side::Right => (m, GEMM_P),
        Side::Left => (n, LEFT_PANEL),
    };
    let panels: Vec<(usize, usize)> = (0..count)
        .step_by(width)
        .map(|s| (s, (s + width).min(count)))
        .collect();
    let compute = |&(p0, p1): &(usize, usize), old: Vec<f64>| -> Vec<f64> {
        let mut out = vec![0.0f64; old.len()];
        match side {
            Side::Right => right_panel(tri, p1 - p0, n, gemm_q, &old, &mut out),
            Side::Left => left_panel(tri, m, p1 - p0, gemm_q, &old, &mut out),
        }
        out
    };
    let wave = rayon::current_num_threads().max(1) * 2;
    for chunk in panels.chunks(wave) {
        let inputs: Vec<Vec<f64>> = chunk
            .iter()
            .map(|&(p0, p1)| match side {
                Side::Right => {
                    let mut t = Vec::with_capacity((p1 - p0) * n);
                    for j in 0..n {
                        t.extend_from_slice(&b[boff + p0 + j * ldb..boff + p1 + j * ldb]);
                    }
                    t
                }
                Side::Left => {
                    let mut t = Vec::with_capacity(m * (p1 - p0));
                    for j in p0..p1 {
                        t.extend_from_slice(&b[boff + j * ldb..boff + m + j * ldb]);
                    }
                    t
                }
            })
            .collect();
        let outputs: Vec<Vec<f64>> = if chunk.len() == 1 {
            vec![compute(&chunk[0], inputs.into_iter().next().expect("one"))]
        } else {
            chunk
                .par_iter()
                .zip(inputs.into_par_iter())
                .map(|(p, old)| compute(p, old))
                .collect()
        };
        for (&(p0, p1), t) in chunk.iter().zip(outputs) {
            match side {
                Side::Right => {
                    let pm = p1 - p0;
                    for (j, col) in t.chunks_exact(pm).enumerate() {
                        b[boff + p0 + j * ldb..boff + p1 + j * ldb].copy_from_slice(col);
                    }
                }
                Side::Left => {
                    for (jj, col) in t.chunks_exact(m).enumerate() {
                        let j = p0 + jj;
                        b[boff + j * ldb..boff + m + j * ldb].copy_from_slice(col);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn groups_follow_the_kernel() {
        assert_eq!(groups(13, 8), vec![(0, 8), (8, 4), (12, 1)]);
        assert_eq!(groups(7, 4), vec![(0, 4), (4, 2), (6, 1)]);
        assert_eq!(groups(3, 8), vec![(0, 2), (2, 1)]);
        assert_eq!(round_m(7), 4);
        assert_eq!(round_m(3), 3);
        assert_eq!(round_m(1030), 512);
    }

    /// Every combination against the plain triangular product (closely: the order differs).
    #[test]
    fn every_variant_is_the_triangular_product() {
        for side in [Side::Left, Side::Right] {
            for uplo in [Uplo::Upper, Uplo::Lower] {
                for transa in [Trans::N, Trans::T] {
                    for diag in [Diag::Unit, Diag::NonUnit] {
                        for &(m, n) in &[(1usize, 1usize), (7, 5), (13, 30), (300, 9), (9, 300)] {
                            let na = if side == Side::Left { m } else { n };
                            let a = values(na * na, 1);
                            let b0 = values(m * n, 2);
                            let mut b = b0.clone();
                            dtrmm(side, uplo, transa, diag, m, n, 0.5, &a, 0, na, &mut b, 0, m);
                            let t = Tri {
                                a: &a,
                                aoff: 0,
                                lda: na,
                                trans: transa,
                                upper: (uplo == Uplo::Upper) != (transa == Trans::T),
                                unit: diag == Diag::Unit,
                            };
                            for j in 0..n {
                                for i in 0..m {
                                    let want: f64 = match side {
                                        Side::Left => (0..m).map(|k| t.tri(i, k) * b0[k + j * m]).sum(),
                                        Side::Right => (0..n).map(|k| b0[i + k * m] * t.tri(k, j)).sum(),
                                    };
                                    let got = b[i + j * m];
                                    assert!(
                                        (got - 0.5 * want).abs() < 1e-12,
                                        "{side:?} {uplo:?} {transa:?} {diag:?} ({m}, {n}) at ({i}, {j})"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn alpha_zero_clears_and_infinities_meet_the_stored_zeros() {
        let a = vec![1.0f64; 16];
        let mut b = vec![f64::NAN; 8];
        dtrmm(Side::Left, Uplo::Upper, Trans::N, Diag::NonUnit, 4, 2, 0.0, &a, 0, 4, &mut b, 0, 4);
        assert!(b.iter().all(|v| v.to_bits() == 0));
        // Rows 1..3 of a 4-row group multiply the stored zero of row 0's step by B(0, j).
        let mut b = vec![1.0f64; 4 * 8];
        b[0] = f64::INFINITY;
        dtrmm(Side::Left, Uplo::Upper, Trans::N, Diag::NonUnit, 4, 8, 1.0, &a, 0, 4, &mut b, 0, 4);
        assert!(b[0].is_infinite());
        assert!(b[1].is_nan() && b[2].is_nan() && b[3].is_nan());
    }
}

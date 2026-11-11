// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! OpenBLAS's `dgemv` on the Haswell kernels at one thread (OpenBLAS 0.3.34 and 0.3.30: the same
//! files), any size, `alpha`, `beta` and positive increments, on every platform: the products
//! with a vector numpy's `matmul` and LAPACK's reflectors make.
//!
//! - **Interface** (`interface/gemv.c`): nothing for `m = 0` or `n = 0`; `y ← beta·y` when
//!   `beta ≠ 1` (`dscal`: `beta = 0` stores +0, whatever `y` held); nothing more for
//!   `alpha = 0`.
//! - **`dgemv_n`** (`kernel/x86_64/dgemv_n_4.c`, `dgemv_n_microk_haswell-4.c`): the first
//!   `m - m mod 4` rows in blocks of `NBMAX = 2048` (the last block shorter), each block's
//!   columns in groups of four (Haswell: per row `t = fma(a2, x2, a0·x0) + fma(a3, x3, a1·x1)`,
//!   `y = fma(alpha, t, y)`), then a pair (`y = fma(alpha, a0·x0 + a1·x1, y)`), then a single
//!   column (SSE2: `y = y + a·(x·alpha)`); with a stride in `x` the columns after the groups of
//!   four go one at a time; with a stride in `y` the block is summed from +0 and then added to
//!   `y`. The last `m mod 4` rows: `temp` from +0 over the columns (each product rounded; with
//!   `lda = m mod 4` and unit stride the columns in pairs, `temp += (p0 + p1)`; one row with
//!   `lda = 1` four at a time, `temp += ((p0 + p1) + p2) + p3`), then `y += alpha·temp`. The C
//!   parts of this file are compiled without fused multiply-adds.
//! - **`dgemv_t`** (`kernel/x86_64/dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`): the same row
//!   blocks; each block's columns in groups of four (four fused lanes per column, rows modulo
//!   4, `(l0 + l2) + (l1 + l3)`; `add_y`: with unit stride `y + (d·alpha)`, otherwise
//!   `fma(d, alpha, y)`), then a pair (two SSE2 lanes) and a single column (four SSE2 lanes),
//!   each `y = fma(d, alpha, y)`; the last `m mod 4` rows with `xt = x·alpha`: three rows
//!   `fma(a2, xt2, fma(a0, xt0, a1·xt1)) + y`, two `fma(a0, xt0, a1·xt1) + y`, one
//!   `fma(a, xt, y)`. The C parts of this file are compiled with contractions (GCC's
//!   `-ffp-contract=fast` with `-mfma`: 47 fused multiply-adds in the Haswell build's
//!   `dgemv_t`), these are their forms.
//!
//! Upstream files (BSD-3-Clause; the notice is in the parent module and NOTICE): OpenBLAS
//! 0.3.34 and 0.3.30 `interface/gemv.c`, `kernel/x86_64/dgemv_n_4.c`,
//! `dgemv_n_microk_haswell-4.c`, `dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`, `dscal.c`. The
//! logic is re-expressed in Rust; no code was copied.

use super::level3::Trans;

/// `NBMAX` of `dgemv_n_4.c` and `dgemv_t_4.c`.
const NBMAX: usize = 2048;

/// The row blocks `(start, length)` of the first `m - m mod 4` rows (both kernels' `while (NB
/// == NBMAX)` loop).
fn row_blocks(m: usize) -> Vec<(usize, usize)> {
    let m3 = m & 3;
    let mut m1 = (m & !3) as isize;
    let m2 = (m & (NBMAX - 1)) - m3;
    let mut out = Vec::new();
    let mut start = 0;
    let mut nb = NBMAX;
    while nb == NBMAX {
        m1 -= NBMAX as isize;
        if m1 < 0 {
            if m2 == 0 {
                break;
            }
            nb = m2;
        }
        out.push((start, nb));
        start += nb;
    }
    out
}

/// `DDOT(N, X, INCX, Y, INCY)` with positive increments (`kernel/x86_64/ddot.c`): unit strides
/// through the Haswell kernel (`numpy_linalg::blas::ddot`); otherwise four at a time, `temp1 +=
/// p0 + p2`, `temp2 += p1 + p3`, the rest into `temp1`, then `temp1 + temp2` (no fused
/// multiply-adds).
pub fn ddot(
    n: usize,
    x: &[f64],
    xoff: usize,
    incx: usize,
    y: &[f64],
    yoff: usize,
    incy: usize,
) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if incx == 1 && incy == 1 {
        return crate::numpy_linalg::blas::ddot(&x[xoff..xoff + n], &y[yoff..yoff + n]);
    }
    let p = |t: usize| y[yoff + t * incy] * x[xoff + t * incx];
    let (mut t1, mut t2) = (0.0f64, 0.0f64);
    let n1 = n & !3;
    let mut i = 0;
    while i < n1 {
        let (m1, m2, m3, m4) = (p(i), p(i + 1), p(i + 2), p(i + 3));
        t1 += m1 + m3;
        t2 += m2 + m4;
        i += 4;
    }
    while i < n {
        t1 += p(i);
        i += 1;
    }
    t1 + t2
}

/// `y ← alpha·op(A)·x + beta·y` (`DGEMV(TRANS, M, N, ALPHA, A, LDA, X, INCX, BETA, Y,
/// INCY)`), `A` of `m × n` column-major at `a[aoff + i + j·lda]`, `x` at `x[xoff + t·incx]`,
/// `y` at `y[yoff + t·incy]` (module documentation).
#[allow(clippy::too_many_arguments)]
pub fn dgemv(
    trans: Trans,
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
    xoff: usize,
    incx: usize,
    beta: f64,
    y: &mut [f64],
    yoff: usize,
    incy: usize,
) {
    assert!(lda >= m.max(1), "DGEMV: LDA ({lda}) < M ({m})");
    assert!(
        incx > 0 && incy > 0,
        "DGEMV: increments must be positive here"
    );
    if m == 0 || n == 0 {
        return;
    }
    let leny = if trans == Trans::N { m } else { n };
    if beta != 1.0 {
        for t in 0..leny {
            let v = &mut y[yoff + t * incy];
            *v = if beta == 0.0 { 0.0 } else { *v * beta };
        }
    }
    if alpha == 0.0 {
        return;
    }
    match trans {
        Trans::N => gemv_n(m, n, alpha, a, aoff, lda, x, xoff, incx, y, yoff, incy),
        Trans::T => gemv_t(m, n, alpha, a, aoff, lda, x, xoff, incx, y, yoff, incy),
    }
}

#[allow(clippy::too_many_arguments)]
fn gemv_n(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
    xoff: usize,
    incx: usize,
    y: &mut [f64],
    yoff: usize,
    incy: usize,
) {
    let xv = |j: usize| x[xoff + j * incx];
    let av = |i: usize, j: usize| a[aoff + i + j * lda];
    let n1 = n >> 2;
    for (r0, nb) in row_blocks(m) {
        let mut buf = vec![0.0f64; nb];
        if incy == 1 {
            buf.copy_from_slice(&y[yoff + r0..yoff + r0 + nb]);
        }
        let mut j = 0;
        for _ in 0..n1 {
            let (x0, x1, x2, x3) = (xv(j), xv(j + 1), xv(j + 2), xv(j + 3));
            for (r, yr) in buf.iter_mut().enumerate() {
                let i = r0 + r;
                let t4 = av(i, j + 2).mul_add(x2, av(i, j) * x0);
                let t5 = av(i, j + 3).mul_add(x3, av(i, j + 1) * x1);
                *yr = alpha.mul_add(t4 + t5, *yr);
            }
            j += 4;
        }
        let single = |j: usize, buf: &mut [f64]| {
            let xs = xv(j) * alpha;
            for (r, yr) in buf.iter_mut().enumerate() {
                let p = av(r0 + r, j) * xs;
                *yr += p;
            }
        };
        if incx == 1 {
            if n & 2 != 0 {
                let (x0, x1) = (xv(j), xv(j + 1));
                for (r, yr) in buf.iter_mut().enumerate() {
                    let i = r0 + r;
                    let t = av(i, j) * x0 + av(i, j + 1) * x1;
                    *yr = alpha.mul_add(t, *yr);
                }
                j += 2;
            }
            if n & 1 != 0 {
                single(j, &mut buf);
            }
        } else {
            while j < n {
                single(j, &mut buf);
                j += 1;
            }
        }
        if incy == 1 {
            y[yoff + r0..yoff + r0 + nb].copy_from_slice(&buf);
        } else {
            for (r, v) in buf.iter().enumerate() {
                y[yoff + (r0 + r) * incy] += v;
            }
        }
    }
    let m3 = m & 3;
    let base = m - m3;
    for r in 0..m3 {
        let i = base + r;
        let mut temp = 0.0f64;
        if lda == m3 && incx == 1 && m3 >= 2 {
            let mut j = 0;
            while j < (n & !3) {
                for half in 0..2 {
                    let c = j + 2 * half;
                    let (p0, p1) = (av(i, c) * xv(c), av(i, c + 1) * xv(c + 1));
                    temp += p0 + p1;
                }
                j += 4;
            }
            for c in j..n {
                let p = av(i, c) * xv(c);
                temp += p;
            }
        } else if lda == 1 && incx == 1 && m3 == 1 {
            let mut c = 0;
            while c < (n & !3) {
                let p: [f64; 4] = std::array::from_fn(|t| av(i, c + t) * xv(c + t));
                temp += ((p[0] + p[1]) + p[2]) + p[3];
                c += 4;
            }
            for c in c..n {
                let p = av(i, c) * xv(c);
                temp += p;
            }
        } else {
            for c in 0..n {
                let p = av(i, c) * xv(c);
                temp += p;
            }
        }
        let s = alpha * temp;
        y[yoff + i * incy] += s;
    }
}

/// `dgemv_kernel_4x4` (Haswell) for one column of a row block.
fn dot_fma4(a: &[f64], x: &[f64]) -> f64 {
    let mut l = [0.0f64; 4];
    for (ra, rx) in a.as_chunks::<4>().0.iter().zip(x.as_chunks::<4>().0) {
        for t in 0..4 {
            l[t] = ra[t].mul_add(rx[t], l[t]);
        }
    }
    (l[0] + l[2]) + (l[1] + l[3])
}

/// `dgemv_kernel_4x2` (SSE2) for one column: two lanes, each product rounded, then `haddpd`.
fn dot_pair(a: &[f64], x: &[f64]) -> f64 {
    let (mut l0, mut l1) = (0.0f64, 0.0f64);
    for (ra, rx) in a.as_chunks::<2>().0.iter().zip(x.as_chunks::<2>().0) {
        let (p0, p1) = (rx[0] * ra[0], rx[1] * ra[1]);
        l0 += p0;
        l1 += p1;
    }
    l0 + l1
}

/// `dgemv_kernel_4x1` (SSE2): four lanes, each product rounded, `(s0 + s2) + (s1 + s3)`.
fn dot_single(a: &[f64], x: &[f64]) -> f64 {
    let mut s = [0.0f64; 4];
    for (ra, rx) in a.as_chunks::<4>().0.iter().zip(x.as_chunks::<4>().0) {
        for t in 0..4 {
            let p = ra[t] * rx[t];
            s[t] += p;
        }
    }
    (s[0] + s[2]) + (s[1] + s[3])
}

#[allow(clippy::too_many_arguments)]
fn gemv_t(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
    xoff: usize,
    incx: usize,
    y: &mut [f64],
    yoff: usize,
    incy: usize,
) {
    let n4 = n & !3;
    for (r0, nb) in row_blocks(m) {
        let xb: Vec<f64> = (0..nb).map(|t| x[xoff + (r0 + t) * incx]).collect();
        for j in 0..n {
            let col = &a[aoff + r0 + j * lda..aoff + r0 + j * lda + nb];
            let slot = &mut y[yoff + j * incy];
            if j < n4 {
                let d = dot_fma4(col, &xb);
                *slot = if incy == 1 {
                    let s = d * alpha;
                    *slot + s
                } else {
                    d.mul_add(alpha, *slot)
                };
            } else if n & 2 != 0 && j < n4 + 2 {
                *slot = dot_pair(col, &xb).mul_add(alpha, *slot);
            } else {
                *slot = dot_single(col, &xb).mul_add(alpha, *slot);
            }
        }
    }
    let m3 = m & 3;
    if m3 == 0 {
        return;
    }
    let base = m - m3;
    let xt: Vec<f64> = (0..m3)
        .map(|t| x[xoff + (base + t) * incx] * alpha)
        .collect();
    for j in 0..n {
        let col = aoff + base + j * lda;
        let slot = &mut y[yoff + j * incy];
        *slot = match m3 {
            1 => a[col].mul_add(xt[0], *slot),
            2 => a[col].mul_add(xt[0], a[col + 1] * xt[1]) + *slot,
            _ => {
                let t = a[col].mul_add(xt[0], a[col + 1] * xt[1]);
                a[col + 2].mul_add(xt[2], t) + *slot
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_blocks_cover_the_body() {
        for m in [0usize, 3, 4, 7, 2047, 2048, 2051, 4096, 4100, 6001] {
            let b = row_blocks(m);
            let total: usize = b.iter().map(|x| x.1).sum();
            assert_eq!(total, m - (m & 3), "m = {m}");
            assert!(b.iter().all(|x| x.1 % 4 == 0 && x.1 <= NBMAX));
        }
        assert_eq!(row_blocks(4100), vec![(0, 2048), (2048, 2048), (4096, 4)]);
    }
}

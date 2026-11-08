// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `numpy.dot` of float64 vectors and matrices as numpy computes it on x86-64 with the OpenBLAS
//! its wheels bundle (OpenBLAS 0.3.34 for numpy 2.5.4, 0.3.30 for numpy 2.3.5: the same kernel
//! files, compiled to the same arithmetic), on the Haswell kernels
//! (`OPENBLAS_CORETYPE=Haswell`, `docs/findings/numpy-blas.md`), at one BLAS thread, on every
//! platform. SciPy's `least_squares` (the trust-region reflective method) makes these products
//! on its Jacobian, its residuals and the factors of its SVD.
//!
//! numpy's `dot` (`common/cblasfuncs.c`, `cblas_matrixproduct`; `multiarray/arraytypes.c.src`,
//! `DOUBLE_dot`) hands them to CBLAS:
//!
//! - two vectors: `0. + cblas_ddot(n, x, 1, y, 1)` ([`dot`]); OpenBLAS's `ddot.c` runs the
//!   first `16·⌊n/16⌋` elements through `ddot_microk_haswell-2.c` (sixteen fused multiply-add
//!   lanes: element `k` in lane `k mod 16`; the four registers' halves added, then the
//!   registers pairwise, then the two halves) and adds the rest from there, each product
//!   rounded (`ddot.c` is compiled without `-mfma`);
//! - a matrix and a vector: `cblas_dgemv(order, NoTrans, ...)` with `order` row-major when the
//!   matrix is C-contiguous, column-major otherwise; OpenBLAS turns a row-major call into a
//!   column-major one with the matrix transposed, so a C-contiguous `rows × cols` matrix
//!   reaches `dgemv_t` ([`gemv_t`]: `dgemv_t_4.c`, the kernels of the parent module's
//!   `lstsq`), and an F-contiguous one `dgemv_n` ([`gemv_n`]: `dgemv_n_4.c`'s paths for fewer
//!   than four rows, compiled without `-mfma`: each product rounded, and with a leading
//!   dimension equal to the number of rows two columns added before they reach the sum).
//!   `y` is cleared to +0 first (`beta = 0`) and `alpha = 1`.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.34 and 0.3.30 `interface/gemv.c`, `interface/dot.c`, `kernel/x86_64/ddot.c`,
//! `ddot_microk_haswell-2.c`, `dgemv_n_4.c`, `dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`,
//! `KERNEL.HASWELL`; numpy 2.5.4 and 2.3.5 `numpy/_core/src/common/cblasfuncs.c` and
//! `numpy/_core/src/multiarray/arraytypes.c.src`. The logic is re-expressed in Rust; no code was
//! copied.

use super::lstsq::{gemv_class, gemv_t_column};

/// The memory order of a two-dimensional numpy array, which decides the BLAS call numpy makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// C-contiguous (row-major): element `(i, j)` at `i · cols + j`.
    C,
    /// F-contiguous (column-major): element `(i, j)` at `i + j · rows`.
    F,
}

/// `cblas_ddot(n, x, 1, y, 1)` with OpenBLAS's Haswell kernel, at one thread (module
/// documentation). `x` and `y` have the same length.
pub fn ddot(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "ddot needs two vectors of one length");
    let n = x.len();
    let n1 = n & !15;
    let mut dot = 0.0f64;
    if n1 > 0 {
        // ymm4..ymm7: lane l of register g holds the elements k with k mod 16 = 4g + l.
        let mut acc = [[0.0f64; 4]; 4];
        for (cx, cy) in x[..n1]
            .as_chunks::<16>()
            .0
            .iter()
            .zip(y[..n1].as_chunks::<16>().0)
        {
            for (k, (xk, yk)) in cx.iter().zip(cy).enumerate() {
                let lane = &mut acc[k / 4][k % 4];
                *lane = xk.mul_add(*yk, *lane);
            }
        }
        // vextractf128 + vaddpd: (l0 + l2, l1 + l3) per register; then (4 + 5) and (6 + 7),
        // their sum, and vhaddpd of the two halves.
        let half = |g: usize| [acc[g][0] + acc[g][2], acc[g][1] + acc[g][3]];
        let (h0, h1, h2, h3) = (half(0), half(1), half(2), half(3));
        let a = [h1[0] + h0[0], h1[1] + h0[1]];
        let b = [h3[0] + h2[0], h3[1] + h2[1]];
        let c = [b[0] + a[0], b[1] + a[1]];
        dot = c[0] + c[1];
    }
    for i in n1..n {
        let p = y[i] * x[i];
        dot += p;
    }
    dot
}

/// `numpy.dot(x, y)` of two contiguous float64 vectors: `0. + cblas_ddot` (`DOUBLE_dot`).
pub fn dot(x: &[f64], y: &[f64]) -> f64 {
    0.0 + ddot(x, y)
}

/// `DGEMV('T', m, n, 1, A, lda, x, 1, 0, y, 1)` (`dgemv_t_4.c` on the Haswell kernels, any
/// size): `y[j]` is column `j` of the column-major `A` (`m` values from `j · lda`) times `x`,
/// with the kernel class of column `j` among `n` (the parent module's `lstsq`).
pub fn gemv_t(m: usize, n: usize, a: &[f64], lda: usize, x: &[f64]) -> Vec<f64> {
    if m == 0 || n == 0 {
        return vec![0.0; n];
    }
    (0..n)
        .map(|j| gemv_t_column(gemv_class(j, n), &a[j * lda..j * lda + m], &x[..m]))
        .collect()
}

/// `DGEMV('N', m, n, 1, A, lda, x, 1, 0, y, 1)` for `m = 2` (`dgemv_n_4.c`'s path for two
/// rows; no fused multiply-adds): with `lda = 2` the columns go in groups of four as two
/// pairs, `temp += (a0·x0 + a2·x1)` twice, then one at a time; otherwise one at a time; then
/// `y = 0 + temp`.
pub fn gemv_n(m: usize, n: usize, a: &[f64], lda: usize, x: &[f64]) -> Vec<f64> {
    assert_eq!(m, 2, "gemv_n is ported for two rows");
    if n == 0 {
        return vec![0.0; 2];
    }
    let (mut t0, mut t1) = (0.0f64, 0.0f64);
    let mut j = 0;
    if lda == 2 {
        while j < (n & !3) {
            for half in 0..2 {
                let c = j + 2 * half;
                let (p00, p01) = (a[2 * c] * x[c], a[2 * c + 2] * x[c + 1]);
                let (p10, p11) = (a[2 * c + 1] * x[c], a[2 * c + 3] * x[c + 1]);
                t0 += p00 + p01;
                t1 += p10 + p11;
            }
            j += 4;
        }
    }
    while j < n {
        let p0 = a[j * lda] * x[j];
        let p1 = a[j * lda + 1] * x[j];
        t0 += p0;
        t1 += p1;
        j += 1;
    }
    vec![0.0 + t0, 0.0 + t1]
}

/// `a.dot(x)` for a two-dimensional float64 numpy array `a` (`rows × cols`, in `layout`) and a
/// contiguous vector of `cols` values (module documentation): `dgemv_t` for a C-contiguous
/// `a`, `dgemv_n` for an F-contiguous one (ported for two rows).
pub fn matvec(a: &[f64], rows: usize, cols: usize, layout: Layout, x: &[f64]) -> Vec<f64> {
    assert_eq!(a.len(), rows * cols, "a must hold rows × cols values");
    assert_eq!(x.len(), cols, "x must hold cols values");
    match layout {
        // RowMajor, NoTrans → dgemv_t(m = cols, n = rows, lda = cols).
        Layout::C => gemv_t(cols, rows, a, cols.max(1), x),
        // ColMajor, NoTrans → dgemv_n(m = rows, n = cols, lda = rows).
        Layout::F => gemv_n(rows, cols, a, rows.max(1), x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(n: usize, seed: u64) -> Vec<f64> {
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
    fn short_dot_is_the_plain_sum() {
        let x = data(15, 1);
        let y = data(15, 2);
        let mut want = 0.0f64;
        for i in 0..15 {
            want += y[i] * x[i];
        }
        assert_eq!(ddot(&x, &y).to_bits(), want.to_bits());
        assert_eq!(dot(&[], &[]).to_bits(), 0.0f64.to_bits());
    }

    #[test]
    fn long_dot_is_close() {
        let x = data(1003, 3);
        let y = data(1003, 4);
        let exact: f64 = x.iter().zip(&y).map(|(a, b)| a * b).sum();
        assert!((ddot(&x, &y) - exact).abs() < 1e-12);
    }

    #[test]
    fn matvec_layouts_agree_closely() {
        let (rows, cols) = (2usize, 9usize);
        let c = data(rows * cols, 5);
        let mut f = vec![0.0; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                f[i + j * rows] = c[i * cols + j];
            }
        }
        let x = data(cols, 6);
        let yc = matvec(&c, rows, cols, Layout::C, &x);
        let yf = matvec(&f, rows, cols, Layout::F, &x);
        for (p, q) in yc.iter().zip(&yf) {
            assert!((p - q).abs() < 1e-14);
        }
    }
}

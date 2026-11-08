// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's dot product and vector norm of two contiguous `float32` or `float64` vectors, in the
//! operation order of OpenBLAS's Haswell `sdot` and `ddot` kernels (OpenBLAS 0.3.34, the same in
//! 0.3.30; BSD-3-Clause, notice below), the ones numpy's x86-64 wheels run on CPUs with AVX2 and
//! FMA (`docs/findings/numpy-blas.md`). SciPy's conjugate gradients (`scipy.sparse.linalg.cg`)
//! take every inner product and norm here (scikit-learn's ridge solver under sdcflows' B-spline
//! fit, scikit-image's random walker under sdcflows' `brain_masker`), so the bits of their
//! solutions depend on this order. (Moved here from `larmorx-sdcflows`' `blas` module, which
//! re-exports it, when the random walker needed it too.)
//!
//! - **numpy** (`numpy/_core/src/multiarray/arraytypes.c.src`, `FLOAT_dot` and `DOUBLE_dot`,
//!   reached by `np.dot` of two vectors through `cblas_matrixproduct`, and by `x.dot(x)` in
//!   `np.linalg.norm`; numpy 2.5.4 and 2.3.5): `double sum = 0.; sum += cblas_?dot(n, x, 1, y,
//!   1)` in chunks of `CBLAS_INT_MAX / 2 + 1` elements (one chunk with the 64-bit integers of
//!   scipy-openblas64), then the sum in the vector's type. With one chunk this is the BLAS result
//!   unchanged (but a `-0` becomes `+0`). numpy treats a one-element vector as a scalar
//!   (`cblasfuncs.c`, `_select_matrix_shape`): its dot product is the plain product.
//!   `np.linalg.norm` of a vector is `sqrt(x.dot(x))` in the vector's type.
//! - **OpenBLAS `sdot`** (`interface/dot.c`, `kernel/x86_64/sdot.c` with
//!   `sdot_microk_haswell-2.c`, selected by `KERNEL.HASWELL` and `KERNEL.ZEN`): no threads. For
//!   unit strides, the first `n1 = n & -32` elements go through the assembly kernel: four ymm
//!   accumulators of eight float32 lanes, cleared to +0, each lane a chain of fused multiply-adds
//!   (`vfmadd231ps`) over the elements `32 t + 8 k + j`; then each accumulator's high half is
//!   added to its low half, the halves of accumulators 4 and 5 and of 6 and 7 are added, the two
//!   sums are added, and two horizontal adds (`vhaddps`) leave `(s0 + s1) + (s2 + s3)`. The
//!   kernel stores that float (`mydot`). The remaining `n - n1` elements are added to a
//!   **double** `dot` (from +0), each product rounded to float first (`dot += y[i] * x[i]` with
//!   float operands), then `dot += mydot`, and the function returns `(float)dot`.
//! - **OpenBLAS `ddot`** (`kernel/x86_64/ddot.c` with `ddot_microk_haswell-2.c`): the first
//!   `n1 = n & -16` elements through the assembly kernel: four ymm accumulators of four doubles,
//!   cleared to +0, each lane a chain of `vfmadd231pd` over the elements `16 t + 4 k + j`; each
//!   accumulator's high half added to its low half, then `(h4 + h5) + (h6 + h7)` and one
//!   `vhaddpd`; the kernel stores that double into `dot`. The remaining elements are added to
//!   `dot` one by one (`dot += y[i] * x[i]`: a rounded product, then a rounded sum; measured
//!   against numpy 2.3.5 and 2.5.4, the compiler did not fuse them).
//!   With several OpenBLAS threads, vectors of more than 10,000 elements are split between them
//!   (`dot_thread_function`); larmorx follows the one-thread order (`OPENBLAS_NUM_THREADS=1`,
//!   the validation environments').
//!
//! `f32::mul_add` and `f64::mul_add` are correctly rounded fused multiply-adds on every target,
//! so they give the bits of `vfmadd231ps`/`vfmadd231pd` on every platform; on x86-64 CPUs with
//! FMA the kernels are compiled with it (an audited dispatch, below), elsewhere `mul_add` is the
//! software fused multiply-add with the same bits.
//!
//! OpenBLAS: Copyright (c) 2014, The OpenBLAS Project. All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the OpenBLAS project nor the names of its contributors may be used to
//!    endorse or promote products derived from this software without specific prior written
//!    permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
//! GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
//! ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
//! NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//! OF THE POSSIBILITY OF SUCH DAMAGE.
//!
//! numpy: notice in `super`.

/// The assembly kernel of `sdot_microk_haswell-2.c` on the first `n1` elements (`n1` a
/// multiple of 32): the float it stores.
#[inline(always)]
fn sdot_kernel(x: &[f32], y: &[f32]) -> f32 {
    debug_assert!(x.len() == y.len() && x.len().is_multiple_of(32));
    // acc[k][j]: lane j of ymm(4 + k).
    let mut acc = [[0.0f32; 8]; 4];
    for (xb, yb) in x.as_chunks::<32>().0.iter().zip(y.as_chunks::<32>().0) {
        for (k, a) in acc.iter_mut().enumerate() {
            for (j, lane) in a.iter_mut().enumerate() {
                let i = 8 * k + j;
                *lane = xb[i].mul_add(yb[i], *lane);
            }
        }
    }
    // vextractf128 + vaddps: low half plus high half, per accumulator.
    let h: [[f32; 4]; 4] =
        std::array::from_fn(|k| std::array::from_fn(|j| acc[k][j + 4] + acc[k][j]));
    // vaddps xmm4, xmm5 -> xmm4; vaddps xmm6, xmm7 -> xmm6; vaddps xmm4, xmm6 -> xmm4.
    let s: [f32; 4] = std::array::from_fn(|j| {
        let s45 = h[1][j] + h[0][j];
        let s67 = h[3][j] + h[2][j];
        s67 + s45
    });
    // Two vhaddps of the register with itself: [s0 + s1, s2 + s3, ..], then their sum.
    (s[0] + s[1]) + (s[2] + s[3])
}

/// The assembly kernel of `ddot_microk_haswell-2.c` on the first `n1` elements (`n1` a multiple
/// of 16): the double it stores.
#[inline(always)]
fn ddot_kernel(x: &[f64], y: &[f64]) -> f64 {
    debug_assert!(x.len() == y.len() && x.len().is_multiple_of(16));
    let mut acc = [[0.0f64; 4]; 4];
    for (xb, yb) in x.as_chunks::<16>().0.iter().zip(y.as_chunks::<16>().0) {
        for (k, a) in acc.iter_mut().enumerate() {
            for (j, lane) in a.iter_mut().enumerate() {
                let i = 4 * k + j;
                *lane = xb[i].mul_add(yb[i], *lane);
            }
        }
    }
    // vextractf128 + vaddpd (xmm(4 + k) = low + high), then 4 + 5, 6 + 7, their sum.
    let h: [[f64; 2]; 4] =
        std::array::from_fn(|k| std::array::from_fn(|j| acc[k][j] + acc[k][j + 2]));
    let s: [f64; 2] = std::array::from_fn(|j| {
        let s45 = h[0][j] + h[1][j];
        let s67 = h[2][j] + h[3][j];
        s45 + s67
    });
    // vhaddpd xmm4, xmm4.
    s[0] + s[1]
}

#[inline(always)]
fn sdot_body(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len();
    let n1 = n & !31;
    let mydot = if n1 > 0 {
        sdot_kernel(&x[..n1], &y[..n1])
    } else {
        0.0
    };
    let mut dot = 0.0f64;
    for (&xi, &yi) in x[n1..].iter().zip(&y[n1..]) {
        dot += f64::from(yi * xi);
    }
    dot += f64::from(mydot);
    dot as f32
}

#[inline(always)]
fn ddot_body(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let n1 = n & !15;
    let mut dot = if n1 > 0 {
        ddot_kernel(&x[..n1], &y[..n1])
    } else {
        0.0
    };
    for (&xi, &yi) in x[n1..].iter().zip(&y[n1..]) {
        dot += yi * xi;
    }
    dot
}

/// OpenBLAS's `sdot(n, x, 1, y, 1)` with the Haswell kernel.
///
/// # Panics
/// When the lengths differ (a programming error: every caller passes vectors of one length).
pub fn sdot(x: &[f32], y: &[f32]) -> f32 {
    assert_eq!(x.len(), y.len(), "sdot of vectors of different lengths");
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return fma::sdot(x, y);
    }
    sdot_body(x, y)
}

/// OpenBLAS's `ddot(n, x, 1, y, 1)` with the Haswell kernel, one thread.
///
/// # Panics
/// When the lengths differ.
pub fn ddot(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "ddot of vectors of different lengths");
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return fma::ddot(x, y);
    }
    ddot_body(x, y)
}

/// `np.dot(x, y)` of two contiguous float32 vectors, as numpy's `cblas_matrixproduct`
/// (`numpy/_core/src/common/cblasfuncs.c`) computes it: a vector of one element counts as a
/// scalar there, so the result is the float product `x[0] * y[0]` (a signed zero stays
/// signed); empty vectors give +0 (the zeroed output); longer ones go to `FLOAT_dot` over
/// OpenBLAS's `sdot` (a double sum of one chunk, cast back to float).
pub fn dot_f32(x: &[f32], y: &[f32]) -> f32 {
    assert_eq!(x.len(), y.len(), "dot of vectors of different lengths");
    match x.len() {
        0 => 0.0,
        1 => y[0] * x[0],
        _ => (0.0f64 + f64::from(sdot(x, y))) as f32,
    }
}

/// `np.dot(x, y)` of two contiguous float64 vectors (`DOUBLE_dot` over OpenBLAS's `ddot`, as
/// [`dot_f32`]).
pub fn dot_f64(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "dot of vectors of different lengths");
    match x.len() {
        0 => 0.0,
        1 => y[0] * x[0],
        _ => 0.0f64 + ddot(x, y),
    }
}

/// `np.linalg.norm(x)` of a float32 vector: `sqrt(x.dot(x))` in float32.
pub fn norm_f32(x: &[f32]) -> f32 {
    dot_f32(x, x).sqrt()
}

/// `np.linalg.norm(x)` of a float64 vector: `sqrt(x.dot(x))`.
pub fn norm_f64(x: &[f64]) -> f64 {
    dot_f64(x, x).sqrt()
}

/// The kernels compiled with AVX2 and FMA. AUDIT (unsafe): each call is guarded by
/// `is_x86_feature_detected!("avx2")` and `("fma")` in [`sdot`] and [`ddot`]; calling a
/// `#[target_feature]` function requires `unsafe` only because the features must be present,
/// and the guard ensures it. The bodies are the portable ones (`mul_add` is the same fused
/// multiply-add either way), so the bits do not depend on the path.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
mod fma {
    #[inline]
    pub(super) fn sdot(x: &[f32], y: &[f32]) -> f32 {
        #[target_feature(enable = "avx2,fma")]
        fn kernel(x: &[f32], y: &[f32]) -> f32 {
            super::sdot_body(x, y)
        }
        // SAFETY: only called after `is_x86_feature_detected!("avx2")` and `("fma")` returned
        // true.
        unsafe { kernel(x, y) }
    }

    #[inline]
    pub(super) fn ddot(x: &[f64], y: &[f64]) -> f64 {
        #[target_feature(enable = "avx2,fma")]
        fn kernel(x: &[f64], y: &[f64]) -> f64 {
            super::ddot_body(x, y)
        }
        // SAFETY: as `sdot`.
        unsafe { kernel(x, y) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded sequence of floats of mixed signs and magnitudes (no transcendental functions).
    fn values(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let m = ((s >> 40) as i64 - (1 << 23)) as f32;
                m * f32::from(((s >> 20) & 7) as u8 + 1) / 4096.0
            })
            .collect()
    }

    #[test]
    fn short_vectors_are_a_double_sum_of_float_products() {
        let (x, y) = (values(31, 1), values(31, 2));
        let mut d = 0.0f64;
        for i in 0..31 {
            d += f64::from(y[i] * x[i]);
        }
        assert_eq!(sdot(&x, &y).to_bits(), (d as f32).to_bits());
        assert_eq!(dot_f32(&[], &[]).to_bits(), 0.0f32.to_bits());
        // One element: numpy's scalar product keeps the signed zero; two: sdot's +0 start.
        assert_eq!(dot_f32(&[-0.0], &[1.0]).to_bits(), (-0.0f32).to_bits());
        assert_eq!(
            dot_f32(&[-0.0, 0.0], &[1.0, -1.0]).to_bits(),
            0.0f32.to_bits()
        );
    }

    #[test]
    fn the_kernel_reduces_lanes_in_the_haswell_order() {
        // 32 elements: every lane holds one product (fma with +0 = the rounded product).
        let (x, y) = (values(32, 3), values(32, 4));
        let p: Vec<f32> = x.iter().zip(&y).map(|(a, b)| a * b).collect();
        let h = |k: usize, j: usize| p[8 * k + j + 4] + p[8 * k + j];
        let s: Vec<f32> = (0..4)
            .map(|j| (h(3, j) + h(2, j)) + (h(1, j) + h(0, j)))
            .collect();
        let want = (s[0] + s[1]) + (s[2] + s[3]);
        assert_eq!(sdot(&x, &y).to_bits(), want.to_bits());
        assert_eq!(norm_f32(&[3.0, 4.0]), 5.0);
    }

    #[test]
    fn ddot_kernel_and_tail() {
        let x: Vec<f64> = values(19, 5).iter().map(|&v| f64::from(v) / 3.0).collect();
        let y: Vec<f64> = values(19, 6).iter().map(|&v| f64::from(v) / 7.0).collect();
        let p: Vec<f64> = (0..16).map(|i| x[i] * y[i]).collect();
        let h = |k: usize, j: usize| p[4 * k + j] + p[4 * k + j + 2];
        let s: Vec<f64> = (0..2)
            .map(|j| (h(0, j) + h(1, j)) + (h(2, j) + h(3, j)))
            .collect();
        let mut want = s[0] + s[1];
        for i in 16..19 {
            want += y[i] * x[i];
        }
        assert_eq!(ddot(&x, &y).to_bits(), want.to_bits());
        assert_eq!(norm_f64(&[3.0, 4.0]), 5.0);
        assert_eq!(dot_f64(&[-0.0], &[2.0]).to_bits(), (-0.0f64).to_bits());
    }
}

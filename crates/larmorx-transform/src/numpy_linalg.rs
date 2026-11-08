// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy.linalg's `svd`, `eigh` and `det` of small float64 matrices (3 × 3 and 4 × 4), as numpy
//! computes them on x86-64 with the OpenBLAS its wheels bundle, on every platform.
//!
//! nibabel stores an affine in a NIfTI qform through `numpy.linalg.svd` (the closest rotation)
//! and `numpy.linalg.eigh` (`quaternions.mat2quat`), and decides `qfac` with
//! `numpy.linalg.det`. numpy hands these to LAPACK (`dgesdd` with `JOBZ='A'`, `dsyevd` with
//! `JOBZ='V'`, `UPLO='L'`, and OpenBLAS's own `dgetrf`), whose last bits, and the signs of
//! their zeros, depend on the LAPACK release and on the BLAS kernels it calls. This module
//! follows them operation for operation, so a port of a Python program that writes NIfTI
//! headers through nibabel writes the same quaternion fields:
//!
//! - LAPACK as OpenBLAS 0.3.34 bundles it (`lapack-netlib`, LAPACK 3.12.1, numpy 2.5.4's
//!   scipy-openblas64), or as OpenBLAS 0.3.30 bundles it (LAPACK 3.12.0, numpy 2.3.5): the two
//!   differ in how a Householder reflector is applied (3.12.1's `DLARF1F` adds the reflector's
//!   implicit leading 1 after the product of the other rows; 3.12.0's `DLARF` stores the 1 and
//!   lets `dgemv` and `dger` include it), see [`LapackRelease`];
//! - the routines on the paths numpy reaches for these sizes, every one unblocked:
//!   `dgesdd` path 5a (`dgebrd` → `dgebd2`, `dbdsdc` → `dlasdq` → `dbdsqr`, `dormbr` →
//!   `dorm2r`/`dorml2`), `dsyevd` (`dsytrd` → `dsytd2`, `dstedc` → `dsteqr`, `dormtr` →
//!   `dorm2r`), with `dlarfg`, `dlapy2`, `dlartg` (the Fortran 90 version), `dlas2`, `dlasv2`,
//!   `dlaev2`, `dlasr`, `dlanst`, `dlascl`, `iladlc`, `iladlr`; gfortran compiled them for the
//!   x86-64 baseline (no fused multiply-adds: checked in the disassembly), so every operation is
//!   rounded on its own, in the source's order;
//! - the BLAS kernels OpenBLAS 0.3.30 and 0.3.34 (identical files) select on a Haswell or Zen
//!   CPU (`OPENBLAS_CORETYPE=Haswell`, the convention of `docs/findings/numpy-blas.md`), on the
//!   lengths these matrices give them: `nrm2.S` (x87: the squares summed in extended precision,
//!   the root rounded to 64 bits, then to a double), the scalar tails of `ddot.c`, `daxpy.c`,
//!   `dscal.c`, `dsymv_L.c`, `dgemv_n_4.c` and the generic `ger.c` (no fused multiply-adds on
//!   these lengths), `dgemv_t_4.c`'s tails (compiled with contractions: `fma(a0, x0, a1·x1)`,
//!   then `fma(a2, x2, ·)`, added to `y`; a single row `fma(a, x, y)`), `drot.c` with its
//!   Haswell kernel (`x' = fma(c, x, s·y)`, `y' = fma(c, y, -(s·x))`, strided too), and the interfaces' early
//!   returns (`alpha == 0` in `daxpy`, `dger`, `dgemv`, `dsymv`, `dsyr2`; `alpha == 1` in
//!   `dscal`; `beta == 0` clears `y` to +0 in `dgemv` and `dsymv`);
//! - OpenBLAS's `dgetrf` (`getf2_k`) for the determinant's sign, and numpy's
//!   `det = sign · exp(Σ log|u_ii|)`.
//!
//! Upstream files (BSD-3-Clause; notices below and in NOTICE): OpenBLAS 0.3.34 and 0.3.30
//! release tarballs, `lapack-netlib/SRC/` (`dgesdd.f`, `dgebrd.f`, `dgebd2.f`, `dbdsdc.f`,
//! `dlasdq.f`, `dbdsqr.f`, `dormbr.f`, `dormqr.f`, `dormlq.f`, `dorm2r.f`, `dorml2.f`,
//! `dlarf.f`, `dlarf1f.f`, `dlarfg.f`, `dlapy2.f`, `dlartg.f90`, `dlas2.f`, `dlasv2.f`,
//! `dsyevd.f`, `dsytrd.f`, `dsytd2.f`, `dstedc.f`, `dsteqr.f`, `dlaev2.f`, `dlasr.f`,
//! `dlanst.f`, `dlansy.f`, `dlange.f`, `dlascl.f`, `dormtr.f`, `iladlc.f`, `iladlr.f`,
//! `ilaenv.f`, `INSTALL/dlamch.f`), `interface/` (`nrm2.c`, `scal.c`, `axpy.c`, `dot.c`,
//! `rot.c`, `gemv.c`, `ger.c`, `symv.c`, `syr2.c`), `kernel/x86_64/` (`nrm2.S`, `dscal.c`,
//! `daxpy.c`, `ddot.c`, `drot.c`, `dsymv_L.c`, `dgemv_n_4.c`, `dgemv_t_4.c`, `KERNEL.HASWELL`),
//! `kernel/generic/ger.c`, `lapack/getf2/getf2_k.c`; numpy 2.5.4 `numpy/linalg/umath_linalg.cpp`
//! (BSD-3-Clause). The logic is re-expressed in Rust; no code was copied.
//!
//! LAPACK: Copyright (c) 1992-2023 The University of Tennessee and The University of Tennessee
//! Research Foundation; Copyright (c) 2000-2023 The University of California Berkeley;
//! Copyright (c) 2006-2023 The University of Colorado Denver. All rights reserved.
//! OpenBLAS: Copyright (c) 2011-2014, The OpenBLAS Project. All rights reserved.
//! numpy: Copyright (c) 2005-2025, NumPy Developers. All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the copyright holders nor the names of its contributors may be used
//!    to endorse or promote products derived from this software without specific prior
//!    written permission.
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

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked
// line by line against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::int_plus_one,
    clippy::assign_op_pattern,
    clippy::type_complexity
)]

use crate::openblas;

pub mod lstsq;
pub use lstsq::{Lstsq, LstsqError, lstsq};

/// The LAPACK release whose arithmetic a call follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LapackRelease {
    /// LAPACK 3.12.0 as OpenBLAS 0.3.30 bundles it (numpy 2.3.5's wheels, fMRIPrep 25.2.6's
    /// conda-forge OpenBLAS 0.3.30): reflectors through `DLARF`.
    V3_12_0,
    /// LAPACK 3.12.1 as OpenBLAS 0.3.34 bundles it (numpy 2.5.4's wheels): reflectors through
    /// `DLARF1F`. The default.
    #[default]
    V3_12_1,
}

/// `DLAMCH('E')`: the relative machine precision with rounding, 2^-53.
const EPS: f64 = f64::EPSILON * 0.5;
/// `DLAMCH('S')`: the safe minimum, 2^-1022.
const SAFMIN: f64 = f64::MIN_POSITIVE;

/// Fortran's `SIGN(A, B)`: `|A|` with the sign bit of `B` (gfortran honours -0).
#[inline]
fn sign(a: f64, b: f64) -> f64 {
    a.abs().copysign(b)
}

// ------------------------------------------------------------------------------------------
// x87 extended precision for `nrm2.S`

/// A positive x87 extended value `m · 2^e`, `2^63 <= m < 2^64`.
#[derive(Clone, Copy)]
struct Ext {
    m: u64,
    e: i32,
}

/// `m · 2^e` (`m > 0`, plus a nonzero part below its last bit when `sticky`) rounded to a
/// 64-bit significand, to nearest with ties to even.
fn round64(m: u128, e: i32, sticky: bool) -> Ext {
    let width = 128 - m.leading_zeros() as i32;
    if width <= 64 {
        debug_assert!(!sticky);
        return Ext {
            m: (m << (64 - width)) as u64,
            e: e - (64 - width),
        };
    }
    let cut = width - 64;
    let mut q = m >> cut;
    let rest = m & ((1u128 << cut) - 1);
    let half = 1u128 << (cut - 1);
    if rest > half || (rest == half && (sticky || q & 1 == 1)) {
        q += 1;
    }
    let mut e = e + cut;
    if q >> 64 != 0 {
        q >>= 1;
        e += 1;
    }
    Ext { m: q as u64, e }
}

/// `fmul %st(0), %st`: `x²` rounded to the extended format (`x` finite and nonzero; the
/// extended exponent range holds the square of every double, subnormals included).
fn ext_square(x: f64) -> Ext {
    let bits = x.abs().to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let (m, e) = if biased == 0 {
        (bits & ((1u64 << 52) - 1), -1074)
    } else {
        ((bits & ((1u64 << 52) - 1)) | (1u64 << 52), biased - 1075)
    };
    round64(u128::from(m) * u128::from(m), 2 * e, false)
}

/// `faddp`: `a + b` of two positive extended values, rounded to the extended format.
fn ext_add(a: Ext, b: Ext) -> Ext {
    let (big, small) = if a.e >= b.e { (a, b) } else { (b, a) };
    let diff = (big.e - small.e) as u32;
    let top = u128::from(big.m) << 63;
    let low = u128::from(small.m) << 63;
    let (shifted, sticky) = if diff >= 127 {
        (0, true)
    } else {
        (low >> diff, low & ((1u128 << diff) - 1) != 0)
    };
    round64(top + shifted, big.e - 63, sticky)
}

/// The integer square root of `n` (`floor(√n)`).
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    // From above: the float estimate is within 2^12 of the root for n < 2^128.
    let mut x = (n as f64).sqrt() as u128 + (1 << 13);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// `fsqrt` then `fstpl`: the square root rounded to the extended format, then to a double.
fn ext_sqrt_to_f64(a: Ext) -> f64 {
    // M = m · 2^k with e - k even and 2^126 <= M < 2^128: its integer root has 64 bits.
    let k = if (a.e - 63).rem_euclid(2) == 0 {
        63
    } else {
        64
    };
    let big = u128::from(a.m) << k;
    let s0 = isqrt(big);
    let rem = big - s0 * s0;
    // √M ≥ s0 + ½ exactly when M ≥ s0² + s0 + ¼, i.e. rem > s0 (no ties: √M is never a
    // half-integer).
    let r = s0 + u128::from(rem > s0);
    let exp = (a.e - k) / 2;
    // Then `fstpl`: from the 64-bit value alone (r ≤ 2^64) to a double.
    round_to_f64(r, exp)
}

/// `r · 2^exp` (`r > 0`) rounded to a double, to nearest with ties to even: 53 bits in the
/// normal range, fewer as a subnormal, infinity above the largest double (what an x87 store
/// to a double does in the default rounding mode).
fn round_to_f64(r: u128, exp: i32) -> f64 {
    let width = 128 - r.leading_zeros() as i32;
    let lead = exp + width - 1;
    // Significant bits available at this magnitude (the unit is 2^-1074 for subnormals).
    let keep = if lead >= -1022 {
        53
    } else {
        53 - (-1022 - lead)
    };
    let shift = width - keep;
    let mut unit = exp + shift;
    let mut q = if shift <= 0 {
        r << (-shift)
    } else if shift > width {
        0
    } else {
        let q = if shift >= 128 { 0 } else { r >> shift };
        let rest = if shift >= 128 {
            r
        } else {
            r & ((1u128 << shift) - 1)
        };
        let half = 1u128 << (shift - 1);
        q + u128::from(rest > half || (rest == half && q & 1 == 1))
    };
    if q == 0 {
        return 0.0;
    }
    if q >> 53 != 0 {
        // The carry of a rounding up to 2^53.
        q >>= 1;
        unit += 1;
    }
    let qw = 128 - q.leading_zeros() as i32;
    let top = unit + qw - 1;
    if top > 1023 {
        f64::INFINITY
    } else if top >= -1022 {
        let mant = ((q << (53 - qw)) as u64) & ((1u64 << 52) - 1);
        f64::from_bits((((top + 1023) as u64) << 52) | mant)
    } else {
        debug_assert_eq!(unit, -1074);
        f64::from_bits(q as u64)
    }
}

// ------------------------------------------------------------------------------------------
// BLAS: OpenBLAS's interfaces and Haswell kernels on these lengths

/// `DNRM2(N, X, INCX)` (`interface/nrm2.c`, `kernel/x86_64/nrm2.S`): `|x0|` for one element;
/// else the squares summed into four x87 accumulators cleared to +0 (the kernel's main loop,
/// unit and other strides alike: element `k` of the first `8·⌊n/8⌋` goes to accumulator
/// `k mod 4`; the last `n mod 8` elements all go to accumulator 0, in order), combined as
/// `acc3 + ((acc2 + acc0) + acc1)`, the root in extended precision, stored as a double (for
/// `n < 8` this is the plain sum in order). The extended exponent range holds every square and
/// sum, so only the final store can underflow or overflow.
fn dnrm2(n: usize, x: &[f64], off: usize, inc: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return x[off].abs();
    }
    let vals: Vec<f64> = (0..n).map(|i| x[off + i * inc]).collect();
    if vals.iter().any(|v| !v.is_finite()) {
        // An infinity gives +inf, a NaN a NaN (x87 and the double formula agree).
        return vals.iter().map(|v| v * v).sum::<f64>().sqrt();
    }
    // Adding +0 to an extended value is exact, so a cleared accumulator is `None`.
    let add = |a: Option<Ext>, b: Option<Ext>| match (a, b) {
        (None, x) | (x, None) => x,
        (Some(a), Some(b)) => Some(ext_add(a, b)),
    };
    let mut acc: [Option<Ext>; 4] = [None; 4];
    let main = n & !7;
    for (k, &v) in vals.iter().enumerate() {
        if v == 0.0 {
            continue;
        }
        let lane = if k < main { k % 4 } else { 0 };
        acc[lane] = add(acc[lane], Some(ext_square(v)));
    }
    // .L998: `faddp %st, %st(2)`, then twice `faddp %st, %st(1)`.
    let total = add(acc[3], add(add(acc[2], acc[0]), acc[1]));
    match total {
        None => 0.0,
        Some(a) => ext_sqrt_to_f64(a),
    }
}

/// `DSCAL(N, A, X, INCX)` from Fortran (`interface/scal.c`: nothing for `a == 1`; the kernel's
/// `x *= a`, a zero `a` included).
fn dscal(n: usize, a: f64, x: &mut [f64], off: usize, inc: usize) {
    if n == 0 || a == 1.0 {
        return;
    }
    for i in 0..n {
        x[off + i * inc] *= a;
    }
}

/// `AXPYU_K`: `y += a·x`, each product and sum rounded (`daxpy.c`, no contraction), with no
/// test of `a`.
fn axpy_k(n: usize, a: f64, x: &[f64], y: &mut [f64], yoff: usize, yinc: usize) {
    for (i, &xi) in x.iter().take(n).enumerate() {
        let p = a * xi;
        y[yoff + i * yinc] += p;
    }
}

/// `DAXPY` from Fortran: nothing for `a == 0` (`interface/axpy.c`), then [`axpy_k`].
fn daxpy(n: usize, a: f64, x: &[f64], y: &mut [f64], yoff: usize, yinc: usize) {
    if n == 0 || a == 0.0 {
        return;
    }
    axpy_k(n, a, x, y, yoff, yinc);
}

/// `DDOT` with unit strides, fewer than 16 elements (`ddot.c`'s tail): from +0, in order.
fn ddot(x: &[f64], y: &[f64]) -> f64 {
    let mut dot = 0.0f64;
    for (xi, yi) in x.iter().zip(y) {
        dot += yi * xi;
    }
    dot
}

/// `DROT` (`drot.c` with `drot_microk_haswell-2.c`, compiled with FMA, its masked tail and its
/// strided loop alike: `x' = fma(c, x, s·y)`, `y' = fma(c, y, -(s·x))`).
fn drot(
    n: usize,
    a: &mut [f64],
    xoff: usize,
    xinc: usize,
    yoff: usize,
    yinc: usize,
    c: f64,
    s: f64,
) {
    for i in 0..n {
        let (xi, yi) = (xoff + i * xinc, yoff + i * yinc);
        let (x, y) = (a[xi], a[yi]);
        a[xi] = c.mul_add(x, s * y);
        a[yi] = c.mul_add(y, -(s * x));
    }
}

/// `DSWAP` of two vectors in one array.
fn dswap(n: usize, a: &mut [f64], xoff: usize, xinc: usize, yoff: usize, yinc: usize) {
    for i in 0..n {
        a.swap(xoff + i * xinc, yoff + i * yinc);
    }
}

/// `DGEMV('T', M, N, ALPHA, A, LDA, X, 1, 0, Y, 1)` for `M < 4` (`gemv.c`: `y` cleared to
/// +0, nothing more for `alpha == 0`; `dgemv_t_4.c`'s tails as GCC contracted them). `x` holds
/// the `M` values of X; returns Y (`N` values).
fn dgemv_t(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
) -> Vec<f64> {
    let mut y = vec![0.0f64; n];
    if m == 0 || n == 0 || alpha == 0.0 {
        return y;
    }
    assert!(m < 4, "dgemv_t is ported for fewer than 4 rows");
    let xt: Vec<f64> = x.iter().take(m).map(|v| v * alpha).collect();
    for (j, yj) in y.iter_mut().enumerate() {
        let col = aoff + j * lda;
        *yj = match m {
            1 => a[col].mul_add(xt[0], *yj),
            2 => {
                let t = a[col].mul_add(xt[0], a[col + 1] * xt[1]);
                t + *yj
            }
            _ => {
                let t = a[col].mul_add(xt[0], a[col + 1] * xt[1]);
                let t = a[col + 2].mul_add(xt[2], t);
                t + *yj
            }
        };
    }
    y
}

/// `DGEMV('N', M, N, ALPHA, A, LDA, X, INCX, 0, Y, 1)` for `M < 4` (`dgemv_n_4.c`'s tails:
/// `temp += a·x` from +0 over the columns, then `y += alpha·temp`, `y` cleared to +0).
fn dgemv_n(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
) -> Vec<f64> {
    let mut y = vec![0.0f64; m];
    if m == 0 || n == 0 || alpha == 0.0 {
        return y;
    }
    assert!(m < 4, "dgemv_n is ported for fewer than 4 rows");
    for (r, yr) in y.iter_mut().enumerate() {
        let mut temp = 0.0f64;
        for (j, xj) in x.iter().take(n).enumerate() {
            temp += a[aoff + r + j * lda] * xj;
        }
        *yr += alpha * temp;
    }
    y
}

/// `DGER(M, N, ALPHA, X, ., Y, ., A, LDA)` (`ger.c`: nothing for `alpha == 0`; per column
/// `AXPYU_K(m, alpha·y[j], x, a[:, j])`).
fn dger(
    m: usize,
    n: usize,
    alpha: f64,
    x: &[f64],
    y: &[f64],
    a: &mut [f64],
    aoff: usize,
    lda: usize,
) {
    if m == 0 || n == 0 || alpha == 0.0 {
        return;
    }
    for (j, yj) in y.iter().take(n).enumerate() {
        axpy_k(m, alpha * yj, x, a, aoff + j * lda, 1);
    }
}

/// `DSYMV('L', N, ALPHA, A, LDA, X, 1, 0, Y, 1)` for `N < 4` (`symv.c`: `y` cleared to +0;
/// `dsymv_L.c` below its blocked sizes).
fn dsymv_lower(n: usize, alpha: f64, a: &[f64], aoff: usize, lda: usize, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0f64; n];
    if n == 0 || alpha == 0.0 {
        return y;
    }
    assert!(n < 4, "dsymv is ported for fewer than 4 rows");
    for j in 0..n {
        let temp1 = alpha * x[j];
        let mut temp2 = 0.0f64;
        y[j] += temp1 * a[aoff + j * lda + j];
        for i in j + 1..n {
            y[i] += temp1 * a[aoff + j * lda + i];
            temp2 += a[aoff + j * lda + i] * x[i];
        }
        y[j] += alpha * temp2;
    }
    y
}

/// `DSYR2('L', N, ALPHA, X, 1, Y, 1, A, LDA)` for `N < 100` (`syr2.c`'s unit-stride path:
/// per column `i`, `AXPYU_K(n - i, alpha·x[i], y[i..], a[i.., i])` then the same with `x`
/// and `y` exchanged).
fn dsyr2_lower(n: usize, alpha: f64, x: &[f64], y: &[f64], a: &mut [f64], aoff: usize, lda: usize) {
    if n == 0 || alpha == 0.0 {
        return;
    }
    for i in 0..n {
        let col = aoff + i * lda + i;
        axpy_k(n - i, alpha * x[i], &y[i..], a, col, 1);
        axpy_k(n - i, alpha * y[i], &x[i..], a, col, 1);
    }
}

// ------------------------------------------------------------------------------------------
// LAPACK auxiliaries

/// `DLAPY2(X, Y)`: `√(x² + y²)` avoiding overflow.
fn dlapy2(x: f64, y: f64) -> f64 {
    let xnan = x.is_nan();
    let ynan = y.is_nan();
    let mut r = 0.0;
    if xnan {
        r = x;
    }
    if ynan {
        r = y;
    }
    if !(xnan || ynan) {
        let xa = x.abs();
        let ya = y.abs();
        let w = xa.max(ya);
        let z = xa.min(ya);
        if z == 0.0 || w > f64::MAX {
            r = w;
        } else {
            let q = z / w;
            r = w * (1.0 + q * q).sqrt();
        }
    }
    r
}

/// `DLARFG(N, ALPHA, X, INCX, TAU)`: the elementary reflector; `x` (N - 1 values at `xoff`,
/// stride `inc` in `a`) is overwritten with `v(2:n)`, `alpha` with `beta`. Returns `tau`.
fn dlarfg(n: usize, alpha: &mut f64, a: &mut [f64], xoff: usize, inc: usize) -> f64 {
    if n <= 1 {
        return 0.0;
    }
    let mut xnorm = dnrm2(n - 1, a, xoff, inc);
    if xnorm == 0.0 {
        return 0.0;
    }
    let mut beta = -sign(dlapy2(*alpha, xnorm), *alpha);
    let safmin = SAFMIN / EPS;
    let mut knt = 0;
    if beta.abs() < safmin {
        let rsafmn = 1.0 / safmin;
        loop {
            knt += 1;
            dscal(n - 1, rsafmn, a, xoff, inc);
            beta *= rsafmn;
            *alpha *= rsafmn;
            if !(beta.abs() < safmin && knt < 20) {
                break;
            }
        }
        xnorm = dnrm2(n - 1, a, xoff, inc);
        beta = -sign(dlapy2(*alpha, xnorm), *alpha);
    }
    let tau = (beta - *alpha) / beta;
    dscal(n - 1, 1.0 / (*alpha - beta), a, xoff, inc);
    for _ in 0..knt {
        beta *= safmin;
    }
    *alpha = beta;
    tau
}

/// `DLARTG(F, G, C, S, R)` (LAPACK 3.10+'s `dlartg.f90`): `(c, s, r)`.
fn dlartg(f: f64, g: f64) -> (f64, f64, f64) {
    let safmin = SAFMIN;
    let safmax = 1.0 / safmin;
    let rtmin = safmin.sqrt();
    let rtmax = (safmax / 2.0).sqrt();
    let f1 = f.abs();
    let g1 = g.abs();
    if g == 0.0 {
        (1.0, 0.0, f)
    } else if f == 0.0 {
        (0.0, sign(1.0, g), g1)
    } else if f1 > rtmin && f1 < rtmax && g1 > rtmin && g1 < rtmax {
        let d = (f * f + g * g).sqrt();
        let c = f1 / d;
        let r = sign(d, f);
        (c, g / r, r)
    } else {
        let u = safmax.min(safmin.max(f1).max(g1));
        let fs = f / u;
        let gs = g / u;
        let d = (fs * fs + gs * gs).sqrt();
        let c = fs.abs() / d;
        let r = sign(d, f);
        let s = gs / r;
        (c, s, r * u)
    }
}

/// `DLAS2(F, G, H, SSMIN, SSMAX)`: the singular values of `[[f, g], [0, h]]`.
fn dlas2(f: f64, g: f64, h: f64) -> (f64, f64) {
    let fa = f.abs();
    let ga = g.abs();
    let ha = h.abs();
    let fhmn = fa.min(ha);
    let fhmx = fa.max(ha);
    if fhmn == 0.0 {
        let ssmax = if fhmx == 0.0 {
            ga
        } else {
            let q = fhmx.min(ga) / fhmx.max(ga);
            fhmx.max(ga) * (1.0 + q * q).sqrt()
        };
        (0.0, ssmax)
    } else if ga < fhmx {
        let as_ = 1.0 + fhmn / fhmx;
        let at = (fhmx - fhmn) / fhmx;
        let q = ga / fhmx;
        let au = q * q;
        let c = 2.0 / ((as_ * as_ + au).sqrt() + (at * at + au).sqrt());
        (fhmn * c, fhmx / c)
    } else {
        let au = fhmx / ga;
        if au == 0.0 {
            ((fhmn * fhmx) / ga, ga)
        } else {
            let as_ = 1.0 + fhmn / fhmx;
            let at = (fhmx - fhmn) / fhmx;
            let p = as_ * au;
            let q = at * au;
            let c = 1.0 / ((1.0 + p * p).sqrt() + (1.0 + q * q).sqrt());
            let ssmin = (fhmn * c) * au;
            (ssmin + ssmin, ga / (c + c))
        }
    }
}

/// `DLASV2(F, G, H, SSMIN, SSMAX, SNR, CSR, SNL, CSL)`: the SVD of `[[f, g], [0, h]]`.
fn dlasv2(f: f64, g: f64, h: f64) -> (f64, f64, f64, f64, f64, f64) {
    let mut ft = f;
    let mut fa = ft.abs();
    let mut ht = h;
    let mut ha = h.abs();
    let mut pmax = 1;
    let swap = ha > fa;
    if swap {
        pmax = 3;
        std::mem::swap(&mut ft, &mut ht);
        std::mem::swap(&mut fa, &mut ha);
    }
    let gt = g;
    let ga = gt.abs();
    let (mut ssmin, mut ssmax, clt, crt, slt, srt);
    if ga == 0.0 {
        ssmin = ha;
        ssmax = fa;
        clt = 1.0;
        crt = 1.0;
        slt = 0.0;
        srt = 0.0;
    } else {
        let mut gasmal = true;
        let mut out = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        if ga > fa {
            pmax = 2;
            if (fa / ga) < EPS {
                gasmal = false;
                let smax = ga;
                let smin = if ha > 1.0 {
                    fa / (ga / ha)
                } else {
                    (fa / ga) * ha
                };
                out = (smin, smax, 1.0, ft / gt, ht / gt, 1.0);
            }
        }
        if gasmal {
            let d = fa - ha;
            let mut l = if d == fa { 1.0 } else { d / fa };
            let m = gt / ft;
            let mut t = 2.0 - l;
            let mm = m * m;
            let tt = t * t;
            let s = (tt + mm).sqrt();
            let r = if l == 0.0 {
                m.abs()
            } else {
                (l * l + mm).sqrt()
            };
            let a = 0.5 * (s + r);
            let smin = ha / a;
            let smax = fa * a;
            if mm == 0.0 {
                if l == 0.0 {
                    t = sign(2.0, ft) * sign(1.0, gt);
                } else {
                    t = gt / sign(d, ft) + m / t;
                }
            } else {
                t = (m / (s + t) + m / (r + l)) * (1.0 + a);
            }
            l = (t * t + 4.0).sqrt();
            let crt_ = 2.0 / l;
            let srt_ = t / l;
            let clt_ = (crt_ + srt_ * m) / a;
            let slt_ = ((ht / ft) * srt_) / a;
            out = (smin, smax, clt_, crt_, slt_, srt_);
        }
        ssmin = out.0;
        ssmax = out.1;
        clt = out.2;
        crt = out.3;
        slt = out.4;
        srt = out.5;
    }
    let (csl, snl, csr, snr) = if swap {
        (srt, crt, slt, clt)
    } else {
        (clt, slt, crt, srt)
    };
    let tsign = match pmax {
        1 => sign(1.0, csr) * sign(1.0, csl) * sign(1.0, f),
        2 => sign(1.0, snr) * sign(1.0, csl) * sign(1.0, g),
        _ => sign(1.0, snr) * sign(1.0, snl) * sign(1.0, h),
    };
    ssmax = sign(ssmax, tsign);
    ssmin = sign(ssmin, tsign * sign(1.0, f) * sign(1.0, h));
    (ssmin, ssmax, snr, csr, snl, csl)
}

/// `DLAEV2(A, B, C, RT1, RT2, CS1, SN1)`: the eigen decomposition of `[[a, b], [b, c]]`.
fn dlaev2(a: f64, b: f64, c: f64) -> (f64, f64, f64, f64) {
    let sm = a + c;
    let df = a - c;
    let adf = df.abs();
    let tb = b + b;
    let ab = tb.abs();
    let (acmx, acmn) = if a.abs() > c.abs() { (a, c) } else { (c, a) };
    let rt = if adf > ab {
        let q = ab / adf;
        adf * (1.0 + q * q).sqrt()
    } else if adf < ab {
        let q = adf / ab;
        ab * (1.0 + q * q).sqrt()
    } else {
        ab * 2f64.sqrt()
    };
    let (rt1, rt2, sgn1);
    if sm < 0.0 {
        rt1 = 0.5 * (sm - rt);
        sgn1 = -1;
        rt2 = (acmx / rt1) * acmn - (b / rt1) * b;
    } else if sm > 0.0 {
        rt1 = 0.5 * (sm + rt);
        sgn1 = 1;
        rt2 = (acmx / rt1) * acmn - (b / rt1) * b;
    } else {
        rt1 = 0.5 * rt;
        rt2 = -0.5 * rt;
        sgn1 = 1;
    }
    let (cs, sgn2) = if df >= 0.0 {
        (df + rt, 1)
    } else {
        (df - rt, -1)
    };
    let acs = cs.abs();
    let (mut cs1, mut sn1);
    if acs > ab {
        let ct = -tb / cs;
        sn1 = 1.0 / (1.0 + ct * ct).sqrt();
        cs1 = ct * sn1;
    } else if ab == 0.0 {
        cs1 = 1.0;
        sn1 = 0.0;
    } else {
        let tn = -cs / tb;
        cs1 = 1.0 / (1.0 + tn * tn).sqrt();
        sn1 = tn * cs1;
    }
    if sgn1 == sgn2 {
        let tn = cs1;
        cs1 = -sn1;
        sn1 = tn;
    }
    (rt1, rt2, cs1, sn1)
}

/// `DLASR(SIDE, 'V', DIRECT, M, N, C, S, A, LDA)` on the `m × n` block of `a` at `aoff`.
fn dlasr(
    left: bool,
    forward: bool,
    m: usize,
    n: usize,
    c: &[f64],
    s: &[f64],
    a: &mut [f64],
    aoff: usize,
    lda: usize,
) {
    let at = |i: usize, j: usize| aoff + (i - 1) + (j - 1) * lda;
    if m == 0 || n == 0 {
        return;
    }
    if left {
        let js: Vec<usize> = if forward {
            (1..m).collect()
        } else {
            (1..m).rev().collect()
        };
        for j in js {
            let (ct, st) = (c[j - 1], s[j - 1]);
            if ct != 1.0 || st != 0.0 {
                for i in 1..=n {
                    let temp = a[at(j + 1, i)];
                    a[at(j + 1, i)] = ct * temp - st * a[at(j, i)];
                    a[at(j, i)] = st * temp + ct * a[at(j, i)];
                }
            }
        }
    } else {
        let js: Vec<usize> = if forward {
            (1..n).collect()
        } else {
            (1..n).rev().collect()
        };
        for j in js {
            let (ct, st) = (c[j - 1], s[j - 1]);
            if ct != 1.0 || st != 0.0 {
                for i in 1..=m {
                    let temp = a[at(i, j + 1)];
                    a[at(i, j + 1)] = ct * temp - st * a[at(i, j)];
                    a[at(i, j)] = st * temp + ct * a[at(i, j)];
                }
            }
        }
    }
}

/// `DLANST('M', N, D, E)`: the largest magnitude (a NaN wins).
fn dlanst_max(d: &[f64], e: &[f64]) -> f64 {
    let n = d.len();
    if n == 0 {
        return 0.0;
    }
    let mut anorm = d[n - 1].abs();
    for i in 0..n - 1 {
        let s = d[i].abs();
        if anorm < s || s.is_nan() {
            anorm = s;
        }
        let s = e[i].abs();
        if anorm < s || s.is_nan() {
            anorm = s;
        }
    }
    anorm
}

/// `DLASCL(TYPE, 0, 0, CFROM, CTO, ...)`: `values *= cto / cfrom` in safe steps; `values` are
/// the elements the type selects.
fn dlascl(cfrom: f64, cto: f64, values: &mut [&mut f64]) {
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let mut cfromc = cfrom;
    let mut ctoc = cto;
    loop {
        let cfrom1 = cfromc * smlnum;
        let (mul, done);
        if cfrom1 == cfromc {
            mul = ctoc / cfromc;
            done = true;
        } else {
            let cto1 = ctoc / bignum;
            if cto1 == ctoc {
                mul = ctoc;
                done = true;
                cfromc = 1.0;
            } else if cfrom1.abs() > ctoc.abs() && ctoc != 0.0 {
                mul = smlnum;
                done = false;
                cfromc = cfrom1;
            } else if cto1.abs() > cfromc.abs() {
                mul = bignum;
                done = false;
                ctoc = cto1;
            } else {
                mul = ctoc / cfromc;
                done = true;
                if mul == 1.0 {
                    return;
                }
            }
        }
        for v in values.iter_mut() {
            **v *= mul;
        }
        if done {
            return;
        }
    }
}

/// `ILADLC(M, N, A, LDA)`: the last nonzero column (1-based; 0 if none).
fn iladlc(m: usize, n: usize, a: &[f64], aoff: usize, lda: usize) -> usize {
    let at = |i: usize, j: usize| a[aoff + (i - 1) + (j - 1) * lda];
    if n == 0 {
        return 0;
    }
    if at(1, n) != 0.0 || at(m, n) != 0.0 {
        return n;
    }
    for j in (1..=n).rev() {
        for i in 1..=m {
            if at(i, j) != 0.0 {
                return j;
            }
        }
    }
    0
}

/// `ILADLR(M, N, A, LDA)`: the last nonzero row (1-based; 0 if none).
fn iladlr(m: usize, n: usize, a: &[f64], aoff: usize, lda: usize) -> usize {
    let at = |i: usize, j: usize| a[aoff + (i - 1) + (j - 1) * lda];
    if m == 0 {
        return 0;
    }
    if at(m, 1) != 0.0 || at(m, n) != 0.0 {
        return m;
    }
    let mut result = 0;
    for j in 1..=n {
        let mut i = m;
        while at(i.max(1), j) == 0.0 && i >= 1 {
            i -= 1;
            if i == 0 {
                break;
            }
        }
        result = result.max(i);
    }
    result
}

/// A Householder reflector `H = I - tau·v·vᵀ` applied to the `m × n` block of `c` at `coff`
/// (`ldc`) from the left or the right; `v` holds the whole vector (its leading 1 included),
/// `incv` its stride in the caller's array (only the zero scan depends on the values).
///
/// [`LapackRelease::V3_12_1`]: `DLARF1F` (the leading 1 implicit: `w = C(2:,:)ᵀ v(2:)`, then
/// `w += C(1,:)`, `C(1,:) -= tau·w`, `C(2:,:) -= tau·v(2:)·wᵀ`); [`LapackRelease::V3_12_0`]:
/// `DLARF` (`w = C ᵀ v`, `C -= tau·v·wᵀ`, the 1 stored).
fn apply_reflector(
    release: LapackRelease,
    left: bool,
    m: usize,
    n: usize,
    v: &[f64],
    tau: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    let at = |i: usize, j: usize| coff + (i - 1) + (j - 1) * ldc;
    let len = if left { m } else { n };
    match release {
        LapackRelease::V3_12_0 => {
            // DLARF: lastv scans down to 0 (v(1) = 1 is stored).
            let mut lastv = 0;
            let mut lastc = 0;
            if tau != 0.0 {
                lastv = len;
                while lastv > 0 && v[lastv - 1] == 0.0 {
                    lastv -= 1;
                }
                lastc = if left {
                    iladlc(lastv, n, c, coff, ldc)
                } else {
                    iladlr(m, lastv, c, coff, ldc)
                };
            }
            if lastv == 0 {
                return;
            }
            if left {
                // w(1:lastc) = C(1:lastv, 1:lastc)ᵀ v(1:lastv); C -= tau·v·wᵀ.
                if lastc == 0 {
                    return;
                }
                let w = dgemv_t_any(lastv, lastc, 1.0, c, coff, ldc, &v[..lastv]);
                dger(lastv, lastc, -tau, &v[..lastv], &w, c, coff, ldc);
            } else {
                if lastc == 0 {
                    return;
                }
                let w = dgemv_n(lastc, lastv, 1.0, c, coff, ldc, &v[..lastv]);
                dger(lastc, lastv, -tau, &w, &v[..lastv], c, coff, ldc);
            }
        }
        LapackRelease::V3_12_1 => {
            // DLARF1F: v(1) = 1 implicit, lastv scans down to 1.
            let mut lastv = 1;
            let mut lastc = 0;
            if tau != 0.0 {
                lastv = len;
                while lastv > 1 && v[lastv - 1] == 0.0 {
                    lastv -= 1;
                }
                lastc = if left {
                    iladlc(lastv, n, c, coff, ldc)
                } else {
                    iladlr(m, lastv, c, coff, ldc)
                };
            }
            if lastc == 0 {
                return;
            }
            if left {
                if lastv == 1 {
                    // C(1, 1:lastc) *= 1 - tau.
                    dscal(lastc, 1.0 - tau, c, at(1, 1), ldc);
                } else {
                    let mut w = dgemv_t(lastv - 1, lastc, 1.0, c, at(2, 1), ldc, &v[1..lastv]);
                    let row1: Vec<f64> = (1..=lastc).map(|j| c[at(1, j)]).collect();
                    daxpy(lastc, 1.0, &row1, &mut w, 0, 1);
                    daxpy(lastc, -tau, &w, c, at(1, 1), ldc);
                    dger(lastv - 1, lastc, -tau, &v[1..lastv], &w, c, at(2, 1), ldc);
                }
            } else if lastv == 1 {
                // C(1:lastc, 1) *= 1 - tau.
                dscal(lastc, 1.0 - tau, c, at(1, 1), 1);
            } else {
                let mut w = dgemv_n(lastc, lastv - 1, 1.0, c, at(1, 2), ldc, &v[1..lastv]);
                let col1: Vec<f64> = (1..=lastc).map(|i| c[at(i, 1)]).collect();
                daxpy(lastc, 1.0, &col1, &mut w, 0, 1);
                daxpy(lastc, -tau, &w, c, at(1, 1), 1);
                dger(lastc, lastv - 1, -tau, &w, &v[1..lastv], c, at(1, 2), ldc);
            }
        }
    }
}

/// [`dgemv_t`] for any number of rows up to 4 (3.12.0's `DLARF` multiplies all `lastv` rows,
/// up to 4 for a 4 × 4 matrix only through `dormtr`, whose rows are at most 3).
fn dgemv_t_any(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    aoff: usize,
    lda: usize,
    x: &[f64],
) -> Vec<f64> {
    dgemv_t(m, n, alpha, a, aoff, lda, x)
}

// ------------------------------------------------------------------------------------------
// dsyevd

/// `DSYTD2('L', N, A, LDA, D, E, TAU)`: `a` column-major `n × n` (lower triangle), reduced in
/// place; returns `(d, e, tau)`.
fn dsytd2_lower(n: usize, a: &mut [f64], lda: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let at = |i: usize, j: usize| (i - 1) + (j - 1) * lda;
    let mut d = vec![0.0; n];
    let mut e = vec![0.0; n.saturating_sub(1)];
    let mut tau = vec![0.0; n.saturating_sub(1)];
    if n == 0 {
        return (d, e, tau);
    }
    for i in 1..n {
        let mut alpha = a[at(i + 1, i)];
        let xoff = at((i + 2).min(n), i);
        let taui = dlarfg(n - i, &mut alpha, a, xoff, 1);
        a[at(i + 1, i)] = alpha;
        e[i - 1] = a[at(i + 1, i)];
        if taui != 0.0 {
            a[at(i + 1, i)] = 1.0;
            let v: Vec<f64> = (i + 1..=n).map(|r| a[at(r, i)]).collect();
            let mut t = dsymv_lower(n - i, taui, a, at(i + 1, i + 1), lda, &v);
            let alpha2 = -0.5 * taui * ddot(&t, &v);
            daxpy(n - i, alpha2, &v, &mut t, 0, 1);
            dsyr2_lower(n - i, -1.0, &v, &t, a, at(i + 1, i + 1), lda);
            a[at(i + 1, i)] = e[i - 1];
            // TAU(i:n-1) held the temporary vector; only TAU(i) is kept below.
        }
        d[i - 1] = a[at(i, i)];
        tau[i - 1] = taui;
    }
    d[n - 1] = a[at(n, n)];
    (d, e, tau)
}

/// `DSTEQR('I', N, D, E, Z, LDZ, WORK, INFO)`: the eigenvalues (ascending, in `d`) and
/// eigenvectors (`z`, column-major `n × n`) of the symmetric tridiagonal matrix. Returns the
/// number of off-diagonal elements that did not converge (LAPACK's `info`).
fn dsteqr_i(n: usize, d: &mut [f64], e: &mut [f64], z: &mut [f64]) -> usize {
    const MAXIT: usize = 30;
    let zat = |i: usize, j: usize| (i - 1) + (j - 1) * n;
    if n == 0 {
        return 0;
    }
    for v in z.iter_mut() {
        *v = 0.0;
    }
    if n == 1 {
        z[0] = 1.0;
        return 0;
    }
    let eps = EPS;
    let eps2 = eps * eps;
    let safmin = SAFMIN;
    let safmax = 1.0 / safmin;
    let ssfmax = safmax.sqrt() / 3.0;
    let ssfmin = safmin.sqrt() / eps2;
    for i in 1..=n {
        z[zat(i, i)] = 1.0;
    }
    let nmaxit = n * MAXIT;
    let mut jtot = 0;
    let mut l1 = 1;
    let nm1 = n - 1;
    let mut work = vec![0.0f64; 2 * (n - 1)];
    // D(k), E(k) 1-based.
    macro_rules! dd {
        ($k:expr) => {
            d[$k - 1]
        };
    }
    macro_rules! ee {
        ($k:expr) => {
            e[$k - 1]
        };
    }
    'outer: loop {
        // label 10
        if l1 > n {
            break 'outer;
        }
        if l1 > 1 {
            ee!(l1 - 1) = 0.0;
        }
        let mut m = n;
        if l1 <= nm1 {
            let mut found = false;
            for mm in l1..=nm1 {
                let tst = ee!(mm).abs();
                if tst == 0.0 {
                    m = mm;
                    found = true;
                    break;
                }
                if tst <= (dd!(mm).abs().sqrt() * dd!(mm + 1).abs().sqrt()) * eps {
                    ee!(mm) = 0.0;
                    m = mm;
                    found = true;
                    break;
                }
            }
            if !found {
                m = n;
            }
        }
        // label 30
        let mut l = l1;
        let lsv = l;
        let mut lend = m;
        let lendsv = lend;
        l1 = m + 1;
        if lend == l {
            continue 'outer;
        }
        // Scale submatrix in rows and columns L to LEND.
        let anorm = dlanst_max(&d[l - 1..lend], &e[l - 1..lend - 1]);
        let mut iscale = 0;
        if anorm == 0.0 {
            continue 'outer;
        }
        if anorm > ssfmax {
            iscale = 1;
            scale_block(d, e, l, lend, anorm, ssfmax);
        } else if anorm < ssfmin {
            iscale = 2;
            scale_block(d, e, l, lend, anorm, ssfmin);
        }
        // Choose between QL and QR iteration.
        if dd!(lend).abs() < dd!(l).abs() {
            lend = lsv;
            l = lendsv;
        }
        if lend > l {
            // QL iteration. Look for small subdiagonal element.
            'ql: loop {
                // label 40
                let mut mm = lend;
                if l != lend {
                    let lendm1 = lend - 1;
                    for k in l..=lendm1 {
                        let tst = ee!(k).abs() * ee!(k).abs();
                        if tst <= (eps2 * dd!(k).abs()) * dd!(k + 1).abs() + safmin {
                            mm = k;
                            break;
                        }
                    }
                }
                let m = mm;
                if m < lend {
                    ee!(m) = 0.0;
                }
                let mut p = dd!(l);
                if m == l {
                    // label 80: eigenvalue found.
                    dd!(l) = p;
                    l += 1;
                    if l <= lend {
                        continue 'ql;
                    }
                    break 'ql;
                }
                // If remaining matrix is 2-by-2, use DLAE2 or SLAEV2 to compute its
                // eigensystem.
                if m == l + 1 {
                    let (rt1, rt2, c, s) = dlaev2(dd!(l), ee!(l), dd!(l + 1));
                    work[l - 1] = c;
                    work[n - 1 + l - 1] = s;
                    let (cs, sn) = (vec![work[l - 1]], vec![work[n - 1 + l - 1]]);
                    dlasr(false, false, n, 2, &cs, &sn, z, zat(1, l), n);
                    dd!(l) = rt1;
                    dd!(l + 1) = rt2;
                    ee!(l) = 0.0;
                    l += 2;
                    if l <= lend {
                        continue 'ql;
                    }
                    break 'ql;
                }
                if jtot == nmaxit {
                    break 'ql;
                }
                jtot += 1;
                // Form shift.
                let mut g = (dd!(l + 1) - p) / (2.0 * ee!(l));
                let mut r = dlapy2(g, 1.0);
                g = dd!(m) - p + (ee!(l) / (g + sign(r, g)));
                let mut s = 1.0;
                let mut c = 1.0;
                p = 0.0;
                // Inner loop.
                let mm1 = m - 1;
                for i in (l..=mm1).rev() {
                    let f = s * ee!(i);
                    let b = c * ee!(i);
                    let (cc, ss, rr) = dlartg(g, f);
                    c = cc;
                    s = ss;
                    r = rr;
                    if i != m - 1 {
                        ee!(i + 1) = r;
                    }
                    g = dd!(i + 1) - p;
                    r = (dd!(i) - g) * s + 2.0 * c * b;
                    p = s * r;
                    dd!(i + 1) = g + p;
                    g = c * r - b;
                    // If eigenvectors are desired, then save rotations.
                    work[i - 1] = c;
                    work[n - 1 + i - 1] = -s;
                }
                // If eigenvectors are desired, then apply saved rotations.
                let mmn = m - l + 1;
                let cs: Vec<f64> = work[l - 1..l - 1 + mmn - 1].to_vec();
                let sn: Vec<f64> = work[n - 1 + l - 1..n - 1 + l - 1 + mmn - 1].to_vec();
                dlasr(false, false, n, mmn, &cs, &sn, z, zat(1, l), n);
                dd!(l) -= p;
                ee!(l) = g;
            }
        } else {
            // QR iteration. Look for small superdiagonal element.
            'qr: loop {
                // label 90
                let mut mm = lend;
                if l != lend {
                    let lendp1 = lend + 1;
                    let mut k = l;
                    while k >= lendp1 {
                        let tst = ee!(k - 1).abs() * ee!(k - 1).abs();
                        if tst <= (eps2 * dd!(k).abs()) * dd!(k - 1).abs() + safmin {
                            mm = k;
                            break;
                        }
                        k -= 1;
                    }
                }
                let m = mm;
                if m > lend {
                    ee!(m - 1) = 0.0;
                }
                let mut p = dd!(l);
                if m == l {
                    // label 130: eigenvalue found.
                    dd!(l) = p;
                    l -= 1;
                    if l >= lend {
                        continue 'qr;
                    }
                    break 'qr;
                }
                if m + 1 == l {
                    let (rt1, rt2, c, s) = dlaev2(dd!(l - 1), ee!(l - 1), dd!(l));
                    work[m - 1] = c;
                    work[n - 1 + m - 1] = s;
                    let (cs, sn) = (vec![work[m - 1]], vec![work[n - 1 + m - 1]]);
                    dlasr(false, true, n, 2, &cs, &sn, z, zat(1, l - 1), n);
                    dd!(l - 1) = rt1;
                    dd!(l) = rt2;
                    ee!(l - 1) = 0.0;
                    if l < 2 + lend {
                        break 'qr;
                    }
                    l -= 2;
                    if l >= lend {
                        continue 'qr;
                    }
                    break 'qr;
                }
                if jtot == nmaxit {
                    break 'qr;
                }
                jtot += 1;
                // Form shift.
                let mut g = (dd!(l - 1) - p) / (2.0 * ee!(l - 1));
                let mut r = dlapy2(g, 1.0);
                g = dd!(m) - p + (ee!(l - 1) / (g + sign(r, g)));
                let mut s = 1.0;
                let mut c = 1.0;
                p = 0.0;
                let lm1 = l - 1;
                for i in m..=lm1 {
                    let f = s * ee!(i);
                    let b = c * ee!(i);
                    let (cc, ss, rr) = dlartg(g, f);
                    c = cc;
                    s = ss;
                    r = rr;
                    if i != m {
                        ee!(i - 1) = r;
                    }
                    g = dd!(i) - p;
                    r = (dd!(i + 1) - g) * s + 2.0 * c * b;
                    p = s * r;
                    dd!(i) = g + p;
                    g = c * r - b;
                    work[i - 1] = c;
                    work[n - 1 + i - 1] = s;
                }
                let mmn = l - m + 1;
                let cs: Vec<f64> = work[m - 1..m - 1 + mmn - 1].to_vec();
                let sn: Vec<f64> = work[n - 1 + m - 1..n - 1 + m - 1 + mmn - 1].to_vec();
                dlasr(false, true, n, mmn, &cs, &sn, z, zat(1, m), n);
                dd!(l) -= p;
                ee!(lm1) = g;
            }
        }
        // label 140: undo scaling if necessary.
        if iscale == 1 {
            scale_block(d, e, lsv, lendsv, ssfmax, anorm);
        } else if iscale == 2 {
            scale_block(d, e, lsv, lendsv, ssfmin, anorm);
        }
        // Check for no convergence to an eigenvalue after a total of N*MAXIT iterations.
        if jtot < nmaxit {
            continue 'outer;
        }
        return e.iter().take(n - 1).filter(|v| **v != 0.0).count();
    }
    // label 160: order eigenvalues and eigenvectors (selection sort).
    for ii in 2..=n {
        let i = ii - 1;
        let mut k = i;
        let mut p = dd!(i);
        for j in ii..=n {
            if dd!(j) < p {
                k = j;
                p = dd!(j);
            }
        }
        if k != i {
            dd!(k) = dd!(i);
            dd!(i) = p;
            dswap(n, z, zat(1, i), 1, zat(1, k), 1);
        }
    }
    0
}

/// `DLASCL('G', 0, 0, CFROM, CTO, ...)` on `D(l:lend)` and `E(l:lend-1)`.
fn scale_block(d: &mut [f64], e: &mut [f64], l: usize, lend: usize, cfrom: f64, cto: f64) {
    {
        let mut vals: Vec<&mut f64> = d[l - 1..lend].iter_mut().collect();
        dlascl(cfrom, cto, &mut vals);
    }
    let mut vals: Vec<&mut f64> = e[l - 1..lend - 1].iter_mut().collect();
    dlascl(cfrom, cto, &mut vals);
}

/// `numpy.linalg.eigh(a)` (`UPLO='L'`: the lower triangle of `a` is read) of a float64 4 × 4
/// matrix: `(w, v)` with the eigenvalues ascending and `v[r][k]` the r-th component of the k-th
/// eigenvector, as numpy's `dsyevd` returns them. `None` where numpy raises `LinAlgError`
/// (no convergence, or a value that is not finite).
pub fn eigh4(a: &[[f64; 4]; 4], release: LapackRelease) -> Option<([f64; 4], [[f64; 4]; 4])> {
    const N: usize = 4;
    if a.iter()
        .enumerate()
        .any(|(i, r)| r[..=i].iter().any(|v| !v.is_finite()))
    {
        return None;
    }
    // numpy's linearize: column-major, A(i, j) = a[i][j].
    let mut am = [0.0f64; N * N];
    for i in 0..N {
        for j in 0..N {
            am[i + j * N] = a[i][j];
        }
    }
    // DSYEVD: scale if the largest element is outside [RMIN, RMAX] (DLANSY('M', 'L')).
    let smlnum = SAFMIN / f64::EPSILON;
    let bignum = 1.0 / smlnum;
    let rmin = smlnum.sqrt();
    let rmax = bignum.sqrt();
    let mut anrm = 0.0f64;
    for j in 0..N {
        for i in j..N {
            let s = am[i + j * N].abs();
            if anrm < s || s.is_nan() {
                anrm = s;
            }
        }
    }
    let mut sigma = 1.0;
    let mut iscale = false;
    if anrm > 0.0 && anrm < rmin {
        iscale = true;
        sigma = rmin / anrm;
    } else if anrm > rmax {
        iscale = true;
        sigma = rmax / anrm;
    }
    if iscale {
        let mut vals: Vec<&mut f64> = Vec::new();
        for (idx, v) in am.iter_mut().enumerate() {
            let (i, j) = (idx % N, idx / N);
            if i >= j {
                vals.push(v);
            }
        }
        dlascl(1.0, sigma, &mut vals);
    }
    let (mut w, mut e, tau) = dsytd2_lower(N, &mut am, N);
    let mut z = [0.0f64; N * N];
    if dsteqr_i(N, &mut w, &mut e, &mut z) != 0 {
        return None;
    }
    // DORMTR('L', 'L', 'N', N, N, A, LDA, TAU, Z, N): DORMQR on A(2:, 1:), Z(2:, :) →
    // DORM2R (left, no transpose: reflectors K down to 1).
    let k = N - 1;
    for i in (1..=k).rev() {
        // v = A(i+1 : N, i) of the full matrix; the sub-problem's A(i, i) is A(i+1, i).
        let mi = (N - 1) - i + 1;
        let v: Vec<f64> = match release {
            LapackRelease::V3_12_0 => (0..mi)
                .map(|r| {
                    if r == 0 {
                        1.0
                    } else {
                        am[(i + r) + (i - 1) * N]
                    }
                })
                .collect(),
            LapackRelease::V3_12_1 => (0..mi)
                .map(|r| {
                    if r == 0 {
                        1.0
                    } else {
                        am[(i + r) + (i - 1) * N]
                    }
                })
                .collect(),
        };
        // C = Z(2:N, 1:N) rows i..: global row i+1.
        let coff = i;
        apply_reflector(release, true, mi, N, &v, tau[i - 1], &mut z, coff, N);
    }
    if iscale {
        let mut vals: Vec<&mut f64> = w.iter_mut().collect();
        let r = 1.0 / sigma;
        if r != 1.0 {
            for v in vals.iter_mut() {
                **v *= r;
            }
        }
    }
    let mut vecs = [[0.0f64; N]; N];
    for (r, row) in vecs.iter_mut().enumerate() {
        for (c, x) in row.iter_mut().enumerate() {
            *x = z[r + c * N];
        }
    }
    Some(([w[0], w[1], w[2], w[3]], vecs))
}

// ------------------------------------------------------------------------------------------
// dgesdd

/// `DGEBD2(M, N, A, ...)` for `M >= N`: `a` (column-major, `lda`) reduced to upper bidiagonal
/// form in place; returns `(d, e, tauq, taup)`.
fn dgebd2(
    release: LapackRelease,
    m: usize,
    n: usize,
    a: &mut [f64],
    lda: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let at = |i: usize, j: usize| (i - 1) + (j - 1) * lda;
    let mut d = vec![0.0; n];
    let mut e = vec![0.0; n.saturating_sub(1)];
    let mut tauq = vec![0.0; n];
    let mut taup = vec![0.0; n];
    for i in 1..=n {
        // H(i) to annihilate A(i+1:m, i).
        let mut alpha = a[at(i, i)];
        let xoff = at((i + 1).min(m), i);
        tauq[i - 1] = dlarfg(m - i + 1, &mut alpha, a, xoff, 1);
        a[at(i, i)] = alpha;
        d[i - 1] = a[at(i, i)];
        if i < n {
            let v: Vec<f64> = (i..=m)
                .map(|r| if r == i { 1.0 } else { a[at(r, i)] })
                .collect();
            if release == LapackRelease::V3_12_0 {
                a[at(i, i)] = 1.0;
            }
            apply_reflector(
                release,
                true,
                m - i + 1,
                n - i,
                &v,
                tauq[i - 1],
                a,
                at(i, i + 1),
                lda,
            );
            if release == LapackRelease::V3_12_0 {
                a[at(i, i)] = d[i - 1];
            }
        }
        if i < n {
            // G(i) to annihilate A(i, i+2:n).
            let mut alpha = a[at(i, i + 1)];
            let xoff = at(i, (i + 2).min(n));
            taup[i - 1] = dlarfg(n - i, &mut alpha, a, xoff, lda);
            a[at(i, i + 1)] = alpha;
            e[i - 1] = a[at(i, i + 1)];
            let v: Vec<f64> = (i + 1..=n)
                .map(|c| if c == i + 1 { 1.0 } else { a[at(i, c)] })
                .collect();
            if release == LapackRelease::V3_12_0 {
                a[at(i, i + 1)] = 1.0;
            }
            apply_reflector(
                release,
                false,
                m - i,
                n - i,
                &v,
                taup[i - 1],
                a,
                at(i + 1, i + 1),
                lda,
            );
            if release == LapackRelease::V3_12_0 {
                a[at(i, i + 1)] = e[i - 1];
            }
        } else {
            taup[i - 1] = 0.0;
        }
    }
    (d, e, tauq, taup)
}

/// `DBDSQR('U', N, NCVT, NRU, 0, D, E, VT, LDVT, U, LDU, ...)` with `NCVT = NRU = N` (`vt`
/// and `u` column-major `n × n`). Returns LAPACK's `info` (0 on convergence).
fn dbdsqr_upper(n: usize, d: &mut [f64], e: &mut [f64], vt: &mut [f64], u: &mut [f64]) -> usize {
    let (ncvt, nru) = (n, n);
    dbdsqr_upper_ncc(n, d, e, (vt, ncvt, n), (u, nru, n), (&mut [], 0, 1), true)
}

/// `DBDSQR('U', N, NCVT, NRU, NCC, D, E, VT, LDVT, U, LDU, C, LDC, ...)`: `vt` (`ncvt` columns,
/// leading dimension `ldvt`), `u` (`nru` rows, `ldu`) and `c` (`ncc` columns, `ldc`), each
/// column-major with its block at offset 0. `clear_negative_zero`: LAPACK 3.12.1's "avoid
/// -ZERO" when the singular values are made positive (3.12.0 has no such step). Returns
/// LAPACK's `info` (0 on convergence).
fn dbdsqr_upper_ncc(
    n: usize,
    d: &mut [f64],
    e: &mut [f64],
    (vt, ncvt, ldvt): (&mut [f64], usize, usize),
    (u, nru, ldu): (&mut [f64], usize, usize),
    (cm, ncc, ldc): (&mut [f64], usize, usize),
    clear_negative_zero: bool,
) -> usize {
    const MAXITR: usize = 6;
    let vat = |i: usize, j: usize| (i - 1) + (j - 1) * ldvt;
    let uat = |i: usize, j: usize| (i - 1) + (j - 1) * ldu;
    let cat = |i: usize, j: usize| (i - 1) + (j - 1) * ldc;
    macro_rules! dd {
        ($k:expr) => {
            d[$k - 1]
        };
    }
    macro_rules! ee {
        ($k:expr) => {
            e[$k - 1]
        };
    }
    if n == 0 {
        return 0;
    }
    if n > 1 {
        let nm1 = n - 1;
        let nm12 = nm1 + nm1;
        let nm13 = nm12 + nm1;
        let mut work = vec![0.0f64; 4 * n];
        let mut idir = 0;
        let eps = EPS;
        let unfl = SAFMIN;
        // TOLMUL = MAX(10, MIN(100, EPS**(-1/8))): glibc's pow(2^-53, -0.125), correctly
        // rounded (2^6.625).
        let tolmul = 10f64.max(100f64.min(f64::from_bits(0x4058_ACDB_EF8F_9B61)));
        let tol = tolmul * eps;
        // Compute approximate maximum, minimum singular values.
        let mut smax = 0.0f64;
        for i in 1..=n {
            smax = smax.max(dd!(i).abs());
        }
        for i in 1..n {
            smax = smax.max(ee!(i).abs());
        }
        let mut smin;
        let thresh;
        // TOL >= 0: relative accuracy.
        {
            let mut sminoa = dd!(1).abs();
            if sminoa != 0.0 {
                let mut mu = sminoa;
                for i in 2..=n {
                    mu = dd!(i).abs() * (mu / (mu + ee!(i - 1).abs()));
                    sminoa = sminoa.min(mu);
                    if sminoa == 0.0 {
                        break;
                    }
                }
            }
            sminoa /= (n as f64).sqrt();
            thresh = (tol * sminoa).max((MAXITR as f64) * ((n as f64) * ((n as f64) * unfl)));
        }
        let maxitdivn = MAXITR * n;
        let mut iterdivn = 0;
        let mut iter: isize = -1;
        let mut oldll: isize = -1;
        let mut oldm: isize = -1;
        let mut m = n;
        'main: loop {
            // label 60
            if m <= 1 {
                break 'main;
            }
            if iter >= n as isize {
                iter -= n as isize;
                iterdivn += 1;
                if iterdivn >= maxitdivn {
                    // label 200: no convergence.
                    return e.iter().take(n - 1).filter(|v| **v != 0.0).count();
                }
            }
            // Find diagonal block of matrix to work on.
            smax = dd!(m).abs();
            let mut ll = 0usize;
            let mut split = false;
            for lll in 1..m {
                ll = m - lll;
                let abss = dd!(ll).abs();
                let abse = ee!(ll).abs();
                if abse <= thresh {
                    split = true;
                    break;
                }
                smax = smax.max(abss).max(abse);
            }
            if split {
                // label 80
                ee!(ll) = 0.0;
                if ll == m - 1 {
                    m -= 1;
                    continue 'main;
                }
            } else {
                ll = 0;
            }
            // label 90
            ll += 1;
            // E(LL) through E(M-1) are nonzero, E(LL-1) is zero.
            if ll == m - 1 {
                // 2 by 2 block, handle separately.
                let (sigmn, sigmx, sinr, cosr, sinl, cosl) = dlasv2(dd!(m - 1), ee!(m - 1), dd!(m));
                dd!(m - 1) = sigmx;
                ee!(m - 1) = 0.0;
                dd!(m) = sigmn;
                drot(ncvt, vt, vat(m - 1, 1), ldvt, vat(m, 1), ldvt, cosr, sinr);
                drot(nru, u, uat(1, m - 1), 1, uat(1, m), 1, cosl, sinl);
                drot(ncc, cm, cat(m - 1, 1), ldc, cat(m, 1), ldc, cosl, sinl);
                m -= 2;
                continue 'main;
            }
            // If working on new submatrix, choose shift direction (from larger end diagonal
            // element towards smaller).
            if ll as isize > oldm || (m as isize) < oldll {
                if dd!(ll).abs() >= dd!(m).abs() {
                    idir = 1;
                } else {
                    idir = 2;
                }
            }
            // Apply convergence tests.
            if idir == 1 {
                if ee!(m - 1).abs() <= tol.abs() * dd!(m).abs() {
                    ee!(m - 1) = 0.0;
                    continue 'main;
                }
                let mut mu = dd!(ll).abs();
                smin = mu;
                for lll in ll..m {
                    if ee!(lll).abs() <= tol * mu {
                        ee!(lll) = 0.0;
                        continue 'main;
                    }
                    mu = dd!(lll + 1).abs() * (mu / (mu + ee!(lll).abs()));
                    smin = smin.min(mu);
                }
            } else {
                if ee!(ll).abs() <= tol.abs() * dd!(ll).abs() {
                    ee!(ll) = 0.0;
                    continue 'main;
                }
                let mut mu = dd!(m).abs();
                smin = mu;
                let mut lll = m - 1;
                loop {
                    if ee!(lll).abs() <= tol * mu {
                        ee!(lll) = 0.0;
                        continue 'main;
                    }
                    mu = dd!(lll).abs() * (mu / (mu + ee!(lll).abs()));
                    smin = smin.min(mu);
                    if lll == ll {
                        break;
                    }
                    lll -= 1;
                }
            }
            oldll = ll as isize;
            oldm = m as isize;
            // Compute shift.
            let mut shift;
            if (n as f64) * tol * (smin / smax) <= eps.max(0.01 * tol) {
                shift = 0.0;
            } else {
                let sll;
                let r;
                if idir == 1 {
                    sll = dd!(ll).abs();
                    let (s, rr) = dlas2(dd!(m - 1), ee!(m - 1), dd!(m));
                    shift = s;
                    r = rr;
                } else {
                    sll = dd!(m).abs();
                    let (s, rr) = dlas2(dd!(ll), ee!(ll), dd!(ll + 1));
                    shift = s;
                    r = rr;
                }
                let _ = r;
                if sll > 0.0 {
                    let q = shift / sll;
                    if q * q < eps {
                        shift = 0.0;
                    }
                }
            }
            iter += (m - ll) as isize;
            if shift == 0.0 {
                if idir == 1 {
                    // Chase bulge from top to bottom.
                    let mut cs = 1.0;
                    let mut oldcs = 1.0;
                    let mut oldsn = 0.0;
                    for i in ll..m {
                        let (c1, sn, r) = dlartg(dd!(i) * cs, ee!(i));
                        cs = c1;
                        if i > ll {
                            ee!(i - 1) = oldsn * r;
                        }
                        let (c2, s2, r2) = dlartg(oldcs * r, dd!(i + 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        dd!(i) = r2;
                        work[i - ll] = cs;
                        work[i - ll + nm1] = sn;
                        work[i - ll + nm12] = oldcs;
                        work[i - ll + nm13] = oldsn;
                    }
                    let h = dd!(m) * cs;
                    dd!(m) = h * oldcs;
                    ee!(m - 1) = h * oldsn;
                    let k = m - ll + 1;
                    let (c, s) = (work[0..k - 1].to_vec(), work[nm1..nm1 + k - 1].to_vec());
                    dlasr(true, true, k, ncvt, &c, &s, vt, vat(ll, 1), ldvt);
                    let (c, s) = (
                        work[nm12..nm12 + k - 1].to_vec(),
                        work[nm13..nm13 + k - 1].to_vec(),
                    );
                    dlasr(false, true, nru, k, &c, &s, u, uat(1, ll), ldu);
                    dlasr(true, true, k, ncc, &c, &s, cm, cat(ll, 1), ldc);
                    if ee!(m - 1).abs() <= thresh {
                        ee!(m - 1) = 0.0;
                    }
                } else {
                    // Chase bulge from bottom to top.
                    let mut cs = 1.0;
                    let mut oldcs = 1.0;
                    let mut oldsn = 0.0;
                    let mut i = m;
                    while i >= ll + 1 {
                        let (c1, sn, r) = dlartg(dd!(i) * cs, ee!(i - 1));
                        cs = c1;
                        if i < m {
                            ee!(i) = oldsn * r;
                        }
                        let (c2, s2, r2) = dlartg(oldcs * r, dd!(i - 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        dd!(i) = r2;
                        work[i - ll - 1] = cs;
                        work[i - ll - 1 + nm1] = -sn;
                        work[i - ll - 1 + nm12] = oldcs;
                        work[i - ll - 1 + nm13] = -oldsn;
                        i -= 1;
                    }
                    let h = dd!(ll) * cs;
                    dd!(ll) = h * oldcs;
                    ee!(ll) = h * oldsn;
                    let k = m - ll + 1;
                    let (c, s) = (
                        work[nm12..nm12 + k - 1].to_vec(),
                        work[nm13..nm13 + k - 1].to_vec(),
                    );
                    dlasr(true, false, k, ncvt, &c, &s, vt, vat(ll, 1), ldvt);
                    let (c, s) = (work[0..k - 1].to_vec(), work[nm1..nm1 + k - 1].to_vec());
                    dlasr(false, false, nru, k, &c, &s, u, uat(1, ll), ldu);
                    dlasr(true, false, k, ncc, &c, &s, cm, cat(ll, 1), ldc);
                    if ee!(ll).abs() <= thresh {
                        ee!(ll) = 0.0;
                    }
                }
            } else if idir == 1 {
                // Chase bulge from top to bottom (shifted).
                let mut f = (dd!(ll).abs() - shift) * (sign(1.0, dd!(ll)) + shift / dd!(ll));
                let mut g = ee!(ll);
                for i in ll..m {
                    let (cosr, sinr, r) = dlartg(f, g);
                    if i > ll {
                        ee!(i - 1) = r;
                    }
                    f = cosr * dd!(i) + sinr * ee!(i);
                    ee!(i) = cosr * ee!(i) - sinr * dd!(i);
                    g = sinr * dd!(i + 1);
                    dd!(i + 1) = cosr * dd!(i + 1);
                    let (cosl, sinl, r) = dlartg(f, g);
                    dd!(i) = r;
                    f = cosl * ee!(i) + sinl * dd!(i + 1);
                    dd!(i + 1) = cosl * dd!(i + 1) - sinl * ee!(i);
                    if i < m - 1 {
                        g = sinl * ee!(i + 1);
                        ee!(i + 1) = cosl * ee!(i + 1);
                    }
                    work[i - ll] = cosr;
                    work[i - ll + nm1] = sinr;
                    work[i - ll + nm12] = cosl;
                    work[i - ll + nm13] = sinl;
                }
                ee!(m - 1) = f;
                let k = m - ll + 1;
                let (c, s) = (work[0..k - 1].to_vec(), work[nm1..nm1 + k - 1].to_vec());
                dlasr(true, true, k, ncvt, &c, &s, vt, vat(ll, 1), ldvt);
                let (c, s) = (
                    work[nm12..nm12 + k - 1].to_vec(),
                    work[nm13..nm13 + k - 1].to_vec(),
                );
                dlasr(false, true, nru, k, &c, &s, u, uat(1, ll), ldu);
                dlasr(true, true, k, ncc, &c, &s, cm, cat(ll, 1), ldc);
                if ee!(m - 1).abs() <= thresh {
                    ee!(m - 1) = 0.0;
                }
            } else {
                // Chase bulge from bottom to top (shifted).
                let mut f = (dd!(m).abs() - shift) * (sign(1.0, dd!(m)) + shift / dd!(m));
                let mut g = ee!(m - 1);
                let mut i = m;
                while i >= ll + 1 {
                    let (cosr, sinr, r) = dlartg(f, g);
                    if i < m {
                        ee!(i) = r;
                    }
                    f = cosr * dd!(i) + sinr * ee!(i - 1);
                    ee!(i - 1) = cosr * ee!(i - 1) - sinr * dd!(i);
                    g = sinr * dd!(i - 1);
                    dd!(i - 1) = cosr * dd!(i - 1);
                    let (cosl, sinl, r) = dlartg(f, g);
                    dd!(i) = r;
                    f = cosl * ee!(i - 1) + sinl * dd!(i - 1);
                    dd!(i - 1) = cosl * dd!(i - 1) - sinl * ee!(i - 1);
                    if i > ll + 1 {
                        g = sinl * ee!(i - 2);
                        ee!(i - 2) = cosl * ee!(i - 2);
                    }
                    work[i - ll - 1] = cosr;
                    work[i - ll - 1 + nm1] = -sinr;
                    work[i - ll - 1 + nm12] = cosl;
                    work[i - ll - 1 + nm13] = -sinl;
                    i -= 1;
                }
                ee!(ll) = f;
                if ee!(ll).abs() <= thresh {
                    ee!(ll) = 0.0;
                }
                let k = m - ll + 1;
                let (c, s) = (
                    work[nm12..nm12 + k - 1].to_vec(),
                    work[nm13..nm13 + k - 1].to_vec(),
                );
                dlasr(true, false, k, ncvt, &c, &s, vt, vat(ll, 1), ldvt);
                let (c, s) = (work[0..k - 1].to_vec(), work[nm1..nm1 + k - 1].to_vec());
                dlasr(false, false, nru, k, &c, &s, u, uat(1, ll), ldu);
                dlasr(true, false, k, ncc, &c, &s, cm, cat(ll, 1), ldc);
            }
        }
    }
    // label 160: all singular values converged, so make them positive.
    for i in 1..=n {
        if clear_negative_zero && dd!(i) == 0.0 {
            dd!(i) = 0.0;
        }
        if dd!(i) < 0.0 {
            dd!(i) = -dd!(i);
            dscal(ncvt, -1.0, vt, vat(i, 1), ldvt);
        }
    }
    // Sort the singular values into decreasing order (insertion sort on singular values, but
    // only one transposition per singular vector).
    for i in 1..n {
        let mut isub = 1;
        let mut smin = dd!(1);
        for j in 2..=n + 1 - i {
            if dd!(j) <= smin {
                isub = j;
                smin = dd!(j);
            }
        }
        if isub != n + 1 - i {
            dd!(isub) = dd!(n + 1 - i);
            dd!(n + 1 - i) = smin;
            dswap(ncvt, vt, vat(isub, 1), ldvt, vat(n + 1 - i, 1), ldvt);
            dswap(nru, u, uat(1, isub), 1, uat(1, n + 1 - i), 1);
            dswap(ncc, cm, cat(isub, 1), ldc, cat(n + 1 - i, 1), ldc);
        }
    }
    0
}

/// `DBDSDC('U', 'I', N, D, E, U, LDU, VT, LDVT, ...)` for `N <= 25`: `DLASDQ` (→ `DBDSQR`,
/// then its ascending sort), then the descending selection sort. Returns `info`.
fn dbdsdc_upper(n: usize, d: &mut [f64], e: &mut [f64], u: &mut [f64], vt: &mut [f64]) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        u[0] = sign(1.0, d[0]);
        vt[0] = 1.0;
        d[0] = d[0].abs();
        return 0;
    }
    // DLASET(U, VT) = I.
    for v in u.iter_mut().chain(vt.iter_mut()) {
        *v = 0.0;
    }
    for i in 0..n {
        u[i + i * n] = 1.0;
        vt[i + i * n] = 1.0;
    }
    // DLASDQ('U', 0, N, N, N, 0, ...): upper, square: straight to DBDSQR, then its sort.
    let info = dbdsqr_upper(n, d, e, vt, u);
    if info != 0 {
        return info;
    }
    for i in 1..=n {
        let mut isub = i;
        let mut smin = d[i - 1];
        for j in i + 1..=n {
            if d[j - 1] < smin {
                isub = j;
                smin = d[j - 1];
            }
        }
        if isub != i {
            d[isub - 1] = d[i - 1];
            d[i - 1] = smin;
            dswap(n, vt, isub - 1, n, i - 1, n);
            dswap(n, u, (isub - 1) * n, 1, (i - 1) * n, 1);
        }
    }
    // Label 40: selection sort into decreasing order.
    for ii in 2..=n {
        let i = ii - 1;
        let mut kk = i;
        let mut p = d[i - 1];
        for j in ii..=n {
            if d[j - 1] > p {
                kk = j;
                p = d[j - 1];
            }
        }
        if kk != i {
            d[kk - 1] = d[i - 1];
            d[i - 1] = p;
            dswap(n, u, (i - 1) * n, 1, (kk - 1) * n, 1);
            dswap(n, vt, i - 1, n, kk - 1, n);
        }
    }
    0
}

/// `numpy.linalg.svd(a)` (`full_matrices=True`: `dgesdd` with `JOBZ='A'`) of a float64 3 × 3
/// matrix: `(u, s, vt)` as numpy returns them (`a = u · diag(s) · vt`, `s` descending).
/// `None` where numpy raises `LinAlgError` (no convergence, a value that is not finite).
pub fn svd3(
    a: &[[f64; 3]; 3],
    release: LapackRelease,
) -> Option<([[f64; 3]; 3], [f64; 3], [[f64; 3]; 3])> {
    const N: usize = 3;
    if a.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let mut am = [0.0f64; N * N];
    for i in 0..N {
        for j in 0..N {
            am[i + j * N] = a[i][j];
        }
    }
    // DGESDD: scale A if max element outside range [SMLNUM, BIGNUM] (DLANGE('M')).
    let eps = f64::EPSILON; // DLAMCH('P')
    let smlnum = SAFMIN.sqrt() / eps;
    let bignum = 1.0 / smlnum;
    let anrm = am
        .iter()
        .fold(0.0f64, |m, v| if m < v.abs() { v.abs() } else { m });
    let mut iscl = false;
    if anrm > 0.0 && anrm < smlnum {
        iscl = true;
        let mut vals: Vec<&mut f64> = am.iter_mut().collect();
        dlascl(anrm, smlnum, &mut vals);
    } else if anrm > bignum {
        iscl = true;
        let mut vals: Vec<&mut f64> = am.iter_mut().collect();
        dlascl(anrm, bignum, &mut vals);
    }
    // Path 5 (M >= N, M < MNTHR = 5): bidiagonalize without QR; JOBZ='A'.
    let (mut s, mut e, tauq, taup) = dgebd2(release, N, N, &mut am, N);
    let mut u = [0.0f64; N * N];
    let mut vt = [0.0f64; N * N];
    if dbdsdc_upper(N, &mut s, &mut e, &mut u, &mut vt) != 0 {
        return None;
    }
    // DORMBR('Q', 'L', 'N', M, M, N, A, LDA, TAUQ, U, LDU): NQ = M >= K → DORMQR → DORM2R
    // (left, no transpose: reflectors N down to 1), H(i) on U(i:M, 1:M) with v = A(i:M, i).
    for i in (1..=N).rev() {
        let mi = N - i + 1;
        let v: Vec<f64> = (0..mi)
            .map(|r| {
                if r == 0 {
                    1.0
                } else {
                    am[(i - 1 + r) + (i - 1) * N]
                }
            })
            .collect();
        apply_reflector(release, true, mi, N, &v, tauq[i - 1], &mut u, i - 1, N);
    }
    // DORMBR('P', 'R', 'T', N, N, M, A, LDA, TAUP, VT, LDVT): NQ = N = K → DORMLQ('R', 'N',
    // N, N-1, N-1, A(1, 2), LDA, TAUP, VT(1, 2), LDVT) → DORML2 (right, no transpose:
    // reflectors K down to 1), G(i) on VT(1:N, i+1:N) with v = A(i, i+1:N).
    let k = N - 1;
    for i in (1..=k).rev() {
        let ni = (N - 1) - i + 1;
        let v: Vec<f64> = (0..ni)
            .map(|c| {
                if c == 0 {
                    1.0
                } else {
                    am[(i - 1) + (i + c) * N]
                }
            })
            .collect();
        apply_reflector(release, false, N, ni, &v, taup[i - 1], &mut vt, i * N, N);
    }
    if iscl {
        let mut vals: Vec<&mut f64> = s.iter_mut().collect();
        if anrm > bignum {
            dlascl(bignum, anrm, &mut vals);
        }
        if anrm < smlnum {
            dlascl(smlnum, anrm, &mut vals);
        }
    }
    let mut um = [[0.0f64; N]; N];
    let mut vm = [[0.0f64; N]; N];
    for r in 0..N {
        for c in 0..N {
            um[r][c] = u[r + c * N];
            vm[r][c] = vt[r + c * N];
        }
    }
    Some((um, [s[0], s[1], s[2]], vm))
}

/// `numpy.linalg.det(a) > 0` for a float64 3 × 3 matrix: OpenBLAS's `dgetrf` (`getf2_k`),
/// then numpy's `sign · exp(Σ log|u_ii|)` (`det_from_slogdet`); a singular matrix (an exactly
/// zero pivot) has determinant 0.
pub fn det3_is_positive(a: &[[f64; 3]; 3]) -> bool {
    match openblas::lu_sign_and_diagonal3(a) {
        None => false,
        Some((mut sign, diag)) => {
            // slogdet_from_factored_diagonal: the sign of each pivot, then Σ log|u_ii|.
            let mut logdet = 0.0f64;
            for v in diag {
                let mut a = v;
                if a < 0.0 {
                    sign = -sign;
                    a = -a;
                }
                logdet += larmorx_core::math::log(a);
            }
            let det = sign * larmorx_core::math::exp(logdet);
            det > 0.0
        }
    }
}

/// nibabel's `io_orientation` polar step (`orientations.py`, nibabel 5.4.2): `P, S, Qs =
/// numpy.linalg.svd(RS, full_matrices=False)` (for a square matrix `dgesdd`'s `JOBZ='S'` path
/// is `JOBZ='A'`'s), `keep = S > S.max() * 3 * eps`, `R = numpy.dot(P[:, keep], Qs[keep])`.
/// `None` where numpy raises `LinAlgError`.
pub fn io_orientation_polar(rs: &[[f64; 3]; 3], release: LapackRelease) -> Option<[[f64; 3]; 3]> {
    let (p, s, qs) = svd3(rs, release)?;
    let smax = s
        .iter()
        .fold(f64::NEG_INFINITY, |m, &v| if v > m { v } else { m });
    let tol = smax * 3.0 * f64::EPSILON;
    let keep: Vec<usize> = (0..3).filter(|&k| s[k] > tol).collect();
    let k = keep.len();
    if k == 0 {
        return Some([[0.0; 3]; 3]);
    }
    let a: Vec<f64> = (0..3)
        .flat_map(|i| keep.iter().map(move |&c| (i, c)))
        .map(|(i, c)| p[i][c])
        .collect();
    let b: Vec<f64> = keep.iter().flat_map(|&r| qs[r]).collect();
    let c = openblas::gemm(&a, &b, 3, k, 3);
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| c[i * 3 + j])
    }))
}

/// `numpy.dot(p, q)` of two float64 3 × 3 matrices (OpenBLAS's Haswell `dgemm`).
pub fn matmul3(p: &[[f64; 3]; 3], q: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let a: Vec<f64> = p.iter().flatten().copied().collect();
    let b: Vec<f64> = q.iter().flatten().copied().collect();
    let c = openblas::gemm(&a, &b, 3, 3, 3);
    std::array::from_fn(|i| std::array::from_fn(|j| c[i * 3 + j]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_root_of_two_squares() {
        // √(3² + 4²) = 5 exactly; √(1 + 1) = √2 rounded twice.
        assert_eq!(dnrm2(2, &[3.0, 4.0], 0, 1), 5.0);
        assert_eq!(dnrm2(2, &[1.0, 1.0], 0, 1), 2f64.sqrt());
        assert_eq!(dnrm2(1, &[-2.5], 0, 1), 2.5);
        // The main loop's four accumulators: nine ones sum exactly; tiny and huge entries keep
        // the extended range.
        assert_eq!(dnrm2(9, &[1.0; 9], 0, 1), 3.0);
        assert_eq!(dnrm2(2, &[3e-300, 4e-300], 0, 1), 5e-300);
        assert_eq!(dnrm2(2, &[3e300, 4e300], 0, 1), 5e300);
        assert_eq!(dnrm2(2, &[f64::MAX, f64::MAX], 0, 1), f64::INFINITY);
        let tiny = f64::from_bits(3);
        assert_eq!(dnrm2(1, &[tiny], 0, 1), tiny);
        assert_eq!(dnrm2(2, &[tiny, 0.0], 0, 1), tiny);
    }

    #[test]
    fn rounding_to_double() {
        assert_eq!(round_to_f64(3, 0), 3.0);
        assert_eq!(round_to_f64((1u128 << 53) + 1, 0), 9007199254740992.0);
        assert_eq!(round_to_f64((1u128 << 53) + 3, 0), 9007199254740996.0);
        assert_eq!(round_to_f64(u128::from(u64::MAX), -64), 1.0);
        assert_eq!(round_to_f64(1, -1074), f64::from_bits(1));
        assert_eq!(round_to_f64(1, -1075), 0.0);
        assert_eq!(round_to_f64(3, -1076), f64::from_bits(1));
        assert_eq!(round_to_f64(3, -1075), f64::from_bits(2));
        assert_eq!(round_to_f64(1, 1024), f64::INFINITY);
        assert_eq!(round_to_f64((1u128 << 53) - 1, 971), f64::MAX);
        assert_eq!(round_to_f64((1u128 << 54) - 1, 970), f64::INFINITY);
        // The largest subnormal rounding up into the normal range.
        assert_eq!(round_to_f64((1u128 << 53) - 1, -1075), f64::MIN_POSITIVE);
    }

    #[test]
    fn svd_and_eigh_reconstruct() {
        let a = [[0.8, -0.6, 0.1], [0.6, 0.8, 0.05], [0.02, -0.03, 1.0]];
        for rel in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let (u, s, vt) = svd3(&a, rel).unwrap();
            for i in 0..3 {
                for j in 0..3 {
                    let r: f64 = (0..3).map(|k| u[i][k] * s[k] * vt[k][j]).sum();
                    assert!((r - a[i][j]).abs() < 1e-14, "{rel:?} {i} {j}");
                }
            }
            assert!(s[0] >= s[1] && s[1] >= s[2]);
            let k = [
                [0.5, 0.0, 0.0, 0.0],
                [0.1, -0.3, 0.0, 0.0],
                [0.2, 0.05, 0.4, 0.0],
                [-0.1, 0.3, 0.2, 0.9],
            ];
            let (w, v) = eigh4(&k, rel).unwrap();
            assert!(w[0] <= w[1] && w[1] <= w[2] && w[2] <= w[3]);
            let full = |i: usize, j: usize| if i >= j { k[i][j] } else { k[j][i] };
            for c in 0..4 {
                for r in 0..4 {
                    let av: f64 = (0..4).map(|t| full(r, t) * v[t][c]).sum();
                    assert!((av - w[c] * v[r][c]).abs() < 1e-14);
                }
            }
        }
    }
}

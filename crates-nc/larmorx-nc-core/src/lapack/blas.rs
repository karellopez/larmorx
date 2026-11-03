// SPDX-License-Identifier: LicenseRef-FSL AND BSD-3-Clause
// Translated from OpenBLAS 0.3.33 (BSD-3-Clause; Copyright (c) 2011-2014, The OpenBLAS Project;
// the notice is in ../lapack.rs and ../../NOTICE). Part of larmorx-nc (FSL Licence terms; see
// LICENSE).
//! The BLAS routines the reference LAPACK code of FSL's eigen- and singular-value
//! decompositions calls, as OpenBLAS 0.3.33 computes them on its **Haswell** kernels (the
//! kernels the validation pins: `OPENBLAS_CORETYPE=Haswell`), for the small sizes FSL's
//! programs reach (vectors of fewer than 16 elements, matrices of a few rows and columns), and
//! for the medium sizes of LAPACK's symmetric indefinite solver ([`super::sytrf`]: `daxpy` of any
//! length, `dger`, `dsyr`, `dgemv` with strided vectors).
//!
//! Each routine is the OpenBLAS interface (`interface/*.c`: the quick returns, `beta`'s scaling)
//! followed by its Haswell kernel (`kernel/x86_64/KERNEL.HASWELL`): `ddot.c`, `daxpy.c` and
//! `daxpy_microk_haswell-2.c`, `dscal.c`, `drot.c`, `nrm2.S` (x87 extended precision),
//! `dgemv_n_4.c` and `dgemv_n_microk_haswell-4.c`, `dgemv_t_4.c` and
//! `dgemv_t_microk_haswell-4.c`, `dsymv_U.c`, `../generic/ger.c`, `interface/syr.c`,
//! `interface/syr2.c`. The C kernels were compiled by GCC with `-mavx2` and
//! without `-mfma` (`kernel/Makefile`'s `AVX2OPT` for GCC), so their products and sums round
//! separately; the hand-written assembly fuses where its instructions do (`vfmadd231pd`).
//! The logic is re-expressed in Rust; no code was copied.
//!
//! Vectors are passed **gathered** (their elements in order, as a contiguous slice) with the
//! increment the caller declared, which only selects the kernel's code path (a strided `ddot`
//! sums in two accumulators, a unit one in one). Matrices are a column-major array, the offset
//! of their first element and the leading dimension. Armadillo refuses non-finite input before
//! it calls LAPACK (`arma_config::check_nonfinite`), so the NaN rules of SSE are not modelled
//! here.

/// `vfmadd231pd`: `c + a·b` with one rounding.
#[inline(always)]
fn fma(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}

/// `DDOT` (`interface/dot.c`, the Haswell `ddot.c` for fewer than 16 elements): unit strides
/// sum `y[i]·x[i]` from 0 in order; other strides sum the products of each group of four in two
/// accumulators (`t1 += p0 + p2`, `t2 += p1 + p3`), the rest into the first, then `t1 + t2`.
pub fn ddot(x: &[f64], incx: usize, y: &[f64], incy: usize) -> f64 {
    let n = x.len();
    assert_eq!(n, y.len());
    if n == 0 {
        return 0.0;
    }
    if incx == 1 && incy == 1 {
        assert!(
            n < 16,
            "ddot's 8-way kernel (16 or more elements) is not ported"
        );
        let mut dot = 0.0;
        for i in 0..n {
            dot += y[i] * x[i];
        }
        return dot;
    }
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

/// `DDOT` with unit strides and any length (`ddot.c`'s `dot_compute`, single thread): the first
/// `n & -16` elements by the Haswell kernel `ddot_kernel_8` (`ddot_microk_haswell-2.c`: four
/// accumulators of four lanes, element `16·q + 4·a + l` fused into lane `l` of accumulator `a`;
/// then each accumulator's high and low halves added, accumulators 0 + 1 and 2 + 3, their sum,
/// and its two lanes), the rest added one product at a time.
pub fn ddot_unit_compute(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    assert_eq!(n, y.len());
    let n1 = n & !15;
    let mut dot = 0.0;
    if n1 > 0 {
        let mut acc = [[0.0f64; 4]; 4];
        let mut i = 0;
        while i < n1 {
            for (a, lanes) in acc.iter_mut().enumerate() {
                for (l, v) in lanes.iter_mut().enumerate() {
                    let e = i + 4 * a + l;
                    *v = fma(x[e], y[e], *v);
                }
            }
            i += 16;
        }
        let h: Vec<[f64; 2]> = acc.iter().map(|a| [a[0] + a[2], a[1] + a[3]]).collect();
        let s01 = [h[0][0] + h[1][0], h[0][1] + h[1][1]];
        let s23 = [h[2][0] + h[3][0], h[2][1] + h[3][1]];
        let s = [s01[0] + s23[0], s01[1] + s23[1]];
        dot = s[0] + s[1];
    }
    for i in n1..n {
        dot += y[i] * x[i];
    }
    dot
}

/// `DDOT` with unit strides (`ddot.c`'s `CNAME`) on OpenBLAS's level-1 threads: up to 10000
/// elements, or one thread, [`ddot_unit_compute`]; above, the vector cut into `nthreads` parts
/// (`blas_level1_thread_with_return_value`: each part `ceil(left / threads left)` elements),
/// each part's [`ddot_unit_compute`] added in order to 0. `nthreads` is OpenBLAS's thread
/// count (`OPENBLAS_NUM_THREADS`, else the machine's cores).
pub fn ddot_unit(x: &[f64], y: &[f64], nthreads: usize) -> f64 {
    let n = x.len();
    assert_eq!(n, y.len());
    if n <= 10000 || nthreads <= 1 {
        return ddot_unit_compute(x, y);
    }
    let mut parts = Vec::with_capacity(nthreads);
    let mut start = 0usize;
    let mut left = n as i64;
    let mut num_cpu = 0usize;
    while left > 0 {
        let threads_left = (nthreads - num_cpu) as i64;
        let mut width = (left + threads_left - 1) / threads_left;
        left -= width;
        if left < 0 {
            width += left;
        }
        let w = width as usize;
        parts.push(ddot_unit_compute(
            &x[start..start + w],
            &y[start..start + w],
        ));
        start += w;
        num_cpu += 1;
    }
    let mut dot = 0.0;
    for p in parts {
        dot += p;
    }
    dot
}

/// `DAXPY_K` (`daxpy.c`): with unit strides the first `n & -16` elements by the Haswell kernel
/// `daxpy_kernel_8` (`daxpy_microk_haswell-2.c`: `vfmadd231pd`, `y = fma(x, da, y)`), the rest
/// `y[i] + da·x[i]` with two roundings (the C tail is not contracted: measured on FSL's
/// library); other strides `y[i] + da·x[i]` with two roundings. The kernel computes even for
/// `da == 0` (the interface's `alpha == 0` return is the caller's: [`daxpy`]; `GER` and `SYR`
/// call the kernel directly).
pub fn axpy_k(da: f64, x: &[f64], incx: usize, y: &mut [f64], incy: usize) {
    let n = x.len();
    assert!(y.len() >= n);
    let n1 = if incx == 1 && incy == 1 { n & !15 } else { 0 };
    for i in 0..n1 {
        y[i] = fma(x[i], da, y[i]);
    }
    for i in n1..n {
        y[i] += da * x[i];
    }
}

/// `DAXPY` (`interface/axpy.c`): nothing for `n <= 0` or `alpha == 0`, else [`axpy_k`].
pub fn daxpy(da: f64, x: &[f64], incx: usize, y: &mut [f64], incy: usize) {
    if x.is_empty() || da == 0.0 {
        return;
    }
    axpy_k(da, x, incx, y, incy);
}

/// `DSCAL` (`interface/scal.c`): nothing for `n <= 0` or `alpha == 1`; otherwise every element
/// times `alpha` (the interface's call passes `dummy2 = 1`, so a zero `alpha` multiplies too:
/// OpenBLAS issue 4728).
pub fn dscal(da: f64, x: &mut [f64]) {
    if x.is_empty() || da == 1.0 {
        return;
    }
    for v in x.iter_mut() {
        *v *= da;
    }
}

/// `SCAL_K` as the level-2 interfaces call it for `beta` (`dummy2 = 0`): zeros for `beta == 0`,
/// else the products.
fn scal_beta(beta: f64, y: &mut [f64]) {
    for v in y.iter_mut() {
        *v = if beta == 0.0 { 0.0 } else { *v * beta };
    }
}

/// `DROT` (`interface/rot.c`, Haswell `drot.c`): `x' = fma(c, x, s·y)`, `y' = fma(c, y, −(s·x))`.
/// Unit strides run `drot_microk_haswell-2.c` (`_mm256_fmadd_pd`, `_mm256_fmsub_pd`), other
/// strides the scalar loop as GCC contracts it: the same two roundings (measured on FSL's
/// library, `validation/investigations/fsl-flirt/lapack_eig_svd_probe.py`).
pub fn drot(x: &mut [f64], incx: usize, y: &mut [f64], incy: usize, c: f64, s: f64) {
    let n = x.len();
    assert_eq!(n, y.len());
    let _ = (incx, incy);
    for i in 0..n {
        let temp = fma(c, x[i], s * y[i]);
        y[i] = fma(c, y[i], -(s * x[i]));
        x[i] = temp;
    }
}

/// An x87 extended-precision value (64-bit significand), non-negative and finite: `m · 2^e`
/// with `m`'s top bit set, or zero (`m == 0`).
#[derive(Clone, Copy, Debug)]
struct X87 {
    m: u64,
    e: i32,
}

impl X87 {
    const ZERO: X87 = X87 { m: 0, e: 0 };

    /// Rounds the exact `v · 2^e` (`v > 0`, with `sticky` for bits below `v`) to 64 bits, ties
    /// to even.
    fn round(v: u128, e: i32, sticky: bool) -> X87 {
        let bits = 128 - v.leading_zeros() as i32;
        if bits <= 64 {
            return X87 {
                m: (v << (64 - bits)) as u64,
                e: e - (64 - bits),
            };
        }
        let shift = bits - 64;
        let kept = (v >> shift) as u64;
        let rem = v & ((1u128 << shift) - 1);
        let half = 1u128 << (shift - 1);
        let up = rem > half || (rem == half && (sticky || kept & 1 == 1));
        let (m, e2) = if up {
            match kept.checked_add(1) {
                Some(k) => (k, e + shift),
                None => (1u64 << 63, e + shift + 1),
            }
        } else {
            (kept, e + shift)
        };
        X87 { m, e: e2 }
    }

    /// `fmul %st(0), %st` of a loaded double: `x²` rounded to 64 bits.
    fn square(x: f64) -> X87 {
        if x == 0.0 {
            return X87::ZERO;
        }
        let bits = x.abs().to_bits();
        let exp = (bits >> 52) as i32;
        let frac = bits & ((1u64 << 52) - 1);
        let (m, e) = if exp == 0 {
            (frac, -1074)
        } else {
            (frac | (1u64 << 52), exp - 1075)
        };
        let p = u128::from(m) * u128::from(m);
        X87::round(p, 2 * e, false)
    }

    /// `faddp`: the sum of two non-negative values rounded to 64 bits.
    fn add(a: X87, b: X87) -> X87 {
        if a.m == 0 {
            return b;
        }
        if b.m == 0 {
            return a;
        }
        let (hi, lo) = if a.e >= b.e { (a, b) } else { (b, a) };
        // Align on a common exponent with 63 guard bits below the smaller value's significand
        // at most; bits shifted out further only set the sticky flag.
        let d = (hi.e - lo.e) as u32;
        let hv = u128::from(hi.m) << 63;
        let (lv, sticky) = if d >= 127 {
            (0u128, true)
        } else {
            let full = u128::from(lo.m) << 63;
            let shifted = full >> d;
            (shifted, (shifted << d) != full)
        };
        X87::round(hv + lv, lo.e.max(hi.e) - 63, sticky)
    }

    /// `fsqrt`: the square root rounded to 64 bits.
    fn sqrt(a: X87) -> X87 {
        if a.m == 0 {
            return X87::ZERO;
        }
        // a = m·2^e; scale to an even exponent with the radicand holding 128 bits.
        let mut v = u128::from(a.m) << 64;
        let mut e = a.e - 64;
        if e & 1 != 0 {
            v >>= 1;
            e += 1;
        }
        // v = m·2^64 or m·2^63 (nothing shifted out): its root has exactly 64 bits; it rounds up
        // when the root exceeds r + 1/2, i.e. when v − r² > r (a tie cannot occur).
        let r = isqrt(v);
        let up = v - r * r > r;
        let m = if up { r + 1 } else { r };
        X87::round(m, e / 2, false)
    }

    /// `FST` to a double: rounded to 53 bits (normal results; subnormals rounded at their
    /// fixed exponent).
    fn to_f64(self) -> f64 {
        if self.m == 0 {
            return 0.0;
        }
        // Value = m·2^e, m in [2^63, 2^64).
        let top = self.e + 63; // exponent of the leading bit
        let keep = if top >= -1022 {
            53
        } else {
            (53 - (-1022 - top)).max(0)
        };
        let shift = 64 - keep;
        let kept = if shift >= 64 { 0 } else { self.m >> shift };
        let rem = if shift >= 64 {
            self.m
        } else {
            self.m & ((1u64 << shift) - 1)
        };
        let half = if shift >= 65 { 0 } else { 1u64 << (shift - 1) };
        let mut k = kept;
        if rem > half || (rem == half && k & 1 == 1) {
            k += 1;
        }
        let scale = self.e + shift;
        (k as f64) * 2f64.powi(scale)
    }
}

/// Integer square root (floor) of a `u128`.
fn isqrt(v: u128) -> u128 {
    v.isqrt()
}

/// `DNRM2` (`interface/nrm2.c`, x86-64 `nrm2.S` on the x87 unit): 0 for no elements, `|x0|` for
/// one; otherwise the squares in extended precision summed in four accumulators (element `i`
/// of each group of eight into accumulator `i mod 4`, the rest into the first), the
/// accumulators added as `((a2 + a0) + a1) + a3`, the square root in extended precision, then
/// rounded to a double. The same for every increment.
pub fn dnrm2(x: &[f64], incx: usize) -> f64 {
    let n = x.len();
    let _ = incx;
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return x[0].abs();
    }
    let mut acc = [X87::ZERO; 4];
    let groups = n / 8;
    for g in 0..groups {
        for half in 0..2 {
            let b = g * 8 + half * 4;
            // faddp order inside a half: a3, a2, a1, a0 (each a separate accumulator).
            for k in (0..4).rev() {
                acc[k] = X87::add(acc[k], X87::square(x[b + k]));
            }
        }
    }
    for &v in &x[groups * 8..] {
        acc[0] = X87::add(acc[0], X87::square(v));
    }
    let s = X87::add(X87::add(X87::add(acc[2], acc[0]), acc[1]), acc[3]);
    X87::sqrt(s).to_f64()
}

/// The matrix operand of a level-2 or level-3 routine: element `(r, c)` of the column-major
/// array is `a[off + c·ld + r]`.
#[derive(Clone, Copy, Debug)]
pub struct MatRef {
    pub off: usize,
    pub ld: usize,
}

impl MatRef {
    #[inline(always)]
    pub fn at(&self, r: usize, c: usize) -> usize {
        self.off + c * self.ld + r
    }
}

/// `DGEMV` with no transpose (`interface/gemv.c`, Haswell `dgemv_n_4.c`): `y = beta·y + alpha·A·x`
/// for the `m × n` matrix `A`.
///
/// After `beta`'s scaling, rows in blocks of four (`m & -4`) run the kernels over columns in
/// groups: four columns (`dgemv_kernel_4x4`: `t = fma(a2, x2, a0·x0) + fma(a3, x3, a1·x1)`, then
/// `y = fma(alpha, t, y)`), then two (`dgemv_kernel_4x2`: `y = fma(alpha, a0·x0 + a1·x1, y)`),
/// then one (`dgemv_kernel_4x1`: `y + a·(x·alpha)`); a strided `x` runs the remaining columns one
/// at a time. The last `m & 3` rows sum their products from 0 (`lda == m & 3` with a unit `x`
/// adds them in pairs) and add `alpha·t` to `y`.
#[allow(clippy::too_many_arguments)]
pub fn dgemv_n(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    am: MatRef,
    x: &[f64],
    incx: usize,
    beta: f64,
    y: &mut [f64],
) {
    if m == 0 || n == 0 {
        return;
    }
    if beta != 1.0 {
        scal_beta(beta, &mut y[..m]);
    }
    if alpha == 0.0 {
        return;
    }
    let el = |r: usize, c: usize| a[am.at(r, c)];
    let m1 = m & !3;
    let m3 = m & 3;
    // The blocks of four rows (NB = m1 when m < 2048).
    if m1 > 0 {
        assert!(m < 2048, "dgemv_n beyond NBMAX rows is not ported");
        let n1 = n >> 2;
        let n2 = n & 3;
        let mut col = 0;
        for _ in 0..n1 {
            for r in 0..m1 {
                let mut t4 = el(r, col) * x[col];
                let mut t5 = el(r, col + 1) * x[col + 1];
                t4 = fma(el(r, col + 2), x[col + 2], t4);
                t5 = fma(el(r, col + 3), x[col + 3], t5);
                let t = t5 + t4;
                y[r] = fma(alpha, t, y[r]);
            }
            col += 4;
        }
        if incx == 1 {
            if n2 & 2 != 0 {
                for r in 0..m1 {
                    let t4 = el(r, col) * x[col];
                    let t5 = el(r, col + 1) * x[col + 1];
                    y[r] = fma(alpha, t5 + t4, y[r]);
                }
                col += 2;
            }
            if n2 & 1 != 0 {
                let xa = x[col] * alpha;
                for r in 0..m1 {
                    y[r] += el(r, col) * xa;
                }
            }
        } else {
            for _ in 0..n2 {
                let xa = x[col] * alpha;
                for r in 0..m1 {
                    y[r] += el(r, col) * xa;
                }
                col += 1;
            }
        }
    }
    if m3 == 0 {
        return;
    }
    let pairs = am.ld == m3 && incx == 1;
    let mut t = [0.0f64; 3];
    if pairs && m3 >= 2 {
        // `lda == 3` or `lda == 2` with a unit x: products added two at a time.
        let n4 = n & !3;
        let mut i = 0;
        while i < n4 {
            for r in 0..m3 {
                t[r] += el(m1 + r, i) * x[i] + el(m1 + r, i + 1) * x[i + 1];
            }
            for r in 0..m3 {
                t[r] += el(m1 + r, i + 2) * x[i + 2] + el(m1 + r, i + 3) * x[i + 3];
            }
            i += 4;
        }
        while i < n {
            for r in 0..m3 {
                t[r] += el(m1 + r, i) * x[i];
            }
            i += 1;
        }
    } else if pairs && m3 == 1 {
        let n4 = n & !3;
        let mut i = 0;
        while i < n4 {
            t[0] += el(m1, i) * x[i]
                + el(m1, i + 1) * x[i + 1]
                + el(m1, i + 2) * x[i + 2]
                + el(m1, i + 3) * x[i + 3];
            i += 4;
        }
        while i < n {
            t[0] += el(m1, i) * x[i];
            i += 1;
        }
    } else {
        for i in 0..n {
            for r in 0..m3 {
                t[r] += el(m1 + r, i) * x[i];
            }
        }
    }
    for r in 0..m3 {
        y[m1 + r] += alpha * t[r];
    }
}

/// `DGEMV` transposed (`interface/gemv.c`, Haswell `dgemv_t_4.c`): `y = beta·y + alpha·Aᵀ·x` for
/// the `m × n` matrix `A` (`x` has `m` elements, `y` has `n`).
///
/// After `beta`'s scaling, the rows in blocks of four (`m & -4`) run the kernels for each column:
/// groups of four columns (`dgemv_kernel_4x4`: four FMA lanes, row `r` into lane `r mod 4`,
/// reduced as `(l0 + l2) + (l1 + l3)`), a pair (`dgemv_kernel_4x2`: two SSE lanes of rounded
/// products, even and odd rows, `l0 + l1`), a single column (`dgemv_kernel_4x1`: lanes for rows
/// `r mod 4`, reduced as `(l0 + l2) + (l1 + l3)`); each total `t` is added as `y + t·alpha`. The
/// last `m & 3` rows add `((a0·x0α + a1·x1α) + a2·x2α)` to each `y` with `xα = x·alpha`.
#[allow(clippy::too_many_arguments)]
pub fn dgemv_t(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    am: MatRef,
    x: &[f64],
    incx: usize,
    beta: f64,
    y: &mut [f64],
) {
    dgemv_t_inc(m, n, alpha, a, am, x, incx, beta, y, 1);
}

/// [`dgemv_t`] with `y`'s increment `incy` (`y` gathered): a `y` of increment other than 1 takes
/// `dgemv_t_4.c`'s C loop of `add_y` for the totals of the groups of four columns, which GCC
/// contracts (`y = fma(t, alpha, y)`; the unit increment's `add_y` is assembly: `y + t·alpha`
/// with two roundings, measured on FSL's library); every other path is the same for every
/// increment.
#[allow(clippy::too_many_arguments)]
pub fn dgemv_t_inc(
    m: usize,
    n: usize,
    alpha: f64,
    a: &[f64],
    am: MatRef,
    x: &[f64],
    incx: usize,
    beta: f64,
    y: &mut [f64],
    incy: usize,
) {
    let _ = incx;
    if m == 0 || n == 0 {
        return;
    }
    if beta != 1.0 {
        scal_beta(beta, &mut y[..n]);
    }
    if alpha == 0.0 {
        return;
    }
    let el = |r: usize, c: usize| a[am.at(r, c)];
    let m1 = m & !3;
    let m3 = m & 3;
    if m1 > 0 {
        assert!(m < 2048 && n < 2048, "dgemv_t beyond NBMAX is not ported");
        let n1 = n >> 2;
        let n2 = n & 3;
        let lanes4 = |c: usize| -> f64 {
            let mut l = [0.0f64; 4];
            for r in 0..m1 {
                l[r & 3] = fma(el(r, c), x[r], l[r & 3]);
            }
            (l[2] + l[0]) + (l[3] + l[1])
        };
        let mut col = 0;
        for _ in 0..n1 {
            let t: Vec<f64> = (0..4).map(|k| lanes4(col + k)).collect();
            for k in 0..4 {
                y[col + k] = if incy == 1 {
                    y[col + k] + t[k] * alpha
                } else {
                    fma(t[k], alpha, y[col + k])
                };
            }
            col += 4;
        }
        if n2 & 2 != 0 {
            for k in 0..2 {
                let mut l = [0.0f64; 2];
                for r in 0..m1 {
                    l[r & 1] += el(r, col + k) * x[r];
                }
                let t = l[0] + l[1];
                // `*y_ptr += ybuffer[k] * alpha`: contracted by GCC.
                y[col + k] = fma(t, alpha, y[col + k]);
            }
            col += 2;
        }
        if n2 & 1 != 0 {
            // xmm10 lanes take rows 4k and 4k+1, xmm9 lanes rows 4k+2 and 4k+3; then
            // xmm10 + xmm9 and the horizontal sum.
            let mut l10 = [0.0f64; 2];
            let mut l9 = [0.0f64; 2];
            for r in 0..m1 {
                let p = el(r, col) * x[r];
                if r & 2 == 0 {
                    l10[r & 1] += p;
                } else {
                    l9[r & 1] += p;
                }
            }
            let s0 = l10[0] + l9[0];
            let s1 = l10[1] + l9[1];
            y[col] = fma(s0 + s1, alpha, y[col]);
        }
    }
    if m3 == 0 {
        return;
    }
    // The C tail is contracted by GCC (`dgemv_t_4.c` is built with FMA available, unlike
    // `dgemv_n_4.c`'s tails, measured on FSL's library): `y += a0·x0α` is one FMA,
    // `y += a0·x0α + a1·x1α` is `y + fma(a0, x0α, a1·x1α)`, the three-row sum
    // `y + fma(a2, x2α, fma(a0, x0α, a1·x1α))`.
    let xt: Vec<f64> = (0..m3).map(|k| x[m1 + k] * alpha).collect();
    for j in 0..n {
        let a0 = el(m1, j);
        y[j] = match m3 {
            1 => fma(a0, xt[0], y[j]),
            2 => y[j] + fma(a0, xt[0], el(m1 + 1, j) * xt[1]),
            _ => {
                let s = fma(a0, xt[0], el(m1 + 1, j) * xt[1]);
                y[j] + fma(el(m1 + 2, j), xt[2], s)
            }
        };
    }
}

/// `DGER` (`interface/ger.c`, `../generic/ger.c`): `A += alpha·x·yᵀ` for the `m × n` matrix `A`,
/// one column at a time by `daxpy`'s kernel ([`axpy_k`] with unit strides: a strided `x` is
/// copied to a buffer first, and every column of `A` is contiguous) with `alpha·y[j]` (a zero
/// `y[j]` included); nothing when `m`, `n` or `alpha` is zero. One thread whatever the size
/// (the threaded driver is compiled only under `SMPTEST`). `x` and `y` are gathered; `y`'s
/// increment does not change any value.
pub fn dger(alpha: f64, x: &[f64], y: &[f64], a: &mut [f64], am: MatRef) {
    let m = x.len();
    let n = y.len();
    if m == 0 || n == 0 || alpha == 0.0 {
        return;
    }
    for j in 0..n {
        let da = alpha * y[j];
        let s = am.at(0, j);
        axpy_k(da, x, 1, &mut a[s..s + m], 1);
    }
}

/// `DSYR` with the lower triangle and a unit increment (`interface/syr.c`'s path for fewer than
/// 100 rows): `A += alpha·x·xᵀ` on the lower triangle of the `n × n` matrix `A`, column `i` by
/// `daxpy`'s kernel ([`axpy_k`]) adding `alpha·x[i]·x[i..n]` to its rows `i..n`, skipped when
/// `x[i]` is zero; nothing when `n` or `alpha` is zero.
pub fn dsyr_l(alpha: f64, x: &[f64], a: &mut [f64], am: MatRef) {
    let n = x.len();
    if n == 0 || alpha == 0.0 {
        return;
    }
    assert!(
        n < 100,
        "dsyr's driver for 100 or more rows (driver/level2/syr_k.c) is not ported"
    );
    for i in 0..n {
        if x[i] != 0.0 {
            let s = am.at(i, i);
            axpy_k(alpha * x[i], &x[i..], 1, &mut a[s..s + n - i], 1);
        }
    }
}

/// `DSYMV` with the upper triangle (`interface/symv.c`, Haswell `dsymv_U.c` for fewer than 16
/// rows): `y = beta·y + alpha·A·x`. For each column `j`: `t1 = alpha·x[j]`, the rows above add
/// `t1·a` to `y` and `a·x` to `t2`, then `y[j] += t1·a[j][j] + alpha·t2`.
pub fn dsymv_u(alpha: f64, a: &[f64], am: MatRef, x: &[f64], beta: f64, y: &mut [f64]) {
    let n = x.len();
    if n == 0 {
        return;
    }
    assert!(
        n < 16,
        "dsymv_U's 4 x 4 kernel (16 or more rows) is not ported"
    );
    if beta != 1.0 {
        scal_beta(beta, &mut y[..n]);
    }
    if alpha == 0.0 {
        return;
    }
    for j in 0..n {
        let temp1 = alpha * x[j];
        let mut temp2 = 0.0f64;
        for i in 0..j {
            let aij = a[am.at(i, j)];
            y[i] += temp1 * aij;
            temp2 += aij * x[i];
        }
        y[j] += temp1 * a[am.at(j, j)] + alpha * temp2;
    }
}

/// `DSYMV` with the lower triangle (`interface/symv.c`, one thread; Haswell `dsymv_L.c`):
/// `y = beta·y + alpha·A·x` for the symmetric `n × n` matrix whose lower triangle `a` holds.
///
/// The columns in groups of four (`t1 = alpha·x[j..j+4]`): each diagonal term added to `y`, then
/// the rows below the group's corner. With at least 12 rows below the group's first column, the
/// corner's rows by the C loops, the rows up to the last multiple of four by the assembly kernel
/// `dsymv_kernel_4x4` (`dsymv_L_microk_haswell-2.c`: for each row the four columns' `y =
/// fma(t1, a, y)` in order, and each column's `a·x` fused into four lanes by row modulo 4,
/// reduced as `(l0 + l2) + (l1 + l3)` and added to the corner's sum), the last rows by the C
/// loop; otherwise every row by the C loops (each row's four columns interleaved: `y += t1·a`,
/// `t2 += a·x`, two roundings: the C is not contracted, disassembled); then `y[j+c] +=
/// alpha·t2[c]`. The last `n mod 4` columns one at a time by the C loop.
pub fn dsymv_l(alpha: f64, a: &[f64], am: MatRef, x: &[f64], beta: f64, y: &mut [f64]) {
    let m = x.len();
    if m == 0 {
        return;
    }
    if beta != 1.0 {
        scal_beta(beta, &mut y[..m]);
    }
    if alpha == 0.0 {
        return;
    }
    let el = |r: usize, c: usize| a[am.at(r, c)];
    let offset1 = m / 4 * 4;
    let mut j = 0;
    while j < offset1 {
        let tmp1 = [
            alpha * x[j],
            alpha * x[j + 1],
            alpha * x[j + 2],
            alpha * x[j + 3],
        ];
        let mut tmp2 = [0.0f64; 4];
        for c in 0..4 {
            y[j + c] += tmp1[c] * el(j + c, j + c);
        }
        // the corner: column c's rows j+c+1 .. j+4
        for c in 0..3 {
            for i in j + c + 1..j + 4 {
                let v = el(i, j + c);
                y[i] += tmp1[c] * v;
                tmp2[c] += v * x[i];
            }
        }
        let from = j + 1;
        let mut i = j + 4;
        if m - from >= 12 {
            let m2 = m / 4 * 4;
            if m2 > j + 4 {
                let mut lanes = [[0.0f64; 4]; 4];
                while i < m2 {
                    for l in 0..4 {
                        let r = i + l;
                        let mut yv = y[r];
                        for c in 0..4 {
                            let v = el(r, j + c);
                            yv = fma(tmp1[c], v, yv);
                            lanes[c][l] = fma(x[r], v, lanes[c][l]);
                        }
                        y[r] = yv;
                    }
                    i += 4;
                }
                for c in 0..4 {
                    let s = (lanes[c][0] + lanes[c][2]) + (lanes[c][1] + lanes[c][3]);
                    tmp2[c] += s;
                }
            }
        }
        while i < m {
            for c in 0..4 {
                let v = el(i, j + c);
                y[i] += tmp1[c] * v;
                tmp2[c] += v * x[i];
            }
            i += 1;
        }
        for c in 0..4 {
            y[j + c] += alpha * tmp2[c];
        }
        j += 4;
    }
    for j in offset1..m {
        let temp1 = alpha * x[j];
        let mut temp2 = 0.0f64;
        y[j] += temp1 * el(j, j);
        for i in j + 1..m {
            let v = el(i, j);
            y[i] += temp1 * v;
            temp2 += v * x[i];
        }
        y[j] += alpha * temp2;
    }
}

/// `DSYR2` with the upper triangle (`interface/syr2.c`): for each column `i`, `daxpy`'s kernel
/// adds `alpha·x[i]·y[0..=i]`, then `alpha·y[i]·x[0..=i]`, to the column's top `i + 1` rows.
pub fn dsyr2_u(alpha: f64, x: &[f64], y: &[f64], a: &mut [f64], am: MatRef) {
    let n = x.len();
    if n == 0 || alpha == 0.0 {
        return;
    }
    assert!(n < 16, "dsyr2's columns of 16 or more rows are not ported");
    for i in 0..n {
        let dx = alpha * x[i];
        for r in 0..=i {
            let k = am.at(r, i);
            a[k] += dx * y[r];
        }
        let dy = alpha * y[i];
        for r in 0..=i {
            let k = am.at(r, i);
            a[k] += dy * x[r];
        }
    }
}

/// `GEMM_Q` for Haswell's double precision (`param.h`): the depth of one block of the inner
/// dimension.
const GEMM_Q: usize = 256;
/// `GEMM_UNROLL_M` for Haswell's double precision.
const GEMM_UNROLL_M: usize = 4;
/// `GEMM_P` for Haswell's double precision: the rows of one block.
const GEMM_P: usize = 512;

/// `DGEMM` (`interface/gemm.c`, `driver/level3/level3.c`, Haswell `dgemm_kernel_4x8_haswell.S`):
/// `C = beta·C + alpha·op(A)·op(B)` for `op(A)` of `m × k`, `op(B)` of `k × n`.
///
/// No small-matrix path (Haswell's permit refuses every size) and no GEMV forwarding on x86-64:
/// `beta` first (`dgemm_beta`: zeros for 0, the products otherwise, nothing for 1), nothing more
/// when `k` or `alpha` is 0; then the inner dimension in blocks (`GEMM_Q`, the level-3 driver's
/// split of a remainder between `Q` and `2Q`), each element's block total accumulated by
/// `vmulpd` then `vfmadd231pd` in the order of `k` (one accumulator per element in every tile
/// shape but the 4-row, 1-column tile of an odd last column, which keeps four partial sums:
/// [`super::blocked::ksum`]), then added as `C + acc·alpha`.
#[allow(clippy::too_many_arguments)]
pub fn dgemm(
    ta: bool,
    tb: bool,
    m: usize,
    n: usize,
    k: usize,
    alpha: f64,
    a: &[f64],
    am: MatRef,
    b: &[f64],
    bm: MatRef,
    beta: f64,
    c: &mut [f64],
    cm: MatRef,
) {
    if m == 0 || n == 0 {
        return;
    }
    if beta != 1.0 {
        for j in 0..n {
            for i in 0..m {
                let p = cm.at(i, j);
                c[p] = if beta == 0.0 { 0.0 } else { c[p] * beta };
            }
        }
    }
    if k == 0 || alpha == 0.0 {
        return;
    }
    let ael = |i: usize, l: usize| if ta { a[am.at(l, i)] } else { a[am.at(i, l)] };
    let bel = |l: usize, j: usize| if tb { b[bm.at(j, l)] } else { b[bm.at(l, j)] };
    let mut ls = 0;
    while ls < k {
        let mut min_l = k - ls;
        if min_l >= GEMM_Q * 2 {
            min_l = GEMM_Q;
        } else if min_l > GEMM_Q {
            min_l = (min_l / 2).div_ceil(GEMM_UNROLL_M) * GEMM_UNROLL_M;
        }
        // The rows in chunks of GEMM_P (level3.c's rule), each element summed as its tile of
        // the kernel sums it (`blocked::tile_sum`: one accumulator, but four partial sums in
        // the 4-row, 1-column tile of an odd last column).
        let mut is = 0;
        while is < m {
            let mut min_i = m - is;
            if min_i >= GEMM_P * 2 {
                min_i = GEMM_P;
            } else if min_i > GEMM_P {
                min_i = (min_i / 2).div_ceil(GEMM_UNROLL_M) * GEMM_UNROLL_M;
            }
            for j in 0..n {
                for i in is..is + min_i {
                    let kind = super::blocked::tile_sum(i - is, j, min_i, n, min_l);
                    let acc =
                        super::blocked::ksum(kind, min_l, |l| (ael(i, ls + l), bel(ls + l, j)));
                    let p = cm.at(i, j);
                    c[p] += acc * alpha;
                }
            }
            is += min_i;
        }
        ls += min_l;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `validation/probes/fsl-basisfield/ddot_threads.c`'s vectors.
    fn xorshift_vectors(n: usize) -> (Vec<f64>, Vec<f64>) {
        let mut s: u64 = 88172645463325252;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let mut x = Vec::with_capacity(n);
        let mut y = Vec::with_capacity(n);
        for _ in 0..n {
            x.push((next() % 100000) as f64 / 977.0 - 50.0);
            y.push((next() % 100000) as f64 / 1013.0 - 40.0);
        }
        (x, y)
    }

    #[test]
    fn ddot_unit_is_fsls() {
        // FSL 6.0.7.23's ddot (OpenBLAS 0.3.33, Haswell), OPENBLAS_NUM_THREADS = 1, 2, 4, 32
        // (ddot_threads.c, 2026-11-03).
        let cases: [(usize, [u64; 4]); 5] = [
            (33, [0xc08b4ef4e6416408; 4]),
            (100, [0x408ab33ce3c749ae; 4]),
            (
                10001,
                [
                    0x40f373f36a1a26c5,
                    0x40f373f36a1a26c6,
                    0x40f373f36a1a26c8,
                    0x40f373f36a1a26c7,
                ],
            ),
            (
                10560,
                [
                    0x40fc36a8fa57e104,
                    0x40fc36a8fa57e108,
                    0x40fc36a8fa57e108,
                    0x40fc36a8fa57e108,
                ],
            ),
            (
                50000,
                [
                    0x411f944a4c478cb0,
                    0x411f944a4c478cac,
                    0x411f944a4c478caf,
                    0x411f944a4c478caf,
                ],
            ),
        ];
        for (n, want) in cases {
            let (x, y) = xorshift_vectors(n);
            for (t, w) in [1usize, 2, 4, 32].iter().zip(want) {
                assert_eq!(ddot_unit(&x, &y, *t).to_bits(), w, "n {n}, {t} threads");
            }
        }
    }

    #[test]
    fn nrm2_small_cases() {
        assert_eq!(dnrm2(&[], 1), 0.0);
        assert_eq!(dnrm2(&[-3.0], 1), 3.0);
        assert_eq!(dnrm2(&[3.0, 4.0], 1), 5.0);
        assert_eq!(dnrm2(&[0.0, 0.0, 0.0], 1), 0.0);
        let v: Vec<f64> = (1..=12).map(f64::from).collect();
        // sqrt(650): the extended-precision sum is exact, the root rounded twice.
        assert_eq!(dnrm2(&v, 1), 650f64.sqrt());
    }

    #[test]
    fn x87_rounding() {
        // 1 + 2^-64 rounds to 1 in 64 bits (tie to even), 1 + 3·2^-65 rounds up.
        let one = X87::square(1.0);
        let tiny = X87 {
            m: 1u64 << 63,
            e: -64 - 63,
        };
        assert_eq!(X87::add(one, tiny).m, 1u64 << 63);
        let s = X87::sqrt(X87::square(3.0));
        assert_eq!(s.to_f64(), 3.0);
    }
}

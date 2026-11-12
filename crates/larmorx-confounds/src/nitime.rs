// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! nitime's autocorrelation and its Yule-Walker AR(1) estimate as nipype's DVARS calls them
//! (nitime 0.12.1 `utils.py`: `crosscov`, `autocov`, `autocorr`, `fftconvolve`;
//! `algorithms/autoregressive.py`: `AR_est_YW`; numerically the same in nitime 0.11, fMRIPrep
//! 25.2.6's), with the arithmetic of the libraries under them, bit for bit:
//!
//! - **`fftconvolve(x, y[::-1].conj(), axis=-1, mode='full')`** of two float32 series of length
//!   `n`: `size = 2 n - 1`, `fsize = 2 ** ceil(log2(size))` (a power of two; `1` for `n = 1`),
//!   `IN1 = fftpack.fft(x, fsize)` and `fftpack.fft(y[::-1], fsize)` (real float32 input,
//!   zero-padded: SciPy's [`RealFft`]), `IN1 *= ...` (numpy's complex64 product: [`cmul`]),
//!   `fftpack.ifft(IN1)[:size].real` (SciPy's [`ComplexFft`], scaled by `1 / fsize`).
//! - **`crosscov(x, y, debias=False)`** divides that by `n` in float32 (`cxy /= N`: numpy's
//!   weak Python integer becomes a float32), and keeps the lags `0 .. n` (`[n - 1 : 2 n - 1]`)
//!   unless `all_lags`; `autocorr(x)` is `crosscov(x, x, debias=False)`.
//! - **`AR_est_YW(x, 1)`**: `r = autocorr(x)[:2]`, `ak = scipy.linalg.solve(toeplitz(r[:1]),
//!   r[1:])` ([`scipy_solve_1x1`]: each SciPy release's 1 × 1 path), `sigma_v = r[0] -
//!   np.dot(r[1:], ak)` (float32: OpenBLAS's `sdot` of one element, the product rounded to
//!   float32). nipype keeps `ak` only (`_AR_est_YW`).
//!
//! **numpy's complex64 product** ([`cmul`]): numpy's SIMD loop (`loops_arithm_fp.dispatch.c.src`,
//! `simd_cmul_f32`: `npyv_muladdsub_f32(a_re, b, a_im · b_rev)`) on x86-64 with AVX2 and FMA3
//! (both oracle machines; also AVX-512) computes `re = fma(a.re, b.re, -(a.im · b.im))` and `im =
//! fma(a.re, b.im, a.im · b.re)`, the second products rounded once; without FMA it would round
//! all four products (`NPY_DISABLE_CPU_FEATURES`; 26 % of products differ:
//! `validation/investigations/confounds/probe_cmul.py`). larmorx computes the fused form on every
//! platform. A NaN result takes the first NaN operand in the order `a.re`, `b.re`, `a.im · b.im`
//! (real part) or `a.re`, `b.im`, `a.im · b.re` (imaginary part), otherwise x86's default NaN; the
//! operand order of the hardware's fused instruction is the compiler's, so a NaN's sign may
//! differ from numpy's (finite inputs never reach it).
//!
//! Upstream: nitime 0.12.1 (PyPI sdist `nitime-0.12.1.tar.gz`, SHA-256
//! `12cbf488b0655dc05aa18a0ca5982de970302604e461fc92e87ebd9b7539b6ff`), Copyright (c) 2006-2011,
//! NIPY Developers, BSD-3-Clause; SciPy 1.18.1 and 1.15.3 `scipy/linalg/_basic.py` (`solve`,
//! `_find_matrix_structure`, `_matrix_norm_diagonal`, `_solve_check`), `_special_matrices.py`
//! (`toeplitz`), Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers, BSD-3-Clause;
//! numpy 2.5.4 `numpy/_core/src/umath/loops_arithm_fp.dispatch.c.src` (`simd_cmul_f32`),
//! `numpy/_core/src/common/arraytypes.c.src` (`FLOAT_dot`), Copyright (c) 2005-2025, NumPy
//! Developers, BSD-3-Clause. Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the conditions of the BSD 3-Clause licence are met
//! (the full terms are in `PROVENANCE.md` and NOTICE). The logic is re-expressed in Rust; no
//! code was copied.

use crate::scipy_fft::{C32, ComplexFft, FftError, FftLibrary, RealFft};
use larmorx_core::numpy::NpFloat;

/// x86's default NaN (the result of an invalid operation without a NaN operand).
const DEFAULT_NAN: f32 = f32::from_bits(0xffc0_0000);

/// A NaN operand quieted, as x86 passes it on.
#[inline(always)]
fn quiet(x: f32) -> f32 {
    f32::from_bits(x.to_bits() | 0x0040_0000)
}

/// `fma(a, b, c)` with one rounding, x86's NaN rules (the first NaN operand, else the default
/// NaN for an invalid operation).
#[inline(always)]
fn fma(a: f32, b: f32, c: f32) -> f32 {
    if a.is_nan() {
        return quiet(a);
    }
    if b.is_nan() {
        return quiet(b);
    }
    if c.is_nan() {
        return quiet(c);
    }
    let r = a.mul_add(b, c);
    if r.is_nan() { DEFAULT_NAN } else { r }
}

/// numpy's complex64 product `a * b` on x86-64 with AVX2 and FMA3 (module documentation).
#[inline]
pub fn cmul(a: C32, b: C32) -> C32 {
    let bd = NpFloat::mul(a.im, b.im);
    let bc = NpFloat::mul(a.im, b.re);
    // fmaddsub's even lane: a.re · b.re - bd (the subtraction inside the fused operation).
    let re = fma(a.re, b.re, -bd);
    let im = fma(a.re, b.im, bc);
    C32::new(re, im)
}

/// numpy's scalar complex64 loop: `a_r*b_r - a_i*b_i`, `a_r*b_i + a_i*b_r`, each product
/// rounded (no fused operation).
#[inline]
pub fn cmul_plain(a: C32, b: C32) -> C32 {
    let re = NpFloat::sub(NpFloat::mul(a.re, b.re), NpFloat::mul(a.im, b.im));
    let im = NpFloat::add(NpFloat::mul(a.re, b.im), NpFloat::mul(a.im, b.re));
    C32::new(re, im)
}

/// `a *= b` element by element (numpy's in-place complex64 product): [`cmul`], but for arrays of
/// one element, which numpy multiplies in its scalar loop ([`cmul_plain`]: the iterator gives
/// the one-element output a stride of 0, which the SIMD loop refuses; measured in numpy 2.2.6,
/// 2.3.5 and 2.5.4, `validation/investigations/confounds/probe_cmul_length.py`). nitime meets
/// it for a series of one value (an FFT of length 1).
///
/// # Panics
/// When the lengths differ.
pub fn cmul_inplace(a: &mut [C32], b: &[C32]) {
    assert_eq!(a.len(), b.len(), "the operands' lengths differ");
    if a.len() == 1 {
        a[0] = cmul_plain(a[0], b[0]);
        return;
    }
    for (x, &y) in a.iter_mut().zip(b) {
        *x = cmul(*x, y);
    }
}

/// nitime's FFT length for two series of length `n`: the power of two `2 ** ceil(log2(2 n -
/// 1))` (`1` for `n = 1`).
pub fn fft_size(n: usize) -> usize {
    (2 * n - 1).next_power_of_two()
}

/// The FFT plans of nitime's `crosscov` for series of one length, reusable across series (the
/// DVARS loop builds one per run).
#[derive(Clone, Debug)]
pub struct CrosscovPlan {
    n: usize,
    real: RealFft,
    complex: ComplexFft,
}

impl CrosscovPlan {
    /// The plans of SciPy `library` for series of length `n` (at least one; the FFT length is
    /// [`fft_size`]`(n)`).
    ///
    /// # Errors
    /// [`FftError::InvalidLength`] for `n = 0`; [`FftError::TooLong`] when ducc0 would compute
    /// the inverse transform by Bailey's algorithm (`n > 32768`: an FFT length of 131072).
    pub fn new(library: FftLibrary, n: usize) -> Result<CrosscovPlan, FftError> {
        if n == 0 {
            return Err(FftError::InvalidLength(0));
        }
        let fsize = fft_size(n);
        Ok(CrosscovPlan {
            n,
            real: RealFft::new(library, fsize)?,
            complex: ComplexFft::new(library, fsize)?,
        })
    }

    /// The series' length.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the series are empty (never: a plan has a length of at least one).
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The FFT length.
    pub fn fft_size(&self) -> usize {
        self.real.len()
    }

    /// `fftconvolve(x, y[::-1].conj(), axis=-1, mode='full')`: the `2 n - 1` real parts.
    ///
    /// # Panics
    /// When `x` or `y` is not of the plan's length.
    pub fn fftconvolve_reversed(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let n = self.n;
        assert!(
            x.len() == n && y.len() == n,
            "the series are of the plan's length"
        );
        let fsize = self.fft_size();
        let mut buf = vec![0.0f32; fsize];
        buf[..n].copy_from_slice(x);
        let mut in1 = self.real.transform(&buf, true);
        for (b, &v) in buf.iter_mut().zip(y.iter().rev()) {
            *b = v;
        }
        let in2 = self.real.transform(&buf, true);
        cmul_inplace(&mut in1, &in2);
        self.complex.transform(&mut in1, false);
        in1[..2 * n - 1].iter().map(|v| v.re).collect()
    }

    /// `crosscov(x, y, debias=False, all_lags=True)`: the `2 n - 1` lags `-(n - 1) .. n`, divided
    /// by `n` in float32.
    pub fn crosscov_all_lags(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let d = self.n as f32;
        let mut c = self.fftconvolve_reversed(x, y);
        for v in c.iter_mut() {
            *v = NpFloat::div(*v, d);
        }
        c
    }

    /// `crosscov(x, y, debias=False)` (`all_lags=False`): the lags `0 .. n`.
    pub fn crosscov(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let mut c = self.crosscov_all_lags(x, y);
        c.drain(..self.n - 1);
        c
    }

    /// `autocorr(x)[:2]`: the lags 0 and, for `n >= 2`, 1.
    pub fn autocorr_01(&self, x: &[f32]) -> (f32, Option<f32>) {
        let n = self.n;
        let d = n as f32;
        let c = self.fftconvolve_reversed(x, x);
        let r0 = NpFloat::div(c[n - 1], d);
        let r1 = (n >= 2).then(|| NpFloat::div(c[n], d));
        (r0, r1)
    }
}

/// The SciPy release whose `scipy.linalg.solve` path a 1 × 1 system takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SolveRelease {
    /// SciPy 1.18.1 (`a_is_scalar`: `LinAlgError("A singular matrix detected.")` for `a == 0`,
    /// `b1 / a1` otherwise; both arrays checked for infinities and NaNs first). The default.
    #[default]
    V1_18,
    /// SciPy 1.15 (1.15.3, 1.15.2: a 1 × 1 matrix is "diagonal", `x = (b.T / diag(a)).T`; only
    /// `a`'s diagonal is checked for infinities and NaNs, through `_matrix_norm_diagonal`).
    V1_15,
}

impl SolveRelease {
    /// The release of a SciPy version string (`1.15.x`, `1.18.x`); `None` for the others.
    pub fn from_scipy_version(version: &str) -> Option<SolveRelease> {
        match FftLibrary::from_scipy_version(version)? {
            FftLibrary::Ducc0 => Some(SolveRelease::V1_18),
            FftLibrary::Pocketfft => Some(SolveRelease::V1_15),
        }
    }
}

/// How SciPy's `solve` of a 1 × 1 system fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SolveError {
    /// `ValueError` from `check_finite`.
    #[error("array must not contain infs or NaNs")]
    NotFinite,
    /// SciPy 1.18's `LinAlgError` for a zero matrix.
    #[error("A singular matrix detected.")]
    Singular,
    /// SciPy 1.15's `ValueError` for an empty right-hand side (`AR_est_YW` of a series of one
    /// value).
    #[error("Input b has to have same number of rows as input a")]
    RowsMismatch,
}

/// `scipy.linalg.solve([[a]], [b])` of float32 values as SciPy `release` computes it: `b / a` in
/// float32 (SciPy 1.15: inf or NaN for `a == 0`, with RuntimeWarnings only; an infinity or NaN
/// in `b` passes), or the release's error.
///
/// # Errors
/// [`SolveError::NotFinite`] (SciPy 1.15: `a` not finite; 1.18: `a` or `b`),
/// [`SolveError::Singular`] (SciPy 1.18, `a == 0`).
pub fn scipy_solve_1x1(release: SolveRelease, a: f32, b: f32) -> Result<f32, SolveError> {
    match release {
        SolveRelease::V1_15 => {
            if !a.is_finite() {
                return Err(SolveError::NotFinite);
            }
        }
        SolveRelease::V1_18 => {
            if !a.is_finite() || !b.is_finite() {
                return Err(SolveError::NotFinite);
            }
            if a == 0.0 {
                return Err(SolveError::Singular);
            }
        }
    }
    Ok(NpFloat::div(b, a))
}

/// `AR_est_YW(x, 1)` of a float32 series: `(ak, sigma_v)`, `ak` `None` for a series of one value
/// (SciPy 1.18 returns an empty array there; SciPy 1.15 fails).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArYw1 {
    /// The AR(1) coefficient (`ak[0]`).
    pub ak: Option<f32>,
    /// The innovations variance `r[0] - r[1] · ak`.
    pub sigma_v: f32,
}

/// `AR_est_YW(x, 1)` with the plans of `plan` (of `x`'s length) and SciPy `release`'s solve.
///
/// # Errors
/// [`scipy_solve_1x1`]'s, and [`SolveError::RowsMismatch`] (SciPy 1.15) or
/// [`SolveError::NotFinite`] (SciPy 1.18, `r[0]` not finite) for one value.
pub fn ar_est_yw1(
    plan: &CrosscovPlan,
    release: SolveRelease,
    x: &[f32],
) -> Result<ArYw1, SolveError> {
    let (r0, r1) = plan.autocorr_01(x);
    match r1 {
        Some(r1) => {
            let ak = scipy_solve_1x1(release, r0, r1)?;
            // np.dot of two float32 vectors of one element: OpenBLAS's sdot, the product
            // rounded to float32, then added to 0 in double precision and rounded back.
            let sigma_v = NpFloat::sub(r0, NpFloat::mul(r1, ak));
            Ok(ArYw1 {
                ak: Some(ak),
                sigma_v,
            })
        }
        None => match release {
            SolveRelease::V1_15 => Err(SolveError::RowsMismatch),
            SolveRelease::V1_18 => {
                if !r0.is_finite() {
                    return Err(SolveError::NotFinite);
                }
                // np.dot of two empty float32 arrays is 0.
                Ok(ArYw1 {
                    ak: None,
                    sigma_v: NpFloat::sub(r0, 0.0),
                })
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fused_product() {
        // (1 + 2^-12 i)(1 + 2^-12 i): the real part 1 - 2^-24 keeps its last bit only when the
        // product of the imaginary parts is subtracted inside the fused operation.
        let e = 2.0f32.powi(-12);
        let a = C32::new(1.0 + e, e);
        let r = cmul(a, a);
        let exact_re = ((1.0 + e as f64).powi(2) - (e as f64).powi(2)) as f32;
        assert_eq!(r.re, exact_re);
        assert_eq!(r.im, (2.0 * (1.0 + e as f64) * e as f64) as f32);
        assert!(cmul(C32::new(f32::NAN, 0.0), a).re.is_nan());
    }

    #[test]
    fn autocorrelation_of_a_short_series() {
        let x = [1.0f32, -2.0, 0.5, 3.0];
        for lib in [FftLibrary::Ducc0, FftLibrary::Pocketfft] {
            let plan = CrosscovPlan::new(lib, x.len()).unwrap();
            assert_eq!(plan.fft_size(), 8);
            let c = plan.crosscov(&x, &x);
            let exact: Vec<f64> = (0..4)
                .map(|k| (0..4 - k).map(|t| (x[t] * x[t + k]) as f64).sum::<f64>() / 4.0)
                .collect();
            for (v, e) in c.iter().zip(&exact) {
                assert!((*v as f64 - e).abs() < 1e-5, "{v} {e}");
            }
            let (r0, r1) = plan.autocorr_01(&x);
            assert_eq!((r0, r1), (c[0], Some(c[1])));
            let ar = ar_est_yw1(&plan, SolveRelease::V1_18, &x).unwrap();
            assert_eq!(ar.ak, Some(c[1] / c[0]));
        }
        assert_eq!(fft_size(1), 1);
        assert_eq!(fft_size(129), 512);
        assert_eq!(fft_size(128), 256);
    }

    #[test]
    fn the_releases_solve() {
        use SolveRelease::{V1_15, V1_18};
        assert_eq!(scipy_solve_1x1(V1_15, 2.0, 1.0), Ok(0.5));
        assert_eq!(scipy_solve_1x1(V1_18, 2.0, 1.0), Ok(0.5));
        assert!(scipy_solve_1x1(V1_15, 0.0, 0.0).unwrap().is_nan());
        assert_eq!(scipy_solve_1x1(V1_15, -0.0, 1.0), Ok(f32::NEG_INFINITY));
        assert_eq!(scipy_solve_1x1(V1_18, -0.0, 0.0), Err(SolveError::Singular));
        assert_eq!(
            scipy_solve_1x1(V1_15, f32::NAN, 1.0),
            Err(SolveError::NotFinite)
        );
        assert_eq!(
            scipy_solve_1x1(V1_15, 1.0, f32::INFINITY),
            Ok(f32::INFINITY)
        );
        assert_eq!(
            scipy_solve_1x1(V1_18, 1.0, f32::INFINITY),
            Err(SolveError::NotFinite)
        );
        let one = CrosscovPlan::new(FftLibrary::Ducc0, 1).unwrap();
        assert_eq!(
            ar_est_yw1(&one, V1_15, &[3.0]),
            Err(SolveError::RowsMismatch)
        );
        let r = ar_est_yw1(&one, V1_18, &[3.0]).unwrap();
        assert_eq!((r.ak, r.sigma_v), (None, 9.0));
    }
}

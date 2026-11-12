// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy's `scipy.fftpack.fft` and `ifft` of one-dimensional single-precision arrays of
//! power-of-two length, as SciPy 1.15 (pocketfft, `scipy.fft._pocketfft`) and SciPy 1.18.1
//! (ducc0, `scipy.fft._duccfft`) compute them, bit for bit: nitime's `fftconvolve` under the
//! AR(1) coefficients of nipype's DVARS (`crosscov` pads each voxel's float32 series to the
//! next power of two of `2 T - 1`, transforms it and its reverse, multiplies, transforms back).
//!
//! **SciPy's layers** (`scipy/fftpack/_basic.py`, `scipy/fft/_pocketfft/` and `_duccfft/`:
//! `basic.py` (`c2c`), `helper.py` (`_asfarray`, `_fix_shape_1d`, `_normalization`),
//! `pypocketfft.cxx` / `pyduccfft.cxx` (`c2c`, `c2c_sym_internal`, `norm_fct`)): the input is
//! cut to `n` values or padded with zeros; a complex array goes through the complex transform
//! (`ifft` scaled by `float(1 / n)`, exact for a power of two); a real array through the forward
//! real transform (`r2c`: in both directions, `ifft` negating the imaginary parts, both scaled as
//! the complex one), its half spectrum written as complex values (the zero frequency and, for an
//! even length, the last one with imaginary part `+0`), then the other half filled with the
//! conjugates (`rev_iter` / `hermiteHelper`, which also conjugate the zero frequency and the
//! middle one in place: their imaginary parts become `-0`).
//!
//! **The two libraries** agree up to length 256 for a complex array and 512 for a real one;
//! above, ducc0's plans are other algorithms (`cpass`, `rpass`), whose rounding differs from
//! pocketfft's in many values. ducc0 computes a complex array of length 131072 or more by
//! Bailey's two-dimensional algorithm (`fftnd_impl.h`'s `c2c`), not ported:
//! [`FftError::TooLong`] (nitime's lengths stay below for series of up to 32768 volumes).
//! Lengths that are not powers of two (other radices, Bluestein's algorithm) are not ported
//! either ([`FftError::NotPowerOfTwo`]): nitime pads to a power of two.
//!
//! - [`roots`]: the twiddle tables (`sincos_2pibyn`, `UnityRoots`).
//! - [`cpass`]: the complex passes and plans.
//! - [`rpass`]: the real passes and plans.

pub(crate) mod cpass;
pub(crate) mod roots;
pub(crate) mod rpass;

pub use roots::twiddle_survey;

use larmorx_core::Complex;
use larmorx_core::numpy::NpFloat;

/// A single-precision complex value (numpy's `complex64`).
pub type C32 = Complex<f32>;

/// The FFT library of a SciPy release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FftLibrary {
    /// ducc0 (`scipy.fft._duccfft`), SciPy 1.18.1's. The default.
    #[default]
    Ducc0,
    /// pocketfft (`scipy.fft._pocketfft`), SciPy 1.15's (1.15.3, the shared environment's;
    /// 1.15.2, fMRIPrep 25.2.6's image).
    Pocketfft,
}

impl FftLibrary {
    /// The library of a SciPy version string: `1.15.x` pocketfft, `1.18.x` ducc0; `None` for
    /// the releases whose FFTs are not ported.
    pub fn from_scipy_version(version: &str) -> Option<FftLibrary> {
        let mut it = version.split('.');
        let (major, minor) = (it.next()?, it.next()?);
        match (major, minor) {
            ("1", "15") => Some(FftLibrary::Pocketfft),
            ("1", "18") => Some(FftLibrary::Ducc0),
            _ => None,
        }
    }
}

/// A transform this port cannot run.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FftError {
    /// SciPy's `ValueError` for a length below one (an `n` below one, or an empty array
    /// without `n`).
    #[error("invalid number of data points ({0}) specified")]
    InvalidLength(i64),
    /// A length that is not a power of two (SciPy's other radices and Bluestein's algorithm are
    /// not ported).
    #[error(
        "FFT length {0} is not a power of two: larmorx ports SciPy's FFTs for powers of two only \
         (nitime's fftconvolve)"
    )]
    NotPowerOfTwo(usize),
    /// ducc0's complex transform of length 131072 or more (Bailey's algorithm, not ported).
    #[error(
        "complex FFT length {n} with SciPy 1.18's ducc0: lengths of {limit} and more use \
         Bailey's two-dimensional algorithm, which larmorx does not port"
    )]
    TooLong { n: usize, limit: usize },
}

/// ducc0's first length that goes through Bailey's algorithm.
pub const DUCC0_BAILEY_LENGTH: usize = 131_072;

// SSE's single-precision arithmetic (NaNs included).
#[inline(always)]
pub(crate) fn add(a: f32, b: f32) -> f32 {
    NpFloat::add(a, b)
}
#[inline(always)]
pub(crate) fn sub(a: f32, b: f32) -> f32 {
    NpFloat::sub(a, b)
}
#[inline(always)]
pub(crate) fn mul(a: f32, b: f32) -> f32 {
    NpFloat::mul(a, b)
}
#[inline(always)]
pub(crate) fn cadd(a: C32, b: C32) -> C32 {
    C32::new(add(a.re, b.re), add(a.im, b.im))
}
#[inline(always)]
pub(crate) fn csub(a: C32, b: C32) -> C32 {
    C32::new(sub(a.re, b.re), sub(a.im, b.im))
}
/// `PM(a, b, c, d)`: `a = c + d`, `b = c - d`.
#[inline(always)]
pub(crate) fn pm(c: C32, d: C32) -> (C32, C32) {
    (cadd(c, d), csub(c, d))
}
/// `PMINPLACE(a, b)`: `a = a + b`, `b = a - b` (the old `a`).
#[inline(always)]
pub(crate) fn pminplace(a: &mut C32, b: &mut C32) {
    let t = *a;
    *a = cadd(*a, *b);
    *b = csub(t, *b);
}
/// `special_mul<fwd>(v1, v2)`: `v1 conj(v2)` forward, `v1 v2` backward, in the libraries'
/// operation order.
#[inline(always)]
pub(crate) fn special_mul<const FWD: bool>(v1: C32, v2: C32) -> C32 {
    if FWD {
        C32::new(
            add(mul(v1.re, v2.re), mul(v1.im, v2.im)),
            sub(mul(v1.im, v2.re), mul(v1.re, v2.im)),
        )
    } else {
        C32::new(
            sub(mul(v1.re, v2.re), mul(v1.im, v2.im)),
            add(mul(v1.re, v2.im), mul(v1.im, v2.re)),
        )
    }
}
/// `ROTX90<fwd>`: multiplication by `-i` forward, `i` backward (exact).
#[inline(always)]
pub(crate) fn rotx90<const FWD: bool>(a: C32) -> C32 {
    if FWD {
        C32::new(a.im, -a.re)
    } else {
        C32::new(-a.im, a.re)
    }
}
/// `ROTX45<fwd>` of `cfftp8` / `pass8`.
#[inline(always)]
pub(crate) fn rotx45<const FWD: bool>(a: C32) -> C32 {
    let h = rpass::HSQT2;
    if FWD {
        C32::new(mul(h, add(a.re, a.im)), mul(h, sub(a.im, a.re)))
    } else {
        C32::new(mul(h, sub(a.re, a.im)), mul(h, add(a.im, a.re)))
    }
}
/// `ROTX135<fwd>` of `cfftp8` / `pass8`.
#[inline(always)]
pub(crate) fn rotx135<const FWD: bool>(a: C32) -> C32 {
    let h = rpass::HSQT2;
    if FWD {
        C32::new(mul(h, sub(a.im, a.re)), mul(h, sub(-a.re, a.im)))
    } else {
        C32::new(mul(h, sub(-a.re, a.im)), mul(h, sub(a.re, a.im)))
    }
}

fn check_length(n: usize) -> Result<(), FftError> {
    if n == 0 {
        return Err(FftError::InvalidLength(0));
    }
    if !n.is_power_of_two() {
        return Err(FftError::NotPowerOfTwo(n));
    }
    Ok(())
}

/// The plan of a complex transform of one length (`pocketfft_c`), reusable across arrays.
#[derive(Clone, Debug)]
pub struct ComplexFft {
    n: usize,
    plan: cpass::CPass,
}

impl ComplexFft {
    /// The plan of `library` for length `n` (a power of two; for ducc0 below
    /// [`DUCC0_BAILEY_LENGTH`]).
    pub fn new(library: FftLibrary, n: usize) -> Result<ComplexFft, FftError> {
        check_length(n)?;
        let plan = match library {
            FftLibrary::Pocketfft => cpass::pocketfft_plan(n),
            FftLibrary::Ducc0 => {
                if n >= DUCC0_BAILEY_LENGTH {
                    return Err(FftError::TooLong {
                        n,
                        limit: DUCC0_BAILEY_LENGTH,
                    });
                }
                cpass::ducc0_plan(n)
            }
        };
        Ok(ComplexFft { n, plan })
    }

    /// The length.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the length is zero (never: plans have a length of at least one).
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `scipy.fftpack.fft` (`forward`) or `ifft` of `data` (of the plan's length), in place:
    /// the backward transform scaled by `1 / n`.
    pub fn transform(&self, data: &mut Vec<C32>, forward: bool) {
        assert_eq!(data.len(), self.n, "the array's length is the plan's");
        let mut tmp = Vec::new();
        if forward {
            self.plan.exec::<true>(data, &mut tmp);
        } else {
            self.plan.exec::<false>(data, &mut tmp);
            let fct = 1.0 / self.n as f32;
            if fct != 1.0 {
                for v in data.iter_mut() {
                    *v = C32::new(mul(v.re, fct), mul(v.im, fct));
                }
            }
        }
    }
}

/// The plan of a real transform of one length (`pocketfft_r`), reusable across arrays.
#[derive(Clone, Debug)]
pub struct RealFft {
    n: usize,
    plan: rpass::RPass,
}

impl RealFft {
    /// The plan of `library` for length `n` (a power of two).
    pub fn new(library: FftLibrary, n: usize) -> Result<RealFft, FftError> {
        check_length(n)?;
        let plan = match library {
            FftLibrary::Pocketfft => rpass::pocketfft_plan(n),
            FftLibrary::Ducc0 => rpass::ducc0_plan(n),
        };
        Ok(RealFft { n, plan })
    }

    /// The length.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the length is zero (never).
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `scipy.fftpack.fft` (`forward`) or `ifft` of the real array `x` (of the plan's length):
    /// the whole spectrum (`c2c_sym_internal`).
    pub fn transform(&self, x: &[f32], forward: bool) -> Vec<C32> {
        let n = self.n;
        assert_eq!(x.len(), n, "the array's length is the plan's");
        let mut res = x.to_vec();
        let mut tmp = Vec::new();
        self.plan.exec(&mut res, &mut tmp);
        if !forward {
            let fct = 1.0 / n as f32;
            if fct != 1.0 {
                for v in res.iter_mut() {
                    *v = mul(*v, fct);
                }
            }
        }
        // general_r2c's packing of the halfcomplex values.
        let mut out = vec![C32::new(0.0, 0.0); n];
        out[0] = C32::new(res[0], 0.0);
        let (mut i, mut ii) = (1usize, 1usize);
        while i + 1 < n {
            out[ii] = C32::new(res[i], if forward { res[i + 1] } else { -res[i + 1] });
            i += 2;
            ii += 1;
        }
        if i < n {
            out[ii] = C32::new(res[i], 0.0);
        }
        // The other half: out[n - i] = conj(out[i]) for i = 0 ..= n / 2 (index 0 onto itself).
        for i in 0..=n / 2 {
            let ic = if i == 0 { 0 } else { n - i };
            let v = out[i];
            out[ic] = C32::new(v.re, -v.im);
        }
        out
    }
}

/// `_fix_shape_1d`: the first `n` values, or the values padded with zeros to `n`.
fn fix_shape<T: Copy>(x: &[T], n: Option<usize>, zero: T) -> Result<Vec<T>, FftError> {
    let n = match n {
        Some(n) => n,
        None if x.is_empty() => return Err(FftError::InvalidLength(0)),
        None => x.len(),
    };
    if n == 0 {
        return Err(FftError::InvalidLength(0));
    }
    let mut v: Vec<T> = x[..x.len().min(n)].to_vec();
    v.resize(n, zero);
    Ok(v)
}

/// `scipy.fftpack.fft(x, n)` (`forward`) or `ifft(x, n)` of a one-dimensional float32 array.
pub fn fftpack_real(
    library: FftLibrary,
    x: &[f32],
    n: Option<usize>,
    forward: bool,
) -> Result<Vec<C32>, FftError> {
    let x = fix_shape(x, n, 0.0f32)?;
    Ok(RealFft::new(library, x.len())?.transform(&x, forward))
}

/// `scipy.fftpack.fft(x, n)` (`forward`) or `ifft(x, n)` of a one-dimensional complex64 array.
pub fn fftpack_complex(
    library: FftLibrary,
    x: &[C32],
    n: Option<usize>,
    forward: bool,
) -> Result<Vec<C32>, FftError> {
    let mut x = fix_shape(x, n, C32::new(0.0, 0.0))?;
    ComplexFft::new(library, x.len())?.transform(&mut x, forward);
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The naive DFT in double precision, for an accuracy check (not SciPy's bits).
    fn dft(x: &[C32], forward: bool) -> Vec<(f64, f64)> {
        let n = x.len();
        let s = if forward { -1.0 } else { 1.0 };
        (0..n)
            .map(|k| {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (t, v) in x.iter().enumerate() {
                    let a = s * 2.0 * std::f64::consts::PI * ((k * t) % n) as f64 / n as f64;
                    let (sa, ca) = a.sin_cos();
                    re += v.re as f64 * ca - v.im as f64 * sa;
                    im += v.re as f64 * sa + v.im as f64 * ca;
                }
                if forward {
                    (re, im)
                } else {
                    (re / n as f64, im / n as f64)
                }
            })
            .collect()
    }

    fn seeded(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 40) as f32 / (1u64 << 24) as f32) - 0.5
            })
            .collect()
    }

    #[test]
    fn transforms_are_dfts() {
        for lib in [FftLibrary::Pocketfft, FftLibrary::Ducc0] {
            for k in 0..=12 {
                let n = 1usize << k;
                let re = seeded(n, k as u64);
                let im = seeded(n, 100 + k as u64);
                let z: Vec<C32> = re.iter().zip(&im).map(|(a, b)| C32::new(*a, *b)).collect();
                for forward in [true, false] {
                    let got = fftpack_complex(lib, &z, None, forward).unwrap();
                    let want = dft(&z, forward);
                    let scale = if forward {
                        (n as f64).sqrt()
                    } else {
                        1.0 / (n as f64).sqrt()
                    };
                    for (g, w) in got.iter().zip(&want) {
                        assert!(
                            (g.re as f64 - w.0).abs() < 1e-5 * scale + 1e-6,
                            "{lib:?} {n}"
                        );
                        assert!(
                            (g.im as f64 - w.1).abs() < 1e-5 * scale + 1e-6,
                            "{lib:?} {n}"
                        );
                    }
                    let real: Vec<C32> = re.iter().map(|a| C32::new(*a, 0.0)).collect();
                    let got = fftpack_real(lib, &re, None, forward).unwrap();
                    let want = dft(&real, forward);
                    for (g, w) in got.iter().zip(&want) {
                        assert!(
                            (g.re as f64 - w.0).abs() < 1e-5 * scale + 1e-6,
                            "{lib:?} {n}"
                        );
                        assert!(
                            (g.im as f64 - w.1).abs() < 1e-5 * scale + 1e-6,
                            "{lib:?} {n}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn libraries_agree_on_small_lengths_and_differ_above() {
        // SciPy 1.15.3 and 1.18.1 give the same bits up to 256 (complex) and 512 (real).
        let x = seeded(4096, 9);
        let z: Vec<C32> = x
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| C32::new(p[0], p[1]))
            .collect();
        let bits = |v: &[C32]| {
            v.iter()
                .map(|c| (c.re.to_bits(), c.im.to_bits()))
                .collect::<Vec<_>>()
        };
        for k in 0..=8 {
            let n = 1 << k;
            let a = fftpack_complex(FftLibrary::Pocketfft, &z[..n], None, false).unwrap();
            let b = fftpack_complex(FftLibrary::Ducc0, &z[..n], None, false).unwrap();
            assert_eq!(bits(&a), bits(&b), "complex {n}");
        }
        for k in 0..=9 {
            let n = 1 << k;
            let a = fftpack_real(FftLibrary::Pocketfft, &x[..n], None, true).unwrap();
            let b = fftpack_real(FftLibrary::Ducc0, &x[..n], None, true).unwrap();
            assert_eq!(bits(&a), bits(&b), "real {n}");
        }
        let a = fftpack_complex(FftLibrary::Pocketfft, &z[..512], None, false).unwrap();
        let b = fftpack_complex(FftLibrary::Ducc0, &z[..512], None, false).unwrap();
        assert_ne!(bits(&a), bits(&b));
        let a = fftpack_real(FftLibrary::Pocketfft, &x[..1024], None, true).unwrap();
        let b = fftpack_real(FftLibrary::Ducc0, &x[..1024], None, true).unwrap();
        assert_ne!(bits(&a), bits(&b));
    }

    #[test]
    fn shapes_and_refusals() {
        let lib = FftLibrary::Ducc0;
        assert_eq!(
            fftpack_real(lib, &[], None, true),
            Err(FftError::InvalidLength(0))
        );
        assert_eq!(
            fftpack_real(lib, &[1.0], Some(0), true),
            Err(FftError::InvalidLength(0))
        );
        assert_eq!(
            fftpack_real(lib, &[1.0; 3], None, true),
            Err(FftError::NotPowerOfTwo(3))
        );
        assert!(matches!(
            fftpack_complex(lib, &[C32::new(0.0, 0.0)], Some(DUCC0_BAILEY_LENGTH), true),
            Err(FftError::TooLong { .. })
        ));
        // Truncation and padding.
        let a = fftpack_real(lib, &[1.0, 2.0, 3.0, 4.0, 5.0], Some(4), true).unwrap();
        let b = fftpack_real(lib, &[1.0, 2.0, 3.0, 4.0], None, true).unwrap();
        assert_eq!(a, b);
        let c = fftpack_real(lib, &[1.0, 2.0, 3.0], Some(4), true).unwrap();
        let d = fftpack_real(lib, &[1.0, 2.0, 3.0, 0.0], None, true).unwrap();
        assert_eq!(c, d);
        // The zero frequency's and the middle one's imaginary parts are -0.
        assert!(b[0].im.is_sign_negative() && b[2].im.is_sign_negative());
        assert_eq!(
            FftLibrary::from_scipy_version("1.15.2"),
            Some(FftLibrary::Pocketfft)
        );
        assert_eq!(
            FftLibrary::from_scipy_version("1.18.1"),
            Some(FftLibrary::Ducc0)
        );
        assert_eq!(FftLibrary::from_scipy_version("1.16.0"), None);
    }
}

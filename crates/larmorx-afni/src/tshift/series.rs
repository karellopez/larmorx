//! What happens to one voxel's time series (`specs/3dTshift.md` §4–§5).
//!
//! Arithmetic is float32 except where the spec (or a black-box observation) says otherwise;
//! each step notes its precision. Samples outside the series count as 0.

use larmorx_core::Complex;
use larmorx_core::math::{cos, sin};

use super::{Method, Restore};
use crate::fft::Fft;

type C = Complex<f64>;

/// The linear trend `a + b·i` of `v` by least squares: sums in double (each `v[i]·i` a
/// float32 product), `a` and `b` in double, rounded to float32.
pub fn linear_fit(v: &[f32]) -> (f32, f32) {
    let m = v.len() as f64;
    let (mut sy, mut sxy) = (0f64, 0f64);
    for (i, &x) in v.iter().enumerate() {
        sy += f64::from(x);
        sxy += f64::from(x * i as f32);
    }
    let sx = m * (m - 1.0) / 2.0;
    let sxx = (m - 1.0) * m * (2.0 * m - 1.0) / 6.0;
    let b = (m * sxy - sx * sy) / (m * sxx - sx * sx);
    let a = (sy - b * sx) / m;
    (a as f32, b as f32)
}

/// The mean of `v`, summed and divided in float32.
pub fn mean(v: &[f32]) -> f32 {
    let mut sum = 0f32;
    for &x in v {
        sum += x;
    }
    sum / v.len() as f32
}

/// `a + b·i` as AFNI forms it: `b·i`, then `a +`, in float32.
#[inline]
fn trend(a: f32, b: f32, i: usize) -> f32 {
    a + b * i as f32
}

/// `x` limited to `[lo, hi]` (NaN stays NaN; never panics).
#[inline]
fn clip(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

fn range(v: &[f32]) -> (f32, f32) {
    v.iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        })
}

/// What is removed from a series before it is shifted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// The least-squares line (AFNI's default).
    Trend,
    /// The mean (`-no_detrend`).
    Mean,
    /// Nothing: the raw values are shifted (what AFNI does to every other voxel with
    /// `-no_detrend`, observed).
    Nothing,
}

/// The detrended series and what is needed to put the trend back.
pub struct Detrended {
    removal: Removal,
    a: f32,
    b: f32,
    /// The input's range (for the default restore).
    lo0: f32,
    hi0: f32,
    /// The residual's range.
    lo1: f32,
    hi1: f32,
}

/// Removes the linear trend, the mean or nothing from `v` in place.
pub fn detrend(v: &mut [f32], removal: Removal) -> Detrended {
    let (lo0, hi0) = range(v);
    let (a, b) = match removal {
        Removal::Trend => linear_fit(v),
        Removal::Mean => (mean(v), 0.0),
        Removal::Nothing => (0.0, 0.0),
    };
    if removal != Removal::Nothing {
        for (i, x) in v.iter_mut().enumerate() {
            *x -= trend(a, b, i);
        }
    }
    let (lo1, hi1) = range(v);
    Detrended {
        removal,
        a,
        b,
        lo0,
        hi0,
        lo1,
        hi1,
    }
}

/// Clips the shifted residual to its original range, then puts back what `restore` says.
pub fn retrend(v: &mut [f32], d: &Detrended, restore: Restore) {
    for x in v.iter_mut() {
        *x = clip(*x, d.lo1, d.hi1);
    }
    match (d.removal, restore) {
        (Removal::Trend, Restore::Trend) => {
            for (i, x) in v.iter_mut().enumerate() {
                *x = clip(*x + trend(d.a, d.b, i), d.lo0, d.hi0);
            }
        }
        (Removal::Trend, Restore::None) | (Removal::Nothing, _) => {}
        // -rlt+ restores the intercept; without detrending, the mean (no clip either way).
        (Removal::Trend, Restore::Intercept) | (Removal::Mean, _) => {
            for x in v.iter_mut() {
                *x += d.a;
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Fourier

/// The FFT length for `nt` time points: the smallest `L ≥ nt + 4` of the form `2^a·3^b·5^c`
/// with `a ≥ 1` and `b, c ∈ {0, 1}`.
pub fn fourier_length(nt: usize) -> usize {
    (nt + 4..)
        .find(|&n| {
            n % 2 == 0 && {
                let mut m = n;
                while m % 2 == 0 {
                    m /= 2;
                }
                matches!(m, 1 | 3 | 5 | 15)
            }
        })
        .expect("such a length exists")
}

/// The frequency response that shifts a real series of FFT length `L` by `s` samples (AFNI's
/// sign: the output at `n` is the input at `n − s`): bin `k` of `1..=L/2` gets `φ_k = φ_1^k`
/// formed by repeated float32 complex products, with `φ_1 = (cos θ, sin θ)`,
/// `θ = −s·(2π/L)` in float32 and the cosine and sine rounded to float32. The Nyquist bin keeps
/// only the real part and the negative frequencies are the conjugates, so real series stay
/// real; this lets two series be shifted by one complex transform.
pub fn fourier_response(len: usize, s: f32) -> Vec<C> {
    let w = (2.0 * std::f64::consts::PI / len as f64) as f32;
    let theta = -s * w;
    let (c, sn) = (cos(f64::from(theta)) as f32, sin(f64::from(theta)) as f32);
    let half = len / 2;
    let mut out = vec![C::new(1.0, 0.0); len];
    let (mut re, mut im) = (c, sn);
    for k in 1..=half {
        if k > 1 {
            (re, im) = (c * re - sn * im, c * im + sn * re);
        }
        let phi = C::new(f64::from(re), f64::from(im));
        if k == half {
            out[k] = C::new(phi.re, 0.0);
        } else {
            out[k] = phi;
            out[len - k] = phi.conj();
        }
    }
    out
}

/// Workspace for [`fourier_shift`].
pub struct FourierWork {
    buf: Vec<C>,
    scratch: Vec<C>,
}

impl FourierWork {
    pub fn new(len: usize) -> Self {
        FourierWork {
            buf: vec![C::new(0.0, 0.0); len],
            scratch: vec![C::new(0.0, 0.0); len],
        }
    }
}

/// Shifts `x` (and `y`, if given: two real series share one complex transform) through the
/// frequency `response`, zero-padded to the FFT length, in double precision; results are
/// rounded to float32.
pub fn fourier_shift(
    x: &mut [f32],
    y: Option<&mut [f32]>,
    fft: &Fft,
    response: &[C],
    work: &mut FourierWork,
) {
    let (m, len) = (x.len(), fft.len());
    let buf = &mut work.buf;
    for (n, b) in buf.iter_mut().enumerate() {
        *b = if n < m {
            C::new(f64::from(x[n]), y.as_ref().map_or(0.0, |y| f64::from(y[n])))
        } else {
            C::new(0.0, 0.0)
        };
    }
    fft.forward(buf, &mut work.scratch);
    for (b, r) in buf.iter_mut().zip(response) {
        *b *= r;
    }
    fft.inverse(buf, &mut work.scratch);
    let scale = len as f64;
    for (n, v) in x.iter_mut().enumerate() {
        *v = (buf[n].re / scale) as f32;
    }
    if let Some(y) = y {
        for (n, v) in y.iter_mut().enumerate() {
            *v = (buf[n].im / scale) as f32;
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Lagrange polynomials

/// Lagrange interpolation weights for one slice.
#[derive(Clone, Debug)]
pub struct Lagrange {
    /// Integer part of the source offset.
    i0: i64,
    /// Sample offsets (relative to `i0 + n`) and their weights, in summation order.
    terms: Vec<(i64, f32)>,
    lo: i64,
    hi: i64,
}

/// `1/|∏(j − k)|` for the Lagrange basis polynomials, as the decimal constants AFNI uses.
fn lagrange_constant(denominator: i64) -> f64 {
    match denominator.abs() {
        1 => 1.0,
        2 => 0.5,
        6 => 0.1666667,
        12 => 0.083333333,
        24 => 0.041666667,
        120 => 0.008333333,
        144 => 0.006944444444,
        240 => 0.004166666667,
        720 => 0.001388888889,
        5040 => 0.0001984126984,
        d => unreachable!("no Lagrange constant for {d}"),
    }
}

impl Lagrange {
    /// The weights that shift by `s` samples with `method` (a polynomial method).
    pub fn new(method: Method, s: f32) -> Lagrange {
        let (lo, hi): (i64, i64) = match method {
            Method::Linear => (0, 1),
            Method::Cubic => (-1, 2),
            Method::Quintic => (-2, 3),
            Method::Heptic => (-3, 4),
            _ => unreachable!("not a polynomial method"),
        };
        let d = -s;
        let mut i0 = d.trunc() as i64;
        if d < 0.0 {
            i0 -= 1;
        }
        let f = d - i0 as f32;
        let nodes: Vec<i64> = (lo..=hi).collect();
        let weight = |j: i64| -> f32 {
            if method == Method::Linear {
                return if j == 0 { 1.0 - f } else { f };
            }
            let denominator: i64 = nodes.iter().filter(|&&k| k != j).map(|&k| j - k).product();
            let mut p = lagrange_constant(denominator).copysign(denominator as f64);
            for &k in nodes.iter().filter(|&&k| k != j) {
                p *= f64::from(f) - k as f64;
            }
            p as f32
        };
        // AFNI sums the heptic terms with the two outermost last (observed).
        let order: Vec<i64> = if method == Method::Heptic {
            vec![-2, -1, 0, 1, 2, 3, -3, 4]
        } else {
            nodes.clone()
        };
        Lagrange {
            i0,
            terms: order.into_iter().map(|j| (j, weight(j))).collect(),
            lo,
            hi,
        }
    }

    /// Interpolates `v` in place (`scratch` holds a copy of the input).
    pub fn apply(&self, v: &mut [f32], scratch: &mut Vec<f32>) {
        let m = v.len() as i64;
        if self.i0.abs() >= m {
            v.fill(0.0);
            return;
        }
        scratch.clear();
        scratch.extend_from_slice(v);
        let src = &scratch[..];
        for (n, out) in v.iter_mut().enumerate() {
            let base = self.i0 + n as i64;
            if base + self.lo >= 0 && base + self.hi < m {
                // Inside: float32 products and sums.
                let mut terms = self.terms.iter();
                let &(j, w) = terms.next().expect("at least two terms");
                let mut acc = w * src[(base + j) as usize];
                for &(j, w) in terms {
                    acc += w * src[(base + j) as usize];
                }
                *out = acc;
            } else {
                // Near the edges: in double.
                let mut acc = 0f64;
                for &(j, w) in &self.terms {
                    let q = base + j;
                    if (0..m).contains(&q) {
                        acc += f64::from(w) * f64::from(src[q as usize]);
                    }
                }
                *out = acc as f32;
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Weighted sinc

/// π in float32.
const PI_F32: f32 = std::f32::consts::PI;

/// The weight of a sample at distance `x` from the source position: `sinc(x)·W(u)`, zero
/// beyond the radius; all in float32 (the sine and cosine rounded to float32).
fn wsinc_weight(x: f32, radius: i64) -> f32 {
    let x = x.abs();
    if x > radius as f32 {
        return 0.0;
    }
    // AFNI's constant (π²/6 to 8 digits), kept as written although float32 holds fewer.
    #[allow(clippy::excessive_precision)]
    const PI2_6: f32 = 1.644_934_1;
    let sinc = if x <= 0.01 {
        1.0 - PI2_6 * (x * x)
    } else {
        let px = PI_F32 * x;
        sin(f64::from(px)) as f32 / px
    };
    let scale: f32 = if radius == 5 { 0.19999 } else { 0.11111 };
    let u = scale * x;
    let pu = PI_F32 * u;
    let window = 0.424_380_1
        + 0.497_340_6 * cos(f64::from(pu)) as f32
        + 0.078_279_3 * cos(f64::from(2.0 * pu)) as f32;
    sinc * window
}

/// Weighted-sinc interpolation of `v` in place, shifted by `s` samples with `radius` 5 or 9.
pub fn wsinc(v: &mut [f32], s: f32, radius: i64, scratch: &mut Vec<f32>) {
    if s.abs() < 0.0001 {
        return;
    }
    let m = v.len() as i64;
    let d = -s;
    scratch.clear();
    scratch.extend_from_slice(v);
    let src = &scratch[..];
    for (n, out) in v.iter_mut().enumerate() {
        let t = n as f32 + d;
        let it = t.floor() as i64;
        let f = t - it as f32;
        // AFNI sums in float32 only when the whole window ±radius is inside (observed).
        if it - radius >= 0 && it + radius < m {
            let mut acc = 0f32;
            for j in -(radius - 1)..=radius {
                acc += wsinc_weight(j as f32 - f, radius) * src[(it + j) as usize];
            }
            *out = acc;
        } else {
            let mut acc = 0f64;
            for j in -(radius - 1)..=radius {
                let q = it + j;
                if (0..m).contains(&q) {
                    acc +=
                        f64::from(wsinc_weight(j as f32 - f, radius)) * f64::from(src[q as usize]);
                }
            }
            *out = acc as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourier_lengths() {
        assert_eq!(fourier_length(100), 120);
        assert_eq!(fourier_length(240), 256);
        assert_eq!(fourier_length(236), 240);
        assert_eq!(fourier_length(12), 16);
        assert_eq!(fourier_length(16), 20);
        assert_eq!(fourier_length(20), 24);
        assert_eq!(fourier_length(260), 320);
        assert_eq!(fourier_length(4092), 4096);
    }

    #[test]
    fn linear_fit_of_a_line_is_exact() {
        let v: Vec<f32> = (0..50).map(|i| 3.0 + 0.5 * i as f32).collect();
        assert_eq!(linear_fit(&v), (3.0, 0.5));
    }

    /// A smooth periodic series and its exact shifted version.
    fn sine(m: usize, shift: f64) -> Vec<f32> {
        (0..m)
            .map(|n| {
                (2.0 * std::f64::consts::PI * 3.0 * (n as f64 - shift) / m as f64).sin() as f32
            })
            .collect()
    }

    #[test]
    fn fourier_shift_by_whole_samples_is_a_circular_shift() {
        // A whole-sample shift of the zero-padded series moves every sample exactly (up to
        // rounding): with s = −1 the output at n is the input at n + 1, and 0 past the end.
        let m = 60;
        let len = fourier_length(m);
        let fft = Fft::new(len).unwrap();
        let x0 = sine(m, 0.0);
        let y0: Vec<f32> = x0
            .iter()
            .enumerate()
            .map(|(n, v)| v * 2.0 + n as f32)
            .collect();
        let (mut x, mut y) = (x0.clone(), y0.clone());
        let mut work = FourierWork::new(len);
        fourier_shift(
            &mut x,
            Some(&mut y),
            &fft,
            &fourier_response(len, -1.0),
            &mut work,
        );
        for n in 0..m {
            let (ex, ey) = if n + 1 < m {
                (x0[n + 1], y0[n + 1])
            } else {
                (0.0, 0.0)
            };
            assert!((x[n] - ex).abs() < 1e-4, "x at {n}: {} vs {ex}", x[n]);
            assert!((y[n] - ey).abs() < 1e-3, "y at {n}: {} vs {ey}", y[n]);
        }
        // One series alone gives the same result as in a pair (up to rounding).
        let mut alone = x0.clone();
        fourier_shift(
            &mut alone,
            None,
            &fft,
            &fourier_response(len, -1.0),
            &mut work,
        );
        for (a, b) in alone.iter().zip(&x) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn polynomial_methods_shift_smooth_series() {
        let m = 120;
        let exact = sine(m, 0.25);
        for method in [Method::Cubic, Method::Quintic, Method::Heptic] {
            let mut v = sine(m, 0.0);
            let mut scratch = Vec::new();
            Lagrange::new(method, 0.25).apply(&mut v, &mut scratch);
            for n in 4..m - 4 {
                assert!((v[n] - exact[n]).abs() < 2e-3, "{method:?} at {n}");
            }
        }
        // Linear interpolation between neighbours.
        let mut v = vec![0.0, 1.0, 2.0, 3.0];
        Lagrange::new(Method::Linear, 0.5).apply(&mut v, &mut Vec::new());
        assert_eq!(v, [0.0, 0.5, 1.5, 2.5]);
    }

    #[test]
    fn wsinc_shifts_smooth_series() {
        let m = 200;
        let exact = sine(m, -0.4);
        for radius in [5, 9] {
            let mut v = sine(m, 0.0);
            wsinc(&mut v, -0.4, radius, &mut Vec::new());
            for n in 12..m - 12 {
                assert!((v[n] - exact[n]).abs() < 2e-2, "radius {radius} at {n}");
            }
        }
    }

    #[test]
    fn retrend_restores_and_clips() {
        let original: Vec<f32> = (0..20).map(|i| 10.0 + i as f32 + (i % 3) as f32).collect();
        let mut v = original.clone();
        let d = detrend(&mut v, Removal::Trend);
        let residual = v.clone();
        retrend(&mut v, &d, Restore::Trend);
        for (a, b) in v.iter().zip(&original) {
            assert!((a - b).abs() < 1e-4);
        }
        let mut w = residual.clone();
        retrend(&mut w, &d, Restore::None);
        assert_eq!(w, residual);
        // Without removal the series is untouched both ways.
        let mut raw = original.clone();
        let d = detrend(&mut raw, Removal::Nothing);
        assert_eq!(raw, original);
        retrend(&mut raw, &d, Restore::Trend);
        assert_eq!(raw, original);
    }
}

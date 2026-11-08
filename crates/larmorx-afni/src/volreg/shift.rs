// SPDX-License-Identifier: Apache-2.0
//! Shifting one row of a volume by a fraction of a voxel (`specs/3dvolreg.md` §5.6): nearest
//! neighbour, linear, cubic, quintic and heptic Lagrange interpolation, and the Fourier shift
//! of two rows at once.
//!
//! The polynomial methods follow the spec's arithmetic exactly (the weights' operations, the
//! float32 sums where every sample lies inside the row and the double sums at the row's ends,
//! the order of the terms): a row's weights are the same for every output sample, so the inner
//! loop is a fixed-tap filter over contiguous samples that the compiler vectorises. The Fourier
//! shift follows §5.6 step by step in float32, with larmorx's own FFT in double precision
//! (`crate::fft`; the spec leaves the FFT's arithmetic open, §11).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use larmorx_core::Complex;

use crate::fft::Fft;

use super::rotate::Trig;

/// An interpolation method of 3dvolreg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    NN,
    Linear,
    Cubic,
    Quintic,
    Heptic,
    Fourier,
}

impl Method {
    /// The `3drotate` option of the method (`-rotcom`).
    pub fn rotate_option(self) -> &'static str {
        match self {
            Method::NN => "-NN",
            Method::Linear => "-linear",
            Method::Cubic => "-cubic",
            Method::Quintic => "-quintic",
            Method::Heptic => "-heptic",
            Method::Fourier => "-Fourier",
        }
    }

    /// Whether the output of this final method is clipped to the input's range (§7.2).
    pub fn clips(self) -> bool {
        matches!(
            self,
            Method::Cubic | Method::Quintic | Method::Heptic | Method::Fourier
        )
    }
}

/// The first sample offset and the weights of a polynomial method at fraction `x`, in the
/// order the terms are summed (§5.6), with each weight's sample offset.
pub(crate) fn weights(method: Method, x: f32) -> ([f32; 8], [i64; 8], usize) {
    let xd = f64::from(x);
    let x2 = f64::from(x * x);
    let w = |v: f64| v as f32;
    match method {
        Method::Linear => {
            let mut ws = [0f32; 8];
            ws[0] = w(1.0 - xd);
            ws[1] = x;
            (ws, [0, 1, 0, 0, 0, 0, 0, 0], 2)
        }
        Method::Cubic => {
            let ws = [
                w(xd * (1.0 - xd) * (xd - 2.0) * 0.1666667),
                w((xd + 1.0) * (xd - 1.0) * (xd - 2.0) * 0.5),
                w(xd * (xd + 1.0) * (2.0 - xd) * 0.5),
                w(xd * (xd + 1.0) * (xd - 1.0) * 0.1666667),
                0.0,
                0.0,
                0.0,
                0.0,
            ];
            (ws, [-1, 0, 1, 2, 0, 0, 0, 0], 4)
        }
        Method::Quintic => {
            let ws = [
                w(xd * (x2 - 1.0) * (2.0 - xd) * (xd - 3.0) * 0.008333333),
                w(xd * (x2 - 4.0) * (xd - 1.0) * (xd - 3.0) * 0.041666667),
                w((x2 - 4.0) * (x2 - 1.0) * (3.0 - xd) * 0.083333333),
                w(xd * (x2 - 4.0) * (xd + 1.0) * (xd - 3.0) * 0.083333333),
                w(xd * (x2 - 1.0) * (xd + 2.0) * (3.0 - xd) * 0.041666667),
                w(xd * (x2 - 1.0) * (x2 - 4.0) * 0.008333333),
                0.0,
                0.0,
            ];
            (ws, [-2, -1, 0, 1, 2, 3, 0, 0], 6)
        }
        Method::Heptic => {
            // k = -3 ... 4, then reordered as summed: -2, -1, 0, 1, 2, 3, -3, 4.
            let m3 = w(xd * (x2 - 1.0) * (x2 - 4.0) * (xd - 3.0) * (4.0 - xd) * 0.0001984126984);
            let m2 = w(xd * (x2 - 1.0) * (xd - 2.0) * (x2 - 9.0) * (xd - 4.0) * 0.001388888889);
            let m1 = w(xd * (xd - 1.0) * (x2 - 4.0) * (x2 - 9.0) * (4.0 - xd) * 0.004166666667);
            let z0 = w((x2 - 1.0) * (x2 - 4.0) * (x2 - 9.0) * (xd - 4.0) * 0.006944444444);
            let p1 = w(xd * (xd + 1.0) * (x2 - 4.0) * (x2 - 9.0) * (4.0 - xd) * 0.006944444444);
            let p2 = w(xd * (x2 - 1.0) * (xd + 2.0) * (x2 - 9.0) * (xd - 4.0) * 0.004166666667);
            let p3 = w(xd * (x2 - 1.0) * (x2 - 4.0) * (xd + 3.0) * (4.0 - xd) * 0.001388888889);
            let p4 = w(xd * (x2 - 1.0) * (x2 - 4.0) * (x2 - 9.0) * 0.0001984126984);
            (
                [m2, m1, z0, p1, p2, p3, m3, p4],
                [-2, -1, 0, 1, 2, 3, -3, 4],
                8,
            )
        }
        Method::NN | Method::Fourier => ([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [0; 8], 1),
    }
}

/// The integer and fractional parts of the position of a shift by `alpha` (§5.6): `ia` the
/// floor of `−alpha` (one lower for an exact negative integer), `x = −alpha − ia` in float32.
/// `None` when the row becomes all zeros.
pub(crate) fn split(alpha: f32, n: usize) -> Option<(i64, f32)> {
    let beta = -alpha;
    if beta.is_nan() || beta.abs() >= 1.0e9 {
        return None;
    }
    let mut ia = beta.trunc() as i64;
    if beta < 0.0 {
        ia -= 1;
    }
    let n = n as i64;
    if ia <= -n || ia >= n {
        return None;
    }
    Some((ia, beta - ia as f32))
}

/// Shifts `f` by `alpha` with a polynomial method (or nearest neighbour) into `out` (the same
/// length): `out[i] = f(i − alpha)`, zeros outside the row.
pub fn shift_poly(method: Method, f: &[f32], alpha: f32, out: &mut [f32]) {
    let n = f.len();
    debug_assert_eq!(out.len(), n);
    let Some((ia, x)) = split(alpha, n) else {
        out.fill(0.0);
        return;
    };
    let ni = n as i64;
    if method == Method::NN {
        // out[i] = f[i + ia] where that sample exists, else 0.
        let lo = (-ia).clamp(0, ni) as usize;
        let hi = (ni - ia).clamp(0, ni) as usize;
        out[..lo].fill(0.0);
        out[hi.max(lo)..].fill(0.0);
        if lo < hi {
            let s0 = (lo as i64 + ia) as usize;
            out[lo..hi].copy_from_slice(&f[s0..s0 + (hi - lo)]);
        }
        return;
    }
    let (w, k, taps) = weights(method, x);
    let (kmin, kmax) = (
        *k[..taps].iter().min().expect("taps"),
        *k[..taps].iter().max().expect("taps"),
    );
    // Interior outputs: i + ia + kmin >= 0 and i + ia + kmax <= n - 1.
    let lo = (-ia - kmin).max(0);
    let hi = (ni - 1 - ia - kmax).min(ni - 1);
    let edge = |i: i64| -> f32 {
        let mut s = 0.0f64;
        for t in 0..taps {
            let j = i + ia + k[t];
            let v = if (0..ni).contains(&j) {
                f64::from(f[j as usize])
            } else {
                0.0
            };
            s += f64::from(w[t]) * v;
        }
        s as f32
    };
    if lo > hi {
        for (i, o) in out.iter_mut().enumerate() {
            *o = edge(i as i64);
        }
        return;
    }
    for i in 0..lo {
        out[i as usize] = edge(i);
    }
    for i in (hi + 1)..ni {
        out[i as usize] = edge(i);
    }
    let len = (hi - lo + 1) as usize;
    let tap = |t: usize| -> &[f32] {
        let start = (lo + ia + k[t]) as usize;
        &f[start..start + len]
    };
    let dst = &mut out[lo as usize..=hi as usize];
    match taps {
        2 => {
            let (a, b) = (tap(0), tap(1));
            for i in 0..len {
                dst[i] = w[0] * a[i] + w[1] * b[i];
            }
        }
        4 => {
            let (a, b, c, d) = (tap(0), tap(1), tap(2), tap(3));
            for i in 0..len {
                dst[i] = w[0] * a[i] + w[1] * b[i] + w[2] * c[i] + w[3] * d[i];
            }
        }
        6 => {
            let (a, b, c, d, e, g) = (tap(0), tap(1), tap(2), tap(3), tap(4), tap(5));
            for i in 0..len {
                dst[i] = w[0] * a[i]
                    + w[1] * b[i]
                    + w[2] * c[i]
                    + w[3] * d[i]
                    + w[4] * e[i]
                    + w[5] * g[i];
            }
        }
        _ => {
            let t: [&[f32]; 8] = std::array::from_fn(tap);
            for i in 0..len {
                dst[i] = w[0] * t[0][i]
                    + w[1] * t[1][i]
                    + w[2] * t[2][i]
                    + w[3] * t[3][i]
                    + w[4] * t[4][i]
                    + w[5] * t[5][i]
                    + w[6] * t[6][i]
                    + w[7] * t[7][i];
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Fourier

/// The Fourier length for a row of `n` samples and a shear of extent `st` (§5.6 step 2): the
/// smallest `2^p·3^q·5^r ≥ trunc(n + 0.5·st)` with `p ≥ 1` and `q, r ∈ {0, 1}`.
pub fn fourier_length(n: usize, st: f32) -> usize {
    let want = (n as f64 + 0.5 * f64::from(st)).trunc().max(2.0) as usize;
    let mut best = usize::MAX;
    for odd in [1usize, 3, 5, 15] {
        let mut l = 2 * odd;
        while l < want {
            l *= 2;
        }
        best = best.min(l);
    }
    best
}

/// FFT plans by length, shared by every thread (built once per length).
fn plan(l: usize) -> Arc<Fft> {
    static PLANS: OnceLock<Mutex<HashMap<usize, Arc<Fft>>>> = OnceLock::new();
    let mut plans = PLANS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    plans
        .entry(l)
        .or_insert_with(|| Arc::new(Fft::new(l).expect("a 2^p 3^q 5^r length")))
        .clone()
}

/// The buffers of the Fourier shift of one length.
pub struct FourierWork {
    l: usize,
    fft: Arc<Fft>,
    data: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    spec: Vec<[f32; 2]>,
    gspec: Vec<[f32; 2]>,
    /// `2π/L` rounded to float32.
    dk: f32,
}

impl FourierWork {
    pub fn new(l: usize) -> FourierWork {
        FourierWork {
            l,
            fft: plan(l),
            data: vec![Complex::new(0.0, 0.0); l],
            scratch: vec![Complex::new(0.0, 0.0); l],
            spec: vec![[0.0; 2]; l / 2 + 1],
            gspec: vec![[0.0; 2]; l / 2 + 1],
            dk: (2.0 * std::f64::consts::PI / l as f64) as f32,
        }
    }

    pub fn len(&self) -> usize {
        self.l
    }

    pub fn is_empty(&self) -> bool {
        self.l == 0
    }
}

#[inline]
fn cmul(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}

/// The phase factor `(cos φ, sin φ)` of a float32 angle, rounded to float32 (glibc's `sincos`
/// with `reproduce`).
fn unit(phi: f32, trig: Trig) -> [f32; 2] {
    let (s, c) = trig.sincos(f64::from(phi));
    [c as f32, s as f32]
}

/// Multiplies `spec[1..=L/2]` by the accumulated phase factors of a shift by `alpha` (§5.6
/// step 5), the real or the imaginary half selected by `which`.
fn phase(spec: &mut [[f32; 2]], half: usize, alpha: f32, dk: f32, trig: Trig) {
    let phi = -alpha * dk;
    let e = unit(phi, trig);
    let mut p = [1.0f32, 0.0];
    for s in spec.iter_mut().take(half + 1).skip(1) {
        p = cmul(p, e);
        *s = cmul(p, *s);
    }
    spec[half][1] = 0.0;
}

/// Shifts the rows `f` and `g` (the padded row length `n ≤ L`) by `a1` and `a2` with the
/// Fourier method (§5.6 steps 3 to 6), in place; `g` is `None` for a lone row.
pub fn shift_fourier_pair(
    work: &mut FourierWork,
    f: &mut [f32],
    mut g: Option<&mut [f32]>,
    a1: f32,
    a2: f32,
    trig: Trig,
) {
    let l = work.l;
    let n = f.len();
    let half = l / 2;
    match &g {
        Some(g) => {
            for ((d, &a), &b) in work.data.iter_mut().zip(f.iter()).zip(g.iter()) {
                *d = Complex::new(f64::from(a), f64::from(b));
            }
        }
        None => {
            for (d, &a) in work.data.iter_mut().zip(f.iter()) {
                *d = Complex::new(f64::from(a), 0.0);
            }
        }
    }
    for d in &mut work.data[n..] {
        *d = Complex::new(0.0, 0.0);
    }
    work.fft.forward(&mut work.data, &mut work.scratch);
    let x = |k: usize| -> [f32; 2] {
        let c = work.data[k];
        [c.re as f32, c.im as f32]
    };
    // Separate the two spectra (each twice its size): F in `spec`, G in `scratch`'s first half.
    let mut fs = std::mem::take(&mut work.spec);
    let mut gs = std::mem::take(&mut work.gspec);
    let x0 = x(0);
    fs[0] = [2.0 * x0[0], 0.0];
    gs[0] = [2.0 * x0[1], 0.0];
    for k in 1..half {
        let (a, b) = (x(k), x(l - k));
        fs[k] = [a[0] + b[0], a[1] - b[1]];
        gs[k] = [a[1] + b[1], -a[0] + b[0]];
    }
    let xh = x(half);
    fs[half] = [2.0 * xh[0], 0.0];
    gs[half] = [2.0 * xh[1], 0.0];
    phase(&mut fs, half, a1, work.dk, trig);
    phase(&mut gs, half, a2, work.dk, trig);
    // Rejoin and transform back.
    work.data[0] = Complex::new(f64::from(fs[0][0]), f64::from(gs[0][0]));
    for k in 1..half {
        let (fk, gk) = (fs[k], gs[k]);
        work.data[k] = Complex::new(f64::from(fk[0] - gk[1]), f64::from(fk[1] + gk[0]));
        work.data[l - k] = Complex::new(f64::from(fk[0] + gk[1]), f64::from(-fk[1] + gk[0]));
    }
    work.data[half] = Complex::new(f64::from(fs[half][0]), f64::from(gs[half][0]));
    work.spec = fs;
    work.gspec = gs;
    work.fft.inverse(&mut work.data, &mut work.scratch);
    let factor = 0.5f32 / l as f32;
    for m in 0..n {
        let c = work.data[m];
        f[m] = factor * (c.re as f32);
        if let Some(g) = g.as_deref_mut() {
            g[m] = factor * (c.im as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_shifts_move_samples() {
        let f: Vec<f32> = (0..10).map(|i| (i * i) as f32).collect();
        let mut out = vec![0f32; 10];
        for m in [
            Method::Linear,
            Method::Cubic,
            Method::Quintic,
            Method::Heptic,
        ] {
            shift_poly(m, &f, 2.0, &mut out);
            assert_eq!(&out[2..], &f[..8], "{m:?}");
            assert_eq!(&out[..2], &[0.0, 0.0], "{m:?}");
            shift_poly(m, &f, -3.0, &mut out);
            assert_eq!(&out[..7], &f[3..], "{m:?}");
        }
        // NN takes the floor of the position: a shift of +1 takes f[i - 2].
        shift_poly(Method::NN, &f, 1.0, &mut out);
        assert_eq!(out[5], f[3]);
        shift_poly(Method::NN, &f, 0.5, &mut out);
        assert_eq!(out[5], f[4]);
        shift_poly(Method::Heptic, &f, 10.5, &mut out);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn lengths() {
        assert_eq!(fourier_length(72, 0.0), 80);
        assert_eq!(fourier_length(64, 1.0), 64);
        assert_eq!(fourier_length(65, 0.0), 80);
        assert_eq!(fourier_length(97, 0.0), 120);
        assert_eq!(fourier_length(1, 0.0), 2);
    }

    #[test]
    fn fourier_shifts_a_pair() {
        let n = 30;
        let l = fourier_length(n, 0.0);
        let mut w = FourierWork::new(l);
        let base: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 - 15.0;
                100.0 / (1.0 + 0.05 * t * t)
            })
            .collect();
        let (mut f, mut g) = (base.clone(), base.clone());
        shift_fourier_pair(&mut w, &mut f, Some(&mut g), 1.0, -2.0, Trig::Correct);
        for i in 5..25 {
            assert!((f[i] - base[i - 1]).abs() < 1e-3, "{i}");
            assert!((g[i] - base[i + 2]).abs() < 1e-3, "{i}");
        }
    }
}

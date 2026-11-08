// SPDX-License-Identifier: Apache-2.0
//! The Gaussian blurs of 3dvolreg (`specs/3dvolreg.md` §6): the FIR filter with AFNI's taps,
//! half-widths, row-end mirrors and sum orders (float32, exact), and the FFT blur of wide
//! kernels.
//!
//! The x blur filters each row through a mirrored line buffer; the y and z blurs combine whole
//! x lines (the taps are the same for every row), so their inner loops run over contiguous
//! samples with each output's terms in the spec's order.

use rayon::prelude::*;

use larmorx_core::Complex;

use super::rotate::Trig;
use super::shift;
use crate::fft::Fft;

/// The FIR settings: `AFNI_BLUR_FIRFAC` and `AFNI_BLUR_FIROLD`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlurSettings {
    /// `AFNI_BLUR_FIRFAC` as read (`None` when unset or not a number).
    pub firfac: Option<f64>,
    pub old: bool,
    pub trig: Trig,
}

impl BlurSettings {
    /// The factor F of the program's main blurs (a value ≤ 0 keeps 2.5).
    pub fn main_factor(&self) -> f64 {
        match self.firfac {
            Some(f) if f > 0.0 => f,
            _ => 2.5,
        }
    }

    /// The factor of the coarse search's blur (a value below 2 keeps 2.5).
    pub fn coarse_factor(&self) -> f64 {
        match self.firfac {
            Some(f) if f >= 2.0 => f,
            _ => 2.5,
        }
    }
}

/// The half-width and normalised taps `w_0 … w_m` of a Gaussian of σ over steps of δ (§6.1).
pub fn taps(sigma: f32, delta: f32, factor: f64, s: &BlurSettings) -> Vec<f32> {
    let m = half_width(sigma, delta, factor);
    let u = delta / sigma;
    let fi = |i: usize| f64::from(i as f32 * u);
    let mut w: Vec<f32> = Vec::with_capacity(m + 1);
    if s.old {
        w.push(1.0);
        let mut norm = 1.0f32;
        for i in 1..=m {
            let x = fi(i);
            let v = s.trig.exp(-0.5 * x * x) as f32;
            w.push(v);
            norm += 2.0 * v;
        }
        let r = 1.0f32 / norm;
        return w.into_iter().map(|v| v * r).collect();
    }
    let du = f64::from(u) / (2.0 * 11.0);
    let mut norm = 0.0f32;
    for i in 0..=m {
        let base = fi(i);
        let mut sum = 0.0f64;
        for j in -11i32..=11 {
            let x = base + f64::from(j) * du;
            let c = if j.abs() == 11 { 0.5 } else { 1.0 };
            sum += c * s.trig.exp(-0.5 * x * x);
        }
        w.push(sum as f32);
        norm = if i == 0 {
            (f64::from(norm) + sum) as f32
        } else {
            (f64::from(norm) + 2.0 * sum) as f32
        };
    }
    let r = 1.0f32 / norm;
    w.into_iter().map(|v| v * r).collect()
}

/// `m = ceil(F·σ/δ)`, at least 1.
pub fn half_width(sigma: f32, delta: f32, factor: f64) -> usize {
    let m = (factor * f64::from(sigma) / f64::from(delta)).ceil();
    if m.is_finite() && m >= 1.0 {
        m as usize
    } else {
        1
    }
}

/// How a row's ends are extended (§6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ends {
    /// About the first sample at the start, half a sample past the last at the end (x, y).
    HalfEnd,
    /// About both end samples (z).
    Both,
    /// One mirrored sample at each end, zeros beyond (the y blur when nx ≥ 512).
    One,
}

/// The input index for position `p` (may be outside `0..n`) of a row of `n`, or `None` (zero).
#[inline]
fn source(p: i64, n: usize, ends: Ends) -> Option<usize> {
    let n = n as i64;
    if (0..n).contains(&p) {
        return Some(p as usize);
    }
    let q = if p < 0 {
        match ends {
            Ends::One if p < -1 => return None,
            _ => -p,
        }
    } else {
        let over = p - (n - 1);
        match ends {
            Ends::HalfEnd => n - over,
            Ends::Both => n - 1 - over,
            Ends::One if over > 1 => return None,
            Ends::One => n - 2,
        }
    };
    (0..n).contains(&q).then_some(q as usize)
}

/// The FIR sums of §6.1 for a whole line at once: `out[i]` from the centre line `c` and, for
/// `q = 1 … m`, the lines `lines(q) = (r[· − q], r[· + q])`; every output's terms in the spec's
/// order (for `m ≤ 7` the outermost pair first and the centre last, else the centre first),
/// each pair summed before its product. `acc` is a line of scratch.
fn fir_lines<'a>(
    w: &[f32],
    c: &[f32],
    lines: impl Fn(usize) -> (&'a [f32], &'a [f32]),
    out: &mut [f32],
    acc: &mut [f32],
) {
    let m = w.len() - 1;
    let n = out.len();
    let acc = &mut acc[..n];
    if m <= 7 {
        let (a, b) = lines(m);
        for i in 0..n {
            acc[i] = w[m] * (a[i] + b[i]);
        }
        for q in (1..m).rev() {
            let (a, b) = lines(q);
            for i in 0..n {
                acc[i] += w[q] * (a[i] + b[i]);
            }
        }
        for i in 0..n {
            out[i] = acc[i] + w[0] * c[i];
        }
    } else {
        for i in 0..n {
            acc[i] = w[0] * c[i];
        }
        for (q, &wq) in w.iter().enumerate().skip(1) {
            let (a, b) = lines(q);
            for i in 0..n {
                acc[i] += wq * (a[i] + b[i]);
            }
        }
        out.copy_from_slice(acc);
    }
}

/// Blurs the rows of `v` along x with taps `w` (in place), a plane per task.
fn blur_x(v: &mut [f32], dims: [usize; 3], w: &[f32]) {
    let nx = dims[0];
    let m = w.len() - 1;
    if nx < m + 1 {
        return;
    }
    v.par_chunks_mut(nx * dims[1]).for_each_init(
        || (vec![0f32; nx + 2 * m], vec![0f32; nx]),
        |(ext, acc), plane| {
            for row in plane.chunks_exact_mut(nx) {
                for (p, e) in ext.iter_mut().enumerate() {
                    let s = source(p as i64 - m as i64, nx, Ends::HalfEnd);
                    *e = s.map_or(0.0, |s| row[s]);
                }
                let ext: &[f32] = ext;
                let lines = |q: usize| (&ext[m - q..m - q + nx], &ext[m + q..m + q + nx]);
                fir_lines(w, &ext[m..m + nx], lines, row, acc);
            }
        },
    );
}

/// Blurs along y (in place), a plane per task: each output x line from the plane's lines.
fn blur_y(v: &mut [f32], dims: [usize; 3], w: &[f32], ends: Ends) {
    let [nx, ny, _] = dims;
    let m = w.len() - 1;
    if ny < m + 1 {
        return;
    }
    let zeros = vec![0f32; nx];
    v.par_chunks_mut(nx * ny).for_each_init(
        || (vec![0f32; nx * ny], vec![0f32; nx]),
        |(copy, acc), plane| {
            copy.copy_from_slice(plane);
            let copy: &[f32] = copy;
            let line = |p: i64| -> &[f32] {
                source(p, ny, ends).map_or(&zeros[..], |s| &copy[nx * s..nx * (s + 1)])
            };
            for (j, out) in plane.chunks_exact_mut(nx).enumerate() {
                let t = j as i64;
                let lines = |q: usize| (line(t - q as i64), line(t + q as i64));
                fir_lines(w, &copy[nx * j..nx * (j + 1)], lines, out, acc);
            }
        },
    );
}

/// The columns per task of the z blur.
const COLUMNS: usize = 1024;

/// Blurs along z (in place): blocks of columns (the same voxels of every plane) gathered into
/// a buffer of their own, blurred along z, written back.
fn blur_z(v: &mut [f32], dims: [usize; 3], w: &[f32], ends: Ends) {
    let [nx, ny, nz] = dims;
    let m = w.len() - 1;
    if nz < m + 1 {
        return;
    }
    let plane = nx * ny;
    let blocks = plane.div_ceil(COLUMNS);
    let mut parts: Vec<Vec<&mut [f32]>> = (0..blocks).map(|_| Vec::with_capacity(nz)).collect();
    for pl in v.chunks_mut(plane) {
        for (b, part) in pl.chunks_mut(COLUMNS).enumerate() {
            parts[b].push(part);
        }
    }
    parts.into_par_iter().for_each(|mut slices| {
        let len = slices[0].len();
        let mut copy = vec![0f32; nz * len];
        for (k, sl) in slices.iter().enumerate() {
            copy[k * len..(k + 1) * len].copy_from_slice(sl);
        }
        let zeros = vec![0f32; len];
        let mut acc = vec![0f32; len];
        let line = |p: i64| -> &[f32] {
            source(p, nz, ends).map_or(&zeros[..], |s| &copy[len * s..len * (s + 1)])
        };
        for (k, out) in slices.iter_mut().enumerate() {
            let t = k as i64;
            let lines = |q: usize| (line(t - q as i64), line(t + q as i64));
            fir_lines(w, &copy[len * k..len * (k + 1)], lines, out, &mut acc);
        }
    });
}

/// The FIR blur of §6.1 with σ (mm) per axis over steps `delta` (mm), factor F, and an optional
/// cap of the half-widths at `n/2` (the coarse search's). Axes of one voxel are skipped.
pub fn fir(
    v: &mut [f32],
    dims: [usize; 3],
    sigma: [f32; 3],
    delta: [f32; 3],
    factor: f64,
    cap: bool,
    s: &BlurSettings,
) {
    for axis in 0..3 {
        if dims[axis] < 2 {
            continue;
        }
        let mut w = taps(sigma[axis], delta[axis], factor, s);
        if cap {
            let limit = (dims[axis] / 2).max(1);
            if w.len() - 1 > limit {
                w = taps_capped(sigma[axis], delta[axis], limit, s);
            }
        }
        match axis {
            0 => blur_x(v, dims, &w),
            1 => {
                let ends = if dims[0] >= 512 {
                    Ends::One
                } else {
                    Ends::HalfEnd
                };
                blur_y(v, dims, &w, ends);
            }
            _ => blur_z(v, dims, &w, Ends::Both),
        }
    }
}

/// The taps of a half-width given (the coarse search's cap): the same sums over `0..=m`.
fn taps_capped(sigma: f32, delta: f32, m: usize, s: &BlurSettings) -> Vec<f32> {
    // The factor that gives exactly this half-width: the taps depend only on m and u.
    let u = delta / sigma;
    let fi = |i: usize| f64::from(i as f32 * u);
    let du = f64::from(u) / (2.0 * 11.0);
    let mut w = Vec::with_capacity(m + 1);
    let mut norm = 0.0f32;
    for i in 0..=m {
        let base = fi(i);
        let mut sum = 0.0f64;
        for j in -11i32..=11 {
            let x = base + f64::from(j) * du;
            let c = if j.abs() == 11 { 0.5 } else { 1.0 };
            sum += c * s.trig.exp(-0.5 * x * x);
        }
        w.push(sum as f32);
        norm = if i == 0 {
            (f64::from(norm) + sum) as f32
        } else {
            (f64::from(norm) + 2.0 * sum) as f32
        };
    }
    let r = 1.0f32 / norm;
    w.into_iter().map(|v| v * r).collect()
}

/// The FFT blur of §6.2 along every axis (σ and δ the same in voxel units on every axis), then
/// every value clipped to the range of the volume before the blur.
pub fn fft_blur(v: &mut [f32], dims: [usize; 3], sigma: f32, delta: f32, trig: Trig) {
    let (lo, hi) = v
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| {
            (a.min(x), b.max(x))
        });
    let [nx, ny, nz] = dims;
    for axis in 0..3 {
        let n = dims[axis];
        if n < 2 {
            continue;
        }
        let extra = (3.0 * f64::from(sigma) / f64::from(delta)).trunc() as usize;
        let l = shift::fourier_length(n + extra, 0.0);
        let fft = Fft::new(l).expect("a 2^p 3^q 5^r length");
        let dk = (2.0 * std::f64::consts::PI / (l as f64 * f64::from(delta))) as f32;
        let s2 = sigma * sigma * 0.5f32;
        let gains: Vec<f32> = (0..=l / 2)
            .map(|k| {
                let kd = k as f32 * dk;
                let arg = s2 * (kd * kd);
                ((1.0 / l as f64) * trig.exp(-f64::from(arg))) as f32
            })
            .collect();
        let stride = [1, nx, nx * ny][axis];
        let rows: Vec<usize> = (0..nx * ny * nz)
            .filter(|&p| {
                let c = [p % nx, (p / nx) % ny, p / (nx * ny)];
                c[axis] == 0
            })
            .collect();
        let src = v.to_vec();
        let results: Vec<(usize, Vec<f32>)> = rows
            .par_chunks(2)
            .flat_map_iter(|pair| {
                let mut data = vec![Complex::new(0.0f64, 0.0); l];
                let mut scratch = data.clone();
                for (m, d) in data.iter_mut().enumerate().take(n) {
                    let re = f64::from(src[pair[0] + stride * m]);
                    let im = pair.get(1).map_or(0.0, |&p| f64::from(src[p + stride * m]));
                    *d = Complex::new(re, im);
                }
                fft.forward(&mut data, &mut scratch);
                for k in 0..l {
                    let g = gains[k.min(l - k)];
                    let c = data[k];
                    data[k] = Complex::new(f64::from(c.re as f32 * g), f64::from(c.im as f32 * g));
                }
                fft.inverse(&mut data, &mut scratch);
                let mut out = Vec::new();
                out.push((pair[0], (0..n).map(|m| data[m].re as f32).collect()));
                if let Some(&p) = pair.get(1) {
                    out.push((p, (0..n).map(|m| data[m].im as f32).collect()));
                }
                out
            })
            .collect();
        for (start, row) in results {
            for (m, x) in row.into_iter().enumerate() {
                v[start + stride * m] = x;
            }
        }
    }
    for x in v.iter_mut() {
        *x = x.clamp(lo, hi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> BlurSettings {
        BlurSettings {
            firfac: None,
            old: false,
            trig: Trig::Correct,
        }
    }

    #[test]
    fn taps_are_normalised() {
        let w = taps(9.0, 3.0, 2.5, &settings());
        assert_eq!(w.len(), 9);
        let total: f32 = w[0] + 2.0 * w[1..].iter().sum::<f32>();
        assert!((total - 1.0).abs() < 1e-5);
        assert_eq!(half_width(2.0, 1.0, 2.5), 5);
    }

    #[test]
    fn mirrors() {
        assert_eq!(source(-2, 5, Ends::HalfEnd), Some(2));
        assert_eq!(source(5, 5, Ends::HalfEnd), Some(4));
        assert_eq!(source(6, 5, Ends::HalfEnd), Some(3));
        assert_eq!(source(5, 5, Ends::Both), Some(3));
        assert_eq!(source(-1, 5, Ends::One), Some(1));
        assert_eq!(source(-2, 5, Ends::One), None);
        assert_eq!(source(5, 5, Ends::One), Some(3));
        assert_eq!(source(6, 5, Ends::One), None);
    }

    #[test]
    fn a_constant_stays_constant() {
        let dims = [10, 9, 8];
        let mut v = vec![3.0f32; 720];
        fir(&mut v, dims, [2.0; 3], [1.0; 3], 2.5, false, &settings());
        assert!(v.iter().all(|&x| (x - 3.0).abs() < 1e-4));
    }
}

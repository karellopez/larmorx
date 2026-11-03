// SPDX-License-Identifier: Apache-2.0
//! fslmaths' ROC analysis (`specs/fslmaths.md` §15): `-roc L curve [noise] truth` tests the
//! current image (one volume per realisation) against a truth image, at thresholds from its
//! largest log-transformed value down, and writes the curve and the area under it up to the
//! false-positive rate `|L|`. The current image is unchanged.
//!
//! Black-box runs of FSL 6.0.7.23 give the arithmetic the spec leaves open (findings 19 of
//! `docs/findings/fsl-maths-original.md`, `validation/investigations/cleanroom-maths/fit_roc.py`):
//! the noise image's counts take every voxel of a noise volume (the truth's mask is not applied
//! to them); the rates' means are double sums rounded to single precision; with the double type
//! the log values are compared with the thresholds in single precision; each volume's area
//! takes the common false-positive rate as its axis; a run whose rate never reaches `|L|` adds
//! the rectangle from the last rate to `|L|` at the last true-positive rate; the quartiles are
//! taken of the volumes' areas before the division by `|L|`.
//!
//! Built for speed: each volume's log values over the true voxels, the false-positive
//! candidates and the noise volumes are sorted once, so a threshold's counts are binary
//! searches instead of passes over the image (the counts are those of the comparisons `v >= h`,
//! exactly).
//!
//! Defects: FSL-153 (a NaN first voxel makes the extremes NaN), FSL-179 (a noise image of other
//! sizes), FSL-180 (no true voxels, or no zero voxels without a noise image).

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use rayon::prelude::*;

use super::filters::abort_lines;
use super::image::Img;
use super::num::{self, Kind, Num};
use super::ops::{self, Stop};
use crate::cli::fmt;

/// The noise-only image (single precision, FSL's frame) and its sizes.
pub struct Noise {
    pub data: Vec<f32>,
    pub dims: [usize; 4],
}

/// What `-roc` gives: the curve file's text, and either the line it prints on standard output
/// or FSL's abort (FSL-180's sort of NaN areas), printed after the curve is written.
pub struct Roc {
    pub curve: String,
    pub result: Result<String, String>,
}

/// The truth's mask: values in `[-0.001, FLT_MAX]`.
const MASK_LOW: f32 = -0.001;
/// The true voxels: at least this fraction of the truth's maximum.
const TRUE_FRACTION: f64 = 0.05;

/// A count as FSL's single-precision sum of ones gives it (exact to 2^24, which a sum of ones
/// never passes).
fn count_f32(n: usize) -> f32 {
    n.min(1 << 24) as f32
}

/// `a / b` in single precision, x86-64's NaN for 0 / 0.
fn div32(a: f32, b: f32) -> f32 {
    num::nan32(a, b, a / b)
}

/// The number of values `>= h` in an ascending slice of values that are not NaN.
fn at_or_above(sorted: &[f32], h: f32) -> usize {
    sorted.len() - sorted.partition_point(|&v| v < h)
}

/// The values of `data` at `idx` (in order) that are not NaN, sorted ascending.
fn sorted_at(data: &[f32], idx: impl Iterator<Item = usize>) -> Vec<f32> {
    let mut v: Vec<f32> = idx.map(|k| data[k]).filter(|v| !v.is_nan()).collect();
    v.sort_unstable_by(f32::total_cmp);
    v
}

/// The mean of the rates: a double sum rounded to single precision (observed).
fn mean32(v: &[f64]) -> f64 {
    let mut s = 0.0f64;
    for &x in v {
        s = num::nan64(s, x, s + x);
    }
    let n = v.len() as f64;
    num::to_f64(num::to_f32(num::nan64(s, n, s / n)))
}

/// The standard deviation with `N - 1` (two passes, double precision); 0 for one value.
fn sd(v: &[f64]) -> f64 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    let mut s = 0.0f64;
    for &x in v {
        s = num::nan64(s, x, s + x);
    }
    let m = num::nan64(s, n as f64, s / n as f64);
    let mut q = 0.0f64;
    for &x in v {
        let d = num::nan64(x, m, x - m);
        q = num::nan64(q, d, q + d * d);
    }
    let var = num::nan64(q, (n - 1) as f64, q / (n - 1) as f64);
    if var.is_nan() { var } else { var.sqrt() }
}

/// `std::min(a, b)` and `std::max(a, b)`: a NaN second value never replaces the first.
fn min_of(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}
fn max_of(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// The extremes of `data` as the mode takes them (FSL-153: from the first value with
/// `reproduce`; NaN values left out by default), and whether the two differ.
fn extremes_of<T: Num>(data: &[T], fix: bool) -> (Option<(T, T)>, bool) {
    let rep = ops::extremes(data, false);
    if !fix {
        return (rep, false);
    }
    let got = ops::extremes(data, true);
    let differs = match (rep, got) {
        (Some((a, b)), Some((c, d))) => !(a.same(c) && b.same(d)),
        _ => false,
    };
    (got, differs)
}

/// `ln(v - minval + 1)` in single precision: `logf` for the single-precision and integer
/// computing types, double `log` rounded to single for a double image (its comparisons with
/// the thresholds are in single precision: observed).
fn log_values<T: Num>(data: &[T], m: f64) -> Vec<f32> {
    data.par_iter()
        .map(|&v| match T::KIND {
            Kind::Double => {
                let x = v.to_f64();
                let a = num::nan64(x, m, x - m);
                let a = num::nan64(a, 1.0, a + 1.0);
                num::to_f32(log64(a))
            }
            Kind::Float => {
                let x = num::to_f32(v.to_f64());
                let mf = num::to_f32(m);
                let a = num::nan32(x, mf, x - mf);
                logf32(num::nan32(a, 1.0, a + 1.0))
            }
            _ => logf32((v.to_f64() - m + 1.0) as f32),
        })
        .collect()
}

/// `logf` (correctly rounded), x86-64's NaN for a negative argument.
fn logf32(x: f32) -> f32 {
    let r = math::logf(x);
    if r.is_nan() && !x.is_nan() {
        f32::from_bits(num::DEFAULT_NAN_32)
    } else {
        r
    }
}

/// `log` (correctly rounded), x86-64's NaN for a negative argument.
fn log64(x: f64) -> f64 {
    let r = math::log(x);
    if r.is_nan() && !x.is_nan() {
        num::default_nan()
    } else {
        r
    }
}

/// The trapezoid between `(x0, y0)` and `(x1, y1)`, or, when the step passes `limit`, its part
/// up to `limit` (`y` interpolated linearly there); single precision.
fn trapezoid32(x0: f32, y0: f32, x1: f32, y1: f32, limit: f32, cross: bool) -> f32 {
    let half = |w: f32, a: f32, b: f32| {
        let s = num::nan32(a, b, a + b);
        let p = num::nan32(w, s, w * s);
        num::nan32(p, 2.0, p / 2.0)
    };
    if !cross {
        return half(num::nan32(x1, x0, x1 - x0), y0, y1);
    }
    let w = num::nan32(limit, x0, limit - x0);
    let dx = num::nan32(x1, x0, x1 - x0);
    let dy = num::nan32(y1, y0, y1 - y0);
    let t = num::nan32(dy, w, dy * w);
    let t = num::nan32(t, dx, t / dx);
    let yl = num::nan32(y0, t, y0 + t);
    half(w, y0, yl)
}

/// [`trapezoid32`] in double precision.
fn trapezoid64(x0: f64, y0: f64, x1: f64, y1: f64, limit: f64, cross: bool) -> f64 {
    let half = |w: f64, a: f64, b: f64| {
        let s = num::nan64(a, b, a + b);
        let p = num::nan64(w, s, w * s);
        num::nan64(p, 2.0, p / 2.0)
    };
    if !cross {
        return half(num::nan64(x1, x0, x1 - x0), y0, y1);
    }
    let w = num::nan64(limit, x0, limit - x0);
    let dx = num::nan64(x1, x0, x1 - x0);
    let dy = num::nan64(y1, y0, y1 - y0);
    let t = num::nan64(dy, w, dy * w);
    let t = num::nan64(t, dx, t / dx);
    let yl = num::nan64(y0, t, y0 + t);
    half(w, y0, yl)
}

/// The quartile at `p` percent of ascending values, by linear interpolation of the values at
/// `100 (i - 0.5) / n` percent, the extremes at 0 % and 100 % (single precision).
fn quartile(sorted: &[f32], p: f32) -> f32 {
    let n = sorted.len();
    let pos = |i: usize| 100.0f32 * (i as f32 - 0.5) / n as f32;
    let mut xs = vec![0.0f32];
    let mut ys = vec![sorted[0]];
    for (i, &v) in sorted.iter().enumerate() {
        xs.push(pos(i + 1));
        ys.push(v);
    }
    xs.push(100.0);
    ys.push(sorted[n - 1]);
    let j = xs.iter().rposition(|&x| x <= p).unwrap_or(0);
    if xs[j] == p || j + 1 >= xs.len() {
        return ys[j];
    }
    let (x0, x1, y0, y1) = (xs[j], xs[j + 1], ys[j], ys[j + 1]);
    let dy = num::nan32(y1, y0, y1 - y0);
    let t = num::nan32(dy, p - x0, dy * (p - x0));
    num::nan32(y0, t, y0 + t / (x1 - x0))
}

/// `-roc` on the current image (§15). `limit` is `L` as read (`< 0`: no noise image); `noise`
/// and `truth` are read in single precision (the truth's first volume, checked to have the
/// input's x, y and z sizes by the caller).
pub fn roc<T: Num>(
    img: &Img<T>,
    limit: f64,
    noise: Option<&mut Noise>,
    truth: &[f32],
    notices: &mut Notices,
) -> Result<Roc, Stop> {
    let fix = notices.fix();
    let [nx, ny, nz, nt] = img.dims;
    let nvox = nx * ny * nz;
    let k = nvox * nt;
    // FSL-179: the noise image is indexed with the input's sizes
    let mut noise = noise;
    if let Some(n) = noise.as_deref()
        && n.dims != img.dims
    {
        let [a, b, c, d] = n.dims;
        if fix || n.data.len() < k {
            notices.add(
                &defects::FSL_179,
                format!(
                    "-roc with a noise image of {a} x {b} x {c} x {d} voxels for an input of \
                     {nx} x {ny} x {nz} x {nt} is refused (FSL indexes it with the input's sizes)"
                ),
            );
            let why = if fix {
                "FSL-179: FSL indexes the noise image with the input's sizes"
            } else {
                "FSL-179: FSL reads and writes outside the noise image here"
            };
            return Err(Stop::Refuse(format!(
                "-roc: the noise image has {a} x {b} x {c} x {d} voxels, the input {nx} x {ny} x \
                 {nz} x {nt} ({why})"
            )));
        }
        notices.add(
            &defects::FSL_179,
            format!(
                "-roc with a noise image of {a} x {b} x {c} x {d} voxels for an input of {nx} x \
                 {ny} x {nz} x {nt}: indexed with the input's sizes, as FSL does"
            ),
        );
    }
    // the extremes (FSL-153)
    let (ext, nan_first) = extremes_of(&img.data, fix);
    let Some((minval, _)) = ext else {
        return Err(Stop::Refuse("-roc: the image has no voxels".into()));
    };
    let mut minval_d = minval.to_f64();
    if let Some(n) = noise.as_deref()
        && let Some((lo, _)) = ops::extremes(&n.data, fix)
    {
        minval_d = min_of(minval_d, num::to_f64(lo));
    }
    let lg = log_values(&img.data, minval_d);
    let (lext, lnan) = extremes_of(&lg, fix);
    let mut maxlog = lext.map_or(0.0, |e| e.1);
    let mf = num::to_f32(minval_d);
    if let Some(n) = noise.as_deref_mut() {
        let one = 1.0f32;
        n.data[..k].par_iter_mut().for_each(|v| {
            let a = num::nan32(*v, mf, *v - mf);
            *v = logf32(num::nan32(a, one, a + one));
        });
        if let Some((_, hi)) = ops::extremes(&n.data, fix) {
            maxlog = max_of(maxlog, hi);
        }
    }
    let raw = num::nan32(maxlog, 1000.0, maxlog / 1000.0);
    let delta = if raw == 0.0 { 1.0 } else { raw };
    // the truth: the mask, the true voxels and the false-positive candidates
    let (text, tnan) = extremes_of(truth, fix);
    let tmax = text.map_or(0.0, |e| e.1);
    let bound = num::to_f32(TRUE_FRACTION * num::to_f64(tmax));
    let inside = |v: f32| (MASK_LOW..=f32::MAX).contains(&v);
    let true_idx: Vec<usize> = (0..nvox)
        .filter(|&i| inside(truth[i]) && truth[i] >= bound)
        .collect();
    // the candidates: the mask's voxels that are not true, `(1 − binarised truth) × mask` with
    // the truth binarised at the true voxels' bound (random testing, 2026-11-03: a truth of
    // values between 0 and its bound counts them; one of zeros only has no candidate)
    let cand_idx: Vec<usize> = (0..nvox)
        .filter(|&i| inside(truth[i]) && !(truth[i] >= bound))
        .collect();
    if fix && (nan_first || lnan || tnan) {
        notices.add(
            &defects::FSL_153,
            "-roc: the extremes of the values that are not NaN (FSL's are NaN: a first voxel is \
             NaN)",
        );
    }
    if fix && (true_idx.is_empty() || (noise.is_none() && cand_idx.is_empty())) {
        let what = if true_idx.is_empty() {
            "no true voxels (none at or above 0.05 of its maximum)"
        } else {
            "no zero voxels for the false-positive rate (and no noise image)"
        };
        notices.add(
            &defects::FSL_180,
            format!("-roc with a truth image of {what} is refused (FSL writes NaN rates)"),
        );
        return Err(Stop::Refuse(format!(
            "-roc: the truth image has {what} (FSL-180: FSL divides 0 by 0 for the rates and \
             aborts in its quartiles' sort)"
        )));
    }
    let ntrue = count_f32(true_idx.len());
    let ncand = count_f32(cand_idx.len());
    // each volume's log values, sorted, over the true voxels and the candidates; each noise
    // volume's over all its voxels (the input's sizes)
    let vols: Vec<(Vec<f32>, Vec<f32>)> = (0..nt)
        .into_par_iter()
        .map(|t| {
            let v = &lg[t * nvox..(t + 1) * nvox];
            let tr = sorted_at(v, true_idx.iter().copied());
            let fc = if noise.is_none() {
                sorted_at(v, cand_idx.iter().copied())
            } else {
                Vec::new()
            };
            (tr, fc)
        })
        .collect();
    let noise_vols: Vec<Vec<f32>> = match noise.as_deref() {
        Some(n) => (0..nt)
            .into_par_iter()
            .map(|t| sorted_at(&n.data[t * nvox..(t + 1) * nvox], 0..nvox))
            .collect(),
        None => Vec::new(),
    };
    let nvox_f = count_f32(nvox);
    let al = limit.abs();
    let al32 = num::to_f32(al);
    let mut curve = String::new();
    let mut h = num::nan32(maxlog, delta, maxlog + delta);
    let (mut fp, mut tp) = (0.0f64, 0.0f64);
    let mut tps = vec![0.0f64; nt];
    let mut area = 0.0f32;
    let mut vareas = vec![0.0f64; nt];
    while h > 0.0 && fp < al {
        let tp_t: Vec<f64> = vols
            .iter()
            .map(|(tr, _)| num::to_f64(div32(count_f32(at_or_above(tr, h)), ntrue)))
            .collect();
        let e = math::expf(h);
        let thr = num::nan32(e, 1.0, e - 1.0);
        let thr = fmt::g(num::to_f64(num::nan32(thr, mf, thr + mf)));
        let new_fp = if noise.is_some() {
            let counts: Vec<usize> = noise_vols.iter().map(|v| at_or_above(v, h)).collect();
            let hits = counts.iter().filter(|&&c| c > 0).count();
            let fwe = num::to_f64(div32(count_f32(hits), count_f32(nt)));
            let rates: Vec<f64> = counts
                .iter()
                .map(|&c| num::to_f64(div32(count_f32(c), nvox_f)))
                .collect();
            let line = [fwe, mean32(&tp_t), sd(&rates), sd(&tp_t)];
            for x in line {
                curve.push_str(&fmt::g(x));
                curve.push(' ');
            }
            curve.push_str(&thr);
            curve.push('\n');
            fwe
        } else {
            let fp_t: Vec<f64> = vols
                .iter()
                .map(|(_, fc)| num::to_f64(div32(count_f32(at_or_above(fc, h)), ncand)))
                .collect();
            for (a, b) in fp_t.iter().zip(&tp_t) {
                curve.push_str(&format!("{} {} {thr}\n", fmt::g(*a), fmt::g(*b)));
            }
            mean32(&fp_t)
        };
        let new_tp = mean32(&tp_t);
        let cross = !(new_fp < al);
        area = num::nan32(
            area,
            0.0,
            area + trapezoid32(
                num::to_f32(fp),
                num::to_f32(tp),
                num::to_f32(new_fp),
                num::to_f32(new_tp),
                al32,
                cross,
            ),
        );
        for ((a, &p), &q) in vareas.iter_mut().zip(&tps).zip(&tp_t) {
            let s = trapezoid64(fp, p, new_fp, q, al, cross);
            *a = num::nan64(*a, s, *a + s);
        }
        (fp, tp) = (new_fp, new_tp);
        tps = tp_t;
        h = num::nan32(h, delta, h - delta);
    }
    if fp < al {
        // the rate never reached |L|: the rectangle up to it at the last true-positive rate
        let w = num::nan64(al, fp, al - fp);
        let r = num::to_f32(num::nan64(w, tp, w * tp));
        area = num::nan32(area, r, area + r);
        for (a, &p) in vareas.iter_mut().zip(&tps) {
            let r = num::nan64(w, p, w * p);
            *a = num::nan64(*a, r, *a + r);
        }
    }
    if vareas.iter().any(|v| v.is_nan()) {
        // FSL-180 reproduced (or a limit that is NaN): the quartiles' sort detects NaN
        return Ok(Roc {
            curve,
            result: Err(abort_lines("std::logic_error", "sort(): detected NaN")),
        });
    }
    let mut sorted: Vec<f32> = vareas.iter().map(|&v| num::to_f32(v)).collect();
    sorted.sort_unstable_by(f32::total_cmp);
    let over = |x: f32| {
        let x = num::to_f64(x);
        fmt::g(num::nan64(x, al, x / al))
    };
    let line = format!(
        "{} {} {}\n",
        over(area),
        over(quartile(&sorted, 25.0)),
        over(quartile(&sorted, 75.0))
    );
    Ok(Roc {
        curve,
        result: Ok(line),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quartiles_and_spreads() {
        // three volumes: positions 16.7, 50, 83.3 percent (values of a black-box run)
        let v = [0.17857143f32, 0.2857143, 0.28839287];
        assert_eq!(fmt::g(f64::from(quartile(&v, 25.0))), "0.205357");
        assert_eq!(fmt::g(f64::from(quartile(&v, 75.0))), "0.287723");
        // two volumes: the values themselves; one: its value
        assert_eq!(quartile(&[0.25, 0.75], 25.0), 0.25);
        assert_eq!(quartile(&[0.5], 75.0), 0.5);
        assert_eq!(sd(&[0.5]), 0.0);
        let r = [1.0 / 336.0, 0.0, 0.0].map(|x: f64| f64::from(x as f32));
        assert_eq!(fmt::g(sd(&r)), "0.0017183");
        assert_eq!(at_or_above(&[1.0, 2.0, 2.0, 3.0], 2.0), 3);
        assert_eq!(count_f32(20_000_000), 16_777_216.0);
        assert!(div32(0.0, 0.0).to_bits() == num::DEFAULT_NAN_32);
    }
}

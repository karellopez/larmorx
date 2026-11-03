// SPDX-License-Identifier: Apache-2.0
//! fslmaths' regions and temporal filtering (`specs/fslmaths.md` §13): `-roi` and `-bptf`.
//!
//! `-bptf` filters every voxel's series on its own (in parallel; each series' sums keep their
//! order, so the result does not depend on the thread count). FSL-171: a sigma whose filter
//! half-width is not a valid `int`.

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use rayon::prelude::*;

use super::image::Img;
use super::num::{self, Num};
use super::ops::Stop;

/// `-roi x₀ nx y₀ ny z₀ nz t₀ nt` (§13.1): every voxel outside the box becomes 0. The x, y, z
/// corners are file coordinates (mapped to the radiological frame of a mirrored image); a size
/// of −1 reaches the axis' last voxel, 0 is an empty range, the corners are put in order.
pub fn roi<T: Num>(img: &mut Img<T>, a: [i32; 8]) {
    let dims = img.dims;
    let mut lo = [0i64; 4];
    let mut hi = [-1i64; 4];
    for k in 0..4 {
        let first = i64::from(a[2 * k]);
        let size = i64::from(a[2 * k + 1]);
        if size == 0 {
            continue;
        }
        let last = if size == -1 {
            dims[k] as i64 - 1
        } else {
            first + size - 1
        };
        let (mut f, mut l) = (first, last);
        if k == 0 && img.flipped {
            let n = dims[0] as i64;
            f = n - 1 - f;
            l = n - 1 - l;
        }
        lo[k] = f.min(l);
        hi[k] = f.max(l);
    }
    let [nx, ny, nz, _] = dims;
    let inside = |k: usize, c: usize| (c as i64) >= lo[k] && (c as i64) <= hi[k];
    let plane = nx * ny;
    img.data
        .par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(p, s)| {
            let (z, t) = (p % nz.max(1), p / nz.max(1));
            let zt = inside(2, z) && inside(3, t);
            for y in 0..ny {
                let row = zt && inside(1, y);
                for x in 0..nx {
                    if !(row && inside(0, x)) {
                        s[x + nx * y] = T::zero();
                    }
                }
            }
        });
}

/// A filter's half-width (§13.2) as FSL's `int` holds it, or why FSL cannot: `Err(true)` when
/// FSL crashes (the width beyond `int`'s range), `Err(false)` when its array's length `2m + 1`
/// is negative (`std::bad_array_new_length`).
fn half_width(m: f64, plus: i32) -> Result<i32, bool> {
    let w = num::cvtt(m);
    if w == i32::MIN {
        return Err(true);
    }
    let w = w.wrapping_add(plus);
    if w < 0 {
        return Err(true);
    }
    if w.wrapping_mul(2).wrapping_add(1) < 0 {
        return Err(false);
    }
    Ok(w)
}

/// Whether a sigma takes part (`σ ≤ 0` or NaN skips its filter).
fn active(sigma: f64) -> bool {
    sigma > 0.0
}

/// The weight `exp(−Δ² / (2σ²))` as FSL evaluates it, `exp(−0.5 Δ² / (σ σ))` (Δ² a 32-bit
/// integer; black-box runs in double: findings 15).
#[inline]
fn weight(d2: i32, sigma: f64) -> f64 {
    math::exp(-0.5 * f64::from(d2) / (sigma * sigma))
}

/// The weights for `|Δ|` up to `n`.
fn weights(sigma: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|d| weight((d as i32).wrapping_mul(d as i32), sigma))
        .collect()
}

/// The high-pass of one series (§13.2): the local weighted line's value at each `t`, `c₀ + x_t −
/// c`, then the mean subtracted.
fn high_pass(x: &mut [f64], w: &[f64], m: usize) {
    let n = x.len();
    let mut out = vec![0.0f64; n];
    let mut c0 = None;
    for t in 0..n {
        let (mut a, mut b, mut c, mut d, mut nn) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let lo = t.saturating_sub(m);
        let hi = (t + m).min(n - 1);
        for s in lo..=hi {
            let delta = s as i64 - t as i64;
            let wd = w[delta.unsigned_abs() as usize];
            let dd = delta as f64;
            let v = x[s];
            // the products left to right, (w Δ) Δ and (w Δ) x (black-box runs in double:
            // findings 15)
            let wdd = wd * dd;
            a += wdd;
            b = num::nan64(b, v, b + wd * v);
            c += wdd * dd;
            d = num::nan64(d, v, d + wdd * v);
            nn += wd;
        }
        let den = c * nn - a * a;
        out[t] = if den == 0.0 {
            x[t]
        } else {
            let bc = num::nan64(b, c, b * c);
            let ad = num::nan64(a, d, a * d);
            let numer = num::nan64(bc, ad, bc - ad);
            let ct = num::nan64(numer, den, numer / den);
            let c0 = *c0.get_or_insert(ct);
            let s = num::nan64(c0, x[t], c0 + x[t]);
            num::nan64(s, ct, s - ct)
        };
    }
    let mut sum = 0.0f64;
    for &v in &out {
        sum = num::nan64(sum, v, sum + v);
    }
    let mean = sum / n as f64;
    for (o, v) in x.iter_mut().zip(out) {
        *o = num::nan64(v, mean, v - mean);
    }
}

/// The low-pass of one series (§13.2): the normalised Gaussian's weighted mean of the values
/// inside the series at each `t`.
fn low_pass(x: &mut [f64], w: &[f64], m: usize) {
    let n = x.len();
    let src = x.to_vec();
    for t in 0..n {
        let (mut sw, mut swx) = (0.0f64, 0.0f64);
        let lo = t.saturating_sub(m);
        let hi = (t + m).min(n - 1);
        for (s, &v) in src.iter().enumerate().take(hi + 1).skip(lo) {
            let wd = w[(s as i64 - t as i64).unsigned_abs() as usize];
            sw += wd;
            swx = num::nan64(swx, v, swx + wd * v);
        }
        x[t] = num::nan64(swx, sw, swx / sw);
    }
}

/// `-bptf hp lp` (§13.2): the high-pass, then the low-pass, of every voxel's series.
pub fn bptf<T: Num>(img: &mut Img<T>, hp: f64, lp: f64, notices: &mut Notices) -> Result<(), Stop> {
    let fix = notices.fix();
    let nt = img.dims[3].max(1);
    let per = img.per_volume();
    let mut width = |sigma: f64,
                     factor: f64,
                     plus: i32,
                     which: &str|
     -> Result<Option<usize>, Stop> {
        if !active(sigma) {
            return Ok(None);
        }
        let crash = match half_width(factor * sigma, plus) {
            Ok(m) => return Ok(Some(m as usize)),
            Err(crash) => crash,
        };
        if fix {
            notices.add(
                &defects::FSL_171,
                format!(
                    "-bptf with a {which} sigma of {sigma}, whose filter half-width or length is \
                     beyond int's range, is refused (FSL {})",
                    if crash {
                        "crashes"
                    } else {
                        "stops: std::bad_array_new_length"
                    }
                ),
            );
        }
        Err(if crash {
            Stop::Refuse(format!(
                "-bptf: the {which} sigma {sigma} gives a filter half-width beyond int's range \
                 (FSL-171: FSL crashes here)"
            ))
        } else if fix {
            Stop::Refuse(format!(
                "-bptf: the {which} sigma {sigma} gives a filter longer than int's range (FSL-171)"
            ))
        } else {
            Stop::Exception(super::filters::abort_lines(
                "std::bad_array_new_length",
                "std::bad_array_new_length",
            ))
        })
    };
    let mh = width(hp, 3.0, 0, "high-pass")?;
    let ml = width(lp, 20.0, 2, "low-pass")?;
    let span = nt - 1;
    let wh = mh.map(|m| weights(hp, m.min(span)));
    let wl = ml.map(|m| {
        // normalised by their total over −m..m, in that order
        let mut total = 0.0f64;
        for d in -(m as i64)..=(m as i64) {
            total += weight((d as i32).wrapping_mul(d as i32), lp);
        }
        weights(lp, m.min(span))
            .into_iter()
            .map(|w| w / total)
            .collect::<Vec<f64>>()
    });
    let data = &img.data;
    let cols: Vec<(Vec<T>, bool)> = (0..per)
        .into_par_iter()
        .map(|k| {
            let mut x: Vec<f64> = (0..nt).map(|t| data[t * per + k].to_f64()).collect();
            if let (Some(w), Some(m)) = (&wh, mh) {
                high_pass(&mut x, w, m.min(span));
            }
            if let (Some(w), Some(m)) = (&wl, ml) {
                low_pass(&mut x, w, m.min(span));
            }
            let mut ch = false;
            let out = x
                .into_iter()
                .map(|v| {
                    let (t, c) = T::convert(v, fix);
                    ch |= c;
                    t
                })
                .collect();
            (out, ch)
        })
        .collect();
    let mut changed = false;
    let mut new = vec![T::zero(); data.len()];
    for (k, (col, ch)) in cols.into_iter().enumerate() {
        for (t, v) in col.into_iter().enumerate() {
            new[t * per + k] = v;
        }
        changed |= ch;
    }
    img.data = new;
    super::ops::note151(notices, changed, "-bptf's values");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_widths_and_fsl_171() {
        assert_eq!(half_width(6.0, 0), Ok(6));
        assert_eq!(half_width(f64::INFINITY, 0), Err(true));
        assert_eq!(half_width(1.2e9, 0), Err(false));
        assert_eq!(half_width(4e9, 2), Err(true));
    }

    #[test]
    fn a_high_passed_line_is_zero() {
        // a straight line is its own local line: c0 + x - c is c0, minus the mean: 0
        let mut x: Vec<f64> = (0..9).map(|t| 2.0 + 3.0 * f64::from(t)).collect();
        let w = weights(2.0, 6);
        high_pass(&mut x, &w, 6);
        assert!(x.iter().all(|v| v.abs() < 1e-12), "{x:?}");
    }
}

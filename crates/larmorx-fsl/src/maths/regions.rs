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

/// The weight `exp(−Δ² / (2σ²))`, evaluated as `exp(−0.5 Δ² / (σ σ))` (Δ² a 32-bit integer;
/// the two give the same values on the black-box probes of findings 15).
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

/// The voxels' series filtered a block at a time (lane = voxel; `x[t * lanes + lane]`): the
/// sums that do not depend on the voxel (the windows' `Σ w Δ`, `Σ w Δ²`, `Σ w`) computed once
/// per `t` in the series' order, the others lane by lane in the same order as
/// [`high_pass`] and [`low_pass`]; the same bits but for NaN payloads (a voxel whose series
/// holds a NaN is filtered again by those, by x86-64's rules).
struct Filters {
    n: usize,
    high: Option<HighPass>,
    low: Option<LowPass>,
}

struct HighPass {
    w: Vec<f64>,
    m: usize,
    /// Per `t`: `Σ w Δ`, `Σ w Δ²`, the denominator `c n − a²`.
    a: Vec<f64>,
    c: Vec<f64>,
    den: Vec<f64>,
    /// The first `t` whose denominator is not 0 (`c₀`'s).
    t0: Option<usize>,
}

struct LowPass {
    w: Vec<f64>,
    m: usize,
    /// Per `t`: `Σ w`.
    sw: Vec<f64>,
}

impl Filters {
    fn new(n: usize, high: Option<(Vec<f64>, usize)>, low: Option<(Vec<f64>, usize)>) -> Filters {
        let window = |t: usize, m: usize| (t.saturating_sub(m), (t + m).min(n - 1));
        let high = high.map(|(w, m)| {
            let (mut a, mut c, mut den) = (Vec::new(), Vec::new(), Vec::new());
            for t in 0..n {
                let (lo, hi) = window(t, m);
                let (mut sa, mut sc, mut nn) = (0.0f64, 0.0f64, 0.0f64);
                for s in lo..=hi {
                    let delta = s as i64 - t as i64;
                    let wd = w[delta.unsigned_abs() as usize];
                    let dd = delta as f64;
                    let wdd = wd * dd;
                    sa += wdd;
                    sc += wdd * dd;
                    nn += wd;
                }
                a.push(sa);
                c.push(sc);
                den.push(sc * nn - sa * sa);
            }
            let t0 = den.iter().position(|&d| d != 0.0);
            HighPass {
                w,
                m,
                a,
                c,
                den,
                t0,
            }
        });
        let low = low.map(|(w, m)| {
            let sw = (0..n)
                .map(|t| {
                    let (lo, hi) = window(t, m);
                    (lo..=hi).fold(0.0f64, |acc, s| {
                        acc + w[(s as i64 - t as i64).unsigned_abs() as usize]
                    })
                })
                .collect();
            LowPass { w, m, sw }
        });
        Filters { n, high, low }
    }

    /// Filters `lanes` series in `x` (the result in `x`; `tmp` and `sums` of the same size).
    fn block(&self, x: &mut [f64], tmp: &mut [f64], sums: &mut [f64], lanes: usize) {
        let n = self.n;
        let window = |t: usize, m: usize| (t.saturating_sub(m), (t + m).min(n - 1));
        if let Some(h) = &self.high {
            let (b, rest) = sums.split_at_mut(lanes);
            let (d, c0) = rest.split_at_mut(lanes);
            let (b, d, c0) = (&mut b[..lanes], &mut d[..lanes], &mut c0[..lanes]);
            for t in 0..n {
                let (lo, hi) = window(t, h.m);
                b.fill(0.0);
                d.fill(0.0);
                for s in lo..=hi {
                    let delta = s as i64 - t as i64;
                    let wd = h.w[delta.unsigned_abs() as usize];
                    let wdd = wd * delta as f64;
                    let xs = &x[s * lanes..(s + 1) * lanes];
                    for ((b, d), &v) in b.iter_mut().zip(d.iter_mut()).zip(xs) {
                        *b += wd * v;
                        *d += wdd * v;
                    }
                }
                let out = &mut tmp[t * lanes..(t + 1) * lanes];
                let xt = &x[t * lanes..(t + 1) * lanes];
                if h.den[t] == 0.0 {
                    out.copy_from_slice(xt);
                    continue;
                }
                let (a, c, den) = (h.a[t], h.c[t], h.den[t]);
                for k in 0..lanes {
                    let ct = (b[k] * c - a * d[k]) / den;
                    if Some(t) == h.t0 {
                        c0[k] = ct;
                    }
                    out[k] = (c0[k] + xt[k]) - ct;
                }
            }
            let mean = &mut sums[..lanes];
            mean.fill(0.0);
            for t in 0..n {
                for (m, &v) in mean.iter_mut().zip(&tmp[t * lanes..(t + 1) * lanes]) {
                    *m += v;
                }
            }
            for m in mean.iter_mut() {
                *m /= n as f64;
            }
            for t in 0..n {
                let xt = &mut x[t * lanes..(t + 1) * lanes];
                for ((o, &v), &m) in xt
                    .iter_mut()
                    .zip(&tmp[t * lanes..(t + 1) * lanes])
                    .zip(&*mean)
                {
                    *o = v - m;
                }
            }
        }
        if let Some(l) = &self.low {
            tmp.copy_from_slice(x);
            for t in 0..n {
                let (lo, hi) = window(t, l.m);
                let swx = &mut sums[..lanes];
                swx.fill(0.0);
                for s in lo..=hi {
                    let wd = l.w[(s as i64 - t as i64).unsigned_abs() as usize];
                    for (a, &v) in swx.iter_mut().zip(&tmp[s * lanes..(s + 1) * lanes]) {
                        *a += wd * v;
                    }
                }
                let sw = l.sw[t];
                for (o, &a) in x[t * lanes..(t + 1) * lanes].iter_mut().zip(&*swx) {
                    *o = a / sw;
                }
            }
        }
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
    let filters = Filters::new(
        nt,
        wh.clone().zip(mh.map(|m| m.min(span))),
        wl.clone().zip(ml.map(|m| m.min(span))),
    );
    // the exact filter of one voxel (a series holding a NaN)
    let exact = |col: &mut [f64]| {
        if let (Some(w), Some(m)) = (&wh, mh) {
            high_pass(col, w, m.min(span));
        }
        if let (Some(w), Some(m)) = (&wl, ml) {
            low_pass(col, w, m.min(span));
        }
    };
    // waves of voxels (a buffer of a wave's series, not of the image), blocks of LANES voxels
    // filtered in parallel, written back in place
    const LANES: usize = 64;
    const WAVE: usize = LANES * 512;
    let mut changed = false;
    let mut wave = vec![T::zero(); WAVE.min(per) * nt];
    for k0 in (0..per).step_by(WAVE) {
        let k1 = (k0 + WAVE).min(per);
        let width = k1 - k0;
        let data = &img.data;
        let ch = wave[..width * nt]
            .par_chunks_mut(LANES * nt)
            .enumerate()
            .map_init(
                || {
                    (
                        vec![0.0f64; LANES * nt],
                        vec![0.0f64; LANES * nt],
                        vec![0.0f64; 3 * LANES],
                    )
                },
                |(x, tmp, sums), (b, out)| {
                    let v0 = k0 + b * LANES;
                    let lanes = out.len() / nt;
                    let x = &mut x[..lanes * nt];
                    for t in 0..nt {
                        let src = &data[t * per + v0..t * per + v0 + lanes];
                        for (o, &v) in x[t * lanes..(t + 1) * lanes].iter_mut().zip(src) {
                            *o = v.fast_f64();
                        }
                    }
                    filters.block(x, &mut tmp[..lanes * nt], sums, lanes);
                    let mut ch = false;
                    let mut col = vec![0.0f64; nt];
                    for k in 0..lanes {
                        let nan = (0..nt).any(|t| x[t * lanes + k].is_nan());
                        if nan {
                            for (t, c) in col.iter_mut().enumerate() {
                                *c = data[t * per + v0 + k].to_f64();
                            }
                            exact(&mut col);
                        }
                        for t in 0..nt {
                            let v = if nan { col[t] } else { x[t * lanes + k] };
                            let (tv, c) = T::convert(v, fix);
                            ch |= c;
                            // the block's output: voxel k's series, t fastest
                            out[k * nt + t] = tv;
                        }
                    }
                    ch
                },
            )
            .reduce(|| false, |a, b| a || b);
        changed |= ch;
        // the wave's series written back, volume by volume
        let wave = &wave;
        img.data
            .par_chunks_mut(per)
            .enumerate()
            .for_each(|(t, vol)| {
                for (k, v) in vol[k0..k1].iter_mut().enumerate() {
                    *v = wave[(k / LANES) * LANES * nt + (k % LANES) * nt + t];
                }
            });
    }
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

    /// `-bptf` built for speed (blocks of voxels, waves) gives the bits of each voxel's series
    /// filtered on its own ([`high_pass`], [`low_pass`]), NaN payloads included, at any thread
    /// count.
    #[test]
    fn the_blocks_are_the_series_filtered_alone() {
        let (per, nt) = (150usize, 23usize);
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let data: Vec<f32> = (0..per * nt)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                match x % 211 {
                    0 => f32::from_bits(0x7fc0_0042),
                    1 => f32::INFINITY,
                    r => (r % 50) as f32 * 0.7 - 11.0,
                }
            })
            .collect();
        let im = crate::maths::filters::tests::img([per, 1, 1, nt], data.clone());
        for (hp, lp) in [(4.0, 1.5), (4.0, -1.0), (-1.0, 2.0), (30.0, 0.5)] {
            let mut want = vec![0.0f32; per * nt];
            let mh = active(hp).then(|| half_width(3.0 * hp, 0).unwrap() as usize);
            let ml = active(lp).then(|| half_width(20.0 * lp, 2).unwrap() as usize);
            let span = nt - 1;
            let wh = mh.map(|m| weights(hp, m.min(span)));
            let wl = ml.map(|m| {
                let mut total = 0.0f64;
                for d in -(m as i64)..=(m as i64) {
                    total += weight((d as i32).wrapping_mul(d as i32), lp);
                }
                weights(lp, m.min(span))
                    .into_iter()
                    .map(|w| w / total)
                    .collect::<Vec<_>>()
            });
            for k in 0..per {
                let mut col: Vec<f64> = (0..nt).map(|t| num::to_f64(data[t * per + k])).collect();
                if let (Some(w), Some(m)) = (&wh, mh) {
                    high_pass(&mut col, w, m.min(span));
                }
                if let (Some(w), Some(m)) = (&wl, ml) {
                    low_pass(&mut col, w, m.min(span));
                }
                for t in 0..nt {
                    want[t * per + k] = num::to_f32(col[t]);
                }
            }
            for threads in [1, 4] {
                let mut got = im.clone();
                larmorx_core::parallel::with_threads(threads, || {
                    bptf(
                        &mut got,
                        hp,
                        lp,
                        &mut Notices::new(defects::UpstreamDefects::Fix),
                    )
                })
                .unwrap()
                .unwrap();
                assert!(
                    got.data
                        .iter()
                        .zip(&want)
                        .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "-bptf {hp} {lp}, {threads} threads"
                );
            }
        }
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

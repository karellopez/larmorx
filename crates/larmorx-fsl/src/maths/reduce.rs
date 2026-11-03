// SPDX-License-Identifier: Apache-2.0
//! fslmaths' reductions along a dimension (`specs/fslmaths.md` §11): `-T`, `-X`, `-Y`, `-Z`
//! (another upper-case letter reduces nothing) with `max`, `min`, `mean`, `std`, `maxn`,
//! `median`, `perc p` and `ar1`.
//!
//! Each output voxel reduces its column of values along the axis, in index order; the columns
//! are independent, so they are computed in parallel and the result does not depend on the
//! thread count. The defects: FSL-153 (a NaN first value), FSL-167 (the unmasked variance),
//! FSL-168 (percentiles of values holding NaN), each named exactly where the default's result
//! differs from FSL's.

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use rayon::prelude::*;

use super::image::Img;
use super::num::{self, Kind, Num};
use super::ops::Stop;

/// The statistic of a reduction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stat {
    Max,
    Min,
    Mean,
    Std,
    Maxn,
    /// A percentile: the fraction in single precision (`median`: 0.5).
    Perc(f32),
    Ar1,
}

impl Stat {
    /// The statistic a reduction's word names after its axis letter (`perc` without its
    /// fraction: 0), or `None`.
    pub fn of(rest: &str) -> Option<Stat> {
        Some(match rest {
            "max" => Stat::Max,
            "min" => Stat::Min,
            "mean" => Stat::Mean,
            "std" => Stat::Std,
            "maxn" => Stat::Maxn,
            "median" => Stat::Perc(0.5),
            "perc" => Stat::Perc(0.0),
            "ar1" => Stat::Ar1,
            _ => return None,
        })
    }
}

/// The axis a reduction's letter names: `T`, `X`, `Y`, `Z` (another letter: none, every column
/// one value).
pub fn axis_of(letter: u8) -> Option<usize> {
    match letter {
        b'X' => Some(0),
        b'Y' => Some(1),
        b'Z' => Some(2),
        b'T' => Some(3),
        _ => None,
    }
}

/// The message of a percentile outside [0, 1] (the image library's exception: an abort).
pub const PERCENTILE_RANGE: &str = "Percentiles must be in the range [0.0,1.0]";

/// The double sum of §18 in blocks (`b = max(⌊√n⌋, 10000)`, every `b + 1` values the running
/// sum added to the total), with x86-64's NaN rules.
pub fn block_sum(vals: impl Iterator<Item = f64>, n: usize) -> f64 {
    let b = ((n as f64).sqrt().floor() as usize).max(10000);
    let (mut total, mut run) = (0.0f64, 0.0f64);
    let mut in_run = 0usize;
    for x in vals {
        run = num::nan64(run, x, run + x);
        in_run += 1;
        if in_run == b + 1 {
            total = num::nan64(total, run, total + run);
            run = 0.0;
            in_run = 0;
        }
    }
    num::nan64(total, run, total + run)
}

/// `√x` with x86-64's NaN rules (a negative value: the default NaN).
#[inline]
pub fn sqrt64(x: f64) -> f64 {
    if x.is_nan() {
        num::quiet(x)
    } else if x < 0.0 {
        num::default_nan()
    } else {
        x.sqrt()
    }
}

/// The mean of a column (§18's blocks), as a double.
pub fn mean<T: Num>(c: &[T]) -> f64 {
    let n = c.len();
    let s = block_sum(c.iter().map(|v| v.to_f64()), n);
    num::nan64(s, n as f64, s / n as f64)
}

/// The standard deviation of a column (§11.1): `√((n / (n − 1)) (S₂ / n − m²))`; FSL's form
/// (`reproduce`: no clamp at 0, NaN for one value: FSL-167) and the masked form (the guard for
/// one value, never below 0; NaN values still give NaN).
pub fn std<T: Num>(c: &[T]) -> (f64, f64) {
    let n = c.len();
    let nf = n as f64;
    let s = block_sum(c.iter().map(|v| v.to_f64()), n);
    let s2 = block_sum(
        c.iter().map(|v| {
            let x = v.to_f64();
            num::nan64(x, x, x * x)
        }),
        n,
    );
    let m = num::nan64(s, nf, s / nf);
    let q = num::nan64(s2, nf, s2 / nf);
    let mm = num::nan64(m, m, m * m);
    let d = num::nan64(q, mm, q - mm);
    let f = nf / (nf - 1.0);
    let var = num::nan64(f, d, f * d);
    let fsl = sqrt64(var);
    let g = nf / (nf - 1.0).max(1.0);
    let masked = num::nan64(g, d, g * d);
    let masked = if masked.is_nan() {
        masked
    } else {
        masked.max(0.0)
    };
    (fsl, sqrt64(masked))
}

/// The extremes and the first maximum's index from the first value (§11.1; FSL-153 with
/// `skip_nan` false): `(min, max, index)`; every value NaN with `skip_nan`: the first, index 0.
fn extremes_at<T: Num>(c: &[T], skip_nan: bool) -> (T, T, usize) {
    let mut it = c.iter().copied().enumerate();
    let first = if skip_nan {
        it.by_ref().find(|(_, v)| !v.is_nan())
    } else {
        it.next()
    };
    let Some((k0, v0)) = first else {
        return (c[0], c[0], 0);
    };
    let (mut lo, mut hi, mut at) = (v0, v0, k0);
    for (k, v) in it {
        if v > hi {
            hi = v;
            at = k;
        } else if v < lo {
            lo = v;
        }
    }
    (lo, hi, at)
}

/// The value at a percentile of a column: sorted ascending (libstdc++'s order), the value at
/// `⌊n p⌋` (the product in single precision), at most `n − 1`; a NaN fraction: the first.
/// `None` where libstdc++'s sort would leave the values (FSL-168 with `reproduce`).
fn percentile<T: Num>(c: &mut [T], p: f32) -> Option<T> {
    let n = c.len();
    if !crate::flirt::schedule::std_sort_checked(c, &|a: &T, b: &T| a < b) {
        return None;
    }
    let pos = (n as f32) * p;
    let k = if pos.is_nan() {
        0
    } else {
        (pos.floor() as usize).min(n - 1)
    };
    Some(c[k])
}

/// The column's percentile without its NaN values (FSL-168's correction): every value NaN, the
/// first NaN, quieted.
fn percentile_skip_nan<T: Num>(c: &[T], p: f32) -> T {
    let mut v: Vec<T> = c.iter().copied().filter(|v| !v.is_nan()).collect();
    if v.is_empty() {
        return T::from_f64_c(num::quiet(c[0].to_f64()));
    }
    percentile(&mut v, p).unwrap_or(v[0])
}

/// `ar1` (§11.1): the column minus its mean (in `T`), `S₂` the double sum of the squares, then
/// `Σ c_k c_{k−1} / S₂` term by term (the product in `T`'s arithmetic, 32-bit for the integer
/// types; divided in double; converted and added in `T`). Returns the value and whether
/// FSL-151 changed a conversion.
fn ar1<T: Num>(c: &[T], fix: bool) -> (T, bool) {
    let (m, ch0) = T::convert(mean(c), fix);
    let d: Vec<T> = c.iter().map(|&v| v.sub(m)).collect();
    let mut s2 = 0.0f64;
    for v in &d {
        let x = v.to_f64();
        let sq = num::nan64(x, x, x * x);
        s2 = num::nan64(s2, sq, s2 + sq);
    }
    if s2 == 0.0 {
        return (T::zero(), ch0);
    }
    let mut acc = T::zero();
    let mut changed = ch0;
    for k in 1..d.len() {
        let p = product(d[k], d[k - 1]);
        let (t, ch) = T::convert(num::nan64(p, s2, p / s2), fix);
        changed |= ch;
        acc = acc.add(t);
    }
    (acc, changed)
}

/// `a · b` in the computing type's arithmetic, as a double: single precision for `float`, a
/// 32-bit integer product for the integer types, double for `double`.
#[inline]
fn product<T: Num>(a: T, b: T) -> f64 {
    match T::KIND {
        Kind::Float => {
            let (x, y) = (num::to_f32(a.to_f64()), num::to_f32(b.to_f64()));
            num::to_f64(num::nan32(x, y, x * y))
        }
        Kind::Double => {
            let (x, y) = (a.to_f64(), b.to_f64());
            num::nan64(x, y, x * y)
        }
        _ => f64::from((a.to_f64() as i32).wrapping_mul(b.to_f64() as i32)),
    }
}

/// What a column's reduction found: its value, and whether the default's value differs from
/// FSL's for each defect of the reductions.
#[derive(Clone, Copy, Default)]
struct Found {
    fsl151: bool,
    fsl153: bool,
    fsl167: bool,
    fsl168: bool,
    /// FSL-168 with `reproduce`: libstdc++'s sort would leave the values.
    outside: bool,
}

impl Found {
    fn merge(self, o: Found) -> Found {
        Found {
            fsl151: self.fsl151 || o.fsl151,
            fsl153: self.fsl153 || o.fsl153,
            fsl167: self.fsl167 || o.fsl167,
            fsl168: self.fsl168 || o.fsl168,
            outside: self.outside || o.outside,
        }
    }
}

/// One column's statistic in the mode, with what it found.
fn column<T: Num>(c: &mut [T], stat: Stat, fix: bool) -> (T, Found) {
    let mut f = Found::default();
    let v = match stat {
        Stat::Max | Stat::Min | Stat::Maxn => {
            let rep = extremes_at(c, false);
            let got = if c[0].is_nan() {
                let ok = extremes_at(c, true);
                f.fsl153 = match stat {
                    Stat::Max => !ok.1.same(rep.1),
                    Stat::Min => !ok.0.same(rep.0),
                    _ => ok.2 != rep.2,
                };
                if fix { ok } else { rep }
            } else {
                rep
            };
            match stat {
                Stat::Max => got.1,
                Stat::Min => got.0,
                _ => {
                    let (t, ch) = T::convert(got.2 as f64, fix);
                    f.fsl151 = ch;
                    t
                }
            }
        }
        Stat::Mean => {
            let (t, ch) = T::convert(mean(c), fix);
            f.fsl151 = ch;
            t
        }
        Stat::Std => {
            let (fsl, masked) = std(c);
            let (a, ch1) = T::convert(fsl, fix);
            let (b, ch2) = T::convert(masked, fix);
            f.fsl167 = !a.same(b);
            if fix {
                f.fsl151 = ch2;
                b
            } else {
                f.fsl151 = ch1;
                a
            }
        }
        Stat::Perc(p) => {
            let nan = c.iter().any(|v| v.is_nan());
            let skip = if nan {
                Some(percentile_skip_nan(c, p))
            } else {
                None
            };
            let rep = percentile(c, p);
            match (rep, skip) {
                (Some(r), Some(s)) => {
                    f.fsl168 = !r.same(s);
                    if fix { s } else { r }
                }
                (Some(r), None) => r,
                (None, s) => {
                    f.fsl168 = true;
                    f.outside = true;
                    s.unwrap_or(c[0])
                }
            }
        }
        Stat::Ar1 => {
            let (t, ch) = ar1(c, fix);
            f.fsl151 = ch;
            t
        }
    };
    (v, f)
}

/// The reduction of the current image along `axis` (`None`: columns of one value).
pub fn reduce<T: Num>(
    img: &mut Img<T>,
    axis: Option<usize>,
    stat: Stat,
    op: &str,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let fix = notices.fix();
    let dims = img.dims;
    let strides = [1, dims[0], dims[0] * dims[1], dims[0] * dims[1] * dims[2]];
    let (len, stride) = match axis {
        Some(a) => (dims[a], strides[a]),
        None => (1, 1),
    };
    let mut out_dims = dims;
    if let Some(a) = axis {
        out_dims[a] = 1;
    }
    let n_out: usize = out_dims.iter().product();
    if len == 0 || n_out == 0 {
        return Ok(());
    }
    let src = &img.data;
    // the output voxel `o`'s column starts at its coordinates with the axis' coordinate 0
    let start = |o: usize| -> usize {
        let mut rest = o;
        let mut at = 0;
        for k in 0..4 {
            let c = rest % out_dims[k];
            rest /= out_dims[k];
            at += c * strides[k];
        }
        at
    };
    const CHUNK: usize = 1024;
    let parts: Vec<(Vec<T>, Found)> = (0..n_out.div_ceil(CHUNK))
        .into_par_iter()
        .map(|b| {
            let lo = b * CHUNK;
            let hi = (lo + CHUNK).min(n_out);
            let mut col = vec![T::zero(); len];
            let mut out = Vec::with_capacity(hi - lo);
            let mut found = Found::default();
            for o in lo..hi {
                let s = start(o);
                for (j, v) in col.iter_mut().enumerate() {
                    *v = src[s + j * stride];
                }
                let (v, f) = column(&mut col, stat, fix);
                out.push(v);
                found = found.merge(f);
            }
            (out, found)
        })
        .collect();
    let mut data = Vec::with_capacity(n_out);
    let mut found = Found::default();
    for (o, f) in parts {
        data.extend(o);
        found = found.merge(f);
    }
    if found.outside && !fix {
        return Err(Stop::Refuse(format!(
            "{op}: values holding NaN, which libstdc++'s sort would read past (FSL-168: FSL reads \
             and writes outside the values here)"
        )));
    }
    let mode = notices.mode();
    if found.fsl153 {
        notices.add(
            &defects::FSL_153,
            match mode {
                UpstreamDefects::Fix => format!(
                    "{op}: the columns' NaN values left out of the extremes (FSL's is NaN, or its \
                     index 0, where the first value is NaN)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "{op}: a column's NaN first value makes its extreme NaN (index 0), as FSL does"
                ),
            },
        );
    }
    if found.fsl167 {
        notices.add(
            &defects::FSL_167,
            match mode {
                UpstreamDefects::Fix => format!(
                    "{op}: the variance clamped at 0 and 0 for one value (FSL's is NaN there)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "{op}: the variance without the clamp at 0 (NaN for one value), as FSL takes it"
                ),
            },
        );
    }
    if found.fsl168 {
        notices.add(
            &defects::FSL_168,
            match mode {
                UpstreamDefects::Fix => {
                    format!("{op}: the NaN values left out of the percentile (FSL's is arbitrary)")
                }
                UpstreamDefects::Reproduce => format!(
                    "{op}: the values holding NaN sorted as libstdc++'s sort leaves them, as FSL \
                     does"
                ),
            },
        );
    }
    super::ops::note151(notices, found.fsl151, "a reduction's results");
    img.data = data;
    img.dims = out_dims;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_and_their_statistics() {
        let c = [3.0f32, 1.0, 4.0, 1.0, 5.0];
        let mut w = c;
        assert_eq!(column(&mut w, Stat::Max, true).0, 5.0);
        let mut w = c;
        assert_eq!(column(&mut w, Stat::Maxn, true).0, 4.0);
        let mut w = c;
        // ⌊5 · 0.5⌋ = 2: the sorted [1, 1, 3, 4, 5]'s third
        assert_eq!(column(&mut w, Stat::Perc(0.5), true).0, 3.0);
        let mut w = [1.0f32, 1.0];
        assert_eq!(column(&mut w, Stat::Perc(1.0), true).0, 1.0);
        let (fsl, masked) = std(&[2.0f32]);
        assert!(fsl.is_nan() && masked == 0.0);
        // a NaN first value: FSL's NaN maximum and index 0, the default's of the rest
        let mut w = [f32::NAN, 2.0, 7.0];
        let (v, f) = column(&mut w, Stat::Max, false);
        assert!(v.is_nan() && f.fsl153);
        let mut w = [f32::NAN, 2.0, 7.0];
        assert_eq!(column(&mut w, Stat::Maxn, true).0, 2.0);
    }

    #[test]
    fn ar1_of_a_ramp_and_under_int() {
        // [1, 2, 3, 4]: demeaned [-1.5, -0.5, 0.5, 1.5], S2 = 5, terms 0.75, -0.25, 0.75
        let (v, _) = ar1(&[1.0f32, 2.0, 3.0, 4.0], true);
        assert_eq!(v, 0.25);
        let (v, _) = ar1(&[1i32, 2, 3, 4], true);
        assert_eq!(v, 0);
    }

    #[test]
    fn block_sums_restart_every_b_plus_one_values() {
        let n = 25_000;
        let v: Vec<f64> = (0..n).map(|k| (k % 7) as f64 * 0.1).collect();
        let mut total = 0.0;
        let mut run = 0.0;
        for (k, &x) in v.iter().enumerate() {
            run += x;
            if (k + 1) % 10_001 == 0 {
                total += run;
                run = 0.0;
            }
        }
        assert_eq!(
            block_sum(v.iter().copied(), n).to_bits(),
            (total + run).to_bits()
        );
    }
}

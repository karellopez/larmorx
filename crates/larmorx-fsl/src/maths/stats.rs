// SPDX-License-Identifier: Apache-2.0
//! fslmaths' statistical transforms (`specs/fslmaths.md` §12) and `-randn` (§8.2): the normal
//! distribution's function `Φ` and its inverse, `-ztop`, `-ptoz`, `-pval`, `-pval0`, `-cpval`,
//! `-rank`, `-ranknorm`.
//!
//! `Φ` follows the spec's form (through `erf` for `|z| < 1`, through `erfc` otherwise and `1 −`
//! it for positive `z`), with the `erf` and `erfc` of the `libm`
//! crate (FreeBSD's msun, errors below one unit in the last place; larmorx's correctly rounded
//! maths has no `erf` yet). `Φ⁻¹` is computed from Abramowitz and Stegun's 26.2.23 start (error
//! below 4.5e-4) by Halley's iterations on `Φ` itself, to the double's precision; FSL's values
//! (Cephes' rational approximations) and these differ in the last bits only (§21's sums'
//! metric). Every series is independent: the voxels run in parallel.

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::math;
use rayon::prelude::*;

use super::image::Img;
use super::num::{self, Num};
use super::ops::Stop;
use super::reduce;

/// `1 / √2`.
const SQRTH: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// `1 / √(2π)`.
const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;
/// The largest double: `Φ⁻¹` of 0 and 1 (with the domain error).
pub const MAXNUM: f64 = f64::MAX;
/// The line `Φ⁻¹` prints on standard output for an argument at or beyond 0 or 1.
pub const DOMAIN_ERROR: &str = "\nndtri domain error\n";

/// `erf` (the `libm` crate's: larmorx's correctly rounded maths has none yet).
#[inline]
fn erf(x: f64) -> f64 {
    libm::erf(x)
}

/// `erfc` (the `libm` crate's, as [`erf`]).
#[inline]
fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// The normal density `φ(z)`.
#[inline]
fn density(z: f64) -> f64 {
    math::exp(-0.5 * z * z) * INV_SQRT_2PI
}

/// `Φ(a)`, the standard normal distribution function, in the spec's form (§12).
pub fn ndtr(a: f64) -> f64 {
    if a.is_nan() {
        return num::quiet(a);
    }
    let x = a * SQRTH;
    let z = x.abs();
    if z < SQRTH {
        0.5 + 0.5 * erf(x)
    } else {
        let y = 0.5 * erfc(z);
        if x > 0.0 { 1.0 - y } else { y }
    }
}

/// Abramowitz and Stegun's 26.2.23: `w` with `Q(w) ≈ r` for `0 < r ≤ 0.5` (error < 4.5e-4).
fn start(r: f64) -> f64 {
    let t = (-2.0 * math::log(r)).sqrt();
    let n = 2.515_517 + t * (0.802_853 + t * 0.010_328);
    let d = 1.0 + t * (1.432_788 + t * (0.189_269 + t * 0.001_308));
    t - n / d
}

/// `Φ⁻¹(p)` for `0 < p < 1`.
fn ndtri_inside(p: f64) -> f64 {
    let q = p - 0.5;
    if q == 0.0 {
        return 0.0;
    }
    if q.abs() <= 0.425 {
        // solve ½ erf(z / √2) = q (exact: q = p − ½ for p in [¼, 1])
        let w = start(if q < 0.0 { p } else { 1.0 - p });
        let mut z = if q < 0.0 { -w } else { w };
        for _ in 0..8 {
            let t = (0.5 * erf(z * SQRTH) - q) / density(z);
            let step = t / (1.0 + 0.5 * z * t);
            z -= step;
            if step.abs() <= 1e-17 * z.abs() {
                break;
            }
        }
        z
    } else {
        // the tail: solve Q(w) = ½ erfc(w / √2) = r for w > 0 (1 − p exact for p ≥ ½)
        let (r, sign) = if q < 0.0 { (p, -1.0) } else { (1.0 - p, 1.0) };
        let mut w = start(r);
        for _ in 0..8 {
            let t = (0.5 * erfc(w * SQRTH) - r) / density(w);
            let step = t / (1.0 - 0.5 * w * t);
            w += step;
            if step.abs() <= 1e-17 * w.abs() {
                break;
            }
        }
        sign * w
    }
}

/// `Φ⁻¹(p)` as fslmaths takes it: NaN passes on; `p ≤ 0` gives `−MAXNUM` and `p ≥ 1` `MAXNUM`,
/// each with the domain error (the `bool`).
pub fn ndtri(p: f64) -> (f64, bool) {
    if p.is_nan() {
        (num::quiet(p), false)
    } else if p <= 0.0 {
        (-MAXNUM, true)
    } else if p >= 1.0 {
        (MAXNUM, true)
    } else {
        (ndtri_inside(p), false)
    }
}

/// Prints the domain errors (one line each) on standard output.
pub fn print_domain_errors(out: &mut dyn std::io::Write, n: usize) {
    for _ in 0..n {
        let _ = out.write_all(DOMAIN_ERROR.as_bytes());
    }
}

/// What a voxelwise pass found: FSL-151 changed a conversion; the default's value differs from
/// FSL's (the pass's defect); the domain errors printed.
#[derive(Clone, Copy, Default)]
struct Pass {
    fsl151: bool,
    differs: bool,
    errors: usize,
}

impl Pass {
    fn merge(self, o: Pass) -> Pass {
        Pass {
            fsl151: self.fsl151 || o.fsl151,
            differs: self.differs || o.differs,
            errors: self.errors + o.errors,
        }
    }
}

/// Maps every voxel through `f(v) -> (fsl's value, the default's value, domain errors)`.
fn voxelwise<T: Num>(
    data: &mut [T],
    fix: bool,
    f: impl Fn(f64) -> (f64, f64, usize) + Sync,
) -> Pass {
    data.par_chunks_mut(super::ops::BLOCK)
        .map(|c| {
            let mut p = Pass::default();
            for v in c {
                let (rep, def, e) = f(v.to_f64());
                let (a, ch1) = T::convert(rep, fix);
                let (b, ch2) = T::convert(def, fix);
                p.differs |= !a.same(b);
                if fix {
                    *v = b;
                    p.fsl151 |= ch2;
                    // the default's own domain errors only (none: FSL-172 corrected)
                    p.errors += if def.to_bits() == rep.to_bits() { e } else { 0 };
                } else {
                    *v = a;
                    p.fsl151 |= ch1;
                    p.errors += e;
                }
            }
            p
        })
        .reduce(Pass::default, Pass::merge)
}

/// `-ztop`: `1 − Φ(v)`; FSL-170: an infinite `v` gives NaN in FSL (the default: 0 for `+∞`, 1 for
/// `−∞`).
pub fn ztop<T: Num>(img: &mut Img<T>, notices: &mut Notices) {
    let fix = notices.fix();
    let p = voxelwise(&mut img.data, fix, |v| {
        let def = 1.0 - ndtr(v);
        let def = if v.is_nan() { num::quiet(v) } else { def };
        // FSL's NaN for an infinite z is positive (black-box runs: findings 16)
        let rep = if v.is_infinite() { f64::NAN } else { def };
        (rep, def, 0)
    });
    if p.differs {
        notices.add(
            &defects::FSL_170,
            match notices.mode() {
                UpstreamDefects::Fix => {
                    "-ztop of an infinite z: 0 for +inf, 1 for -inf (FSL's is NaN)"
                }
                UpstreamDefects::Reproduce => "-ztop of an infinite z: NaN, as FSL gives it",
            },
        );
    }
    super::ops::note151(notices, p.fsl151, "-ztop's values");
}

/// `-ptoz`: 0 outside `(0, 1)`, else `Φ⁻¹(1 − v)`; FSL-172: where `1 − v` rounds to 1 FSL's
/// `Φ⁻¹(1)` is the domain error and `MAXNUM` (the default: `−Φ⁻¹(v)`).
pub fn ptoz<T: Num>(img: &mut Img<T>, notices: &mut Notices, out: &mut dyn std::io::Write) {
    let fix = notices.fix();
    let p = voxelwise(&mut img.data, fix, |v| {
        if v.is_nan() {
            // FSL's -ptoz of NaN is the default NaN (black-box runs: findings 16)
            return (num::default_nan(), num::default_nan(), 0);
        }
        if v <= 0.0 || v >= 1.0 {
            return (0.0, 0.0, 0);
        }
        let (rep, e) = ndtri(1.0 - v);
        let def = if e && 1.0 - v == 1.0 {
            -ndtri(v).0
        } else {
            rep
        };
        (rep, def, usize::from(e))
    });
    print_domain_errors(out, p.errors);
    if p.differs {
        notices.add(
            &defects::FSL_172,
            match notices.mode() {
                UpstreamDefects::Fix => {
                    "-ptoz of p-values whose 1 - p rounds to 1: their z, -Φ⁻¹(p) (FSL gives \
                     +inf with ndtri's domain error)"
                }
                UpstreamDefects::Reproduce => {
                    "-ptoz of p-values whose 1 - p rounds to 1: ndtri's domain error and the \
                     largest value, as FSL gives them"
                }
            },
        );
    }
    super::ops::note151(notices, p.fsl151, "-ptoz's values");
}

/// `-randn` (§8.2): `Φ⁻¹(u)` of the generator's next number (converted) added to every voxel,
/// in storage order; `u = 0` prints the domain error and adds `−MAXNUM`.
pub fn randn<T: Num>(
    img: &mut Img<T>,
    rng: &mut super::ops::WichmannHill,
    notices: &mut Notices,
    out: &mut dyn std::io::Write,
) {
    let fix = notices.fix();
    let (mut errors, mut changed) = (0usize, false);
    for v in &mut img.data {
        let (z, e) = ndtri(rng.next_value());
        errors += usize::from(e);
        let (zt, c) = T::convert(z, fix);
        changed |= c;
        *v = v.add(zt);
    }
    print_domain_errors(out, errors);
    super::ops::note151(notices, changed, "-randn's numbers");
}

/// Which of `-pval`, `-pval0`, `-cpval`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PvalKind {
    Pval,
    Pval0,
    Cpval,
}

/// `-pval`, `-pval0`, `-cpval` (§12): the first volume's statistic against the others (or, for
/// `-cpval`, against each volume's maximum); the output is the first volume's grid. FSL-169 for
/// NaN: the default gives a NaN statistic a NaN p-value and leaves NaN permutation values out.
pub fn pval<T: Num>(img: &mut Img<T>, kind: PvalKind, notices: &mut Notices) {
    let fix = notices.fix();
    let per = img.per_volume();
    let nt = img.dims[3].max(1);
    let data = &img.data;
    let ntf = nt as f32;
    // -cpval: each volume's maximum, started at the image's minimum (single precision; FSL's
    // restarts after a NaN: the maximum after the last NaN), and without the NaN values
    let maxima: Vec<(f32, f32)> = if kind == PvalKind::Cpval {
        let lo = super::ops::extremes(data, false).map_or(0.0, |e| num::to_f32(e.0.to_f64()));
        let lo_ok = super::ops::extremes(data, true).map_or(0.0, |e| num::to_f32(e.0.to_f64()));
        (0..nt)
            .into_par_iter()
            .map(|t| {
                let vol = &data[t * per..(t + 1) * per];
                let (mut m, mut ok) = (lo, lo_ok);
                for v in vol {
                    let x = v.to_f64();
                    if !(num::to_f64(m) > x) {
                        m = num::to_f32(x);
                    }
                    if !x.is_nan() && x > num::to_f64(ok) {
                        ok = num::to_f32(x);
                    }
                }
                (m, ok)
            })
            .collect()
    } else {
        Vec::new()
    };
    let one = |k: usize| -> (f32, f32) {
        let v0 = data[k].to_f64();
        match kind {
            PvalKind::Pval | PvalKind::Pval0 => {
                if kind == PvalKind::Pval0 && v0 == 0.0 {
                    return (0.0, 0.0);
                }
                let (mut hit, mut n, mut hit_ok, mut n_ok) = (1u32, 1u32, 1u32, 1u32);
                for t in 1..nt {
                    let x = data[t * per + k].to_f64();
                    let counted = kind == PvalKind::Pval || x != 0.0;
                    if counted {
                        n += 1;
                        hit += u32::from(v0 <= x);
                        if !x.is_nan() {
                            n_ok += 1;
                            hit_ok += u32::from(v0 <= x);
                        }
                    }
                }
                let rep = if kind == PvalKind::Pval {
                    hit as f32 / ntf
                } else {
                    hit as f32 / n as f32
                };
                let def = if v0.is_nan() {
                    num::to_f32(num::quiet(v0))
                } else {
                    hit_ok as f32 / n_ok as f32
                };
                (rep, def)
            }
            PvalKind::Cpval => {
                let rep = maxima.iter().filter(|m| v0 <= num::to_f64(m.0)).count() as f32 / ntf;
                let def = if v0.is_nan() {
                    num::to_f32(num::quiet(v0))
                } else {
                    maxima.iter().filter(|m| v0 <= num::to_f64(m.1)).count() as f32 / ntf
                };
                (rep, def)
            }
        }
    };
    let results: Vec<(T, bool, bool)> = (0..per)
        .into_par_iter()
        .map(|k| {
            let (rep, def) = one(k);
            let (a, c1) = T::convert(num::to_f64(rep), fix);
            let (b, c2) = T::convert(num::to_f64(def), fix);
            if fix {
                (b, c2, !a.same(b))
            } else {
                (a, c1, !a.same(b))
            }
        })
        .collect();
    let changed = results.iter().any(|r| r.1);
    let differs = results.iter().any(|r| r.2);
    img.data = results.into_iter().map(|r| r.0).collect();
    img.dims[3] = 1;
    let op = match kind {
        PvalKind::Pval => "-pval",
        PvalKind::Pval0 => "-pval0",
        PvalKind::Cpval => "-cpval",
    };
    if differs {
        notices.add(
            &defects::FSL_169,
            match notices.mode() {
                UpstreamDefects::Fix => format!(
                    "{op} with NaN values: a NaN statistic's p-value NaN, NaN permutation values \
                     left out (FSL counts them)"
                ),
                UpstreamDefects::Reproduce => {
                    format!("{op} with NaN values: counted as FSL counts them")
                }
            },
        );
    }
    super::ops::note151(notices, changed, "the p-values");
}

/// The ranks 1 to `n` of a series (§12's procedure for ties: `[2, 1, 2]` gives `[3, 1, 2]`).
fn ranks(v: &[f64]) -> Vec<usize> {
    let n = v.len();
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut r: Vec<usize> = (1..=n).collect();
    for k in 0..n {
        if v[k] != s[k] {
            for l in k + 1..n {
                if s[l] == v[k] {
                    r.swap(k, l);
                    s.swap(k, l);
                }
            }
        }
    }
    r
}

/// `-rank` (`norm` false) and `-ranknorm` (§12) of every voxel's series. FSL-162: a NaN in a
/// series stops FSL (`sort(): detected NaN`); the default leaves the NaN values out (they stay
/// NaN; the others ranked among themselves). FSL-167: `-ranknorm`'s standard deviation.
pub fn rank<T: Num>(
    img: &mut Img<T>,
    norm: bool,
    notices: &mut Notices,
    out: &mut dyn std::io::Write,
) -> Result<(), Stop> {
    let fix = notices.fix();
    let per = img.per_volume();
    let nt = img.dims[3].max(1);
    let data = &img.data;
    let nan = super::ops::any(data, |v| v.is_nan());
    if nan && !fix {
        return Err(Stop::Exception(super::filters::abort_lines(
            "std::logic_error",
            "sort(): detected NaN",
        )));
    }
    let op = if norm { "-ranknorm" } else { "-rank" };
    // per voxel: its series' new values, FSL-151, FSL-167 (the default's value differs), the
    // domain errors
    let cols: Vec<(Vec<T>, bool, bool, usize)> = (0..per)
        .into_par_iter()
        .map(|k| {
            let series: Vec<T> = (0..nt).map(|t| data[t * per + k]).collect();
            let keep: Vec<usize> = (0..nt).filter(|&t| !series[t].is_nan()).collect();
            let vals: Vec<f64> = keep.iter().map(|&t| series[t].to_f64()).collect();
            let r = ranks(&vals);
            let mut res = series.clone();
            let (mut ch, mut d167, mut errs) = (false, false, 0usize);
            if norm {
                let kept: Vec<T> = keep.iter().map(|&t| series[t]).collect();
                let m = if kept.is_empty() {
                    0.0
                } else {
                    reduce::mean(&kept)
                };
                let (s_fsl, s_masked) = if kept.is_empty() {
                    (0.0, 0.0)
                } else {
                    reduce::std(&kept)
                };
                let n = keep.len() as f64;
                for (j, &t) in keep.iter().enumerate() {
                    let (z, e) = ndtri((r[j] as f64 - 0.5) / n);
                    errs += usize::from(e);
                    let with = |s: f64| {
                        let zs = num::nan64(z, s, z * s);
                        num::nan64(m, zs, m + zs)
                    };
                    let (ta, c1) = T::convert(with(s_fsl), fix);
                    let (tb, c2) = T::convert(with(s_masked), fix);
                    d167 |= !ta.same(tb);
                    if fix {
                        res[t] = tb;
                        ch |= c2;
                    } else {
                        res[t] = ta;
                        ch |= c1;
                    }
                }
            } else {
                for (j, &t) in keep.iter().enumerate() {
                    let (v, c) = T::convert(r[j] as f64, fix);
                    res[t] = v;
                    ch |= c;
                }
            }
            (res, ch, d167, errs)
        })
        .collect();
    let mut errors = 0;
    let (mut changed, mut d167) = (false, false);
    let mut new = vec![T::zero(); data.len()];
    for (k, (res, ch, d, e)) in cols.into_iter().enumerate() {
        for (t, v) in res.into_iter().enumerate() {
            new[t * per + k] = v;
        }
        changed |= ch;
        d167 |= d;
        errors += e;
    }
    print_domain_errors(out, errors);
    img.data = new;
    if nan {
        notices.add(
            &defects::FSL_162,
            format!(
                "{op} of series holding NaN: the NaN values left out of the ranks (FSL stops: \
                 sort(): detected NaN)"
            ),
        );
    }
    if d167 {
        notices.add(
            &defects::FSL_167,
            match notices.mode() {
                UpstreamDefects::Fix => format!(
                    "{op}: the series' variance clamped at 0 and 0 for one value (FSL's is NaN \
                     there)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "{op}: the series' variance without the clamp at 0 (NaN for one value), as \
                     FSL takes it"
                ),
            },
        );
    }
    super::ops::note151(notices, changed, "the ranks");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_normal_distribution_and_its_inverse() {
        assert_eq!(ndtr(0.0), 0.5);
        assert!((ndtr(1.959_963_984_540_054) - 0.975).abs() < 1e-15);
        assert_eq!(ndtri(0.5), (0.0, false));
        for &p in &[1e-300, 1e-20, 0.001, 0.0228, 0.3, 0.6, 0.975, 0.999_999] {
            let z = ndtri(p).0;
            let back = ndtr(z);
            // the round trip is bounded by Φ's own rounding in the far tail (2 x² ulp)
            assert!(((back - p) / p).abs() < 1e-12, "{p}: {z} -> {back}");
        }
        assert_eq!(ndtri(0.0), (-MAXNUM, true));
        assert_eq!(ndtri(1.0), (MAXNUM, true));
        // Φ⁻¹(0.025) = −1.95996398454005423552...
        assert!((ndtri(0.025).0 + 1.959_963_984_540_054_2).abs() <= 2.3e-16);
    }

    #[test]
    fn ranks_of_ties() {
        assert_eq!(ranks(&[2.0, 1.0, 2.0]), vec![3, 1, 2]);
        assert_eq!(ranks(&[5.0, 3.0, 9.0]), vec![2, 1, 3]);
    }
}

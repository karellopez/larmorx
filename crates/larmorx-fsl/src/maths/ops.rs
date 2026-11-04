// SPDX-License-Identifier: Apache-2.0
//! fslmaths' voxelwise operations (`specs/fslmaths.md` §5-§8): the binary operations with a
//! number or an image, the thresholds, binarisation and masks, the voxelwise functions, the
//! random numbers, the index and the grid.
//!
//! Every operation is a pass over the values in the computing type `T`, split over the thread
//! pool in blocks: each value depends on its own voxel only, so the result is the same at any
//! thread count (the index, the grid's lines and the random numbers follow the storage order,
//! and run in order). FSL's defects (FSL-151 to FSL-159) are corrected by default; where the
//! correction changed a value its notice says so, which needs FSL's result beside the
//! corrected one only where the two can differ.

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use rayon::prelude::*;

use super::image::Img;
use super::num::{self, Num};

/// Values per parallel task.
pub const BLOCK: usize = 1 << 15;

/// The extremes from the first value (§11.1's convention, FSL-153): each later value strictly
/// larger becomes the maximum, else each strictly smaller the minimum; with `skip_nan` NaN
/// values are left out (all NaN: NaN). `None` for no values.
pub fn extremes<T: Num>(data: &[T], skip_nan: bool) -> Option<(T, T)> {
    let part = |c: &[T]| -> Option<(T, T)> {
        let mut it = c.iter().copied();
        let first = if skip_nan {
            it.by_ref().find(|v| !v.is_nan())?
        } else {
            it.next()?
        };
        let (mut lo, mut hi) = (first, first);
        for v in it {
            if skip_nan && v.is_nan() {
                continue;
            }
            if v > hi {
                hi = v;
            } else if v < lo {
                lo = v;
            }
        }
        Some((lo, hi))
    };
    // the blocks' extremes combined in order: the first block's value wins a tie, as in one
    // pass; without skip_nan only the first value can make them NaN
    let parts: Vec<Option<(T, T)>> = data.par_chunks(BLOCK).map(part).collect();
    let mut it = parts.into_iter().flatten();
    let (mut lo, mut hi) = it.next().or_else(|| {
        // every value NaN
        (skip_nan && !data.is_empty()).then(|| (data[0], data[0]))
    })?;
    for (l, h) in it {
        if h > hi {
            hi = h;
        }
        if l < lo {
            lo = l;
        }
    }
    Some((lo, hi))
}

/// Applies `f` to every value; the `bool` it returns (FSL-151 changed the value) is gathered.
pub fn map<T: Num>(data: &mut [T], f: impl Fn(T) -> (T, bool) + Sync) -> bool {
    data.par_chunks_mut(BLOCK)
        .map(|c| {
            let mut ch = false;
            for v in c {
                let (x, c) = f(*v);
                *v = x;
                ch |= c;
            }
            ch
        })
        .reduce(|| false, |a, b| a || b)
}

/// Whether any value satisfies `f`.
pub fn any<T: Num>(data: &[T], f: impl Fn(T) -> bool + Sync) -> bool {
    data.par_chunks(BLOCK).any(|c| c.iter().any(|&v| f(v)))
}

/// A number argument converted to the computing type (FSL-151), noted.
pub fn number<T: Num>(v: f64, notices: &mut Notices) -> T {
    let (x, changed) = T::convert(v, notices.fix());
    if changed {
        super::image::fsl151(notices, &format!("the number {v}"));
    }
    x
}

/// FSL-151 noted for `what` when `changed`.
pub fn note151(notices: &mut Notices, changed: bool, what: &str) {
    if changed {
        super::image::fsl151(notices, what);
    }
}

// ------------------------------------------------------------------------------------------
// Binary operations (§5)

/// The binary operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Max,
    Min,
}

impl Binary {
    pub fn of(op: &str) -> Option<Binary> {
        Some(match op {
            "-add" => Binary::Add,
            "-sub" => Binary::Sub,
            "-mul" => Binary::Mul,
            "-div" => Binary::Div,
            "-rem" => Binary::Rem,
            "-max" => Binary::Max,
            "-min" => Binary::Min,
            _ => return None,
        })
    }
}

/// Why an operation stopped: FSL-156's division by zero (refused in both modes), or another
/// refusal, with its message.
#[derive(Debug)]
pub enum Stop {
    /// An integer division by zero (FSL-156): FSL crashes.
    DivZero(&'static str),
    /// The image library's exception (FSL aborts): its lines.
    Exception(String),
    /// A refusal of larmorx's (a rule-B error, a crash of FSL's): the message.
    Refuse(String),
}

/// `(int) v` as `-rem` takes it (FSL-151 for values beyond `int` or NaN).
#[inline]
fn int_of<T: Num>(v: T, fix: bool) -> (i32, bool) {
    i32::convert(v.to_f64(), fix)
}

/// `(int) v % (int) d` converted back to `T`; `None` for a remainder by 0 (or `INT_MIN % -1`).
#[inline]
fn rem<T: Num>(v: T, d: T, fix: bool) -> Option<(T, bool)> {
    let (a, c1) = int_of(v, fix);
    let (b, c2) = int_of(d, fix);
    if b == 0 || (a == i32::MIN && b == -1) {
        return None;
    }
    let (r, c3) = T::convert(f64::from(a % b), fix);
    Some((r, c1 || c2 || c3))
}

/// `v op n` with a number `n` (already in `T`); `atof_zero`: the number was 0 (`-div 0`
/// does nothing).
pub fn with_number<T: Num>(
    img: &mut Img<T>,
    op: Binary,
    n: T,
    atof_zero: bool,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let fix = notices.fix();
    let changed = match op {
        Binary::Add => map(&mut img.data, |v| (v.add(n), false)),
        Binary::Sub => map(&mut img.data, |v| (v.sub(n), false)),
        Binary::Mul => map(&mut img.data, |v| (v.mul(n), false)),
        Binary::Div => {
            if atof_zero {
                return Ok(());
            }
            if T::KIND.is_int() && n == T::zero() {
                return Err(Stop::DivZero(
                    "-div by a number whose integer part is 0 under an integer type",
                ));
            }
            if T::KIND.is_int() && n.to_f64() == -1.0 && any(&img.data, |v| v.to_f64() == T::MIN) {
                return Err(Stop::DivZero("the most negative int divided by -1"));
            }
            map(&mut img.data, |v| (v.div(n), false))
        }
        Binary::Rem => {
            let (d, _) = int_of(n, fix);
            if d == 0 {
                return Err(Stop::DivZero("-rem by a number whose integer part is 0"));
            }
            let bad = any(&img.data, |v| rem(v, n, fix).is_none());
            if bad {
                return Err(Stop::DivZero("the most negative int's remainder by -1"));
            }
            map(&mut img.data, |v| rem(v, n, fix).unwrap_or((v, false)))
        }
        Binary::Max => map(&mut img.data, |v| (if v > n { v } else { n }, false)),
        Binary::Min => map(&mut img.data, |v| (if v < n { v } else { n }, false)),
    };
    note151(notices, changed, "-rem's integer conversions");
    Ok(())
}

/// The operand's value for each voxel of the current image (§3.2, FSL-159).
#[derive(Clone, Copy, Debug)]
pub enum Layout {
    /// The operand's values reused cyclically in storage order (`-add`, `-sub`, `-mul`, `-mas`;
    /// `-div` ... `-min` with matching sizes).
    Cyclic,
    /// FSL-159 reproduced for `-div` and `-rem`: the operand's value at the same voxel indices,
    /// 0 outside it.
    Corner,
    /// FSL-159 reproduced for `-max` and `-min`: the operand's storage at the current image's
    /// indices with the operand's strides.
    Strided,
}

/// The operand's value at linear index `i` of the current image.
#[inline]
fn operand_at<T: Num>(w: &Img<T>, cur: [usize; 4], i: usize, layout: Layout) -> Option<T> {
    match layout {
        Layout::Cyclic => Some(w.data[i % w.data.len()]),
        Layout::Corner | Layout::Strided => {
            let [nx, ny, nz, _] = cur;
            let x = i % nx;
            let y = (i / nx) % ny;
            let z = (i / (nx * ny)) % nz;
            let t = i / (nx * ny * nz);
            let [ox, oy, oz, ot] = w.dims;
            match layout {
                Layout::Corner => {
                    if x < ox && y < oy && z < oz && (t < ot || ot == 1) {
                        let t = if ot == 1 { 0 } else { t };
                        Some(w.data[x + ox * (y + oy * (z + oz * t))])
                    } else {
                        Some(T::zero())
                    }
                }
                _ => {
                    let t = if ot == 1 { 0 } else { t };
                    w.data.get(x + ox * (y + oy * (z + oz * t))).copied()
                }
            }
        }
    }
}

/// `v op w` with an image `w` laid out by `layout`.
pub fn with_image<T: Num>(
    img: &mut Img<T>,
    op: Binary,
    w: &Img<T>,
    layout: Layout,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let fix = notices.fix();
    let cur = img.dims;
    if w.data.is_empty() {
        return Ok(());
    }
    if matches!(layout, Layout::Strided)
        && (0..img.data.len()).any(|i| operand_at(w, cur, i, layout).is_none())
    {
        return Err(Stop::Refuse(
            "-max/-min with a smaller image read past its end (FSL-159: FSL reads memory it \
             never wrote)"
                .into(),
        ));
    }
    if op == Binary::Rem {
        let bad = (0..img.data.len()).any(|i| {
            let d = operand_at(w, cur, i, layout).unwrap_or_else(T::zero);
            d != T::zero() && rem(img.data[i], d, fix).is_none()
        });
        if bad {
            return Err(Stop::DivZero(
                "-rem by an image value whose integer part is 0",
            ));
        }
    }
    if op == Binary::Div && T::KIND.is_int() {
        let bad = (0..img.data.len()).any(|i| {
            let d = operand_at(w, cur, i, layout).unwrap_or_else(T::zero);
            d.to_f64() == -1.0 && img.data[i].to_f64() == T::MIN
        });
        if bad {
            return Err(Stop::DivZero("the most negative int divided by -1"));
        }
    }
    let changed = img
        .data
        .par_chunks_mut(BLOCK)
        .enumerate()
        .map(|(b, c)| {
            let mut ch = false;
            for (k, v) in c.iter_mut().enumerate() {
                let i = b * BLOCK + k;
                let d = operand_at(w, cur, i, layout).unwrap_or_else(T::zero);
                *v = match op {
                    Binary::Add => v.add(d),
                    Binary::Sub => v.sub(d),
                    Binary::Mul => v.mul(d),
                    Binary::Div => {
                        if d == T::zero() {
                            T::zero()
                        } else {
                            v.div(d)
                        }
                    }
                    Binary::Rem => {
                        if d == T::zero() {
                            *v
                        } else {
                            let (r, c) = rem(*v, d, fix).unwrap_or((*v, false));
                            ch |= c;
                            r
                        }
                    }
                    Binary::Max => {
                        if *v > d {
                            *v
                        } else {
                            d
                        }
                    }
                    Binary::Min => {
                        if *v < d {
                            *v
                        } else {
                            d
                        }
                    }
                };
            }
            ch
        })
        .reduce(|| false, |a, b| a || b);
    note151(notices, changed, "-rem's integer conversions");
    Ok(())
}

// ------------------------------------------------------------------------------------------
// Thresholds, binarisation, masks (§6)

/// FSL's far side of a threshold (FSL-152, FSL-153): the maximum plus 1 (`upper`) or the
/// minimum minus 1, in the computing type, from FSL's extremes (NaN when the first value is
/// NaN).
fn far_side<T: Num>(data: &[T], upper: bool) -> T {
    match extremes(data, false) {
        Some((lo, hi)) => {
            if upper {
                hi.add(T::one())
            } else {
                lo.sub(T::one())
            }
        }
        None => T::zero(),
    }
}

/// Notes FSL-152 or FSL-153 when the correction kept a value FSL zeroes.
fn note_far<T: Num>(notices: &mut Notices, data: &[T], what: &str) {
    let nan_first = data.first().is_some_and(|v| v.is_nan());
    let (defect, why) = if nan_first {
        (
            &defects::FSL_153,
            "the image's first voxel is NaN, which makes FSL's extremes NaN",
        )
    } else {
        (
            &defects::FSL_152,
            "FSL bounds the other side by the image's maximum plus 1 (or minimum minus 1) in the computing type",
        )
    };
    let detail = match notices.mode() {
        larmorx_core::defects::UpstreamDefects::Fix => {
            format!("{what}: the other side bounds nothing; {why}, which zeroes some voxels")
        }
        larmorx_core::defects::UpstreamDefects::Reproduce => {
            format!("{what}: {why}, so some voxels are zeroed as FSL zeroes them")
        }
    };
    notices.add(defect, detail);
}

/// A threshold: the corrected one and FSL's (they differ only where the robust range's defects
/// moved it), with the defects that moved it.
#[derive(Clone, Debug)]
pub struct Limit<T> {
    pub fix: T,
    pub rep: T,
    pub causes: Vec<(&'static defects::Defect, &'static str)>,
}

impl<T: Num> Limit<T> {
    /// A threshold given as a number.
    pub fn exact(t: T) -> Limit<T> {
        Limit {
            fix: t,
            rep: t,
            causes: Vec::new(),
        }
    }
}

/// `-thr t` (`lower`) or `-uthr t`: voxels below (above) `t` and NaN become 0. Each defect is
/// named where it changes the output: the robust range's (FSL-153, FSL-187: FSL's limit kept
/// other voxels) and the far side's (FSL-152, FSL-153).
pub fn threshold<T: Num>(
    img: &mut Img<T>,
    lim: &Limit<T>,
    lower: bool,
    notices: &mut Notices,
    what: &str,
) {
    let far = far_side(&img.data, lower);
    let (tf, tr) = (lim.fix, lim.rep);
    let keep_fix = |v: T| if lower { v >= tf } else { v <= tf };
    let keep_mid = |v: T| if lower { v >= tr } else { v <= tr };
    let keep_rep = |v: T| keep_mid(v) && if lower { v <= far } else { v >= far };
    if !lim.causes.is_empty() && any(&img.data, |v| keep_fix(v) != keep_mid(v)) {
        for &(d, why) in &lim.causes {
            notices.add(d, format!("{what}: {why}"));
        }
    }
    if any(&img.data, |v| keep_mid(v) != keep_rep(v)) {
        note_far(notices, &img.data, what);
    }
    let fix = notices.fix();
    map(&mut img.data, |v| {
        let keep = if fix { keep_fix(v) } else { keep_rep(v) };
        (if keep { v } else { T::zero() }, false)
    });
}

/// `-bin`: 1 where `v > 0` (FSL: `0 < v < max + 1`), else 0.
pub fn bin_values<T: Num>(data: &mut [T], notices: &mut Notices, what: &str) {
    let far = far_side(data, true);
    let fix = notices.fix();
    let b_fix = |v: T| v > T::zero();
    let b_rep = |v: T| v > T::zero() && v < far;
    if any(data, |v| b_fix(v) != b_rep(v)) {
        note_far(notices, data, what);
    }
    map(data, |v| {
        let b = if fix { b_fix(v) } else { b_rep(v) };
        (if b { T::one() } else { T::zero() }, false)
    });
}

/// `-binv`: 1 where `-bin` gives 0.
pub fn binv<T: Num>(img: &mut Img<T>, notices: &mut Notices) {
    bin_values(&mut img.data, notices, "-binv");
    map(&mut img.data, |v| {
        (if v == T::zero() { T::one() } else { T::zero() }, false)
    });
}

/// `-invbin` (hidden): 1 where `v = 0`.
pub fn invbin<T: Num>(img: &mut Img<T>) {
    map(&mut img.data, |v| {
        (if v == T::zero() { T::one() } else { T::zero() }, false)
    });
}

// ------------------------------------------------------------------------------------------
// The robust range (§6.4)

/// A robust range: the corrected one and FSL's (`None`: an empty mask), and the defects that
/// part them.
#[derive(Clone, Debug)]
pub struct Range<T> {
    pub fix: Option<(T, T)>,
    pub rep: Option<(T, T)>,
    pub causes: Vec<(&'static defects::Defect, &'static str)>,
}

/// The robust range of the values `mask` keeps (`None`: every value), in the computing type.
/// FSL-153 (NaN values counted in the lowest bin, NaN extremes from a NaN first value) and
/// FSL-187 (a value far above a pass's range counted in the first bin) are corrected by
/// default; FSL's range is computed beside it only where they can part the two.
pub fn robust_range<T: Num>(
    data: &[T],
    mask: Option<&(dyn Fn(T) -> bool + Sync)>,
    fix: bool,
) -> Range<T> {
    if !fix {
        let r = robust_pass(data, mask, false, false).map(|x| x.limits);
        return Range {
            fix: r,
            rep: r,
            causes: Vec::new(),
        };
    }
    let f = robust_pass(data, mask, true, true);
    let nan = any(data, |v| v.is_nan() && mask.is_none_or(|m| m(v)));
    let far = f.as_ref().is_some_and(|x| x.far);
    if !nan && !far {
        let r = f.map(|x| x.limits);
        return Range {
            fix: r,
            rep: r,
            causes: Vec::new(),
        };
    }
    let r = robust_pass(data, mask, false, false);
    let mut causes = Vec::new();
    if nan {
        causes.push((
            &defects::FSL_153,
            "the robust range leaves NaN values out (FSL counts them in its lowest bin, and a NaN first value makes its extremes NaN)",
        ));
    }
    if far || r.as_ref().is_some_and(|x| x.far) {
        causes.push((
            &defects::FSL_187,
            "the robust range counts values far above a pass's range in its last bin (FSL counts them in its first)",
        ));
    }
    Range {
        fix: f.map(|x| x.limits),
        rep: r.map(|x| x.limits),
        causes,
    }
}

struct Robust<T> {
    limits: (T, T),
    /// A value's bin was beyond 2^31 (FSL-187's case).
    far: bool,
}

fn robust_pass<T: Num>(
    data: &[T],
    mask: Option<&(dyn Fn(T) -> bool + Sync)>,
    skip_nan: bool,
    last_bin: bool,
) -> Option<Robust<T>> {
    let keep = |v: T| mask.is_none_or(|m| m(v)) && !(skip_nan && v.is_nan());
    // the extremes of the kept values, from the first (FSL-153)
    let kept_first = data.par_chunks(BLOCK).map(|c| {
        let mut it = c.iter().copied().filter(|&v| keep(v));
        let first = it.next()?;
        let (mut lo, mut hi) = (first, first);
        let mut n = 1usize;
        for v in it {
            n += 1;
            if v > hi {
                hi = v;
            } else if v < lo {
                lo = v;
            }
        }
        Some((lo, hi, n))
    });
    let parts: Vec<Option<(T, T, usize)>> = kept_first.collect();
    let mut it = parts.into_iter().flatten();
    let (mut lo0, mut hi0, mut total) = it.next()?;
    for (l, h, n) in it {
        total += n;
        if h > hi0 {
            hi0 = h;
        }
        if l < lo0 {
            lo0 = l;
        }
    }
    if lo0.same(hi0) || lo0 == hi0 {
        return Some(Robust {
            limits: (lo0, hi0),
            far: false,
        });
    }
    let mut far = false;
    let (mut lo, mut hi) = (lo0, hi0);
    let (mut t2, mut t98) = (lo0, hi0);
    let mut full = false;
    for pass in 0..10 {
        let last = pass == 9;
        if last || full {
            (lo, hi) = (lo0, hi0);
        }
        let ends_out = last || full;
        full = false;
        let w = hi.diff(lo);
        let lof = lo.to_f64();
        let a = 1000.0 / w;
        let b = 1000.0 * (-lof) / w;
        let (hist, f) = histogram(data, &keep, a, b, last_bin);
        far |= f;
        let (first, end) = if ends_out {
            (1usize, 998usize)
        } else {
            (0, 999)
        };
        let n = if ends_out {
            total - hist[0] - hist[999]
        } else {
            total
        };
        let target = n / 50;
        let mut count = 0usize;
        let mut bl = first as i64 - 1;
        while count < target && bl < end as i64 {
            bl += 1;
            count += hist[bl as usize];
        }
        count = 0;
        let mut u = end as i64 + 1;
        while count < target && u > first as i64 {
            u -= 1;
            count += hist[u as usize];
        }
        let delta = w / 1000.0;
        let at = |k: i64| lo.add(T::from_f64_c(k as f64 * delta));
        t2 = at(bl);
        t98 = at(u + 1);
        if last || !(t98.diff(t2) < w / 10.0) {
            break;
        }
        let b1 = (bl - 1).max(0) as f64;
        let u1 = ((u + 1).min(999) + 1) as f64;
        lo = T::from_f64_c(lof + (b1 / 1000.0) * w);
        hi = T::from_f64_c(lof + (u1 / 1000.0) * w);
        if lo == hi {
            full = true;
        }
    }
    Some(Robust {
        limits: (t2, t98),
        far,
    })
}

/// The 1000-bin histogram of the kept values: bin `trunc(A v + B)` clamped to [0, 999]; a bin
/// beyond `int`'s range (x86-64's conversion gives `INT_MIN`) in the first bin unless
/// `last_bin` (FSL-187's correction), and whether that happened.
fn histogram<T: Num>(
    data: &[T],
    keep: &(dyn Fn(T) -> bool + Sync),
    a: f64,
    b: f64,
    last_bin: bool,
) -> (Vec<usize>, bool) {
    data.par_chunks(BLOCK)
        .map(|c| {
            let mut h = vec![0usize; 1000];
            let mut far = false;
            for &v in c {
                if !keep(v) {
                    continue;
                }
                let x = v.to_f64() * a + b;
                let k = if x >= 2_147_483_648.0 {
                    far = true;
                    if last_bin { 999 } else { 0 }
                } else {
                    num::cvtt(x).clamp(0, 999)
                };
                h[k as usize] += 1;
            }
            (h, far)
        })
        .reduce(
            || (vec![0usize; 1000], false),
            |(mut x, fx), (y, fy)| {
                for (p, q) in x.iter_mut().zip(y) {
                    *p += q;
                }
                (x, fx || fy)
            },
        )
}

/// The limit of `-thrp`, `-uthrp`, `-thrP`, `-uthrP`: `rmin + (p/100)(rmax − rmin)`, the sum in
/// double, converted to the computing type. The robust range is computed three times, as FSL
/// does (its warning printed each time for an empty mask).
pub fn percent_limit<T: Num>(
    data: &[T],
    p: f64,
    positive: bool,
    notices: &mut Notices,
    err: &mut dyn std::io::Write,
) -> Limit<T> {
    let pos = |v: T| {
        let x = v.to_f64();
        x >= T::MIN_POSITIVE && x <= T::MAX
    };
    let mask: Option<&(dyn Fn(T) -> bool + Sync)> = if positive { Some(&pos) } else { None };
    let fix = notices.fix();
    let range = robust_range(data, mask, fix);
    let active = if fix { range.fix } else { range.rep };
    if active.is_none() {
        for _ in 0..3 {
            let _ = writeln!(err, "WARNING:: Empty mask image in calc_robustlimits");
        }
    }
    let limit = |r: Option<(T, T)>, notices: &mut Notices| -> T {
        let (rmin, rmax) = r.unwrap_or((T::zero(), T::zero()));
        let lim = rmin.to_f64() + (p / 100.0) * rmax.diff(rmin);
        let (x, changed) = T::convert(lim, fix);
        note151(notices, changed, "the threshold");
        x
    };
    let f = limit(range.fix, notices);
    let r = if range.causes.is_empty() {
        f
    } else {
        limit(range.rep, notices)
    };
    Limit {
        fix: f,
        rep: r,
        causes: range.causes,
    }
}

// ------------------------------------------------------------------------------------------
// Voxelwise functions (§7)

/// The voxelwise functions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unary {
    Exp,
    Log,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sqr,
    Sqrt,
    Recip,
    Abs,
    Nan,
    Nanm,
    Pow(f64),
}

impl Unary {
    pub fn of(op: &str) -> Option<Unary> {
        Some(match op {
            "-exp" => Unary::Exp,
            "-log" => Unary::Log,
            "-sin" => Unary::Sin,
            "-cos" => Unary::Cos,
            "-tan" => Unary::Tan,
            "-asin" => Unary::Asin,
            "-acos" => Unary::Acos,
            "-atan" => Unary::Atan,
            "-sqr" => Unary::Sqr,
            "-sqrt" => Unary::Sqrt,
            "-recip" => Unary::Recip,
            "-abs" => Unary::Abs,
            "-nan" => Unary::Nan,
            "-nanm" => Unary::Nanm,
            _ => return None,
        })
    }
}

/// A double function of §7 with FSL's NaN results: a NaN argument passes on quieted; `asin`
/// and `acos` outside [-1, 1] give a positive NaN; `sin`, `cos`, `tan` of an infinity the
/// default NaN. The values are correctly rounded (GLIBC-07: glibc's are not everywhere).
#[inline]
fn double_fn(f: Unary, x: f64) -> f64 {
    if x.is_nan() {
        return num::quiet(x);
    }
    match f {
        Unary::Exp => math::exp(x),
        Unary::Log => math::log(x),
        Unary::Sin | Unary::Cos | Unary::Tan if x.is_infinite() => num::default_nan(),
        Unary::Sin => math::sin(x),
        Unary::Cos => math::cos(x),
        Unary::Tan => math::tan(x),
        Unary::Asin | Unary::Acos if !(-1.0..=1.0).contains(&x) => f64::NAN,
        Unary::Asin => math::asin(x),
        Unary::Acos => math::acos(x),
        Unary::Atan => math::atan2(x, 1.0),
        Unary::Pow(p) => {
            if p.is_nan() {
                return if x == 1.0 { 1.0 } else { num::quiet(p) };
            }
            let r = math::pow(x, p);
            if r.is_nan() { num::default_nan() } else { r }
        }
        _ => x,
    }
}

/// Applies a voxelwise function.
pub fn unary<T: Num>(img: &mut Img<T>, f: Unary, notices: &mut Notices) {
    let fix = notices.fix();
    let changed = match f {
        Unary::Log => map(&mut img.data, |v| {
            if v > T::zero() {
                T::convert(math::log(v.to_f64()), fix)
            } else {
                (v, false)
            }
        }),
        Unary::Sqr => map(&mut img.data, |v| (v.mul(v), false)),
        Unary::Sqrt => map(&mut img.data, |v| {
            if v > T::zero() {
                if T::KIND == num::Kind::Float {
                    T::convert(f64::from((v.to_f64() as f32).sqrt()), fix)
                } else {
                    T::convert(v.to_f64().sqrt(), fix)
                }
            } else {
                (T::zero(), false)
            }
        }),
        Unary::Recip => map(&mut img.data, |v| {
            if v == T::zero() {
                (v, false)
            } else {
                (T::one().div(v), false)
            }
        }),
        Unary::Abs => map(&mut img.data, |v| {
            if T::KIND.is_int() {
                T::convert(v.to_f64().abs(), fix)
            } else {
                // the sign bit cleared, a NaN's too
                (T::from_f64_c(v.to_f64().abs()), false)
            }
        }),
        Unary::Nan => map(&mut img.data, |v| {
            (if v.non_finite() { T::zero() } else { v }, false)
        }),
        Unary::Nanm => map(&mut img.data, |v| {
            (if v.non_finite() { T::one() } else { T::zero() }, false)
        }),
        _ => map(&mut img.data, |v| T::convert(double_fn(f, v.to_f64()), fix)),
    };
    note151(notices, changed, "a voxelwise function's results");
}

// ------------------------------------------------------------------------------------------
// Random numbers, the index, the grid (§8)

/// Wichmann and Hill's generator (AS 183) as fslmaths runs it.
#[derive(Clone, Copy, Debug)]
pub struct WichmannHill {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Default for WichmannHill {
    fn default() -> Self {
        WichmannHill {
            x: 1,
            y: 10000,
            z: 3000,
        }
    }
}

impl WichmannHill {
    /// `-seed i`: glibc's `srand(i)`, three `rand()` draws `r1, r2, r3`, and the states
    /// `r3 mod 30000 + 1`, `r2 mod 30000 + 1`, `r1 mod 30000 + 1`.
    pub fn seeded(seed: i32) -> WichmannHill {
        let mut g = crate::fast::rng::GlibcRand::new(seed as u32);
        let r1 = g.next_value() as i32;
        let r2 = g.next_value() as i32;
        let r3 = g.next_value() as i32;
        WichmannHill {
            x: r3 % 30000 + 1,
            y: r2 % 30000 + 1,
            z: r1 % 30000 + 1,
        }
    }

    /// The next number in [0, 1): the states advanced by Schrage's method, then
    /// `((s − ⌊s⌋) + 1) − 1` of `s = x/30269 + y/30307 + z/30323`.
    #[inline]
    pub fn next_value(&mut self) -> f64 {
        fn step(v: i32, a: i32, q: i32, r: i32, m: i32) -> i32 {
            let k = v / q;
            let mut n = a * (v - q * k) - r * k;
            if n < 0 {
                n += m;
            }
            n
        }
        self.x = step(self.x, 171, 177, 2, 30269);
        self.y = step(self.y, 172, 176, 35, 30307);
        self.z = step(self.z, 170, 178, 63, 30323);
        let s =
            f64::from(self.x) / 30269.0 + f64::from(self.y) / 30307.0 + f64::from(self.z) / 30323.0;
        ((s - s.floor()) + 1.0) - 1.0
    }
}

/// `-rand`: a uniform number in [0, 1) added to every voxel, in storage order.
pub fn rand<T: Num>(img: &mut Img<T>, rng: &mut WichmannHill, notices: &mut Notices) {
    let fix = notices.fix();
    for v in &mut img.data {
        let (u, _) = T::convert(rng.next_value(), fix);
        *v = v.add(u);
    }
}

/// `-index`: the voxels above 0 numbered 0, 1, 2, ... in storage order over every volume, the
/// 32-bit counter converted to the computing type (wrapping under `char` and `short`: rule C).
pub fn index<T: Num>(img: &mut Img<T>) {
    let mut k: i32 = 0;
    for v in &mut img.data {
        if *v > T::zero() {
            *v = T::from_f64_c(f64::from(k));
            k = k.wrapping_add(1);
        }
    }
}

/// `-grid v s`: the voxels whose x, y or z index is a multiple of `s` set to `v`.
pub fn grid<T: Num>(img: &mut Img<T>, v: T, s: i32) {
    let s = s.unsigned_abs() as usize;
    let [nx, ny, nz, _] = img.dims;
    let per = nx * ny * nz;
    img.data.par_chunks_mut(per.max(1)).for_each(|vol| {
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    if x % s == 0 || y % s == 0 || z % s == 0 {
                        vol[x + nx * (y + ny * z)] = v;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extremes_follow_the_first_value() {
        assert_eq!(extremes(&[3.0f32, 1.0, 5.0], false), Some((1.0, 5.0)));
        let (lo, hi) = extremes(&[f32::NAN, 1.0, 5.0], false).unwrap();
        assert!(lo.is_nan() && hi.is_nan());
        assert_eq!(extremes(&[f32::NAN, 1.0, 5.0], true), Some((1.0, 5.0)));
        // the sign of a zero maximum: the first zero
        let (_, hi) = extremes(&[-0.0f32, 0.0], false).unwrap();
        assert!(hi.is_sign_negative());
    }

    #[test]
    fn wichmann_hill_starts_from_its_fixed_state() {
        let mut g = WichmannHill::default();
        let u = g.next_value();
        assert!((0.0..1.0).contains(&u));
        assert_eq!((g.x, g.y, g.z), (171, 22808, 24832));
    }
}

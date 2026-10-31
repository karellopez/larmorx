// SPDX-License-Identifier: Apache-2.0
//! The intensity profiles of betsurf (`specs/bet.md` §6.4): samples of the T1 (and the T2)
//! every 0.5 mm along a vertex's normal, the searches over them, and the estimates of the inner
//! skull, the outer skull and the scalp each profile gives.
//!
//! A profile holds its samples at the positions `s0, s0 + 0.5, ...` (mm along the normal) and a
//! region `[L, R)` of them; the thresholds `th(a) = min + a (max - min)` use the region's
//! minimum and maximum. Forward searches start at the first sample at or after a position and
//! stop at the region's end; backward searches start at the last sample at or before a position
//! and stop at the region's start. A search that finds nothing gives [`NONE`].

/// What a search gives when it finds nothing.
pub const NONE: f64 = -500.0;

/// The spacing of the samples (mm).
pub const STEP: f64 = 0.5;

/// One profile's samples and its region.
#[derive(Clone, Debug)]
pub struct Profile {
    /// The position of the first sample (mm).
    pub start: f64,
    pub values: Vec<f32>,
    /// The region `[lo, hi)`, as sample indices (`hi` may be at or below `lo`: empty).
    lo: usize,
    hi: usize,
    /// Whether a backward search started past the last sample (FSL-51).
    pub overrun: bool,
    /// Probe variants (`V_*` bits).
    pub variant: u32,
}

pub const V_BWD_END: u32 = 1;
pub const V_FWD_START: u32 = 2;
pub const V_TH_F32: u32 = 4;
pub const V_WITHIN_LT: u32 = 8;
pub const V_SCAN_SKIP_A: u32 = 16;
pub const V_OS2_FALLBACK: u32 = 32;
pub const V_COUNTER_FIRST: u32 = 64;
pub const V_REGION_INCL: u32 = 128;
pub const V_B_NONE: u32 = 256;
pub const V_NO_A_MIN: u32 = 512;

impl Profile {
    pub fn new(start: f64, values: Vec<f32>) -> Profile {
        let hi = values.len();
        Profile {
            start,
            values,
            lo: 0,
            hi,
            overrun: false,
            variant: 0,
        }
    }

    /// The position of sample `k`.
    #[inline]
    pub fn position(&self, k: usize) -> f64 {
        self.start + k as f64 * STEP
    }

    /// The index of the first sample at or after `s` (0 below the first sample).
    fn first_at_or_after(&self, s: f64) -> usize {
        let x = ((s - self.start) / STEP).ceil();
        if x.is_nan() {
            usize::MAX
        } else if x <= 0.0 {
            0
        } else {
            x.min(usize::MAX as f64 / 2.0) as usize
        }
    }

    /// The index of the last sample at or before `s`, `None` below the first sample.
    fn last_at_or_before(&self, s: f64) -> Option<usize> {
        let x = ((s - self.start) / STEP).floor();
        if !(x >= 0.0) {
            None
        } else {
            Some(x.min(usize::MAX as f64 / 2.0) as usize)
        }
    }

    /// The region starts at the first sample at or after `s`.
    pub fn set_start(&mut self, s: f64) {
        self.lo = self.first_at_or_after(s).min(self.values.len());
    }

    /// The region ends before the first sample at or after `s`.
    pub fn set_end(&mut self, s: f64) {
        // observed (single-profile runs of FSL's betsurf): the region holds the sample at `s`
        let k = if self.variant & V_REGION_INCL == 0 {
            self.last_at_or_before(s).map_or(0, |k| k + 1)
        } else {
            self.first_at_or_after(s)
        };
        self.hi = k.min(self.values.len());
    }

    /// The region's start (mm).
    pub fn region_start(&self) -> f64 {
        self.position(self.lo)
    }

    /// The position of the region's last sample (the start of a search "before the end").
    pub fn last_position(&self) -> f64 {
        if self.hi == 0 {
            NONE
        } else {
            self.position(self.hi - 1)
        }
    }

    /// `th(a) = min + a (max - min)` over the region (NaN for an empty region).
    pub fn th(&self, a: f64) -> f64 {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        if self.lo >= self.hi {
            return f64::NAN;
        }
        for &v in &self.values[self.lo..self.hi] {
            let v = f64::from(v);
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
        if self.variant & V_TH_F32 != 0 {
            let (l, h) = (lo as f32, hi as f32);
            return f64::from(l + (a as f32) * (h - l));
        }
        lo + a * (hi - lo)
    }

    /// The first sample at or after `s`, within the region's end, that satisfies `test`.
    fn forward(&self, s: f64, test: impl Fn(f64) -> bool) -> f64 {
        let mut k = self.first_at_or_after(s);
        if self.variant & V_FWD_START != 0 {
            k = k.max(self.lo);
        }
        while k < self.hi {
            if test(f64::from(self.values[k])) {
                return self.position(k);
            }
            k += 1;
        }
        NONE
    }

    /// The last sample at or before `s`, down to the region's start, that satisfies `test`; a
    /// start past the last sample reads "nothing there" for the samples beyond it (FSL-51:
    /// noted in [`Profile::overrun`]).
    fn backward(&mut self, s: f64, test: impl Fn(f64) -> bool) -> f64 {
        let Some(mut k) = self.last_at_or_before(s) else {
            return NONE;
        };
        if self.variant & V_BWD_END != 0 && k >= self.hi {
            if self.hi == 0 {
                return NONE;
            }
            k = self.hi - 1;
        }
        if k >= self.values.len() {
            self.overrun = true;
            k = self.values.len().saturating_sub(1);
            if self.values.is_empty() {
                return NONE;
            }
        }
        loop {
            if k < self.lo {
                return NONE;
            }
            if test(f64::from(self.values[k])) {
                return self.position(k);
            }
            if k == 0 {
                return NONE;
            }
            k -= 1;
        }
    }

    /// The first sample at or after `s` not above `th`.
    pub fn first_le(&self, s: f64, th: f64) -> f64 {
        self.forward(s, |v| v <= th)
    }

    /// The first sample at or after `s` not below `th`.
    pub fn first_ge(&self, s: f64, th: f64) -> f64 {
        self.forward(s, |v| v >= th)
    }

    /// The last sample at or before `s` not below `th`.
    pub fn last_ge(&mut self, s: f64, th: f64) -> f64 {
        self.backward(s, |v| v >= th)
    }

    /// The last sample at or before `s` not above `th`.
    pub fn last_le(&mut self, s: f64, th: f64) -> f64 {
        self.backward(s, |v| v <= th)
    }

    /// The samples from index of position `s` (inclusive) to the region's end.
    pub fn samples_from(&self, s: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let k0 = self.first_at_or_after(s);
        (k0..self.hi.max(k0)).map(|k| (self.position(k), f64::from(self.values[k])))
    }
}

/// What a profile gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Estimate {
    /// Dropped: it counts for nothing (the verbose line's "removed profiles").
    Dropped,
    /// Only its scalp counts.
    Scalp(f64),
    /// The inner skull, the outer skull and the scalp.
    Kept { inner: f64, outer: f64, scalp: f64 },
    /// FSL ends the program here (FSL-50, reproduced).
    Stop,
}

/// Probe counters (temporary).
pub static TALLY: [std::sync::atomic::AtomicUsize; 16] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; 16];

fn tally(k: usize) {
    TALLY[k].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Whether `a` lies within 10 mm of `b`.
#[inline]
fn within_10(a: f64, b: f64, variant: u32) -> bool {
    if variant & V_WITHIN_LT != 0 {
        return (a - b).abs() < 10.0;
    }
    (a - b).abs() <= 10.0
}

/// The scalp search of step 2: the last sample before the end not below `th(0.2)`, and the one
/// before it from 1.5 mm further in; `(S, S')`.
fn scalp(p: &mut Profile) -> (f64, f64) {
    let th = p.th(0.2);
    let s = p.last_ge(p.last_position(), th);
    let s1 = p.last_ge(s - 1.5, th);
    (s, s1)
}

/// A T1 and T2 profile away from the eyes (§6.4, steps 1 to 8).
pub fn t1t2(t1: &mut Profile, t2: &mut Profile, fsl50: bool) -> Estimate {
    // 1.
    t1.set_start(-3.0);
    t2.set_start(-3.0);
    // 2.
    let (mut s, s1) = scalp(t1);
    if s - s1 > 2.0 {
        s = s1;
    }
    let th2 = t2.th(0.25);
    let s2 = t2.last_ge(t2.last_position(), th2);
    // 3.
    if (s - s2).abs() > 5.0 {
        let m = s.min(s2) + 15.0;
        if s >= s2 {
            let th = t1.th(0.2);
            let x = t1.last_ge(m, th);
            if within_10(x, s2, t1.variant) {
                s = x;
            } else {
                tally(0);
                return Estimate::Dropped;
            }
        } else {
            let th = t2.th(0.25);
            let y = t2.last_ge(m, th);
            if !within_10(y, s, t1.variant) {
                tally(1);
                return Estimate::Dropped;
            }
            // FSL-49: the T1's scalp kept (rule C)
        }
    }
    // 4.
    t1.set_end(s);
    t2.set_end(s);
    let mut is = t1.first_le(t2.region_start(), t1.th(0.25));
    let is2 = t2.first_le(is, t2.th(0.30));
    if is2 > is {
        is = is2;
    }
    t2.set_start(is);
    // 5.
    let (bright, dark) = (t2.th(0.40), t2.th(0.30));
    let mut pos = is;
    let mut first_bright = NONE;
    let mut first_gap = NONE;
    // the bright samples found (observed: "more than one" counts them, a bright sample with
    // no dark one after it included)
    let mut brights = 0usize;
    let mut gaps = 0usize;
    while gaps < 6 {
        let b = t2.first_ge(pos, bright);
        if b == NONE {
            break;
        }
        brights += 1;
        if first_bright == NONE {
            first_bright = b;
        }
        let g = t2.first_le(b, dark);
        if g == NONE {
            break;
        }
        if first_gap == NONE {
            first_gap = g;
        }
        gaps += 1;
        pos = g;
    }
    let from = if brights > 1 && within_10(first_bright, is, t1.variant) {
        first_gap
    } else {
        is + 0.5
    };
    t1.set_start(from);
    // 6.
    let x = t1.last_ge(s, t1.th(0.75));
    let mut os2 = t1.last_le(x, t1.th(0.30));
    if os2 == NONE {
        // observed: the outer skull search's start
        os2 = t1.region_start();
    }
    // 7.
    let a = t1.first_le(t1.region_start(), t1.th(0.30));
    if a == NONE {
        return if fsl50 {
            Estimate::Stop
        } else {
            Estimate::Dropped
        };
    }
    let (up, low) = (t1.th(0.25), t1.th(0.15));
    let mut b = a;
    // observed: the scan includes `a` itself
    for (pos, v) in t1.samples_from(a) {
        if v > up {
            break;
        }
        if v < low {
            b = pos;
        }
    }
    let os = t1.first_ge(b, low);
    // 8.
    for (k, ok) in [os2 - os < 3.0, os - os2 < 2.0, s - os >= 2.0, os >= is]
        .into_iter()
        .enumerate()
    {
        if !ok {
            tally(4 + k);
        }
    }
    if os2 - os < 3.0 && os - os2 < 2.0 && s - os >= 2.0 && os >= is {
        Estimate::Kept {
            inner: is,
            outer: os,
            scalp: s,
        }
    } else {
        Estimate::Scalp(s)
    }
}

/// A T1 and T2 profile near the eyes (§6.4).
pub fn t1t2_eyes(t1: &mut Profile, t2: &mut Profile) -> Estimate {
    t1.set_start(-3.0);
    t2.set_start(-3.0);
    let mut is = t1.first_le(t1.region_start(), t1.th(0.25));
    let is2 = t2.first_le(is, t2.th(0.30));
    if is2 > is {
        is = is2;
    }
    if is > 15.0 {
        is = 0.0;
    }
    let (s, s1) = scalp(t1);
    if s - s1 > 2.0 {
        return Estimate::Dropped;
    }
    t1.set_end(s);
    let w = t1.first_le(is, t1.th(0.25));
    let mut os = t1.first_ge(w, t1.th(0.35));
    if w - is >= 7.0 || os < is {
        os = is + 0.5;
    }
    Estimate::Kept {
        inner: is,
        outer: os,
        scalp: s,
    }
}

/// A T1-only profile away from the eyes (§6.4).
pub fn t1only(t1: &mut Profile) -> Estimate {
    let (mut s, s1) = scalp(t1);
    if s - s1 > 2.0 {
        s = s1;
    }
    t1.set_end(s);
    let is = t1.first_le(t1.region_start(), t1.th(0.25));
    t1.set_start(is);
    let x = t1.last_ge(s, t1.th(0.75));
    let mut os2 = t1.last_le(x, t1.th(0.20));
    if os2 == NONE && t1.variant & V_OS2_FALLBACK != 0 {
        os2 = x;
    }
    let a = t1.first_le(t1.region_start(), t1.th(0.30));
    if a == NONE {
        return Estimate::Dropped;
    }
    let a = if t1.variant & V_NO_A_MIN != 0 {
        a
    } else {
        a.max(-3.0)
    };
    let (stop, mid) = (t1.th(0.60), t1.th(0.15));
    let mut counter = 0u32;
    let mut b = if t1.variant & V_B_NONE != 0 { NONE } else { a };
    let skip = usize::from(t1.variant & V_SCAN_SKIP_A != 0);
    let counter_first = t1.variant & V_COUNTER_FIRST != 0;
    for (pos, v) in t1.samples_from(a).skip(skip) {
        if counter_first && counter >= 12 {
            break;
        }
        if v > stop {
            break;
        }
        if v > mid {
            counter += 1;
        } else if v < mid {
            counter = counter.saturating_sub(1);
            b = pos;
        }
        if !counter_first && counter >= 12 {
            break;
        }
    }
    let os = t1.first_ge(b, mid);
    let d = os2 - os;
    // observed (single-profile runs): an outer skull found inside the inner skull is not kept
    // (FSL sets the profile aside where both searches found nothing); `OS ≥ IS` would give the
    // same decisions on every profile probed (a question for the spec writer)
    if (-2.0..2.0).contains(&d) && s - os2 >= 2.0 && os2 >= is {
        Estimate::Kept {
            inner: is,
            outer: os2,
            scalp: s,
        }
    } else {
        Estimate::Scalp(s)
    }
}

/// A T1-only profile near the eyes (§6.4).
pub fn t1only_eyes(t1: &mut Profile) -> Estimate {
    let th = t1.th(0.2);
    let s = t1.last_ge(t1.last_position(), th);
    let ek: u32 = std::env::var("LX_EYE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if ek & 1 == 0 {
        t1.set_end(s);
    }
    let mut is = t1.first_le(-3.0, t1.th(0.25));
    if is > 5.0 || (ek & 4 != 0 && is >= 5.0) {
        is = 0.0;
    }
    let os = t1.first_ge(if ek & 2 != 0 { is + 0.5 } else { is }, t1.th(0.35));
    Estimate::Kept {
        inner: is,
        outer: os,
        scalp: s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches() {
        // positions -3, -2.5, ..., values a ramp up then down
        let v: Vec<f32> = (0..20)
            .map(|k| if k < 10 { k as f32 } else { (19 - k) as f32 })
            .collect();
        let mut p = Profile::new(-3.0, v);
        assert_eq!(p.th(0.0), 0.0);
        assert_eq!(p.th(1.0), 9.0);
        assert_eq!(p.first_ge(-3.0, 5.0), -3.0 + 5.0 * 0.5);
        assert_eq!(p.first_le(-1.0, 1.0), -3.0 + 18.0 * 0.5);
        assert_eq!(p.last_ge(p.last_position(), 8.0), -3.0 + 11.0 * 0.5);
        assert_eq!(p.last_le(0.0, 0.5), -3.0);
        assert!(!p.overrun);
        assert_eq!(p.last_ge(100.0, 8.0), -3.0 + 11.0 * 0.5);
        assert!(p.overrun);
        p.set_end(0.0);
        assert_eq!(p.first_le(-1.0, -1.0), NONE);
        assert_eq!(p.th(1.0), 5.0);
    }
}

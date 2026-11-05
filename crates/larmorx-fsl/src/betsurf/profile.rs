// SPDX-License-Identifier: Apache-2.0
//! The intensity profiles of betsurf (`specs/bet.md` §6.4): samples of the T1 (and the T2)
//! every 0.5 mm along a vertex's normal, the searches over them, and the estimates of the inner
//! skull, the outer skull and the scalp each profile gives.
//!
//! A profile holds its samples at the positions `s0, s0 + 0.5, ...` (mm along the normal) and a
//! region of them; the thresholds `th(a) = min + a (max - min)` use the region's minimum and
//! maximum. Forward searches start at the first sample at or after a position and stop at the
//! region's end; backward searches start at the last sample at or before a position and stop at
//! the region's start. A search that finds nothing gives [`NONE`].
//!
//! What the spec leaves open was settled by black-box runs of FSL 6.0.7.23's betsurf on single
//! profiles (`validation/investigations/cleanroom-bet/betsurf_profile_oracle.py`: a tiny
//! triangle that casts exactly one profile, FSL's `-v` saying whether it set it aside; 330 of
//! 330 profiles agree, `docs/findings/fsl-bet-original.md`, "betsurf"): a region that ends at a
//! position holds the sample there; step 5's "more than one" counts the bright samples found;
//! step 7's scan includes `a`; step 6's fallback is the region's start; a T1-only profile is not
//! kept when its outer skull lies inside its inner skull. And from single profiles of the
//! random-testing trial of 2026-11-04 (tiny heads, 2026-11-05): the backward searches never
//! read a profile's first sample, and neither does the T1-only scan for the outer skull (3,284 of
//! 3,288 T1-only profiles of eight images as FSL decides); in step 5 each search starts after
//! the sample the previous one found, a first bright sample must lie less than 10 mm from the
//! inner skull, and an outer-skull search range holding no sample counts the scalp only (2,002
//! of 2,002 T1-and-T2 profiles of three images away from the eyes).

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
}

impl Profile {
    pub fn new(start: f64, values: Vec<f32>) -> Profile {
        let hi = values.len();
        Profile {
            start,
            values,
            lo: 0,
            hi,
            overrun: false,
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

    /// The region ends at `s`, the sample there included (observed).
    pub fn set_end(&mut self, s: f64) {
        let k = self.last_at_or_before(s).map_or(0, |k| k + 1);
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
        if self.lo >= self.hi {
            return f64::NAN;
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in &self.values[self.lo..self.hi] {
            let v = f64::from(v);
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
        lo + a * (hi - lo)
    }

    /// The first sample at or after `s`, within the region's end, that satisfies `test`.
    fn forward(&self, s: f64, test: impl Fn(f64) -> bool) -> f64 {
        let mut k = self.first_at_or_after(s);
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
    /// noted in [`Profile::overrun`]; observed: FSL's decisions on such profiles are those).
    fn backward(&mut self, s: f64, test: impl Fn(f64) -> bool) -> f64 {
        let Some(mut k) = self.last_at_or_before(s) else {
            return NONE;
        };
        if k >= self.values.len() {
            self.overrun = true;
            if self.values.is_empty() {
                return NONE;
            }
            k = self.values.len() - 1;
        }
        // observed: the profile's first sample is never reached (a search whose only match is
        // there finds nothing)
        loop {
            if k < self.lo || k == 0 {
                return NONE;
            }
            if test(f64::from(self.values[k])) {
                return self.position(k);
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

    /// The samples from the one at or after position `s` to the region's end.
    pub fn samples_from(&self, s: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let k0 = self.first_at_or_after(s);
        (k0..self.hi.max(k0)).map(|k| (self.position(k), f64::from(self.values[k])))
    }
}

/// What a profile gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Estimate {
    /// Dropped: it counts for nothing.
    Dropped,
    /// Only its scalp counts.
    Scalp(f64),
    /// The inner skull, the outer skull and the scalp.
    Kept { inner: f64, outer: f64, scalp: f64 },
    /// FSL ends the program here (FSL-50 as `specs/bet.md` §6.4 step 7 states it; not observed
    /// in black-box runs, so the command line never asks for it).
    Stop,
}

/// Whether `a` lies within 10 mm of `b`.
#[inline]
fn within_10(a: f64, b: f64) -> bool {
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
            if within_10(x, s2) {
                s = x;
            } else {
                return Estimate::Dropped;
            }
        } else {
            let th = t2.th(0.25);
            let y = t2.last_ge(m, th);
            if !within_10(y, s) {
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
    // observed: "more than one" counts the bright samples found, a bright one with no dark
    // sample after it included
    let mut brights = 0usize;
    let mut gaps = 0usize;
    // observed (single profiles, 2026-11-05): each search of the alternation starts after the
    // sample the previous one found (a flat region's samples are bright and dark at once)
    while gaps < 6 {
        let b = t2.first_ge(if gaps == 0 { pos } else { pos + STEP }, bright);
        if b == NONE {
            break;
        }
        brights += 1;
        if first_bright == NONE {
            first_bright = b;
        }
        let g = t2.first_le(b + STEP, dark);
        if g == NONE {
            break;
        }
        if first_gap == NONE {
            first_gap = g;
        }
        gaps += 1;
        pos = g;
    }
    // observed (single profiles, 2026-11-05): "within 10 mm" of the inner skull is strict here
    // (a first bright sample exactly 10 mm away does not count)
    let from = if brights > 1 && (first_bright - is).abs() < 10.0 {
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
    // 7. (observed: a search range holding no sample counts the scalp only, FSL running on)
    let a = t1.first_le(t1.region_start(), t1.th(0.30));
    if a == NONE {
        return if fsl50 {
            Estimate::Stop
        } else {
            Estimate::Scalp(s)
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
    let os2 = t1.last_le(x, t1.th(0.20));
    let a = t1.first_le(t1.region_start(), t1.th(0.30));
    if a == NONE {
        return Estimate::Dropped;
    }
    let a = a.max(-3.0);
    let (stop, mid) = (t1.th(0.60), t1.th(0.15));
    let mut counter = 0u32;
    let mut b = a;
    // observed (single profiles, 2026-11-05): the scan does not read the profile's first
    // sample, though `a` may be there (`b` starts at `a`)
    let from = if a <= t1.start { t1.start + STEP } else { a };
    for (pos, v) in t1.samples_from(from) {
        if v > stop {
            break;
        }
        if v > mid {
            counter += 1;
        } else if v < mid {
            counter = counter.saturating_sub(1);
            b = pos;
        }
        if counter >= 12 {
            break;
        }
    }
    let os = t1.first_ge(b, mid);
    let d = os2 - os;
    // observed: an outer skull inside the inner skull is not kept (FSL sets the profile aside
    // where both outer searches found nothing); `OS ≥ IS` gives the same decisions on every
    // profile probed (a question for the spec writer)
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
    t1.set_end(s);
    let mut is = t1.first_le(-3.0, t1.th(0.25));
    if is > 5.0 {
        is = 0.0;
    }
    let os = t1.first_ge(is, t1.th(0.35));
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
        // the first sample is never read backwards (observed): its only match finds nothing
        assert_eq!(p.last_le(0.0, 0.5), NONE);
        assert_eq!(p.last_le(0.0, 1.0), -2.5);
        // forwards it is
        assert_eq!(p.first_le(-3.0, 0.5), -3.0);
        assert!(!p.overrun);
        assert_eq!(p.last_ge(100.0, 8.0), -3.0 + 11.0 * 0.5);
        assert!(p.overrun);
        // the region ends at 0.0, the sample there (index 6, value 6) included
        p.set_end(0.0);
        assert_eq!(p.first_le(-1.0, -1.0), NONE);
        assert_eq!(p.th(1.0), 6.0);
    }

    /// Single profiles of the random-testing trial of 2026-11-04 (case 1156, profile 0;
    /// case 678, profile 120), decided as FSL 6.0.7.23 decides them.
    #[test]
    fn t1only_profiles_set_aside_and_kept_as_fsl() {
        // an inner and outer skull both at the first sample: FSL sets it aside (only the
        // backward search's match is there)
        let v = vec![
            0.0f32, 32.3, 96.2, 95.2, 94.1, 93.1, 92.0, 90.8, 89.7, 88.5, 87.3, 86.1, 85.0, 83.8,
            82.6, 81.5, 80.5, 79.4, 63.0,
        ];
        let mut v = v;
        v.extend([0.0f32; 107]);
        assert_eq!(t1only(&mut Profile::new(-3.0, v)), Estimate::Scalp(6.0));
        // noise where `a` is the first sample: the scan from the second one finds the outer
        // skull at 6.0 (from the first it would stop at 3.5 and set the profile aside)
        let mut v = vec![
            0.616f32, 0.919, 0.838, 1.091, 1.036, 0.697, 0.219, 0.638, 0.617, 0.682, 1.082, 0.942,
            0.708, 0.718, 0.407, 0.529, 0.466, 0.38, 0.856, 1.961, 1.316, 1.268, 0.83, 0.709,
            0.576, 0.823, 0.829, 0.59, 1.331, 1.185, 0.691, 0.718, 0.663, 0.623, 0.931, 1.394,
            0.705, 0.111,
        ];
        v.extend([0.0f32; 88]);
        match t1only(&mut Profile::new(-3.0, v)) {
            Estimate::Kept { outer, scalp, .. } => assert_eq!((outer, scalp), (5.5, 15.0)),
            e => panic!("{e:?}"),
        }
    }

    /// T1-and-T2 profiles of the random-testing trial of 2026-11-04 that FSL 6.0.7.23 sets
    /// aside (single-profile runs): case 1746's profile (a first bright T2 sample exactly 10 mm
    /// past the inner skull) and case 617's flat one (T1 and T2 alike, 161 to 159, then air).
    #[test]
    fn t1t2_profiles_set_aside_as_fsl() {
        let pad = |mut v: Vec<f32>| {
            let mut w = vec![0.0f32; 54];
            w.append(&mut v);
            w.resize(180, 0.0);
            w
        };
        let t1 = pad(vec![
            0.6, 0.6, 0.7, 0.7, 0.9, 1.2, 1.4, 1.1, 0.9, 0.7, 0.8, 1.1, 1.4, 1.6, 1.7, 1.5, 1.0,
            0.6, 30.1, 63.8, 97.2, 96.4, 63.5, 16.1, 41.7, 73.9, 106.6, 138.8, 171.0, 166.7, 124.1,
            104.0, 113.3, 143.2, 168.4, 165.6, 163.7, 162.3, 151.5, 152.7, 157.4, 124.1, 101.0,
            97.9, 123.7, 149.1, 143.1, 109.2, 93.9, 92.0, 70.7, 26.8, 0.4, 0.2, 0.0, 0.1,
        ]);
        let t2 = pad(vec![
            0.9, 1.0, 0.9, 0.8, 0.9, 1.2, 1.6, 1.3, 1.1, 1.1, 0.8, 0.4, 0.2, 0.3, 0.3, 0.4, 1.0,
            1.8, 21.2, 43.0, 65.0, 64.4, 42.7, 11.5, 28.6, 49.9, 71.3, 92.3, 113.4, 110.5, 82.2,
            68.5, 74.5, 94.5, 112.7, 110.1, 108.9, 107.3, 99.6, 100.5, 104.3, 83.0, 68.3, 66.8,
            84.3, 101.4, 96.9, 73.7, 63.6, 62.7, 48.2, 18.3, 0.4, 0.5, 0.5, 0.3,
        ]);
        let (mut p1, mut p2) = (Profile::new(-30.0, t1), Profile::new(-30.0, t2));
        assert_eq!(t1t2(&mut p1, &mut p2, false), Estimate::Scalp(22.0));
        let mut flat = vec![0.0f32; 53];
        flat.extend([161.0f32; 11]);
        flat.extend([160.0f32; 13]);
        flat.extend([159.0f32; 5]);
        flat.resize(180, 0.0);
        let (mut p1, mut p2) = (Profile::new(-30.0, flat.clone()), Profile::new(-30.0, flat));
        assert_eq!(t1t2(&mut p1, &mut p2, false), Estimate::Scalp(10.5));
    }

    #[test]
    fn a_t1only_profile() {
        // brain (bright), a dark skull, the scalp, then air: kept, the estimates in order
        let mut v = vec![900.0f32; 6];
        v.extend([100.0f32; 8]);
        v.extend([800.0f32; 10]);
        v.extend([5.0f32; 102]);
        let mut p = Profile::new(-3.0, v);
        match t1only(&mut p) {
            Estimate::Kept {
                inner,
                outer,
                scalp,
            } => assert!(inner < outer && outer < scalp, "{inner} {outer} {scalp}"),
            e => panic!("{e:?}"),
        }
    }
}

// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! `Profile` (fsl-meshclass 2111.0, `profile.cpp`): intensities sampled along a line, with a
//! region of interest `[lroi, rroi)` and searches for the first or last sample above or below a
//! threshold between the region's minimum and maximum. betsurf reads the skull and the skin from
//! such profiles.
//!
//! A few of FSL's searches can read past the last sample, into memory the profile never set
//! (when the region of interest is empty at the end of the profile, or a search starts beyond
//! the last sample while the region ends earlier). There the replica reads a sample that is not
//! there (position +infinity, value NaN: nothing found) and records it in
//! [`Profile::past_end`] (FSL-51): FSL's result depends on memory contents there.

/// `pro_pair`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProPair {
    pub abs: f64,
    pub val: f64,
}

/// What a profile holds past its last sample.
const PAST_END: ProPair = ProPair {
    abs: f64::INFINITY,
    val: f64::NAN,
};

/// What FSL's searches return when they find nothing.
pub const NOT_FOUND: f64 = -500.0;

/// `Profile`.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub v: Vec<ProPair>,
    lroi: usize,
    rroi: usize,
    mindef: bool,
    maxdef: bool,
    amin: usize,
    amax: usize,
    /// A search read past the last sample, where FSL reads memory it never set (FSL-51).
    pub past_end: bool,
}

impl Profile {
    pub fn new() -> Profile {
        Profile {
            rroi: 1,
            ..Profile::default()
        }
    }

    /// The sample at `k`, or nothing past the end (recorded).
    #[inline]
    fn at(&mut self, k: usize) -> ProPair {
        if k < self.v.len() {
            self.v[k]
        } else {
            self.past_end = true;
            PAST_END
        }
    }

    /// The sample at `k` where reading past the end does not change FSL's result.
    #[inline]
    fn peek(&self, k: usize) -> ProPair {
        self.v.get(k).copied().unwrap_or(PAST_END)
    }

    /// `add(d, t)`.
    pub fn add(&mut self, d: f64, t: f64) {
        self.v.push(ProPair { abs: d, val: t });
        self.rroi = self.v.len();
        self.maxdef = false;
        self.mindef = false;
    }

    /// `init_roi()`.
    pub fn init_roi(&mut self) {
        self.lroi = 0;
        self.rroi = self.v.len();
        self.maxdef = false;
        self.mindef = false;
    }

    /// The index of the first sample at or after `abs` (the first loop of FSL's forward
    /// searches), and whether it ran to the end.
    fn forward(&self, abs: f64) -> (usize, bool) {
        let mut i = 0;
        while self.peek(i).abs < abs && i != self.v.len() {
            i += 1;
        }
        (i, i == self.v.len())
    }

    /// `set_lroi(abs)`.
    pub fn set_lroi(&mut self, abs: f64) {
        let (counter, _) = self.forward(abs);
        self.lroi = counter;
        self.maxdef = false;
        self.mindef = false;
        if self.rroi < self.lroi {
            self.rroi = self.lroi;
        }
    }

    /// `set_rroi(abs)`.
    pub fn set_rroi(&mut self, abs: f64) {
        let mut i = self.v.len() - 1;
        let mut counter = self.v.len();
        while self.v[i].abs > abs && i != 0 {
            counter -= 1;
            i -= 1;
        }
        self.rroi = counter;
        self.maxdef = false;
        self.mindef = false;
        if self.rroi < self.lroi {
            self.lroi = self.rroi;
        }
    }

    /// `value(d)`: the first sample at or after `d`; `None` past the end (FSL prints
    /// "out of range" and exits).
    pub fn value(&self, d: f64) -> Option<f64> {
        let (i, end) = self.forward(d);
        (!end).then(|| self.v[i].val)
    }

    /// `min()`.
    pub fn min(&mut self) -> f64 {
        if self.mindef {
            return self.at(self.amin).val;
        }
        let mut result = self.at(self.lroi).val;
        let mut abs = self.lroi;
        for i in self.lroi..self.rroi {
            if self.v[i].val < result {
                result = self.v[i].val;
                abs = i;
            }
        }
        self.mindef = true;
        self.amin = abs;
        result
    }

    /// `max()`.
    pub fn max(&mut self) -> f64 {
        if self.maxdef {
            return self.at(self.amax - 1).val;
        }
        let mut result = self.at(self.lroi).val;
        let mut abs = self.lroi;
        for i in self.lroi..self.rroi {
            if self.v[i].val > result {
                result = self.v[i].val;
                abs = i;
            }
        }
        self.maxdef = true;
        self.amax = abs + 1;
        result
    }

    /// `minabs()`.
    pub fn minabs(&mut self) -> f64 {
        if !self.mindef {
            self.min();
        }
        self.at(self.amin).abs
    }

    /// `maxabs()`.
    pub fn maxabs(&mut self) -> f64 {
        if !self.maxdef {
            self.max();
        }
        self.at(self.amax - 1).abs
    }

    /// `threshold(d)`: `min() + d * (max() - min())`.
    pub fn threshold(&mut self, d: f64) -> f64 {
        let min = self.min();
        let max = self.max();
        min + d * (max - self.min())
    }

    /// `begin()`: the first position of the region.
    pub fn begin(&mut self) -> f64 {
        self.at(self.lroi).abs
    }

    /// `end()`: the last position of the region.
    pub fn end(&self) -> f64 {
        self.v[self.rroi - 1].abs
    }

    /// `next_point_over(abs, thr)`: the first sample of the region at or after `abs` whose
    /// value is not below the threshold.
    pub fn next_point_over(&mut self, abs: f64, thr: f64) -> f64 {
        let t = self.threshold(thr);
        let (mut i, end) = self.forward(abs);
        if end {
            // FSL may step one past the end here and return memory it never set
            if self.rroi < self.v.len() {
                self.past_end = true;
            }
            return NOT_FOUND;
        }
        let mut counter = i;
        while self.v[i].val < t && counter < self.rroi {
            counter += 1;
            i += 1;
            if i == self.v.len() {
                return NOT_FOUND;
            }
        }
        if counter == self.rroi {
            NOT_FOUND
        } else {
            self.v[counter].abs
        }
    }

    /// `next_point_under(abs, thr)`: the first sample of the region at or after `abs` whose
    /// value is not above the threshold.
    pub fn next_point_under(&mut self, abs: f64, thr: f64) -> f64 {
        let t = self.threshold(thr);
        let (mut i, end) = self.forward(abs);
        if end {
            // FSL reads past the end; when the region ends earlier it returns that memory
            if self.rroi < self.v.len() {
                self.past_end = true;
            }
            return NOT_FOUND;
        }
        let mut counter = i;
        while self.v[i].val > t && counter < self.rroi {
            counter += 1;
            i += 1;
            if i == self.v.len() {
                return NOT_FOUND;
            }
        }
        if counter == self.rroi {
            NOT_FOUND
        } else {
            self.v[counter].abs
        }
    }

    /// `last_point_under(abs, thr)`: searching back from `abs`, the last sample of the region
    /// before the first one whose value is not above the threshold.
    pub fn last_point_under(&mut self, abs: f64, thr: f64) -> f64 {
        let t = self.threshold(thr);
        let mut i = self.v.len() - 1;
        let mut counter = self.v.len();
        while self.v[i].abs > abs && i != 0 {
            counter -= 1;
            i -= 1;
        }
        while counter > self.lroi && self.v[i].val > t && i != 0 {
            counter -= 1;
            i -= 1;
        }
        if counter == self.lroi || i == 0 {
            NOT_FOUND
        } else {
            self.v[counter - 1].abs
        }
    }

    /// `last_point_over(abs, thr)`: searching back from `abs`, the last sample of the region
    /// before the first one whose value is not below the threshold.
    pub fn last_point_over(&mut self, abs: f64, thr: f64) -> f64 {
        let t = self.threshold(thr);
        let mut i = self.v.len() - 1;
        let mut counter = self.v.len();
        while self.v[i].abs > abs && i != 0 {
            counter -= 1;
            i -= 1;
        }
        while self.v[i].val < t && counter > self.lroi && i != 0 {
            counter -= 1;
            i -= 1;
        }
        if counter == self.lroi || i == 0 {
            NOT_FOUND
        } else {
            self.v[counter - 1].abs
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(vals: &[f64]) -> Profile {
        let mut p = Profile::new();
        for (k, &v) in vals.iter().enumerate() {
            p.add(k as f64 * 0.5 - 1.0, v);
        }
        p.init_roi();
        p
    }

    #[test]
    fn searches() {
        let mut p = profile(&[10.0, 8.0, 2.0, 1.0, 5.0, 9.0, 9.0, 3.0, 0.0]);
        assert_eq!(p.threshold(0.5), 5.0);
        assert_eq!(p.next_point_under(-1.0, 0.5), 0.0);
        assert_eq!(p.next_point_over(0.0, 0.5), 1.0);
        assert_eq!(p.last_point_over(p.end(), 0.5), 2.0);
        assert_eq!(p.last_point_under(2.0, 0.5), 1.0);
        p.set_rroi(2.0);
        assert_eq!(p.end(), 2.0);
        p.set_lroi(0.25);
        assert_eq!(p.begin(), 0.5);
        assert_eq!(p.min(), 1.0);
        assert_eq!(p.max(), 9.0);
        assert!(!p.past_end);
        assert_eq!(p.value(1.2), Some(9.0));
        assert_eq!(p.value(100.0), None);
    }

    #[test]
    fn past_the_end() {
        let mut p = profile(&[3.0, 2.0, 1.0]);
        // an empty region at the end: its minimum is memory FSL never set
        p.set_lroi(10.0);
        assert!(p.threshold(0.3).is_nan());
        assert!(p.past_end);
        let mut p = profile(&[3.0, 2.0, 1.0]);
        p.set_rroi(-0.5);
        assert_eq!(p.next_point_under(10.0, 0.3), NOT_FOUND);
        assert!(p.past_end);
        let mut p = profile(&[3.0, 2.0, 1.0]);
        assert_eq!(p.next_point_under(10.0, 0.3), NOT_FOUND);
        assert!(!p.past_end);
    }
}

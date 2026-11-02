// SPDX-License-Identifier: Apache-2.0
//! Sums and percentiles over a mask (`specs/fast.md` §8.3), the class statistics of one channel
//! (§4.4) and the negative log Gaussian (§8.2).

use larmorx_core::math;

/// π in single precision (`3.14159265` read as a float), as FAST's Gaussian uses it.
pub const PI_F32: f32 = std::f32::consts::PI;

/// How many values each of FSL's partial sums holds: `N + 1`, `N = max(trunc(sqrt(V)), 10000)`
/// with `V` the volume's voxel count.
pub fn block_len(voxels: usize) -> usize {
    ((voxels as f64).sqrt().trunc() as usize).max(10_000) + 1
}

/// FSL's blocked sum: the values, in order, as doubles, into a partial sum that is added to the
/// total and reset after every [`block_len`] values.
#[derive(Clone, Copy, Debug)]
pub struct Blocked {
    total: f64,
    part: f64,
    k: usize,
    len: usize,
}

impl Blocked {
    pub fn new(len: usize) -> Blocked {
        Blocked {
            total: 0.0,
            part: 0.0,
            k: 0,
            len: len.max(1),
        }
    }

    #[inline]
    pub fn add(&mut self, v: f64) {
        self.part += v;
        self.k += 1;
        if self.k == self.len {
            self.total += self.part;
            self.part = 0.0;
            self.k = 0;
        }
    }

    pub fn value(&self) -> f64 {
        self.total + self.part
    }
}

/// The blocked sum of `values`.
pub fn sum(values: impl IntoIterator<Item = f64>, len: usize) -> f64 {
    let mut s = Blocked::new(len);
    for v in values {
        s.add(v);
    }
    s.value()
}

/// FSL's order for a percentile: ascending, NaN last.
fn nan_last(a: &f32, b: &f32) -> std::cmp::Ordering {
    match (a.is_nan(), b.is_nan()) {
        (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
    }
}

/// The values sorted as a percentile sorts them.
pub fn sorted(mut values: Vec<f32>) -> Vec<f32> {
    values.sort_unstable_by(nan_last);
    values
}

/// The percentile `p` of values sorted by [`sorted`]: the one at `trunc((float) count · p)`, at
/// most `count - 1`; 0 when there are none.
pub fn percentile_of_sorted(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let k = ((sorted.len() as f32) * p).trunc();
    let k = if k > 0.0 { k as usize } else { 0 };
    sorted[k.min(sorted.len() - 1)]
}

/// The percentile `p` of `values` (copied).
pub fn percentile(values: &[f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let k = ((values.len() as f32) * p).trunc();
    let k = (if k > 0.0 { k as usize } else { 0 }).min(values.len() - 1);
    let mut v = values.to_vec();
    let (_, x, _) = v.select_nth_unstable_by(k, nan_last);
    *x
}

/// One class's Gaussian in the log domain or the intensity domain: its mean, variance and the
/// normalisation `a = log(sqrt(2 π σ²))` (§8.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauss {
    pub mean: f32,
    pub var: f32,
    pub a: f32,
}

impl Gauss {
    pub fn new(mean: f32, var: f32) -> Gauss {
        let t = 2.0 * f64::from(PI_F32) * f64::from(var);
        Gauss {
            mean,
            var,
            a: math::log(t.sqrt()) as f32,
        }
    }

    /// The negative log Gaussian `G(x) = a + 0.5 (x - μ)² / σ²` (§8.2: the difference in
    /// single precision, the quotient in double, both terms rounded to single).
    #[inline(always)]
    pub fn neg_log(&self, x: f32) -> f32 {
        let d = f64::from(x - self.mean);
        let g = ((0.5 * d * d) / f64::from(self.var)) as f32;
        self.a + g
    }
}

/// Each class's mean and variance from the posteriors `post` (`n` per voxel) and the values
/// `y` over the mask (§4.4): `μ = Σ p y / Σ p` rounded to single, `σ² = Σ p y y / Σ p - μ²`
/// with the quotient in double and `μ²` in single, rounded to single.
pub fn class_stats(post: &[f32], y: &[f32], n: usize, len: usize) -> Vec<(f32, f32)> {
    let mut sums = vec![[Blocked::new(len); 3]; n];
    for (i, &yi) in y.iter().enumerate() {
        let row = &post[i * n..i * n + n];
        for (c, &p) in row.iter().enumerate() {
            let py = p * yi;
            let pyy = py * yi;
            let s = &mut sums[c];
            s[0].add(f64::from(p));
            s[1].add(f64::from(py));
            s[2].add(f64::from(pyy));
        }
    }
    sums.iter()
        .map(|s| {
            let w = s[0].value();
            let mean = (s[1].value() / w) as f32;
            // the second moment stays in double (observed against FSL with correctly rounded
            // maths, 2026-11-03: the spec's single-precision quotient gives other variances)
            let var = (s[2].value() / w - f64::from(mean * mean)) as f32;
            (mean, var)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_and_percentiles() {
        assert_eq!(block_len(256 * 256 * 256), 10_001);
        assert_eq!(block_len(30_000 * 30_000 * 2), 42_427);
        let mut b = Blocked::new(3);
        for v in [1.0, 2.0, 3.0, 4.0] {
            b.add(v);
        }
        assert_eq!(b.value(), 10.0);
        let v = [3.0, f32::NAN, 1.0, 2.0];
        let s = sorted(v.to_vec());
        assert_eq!(&s[..3], &[1.0, 2.0, 3.0]);
        assert!(s[3].is_nan());
        assert_eq!(percentile_of_sorted(&s, 0.25), 2.0);
        assert_eq!(percentile(&v, 0.5), 3.0);
        assert!(percentile(&v, 2.0).is_nan());
        assert_eq!(percentile(&[], 0.5), 0.0);
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Numbers and lists (spec §11): the IEEE tests, the angle difference, squares, rounding
//! (FS-12), the standard deviation from sums, the sort comparisons, the integer lists (FS-10),
//! the two selections, the median (FS-09) and the list statistics.

use std::cmp::Ordering;

use larmorx_core::defects::{FS_09, FS_10, FS_12};
use larmorx_core::x87::F80;

use crate::diag::double_to_int;
use crate::process::Process;
use crate::sse::{self, NAN32};

/// `devIsinf`: +1 for +inf, -1 for -inf, else 0 (from the bits).
pub fn dev_isinf(x: f32) -> i32 {
    match x.to_bits() {
        0x7f80_0000 => 1,
        0xff80_0000 => -1,
        _ => 0,
    }
}

/// `devIsnan`.
pub fn dev_isnan(x: f32) -> bool {
    x.to_bits() & 0x7fff_ffff > 0x7f80_0000
}

/// `devFinite`.
pub fn dev_finite(x: f32) -> bool {
    x.to_bits() & 0x7f80_0000 != 0x7f80_0000
}

/// `deltaAngle(a, b)`: `a - b` in float, brought once into [-π, π] by `2·(float)π`.
pub fn delta_angle(a: f32, b: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let two_pi = 2.0f32 * pi;
    let d = sse::sub32(a, b);
    if f64::from(d) > std::f64::consts::PI {
        sse::sub32(two_pi, d)
    } else if f64::from(d) < -std::f64::consts::PI {
        sse::sub32(-two_pi, d)
    } else {
        d
    }
}

/// `squaref`.
pub fn squaref(x: f32) -> f32 {
    sse::mul32(x, x)
}

/// `squared`.
pub fn squared(x: f64) -> f64 {
    sse::mul64(x, x)
}

impl Process {
    /// `nint(x)`: x + 0.5 (x - 0.5 below 0), truncated. FS-12 beyond the int range or NaN.
    pub fn nint(&mut self, x: f64) -> i32 {
        let y = if x < 0.0 { x - 0.5 } else { x + 0.5 };
        self.rounded(y)
    }

    /// `nint2(x)`: as [`Process::nint`] with x + 0.49999999 at or above 0.
    pub fn nint2(&mut self, x: f64) -> i32 {
        let y = if x < 0.0 { x - 0.5 } else { x + 0.49999999 };
        self.rounded(y)
    }

    fn rounded(&mut self, y: f64) -> i32 {
        let (v, differs) = double_to_int(y, self.mode.reproduce());
        if differs {
            if self.mode.reproduce() {
                self.note(
                    &FS_12,
                    "a rounding beyond the int range or of NaN gave -2147483648",
                );
            } else {
                self.note(&FS_12, "a rounding beyond the int range saturated (NaN 0)");
            }
        }
        v
    }
}

/// `sum2stddev(Σx, Σx², n)`: `sqrt((Σx² - 2·mean·Σx + n·mean·mean) / (n - 1))`, left to right in
/// double.
pub fn sum2stddev(xsum: f64, xsum2: f64, n: i32) -> f64 {
    let nf = f64::from(n);
    let mean = sse::div64(xsum, nf);
    let a = sse::mul64(sse::mul64(2.0, mean), xsum);
    let b = sse::mul64(sse::mul64(nf, mean), mean);
    let num = sse::add64(sse::sub64(xsum2, a), b);
    sse::sqrt64(sse::div64(num, f64::from(n - 1)))
}

/// `compare_ints`: -1, 0, 1.
pub fn compare_ints(a: i32, b: i32) -> i32 {
    match a.cmp(&b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// `compare_floats`: -1, 1, or 0 when equal or either is NaN.
pub fn compare_floats(a: f32, b: f32) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

impl Process {
    fn empty_int_list(&mut self) {
        if self.mode.reproduce() {
            self.note(&FS_10, "an empty list reported as one item of value 0");
        } else {
            self.note(&FS_10, "an empty list reported as no items");
        }
    }

    /// `nunqiue_int_list`: the list sorted in place, then its count of distinct values.
    pub fn nunique_int_list(&mut self, list: &mut [i32]) -> i32 {
        if list.is_empty() {
            self.empty_int_list();
            return i32::from(self.mode.reproduce());
        }
        list.sort_unstable();
        1 + list.windows(2).filter(|w| w[0] != w[1]).count() as i32
    }

    /// `unqiue_int_list`: the list sorted in place, then its distinct values in order.
    pub fn unique_int_list(&mut self, list: &mut [i32]) -> Vec<i32> {
        if list.is_empty() {
            self.empty_int_list();
            return if self.mode.reproduce() {
                vec![0]
            } else {
                Vec::new()
            };
        }
        list.sort_unstable();
        let mut u = list.to_vec();
        u.dedup();
        u
    }

    /// `most_frequent_int_list`: the list sorted in place, then the most frequent value (the
    /// smallest of a tie) and its count.
    pub fn most_frequent_int_list(&mut self, list: &mut [i32]) -> (i32, i32) {
        if list.is_empty() {
            self.empty_int_list();
            return (0, 0);
        }
        list.sort_unstable();
        let (mut best, mut best_n) = (list[0], 0);
        for run in list.chunk_by(|a, b| a == b) {
            if run.len() as i32 > best_n {
                best = run[0];
                best_n = run.len() as i32;
            }
        }
        (best, best_n)
    }
}

/// `kth_smallest(a, n, k)` (k from 1 to n; panics outside that range, where FreeSurfer reads
/// outside the list): Wirth's selection, reordering the list (§11's steps).
pub fn kth_smallest(a: &mut [f32], k: usize) -> f32 {
    let n = a.len();
    let t = k - 1;
    let (mut l, mut m) = (0isize, n as isize - 1);
    let t_i = t as isize;
    while l < m {
        let x = a[t];
        let (mut i, mut j) = (l, m);
        loop {
            while a[i as usize] < x {
                i += 1;
            }
            while x < a[j as usize] {
                j -= 1;
            }
            if i <= j {
                a.swap(i as usize, j as usize);
                i += 1;
                j -= 1;
            }
            if i > j {
                break;
            }
        }
        if j < t_i {
            l = i;
        }
        if t_i < i {
            m = j;
        }
    }
    a[t]
}

/// `quick_select(a, n, k)` (k from 1 to n; panics outside that range): the median-of-three
/// partition (§11's steps), reordering the list.
pub fn quick_select(a: &mut [f32], k: usize) -> f32 {
    let t = k - 1;
    let (mut low, mut high) = (0usize, a.len() - 1);
    loop {
        if high <= low {
            return a[t];
        }
        if high == low + 1 {
            if a[low] > a[high] {
                a.swap(low, high);
            }
            return a[t];
        }
        let middle = (low + high) / 2;
        if a[middle] > a[high] {
            a.swap(middle, high);
        }
        if a[low] > a[high] {
            a.swap(low, high);
        }
        if a[middle] > a[low] {
            a.swap(middle, low);
        }
        a.swap(middle, low + 1);
        let (mut ll, mut hh) = (low + 1, high);
        loop {
            loop {
                ll += 1;
                if a[low].partial_cmp(&a[ll]) != Some(std::cmp::Ordering::Greater) {
                    break;
                }
            }
            loop {
                hh -= 1;
                if a[hh].partial_cmp(&a[low]) != Some(std::cmp::Ordering::Greater) {
                    break;
                }
            }
            if hh < ll {
                break;
            }
            a.swap(ll, hh);
        }
        a.swap(low, hh);
        if hh <= t {
            low = ll;
        }
        if hh >= t {
            high = hh - 1;
        }
    }
}

impl Process {
    /// `median(a, n)` (reorders the list). FS-09: an empty list gives NaN in both modes, noted.
    pub fn median(&mut self, a: &mut [f32]) -> f32 {
        let n = a.len();
        if n == 0 {
            self.note(
                &FS_09,
                "the median of an empty list is NaN (FreeSurfer reads memory before the list)",
            );
            return f32::from_bits(NAN32);
        }
        if n % 2 == 1 {
            return kth_smallest(a, n.div_ceil(2));
        }
        let q1 = quick_select(a, n / 2);
        let q2 = quick_select(a, n / 2 + 1);
        sse::narrow(sse::mul64(0.5, sse::widen(sse::add32(q1, q2))))
    }
}

/// The result of `DListStats`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListStats {
    /// `[n, mean, sd, min, max]`.
    Stats([f64; 5]),
    /// n ≤ 0 or no list: the error line was written, nothing returned.
    None,
}

impl Process {
    /// `DListStats(list, n)`: n ≤ 0 or no list writes its error line on standard output; else
    /// `[n, mean, sd, min, max]` with the sums in x87 extended precision.
    pub fn dlist_stats(&mut self, list: Option<&[f64]>, n: i32) -> ListStats {
        if n <= 0 {
            self.put_out(format!("ERROR: DListStats() nlist = {n}\n").as_bytes());
            return ListStats::None;
        }
        let Some(list) = list else {
            self.put_out(b"ERROR: DListStats() dlist is NULL\n");
            return ListStats::None;
        };
        let list = &list[..(n as usize).min(list.len())];
        let (mut s, mut q) = (F80::ZERO, F80::ZERO);
        for &x in list {
            s += F80::from_f64(x);
            q += F80::from_f64(sse::mul64(x, x));
        }
        let ne = F80::from_i64(i64::from(n));
        let mean = (s / ne).to_f64();
        let var = q / ne - F80::from_f64(sse::mul64(mean, mean));
        let sd = var.sqrt().to_f64();
        let (mut lo, mut hi) = (list[0], list[0]);
        for &x in &list[1..] {
            if x < lo {
                lo = x;
            }
            if x > hi {
                hi = x;
            }
        }
        ListStats::Stats([f64::from(n), mean, sd, lo, hi])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn list_stats_example() {
        let mut p = Process::silent(crate::Environment::new(), UpstreamDefects::Fix);
        let ListStats::Stats(s) = p.dlist_stats(Some(&[1.0, 2.0, 3.0]), 3) else {
            panic!()
        };
        let bits: Vec<u64> = s.iter().map(|v| v.to_bits()).collect();
        assert_eq!(
            bits,
            [
                0x4008000000000000,
                0x4000000000000000,
                0x3fea20bd700c2c3e,
                0x3ff0000000000000,
                0x4008000000000000
            ]
        );
    }

    #[test]
    fn rounding() {
        let mut p = Process::silent(crate::Environment::new(), UpstreamDefects::Fix);
        assert_eq!(p.nint2(0.5), 0);
        assert_eq!(p.nint(-0.5), -1);
        assert_eq!(p.nint(1e10), i32::MAX);
        assert!(p.notices().contains(&FS_12));
    }
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `RobustGaussian<T>` (queue 3.2-03): the robust registration's median and median absolute
//! deviation, translated from FreeSurfer 8.2.0's `mri_robust_register/RobustGaussian.cpp` for
//! `float` and `double`:
//!
//! - [`quick_select`], [`quick_select_i`]: Numerical Recipes' selection of the `k`-th smallest
//!   (after N. Devillard's public-domain code; the `_i` form also follows each element's original
//!   position);
//! - [`kth_smallest`], [`kth_smallest_i`]: Wirth's selection (likewise);
//! - [`median`], [`median_i`]: Wirth's selection of the middle for an odd count, the mean of the
//!   two Numerical Recipes selections for an even one (each reorders its array);
//! - [`mad`]: the median absolute deviation times `d` (FreeSurfer's default `d` is 1.4826).
//!
//! Comparisons are C's (a NaN compares false), so a list with NaNs is reordered exactly as
//! FreeSurfer reorders it. **FS-1064:** the median (and the deviation) of an empty list reads
//! memory FreeSurfer does not own; both modes give NaN, noted.

use larmorx_core::defects::FS_1064;
use larmorx_freesurfer_core::globals::Globals;

use larmorx_core::itk_real::ItkReal;

use crate::real::Real;

fn empty_notice(g: &mut Globals, what: &str) {
    g.notices.add(
        &FS_1064,
        format!("RobustGaussian's {what} of an empty list: NaN (FreeSurfer reads memory it does not own)"),
    );
}

/// `RobustGaussian<T>::quick_select(arr, n, k)`: the `k`-th smallest (from 1) of `arr`,
/// reordering it. Requires `1 <= k <= arr.len()`.
pub fn quick_select<T: Real>(arr: &mut [T], k: usize) -> T {
    select_nr(arr, None, k)
}

/// `RobustGaussian<T>::quick_selectI(arr, n, k)`: [`quick_select`] and the element's original
/// position (from 0).
pub fn quick_select_i<T: Real>(arr: &mut [T], k: usize) -> (T, i32) {
    let mut pos: Vec<i32> = (0..arr.len() as i32).collect();
    let v = select_nr(arr, Some(&mut pos), k);
    (v, pos[k - 1])
}

/// Numerical Recipes' selection, the positions swapped along when given.
fn select_nr<T: Real>(arr: &mut [T], mut pos: Option<&mut Vec<i32>>, k: usize) -> T {
    let mut swap = |arr: &mut [T], a: usize, b: usize| {
        arr.swap(a, b);
        if let Some(p) = pos.as_deref_mut() {
            p.swap(a, b);
        }
    };
    let (mut low, mut high) = (0usize, arr.len() - 1);
    let median = k - 1;
    loop {
        if high <= low {
            return arr[median];
        }
        if high == low + 1 {
            if arr[low] > arr[high] {
                swap(arr, low, high);
            }
            return arr[median];
        }
        let middle = (low + high) / 2;
        if arr[middle] > arr[high] {
            swap(arr, middle, high);
        }
        if arr[low] > arr[high] {
            swap(arr, low, high);
        }
        if arr[middle] > arr[low] {
            swap(arr, middle, low);
        }
        swap(arr, middle, low + 1);
        let (mut ll, mut hh) = (low + 1, high);
        loop {
            ll += 1;
            while arr[low] > arr[ll] {
                ll += 1;
            }
            hh -= 1;
            while arr[hh] > arr[low] {
                hh -= 1;
            }
            if hh < ll {
                break;
            }
            swap(arr, ll, hh);
        }
        swap(arr, low, hh);
        if hh <= median {
            low = ll;
        }
        if hh >= median {
            high = hh - 1;
        }
    }
}

/// `RobustGaussian<T>::kth_smallest(a, n, k)`: Wirth's selection of the `k`-th smallest (from
/// 1), reordering `a`. Requires `1 <= k <= a.len()`.
pub fn kth_smallest<T: Real>(a: &mut [T], k: usize) -> T {
    select_wirth(a, None, k)
}

/// `RobustGaussian<T>::kth_smallestI(a, n, k)`: [`kth_smallest`] and the element's original
/// position (from 0).
pub fn kth_smallest_i<T: Real>(a: &mut [T], k: usize) -> (T, i32) {
    let mut pos: Vec<i32> = (0..a.len() as i32).collect();
    let v = select_wirth(a, Some(&mut pos), k);
    (v, pos[k - 1])
}

fn select_wirth<T: Real>(a: &mut [T], mut pos: Option<&mut Vec<i32>>, k: usize) -> T {
    let kk = k as isize - 1;
    let (mut l, mut m) = (0isize, a.len() as isize - 1);
    while l < m {
        let x = a[kk as usize];
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
                if let Some(p) = pos.as_deref_mut() {
                    p.swap(i as usize, j as usize);
                }
                i += 1;
                j -= 1;
            }
            if i > j {
                break;
            }
        }
        if j < kk {
            l = i;
        }
        if kk < i {
            m = j;
        }
    }
    a[kk as usize]
}

/// `0.5 * (q + q2)`: the sum in `T`, halved in `double`, back to `T`.
#[inline]
fn half_sum<T: Real>(q: T, q2: T) -> T {
    T::narrow(0.5f64.sse_mul(q.sse_add(q2).wide()))
}

/// `RobustGaussian<T>::median(t, n)`: [`kth_smallest`] of the middle for an odd count; for an
/// even one the mean of [`quick_select`] of `n / 2` and of `n / 2 + 1` (`0.5 * (q + q2)`, the sum in
/// `T`). Reorders `t`. An empty list: NaN (FS-1064).
pub fn median<T: Real>(g: &mut Globals, t: &mut [T]) -> T {
    let n = t.len();
    if n == 0 {
        empty_notice(g, "median");
        return T::narrow(f64::NAN);
    }
    if n % 2 == 1 {
        return kth_smallest(t, n.div_ceil(2));
    }
    let q = quick_select(t, n / 2);
    let q2 = quick_select(t, n / 2 + 1);
    half_sum(q, q2)
}

/// `RobustGaussian<T>::medianI(t, n)`: the median and its position, as [`median`] (the selections
/// with positions: for an even count the mean of the two positions, `0.5 * (i + i2)` of their
/// integer sum). Reorders `t`. An empty list: NaN and NaN (FS-1064).
pub fn median_i<T: Real>(g: &mut Globals, t: &mut [T]) -> (T, T) {
    let n = t.len();
    if n == 0 {
        empty_notice(g, "median");
        return (T::narrow(f64::NAN), T::narrow(f64::NAN));
    }
    if n % 2 == 1 {
        let (v, p) = kth_smallest_i(t, n.div_ceil(2));
        return (v, T::narrow(f64::from(p)));
    }
    let (q, p) = quick_select_i(t, n / 2);
    let (q2, p2) = quick_select_i(t, n / 2 + 1);
    (
        half_sum(q, q2),
        T::narrow(0.5 * f64::from(p.wrapping_add(p2))),
    )
}

/// `RobustGaussian<T>::mad(a, n, d)`: [`median`] of `a` (reordering it), then `d` times the
/// median of `|a[i] - median|` (in `T`, over the reordered `a`). An empty list: NaN (FS-1064).
pub fn mad<T: Real>(g: &mut Globals, a: &mut [T], d: T) -> T {
    let medi = median(g, a);
    let mut t: Vec<T> = a.iter().map(|&x| x.sse_sub(medi).fabs()).collect();
    let mm = median(g, &mut t);
    d.sse_mul(mm)
}

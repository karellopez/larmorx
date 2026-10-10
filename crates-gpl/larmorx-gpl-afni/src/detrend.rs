// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/thd_detrend.c.
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! Removing the mean or the linear trend of a series: `THD_const_detrend`,
//! `get_linear_trend` and `THD_linear_detrend` (`thd_detrend.c`).
//!
//! The precisions are AFNI's: the mean is a float sum divided in float; the least-squares
//! sums are double, but each `xx[i] * i` is a float product; the two coefficients are computed
//! in double and stored as float; the trend is subtracted in float.

/// `THD_const_detrend(npt, xx, &xx0)`: removes the mean (float sum, float division) and
/// returns it. With fewer than 2 points nothing happens and `None` is returned (AFNI leaves
/// `*xx0` unset).
pub fn const_detrend(xx: &mut [f32]) -> Option<f32> {
    let npt = xx.len();
    if npt < 2 {
        return None;
    }
    let mut xbar = 0.0f32;
    for &v in xx.iter() {
        xbar += v;
    }
    xbar /= npt as f32;
    for v in xx.iter_mut() {
        *v -= xbar;
    }
    Some(xbar)
}

/// `get_linear_trend(npt, xx, &f0, &f1)`: the least-squares line `f0 + f1*i`. `None` with
/// fewer than 2 points (AFNI returns without setting them).
pub fn linear_trend(xx: &[f32]) -> Option<(f32, f32)> {
    let npt = xx.len();
    if npt < 2 {
        return None;
    }
    let mut x0 = f64::from(xx[0]);
    let mut x1 = 0.0f64;
    for (ii, &v) in xx.iter().enumerate().skip(1) {
        x0 += f64::from(v);
        // `xx[ii] * ii`: int converted to float, a float product, added in double.
        x1 += f64::from(v * ii as f32);
    }
    let nd = npt as f64;
    let t1 = nd * x0;
    let t3 = 1.0 / nd;
    // `t10 = npt*npt`: an int product.
    let t10 = (npt as i32).wrapping_mul(npt as i32) as f64;
    let f0 = (2.0 / (nd + 1.0) * t3 * (2.0 * t1 - 3.0 * x1 - x0)) as f32;
    let f1 = (-6.0 / (t10 - 1.0) * t3 * (-x0 - 2.0 * x1 + t1)) as f32;
    Some((f0, f1))
}

/// `THD_linear_detrend(npt, far, &f0, &f1)`: removes the least-squares line, `far[i] -= f0 +
/// f1*i` in float, and returns `(f0, f1)`. With fewer than 3 points nothing happens and
/// `None` is returned.
pub fn linear_detrend(far: &mut [f32]) -> Option<(f32, f32)> {
    if far.len() < 3 {
        return None;
    }
    let (f0, f1) = linear_trend(far)?;
    far[0] -= f0;
    for (ii, v) in far.iter_mut().enumerate().skip(1) {
        *v -= f0 + f1 * ii as f32;
    }
    Some((f0, f1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_removed() {
        let mut x: Vec<f32> = (0..20).map(|i| 3.0 + 0.5 * i as f32).collect();
        let (f0, f1) = linear_detrend(&mut x).unwrap();
        assert!(
            (f0 - 3.0).abs() < 1e-5 && (f1 - 0.5).abs() < 1e-6,
            "{f0} {f1}"
        );
        assert!(x.iter().all(|v| v.abs() < 1e-4), "{x:?}");
    }

    #[test]
    fn the_mean_is_removed() {
        let mut x = vec![1.0f32, 2.0, 3.0, 6.0];
        assert_eq!(const_detrend(&mut x), Some(3.0));
        assert_eq!(x, [-2.0, -1.0, 0.0, 3.0]);
        assert_eq!(const_detrend(&mut [5.0]), None);
        assert_eq!(linear_detrend(&mut [1.0, 2.0]), None);
    }
}

// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy 1.18.1's `gaussian_filter1d` (order 0) and `correlate1d` in mode `reflect`
//! (`scipy/ndimage/_filters.py`, `src/ni_filters.c` `NI_Correlate1D`, `src/ni_support.c`
//! `NI_ExtendLine`; BSD-3-Clause, "Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy
//! Developers", the notice in NOTICE), as nilearn's `smooth_array` uses them.
//!
//! Each line along the axis is copied into a `float64` buffer, extended by reflection
//! (`d c b a | a b c d | d c b a`, repeated for lines shorter than the kernel), correlated with
//! the weights in `float64` (for symmetric weights `w0·x + Σ (x[-j] + x[j])·w[-j]`, the outer
//! pairs first, as SciPy's loop runs) and cast back to the array's type. SciPy's wheels compile
//! this C without fused multiply-adds (checked in both environments' `_nd_image` modules), so
//! each product and sum is rounded. The values SciPy reads in place (`output=input`) are the
//! same as out of place: every line is read before it is written.

use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::reduce::add_reduce;

/// `_gaussian_kernel1d(sigma, 0, radius)[::-1]`, the weights `gaussian_filter1d` correlates
/// with: `radius = int(truncate * sigma + 0.5)`, `phi = exp(-0.5 / sigma² * x²)` for
/// `x = -radius..=radius` (numpy's float64 `exp`, given as `exp`), divided by its sum (numpy's
/// pairwise sum), reversed.
pub fn gaussian_weights(sigma: f64, truncate: f64, exp: fn(f64) -> f64) -> Vec<f64> {
    // Python's int() truncates toward zero.
    let radius = (truncate * sigma + 0.5).trunc() as i64;
    let sigma2 = sigma * sigma;
    let c = -0.5 / sigma2;
    let phi: Vec<f64> = (-radius..=radius)
        .map(|x| exp(c.mul((x * x) as f64)))
        .collect();
    let sum = add_reduce(&phi);
    let mut w: Vec<f64> = phi.iter().map(|&p| p.div(sum)).collect();
    w.reverse();
    w
}

/// `NI_ExtendLine` in mode `NI_EXTEND_REFLECT`: `buf` holds `before` free places, the line's
/// `len` values and `after` free places.
fn extend_reflect(buf: &mut [f64], before: usize, len: usize, after: usize) {
    let first = before;
    let last = before + len;
    // before the line
    let mut size_before = before;
    let mut src = first;
    let mut dst = first as isize - 1;
    while size_before > 0 && src < last {
        buf[dst as usize] = buf[src];
        dst -= 1;
        src += 1;
        size_before -= 1;
    }
    let mut src = last as isize - 1;
    while size_before > 0 {
        buf[dst as usize] = buf[src as usize];
        dst -= 1;
        src -= 1;
        size_before -= 1;
    }
    // after the line
    let mut size_after = after;
    let mut src = last as isize - 1;
    let mut dst = last;
    while size_after > 0 && src >= first as isize {
        buf[dst] = buf[src as usize];
        dst += 1;
        src -= 1;
        size_after -= 1;
    }
    let mut src = first;
    while size_after > 0 {
        buf[dst] = buf[src];
        dst += 1;
        src += 1;
        size_after -= 1;
    }
}

/// `scipy.ndimage.correlate1d(a, weights, axis, output=a, mode="reflect", origin=0)` on a
/// Fortran-ordered array of `shape` (axis 0 fastest), in place.
pub fn correlate1d_reflect<T: NpFloat>(a: &mut [T], shape: &[usize], axis: usize, w: &[f64]) {
    let n = shape.iter().product::<usize>();
    assert_eq!(a.len(), n, "array length and shape disagree");
    if n == 0 || w.is_empty() {
        return;
    }
    let size = w.len();
    let size1 = size / 2;
    let size2 = size - size1 - 1;
    // SciPy's symmetry test (|w[c+i] - w[c-i]| > DBL_EPSILON breaks it), then antisymmetry.
    let mut symmetric = 0i32;
    if size & 1 == 1 {
        symmetric = 1;
        for i in 1..=size / 2 {
            if (w[i + size1] - w[size1 - i]).abs() > f64::EPSILON {
                symmetric = 0;
                break;
            }
        }
        if symmetric == 0 {
            symmetric = -1;
            for i in 1..=size / 2 {
                if (w[size1 + i] + w[size1 - i]).abs() > f64::EPSILON {
                    symmetric = 0;
                    break;
                }
            }
        }
    }
    let len = shape[axis];
    let stride: usize = shape[..axis].iter().product();
    let mut buf = vec![0.0f64; size1 + len + size2];
    let mut out = vec![0.0f64; len];
    let lines = n / len;
    for line in 0..lines {
        // the line's start: `line` counts the other axes in Fortran order
        let lo = line % stride;
        let hi = line / stride;
        let start = lo + hi * stride * len;
        for (i, b) in buf[size1..size1 + len].iter_mut().enumerate() {
            *b = a[start + i * stride].to_f64();
        }
        extend_reflect(&mut buf, size1, len, size2);
        for (ll, o) in out.iter_mut().enumerate() {
            let c = ll + size1; // the centre in `buf`
            let mut acc;
            if symmetric > 0 {
                acc = buf[c].mul(w[size1]);
                for j in (1..=size1).rev() {
                    acc = acc.add(buf[c - j].add(buf[c + j]).mul(w[size1 - j]));
                }
            } else if symmetric < 0 {
                acc = buf[c].mul(w[size1]);
                for j in (1..=size1).rev() {
                    acc = acc.add(buf[c - j].sub(buf[c + j]).mul(w[size1 - j]));
                }
            } else {
                acc = buf[c + size2].mul(w[size1 + size2]);
                for j in 0..size1 + size2 {
                    // jj = -size1 .. size2 - 1
                    acc = acc.add(buf[c + j - size1].mul(w[j]));
                }
            }
            *o = acc;
        }
        for (i, &v) in out.iter().enumerate() {
            a[start + i * stride] = T::from_f64(v);
        }
    }
}

/// `scipy.ndimage.gaussian_filter1d(a, sigma, axis=axis, output=a)` (order 0, mode `reflect`,
/// truncate 4) on a Fortran-ordered array, in place; `exp` is numpy's float64 `exp`.
pub fn gaussian_filter1d<T: NpFloat>(
    a: &mut [T],
    shape: &[usize],
    axis: usize,
    sigma: f64,
    exp: fn(f64) -> f64,
) {
    let w = gaussian_weights(sigma, 4.0, exp);
    correlate1d_reflect(a, shape, axis, &w);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect_extends_short_lines_by_repetition() {
        // SciPy: [a] -> a a | a | a a; [a b] with 3 before, 3 after -> b b a | a b | b a a
        let mut buf = vec![0.0; 8];
        buf[3] = 1.0;
        buf[4] = 2.0;
        extend_reflect(&mut buf, 3, 2, 3);
        assert_eq!(buf, vec![2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0]);
        let mut one = vec![0.0, 0.0, 5.0, 0.0, 0.0];
        extend_reflect(&mut one, 2, 1, 2);
        assert_eq!(one, vec![5.0; 5]);
    }

    #[test]
    fn a_constant_line_stays_constant() {
        let mut a = vec![3.0f32; 7 * 2];
        let w = gaussian_weights(0.424_660_900_144_009_5, 4.0, f64::exp);
        assert_eq!(w.len(), 5);
        correlate1d_reflect(&mut a, &[7, 2], 0, &w);
        for v in a {
            assert!((v - 3.0).abs() < 1e-6);
        }
    }
}

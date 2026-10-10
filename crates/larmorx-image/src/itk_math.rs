// SPDX-License-Identifier: Apache-2.0
//! ITK's floating-point comparisons (`itk::Math::FloatAlmostEqual`, `AlmostEquals`,
//! `itkMath.h` and `itkMathDetail.h`, ITK v5.4.5).
//!
//! ANTs and ITK use these wherever they test a float for "zero" or "equal", so the exact
//! tolerance decides which voxels an operation touches.

/// `FloatIEEE<T>::AsULP` for `f32`: the bits as a signed integer, with negative numbers
/// mapped so that adjacent floats differ by one.
fn as_ulp_f32(x: f32) -> i32 {
    let u = x.to_bits();
    if u >> 31 != 0 {
        (0x8000_0000u32.wrapping_sub(u)) as i32
    } else {
        u as i32
    }
}

fn as_ulp_f64(x: f64) -> i64 {
    let u = x.to_bits();
    if u >> 63 != 0 {
        (0x8000_0000_0000_0000u64.wrapping_sub(u)) as i64
    } else {
        u as i64
    }
}

/// `FloatAlmostEqual<float>(x1, x2, max_ulps, max_abs)`: true when `|x1 − x2| ≤ max_abs`, or
/// when both have the same sign and are at most `max_ulps` floats apart. NaN is never equal
/// to a number.
pub fn float_almost_equal_f32(x1: f32, x2: f32, max_ulps: i32, max_abs: f32) -> bool {
    if (x1 - x2).abs() <= max_abs {
        return true;
    }
    if x1.is_sign_negative() != x2.is_sign_negative() {
        return false;
    }
    let ulps = as_ulp_f32(x1).wrapping_sub(as_ulp_f32(x2));
    ulps.wrapping_abs() <= max_ulps
}

/// [`float_almost_equal_f32`] for `f64`.
pub fn float_almost_equal_f64(x1: f64, x2: f64, max_ulps: i64, max_abs: f64) -> bool {
    if (x1 - x2).abs() <= max_abs {
        return true;
    }
    if x1.is_sign_negative() != x2.is_sign_negative() {
        return false;
    }
    let ulps = as_ulp_f64(x1).wrapping_sub(as_ulp_f64(x2));
    ulps.wrapping_abs() <= max_ulps
}

/// `FloatAlmostEqual<float>` with ITK's defaults: 4 ulps, or an absolute difference of at most
/// `0.1 · FLT_EPSILON` (computed in double, stored as float).
pub fn almost_equal_f32(x1: f32, x2: f32) -> bool {
    float_almost_equal_f32(x1, x2, 4, (0.1 * f64::from(f32::EPSILON)) as f32)
}

/// `FloatAlmostEqual<double>` with ITK's defaults: 4 ulps, or `0.1 · DBL_EPSILON`.
pub fn almost_equal_f64(x1: f64, x2: f64) -> bool {
    float_almost_equal_f64(x1, x2, 4, 0.1 * f64::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_tolerance_is_absolute() {
        assert!(almost_equal_f32(0.0, 0.0));
        assert!(almost_equal_f32(-0.0, 0.0));
        assert!(almost_equal_f32(1.0e-8, 0.0));
        assert!(!almost_equal_f32(2.0e-8, 0.0));
        assert!(!almost_equal_f32(f32::NAN, 0.0));
        assert!(!almost_equal_f32(f32::INFINITY, 0.0));
    }

    #[test]
    fn ulps_count_across_the_representation() {
        let x = 1.0f32;
        let up = |v: f32, k: u32| f32::from_bits(v.to_bits() + k);
        assert!(almost_equal_f32(x, up(x, 4)));
        assert!(!almost_equal_f32(x, up(x, 5)));
        assert!(almost_equal_f32(-x, -up(x, 4)));
        assert!(!almost_equal_f32(-1.0, 1.0));
        assert!(almost_equal_f64(1.0, 1.0 + 4.0 * f64::EPSILON));
        assert!(!almost_equal_f64(1.0, 1.0 + 5.0 * f64::EPSILON));
    }
}

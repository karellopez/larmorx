// SPDX-License-Identifier: Apache-2.0
//! The two floating-point precisions an ITK pipeline is instantiated with, as a type
//! parameter.
//!
//! ANTs programs compute in `double` by default and in `float` when asked (`--float 1`): the
//! same C++ templates instantiated with `T = float`. Where a port of such code depends on `T`,
//! larmorx writes it once, generic over [`ItkReal`], and the arithmetic follows C++'s type
//! rules for each instantiation: an operation on two `T` values is done in `T`; a `double`
//! literal or operand makes it double until the result is stored back into a `T`. Values
//! that ITK keeps in `double` whatever `T` is (image geometry, fixed parameters, interpolated
//! intensities) stay `f64`.
//!
//! [`Precision`] names the choice at run time, for options and command lines.

use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::element::RealElement;
use crate::{linalg, math, vnl_svd_f32};

/// The computation type of an ITK pipeline: `double` (ANTs' default) or `float`
/// (`--float 1`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Precision {
    /// `double`.
    #[default]
    Double,
    /// `float`.
    Float,
}

impl Precision {
    /// `float64` or `float32`.
    pub fn name(self) -> &'static str {
        match self {
            Precision::Double => "float64",
            Precision::Float => "float32",
        }
    }

    /// `float64`/`double` or `float32`/`float`.
    pub fn parse(name: &str) -> Option<Precision> {
        match name {
            "float64" | "double" => Some(Precision::Double),
            "float32" | "float" => Some(Precision::Float),
            _ => None,
        }
    }
}

/// `float` or `double` as an ITK template argument: the arithmetic of `T`, conversions with
/// C++'s `static_cast`, and the few library functions whose result depends on `T`.
pub trait ItkReal:
    RealElement
    + Copy
    + Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// The [`Precision`] this type stands for.
    const PRECISION: Precision;
    const ZERO: Self;
    const ONE: Self;

    /// `static_cast<T>(x)`: round to nearest (ties to even).
    fn from_f64(x: f64) -> Self;

    /// `std::sqrt(T)`, correctly rounded (IEEE).
    fn sqrt(self) -> Self;

    /// `std::sin(T)`, correctly rounded ([`math::sin`], [`math::sinf`]).
    fn sin(self) -> Self;

    /// `std::cos(T)`, correctly rounded ([`math::cos`], [`math::cosf`]).
    fn cos(self) -> Self;

    /// `itk::Math::Floor<int64>(T)` as ITK v5.4.5 is built on x86-64 (SSE2,
    /// `itkMathDetail.h`): `round_half_to_even(2x − 0.5) >> 1`, with `2x − 0.5` computed in
    /// `T`. It is the true floor except for tiny negative values (`[−2⁻⁵⁵, 0)` in double,
    /// `[−2⁻²⁶, 0)` in float), whose floor is 0.
    fn itk_floor(self) -> i64;

    /// `itk::Math::RoundHalfIntegerUp<int64>(T)` on x86-64 (SSE2):
    /// `round_half_to_even(2x + 0.5) >> 1`, with `2x + 0.5` computed in `T` (nearest
    /// neighbour interpolation).
    fn itk_round_half_up(self) -> i64;

    /// `itk::Matrix<T, 3, 3>::GetInverse`: vnl's determinant compared with 0, then vnl's SVD
    /// inverse in `T` (`None` where ITK throws).
    fn itk_inverse3(m: &[[Self; 3]; 3]) -> Option<[[Self; 3]; 3]>;
}

/// x86-64's `cvtsd2si` / `cvtss2si` with a 64-bit destination: round to nearest (ties to
/// even), NaN and values out of range giving `i64::MIN`.
#[inline]
fn round_to_i64(r: f64) -> i64 {
    // Round to nearest, ties to even, by adding and subtracting 2^52 (exact below 2^52;
    // larger values are integers already). `f64::round_ties_even` would be a call into the C
    // library on baseline x86-64, which lacks SSE4.1's `roundsd`.
    const BIG: f64 = 4_503_599_627_370_496.0; // 2^52
    let r = if r.abs() < BIG {
        ((r.abs() + BIG) - BIG).copysign(r)
    } else {
        r
    };
    let limit = 9_223_372_036_854_775_808.0; // 2^63
    if r.is_nan() || !(-limit..limit).contains(&r) {
        i64::MIN
    } else {
        r as i64
    }
}

impl ItkReal for f64 {
    const PRECISION: Precision = Precision::Double;
    const ZERO: f64 = 0.0;
    const ONE: f64 = 1.0;

    #[inline]
    fn from_f64(x: f64) -> f64 {
        x
    }

    #[inline]
    fn sqrt(self) -> f64 {
        f64::sqrt(self)
    }

    #[inline]
    fn sin(self) -> f64 {
        math::sin(self)
    }

    #[inline]
    fn cos(self) -> f64 {
        math::cos(self)
    }

    #[inline]
    fn itk_floor(self) -> i64 {
        round_to_i64(2.0 * self - 0.5) >> 1
    }

    #[inline]
    fn itk_round_half_up(self) -> i64 {
        round_to_i64(2.0 * self + 0.5) >> 1
    }

    /// [`linalg::inverse3_itk`], which checks the determinant with its own formula; the two
    /// agree on which matrices are singular except in degenerate cases.
    fn itk_inverse3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
        linalg::inverse3_itk(m)
    }
}

impl ItkReal for f32 {
    const PRECISION: Precision = Precision::Float;
    const ZERO: f32 = 0.0;
    const ONE: f32 = 1.0;

    #[inline]
    fn from_f64(x: f64) -> f32 {
        x as f32
    }

    #[inline]
    fn sqrt(self) -> f32 {
        f32::sqrt(self)
    }

    #[inline]
    fn sin(self) -> f32 {
        math::sinf(self)
    }

    #[inline]
    fn cos(self) -> f32 {
        math::cosf(self)
    }

    #[inline]
    fn itk_floor(self) -> i64 {
        round_to_i64(f64::from(2.0 * self - 0.5)) >> 1
    }

    #[inline]
    fn itk_round_half_up(self) -> i64 {
        round_to_i64(f64::from(2.0 * self + 0.5)) >> 1
    }

    fn itk_inverse3(m: &[[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
        if vnl_svd_f32::vnl_determinant3_f32(m) == 0.0 || m.iter().flatten().any(|v| !v.is_finite())
        {
            return None;
        }
        vnl_svd_f32::vnl_inverse3_f32(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn itk_floor_and_rounding() {
        for (x, floor, round) in [
            (0.0f64, 0, 0),
            (0.5, 0, 1),
            (1.5, 1, 2),
            (-0.5, -1, 0),
            (-1.5, -2, -1),
            (2.9999, 2, 3),
            (-1e-300, 0, 0), // ITK's floor of a tiny negative value is 0
        ] {
            assert_eq!(x.itk_floor(), floor, "floor({x})");
            assert_eq!(x.itk_round_half_up(), round, "round({x})");
        }
        // float: 2x + 0.5f rounds in float, so the largest float below 0.5 rounds up.
        let below_half = f32::from_bits(0.5f32.to_bits() - 1);
        assert_eq!(below_half.itk_round_half_up(), 1);
        assert_eq!(f64::from(below_half).itk_round_half_up(), 0);
        assert_eq!((-1e-9f32).itk_floor(), 0);
        assert_eq!((-1e-6f32).itk_floor(), -1);
        assert_eq!(f32::NAN.itk_floor(), i64::MIN >> 1);
        // The rounding is round_ties_even.
        let mut x = 0x1234_5678_9abc_def0u64;
        for _ in 0..100_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let v = f64::from_bits(x);
            let exact = v.round_ties_even();
            let want = if exact.is_nan() || exact.abs() >= 9_223_372_036_854_775_808.0 {
                i64::MIN
            } else {
                exact as i64
            };
            assert_eq!(round_to_i64(v), want, "{v:e}");
            let small = (x >> 11) as f64 / 1024.0 - 4e12;
            assert_eq!(round_to_i64(small), small.round_ties_even() as i64);
            assert_eq!(
                round_to_i64(small + 0.5),
                (small + 0.5).round_ties_even() as i64
            );
        }
    }

    /// For doubles, ITK's `RoundHalfIntegerUp` equals `trunc(x + 0.5)` (what larmorx's
    /// double nearest-neighbour path computes) over the indices of an image, ties included.
    #[test]
    fn double_round_half_up_is_truncation_of_x_plus_half() {
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        for i in 0..200_000u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let base = (x % 4096) as f64 + if i % 2 == 0 { 0.5 } else { 0.0 };
            for c in [
                base,
                f64::from_bits(base.to_bits() + 1),
                f64::from_bits(base.to_bits().wrapping_sub(1)),
                (x >> 11) as f64 / (1u64 << 41) as f64 - 0.5,
            ] {
                if c >= -0.5 {
                    assert_eq!(c.itk_round_half_up(), (c + 0.5) as i64, "{c:e}");
                }
            }
        }
    }

    #[test]
    fn precision_names() {
        for p in [Precision::Double, Precision::Float] {
            assert_eq!(Precision::parse(p.name()), Some(p));
        }
        assert_eq!(Precision::parse("float"), Some(Precision::Float));
        assert_eq!(Precision::parse("half"), None);
    }
}

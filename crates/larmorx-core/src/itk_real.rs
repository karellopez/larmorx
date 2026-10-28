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
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::defects::UpstreamDefects;
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
    // Below 2^51 in magnitude (every index of an image), `r + 1.5·2^52` lies in [2^52, 2^53),
    // where the representable numbers are the integers and consecutive integers have
    // consecutive bit patterns: the addition rounds `r` to the nearest integer, ties to even
    // (1.5·2^52 is even), and the bits give it back. Two operations on the path to the index,
    // where the general method takes five (the image metrics' interpolation spends much of its
    // time there); the same integer for every such `r` (`round_to_i64_general`, tested).
    const MAGIC: f64 = 6_755_399_441_055_744.0; // 1.5 · 2^52
    const LIMIT: f64 = 2_251_799_813_685_248.0; // 2^51
    if r.abs() < LIMIT {
        return (r + MAGIC).to_bits() as i64 - MAGIC.to_bits() as i64;
    }
    round_to_i64_general(r)
}

/// [`round_to_i64`] for any value.
#[inline]
fn round_to_i64_general(r: f64) -> i64 {
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

/// The floor ITK's interpolators take of a continuous index (defect ITK-01), and how often
/// the choice changed a value.
///
/// On x86-64 `itk::Math::Floor` (and `vnl_math::floor`) is `round_half_to_even(2x − 0.5) >> 1`,
/// which gives 0 instead of −1 for negative values closer to 0 than 2⁻²⁶ (float) or 2⁻⁵⁵
/// (double), so linear interpolation along a fourth axis, the windowed-sinc kernel and
/// displacement-field lookups then mix in the next voxel with a tiny negative weight. Callers
/// compute ITK's floor as before and pass it to [`IndexFloor::pick`] with the index: with
/// [`UpstreamDefects::Fix`] (the default) the exact floor replaces it where the two differ;
/// with [`UpstreamDefects::Reproduce`] ITK's is kept.
///
/// The two floors differ only in the last bits of the values they give, and most such
/// differences vanish when the value is stored. So a caller computes each output value
/// through [`IndexFloor::checked`], which computes it again with the other floor where a
/// floor differed and counts the values that changed (atomically: one `IndexFloor` serves
/// every thread), for the notice ([`IndexFloor::notice`]).
#[derive(Debug, Default)]
pub struct IndexFloor {
    exact: bool,
    differing: AtomicUsize,
    changed: AtomicUsize,
}

impl Clone for IndexFloor {
    fn clone(&self) -> Self {
        IndexFloor {
            exact: self.exact,
            differing: AtomicUsize::new(self.differing.load(Ordering::Relaxed)),
            changed: AtomicUsize::new(self.changed.load(Ordering::Relaxed)),
        }
    }
}

impl IndexFloor {
    /// The exact floor with [`UpstreamDefects::Fix`], ITK's with [`UpstreamDefects::Reproduce`].
    pub fn new(mode: UpstreamDefects) -> Self {
        IndexFloor {
            exact: mode.fix(),
            differing: AtomicUsize::new(0),
            changed: AtomicUsize::new(0),
        }
    }

    /// ITK's floor (as computed in `R`), as ITK uses it.
    pub fn itk() -> Self {
        Self::new(UpstreamDefects::Reproduce)
    }

    /// A fresh floor of the same kind (counts at 0).
    pub fn local(&self) -> Self {
        IndexFloor {
            exact: self.exact,
            ..IndexFloor::default()
        }
    }

    /// A fresh floor of the other kind (counts at 0).
    pub fn alternative(&self) -> Self {
        IndexFloor {
            exact: !self.exact,
            ..IndexFloor::default()
        }
    }

    /// The floor of `x` to use, given ITK's floor `itk` of it.
    #[inline]
    pub fn pick(&self, itk: i64, x: f64) -> i64 {
        if !(x.is_finite() && x.abs() < 4.0e18) {
            return itk;
        }
        let exact = x.floor() as i64;
        if exact == itk {
            return itk;
        }
        self.differing.fetch_add(1, Ordering::Relaxed);
        if self.exact { exact } else { itk }
    }

    /// `value(floor)`, one output value computed with this kind of floor. Where a floor
    /// differed from ITK's while computing it, the value is computed again with the other
    /// floor, and counted as changed if `same` says the two differ.
    #[inline]
    pub fn checked<V>(&self, value: impl Fn(&IndexFloor) -> V, same: impl Fn(&V, &V) -> bool) -> V {
        let local = self.local();
        let v = value(&local);
        let differing = local.differing();
        if differing > 0 {
            self.differing.fetch_add(differing, Ordering::Relaxed);
            if !same(&v, &value(&self.alternative())) {
                self.changed.fetch_add(1, Ordering::Relaxed);
            }
        }
        v
    }

    /// Records one output value computed in parts (as [`IndexFloor::checked`] does in one):
    /// `differing` floors differed from ITK's while computing it, and the other floor
    /// `changed` it or not.
    pub fn record(&self, differing: usize, changed: bool) {
        self.differing.fetch_add(differing, Ordering::Relaxed);
        if changed {
            self.changed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// How many floors differed from ITK's so far.
    pub fn differing(&self) -> usize {
        self.differing.load(Ordering::Relaxed)
    }

    /// How many values computed through [`IndexFloor::checked`] the choice of floor changed.
    pub fn changed(&self) -> usize {
        self.changed.load(Ordering::Relaxed)
    }

    /// Adds the ITK-01 notice to `notices` if the choice of floor changed any value (`what`
    /// names the values: "output voxels").
    pub fn notice(&self, notices: &mut crate::defects::Notices, what: &str) {
        let n = self.changed();
        if n == 0 {
            return;
        }
        let detail = if self.exact {
            format!(
                "{n} {what} use the exact floor of an index just below 0, where ITK's floor (on \
                 x86-64) is 0 and changes them"
            )
        } else {
            format!(
                "{n} {what} use ITK's floor of an index just below 0, which is 0 on x86-64, as \
                 ANTs does; the exact floor changes them"
            )
        };
        notices.add(&crate::defects::ITK_01, detail);
    }
}

/// Whether two stored values are the same, NaN equal to NaN (for [`IndexFloor::checked`]).
#[inline]
pub fn same_value<V: PartialEq>(a: &V, b: &V) -> bool {
    #[allow(clippy::eq_op)]
    let both_nan = a != a && b != b;
    a == b || both_nan
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fast rounding below 2^51 gives the general method's integer: ties, values next to
    /// them, signed zeros, the edges of the fast range and random values of every magnitude.
    #[test]
    fn rounding_to_integers_takes_the_general_path_value() {
        let mut values = vec![
            0.0f64, -0.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 1e-300, -1e-300,
        ];
        for k in -40i64..40 {
            let x = k as f64 * 0.5;
            values.extend([
                x,
                f64::from_bits(x.to_bits() + 1),
                f64::from_bits(x.to_bits().wrapping_sub(1)),
            ]);
        }
        let limit = 2_251_799_813_685_248.0f64;
        for x in [
            limit,
            -limit,
            limit - 0.5,
            -(limit - 0.5),
            limit - 1.0,
            1e300,
            -1e300,
        ] {
            values.extend([
                x,
                f64::from_bits(x.to_bits() - 1),
                f64::from_bits(x.to_bits() + 1),
            ]);
        }
        values.extend([f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 9.3e18, -9.3e18]);
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..200_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mantissa = (state >> 11) as f64 / (1u64 << 53) as f64;
            let exponent = (state % 120) as i32 - 60;
            let x = (mantissa - 0.5) * 2f64.powi(exponent);
            values.extend([x, x + 0.5, (x * 2.0).round() / 2.0]);
        }
        for x in values {
            assert_eq!(round_to_i64(x), round_to_i64_general(x), "{x:e}");
        }
    }

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
        // ITK-01: the exact floor where ITK's differs, and the count.
        let fix = IndexFloor::new(UpstreamDefects::Fix);
        let x = -1e-9f32;
        assert_eq!(fix.pick(x.itk_floor(), f64::from(x)), -1);
        assert_eq!(fix.pick((-0.5f32).itk_floor(), -0.5), -1);
        assert_eq!(fix.differing(), 1);
        let itk = IndexFloor::itk();
        assert_eq!(itk.pick(x.itk_floor(), f64::from(x)), 0);
        assert_eq!(itk.differing(), 1);
        assert_eq!(itk.pick(f32::NAN.itk_floor(), f64::NAN), i64::MIN >> 1);
        // A notice only where a value changed.
        let mut notices = crate::defects::Notices::new(UpstreamDefects::Reproduce);
        itk.notice(&mut notices, "lookups");
        assert!(notices.is_empty());
        let lookup = |f: &IndexFloor| f.pick(x.itk_floor(), f64::from(x));
        assert_eq!(itk.checked(lookup, same_value), 0);
        assert_eq!(itk.checked(|f| lookup(f).max(0), same_value), 0);
        assert_eq!((itk.differing(), itk.changed()), (3, 1));
        itk.notice(&mut notices, "lookups");
        assert!(notices.contains(&crate::defects::ITK_01));
        assert_eq!(fix.checked(lookup, same_value), -1);
        assert_eq!(fix.changed(), 1);
        assert!(same_value(&f32::NAN, &f32::NAN) && !same_value(&1.0, &f64::NAN));
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

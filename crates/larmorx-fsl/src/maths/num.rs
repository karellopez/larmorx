// SPDX-License-Identifier: Apache-2.0
//! The computing types of fslmaths (`specs/fslmaths.md` §2.4, §4): signed 8-bit (`char`), its
//! unsigned form (FSL-154's correction for a uint8 image), 16-bit (`short`), 32-bit (`int`),
//! IEEE single (`float`) and double (`double`) precision, with C's arithmetic on x86-64.
//!
//! - `char` and `short` values are promoted to `int` for arithmetic and the result's low bits
//!   kept; `int` arithmetic wraps; integer division truncates toward zero (the callers refuse a
//!   division by zero and `INT_MIN / -1`, FSL-156).
//! - A double stored into an integer type is truncated toward zero; out of the type's range or
//!   NaN it takes x86-64's value (the conversion's 32-bit result `INT_MIN`, then its low bits)
//!   with `reproduce`, and is clamped (NaN as 0) by default (FSL-151): [`Num::convert`] gives
//!   both and says whether they differ, so the notice names the defect exactly where the
//!   correction changed a value.
//! - NaN values keep their sign and payload through the floating types' conversions (x86-64's
//!   `cvtsd2ss`/`cvtss2sd`), and a NaN made from numbers is the default NaN, sign set.

/// The default NaN of x86-64 in double precision (`0/0`, `inf - inf`): sign set, quiet.
pub const DEFAULT_NAN_64: u64 = 0xfff8_0000_0000_0000;
/// The default NaN in single precision.
pub const DEFAULT_NAN_32: u32 = 0xffc0_0000;

/// x86-64's default NaN, double precision.
#[inline]
pub fn default_nan() -> f64 {
    f64::from_bits(DEFAULT_NAN_64)
}

/// A NaN passed on as x86-64's arithmetic passes it: its sign and payload kept, quieted.
#[inline]
pub fn quiet(x: f64) -> f64 {
    f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
}

/// A double rounded to single precision as `cvtsd2ss` does it (a NaN keeps its sign and the
/// top bits of its payload, quieted).
#[inline]
pub fn to_f32(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        let sign = ((b >> 32) as u32) & 0x8000_0000;
        f32::from_bits(sign | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff))
    } else {
        x as f32
    }
}

/// A single-precision value widened to double as `cvtss2sd` does it.
#[inline]
pub fn to_f64(x: f32) -> f64 {
    if x.is_nan() {
        let b = u64::from(x.to_bits());
        let sign = (b & 0x8000_0000) << 32;
        f64::from_bits(sign | 0x7ff8_0000_0000_0000 | ((b & 0x003f_ffff) << 29))
    } else {
        f64::from(x)
    }
}

/// `r = a op b` with x86-64's NaN rules on every platform: a NaN operand passes on quieted (the
/// first one when both are NaN), and a NaN made from numbers (0/0, inf - inf, 0 * inf) is the
/// default NaN, sign set (AArch64's has the sign clear; compilers may also swap the operands).
#[inline]
pub fn nan32(a: f32, b: f32, r: f32) -> f32 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f32::from_bits(a.to_bits() | 0x0040_0000)
    } else if b.is_nan() {
        f32::from_bits(b.to_bits() | 0x0040_0000)
    } else {
        f32::from_bits(DEFAULT_NAN_32)
    }
}

/// [`nan32`] in double precision.
#[inline]
pub fn nan64(a: f64, b: f64, r: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        quiet(a)
    } else if b.is_nan() {
        quiet(b)
    } else {
        default_nan()
    }
}

/// `float(acc + x)`: a double added to a single-precision running sum, rounded back (x86-64's
/// NaN rules).
#[inline]
pub fn acc32(acc: f32, x: f64) -> f32 {
    let a = to_f64(acc);
    to_f32(nan64(a, x, a + x))
}

/// x86-64's conversion of a double to a 32-bit integer (`cvttsd2si`): truncation toward zero,
/// `INT_MIN` for NaN and for values outside the range.
#[inline]
pub fn cvtt(v: f64) -> i32 {
    if v > -2_147_483_649.0 && v < 2_147_483_648.0 {
        v as i32
    } else {
        i32::MIN
    }
}

/// The computing types (§2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Char,
    /// `char` for a uint8 image, unsigned (FSL-154's correction).
    UChar,
    Short,
    Int,
    Float,
    Double,
}

impl Kind {
    /// The name fslmaths gives it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Char | Kind::UChar => "char",
            Kind::Short => "short",
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Double => "double",
        }
    }

    /// Whether it is an integer type.
    pub fn is_int(self) -> bool {
        !matches!(self, Kind::Float | Kind::Double)
    }
}

/// A value of a computing type, with C's arithmetic on x86-64.
pub trait Num:
    Copy + Send + Sync + PartialOrd + PartialEq + Default + std::fmt::Debug + 'static
{
    const KIND: Kind;
    /// The type's range (integer types; the floats' finite range).
    const MIN: f64;
    const MAX: f64;
    /// The smallest positive normal value (`-thrP`'s positive voxels: 1 for the integer types).
    const MIN_POSITIVE: f64;

    fn to_f64(self) -> f64;
    /// C's conversion of a double, x86-64's values outside the range (`reproduce`).
    fn from_f64_c(v: f64) -> Self;
    /// The conversion with FSL-151's correction: clamped to the type's range, NaN as 0.
    fn from_f64_clamped(v: f64) -> Self;
    /// The same bits (NaN equal to the same NaN).
    fn same(self, o: Self) -> bool;

    fn zero() -> Self;
    fn one() -> Self;
    fn add(self, o: Self) -> Self;
    fn sub(self, o: Self) -> Self;
    fn mul(self, o: Self) -> Self;
    /// `self / o`; for the integer types `o` is never 0 (nor `INT_MIN / -1`).
    fn div(self, o: Self) -> Self;
    /// `self - o` as the robust range takes a difference of limits (§6.4): in the type's
    /// arithmetic (an `int` for `char` and `short`, wrapping for `int`), as a double; x86-64's
    /// NaN rules for the floating types (`inf - inf`: the default NaN on every platform).
    fn diff(self, o: Self) -> f64;
    fn is_nan(self) -> bool;
    /// Whether the value is NaN or infinite.
    fn non_finite(self) -> bool;
    /// The value as a double by the plain conversion (a NaN's payload as the platform passes
    /// it): for the fast paths, which recompute every NaN result by the exact rules.
    #[inline]
    fn fast_f64(self) -> f64 {
        self.to_f64()
    }

    /// The conversion of `v` in the mode (`fix`: FSL-151's correction), and whether the two
    /// modes' values differ.
    #[inline]
    fn convert(v: f64, fix: bool) -> (Self, bool) {
        if Self::KIND.is_int() {
            let t = v.trunc();
            if t >= Self::MIN && t <= Self::MAX {
                return (Self::from_f64_c(v), false);
            }
            let r = Self::from_f64_c(v);
            let c = Self::from_f64_clamped(v);
            (if fix { c } else { r }, !r.same(c))
        } else {
            (Self::from_f64_c(v), false)
        }
    }
}

macro_rules! int_num {
    ($t:ty, $kind:ident) => {
        impl Num for $t {
            const KIND: Kind = Kind::$kind;
            const MIN: f64 = <$t>::MIN as f64;
            const MAX: f64 = <$t>::MAX as f64;
            const MIN_POSITIVE: f64 = 1.0;

            #[inline]
            fn to_f64(self) -> f64 {
                self as f64
            }
            #[inline]
            fn from_f64_c(v: f64) -> Self {
                cvtt(v) as $t
            }
            #[inline]
            fn from_f64_clamped(v: f64) -> Self {
                if v.is_nan() {
                    0
                } else {
                    v.trunc().clamp(<$t as Num>::MIN, <$t as Num>::MAX) as $t
                }
            }
            #[inline]
            fn same(self, o: Self) -> bool {
                self == o
            }
            #[inline]
            fn zero() -> Self {
                0
            }
            #[inline]
            fn one() -> Self {
                1
            }
            #[inline]
            fn add(self, o: Self) -> Self {
                (i32::from(self).wrapping_add(i32::from(o))) as $t
            }
            #[inline]
            fn sub(self, o: Self) -> Self {
                (i32::from(self).wrapping_sub(i32::from(o))) as $t
            }
            #[inline]
            fn mul(self, o: Self) -> Self {
                (i32::from(self).wrapping_mul(i32::from(o))) as $t
            }
            #[inline]
            fn div(self, o: Self) -> Self {
                (i32::from(self).wrapping_div(i32::from(o))) as $t
            }
            #[inline]
            fn diff(self, o: Self) -> f64 {
                f64::from(i32::from(self).wrapping_sub(i32::from(o)))
            }
            #[inline]
            fn is_nan(self) -> bool {
                false
            }
            #[inline]
            fn non_finite(self) -> bool {
                false
            }
        }
    };
}

int_num!(i8, Char);
int_num!(u8, UChar);
int_num!(i16, Short);
int_num!(i32, Int);

impl Num for f32 {
    const KIND: Kind = Kind::Float;
    const MIN: f64 = f32::MIN as f64;
    const MAX: f64 = f32::MAX as f64;
    const MIN_POSITIVE: f64 = f32::MIN_POSITIVE as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        to_f64(self)
    }
    #[inline]
    fn fast_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn from_f64_c(v: f64) -> Self {
        to_f32(v)
    }
    #[inline]
    fn from_f64_clamped(v: f64) -> Self {
        to_f32(v)
    }
    #[inline]
    fn same(self, o: Self) -> bool {
        self.to_bits() == o.to_bits()
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn add(self, o: Self) -> Self {
        nan32(self, o, self + o)
    }
    #[inline]
    fn sub(self, o: Self) -> Self {
        nan32(self, o, self - o)
    }
    #[inline]
    fn mul(self, o: Self) -> Self {
        nan32(self, o, self * o)
    }
    #[inline]
    fn div(self, o: Self) -> Self {
        nan32(self, o, self / o)
    }
    #[inline]
    fn diff(self, o: Self) -> f64 {
        to_f64(nan32(self, o, self - o))
    }
    #[inline]
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
    #[inline]
    fn non_finite(self) -> bool {
        !self.is_finite()
    }
}

impl Num for f64 {
    const KIND: Kind = Kind::Double;
    const MIN: f64 = f64::MIN;
    const MAX: f64 = f64::MAX;
    const MIN_POSITIVE: f64 = f64::MIN_POSITIVE;

    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn from_f64_c(v: f64) -> Self {
        v
    }
    #[inline]
    fn from_f64_clamped(v: f64) -> Self {
        v
    }
    #[inline]
    fn same(self, o: Self) -> bool {
        self.to_bits() == o.to_bits()
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn add(self, o: Self) -> Self {
        nan64(self, o, self + o)
    }
    #[inline]
    fn sub(self, o: Self) -> Self {
        nan64(self, o, self - o)
    }
    #[inline]
    fn mul(self, o: Self) -> Self {
        nan64(self, o, self * o)
    }
    #[inline]
    fn div(self, o: Self) -> Self {
        nan64(self, o, self / o)
    }
    #[inline]
    fn diff(self, o: Self) -> f64 {
        nan64(self, o, self - o)
    }
    #[inline]
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
    #[inline]
    fn non_finite(self) -> bool {
        !self.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_follow_x86_64_and_fsl_151() {
        assert_eq!(i8::convert(300.0, false), (44, true));
        assert_eq!(i8::convert(300.0, true), (127, true));
        assert_eq!(u8::convert(-1.0, false), (255, true));
        assert_eq!(u8::convert(-1.0, true), (0, true));
        assert_eq!(i32::convert(f64::NAN, false), (i32::MIN, true));
        assert_eq!(i32::convert(f64::NAN, true), (0, true));
        // NaN to char: x86-64's INT_MIN has a low byte of 0, the correction's too
        assert_eq!(i8::convert(f64::NAN, true), (0, false));
        assert_eq!(i16::convert(-2.7, true), (-2, false));
        assert_eq!(i16::convert(4e9, false), (0, true));
        // char arithmetic: promoted to int, the low bits kept
        assert_eq!(100i8.add(100), -56);
        assert_eq!((-128i8).div(-1), -128);
        assert_eq!(7i8.diff(-128), 135.0);
        assert_eq!(i32::MAX.diff(-1), f64::from(i32::MIN));
    }

    #[test]
    fn nan_conversions_keep_sign_and_payload() {
        let n = f64::from_bits(DEFAULT_NAN_64);
        assert_eq!(to_f32(n).to_bits(), DEFAULT_NAN_32);
        assert_eq!(
            to_f64(f32::from_bits(DEFAULT_NAN_32)).to_bits(),
            DEFAULT_NAN_64
        );
        assert_eq!(to_f32(f64::NAN).to_bits(), 0x7fc0_0000);
    }

    #[test]
    fn differences_make_x86_64s_default_nan() {
        // inf - inf: the default NaN (sign set) in both floating types, on every platform
        assert_eq!(f32::INFINITY.diff(f32::INFINITY).to_bits(), DEFAULT_NAN_64);
        assert_eq!(f64::INFINITY.diff(f64::INFINITY).to_bits(), DEFAULT_NAN_64);
        // a NaN operand passes on with its sign and payload
        let pos = f32::from_bits(0x7fc0_0001);
        assert_eq!(pos.diff(1.0).to_bits(), to_f64(pos).to_bits());
        assert_eq!(1.0f64.diff(f64::NAN).to_bits(), f64::NAN.to_bits());
        assert_eq!(2.0f32.diff(0.5), 1.5);
    }
}

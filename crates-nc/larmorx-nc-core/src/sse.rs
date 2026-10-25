// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Written for the translation of FSL
// 6.0.7.17; it holds no FSL code.
//! The NaNs of x86-64's SSE arithmetic, on every platform.
//!
//! FSL's Linux build computes with SSE, whose NaN results the replicas must give bit for bit:
//! they are written into images and printed (`-nan` when the sign bit is set).
//!
//! - **A NaN made from numbers** (`0 * inf`, `inf - inf`, `0 / 0`, the square root of a negative
//!   number) is SSE's *default NaN*, `0xffc00000` (`0xfff8000000000000` in double): its sign bit
//!   is set, so glibc prints it `-nan`. AArch64's default NaN has the sign bit clear (`nan`), so
//!   the same Rust code gives other bits on ARM.
//! - **A NaN operand** is passed on quieted. When both operands are NaN, SSE passes on the first
//!   source operand (the destination register of `addss`, `mulss`, ...): which one that is depends
//!   on the instruction order the compiler chose, in FSL's binary as in this one, so the functions
//!   here take the order explicitly (`a op b`: `a`'s NaN).
//!
//! The functions compute in hardware and look at NaNs only when the result is one, so they cost a
//! comparison on numbers. Code that NaN or infinite voxels can reach computes with them (or
//! recomputes a result that came out NaN with them, which gives the same bits).

/// SSE's default NaN in single precision: sign set, quiet, no payload.
pub const DEFAULT_NAN_F32: f32 = f32::from_bits(0xffc0_0000);
/// SSE's default NaN in double precision.
pub const DEFAULT_NAN_F64: f64 = f64::from_bits(0xfff8_0000_0000_0000);

/// `a` quieted (the quiet bit set), as SSE passes a NaN operand on.
#[inline]
pub fn quiet_f32(a: f32) -> f32 {
    f32::from_bits(a.to_bits() | 0x0040_0000)
}

/// `a` quieted.
#[inline]
pub fn quiet_f64(a: f64) -> f64 {
    f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
}

/// The NaN SSE gives for `a op b` when the result is NaN: `a`'s, else `b`'s, else the default.
#[cold]
#[inline(never)]
pub fn nan_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        quiet_f32(a)
    } else if b.is_nan() {
        quiet_f32(b)
    } else {
        DEFAULT_NAN_F32
    }
}

/// The NaN SSE gives for `a op b` in double precision.
#[cold]
#[inline(never)]
pub fn nan_f64(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        quiet_f64(a)
    } else if b.is_nan() {
        quiet_f64(b)
    } else {
        DEFAULT_NAN_F64
    }
}

macro_rules! binary {
    ($name:ident, $t:ty, $op:tt, $nan:ident, $doc:literal) => {
        #[doc = $doc]
        #[inline(always)]
        pub fn $name(a: $t, b: $t) -> $t {
            let r = a $op b;
            if r.is_nan() { $nan(a, b) } else { r }
        }
    };
}

binary!(add_f32, f32, +, nan_f32, "`a + b` with SSE's NaN (`addss`, `a` the destination).");
binary!(sub_f32, f32, -, nan_f32, "`a - b` with SSE's NaN (`subss`).");
binary!(mul_f32, f32, *, nan_f32, "`a * b` with SSE's NaN (`mulss`, `a` the destination).");
binary!(div_f32, f32, /, nan_f32, "`a / b` with SSE's NaN (`divss`).");
binary!(add_f64, f64, +, nan_f64, "`a + b` with SSE's NaN (`addsd`, `a` the destination).");
binary!(sub_f64, f64, -, nan_f64, "`a - b` with SSE's NaN (`subsd`).");
binary!(mul_f64, f64, *, nan_f64, "`a * b` with SSE's NaN (`mulsd`, `a` the destination).");
binary!(div_f64, f64, /, nan_f64, "`a / b` with SSE's NaN (`divsd`).");

/// `sqrtf(a)` with SSE's NaN: a NaN operand passed on, the default NaN for a negative number.
#[inline(always)]
pub fn sqrt_f32(a: f32) -> f32 {
    let r = a.sqrt();
    if r.is_nan() { nan_f32(a, 0.0) } else { r }
}

/// `sqrt(a)` with SSE's NaN.
#[inline(always)]
pub fn sqrt_f64(a: f64) -> f64 {
    let r = a.sqrt();
    if r.is_nan() { nan_f64(a, 0.0) } else { r }
}

/// A NaN the computation made from numbers (`r` NaN while none of its operands was) as SSE's
/// default NaN; any other value unchanged. For results whose operands are known not to be NaN.
#[inline(always)]
pub fn made_f32(r: f32) -> f32 {
    if r.is_nan() { DEFAULT_NAN_F32 } else { r }
}

/// [`made_f32`] in double precision.
#[inline(always)]
pub fn made_f64(r: f64) -> f64 {
    if r.is_nan() { DEFAULT_NAN_F64 } else { r }
}

macro_rules! sse_number {
    ($name:ident, $t:ty, $add:ident, $sub:ident, $mul:ident, $div:ident, $sqrt:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
        pub struct $name(pub $t);

        impl $name {
            /// The square root with SSE's NaN.
            #[inline(always)]
            pub fn sqrt(self) -> Self {
                $name($sqrt(self.0))
            }
            /// `|x|` (the sign bit cleared, NaNs included, as `andpd` does).
            #[inline(always)]
            pub fn abs(self) -> Self {
                $name(self.0.abs())
            }
            /// Whether the value is NaN.
            #[inline(always)]
            pub fn is_nan(self) -> bool {
                self.0.is_nan()
            }
        }

        impl From<$t> for $name {
            #[inline(always)]
            fn from(v: $t) -> Self {
                $name(v)
            }
        }

        sse_number!(@op $name, $t, Add, add, AddAssign, add_assign, $add);
        sse_number!(@op $name, $t, Sub, sub, SubAssign, sub_assign, $sub);
        sse_number!(@op $name, $t, Mul, mul, MulAssign, mul_assign, $mul);
        sse_number!(@op $name, $t, Div, div, DivAssign, div_assign, $div);

        impl std::ops::Neg for $name {
            type Output = $name;
            /// The sign bit flipped, NaNs included (`xorpd`).
            #[inline(always)]
            fn neg(self) -> $name {
                $name(-self.0)
            }
        }
    };
    (@op $name:ident, $t:ty, $tr:ident, $f:ident, $tra:ident, $fa:ident, $op:ident) => {
        impl std::ops::$tr for $name {
            type Output = $name;
            #[inline(always)]
            fn $f(self, o: $name) -> $name {
                $name($op(self.0, o.0))
            }
        }
        impl std::ops::$tr<$t> for $name {
            type Output = $name;
            #[inline(always)]
            fn $f(self, o: $t) -> $name {
                $name($op(self.0, o))
            }
        }
        impl std::ops::$tr<$name> for $t {
            type Output = $name;
            #[inline(always)]
            fn $f(self, o: $name) -> $name {
                $name($op(self, o.0))
            }
        }
        impl std::ops::$tra for $name {
            #[inline(always)]
            fn $fa(&mut self, o: $name) {
                *self = $name($op(self.0, o.0));
            }
        }
        impl std::ops::$tra<$t> for $name {
            #[inline(always)]
            fn $fa(&mut self, o: $t) {
                *self = $name($op(self.0, o));
            }
        }
    };
}

sse_number!(
    S64,
    f64,
    add_f64,
    sub_f64,
    mul_f64,
    div_f64,
    sqrt_f64,
    "A `double` that computes as SSE does, NaNs included: `a op b` is `addsd`, `subsd`, ... with \
     `a` the first source (its NaN wins over `b`'s), and a NaN made from numbers is the default \
     NaN. Expressions keep their text: `a * b + c` is `(a * b) + c`, as Rust and C++ parse it."
);
sse_number!(
    S32,
    f32,
    add_f32,
    sub_f32,
    mul_f32,
    div_f32,
    sqrt_f32,
    "A `float` that computes as SSE does, NaNs included (see [`S64`])."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_keep_their_expressions() {
        let (a, b, c) = (S64(f64::NAN), S64(-f64::NAN), S64(2.0));
        // `a + b`: a's NaN; `b + a`: b's; `-a`: the sign flipped.
        assert_eq!((a + b).0.to_bits(), f64::NAN.to_bits());
        assert_eq!((b + a).0.to_bits(), (-f64::NAN).to_bits());
        assert_eq!((-a).0.to_bits(), (-f64::NAN).to_bits());
        assert_eq!(
            (S64(f64::INFINITY) * 0.0).0.to_bits(),
            0xfff8_0000_0000_0000
        );
        assert_eq!((c * c - 1.0).0, 3.0);
        assert_eq!((1.0 - c).0, -1.0);
        let mut x = S32(1.0);
        x += S32(f32::INFINITY);
        x -= f32::INFINITY;
        assert_eq!(x.0.to_bits(), 0xffc0_0000);
        assert_eq!(S32(-4.0).sqrt().0.to_bits(), 0xffc0_0000);
    }

    const INF: f32 = f32::INFINITY;

    /// The bits SSE gives, pinned: these fail on a platform whose hardware NaN differs if the
    /// functions ever pass it through.
    #[test]
    fn made_nans_are_sse_default() {
        assert_eq!(add_f32(INF, -INF).to_bits(), 0xffc0_0000);
        assert_eq!(sub_f32(INF, INF).to_bits(), 0xffc0_0000);
        assert_eq!(mul_f32(0.0, INF).to_bits(), 0xffc0_0000);
        assert_eq!(div_f32(0.0, 0.0).to_bits(), 0xffc0_0000);
        assert_eq!(sqrt_f32(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(made_f32(0.0 * INF).to_bits(), 0xffc0_0000);
        let inf = f64::INFINITY;
        assert_eq!(add_f64(inf, -inf).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(mul_f64(inf, 0.0).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(div_f64(inf, inf).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(sqrt_f64(-2.0).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(made_f64(inf - inf).to_bits(), 0xfff8_0000_0000_0000);
    }

    #[test]
    fn nan_operands_pass_on_the_first() {
        let p = f32::from_bits(0x7fc0_0000);
        let n = f32::from_bits(0xffc0_0000);
        let payload = f32::from_bits(0x7fc1_2345);
        let signalling = f32::from_bits(0x7f80_0001);
        assert_eq!(add_f32(p, n).to_bits(), 0x7fc0_0000);
        assert_eq!(add_f32(n, p).to_bits(), 0xffc0_0000);
        assert_eq!(mul_f32(1.0, payload).to_bits(), 0x7fc1_2345);
        assert_eq!(sub_f32(payload, n).to_bits(), 0x7fc1_2345);
        assert_eq!(div_f32(signalling, 2.0).to_bits(), 0x7fc0_0001);
        assert_eq!(sqrt_f32(n).to_bits(), 0xffc0_0000);
        let pd = f64::from_bits(0x7ff8_0000_0000_0000);
        let nd = f64::from_bits(0xfff8_0000_0000_0000);
        assert_eq!(add_f64(pd, nd).to_bits(), 0x7ff8_0000_0000_0000);
        assert_eq!(mul_f64(nd, pd).to_bits(), 0xfff8_0000_0000_0000);
        // Numbers are untouched.
        assert_eq!(add_f32(1.5, 2.25), 3.75);
        assert_eq!(made_f64(-0.0).to_bits(), (-0.0f64).to_bits());
    }
}

// SPDX-License-Identifier: Apache-2.0
//! The NaNs of x86-64's SSE arithmetic, on every platform: for the functions with glibc 2.35's
//! bits (`*_glibc`), whose NaN results must be the x86-64 build's.
//!
//! - **A NaN made from numbers** (`inf - inf`, `0 / 0`, `0 * inf`) is SSE's *default NaN*, with
//!   the sign bit set: `0xffc00000` in single precision, `0xfff8000000000000` in double. AArch64's
//!   default NaN has the sign bit clear, so plain Rust arithmetic gives other bits there.
//! - **A NaN operand** is passed on quieted (its quiet bit set, sign and payload kept). Of two NaN
//!   operands SSE passes on the first source operand (the destination register of `addss`,
//!   `subsd`, ...), so each helper takes the order explicitly: `a op b` gives `a`'s NaN.
//!
//! The arithmetic is the hardware's; NaNs are looked at only when the result is one.

/// SSE's default NaN in single precision.
pub(crate) const DEFAULT_NAN_F32: f32 = f32::from_bits(0xffc0_0000);
/// SSE's default NaN in double precision.
pub(crate) const DEFAULT_NAN_F64: f64 = f64::from_bits(0xfff8_0000_0000_0000);

/// The NaN SSE gives for `a op b` when the result is NaN: `a`'s quieted, else `b`'s, else the
/// default NaN.
#[cold]
#[inline(never)]
fn nan_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        f32::from_bits(a.to_bits() | 0x0040_0000)
    } else if b.is_nan() {
        f32::from_bits(b.to_bits() | 0x0040_0000)
    } else {
        DEFAULT_NAN_F32
    }
}

/// [`nan_f32`] in double precision.
#[cold]
#[inline(never)]
fn nan_f64(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else if b.is_nan() {
        f64::from_bits(b.to_bits() | 0x0008_0000_0000_0000)
    } else {
        DEFAULT_NAN_F64
    }
}

macro_rules! sse_op {
    ($name:ident, $t:ty, $op:tt, $nan:ident, $doc:literal) => {
        #[doc = $doc]
        #[inline(always)]
        pub(crate) fn $name(a: $t, b: $t) -> $t {
            let r = a $op b;
            if r.is_nan() { $nan(a, b) } else { r }
        }
    };
}

sse_op!(add_f32, f32, +, nan_f32, "`a + b` with SSE's NaN (`addss`, `a` the destination).");
sse_op!(sub_f32, f32, -, nan_f32, "`a - b` with SSE's NaN (`subss`).");
sse_op!(mul_f32, f32, *, nan_f32, "`a * b` with SSE's NaN (`mulss`, `a` the destination).");
sse_op!(div_f32, f32, /, nan_f32, "`a / b` with SSE's NaN (`divss`).");
sse_op!(add_f64, f64, +, nan_f64, "`a + b` with SSE's NaN (`addsd`, `a` the destination).");
sse_op!(sub_f64, f64, -, nan_f64, "`a - b` with SSE's NaN (`subsd`).");
sse_op!(mul_f64, f64, *, nan_f64, "`a * b` with SSE's NaN (`mulsd`, `a` the destination).");
sse_op!(div_f64, f64, /, nan_f64, "`a / b` with SSE's NaN (`divsd`).");

/// `__math_invalidf(x)`: `(x - x) / (x - x)` as SSE computes it: a NaN `x` quieted, the default
/// NaN for an infinity (or a finite `x`, where the quotient is `0 / 0`).
#[inline(always)]
pub(crate) fn invalid_f32(x: f32) -> f32 {
    let d = sub_f32(x, x);
    div_f32(d, d)
}

/// `__math_invalid(x)` in double precision.
#[inline(always)]
pub(crate) fn invalid_f64(x: f64) -> f64 {
    let d = sub_f64(x, x);
    div_f64(d, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_x86_64_nans() {
        let inf = f32::INFINITY;
        assert_eq!(sub_f32(inf, inf).to_bits(), 0xffc0_0000);
        assert_eq!(invalid_f32(inf).to_bits(), 0xffc0_0000);
        assert_eq!(invalid_f32(-2.0).to_bits(), 0xffc0_0000);
        assert_eq!(
            invalid_f64(f64::NEG_INFINITY).to_bits(),
            0xfff8_0000_0000_0000
        );
        let (a, b) = (f32::from_bits(0x7fa0_0001), f32::from_bits(0xffc0_0002));
        assert_eq!(add_f32(a, b).to_bits(), 0x7fe0_0001);
        assert_eq!(add_f32(b, a).to_bits(), 0xffc0_0002);
        assert_eq!(mul_f32(2.0, a).to_bits(), 0x7fe0_0001);
        assert_eq!(invalid_f32(a).to_bits(), 0x7fe0_0001);
        let (c, d) = (
            f64::from_bits(0x7ff0_0000_0000_0001),
            f64::from_bits(0xfff8_0000_0000_0005),
        );
        assert_eq!(add_f64(c, d).to_bits(), 0x7ff8_0000_0000_0001);
        assert_eq!(add_f64(d, c).to_bits(), 0xfff8_0000_0000_0005);
        assert_eq!(div_f64(1.0, d).to_bits(), d.to_bits());
        assert_eq!(mul_f64(3.0, 4.0), 12.0);
        assert_eq!(DEFAULT_NAN_F64.to_bits(), 0xfff8_0000_0000_0000);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL version 3 or later; see crates-gpl/LICENSE). Copyright 2026 Karel
// Lopez Vilaret.
//! The NaNs of FreeSurfer's official build (x86-64, SSE2 scalar arithmetic, no fused operations),
//! on every platform: larmorx's own code (the porting guide's rule "NaN bits are part of the
//! output").
//!
//! - A NaN made from numbers (`inf - inf`, `0 * inf`, `0 / 0`, the square root of a negative
//!   number) is SSE's default NaN, `0xfff8000000000000` (the sign set; AArch64's has it clear).
//! - A NaN operand is passed on quieted, its sign and payload kept; of two, the first source
//!   operand's (the destination register of `addsd`, `mulsd`, ...). Each helper takes the operands
//!   in the order the official build's instructions have them (the disassembly of `numerics.cpp`'s
//!   object, `PROVENANCE.md`): `a op b` gives `a`'s NaN.
//!
//! Plain Rust arithmetic gives the same numbers; only a NaN result is looked at again.

/// SSE's default NaN in double precision.
pub const DEFAULT_NAN: f64 = f64::from_bits(0xfff8_0000_0000_0000);

#[cold]
#[inline(never)]
fn nan(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else if b.is_nan() {
        f64::from_bits(b.to_bits() | 0x0008_0000_0000_0000)
    } else {
        DEFAULT_NAN
    }
}

macro_rules! op {
    ($name:ident, $op:tt, $insn:literal) => {
        #[doc = concat!("`a ", stringify!($op), " b` as `", $insn, "` computes it (`a` the destination).")]
        #[inline(always)]
        pub fn $name(a: f64, b: f64) -> f64 {
            let r = a $op b;
            if r.is_nan() { nan(a, b) } else { r }
        }
    };
}

op!(add, +, "addsd");
op!(sub, -, "subsd");
op!(mul, *, "mulsd");
op!(div, /, "divsd");

/// The C library's `sqrt` (glibc's is `sqrtsd`): the default NaN for a negative number, a NaN
/// passed on quieted, `-0` for `-0`.
#[inline(always)]
pub fn sqrt(x: f64) -> f64 {
    let r = x.sqrt();
    if r.is_nan() { nan(x, x) } else { r }
}

/// `x ^ 0x8000000000000000` (`xorpd` with the sign mask): the negation, exact, NaNs included.
#[inline(always)]
pub fn neg(x: f64) -> f64 {
    f64::from_bits(x.to_bits() ^ 0x8000_0000_0000_0000)
}

/// `cvttsd2si` to a 64-bit register: the truncation toward zero, or x86-64's "integer
/// indefinite" value `0x8000000000000000` for NaN, the infinities and every value outside
/// [-2⁶³, 2⁶³).
#[inline(always)]
pub fn cvttsd2si64(x: f64) -> i64 {
    if x.is_nan() || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
        i64::MIN
    } else {
        x as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nans_as_sse() {
        let p = f64::from_bits(0x7ff0_0000_0000_0001);
        let q = f64::from_bits(0xfff8_0000_0000_0005);
        assert_eq!(
            add(f64::INFINITY, f64::NEG_INFINITY).to_bits(),
            0xfff8_0000_0000_0000
        );
        assert_eq!(add(p, q).to_bits(), 0x7ff8_0000_0000_0001);
        assert_eq!(add(q, p).to_bits(), 0xfff8_0000_0000_0005);
        assert_eq!(mul(0.0, f64::INFINITY).to_bits(), DEFAULT_NAN.to_bits());
        assert_eq!(div(1.0, q).to_bits(), q.to_bits());
        assert_eq!(sqrt(-1.0).to_bits(), DEFAULT_NAN.to_bits());
        assert_eq!(sqrt(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(neg(q).to_bits(), 0x7ff8_0000_0000_0005);
        assert_eq!(cvttsd2si64(f64::NAN), i64::MIN);
        assert_eq!(cvttsd2si64(1e300), i64::MIN);
        assert_eq!(cvttsd2si64(-2.0), -2);
        assert_eq!(cvttsd2si64(-9_223_372_036_854_775_808.0), i64::MIN);
        assert_eq!(cvttsd2si64(4_294_967_296.5), 4_294_967_296);
    }
}

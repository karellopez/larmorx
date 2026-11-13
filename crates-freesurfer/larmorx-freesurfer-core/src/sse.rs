// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The NaNs of FreeSurfer's official build (x86-64, SSE2 arithmetic, no fused operations) on
//! every platform (larmorx's own code; the porting guide's rule "NaN bits are part of the
//! output"):
//!
//! - a NaN made from numbers (`inf - inf`, `0 * inf`, `0 / 0`, the square root of a negative
//!   number) is SSE's default NaN, with the sign bit set (`0xffc00000`, `0xfff8000000000000`;
//!   AArch64's has it clear);
//! - a NaN operand is passed on quieted, its sign and payload kept; of two, the first source
//!   operand's (the destination register): each helper takes the C expression's order, `a op b`
//!   giving `a`'s NaN, as GCC compiles `x += y` and `x * y - z * w` (the left operand in the
//!   destination).
//!
//! Plain Rust arithmetic gives the same numbers; only a NaN result is looked at again, so the
//! cost is one comparison per operation. The fs.maths probe checks the bits against the official
//! code (`validation/probes/fs-maths`, its NaN and infinity inputs).

/// SSE's default NaN in single precision.
pub const DEFAULT_NAN32: f32 = f32::from_bits(0xffc0_0000);
/// SSE's default NaN in double precision.
pub const DEFAULT_NAN64: f64 = f64::from_bits(0xfff8_0000_0000_0000);

#[cold]
#[inline(never)]
fn nan32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        f32::from_bits(a.to_bits() | 0x0040_0000)
    } else if b.is_nan() {
        f32::from_bits(b.to_bits() | 0x0040_0000)
    } else {
        DEFAULT_NAN32
    }
}

#[cold]
#[inline(never)]
fn nan64(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else if b.is_nan() {
        f64::from_bits(b.to_bits() | 0x0008_0000_0000_0000)
    } else {
        DEFAULT_NAN64
    }
}

macro_rules! op {
    ($name:ident, $t:ty, $op:tt, $nan:ident) => {
        #[doc = concat!("`a ", stringify!($op), " b` with SSE's NaN (`a` the destination).")]
        #[inline(always)]
        pub fn $name(a: $t, b: $t) -> $t {
            let r = a $op b;
            if r.is_nan() { $nan(a, b) } else { r }
        }
    };
}

op!(add, f32, +, nan32);
op!(sub, f32, -, nan32);
op!(mul, f32, *, nan32);
op!(div, f32, /, nan32);
op!(add64, f64, +, nan64);
op!(sub64, f64, -, nan64);
op!(mul64, f64, *, nan64);
op!(div64, f64, /, nan64);

/// `sqrtf(x)` (`sqrtss`): the default NaN for a negative `x`, `x` quieted for a NaN.
#[inline(always)]
pub fn sqrt(x: f32) -> f32 {
    let r = x.sqrt();
    if r.is_nan() { nan32(x, x) } else { r }
}

/// `sqrt(x)` (`sqrtsd`), as [`sqrt`].
#[inline(always)]
pub fn sqrt64(x: f64) -> f64 {
    let r = x.sqrt();
    if r.is_nan() { nan64(x, x) } else { r }
}

/// A `float` sum `val += x[i]` over `x` as GCC 8.4.0 vectorises it (the official build's NaN
/// operands): with four or more terms, blocks of four from the first, term `k` of a block added
/// as `val + x` where `val_first[k]` and as `x + val` elsewhere; the rest (or all of fewer than
/// four terms) one by one as `val + x`. Each function's loop has its own order (its disassembly:
/// `shufps` and `unpckhps` lanes added with `addss`).
pub fn sum_blocks(mut val: f32, x: &[f32], val_first: [bool; 4]) -> f32 {
    let blocks = if x.len() >= 4 { x.len() / 4 } else { 0 };
    for k in 0..blocks {
        for (l, &first) in val_first.iter().enumerate() {
            let v = x[4 * k + l];
            val = if first { add(val, v) } else { add(v, val) };
        }
    }
    for &v in &x[4 * blocks..] {
        val = add(val, v);
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nans_as_sse() {
        let p = f32::from_bits(0x7fc0_0001);
        let q = f32::from_bits(0xff80_0002);
        assert_eq!(add(f32::INFINITY, f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(add(p, q).to_bits(), 0x7fc0_0001);
        assert_eq!(add(q, p).to_bits(), 0xffc0_0002);
        assert_eq!(mul(1.0, q).to_bits(), 0xffc0_0002);
        assert_eq!(sqrt(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(sub64(0.0, f64::NAN).to_bits(), f64::NAN.to_bits());
        assert_eq!(div64(0.0, 0.0).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(add(1.0, 2.0), 3.0);
    }

    #[test]
    fn blocked_sums() {
        let (p, q) = (f32::from_bits(0x7fc0_0001), f32::from_bits(0xffc0_0002));
        // val + p, then q + val: q's NaN
        assert_eq!(
            sum_blocks(0.0, &[p, 1.0, q, 2.0], [true, true, false, true]).to_bits(),
            0xffc0_0002
        );
        // val first throughout: p's
        assert_eq!(
            sum_blocks(0.0, &[p, 1.0, q, 2.0], [true; 4]).to_bits(),
            0x7fc0_0001
        );
        // fewer than four: one by one, val first
        assert_eq!(sum_blocks(0.0, &[p, q], [false; 4]).to_bits(), 0x7fc0_0001);
        assert_eq!(
            sum_blocks(1.0, &[2.0, 3.0, 4.0, 5.0, 6.0], [false; 4]),
            21.0
        );
    }
}

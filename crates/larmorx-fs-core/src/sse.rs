// SPDX-License-Identifier: Apache-2.0
//! x86-64's SSE rules for NaNs, made explicit (spec §0, porting guide §9), so that every
//! platform gives the official build's bits: a NaN made from numbers is the default NaN with its
//! sign set; a NaN operand passes on quieted with its sign and payload; of two NaN operands the
//! left one of the expression as written.

/// SSE's default NaN in float.
pub const NAN32: u32 = 0xffc0_0000;
/// SSE's default NaN in double.
pub const NAN64: u64 = 0xfff8_0000_0000_0000;

#[inline]
fn pick32(r: f32, a: f32, b: f32) -> f32 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f32::from_bits(a.to_bits() | 0x0040_0000)
    } else if b.is_nan() {
        f32::from_bits(b.to_bits() | 0x0040_0000)
    } else {
        f32::from_bits(NAN32)
    }
}

#[inline]
fn pick64(r: f64, a: f64, b: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else if b.is_nan() {
        f64::from_bits(b.to_bits() | 0x0008_0000_0000_0000)
    } else {
        f64::from_bits(NAN64)
    }
}

/// `a + b` in float.
#[inline]
pub fn add32(a: f32, b: f32) -> f32 {
    pick32(a + b, a, b)
}
/// `a - b` in float.
#[inline]
pub fn sub32(a: f32, b: f32) -> f32 {
    pick32(a - b, a, b)
}
/// `a * b` in float.
#[inline]
pub fn mul32(a: f32, b: f32) -> f32 {
    pick32(a * b, a, b)
}
/// `a / b` in float.
#[inline]
pub fn div32(a: f32, b: f32) -> f32 {
    pick32(a / b, a, b)
}
/// `a + b` in double.
#[inline]
pub fn add64(a: f64, b: f64) -> f64 {
    pick64(a + b, a, b)
}
/// `a - b` in double.
#[inline]
pub fn sub64(a: f64, b: f64) -> f64 {
    pick64(a - b, a, b)
}
/// `a * b` in double.
#[inline]
pub fn mul64(a: f64, b: f64) -> f64 {
    pick64(a * b, a, b)
}
/// `a / b` in double.
#[inline]
pub fn div64(a: f64, b: f64) -> f64 {
    pick64(a / b, a, b)
}
/// `sqrt(a)` in double (`sqrtsd`: a negative number gives the default NaN).
#[inline]
pub fn sqrt64(a: f64) -> f64 {
    pick64(a.sqrt(), a, a)
}
/// A float widened to double (a NaN keeps its sign and payload, quieted).
#[inline]
pub fn widen(a: f32) -> f64 {
    if a.is_nan() {
        let b = a.to_bits();
        f64::from_bits(
            (u64::from(b >> 31) << 63) | 0x7ff8_0000_0000_0000 | (u64::from(b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(a)
    }
}
/// A double rounded to float (a NaN keeps its sign and the payload's high bits, quieted).
#[inline]
pub fn narrow(a: f64) -> f32 {
    if a.is_nan() {
        let b = a.to_bits();
        f32::from_bits(((b >> 63) as u32) << 31 | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff))
    } else {
        a as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nans() {
        assert_eq!(sub64(f64::INFINITY, f64::INFINITY).to_bits(), NAN64);
        assert_eq!(sqrt64(-1.0).to_bits(), NAN64);
        let p = f64::from_bits(0x7ff0_0000_0000_0001);
        assert_eq!(add64(p, 1.0).to_bits(), 0x7ff8_0000_0000_0001);
        let q = f64::from_bits(0xfff8_0000_0000_0002);
        assert_eq!(add64(q, p).to_bits(), 0xfff8_0000_0000_0002);
        assert_eq!(
            widen(f32::from_bits(0x7f80_0001)).to_bits(),
            0x7ff8_0000_2000_0000
        );
        assert_eq!(
            narrow(f64::from_bits(0x7ff8_0000_2000_0000)).to_bits(),
            0x7fc0_0001
        );
        assert_eq!(div32(0.0, 0.0).to_bits(), NAN32);
    }
}

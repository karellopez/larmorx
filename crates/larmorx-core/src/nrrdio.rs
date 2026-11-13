// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! NrrdIO 1.9.0's IEEE 754 constants (Teem's `754.c`, Gordon Kindlmann and the University of
//! Utah; NrrdIO's zlib-style terms): the bit patterns of its float NaNs and infinities, built
//! with `TEEM_QNANHIBIT=1` (the quiet NaN has the high mantissa bit set, as on x86 and ARM).
//! `AIR_NAN` is `airFloatQNaN`'s float.

/// `airFloatQNaN`.
pub const AIR_FLOAT_QNAN: u32 = 0x7fff_ffff;
/// `airFloatSNaN`.
pub const AIR_FLOAT_SNAN: u32 = 0x7fbf_ffff;
/// `airFloatPosInf`.
pub const AIR_FLOAT_POS_INF: u32 = 0x7f80_0000;
/// `airFloatNegInf`.
pub const AIR_FLOAT_NEG_INF: u32 = 0xff80_0000;

/// `AIR_NAN`.
pub fn air_nan() -> f32 {
    f32::from_bits(AIR_FLOAT_QNAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bits_survive() {
        assert_eq!(air_nan().to_bits(), 0x7fff_ffff);
        assert!(air_nan().is_nan());
        assert_eq!(f32::from_bits(AIR_FLOAT_POS_INF), f32::INFINITY);
        assert_eq!(f32::from_bits(AIR_FLOAT_NEG_INF), f32::NEG_INFINITY);
        assert!(f32::from_bits(AIR_FLOAT_SNAN).is_nan());
    }
}

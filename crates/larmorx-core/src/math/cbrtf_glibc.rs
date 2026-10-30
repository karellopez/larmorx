// SPDX-License-Identifier: Apache-2.0
//! The cube root for binary32 with the bits of glibc 2.35's `cbrtf` on x86-64, on every input.
//!
//! A **clean-room implementation** (Apache-2.0), written only from `specs/cbrtf.md` (the method
//! as mathematics: a quadratic start, its cube, one Halley step in double precision, and the
//! constants as bits) and black-box runs of the system's `cbrtf`; glibc's source was never
//! opened. It is not correctly rounded (about one input in ten is one unit in the last place
//! from the exact cube root: GLIBC-05); [`super::cbrtf`] is, and is larmorx's default. This one
//! serves `reproduce` where an original program's result depends on glibc's last bit
//! (3dUnifize `-EPI`'s radius, `specs/3dUnifize.md` §7).
//!
//! Every operation is one IEEE operation rounded to nearest, in the order and the format the
//! spec states, with no fused multiply-add (plain `*`, `+`, `/`; never `mul_add`), so the bits
//! are the same on every platform. Proved equal to the system's `cbrtf` (glibc 2.35, x86-64) on
//! all 2^32 inputs, NaN bits included (`validation/investigations/cleanroom-cbrtf/`).

#![allow(clippy::unreadable_literal)]

/// The quadratic start's coefficients `c0`, `c1`, `c2` (spec §5), as bits.
const C0: f64 = f64::from_bits(0x3fdf87bc378ed415);
const C1: f64 = f64::from_bits(0x3fe6527f4927f555);
const C2: f64 = f64::from_bits(0x3fc8832490c2fedd);

/// The factors standing for `2^(rho/3)`, `rho = -2 ..= 2` (spec §5), as bits: the rounded
/// quotients `1/K2`, `1/K1`, then 1, `K1`, `K2`.
const F: [f64; 5] = [
    f64::from_bits(0x3fe428a2f98d728a),
    f64::from_bits(0x3fe965fea53d6e3c),
    f64::from_bits(0x3ff0000000000000),
    f64::from_bits(0x3ff428a2f98d728b),
    f64::from_bits(0x3ff965fea53d6e3d),
];

/// The cube root of `x` for `f32` with glibc 2.35's bits (x86-64): `±0` and `±∞` as given, a
/// NaN quietened (`x + x`: its quiet bit set, sign and payload kept), every other input the
/// spec's method (`specs/cbrtf.md` §3 to §6).
#[must_use]
#[inline]
pub fn cbrtf_glibc(x: f32) -> f32 {
    let bits = x.to_bits();
    let abs = bits & 0x7fff_ffff;
    if abs >= 0x7f80_0000 {
        return x + x; // ±∞ as given; a NaN quietened
    }
    if abs == 0 {
        return x; // ±0, the sign kept
    }
    // §3: |x| = m · 2^e with m in [0.5, 1); a subnormal normalised the same way.
    let (fraction, e) = if abs < 0x0080_0000 {
        let shift = abs.leading_zeros() - 8; // the leading bit to bit 23
        (abs << shift, -125 - shift as i32)
    } else {
        (abs, (abs >> 23) as i32 - 126)
    };
    let m = f32::from_bits((fraction & 0x007f_ffff) | (126 << 23));
    let q = e / 3; // truncating toward zero
    let rho = e % 3; // the sign of e
    // §4, step 1: the quadratic start in double, rounded to float.
    let big_m = f64::from(m);
    let p = C2 * big_m;
    let d = C1 - p;
    let r = d * big_m;
    let s = C0 + r;
    let u = s as f32;
    // Step 2: its cube in float, two roundings.
    let t = (u * u) * u;
    // Step 3: one Halley step in double, then the factor, then float.
    let (uu, tt) = (f64::from(u), f64::from(t));
    let a = tt + 2.0 * big_m;
    let b = 2.0 * tt + big_m;
    let y1 = uu * a;
    let y2 = y1 / b;
    let y3 = y2 * F[(rho + 2) as usize];
    let y = y3 as f32;
    // §6: the sign and 2^q, both exact (the result is normal: q + 127 is in 78 ..= 169).
    let scale = f32::from_bits(((q + 127) as u32) << 23);
    let v = y * scale;
    if bits >> 31 == 1 { -v } else { v }
}

#[cfg(test)]
mod tests {
    use super::cbrtf_glibc;
    use crate::math::logf::tests::checksum;

    /// The spec's examples (§6, observed on glibc 2.35): input bits, result bits.
    const EXAMPLES: [(u32, u32); 18] = [
        (0x3f800000, 0x3f800000),
        (0x41000000, 0x40000000),
        (0x41d80000, 0x40400000),
        (0xc1000000, 0xc0000000),
        (0x3f000000, 0x3f4b2ff5),
        (0x3f400000, 0x3f689768),
        (0x40000000, 0x3fa14518),
        (0x40800000, 0x3fcb2ff5),
        (0x3f7fffff, 0x3f800000),
        (0x413f83e6, 0x40126666),
        (0x3a83127b, 0x3dccccd4),
        (0x3f143f57, 0x3f556188),
        (0x3dd561ac, 0x3ef0ec61),
        (0x00000001, 0x26a14518),
        (0x80000001, 0xa6a14518),
        (0x007fffff, 0x2a7fffff),
        (0x00800000, 0x2a800000),
        (0x7f7fffff, 0x54cb2ff5),
    ];

    #[test]
    fn the_spec_examples() {
        for (x, y) in EXAMPLES {
            assert_eq!(
                cbrtf_glibc(f32::from_bits(x)).to_bits(),
                y,
                "cbrtf_glibc({x:#010x})"
            );
        }
    }

    /// Where glibc's last bit differs from the correctly rounded cube root (§6's examples).
    #[test]
    fn not_correctly_rounded_where_glibc_is_not() {
        for (x, glibc, correct) in [
            (0x413f83e6u32, 0x40126666u32, 0x40126667u32),
            (0x3a83127b, 0x3dccccd4, 0x3dccccd3),
            (0x3f143f57, 0x3f556188, 0x3f556187),
            (0x3dd561ac, 0x3ef0ec61, 0x3ef0ec62),
        ] {
            let x = f32::from_bits(x);
            assert_eq!(cbrtf_glibc(x).to_bits(), glibc);
            assert_eq!(crate::math::cbrtf(x).to_bits(), correct);
        }
    }

    /// §2: zeros and infinities as given; a NaN quietened, its sign and payload kept (quiet
    /// and signalling, both signs).
    #[test]
    fn special_values_and_nan_bits() {
        for x in [0.0f32, -0.0, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(cbrtf_glibc(x).to_bits(), x.to_bits());
        }
        for nan in [
            0x7fc00000u32,
            0xffc00000,
            0x7fc00001,
            0xffd23456,
            0x7f800001, // signalling
            0xff800001,
            0x7fa00000,
            0xffbfffff,
            0x7fffffff,
        ] {
            let r = cbrtf_glibc(f32::from_bits(nan)).to_bits();
            assert_eq!(r, nan | 0x0040_0000, "{nan:#010x}");
        }
    }

    /// Odd and negative exponents (quotient and remainder toward zero), each of the five
    /// factors: exact cubes come out exact.
    #[test]
    fn exact_cubes() {
        for k in -40i32..=40 {
            for c in [1.0f32, 2.0, 3.0, 5.0] {
                let y = c * 2f32.powi(k);
                let x = y * y * y;
                if x.is_finite() && x != 0.0 && x.is_normal() {
                    assert_eq!(cbrtf_glibc(x), y, "{x:e}");
                    assert_eq!(cbrtf_glibc(-x), -y, "{x:e}");
                }
            }
        }
    }

    /// The system's `cbrtf` (glibc 2.35, x86-64), run as a black box, on every 4099th bit
    /// pattern: the spec's hash (`validation/investigations/cleanroom-cbrtf/`).
    #[test]
    fn matches_glibc_on_a_stride() {
        assert_eq!(checksum(cbrtf_glibc, 4099), STRIDE_4099);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`): the hash of glibc
    /// 2.35's whole sequence, as the black-box verifier computed it from the system's results
    /// (the spec's §7.1 hash run through all inputs; the README says why it is not §7.2's
    /// number).
    #[test]
    #[ignore]
    fn matches_glibc_everywhere() {
        assert_eq!(checksum(cbrtf_glibc, 1), 0x335d_1fb5_fb71_4c95);
    }

    /// The hash of glibc 2.35's results on every 4099th input, from the black-box verifier.
    const STRIDE_4099: u64 = 0xfcc5_fa1d_7760_6cb4;
}

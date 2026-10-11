// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded exponential for binary32 (CORE-MATH's `cr_expf`), in pure Rust.
//!
//! The result is `e^x` rounded to the nearest `f32` (ties to even) for every input, so it is
//! bit-identical on every platform. glibc 2.35's `expf` (which ITK calls through
//! `std::exp(float)`, for example in N4's Gaussian kernel and convergence measure) is not
//! correctly rounded for 170,648 of the 4,278,190,080 finite floats (0.004 %); there the two
//! differ by one unit in the last place (`docs/findings/platform-math.md`).
//!
//! Port of CORE-MATH's `src/binary32/exp/expf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2023-2025 Alexei
//! Sibidanov (the copyright and permission notice are reproduced below).
//! The algorithm, tables and operation order are kept exactly; it is re-expressed in Rust for
//! round-to-nearest only. Differences from the C file: floating-point exception flags and
//! `errno` are not modelled. Hex float literals are written as `f64` bit patterns, generated
//! from the C source, each with the original literal in a comment.

// Copyright (c) 2023-2025 Alexei Sibidanov.
//
// This file is part of the CORE-MATH project
// (https://core-math.gitlabpages.inria.fr/).
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// `c[]`: the polynomial of the accurate path.
const C: [f64; 6] = [
    f64::from_bits(0x3fe6_2e42_fefa_39ef), // 0x1.62e42fefa39efp-1
    f64::from_bits(0x3fce_bfbd_ff82_c58f), // 0x1.ebfbdff82c58fp-3
    f64::from_bits(0x3fac_6b08_d702_e0ed), // 0x1.c6b08d702e0edp-5
    f64::from_bits(0x3f83_b2ab_6fb9_2e5e), // 0x1.3b2ab6fb92e5ep-7
    f64::from_bits(0x3f55_d886_e6d5_4203), // 0x1.5d886e6d54203p-10
    f64::from_bits(0x3f24_3097_6b8c_e6ef), // 0x1.430976b8ce6efp-13
];

/// `b[]`: the polynomial of the fast path.
const B: [f64; 4] = [
    1.0,                                   // 1
    f64::from_bits(0x3fe6_2e42_fef4_c4e7), // 0x1.62e42fef4c4e7p-1
    f64::from_bits(0x3fce_bfd1_b232_f475), // 0x1.ebfd1b232f475p-3
    f64::from_bits(0x3fac_6b19_384e_cd93), // 0x1.c6b19384ecd93p-5
];

/// `tb[i]`: the bits of `2^(i/64)`.
const TB: [u64; 64] = [
    0x3ff0_0000_0000_0000,
    0x3ff0_2c9a_3e77_8061,
    0x3ff0_59b0_d315_8574,
    0x3ff0_8745_1875_9bc8,
    0x3ff0_b558_6cf9_890f,
    0x3ff0_e3ec_32d3_d1a2,
    0x3ff1_1301_d012_5b51,
    0x3ff1_429a_aea9_2de0,
    0x3ff1_72b8_3c7d_517b,
    0x3ff1_a35b_eb6f_cb75,
    0x3ff1_d487_3168_b9aa,
    0x3ff2_063b_8862_8cd6,
    0x3ff2_387a_6e75_6238,
    0x3ff2_6b45_65e2_7cdd,
    0x3ff2_9e9d_f51f_dee1,
    0x3ff2_d285_a6e4_030b,
    0x3ff3_06fe_0a31_b715,
    0x3ff3_3c08_b264_16ff,
    0x3ff3_71a7_373a_a9cb,
    0x3ff3_a7db_34e5_9ff7,
    0x3ff3_dea6_4c12_3422,
    0x3ff4_160a_21f7_2e2a,
    0x3ff4_4e08_6061_892d,
    0x3ff4_86a2_b5c1_3cd0,
    0x3ff4_bfda_d536_2a27,
    0x3ff4_f9b2_769d_2ca7,
    0x3ff5_342b_569d_4f82,
    0x3ff5_6f47_36b5_27da,
    0x3ff5_ab07_dd48_5429,
    0x3ff5_e76f_15ad_2148,
    0x3ff6_247e_b03a_5585,
    0x3ff6_6238_8255_2225,
    0x3ff6_a09e_667f_3bcd,
    0x3ff6_dfb2_3c65_1a2f,
    0x3ff7_1f75_e8ec_5f74,
    0x3ff7_5feb_5642_67c9,
    0x3ff7_a114_73eb_0187,
    0x3ff7_e2f3_36cf_4e62,
    0x3ff8_2589_994c_ce13,
    0x3ff8_68d9_9b44_92ed,
    0x3ff8_ace5_422a_a0db,
    0x3ff8_f1ae_9915_7736,
    0x3ff9_3737_b0cd_c5e5,
    0x3ff9_7d82_9fde_4e50,
    0x3ff9_c491_82a3_f090,
    0x3ffa_0c66_7b5d_e565,
    0x3ffa_5503_b23e_255d,
    0x3ffa_9e6b_5579_fdbf,
    0x3ffa_e89f_995a_d3ad,
    0x3ffb_33a2_b84f_15fb,
    0x3ffb_7f76_f2fb_5e47,
    0x3ffb_cc1e_904b_c1d2,
    0x3ffc_199b_dd85_529c,
    0x3ffc_67f1_2e57_d14b,
    0x3ffc_b720_dcef_9069,
    0x3ffd_072d_4a07_897c,
    0x3ffd_5818_dcfb_a487,
    0x3ffd_a9e6_03db_3285,
    0x3ffd_fc97_337b_9b5f,
    0x3ffe_502e_e78b_3ff6,
    0x3ffe_a4af_a2a4_90da,
    0x3ffe_fa1b_ee61_5a27,
    0x3fff_5076_5b6e_4540,
    0x3fff_a7c1_819e_90d8,
];

/// `e^x`, correctly rounded to `f32`.
#[must_use]
pub fn expf(x: f32) -> f32 {
    let iln2 = f64::from_bits(0x3ff7_1547_652b_82fe); // 0x1.71547652b82fep+0
    let big = f64::from_bits(0x42d8_0000_0000_0000); // 0x1.8p46
    let t = x.to_bits();
    let z = f64::from(x);
    let a = iln2 * z;
    let u = (a + big).to_bits();
    let ux = t << 1;
    if !(0x6f93_813e..=0x8562_e42e).contains(&ux) {
        // |x| > 0x1.62e42ep+6, or x = nan, or |x| < 0x1.93813ep-16
        if ux < 0x6f93_813e {
            return (1.0 + z * (1.0 + z * 0.5)) as f32;
        }
        if ux >= 0xff << 24 {
            if ux > 0xff << 24 {
                return x + x; // nan
            }
            return if t >> 31 == 0 { f32::INFINITY } else { 0.0 };
        }
        if t > 0xc2ce_8ec0 {
            // x < -0x1.9d1d8p+6
            let y = f64::from_bits(0x36a0_0000_0000_0000) // 0x1p-149
                + (z + f64::from_bits(0x4059_d1d9_fccf_4770)) // 0x1.9d1d9fccf477p+6
                    * f64::from_bits(0x3697_1547_652b_82ed); // 0x1.71547652b82edp-150
            let y = y.max(f64::from_bits(0x3680_0000_0000_0000)); // 0x1p-151
            return y as f32;
        }
        if t >> 31 == 0 && t > 0x42b1_7217 {
            return f32::INFINITY; // x > 0x1.62e42ep+6: overflow
        }
    }
    let ia = big - f64::from_bits(u);
    let mut h = a + ia;
    let sv = f64::from_bits(TB[(u & 0x3f) as usize].wrapping_add((u >> 6) << 52));
    let mut h2 = h * h;
    let mut r = ((B[0] + h * B[1]) + h2 * (B[2] + h * B[3])) * sv;
    let mut ub = r as f32;
    let lb = (r - r * 1.45e-10) as f32;
    if ub != lb {
        let iln2h = f64::from_bits(0x3ff7_1547_6500_0000); // 0x1.7154765p+0
        let iln2l = f64::from_bits(0x3e05_c17f_0bbb_e880); // 0x1.5c17f0bbbe88p-31
        h = (iln2h * z + ia) + iln2l * z;
        let s = sv;
        h2 = h * h;
        let w = s * h;
        r = s + w * ((C[0] + h * C[1]) + h2 * ((C[2] + h * C[3]) + h2 * (C[4] + h * C[5])));
        ub = r as f32;
    }
    ub
}

#[cfg(test)]
mod tests {
    use super::expf;
    use crate::math::logf::tests::checksum;

    #[test]
    fn special_values() {
        assert_eq!(expf(0.0), 1.0);
        assert_eq!(expf(f32::NEG_INFINITY), 0.0);
        assert_eq!(expf(f32::INFINITY), f32::INFINITY);
        assert_eq!(expf(89.0), f32::INFINITY);
        assert!(expf(f32::NAN).is_nan());
    }

    /// Correctly rounded results (CORE-MATH's C code), including two where glibc 2.35's
    /// `expf` is one ulp off and the subnormal range.
    #[test]
    fn known_values() {
        for (x, bits) in [
            (f32::from_bits(0x37ff_7f01), 0x3f80_00ff_u32), // 0x1.fefe02p-16, glibc 0x3f800100
            (f32::from_bits(0x38ad_9e29), 0x3f80_02b6),     // 0x1.5b3c52p-14, glibc 0x3f8002b7
            (-3.5, 0x3cf7_6081),
            (-87.5, 0x006c_b2bc),
            (-103.9, 0x0000_0001),
            (88.7, 0x7f7a_37fc),
        ] {
            assert_eq!(expf(x).to_bits(), bits, "expf({x:e})");
        }
    }

    /// Bit-identical to CORE-MATH's C code (gcc 11, -O2) on every 4099th bit pattern.
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(expf, 4099), 0xe26e_e38c_88f0_da64);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(expf, 1), 0xadf2_83b2_b8a0_4772);
    }
}

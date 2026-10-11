// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded natural logarithm for binary32 (CORE-MATH's `cr_logf`), in pure Rust.
//!
//! The result is `log(x)` rounded to the nearest `f32` (ties to even) for every input, so it
//! is bit-identical on every platform. glibc 2.35's `logf` (which ITK calls through
//! `std::log(float)`, for example in N4's log transform) is not correctly rounded for 416,908
//! of the 2,139,095,039 positive finite floats (0.019 %; among the integers up to 65,536, for
//! 10 of them); there the two differ by one unit in the last place
//! (`docs/findings/platform-math.md`).
//!
//! Port of CORE-MATH's `src/binary32/log/logf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2023-2026 Alexei
//! Sibidanov and Paul Zimmermann (the copyright and permission notice are reproduced below).
//! The algorithm, tables and operation order are kept exactly; it is re-expressed in Rust for
//! round-to-nearest only. Differences from the C file: floating-point exception flags and
//! `errno` are not modelled, and the NaN returned for negative arguments is the canonical
//! quiet NaN. Hex float literals are written as Rust's hexadecimal `f64` bit patterns
//! (`f64::from_bits`), each with the original literal in a comment.

// Copyright (c) 2023-2026 Alexei Sibidanov and Paul Zimmermann.
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

/// `tr[j]`: `1/(1 + j/64)`, rounded to 28 bits, for `j` = 0 to 64.
const TR: [f64; 65] = [
    f64::from_bits(0x3ff0_0000_0000_0000), // 0x1p+0
    f64::from_bits(0x3fef_81f8_2000_0000), // 0x1.f81f82p-1
    f64::from_bits(0x3fef_07c1_f000_0000), // 0x1.f07c1fp-1
    f64::from_bits(0x3fee_9131_ac00_0000), // 0x1.e9131acp-1
    f64::from_bits(0x3fee_1e1e_1e00_0000), // 0x1.e1e1e1ep-1
    f64::from_bits(0x3fed_ae60_7700_0000), // 0x1.dae6077p-1
    f64::from_bits(0x3fed_41d4_1d00_0000), // 0x1.d41d41dp-1
    f64::from_bits(0x3fec_d856_8900_0000), // 0x1.cd85689p-1
    f64::from_bits(0x3fec_71c7_1c00_0000), // 0x1.c71c71cp-1
    f64::from_bits(0x3fec_0e07_0400_0000), // 0x1.c0e0704p-1
    f64::from_bits(0x3feb_acf9_1500_0000), // 0x1.bacf915p-1
    f64::from_bits(0x3feb_4e81_b500_0000), // 0x1.b4e81b5p-1
    f64::from_bits(0x3fea_f286_bd00_0000), // 0x1.af286bdp-1
    f64::from_bits(0x3fea_98ef_6000_0000), // 0x1.a98ef6p-1
    f64::from_bits(0x3fea_41a4_1a00_0000), // 0x1.a41a41ap-1
    f64::from_bits(0x3fe9_ec8e_9500_0000), // 0x1.9ec8e95p-1
    f64::from_bits(0x3fe9_9999_9a00_0000), // 0x1.999999ap-1
    f64::from_bits(0x3fe9_48b0_fd00_0000), // 0x1.948b0fdp-1
    f64::from_bits(0x3fe8_f9c1_9000_0000), // 0x1.8f9c19p-1
    f64::from_bits(0x3fe8_acb9_0f00_0000), // 0x1.8acb90fp-1
    f64::from_bits(0x3fe8_6186_1800_0000), // 0x1.8618618p-1
    f64::from_bits(0x3fe8_1818_1800_0000), // 0x1.8181818p-1
    f64::from_bits(0x3fe7_d05f_4100_0000), // 0x1.7d05f41p-1
    f64::from_bits(0x3fe7_8a4c_8100_0000), // 0x1.78a4c81p-1
    f64::from_bits(0x3fe7_45d1_7400_0000), // 0x1.745d174p-1
    f64::from_bits(0x3fe7_02e0_5c00_0000), // 0x1.702e05cp-1
    f64::from_bits(0x3fe6_c16c_1700_0000), // 0x1.6c16c17p-1
    f64::from_bits(0x3fe6_8168_1700_0000), // 0x1.6816817p-1
    f64::from_bits(0x3fe6_42c8_5900_0000), // 0x1.642c859p-1
    f64::from_bits(0x3fe6_0581_6000_0000), // 0x1.605816p-1
    f64::from_bits(0x3fe5_c988_2c00_0000), // 0x1.5c9882cp-1
    f64::from_bits(0x3fe5_8ed2_3100_0000), // 0x1.58ed231p-1
    f64::from_bits(0x3fe5_5555_5500_0000), // 0x1.5555555p-1
    f64::from_bits(0x3fe5_1d07_eb00_0000), // 0x1.51d07ebp-1
    f64::from_bits(0x3fe4_e5e0_a700_0000), // 0x1.4e5e0a7p-1
    f64::from_bits(0x3fe4_afd6_a000_0000), // 0x1.4afd6ap-1
    f64::from_bits(0x3fe4_7ae1_4800_0000), // 0x1.47ae148p-1
    f64::from_bits(0x3fe4_46f8_6500_0000), // 0x1.446f865p-1
    f64::from_bits(0x3fe4_1414_1400_0000), // 0x1.4141414p-1
    f64::from_bits(0x3fe3_e22c_bd00_0000), // 0x1.3e22cbdp-1
    f64::from_bits(0x3fe3_b13b_1400_0000), // 0x1.3b13b14p-1
    f64::from_bits(0x3fe3_8138_1400_0000), // 0x1.3813814p-1
    f64::from_bits(0x3fe3_521c_fb00_0000), // 0x1.3521cfbp-1
    f64::from_bits(0x3fe3_23e3_4a00_0000), // 0x1.323e34ap-1
    f64::from_bits(0x3fe2_f684_be00_0000), // 0x1.2f684bep-1
    f64::from_bits(0x3fe2_c9fb_4e00_0000), // 0x1.2c9fb4ep-1
    f64::from_bits(0x3fe2_9e41_2a00_0000), // 0x1.29e412ap-1
    f64::from_bits(0x3fe2_7350_b900_0000), // 0x1.27350b9p-1
    f64::from_bits(0x3fe2_4924_9200_0000), // 0x1.2492492p-1
    f64::from_bits(0x3fe2_1fb7_8100_0000), // 0x1.21fb781p-1
    f64::from_bits(0x3fe1_f704_7e00_0000), // 0x1.1f7047ep-1
    f64::from_bits(0x3fe1_cf06_ae00_0000), // 0x1.1cf06aep-1
    f64::from_bits(0x3fe1_a7b9_6100_0000), // 0x1.1a7b961p-1
    f64::from_bits(0x3fe1_8118_1200_0000), // 0x1.1811812p-1
    f64::from_bits(0x3fe1_5b1e_5f00_0000), // 0x1.15b1e5fp-1
    f64::from_bits(0x3fe1_35c8_1100_0000), // 0x1.135c811p-1
    f64::from_bits(0x3fe1_1111_1100_0000), // 0x1.1111111p-1
    f64::from_bits(0x3fe0_ecf5_6c00_0000), // 0x1.0ecf56cp-1
    f64::from_bits(0x3fe0_c971_5000_0000), // 0x1.0c9715p-1
    f64::from_bits(0x3fe0_a681_0a00_0000), // 0x1.0a6810ap-1
    f64::from_bits(0x3fe0_8421_0800_0000), // 0x1.0842108p-1
    f64::from_bits(0x3fe0_624d_d300_0000), // 0x1.0624dd3p-1
    f64::from_bits(0x3fe0_4104_1000_0000), // 0x1.041041p-1
    f64::from_bits(0x3fe0_2040_8100_0000), // 0x1.0204081p-1
    0.5,                                   // 0.5
];

/// `tl[j]`: `-log(tr[j])`.
const TL: [f64; 65] = [
    f64::from_bits(0xbd23_b408_15cd_0628), // -0x1.3b40815cd0628p-45
    f64::from_bits(0x3f8f_c0a8_90fb_b514), // 0x1.fc0a890fbb514p-7
    f64::from_bits(0x3f9f_829b_1e78_0b98), // 0x1.f829b1e780b98p-6
    f64::from_bits(0x3fa7_7458_f532_c948), // 0x1.77458f532c948p-5
    f64::from_bits(0x3faf_0a30_c211_4ef2), // 0x1.f0a30c2114ef2p-5
    f64::from_bits(0x3fb3_41d7_93bb_c7f7), // 0x1.341d793bbc7f7p-4
    f64::from_bits(0x3fb6_f0d2_8d25_6172), // 0x1.6f0d28d256172p-4
    f64::from_bits(0x3fba_926d_3a6a_cb89), // 0x1.a926d3a6acb89p-4
    f64::from_bits(0x3fbe_2707_722a_e90c), // 0x1.e2707722ae90cp-4
    f64::from_bits(0x3fc0_d77e_7a90_896c), // 0x1.0d77e7a90896cp-3
    f64::from_bits(0x3fc2_9552_f6ff_f036), // 0x1.29552f6fff036p-3
    f64::from_bits(0x3fc4_4d2b_6c5b_7831), // 0x1.44d2b6c5b7831p-3
    f64::from_bits(0x3fc5_ff30_6ee7_8ee7), // 0x1.5ff306ee78ee7p-3
    f64::from_bits(0x3fc7_ab89_0410_d41c), // 0x1.7ab890410d41cp-3
    f64::from_bits(0x3fc9_525a_9e34_51c7), // 0x1.9525a9e3451c7p-3
    f64::from_bits(0x3fca_f3c9_4ed0_bb06), // 0x1.af3c94ed0bb06p-3
    f64::from_bits(0x3fcc_8ff7_c59a_9535), // 0x1.c8ff7c59a9535p-3
    f64::from_bits(0x3fce_2707_6d5a_edf9), // 0x1.e27076d5aedf9p-3
    f64::from_bits(0x3fcf_b918_6b5e_393e), // 0x1.fb9186b5e393ep-3
    f64::from_bits(0x3fd0_a324_e38b_8e6d), // 0x1.0a324e38b8e6dp-2
    f64::from_bits(0x3fd1_675c_acab_a398), // 0x1.1675cacaba398p-2
    f64::from_bits(0x3fd2_2941_fc0f_76ef), // 0x1.22941fc0f76efp-2
    f64::from_bits(0x3fd2_e8e2_bc31_1aba), // 0x1.2e8e2bc311abap-2
    f64::from_bits(0x3fd3_a64c_56b1_4373), // 0x1.3a64c56b14373p-2
    f64::from_bits(0x3fd4_618b_c31c_5c4c), // 0x1.4618bc31c5c4cp-2
    f64::from_bits(0x3fd5_1aad_874d_f5b7), // 0x1.51aad874df5b7p-2
    f64::from_bits(0x3fd5_d1bd_bea8_0754), // 0x1.5d1bdbea80754p-2
    f64::from_bits(0x3fd6_86c8_1d33_1238), // 0x1.686c81d331238p-2
    f64::from_bits(0x3fd7_39d7_f6db_cd90), // 0x1.739d7f6dbcd9p-2
    f64::from_bits(0x3fd7_eaf8_3c82_ad4d), // 0x1.7eaf83c82ad4dp-2
    f64::from_bits(0x3fd8_9a33_8581_3fe4), // 0x1.89a3385813fe4p-2
    f64::from_bits(0x3fd9_4794_1aa9_1484), // 0x1.947941aa91484p-2
    f64::from_bits(0x3fd9_f323_edbf_95d5), // 0x1.9f323edbf95d5p-2
    f64::from_bits(0x3fda_9cec_9a42_05d3), // 0x1.a9cec9a4205d3p-2
    f64::from_bits(0x3fdb_44f7_7c5c_8cec), // 0x1.b44f77c5c8cecp-2
    f64::from_bits(0x3fdb_eb4d_9ea7_1905), // 0x1.beb4d9ea71905p-2
    f64::from_bits(0x3fdc_8ff7_c69a_97ab), // 0x1.c8ff7c69a97abp-2
    f64::from_bits(0x3fdd_32fe_7f38_e95f), // 0x1.d32fe7f38e95fp-2
    f64::from_bits(0x3fdd_d46a_0501_c22a), // 0x1.dd46a0501c22ap-2
    f64::from_bits(0x3fde_7442_617e_8511), // 0x1.e7442617e8511p-2
    f64::from_bits(0x3fdf_128f_5eaf_0476), // 0x1.f128f5eaf0476p-2
    f64::from_bits(0x3fdf_af58_8dd8_f0a8), // 0x1.faf588dd8f0a8p-2
    f64::from_bits(0x3fe0_2552_a5ed_cfc4), // 0x1.02552a5edcfc4p-1
    f64::from_bits(0x3fe0_723e_5c64_de05), // 0x1.0723e5c64de05p-1
    f64::from_bits(0x3fe0_be72_e385_2947), // 0x1.0be72e3852947p-1
    f64::from_bits(0x3fe1_09f3_9d55_4b5c), // 0x1.109f39d554b5cp-1
    f64::from_bits(0x3fe1_54c3_d2c4_d4ae), // 0x1.154c3d2c4d4aep-1
    f64::from_bits(0x3fe1_9ee6_b38b_c834), // 0x1.19ee6b38bc834p-1
    f64::from_bits(0x3fe1_e85f_5ef0_3f95), // 0x1.1e85f5ef03f95p-1
    f64::from_bits(0x3fe2_3130_d7fa_be07), // 0x1.23130d7fabe07p-1
    f64::from_bits(0x3fe2_795e_1219_afe0), // 0x1.2795e1219afep-1
    f64::from_bits(0x3fe2_c0e9_ec9c_8d50), // 0x1.2c0e9ec9c8d5p-1
    f64::from_bits(0x3fe3_07d7_337f_0f83), // 0x1.307d7337f0f83p-1
    f64::from_bits(0x3fe3_4e28_9cb4_e098), // 0x1.34e289cb4e098p-1
    f64::from_bits(0x3fe3_93e0_d42e_28de), // 0x1.393e0d42e28dep-1
    f64::from_bits(0x3fe3_d902_6ad5_55bf), // 0x1.3d9026ad555bfp-1
    f64::from_bits(0x3fe4_1d8f_e866_7173), // 0x1.41d8fe8667173p-1
    f64::from_bits(0x3fe4_618b_c1ec_5d87), // 0x1.4618bc1ec5d87p-1
    f64::from_bits(0x3fe4_a4f8_5d30_3d80), // 0x1.4a4f85d303d8p-1
    f64::from_bits(0x3fe4_e7d8_127f_5a75), // 0x1.4e7d8127f5a75p-1
    f64::from_bits(0x3fe5_2a2d_26db_c470), // 0x1.52a2d26dbc47p-1
    f64::from_bits(0x3fe5_6bf9_d597_f25e), // 0x1.56bf9d597f25ep-1
    f64::from_bits(0x3fe5_ad40_4cb5_9df2), // 0x1.5ad404cb59df2p-1
    f64::from_bits(0x3fe5_ee02_a928_153a), // 0x1.5ee02a928153ap-1
    f64::from_bits(0x3fe6_2e42_fefa_38b4), // 0x1.62e42fefa38b4p-1
];

/// `b[]`: the polynomial of the fast path.
const B: [f64; 3] = [
    f64::from_bits(0x3ff0_0000_0063_42ea), // 0x1.00000006342eap+0
    f64::from_bits(0xbfe0_001f_7fdc_3977), // -0x1.0001f7fdc3977p-1
    f64::from_bits(0x3fd5_54a4_e5ca_e9cf), // 0x1.554a4e5cae9cfp-2
];

/// `c[]`: the polynomial of the accurate path.
const C: [f64; 7] = [
    f64::from_bits(0xbfe0_0000_0000_0000), // -0x1p-1
    f64::from_bits(0x3fd5_5555_5555_71ca), // 0x1.55555555571cap-2
    f64::from_bits(0xbfd0_0000_0000_2d85), // -0x1.0000000002d85p-2
    f64::from_bits(0x3fc9_9999_87d0_c963), // 0x1.9999987d0c963p-3
    f64::from_bits(0xbfc5_5555_4059_a8bb), // -0x1.555554059a8bbp-3
    f64::from_bits(0x3fc2_4aeb_cf71_a38f), // 0x1.24aebcf71a38fp-3
    f64::from_bits(0xbfc0_01c7_3915_d758), // -0x1.001c73915d758p-3
];

const LN2: f64 = f64::from_bits(0x3fe6_2e42_fefa_39ef); // 0x1.62e42fefa39efp-1
const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fefa_3a00); // 0x1.62e42fefa3ap-1
const LN2_LO: f64 = f64::from_bits(0x3ce0_ca86_c389_8d00); // 0x1.0ca86c3898dp-49

/// `as_special`: ±0 (−inf), +inf, NaN, and negative numbers (NaN).
#[cold]
fn as_special(x: f32) -> f32 {
    let ux = x.to_bits();
    let ax = ux << 1;
    if ax == 0 {
        return f32::NEG_INFINITY;
    }
    if ux == 0x7f80_0000 {
        return x;
    }
    if ax > 0xff00_0000 {
        return x + x;
    }
    f32::NAN
}

/// The natural logarithm of `x`, correctly rounded to `f32`.
#[must_use]
pub fn logf(x: f32) -> f32 {
    let mut ux = x.to_bits();
    if !((1 << 23)..0x7f80_0000).contains(&ux) {
        if ux == 0 || ux >= 0x7f80_0000 {
            return as_special(x); // <= 0, nan, inf
        }
        // Subnormal: normalise (the exponent field may wrap below zero, as in the C code).
        let n = ux.leading_zeros() - 8;
        ux <<= n;
        ux = ux.wrapping_sub(n << 23);
    }
    if ux == 127 << 23 {
        return 0.0;
    }
    let m = ux & ((1 << 23) - 1);
    let j = ((m + (1 << (23 - 7))) >> (23 - 6)) as usize;
    let e = ((ux as i32) >> 23) - 127;
    let tz = f64::from_bits((u64::from(m) | (1023u64 << 23)) << (52 - 23));
    let z = tz * TR[j] - 1.0;
    let z2 = z * z;
    let mut r = ((f64::from(e) * LN2 + TL[j]) + z * B[0]) + z2 * (B[1] + z * B[2]);
    let mut ub = r as f32;
    let lb = (r + f64::from_bits(0x3def_0600_0000_0000)) as f32; // r + 0x1.f06p-33
    if ub != lb {
        let mut f = z2
            * ((C[0] + z * C[1]) + z2 * ((C[2] + z * C[3]) + z2 * (C[4] + z * C[5] + z2 * C[6])));
        if (x - 1.0).abs() < f32::from_bits(0x3a80_0000) {
            // |x - 1| < 0x1p-10
            return (z + f) as f32;
        }
        f -= LN2_LO * f64::from(e);
        f += z;
        f += TL[j] - TL[0];
        let el = f64::from(e) * LN2_HI;
        r = el + f;
        ub = r as f32;
        if r.to_bits() & ((1u64 << 28) - 1) == 0 {
            let dr = (el - r) + f;
            r += dr * 64.0;
            ub = r as f32;
        }
    }
    ub
}

#[cfg(test)]
pub(crate) mod tests {
    use super::logf;

    /// FNV-1a-like checksum of the results' bits over every `stride`-th `f32` bit pattern
    /// (NaN results canonical), the same as `docs/findings/platform-math.md`'s C harness.
    pub(crate) fn checksum(f: impl Fn(f32) -> f32, stride: u64) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut i = 0u64;
        while i < 1 << 32 {
            let mut b = f(f32::from_bits(i as u32)).to_bits();
            if b & 0x7fff_ffff > 0x7f80_0000 {
                b = 0x7fc0_0000;
            }
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            i += stride;
        }
        h
    }

    #[test]
    fn special_values() {
        assert_eq!(logf(1.0), 0.0);
        assert_eq!(logf(0.0), f32::NEG_INFINITY);
        assert_eq!(logf(-0.0), f32::NEG_INFINITY);
        assert_eq!(logf(f32::INFINITY), f32::INFINITY);
        assert!(logf(-1.0).is_nan());
        assert!(logf(f32::NAN).is_nan());
    }

    /// Integers where glibc 2.35's `logf` is one ulp off: the correctly rounded results
    /// (CORE-MATH's C code).
    #[test]
    fn integers_glibc_misrounds() {
        for (x, bits) in [
            (1579.0f32, 0x40eb_aa5f_u32),
            (3100.0, 0x4100_a063),
            (4935.0, 0x4108_10d3),
            (12014.0, 0x4116_4d1f),
            (18627.0, 0x411d_5160),
            (64341.0, 0x4131_26b8),
        ] {
            assert_eq!(logf(x).to_bits(), bits, "logf({x})");
        }
    }

    /// Bit-identical to CORE-MATH's C code (gcc 11, -O2) on every 4099th bit pattern.
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(logf, 4099), 0x7c89_59df_3a6a_7a51);
    }

    /// Every one of the 2^32 inputs (about 30 s; `cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(logf, 1), 0xd5f1_51cb_ded3_238d);
    }
}

// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded base-10 logarithm for binary32 (CORE-MATH's `cr_log10f`), in pure Rust.
//!
//! glibc 2.35's `log10f` (fdlibm's; `log10f_glibc` has its bits, for `reproduce`) is not
//! correctly rounded on some arguments; this one is, on every input, so it gives the same bits
//! on every platform.
//!
//! Port of CORE-MATH's `src/binary32/log10/log10f.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2022-2026 Alexei
//! Sibidanov (the copyright and permission notice are reproduced below). The operations and
//! their order are kept exactly (the C has no fused multiply-adds); hex-float constants are
//! written as their exact bit patterns, each with the original literal in a comment. Errno and
//! the exception flags are not modelled. A zero gives -inf, a negative argument (-0 excepted)
//! the negative quiet NaN `0xffc00000` the C function returns, a NaN argument itself quieted.
//! Checked against the C function (compiled without fused multiply-adds) on every one of the
//! 2^32 floats (`validation/investigations/coremath-log10f/`).

// Copyright (c) 2022-2026 Alexei Sibidanov.
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

#![allow(clippy::unreadable_literal)]

const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

// reciprocal of 1+i/64 (i=0..64) rounded to 29 bits: TR
// logarithms of the reciprocals with offset: TL
// 10^n, as float bit patterns, indexed by bits 24..27 of the argument: ST
const TR: [f64; 65] = [
    fb(0x3ff0000000000000), // 0x1p+0
    fb(0x3fef81f820000000), // 0x1.f81f82p-1
    fb(0x3fef07c1f0000000), // 0x1.f07c1fp-1
    fb(0x3fee9131ac000000), // 0x1.e9131acp-1
    fb(0x3fee1e1e1e000000), // 0x1.e1e1e1ep-1
    fb(0x3fedae6077000000), // 0x1.dae6077p-1
    fb(0x3fed41d41d000000), // 0x1.d41d41dp-1
    fb(0x3fecd85689000000), // 0x1.cd85689p-1
    fb(0x3fec71c71c000000), // 0x1.c71c71cp-1
    fb(0x3fec0e0704000000), // 0x1.c0e0704p-1
    fb(0x3febacf915000000), // 0x1.bacf915p-1
    fb(0x3feb4e81b5000000), // 0x1.b4e81b5p-1
    fb(0x3feaf286bd000000), // 0x1.af286bdp-1
    fb(0x3fea98ef60000000), // 0x1.a98ef6p-1
    fb(0x3fea41a41a000000), // 0x1.a41a41ap-1
    fb(0x3fe9ec8e95000000), // 0x1.9ec8e95p-1
    fb(0x3fe999999a000000), // 0x1.999999ap-1
    fb(0x3fe948b0fd000000), // 0x1.948b0fdp-1
    fb(0x3fe8f9c190000000), // 0x1.8f9c19p-1
    fb(0x3fe8acb90f000000), // 0x1.8acb90fp-1
    fb(0x3fe8618618000000), // 0x1.8618618p-1
    fb(0x3fe8181818000000), // 0x1.8181818p-1
    fb(0x3fe7d05f41000000), // 0x1.7d05f41p-1
    fb(0x3fe78a4c81000000), // 0x1.78a4c81p-1
    fb(0x3fe745d174000000), // 0x1.745d174p-1
    fb(0x3fe702e05c000000), // 0x1.702e05cp-1
    fb(0x3fe6c16c17000000), // 0x1.6c16c17p-1
    fb(0x3fe6816817000000), // 0x1.6816817p-1
    fb(0x3fe642c859000000), // 0x1.642c859p-1
    fb(0x3fe6058160000000), // 0x1.605816p-1
    fb(0x3fe5c9882c000000), // 0x1.5c9882cp-1
    fb(0x3fe58ed231000000), // 0x1.58ed231p-1
    fb(0x3fe5555555000000), // 0x1.5555555p-1
    fb(0x3fe51d07eb000000), // 0x1.51d07ebp-1
    fb(0x3fe4e5e0a7000000), // 0x1.4e5e0a7p-1
    fb(0x3fe4afd6a0000000), // 0x1.4afd6ap-1
    fb(0x3fe47ae148000000), // 0x1.47ae148p-1
    fb(0x3fe446f865000000), // 0x1.446f865p-1
    fb(0x3fe4141414000000), // 0x1.4141414p-1
    fb(0x3fe3e22cbd000000), // 0x1.3e22cbdp-1
    fb(0x3fe3b13b14000000), // 0x1.3b13b14p-1
    fb(0x3fe3813814000000), // 0x1.3813814p-1
    fb(0x3fe3521cfb000000), // 0x1.3521cfbp-1
    fb(0x3fe323e34a000000), // 0x1.323e34ap-1
    fb(0x3fe2f684be000000), // 0x1.2f684bep-1
    fb(0x3fe2c9fb4e000000), // 0x1.2c9fb4ep-1
    fb(0x3fe29e412a000000), // 0x1.29e412ap-1
    fb(0x3fe27350b9000000), // 0x1.27350b9p-1
    fb(0x3fe2492492000000), // 0x1.2492492p-1
    fb(0x3fe21fb781000000), // 0x1.21fb781p-1
    fb(0x3fe1f7047e000000), // 0x1.1f7047ep-1
    fb(0x3fe1cf06ae000000), // 0x1.1cf06aep-1
    fb(0x3fe1a7b961000000), // 0x1.1a7b961p-1
    fb(0x3fe1811812000000), // 0x1.1811812p-1
    fb(0x3fe15b1e5f000000), // 0x1.15b1e5fp-1
    fb(0x3fe135c811000000), // 0x1.135c811p-1
    fb(0x3fe1111111000000), // 0x1.1111111p-1
    fb(0x3fe0ecf56c000000), // 0x1.0ecf56cp-1
    fb(0x3fe0c97150000000), // 0x1.0c9715p-1
    fb(0x3fe0a6810a000000), // 0x1.0a6810ap-1
    fb(0x3fe0842108000000), // 0x1.0842108p-1
    fb(0x3fe0624dd3000000), // 0x1.0624dd3p-1
    fb(0x3fe0410410000000), // 0x1.041041p-1
    fb(0x3fe0204081000000), // 0x1.0204081p-1
    fb(0x3fe0000000000000), // 0.5
];
const TL: [f64; 65] = [
    fb(0xbd12000000000000), // -0x1.2p-46
    fb(0x3f7b947689310dfa), // 0x1.b947689310dfap-8
    fb(0x3f8b5e909c96d11b), // 0x1.b5e909c96d11bp-7
    fb(0x3f945f4f59ed1e08), // 0x1.45f4f59ed1e08p-6
    fb(0x3f9af5f92cbd8bc1), // 0x1.af5f92cbd8bc1p-6
    fb(0x3fa0ba01a606dcde), // 0x1.0ba01a606dcdep-5
    fb(0x3fa3ed119b9a29cd), // 0x1.3ed119b9a29cdp-5
    fb(0x3fa714834298ed14), // 0x1.714834298ed14p-5
    fb(0x3faa30a9d983564c), // 0x1.a30a9d983564cp-5
    fb(0x3fad41d512670665), // 0x1.d41d512670665p-5
    fb(0x3fb02428c0f65442), // 0x1.02428c0f65442p-4
    fb(0x3fb1a23444eecb67), // 0x1.1a23444eecb67p-4
    fb(0x3fb31b30543f4bdd), // 0x1.31b30543f4bddp-4
    fb(0x3fb48f3ed39bfc2d), // 0x1.48f3ed39bfc2dp-4
    fb(0x3fb5fe8049a0e34b), // 0x1.5fe8049a0e34bp-4
    fb(0x3fb769140a6a9f30), // 0x1.769140a6a9f3p-4
    fb(0x3fb8cf1836c98bdb), // 0x1.8cf1836c98bdbp-4
    fb(0x3fba30a9d55540ca), // 0x1.a30a9d55540cap-4
    fb(0x3fbb8de4d1ee8167), // 0x1.b8de4d1ee8167p-4
    fb(0x3fbce6e4202ca20f), // 0x1.ce6e4202ca20fp-4
    fb(0x3fbe3bc1accacd30), // 0x1.e3bc1accacd3p-4
    fb(0x3fbf8c9683b5aafd), // 0x1.f8c9683b5aafdp-4
    fb(0x3fc06cbd68ca9a03), // 0x1.06cbd68ca9a03p-3
    fb(0x3fc11142f19df6c5), // 0x1.11142f19df6c5p-3
    fb(0x3fc1b3e71fa7a913), // 0x1.1b3e71fa7a913p-3
    fb(0x3fc254b4d37a4677), // 0x1.254b4d37a4677p-3
    fb(0x3fc2f3b6912cbe9c), // 0x1.2f3b6912cbe9cp-3
    fb(0x3fc390f68311581a), // 0x1.390f68311581ap-3
    fb(0x3fc42c7e7fffc53d), // 0x1.42c7e7fffc53dp-3
    fb(0x3fc4c65808c78cd1), // 0x1.4c65808c78cd1p-3
    fb(0x3fc55e8c50751bea), // 0x1.55e8c50751beap-3
    fb(0x3fc5f52445dec36c), // 0x1.5f52445dec36cp-3
    fb(0x3fc68a288c3f1195), // 0x1.68a288c3f1195p-3
    fb(0x3fc71da17bdf0cad), // 0x1.71da17bdf0cadp-3
    fb(0x3fc7af973608af6e), // 0x1.7af973608af6ep-3
    fb(0x3fc84011952a250e), // 0x1.84011952a250ep-3
    fb(0x3fc8cf1837a7e9f4), // 0x1.8cf1837a7e9f4p-3
    fb(0x3fc95cb2891e436a), // 0x1.95cb2891e436ap-3
    fb(0x3fc9e8e7b0f86974), // 0x1.9e8e7b0f86974p-3
    fb(0x3fca73beaa5db121), // 0x1.a73beaa5db121p-3
    fb(0x3fcafd3e39455867), // 0x1.afd3e39455867p-3
    fb(0x3fcb856cf060d985), // 0x1.b856cf060d985p-3
    fb(0x3fcc0c5134de1f90), // 0x1.c0c5134de1f9p-3
    fb(0x3fcc91f1371bc934), // 0x1.c91f1371bc934p-3
    fb(0x3fcd1652ffcd3ee8), // 0x1.d1652ffcd3ee8p-3
    fb(0x3fcd997c6f635e09), // 0x1.d997c6f635e09p-3
    fb(0x3fce1b733ab90ed0), // 0x1.e1b733ab90edp-3
    fb(0x3fce9c3ceadac7eb), // 0x1.e9c3ceadac7ebp-3
    fb(0x3fcf1bdeec43a29a), // 0x1.f1bdeec43a29ap-3
    fb(0x3fcf9a5e7a5fa392), // 0x1.f9a5e7a5fa392p-3
    fb(0x3fd00be05ac02ef5), // 0x1.00be05ac02ef5p-2
    fb(0x3fd04a054d81a29e), // 0x1.04a054d81a29ep-2
    fb(0x3fd087a0835957c5), // 0x1.087a0835957c5p-2
    fb(0x3fd0c4b4570994e1), // 0x1.0c4b4570994e1p-2
    fb(0x3fd101431aa1fe1b), // 0x1.101431aa1fe1bp-2
    fb(0x3fd13d4f08b98da2), // 0x1.13d4f08b98da2p-2
    fb(0x3fd178da53edb85c), // 0x1.178da53edb85cp-2
    fb(0x3fd1b3e71e9f9d22), // 0x1.1b3e71e9f9d22p-2
    fb(0x3fd1ee777defdeb7), // 0x1.1ee777defdeb7p-2
    fb(0x3fd2288d7b48e205), // 0x1.2288d7b48e205p-2
    fb(0x3fd2622b0f52e469), // 0x1.2622b0f52e469p-2
    fb(0x3fd29b522a4c62de), // 0x1.29b522a4c62dep-2
    fb(0x3fd2d404b0e30f4a), // 0x1.2d404b0e30f4ap-2
    fb(0x3fd30c4478f3fbaf), // 0x1.30c4478f3fbafp-2
    fb(0x3fd34413509f78df), // 0x1.34413509f78dfp-2
];
const B: [f64; 3] = [
    fb(0x3fdbcb7b15d35067), // 0x1.bcb7b15d35067p-2
    fb(0xbfcbcbb1cd29cbaf), // -0x1.bcbb1cd29cbafp-3
    fb(0x3fc2870e2624ce4e), // 0x1.2870e2624ce4ep-3
];
const C: [f64; 7] = [
    fb(0x3fdbcb7b1526e50e), // 0x1.bcb7b1526e50ep-2
    fb(0xbfcbcb7b1526e53d), // -0x1.bcb7b1526e53dp-3
    fb(0x3fc287a7636f3fa2), // 0x1.287a7636f3fa2p-3
    fb(0xbfbbcb7b146a14b3), // -0x1.bcb7b146a14b3p-4
    fb(0x3fb63c627d5219cb), // 0x1.63c627d5219cbp-4
    fb(0xbfb2880736c8762d), // -0x1.2880736c8762dp-4
    fb(0x3fafc1ecf913961a), // 0x1.fc1ecf913961ap-5
];

const ST: [u32; 16] = [
    0x501502f9, // 0x1.2a05f2p+33
    0x41200000, // 0x1.4p+3
    0x42c80000, // 0x1.9p+6
    0x00000000, // 0
    0x447a0000, // 0x1.f4p+9
    0x00000000, // 0
    0x461c4000, // 0x1.388p+13
    0x47c35000, // 0x1.86ap+16
    0x00000000, // 0
    0x49742400, // 0x1.e848p+19
    0x00000000, // 0
    0x4b189680, // 0x1.312dp+23
    0x4cbebc20, // 0x1.7d784p+26
    0x00000000, // 0
    0x4e6e6b28, // 0x1.dcd65p+29
    0x3f800000, // 0x1p+0
];

// ln(2)/ln(10)
const LN10: f64 = fb(0x3fd34413509f79ff); // 0x1.34413509f79ffp-2
const LN10H: f64 = fb(0x3fd34413509f7a00); // 0x1.34413509f7ap-2
const LN10L: f64 = fb(0xbc90cee0ed4ca7e9); // -0x1.0cee0ed4ca7e9p-54
/// `0x1.af23fp-34`, the width of the fast path's error interval.
const EPS: f64 = fb(0x3ddaf23f00000000);

/// The C's `as_special`: zeros, +inf, NaNs and negative arguments.
#[cold]
#[inline(never)]
fn as_special(x: f32) -> f32 {
    let ux = x.to_bits();
    let ax = ux << 1;
    if ax == 0 {
        // x = +/-0.0: -inf (the C raises FE_DIVBYZERO)
        return f32::from_bits(0x1ff << 23);
    }
    if ux == 0x7f80_0000 {
        return x; // x = +inf
    }
    if ax > 0xff00_0000 {
        return x + x; // x = NaN
    }
    // x < 0: the C raises FE_INVALID
    f32::from_bits(0x1ff8 << 19)
}

/// `log10(x)` for `f32`, correctly rounded (CORE-MATH's `cr_log10f`).
#[must_use]
pub fn log10f(x: f32) -> f32 {
    let mut ux = x.to_bits();
    if ux >= 0x7f80_0000 {
        return as_special(x); // <= -0, NaN, inf
    }
    if ux == ST[((ux >> 24) & 0xf) as usize] {
        // x = 10^n: the C's unsigned arithmetic
        let je = (((ux as i32) >> 23) - 126) as u32;
        let je = je.wrapping_mul(0x4d1_04d4) >> 28;
        return je as f32;
    }
    if ux < 0x0080_0000 {
        if ux == 0 {
            return as_special(x); // x = +0
        }
        // subnormal
        let n = ux.leading_zeros() as i32 - 8;
        ux <<= n;
        ux = ux.wrapping_sub((n << 23) as u32);
    }
    let e = ((ux as i32) >> 23) - 127;
    let m = i64::from(ux & ((1 << 23) - 1));
    let j = ((m + (1 << (23 - 7))) >> (23 - 6)) as usize;
    let mut tz = f64::from_bits(((m << (52 - 23)) as u64) | (0x3ff_u64 << 52));
    let ef = f64::from(e);
    let z = tz * TR[j] - 1.0;
    let z2 = z * z;
    let mut r = ((ef * LN10 + TL[j]) + z * B[0]) + z2 * (B[1] + z * B[2]);
    let mut ub = r as f32;
    let lb = (r + EPS) as f32;
    if ub != lb {
        let mut f = z
            * ((C[0] + z * C[1])
                + z2 * ((C[2] + z * C[3]) + z2 * (C[4] + z * C[5] + z2 * C[6])));
        f += LN10L * ef;
        f += TL[j] - TL[0];
        let el = ef * LN10H;
        r = el + f;
        ub = r as f32;
        tz = r;
        if tz.to_bits() & ((1 << 28) - 1) == 0 {
            let dr = (el - r) + f;
            r += dr * 32.0;
            ub = r as f32;
        }
    }
    ub
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::logf::tests::checksum;

    /// Bit-identical to CORE-MATH's C code (gcc 11.4, `-O2 -ffp-contract=off`) on every 4099th
    /// bit pattern (`validation/investigations/coremath-log10f/`).
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(log10f, 4099), 0xf899_7ea7_e41e_96f3);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(log10f, 1), 0x1672_ffa8_2b3c_084c);
    }

    #[test]
    fn special_values() {
        assert_eq!(log10f(1.0).to_bits(), 0);
        assert_eq!(log10f(0.0), f32::NEG_INFINITY);
        assert_eq!(log10f(-0.0), f32::NEG_INFINITY);
        assert_eq!(log10f(f32::INFINITY), f32::INFINITY);
        assert_eq!(log10f(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(log10f(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
        assert!(log10f(f32::NAN).is_nan());
        assert_eq!(log10f(f32::from_bits(0x7f80_0001)).to_bits(), 0x7fc0_0001);
        for n in 0..=10 {
            assert_eq!(log10f(10f32.powi(n)), n as f32, "{n}");
        }
        // the smallest subnormal: log10(2^-149)
        assert_eq!(log10f(f32::from_bits(1)).to_bits(), 0xc233_69f4);
    }

    /// The one positive float where CORE-MATH's double `cr_log10` rounded to float is one unit
    /// off (double rounding), and floats where glibc 2.35's `log10f` is (the C programs of
    /// `validation/investigations/coremath-log10f/`).
    #[test]
    fn where_the_others_round_otherwise() {
        let x = f32::from_bits(0x0efe_ee7a);
        assert_eq!(log10f(x).to_bits(), 0xc1e9_9d23);
        assert_eq!((super::super::log10(f64::from(x)) as f32).to_bits(), 0xc1e9_9d22);
        for (x, cr) in [
            (0x0000_0142u32, 0xc229_61e9u32),
            (0x0000_0437, 0xc227_4823),
            (0x0000_2318, 0xc223_9999),
        ] {
            assert_eq!(log10f(f32::from_bits(x)).to_bits(), cr, "{x:#x}");
        }
    }
}

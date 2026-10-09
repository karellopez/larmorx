// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded exponential `exp(x)` for binary64 (CORE-MATH's `cr_exp`), in pure Rust.
//!
//! The result is `e^x` rounded to the nearest `f64` (ties to even) for every input, so it is
//! bit-identical on every platform and agrees with any other correctly rounded `exp`.
//!
//! Port of CORE-MATH's `src/binary64/exp/exp.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2022-2025 Alexei
//! Sibidanov (the copyright and permission notice are reproduced below). The algorithm,
//! tables and operation order are kept exactly; it is re-expressed in Rust for
//! round-to-nearest only. Differences from the C file:
//!
//! - `__builtin_fma(a, b, c)` is `a.mul_add(b, c)` (a single rounding on every target). Rust
//!   never contracts `a * b + c` into an FMA, so every other expression is a separate multiply
//!   and add, as when `exp.c` is compiled with `-ffp-contract=off` or for baseline x86-64.
//!   CORE-MATH supports both (see the `FastFMA_DW` comment in exp.c), and the results are
//!   bit-identical to `exp.c` built either way.
//! - `roundeven_finite` is [`f64::round_ties_even`] (what `__builtin_roundeven` computes).
//! - `as_ldexp` / `as_todenormal` use the portable integer branch of the C code (the x86-64
//!   SSE2 branch does the same integer operations on the same bits).
//! - Floating-point exception flags and `errno` are not modelled: the `MXCSR` / `feraiseexcept`
//!   underflow signalling and the `CORE_MATH_SUPPORT_ERRNO` code are dropped, and the
//!   `volatile` overflow/underflow products are plain products (same results, `+inf` / `+0`).
//! - In `as_exp_database`, the branch the C code marks "we should never go here" (where the C
//!   loop would spin forever) returns its input `f`; the tests run every database entry.
//! - Hex float literals are written as `f64` bit patterns, generated from the C source and
//!   checked against it; each table line carries the original literals as a comment.

// Copyright (c) 2022-2025 Alexei Sibidanov.
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

/// `f64` from its IEEE-754 bit pattern (Rust has no hex float literals).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// A table of `f64` pairs from their bit patterns.
const fn fb_pairs<const N: usize>(bits: [[u64; 2]; N]) -> [[f64; 2]; N] {
    let mut out = [[0.0; 2]; N];
    let mut i = 0;
    while i < N {
        out[i] = [fb(bits[i][0]), fb(bits[i][1])];
        i += 1;
    }
    out
}

/// `0x1.71547652b82fep+12` ≈ 2^12 / log(2) (`s` in both C functions).
const S: f64 = fb(0x40b71547652b82fe);
/// `0x1.62e42ffp-13` (`l2h`): high part of log(2) / 2^12, 29 significant bits.
const L2H: f64 = fb(0x3f262e42ff000000);
/// `0x1.718432a1b0e26p-47` (`l2l`): `l2h - l2l` ≈ log(2) / 2^12.
const L2L: f64 = fb(0x3d0718432a1b0e26);
/// `0x1.9ff0342542fc3p-102` (`l2ll`): third part, accurate path only.
const L2LL: f64 = fb(0x3999ff0342542fc3);

#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    let e = y - z;
    (s, e)
}

#[inline(always)]
fn fastsum(xh: f64, xl: f64, yh: f64, yl: f64) -> (f64, f64) {
    let (sh, sl) = fasttwosum(xh, yh);
    let e = (xl + yl) + sl;
    (sh, e)
}

#[inline(always)]
fn muldd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahhh = ch * xh;
    let l = (ch * xl + cl * xh) + ch.mul_add(xh, -ahhh);
    (ahhh, l)
}

/// Algorithm 4 of Jeannerod, Joldes, Louvet and Muller, "Extended-Precision FMA under
/// Parameterized Double-Word Overlap: Tight Error Bounds and Examples", Arith 2026
/// (<https://inria.hal.science/hal-05517451>), with CORE-MATH's change: `g` and `l` use a
/// multiply and an add instead of an FMA. Returns `(dh, dl)`.
#[inline(always)]
fn fast_fma_dw(ah: f64, al: f64, bh: f64, bl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let dh = ah.mul_add(bh, ch);
    let t = ch - dh;
    let e = ah.mul_add(bh, t);
    let f = e + cl;
    // let g = ah.mul_add(bl, f); // original algorithm
    let g = ah * bl + f;
    // let l = al.mul_add(bh, g); // original algorithm
    let l = al * bh + g;
    // l is dl in the paper
    (dh, l)
}

#[inline(always)]
fn opolydd(xh: f64, xl: f64, c: &[[f64; 2]]) -> (f64, f64) {
    let n = c.len();
    let (mut ch, mut cl) = (c[n - 1][0], c[n - 1][1]);
    for ci in c[..n - 1].iter().rev() {
        (ch, cl) = fast_fma_dw(xh, xl, ch, cl, ci[0], ci[1]);
    }
    (ch, cl)
}

#[inline(always)]
fn as_ldexp(x: f64, i: i64) -> f64 {
    f64::from_bits(x.to_bits().wrapping_add((i as u64) << 52))
}

/// Sets the exponent of a binary64 number to 0 (subnormal range).
#[inline(always)]
fn as_todenormal(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & (u64::MAX >> 12))
}

/// Inputs for which the accurate path's rounding test fails, sorted by bit pattern for the
/// binary search in [`as_exp_database`]. Under round-to-nearest only some are looked up (18 of
/// the 51 in this port); all are kept so the table matches the C file.
const DB: [u64; 51] = [
    0x3cafffffffffffff, // 0x1.fffffffffffffp-53
    0x3f1ba07d73250de7, // 0x1.ba07d73250de7p-14
    0x3f76a4d1af9cc989, // 0x1.6a4d1af9cc989p-8
    0x3f95a75293a5dcda, // 0x1.5a75293a5dcdap-6
    0x3fa42ea46949b3c7, // 0x1.42ea46949b3c7p-5
    0x3fa7c8bb0cf5d160, // 0x1.7c8bb0cf5d16p-5
    0x3fc0948d39a41695, // 0x1.0948d39a41695p-3
    0x3fca065fefae814f, // 0x1.a065fefae814fp-3
    0x3fcf6e4c3ced7c72, // 0x1.f6e4c3ced7c72p-3
    0x3fd1a0408712e00a, // 0x1.1a0408712e00ap-2
    0x3fdbcab27d05abde, // 0x1.bcab27d05abdep-2
    0x3fe005ae04256bab, // 0x1.005ae04256babp-1
    0x401273c188aa7b14, // 0x1.273c188aa7b14p+2
    0x40183d4bcdebb3f4, // 0x1.83d4bcdebb3f4p+2
    0x40308f51434652c3, // 0x1.08f51434652c3p+4
    0x4031d5c2daebe367, // 0x1.1d5c2daebe367p+4
    0x403c44ce0d716a1a, // 0x1.c44ce0d716a1ap+4
    0x404e07e71bfcf06f, // 0x1.e07e71bfcf06fp+5
    0x404f7216c4b435c9, // 0x1.f7216c4b435c9p+5
    0x40654cd1fea7663a, // 0x1.54cd1fea7663ap+7
    0x407d6479eba7c971, // 0x1.d6479eba7c971p+8
    0xbf1664716b68a409, // -0x1.664716b68a409p-14
    0xbf2a2fefefd580df, // -0x1.a2fefefd580dfp-13
    0xbf3ce3f638d0c742, // -0x1.ce3f638d0c742p-12
    0xbf3ceff32831e2c2, // -0x1.ceff32831e2c2p-12
    0xbf433accae78b371, // -0x1.33accae78b371p-11
    0xbf4d792b60084f92, // -0x1.d792b60084f92p-11
    0xbf77fb235d76cce7, // -0x1.7fb235d76cce7p-8
    0xbf81ff9b8e8b38be, // -0x1.1ff9b8e8b38bep-7
    0xbf854511e930898c, // -0x1.54511e930898cp-7
    0xbf95c5ed0ec83666, // -0x1.5c5ed0ec83666p-6
    0xbf98c56ff5326197, // -0x1.8c56ff5326197p-6
    0xbf9a4187f2ca71f9, // -0x1.a4187f2ca71f9p-6
    0xbfba8f783d749a8f, // -0x1.a8f783d749a8fp-4
    0xbfbbd44fdaed819f, // -0x1.bd44fdaed819fp-4
    0xbfbdaf693d64fada, // -0x1.daf693d64fadap-4
    0xbfc290ea09e36479, // -0x1.290ea09e36479p-3
    0xbfc8aeb636f3ce35, // -0x1.8aeb636f3ce35p-3
    0xbfcd3f3799439415, // -0x1.d3f3799439415p-3
    0xbfcea16274b0109b, // -0x1.ea16274b0109bp-3
    0xbfe22e24fa3d5cf9, // -0x1.22e24fa3d5cf9p-1
    0xbfe85068c07fbbf6, // -0x1.85068c07fbbf6p-1
    0xbfebdc7955d1482c, // -0x1.bdc7955d1482cp-1
    0xbff2a9cad9998262, // -0x1.2a9cad9998262p+0
    0xbffcc37ef7de7501, // -0x1.cc37ef7de7501p+0
    0xc0002393d5976769, // -0x1.02393d5976769p+1
    0xc0065061daf79a78, // -0x1.65061daf79a78p+1
    0xc02e8bdbfcd9144e, // -0x1.e8bdbfcd9144ep+3
    0xc038f80e06f3a04c, // -0x1.8f80e06f3a04cp+4
    0xc0559f038076039c, // -0x1.59f038076039cp+6
    0xc06981587ad4542f, // -0x1.981587ad4542fp+7
];

#[inline(never)]
fn as_exp_database(x: f64, f: f64) -> f64 {
    let ix = x.to_bits();
    let mut a: isize = 0;
    let mut b: isize = DB.len() as isize - 1;
    let mut m = (a + b) / 2;
    while a <= b {
        let c = DB[m as usize];
        if c < ix {
            a = m + 1;
        } else if c == ix {
            // `s2` and `s` in the C code: for entry m, the 2 low bits of the correctly rounded
            // result, and the sign of the +-2^-54 correction (for the directed roundings).
            const S2: [u64; 2] = [0x57f5fe2e5bde4075, 0x3c1f16b8ed];
            const SG: u64 = 333811522313371;
            let jf = f.to_bits();
            let dr = fb(((SG >> m) << 63) | 0x3c90000000000000);
            let t = (S2[(m >> 5) as usize] >> ((m << 1) & 63)) & 3;
            for k in -1i64..=1 {
                let r = jf.wrapping_add_signed(k);
                if (r & 3) == t {
                    return fb(r) + dr;
                }
            }
            // we should never go here (the C loop would spin forever)
            return f;
        } else {
            b = m - 1;
        }
        m = (a + b) >> 1;
    }
    f
}

/// For 0 <= i < 2^6, `T0[i]` is a double-double approximation of 2^(i/2^6), as `[lo, hi]`.
const T0: [[f64; 2]; 64] = fb_pairs([
    [0x0000000000000000, 0x3ff0000000000000], // {0x0p+0, 0x1p+0}
    [0xbc719083535b085e, 0x3ff02c9a3e778061], // {-0x1.19083535b085ep-56, 0x1.02c9a3e778061p+0}
    [0x3c8d73e2a475b466, 0x3ff059b0d3158574], // {0x1.d73e2a475b466p-55, 0x1.059b0d3158574p+0}
    [0x3c6186be4bb28500, 0x3ff0874518759bc8], // {0x1.186be4bb285p-57, 0x1.0874518759bc8p+0}
    [0x3c98a62e4adc610a, 0x3ff0b5586cf9890f], // {0x1.8a62e4adc610ap-54, 0x1.0b5586cf9890fp+0}
    [0x3c403a1727c57b52, 0x3ff0e3ec32d3d1a2], // {0x1.03a1727c57b52p-59, 0x1.0e3ec32d3d1a2p+0}
    [0xbc96c51039449b3a, 0x3ff11301d0125b51], // {-0x1.6c51039449b3ap-54, 0x1.11301d0125b51p+0}
    [0xbc932fbf9af1369e, 0x3ff1429aaea92de0], // {-0x1.32fbf9af1369ep-54, 0x1.1429aaea92dep+0}
    [0xbc819041b9d78a76, 0x3ff172b83c7d517b], // {-0x1.19041b9d78a76p-55, 0x1.172b83c7d517bp+0}
    [0x3c8e5b4c7b4968e4, 0x3ff1a35beb6fcb75], // {0x1.e5b4c7b4968e4p-55, 0x1.1a35beb6fcb75p+0}
    [0x3c9e016e00a2643c, 0x3ff1d4873168b9aa], // {0x1.e016e00a2643cp-54, 0x1.1d4873168b9aap+0}
    [0x3c8dc775814a8494, 0x3ff2063b88628cd6], // {0x1.dc775814a8494p-55, 0x1.2063b88628cd6p+0}
    [0x3c99b07eb6c70572, 0x3ff2387a6e756238], // {0x1.9b07eb6c70572p-54, 0x1.2387a6e756238p+0}
    [0x3c82bd339940e9da, 0x3ff26b4565e27cdd], // {0x1.2bd339940e9dap-55, 0x1.26b4565e27cddp+0}
    [0x3c8612e8afad1256, 0x3ff29e9df51fdee1], // {0x1.612e8afad1256p-55, 0x1.29e9df51fdee1p+0}
    [0x3c90024754db41d4, 0x3ff2d285a6e4030b], // {0x1.0024754db41d4p-54, 0x1.2d285a6e4030bp+0}
    [0x3c86f46ad23182e4, 0x3ff306fe0a31b715], // {0x1.6f46ad23182e4p-55, 0x1.306fe0a31b715p+0}
    [0x3c932721843659a6, 0x3ff33c08b26416ff], // {0x1.32721843659a6p-54, 0x1.33c08b26416ffp+0}
    [0xbc963aeabf42eae2, 0x3ff371a7373aa9cb], // {-0x1.63aeabf42eae2p-54, 0x1.371a7373aa9cbp+0}
    [0xbc75e436d661f5e2, 0x3ff3a7db34e59ff7], // {-0x1.5e436d661f5e2p-56, 0x1.3a7db34e59ff7p+0}
    [0x3c8ada0911f09ebc, 0x3ff3dea64c123422], // {0x1.ada0911f09ebcp-55, 0x1.3dea64c123422p+0}
    [0xbc5ef3691c309278, 0x3ff4160a21f72e2a], // {-0x1.ef3691c309278p-58, 0x1.4160a21f72e2ap+0}
    [0x3c489b7a04ef80d0, 0x3ff44e086061892d], // {0x1.89b7a04ef80dp-59, 0x1.44e086061892dp+0}
    [0x3c73c1a3b69062f0, 0x3ff486a2b5c13cd0], // {0x1.3c1a3b69062fp-56, 0x1.486a2b5c13cdp+0}
    [0x3c7d4397afec42e2, 0x3ff4bfdad5362a27], // {0x1.d4397afec42e2p-56, 0x1.4bfdad5362a27p+0}
    [0xbc94b309d25957e4, 0x3ff4f9b2769d2ca7], // {-0x1.4b309d25957e4p-54, 0x1.4f9b2769d2ca7p+0}
    [0xbc807abe1db13cac, 0x3ff5342b569d4f82], // {-0x1.07abe1db13cacp-55, 0x1.5342b569d4f82p+0}
    [0x3c99bb2c011d93ac, 0x3ff56f4736b527da], // {0x1.9bb2c011d93acp-54, 0x1.56f4736b527dap+0}
    [0x3c96324c054647ac, 0x3ff5ab07dd485429], // {0x1.6324c054647acp-54, 0x1.5ab07dd485429p+0}
    [0x3c9ba6f93080e65e, 0x3ff5e76f15ad2148], // {0x1.ba6f93080e65ep-54, 0x1.5e76f15ad2148p+0}
    [0xbc9383c17e40b496, 0x3ff6247eb03a5585], // {-0x1.383c17e40b496p-54, 0x1.6247eb03a5585p+0}
    [0xbc9bb60987591c34, 0x3ff6623882552225], // {-0x1.bb60987591c34p-54, 0x1.6623882552225p+0}
    [0xbc9bdd3413b26456, 0x3ff6a09e667f3bcd], // {-0x1.bdd3413b26456p-54, 0x1.6a09e667f3bcdp+0}
    [0xbc6bbe3a683c88aa, 0x3ff6dfb23c651a2f], // {-0x1.bbe3a683c88aap-57, 0x1.6dfb23c651a2fp+0}
    [0xbc816e4786887a9a, 0x3ff71f75e8ec5f74], // {-0x1.16e4786887a9ap-55, 0x1.71f75e8ec5f74p+0}
    [0xbc90245957316dd4, 0x3ff75feb564267c9], // {-0x1.0245957316dd4p-54, 0x1.75feb564267c9p+0}
    [0xbc841577ee049930, 0x3ff7a11473eb0187], // {-0x1.41577ee04993p-55, 0x1.7a11473eb0187p+0}
    [0x3c705d02ba15797e, 0x3ff7e2f336cf4e62], // {0x1.05d02ba15797ep-56, 0x1.7e2f336cf4e62p+0}
    [0xbc9d4c1dd41532d8, 0x3ff82589994cce13], // {-0x1.d4c1dd41532d8p-54, 0x1.82589994cce13p+0}
    [0xbc9fc6f89bd4f6ba, 0x3ff868d99b4492ed], // {-0x1.fc6f89bd4f6bap-54, 0x1.868d99b4492edp+0}
    [0x3c96e9f156864b26, 0x3ff8ace5422aa0db], // {0x1.6e9f156864b26p-54, 0x1.8ace5422aa0dbp+0}
    [0x3c85cc13a2e3976c, 0x3ff8f1ae99157736], // {0x1.5cc13a2e3976cp-55, 0x1.8f1ae99157736p+0}
    [0xbc675fc781b57ebc, 0x3ff93737b0cdc5e5], // {-0x1.75fc781b57ebcp-57, 0x1.93737b0cdc5e5p+0}
    [0xbc9d185b7c1b85d0, 0x3ff97d829fde4e50], // {-0x1.d185b7c1b85dp-54, 0x1.97d829fde4e5p+0}
    [0x3c7c7c46b071f2be, 0x3ff9c49182a3f090], // {0x1.c7c46b071f2bep-56, 0x1.9c49182a3f09p+0}
    [0xbc9359495d1cd532, 0x3ffa0c667b5de565], // {-0x1.359495d1cd532p-54, 0x1.a0c667b5de565p+0}
    [0xbc9d2f6edb8d41e2, 0x3ffa5503b23e255d], // {-0x1.d2f6edb8d41e2p-54, 0x1.a5503b23e255dp+0}
    [0x3c90fac90ef7fd32, 0x3ffa9e6b5579fdbf], // {0x1.0fac90ef7fd32p-54, 0x1.a9e6b5579fdbfp+0}
    [0x3c97a1cd345dcc82, 0x3ffae89f995ad3ad], // {0x1.7a1cd345dcc82p-54, 0x1.ae89f995ad3adp+0}
    [0xbc62805e3084d708, 0x3ffb33a2b84f15fb], // {-0x1.2805e3084d708p-57, 0x1.b33a2b84f15fbp+0}
    [0xbc75584f7e54ac3a, 0x3ffb7f76f2fb5e47], // {-0x1.5584f7e54ac3ap-56, 0x1.b7f76f2fb5e47p+0}
    [0x3c823dd07a2d9e84, 0x3ffbcc1e904bc1d2], // {0x1.23dd07a2d9e84p-55, 0x1.bcc1e904bc1d2p+0}
    [0x3c811065895048de, 0x3ffc199bdd85529c], // {0x1.11065895048dep-55, 0x1.c199bdd85529cp+0}
    [0x3c92884dff483cac, 0x3ffc67f12e57d14b], // {0x1.2884dff483cacp-54, 0x1.c67f12e57d14bp+0}
    [0x3c7503cbd1e949dc, 0x3ffcb720dcef9069], // {0x1.503cbd1e949dcp-56, 0x1.cb720dcef9069p+0}
    [0xbc9cbc3743797a9c, 0x3ffd072d4a07897c], // {-0x1.cbc3743797a9cp-54, 0x1.d072d4a07897cp+0}
    [0x3c82ed02d75b3706, 0x3ffd5818dcfba487], // {0x1.2ed02d75b3706p-55, 0x1.d5818dcfba487p+0}
    [0x3c9c2300696db532, 0x3ffda9e603db3285], // {0x1.c2300696db532p-54, 0x1.da9e603db3285p+0}
    [0xbc91a5cd4f184b5c, 0x3ffdfc97337b9b5f], // {-0x1.1a5cd4f184b5cp-54, 0x1.dfc97337b9b5fp+0}
    [0x3c839e8980a9cc90, 0x3ffe502ee78b3ff6], // {0x1.39e8980a9cc9p-55, 0x1.e502ee78b3ff6p+0}
    [0xbc9e9c23179c2894, 0x3ffea4afa2a490da], // {-0x1.e9c23179c2894p-54, 0x1.ea4afa2a490dap+0}
    [0x3c9dc7f486a4b6b0, 0x3ffefa1bee615a27], // {0x1.dc7f486a4b6bp-54, 0x1.efa1bee615a27p+0}
    [0x3c99d3e12dd8a18a, 0x3fff50765b6e4540], // {0x1.9d3e12dd8a18ap-54, 0x1.f50765b6e454p+0}
    [0x3c874853f3a5931e, 0x3fffa7c1819e90d8], // {0x1.74853f3a5931ep-55, 0x1.fa7c1819e90d8p+0}
]);

/// For 0 <= i < 2^6, `T1[i]` is a double-double approximation of 2^(i/2^12), as `[lo, hi]`.
const T1: [[f64; 2]; 64] = fb_pairs([
    [0x0000000000000000, 0x3ff0000000000000], // {0x0p+0, 0x1p+0}
    [0x3c9ae8e38c59c72a, 0x3ff000b175effdc7], // {0x1.ae8e38c59c72ap-54, 0x1.000b175effdc7p+0}
    [0xbc57b5d0d58ea8f4, 0x3ff00162f3904052], // {-0x1.7b5d0d58ea8f4p-58, 0x1.00162f3904052p+0}
    [0x3c94115cb6b16a8e, 0x3ff0021478e11ce6], // {0x1.4115cb6b16a8ep-54, 0x1.0021478e11ce6p+0}
    [0xbc8d7c96f201bb2e, 0x3ff002c605e2e8cf], // {-0x1.d7c96f201bb2ep-55, 0x1.002c605e2e8cfp+0}
    [0x3c984711d4c35ea0, 0x3ff003779a95f959], // {0x1.84711d4c35eap-54, 0x1.003779a95f959p+0}
    [0xbc80484245243778, 0x3ff0042936faa3d8], // {-0x1.0484245243778p-55, 0x1.0042936faa3d8p+0}
    [0xbc94b237da2025fa, 0x3ff004dadb113da0], // {-0x1.4b237da2025fap-54, 0x1.004dadb113dap+0}
    [0xbc75e00e62d6b30e, 0x3ff0058c86da1c0a], // {-0x1.5e00e62d6b30ep-56, 0x1.0058c86da1c0ap+0}
    [0x3c9a1d6cedbb9480, 0x3ff0063e3a559473], // {0x1.a1d6cedbb948p-54, 0x1.0063e3a559473p+0}
    [0xbc94acf197a00142, 0x3ff006eff583fc3d], // {-0x1.4acf197a00142p-54, 0x1.006eff583fc3dp+0}
    [0xbc6eaf2ea42391a6, 0x3ff007a1b865a8ca], // {-0x1.eaf2ea42391a6p-57, 0x1.007a1b865a8cap+0}
    [0x3c7da93f90835f76, 0x3ff0085382faef83], // {0x1.da93f90835f76p-56, 0x1.0085382faef83p+0}
    [0xbc86a79084ab093c, 0x3ff00905554425d4], // {-0x1.6a79084ab093cp-55, 0x1.00905554425d4p+0}
    [0x3c986364f8fbe8f8, 0x3ff009b72f41a12b], // {0x1.86364f8fbe8f8p-54, 0x1.009b72f41a12bp+0}
    [0xbc882e8e14e3110e, 0x3ff00a6910f3b6fd], // {-0x1.82e8e14e3110ep-55, 0x1.00a6910f3b6fdp+0}
    [0xbc84f6b2a7609f72, 0x3ff00b1afa5abcbf], // {-0x1.4f6b2a7609f72p-55, 0x1.00b1afa5abcbfp+0}
    [0xbc7e1a258ea8f71a, 0x3ff00bcceb7707ec], // {-0x1.e1a258ea8f71ap-56, 0x1.00bcceb7707ecp+0}
    [0x3c74362ca5bc26f2, 0x3ff00c7ee448ee02], // {0x1.4362ca5bc26f2p-56, 0x1.00c7ee448ee02p+0}
    [0x3c9095a56c919d02, 0x3ff00d30e4d0c483], // {0x1.095a56c919d02p-54, 0x1.00d30e4d0c483p+0}
    [0xbc6406ac4e81a646, 0x3ff00de2ed0ee0f5], // {-0x1.406ac4e81a646p-57, 0x1.00de2ed0ee0f5p+0}
    [0x3c9b5a6902767e08, 0x3ff00e94fd0398e0], // {0x1.b5a6902767e08p-54, 0x1.00e94fd0398ep+0}
    [0xbc991b2060859320, 0x3ff00f4714af41d3], // {-0x1.91b206085932p-54, 0x1.00f4714af41d3p+0}
    [0x3c8427068ab22306, 0x3ff00ff93412315c], // {0x1.427068ab22306p-55, 0x1.00ff93412315cp+0}
    [0x3c9c1d0660524e08, 0x3ff010ab5b2cbd11], // {0x1.c1d0660524e08p-54, 0x1.010ab5b2cbd11p+0}
    [0xbc9e7bdfb3204be8, 0x3ff0115d89ff3a8b], // {-0x1.e7bdfb3204be8p-54, 0x1.0115d89ff3a8bp+0}
    [0x3c8843aa8b9cbbc6, 0x3ff0120fc089ff63], // {0x1.843aa8b9cbbc6p-55, 0x1.0120fc089ff63p+0}
    [0xbc734104ee7edae8, 0x3ff012c1fecd613b], // {-0x1.34104ee7edae8p-56, 0x1.012c1fecd613bp+0}
    [0xbc72b6aeb6176892, 0x3ff0137444c9b5b5], // {-0x1.2b6aeb6176892p-56, 0x1.0137444c9b5b5p+0}
    [0x3c7a8cd33b8a1bb2, 0x3ff01426927f5278], // {0x1.a8cd33b8a1bb2p-56, 0x1.01426927f5278p+0}
    [0x3c72edc08e5da99a, 0x3ff014d8e7ee8d2f], // {0x1.2edc08e5da99ap-56, 0x1.014d8e7ee8d2fp+0}
    [0x3c857ba2dc7e0c72, 0x3ff0158b4517bb88], // {0x1.57ba2dc7e0c72p-55, 0x1.0158b4517bb88p+0}
    [0x3c9b61299ab8cdb8, 0x3ff0163da9fb3335], // {0x1.b61299ab8cdb8p-54, 0x1.0163da9fb3335p+0}
    [0xbc990565902c5f44, 0x3ff016f0169949ed], // {-0x1.90565902c5f44p-54, 0x1.016f0169949edp+0}
    [0x3c870fc41c5c2d54, 0x3ff017a28af25567], // {0x1.70fc41c5c2d54p-55, 0x1.017a28af25567p+0}
    [0x3c94b9a6e145d76c, 0x3ff018550706ab62], // {0x1.4b9a6e145d76cp-54, 0x1.018550706ab62p+0}
    [0xbc7008eff5142bfa, 0x3ff019078ad6a19f], // {-0x1.008eff5142bfap-56, 0x1.019078ad6a19fp+0}
    [0xbc977669f033c7de, 0x3ff019ba16628de2], // {-0x1.77669f033c7dep-54, 0x1.019ba16628de2p+0}
    [0xbc909bb78eeead0a, 0x3ff01a6ca9aac5f3], // {-0x1.09bb78eeead0ap-54, 0x1.01a6ca9aac5f3p+0}
    [0x3c9371231477ece6, 0x3ff01b1f44af9f9e], // {0x1.371231477ece6p-54, 0x1.01b1f44af9f9ep+0}
    [0x3c75e7626621eb5a, 0x3ff01bd1e77170b4], // {0x1.5e7626621eb5ap-56, 0x1.01bd1e77170b4p+0}
    [0xbc9bc72b100828a4, 0x3ff01c8491f08f08], // {-0x1.bc72b100828a4p-54, 0x1.01c8491f08f08p+0}
    [0xbc6ce39cbbab8bbe, 0x3ff01d37442d5070], // {-0x1.ce39cbbab8bbep-57, 0x1.01d37442d507p+0}
    [0x3c816996709da2e2, 0x3ff01de9fe280ac8], // {0x1.16996709da2e2p-55, 0x1.01de9fe280ac8p+0}
    [0xbc8c11f5239bf536, 0x3ff01e9cbfe113ef], // {-0x1.c11f5239bf536p-55, 0x1.01e9cbfe113efp+0}
    [0x3c8e1d4eb5edc6b4, 0x3ff01f4f8958c1c6], // {0x1.e1d4eb5edc6b4p-55, 0x1.01f4f8958c1c6p+0}
    [0xbc9afb99946ee3f0, 0x3ff020025a8f6a35], // {-0x1.afb99946ee3fp-54, 0x1.020025a8f6a35p+0}
    [0xbc98f06d8a148a32, 0x3ff020b533856324], // {-0x1.8f06d8a148a32p-54, 0x1.020b533856324p+0}
    [0xbc82bf310fc54eb6, 0x3ff02168143b0281], // {-0x1.2bf310fc54eb6p-55, 0x1.02168143b0281p+0}
    [0xbc9c95a035eb4176, 0x3ff0221afcb09e3e], // {-0x1.c95a035eb4176p-54, 0x1.0221afcb09e3ep+0}
    [0xbc9491793e46834c, 0x3ff022cdece68c4f], // {-0x1.491793e46834cp-54, 0x1.022cdece68c4fp+0}
    [0xbc73e8d0d9c49090, 0x3ff02380e4dd22ad], // {-0x1.3e8d0d9c4909p-56, 0x1.02380e4dd22adp+0}
    [0xbc9314aa16278aa4, 0x3ff02433e494b755], // {-0x1.314aa16278aa4p-54, 0x1.02433e494b755p+0}
    [0x3c848daf888e9650, 0x3ff024e6ec0da046], // {0x1.48daf888e965p-55, 0x1.024e6ec0da046p+0}
    [0x3c856dc8046821f4, 0x3ff02599fb483385], // {0x1.56dc8046821f4p-55, 0x1.02599fb483385p+0}
    [0x3c945b42356b9d46, 0x3ff0264d1244c719], // {0x1.45b42356b9d46p-54, 0x1.0264d1244c719p+0}
    [0xbc7082ef51b61d7e, 0x3ff027003103b10e], // {-0x1.082ef51b61d7ep-56, 0x1.027003103b10ep+0}
    [0x3c72106ed0920a34, 0x3ff027b357854772], // {0x1.2106ed0920a34p-56, 0x1.027b357854772p+0}
    [0xbc9fd4cf26ea5d0e, 0x3ff0286685c9e059], // {-0x1.fd4cf26ea5d0ep-54, 0x1.0286685c9e059p+0}
    [0xbc909f8775e78084, 0x3ff02919bbd1d1d8], // {-0x1.09f8775e78084p-54, 0x1.02919bbd1d1d8p+0}
    [0x3c564cbba902ca28, 0x3ff029ccf99d720a], // {0x1.64cbba902ca28p-58, 0x1.029ccf99d720ap+0}
    [0x3c94383ef231d206, 0x3ff02a803f2d170d], // {0x1.4383ef231d206p-54, 0x1.02a803f2d170dp+0}
    [0x3c94a47a505b3a46, 0x3ff02b338c811703], // {0x1.4a47a505b3a46p-54, 0x1.02b338c811703p+0}
    [0x3c9e471202234680, 0x3ff02be6e199c811], // {0x1.e47120223468p-54, 0x1.02be6e199c811p+0}
]);

/// Accurate-path polynomial, double-double coefficients as `[hi, lo]`.
const CH_ACC: [[f64; 2]; 7] = fb_pairs([
    [0x3ff0000000000000, 0x0000000000000000], // {0x1p+0, 0}
    [0x3fe0000000000000, 0x39c712f72ecec2cf], // {0x1p-1, 0x1.712f72ecec2cfp-99}
    [0x3fc5555555555555, 0x3c65555555554d07], // {0x1.5555555555555p-3, 0x1.5555555554d07p-57}
    [0x3fa5555555555555, 0x3c455194d28275da], // {0x1.5555555555555p-5, 0x1.55194d28275dap-59}
    [0x3f81111111111111, 0x3c012faa0e1c0f7b], // {0x1.1111111111111p-7, 0x1.12faa0e1c0f7bp-63}
    [0x3f56c16c16da6973, 0xbbf4ba45ab25d2a3], // {0x1.6c16c16da6973p-10, -0x1.4ba45ab25d2a3p-64}
    [0x3f2a01a019eb7f31, 0xbbc9091d845ecd36], // {0x1.a01a019eb7f31p-13, -0x1.9091d845ecd36p-67}
]);

/// Fast-path polynomial: `1 + x*(c0 + c1*x + c2*x^2 + c3*x^3)` approximates exp(x) on
/// [-log(2)/2^13, log(2)/2^13] with absolute error below 2^-76.173 (Sollya).
const CH_FAST: [f64; 4] = [
    fb(0x3ff0000000000000), // 0x1p+0
    fb(0x3fe0000000000000), // 0x1p-1
    fb(0x3fc55555557e54ff), // 0x1.55555557e54ffp-3
    fb(0x3fa55555553a12f4), // 0x1.55555553a12f4p-5
];

/// Assumes |x| > 2^-54, since |x| <= 2^-54 is handled at the beginning of [`exp`].
#[cold]
#[inline(never)]
fn as_exp_accurate(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let t = (x * S).round_ties_even();
    let jt = t as i64;
    let i0 = ((jt >> 6) & 0x3f) as usize;
    let i1 = (jt & 0x3f) as usize;
    let ie = jt >> 12;
    let (t0h, t0l) = (T0[i0][1], T0[i0][0]);
    let (t1h, t1l) = (T1[i1][1], T1[i1][0]);
    let (th, tl) = muldd(t0h, t0l, t1h, t1l);

    // Cody-Waite argument reduction: since |x| < 745, we have |t| < 2^23, thus since l2h is
    // exactly representable on 29 bits, l2h*t is exact.
    let dx = x - L2H * t;
    let mut dxl = L2L * t;
    let dxll = L2LL * t + L2L.mul_add(t, -dxl);
    let dxh = dx + dxl;
    dxl = (dx - dxh) + dxl + dxll;
    let (mut fh, mut fl) = opolydd(dxh, dxl, &CH_ACC);
    (fh, fl) = muldd(dxh, dxl, fh, fl);
    if ix > 0xc086232bdd7abcd2 {
        // x < -0x1.6232bdd7abcd2p+9
        ix = ((1 - ie) << 52) as u64;
        (fh, fl) = muldd(fh, fl, th, tl);
        (fh, fl) = fastsum(th, tl, fh, fl);
        let e;
        (fh, e) = fasttwosum(fb(ix), fh);
        fl += e;
        fh = as_todenormal(fh + fl);
    } else {
        if th == 1.0 {
            let mut e;
            (fh, e) = fasttwosum(th, fh);
            (fl, e) = fasttwosum(e, fl);
            ix = fl.to_bits();
            if (ix & (u64::MAX >> 12)) == 0 {
                let v = e.to_bits();
                let d = (((((ix as i64) >> 63) ^ ((v as i64) >> 63)) as u64) << 1) + 1;
                ix = ix.wrapping_add(d);
                fl = fb(ix);
            }
        } else {
            (fh, fl) = muldd(fh, fl, th, tl);
            (fh, fl) = fastsum(th, tl, fh, fl);
        }
        (fh, fl) = fasttwosum(fh, fl);
        ix = fl.to_bits();
        let d = ix.wrapping_add(2) & (u64::MAX >> 12);
        if d <= 2 {
            fh = as_exp_database(x, fh);
        }
        fh = as_ldexp(fh, ie);
    }
    fh
}

/// Correctly rounded `e^x` (round to nearest, ties to even).
///
/// - `exp(±0) = 1`, `exp(+inf) = +inf`, `exp(-inf) = +0`, `exp(NaN)` is NaN.
/// - `+inf` for `x >= 0x1.62e42fefa39fp+9` (≈ 709.782712893384); `+0` for
///   `x <= -0x1.74910d52d3052p+9` (≈ -745.1332191019412); correctly rounded subnormal results
///   in between.
///
/// Port of CORE-MATH's `cr_exp` (see the module documentation).
#[must_use]
#[inline(always)]
pub fn exp(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let aix = ix & (u64::MAX >> 1);
    // exp(x) rounds to 1 to nearest for |x| <= 0x1p-54
    if aix <= 0x3c90000000000000 {
        // |x| <= 0x1p-54
        return 1.0 + x;
    }
    if aix >= 0x40862e42fefa39f0 {
        // |x| >= 0x1.62e42fefa39fp+9
        if aix > 0x7ff0000000000000 {
            return x + x; // nan
        }
        if aix == 0x7ff0000000000000 {
            // |x| = inf
            return if (ix >> 63) != 0 {
                0.0 // x = -inf
            } else {
                x // x = inf
            };
        }
        if (ix >> 63) == 0 {
            // x >= 0x1.62e42fefa39fp+9: overflow
            let z = fb(0x7fe0000000000000); // 0x1p1023
            return z * z;
        }
        if aix >= 0x40874910d52d3052 {
            // x <= -0x1.74910d52d3052p+9: underflow
            let z = fb(0x0010000000000000); // 0x1p-1022
            return z * z;
        }
    }
    let t = (x * S).round_ties_even();
    let jt = t as i64;
    let i0 = ((jt >> 6) & 0x3f) as usize;
    let i1 = (jt & 0x3f) as usize;
    let ie = jt >> 12;
    let (t0h, t0l) = (T0[i0][1], T0[i0][0]);
    let (t1h, t1l) = (T1[i1][1], T1[i1][0]);
    let (th, tl) = muldd(t0h, t0l, t1h, t1l);
    // Cody-Waite argument reduction: since |x| < 745, we have |t| < 2^23, thus since l2h is
    // exactly representable on 29 bits, l2h*t is exact.
    let dx = (x - L2H * t) + L2L * t;
    let dx2 = dx * dx;
    // |dx| < log(2)/2^13 (experimentally)
    let p = (CH_FAST[0] + dx * CH_FAST[1]) + dx2 * (CH_FAST[2] + dx * CH_FAST[3]);
    let mut fh = th;
    let tx = th * dx;
    let mut fl = tl + tx * p;
    let eps = 1.64e-19;
    if ix > 0xc086232bdd7abcd2 {
        // subnormal case: x < -0x1.6232bdd7abcd2p+9
        ix = ((1 - ie) << 52) as u64;
        let e;
        (fh, e) = fasttwosum(fb(ix), fh);
        fl += e;
        let ub = fh + (fl + eps);
        let lb = fh + (fl - eps);
        if ub != lb {
            return as_exp_accurate(x);
        }
        fh = as_todenormal(lb);
    } else {
        let ub = fh + (fl + eps);
        let lb = fh + (fl - eps);
        if ub != lb {
            return as_exp_accurate(x);
        }
        fh = as_ldexp(lb, ie);
    }
    fh
}

#[cfg(test)]
mod tests {
    use super::{DB, exp};

    fn check(cases: &[(u64, u64)]) {
        for &(x, y) in cases {
            let got = exp(f64::from_bits(x)).to_bits();
            assert_eq!(
                got,
                y,
                "exp({:e}) = {got:#018x}, expected {y:#018x}",
                f64::from_bits(x)
            );
        }
    }

    #[test]
    fn special_values() {
        assert_eq!(exp(0.0).to_bits(), 1.0f64.to_bits());
        assert_eq!(exp(-0.0).to_bits(), 1.0f64.to_bits());
        assert_eq!(exp(f64::INFINITY), f64::INFINITY);
        assert_eq!(exp(f64::NEG_INFINITY).to_bits(), 0); // +0
        assert!(exp(f64::NAN).is_nan());
        assert!(exp(-f64::NAN).is_nan());
        assert!(exp(f64::from_bits(0x7ff0000000000001)).is_nan()); // signalling NaN
    }

    /// Expected values are e^x rounded to nearest (mpmath, 300 bits, rounding certified).
    #[test]
    fn known_values() {
        check(&[
            (0x3ff0000000000000, 0x4005bf0a8b145769), // 0x1p+0 (e)
            (0xbff0000000000000, 0x3fd78b56362cef38), // -0x1p+0
            (0x3fe0000000000000, 0x3ffa61298e1e069c), // 0x1p-1
            (0x4000000000000000, 0x401d8e64b8d4ddae), // 0x1p+1
            (0x4024000000000000, 0x40d5829dcf950560), // 0x1.4p+3
            (0xc024000000000000, 0x3f07cd79b5647c9b), // -0x1.4p+3
            (0x4059000000000000, 0x48f3494a9b171bf5), // 0x1.9p+6
            (0xc059000000000000, 0x36ea8c1f14e2af5d), // -0x1.9p+6
            (0x3fe62e42fefa39ef, 0x4000000000000000), // 0x1.62e42fefa39efp-1 (log(2))
        ]);
        assert_eq!(exp(1.0).to_bits(), core::f64::consts::E.to_bits());
    }

    #[test]
    fn tiny_arguments() {
        check(&[
            (0x3c90000000000000, 0x3ff0000000000000), // 0x1p-54 (1 + x shortcut)
            (0xbc90000000000000, 0x3ff0000000000000), // -0x1p-54 (1 + x shortcut)
            (0x3c90000000000001, 0x3ff0000000000000), // 0x1.0000000000001p-54
            (0xbc90000000000001, 0x3fefffffffffffff), // -0x1.0000000000001p-54
            (0x3ca0000000000000, 0x3ff0000000000001), // 0x1p-53
            (0xbca0000000000000, 0x3fefffffffffffff), // -0x1p-53
            (0x00000000000007e8, 0x3ff0000000000000), // 0x0.00000000007e8p-1022 (subnormal x)
            (0x80000000000007e8, 0x3ff0000000000000), // -0x0.00000000007e8p-1022 (subnormal x)
            (0x0000000000000001, 0x3ff0000000000000), // 0x0.0000000000001p-1022 (min subnormal x)
            (0x0010000000000000, 0x3ff0000000000000), // 0x1p-1022 (min normal x)
        ]);
    }

    #[test]
    fn overflow() {
        check(&[
            (0x40862e42fefa39ef, 0x7fefffffffffff2a), // 0x1.62e42fefa39efp+9 (max finite result)
        ]);
        for x in [
            f64::from_bits(0x40862e42fefa39f0),
            709.79,
            710.0,
            1e300,
            f64::MAX,
        ] {
            assert_eq!(exp(x), f64::INFINITY, "exp({x:e})");
        }
    }

    #[test]
    fn underflow_and_subnormal_results() {
        check(&[
            (0xc086232bdd7abcd2, 0x001000000000007c), // -0x1.6232bdd7abcd2p+9 (least normal result)
            (0xc086232bdd7abcd3, 0x000ffffffffffe7c), // -0x1.6232bdd7abcd3p+9 (subnormal result)
            (0xc086233333333333, 0x000ff15b469edf89), // -0x1.6233333333333p+9 (subnormal result)
            (0xc086800000000000, 0x0000000993b4dc95), // -0x1.68p+9 (subnormal result)
            (0xc087480000000000, 0x0000000000000001), // -0x1.748p+9 (subnormal result)
            (0xc0874385446d71c3, 0x0000000000000001), // -0x1.74385446d71c3p+9 (exp(x) >= 2^-1074)
            (0xc0874910d52d3051, 0x0000000000000001), // -0x1.74910d52d3051p+9 (exp(x) > 2^-1075)
            (0xc0874910d52d3052, 0x0000000000000000), // -0x1.74910d52d3052p+9 (rounds to +0)
        ]);
        for x in [-746.0, -1000.0, -1e300, f64::MIN] {
            assert_eq!(exp(x).to_bits(), 0, "exp({x:e})"); // +0
        }
    }

    /// Hard-to-round inputs from CORE-MATH's exp.wc that take the accurate path: its subnormal
    /// branch, its `th == 1` branch (including the `fl` nudge), the general branch, and
    /// database lookups that miss.
    #[test]
    fn hard_cases_accurate_path() {
        check(&[
            (0xc08694431f063c66, 0x00000000c2c26ff1), // -0x1.694431f063c66p+9 (subnormal branch)
            (0xc086668cf1c012f3, 0x000000e691298980), // -0x1.6668cf1c012f3p+9 (subnormal branch)
            (0xc086396c9f346e1c, 0x0000fdb2d2bd145a), // -0x1.6396c9f346e1cp+9 (subnormal branch)
            (0xbedb26d9ff57f21a, 0x3feffff26c95e18c), // -0x1.b26d9ff57f21ap-18 (th == 1 branch)
            (0x3d465bfffffffe0e, 0x3ff00000000002cc), // 0x1.65bfffffffe0ep-43 (th == 1 branch)
            (0x3ca0000000000482, 0x3ff0000000000001), // 0x1.0000000000482p-53 (th == 1 branch)
            (0xbf39cc1dfbe77eeb, 0x3feffcc6a5d7271d), // -0x1.9cc1dfbe77eebp-12 (general branch)
            (0xbf3b68ee6aaad332, 0x3feffc931125d27c), // -0x1.b68ee6aaad332p-12 (general branch)
            (0x3f73b1b951bc1117, 0x3ff013bddd337fca), // 0x1.3b1b951bc1117p-8 (general branch)
            (0xc0254c9218baa581, 0x3ef8dcff0c6f3934), // -0x1.54c9218baa581p+3 (database miss)
            (0x3f3d83e8bdd4f7e4, 0x3ff001d859c61ec6), // 0x1.d83e8bdd4f7e4p-12 (database miss)
            (0x3e09e9cbbfd6080b, 0x3ff000000033d398), // 0x1.9e9cbbfd6080bp-31 (th == 1, db miss)
            (0xbe76ff486dc6f81a, 0x3fefffffd2016f46), // -0x1.6ff486dc6f81ap-24 (th == 1, db miss)
            (0x3eb282aae3169ee7, 0x3ff00001282ab8e7), // 0x1.282aae3169ee7p-20 (th == 1, fl nudged)
            (0xbe30401ae48409b5, 0x3feffffffdf7fca3), // -0x1.0401ae48409b5p-28 (th == 1, fl nudged)
        ]);
    }

    /// Every database entry, with its correctly rounded result (index-aligned with `DB`).
    #[test]
    fn database_entries() {
        const EXPECTED: [u64; 51] = [
            0x3ff0000000000001, // exp(0x1.fffffffffffffp-53)
            0x3ff0006e83736f8d, // exp(0x1.ba07d73250de7p-14)
            0x3ff016b4df3299d7, // exp(0x1.6a4d1af9cc989p-8)
            0x3ff05789640bc8ad, // exp(0x1.5a75293a5dcdap-6)
            0x3ff0a4ae9718080c, // exp(0x1.42ea46949b3c7p-5)
            0x3ff0c2c2efbe6960, // exp(0x1.7c8bb0cf5d16p-5)
            0x3ff23677186be250, // exp(0x1.0948d39a41695p-3)
            0x3ff39b8021bc065d, // exp(0x1.a065fefae814fp-3)
            0x3ff47408cb9583ce, // exp(0x1.f6e4c3ced7c72p-3)
            0x3ff512b3126454f3, // exp(0x1.1a0408712e00ap-2)
            0x3ff8b367381d82f5, // exp(0x1.bcab27d05abdep-2)
            0x3ffa65d89abf3d1f, // exp(0x1.005ae04256babp-1)
            0x40593295a96ec6eb, // exp(0x1.273c188aa7b14p+2)
            0x407ac50b409c8aee, // exp(0x1.83d4bcdebb3f4p+2)
            0x416daac459b157e5, // exp(0x1.08f51434652c3p+4)
            0x418a8c02e974c315, // exp(0x1.1d5c2daebe367p+4)
            0x427b890ca8637ae2, // exp(0x1.c44ce0d716a1ap+4)
            0x45591ec4412c344f, // exp(0x1.e07e71bfcf06fp+5)
            0x459a97e7be23e65a, // exp(0x1.f7216c4b435c9p+5)
            0x4f4c90810d354618, // exp(0x1.54cd1fea7663ap+7)
            0x6a562a88613629b6, // exp(0x1.d6479eba7c971p+8)
            0x3fefff4cde6a0bfb, // exp(-0x1.664716b68a409p-14)
            0x3feffe5d0bb7eabf, // exp(-0x1.a2fefefd580dfp-13)
            0x3feffc63b5617d47, // exp(-0x1.ce3f638d0c742p-12)
            0x3feffc6235eee28d, // exp(-0x1.ceff32831e2c2p-12)
            0x3feffb31a941b9b1, // exp(-0x1.33accae78b371p-11)
            0x3feff8a28e429f33, // exp(-0x1.d792b60084f92p-11)
            0x3fefd02d98c24bbb, // exp(-0x1.7fb235d76cce7p-8)
            0x3fefb85251a3f26f, // exp(-0x1.1ff9b8e8b38bep-7)
            0x3fefab5c6e464e0d, // exp(-0x1.54511e930898cp-7)
            0x3fef53a751d7db49, // exp(-0x1.5c5ed0ec83666p-6)
            0x3fef3c35328f1d5d, // exp(-0x1.8c56ff5326197p-6)
            0x3fef309f46111221, // exp(-0x1.a4187f2ca71f9p-6)
            0x3fecd8abd4de5c33, // exp(-0x1.a8f783d749a8fp-4)
            0x3fecb4287f11060a, // exp(-0x1.bd44fdaed819fp-4)
            0x3fec7f14af0a08eb, // exp(-0x1.daf693d64fadap-4)
            0x3febaded30cbf1c4, // exp(-0x1.290ea09e36479p-3)
            0x3fea634ae87df6ae, // exp(-0x1.8aeb636f3ce35p-3)
            0x3fe976a4c9985f5b, // exp(-0x1.d3f3799439415p-3)
            0x3fe9309142b73ea6, // exp(-0x1.ea16274b0109bp-3)
            0x3fe2217147b85eaa, // exp(-0x1.22e24fa3d5cf9p-1)
            0x3fddefa8f4a8af21, // exp(-0x1.85068c07fbbf6p-1)
            0x3fdacb8cf13bc769, // exp(-0x1.bdc7955d1482cp-1)
            0x3fd3ef1e9b3a81c8, // exp(-0x1.2a9cad9998262p+0)
            0x3fc534d4de870713, // exp(-0x1.cc37ef7de7501p+0)
            0x3fc1064b2c103ddb, // exp(-0x1.02393d5976769p+1)
            0x3faf78a60182b74d, // exp(-0x1.65061daf79a78p+1)
            0x3e8f3e558cf4de54, // exp(-0x1.e8bdbfcd9144ep+3)
            0x3daf80aafa92b498, // exp(-0x1.8f80e06f3a04cp+4)
            0x3822c0fa76a0e15f, // exp(-0x1.59f038076039cp+6)
            0x2d88c0d4140c77b7, // exp(-0x1.981587ad4542fp+7)
        ];
        assert!(
            DB.windows(2).all(|w| w[0] < w[1]),
            "DB must be sorted for the binary search"
        );
        for (&x, &y) in DB.iter().zip(EXPECTED.iter()) {
            check(&[(x, y)]);
        }
    }
}

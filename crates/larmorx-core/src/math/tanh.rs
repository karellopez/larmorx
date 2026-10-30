// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded hyperbolic tangent for binary64 (CORE-MATH's `cr_tanh`), in pure Rust.
//!
//! glibc 2.35's `tanh` (fdlibm's, [`super::tanh_glibc`]) is not correctly rounded on some
//! arguments (GLIBC-06); this one is, on every input, so it gives the same bits on every
//! platform. larmorx's default where an original program calls the C library's `tanh` (BET's
//! smoothness weight, `specs/bet.md` §8.1); `reproduce` keeps glibc's bits.
//!
//! Port of CORE-MATH's `src/binary64/tanh/tanh.c` (sha256 `e89441340424…`, the checkout in
//! `<workspace>/tags/core-math`), MIT licence, Copyright (c) 2023-2026 Alexei Sibidanov, Cyprien
//! Peignier, Paul Zimmermann (the notice is reproduced below). Round to nearest only; errno and
//! the exception flags are not modelled. The operations and their order are kept exactly;
//! `__builtin_fma` is [`f64::mul_add`]; the hex-float constants are written as their exact bit
//! patterns (the tables transcribed mechanically by
//! `validation/investigations/cleanroom-bet/tanh-verify/coremath_tables.py`). Checked against
//! the C function (`-O2 -ffp-contract=off`) on CORE-MATH's 31,436 worst cases and their
//! negatives and on a large random sample (`validation/investigations/cleanroom-bet/tanh-verify/`).

// Copyright (c) 2023-2026 Alexei Sibidanov, Cyprien Peignier, Paul Zimmermann
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

#![allow(clippy::unreadable_literal, clippy::many_single_char_names)]

/// `f64` from its IEEE-754 bit pattern.
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
}

#[inline(always)]
fn fasttwosub(x: f64, y: f64) -> (f64, f64) {
    let s = x - y;
    let z = x - s;
    (s, z - y)
}

#[inline(always)]
fn muldd_acc(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let plh = xl * ch;
    let phl = xh * cl;
    let phh = xh * ch;
    let mut phh_rest = xh.mul_add(ch, -phh);
    phh_rest += phl + plh;
    fasttwosum(phh, phh_rest)
}

#[inline(always)]
fn mulddd_acc(xh: f64, xl: f64, c: f64) -> (f64, f64) {
    let pl = xl * c;
    let ph = xh * c;
    let mut ph_rest = xh.mul_add(c, -ph);
    ph_rest += pl;
    fasttwosum(ph, ph_rest)
}

/// `l` at input: the approximation of the polynomial's upper part (double arithmetic only).
#[inline(always)]
fn polydd(xh: f64, xl: f64, c: &[[f64; 2]], l: f64) -> (f64, f64) {
    let n = c.len();
    let mut i = n - 1;
    let (mut ch, t) = fasttwosum(c[i][0], l);
    let mut cl = t + c[i][1];
    while i > 0 {
        i -= 1;
        let (h, lo) = muldd_acc(xh, xl, ch, cl);
        ch = h;
        cl = lo;
        let (th, tl) = fasttwosum(c[i][0], ch);
        ch = th;
        cl = (cl + c[i][1]) + tl;
    }
    (ch, cl)
}

#[inline(always)]
fn as_exp_accurate(x: f64, t: f64, th: f64, tl: f64) -> (f64, f64) {
    const CH: [[f64; 2]; 3] = [
        [fb(0x3ff0000000000000), fb(0x3a16c16bd194535d)],
        [fb(0x3fe0000000000000), -fb(0x3a28259d904fd34f)],
        [fb(0x3fc5555555555555), fb(0x3c653e93e9f26e62)],
    ];
    let l2h = fb(0x3f262e42ff000000);
    let l2l = fb(0x3d0718432a1b0e26);
    let l2ll = fb(0x3999ff0342542fc3);
    let dx = x - l2h * t;
    let mut dxl = l2l * t;
    let dxll = l2ll * t + l2l.mul_add(t, -dxl);
    let dxh = dx + dxl;
    dxl = ((dx - dxh) + dxl) + dxll;
    let fl = dxh
        * (fb(0x3fa5555555555555) + dxh * (fb(0x3f811111113e93e9) + dxh * fb(0x3f56c16c169400a7)));
    let (fh, fl) = polydd(dxh, dxl, &CH, fl);
    let (fh, fl) = muldd_acc(dxh, dxl, fh, fl);
    let (fh, fl) = muldd_acc(th, tl, fh, fl);
    let zh = th + fh;
    let zl = (th - zh) + fh;
    let uh = zh + tl;
    let ul = ((zh - uh) + tl) + zl;
    let vh = uh + fl;
    let vl = ((uh - vh) + fl) + ul;
    (vh, vl)
}

/// `|x| < 0.25`.
#[inline(always)]
fn as_tanh_zero(x: f64) -> f64 {
    const CH: [[f64; 2]; 9] = [
        [-fb(0x3fd5555555555555), -fb(0x3c75555555554cc4)],
        [fb(0x3fc1111111111111), fb(0x3c4111110f8c0178)],
        [-fb(0x3faba1ba1ba1ba1c), fb(0x3c47917c1d676ff5)],
        [fb(0x3f9664f4882c10fa), -fb(0x3c09d5cb27c0af28)],
        [-fb(0x3f8226e355e6c23c), -fb(0x3c2c9674586913f3)],
        [fb(0x3f6d6d3d0e157db3), -fb(0x3be71376fa06ce94)],
        [-fb(0x3f57da36452b5e46), -fb(0x3beaba8d51bd9c00)],
        [fb(0x3f43558247faa32d), -fb(0x3bee0cfb423aedfd)],
        [-fb(0x3f2f57d76ea30928), -fb(0x3bcc30601213cae9)],
    ];
    const CL: [f64; 5] = [
        fb(0x3f1967e0a63ca836),
        -fb(0x3f0497b99d2a77d1),
        fb(0x3ef0ae346258cbde),
        -fb(0x3edaade68fb2f076),
        fb(0x3ec22e609bf8671f),
    ];
    let x2 = x * x;
    let x2l = x.mul_add(x, -x2);
    let y2 = x2 * (CL[0] + x2 * (CL[1] + x2 * (CL[2] + x2 * (CL[3] + x2 * CL[4]))));
    let (y1, y2) = polydd(x2, x2l, &CH, y2);
    let (y1, y2) = mulddd_acc(y1, y2, x);
    let (y1, y2) = muldd_acc(y1, y2, x2, x2l);
    let (y0, y1) = fasttwosum(x, y1);
    let (mut y1, y2) = fasttwosum(y1, y2);
    let mut t = y1.to_bits();
    if t & (!0u64 >> 12) == 0 {
        let w = y2.to_bits();
        if (w ^ t) >> 63 != 0 {
            t = t.wrapping_sub(1);
        } else {
            t = t.wrapping_add(1);
        }
        y1 = f64::from_bits(t);
        if x.abs() == fb(0x3fcac343b179fec4) {
            return as_tanh_database(x, y0 + y1);
        }
    }
    y0 + y1
}

fn as_tanh_database(x: f64, f: f64) -> f64 {
    const DB: [[f64; 3]; 12] = [
        [
            fb(0x3fcac343b179fec4),
            fb(0x3fca612499c53078),
            fb(0x3c60000000000000),
        ],
        [
            fb(0x3fd00764a988bf73),
            fb(0x3fcf676484c0703b),
            -fb(0x3970000000000000),
        ],
        [
            fb(0x3fd17d1e8a63711f),
            fb(0x3fd110e96a6c2d96),
            -fb(0x3960000000000000),
        ],
        [
            fb(0x3fd291c601a05276),
            fb(0x3fd210b7d0c03743),
            fb(0x3c70000000000000),
        ],
        [
            fb(0x3fd36f33d51c264d),
            fb(0x3fd2dbb7b1c91363),
            -fb(0x3950000000000000),
        ],
        [
            fb(0x3fd43eaea23649c3),
            fb(0x3fd39877ed028641),
            -fb(0x3c80000000000000),
        ],
        [
            fb(0x3fdd88d7550b2826),
            fb(0x3fdb9a3637366afd),
            fb(0x3c70000000000000),
        ],
        [
            fb(0x3fde611aa58ab608),
            fb(0x3fdc493dc899e4a6),
            -fb(0x3c80000000000000),
        ],
        [
            fb(0x3fe01efe7ac8c15d),
            fb(0x3fddc3fe1b524821),
            -fb(0x3970000000000000),
        ],
        [
            fb(0x3fe1005ec0bccabb),
            fb(0x3fdf20b1c8557ded),
            -fb(0x3c80000000000000),
        ],
        [
            fb(0x3fe33dfeb0fa4bfe),
            fb(0x3fe1372f9ee76e99),
            fb(0x3c80000000000000),
        ],
        [
            fb(0x3fe49f24ac5cac35),
            fb(0x3fe22c495ff06104),
            -fb(0x3970000000000000),
        ],
    ];
    let mut f = f;
    let (mut a, mut b) = (0i64, DB.len() as i64 - 1);
    let mut m = (a + b) / 2;
    let ax = x.abs();
    while a <= b {
        let e = &DB[m as usize];
        if e[0] < ax {
            a = m + 1;
        } else if e[0] == ax {
            let s = 1f64.copysign(x);
            f = s * e[1] + s * e[2];
            break;
        } else {
            b = m - 1;
        }
        m = (a + b) / 2;
    }
    f
}

/// CORE-MATH's `t0` (64 pairs `{low, high}`), as bits.
const T0: [[u64; 2]; 64] = [
    [0x0000000000000000, 0x3ff0000000000000],
    [0xbc719083535b085e, 0x3ff02c9a3e778061],
    [0x3c8d73e2a475b466, 0x3ff059b0d3158574],
    [0x3c6186be4bb28500, 0x3ff0874518759bc8],
    [0x3c98a62e4adc610a, 0x3ff0b5586cf9890f],
    [0x3c403a1727c57b52, 0x3ff0e3ec32d3d1a2],
    [0xbc96c51039449b3a, 0x3ff11301d0125b51],
    [0xbc932fbf9af1369e, 0x3ff1429aaea92de0],
    [0xbc819041b9d78a76, 0x3ff172b83c7d517b],
    [0x3c8e5b4c7b4968e4, 0x3ff1a35beb6fcb75],
    [0x3c9e016e00a2643c, 0x3ff1d4873168b9aa],
    [0x3c8dc775814a8494, 0x3ff2063b88628cd6],
    [0x3c99b07eb6c70572, 0x3ff2387a6e756238],
    [0x3c82bd339940e9da, 0x3ff26b4565e27cdd],
    [0x3c8612e8afad1256, 0x3ff29e9df51fdee1],
    [0x3c90024754db41d4, 0x3ff2d285a6e4030b],
    [0x3c86f46ad23182e4, 0x3ff306fe0a31b715],
    [0x3c932721843659a6, 0x3ff33c08b26416ff],
    [0xbc963aeabf42eae2, 0x3ff371a7373aa9cb],
    [0xbc75e436d661f5e2, 0x3ff3a7db34e59ff7],
    [0x3c8ada0911f09ebc, 0x3ff3dea64c123422],
    [0xbc5ef3691c309278, 0x3ff4160a21f72e2a],
    [0x3c489b7a04ef80d0, 0x3ff44e086061892d],
    [0x3c73c1a3b69062f0, 0x3ff486a2b5c13cd0],
    [0x3c7d4397afec42e2, 0x3ff4bfdad5362a27],
    [0xbc94b309d25957e4, 0x3ff4f9b2769d2ca7],
    [0xbc807abe1db13cac, 0x3ff5342b569d4f82],
    [0x3c99bb2c011d93ac, 0x3ff56f4736b527da],
    [0x3c96324c054647ac, 0x3ff5ab07dd485429],
    [0x3c9ba6f93080e65e, 0x3ff5e76f15ad2148],
    [0xbc9383c17e40b496, 0x3ff6247eb03a5585],
    [0xbc9bb60987591c34, 0x3ff6623882552225],
    [0xbc9bdd3413b26456, 0x3ff6a09e667f3bcd],
    [0xbc6bbe3a683c88aa, 0x3ff6dfb23c651a2f],
    [0xbc816e4786887a9a, 0x3ff71f75e8ec5f74],
    [0xbc90245957316dd4, 0x3ff75feb564267c9],
    [0xbc841577ee049930, 0x3ff7a11473eb0187],
    [0x3c705d02ba15797e, 0x3ff7e2f336cf4e62],
    [0xbc9d4c1dd41532d8, 0x3ff82589994cce13],
    [0xbc9fc6f89bd4f6ba, 0x3ff868d99b4492ed],
    [0x3c96e9f156864b26, 0x3ff8ace5422aa0db],
    [0x3c85cc13a2e3976c, 0x3ff8f1ae99157736],
    [0xbc675fc781b57ebc, 0x3ff93737b0cdc5e5],
    [0xbc9d185b7c1b85d0, 0x3ff97d829fde4e50],
    [0x3c7c7c46b071f2be, 0x3ff9c49182a3f090],
    [0xbc9359495d1cd532, 0x3ffa0c667b5de565],
    [0xbc9d2f6edb8d41e2, 0x3ffa5503b23e255d],
    [0x3c90fac90ef7fd32, 0x3ffa9e6b5579fdbf],
    [0x3c97a1cd345dcc82, 0x3ffae89f995ad3ad],
    [0xbc62805e3084d708, 0x3ffb33a2b84f15fb],
    [0xbc75584f7e54ac3a, 0x3ffb7f76f2fb5e47],
    [0x3c823dd07a2d9e84, 0x3ffbcc1e904bc1d2],
    [0x3c811065895048de, 0x3ffc199bdd85529c],
    [0x3c92884dff483cac, 0x3ffc67f12e57d14b],
    [0x3c7503cbd1e949dc, 0x3ffcb720dcef9069],
    [0xbc9cbc3743797a9c, 0x3ffd072d4a07897c],
    [0x3c82ed02d75b3706, 0x3ffd5818dcfba487],
    [0x3c9c2300696db532, 0x3ffda9e603db3285],
    [0xbc91a5cd4f184b5c, 0x3ffdfc97337b9b5f],
    [0x3c839e8980a9cc90, 0x3ffe502ee78b3ff6],
    [0xbc9e9c23179c2894, 0x3ffea4afa2a490da],
    [0x3c9dc7f486a4b6b0, 0x3ffefa1bee615a27],
    [0x3c99d3e12dd8a18a, 0x3fff50765b6e4540],
    [0x3c874853f3a5931e, 0x3fffa7c1819e90d8],
];
/// CORE-MATH's `t1` (64 pairs `{low, high}`), as bits.
const T1: [[u64; 2]; 64] = [
    [0x0000000000000000, 0x3ff0000000000000],
    [0x3c9ae8e38c59c72a, 0x3ff000b175effdc7],
    [0xbc57b5d0d58ea8f4, 0x3ff00162f3904052],
    [0x3c94115cb6b16a8e, 0x3ff0021478e11ce6],
    [0xbc8d7c96f201bb2e, 0x3ff002c605e2e8cf],
    [0x3c984711d4c35ea0, 0x3ff003779a95f959],
    [0xbc80484245243778, 0x3ff0042936faa3d8],
    [0xbc94b237da2025fa, 0x3ff004dadb113da0],
    [0xbc75e00e62d6b30e, 0x3ff0058c86da1c0a],
    [0x3c9a1d6cedbb9480, 0x3ff0063e3a559473],
    [0xbc94acf197a00142, 0x3ff006eff583fc3d],
    [0xbc6eaf2ea42391a6, 0x3ff007a1b865a8ca],
    [0x3c7da93f90835f76, 0x3ff0085382faef83],
    [0xbc86a79084ab093c, 0x3ff00905554425d4],
    [0x3c986364f8fbe8f8, 0x3ff009b72f41a12b],
    [0xbc882e8e14e3110e, 0x3ff00a6910f3b6fd],
    [0xbc84f6b2a7609f72, 0x3ff00b1afa5abcbf],
    [0xbc7e1a258ea8f71a, 0x3ff00bcceb7707ec],
    [0x3c74362ca5bc26f2, 0x3ff00c7ee448ee02],
    [0x3c9095a56c919d02, 0x3ff00d30e4d0c483],
    [0xbc6406ac4e81a646, 0x3ff00de2ed0ee0f5],
    [0x3c9b5a6902767e08, 0x3ff00e94fd0398e0],
    [0xbc991b2060859320, 0x3ff00f4714af41d3],
    [0x3c8427068ab22306, 0x3ff00ff93412315c],
    [0x3c9c1d0660524e08, 0x3ff010ab5b2cbd11],
    [0xbc9e7bdfb3204be8, 0x3ff0115d89ff3a8b],
    [0x3c8843aa8b9cbbc6, 0x3ff0120fc089ff63],
    [0xbc734104ee7edae8, 0x3ff012c1fecd613b],
    [0xbc72b6aeb6176892, 0x3ff0137444c9b5b5],
    [0x3c7a8cd33b8a1bb2, 0x3ff01426927f5278],
    [0x3c72edc08e5da99a, 0x3ff014d8e7ee8d2f],
    [0x3c857ba2dc7e0c72, 0x3ff0158b4517bb88],
    [0x3c9b61299ab8cdb8, 0x3ff0163da9fb3335],
    [0xbc990565902c5f44, 0x3ff016f0169949ed],
    [0x3c870fc41c5c2d54, 0x3ff017a28af25567],
    [0x3c94b9a6e145d76c, 0x3ff018550706ab62],
    [0xbc7008eff5142bfa, 0x3ff019078ad6a19f],
    [0xbc977669f033c7de, 0x3ff019ba16628de2],
    [0xbc909bb78eeead0a, 0x3ff01a6ca9aac5f3],
    [0x3c9371231477ece6, 0x3ff01b1f44af9f9e],
    [0x3c75e7626621eb5a, 0x3ff01bd1e77170b4],
    [0xbc9bc72b100828a4, 0x3ff01c8491f08f08],
    [0xbc6ce39cbbab8bbe, 0x3ff01d37442d5070],
    [0x3c816996709da2e2, 0x3ff01de9fe280ac8],
    [0xbc8c11f5239bf536, 0x3ff01e9cbfe113ef],
    [0x3c8e1d4eb5edc6b4, 0x3ff01f4f8958c1c6],
    [0xbc9afb99946ee3f0, 0x3ff020025a8f6a35],
    [0xbc98f06d8a148a32, 0x3ff020b533856324],
    [0xbc82bf310fc54eb6, 0x3ff02168143b0281],
    [0xbc9c95a035eb4176, 0x3ff0221afcb09e3e],
    [0xbc9491793e46834c, 0x3ff022cdece68c4f],
    [0xbc73e8d0d9c49090, 0x3ff02380e4dd22ad],
    [0xbc9314aa16278aa4, 0x3ff02433e494b755],
    [0x3c848daf888e9650, 0x3ff024e6ec0da046],
    [0x3c856dc8046821f4, 0x3ff02599fb483385],
    [0x3c945b42356b9d46, 0x3ff0264d1244c719],
    [0xbc7082ef51b61d7e, 0x3ff027003103b10e],
    [0x3c72106ed0920a34, 0x3ff027b357854772],
    [0xbc9fd4cf26ea5d0e, 0x3ff0286685c9e059],
    [0xbc909f8775e78084, 0x3ff02919bbd1d1d8],
    [0x3c564cbba902ca28, 0x3ff029ccf99d720a],
    [0x3c94383ef231d206, 0x3ff02a803f2d170d],
    [0x3c94a47a505b3a46, 0x3ff02b338c811703],
    [0x3c9e471202234680, 0x3ff02be6e199c811],
];

/// `(x, tanh(x))` as bits from CORE-MATH's C (`tanh-glibc-verify coremath`): one argument of
/// each branch, then every 1310th worst case of `tanh.wc`.
#[cfg(test)]
const RECORDED: &[(u64, u64)] = &[
    (0x39b4484bfeebc2a0, 0x39b4484bfeebc2a0),
    (0x3e112e0be826d695, 0x3e112e0be826d695),
    (0x3fb999999999999a, 0x3fb983d7795f413a),
    (0x3fc999999999999a, 0x3fc9439830b3a590),
    (0x3fd3333333333333, 0x3fd2a4dda7d914fa),
    (0x3ff0000000000000, 0x3fe85efab514f394),
    (0x4004000000000000, 0x3fef9258260a71c2),
    (0x400ccccccccccccd, 0x3feff3c6eda3a296),
    (0x400d99999999999a, 0x3feff5fdc948488c),
    (0x4014000000000000, 0x3fefff419668df11),
    (0x4028000000000000, 0x3feffffffff59f7c),
    (0x4033000000000000, 0x3fefffffffffffff),
    (0x3f6000b47bdb383f, 0x3f6000b32658e41f),
    (0x3fbb61dabc08bbc9, 0x3fbb473e4a1b96cb),
    (0x3e57137449123ec0, 0x3e57137449123ebf),
    (0x3e7ac6fe500ab577, 0x3e7ac6fe500ab55e),
    (0x3ea1708452fe1f17, 0x3ea1708452fe1d5d),
    (0x3ec6da3bbf3da14f, 0x3ec6da3bbf3d6327),
    (0x3f07d9c287de52af, 0x3f07d9c28797a8b5),
    (0x3f1cde51e29af3b1, 0x3f1cde51e0a5bb51),
    (0x3f3324da080bc7fa, 0x3f3324d9fee91628),
    (0x4022294a1a399af0, 0x3feffffff2152fb0),
    (0x3e7ec9baf60b9f8c, 0x3e7ec9baf60b9f66),
    (0x3ebbb3d1aed36cdc, 0x3ebbb3d1aed3512e),
    (0x3f3f3a1bfc2adb23, 0x3f3f3a1bd484c917),
    (0x3f59d3796b17f668, 0x3f59d378043a0ac0),
    (0x3f7500cd4b83a929, 0x3f7500c13b2a4545),
    (0x3f8d2952a1ea6ef6, 0x3f8d28d17be528c1),
    (0x3fa68174a0496fc8, 0x3fa67dbf6bd25f44),
    (0x3fd2ba55146c9962, 0x3fd23604e8b9e685),
    (0x402c15befae7a4cd, 0x3fefffffffffd34f),
    (0x3fbec2d8b2eadf46, 0x3fbe9d29e05d0cc5),
    (0x3fddf6771fb6c083, 0x3fdbf32fd3913c32),
    (0x3ff8fe9006256580, 0x3fed4df88d7e3a74),
    (0x401d28f81d481137, 0x3feffffe0c1b04e9),
    (0x3eef9e32186c4864, 0x3eef9e321861fe5e),
];

/// `tanh(x)`, correctly rounded (round to nearest).
#[inline(always)]
pub(crate) fn tanh(x: f64) -> f64 {
    let ax = x.abs();
    let aix = ax.to_bits();
    // for |x| >= 0x1.30fc1931f09cap+4, tanh(x) rounds to ±1
    if aix >= 0x40330fc1931f09ca {
        if aix > 0x7ff0000000000000 {
            return x + x; // NaN
        }
        let f = 1f64.copysign(x);
        if aix == 0x7ff0000000000000 {
            return f;
        }
        let df = fb(0x3c80000000000000).copysign(x);
        return f - df;
    }
    let s = -fb(0x40c71547652b82fe);
    let v0 = ax.mul_add(s, fb(0x4188000004000000));
    let jt = v0.to_bits();
    let t = f64::from_bits(jt & !((1u64 << 27) - 1)) - fb(0x4188000000000000);
    let i1 = ((jt >> 27) & 0x3f) as usize;
    let i0 = ((jt >> 33) & 0x3f) as usize;
    let ie = ((jt << 13) as i64) >> 52;
    let sp = f64::from_bits(((1023 + ie) as u64) << 52);
    let ch = [
        fb(0x4000000000000000),
        fb(0x4000000000000000),
        fb(0x3ff55555557e54ff),
        fb(0x3fe55555553a12f4),
    ];
    let t0h = fb(T0[i0][1]);
    let t1h = fb(T1[i1][1]);
    let mut th = t0h * t1h;
    let mut tl;
    if aix < 0x400d76c8b4395810 {
        // |x| ~< 3.683
        if aix < 0x3fd0000000000000 {
            // |x| < 2^-2
            if aix <= 0x3e4d12ed0af1a27f {
                // |x| <= 0x1.d12ed0af1a27fp-27
                if aix == 0 {
                    return x;
                }
                return x.mul_add(-fb(0x3c80000000000000), x);
            }
            const C: [f64; 8] = [
                -fb(0x3fd5555555555555),
                fb(0x3fc1111111110f33),
                -fb(0x3faba1ba1b9b8ea6),
                fb(0x3f9664f4838e0a43),
                -fb(0x3f8226e17d1bc09b),
                fb(0x3f6d6c64dfba2565),
                -fb(0x3f57bdd094d327af),
                fb(0x3f41535ad0c31d0e),
            ];
            let x2 = x * x;
            let x3 = x2 * x;
            let x4 = x2 * x2;
            let x8 = x4 * x4;
            let p1 = (C[4] + x2 * C[5]) + x4 * (C[6] + x2 * C[7]);
            let mut p0 = (C[0] + x2 * C[1]) + x4 * (C[2] + x2 * C[3]);
            p0 += x8 * p1;
            p0 *= x3;
            let (rh, rl) = fasttwosum(x, p0);
            let e = x3 * fb(0x3cbc000000000000);
            let lb = rh + (rl - e);
            let ub = rh + (rl + e);
            if lb == ub {
                return lb;
            }
            return as_tanh_zero(x);
        }
        let t0l = fb(T0[i0][0]);
        let t1l = fb(T1[i1][0]);
        tl = t0h * t1l + t1h * t0l + t0h.mul_add(t1h, -th);
        th *= sp;
        tl *= sp;
        let l2h = -fb(0x3f162e42ff000000);
        let l2l = -fb(0x3cf718432a1b0e26);
        let dx = (l2h * t - ax) - l2l * t;
        let dx2 = dx * dx;
        let p = dx * ((ch[0] + dx * ch[1]) + dx2 * (ch[2] + dx * ch[3]));
        let rh = th;
        let rl = tl + rh * p;
        let (rh, rl) = fasttwosum(rh, rl);

        let (ph, pl) = (rh, rl);
        let (qh, qd) = fasttwosum(1.0, rh);
        let ql = rl + qd;

        let rqh = 1.0 / qh;
        let rql = (ql * rqh + rqh.mul_add(qh, -1.0)) * -rqh;
        let (ph, pl) = muldd_acc(ph, pl, rqh, rql);

        let e = rh * fb(0x3c10d00000000000);
        let (mut rh, mut rl) = fasttwosub(0.5, ph);
        rl -= pl;
        rh *= 2f64.copysign(x);
        rl *= 2f64.copysign(x);
        let lb = rh + (rl - e);
        let ub = rh + (rl + e);
        if lb == ub {
            return lb;
        }
    } else {
        // 3.683 ~< x < 0x1.30fc1931f09cap+4
        let l2 = -fb(0x3f162e42fefa39ef);
        let dx = l2.mul_add(t, -ax);
        let dx2 = dx * dx;
        let p = dx * ((ch[0] + dx * ch[1]) + dx2 * (ch[2] + dx * ch[3]));
        let mut rh = th * sp;
        rh += (p + ((2.0 * fb(0x3c83000000000000)) * ax)) * rh;
        let e = rh * fb(0x3ce1000000000000);
        rh = (2.0 * rh) / (1.0 + rh);
        let one = 1f64.copysign(x);
        rh = rh.copysign(x);
        let lb = one - (rh + e);
        let ub = one - (rh - e);
        if lb == ub {
            return lb;
        }
        let t0l = fb(T0[i0][0]);
        let t1l = fb(T1[i1][0]);
        tl = t0h * t1l + t1h * t0l + t0h.mul_add(t1h, -th);
        th *= sp;
        tl *= sp;
    }
    let (rh, rl) = as_exp_accurate(-2.0 * ax, t, th, tl);
    let (qh, qd) = fasttwosum(1.0, rh);
    let ql = rl + qd;
    let (qh, ql) = fasttwosum(qh, ql);
    let rqh = 1.0 / qh;
    let rql = (ql * rqh + rqh.mul_add(qh, -1.0)) * -rqh;
    let (ph, pl) = muldd_acc(rh, rl, rqh, rql);
    let (rh, rl) = fasttwosub(0.5, ph);
    let rl = rl - pl;
    let (rh, rl) = fasttwosum(rh, rl);
    let res = 2f64.copysign(x) * rh + 2f64.copysign(x) * rl;
    let lu = rl.to_bits();
    if (lu.wrapping_add(32) & (!0u64 >> 12)) < 65 {
        return as_tanh_database(x, res);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::tanh;

    #[test]
    fn special_values() {
        assert_eq!(tanh(0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(tanh(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(tanh(f64::INFINITY), 1.0);
        assert_eq!(tanh(f64::NEG_INFINITY), -1.0);
        assert!(tanh(f64::NAN).is_nan());
        assert_eq!(tanh(30.0), 1.0);
        assert_eq!(tanh(1e-300), 1e-300);
    }

    /// `(x, tanh(x))` as bits from CORE-MATH's C (`-O2 -ffp-contract=off`): one argument of
    /// each branch and worst cases of `tanh.wc`.
    #[test]
    fn matches_core_math() {
        for &(x, y) in super::RECORDED {
            assert_eq!(
                tanh(f64::from_bits(x)).to_bits(),
                y,
                "tanh({:e})",
                f64::from_bits(x)
            );
            assert_eq!(
                tanh(-f64::from_bits(x)).to_bits(),
                (-f64::from_bits(y)).to_bits()
            );
        }
    }
}

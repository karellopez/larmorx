// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded `log(1 + x)` of binary64 values: [`log1p`].
//!
//! `log1p(x)` is the binary64 value nearest to the exact ln(1 + x) (round-to-nearest,
//! ties-to-even), the same on every platform.
//!
//! Port of CORE-MATH's `src/binary64/log1p/log1p.c` (`cr_log1p`, Alexei Sibidanov's) at commit
//! 040ee482a8ca (MIT licence, © its author below), re-expressed in Rust for round-to-nearest
//! only. The algorithm and the order of every floating-point operation are CORE-MATH's: a fast
//! path (a double-double approximation from a 64-entry table near 1/sqrt(2)..sqrt(2) and
//! polynomials, with an error bound `eps`), and when its two bounds round differently the
//! accurate path (`as_log1p_refine`: a quadripartite table of 4 x 16 inverses and their
//! triple-double logarithms, double-double polynomials, a final correction of exact halves).
//!
//! Differences from the C code, none of which changes a non-NaN result in round-to-nearest:
//! rounding-mode, `errno` and floating-point-exception handling are dropped; `__builtin_fma` is
//! `f64::mul_add` (a correctly rounded fused multiply-add on every target),
//! `__builtin_roundeven` is `f64::round_ties_even`; hex-float constants are written as their
//! exact bit patterns; and for `x < -1` the result is `f64::NAN`, where C's `0.0 / 0.0` returns
//! the target's default NaN.
//
// Original copyright and licence (MIT) notice of log1p.c:
//
//   Copyright (c) 2024-2026 Alexei Sibidanov <sibid@uvic.ca>.
//
//   This file is part of the CORE-MATH project
//   (https://core-math.gitlabpages.inria.fr/).
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy
//   of this software and associated documentation files (the "Software"), to deal
//   in the Software without restriction, including without limitation the rights
//   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//   copies of the Software, and to permit persons to whom the Software is
//   furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all
//   copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//   SOFTWARE.

// `rt` and `ln`: the accurate path's quadripartite tables (-ln(rt[][]) = ln[][][2] + ln[][][1] +
// ln[][][0]); `rf` and `lf`: the fast path's (-ln(rf[]) = lf[][1] + lf[][0]).
#[rustfmt::skip]
static RT: [[f64; 16]; 4] = [[b(0x3ff6a09e68000000), b(0x3ff5ab07e0000000), b(0x3ff4bfdad8000000), b(0x3ff3dea650000000), b(0x3ff306fe08000000), b(0x3ff2387a70000000), b(0x3ff172b840000000), b(0x3ff0b55870000000), b(0x3ff0000000000000), b(0x3feea4afa0000000), b(0x3fed5818e0000000), b(0x3fec199be0000000), b(0x3feae89f98000000), b(0x3fe9c49180000000), b(0x3fe8ace540000000), b(0x3fe7a11470000000)], [b(0x3ff059b0d0000000), b(0x3ff04e5f70000000), b(0x3ff04315e8000000), b(0x3ff037d430000000), b(0x3ff02c9a40000000), b(0x3ff0216818000000), b(0x3ff0163da8000000), b(0x3ff00b1af8000000), b(0x3ff0000000000000), b(0x3fefe9d968000000), b(0x3fefd3c228000000), b(0x3fefbdba38000000), b(0x3fefa7c180000000), b(0x3fef91d800000000), b(0x3fef7bfdb0000000), b(0x3fef663278000000)], [b(0x3ff0058c88000000), b(0x3ff004dad8000000), b(0x3ff0042938000000), b(0x3ff0037798000000), b(0x3ff002c608000000), b(0x3ff0021478000000), b(0x3ff00162f0000000), b(0x3ff000b178000000), b(0x3ff0000000000000), b(0x3feffe9d20000000), b(0x3feffd3a58000000), b(0x3feffbd798000000), b(0x3feffa74e8000000), b(0x3feff91248000000), b(0x3feff7afb8000000), b(0x3feff64d38000000)], [b(0x3ff00058b8000000), b(0x3ff0004da0000000), b(0x3ff0004288000000), b(0x3ff0003778000000), b(0x3ff0002c60000000), b(0x3ff0002148000000), b(0x3ff0001630000000), b(0x3ff0000b18000000), b(0x3ff0000000000000), b(0x3fefffe9d0000000), b(0x3fefffd3a0000000), b(0x3fefffbd78000000), b(0x3fefffa748000000), b(0x3fefff9118000000), b(0x3fefff7ae8000000), b(0x3fefff64c0000000)]];

#[rustfmt::skip]
static LN: [[[f64; 3]; 16]; 4] = [[[b(0x3a061fa45b636ea2), b(0xbd2497697cb31340), b(0xbfd62e43033a8000)], [b(0xba242b671e5e6d5e), b(0x3d3fc50ee605bd80), b(0xbfd3687aa721c000)], [b(0x3a00cc2ea968d329), b(0x3d4f48666c24eac0), b(0xbfd0a2b247d58000)], [b(0xba2963904db0a34e), b(0xbd458495e50d36a0), b(0xbfcbb9d3d8080000)], [b(0x3a0337e0d9f81652), b(0x3d2c89a0bede9780), b(0xbfc62e42f0378000)], [b(0xba01bdf53871d50b), b(0x3d52d67e8a0c1d70), b(0xbfc0a2b24a100000)], [b(0xb9d781dd6fc40530), b(0x3d507a22a9e81ed8), b(0xbfb62e43327c0000)], [b(0xb9f84e4562b8f2f3), b(0xbd4868afe1489500), b(0xbfa62e435bae0000)], [b(0x0000000000000000), b(0x0000000000000000), b(0x0000000000000000)], [b(0xb9f17e3ec05cde70), b(0x3d5174a196895690), b(0x3fa62e432b220000)], [b(0xba08067ec2a27738), b(0x3d4111a4eadf3130), b(0x3fb62e42e4a80000)], [b(0xba260bef06b14405), b(0x3d4dd4ec4e1d4210), b(0x3fc0a2b233f08000)], [b(0xba1b18e160362c24), b(0x3d26bd65e8b0b700), b(0x3fc62e43056c0000)], [b(0xba2c6ac3f1862a6b), b(0xbd30f5aad509ea80), b(0x3fcbb9d3cbd68000)], [b(0xba2dead1a4581acf), b(0xbd4ac842de005640), b(0x3fd0a2b244da4000)], [b(0xba296b1f2f60e3e4), b(0x3d59cec9a50db228), b(0x3fd3687aa9b78000)]], [[b(0x39fdbd93cdc08613), b(0x3d396f39c49fa8a0), b(0xbf962e423dd40000)], [b(0xba13c008f058a015), b(0xbd4f26429af65b70), b(0xbf936879e5140000)], [b(0x3a2f96a86136c1d1), b(0xbd5bfc5afb2bd378), b(0xbf90a2b2240c0000)], [b(0x3a23917cea885be0), b(0xbd1421e0c2976380), b(0xbf8bb9d4b2880000)], [b(0x3a2f0f1b5d989675), b(0x3d592a7506c90a18), b(0xbf862e43c1200000)], [b(0xba2f7f8d6a190051), b(0xbd594925ae0e5240), b(0xbf80a2b41dd00000)], [b(0x3a1ccb0919263869), b(0xbd50fb337ff2cf28), b(0xbf762e4106800000)], [b(0x3a2c9a5efe4b7c6c), b(0xbd3966c27788d1c0), b(0xbf662e3e4cc00000)], [b(0x0000000000000000), b(0x0000000000000000), b(0x0000000000000000)], [b(0xba040288ccae8f0f), b(0xbd5f3c5ff88d19c8), b(0x3f662e462b600000)], [b(0xba1f7e788a871350), b(0xbd4a9610028771c0), b(0x3f762e44c9300000)], [b(0x3a2574cc6d3f577d), b(0x3d52a3a1a65aa398), b(0x3f80a2b1e3300000)], [b(0xb9c56bb79b254f33), b(0xbd34a995b6d9ddc0), b(0x3f862e4367c00000)], [b(0x3a1c4c209ca6783d), b(0xbd4d7b98ef459110), b(0x3f8bb9d449a80000)], [b(0x3a2e9731de7f0155), b(0xbd4de722390bbd60), b(0x3f90a2b1f1940000)], [b(0x39ffadc62522444d), b(0xbd2fd95cb835e380), b(0x3f93687ad1140000)]], [[b(0xba01f72d2a6a460e), b(0x3d4a25045c37c330), b(0xbf562e4795000000)], [b(0x3a22847e318fd3ff), b(0xbd5c633239e1cdc8), b(0xbf53686e5dc00000)], [b(0x3a263f5750b9d826), b(0xbd5cb15590f1cd78), b(0xbf50a2b653800000)], [b(0xba11382d6395c24c), b(0x3d47636a5400e2b0), b(0xbf4bb9bf13800000)], [b(0x3a1be4491ec20322), b(0x3d5d0c5e2c9b6be8), b(0xbf462e53e5000000)], [b(0xba2706f04cc2c9f6), b(0xbd5f3bc0ce9b9a08), b(0xbf40a2ab37000000)], [b(0x3a2ab6f41df01d7f), b(0x3d3a7cbc9a97ba40), b(0xbf362e0a00000000)], [b(0xb9b67e487663ca3e), b(0xbd4957976dc5f360), b(0xbf262e84fc000000)], [b(0x0000000000000000), b(0x0000000000000000), b(0x0000000000000000)], [b(0x3a29681e48dde135), b(0xbd4868625640a690), b(0x3f262e7b00000000)], [b(0x3a2a2948cd558655), b(0xbd52ee3d96b696a0), b(0x3f362e35f6000000)], [b(0xb9fcfc26ccf6d0e4), b(0x3d153edbcf116500), b(0x3f40a2b4b2000000)], [b(0x3a2f68d24b9e338d), b(0x3cc783e334613000), b(0x3f462e4be1000000)], [b(0xba0f33369bf7dff1), b(0xbd560785f20acb20), b(0x3f4bb9e085000000)], [b(0xba0685a35575eff1), b(0xbcf5a62ec6656800), b(0x3f50a2b94d400000)], [b(0xba04c4d1abca79bf), b(0x3d17ded26dc81300), b(0x3f5368810f800000)]], [[b(0x3a140676dea39b19), b(0x3d525337681fa900), b(0xbf162dc284000000)], [b(0xba1ec76c964ac8d6), b(0xbd5504032e2ed388), b(0xbf1367d0ec000000)], [b(0x3a08ecaeb6d9523c), b(0x3d3c28fde83047e0), b(0xbf10a1dd6c000000)], [b(0xba2e4069214576be), b(0x3d558f2757976ef8), b(0xbf0bbbcff0000000)], [b(0x3a286146d1f9b91f), b(0xbd5d4717ca0a3230), b(0xbf062fe138000000)], [b(0x3a21eb44b07102c5), b(0xbd486ffcda252780), b(0xbf00a3eeb0000000)], [b(0xba295bbb5fd5a466), b(0x3d41ce399729e7b0), b(0xbef62ff0a0000000)], [b(0xba279059023f8767), b(0xbd5dbf1c6a400408), b(0xbee62ff840000000)], [b(0x0000000000000000), b(0x0000000000000000), b(0x0000000000000000)], [b(0xba2b4cb08d6fd48f), b(0xbd5db0e38e5aaa98), b(0x3ee63007c0000000)], [b(0xba20e6a88bfc3838), b(0x3d42b1c755804390), b(0x3ef6300f60000000)], [b(0x3a18074feacfe49d), b(0x3d5401bb919f14e0), b(0x3f00a21148000000)], [b(0xba2f56f5f168db15), b(0xbd385d6f6487ce20), b(0x3f062e1ec0000000)], [b(0x3a29eae7e05a0143), b(0xbd5af5d58a7c9218), b(0x3f0bba3010000000)], [b(0xba1a859095999ae3), b(0x3d5590faa0883bd8), b(0x3f10a32298000000)], [b(0x3a24f787e495e5e0), b(0xbd1fbb791220a180), b(0x3f13682f14000000)]]];

#[rustfmt::skip]
static RF: [f64; 64] = [b(0x3ff6816818000000), b(0x3ff642c858000000), b(0x3ff6058160000000), b(0x3ff5c98828000000), b(0x3ff58ed230000000), b(0x3ff5555558000000), b(0x3ff51d07e8000000), b(0x3ff4e5e0a8000000), b(0x3ff4afd6a0000000), b(0x3ff47ae148000000), b(0x3ff446f868000000), b(0x3ff4141418000000), b(0x3ff3e22cc0000000), b(0x3ff3b13b10000000), b(0x3ff3813810000000), b(0x3ff3521cf8000000), b(0x3ff323e348000000), b(0x3ff2f684c0000000), b(0x3ff2c9fb50000000), b(0x3ff29e4128000000), b(0x3ff27350b8000000), b(0x3ff2492490000000), b(0x3ff21fb780000000), b(0x3ff1f70480000000), b(0x3ff1cf06b0000000), b(0x3ff1a7b960000000), b(0x3ff1811810000000), b(0x3ff15b1e60000000), b(0x3ff135c810000000), b(0x3ff1111110000000), b(0x3ff0ecf568000000), b(0x3ff0c97150000000), b(0x3ff0a68108000000), b(0x3ff0842108000000), b(0x3ff0624dd0000000), b(0x3ff0410410000000), b(0x3ff0204080000000), b(0x3ff0000000000000), b(0x3fef81f820000000), b(0x3fef07c1f0000000), b(0x3fee9131a8000000), b(0x3fee1e1e20000000), b(0x3fedae6078000000), b(0x3fed41d420000000), b(0x3fecd85688000000), b(0x3fec71c720000000), b(0x3fec0e0700000000), b(0x3febacf918000000), b(0x3feb4e81b8000000), b(0x3feaf286c0000000), b(0x3fea98ef60000000), b(0x3fea41a418000000), b(0x3fe9ec8e98000000), b(0x3fe9999998000000), b(0x3fe948b100000000), b(0x3fe8f9c190000000), b(0x3fe8acb910000000), b(0x3fe8618618000000), b(0x3fe8181818000000), b(0x3fe7d05f40000000), b(0x3fe78a4c80000000), b(0x3fe745d178000000), b(0x3fe702e060000000), b(0x3fe6c16c18000000)];

#[rustfmt::skip]
static LF: [[f64; 2]; 64] = [[b(0xbd5f2f8281bade6a), b(0xbfd5d5bde3994000)], [b(0x3d5c2843fdd367a4), b(0xbfd522ae0438c000)], [b(0xbd306c10c34c14b0), b(0xbfd4718dc171c000)], [b(0x3d4cfa4e853f5890), b(0xbfd3c2526cb34000)], [b(0xbd5ce3ac179bd856), b(0xbfd314f1e0534000)], [b(0xbd5b91f82deb8122), b(0xbfd269621934c000)], [b(0x3d546bbb83d7163e), b(0xbfd1bf995a9a8000)], [b(0x3d5b842e5a74bdb0), b(0xbfd1178e84a80000)], [b(0xbd5862715e5bb534), b(0xbfd071385f4d4000)], [b(0xbd59bcbcbea0cdf8), b(0xbfcf991c6eb38000)], [b(0xbd401101cb605958), b(0xbfce530f10670000)], [b(0x3d50c38c81ad8f06), b(0xbfcd10380b658000)], [b(0x3d53aa40992a6d82), b(0xbfcbd0874c3c0000)], [b(0x3d530f68780ae82e), b(0xbfca93ed248b0000)], [b(0xbd07d116989d0980), b(0xbfc95a5ac5f70000)], [b(0xbd51e0012ba619ca), b(0xbfc823c150518000)], [b(0x3d454535d5671858), b(0xbfc6f0127cf58000)], [b(0xbd5ed87db3498128), b(0xbfc5bf407b540000)], [b(0xbd5aafde9c9fc39a), b(0xbfc4913d94338000)], [b(0xbd4015868c234000), b(0xbfc365fca3158000)], [b(0x3d5eff33f502c226), b(0xbfc23d7126ca0000)], [b(0x3d4b8521e874d358), b(0xbfc1178e72280000)], [b(0x3d454d75afe84568), b(0xbfbfe89129dc0000)], [b(0xbd51a813f3fa7c1e), b(0xbfbda72783840000)], [b(0xbd56c6676f40963e), b(0xbfbb6ac8afad0000)], [b(0xbd52620b7957a7a6), b(0xbfb9335e4d590000)], [b(0x3d4f8ffee5598f38), b(0xbfb700d2f4eb0000)], [b(0xbd5fab0f5bf42ca2), b(0xbfb4d31165200000)], [b(0xbd37a3e970b1c3a8), b(0xbfb2aa0492470000)], [b(0xbd4d030435fecb50), b(0xbfb08598a59e0000)], [b(0x3d435084a4fb8ab8), b(0xbfaccb7357de0000)], [b(0x3d432f36d60b44c4), b(0xbfa894aa1ca00000)], [b(0x3d2c1bcce5be8110), b(0xbfa466ae8a2e0000)], [b(0x3d5777740b18714a), b(0xbfa0415d81e80000)], [b(0xbd4955c057693d94), b(0xbf98492470c80000)], [b(0x3d44f71addb8be00), b(0xbf90205648940000)], [b(0xbd3bcda4e198afb0), b(0xbf801014f5880000)], [b(0x3bcc000000000000), b(0x0000000000000000)], [b(0xbd5fe0df75092c5e), b(0x3f8fc0a891000000)], [b(0x3d298036ec7e0a10), b(0x3f9f829b1e780000)], [b(0x3d5ba010f49e5ff0), b(0x3fa7745938320000)], [b(0xbd53ab13c266d328), b(0x3faf0a30a0120000)], [b(0xbd471798573e45d4), b(0x3fb341d78b1c0000)], [b(0x3d5ad32f072669fc), b(0x3fb6f0d272e50000)], [b(0xbd454e391e16ea38), b(0x3fba926d434b0000)], [b(0xbd2a302bbaf05590), b(0x3fbe27074e2b0000)], [b(0x3d3cb4cd66e31f30), b(0x3fc0d77e8cd08000)], [b(0xbd35b7a5bc474128), b(0x3fc29552e9200000)], [b(0xbd17062e8135f740), b(0x3fc44d2b5e4b8000)], [b(0x3d43d4c88fe1f4b0), b(0x3fc5ff3060a78000)], [b(0xbd537b70004a6946), b(0x3fc7ab8904110000)], [b(0xbd54a5885167c1ec), b(0x3fc9525aa7f48000)], [b(0x3d5ff9d5953004ac), b(0x3fcaf3c940008000)], [b(0x3d4a21ec41d8219c), b(0x3fcc8ff7cf9a8000)], [b(0xbd3a322bf2f02ae8), b(0x3fce27075e2b0000)], [b(0x3d5f1548b8a33616), b(0x3fcfb9186b5e0000)], [b(0x3d50e36401f7a006), b(0x3fd0a324e0f38000)], [b(0xbd59f1fa55382a8a), b(0x3fd1675cacabc000)], [b(0xbd3a69763deb0960), b(0x3fd22941fc0f8000)], [b(0x3d5d30bc3ac91bda), b(0x3fd2e8e2bee10000)], [b(0x3d37a79cf4d73b28), b(0x3fd3a64c59694000)], [b(0x3d5ec345197b22de), b(0x3fd4618bb81c4000)], [b(0xbd3f4810a30aeba8), b(0x3fd51aad7c2e0000)], [b(0x3d4394d2371c1d1c), b(0x3fd5d1bdbbd80000)]];

#[inline(always)]
const fn b(x: u64) -> f64 {
    f64::from_bits(x)
}

#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
}

#[inline(always)]
fn twosum(xh: f64, ch: f64) -> (f64, f64) {
    let s = xh + ch;
    let d = s - xh;
    (s, (ch - d) + (xh + (d - s)))
}

#[inline(always)]
fn fastsum(xh: f64, xl: f64, yh: f64, yl: f64) -> (f64, f64) {
    let (sh, sl) = fasttwosum(xh, yh);
    (sh, (xl + yl) + sl)
}

#[inline(always)]
fn sum(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let (sh, sl) = twosum(xh, ch);
    (sh, (xl + cl) + sl)
}

#[inline(always)]
fn muldd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahhh = ch * xh;
    (ahhh, (cl * xh + ch * xl) + ch.mul_add(xh, -ahhh))
}

#[inline(always)]
fn mulddd(x: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahhh = ch * x;
    (ahhh, cl * x + ch.mul_add(x, -ahhh))
}

/// `polydd(xh, xl, n, c, l)`: `l` in, the low part out.
fn polydd(xh: f64, xl: f64, c: &[[f64; 2]], l: f64) -> (f64, f64) {
    let mut i = c.len() - 1;
    let (mut ch, l1) = fasttwosum(c[i][0], l);
    let mut cl = c[i][1] + l1;
    while i > 0 {
        i -= 1;
        let (h, lo) = muldd(xh, xl, ch, cl);
        let (h2, lo2) = fastsum(c[i][0], c[i][1], h, lo);
        ch = h2;
        cl = lo2;
    }
    (ch, cl)
}

fn polyddd(x: f64, c: &[[f64; 2]], l: f64) -> (f64, f64) {
    let mut i = c.len() - 1;
    let (mut ch, l1) = fasttwosum(c[i][0], l);
    let mut cl = c[i][1] + l1;
    while i > 0 {
        i -= 1;
        let (h, lo) = mulddd(x, ch, cl);
        let (h2, lo2) = fastsum(c[i][0], c[i][1], h, lo);
        ch = h2;
        cl = lo2;
    }
    (ch, cl)
}

/// The correctly rounded `log(1 + x)` (CORE-MATH's `cr_log1p`, round-to-nearest).
pub fn log1p(x: f64) -> f64 {
    let ix = x.to_bits();
    let ax = ix << 1;
    let (ln1, ln0, eps);
    if ax < 0x7f60_0000_0000_0000 {
        // |x| < 0.0625
        if ax < 0x7940_0000_0000_0000 {
            // |x| < 2^-53
            if ax == 0 {
                return x;
            }
            return x.abs().mul_add(b(0xbc90000000000000), x);
        }
        let x2 = x * x;
        if ax < 0x7e60_0000_0000_0000 {
            // |x| < 2^-12
            ln1 = x;
            eps = b(0x3bfa000000000000) * x;
            if ax < 0x7d43_3600_0000_0000 {
                // |x| < 0x1.19bp-21
                let c = [b(0xbfe00000000001d1), b(0x3fd55555555558f7)];
                ln0 = x2 * (c[0] + x * c[1]);
            } else {
                let c = [
                    b(0xbfdffffffffffffd),
                    b(0x3fd5555555555551),
                    b(0xbfd000000d5555e1),
                    b(0x3fc99999b442f73f),
                ];
                ln0 = x2 * ((c[0] + x * c[1]) + x2 * (c[2] + x * c[3]));
            }
        } else {
            let c = [
                b(0x3fd5555555555555),
                b(0xbfd0000000000000),
                b(0x3fc9999999999b41),
                b(0xbfc555555555583b),
                b(0x3fc24924923f39e0),
                b(0xbfbfffffffe42e43),
                b(0x3fbc71c75511d70b),
                b(0xbfb99999de10510f),
                b(0x3fb7457e81b175f6),
                b(0xbfb554fb43e54e0f),
                b(0x3fb3ed68744f3d18),
                b(0xbfb28558ad5a7ac4),
            ];
            let x3 = x2 * x;
            let x4 = x2 * x2;
            let hx = -0.5 * x;
            ln1 = hx.mul_add(x, x);
            let mut l0 = hx.mul_add(x, x - ln1);
            let f = ((c[0] + x * c[1]) + x2 * (c[2] + x * c[3]))
                + x4 * (((c[4] + x * c[5]) + x2 * (c[6] + x * c[7]))
                    + x4 * ((c[8] + x * c[9]) + x2 * (c[10] + x * c[11])));
            l0 += x3 * f;
            ln0 = l0;
            eps = x3 * b(0x3cbb600000000000);
        }
    } else {
        // |x| >= 0.0625
        let c = [
            b(0xbfe000000000003d),
            b(0x3fd5555555554cf5),
            b(0xbfcffffffeca2939),
            b(0x3fc99999a3661724),
            b(0xbfc555d345bfe6fd),
            b(0x3fc247b887a6e5ed),
        ];
        let (mut t, mut dt);
        if (ix as i64) < 0x4340_0000_0000_0000 && ix < 0xbff0_0000_0000_0000 {
            let (s, e) = fasttwosum(1.0, x);
            t = s;
            dt = e;
        } else if ix < 0x4690_0000_0000_0000 {
            // x < 2^106
            t = x;
            dt = 1.0;
        } else if ix < 0x7ff0_0000_0000_0000 {
            // x < 2^1024
            t = x;
            dt = 0.0;
        } else {
            if ax > 0xffe0_0000_0000_0000 {
                return x + x; // NaN
            }
            if ix == 0x7ff0_0000_0000_0000 {
                return x; // +inf
            }
            if ix == 0xbff0_0000_0000_0000 {
                return f64::NEG_INFINITY; // -1
            }
            return f64::NAN; // below -1
        }
        let j = (t.to_bits() as i64).wrapping_sub(0x3fe6_a000_0000_0000);
        let j1 = ((j >> (52 - 6)) & 0x3f) as usize;
        let je = j >> 52;
        let eoff = (je as u64) << 52;
        let mut rs = RF[j1].to_bits();
        if je < 1022 {
            rs = rs.wrapping_sub(eoff);
        } else {
            rs = rs.wrapping_sub(1021u64 << 52);
            let sc = [
                b(0x3fe0000000000000),
                b(0x3fd0000000000000),
                b(0x3fc0000000000000),
            ];
            t *= sc[(je - 1022) as usize];
            dt *= sc[(je - 1022) as usize];
        }
        let rs = f64::from_bits(rs);
        let dh = rs * t;
        let dl = rs.mul_add(t, -dh) + rs * dt;
        let (xh, mut xl) = fasttwosum(dh - 1.0, dl);
        let x2 = xh * xh;
        xl += x2 * ((c[0] + xh * c[1]) + x2 * ((c[2] + xh * c[3]) + x2 * (c[4] + xh * c[5])));
        let l1 = b(0x3fe62e42fefa4000) * je as f64;
        let l0 = b(0xbd48432a1b0e2634) * je as f64;
        let a1 = LF[j1][1] + l1;
        let a0 = LF[j1][0] + l0;
        let (s1, s0) = fastsum(a1, a0, xh, xl);
        ln1 = s1;
        ln0 = s0;
        eps = b(0x3bea000000000000);
    }
    let lb = ln1 + (ln0 - eps);
    let ub = ln1 + (ln0 + eps);
    if lb != ub {
        return log1p_refine(x, lb);
    }
    lb
}

#[cold]
#[inline(never)]
fn log1p_refine(x: f64, a: f64) -> f64 {
    let cz = [
        [b(0x3fd5555555555555), b(0x3c75555555555556)],
        [b(0xbfd0000000000000), b(0x3a925558eff3c1ef)],
        [b(0x3fc999999999999a), b(0xbc6999999a91d6ca)],
        [b(0xbfc5555555555555), b(0xbc6588aab185593a)],
        [b(0x3fc2492492492492), b(0x3c628c490abc528f)],
    ];
    let czl = [
        b(0xbfbfffffffffc555),
        b(0x3fbc71c71c7185aa),
        b(0xbfb9999d44449a31),
        b(0x3fb745d51f1817c7),
    ];
    let cy = [
        [1.0, 0.0],
        [-0.5, 0.0],
        [b(0x3fd5555555555555), b(0x3c75555555555555)],
        [b(0xbfd0000000000000), b(0xb9480007ed9858b7)],
        [b(0x3fc999999999999a), b(0xbc69999999999991)],
    ];
    let cl = [
        b(0xbfc5555555555555),
        b(0x3fc2492492492492),
        b(0xbfc0000000073334),
        b(0x3fbc71c71c802f68),
    ];
    let ix = x.to_bits();
    let ax = ix << 1;
    let (mut ln22, mut ln21, mut ln20);
    if ax < 0x7ea0_0000_0000_0000 {
        if ax < 0x7940_0000_0000_0000 {
            if ax == 0 {
                return x;
            }
            return x.abs().mul_add(b(0xbc90000000000000), x);
        }
        let mut x2h = x * x;
        let mut x2l = x.mul_add(x, -x2h);
        let (x3h, x3l) = mulddd(x, x2h, x2l);
        let sl = x * ((czl[0] + x * czl[1]) + x2h * (czl[2] + x * czl[3]));
        let (sh, sl) = polyddd(x, &cz, sl);
        let (sh, sl) = muldd(sh, sl, x3h, x3l);
        x2h *= -0.5;
        x2l *= -0.5;
        ln22 = x;
        let (a, b) = fastsum(x2h, x2l, sh, sl);
        ln21 = a;
        ln20 = b;
    } else {
        let j = (a * b(0x40f71547652b82fe)).round_ties_even() as i64;
        let i = j + 34952;
        let j1 = ((i >> 12) & 0xf) as usize;
        let j2 = ((i >> 8) & 0xf) as usize;
        let j3 = ((i >> 4) & 0xf) as usize;
        let j4 = (i & 0xf) as usize;
        let je = i >> 16;
        let l0 = (LN[0][j1][0] + LN[1][j2][0]) + (LN[2][j3][0] + LN[3][j4][0]);
        let l1 = (LN[0][j1][1] + LN[1][j2][1]) + (LN[2][j3][1] + LN[3][j4][1]);
        let l2 = (LN[0][j1][2] + LN[1][j2][2]) + (LN[2][j3][2] + LN[3][j4][2]);
        let (mut t, mut dt);
        if (ix as i64) < 0x4690_0000_0000_0000 && ix < 0xbfe0_0000_0000_0000 {
            let (s, e) = twosum(1.0, x);
            t = s.to_bits();
            dt = e.to_bits();
            if dt << 1 == 0 {
                dt = 0;
            }
        } else {
            t = if (ix as i64) >= 0x4690_0000_0000_0000 {
                x.to_bits()
            } else {
                (1.0 + x).to_bits()
            };
            dt = 0;
        }
        t = t.wrapping_sub((je as u64) << 52);
        let tf = f64::from_bits(t);
        let t12 = RT[0][j1] * RT[1][j2];
        let t34 = RT[2][j3] * RT[3][j4];
        let th = t12 * t34;
        let tl = t12.mul_add(t34, -th);
        let dh = th * tf;
        let dl = th.mul_add(tf, -dh);
        let sh = tl * tf;
        let sl = tl.mul_add(tf, -sh);
        let (xh, xl) = fasttwosum(dh - 1.0, dl);
        let (mut xh, mut xl) = fastsum(xh, xl, sh, sl);
        if dt != 0 {
            dt = dt.wrapping_sub((je as u64) << 52);
            let dtf = f64::from_bits(dt);
            let ddh = th * dtf;
            let ddl = th.mul_add(dtf, -ddh) + tl * dtf;
            let (h, l) = fastsum(xh, xl, ddh, ddl);
            xh = h;
            xl = l;
        }
        let sl = xh * ((cl[0] + xh * cl[1]) + (xh * xh) * (cl[2] + xh * cl[3]));
        let (sh, sl) = polydd(xh, xl, &cy, sl);
        let (sh, sl) = muldd(xh, xl, sh, sl);
        if je != 0 {
            let jf = je as f64;
            ln22 = b(0x3fe62e42fefa4000) * jf;
            ln21 = b(0xbd48432a1b0e2800) * jf;
            ln20 = b(0x3a8cc01f97b57a08) * jf;
            ln22 += l2;
            let (a, b) = fastsum(ln21, ln20, l1, l0);
            ln21 = a;
            ln20 = b;
        } else {
            ln22 = l2;
            ln21 = l1;
            ln20 = l0;
        }
        let (a, b) = sum(ln21, ln20, sh, sl);
        ln21 = a;
        ln20 = b;
    }
    let (a, b) = fasttwosum(ln22, ln21);
    ln22 = a;
    ln21 = b;
    let (a, b) = fasttwosum(ln21, ln20);
    ln21 = a;
    ln20 = b;
    let mut t = ln21.to_bits();
    if t & (!0u64 >> 12) == 0 {
        let w = ln20.to_bits();
        if (w ^ t) >> 63 != 0 {
            t = t.wrapping_sub(1);
        } else {
            t = t.wrapping_add(1);
        }
        ln21 = f64::from_bits(t);
    }
    ln22 + ln21
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values of CORE-MATH's `cr_log1p` (the C code at the same commit, compiled with `gcc -O2
    /// -ffp-contract=off`), on each path.
    #[test]
    fn matches_core_math() {
        for (x, r) in [
            (0x3ff0000000000000u64, 0x3fe62e42fefa39efu64),
            (0x3fb0000000000000, 0x3faf0a30c01162a6),
            (0xbfe0000000000000, 0xbfe62e42fefa39ef),
            (0x3e80000000000000, 0x3e7fffffe000002b),
            (0x3c80000000000000, 0x3c80000000000000),
        ] {
            assert_eq!(log1p(f64::from_bits(x)).to_bits(), r, "log1p({x:#x})");
        }
        assert_eq!(log1p(-1.0), f64::NEG_INFINITY);
        assert_eq!(log1p(0.0).to_bits(), 0);
        assert_eq!(log1p(-0.0).to_bits(), 0x8000_0000_0000_0000);
        assert!(log1p(-2.0).is_nan());
    }
}

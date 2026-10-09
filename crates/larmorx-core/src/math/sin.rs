//! Correctly rounded sine for binary64 (`sin`).
//!
//! Port of CORE-MATH `src/binary64/sin/sin.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only: the rounding-mode, errno and floating-point exception code of
//! the C file is dropped. Algorithms, tables and the order of every floating-point
//! operation are kept exactly; `__builtin_fma` is [`f64::mul_add`] (correctly rounded on
//! every target), `u128` arithmetic wraps like C unsigned arithmetic, and hex-float
//! constants are written as their exact binary64 bit patterns.
//!
//! Original licence of sin.c (MIT):
//!
//! Copyright (c) 2022-2026 Paul Zimmermann and Tom Hubrecht and Alexei Sibidanov
//!
//! This file is part of the CORE-MATH project
//! (<https://core-math.gitlabpages.inria.fr/>).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.
//!
//! References (from sin.c):
//! 1. Handbook of Floating-Point Arithmetic (2nd edition), Muller et al., Birkhäuser, 2018.
//! 2. Computing hard-to-round cases of sin, cos, tan in double precision,
//!    V. Lefèvre, T. Ly, P. Zimmermann, ARITH 2026.

/// `f64` from its IEEE-754 bit pattern (tables are stored as exact bit patterns).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// Round `x` to the nearest integer, ties to even (`roundeven_finite` in sin.c).
#[inline(always)]
fn roundeven_finite(x: f64) -> f64 {
    x.round_ties_even()
}

/// Round `(-1)^s * r / 2^128` to `f64`, assuming `r` is non-zero and not in the
/// subnormal range (`u128_tod` in sin.c).
#[inline]
fn u128_tod(r: u128, s: usize) -> f64 {
    let sh = u64::from(((r >> 64) as u64).leading_zeros());
    // since the smallest distance from a binary64 number to a multiple of pi/2
    // is 2^-60.888 (see [1]), the smallest value of r/2^128 is about 2^-60.888
    // too (taking into account approximation errors), thus sh <= 60.
    let h = (r >> (75 - sh)) as u64; // upper 53 non-zero bits
    let rbit = ((r >> (74 - sh)) & 1) as usize; // round bit
    // { 0x1p-53, -0x1p-53 }
    const SGN: [f64; 2] = [fb(0x3ca0000000000000), fb(0xbca0000000000000)];
    let v = f64::from_bits(SGN[s].to_bits().wrapping_sub(sh << 52)); // scale by 2^-sh
    // { 0x1.8p-2, 0x1.8p-1 }
    const LOW: [f64; 2] = [fb(0x3fd8000000000000), fb(0x3fe8000000000000)];
    let a = h as f64 * v;
    let b = LOW[rbit] * v;
    // When rbit is 0, b < ulp(a)/2 and a + b rounds to a; when rbit is 1,
    // b > ulp(a)/2 and a + b rounds to nextup(a) (proof in sin.pdf; hard-to-round
    // cases are checked by sin.wc).
    a + b
}

/// This table approximates 1/(2pi) downwards with precision 1280:
/// 1/(2*pi) ~ T[0]/2^0 + T[1]/2^64 + ... + T[i]/2^(i*64) + ...
/// (`_T` in sin.c; entry 0 added manually there, entry 19 only used in
/// `reduce_large_acc`).
#[rustfmt::skip]
static T: [u64; 20] = [
    0x0000000000000000, // i=0
    0x28be60db9391054a, // i=1
    0x7f09d5f47d4d3770, // i=2
    0x36d8a5664f10e410, // i=3
    0x7f9458eaf7aef158, // i=4
    0x6dc91b8e909374b8, // i=5
    0x01924bba82746487, // i=6
    0x3f877ac72c4a69cf, // i=7
    0xba208d7d4baed121, // i=8
    0x3a671c09ad17df90, // i=9
    0x4e64758e60d4ce7d, // i=10
    0x272117e2ef7e4a0e, // i=11
    0xc7fe25fff7816603, // i=12
    0xfbcbc462d6829b47, // i=13
    0xdb4d9fb3c9f2c26d, // i=14
    0xd3d18fd9a797fa8b, // i=15
    0x5d49eeb1faf97c5e, // i=16
    0xcf41ce7de294a4ba, // i=17
    0x9afed7ec47e35742, // i=18
    0x1580cc11bf1edaea, // i=19
];

/// Degree-7 odd polynomial approximating sin(x) for 0 <= x < 2^-11.348 with absolute
/// error < 2^-128.601. Coefficients of degree 1, 3, 5 are PS[i]/2^128, that of degree 7
/// is PS[i]/2^64 (fixed point); the degree-3 and degree-7 signs are implicit (negative).
#[rustfmt::skip]
static PS: [u128; 4] = [
    0xffffffffffffffff_ffffffffffffc396, // 0
    0x2aaaaaaaaaaaaaaa_aaaaa9646e8f872e, // 1
    0x0222222222222220_467b898367fb1249, // 2
    0x0000000000000000_000d00d00bfff5ee, // 3
];

/// Degree-8 even polynomial approximating cos(x) for 0 <= x < 2^-11.348 with absolute
/// error < 2^-128. Coefficients are PC[i]/2^128, except degree 8 (PC[i]/2^64); the
/// degree-2 and degree-6 signs are implicit (negative).
#[rustfmt::skip]
static PC: [u128; 5] = [
    0xffffffffffffffff_ffffffffffffffff, // 0
    0x7fffffffffffffff_fffffffffa998bf0, // 1
    0x0aaaaaaaaaaaaaaa_aaa2caecc94962c7, // 2
    0x005b05b05b05ac1a_642013bc289028b6, // 3
    0x0000000000000000_0001a0193d9a550c, // 4
];

/// High 128 bits of the 256-bit product a*b, ignoring the low*low term (`mhUU`).
#[inline(always)]
fn mh_uu(a: u128, b: u128) -> u128 {
    let (ah, al) = ((a >> 64) as u64, a as u64);
    let (bh, bl) = ((b >> 64) as u64, b as u64);
    let ahbh = u128::from(ah) * u128::from(bh);
    let ahbl = u128::from(ah) * u128::from(bl);
    let albh = u128::from(al) * u128::from(bh);
    ahbh.wrapping_add((ahbl >> 64).wrapping_add(albh >> 64))
}

/// Return Sr such that Sr/2^128 approximates sin2pi(r), for 0 <= r < 2^-14,
/// where u/2^128 approximates r, u2/2^128 approximates r^2,
/// u4/2^128 approximates r^4, and u2h = floor(u2/2^64) (`evalPS`).
#[inline]
fn eval_ps(u: u128, u2: u128, u2h: u128, u4: u128) -> u128 {
    // fixed point: each variable a is interpreted as a/2^128.
    // Estrin's scheme: degree-{1,3} part and degree-{5,7} part times r^4.
    let mut sh = PS[2].wrapping_sub(PS[3].wrapping_mul(u2h));
    sh = mh_uu(sh, u4);
    let mut s = mh_uu(PS[1], u2);
    s = PS[0].wrapping_sub(s);
    s = s.wrapping_add(sh);
    mh_uu(s, u) // multiply by r
}

/// Return Cr such that Cr/2^128 approximates cos2pi(r), for 0 <= r < 2^-14,
/// where u2/2^128 approximates r^2, u4/2^128 approximates r^4, and
/// u2h = floor(u2/2^64) (`evalPC`).
#[inline]
fn eval_pc(u2: u128, u2h: u128, u4: u128) -> u128 {
    // Estrin's scheme
    let mut sh = PC[3].wrapping_sub(u2h.wrapping_mul(PC[4]));
    sh = PC[2].wrapping_sub(mh_uu(sh, u2));
    let mut s = mh_uu(PC[1], u2);
    s = PC[0].wrapping_sub(s);
    s.wrapping_add(mh_uu(sh, u4))
}

/// Argument reduction for |x| >= 2^31 (`reduce_large`).
/// Return (k, r) such that x/(2pi) mod 1 = k/2^15 + r + s with 0 <= r < 2^-15
/// and 0 <= s < 2^-67.988.
#[inline]
fn reduce_large(x: f64) -> (u64, f64) {
    let tu = x.to_bits();
    let e = ((tu >> 52) & 0x7ff) as i32; // 1054 <= e <= 2046
    let m: u64 = (1u64 << 52) | (tu & 0x000f_ffff_ffff_ffff);
    // x = m * 2^(e-1075)
    let i = ((e - 1011) / 64) as usize; // i = ceil((e-1074)/64), 0 <= i <= 16
    let f = (e - 1011) & 0x3f;
    // the number of fractional bits from m*_T[i] is 64-f, thus we have to
    // shift _T[i] by f bits to get 64 fractional bits
    let (v0, v1);
    if f == 0 {
        v0 = T[i];
        v1 = T[i + 1];
    } else {
        v0 = (T[i] << f) | (T[i + 1] >> (64 - f));
        v1 = (T[i + 1] << f) | (T[i + 2] >> (64 - f));
    }
    let mut u: u128 = u128::from(v1) | (u128::from(v0) << 64);
    u = u128::from(m).wrapping_mul(u);
    // round r to nearest, where 0x810000000000000 = 2^59 + 2^52
    const MAGIC: u128 = (1u128 << 112) + 0x0810_0000_0000_0000;
    u = u.wrapping_add(MAGIC);
    let t = ((u << 15) >> 75) as u64 as f64; // next 53 bits of u after the first 15
    // 0x1p-68, 0x1p-16
    let r = t * fb(0x3bb0000000000000) - fb(0x3ef0000000000000);
    ((u >> 113) as u64, r)
    // since we return 15 bits in i and 53 in h, the accuracy is at most 2^-68
}

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
    let ahhh = xh * ch;
    let l = (xh * cl + xl * ch) + xh.mul_add(ch, -ahhh);
    (ahhh, l)
}

/// For each j, 0 <= j < 128, U1[j] contains sh, sl, ch, cl where sh+sl is a
/// double-double approximation of sin(j*pi/2^7) and ch+cl is a double-double
/// approximation of cos(j*pi/2^7). Generated by U1() from sin.sage.
#[rustfmt::skip]
static U1: [[f64; 4]; 128] = [
    [fb(0x0000000000000000), fb(0x0000000000000000), fb(0x3ff0000000000000), fb(0x0000000000000000)], // 0
    [fb(0x3f992155f7a3667e), fb(0xbbfb1d63091a0130), fb(0x3feffd886084cd0d), fb(0xbc81354d4556e4cb)], // 1
    [fb(0x3fa91f65f10dd814), fb(0xbc2912bd0d569a90), fb(0x3feff621e3796d7e), fb(0xbc6c57bc2e24aa15)], // 2
    [fb(0x3fb2d52092ce19f6), fb(0xbc49a088a8bf6b2c), fb(0x3fefe9cdad01883a), fb(0x3c6521ecd0c67e35)], // 3
    [fb(0x3fb917a6bc29b42c), fb(0xbc3e2718d26ed688), fb(0x3fefd88da3d12526), fb(0xbc887df6378811c7)], // 4
    [fb(0x3fbf564e56a9730e), fb(0x3c4a2704729ae56d), fb(0x3fefc26470e19fd3), fb(0x3c81ec8668ecacee)], // 5
    [fb(0x3fc2c8106e8e613a), fb(0x3c513000a89a11e0), fb(0x3fefa7557f08a517), fb(0xbc87a0a8ca13571f)], // 6
    [fb(0x3fc5e214448b3fc6), fb(0x3c6531ff779ddac6), fb(0x3fef8764fa714ba9), fb(0x3c7ab256778ffcb6)], // 7
    [fb(0x3fc8f8b83c69a60b), fb(0xbc626d19b9ff8d82), fb(0x3fef6297cff75cb0), fb(0x3c7562172a361fd3)], // 8
    [fb(0x3fcc0b826a7e4f63), fb(0xbc1af1439e521935), fb(0x3fef38f3ac64e589), fb(0xbc7d7bafb51f72e6)], // 9
    [fb(0x3fcf19f97b215f1b), fb(0xbc642deef11da2c4), fb(0x3fef0a7efb9230d7), fb(0x3c752c7adc6b4989)], // 10
    [fb(0x3fd111d262b1f677), fb(0x3c7824c20ab7aa9a), fb(0x3feed740e7684963), fb(0x3c7e82c791f59cc2)], // 11
    [fb(0x3fd294062ed59f06), fb(0xbc75d28da2c4612d), fb(0x3fee9f4156c62dda), fb(0x3c8760b1e2e3f81e)], // 12
    [fb(0x3fd4135c94176601), fb(0x3c70c97c4afa2518), fb(0x3fee6288ec48e112), fb(0xbc616b56f2847754)], // 13
    [fb(0x3fd58f9a75ab1fdd), fb(0xbc1efdc0d58cf620), fb(0x3fee212104f686e5), fb(0xbc8014c76c126527)], // 14
    [fb(0x3fd7088530fa459f), fb(0xbc744b19e0864c5d), fb(0x3feddb13b6ccc23c), fb(0x3c883c37c6107db3)], // 15
    [fb(0x3fd87de2a6aea963), fb(0xbc672cedd3d5a610), fb(0x3fed906bcf328d46), fb(0x3c7457e610231ac2)], // 16
    [fb(0x3fd9ef7943a8ed8a), fb(0x3c66da81290bdbab), fb(0x3fed4134d14dc93a), fb(0xbc84ef5295d25af2)], // 17
    [fb(0x3fdb5d1009e15cc0), fb(0x3c65b362cb974183), fb(0x3feced7af43cc773), fb(0xbc5e7b6bb5ab58ae)], // 18
    [fb(0x3fdcc66e9931c45e), fb(0x3c56850e59c37f8f), fb(0x3fec954b213411f5), fb(0xbc52fb761e946603)], // 19
    [fb(0x3fde2b5d3806f63b), fb(0x3c5e0d891d3c6841), fb(0x3fec38b2f180bdb1), fb(0xbc76e0b1757c8d07)], // 20
    [fb(0x3fdf8ba4dbf89aba), fb(0xbc32ec1fc1b776b8), fb(0x3febd7c0ac6f952a), fb(0xbc8825a732ac700a)], // 21
    [fb(0x3fe073879922ffee), fb(0xbc8a5a014347406c), fb(0x3feb728345196e3e), fb(0xbc8bc69f324e6d61)], // 22
    [fb(0x3fe11eb3541b4b23), fb(0xbc8ef23b69abe4f1), fb(0x3feb090a58150200), fb(0xbc8926da300ffcce)], // 23
    [fb(0x3fe1c73b39ae68c8), fb(0x3c8b25dd267f6600), fb(0x3fea9b66290ea1a3), fb(0x3c39f630e8b6dac8)], // 24
    [fb(0x3fe26d054cdd12df), fb(0xbc85da743ef3770c), fb(0x3fea29a7a0462782), fb(0xbc7128bb015df175)], // 25
    [fb(0x3fe30ff7fce17035), fb(0xbc6efcc626f74a6f), fb(0x3fe9b3e047f38741), fb(0xbc830ee286712474)], // 26
    [fb(0x3fe3affa292050b9), fb(0x3c7e3e25e3954964), fb(0x3fe93a22499263fb), fb(0x3c83d419a920df0b)], // 27
    [fb(0x3fe44cf325091dd6), fb(0x3c68076a2cfdc6b3), fb(0x3fe8bc806b151741), fb(0xbc82c5e12ed1336d)], // 28
    [fb(0x3fe4e6cabbe3e5e9), fb(0x3c63c293edceb327), fb(0x3fe83b0e0bff976e), fb(0xbc76f420f8ea3475)], // 29
    [fb(0x3fe57d69348ceca0), fb(0xbc875720992bfbb2), fb(0x3fe7b5df226aafaf), fb(0xbc70f537acdf0ad7)], // 30
    [fb(0x3fe610b7551d2cdf), fb(0xbc7251b352ff2a37), fb(0x3fe72d0837efff96), fb(0x3c80d4ef0f1d915c)], // 31
    [fb(0x3fe6a09e667f3bcd), fb(0xbc8bdd3413b26456), fb(0x3fe6a09e667f3bcd), fb(0xbc8bdd3413b26456)], // 32
    [fb(0x3fe72d0837efff96), fb(0x3c80d4ef0f1d915c), fb(0x3fe610b7551d2cdf), fb(0xbc7251b352ff2a37)], // 33
    [fb(0x3fe7b5df226aafaf), fb(0xbc70f537acdf0ad7), fb(0x3fe57d69348ceca0), fb(0xbc875720992bfbb2)], // 34
    [fb(0x3fe83b0e0bff976e), fb(0xbc76f420f8ea3475), fb(0x3fe4e6cabbe3e5e9), fb(0x3c63c293edceb327)], // 35
    [fb(0x3fe8bc806b151741), fb(0xbc82c5e12ed1336d), fb(0x3fe44cf325091dd6), fb(0x3c68076a2cfdc6b3)], // 36
    [fb(0x3fe93a22499263fb), fb(0x3c83d419a920df0b), fb(0x3fe3affa292050b9), fb(0x3c7e3e25e3954964)], // 37
    [fb(0x3fe9b3e047f38741), fb(0xbc830ee286712474), fb(0x3fe30ff7fce17035), fb(0xbc6efcc626f74a6f)], // 38
    [fb(0x3fea29a7a0462782), fb(0xbc7128bb015df175), fb(0x3fe26d054cdd12df), fb(0xbc85da743ef3770c)], // 39
    [fb(0x3fea9b66290ea1a3), fb(0x3c39f630e8b6dac8), fb(0x3fe1c73b39ae68c8), fb(0x3c8b25dd267f6600)], // 40
    [fb(0x3feb090a58150200), fb(0xbc8926da300ffcce), fb(0x3fe11eb3541b4b23), fb(0xbc8ef23b69abe4f1)], // 41
    [fb(0x3feb728345196e3e), fb(0xbc8bc69f324e6d61), fb(0x3fe073879922ffee), fb(0xbc8a5a014347406c)], // 42
    [fb(0x3febd7c0ac6f952a), fb(0xbc8825a732ac700a), fb(0x3fdf8ba4dbf89aba), fb(0xbc32ec1fc1b776b8)], // 43
    [fb(0x3fec38b2f180bdb1), fb(0xbc76e0b1757c8d07), fb(0x3fde2b5d3806f63b), fb(0x3c5e0d891d3c6841)], // 44
    [fb(0x3fec954b213411f5), fb(0xbc52fb761e946603), fb(0x3fdcc66e9931c45e), fb(0x3c56850e59c37f8f)], // 45
    [fb(0x3feced7af43cc773), fb(0xbc5e7b6bb5ab58ae), fb(0x3fdb5d1009e15cc0), fb(0x3c65b362cb974183)], // 46
    [fb(0x3fed4134d14dc93a), fb(0xbc84ef5295d25af2), fb(0x3fd9ef7943a8ed8a), fb(0x3c66da81290bdbab)], // 47
    [fb(0x3fed906bcf328d46), fb(0x3c7457e610231ac2), fb(0x3fd87de2a6aea963), fb(0xbc672cedd3d5a610)], // 48
    [fb(0x3feddb13b6ccc23c), fb(0x3c883c37c6107db3), fb(0x3fd7088530fa459f), fb(0xbc744b19e0864c5d)], // 49
    [fb(0x3fee212104f686e5), fb(0xbc8014c76c126527), fb(0x3fd58f9a75ab1fdd), fb(0xbc1efdc0d58cf620)], // 50
    [fb(0x3fee6288ec48e112), fb(0xbc616b56f2847754), fb(0x3fd4135c94176601), fb(0x3c70c97c4afa2518)], // 51
    [fb(0x3fee9f4156c62dda), fb(0x3c8760b1e2e3f81e), fb(0x3fd294062ed59f06), fb(0xbc75d28da2c4612d)], // 52
    [fb(0x3feed740e7684963), fb(0x3c7e82c791f59cc2), fb(0x3fd111d262b1f677), fb(0x3c7824c20ab7aa9a)], // 53
    [fb(0x3fef0a7efb9230d7), fb(0x3c752c7adc6b4989), fb(0x3fcf19f97b215f1b), fb(0xbc642deef11da2c4)], // 54
    [fb(0x3fef38f3ac64e589), fb(0xbc7d7bafb51f72e6), fb(0x3fcc0b826a7e4f63), fb(0xbc1af1439e521935)], // 55
    [fb(0x3fef6297cff75cb0), fb(0x3c7562172a361fd3), fb(0x3fc8f8b83c69a60b), fb(0xbc626d19b9ff8d82)], // 56
    [fb(0x3fef8764fa714ba9), fb(0x3c7ab256778ffcb6), fb(0x3fc5e214448b3fc6), fb(0x3c6531ff779ddac6)], // 57
    [fb(0x3fefa7557f08a517), fb(0xbc87a0a8ca13571f), fb(0x3fc2c8106e8e613a), fb(0x3c513000a89a11e0)], // 58
    [fb(0x3fefc26470e19fd3), fb(0x3c81ec8668ecacee), fb(0x3fbf564e56a9730e), fb(0x3c4a2704729ae56d)], // 59
    [fb(0x3fefd88da3d12526), fb(0xbc887df6378811c7), fb(0x3fb917a6bc29b42c), fb(0xbc3e2718d26ed688)], // 60
    [fb(0x3fefe9cdad01883a), fb(0x3c6521ecd0c67e35), fb(0x3fb2d52092ce19f6), fb(0xbc49a088a8bf6b2c)], // 61
    [fb(0x3feff621e3796d7e), fb(0xbc6c57bc2e24aa15), fb(0x3fa91f65f10dd814), fb(0xbc2912bd0d569a90)], // 62
    [fb(0x3feffd886084cd0d), fb(0xbc81354d4556e4cb), fb(0x3f992155f7a3667e), fb(0xbbfb1d63091a0130)], // 63
    [fb(0x3ff0000000000000), fb(0x0000000000000000), fb(0x0000000000000000), fb(0x0000000000000000)], // 64
    [fb(0x3feffd886084cd0d), fb(0xbc81354d4556e4cb), fb(0xbf992155f7a3667e), fb(0x3bfb1d63091a0130)], // 65
    [fb(0x3feff621e3796d7e), fb(0xbc6c57bc2e24aa15), fb(0xbfa91f65f10dd814), fb(0x3c2912bd0d569a90)], // 66
    [fb(0x3fefe9cdad01883a), fb(0x3c6521ecd0c67e35), fb(0xbfb2d52092ce19f6), fb(0x3c49a088a8bf6b2c)], // 67
    [fb(0x3fefd88da3d12526), fb(0xbc887df6378811c7), fb(0xbfb917a6bc29b42c), fb(0x3c3e2718d26ed688)], // 68
    [fb(0x3fefc26470e19fd3), fb(0x3c81ec8668ecacee), fb(0xbfbf564e56a9730e), fb(0xbc4a2704729ae56d)], // 69
    [fb(0x3fefa7557f08a517), fb(0xbc87a0a8ca13571f), fb(0xbfc2c8106e8e613a), fb(0xbc513000a89a11e0)], // 70
    [fb(0x3fef8764fa714ba9), fb(0x3c7ab256778ffcb6), fb(0xbfc5e214448b3fc6), fb(0xbc6531ff779ddac6)], // 71
    [fb(0x3fef6297cff75cb0), fb(0x3c7562172a361fd3), fb(0xbfc8f8b83c69a60b), fb(0x3c626d19b9ff8d82)], // 72
    [fb(0x3fef38f3ac64e589), fb(0xbc7d7bafb51f72e6), fb(0xbfcc0b826a7e4f63), fb(0x3c1af1439e521935)], // 73
    [fb(0x3fef0a7efb9230d7), fb(0x3c752c7adc6b4989), fb(0xbfcf19f97b215f1b), fb(0x3c642deef11da2c4)], // 74
    [fb(0x3feed740e7684963), fb(0x3c7e82c791f59cc2), fb(0xbfd111d262b1f677), fb(0xbc7824c20ab7aa9a)], // 75
    [fb(0x3fee9f4156c62dda), fb(0x3c8760b1e2e3f81e), fb(0xbfd294062ed59f06), fb(0x3c75d28da2c4612d)], // 76
    [fb(0x3fee6288ec48e112), fb(0xbc616b56f2847754), fb(0xbfd4135c94176601), fb(0xbc70c97c4afa2518)], // 77
    [fb(0x3fee212104f686e5), fb(0xbc8014c76c126527), fb(0xbfd58f9a75ab1fdd), fb(0x3c1efdc0d58cf620)], // 78
    [fb(0x3feddb13b6ccc23c), fb(0x3c883c37c6107db3), fb(0xbfd7088530fa459f), fb(0x3c744b19e0864c5d)], // 79
    [fb(0x3fed906bcf328d46), fb(0x3c7457e610231ac2), fb(0xbfd87de2a6aea963), fb(0x3c672cedd3d5a610)], // 80
    [fb(0x3fed4134d14dc93a), fb(0xbc84ef5295d25af2), fb(0xbfd9ef7943a8ed8a), fb(0xbc66da81290bdbab)], // 81
    [fb(0x3feced7af43cc773), fb(0xbc5e7b6bb5ab58ae), fb(0xbfdb5d1009e15cc0), fb(0xbc65b362cb974183)], // 82
    [fb(0x3fec954b213411f5), fb(0xbc52fb761e946603), fb(0xbfdcc66e9931c45e), fb(0xbc56850e59c37f8f)], // 83
    [fb(0x3fec38b2f180bdb1), fb(0xbc76e0b1757c8d07), fb(0xbfde2b5d3806f63b), fb(0xbc5e0d891d3c6841)], // 84
    [fb(0x3febd7c0ac6f952a), fb(0xbc8825a732ac700a), fb(0xbfdf8ba4dbf89aba), fb(0x3c32ec1fc1b776b8)], // 85
    [fb(0x3feb728345196e3e), fb(0xbc8bc69f324e6d61), fb(0xbfe073879922ffee), fb(0x3c8a5a014347406c)], // 86
    [fb(0x3feb090a58150200), fb(0xbc8926da300ffcce), fb(0xbfe11eb3541b4b23), fb(0x3c8ef23b69abe4f1)], // 87
    [fb(0x3fea9b66290ea1a3), fb(0x3c39f630e8b6dac8), fb(0xbfe1c73b39ae68c8), fb(0xbc8b25dd267f6600)], // 88
    [fb(0x3fea29a7a0462782), fb(0xbc7128bb015df175), fb(0xbfe26d054cdd12df), fb(0x3c85da743ef3770c)], // 89
    [fb(0x3fe9b3e047f38741), fb(0xbc830ee286712474), fb(0xbfe30ff7fce17035), fb(0x3c6efcc626f74a6f)], // 90
    [fb(0x3fe93a22499263fb), fb(0x3c83d419a920df0b), fb(0xbfe3affa292050b9), fb(0xbc7e3e25e3954964)], // 91
    [fb(0x3fe8bc806b151741), fb(0xbc82c5e12ed1336d), fb(0xbfe44cf325091dd6), fb(0xbc68076a2cfdc6b3)], // 92
    [fb(0x3fe83b0e0bff976e), fb(0xbc76f420f8ea3475), fb(0xbfe4e6cabbe3e5e9), fb(0xbc63c293edceb327)], // 93
    [fb(0x3fe7b5df226aafaf), fb(0xbc70f537acdf0ad7), fb(0xbfe57d69348ceca0), fb(0x3c875720992bfbb2)], // 94
    [fb(0x3fe72d0837efff96), fb(0x3c80d4ef0f1d915c), fb(0xbfe610b7551d2cdf), fb(0x3c7251b352ff2a37)], // 95
    [fb(0x3fe6a09e667f3bcd), fb(0xbc8bdd3413b26456), fb(0xbfe6a09e667f3bcd), fb(0x3c8bdd3413b26456)], // 96
    [fb(0x3fe610b7551d2cdf), fb(0xbc7251b352ff2a37), fb(0xbfe72d0837efff96), fb(0xbc80d4ef0f1d915c)], // 97
    [fb(0x3fe57d69348ceca0), fb(0xbc875720992bfbb2), fb(0xbfe7b5df226aafaf), fb(0x3c70f537acdf0ad7)], // 98
    [fb(0x3fe4e6cabbe3e5e9), fb(0x3c63c293edceb327), fb(0xbfe83b0e0bff976e), fb(0x3c76f420f8ea3475)], // 99
    [fb(0x3fe44cf325091dd6), fb(0x3c68076a2cfdc6b3), fb(0xbfe8bc806b151741), fb(0x3c82c5e12ed1336d)], // 100
    [fb(0x3fe3affa292050b9), fb(0x3c7e3e25e3954964), fb(0xbfe93a22499263fb), fb(0xbc83d419a920df0b)], // 101
    [fb(0x3fe30ff7fce17035), fb(0xbc6efcc626f74a6f), fb(0xbfe9b3e047f38741), fb(0x3c830ee286712474)], // 102
    [fb(0x3fe26d054cdd12df), fb(0xbc85da743ef3770c), fb(0xbfea29a7a0462782), fb(0x3c7128bb015df175)], // 103
    [fb(0x3fe1c73b39ae68c8), fb(0x3c8b25dd267f6600), fb(0xbfea9b66290ea1a3), fb(0xbc39f630e8b6dac8)], // 104
    [fb(0x3fe11eb3541b4b23), fb(0xbc8ef23b69abe4f1), fb(0xbfeb090a58150200), fb(0x3c8926da300ffcce)], // 105
    [fb(0x3fe073879922ffee), fb(0xbc8a5a014347406c), fb(0xbfeb728345196e3e), fb(0x3c8bc69f324e6d61)], // 106
    [fb(0x3fdf8ba4dbf89aba), fb(0xbc32ec1fc1b776b8), fb(0xbfebd7c0ac6f952a), fb(0x3c8825a732ac700a)], // 107
    [fb(0x3fde2b5d3806f63b), fb(0x3c5e0d891d3c6841), fb(0xbfec38b2f180bdb1), fb(0x3c76e0b1757c8d07)], // 108
    [fb(0x3fdcc66e9931c45e), fb(0x3c56850e59c37f8f), fb(0xbfec954b213411f5), fb(0x3c52fb761e946603)], // 109
    [fb(0x3fdb5d1009e15cc0), fb(0x3c65b362cb974183), fb(0xbfeced7af43cc773), fb(0x3c5e7b6bb5ab58ae)], // 110
    [fb(0x3fd9ef7943a8ed8a), fb(0x3c66da81290bdbab), fb(0xbfed4134d14dc93a), fb(0x3c84ef5295d25af2)], // 111
    [fb(0x3fd87de2a6aea963), fb(0xbc672cedd3d5a610), fb(0xbfed906bcf328d46), fb(0xbc7457e610231ac2)], // 112
    [fb(0x3fd7088530fa459f), fb(0xbc744b19e0864c5d), fb(0xbfeddb13b6ccc23c), fb(0xbc883c37c6107db3)], // 113
    [fb(0x3fd58f9a75ab1fdd), fb(0xbc1efdc0d58cf620), fb(0xbfee212104f686e5), fb(0x3c8014c76c126527)], // 114
    [fb(0x3fd4135c94176601), fb(0x3c70c97c4afa2518), fb(0xbfee6288ec48e112), fb(0x3c616b56f2847754)], // 115
    [fb(0x3fd294062ed59f06), fb(0xbc75d28da2c4612d), fb(0xbfee9f4156c62dda), fb(0xbc8760b1e2e3f81e)], // 116
    [fb(0x3fd111d262b1f677), fb(0x3c7824c20ab7aa9a), fb(0xbfeed740e7684963), fb(0xbc7e82c791f59cc2)], // 117
    [fb(0x3fcf19f97b215f1b), fb(0xbc642deef11da2c4), fb(0xbfef0a7efb9230d7), fb(0xbc752c7adc6b4989)], // 118
    [fb(0x3fcc0b826a7e4f63), fb(0xbc1af1439e521935), fb(0xbfef38f3ac64e589), fb(0x3c7d7bafb51f72e6)], // 119
    [fb(0x3fc8f8b83c69a60b), fb(0xbc626d19b9ff8d82), fb(0xbfef6297cff75cb0), fb(0xbc7562172a361fd3)], // 120
    [fb(0x3fc5e214448b3fc6), fb(0x3c6531ff779ddac6), fb(0xbfef8764fa714ba9), fb(0xbc7ab256778ffcb6)], // 121
    [fb(0x3fc2c8106e8e613a), fb(0x3c513000a89a11e0), fb(0xbfefa7557f08a517), fb(0x3c87a0a8ca13571f)], // 122
    [fb(0x3fbf564e56a9730e), fb(0x3c4a2704729ae56d), fb(0xbfefc26470e19fd3), fb(0xbc81ec8668ecacee)], // 123
    [fb(0x3fb917a6bc29b42c), fb(0xbc3e2718d26ed688), fb(0xbfefd88da3d12526), fb(0x3c887df6378811c7)], // 124
    [fb(0x3fb2d52092ce19f6), fb(0xbc49a088a8bf6b2c), fb(0xbfefe9cdad01883a), fb(0xbc6521ecd0c67e35)], // 125
    [fb(0x3fa91f65f10dd814), fb(0xbc2912bd0d569a90), fb(0xbfeff621e3796d7e), fb(0x3c6c57bc2e24aa15)], // 126
    [fb(0x3f992155f7a3667e), fb(0xbbfb1d63091a0130), fb(0xbfeffd886084cd0d), fb(0x3c81354d4556e4cb)], // 127
];

/// For each j, 0 <= j < 128, U2[j] contains sh, sl, ch, cl where sh+sl is a
/// double-double approximation of sin(j*pi/2^14) and ch+cl is a double-double
/// approximation of cos(j*pi/2^14). Generated by U2() from sin.sage.
#[rustfmt::skip]
static U2: [[f64; 4]; 128] = [
    [fb(0x0000000000000000), fb(0x0000000000000000), fb(0x3ff0000000000000), fb(0x0000000000000000)], // 0
    [fb(0x3f2921fb51aeb57c), fb(0xbbca6e1d4916c435), fb(0x3feffffff621619c), fb(0xbc87507dbbbd8fe6)], // 1
    [fb(0x3f3921fb49ee4ea6), fb(0x3bde894d744a453e), fb(0x3fefffffd8858675), fb(0xbc879f0e54748eab)], // 2
    [fb(0x3f42d97c6dc23a75), fb(0xbbd1c27727253849), fb(0x3fefffffa72c6e9d), fb(0x3c7ffa0e2a0e5c29)], // 3
    [fb(0x3f4921fb2aecb360), fb(0x3be876157e566b4c), fb(0x3fefffff62161a34), fb(0xbc6136dcb1f9b9c4)], // 4
    [fb(0x3f4f6a79d8965eba), fb(0xbbea2a951670a396), fb(0x3fefffff09428963), fb(0x3c82f505fba9c0b0)], // 5
    [fb(0x3f52d97c396f8497), fb(0xbbd45cb4cc3d0fb1), fb(0x3feffffe9cb1bc62), fb(0x3c70fe9fa3478ec9)], // 6
    [fb(0x3f55fdbb7af33fbb), fb(0x3bec0c37a5d5101d), fb(0x3feffffe1c63b373), fb(0x3c8a50f155e87e56)], // 7
    [fb(0x3f5921faaee6472e), fb(0xbbfee52e284a9df8), fb(0x3feffffd88586ee6), fb(0x3c81af64f173ae5b)], // 8
    [fb(0x3f5c4639d358815a), fb(0x3ba0ffa0e86b4dcc), fb(0x3feffffce08fef16), fb(0x3c592e420a03bf59)], // 9
    [fb(0x3f5f6a78e659d4b6), fb(0x3bfcb8a9b355ac80), fb(0x3feffffc250a346a), fb(0x3c7d099c02280c58)], // 10
    [fb(0x3f61475bf2fd13e2), fb(0xbc04ae6ded9797cc), fb(0x3feffffb55c73f56), fb(0x3c8ee2ae1596963a)], // 11
    [fb(0x3f62d97b6824b087), fb(0xbc09dae69dd4f97f), fb(0x3feffffa72c7105b), fb(0xbc8564ec4a452371)], // 12
    [fb(0x3f646b9ad1abb397), fb(0xbbfac1b1950e9dc6), fb(0x3feffff97c09a803), fb(0xbc7961ec991858f1)], // 13
    [fb(0x3f65fdba2e9a1066), fb(0x3bff1ed8abaaa608), fb(0x3feffff8718f06e7), fb(0x3c6793938ee3fc9a)], // 14
    [fb(0x3f678fd97df7ba51), fb(0xbc02f2580c9ba856), fb(0x3feffff753572dac), fb(0xbc65df4d689b3227)], // 15
    [fb(0x3f6921f8becca4ba), fb(0x3c02ba407bcab5b2), fb(0x3feffff621621d02), fb(0xbc76acfcebc82813)], // 16
    [fb(0x3f6ab417f020c310), fb(0x3be7fb6c9ae3781a), fb(0x3feffff4dbafd5a6), fb(0xbc8d017de84ebc3f)], // 17
    [fb(0x3f6c463710fc08c9), fb(0xbbff621bbef801cf), fb(0x3feffff382405860), fb(0xbc2f949383d834e0)], // 18
    [fb(0x3f6dd85620666965), fb(0x3be5a1a4ecd8345a), fb(0x3feffff21513a606), fb(0x3c7ee48b39cc31cf)], // 19
    [fb(0x3f6f6a751d67d871), fb(0xbc066bdef183ff59), fb(0x3feffff09429bf7a), fb(0xbc8cabd3ded13a63)], // 20
    [fb(0x3f707e4a038424c1), fb(0x3c07e6886ed9bec7), fb(0x3fefffeeff82a5a7), fb(0x3c86efc1cf23e273)], // 21
    [fb(0x3f7147596e27d81f), fb(0xbc1bbfe9edba714a), fb(0x3fefffed571e5989), fb(0x3c8151c0e15503cf)], // 22
    [fb(0x3f721068ce230028), fb(0x3c06ce44ca79798d), fb(0x3fefffeb9afcdc25), fb(0x3c632c5af9456b9f)], // 23
    [fb(0x3f72d97822f996bc), fb(0x3c13e5a15ed6aa3e), fb(0x3fefffe9cb1e2e8d), fb(0xbc810f44663fd601)], // 24
    [fb(0x3f73a2876c2f95c0), fb(0x3c003f5805a36f7e), fb(0x3fefffe7e78251de), fb(0x3c889b8834c8800c)], // 25
    [fb(0x3f746b96a948f720), fb(0xbbe3fbff884c87da), fb(0x3fefffe5f0294744), fb(0x3c85e809bdc855fc)], // 26
    [fb(0x3f7534a5d9c9b4d0), fb(0xbc18653d3b3f3bf5), fb(0x3fefffe3e5130ff5), fb(0x3c68d94f3d6ca2d6)], // 27
    [fb(0x3f75fdb4fd35c8cb), fb(0xbc14b73d73e9b437), fb(0x3fefffe1c63fad33), fb(0x3c8429d08eb02c47)], // 28
    [fb(0x3f76c6c413112d15), fb(0xbc0c66913985b2a8), fb(0x3fefffdf93af204e), fb(0xbc74ade22d9f9483)], // 29
    [fb(0x3f778fd31adfdbba), fb(0xbbf03bf7bee2893d), fb(0x3fefffdd4d616aa0), fb(0xbc8422710e8c8595)], // 30
    [fb(0x3f7858e21425cecf), fb(0xbc05af2befb31c3c), fb(0x3fefffdaf3568d90), fb(0x3c599c62dc3a22ac)], // 31
    [fb(0x3f7921f0fe670071), fb(0x3bfab967fe6b7a9b), fb(0x3fefffd8858e8a92), fb(0x3c8359c71883bcf7)], // 32
    [fb(0x3f79eaffd9276ac8), fb(0xbc1558e4e5ccafc7), fb(0x3fefffd604096326), fb(0xbc37d6bdd1c1f436)], // 33
    [fb(0x3f7ab40ea3eb0803), fb(0xbbc7fe2a8e15f257), fb(0x3fefffd36ec718d7), fb(0xbc75c59ffcfba7e4)], // 34
    [fb(0x3f7b7d1d5e35d25d), fb(0x3c19cfe67f192cd7), fb(0x3fefffd0c5c7ad3d), fb(0xbc819d99906d21e9)], // 35
    [fb(0x3f7c462c078bc41b), fb(0x3c19f6db9a6ada3c), fb(0x3fefffce090b21fc), fb(0xbc80389138f7e5ef)], // 36
    [fb(0x3f7d0f3a9f70d78c), fb(0xbbea08e48b732df4), fb(0x3fefffcb389178c4), fb(0x3c72883b1fe7d578)], // 37
    [fb(0x3f7dd84925690709), fb(0xbc10441c939bd6da), fb(0x3fefffc8545ab352), fb(0x3c861157c68f984b)], // 38
    [fb(0x3f7ea15798f84cf7), fb(0xbc1bd41d0fe0c595), fb(0x3fefffc55c66d36f), fb(0xbc5ab98d6fcf9cd6)], // 39
    [fb(0x3f7f6a65f9a2a3c6), fb(0xbc1de8c48783f3ae), fb(0x3fefffc250b5daef), fb(0xbc813b48657081cd)], // 40
    [fb(0x3f8019ba237602f9), fb(0xbc22479c7145927d), fb(0x3fefffbf3147cbb3), fb(0xbc86a078c1216e21)], // 41
    [fb(0x3f807e41402c3701), fb(0xbbc93e58a2a04d27), fb(0x3fefffbbfe1ca7a8), fb(0xbc7698b82e41cfae)], // 42
    [fb(0x3f80e2c852b5eb46), fb(0xbc29b8495e4ec41d), fb(0x3fefffb8b73470c8), fb(0xbc8bc4e4be396442)], // 43
    [fb(0x3f81474f5ad51d17), fb(0xbc1a0fb850f9330d), fb(0x3fefffb55c8f2917), fb(0x3c86af540576eb51)], // 44
    [fb(0x3f81abd6584bc9cb), fb(0x3c221b927d9670cc), fb(0x3fefffb1ee2cd2a9), fb(0xbc73ee83959a5f1d)], // 45
    [fb(0x3f82105d4adbeec0), fb(0x3c24e6ed5742dcf7), fb(0x3fefffae6c0d6f9a), fb(0xbc7102fd2cb5b720)], // 46
    [fb(0x3f8274e43247895a), fb(0xbc046ca3539434ea), fb(0x3fefffaad6310214), fb(0x3c8b8629443a759a)], // 47
    [fb(0x3f82d96b0e509703), fb(0xbc01e9131ff52dc9), fb(0x3fefffa72c978c4f), fb(0xbc822cb000328f91)], // 48
    [fb(0x3f833df1deb9152d), fb(0x3c1f5fb935dbf892), fb(0x3fefffa36f41108b), fb(0x3c655d1c829905c6)], // 49
    [fb(0x3f83a278a3430152), fb(0xbc02c26d82855518), fb(0x3fefff9f9e2d9118), fb(0x3c6102f90f3495e0)], // 50
    [fb(0x3f8406ff5bb058f1), fb(0x3c26e66ff4bcd327), fb(0x3fefff9bb95d1050), fb(0x3c87fb680db05f83)], // 51
    [fb(0x3f846b8607c31993), fb(0x3c291d1ac2a2ced6), fb(0x3fefff97c0cf909b), fb(0xbc6b418b1f88cf02)], // 52
    [fb(0x3f84d00ca73d40c8), fb(0xbc2fae7469c8dbd1), fb(0x3fefff93b485146b), fb(0xbc843e02406fb373)], // 53
    [fb(0x3f85349339e0cc25), fb(0xbbfec016ea8e8f27), fb(0x3fefff8f947d9e3f), fb(0xbc42cefd02ebe75c)], // 54
    [fb(0x3f859919bf6fb94b), fb(0xbc11ca7261e51e18), fb(0x3fefff8b60b930a3), fb(0x3c797dbe1fdcab90)], // 55
    [fb(0x3f85fda037ac05e1), fb(0xbc2ff59bf4b574ee), fb(0x3fefff871937ce2f), fb(0xbc638dae49f0be32)], // 56
    [fb(0x3f866226a257af95), fb(0x3c23cd7a66dfafc8), fb(0x3fefff82bdf97986), fb(0xbc6507b1e2913e3b)], // 57
    [fb(0x3f86c6acff34b421), fb(0x3c0b03798bbe7197), fb(0x3fefff7e4efe3558), fb(0x3c6ebe425d873a16)], // 58
    [fb(0x3f872b334e051144), fb(0x3c1caf2df425ef35), fb(0x3fefff79cc460462), fb(0xbc86c8d94412b92a)], // 59
    [fb(0x3f878fb98e8ac4c8), fb(0xbc2555202816f3a5), fb(0x3fefff7535d0e96b), fb(0xbc7c5727ff41299b)], // 60
    [fb(0x3f87f43fc087cc7d), fb(0x3c200a2a3a85d253), fb(0x3fefff708b9ee748), fb(0xbc37ff8f5cefe85a)], // 61
    [fb(0x3f8858c5e3be2640), fb(0xbc1ee9c7105192d8), fb(0x3fefff6bcdb000da), fb(0xbc665348e4f5cd16)], // 62
    [fb(0x3f88bd4bf7efcff3), fb(0x3c066640846e6109), fb(0x3fefff66fc04390d), fb(0x3c877f5704f2756c)], // 63
    [fb(0x3f8921d1fcdec784), fb(0x3c29878ebe836d9d), fb(0x3fefff62169b92db), fb(0x3c85dda3c81fbd0d)], // 64
    [fb(0x3f898657f24d0aea), fb(0x3c211d4d1d1806d2), fb(0x3fefff5d1d761149), fb(0x3c0e7b86f31e2875)], // 65
    [fb(0x3f89eaddd7fc9825), fb(0xbc15adb5adfd3669), fb(0x3fefff581093b768), fb(0xbc83eeee9513ae4c)], // 66
    [fb(0x3f8a4f63adaf6d3e), fb(0xbbff838d8dd726dc), fb(0x3fefff52eff48855), fb(0xbc6514532e82de3f)], // 67
    [fb(0x3f8ab3e973278849), fb(0x3c235b145a18353c), fb(0x3fefff4dbb98873a), fb(0x3c8857663dc92252)], // 68
    [fb(0x3f8b186f2826e765), fb(0xbc26b3b32921e88c), fb(0x3fefff48737fb74e), fb(0xbc5d462b0c1681e7)], // 69
    [fb(0x3f8b7cf4cc6f88b8), fb(0xbc132c6a46235330), fb(0x3fefff4317aa1bd2), fb(0xbc86a0039355b637)], // 70
    [fb(0x3f8be17a5fc36a75), fb(0x3c0c404548edfc51), fb(0x3fefff3da817b814), fb(0xbc82b464e14730c0)], // 71
    [fb(0x3f8c45ffe1e48ad9), fb(0x3c04060e4bd32e79), fb(0x3fefff3824c88f6f), fb(0xbc8ed820f3fe6980)], // 72
    [fb(0x3f8caa855294e82b), fb(0x3bdeca8fa79834de), fb(0x3fefff328dbca549), fb(0xbc86ca1af321dc1c)], // 73
    [fb(0x3f8d0f0ab19680bd), fb(0xbbfe9c612f8a4102), fb(0x3fefff2ce2f3fd15), fb(0xbc762ccf8bdb122f)], // 74
    [fb(0x3f8d738ffeab52ec), fb(0xbc1c110bc5257c8c), fb(0x3fefff27246e9a52), fb(0xbc416d6346f3f55f)], // 75
    [fb(0x3f8dd81539955d20), fb(0xbc2eaf6d880c7f01), fb(0x3fefff21522c808b), fb(0x3c7a190e2d2eaf6e)], // 76
    [fb(0x3f8e3c9a62169dcb), fb(0x3c26666333e35e32), fb(0x3fefff1b6c2db358), fb(0xbc7f4664a9ad833c)], // 77
    [fb(0x3f8ea11f77f1136e), fb(0xbc17a617506aacb8), fb(0x3fefff157272365b), fb(0x3c644ef643864760)], // 78
    [fb(0x3f8f05a47ae6bc91), fb(0xbbf0935bb936be66), fb(0x3fefff0f64fa0d45), fb(0xbc7ac9acb9bb8d2d)], // 79
    [fb(0x3f8f6a296ab997cb), fb(0xbc2f2943d8fe7033), fb(0x3fefff0943c53bd1), fb(0xbc847399f361d158)], // 80
    [fb(0x3f8fceae472ba3bc), fb(0xbbf3a08b18043960), fb(0x3fefff030ed3c5c7), fb(0xbc82504b1dc047e8)], // 81
    [fb(0x3f90199987ff6f89), fb(0x3c38daccef2e2556), fb(0x3feffefcc625aefb), fb(0x3c4fbb0dd6af1603)], // 82
    [fb(0x3f904bdbe27aa444), fb(0x3c30c87a06c6956d), fb(0x3feffef669bafb4e), fb(0xbc5b5a4e5318177b)], // 83
    [fb(0x3f907e1e32e86f72), fb(0xbc3f40145dde0463), fb(0x3feffeeff993aeac), fb(0xbc599db334be163b)], // 84
    [fb(0x3f90b0607929d07a), fb(0x3c2f989b780d54e7), fb(0x3feffee975afcd0e), fb(0xbc82df82b9320ecd)], // 85
    [fb(0x3f90e2a2b51fc6ce), fb(0xbc22ca118aaa212f), fb(0x3feffee2de0f5a78), fb(0x3c59daa217acc1b0)], // 86
    [fb(0x3f9114e4e6ab51e2), fb(0x3c110d53d5e0ffc5), fb(0x3feffedc32b25afc), fb(0xbbfbb89d7b0d4487)], // 87
    [fb(0x3f9147270dad7133), fb(0x3c08769e00e01800), fb(0x3feffed57398d2b7), fb(0xbc80bb0db6384cf3)], // 88
    [fb(0x3f9179692a072443), fb(0x3c3d40c977f2fe27), fb(0x3feffecea0c2c5d2), fb(0xbc87a22a10e4cb4f)], // 89
    [fb(0x3f91abab3b996a9d), fb(0xbc3e21a2ef2391d3), fb(0x3feffec7ba303882), fb(0x3c7b209ec7100e1a)], // 90
    [fb(0x3f91dded424543ce), fb(0x3c22ecdb3d03a70b), fb(0x3feffec0bfe12f0a), fb(0x3c78b1483b4090ed)], // 91
    [fb(0x3f92102f3debaf6f), fb(0xbc145a1ea37e10ea), fb(0x3feffeb9b1d5adb7), fb(0x3c8d5b363c2f4370)], // 92
    [fb(0x3f9242712e6dad1c), fb(0x3c30317d8ce1d1b9), fb(0x3feffeb2900db8e4), fb(0x3c618f6a66301d60)], // 93
    [fb(0x3f9274b313ac3c7a), fb(0x3c3b29e49acf7e7e), fb(0x3feffeab5a8954f6), fb(0x3c80652cfe4cebfa)], // 94
    [fb(0x3f92a6f4ed885d35), fb(0xbc35a8a13d8f9966), fb(0x3feffea411488660), fb(0x3c8b77e2735c69f9)], // 95
    [fb(0x3f92d936bbe30efd), fb(0x3c2b5f91ee371d64), fb(0x3feffe9cb44b51a1), fb(0x3c75b43366df6670)], // 96
    [fb(0x3f930b787e9d518d), fb(0x3c3b620c940df42a), fb(0x3feffe954391bb43), fb(0x3c7ddd5377beef1a)], // 97
    [fb(0x3f933dba359824a6), fb(0xbc26c4020b5f32a8), fb(0x3feffe8dbf1bc7de), fb(0xbc8cc391831cbcd0)], // 98
    [fb(0x3f936ffbe0b4880e), fb(0x3c24b88026f4e07b), fb(0x3feffe8626e97c13), fb(0x3c7cdb27525fcaf7)], // 99
    [fb(0x3f93a23d7fd37b96), fb(0xbc3dd9e849a0a5d5), fb(0x3feffe7e7afadc94), fb(0xbc8db1225b06b02c)], // 100
    [fb(0x3f93d47f12d5ff12), fb(0x3c3add51eab25dd4), fb(0x3feffe76bb4fee1a), fb(0xbc60c43dae9e89c6)], // 101
    [fb(0x3f9406c0999d1263), fb(0xbc2cf7cbeea1ddcf), fb(0x3feffe6ee7e8b56e), fb(0x3c57ecac48a7b7bc)], // 102
    [fb(0x3f9439021409b56c), fb(0x3c181352c74c7ae2), fb(0x3feffe6700c53764), fb(0xbc84f502302afd49)], // 103
    [fb(0x3f946b4381fce81b), fb(0x3c3ff9f89fb65be3), fb(0x3feffe5f05e578db), fb(0xbc7b71f7469ecc13)], // 104
    [fb(0x3f949d84e357aa66), fb(0xbc29143373e36698), fb(0x3feffe56f7497ec0), fb(0xbc8ddf92a8d40b30)], // 105
    [fb(0x3f94cfc637fafc48), fb(0xbc3a3541bb15d5ba), fb(0x3feffe4ed4f14e0a), fb(0x3c7e48478b2f0ba1)], // 106
    [fb(0x3f9502077fc7ddc5), fb(0x3c325236a8347d96), fb(0x3feffe469edcebbf), fb(0x3c88fc7557d6bc53)], // 107
    [fb(0x3f953448ba9f4eeb), fb(0x3c2e6adde30ec695), fb(0x3feffe3e550c5cf0), fb(0xbc858fb3ae72192e)], // 108
    [fb(0x3f956689e8624fce), fb(0xbc2b967a2d3ee5b3), fb(0x3feffe35f77fa6b8), fb(0xbc6a7661b121429a)], // 109
    [fb(0x3f9598cb08f1e089), fb(0x3c356f9c34c216b4), fb(0x3feffe2d8636ce41), fb(0x3c6b4243168ab5a4)], // 110
    [fb(0x3f95cb0c1c2f0142), fb(0x3c3c2c42ac4d436c), fb(0x3feffe250131d8c0), fb(0x3c8ed2d09d5789d3)], // 111
    [fb(0x3f95fd4d21fab226), fb(0xbc20c0a91c37851c), fb(0x3feffe1c6870cb77), fb(0x3c889aa14768323e)], // 112
    [fb(0x3f962f8e1a35f369), fb(0xbc344ab93e20133f), fb(0x3feffe13bbf3abb3), fb(0x3c86721eee289b1d)], // 113
    [fb(0x3f9661cf04c1c548), fb(0x3c33d6cc7ce6ff57), fb(0x3feffe0afbba7ece), fb(0x3c66ecd981464044)], // 114
    [fb(0x3f96940fe17f280b), fb(0xbc384bf8988cc420), fb(0x3feffe0227c54a2d), fb(0x3c8eeed1d70e13e2)], // 115
    [fb(0x3f96c650b04f1bfe), fb(0x3ba53e382471fa0c), fb(0x3feffdf940141344), fb(0xbc8a63fc248c2dac)], // 116
    [fb(0x3f96f8917112a179), fb(0x3c3b6de0ad6e9445), fb(0x3feffdf044a6df8f), fb(0xbc8798cfb498d8b1)], // 117
    [fb(0x3f972ad223aab8dd), fb(0xbc3cf17a0ad13b4a), fb(0x3feffde7357db499), fb(0x3c409337deb3ff5a)], // 118
    [fb(0x3f975d12c7f86290), fb(0xbc307d8473495aae), fb(0x3feffdde129897f9), fb(0x3c844e8786dedb6b)], // 119
    [fb(0x3f978f535ddc9f04), fb(0x3c2b194ad9b1aa97), fb(0x3feffdd4dbf78f52), fb(0x3c8216679a26323d)], // 120
    [fb(0x3f97c193e5386eb4), fb(0x3c0725118c2860e7), fb(0x3feffdcb919aa053), fb(0xbc755c1ce37ae6f6)], // 121
    [fb(0x3f97f3d45decd222), fb(0x3c32fe8cfe80809d), fb(0x3feffdc23381d0b6), fb(0x3c600d15f9603fed)], // 122
    [fb(0x3f982614c7dac9db), fb(0x3c0e95f011ac18c6), fb(0x3feffdb8c1ad2643), fb(0x3c7e79471744a0cb)], // 123
    [fb(0x3f98585522e35674), fb(0xbc3d5a766c5894c1), fb(0x3feffdaf3c1ca6ce), fb(0xbc79c0f0834fa422)], // 124
    [fb(0x3f988a956ee7788a), fb(0x3c219dfbd12ff726), fb(0x3feffda5a2d05835), fb(0x3c86dbc405ff8e25)], // 125
    [fb(0x3f98bcd5abc830c7), fb(0xbc3f7d609f14a311), fb(0x3feffd9bf5c84066), fb(0xbc835167a6ce828d)], // 126
    [fb(0x3f98ef15d9667fda), fb(0xbc3668dc9ba82a70), fb(0x3feffd9235046557), fb(0xbc7c3c6e16616fb2)], // 127
];

/// -0x1.921fb54442d18p-13
const PIH: f64 = fb(0xbf2921fb54442d18);
/// -0x1.1a62633145c07p-67
const PIL: f64 = fb(0xbbc1a62633145c07);

/// Argument reduction for the accurate path (`reduce_large_acc`).
/// Return (k, r, neg) such that x/(2pi) mod 1 = k/2^13 + r/2^128 + eps with
/// |r/2^128| <= 2^-14 and 0 <= eps < 2^-128 + 2^-139 < 2^-127.999;
/// neg = 0 if r >= 0, neg = 1 if r < 0 (r holds |r|).
#[inline]
fn reduce_large_acc(x: f64) -> (u64, u128, i32) {
    let tu = x.to_bits();
    let e = ((tu >> 52) & 0x7ff) as i32;
    let m: u64 = (1u64 << 52) | (tu & 0x000f_ffff_ffff_ffff);
    // x = m * 2^(e-1075)
    let i: i32 = -1 + (e - 947) / 64; // i = ceil((e-1074)/64)
    let f = (e - 1011) & 0x3f;
    // shift the table entries
    let i1 = (i + 1) as usize;
    let mut v0: u64 = if i >= 0 { T[i as usize] } else { 0 };
    let mut v1: u64 = T[i1];
    let mut v2: u64 = T[i1 + 1];
    if f != 0 {
        v0 = (v0 << f) | (v1 >> (64 - f));
        v1 = (v1 << f) | (v2 >> (64 - f));
        v2 = (v2 << f) | (T[i1 + 2] >> (64 - f));
    }
    // m*V0 contributes to 64 bits to the fractional part of x/(2pi),
    // from weight 2^-1 to 2^-64, m*V1 contributes to 53+64 bits from weight
    // 2^-12 to 2^-128, m*V2 contributes to 53+64 bits from weight 2^-76 to 2^-192,
    // the ignored part m*(V3+V4+...) contributes to less than 2^-139
    let mut u: u128 = u128::from(v1) | (u128::from(v0) << 64);
    u = u128::from(m).wrapping_mul(u);
    let v: u128 = u128::from(m) * u128::from(v2);
    u = u.wrapping_add(v >> 64); // add contribution of m*V2 from 2^-76 to 2^-128
    // the ignored part of v contributes to less than 2^-128
    let mut k: u64 = (u >> (128 - 13)) as u64;
    const MASK: u128 = (0x0007_ffff_ffff_ffff_u128 << 64) | 0xffff_ffff_ffff_ffff_u128;
    u &= MASK; // ignore leading 13 bits
    // round k to nearest to have |r/2^128| < 2^-14
    let neg = (u >> 114) as i32;
    if neg != 0 {
        // add 1 to k and subtract 1/2^13 to r
        k = (k + 1) & ((1u64 << 13) - 1);
        u = MASK.wrapping_add(1).wrapping_sub(u);
    }
    (k, u, neg)
}

/// For 0 <= i <= 32, S1u[i] contains a 128-bit unsigned integer m such that
/// m/2^128 approximates sin(pi*i/2^6) to nearest. The entry for i=32 is capped
/// to 2^128-1. Generated by computeS1u() from sin.sage.
#[rustfmt::skip]
static S1U: [u128; 33] = [
    0x0000000000000000_0000000000000000, // 0
    0x0c8fb2f886ec09f3_76a17954b2b7c517, // 1
    0x1917a6bc29b42be1_d8e72d912977ee71, // 2
    0x259020dd1cc27444_c002a2684781f080, // 3
    0x31f17078d34c156c_9732300393f33614, // 4
    0x3e33f2f642be355e_90887712e9dc9663, // 5
    0x4a5018bb567c16a2_d725d3b9ed35fbaa, // 6
    0x563e69d6ac7f73f8_408fca9cc277fc1f, // 7
    0x61f78a9abaa58b46_98916152cf7eee1c, // 8
    0x6d744027857300ad_9b165cba0c171818, // 9
    0x78ad74e01bd8ec78_362474f1a105878f, // 10
    0x839c3cc917ff6cb4_bfd79717f2880abf, // 11
    0x8e39d9cd73464364_bba4cfecbff54867, // 12
    0x987fbfe70b81a708_19cec845ac87a5c6, // 13
    0xa267992848eeb0c0_3b5167ee359a234e, // 14
    0xabeb49a46764fd15_1becda8089c1a94c, // 15
    0xb504f333f9de6484_597d89b3754abe9f, // 16
    0xbdaef913557d76f0_ac85320f528d6d5d, // 17
    0xc5e40358a8ba05a7_43da25d99267326b, // 18
    0xcd9f023f9c3a059e_23af31db7179a4aa, // 19
    0xd4db3148750d1819_f630e8b6dac83e69, // 20
    0xdb941a28cb71ec87_2c19b63253da43fc, // 21
    0xe1c5978c05ed8691_f4e8a8372f8c5810, // 22
    0xe76bd7a1e63b9786_125129529d48a92f, // 23
    0xec835e79946a3145_7e610231ac1d6181, // 24
    0xf1090827b43725fd_67127db35b287316, // 25
    0xf4fa0ab6316ed2ec_163c5c7f03b718c5, // 26
    0xf853f7dc9186b952_c7adc6b4988891bb, // 27
    0xfb14be7fbae58156_2172a361fd2a722f, // 28
    0xfd3aabf84528b50b_eae6bd951c1dabbe, // 29
    0xfec46d1e89292cf0_41390efdc726e9ef, // 30
    0xffb10f1bcb6bef1d_421e8edaaf59453e, // 31
    0xffffffffffffffff_ffffffffffffffff, // 32
];

/// For 0 <= i < 64, S2u[i] contains a 128-bit unsigned integer m such that
/// m/2^128 approximates sin(pi*i/2^12) to nearest. Generated by computeS2u() from
/// sin.sage.
#[rustfmt::skip]
static S2U: [u128; 64] = [
    0x0000000000000000_0000000000000000, // 0
    0x003243f655d966c0_c3b0abf2b35a62f8, // 1
    0x006487eabb991cb6_11ad1d7b5620834a, // 2
    0x0096cbdb41258434_c4a32c4560d01e70, // 3
    0x00c90fc5f66525d2_57480f7956b64707, // 4
    0x00fb53a8eb3ec385_328421cf8014d698, // 5
    0x012d97822f996bc4_f96857b5aa8f6208, // 6
    0x015fdb4fd35c8caa_d230a30592f25049, // 7
    0x01921f0fe6700711_ab967fe6b7a9b037, // 8
    0x01c462c078bc41b6_7db6e69ab68f0d10, // 9
    0x01f6a65f9a2a3c58_85cede1f0314607b, // 10
    0x0228e9eb5aa3a2d9_7c11ebc1b33cafb5, // 11
    0x025b2d61ca12e05d_c2dd9c015a46e767, // 12
    0x028d70c0f863326c_8e8d6151676ae94d, // 13
    0x02bfb406f580bc10_053205a54588f0ab, // 14
    0x02f1f731d15898f5_556febf4862da970, // 15
    0x03243a3f9bd8f08c_c3c75f41b6ce7aa8, // 16
    0x03567d2e64f10929_ad8a2d0c1a9e3a8c, // 17
    0x0388bffc3c915b22_80c1c97a65cf2291, // 18
    0x03bb02a732aba3f0_a8493bf9c07f7765, // 19
    0x03ed452d5732f950_6b5e1380c7e6bddb, // 20
    0x041f878cba1bdc60_bfeba221fb9d05be, // 21
    0x0451c9c36b5c4cc3_0ed3c01c030008bb, // 22
    0x04840bcf7aebdbba_e97857207bc589da, // 23
    0x04b64daef8c3bf4d_afc8f71b8eb233ee, // 24
    0x04e88f5ff4dee562_2617b65f5a2a8f5a, // 25
    0x051ad0e07f3a06df_f9f89fb65be2a455, // 26
    0x054d122ea7d3bacf_356ef187634a531d, // 27
    0x057f53487eac8977_9fab71e43d71e2b7, // 28
    0x05b1942c13c6ff80_0a9f1c1238fd05d5, // 29
    0x05e3d4d77727c10d_8ca56cd8d54b97f0, // 30
    0x06161548b8d59ce2_a58993a76b3f4ffe, // 31
    0x0648557de8d99f7e_4e29cf6e5fed0679, // 32
    0x067a9575173f263a_f1fc3edb7a0f1757, // 33
    0x06acd52c5413f26d_51b86c83422cbab4, // 34
    0x06df14a1af683c83_4e68e062eb177e2d, // 35
    0x071153d3394ec722_9c28010f1c93a252, // 36
    0x074392bf01dcf247_5bcb8fd41cb1096f, // 37
    0x0775d163192ace62_9ac20c033d7f5cc3, // 38
    0x07a80fbd8f532f78_b8654aa824578975, // 39
    0x07da4dcc7473c03f_b00590e675e4e556, // 40
    0x080c8b8dd8ad153d_46f0804dae5f13ba, // 41
    0x083ec8ffcc22bfe5_1db725856ff8c284, // 42
    0x087106205efb61b6_a3f67ad05a5be69e, // 43
    0x08a342eda160bf5a_ede5b1068d174bea, // 44
    0x08d57f65a37fd3c2_6aed92d34c17df54, // 45
    0x0907bb867588e342_7c8c5732d89e910b, // 46
    0x0939f74e27af8eb2_ecc93966728e412d, // 47
    0x096c32baca2ae68b_437b2dd49d5fca3c, // 48
    0x099e6dca6d357dff_f9a60c93317892c4, // 49
    0x09d0a87b210d7e1f_8a318ba775fd7b04, // 50
    0x0a02e2caf5f4b8ef_5f3d645e787b94f2, // 51
    0x0a351cb7fc30bc88_9b56007d16d4ad5a, // 52
    0x0a675640440ae634_bdcd0d6bb4b9cd3e, // 53
    0x0a998f61ddd0758a_217954ed6093c44c, // 54
    0x0acbc81ad9d29f88_55213c653bfb79b7, // 55
    0x0afe00694866a1b4_4cd34d2751c2e1da, // 56
    0x0b30384b39e5d534_6b7029d39efd7682, // 57
    0x0b626fbebeadc1ec_63a95642f565102f, // 58
    0x0b94a6c1e7203198_efb8391d83d6da18, // 59
    0x0bc6dd52c3a342eb_5f10bfca3d646401, // 60
    0x0bf9136f64a17ca4_f9530f050886d756, // 61
    0x0c2b4915da89e0b2_35bfac0f965612e2, // 62
    0x0c5d7e4435cfff45_c6718c1dfd2aa612, // 63
];

// Table C1u is not needed, since cos(x) = sin(pi/2+x) for 0 <= x < pi/2,
// and cos(x) = -sin(x-pi/2) for pi/2 <= x < pi, thus
// C1u[i] = S1u[32+i] for 0 <= i < 32, and C1u[i] = -S1u[i-32] for 32 <= i < 64.

/// For 0 <= i < 64, C2u[i] contains a 128-bit unsigned integer m such that
/// m/2^128 approximates cos(pi*i/2^12) to nearest. Generated by computeC2u() from
/// sin.sage.
#[rustfmt::skip]
static C2U: [u128; 64] = [
    0xffffffffffffffff_ffffffffffffffff, // 0
    0xfffffb10b0d19f76_491a703231e0a12e, // 1
    0xffffec42c3773235_ec9e2e75cb525f2c, // 2
    0xffffd3963882d553_6276b75b91de105e, // 3
    0xffffb10b10e80e95_3031437d7eccb9df, // 4
    0xffff84a14dfbcc6a_858425d8b397dee6, // 5
    0xffff4e58f17465de_177338053fd93920, // 6
    0xffff0e31fd699a85_3a11d60588d8b96e, // 7
    0xfffec42c7454926b_38e310779edfec68, // 8
    0xfffe7048590fddf8_edd8e1034213f22b, // 9
    0xfffe1285aed775d8_96f351efc65556cf, // 10
    0xfffdaae47948bad5_ea80aedd6a19710f, // 11
    0xfffd3964bc6275ba_69fff9ae0dedb047, // 12
    0xfffcbe067c84d725_f3a703b987eca44a, // 13
    0xfffc38c9be717763_928db07a0e70ba36, // 14
    0xfffba9ae874b563a_8d800bed6653dcba, // 15
    0xfffb10b4dc96dabb_b47903f7a19f8ee2, // 16
    0xfffa6ddcc439d30a_ecc7b9244a48eb19, // 17
    0xfff9c126447b7424_fbe18032d0016082, // 18
    0xfff90a91640459a1_90e2d2eaf6da4d1e, // 19
    0xfff84a1e29de8571_8cc193c5d508e13f, // 20
    0xfff77fcc9d755f99_89332d07a713477e, // 21
    0xfff6ab9cc695b5e8_9e4938f661aa140c, // 22
    0xfff5cd8ead6dbbab_66c785e86dfbb75f, // 23
    0xfff4e5a25a8d095b_43366df666fd54ff, // 24
    0xfff3f3d7d6e49c49_dbb49f29fa872a83, // 25
    0xfff2f82f2bc6d648_e08b96133ecce0bd, // 26
    0xfff1f2a862e77d4e_098a31bcda3def20, // 27
    0xfff0e343865bbb13_5428ed0647c9e5d1, // 28
    0xffefca00a09a1cb3_807b6e7a4a723dae, // 29
    0xffeea6dfbc7a9242_ccf344c647917821, // 30
    0xffed79e0e5366e63_f0f7cb05bde024f1, // 31
    0xffec4304266865d9_5657552366961732, // 32
    0xffeb02498c0c8f12_9195e99fbca2f10a, // 33
    0xffe9b7b1228061b6_191df31aaa6f7f45, // 34
    0xffe8633af682b627_3b57790bf77b6b2d, // 35
    0xffe704e71533c508_53aa9423bb0adc21, // 36
    0xffe59cb58c1526b9_3e71f7d99688082a, // 37
    0xffe42aa66909d2d2_0be28fbec7cfb8a6, // 38
    0xffe2aeb9ba561f99_f1ed54343fe7be24, // 39
    0xffe128ef8e9fc17a_7d209f32d42d864e, // 40
    0xffdf9947f4edca6f_008e6ee05573d420, // 41
    0xffddffc2fca8a970_44bd28b8d85b530a, // 42
    0xffdc5c60b59a29dc_75a8951fc304b914, // 43
    0xffdaaf212fed72db_4fd8f038449ec436, // 44
    0xffd8f8047c2f06be_8c9611f0b1d8f023, // 45
    0xffd7370aab4cc25e_8d3cd437dc7fa9d2, // 46
    0xffd56c33ce95dc73_45bd035edb0a65f5, // 47
    0xffd3977ff7bae4e9_664649b4d541b9c5, // 48
    0xffd1b8ef38cdc433_c42aac754bedcfde, // 49
    0xffcfd081a441ba99_01fd552bf146b4a6, // 50
    0xffcdde374ceb5f7d_76f487bb853a6989, // 51
    0xffcbe2104600a0a9_5595ca3f421ae09c, // 52
    0xffc9dc0ca318c18b_11b369083a7a62e5, // 53
    0xffc7cc2c782c5a76_05c2a6019679e41f, // 54
    0xffc5b26fd99557dd_579207cfe424dcb7, // 55
    0xffc38ed6dc0ef98b_1c676208aa3be545, // 56
    0xffc1616194b5d1d3_bc8d54e81d94f831, // 57
    0xffbf2a101907c4c5_965827f33d906c7c, // 58
    0xffbce8e27ee40754_e0aa07fcb29eef39, // 59
    0xffba9dd8dc8b1e83_ccfed60a91097c48, // 60
    0xffb848f3489ede86_e907d9a298ab1feb, // 61
    0xffb5ea31da2269e5_bfdfce09aea7ac02, // 62
    0xffb38194a87a3097_badfe70a1ef51116, // 63
];

/// Accurate path for |x| >= 2^31, also used when the moderate fast path fails for
/// |x| >= 2^-16 (`sin_large_accurate`).
#[cold]
#[inline(never)]
fn sin_large_accurate(x: f64) -> f64 {
    let (k, mut r, neg) = reduce_large_acc(x);

    // twopi/2^128 approximates 2pi/2^3
    const TWOPI: u128 = (0xc90f_daa2_2168_c234_u128 << 64) | 0xc4c6_628b_80dc_1cd1_u128;
    r = mh_uu(TWOPI, r << 3); // replace r by 2pi*r
    let u2 = mh_uu(r, r);
    let u4 = mh_uu(u2, u2);
    let u2h = u2 >> 64;

    // x/(2*pi) mod 1 = k/2^13 + r + eps with |r| <= 2^-14 and 0 <= eps < 2^-127.999
    // then sin(x) ~ sin(pi*k/2^12 + 2*pi*r)
    //             ~ sin(pi*k/2^12)*cos(2*pi*r) + cos(pi*k/2^12)*sin(2*pi*r)
    // Write k = 2^12*s + 2^6*i1 + i2 and t1=pi*i1/2^6, t2 = pi*i2/2^12
    // then sin(pi*k/2^12) = (-1)^s*[sin(t1)*cos(t2)+cos(t1)*sin(t2)]
    // and  cos(pi*k/2^12) = (-1)^s*[cos(t1)*cos(t2)-sin(t1)*sin(t2)]
    let mut sbit: usize = if x > 0.0 { 0 } else { 1 };
    sbit ^= (k >> 12) as usize;
    let i1 = ((k >> 6) & 0x3f) as usize;
    let i2 = (k & 0x3f) as usize;

    // approximate |sin(z)| in s1u/2^128
    let s1 = if i1 <= 32 { S1U[i1] } else { S1U[64 - i1] }; // use sin(pi-x) = sin(x)
    // since cos(x) = sin(x+pi/2), we have |C1u[i1]| = |S1u[(i1+32) mod 64]|
    let c1 = if i1 <= 32 { S1U[32 - i1] } else { S1U[i1 - 32] };
    let mut s1u = mh_uu(s1, C2U[i2]);
    let mut t = mh_uu(c1, S2U[i2]);
    // if i1 >= 32, we have to subtract t
    s1u = if i1 < 32 {
        s1u.wrapping_add(t)
    } else {
        s1u.wrapping_sub(t)
    };

    // approximate cos(z) in c1u/2^128
    let mut c1u = mh_uu(c1, C2U[i2]);
    t = mh_uu(s1, S2U[i2]);
    c1u = if i1 < 32 {
        c1u.wrapping_sub(t)
    } else {
        c1u.wrapping_add(t)
    };

    let sr = eval_ps(r, u2, u2h, u4); // Sr/2^128 approximates |sin(2*pi*r)|
    let cr = eval_pc(u2, u2h, u4); // Cr/2^128 approximates cos(2*pi*r)

    // now combine: sin(x) ~ s1*C + c1*S
    s1u = mh_uu(s1u, cr);
    c1u = mh_uu(c1u, sr);

    // s1u/2^128 approximates sin(z)*cos(r) which is always >= 0, while
    // c1u/2^128 approximates cos(z)*sin(r), where cos(z) > 0 for i1 < 32,
    // and cos(z) <= for i1 >= 32, and sign(r) has the sign of r.
    if (i32::from(i1 < 32) ^ neg) != 0 {
        // r >= 0
        s1u = s1u.wrapping_add(c1u);
    } else if s1u < c1u {
        s1u = c1u - s1u;
        sbit ^= 1;
    } else {
        s1u -= c1u;
    }
    u128_tod(s1u, sbit)
}

/// Accurate path for |x| < 2^-16 (`sin_small_accurate`).
#[inline]
fn sin_small_accurate(x: f64) -> f64 {
    // x + (c3h+c3l)*x^3 + c5*x^5 approximates sin(x) on [0,2^-16] with relative
    // error < 2^-112.743, cf sinsmall_acc.sollya, where c3h = c[0], c3l = c[1]
    // and c5 = c[2].
    // {-0x1.5555555555555p-3, -0x1.55554b00de7e8p-57, 0x1.111111110848p-7}
    const C: [f64; 3] = [
        fb(0xbfc5555555555555),
        fb(0xbc655554b00de7e8),
        fb(0x3f81111111108480),
    ];
    let x2h = x * x;
    let x2l = x.mul_add(x, -x2h);
    let mut h = C[2] * x2h; // relative error less than ulp(c5*x^4)/ulp(x) ~ 2^-123
    h += C[1]; // relative error less than ulp(c3l*x^2)/ulp(x) ~ 2^-141
    let mut l;
    (h, l) = fasttwosum(C[0], h);
    (h, l) = muldd(h, l, x2h, x2l);
    (h, l) = muldd(h, l, x, 0.0);
    let t;
    (h, t) = fasttwosum(x, h);
    l += t;
    h + l
}

/// Fast path for 2^-26 <= |x| < 2^31 (`cr_sin_moderate`); the proof of correctness
/// is in sin.pdf.
#[inline]
fn cr_sin_moderate(x: f64, mut sbit: usize) -> f64 {
    let ax = x.abs();
    // 0x1.45f306dc9c883p+12
    const INVPI: f64 = fb(0x40b45f306dc9c883);
    // |invpi/2^14 - 1/pi| < 2^-55.496
    let k = roundeven_finite(INVPI * ax);
    // |2^14*(pih + pil) + pi| < 2^-108.041
    let rh = k.mul_add(PIH, ax); // rh is exact
    let rl = k * PIL;

    let r = rh + rl; // |r| < 2^-13.339 (see sin.pdf)
    let r2 = r * r;
    let j = k as i64;
    sbit ^= ((j >> 14) & 1) as usize; // reduction by an odd multiple of pi?
    let i1 = ((j >> 7) & 0x7f) as usize;
    let i2 = (j & 0x7f) as usize;
    let (s1h, s1l) = muldd(U1[i1][0], U1[i1][1], U2[i2][2], U2[i2][3]);
    let (s2h, s2l) = muldd(U2[i2][0], U2[i2][1], U1[i1][2], U1[i1][3]);
    // Sh + Sl and Ch in sin.c
    let (sh_tab, sl_tab) = fastsum(s1h, s1l, s2h, s2l);
    let ch_tab = U1[i1][2] * U2[i2][2] - U1[i1][0] * U2[i2][0];

    // for |r| <= 2^-13.339, the polynomial r - 0x1.55555553068fp-3 * r^3
    // approximates sin(r) with absolute error < 2^-76.494, and the polynomial
    // -0.5 * r^2 + 0x1.55555553bfd3p-5 * r^4 approximates cos(r)-1 with
    // absolute error < 2^-92.723 (cf sinmoderate.sollya)
    let sh = r * (1.0 - fb(0x3fc55555553068f0) * r2);
    let ch = r2 * (-0.5 + fb(0x3fa55555553bfd30) * r2);
    let mut fh = sh_tab;
    let mut fl = sl_tab + sh_tab * ch + ch_tab * sh;
    const SGN: [f64; 2] = [1.0, -1.0];
    // 0x1.dep-64
    const EPS: f64 = fb(0x3bfde00000000000);
    fh *= SGN[sbit]; // Sgn[sbit] * fh (IEEE multiplication is commutative)
    fl *= SGN[sbit];
    let lb = fh + (fl - EPS);
    let ub = fh + (fl + EPS);
    if ub == lb {
        return lb;
    }
    // 0x1p-16
    if x.abs() < fb(0x3ef0000000000000) {
        return sin_small_accurate(x);
    }
    sin_large_accurate(x)
}

/// Fast path for |x| >= 2^31 (`cr_sin_large`).
#[inline(never)]
fn cr_sin_large(x: f64) -> f64 {
    let ax = x.abs();
    let (j, r) = reduce_large(ax);
    // now x/(2pi) ~ k + j/2^15 + r with 0 <= r < 2^-15
    let mut sbit: usize = if x > 0.0 { 0 } else { 1 };

    let r2 = r * r;
    sbit ^= (j >> 14) as usize; // reduction by an odd multiple of pi?
    let i1 = ((j >> 7) & 0x7f) as usize;
    let i2 = (j & 0x7f) as usize;
    let (s1h, s1l) = muldd(U1[i1][0], U1[i1][1], U2[i2][2], U2[i2][3]);
    let (s2h, s2l) = muldd(U2[i2][0], U2[i2][1], U1[i1][2], U1[i1][3]);
    // Sh + Sl and Ch in sin.c
    let (sh_tab, sl_tab) = fastsum(s1h, s1l, s2h, s2l);
    let ch_tab = U1[i1][2] * U2[i2][2] - U1[i1][0] * U2[i2][0];

    // 0x1.921fb54442d18p2, 0x1.4abbcdb6b26d1p5
    let sh = r * (fb(0x401921fb54442d18) - fb(0x4044abbcdb6b26d1) * r2);
    // -0x1.3bd3cc9be45dep4, 0x1.03c1eee483083p6
    let ch = r2 * (fb(0xc033bd3cc9be45de) + fb(0x40503c1eee483083) * r2);
    let mut fh = sh_tab;
    let mut fl = sl_tab + sh_tab * ch + ch_tab * sh;
    const SGN: [f64; 2] = [1.0, -1.0];
    fh *= SGN[sbit]; // Sgn[sbit] * fh (IEEE multiplication is commutative)
    fl *= SGN[sbit];
    // fails with eps=0x1.7fp-64 and x=0x1.54a9ad28f0a25p+225 (rndz, no FMA)
    // 0x1.01p-63
    const EPS: f64 = fb(0x3c00100000000000);
    let lb = fh + (fl - EPS);
    let ub = fh + (fl + EPS);
    if lb == ub {
        return lb;
    }
    sin_large_accurate(x)
}

/// Correctly rounded sine: the binary64 value nearest to the exact sin(x) (ties to
/// even), hence bit-identical on every platform. Port of CORE-MATH `cr_sin`.
///
/// `sin(±0) = ±0`; `sin(±inf)` and `sin(NaN)` return the default quiet NaN
/// (`0x7ff8000000000000`), as in CORE-MATH.
#[inline]
pub fn sin(x: f64) -> f64 {
    let tu = x.to_bits();
    let e = ((tu >> 52) & 0x7ff) as i32;
    // deal with tiny x to avoid underflow
    if e < 0x3ff - 26 {
        // |x| < 2^-26
        // for |x| <= 0x1.7137449123ef6p-26  |sin(x) - x| < 1/2 ulp
        let au = tu << 1;
        if au == 0 {
            return x;
        }
        // Taylor expansion of sin(x) is x - x^3/6 around zero
        // 0x1p-54
        return x.mul_add(fb(0xbc90000000000000), x);
    }
    if e < 0x3ff + 31 {
        // |x| < 2^31
        return cr_sin_moderate(x, (tu >> 63) as usize);
    }
    if e == 0x7ff {
        // NaN, +Inf and -Inf (the invalid exception of the C code is not modelled)
        return f64::from_bits(0x7ff8_0000_0000_0000);
    }
    // now |x| >= 2^31
    cr_sin_large(x)
}

#[cfg(test)]
mod tests {
    use super::sin;

    #[test]
    fn zeros_keep_their_sign() {
        assert_eq!(sin(0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(sin(-0.0).to_bits(), (-0.0f64).to_bits());
    }

    #[test]
    fn non_finite_inputs_give_nan() {
        assert!(sin(f64::INFINITY).is_nan());
        assert!(sin(f64::NEG_INFINITY).is_nan());
        assert!(sin(f64::NAN).is_nan());
        assert!(sin(-f64::NAN).is_nan());
    }

    #[test]
    fn tiny_and_subnormal_inputs() {
        // sin(x) rounds to x for |x| <= 0x1.7137449123ef6p-26
        for x in [
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            1e-300,
            -1e-20,
            1e-10,
        ] {
            assert_eq!(sin(x).to_bits(), x.to_bits(), "{x:e}");
        }
    }

    /// Correctly rounded values (mpmath at 2000 bits, rounded to nearest-even).
    #[test]
    fn known_values() {
        let cases: &[(u64, u64)] = &[
            (0x3ff921fb54442d18, 0x3ff0000000000000), // sin(0x1.921fb54442d18p+0) = 0x1.0000000000000p+0
            (0x400921fb54442d18, 0x3ca1a62633145c07), // sin(0x1.921fb54442d18p+1) = 0x1.1a62633145c07p-53
            (0x401921fb54442d18, 0xbcb1a62633145c07), // sin(0x1.921fb54442d18p+2) = -0x1.1a62633145c07p-52
            (0x3ff0000000000000, 0x3feaed548f090cee), // sin(0x1.0000000000000p+0) = 0x1.aed548f090ceep-1
            (0x3fe0000000000000, 0x3fdeaee8744b05f0), // sin(0x1.0000000000000p-1) = 0x1.eaee8744b05f0p-2
            (0x3e50000000000000, 0x3e50000000000000), // sin(0x1.0000000000000p-26) = 0x1.0000000000000p-26
            (0x3e57137449123ef6, 0x3e57137449123ef6), // sin(0x1.7137449123ef6p-26) = 0x1.7137449123ef6p-26
            (0x3e46a09e667f3bcd, 0x3e46a09e667f3bcd), // sin(0x1.6a09e667f3bcdp-27) = 0x1.6a09e667f3bcdp-27
            (0x3ef0000000000000, 0x3eeffffffffaaaab), // sin(0x1.0000000000000p-16) = 0x1.ffffffffaaaabp-17
            (0x41dfffffffffffff, 0xbfef14f9325a7175), // sin(0x1.fffffffffffffp+30) = -0x1.f14f9325a7175p-1
            (0x41e0000000000000, 0xbfef14f913e9af98), // sin(0x1.0000000000000p+31) = -0x1.f14f913e9af98p-1
            (0x4480f0cf064dd592, 0xbfeb453ab76bf397), // sin(0x1.0f0cf064dd592p+73) = -0x1.b453ab76bf397p-1
            (0x7fe0000000000000, 0x3fe205248cbdb760), // sin(0x1.0000000000000p+1023) = 0x1.205248cbdb760p-1
            (0x7fefffffffffffff, 0x3f7452fc98b34e97), // sin(0x1.fffffffffffffp+1023) = 0x1.452fc98b34e97p-8
            (0x400005023d32fee5, 0x3fed109ad145c88f), // sin(0x1.005023d32fee5p+1) = 0x1.d109ad145c88fp-1
            (0x7fe61a3db8c8d129, 0xbc7dd15f96b823f2), // sin(0x1.61a3db8c8d129p+1023) = -0x1.dd15f96b823f2p-56
            (0x7526ac5b262ca1ff, 0x3c414ae72e6ba22f), // sin(0x1.6ac5b262ca1ffp+851) = 0x1.14ae72e6ba22fp-59
            (0x3f30009effd4beda, 0x3f30009efd29c4ac), // sin(0x1.0009effd4bedap-12) = 0x1.0009efd29c4acp-12
            (0x3ff00147eec5cfa5, 0x3feaeeb6d68887a5), // sin(0x1.00147eec5cfa5p+0) = 0x1.aeeb6d68887a5p-1
            (0x3e88000000000009, 0x3e87ffffffffffe5), // sin(0x1.8000000000009p-23) = 0x1.7ffffffffffe5p-23
            (0x4022400000000000, 0x3fd2e653d9743156), // sin(0x1.2400000000000p+3) = 0x1.2e653d9743156p-2
            (0x408f400000000000, 0x3fea75cc150a206b), // sin(0x1.f400000000000p+9) = 0x1.a75cc150a206bp-1
        ];
        for &(x, y) in cases {
            let got = sin(f64::from_bits(x));
            assert_eq!(
                got.to_bits(),
                y,
                "sin({:e}) = {:e}, expected {:e}",
                f64::from_bits(x),
                got,
                f64::from_bits(y)
            );
            // odd symmetry
            assert_eq!(
                sin(-f64::from_bits(x)).to_bits(),
                (-f64::from_bits(y)).to_bits()
            );
        }
    }
}

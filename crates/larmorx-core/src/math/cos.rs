//! Correctly rounded cosine for binary64 (`cos`).
//!
//! Port of CORE-MATH `src/binary64/cos/cos.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only: the rounding-mode, errno and floating-point exception code of
//! the C file is dropped (`subnormalize_dint` keeps its `FE_TONEAREST` branch). Algorithms,
//! tables and the order of every floating-point operation are kept exactly;
//! `__builtin_fma` is [`f64::mul_add`] (correctly rounded on every target), unsigned
//! integer arithmetic wraps like C, and hex-float constants are written as their exact
//! binary64 bit patterns.
//!
//! cos.c uses a different (older) algorithm than sin.c (128-bit `dint64_t` arithmetic for
//! the accurate path, a 2^-11 table for the fast path), so the two ports share no code.
//!
//! Original licence of cos.c (MIT):
//!
//! Copyright (c) 2022-2025 Paul Zimmermann and Tom Hubrecht
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

/// `f64` from its IEEE-754 bit pattern (tables are stored as exact bit patterns).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

// ****************** code copied from dint.h and pow.[ch] in cos.c ******************

/// 128-bit floating-point value `(hi/2^64 + lo/2^128) * 2^ex * (-1)^sgn` (`dint64_t`).
#[derive(Clone, Copy)]
struct Dint {
    hi: u64,
    lo: u64,
    ex: i64,
    sgn: u64,
}

impl Dint {
    const fn new(hi: u64, lo: u64, ex: i64, sgn: u64) -> Self {
        Self { hi, lo, ex, sgn }
    }

    /// The 128-bit significand (`r` member of the C union).
    #[inline(always)]
    fn r(&self) -> u128 {
        (u128::from(self.hi) << 64) | u128::from(self.lo)
    }

    #[inline(always)]
    fn set_r(&mut self, r: u128) {
        self.hi = (r >> 64) as u64;
        self.lo = r as u64;
    }
}

/// Extract both the mantissa and exponent of a double (`fast_extract`).
#[inline]
fn fast_extract(x: f64) -> (i64, u64) {
    let xu = x.to_bits();
    let e = ((xu >> 52) & 0x7ff) as i64;
    let m = (xu & (!0u64 >> 12)) + if e != 0 { 1u64 << 52 } else { 0 };
    (e - 0x3fe, m)
}

/// Return true if a = 0 (`dint_zero_p`).
#[inline]
fn dint_zero_p(a: &Dint) -> bool {
    a.hi == 0
}

#[inline]
fn cmp(a: i64, b: i64) -> i32 {
    i32::from(a > b) - i32::from(a < b)
}

#[inline]
fn cmpu128(a: u128, b: u128) -> i32 {
    i32::from(a > b) - i32::from(a < b)
}

/// ZERO is a dint64_t representation of 0, which ensures that dint_tod(ZERO) = 0.
const ZERO: Dint = Dint::new(0x0, 0x0, -1076, 0x0);
/// MAGIC is a dint64_t representation of 1/2^11.
const MAGIC: Dint = Dint::new(0x8000_0000_0000_0000, 0x0, -10, 0x0);

/// Compare the absolute values of a and b: -1 if |a| < |b|, 0 if |a| = |b|,
/// +1 if |a| > |b| (`cmp_dint_abs`).
#[inline]
fn cmp_dint_abs(a: &Dint, b: &Dint) -> i32 {
    if dint_zero_p(a) {
        return if dint_zero_p(b) { 0 } else { -1 };
    }
    if dint_zero_p(b) {
        return 1;
    }
    let c1 = cmp(a.ex, b.ex);
    if c1 != 0 { c1 } else { cmpu128(a.r(), b.r()) }
}

/// Add two dint64_t values, with error bounded by 2 ulps (ulp_128)
/// (more precisely 1 ulp when a and b have same sign, 2 ulps otherwise).
/// Moreover, when Sterbenz theorem applies, i.e., |b| <= |a| <= 2|b|
/// and a,b are of different signs, there is no error, i.e., r = a-b (`add_dint`).
/// The C version writes through a pointer that may alias an input; it reads every
/// input field it needs before writing the corresponding output field, so returning
/// the result by value is equivalent.
#[inline]
fn add_dint(a: &Dint, b: &Dint) -> Dint {
    if (a.hi | a.lo) == 0 {
        return *b;
    }

    let (a, b) = match cmp_dint_abs(a, b) {
        0 => {
            if (a.sgn ^ b.sgn) != 0 {
                return ZERO;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => (b, a), // |A| < |B|: swap operands
        _ => (a, b),
    };

    // From now on, |A| > |B| thus a->ex >= b->ex

    let aa = a.r();
    let mut bb = b.r();
    let k = (a.ex.wrapping_sub(b.ex)) as u64;

    if k > 0 {
        // the right shift x >> k is only defined for 0 <= k < n
        bb = if k < 128 { bb >> k } else { 0 };
    }

    let c: u128;
    let sgn = a.sgn;

    let mut rex = a.ex; // tentative exponent for the result

    if (a.sgn ^ b.sgn) != 0 {
        // a and b have different signs C = A + (-B)
        // Sterbenz case |a|/2 <= |b| <= |a| can occur only when:
        // * k=0: then B is not truncated, and C is exact below
        // * k=1 and ex>0 below: then we ensure C is exact
        let mut cc = aa.wrapping_sub(bb);
        let ch = (cc >> 64) as u64;
        // We can't have C=0 here since we excluded the case |A| = |B|
        let mut ex: u64 = if ch != 0 {
            u64::from(ch.leading_zeros())
        } else {
            64 + u64::from((cc as u64).leading_zeros())
        };
        // The error from the truncated part of B (1 ulp) is multiplied by 2^ex,
        // thus by 2 ulps when ex <= 1.
        if ex > 0 {
            if k == 1 {
                // Sterbenz case
                cc = (aa << ex).wrapping_sub(b.r() << (ex - 1));
            } else {
                cc = (aa << ex).wrapping_sub(bb << ex);
            }
            // see cos.c: the value of C, truncated to 128 bits, is the right one;
            // in some rare cases we need to shift by 1 bit to the left.
            rex = rex.wrapping_sub(ex as i64);
            ex = u64::from(((cc >> 64) as u64).leading_zeros());
            // Fall through with the code for ex = 0.
        }
        cc <<= ex;
        rex = rex.wrapping_sub(ex as i64);
        c = cc;
    } else {
        let mut cc = aa.wrapping_add(bb);
        if cc < aa {
            cc = (1u128 << 127) | (cc >> 1);
            rex += 1;
        }
        c = cc;
    }

    let mut r = Dint::new(0, 0, rex, sgn);
    r.set_r(c);
    r
}

/// Multiply two dint64_t numbers, with error bounded by 6 ulps on the 128-bit
/// floating-point numbers (`mul_dint`). The C version allows the output to alias an
/// input; it reads the inputs before overwriting them, so returning by value is
/// equivalent.
#[inline]
fn mul_dint(a: &Dint, b: &Dint) -> Dint {
    let bh = u128::from(b.hi);
    let bl = u128::from(b.lo);

    // compute the two middle terms
    let m1 = u128::from(a.hi) * bl;
    let m2 = u128::from(a.lo) * bh;

    // put the 128-bit product of the high terms in r
    let mut r = u128::from(a.hi) * bh;

    // there can be no overflow in the following addition since r <= (B-1)^2
    // with B=2^64, (m1>>64) <= B-1 and (m2>>64) <= B-1, thus the sum is
    // bounded by (B-1)^2+2*(B-1) = B^2-1
    r = r.wrapping_add((m1 >> 64).wrapping_add(m2 >> 64));

    // Ensure that r->hi starts with a 1
    let ex = (r >> 127) as u64;
    r <<= 1 - ex;

    // Exponent and sign
    // if ex=1, then ex(r) = ex(a) + ex(b)
    // if ex=0, then ex(r) = ex(a) + ex(b) - 1
    let mut out = Dint::new(
        0,
        0,
        a.ex.wrapping_add(b.ex)
            .wrapping_add(ex as i64)
            .wrapping_sub(1),
        a.sgn ^ b.sgn,
    );
    out.set_r(r);
    out
}

/// Multiply two dint64_t numbers, assuming the low part of b is zero,
/// with error bounded by 2 ulps (`mul_dint_21`).
#[inline]
fn mul_dint_21(a: &Dint, b: &Dint) -> Dint {
    let bh = u128::from(b.hi);
    let hi = u128::from(a.hi) * bh;
    let lo = u128::from(a.lo) * bh;

    // put the 128-bit product of the high terms in r, then add the middle term
    let mut r = hi.wrapping_add(lo >> 64);

    // Ensure that r->hi starts with a 1
    let ex = (r >> 127) as u64;
    r <<= 1 - ex;

    // Exponent and sign
    let mut out = Dint::new(
        0,
        0,
        a.ex.wrapping_add(b.ex)
            .wrapping_add(ex as i64)
            .wrapping_sub(1),
        a.sgn ^ b.sgn,
    );
    out.set_r(r);
    out
}

/// Convert a non-zero double to the corresponding dint64_t value (`dint_fromd`).
#[inline]
fn dint_fromd(b: f64) -> Dint {
    let (ex, hi) = fast_extract(b);
    // |b| = 2^(ex-52)*hi
    let t = hi.leading_zeros();
    Dint {
        hi: hi << t,
        lo: 0,
        ex: ex - if t > 11 { i64::from(t) - 12 } else { 0 },
        sgn: u64::from(b < 0.0),
    }
    // b = 2^ex*hi/2^64 where 1/2 <= hi/2^64 < 1
}

/// `subnormalize_dint`, round-to-nearest branch only.
#[inline]
fn subnormalize_dint(a: &mut Dint) {
    if a.ex > -1023 {
        return;
    }

    let ex = (-(1011 + a.ex)) as u64;

    let mut hi = a.hi.wrapping_shr(ex as u32);
    let md = a.hi.wrapping_shr((ex - 1) as u32) & 0x1;
    let lo = u64::from((a.hi & (!0u64).wrapping_shr(ex as u32)) != 0 || a.lo != 0);

    // FE_TONEAREST
    hi += if lo != 0 { md } else { hi & md };

    a.hi = hi.wrapping_shl(ex as u32);
    a.lo = 0;

    if a.hi == 0 {
        a.ex += 1;
        a.hi = 1u64 << 63;
    }
}

/// Convert a dint64_t value to a double (`dint_tod`).
#[inline]
fn dint_tod(a: &mut Dint) -> f64 {
    subnormalize_dint(a);

    let mut r = f64::from_bits((a.hi >> 11) | (0x3ffu64 << 52));

    let mut rd = 0.0;
    if (a.hi >> 10) & 0x1 != 0 {
        rd += fb(0x3ca0000000000000);
    }

    if (a.hi & 0x3ff) != 0 || a.lo != 0 {
        rd += fb(0x3c90000000000000);
    }

    if a.sgn != 0 {
        rd = -rd;
    }

    r = f64::from_bits(r.to_bits() | (a.sgn << 63));
    r += rd;

    let e: f64;

    if a.ex > -1022 {
        // The result is a normal double
        if a.ex > 1024 {
            if a.ex == 1025 {
                r *= fb(0x4000000000000000);
                e = fb(0x7fe0000000000000);
            } else {
                r = fb(0x7fefffffffffffff);
                e = fb(0x7fefffffffffffff);
            }
        } else {
            e = f64::from_bits((((a.ex + 1022) & 0x7ff) as u64) << 52);
        }
    } else if a.ex < -1073 {
        if a.ex == -1074 {
            r *= fb(0x3fe0000000000000);
            e = fb(0x0000000000000001);
        } else {
            // 0x0.0000000000001p-1022
            r = fb(0x0000000000000001);
            e = fb(0x0000000000000001);
        }
    } else {
        e = f64::from_bits(1u64 << (a.ex + 1073));
    }

    r * e
}

// **************** end of code copied from dint.h and pow.[ch] ****************

// **************** the following is copied from sin.c (in cos.c) *************

/// This table approximates 1/(2pi) downwards with precision 1280:
/// 1/(2*pi) ~ T[0]/2^64 + T[1]/2^128 + ... + T[i]/2^((i+1)*64) + ...
/// Computed with computeT() from sin.sage.
#[rustfmt::skip]
static T: [u64; 20] = [
    0x28be60db9391054a, // i=0
    0x7f09d5f47d4d3770, // i=1
    0x36d8a5664f10e410, // i=2
    0x7f9458eaf7aef158, // i=3
    0x6dc91b8e909374b8, // i=4
    0x01924bba82746487, // i=5
    0x3f877ac72c4a69cf, // i=6
    0xba208d7d4baed121, // i=7
    0x3a671c09ad17df90, // i=8
    0x4e64758e60d4ce7d, // i=9
    0x272117e2ef7e4a0e, // i=10
    0xc7fe25fff7816603, // i=11
    0xfbcbc462d6829b47, // i=12
    0xdb4d9fb3c9f2c26d, // i=13
    0xd3d18fd9a797fa8b, // i=14
    0x5d49eeb1faf97c5e, // i=15
    0xcf41ce7de294a4ba, // i=16
    0x9afed7ec47e35742, // i=17
    0x1580cc11bf1edaea, // i=18
    0xfc33ef0826bd0d87, // i=19
];

/// Table containing 128-bit approximations of sin2pi(i/2^11) for 0 <= i < 256
/// (to nearest). Each entry is to be interpreted as (hi/2^64+lo/2^128)*2^ex*(-1)*sgn.
/// Generated with computeS() from sin.sage.
#[rustfmt::skip]
static S: [Dint; 256] = [
    Dint::new(0x0000000000000000, 0x0000000000000000, 128, 0), // 0
    Dint::new(0xc90fc5f66525d257, 0x480f7956b6470765, -8, 0), // 1
    Dint::new(0xc90f87f3380388d5, 0xcb3ff35bd4d81baa, -7, 0), // 2
    Dint::new(0x96cb587284b81770, 0xb767005691b9d9d1, -6, 0), // 3
    Dint::new(0xc90e8fe6f63c2330, 0xf1d7d06db39ea9fc, -6, 0), // 4
    Dint::new(0xfb514b55ccbe541a, 0xd784e031f9af76d6, -6, 0), // 5
    Dint::new(0x96c9b5df1877e9b5, 0xf91ee371d6467dca, -5, 0), // 6
    Dint::new(0xafea690fd5912ef3, 0xf56e3c87ae3c56df, -5, 0), // 7
    Dint::new(0xc90aafbd1b33efc9, 0xc539edcbfda0cf2c, -5, 0), // 8
    Dint::new(0xe22a7a6729d8e453, 0x850021e392744a4f, -5, 0), // 9
    Dint::new(0xfb49b98e8e7807f6, 0x00b21ccebc9caac3, -5, 0), // 10
    Dint::new(0x8a342eda160bf5ae, 0xde5b1068d174be9c, -4, 0), // 11
    Dint::new(0x96c32baca2ae68b4, 0x37b2dd49d5fca3c0, -4, 0), // 12
    Dint::new(0xa351cb7fc30bc889, 0xb56007d16d4ad5a3, -4, 0), // 13
    Dint::new(0xafe00694866a1b44, 0xcd34d2751c2e1da7, -4, 0), // 14
    Dint::new(0xbc6dd52c3a342eb5, 0xf10bfca3d6464012, -4, 0), // 15
    Dint::new(0xc8fb2f886ec09f37, 0x6a17954b2b7c5171, -4, 0), // 16
    Dint::new(0xd5880deafc18b534, 0x73d1472472f4a390, -4, 0), // 17
    Dint::new(0xe214689606bf1676, 0x438b4a73aecd2541, -4, 0), // 18
    Dint::new(0xeea037cc04764844, 0xc4e92d01a2f42935, -4, 0), // 19
    Dint::new(0xfb2b73cfc106ff68, 0xf0a0e36a000c7350, -4, 0), // 20
    Dint::new(0x83db0a7231831d8f, 0x60e782313f6161af, -3, 0), // 21
    Dint::new(0x8a2009a6b84d9402, 0x77724a2b2a669bc4, -3, 0), // 22
    Dint::new(0x9064b3a76a22640c, 0x56e0a8b0d177b55d, -3, 0), // 23
    Dint::new(0x96a9049670cfae65, 0xf77574094d3c35c4, -3, 0), // 24
    Dint::new(0x9cecf8962d14c822, 0x50ffe4f5caa7f1fa, -3, 0), // 25
    Dint::new(0xa3308bc93904ad69, 0xdec1b7f2768bdafa, -3, 0), // 26
    Dint::new(0xa973ba526a6850d9, 0x76f8c63986598c79, -3, 0), // 27
    Dint::new(0xafb68054d520c60b, 0xfdd2fc0936594c2d, -3, 0), // 28
    Dint::new(0xb5f8d9f3cd8945d6, 0x924bef13600f9852, -3, 0), // 29
    Dint::new(0xbc3ac352ead90abe, 0xeb13e106732687f1, -3, 0), // 30
    Dint::new(0xc27c389609850433, 0xb228a03916371f6f, -3, 0), // 31
    Dint::new(0xc8bd35e14da15f0e, 0xc7396c894bbf7389, -3, 0), // 32
    Dint::new(0xcefdb7592542e1e9, 0x6b47b8c44e5b037e, -3, 0), // 33
    Dint::new(0xd53db9224ae01bca, 0x7337412cf70716cb, -3, 0), // 34
    Dint::new(0xdb7d3761c7b263b6, 0xbb286d23e11c8337, -3, 0), // 35
    Dint::new(0xe1bc2e3cf616a7ac, 0x31883b30137c6e62, -3, 0), // 36
    Dint::new(0xe7fa99d983ee098f, 0xeeb8f9c33340a2f2, -3, 0), // 37
    Dint::new(0xee38765d74fe4897, 0xed16b994af6c18ae, -3, 0), // 38
    Dint::new(0xf475bfef2551f5b9, 0x14e1a5488eaeab96, -3, 0), // 39
    Dint::new(0xfab272b54b9871a2, 0x704729ae56d78a37, -3, 0), // 40
    Dint::new(0x8077456b7dc2d967, 0x3eac8308f1113e5e, -2, 0), // 41
    Dint::new(0x8395023dd418e919, 0xdb1f70118c9c2198, -2, 0), // 42
    Dint::new(0x86b26de5933c2e8e, 0xc5a9decdfaad4db5, -2, 0), // 43
    Dint::new(0x89cf8676d7abb55b, 0x97965c9860c34e44, -2, 0), // 44
    Dint::new(0x8cec4a05f12739e8, 0xdcdca90cc73b116a, -2, 0), // 45
    Dint::new(0x9008b6a763de75b7, 0xa6e3df5975cca9da, -2, 0), // 46
    Dint::new(0x9324ca6fe9a04b4e, 0x899c4de737feec22, -2, 0), // 47
    Dint::new(0x964083747309d113, 0x000a89a11e07c1fe, -2, 0), // 48
    Dint::new(0x995bdfca28b53a54, 0x49c4863de522b217, -2, 0), // 49
    Dint::new(0x9c76dd866c689dcc, 0xe7bc08111d0bfca4, -2, 0), // 50
    Dint::new(0x9f917abeda4498df, 0xf3ff913a4aadb85e, -2, 0), // 51
    Dint::new(0xa2abb58949f2ced7, 0xa5dbee6084ee1260, -2, 0), // 52
    Dint::new(0xa5c58bfbcfd4436a, 0x69fcb11e19f58619, -2, 0), // 53
    Dint::new(0xa8defc2cbe2f8fcc, 0x0cd12a1f6ab6b095, -2, 0), // 54
    Dint::new(0xabf80432a65ef190, 0x8c95c4c91179176b, -2, 0), // 55
    Dint::new(0xaf10a22459fe32a6, 0x3feef3bb58b1f10d, -2, 0), // 56
    Dint::new(0xb228d418ec1869ad, 0x16031a34d4fc855d, -2, 0), // 57
    Dint::new(0xb5409827b25591f0, 0xcd73fb5d8d45d302, -2, 0), // 58
    Dint::new(0xb857ec684627fa4c, 0x187e26d290714d70, -2, 0), // 59
    Dint::new(0xbb6ecef285f98a3a, 0xbddd8a0365d6b1d3, -2, 0), // 60
    Dint::new(0xbe853dde9658dc60, 0xdfe1b074e22fc666, -2, 0), // 61
    Dint::new(0xc19b3744e3262dcd, 0xad5a41de48f6b26f, -2, 0), // 62
    Dint::new(0xc4b0b93e20c0213f, 0xdab4e426409b23a0, -2, 0), // 63
    Dint::new(0xc7c5c1e34d3055b2, 0x5cc8c00e4fccd850, -2, 0), // 64
    Dint::new(0xcada4f4db157cf77, 0xfa6171200ab2efc3, -2, 0), // 65
    Dint::new(0xcdee5f96e21b332c, 0x65a3132adfb7dfd5, -2, 0), // 66
    Dint::new(0xd101f0d8c18ed1c1, 0xaadb580a1eba209f, -2, 0), // 67
    Dint::new(0xd415012d802284f0, 0xdf4005ef6a64aa02, -2, 0), // 68
    Dint::new(0xd7278eaf9dcd5b55, 0x1779df36d1cc8912, -2, 0), // 69
    Dint::new(0xda399779eb391377, 0xcbabaeb97af8e8aa, -2, 0), // 70
    Dint::new(0xdd4b19a78aed6515, 0xece7f445cecf1e28, -2, 0), // 71
    Dint::new(0xe05c1353f27b17e5, 0x0ebc61ade6ca83cd, -2, 0), // 72
    Dint::new(0xe36c829aeba6e720, 0x26a0eecdb4f16266, -2, 0), // 73
    Dint::new(0xe67c659895943123, 0x82b0aecadf808123, -2, 0), // 74
    Dint::new(0xe98bba6965ef725f, 0xb91caf23416e7e80, -2, 0), // 75
    Dint::new(0xec9a7f2a2a188aeb, 0x7244ee20f591983b, -2, 0), // 76
    Dint::new(0xefa8b1f8084ccdfc, 0x1050cdf22f34182f, -2, 0), // 77
    Dint::new(0xf2b650f080d0da8d, 0x587f3fa044e2d27d, -2, 0), // 78
    Dint::new(0xf5c35a316f1a3c80, 0x643720de93ba81bd, -2, 0), // 79
    Dint::new(0xf8cfcbd90af8d57a, 0x4221dc4ba772598d, -2, 0), // 80
    Dint::new(0xfbdba405e9c00cca, 0xd24d3023da491920, -2, 0), // 81
    Dint::new(0xfee6e0d6ff6fc5a4, 0x8b74fe2508ab8fc2, -2, 0), // 82
    Dint::new(0x80f8c035cfee8d76, 0xfd958d68e8b49e6b, -1, 0), // 83
    Dint::new(0x827dc071bfed6ffa, 0xfb4c92369f0cf008, -1, 0), // 84
    Dint::new(0x8402702f5b30f2a9, 0xcb07b25a7b0372a7, -1, 0), // 85
    Dint::new(0x8586ce7ededc809d, 0x9d3dc689006896f4, -1, 0), // 86
    Dint::new(0x870ada70ba4e6d49, 0x009d52755ece3f70, -1, 0), // 87
    Dint::new(0x888e93158fb3bb04, 0x984156f553344306, -1, 0), // 88
    Dint::new(0x8a11f77e349bc245, 0xa66d1d936c38c329, -1, 0), // 89
    Dint::new(0x8b9506bbb28bb922, 0x575f33366be0afef, -1, 0), // 90
    Dint::new(0x8d17bfdf47921ac8, 0xcb590d74f64e77c9, -1, 0), // 91
    Dint::new(0x8e9a21fa66d9ee8d, 0xf2be3ecae62789d4, -1, 0), // 92
    Dint::new(0x901c2c1eb93dee39, 0x632b9cff5cfee724, -1, 0), // 93
    Dint::new(0x919ddd5e1ddb8b33, 0x609c464b3dd676ec, -1, 0), // 94
    Dint::new(0x931f34caaaa5d23a, 0x6a1ff8bfe6396e28, -1, 0), // 95
    Dint::new(0x94a03176acf82d45, 0xae4ba773da6bf754, -1, 0), // 96
    Dint::new(0x9620d274aa290339, 0xe06a955a5b8e301d, -1, 0), // 97
    Dint::new(0x97a116d7601c3515, 0xfc8b7184b21f2d50, -1, 0), // 98
    Dint::new(0x9920fdb1c5d5783d, 0x9dd1eedf18a2e4df, -1, 0), // 99
    Dint::new(0x9aa086170c0a8d86, 0x9ffa0d23f3c26c62, -1, 0), // 100
    Dint::new(0x9c1faf1a9db554af, 0xdab6b478577e7be5, -1, 0), // 101
    Dint::new(0x9d9e77d020a5bbe6, 0xdb895384528d0d60, -1, 0), // 102
    Dint::new(0x9f1cdf4b76138b02, 0x98dbd3555ebcdefe, -1, 0), // 103
    Dint::new(0xa09ae4a0bb300a19, 0x2f895f44a303cc0b, -1, 0), // 104
    Dint::new(0xa21886e449b78316, 0xd29d23a624acd00c, -1, 0), // 105
    Dint::new(0xa395c52ab8829dfc, 0x2be036401ba87cc2, -1, 0), // 106
    Dint::new(0xa5129e88dc17976a, 0x82d9495ead5be348, -1, 0), // 107
    Dint::new(0xa68f1213c73b5124, 0x17218792857f4c5a, -1, 0), // 108
    Dint::new(0xa80b1ee0cb823c27, 0x3269f4702b88324a, -1, 0), // 109
    Dint::new(0xa986c40579e11c0a, 0x8e3bdf8085321556, -1, 0), // 110
    Dint::new(0xab020097a33da341, 0xc1654b64a0081b46, -1, 0), // 111
    Dint::new(0xac7cd3ad58fee7f0, 0x811f953984eff83e, -1, 0), // 112
    Dint::new(0xadf73c5ced9db0f3, 0x9a5318ac6fe94e4d, -1, 0), // 113
    Dint::new(0xaf7139bcf5349ac6, 0x9fe5f4ea48965e2c, -1, 0), // 114
    Dint::new(0xb0eacae4461013ed, 0x63c66682bae74898, -1, 0), // 115
    Dint::new(0xb263eee9f93e3088, 0x695a5332090bb09b, -1, 0), // 116
    Dint::new(0xb3dca4e56b1e54bb, 0x992d96e5021e3c37, -1, 0), // 117
    Dint::new(0xb554ebee3bf0b58e, 0x971f4da709ad4378, -1, 0), // 118
    Dint::new(0xb6ccc31c5065afee, 0x35ebacd79f209137, -1, 0), // 119
    Dint::new(0xb8442987d22cf576, 0x9cc3ef36746de3b8, -1, 0), // 120
    Dint::new(0xb9bb1e4930848ead, 0xcdb0531c4e58484b, -1, 0), // 121
    Dint::new(0xbb31a07920c7b256, 0x55b92083658bb897, -1, 0), // 122
    Dint::new(0xbca7af309efd7182, 0x0a4b0d21fc5036a5, -1, 0), // 123
    Dint::new(0xbe1d4988ee67380c, 0xd1f90f79f46c7e01, -1, 0), // 124
    Dint::new(0xbf926e9b9a0f2127, 0x91a1b5eb79658c67, -1, 0), // 125
    Dint::new(0xc1071d8275561f9b, 0x721853f8e528a934, -1, 0), // 126
    Dint::new(0xc27b55579c81f96d, 0xcdc2bd470675104d, -1, 0), // 127
    Dint::new(0xc3ef1535754b168d, 0x3122c2a59efddc37, -1, 0), // 128
    Dint::new(0xc5625c36af6a222f, 0xf4ff2895ab6ebe89, -1, 0), // 129
    Dint::new(0xc6d5297645257e8d, 0x14d24739de27e2e9, -1, 0), // 130
    Dint::new(0xc8477c0f7bde8a98, 0x004ce0246ad4fa74, -1, 0), // 131
    Dint::new(0xc9b9531de49eb968, 0x4319e5ad5b0dcb84, -1, 0), // 132
    Dint::new(0xcb2aadbd5ca47af5, 0xfaa3dfe675a65ee2, -1, 0), // 133
    Dint::new(0xcc9b8b0a0deff5d4, 0x2e663b3c7555a6c3, -1, 0), // 134
    Dint::new(0xce0bea206fcf9192, 0x3c540a9eec47af38, -1, 0), // 135
    Dint::new(0xcf7bca1d476c516d, 0xa81290bdbaad62e4, -1, 0), // 136
    Dint::new(0xd0eb2a1da855fefd, 0xb9302788604e88f1, -1, 0), // 137
    Dint::new(0xd25a093ef50f2482, 0x721fc87ba1d42456, -1, 0), // 138
    Dint::new(0xd3c8669edf98d680, 0x87967926fdcecec4, -1, 0), // 139
    Dint::new(0xd536415b69fe4c54, 0x1df22346611c6b4b, -1, 0), // 140
    Dint::new(0xd6a39892e6e04764, 0x3090d44db12c418c, -1, 0), // 141
    Dint::new(0xd8106b63fa0048a0, 0xa573f2aa90434ba5, -1, 0), // 142
    Dint::new(0xd97cb8ed98cb93f5, 0x2e349483e3fb2a6a, -1, 0), // 143
    Dint::new(0xdae8804f0ae6015b, 0x362cb974182e3030, -1, 0), // 144
    Dint::new(0xdc53c0a7eab49b35, 0x3ccca3982328ed8b, -1, 0), // 145
    Dint::new(0xddbe791825e8099e, 0x1a5bd9269d408d7e, -1, 0), // 146
    Dint::new(0xdf28a8bffe06ca56, 0xcce2634be2bf54df, -1, 0), // 147
    Dint::new(0xe0924ec008f734fd, 0x8aa895d5bf3e84ea, -1, 0), // 148
    Dint::new(0xe1fb6a3931894b38, 0xf7a1f9bd9ba13b6b, -1, 0), // 149
    Dint::new(0xe363fa4cb8005482, 0x7b32c72e31824e51, -1, 0), // 150
    Dint::new(0xe4cbfe1c329c453a, 0xd40e9e6b989f89e5, -1, 0), // 151
    Dint::new(0xe63374c98e22f0b4, 0x2872ce1bfc7ad1cd, -1, 0), // 152
    Dint::new(0xe79a5d770e6905dc, 0xf1b65cc5fd780262, -1, 0), // 153
    Dint::new(0xe900b7474edad637, 0x431626c10485bdda, -1, 0), // 154
    Dint::new(0xea66815d4304e6c8, 0x0cc39cfcc29960b1, -1, 0), // 155
    Dint::new(0xebcbbadc371c4aaa, 0x1d90f780ae951140, -1, 0), // 156
    Dint::new(0xed3062e7d086c6f0, 0xc71debc372b6f9d4, -1, 0), // 157
    Dint::new(0xee9478a40e62bf86, 0x2a24164daec85ccb, -1, 0), // 158
    Dint::new(0xeff7fb354a0eecb1, 0x527233b40d3432bb, -1, 0), // 159
    Dint::new(0xf15ae9c037b1d8f0, 0x6c48e9e3420b0f1e, -1, 0), // 160
    Dint::new(0xf2bd4369e6c126d3, 0x7f232aee178c6323, -1, 0), // 161
    Dint::new(0xf41f0757c2889e84, 0x3c7f10db458c337c, -1, 0), // 162
    Dint::new(0xf58034af92b102a7, 0x93fa6107c4327527, -1, 0), // 163
    Dint::new(0xf6e0ca977bc6ac45, 0xe1079824233fef46, -1, 0), // 164
    Dint::new(0xf840c835ffbfed66, 0xa9a56012067c570c, -1, 0), // 165
    Dint::new(0xf9a02cb1fe833a0d, 0x08da894471de1a18, -1, 0), // 166
    Dint::new(0xfafef732b66d1742, 0x0343fbf4a7d42af3, -1, 0), // 167
    Dint::new(0xfc5d26dfc4d5cfda, 0x27c07c911290b8d1, -1, 0), // 168
    Dint::new(0xfdbabae12696eea4, 0x02377c3799c052fa, -1, 0), // 169
    Dint::new(0xff17b25f38907dad, 0x0a9c6ba50490539f, -1, 0), // 170
    Dint::new(0x803a06415c170525, 0x6f53873e2f1477ff, 0, 0), // 171
    Dint::new(0x80e7e43a61f5b6cb, 0x5ca183dc973abc22, 0, 0), // 172
    Dint::new(0x819572af6decac84, 0x9fba97fdf0c4d24c, 0, 0), // 173
    Dint::new(0x8242b1357110d372, 0x6fb2123fedfa6e22, 0, 0), // 174
    Dint::new(0x82ef9f618dc5b70e, 0x91a965931f1a200a, 0, 0), // 175
    Dint::new(0x839c3cc917ff6cb4, 0xbfd79717f2880abf, 0, 0), // 176
    Dint::new(0x8448890195846099, 0x246efcff30cb064a, 0, 0), // 177
    Dint::new(0x84f483a0be2f0403, 0x51917cac857fd5f5, 0, 0), // 178
    Dint::new(0x85a02c3c7c2f5ca5, 0x327888fe4b62687b, 0, 0), // 179
    Dint::new(0x864b826aec4c74e5, 0x85043222c9bdd18d, 0, 0), // 180
    Dint::new(0x86f685c25e25acf5, 0x7e0b9b07548471a2, 0, 0), // 181
    Dint::new(0x87a135d95473ec89, 0x4e091160e2430712, 0, 0), // 182
    Dint::new(0x884b9246854ab50b, 0x4f14c8afe4560291, 0, 0), // 183
    Dint::new(0x88f59aa0da591421, 0xb892ca8361d8c84c, 0, 0), // 184
    Dint::new(0x899f4e7f712a765e, 0xc88302a31afce54a, 0, 0), // 185
    Dint::new(0x8a48ad799b6759f3, 0x660558a02136130a, 0, 0), // 186
    Dint::new(0x8af1b726df15e13c, 0x545f7d79ead8fa19, 0, 0), // 187
    Dint::new(0x8b9a6b1ef6da4502, 0x21a6675f51580bc4, 0, 0), // 188
    Dint::new(0x8c42c8f9d2372644, 0x101a5adbcb9ffb43, 0, 0), // 189
    Dint::new(0x8cead04f95cdbf66, 0x4d49cbaf15aecd80, 0, 0), // 190
    Dint::new(0x8d9280b89b9df49b, 0xde2d43c6b67a7cbe, 0, 0), // 191
    Dint::new(0x8e39d9cd73464364, 0xbba4cfecbff54867, 0, 0), // 192
    Dint::new(0x8ee0db26e24390f8, 0xaf0e2345f3bd24b4, 0, 0), // 193
    Dint::new(0x8f87845de430d777, 0x9311a82459aa0f72, 0, 0), // 194
    Dint::new(0x902dd50bab06b1b7, 0xb144016c7a30b39a, 0, 0), // 195
    Dint::new(0x90d3ccc99f5ac58b, 0x09d1072e09b72292, 0, 0), // 196
    Dint::new(0x91796b31609f0c54, 0x6714fe6925b78cc4, 0, 0), // 197
    Dint::new(0x921eafdcc560f9c5, 0x33d0a284a8c954ad, 0, 0), // 198
    Dint::new(0x92c39a65db88809d, 0x1f8481e704e4a767, 0, 0), // 199
    Dint::new(0x93682a66e896f544, 0xb17821911e71c16e, 0, 0), // 200
    Dint::new(0x940c5f7a69e5ce1c, 0x0001489a97671a42, 0, 0), // 201
    Dint::new(0x94b0393b14e54156, 0xd6c7af02d5c16fd9, 0, 0), // 202
    Dint::new(0x9553b743d75ac03f, 0xac0106650f4ef023, 0, 0), // 203
    Dint::new(0x95f6d92fd79f4fba, 0xd9f8e1a446e973b9, 0, 0), // 204
    Dint::new(0x96999e9a74ddbde3, 0xa7a7556c3b33abc1, 0, 0), // 205
    Dint::new(0x973c071f4750b49c, 0xc0a03934f0cce19b, 0, 0), // 206
    Dint::new(0x97de125a2080a8ed, 0xd243aa0843a2c144, 0, 0), // 207
    Dint::new(0x987fbfe70b81a708, 0x19cec845ac87a5c6, 0, 0), // 208
    Dint::new(0x99210f624d30facb, 0xc4b992a37fb9b9bd, 0, 0), // 209
    Dint::new(0x99c200686472b4a8, 0x1ab42d43235757b6, 0, 0), // 210
    Dint::new(0x9a6292960a6f0ab0, 0x7e92c655656e6b85, 0, 0), // 211
    Dint::new(0x9b02c58832cf95c0, 0x698b94f50326a043, 0, 0), // 212
    Dint::new(0x9ba298dc0bfc6a88, 0x9a5614e8ffbeac6f, 0, 0), // 213
    Dint::new(0x9c420c2eff590e5f, 0xc7fd954194e6d8aa, 0, 0), // 214
    Dint::new(0x9ce11f1eb18147b1, 0x3e93627de8fd5779, 0, 0), // 215
    Dint::new(0x9d7fd1490285c9e3, 0xe25e39549638ae68, 0, 0), // 216
    Dint::new(0x9e1e224c0e28bc94, 0x2cad377d5c9c35d8, 0, 0), // 217
    Dint::new(0x9ebc11c62c1a1dfb, 0xcc141e10c6460c8b, 0, 0), // 218
    Dint::new(0x9f599f55f0340061, 0xa88d5f46834bbf8d, 0, 0), // 219
    Dint::new(0x9ff6ca9a2ab6a26d, 0x22cc118a0c118aa0, 0, 0), // 220
    Dint::new(0xa0939331e8846237, 0x7cec6df5bea167cf, 0, 0), // 221
    Dint::new(0xa12ff8bc735d8af6, 0x71acea2819360c35, 0, 0), // 222
    Dint::new(0xa1cbfad9521bfd1b, 0x166c36e7bb3c402f, 0, 0), // 223
    Dint::new(0xa267992848eeb0c0, 0x3b5167ee359a234e, 0, 0), // 224
    Dint::new(0xa302d34959951243, 0x9443372e20d4377c, 0, 0), // 225
    Dint::new(0xa39da8dcc39a38e5, 0x0ca9a8a720d4c69c, 0, 0), // 226
    Dint::new(0xa4381983048ff747, 0xbf623cf5301a2dde, 0, 0), // 227
    Dint::new(0xa4d224dcd849c5b0, 0x23d251cc8d7975cc, 0, 0), // 228
    Dint::new(0xa56bca8b391785db, 0x189d39ffe11aaa2b, 0, 0), // 229
    Dint::new(0xa6050a2f60002049, 0x8c33ebf3aa8501fb, 0, 0), // 230
    Dint::new(0xa69de36ac4fbfadc, 0x9b3ad6e4022183d9, 0, 0), // 231
    Dint::new(0xa73655df1f2f489e, 0x149f6e75993468a3, 0, 0), // 232
    Dint::new(0xa7ce612e65243291, 0x6b2a39f856a69781, 0, 0), // 233
    Dint::new(0xa86604facd04d969, 0x3463a2c2e6e9cc55, 0, 0), // 234
    Dint::new(0xa8fd40e6ccd52ffd, 0x6cc14c4f53e2e82d, 0, 0), // 235
    Dint::new(0xa99414951aacae5e, 0xd147625fda929af8, 0, 0), // 236
    Dint::new(0xaa2a7fa8acefdd63, 0xb714ee81b53b4b9d, 0, 0), // 237
    Dint::new(0xaac081c4ba89ba8a, 0xe1b3dfc4dbda9bfd, 0, 0), // 238
    Dint::new(0xab561a8cbb24f410, 0xf17cee69b0d2ecde, 0, 0), // 239
    Dint::new(0xabeb49a46764fd15, 0x1becda8089c1a94c, 0, 0), // 240
    Dint::new(0xac800eafb91ef9a9, 0xf86ba0dde982fb59, 0, 0), // 241
    Dint::new(0xad146952eb9282af, 0x44bf16268608db96, 0, 0), // 242
    Dint::new(0xada859327ba24151, 0x9d30d4cfeb04f1fb, 0, 0), // 243
    Dint::new(0xae3bddf3280c620d, 0x3d53817865422565, 0, 0), // 244
    Dint::new(0xaecef739f1a2df10, 0xf74d099042e8f326, 0, 0), // 245
    Dint::new(0xaf61a4ac1b83a1de, 0xa89a9b8f726b95bf, 0, 0), // 246
    Dint::new(0xaff3e5ef2b507c06, 0x8c679e67fc462d51, 0, 0), // 247
    Dint::new(0xb085baa8e966f6da, 0xe4cad00d5c94bcd2, 0, 0), // 248
    Dint::new(0xb117227f6117f9f9, 0x8d8be132d576e614, 0, 0), // 249
    Dint::new(0xb1a81d18e0df4889, 0x24784f32c3e3e5bd, 0, 0), // 250
    Dint::new(0xb238aa1bfa9ad507, 0x8cc7d4bd05ffd5ae, 0, 0), // 251
    Dint::new(0xb2c8c92f83c1eb87, 0xac9f7ebbc469ef59, 0, 0), // 252
    Dint::new(0xb35879fa959c323c, 0x5d6635109164f740, 0, 0), // 253
    Dint::new(0xb3e7bc248d78802e, 0xa156468ef6c18c60, 0, 0), // 254
    Dint::new(0xb4768f550ce389fd, 0x4a85350f69018c55, 0, 0), // 255
];

/// Table containing 128-bit approximations of cos2pi(i/2^11) for 0 <= i < 256
/// (to nearest). Each entry is to be interpreted as (hi/2^64+lo/2^128)*2^ex*(-1)*sgn.
/// Generated with computeC() from sin.sage.
#[rustfmt::skip]
static C: [Dint; 256] = [
    Dint::new(0x8000000000000000, 0x0000000000000000, 1, 0), // 0
    Dint::new(0xffffb10b10e80e95, 0x3031437d7eccb9df, 0, 0), // 1
    Dint::new(0xfffec42c7454926b, 0x38e310779edfec68, 0, 0), // 2
    Dint::new(0xfffd3964bc6275ba, 0x69fff9ae0dedb047, 0, 0), // 3
    Dint::new(0xfffb10b4dc96dabb, 0xb47903f7a19f8ee2, 0, 0), // 4
    Dint::new(0xfff84a1e29de8571, 0x8cc193c5d508e13f, 0, 0), // 5
    Dint::new(0xfff4e5a25a8d095b, 0x43366df666fd54ff, 0, 0), // 6
    Dint::new(0xfff0e343865bbb13, 0x5428ed0647c9e5d1, 0, 0), // 7
    Dint::new(0xffec4304266865d9, 0x5657552366961732, 0, 0), // 8
    Dint::new(0xffe704e71533c508, 0x53aa9423bb0adc21, 0, 0), // 9
    Dint::new(0xffe128ef8e9fc17a, 0x7d209f32d42d864e, 0, 0), // 10
    Dint::new(0xffdaaf212fed72db, 0x4fd8f038449ec436, 0, 0), // 11
    Dint::new(0xffd3977ff7bae4e9, 0x664649b4d541b9c5, 0, 0), // 12
    Dint::new(0xffcbe2104600a0a9, 0x5595ca3f421ae09c, 0, 0), // 13
    Dint::new(0xffc38ed6dc0ef98b, 0x1c676208aa3be545, 0, 0), // 14
    Dint::new(0xffba9dd8dc8b1e83, 0xccfed60a91097c48, 0, 0), // 15
    Dint::new(0xffb10f1bcb6bef1d, 0x421e8edaaf59453e, 0, 0), // 16
    Dint::new(0xffa6e2a58df6947d, 0xd2c665c2da3e7844, 0, 0), // 17
    Dint::new(0xff9c187c6abade6a, 0x1e1862cca089938b, 0, 0), // 18
    Dint::new(0xff90b0a7098f6443, 0x2dabd3195a05710f, 0, 0), // 19
    Dint::new(0xff84ab2c738d6a03, 0x519c314973ccae6b, 0, 0), // 20
    Dint::new(0xff780814130c893c, 0x3ea4f30adda3016f, 0, 0), // 21
    Dint::new(0xff6ac765b39e1e19, 0x1b9d5851979f28fb, 0, 0), // 22
    Dint::new(0xff5ce92982087867, 0x50a7bb6a6ee3b0f1, 0, 0), // 23
    Dint::new(0xff4e6d680c41d0a9, 0x0f668633f1ab858a, 0, 0), // 24
    Dint::new(0xff3f542a416b0134, 0xb085c1828f69296a, 0, 0), // 25
    Dint::new(0xff2f9d7971ca0364, 0x27e31939e2eec09c, 0, 0), // 26
    Dint::new(0xff1f495f4ec430d7, 0xf5971326a3540ea9, 0, 0), // 27
    Dint::new(0xff0e57e5ead848d1, 0x1f1901544271c3f8, 0, 0), // 28
    Dint::new(0xfefcc917b99839a5, 0xe0abd3a9b64df725, 0, 0), // 29
    Dint::new(0xfeea9cff8fa2ae54, 0xec34413e87ef2740, 0, 0), // 30
    Dint::new(0xfed7d3a8a29c603b, 0x2f88b949a72ff96c, 0, 0), // 31
    Dint::new(0xfec46d1e89292cf0, 0x41390efdc726e9ef, 0, 0), // 32
    Dint::new(0xfeb0696d3ae4f04d, 0xb7b6cc53c3abc817, 0, 0), // 33
    Dint::new(0xfe9bc8a1105c22a5, 0xd3af6ee4f2101c20, 0, 0), // 34
    Dint::new(0xfe868ac6c3043b2e, 0x0b4f70c910505e10, 0, 0), // 35
    Dint::new(0xfe70afeb6d33d6a2, 0x2907cf2b3f6feac2, 0, 0), // 36
    Dint::new(0xfe5a381c8a1aa224, 0xd54faa364b7da8f6, 0, 0), // 37
    Dint::new(0xfe432367f5b90a62, 0x87b8875373a818a4, 0, 0), // 38
    Dint::new(0xfe2b71dbecd7aefc, 0x008598c2c429caf7, 0, 0), // 39
    Dint::new(0xfe1323870cfe9a3d, 0x90cd1d959db674ef, 0, 0), // 40
    Dint::new(0xfdfa3878546c3d28, 0x9bfe5c51e91cbdcd, 0, 0), // 41
    Dint::new(0xfde0b0bf220c2fd4, 0xe276d247626a23fd, 0, 0), // 42
    Dint::new(0xfdc68c6b356db62f, 0x499ddb331d19539d, 0, 0), // 43
    Dint::new(0xfdabcb8caeba091b, 0xfac7397cc07a6470, 0, 0), // 44
    Dint::new(0xfd906e340eaa6401, 0xd6e270740a186977, 0, 0), // 45
    Dint::new(0xfd747472367dd6c5, 0x61beb8cd2696fc78, 0, 0), // 46
    Dint::new(0xfd57de5867eedc39, 0x6c696582f346fd91, 0, 0), // 47
    Dint::new(0xfd3aabf84528b50b, 0xeae6bd951c1dabbe, 0, 0), // 48
    Dint::new(0xfd1cdd63d0bc8735, 0x863b87258f11ad7e, 0, 0), // 49
    Dint::new(0xfcfe72ad6d9641f2, 0xa06fab9f9d106709, 0, 0), // 50
    Dint::new(0xfcdf6be7def1464c, 0xa4e064308f4999f4, 0, 0), // 51
    Dint::new(0xfcbfc926484cd43a, 0xa3e22b4d38917e73, 0, 0), // 52
    Dint::new(0xfc9f8a7c2d603c60, 0x5d582cac7cb4391c, 0, 0), // 53
    Dint::new(0xfc7eaffd720ed673, 0x02880268f2e62955, 0, 0), // 54
    Dint::new(0xfc5d39be5a5bbc4b, 0x1c0d254b6c8da4bd, 0, 0), // 55
    Dint::new(0xfc3b27d38a5d49ab, 0x256778ffcb5c1769, 0, 0), // 56
    Dint::new(0xfc187a52063060c2, 0x9433b49289417ea2, 0, 0), // 57
    Dint::new(0xfbf5314f31eb7375, 0x25aafd7fdba12c5f, 0, 0), // 58
    Dint::new(0xfbd14ce0d191516e, 0x7190c94899dff1b8, 0, 0), // 59
    Dint::new(0xfbaccd1d0903bb09, 0xe63ae8632b84473c, 0, 0), // 60
    Dint::new(0xfb87b21a5bf5b917, 0x75df66f0ec3dd459, 0, 0), // 61
    Dint::new(0xfb61fbefadddb985, 0x61ce9d5ef5a81487, 0, 0), // 62
    Dint::new(0xfb3baab441e770f7, 0xb4b54683879c9c17, 0, 0), // 63
    Dint::new(0xfb14be7fbae58156, 0x2172a361fd2a722f, 0, 0), // 64
    Dint::new(0xfaed376a1b42e559, 0x2079880c450348ac, 0, 0), // 65
    Dint::new(0xfac5158bc4f4211f, 0x4a188aa367f90ab1, 0, 0), // 66
    Dint::new(0xfa9c58fd796837d4, 0x10655ecd5cc771d8, 0, 0), // 67
    Dint::new(0xfa7301d859796671, 0x1fe196a53fb5b237, 0, 0), // 68
    Dint::new(0xfa491035e55da3a3, 0xd24377c77a591e24, 0, 0), // 69
    Dint::new(0xfa1e842ffc96e4e0, 0x431c393c7f62da65, 0, 0), // 70
    Dint::new(0xf9f35de0dde328ab, 0xba5dbf4510eddc8f, 0, 0), // 71
    Dint::new(0xf9c79d63272c4628, 0x4504ae08d19b2980, 0, 0), // 72
    Dint::new(0xf99b42d1d57781eb, 0x78685d850f80ecdc, 0, 0), // 73
    Dint::new(0xf96e4e4844d4e82a, 0x80e8c17bf80e8f02, 0, 0), // 74
    Dint::new(0xf940bfe2304e6c45, 0xc0e2a1352ed7f292, 0, 0), // 75
    Dint::new(0xf91297bbb1d6cdbe, 0x68fc6e4d6a920bd2, 0, 0), // 76
    Dint::new(0xf8e3d5f1423842a0, 0x9701914c7f8fbcd7, 0, 0), // 77
    Dint::new(0xf8b47a9fb902e76c, 0xac9f07f54ff5bc14, 0, 0), // 78
    Dint::new(0xf88485e44c7af48a, 0xb36a9dfaadafc1e1, 0, 0), // 79
    Dint::new(0xf853f7dc9186b952, 0xc7adc6b4988891bb, 0, 0), // 80
    Dint::new(0xf822d0a67b9c5cb5, 0xa776175bd284fe05, 0, 0), // 81
    Dint::new(0xf7f110605caf6390, 0xa76f7efc19aed41c, 0, 0), // 82
    Dint::new(0xf7beb728e51dfcb8, 0x730785813f78aa1e, 0, 0), // 83
    Dint::new(0xf78bc51f239e12c6, 0x214cffcee9dd33ca, 0, 0), // 84
    Dint::new(0xf7583a62852a23b2, 0x4becad887680c197, 0, 0), // 85
    Dint::new(0xf7241712d4edde49, 0xf99107e50d631330, 0, 0), // 86
    Dint::new(0xf6ef5b503c328589, 0x50ca117eb18beed7, 0, 0), // 87
    Dint::new(0xf6ba073b424b19e8, 0x2c791f59cc1ffc23, 0, 0), // 88
    Dint::new(0xf6841af4cc8048a4, 0xce8c455197cdf8a7, 0, 0), // 89
    Dint::new(0xf64d969e1dfc2119, 0x119d358de0493956, 0, 0), // 90
    Dint::new(0xf6167a58d7b59026, 0x9dc7e5954c5a8f24, 0, 0), // 91
    Dint::new(0xf5dec646f85ba1c6, 0xc8c615e72768d6b5, 0, 0), // 92
    Dint::new(0xf5a67a8adc4088ca, 0xed0dd4bf62edd13f, 0, 0), // 93
    Dint::new(0xf56d97473d446cda, 0x275a2bbb2bab6c8a, 0, 0), // 94
    Dint::new(0xf5341c9f32bffeb9, 0x8da64484aaa0febc, 0, 0), // 95
    Dint::new(0xf4fa0ab6316ed2ec, 0x163c5c7f03b718c5, 0, 0), // 96
    Dint::new(0xf4bf61b00b5982b7, 0x890ac4aafa6a37bf, 0, 0), // 97
    Dint::new(0xf48421b0efbf939b, 0xf8f9d3b87d11fd52, 0, 0), // 98
    Dint::new(0xf4484add6b01254b, 0x667e06866c07c369, 0, 0), // 99
    Dint::new(0xf40bdd5a6688662f, 0x5019794a1f5896e5, 0, 0), // 100
    Dint::new(0xf3ced94d28b2ce8a, 0x18ef535a7ffa7a3d, 0, 0), // 101
    Dint::new(0xf3913edb54ba2242, 0x50f29b4b49f31c37, 0, 0), // 102
    Dint::new(0xf3530e2aea9d3966, 0x0d981acdcf6bc3e4, 0, 0), // 103
    Dint::new(0xf314476247088f74, 0xa5486bdc455d56a2, 0, 0), // 104
    Dint::new(0xf2d4eaa8233e997d, 0x431be53f92ece9e6, 0, 0), // 105
    Dint::new(0xf294f82394ffe320, 0xebadcdbf915e8f6c, 0, 0), // 106
    Dint::new(0xf2546ffc0e72f286, 0xaf0eed81e8c51e55, 0, 0), // 107
    Dint::new(0xf21352595e0bf350, 0xe7112e89103cc0c7, 0, 0), // 108
    Dint::new(0xf1d19f63ae7428a2, 0x844e6a35ddc2b713, 0, 0), // 109
    Dint::new(0xf18f574386712643, 0x8f6bac72988088b0, 0, 0), // 110
    Dint::new(0xf14c7a21c8cbd0f4, 0x2730081c758fb42b, 0, 0), // 111
    Dint::new(0xf1090827b43725fd, 0x67127db35b287316, 0, 0), // 112
    Dint::new(0xf0c5017ee336ca0f, 0xc4e557b119ef3185, 0, 0), // 113
    Dint::new(0xf08066514c055f7e, 0x973ea9903ed5125f, 0, 0), // 114
    Dint::new(0xf03b36c9407aa3e8, 0x992d39ec5c561d28, 0, 0), // 115
    Dint::new(0xeff573116df1555d, 0x62aef7b55319d1d4, 0, 0), // 116
    Dint::new(0xefaf1b54dd2cdf0f, 0xf03a18a5e16ab641, 0, 0), // 117
    Dint::new(0xef682fbef23ecda6, 0x767c0e8ad33bc085, 0, 0), // 118
    Dint::new(0xef20b07b6c6c0b37, 0xe2398bf0eeb28cde, 0, 0), // 119
    Dint::new(0xeed89db66611e307, 0x86f8c20fb664b01b, 0, 0), // 120
    Dint::new(0xee8ff79c548acd0f, 0xa1d2c3d018a9279f, 0, 0), // 121
    Dint::new(0xee46be5a0813016b, 0x7872773830d368be, 0, 0), // 122
    Dint::new(0xedfcf21cabacd3b1, 0xfee6a1eebfa13b4a, 0, 0), // 123
    Dint::new(0xedb29311c504d652, 0x11815196b9fbf5df, 0, 0), // 124
    Dint::new(0xed67a1673455c601, 0x7289102076a125e5, 0, 0), // 125
    Dint::new(0xed1c1d4b344c3d4f, 0xddffe98c4f8aa031, 0, 0), // 126
    Dint::new(0xecd006ec59ea306f, 0xa8392eb238578ab0, 0, 0), // 127
    Dint::new(0xec835e79946a3145, 0x7e610231ac1d6181, 0, 0), // 128
    Dint::new(0xec3624222d227bd1, 0x0278047ae3dd0889, 0, 0), // 129
    Dint::new(0xebe85815c767cb00, 0x1e99ccb9adc62ca6, 0, 0), // 130
    Dint::new(0xeb99fa84606ff5ff, 0x0dae311e656e0661, 0, 0), // 131
    Dint::new(0xeb4b0b9e4f345617, 0x39e39c6c2ab3655d, 0, 0), // 132
    Dint::new(0xeafb8b944453f52f, 0x3383bbb5156bf1d7, 0, 0), // 133
    Dint::new(0xeaab7a9749f584fe, 0x24db98ad3a0647a1, 0, 0), // 134
    Dint::new(0xea5ad8d8c3a91f05, 0x4a0ca5ea449b1c83, 0, 0), // 135
    Dint::new(0xea09a68a6e49cd62, 0x15ad45b4a1b5e823, 0, 0), // 136
    Dint::new(0xe9b7e3de5fdedc8b, 0xcd24d4bd1056c826, 0, 0), // 137
    Dint::new(0xe9659107077cf60f, 0x89a92b199adfbafa, 0, 0), // 138
    Dint::new(0xe912ae372d27045d, 0xacb1c26a06e5ae02, 0, 0), // 139
    Dint::new(0xe8bf3ba1f1aedfbb, 0xf8972affb3d98e1f, 0, 0), // 140
    Dint::new(0xe86b397ace95c46f, 0x9fec1e78c4376186, 0, 0), // 141
    Dint::new(0xe816a7f595ec9232, 0xbfe8378abfb87b6f, 0, 0), // 142
    Dint::new(0xe7c187467233d508, 0xdbfb0fe56c6f80fe, 0, 0), // 143
    Dint::new(0xe76bd7a1e63b9786, 0x125129529d48a92f, 0, 0), // 144
    Dint::new(0xe715993ccd02fe9c, 0xe2ba81b9ce96e02e, 0, 0), // 145
    Dint::new(0xe6becc4c5997af06, 0x82fcedb4c6434d76, 0, 0), // 146
    Dint::new(0xe667710616f4fc59, 0xdd2a3e32c3859960, 0, 0), // 147
    Dint::new(0xe60f879fe7e2e1e5, 0x7613b68f6ab03130, 0, 0), // 148
    Dint::new(0xe5b7105006d4c560, 0x9b695cd67c93bd79, 0, 0), // 149
    Dint::new(0xe55e0b4d05c80388, 0x5a7c210a3a15e7ea, 0, 0), // 150
    Dint::new(0xe50478cdce2246bc, 0xe1f5a58c80292554, 0, 0), // 151
    Dint::new(0xe4aa5909a08fa7b4, 0x122785ae67f5515d, 0, 0), // 152
    Dint::new(0xe44fac3814e09856, 0x20d63b5b9e3cd6ac, 0, 0), // 153
    Dint::new(0xe3f4729119e798d9, 0x56992551ae074e99, 0, 0), // 154
    Dint::new(0xe398ac4cf556b732, 0x0d1197dc12c63176, 0, 0), // 155
    Dint::new(0xe33c59a4439cd8ec, 0x36563e2ffad8351a, 0, 0), // 156
    Dint::new(0xe2df7acff7c2cf83, 0xd6fe4dd22e60a4a2, 0, 0), // 157
    Dint::new(0xe28210095b483751, 0xfd39138aa2d508ed, 0, 0), // 158
    Dint::new(0xe224198a0e002123, 0xe0521df01a1be6f5, 0, 0), // 159
    Dint::new(0xe1c5978c05ed8691, 0xf4e8a8372f8c5810, 0, 0), // 160
    Dint::new(0xe1668a498f1f892c, 0xe2f9d4600f4d0325, 0, 0), // 161
    Dint::new(0xe106f1fd4b8d7c96, 0x6ba8a9d9ba877899, 0, 0), // 162
    Dint::new(0xe0a6cee232f2bb9c, 0x6d6c98fe79817946, 0, 0), // 163
    Dint::new(0xe046213392aa486c, 0x55ff6038a5197367, 0, 0), // 164
    Dint::new(0xdfe4e92d0d8a37f5, 0x720588ff6547d884, 0, 0), // 165
    Dint::new(0xdf83270a9bbee890, 0xab01350f013d78dd, 0, 0), // 166
    Dint::new(0xdf20db088aa60404, 0x64a58b2f103485dd, 0, 0), // 167
    Dint::new(0xdebe05637ca94cfb, 0x4b19aa71fec3ae6d, 0, 0), // 168
    Dint::new(0xde5aa65869193805, 0x04248f15548f69ca, 0, 0), // 169
    Dint::new(0xddf6be249c075037, 0xd597b10a01676659, 0, 0), // 170
    Dint::new(0xdd924d05b620678a, 0x739c45b982193b5e, 0, 0), // 171
    Dint::new(0xdd2d5339ac8692fd, 0x49c6e0ea76cbcaac, 0, 0), // 172
    Dint::new(0xdcc7d0fec8aaf2aa, 0xb2069fd0b482b4e8, 0, 0), // 173
    Dint::new(0xdc61c693a82745d5, 0xaca8017e375b64e5, 0, 0), // 174
    Dint::new(0xdbfb34373c974b0e, 0xccb7fd40d543f4a1, 0, 0), // 175
    Dint::new(0xdb941a28cb71ec87, 0x2c19b63253da43fc, 0, 0), // 176
    Dint::new(0xdb2c78a7ede238a9, 0x5a98479cbef2ecbc, 0, 0), // 177
    Dint::new(0xdac44ff490a02710, 0x5b267c1bcff0ab62, 0, 0), // 178
    Dint::new(0xda5ba04ef3c929f4, 0xe257bde73d83dc1a, 0, 0), // 179
    Dint::new(0xd9f269f7aab88c29, 0x28e81dcb6dab91ac, 0, 0), // 180
    Dint::new(0xd988ad2f9bdf9bbb, 0xc4e4dc69fc2fff6f, 0, 0), // 181
    Dint::new(0xd91e6a38009da15a, 0x1bb35ad6d2e74b67, 0, 0), // 182
    Dint::new(0xd8b3a1526517a48b, 0x1ed1a8ff78f1b632, 0, 0), // 183
    Dint::new(0xd84852c0a80ffcdb, 0x24b9fe00663574a4, 0, 0), // 184
    Dint::new(0xd7dc7ec4fabdb011, 0xced12d2899b803db, 0, 0), // 185
    Dint::new(0xd77025a1e0a39d8b, 0x0cb78e80e67ba1b8, 0, 0), // 186
    Dint::new(0xd703479a2f6776cc, 0x6cb3bfd65b38562b, 0, 0), // 187
    Dint::new(0xd695e4f10ea88570, 0x083f082b570611d7, 0, 0), // 188
    Dint::new(0xd627fde9f7d63e7e, 0x7afbefc05e9f7d99, 0, 0), // 189
    Dint::new(0xd5b992c8b606a351, 0x7190b755535d4f18, 0, 0), // 190
    Dint::new(0xd54aa3d165cc7018, 0x7d00ae97abaa4096, 0, 0), // 191
    Dint::new(0xd4db3148750d1819, 0xf630e8b6dac83e69, 0, 0), // 192
    Dint::new(0xd46b3b72a2d68fc9, 0xdc4663a3168698d2, 0, 0), // 193
    Dint::new(0xd3fac294ff34e4d0, 0xb77d4f6bd0ee8591, 0, 0), // 194
    Dint::new(0xd389c6f4eb07a41c, 0xa8faac741a6394dc, 0, 0), // 195
    Dint::new(0xd31848d817d70e16, 0xeeeaddb72f00e0dd, 0, 0), // 196
    Dint::new(0xd2a6488487a91918, 0x4300fd1c1ce507e5, 0, 0), // 197
    Dint::new(0xd233c6408cd64236, 0x981ba7e42537275f, 0, 0), // 198
    Dint::new(0xd1c0c252c9de2c86, 0xda7485a5aeffeb4c, 0, 0), // 199
    Dint::new(0xd14d3d02313c0eed, 0x744fea20e8abef92, 0, 0), // 200
    Dint::new(0xd0d93696053af098, 0x77a18eb13d2ecde5, 0, 0), // 201
    Dint::new(0xd064af55d7c9b43e, 0x6b8a685f6cb61c21, 0, 0), // 202
    Dint::new(0xcfefa7898a4ef23c, 0xdaf200dd81212d10, 0, 0), // 203
    Dint::new(0xcf7a1f794d7ca1b1, 0xdfcb60445c1bf973, 0, 0), // 204
    Dint::new(0xcf04176da12390ac, 0x04d27090f10c454e, 0, 0), // 205
    Dint::new(0xce8d8faf5406ab8b, 0xf5babff66def7892, 0, 0), // 206
    Dint::new(0xce16888783ae13b3, 0x93e391861a034684, 0, 0), // 207
    Dint::new(0xcd9f023f9c3a059e, 0x23af31db7179a4aa, 0, 0), // 208
    Dint::new(0xcd26fd2158358e7d, 0x649474e36b8db9d3, 0, 0), // 209
    Dint::new(0xccae7976c0691177, 0x83e907fbd7aaf0b0, 0, 0), // 210
    Dint::new(0xcc35778a2bac9ca1, 0xf839ce18e08bfb50, 0, 0), // 211
    Dint::new(0xcbbbf7a63eba0dd5, 0x70cbb7f3343451be, 0, 0), // 212
    Dint::new(0xcb41fa15ebff0777, 0x2293661be51140ab, 0, 0), // 213
    Dint::new(0xcac77f24736eb553, 0xd9944be1631846d8, 0, 0), // 214
    Dint::new(0xca4c871d625361a9, 0x5328edeb3e6784de, 0, 0), // 215
    Dint::new(0xc9d1124c931fda7a, 0x8335241be1693225, 0, 0), // 216
    Dint::new(0xc95520fe2d40a74b, 0x83b0e96e1249c2b0, 0, 0), // 217
    Dint::new(0xc8d8b37ea4ed0f62, 0x0b562c00b34ee771, 0, 0), // 218
    Dint::new(0xc85bca1abaf7f0a7, 0x65862939b83382e0, 0, 0), // 219
    Dint::new(0xc7de651f7ca06749, 0x02b31bc86877fd2c, 0, 0), // 220
    Dint::new(0xc76084da43624634, 0xd5c149509e9059f1, 0, 0), // 221
    Dint::new(0xc6e22998b4c6608e, 0xcfe6c1b1a6b4e2a4, 0, 0), // 222
    Dint::new(0xc66353a8c232a43c, 0xe993503baf5afb41, 0, 0), // 223
    Dint::new(0xc5e40358a8ba05a7, 0x43da25d99267326b, 0, 0), // 224
    Dint::new(0xc56438f6f0ec3cca, 0x0ab4906075507e74, 0, 0), // 225
    Dint::new(0xc4e3f4d26ea553b6, 0xdd40950cf1ed92fa, 0, 0), // 226
    Dint::new(0xc463373a40dd06a3, 0x9dd768f30ca8e85c, 0, 0), // 227
    Dint::new(0xc3e2007dd175f5a4, 0xa87e78136665cdb2, 0, 0), // 228
    Dint::new(0xc36050ecd50ca830, 0x8ac9e1386e4cbabb, 0, 0), // 229
    Dint::new(0xc2de28d74ac6628b, 0x74c8f010d986a9e0, 0, 0), // 230
    Dint::new(0xc25b888d7c1fcd38, 0xb7041e9bc8c18b0d, 0, 0), // 231
    Dint::new(0xc1d8705ffcbb6e90, 0xbdf0715cb8b20bd7, 0, 0), // 232
    Dint::new(0xc154e09faa2ff69a, 0x17858573216e0a22, 0, 0), // 233
    Dint::new(0xc0d0d99dabd65d44, 0x2bda5328933c854a, 0, 0), // 234
    Dint::new(0xc04c5bab7297d322, 0x6dd06968e0ed1957, 0, 0), // 235
    Dint::new(0xbfc7671ab8bb84c6, 0xe4e62d86dd136e78, 0, 0), // 236
    Dint::new(0xbf41fc3d81b430db, 0x0d46655d6b012455, 0, 0), // 237
    Dint::new(0xbebc1b6619ed9116, 0x2715ef03f8543355, 0, 0), // 238
    Dint::new(0xbe35c4e716999630, 0x29d7f7b67d43b177, 0, 0), // 239
    Dint::new(0xbdaef913557d76f0, 0xac85320f528d6d5d, 0, 0), // 240
    Dint::new(0xbd27b83dfcbe9279, 0x2ea36923d5d8e213, 0, 0), // 241
    Dint::new(0xbca002ba7aaf25ea, 0x4a48496734be336d, 0, 0), // 242
    Dint::new(0xbc17d8dc859ad583, 0x727c405ffc73af56, 0, 0), // 243
    Dint::new(0xbb8f3af81b93095c, 0xfce8d84068e825b6, 0, 0), // 244
    Dint::new(0xbb062961823b1ddc, 0x5120e35e1c1a250c, 0, 0), // 245
    Dint::new(0xba7ca46d46946802, 0x33201477347447d8, 0, 0), // 246
    Dint::new(0xb9f2ac703cca0db3, 0x39db32d014440024, 0, 0), // 247
    Dint::new(0xb96841bf7ffcb21a, 0x9de1e3b22b8bf4db, 0, 0), // 248
    Dint::new(0xb8dd64b0720df647, 0xa726f4f0828585c9, 0, 0), // 249
    Dint::new(0xb8521598bb6bce26, 0x1c041d1ea5fb3fdb, 0, 0), // 250
    Dint::new(0xb7c654ce4adba9f2, 0x2e7a35723f3ed035, 0, 0), // 251
    Dint::new(0xb73a22a755457448, 0x7f86f63bb23f496a, 0, 0), // 252
    Dint::new(0xb6ad7f7a557e64f2, 0xeb2d28ef943dc88c, 0, 0), // 253
    Dint::new(0xb6206b9e0c13a892, 0xea7c015f12b987f7, 0, 0), // 254
    Dint::new(0xb592e7697f14dd4a, 0x737dd2824b608d13, 0, 0), // 255
];

/// Degree-7 odd polynomial approximating sin2pi(x) for -2^-24 < x < 2^-11+2^-24
/// with relative error 2^-77.306 (degree 1 as h+l). Generated with sin_fast.sollya.
#[rustfmt::skip]
static PSFAST: [f64; 5] = [
    fb(0x401921fb54442d18), // 0x1.921fb54442d18p+2
    fb(0x3cb1a62645446203), // 0x1.1a62645446203p-52
    fb(0xc044abbce625be53), // -0x1.4abbce625be53p+5
    fb(0x405466bc678d8d63), // 0x1.466bc678d8d63p+6
    fb(0xc05331554ca19669), // -0x1.331554ca19669p+6
];

/// Degree-6 even polynomial approximating cos2pi(x) for -2^-24 < x < 2^-11+2^-24
/// with relative error 2^-75.188 (degree 0 as h+l). Generated with cos_fast.sollya.
#[rustfmt::skip]
static PCFAST: [f64; 5] = [
    fb(0x3ff0000000000000), // 0x1.0000000000000p+0
    fb(0xbb2923015c000000), // -0x1.923015c000000p-77
    fb(0xc033bd3cc9be45de), // -0x1.3bd3cc9be45dep+4
    fb(0x40503c1f080ad892), // 0x1.03c1f080ad892p+6
    fb(0xc0555a5c590f9e6a), // -0x1.55a5c590f9e6ap+6
];

/// Degree-11 odd polynomial approximating sin2pi(x) for 0 <= x < 2^-11 with relative
/// error 2^-127.75. Generated with sin_accurate.sollya.
#[rustfmt::skip]
static PS: [Dint; 6] = [
    Dint::new(0xc90fdaa22168c234, 0xc4c6628b80dc1cd1, 3, 0), // 0
    Dint::new(0xa55de7312df295f5, 0x5dc72f712aa57db4, 6, 1), // 1
    Dint::new(0xa335e33bad570e92, 0x3f33be0021aa54d2, 7, 0), // 2
    Dint::new(0x9969667315ec2d9d, 0xe59d6ab8509a2025, 7, 1), // 3
    Dint::new(0xa83c1a43bf1c6485, 0x7d5f8f76fa7d74ed, 6, 0), // 4
    Dint::new(0xf16ab2898eae62f9, 0xa7f0339113b8b3c5, 4, 1), // 5
];

/// Degree-10 even polynomial approximating cos2pi(x) for 0 <= x < 2^-11 with relative
/// error 2^-137.246. Generated with cos_accurate.sollya.
#[rustfmt::skip]
static PC: [Dint; 6] = [
    Dint::new(0x8000000000000000, 0x0000000000000000, 1, 0), // 0
    Dint::new(0x9de9e64df22ef2d2, 0x56e26cd9808c1949, 5, 1), // 1
    Dint::new(0x81e0f840dad61d9a, 0x9980f00630cb655e, 7, 0), // 2
    Dint::new(0xaae9e3f1e5ffcfe2, 0xa508509534006249, 7, 1), // 3
    Dint::new(0xf0fa83448dd1e094, 0x0e0603ce7044eeba, 6, 0), // 4
    Dint::new(0xd368f6f4207cfe49, 0x0ec63157807ebffa, 5, 1), // 5
];

/// Table generated with ./buildSC 15 using accompanying buildSC.c.
/// For each i, 0 <= i < 256, xi=i/2^11+SC[i][0], with SC[i][1] and SC[i][2]
/// approximating sin2pi(xi) and cos2pi(xi) respectively, both with 53+15 bits of
/// accuracy.
#[rustfmt::skip]
static SC: [[f64; 3]; 256] = [
    [fb(0x0000000000000000), fb(0x0000000000000000), fb(0x3ff0000000000000)], // 0
    [fb(0xbdcc0f6c00000000), fb(0x3f6921f892b900fe), fb(0x3feffff621623fa0)], // 1
    [fb(0xbdc9c7935e000000), fb(0x3f7921f0ea27ce01), fb(0x3fefffd8858eca2e)], // 2
    [fb(0xbddd14d1ac000000), fb(0x3f82d96af779b0bb), fb(0x3fefffa72c986392)], // 3
    [fb(0xbdedba8f6a800000), fb(0x3f8921d1ce2d0a1c), fb(0x3fefff62169dddaa)], // 4
    [fb(0x3dfa6b7cdf000000), fb(0x3f8f6a29bdb73770), fb(0x3fefff0943c02419)], // 5
    [fb(0x3deb49618d000000), fb(0x3f92d936d1506f3d), fb(0x3feffe9cb44829c0)], // 6
    [fb(0xbdc398d6fc000000), fb(0x3f95fd4d1e21de6d), fb(0x3feffe1c687174b1)], // 7
    [fb(0xbe0e9e9a8c800000), fb(0x3f99215597791e0a), fb(0x3feffd886097afcf)], // 8
    [fb(0xbdf34e844c000000), fb(0x3f9c454f2e9480c7), fb(0x3feffce09ce95933)], // 9
    [fb(0xbdf989a8a4000000), fb(0x3f9f693709b94f92), fb(0x3feffc251dfbac0c)], // 10
    [fb(0x3e104a9b99000000), fb(0x3fa146860e69a571), fb(0x3feffb55e40a5c43)], // 11
    [fb(0xbdb56947c0000000), fb(0x3fa2d865748774ad), fb(0x3feffa72efff95d1)], // 12
    [fb(0xbdcc348768000000), fb(0x3fa46a396d34121a), fb(0x3feff97c420a8451)], // 13
    [fb(0x3df9e80552000000), fb(0x3fa5fc00e6e4c65c), fb(0x3feff871dacd8761)], // 14
    [fb(0x3dd3f11d74000000), fb(0x3fa78dbaa97099eb), fb(0x3feff753bb18af95)], // 15
    [fb(0x3dec039af4000000), fb(0x3fa91f65fc0abc0a), fb(0x3feff621e370ca7a)], // 16
    [fb(0x3dc53e1f80000000), fb(0x3faab101bf74ac2e), fb(0x3feff4dc54b00181)], // 17
    [fb(0x3e2114a649000000), fb(0x3fac428d7de920e9), fb(0x3feff3830f2e9043)], // 18
    [fb(0x3e0adf0ef4000000), fb(0x3fadd40723a3cdfb), fb(0x3feff21614b9d9ad)], // 19
    [fb(0xbe1d21f591800000), fb(0x3faf656e1e9e59cd), fb(0x3feff09565e83d77)], // 20
    [fb(0xbe14f54d70800000), fb(0x3fb07b612d6be078), fb(0x3fefef0102c634e3)], // 21
    [fb(0xbe11efec9a000000), fb(0x3fb1440118ba7bd0), fb(0x3fefed58ecf342da)], // 22
    [fb(0x3e2cc17ba8800000), fb(0x3fb20c96cf0a7eed), fb(0x3fefeb9d24646fa6)], // 23
    [fb(0x3de121dbe4000000), fb(0x3fb2d5209628edf0), fb(0x3fefe9cdacf99cff)], // 24
    [fb(0xbdd9ecf610000000), fb(0x3fb39d9f103bf7f7), fb(0x3fefe7ea854e6b08)], // 25
    [fb(0xbe004ede8e000000), fb(0x3fb466116c629e5c), fb(0x3fefe5f3af4ee201)], // 26
    [fb(0xbe01821cec000000), fb(0x3fb52e773c9920c7), fb(0x3fefe3e92c0e4108)], // 27
    [fb(0x3e0cdec726000000), fb(0x3fb5f6d02131f0b2), fb(0x3fefe1cafc7f1a24)], // 28
    [fb(0xbe0edece4d000000), fb(0x3fb6bf1b2653648c), fb(0x3fefdf99233c230c)], // 29
    [fb(0xbe02aa4d1c000000), fb(0x3fb787585bc45f0f), fb(0x3fefdd53a01d11d9)], // 30
    [fb(0x3dfd461592000000), fb(0x3fb84f871e32cf68), fb(0x3fefdafa74f16482)], // 31
    [fb(0x3e2f0cbd72800000), fb(0x3fb917a71d3d2956), fb(0x3fefd88da29f302e)], // 32
    [fb(0xbe15832470000000), fb(0x3fb9dfb6c9865b06), fb(0x3fefd60d2e14a6b1)], // 33
    [fb(0xbe12e81bf4000000), fb(0x3fbaa7b706bfdbba), fb(0x3fefd3791484ff50)], // 34
    [fb(0xbe31394141800000), fb(0x3fbb6fa680a05c27), fb(0x3fefd0d15a4b8471)], // 35
    [fb(0x3e171098ff000000), fb(0x3fbc3785eba12b42), fb(0x3fefce15fceddccf)], // 36
    [fb(0xbdfc3519e8000000), fb(0x3fbcff53302f0590), fb(0x3fefcb4703b969e1)], // 37
    [fb(0x3e42f522a5000000), fb(0x3fbdc70fb84af16e), fb(0x3fefc8646987fc1d)], // 38
    [fb(0xbdeae9bed8000000), fb(0x3fbe8eb7f8a589e2), fb(0x3fefc56e3b91ca3a)], // 39
    [fb(0x3e1f8868b2000000), fb(0x3fbf564e87d2330f), fb(0x3fefc264701f9a09)], // 40
    [fb(0xbe2b07985f800000), fb(0x3fc00ee8835051f4), fb(0x3fefbf47105f7439)], // 41
    [fb(0x3e1cbdaa94000000), fb(0x3fc072a05e1d4d8e), fb(0x3fefbc16172a9e36)], // 42
    [fb(0x3e337c5b90800000), fb(0x3fc0d64df9619f0d), fb(0x3fefb8d18b635327)], // 43
    [fb(0xbe3068b5fc800000), fb(0x3fc139f09bc617f5), fb(0x3fefb5797351da85)], // 44
    [fb(0xbe28ea6681800000), fb(0x3fc19d8919fa4ec8), fb(0x3fefb20dc7da8aff)], // 45
    [fb(0x3e36278ceb800000), fb(0x3fc2011719d50b87), fb(0x3fefae8e8bd4427f)], // 46
    [fb(0xbe2096df84000000), fb(0x3fc264993433763a), fb(0x3fefaafbcbfca356)], // 47
    [fb(0x3e29b2534f000000), fb(0x3fc2c810967bbf70), fb(0x3fefa7557d8d987e)], // 48
    [fb(0x3dd215b4e0000000), fb(0x3fc32b7bfa25c91b), fb(0x3fefa39bac71954b)], // 49
    [fb(0xbe194db891000000), fb(0x3fc38edb9d29b39d), fb(0x3fef9fce56700a6d)], // 50
    [fb(0x3e27727f7b800000), fb(0x3fc3f22f7c3cce3a), fb(0x3fef9bed7b8c8d8c)], // 51
    [fb(0xbe20cb3303800000), fb(0x3fc45576971dd530), fb(0x3fef97f925d53c83)], // 52
    [fb(0xbe09071106000000), fb(0x3fc4b8b175c71e22), fb(0x3fef93f14feb8022)], // 53
    [fb(0x3e262741e7800000), fb(0x3fc51bdfa7ea30d5), fb(0x3fef8fd5fe3efac8)], // 54
    [fb(0x3e3f8e16d0c00000), fb(0x3fc57f00e80e6e12), fb(0x3fef8ba733a1ceb1)], // 55
    [fb(0xbe076acbca000000), fb(0x3fc5e2143b7bc1c2), fb(0x3fef8764fad5e9bf)], // 56
    [fb(0xbe10a0f73a000000), fb(0x3fc6451a76411746), fb(0x3fef830f4ad232d8)], // 57
    [fb(0x3e3ca11d1bc00000), fb(0x3fc6a8135d7bd143), fb(0x3fef7ea625eb5af7)], // 58
    [fb(0xbe202f2362800000), fb(0x3fc70afd74071191), fb(0x3fef7a299d3f182a)], // 59
    [fb(0x3e2b34dcb8000000), fb(0x3fc76dda08544b5c), fb(0x3fef7599a1ac7ecd)], // 60
    [fb(0x3df161ff40000000), fb(0x3fc7d0a7bf2d4aba), fb(0x3fef70f64322da74)], // 61
    [fb(0xbe0c49b8b4000000), fb(0x3fc83366ddb3de23), fb(0x3fef6c3f7e7c2707)], // 62
    [fb(0x3e221da851000000), fb(0x3fc8961743b14290), fb(0x3fef6775552a6ba2)], // 63
    [fb(0x3e1ac63eda000000), fb(0x3fc8f8b851098588), fb(0x3fef6297cef0cdd6)], // 64
    [fb(0x3e427ef489c00000), fb(0x3fc95b4a5b9f2ceb), fb(0x3fef5da6e7820551)], // 65
    [fb(0x3e1ae89370000000), fb(0x3fc9bdcc07900146), fb(0x3fef58a2b0689c82)], // 66
    [fb(0x3e2eb48c7e000000), fb(0x3fca203e4a4f950e), fb(0x3fef538b1d392049)], // 67
    [fb(0xbe2bfd282f000000), fb(0x3fca829ffaad0d79), fb(0x3fef4e603d51f1aa)], // 68
    [fb(0x3e27ccf638000000), fb(0x3fcae4f1fa80e1b5), fb(0x3fef492204c5ef9e)], // 69
    [fb(0xbe32435c57800000), fb(0x3fcb4732b72ebc86), fb(0x3fef43d0890e1e72)], // 70
    [fb(0x3e10293fec000000), fb(0x3fcba9634155f866), fb(0x3fef3e6bbb6c2ea4)], // 71
    [fb(0xbe27bb1f92000000), fb(0x3fcc0b82461f65e0), fb(0x3fef38f3ae6f9afc)], // 72
    [fb(0x3e227aaebc000000), fb(0x3fcc6d906faacf65), fb(0x3fef3368589e17a2)], // 73
    [fb(0xbe42e2bcd5000000), fb(0x3fcccf8c3f74a6c9), fb(0x3fef2dc9cfb5fa74)], // 74
    [fb(0xbe16f070ac000000), fb(0x3fcd31773ba218a8), fb(0x3fef2817fd4d045b)], // 75
    [fb(0x3e2469adfc000000), fb(0x3fcd935004779e57), fb(0x3fef2252f59c122d)], // 76
    [fb(0x3df4f51c18000000), fb(0x3fcdf5164301377a), fb(0x3fef1c7abdeaa3ef)], // 77
    [fb(0x3e278e44da000000), fb(0x3fce56ca4202807c), fb(0x3fef168f51c5d5d5)], // 78
    [fb(0x3df49bb5f8000000), fb(0x3fceb86b4a1b7e9b), fb(0x3fef1090bc4b6800)], // 79
    [fb(0xbe367ba541000000), fb(0x3fcf19f9369d5e93), fb(0x3fef0a7effdc937f)], // 80
    [fb(0x3e2c0cab95000000), fb(0x3fcf7b74ab7219d2), fb(0x3fef045a1219e594)], // 81
    [fb(0xbe12b77e32000000), fb(0x3fcfdcdc0ca3288d), fb(0x3feefe220cf5c751)], // 82
    [fb(0xbdee0d8cb0000000), fb(0x3fd01f18054c8362), fb(0x3feef7d6e54c347d)], // 83
    [fb(0xbe2ecd5b9c000000), fb(0x3fd04fb7f6d35d68), fb(0x3feef178a6f9a987)], // 84
    [fb(0x3e2eb24de5000000), fb(0x3fd0804e1d369ff2), fb(0x3feeeb074934fdf0)], // 85
    [fb(0x3e14a897c4000000), fb(0x3fd0b0d9d7b0d042), fb(0x3feee482e14bcde0)], // 86
    [fb(0x3e1336c376000000), fb(0x3fd0e15b555e7bec), fb(0x3feeddeb6908ca8c)], // 87
    [fb(0xbe03952d90000000), fb(0x3fd111d25efd48b8), fb(0x3feed740e7eb8dd6)], // 88
    [fb(0x3e0fc2a5d4000000), fb(0x3fd1423ef5c7e1bd), fb(0x3feed0835dc24e89)], // 89
    [fb(0x3e2a88ed37000000), fb(0x3fd172a0eb8361da), fb(0x3feec9b2d0ec8288)], // 90
    [fb(0xbe48ca4cb9400000), fb(0x3fd1a2f7b10b6d70), fb(0x3feec2cf55d6117c)], // 91
    [fb(0x3e40144524000000), fb(0x3fd1d3446fd0cd3f), fb(0x3feebbd8c1d62f96)], // 92
    [fb(0xbe3abf810c000000), fb(0x3fd203855b85f89a), fb(0x3feeb4cf57454132)], // 93
    [fb(0x3e35d4c5d5800000), fb(0x3fd233bbcca40561), fb(0x3feeadb2e40746ca)], // 94
    [fb(0xbe2a1b0c58000000), fb(0x3fd263e685b1d714), fb(0x3feea68396d87754)], // 95
    [fb(0xbe277c8dac000000), fb(0x3fd294061d2eb611), fb(0x3fee9f41597393c8)], // 96
    [fb(0x3e1915540e000000), fb(0x3fd2c41a580014cf), fb(0x3fee97ec348fb87f)], // 97
    [fb(0xbe3abb6d9b000000), fb(0x3fd2f422b2d0990c), fb(0x3fee90843c55b996)], // 98
    [fb(0xbe3b8ee5d5800000), fb(0x3fd3241f8cea2836), fb(0x3fee890962268c49)], // 99
    [fb(0xbe31cd2982800000), fb(0x3fd35410a8396266), fb(0x3fee817baf85c094)], // 100
    [fb(0xbdfe216af0000000), fb(0x3fd383f5e08283e2), fb(0x3fee79db2a188b0a)], // 101
    [fb(0xbe024afc30000000), fb(0x3fd3b3cef6993c0b), fb(0x3fee7227dbf82004)], // 102
    [fb(0xbe0aa1657c000000), fb(0x3fd3e39be4767224), fb(0x3fee6a61c62d5274)], // 103
    [fb(0xbe1c5b65fa000000), fb(0x3fd4135c898485bb), fb(0x3fee6288ee07fea5)], // 104
    [fb(0x3df23e8978000000), fb(0x3fd44310de3c284b), fb(0x3fee5a9d54bbd26c)], // 105
    [fb(0xbe22b1d77a000000), fb(0x3fd472b8976d498d), fb(0x3fee529f06cb187d)], // 106
    [fb(0xbe0daaa348000000), fb(0x3fd4a253cb97efd1), fb(0x3fee4a8e007231a2)], // 107
    [fb(0xbe3322f570800000), fb(0x3fd4d1e2260c3422), fb(0x3fee426a500f6e33)], // 108
    [fb(0x3e264758e8000000), fb(0x3fd50163eca0b337), fb(0x3fee3a33e996b722)], // 109
    [fb(0x3e31248627800000), fb(0x3fd530d89a17e007), fb(0x3fee31eae3fb917b)], // 110
    [fb(0xbe46c3416cc00000), fb(0x3fd5603fcf8cd8a3), fb(0x3fee298f502a579b)], // 111
    [fb(0x3e2ab481ff000000), fb(0x3fd58f9a896aa209), fb(0x3fee2121016e14fc)], // 112
    [fb(0xbe26eb838b000000), fb(0x3fd5bee77aaf890b), fb(0x3fee18a032eb4df5)], // 113
    [fb(0xbdfd159b80000000), fb(0x3fd5ee2734efeef5), fb(0x3fee100ccaa6bd78)], // 114
    [fb(0xbdda42e4a0000000), fb(0x3fd61d595bedeabc), fb(0x3fee0766d944915e)], // 115
    [fb(0xbe143d0dc0000000), fb(0x3fd64c7dd5cc0cd1), fb(0x3fedfeae63903034)], // 116
    [fb(0xbe48c7bdb7000000), fb(0x3fd67b9453ca2122), fb(0x3fedf5e378482eae)], // 117
    [fb(0x3e11c0ead6000000), fb(0x3fd6aa9d844c980a), fb(0x3feded05f6a23a52)], // 118
    [fb(0x3e07d52600000000), fb(0x3fd6d99867e90d92), fb(0x3fede4160e97b2e2)], // 119
    [fb(0x3e3924e036800000), fb(0x3fd7088555d3c816), fb(0x3feddb13afb14e37)], // 120
    [fb(0xbe174b7c3e000000), fb(0x3fd73763c09fba09), fb(0x3fedd1fef5335416)], // 121
    [fb(0xbe17943ad0000000), fb(0x3fd766340685c982), fb(0x3fedc8d7ccf2567a)], // 122
    [fb(0x3e279dd614000000), fb(0x3fd794f5f7522b88), fb(0x3fedbf9e402aa5c3)], // 123
    [fb(0x3e17b64f32000000), fb(0x3fd7c3a939c32d81), fb(0x3fedb652607e0db1)], // 124
    [fb(0xbe32bea5ce800000), fb(0x3fd7f24db825141c), fb(0x3fedacf43268b5b0)], // 125
    [fb(0x3e1733c024000000), fb(0x3fd820e3b8bf15a0), fb(0x3feda383a7aed887)], // 126
    [fb(0xbe4eac0fc9400000), fb(0x3fd84f6a51d077b3), fb(0x3fed9a00efd84537)], // 127
    [fb(0x3e4aca3733800000), fb(0x3fd87de2f4704f98), fb(0x3fed906bbf17f4da)], // 128
    [fb(0xbe1910c4f0000000), fb(0x3fd8ac4b7dc0d986), fb(0x3fed86c4862b5d6e)], // 129
    [fb(0xbe033bb860000000), fb(0x3fd8daa52b4dc041), fb(0x3fed7d0b0374a559)], // 130
    [fb(0xbe469e1507000000), fb(0x3fd908ef408ad220), fb(0x3fed733f5e71c3bc)], // 131
    [fb(0x3e4cffacf0800000), fb(0x3fd9372ab7784d36), fb(0x3fed696161d786c9)], // 132
    [fb(0xbe58629d9f000000), fb(0x3fd965552b0849ab), fb(0x3fed5f7190eeae23)], // 133
    [fb(0x3e14150000000000), fb(0x3fd99371687c64f3), fb(0x3fed556f5155d9dd)], // 134
    [fb(0xbe4bd37aad800000), fb(0x3fd9c17cf40715cb), fb(0x3fed4b5b2caf8386)], // 135
    [fb(0x3e5d02cde7000000), fb(0x3fd9ef79ea4d995d), fb(0x3fed4134ac5eb246)], // 136
    [fb(0xbe110547ac000000), fb(0x3fda1d653d9adf5e), fb(0x3fed36fc7d291602)], // 137
    [fb(0xbe401a1a22800000), fb(0x3fda4b40f9c0120b), fb(0x3fed2cb22b45236b)], // 138
    [fb(0x3e23ce2bac000000), fb(0x3fda790ce2056b9a), fb(0x3fed2255c3ae11a5)], // 139
    [fb(0xbdfccb4a60000000), fb(0x3fdaa6c828db4ea8), fb(0x3fed17e774d4e3e2)], // 140
    [fb(0x3e25db4b00000000), fb(0x3fdad47321f29847), fb(0x3fed0d672bc0b122)], // 141
    [fb(0x3e232f6a6e000000), fb(0x3fdb020d7a285e23), fb(0x3fed02d4fb84d334)], // 142
    [fb(0x3e5cf8e39bc00000), fb(0x3fdb2f97c27f7494), fb(0x3fecf830c2248c5e)], // 143
    [fb(0x3e18927bb0000000), fb(0x3fdb5d10129a750a), fb(0x3feced7af22cb105)], // 144
    [fb(0xbe33dec3c1000000), fb(0x3fdb8a77f8d0bbc5), fb(0x3fece2b32e50d6cd)], // 145
    [fb(0xbe326ba536000000), fb(0x3fdbb7cf08f0290d), fb(0x3fecd7d98fcf3b1e)], // 146
    [fb(0x3e223c568e000000), fb(0x3fdbe51524e3aa53), fb(0x3fecccee1da3d56e)], // 147
    [fb(0xbe2f3b3af0000000), fb(0x3fdc1249c1f5f2f6), fb(0x3fecc1f0f95e1e24)], // 148
    [fb(0xbe31286a47000000), fb(0x3fdc3f6d2ef7054b), fb(0x3fecb6e20ff37e81)], // 149
    [fb(0x3e2641214e000000), fb(0x3fdc6c7f594003d9), fb(0x3fecabc165bf1b60)], // 150
    [fb(0x3e40cda7c9000000), fb(0x3fdc997ff2bffccb), fb(0x3feca08f0dee434c)], // 151
    [fb(0xbe35557ac9000000), fb(0x3fdcc66e7b42e8f1), fb(0x3fec954b28bca62e)], // 152
    [fb(0x3e3555eb62000000), fb(0x3fdcf34bccc567a1), fb(0x3fec89f57f6e20f3)], // 153
    [fb(0xbe34e0e361000000), fb(0x3fdd2016cbb5e39a), fb(0x3fec7e8e59999e1f)], // 154
    [fb(0x3e2446da1e000000), fb(0x3fdd4cd039d0ed05), fb(0x3fec731585f970eb)], // 155
    [fb(0x3e2103d328000000), fb(0x3fdd797767638dec), fb(0x3fec678b3174afe1)], // 156
    [fb(0x3e35814d60000000), fb(0x3fdda60c7ae9dc22), fb(0x3fec5bef522be6fb)], // 157
    [fb(0xbe25e2321e000000), fb(0x3fddd28f054cbb3f), fb(0x3fec5042052c8c42)], // 158
    [fb(0xbe2a259ffe000000), fb(0x3fddfeff54854631), fb(0x3fec44833611bc7d)], // 159
    [fb(0xbe04f28d80000000), fb(0x3fde2b5d34665b35), fb(0x3fec38b2f278ea7e)], // 160
    [fb(0xbdbde57100000000), fb(0x3fde57a86d137f20), fb(0x3fec2cd1493d05c2)], // 161
    [fb(0x3e2e0d8d14000000), fb(0x3fde83e0ffb7bfb4), fb(0x3fec20de3a08ea07)], // 162
    [fb(0xbe312a858e000000), fb(0x3fdeb0067e48baf4), fb(0x3fec14d9e2bd511e)], // 163
    [fb(0x3e49a17403000000), fb(0x3fdedc19997a4431), fb(0x3fec08c413089b2e)], // 164
    [fb(0x3e268c8636000000), fb(0x3fdf0819163d1bc0), fb(0x3febfc9d21568f32)], // 165
    [fb(0x3e24cc5eb8000000), fb(0x3fdf3405a482e11d), fb(0x3febf064dd580fc9)], // 166
    [fb(0xbe4fce7cd8000000), fb(0x3fdf5fde8f3f11d4), fb(0x3febe41b798f6b97)], // 167
    [fb(0xbe2af81690000000), fb(0x3fdf8ba4c98a9816), fb(0x3febd7c0b1a7f14b)], // 168
    [fb(0x3de6e39e20000000), fb(0x3fdfb7575d1ea750), fb(0x3febcb54cac5dde5)], // 169
    [fb(0x3e330f9256000000), fb(0x3fdfe2f665dcd168), fb(0x3febbed7bd1e17b0)], // 170
    [fb(0x3e0626de20000000), fb(0x3fe00740ca0d5fbb), fb(0x3febb2499f9fe7a3)], // 171
    [fb(0x3e15cc7030000000), fb(0x3fe01cfc8afeea0e), fb(0x3feba5aa650dd495)], // 172
    [fb(0xbdf6191e60000000), fb(0x3fe032ae54fe4057), fb(0x3feb98fa2065a5e6)], // 173
    [fb(0xbe06b14850000000), fb(0x3fe0485624c328c8), fb(0x3feb8c38d39737bc)], // 174
    [fb(0xbe211fbc3a000000), fb(0x3fe05df3e66a716d), fb(0x3feb7f668a580fd0)], // 175
    [fb(0xbe40eca7f0000000), fb(0x3fe07387825589ec), fb(0x3feb728352c44517)], // 176
    [fb(0xbe68073bc9e00000), fb(0x3fe089109ef1284d), fb(0x3feb658f630112ed)], // 177
    [fb(0xbe49dcf0ad000000), fb(0x3fe09e9051603e29), fb(0x3feb588a13ab750f)], // 178
    [fb(0xbe206ea9f0000000), fb(0x3fe0b405820e78e7), fb(0x3feb4b740d3cc07b)], // 179
    [fb(0xbe136a8d0c000000), fb(0x3fe0c9704a1ea4e5), fb(0x3feb3e4d40f5524d)], // 180
    [fb(0x3e163d1f30000000), fb(0x3fe0ded0bc01a533), fb(0x3feb3115a3a628af)], // 181
    [fb(0x3e5f3181f1400000), fb(0x3fe0f4270e4787bf), fb(0x3feb23cd1314c779)], // 182
    [fb(0xbe2f269b78000000), fb(0x3fe109723e75c5cf), fb(0x3feb167430cfebdb)], // 183
    [fb(0x3e41d84dc0800000), fb(0x3fe11eb36bc9db52), fb(0x3feb090a4915ee88)], // 184
    [fb(0xbe408e6006800000), fb(0x3fe133e9ba0061d8), fb(0x3feafb8fe69a6527)], // 185
    [fb(0x3e4cda72ab000000), fb(0x3fe14915d557a7c9), fb(0x3feaee049bc0aee0)], // 186
    [fb(0xbe1f32f950000000), fb(0x3fe15e36dfb6bb55), fb(0x3feae068f6991699)], // 187
    [fb(0x3e3138092d000000), fb(0x3fe1734d6f34d7f0), fb(0x3fead2bc96c1e1f5)], // 188
    [fb(0x3e56b382dd400000), fb(0x3fe188595ae376a5), fb(0x3feac4ff962bdb6d)], // 189
    [fb(0xbe3f12fafa000000), fb(0x3fe19d59f592a587), fb(0x3feab7326685eb57)], // 190
    [fb(0xbe32909e5a000000), fb(0x3fe1b2500aed7ac6), fb(0x3feaa954823cf815)], // 191
    [fb(0xbe6d66a897800000), fb(0x3fe1c73aa0150cf9), fb(0x3fea9b668fb0503f)], // 192
    [fb(0x3e4311ea86000000), fb(0x3fe1dc1b7db74db1), fb(0x3fea8d675d9c6cc8)], // 193
    [fb(0xbe041c02b8000000), fb(0x3fe1f0f08a1a06a4), fb(0x3fea7f5853bb4309)], // 194
    [fb(0xbe5ca1f4ed000000), fb(0x3fe205ba57211271), fb(0x3fea71391146958f)], // 195
    [fb(0xbe3910ce77000000), fb(0x3fe21a7988f8326b), fb(0x3fea63092626202f)], // 196
    [fb(0x3e62bfadbee00000), fb(0x3fe22f2dc71afab6), fb(0x3fea54c8cd9fd0d9)], // 197
    [fb(0xbe45f1c02a800000), fb(0x3fe243d5df4afb93), fb(0x3fea4678dbbe5e73)], // 198
    [fb(0xbe1db12b90000000), fb(0x3fe2587347f493a4), fb(0x3fea38184db0df23)], // 199
    [fb(0xbe17b29e00000000), fb(0x3fe26d05490f2f61), fb(0x3fea29a7a2f40b49)], // 200
    [fb(0xbe2b3ddca4000000), fb(0x3fe2818be6930629), fb(0x3fea1b26d8f070d7)], // 201
    [fb(0x3e2e112744000000), fb(0x3fe2960730ff2bcd), fb(0x3fea0c95e3df5e0e)], // 202
    [fb(0xbe35269766000000), fb(0x3fe2aa76dafcbbf4), fb(0x3fe9fdf4fae1df6f)], // 203
    [fb(0xbe309777e1000000), fb(0x3fe2bedb1b6b4e15), fb(0x3fe9ef43f6cbe162)], // 204
    [fb(0x3e3ae2051f000000), fb(0x3fe2d333e4617f25), fb(0x3fe9e082e148680e)], // 205
    [fb(0xbe436f6ced800000), fb(0x3fe2e780cb47180e), fb(0x3fe9d1b207f383c3)], // 206
    [fb(0xbe323fdc6b000000), fb(0x3fe2fbc23fba2f44), fb(0x3fe9c2d1197130a7)], // 207
    [fb(0x3debc540e0000000), fb(0x3fe30ff7fd6d967d), fb(0x3fe9b3e0478b961b)], // 208
    [fb(0xbe3cfb4ed7000000), fb(0x3fe32421da0bf0e9), fb(0x3fe9a4dfb1c89326)], // 209
    [fb(0x3e555802aec00000), fb(0x3fe3384042a92b1d), fb(0x3fe995cf06920d11)], // 210
    [fb(0x3e360719e4000000), fb(0x3fe34c52608e3a92), fb(0x3fe986aee6d6837e)], // 211
    [fb(0xbe1cbf2e48000000), fb(0x3fe36058ac8863b6), fb(0x3fe9777ef832c986)], // 212
    [fb(0x3e49061c32000000), fb(0x3fe374533ab707d0), fb(0x3fe9683f2ad7e2ec)], // 213
    [fb(0xbe4da84dfe000000), fb(0x3fe3884160f9488f), fb(0x3fe958f000fdd50a)], // 214
    [fb(0x3e292e8a74000000), fb(0x3fe39c23eba6b22a), fb(0x3fe94990dd9cee51)], // 215
    [fb(0xbe2bff5d9a000000), fb(0x3fe3affa20756bdd), fb(0x3fe93a225056084a)], // 216
    [fb(0x3db4c46200000000), fb(0x3fe3c3c4498e98eb), fb(0x3fe92aa41fbb951c)], // 217
    [fb(0xbe3e4613e9000000), fb(0x3fe3d782261dff62), fb(0x3fe91b167e92d706)], // 218
    [fb(0x3e10eb2964000000), fb(0x3fe3eb33ed579bbe), fb(0x3fe90b794146043c)], // 219
    [fb(0xbe260abec2000000), fb(0x3fe3fed94c834d8a), fb(0x3fe8fbcca9583479)], // 220
    [fb(0x3e46954977000000), fb(0x3fe4127281ddac03), fb(0x3fe8ec1085083553)], // 221
    [fb(0x3e2a16fec2000000), fb(0x3fe425ff1f841235), fb(0x3fe8dc452ca328d3)], // 222
    [fb(0xbe427bcdd3000000), fb(0x3fe4397f44aa44f2), fb(0x3fe8cc6a8771e165)], // 223
    [fb(0xbe360dded4000000), fb(0x3fe44cf317a563db), fb(0x3fe8bc8076122736)], // 224
    [fb(0xbe59a8f405c00000), fb(0x3fe4605a2b02d705), fb(0x3fe8ac875232f3ef)], // 225
    [fb(0x3e432777dc000000), fb(0x3fe473b532bc5a67), fb(0x3fe89c7e8713120c)], // 226
    [fb(0xbe51418a7b000000), fb(0x3fe4870306ca20e2), fb(0x3fe88c670a0ea774)], // 227
    [fb(0xbe3fed182e000000), fb(0x3fe49a44886b5340), fb(0x3fe87c401fdf05e5)], // 228
    [fb(0x3e486144d8000000), fb(0x3fe4ad796ea1410c), fb(0x3fe86c0a04dbacc5)], // 229
    [fb(0x3de1bc2e60000000), fb(0x3fe4c0a14640d2af), fb(0x3fe85bc51aa114c2)], // 230
    [fb(0xbe3f53d2fe000000), fb(0x3fe4d3bc5aaa8cd5), fb(0x3fe84b7121b30a13)], // 231
    [fb(0xbe12e100a0000000), fb(0x3fe4e6cab91556be), fb(0x3fe83b0e0e6b6ccc)], // 232
    [fb(0xbe2fa58c62000000), fb(0x3fe4f9cc1c69fdde), fb(0x3fe82a9c1c1ab463)], // 233
    [fb(0x3debb491e0000000), fb(0x3fe50cc09fdcbd92), fb(0x3fe81a1b3342f858)], // 234
    [fb(0x3e3a115410000000), fb(0x3fe51fa82c3aa029), fb(0x3fe8098b67ea8509)], // 235
    [fb(0x3e4ab0a5d3000000), fb(0x3fe53282b20b96b6), fb(0x3fe7f8ecc7919530)], // 236
    [fb(0xbe3cba0438000000), fb(0x3fe5454fe43a7d7c), fb(0x3fe7e83f96af78a0)], // 237
    [fb(0xbe20dd83a4000000), fb(0x3fe5581033a81573), fb(0x3fe7d783712e20ec)], // 238
    [fb(0xbe3e9a8299000000), fb(0x3fe56ac33fbb8253), fb(0x3fe7c6b8acf90fa6)], // 239
    [fb(0x3e2225c4aa000000), fb(0x3fe57d6939d4b513), fb(0x3fe7b5df1da18065)], // 240
    [fb(0xbe482e66e0000000), fb(0x3fe59001b9e64d79), fb(0x3fe7a4f72157cfdf)], // 241
    [fb(0x3e551a6a35400000), fb(0x3fe5a28d5b36d597), fb(0x3fe794002a7c9023)], // 242
    [fb(0x3e513917f4000000), fb(0x3fe5b50b4e10bec1), fb(0x3fe782faf6dc7ba2)], // 243
    [fb(0x3e149310cc000000), fb(0x3fe5c77bc15ab4ef), fb(0x3fe771e75c43942e)], // 244
    [fb(0x3e124d493c000000), fb(0x3fe5d9dee9de49db), fb(0x3fe760c529bc17b0)], // 245
    [fb(0xbe504638f7000000), fb(0x3fe5ec347044e0f4), fb(0x3fe74f94b0af9720)], // 246
    [fb(0xbe23f41b28000000), fb(0x3fe5fe7cb834600c), fb(0x3fe73e55936a5160)], // 247
    [fb(0xbe1a5f6f5c000000), fb(0x3fe610b7515d1562), fb(0x3fe72d083b8214eb)], // 248
    [fb(0x3e319fb2e0000000), fb(0x3fe622e459eafbc1), fb(0x3fe71bac8c7b0592)], // 249
    [fb(0xbe356d2c2b000000), fb(0x3fe6350396fe4e62), fb(0x3fe70a42bec51665)], // 250
    [fb(0xbe33c156c2000000), fb(0x3fe64715385bed93), fb(0x3fe6f8caa4969708)], // 251
    [fb(0xbe2f23e576000000), fb(0x3fe659191d2fd57f), fb(0x3fe6e7445d74f711)], // 252
    [fb(0x3e11e4be38000000), fb(0x3fe66b0f41d484c4), fb(0x3fe6d5afecd4938d)], // 253
    [fb(0xbe4397cc8d800000), fb(0x3fe67cf76eac73df), fb(0x3fe6c40d89625f63)], // 254
    [fb(0xbe3202f686000000), fb(0x3fe68ed1e0990551), fb(0x3fe6b25cf728c350)], // 255
];

/// Multiply exactly a and b, such that hi + lo = a * b (`a_mul`).
#[inline(always)]
fn a_mul(a: f64, b: f64) -> (f64, f64) {
    let hi = a * b;
    let lo = a.mul_add(b, -hi);
    (hi, lo)
}

/// Multiply a double with a double double: a * (bh + bl), with error bounded by
/// ulp(lo) (`s_mul`).
#[inline(always)]
fn s_mul(a: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, lo) = a_mul(a, bh); // exact
    let lo = a.mul_add(bl, lo);
    // the error is bounded by ulp(lo), where |lo| < |a*bl| + ulp(hi)
    (hi, lo)
}

/// Returns (ah + al) * (bh + bl) - (al * bl) (`d_mul`).
/// We can ignore al * bl when assuming al <= ulp(ah) and bl <= ulp(bh).
#[inline(always)]
fn d_mul(ah: f64, al: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, s) = a_mul(ah, bh);
    let t = al.mul_add(bh, s);
    let lo = ah.mul_add(bl, t);
    (hi, lo)
}

#[inline(always)]
fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let hi = a + b;
    let e = hi - a; // exact
    let lo = b - e; // exact
    (hi, lo)
}

/// Put in h+l an approximation of sin2pi(xh+xl), for 2^-24 <= xh+xl < 2^-11 + 2^-24,
/// and |xl| < 2^-52.36, with absolute error < 2^-77.09 (see evalPSfast() in sin.sage).
/// Assume uh + ul approximates (xh+xl)^2 (`evalPSfast`).
#[inline(always)]
fn eval_ps_fast(xh: f64, xl: f64, uh: f64, ul: f64) -> (f64, f64) {
    let mut h = PSFAST[4]; // degree 7
    h = h.mul_add(uh, PSFAST[3]); // degree 5
    h = h.mul_add(uh, PSFAST[2]); // degree 3
    let mut l;
    (h, l) = s_mul(h, uh, ul);
    let t;
    (h, t) = fast_two_sum(PSFAST[0], h);
    l += PSFAST[1] + t;
    // multiply by xh+xl
    d_mul(h, l, xh, xl)
}

/// Put in h+l an approximation of cos2pi(xh+xl), for 2^-24 <= xh+xl < 2^-11 + 2^-24,
/// and |xl| < 2^-52.36, with relative error < 2^-69.96 (see evalPCfast() in sin.sage).
/// Assume uh + ul approximates (xh+xl)^2 (`evalPCfast`).
#[inline(always)]
fn eval_pc_fast(uh: f64, ul: f64) -> (f64, f64) {
    let mut h = PCFAST[4]; // degree 6
    h = h.mul_add(uh, PCFAST[3]); // degree 4
    h = h.mul_add(uh, PCFAST[2]); // degree 2
    let mut l;
    (h, l) = s_mul(h, uh, ul);
    let t;
    (h, t) = fast_two_sum(PCFAST[0], h);
    l += PCFAST[1] + t;
    (h, l)
}

/// Put in Y an approximation of sin2pi(X), for 0 <= X < 2^-11, where X2 approximates
/// X^2. Absolute error bounded by 2^-132.999 with 0 <= Y < 0.003068 (see evalPS() in
/// sin.sage), and relative error bounded by 2^-124.648 (`evalPS`).
#[inline]
fn eval_ps(x: &Dint, x2: &Dint) -> Dint {
    let mut y = mul_dint_21(x2, &PS[5]); // degree 11
    y = add_dint(&y, &PS[4]); // degree 9
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PS[3]); // degree 7
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PS[2]); // degree 5
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PS[1]); // degree 3
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PS[0]); // degree 1
    mul_dint(&y, x) // multiply by X
}

/// Put in Y an approximation of cos2pi(X), for 0 <= X < 2^-11, where X2 approximates
/// X^2. Absolute/relative error bounded by 2^-125.999 with 0.999995 < Y <= 1
/// (see evalPC() in sin.sage) (`evalPC`).
#[inline]
fn eval_pc(x2: &Dint) -> Dint {
    let mut y = mul_dint_21(x2, &PC[5]); // degree 10
    y = add_dint(&y, &PC[4]); // degree 8
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PC[3]); // degree 6
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PC[2]); // degree 4
    y = mul_dint(&y, x2);
    y = add_dint(&y, &PC[1]); // degree 2
    y = mul_dint(&y, x2);
    add_dint(&y, &PC[0]) // degree 0
}

/// Normalize X such that X->hi has its most significant bit set (if X <> 0)
/// (`normalize`).
#[inline]
fn normalize(x: &mut Dint) {
    if x.hi != 0 {
        let cnt = x.hi.leading_zeros();
        if cnt != 0 {
            x.hi = (x.hi << cnt) | (x.lo >> (64 - cnt));
            x.lo <<= cnt;
        }
        x.ex -= i64::from(cnt);
    } else if x.lo != 0 {
        let cnt = x.lo.leading_zeros();
        x.hi = x.lo << cnt;
        x.lo = 0;
        x.ex -= 64 + i64::from(cnt);
    }
}

/// Approximate X/(2pi) mod 1. If Xin is the input value, and Xout the output value,
/// we have |Xout - (Xin/(2pi) mod 1)| < 2^-126.67*|Xout|.
/// Assert X is normalized at input, and normalize X at output (`reduce`).
#[inline]
fn reduce(x: &mut Dint) {
    let mut e = x.ex as i32;
    let mut u: u128;

    if e <= 1 {
        // |X| < 2
        // multiply by T[0]/2^64 + T[1]/2^128, where
        // |T[0]/2^64 + T[1]/2^128 - 1/(2pi)| < 2^-130.22
        u = u128::from(x.hi) * u128::from(T[1]);
        let tiny = u as u64;
        x.lo = (u >> 64) as u64;
        u = u128::from(x.hi) * u128::from(T[0]);
        x.lo = x.lo.wrapping_add(u as u64);
        x.hi = ((u >> 64) as u64).wrapping_add(u64::from(x.lo < u as u64));
        // hi + lo/2^64 + tiny/2^128 = hi_in * (T[0]/2^64 + T[1]/2^128); the
        // normalize() below performs a left shift by at most 3 bits
        e = x.ex as i32;
        normalize(x);
        e -= x.ex as i32;
        // put the upper e bits of tiny into X->lo
        if e != 0 {
            x.lo |= tiny >> (64 - e);
        }
        // maximal relative error: (1 + 2^-130.22) * (1 + 2^-127) - 1 < 2^-126.852.
        return;
    }

    // now 2 <= e <= 1024

    // Only the values of i such that -127 <= e-64i <= 127 contribute;
    // up to 4 consecutive values of T[i] (only 3 when e is a multiple of 64).
    let i = if e < 127 {
        0
    } else {
        ((e - 127 + 64 - 1) / 64) as usize
    }; // ceil((e-127)/64)
    // 0 <= i <= 15
    let mut c = [0u64; 5];
    u = u128::from(x.hi) * u128::from(T[i + 3]); // i+3 <= 18
    c[0] = u as u64;
    c[1] = (u >> 64) as u64;
    u = u128::from(x.hi) * u128::from(T[i + 2]);
    c[1] = c[1].wrapping_add(u as u64);
    c[2] = ((u >> 64) as u64).wrapping_add(u64::from(c[1] < u as u64));
    u = u128::from(x.hi) * u128::from(T[i + 1]);
    c[2] = c[2].wrapping_add(u as u64);
    c[3] = ((u >> 64) as u64).wrapping_add(u64::from(c[2] < u as u64));
    u = u128::from(x.hi) * u128::from(T[i]);
    c[3] = c[3].wrapping_add(u as u64);
    c[4] = ((u >> 64) as u64).wrapping_add(u64::from(c[3] < u as u64));

    // up to here, the ignored part hi*(T[i+4]+T[i+5]+...) can contribute by
    // less than 2^64 in c[0], thus less than 1 in c[1]

    let f = e - 64 * i as i32; // hi*T[i]/2^128 is multiplied by 2^f
    // {c, 5} = hi*(T[i]+T[i+1]/2^64+T[i+2]/2^128+T[i+3]/2^192)
    // now shift c[0..4] by f bits to the left
    let tiny: u64 = if f < 64 {
        x.hi = (c[4] << f) | (c[3] >> (64 - f));
        x.lo = (c[3] << f) | (c[2] >> (64 - f));
        // the ignored part was less than 1 in c[1],
        // thus less than 2^(f-64) <= 1/2 in tiny
        (c[2] << f) | (c[1] >> (64 - f))
    } else if f == 64 {
        x.hi = c[3];
        x.lo = c[2];
        // the ignored part was less than 1 in c[1], thus less than 1 in tiny
        c[1]
    } else {
        // 65 <= f <= 127: this case can only occur when e >= 65
        let g = f - 64; // 1 <= g <= 63
        // we compute an extra term
        u = u128::from(x.hi) * u128::from(T[i + 4]); // i+4 <= 19
        u >>= 64;
        // c[0] += u (the sum is computed in 128 bits and truncated to 64 bits)
        c[0] = (u128::from(c[0]).wrapping_add(u)) as u64;
        let carry = u128::from(c[0]) < u;
        c[1] = c[1].wrapping_add(u64::from(carry));
        c[2] = c[2].wrapping_add(u64::from(carry && c[1] == 0));
        c[3] = c[3].wrapping_add(u64::from(carry && c[1] == 0 && c[2] == 0));
        c[4] = c[4].wrapping_add(u64::from(carry && c[1] == 0 && c[2] == 0 && c[3] == 0));
        x.hi = (c[3] << g) | (c[2] >> (64 - g));
        x.lo = (c[2] << g) | (c[1] >> (64 - g));
        // the ignored part was less than 1 in c[0], thus less than 1/2 in tiny
        (c[1] << g) | (c[0] >> (64 - g))
    };
    // The approximation error between X/in(2pi) mod 1 and
    // X->hi/2^64 + X->lo/2^128 + tiny/2^192 is less than 2^-191 (absolute).
    x.ex = 0;
    normalize(x);
    // the worst case (for 2^25 <= x < 2^1024) is X->ex = -61, attained
    // for |x| = 0x1.6ac5b262ca1ffp+851
    if x.ex < 0 {
        // put the upper -ex bits of tiny into low bits of lo
        x.lo |= tiny >> (64 + x.ex);
    }
    // The relative error is thus bounded by 2^-126.67.
}

/// Given Xin:=X with 0 <= Xin < 1, return i and modify X such that
/// Xin = i/2^11 + Xout, with 0 <= Xout < 2^-11. This operation is exact (`reduce2`).
#[inline]
fn reduce2(x: &mut Dint) -> i32 {
    if x.ex <= -11 {
        return 0;
    }
    let sh = (64 - 11 - x.ex) as i32;
    let i = (x.hi >> sh) as i32;
    x.hi &= (1u64 << sh) - 1;
    normalize(x);
    i
}

/// h+l <- c1/2^64 + c0/2^128 (`set_dd`).
#[inline]
fn set_dd(mut c1: u64, mut c0: u64) -> (f64, f64) {
    let (h, l);
    if c1 != 0 {
        let e = u64::from(c1.leading_zeros());
        if e != 0 {
            c1 = (c1 << e) | (c0 >> (64 - e));
            c0 <<= e;
        }
        let f = 0x3fe - e;
        h = f64::from_bits((f << 52) | ((c1 << 1) >> 12));
        c0 = (c1 << 53) | (c0 >> 11);
        if c0 != 0 {
            let g = u64::from(c0.leading_zeros());
            if g != 0 {
                c0 <<= g;
            }
            l = f64::from_bits(((f - 53 - g) << 52) | ((c0 << 1) >> 12));
        } else {
            l = 0.0;
        }
    } else if c0 != 0 {
        let e = u64::from(c0.leading_zeros());
        let f = 0x3fe - 64 - e;
        c0 <<= e + 1; // most significant bit shifted out
        // put the upper 52 bits of c0 into h
        h = f64::from_bits((f << 52) | (c0 >> 12));
        // put the lower 12 bits of c0 into l
        c0 <<= 52;
        if c0 != 0 {
            let g = u64::from(c0.leading_zeros());
            c0 <<= g + 1;
            l = f64::from_bits(((f - 64 - g) << 52) | (c0 >> 12));
        } else {
            l = 0.0;
        }
    } else {
        h = 0.0;
        l = 0.0;
    }
    // | h + l - frac(x/(2pi)) | < 2^-75.999 + 2^-106 < 2^-75.998
    (h, l)
}

/// Assuming 0x1.6a09e667f3bccp-27 < x < +Inf, return i and set h,l such that
/// i/2^11+h+l approximates frac(x/(2pi)), together with a bound err1 for the absolute
/// error | i/2^11 + h + l - frac(x/(2pi)) | (`reduce_fast`).
/// If x <= 0x1.921fb54442d18p+2:
/// | i/2^11 + h + l - frac(x/(2pi)) | < 2^-104.116 * |i/2^11 + h + l|
/// with |h| < 2^-11 and |l| < 2^-52.36.
/// Otherwise only the absolute error is bounded:
/// | i/2^11 + h + l - frac(x/(2pi)) | < 2^-75.998 with 0 <= h < 2^-11 and |l| < 2^-53.
/// In both cases we have |l| < 2^-51.64*|i/2^11 + h|.
#[inline]
fn reduce_fast(x: f64) -> (i32, f64, f64, f64) {
    let (mut h, l, err1);
    // 0x1.921fb54442d17p+2
    if x <= fb(0x401921fb54442d17) {
        // x < 2*pi
        // | CH+CL - 1/(2pi) | < 2^-110.523
        // CH = 0x1.45f306dc9c883p-3, CL = -0x1.6b01ec5417056p-57
        const CH: f64 = fb(0x3fc45f306dc9c883);
        const CL: f64 = fb(0xbc66b01ec5417056);
        let l0;
        (h, l0) = a_mul(CH, x); // exact
        l = CL.mul_add(x, l0);
        // see cos.c for the error analysis: |h + l - x/(2pi)| < 2^-104.116 * h
        // 0x1.d9p-105
        err1 = fb(0x396d900000000000) * h; // error < 2^-104.116 * h
    } else {
        // x > 0x1.921fb54442d17p+2
        let tu = x.to_bits();
        let mut e = ((tu >> 52) & 0x7ff) as i32; // 1025 <= e <= 2046
        // Let i be the smallest integer such that 2^(e-1075)/2^(64*(i+1))
        // is not an integer, i.e., e - 1139 - 64i < 0, i.e., i >= (e-1138)/64.
        let m: u64 = (1u64 << 52) | (tu & 0x000f_ffff_ffff_ffff);
        let mut c = [0u64; 3];
        let mut u: u128;
        // x = m/2^53 * 2^(e-1022)
        if e <= 1074 {
            // 1025 <= e <= 1074: 2^2 <= x < 2^52
            // In that case the contribution of x*T[2]/2^192 is less than
            // 2^(52+64-192) <= 2^-76.
            u = u128::from(m) * u128::from(T[1]);
            c[0] = u as u64;
            c[1] = (u >> 64) as u64;
            u = u128::from(m) * u128::from(T[0]);
            c[1] = c[1].wrapping_add(u as u64);
            c[2] = ((u >> 64) as u64).wrapping_add(u64::from(c[1] < u as u64));
            // The low 1075-e bits of c[2] contribute to frac(x/(2pi)).
            e = 1075 - e; // 1 <= e <= 50
        // e is the number of low bits of C[2] contributing to frac(x/(2pi))
        } else {
            // 1075 <= e <= 2046, 2^52 <= x < 2^1024
            let i = ((e - 1138 + 63) / 64) as usize; // i = ceil((e-1138)/64), 0 <= i <= 15
            // m*T[i] contributes to f = 1139 + 64*i - e bits to frac(x/(2pi))
            // with 1 <= f <= 64; m*T[i+3] contributes at most to 2^-76
            u = u128::from(m) * u128::from(T[i + 2]);
            c[0] = u as u64;
            c[1] = (u >> 64) as u64;
            u = u128::from(m) * u128::from(T[i + 1]);
            c[1] = c[1].wrapping_add(u as u64);
            c[2] = ((u >> 64) as u64).wrapping_add(u64::from(c[1] < u as u64));
            u = u128::from(m) * u128::from(T[i]);
            c[2] = c[2].wrapping_add(u as u64);
            e = 1139 + ((i as i32) << 6) - e; // 1 <= e <= 64
            // e is the number of low bits of C[2] contributing to frac(x/(2pi))
        }
        if e == 64 {
            c[0] = c[1];
            c[1] = c[2];
        } else {
            c[0] = (c[1] << (64 - e)) | (c[0] >> e);
            c[1] = (c[2] << (64 - e)) | (c[1] >> e);
        }
        // | c[1]/2^64 + c[0]/2^128 - frac(x/(2pi)) | < 2^-76+2^-128 < 2^-75.999
        (h, l) = set_dd(c[1], c[0]);
        // set_dd() ensures |h| < 1 and |l| < ulp(h) <= 2^-53
        // 0x1.01p-76
        err1 = fb(0x3b30100000000000);
    }

    // 0x1p11, -0x1p-11
    let i = (h * fb(0x40a0000000000000)).floor();
    h = i.mul_add(fb(0xbf40000000000000), h);
    (i as i32, h, l, err1)
}

/// Assume x is a regular number and x > 0x1.6a09e667f3bccp-27; return (h, l, err)
/// with a bound err on the maximal absolute error | h + l - cos(x) | (`cos_fast`).
#[inline]
fn cos_fast(x: f64) -> (f64, f64, f64) {
    let mut neg: i32 = 0;
    let mut is_cos: i32 = 1;

    let (mut i, mut h, mut l, err1) = reduce_fast(x);
    // err1 is an absolute bound for | i/2^11 + h + l - frac(x/(2pi)) |

    // if i >= 2^10: 1/2 <= frac(x/(2pi)) < 1 thus pi <= x <= 2pi
    // we use cos(pi+x) = -cos(x)
    neg ^= i >> 10;
    i &= 0x3ff;
    // | i/2^11 + h + l - frac(x/(2pi)) | mod 1/2 < err1

    // now i < 2^10
    // if i >= 2^9: 1/4 <= frac(x/(2pi)) < 1/2 thus pi/2 <= x <= pi
    // we use cos(pi/2+x) = -sin(x)
    is_cos ^= i >> 9;
    neg ^= i >> 9;
    i &= 0x1ff;
    // | i/2^11 + h + l - frac(x/(2pi)) | mod 1/4 < err1

    // now 0 <= i < 2^9
    // if i >= 2^8: 1/8 <= frac(x/(2pi)) < 1/4
    // we use cos(pi/2-x) = sin(x)
    if (i & 0x100) != 0 {
        // case pi/4 <= x_red <= pi/2
        is_cos = i32::from(is_cos == 0);
        i = 0x1ff - i;
        // 0x1p-11 - h is exact below (see cos.c)
        h = fb(0x3f40000000000000) - h;
        l = -l;
    }

    // Now 0 <= i < 256 and 0 <= h+l < 2^-11
    // with | i/2^11 + h + l - frac(x/(2pi)) | cmod 1/4 < err1
    // If is_cos=1, cos(x) = cos2pi(R + err1);
    // if is_cos=0, cos(x) = sin2pi (R + err1).
    // In both cases R = i/2^11 + h + l, 0 <= R < 1/4.
    let iu = i as usize;
    // since the SC[] table evaluates at i/2^11 + SC[i][0] and not at i/2^11,
    // we must subtract SC[i][0] from h+l; h-SC[i][0] is exact (see cos.c)
    h -= SC[iu][0];
    // now -2^-24 < h < 2^-11+2^-24
    // from reduce_fast() we have |l| < 2^-52.36
    let (uh, mut ul) = a_mul(h, h);
    ul = (h + h).mul_add(l, ul);
    // uh+ul approximates (h+l)^2
    let (mut sh, mut sl) = eval_ps_fast(h, l, uh, ul);
    // | sh + sh - sin(h+l) | < 2^-77.09
    let (mut ch, mut cl) = eval_pc_fast(uh, ul);
    // | ch + cl - cos(h+l) | < 2^-69.96 * |ch + cl|
    let err: f64 = if is_cos == 0 {
        (sh, sl) = s_mul(SC[iu][2], sh, sl);
        (ch, cl) = s_mul(SC[iu][1], ch, cl);
        (h, l) = fast_two_sum(ch, sh);
        l += sl + cl;
        // | h + l - cos(x) | < 2^-68.588 + err1
        fb(0x3ba5500000000000) // 2^-66.588 < 0x1.55p-69
    } else {
        (ch, cl) = s_mul(SC[iu][2], ch, cl);
        (sh, sl) = s_mul(SC[iu][1], sh, sl);
        (h, l) = fast_two_sum(ch, -sh);
        l += cl - sl;
        // | h + l - cos(x) | < 2^-68.414 * |h + l| + err1
        fb(0x3ba8100000000000) // 2^-68.414 < 0x1.81p-69
    };
    const SGN: [f64; 2] = [1.0, -1.0];
    h *= SGN[neg as usize];
    l *= SGN[neg as usize];
    (h, l, err + err1)
}

/// Exceptions of the accurate path: (|x|, h, l) with cos(x) = h + l rounded.
#[rustfmt::skip]
static EXCEPTIONS: [[f64; 3]; 5] = [
    [fb(0x3e88000000000009), fb(0x3fefffffffffff70), fb(0x370b56666666666c)],
    // 0x1.8000000000009p-23, 0x1.fffffffffff70p-1, 0x1.b56666666666cp-143
    [fb(0x3e98000000000024), fb(0x3feffffffffffdc0), fb(0x376b56666666667e)],
    // 0x1.8000000000024p-22, 0x1.ffffffffffdc0p-1, 0x1.b56666666667ep-137
    [fb(0x3ea8000000000090), fb(0x3feffffffffff700), fb(0x37cb5666666666c4)],
    // 0x1.8000000000090p-21, 0x1.ffffffffff700p-1, 0x1.b5666666666c4p-131
    [fb(0x3eb20000000000f3), fb(0x3fefffffffffebc0), fb(0x38037642666666fd)],
    // 0x1.20000000000f3p-20, 0x1.fffffffffebc0p-1, 0x1.37642666666fdp-127
    [fb(0x3eb8000000000240), fb(0x3fefffffffffdc00), fb(0x382b5666666667dd)],
    // 0x1.8000000000240p-20, 0x1.fffffffffdc00p-1, 0x1.b5666666667ddp-125
];

/// Accurate path; assume x is a regular number and x > 0x1.6a09e667f3bccp-27
/// (`cos_accurate`).
#[cold]
#[inline(never)]
fn cos_accurate(x: f64) -> f64 {
    let mut xd = dint_fromd(x);

    // reduce argument
    reduce(&mut xd);

    // now |X - x/(2pi) mod 1| < 2^-126.67*X, with 0 <= X < 1.

    let mut neg = 0;
    let mut is_cos = 1;

    // Write X = i/2^11 + r with 0 <= r < 2^11.
    let mut i = reduce2(&mut xd); // exact

    if (i & 0x400) != 0 {
        // pi <= x < 2*pi: cos(x) = -cos(x-pi)
        neg = 1;
        i &= 0x3ff;
    }

    // now i < 2^10

    if (i & 0x200) != 0 {
        // pi/2 <= x < pi: cos(x) = -sin(x-pi/2)
        neg = i32::from(neg == 0);
        is_cos = 0;
        i &= 0x1ff;
    }

    // now 0 <= i < 2^9

    if (i & 0x100) != 0 {
        // pi/4 <= x < pi/2: cos(x) = sin(pi/2-x), sin(x) = cos(pi/2-x)
        is_cos = i32::from(is_cos == 0);
        xd.sgn = 1; // negate X
        xd = add_dint(&MAGIC, &xd); // X -> 2^-11 - X
        // here: 256 <= i <= 511
        i = 0x1ff - i;
        // now 0 <= i < 256
    }

    // now 0 <= i < 256 and 0 <= X < 2^-11

    // If is_cos=1, cos |x| = cos2pi (R * (1 + eps))
    //    (cases 0 <= x < pi/4 and 3pi/4 <= x < pi)
    // if is_cos=0, cos |x| = sin2pi (R * (1 + eps))
    //    (case pi/4 <= x < 3pi/4)
    // In both cases R = i/2^11 + X, 0 <= R < 1/4, and |eps| < 2^-126.67.

    let iu = i as usize;
    let x2 = mul_dint(&xd, &xd); // X2 approximates X^2
    let mut uu = eval_pc(&x2); // cos2pi(X)
    // since 0 <= X < 2^-11, we have 0.999 < U <= 1
    let mut vv = eval_ps(&xd, &x2); // sin2pi(X)
    // since 0 <= X < 2^-11, we have 0 <= V < 0.0005
    if is_cos == 0 {
        // sin2pi(R) ~ sin2pi(i/2^11)*cos2pi(X)+cos2pi(i/2^11)*sin2pi(X)
        uu = mul_dint(&S[iu], &uu);
        // since 0 <= S[i] < 0.705 and 0.999 < Uin <= 1, we have 0 <= U < 0.705
        vv = mul_dint(&C[iu], &vv);
        // | cos(x) - U | < |U| * 2^-122.650 (see cos.c)
    } else {
        // cos2pi(R) ~ cos2pi(i/2^11)*cos2pi(X)-sin2pi(i/2^11)*sin2pi(X)
        uu = mul_dint(&C[iu], &uu);
        vv = mul_dint(&S[iu], &vv);
        vv.sgn = 1 - vv.sgn; // negate V
        // | cos(x) - U | < |U| * 2^-123.367 (see cos.c)
    }
    uu = add_dint(&uu, &vv);
    // In all cases the total error is bounded by |U| * 2^-122.650, which contributes
    // to at most 2^(128-122.650) < 41 ulps relatively to U->lo.
    let err: u64 = 41;
    let lo0 = uu.lo.wrapping_sub(err);
    let hi0 = uu.hi.wrapping_sub(u64::from(lo0 > uu.lo));
    let lo1 = uu.lo.wrapping_add(err);
    let hi1 = uu.hi.wrapping_add(u64::from(lo1 < uu.lo));
    // check the upper 54 bits are equal
    if (hi0 >> 10) != (hi1 >> 10) {
        for ex in &EXCEPTIONS {
            if x.abs() == ex[0] {
                return ex[1] + ex[2];
            }
        }
        // if we go here, we have a hard-to-round case, but since all hard-to-round
        // cases are known and pass all tests, we are ok
    }

    if neg != 0 {
        uu.sgn = 1 - uu.sgn;
    }

    dint_tod(&mut uu)
}

/// Correctly rounded cosine: the binary64 value nearest to the exact cos(x) (ties to
/// even), hence bit-identical on every platform. Port of CORE-MATH `cr_cos`.
///
/// `cos(±0) = 1`; `cos(±inf)` returns the default quiet NaN and `cos(NaN)` returns a
/// quiet NaN (`x + x`), as in CORE-MATH.
#[inline]
pub fn cos(x: f64) -> f64 {
    let mut tu = x.to_bits();
    let e = ((tu >> 52) & 0x7ff) as i32;

    if e == 0x7ff {
        // NaN, +Inf and -Inf.
        if (tu << 1) == (0x7ffu64 << 53) {
            // Inf: 0.0 / 0.0 in cos.c (the invalid exception is not modelled)
            return f64::NAN;
        }
        return x + x; // return qNaN
    }

    // now x is a regular number

    // For |x| <= 0x1.6a09e667f3bccp-27, cos(x) rounds to 1 (to nearest), since
    // |cos(x) - 1| < x^2/2 < ulp(cos(x))/2 = 2^-54 (see cos.c).
    tu &= 0x7fff_ffff_ffff_ffff;
    let ax = f64::from_bits(tu);
    if tu <= 0x3e46_a09e_667f_3bcc {
        // |x| <= 0x1.6a09e667f3bccp-27
        // -0x1p-28
        return ax.mul_add(fb(0xbe30000000000000), 1.0);
    }

    let (h, l, err) = cos_fast(ax);
    let left = h + (l - err);
    let right = h + (l + err);
    // With SC[] from ./buildSC 15 we get 1100 failures out of 50000000
    // random tests, i.e., about 0.002%.
    if left == right {
        return left;
    }

    cos_accurate(ax)
}

#[cfg(test)]
mod tests {
    use super::cos;

    #[test]
    fn zeros_give_one() {
        assert_eq!(cos(0.0).to_bits(), 1.0f64.to_bits());
        assert_eq!(cos(-0.0).to_bits(), 1.0f64.to_bits());
    }

    #[test]
    fn non_finite_inputs_give_nan() {
        assert!(cos(f64::INFINITY).is_nan());
        assert!(cos(f64::NEG_INFINITY).is_nan());
        assert!(cos(f64::NAN).is_nan());
        assert!(cos(-f64::NAN).is_nan());
    }

    #[test]
    fn tiny_and_subnormal_inputs() {
        // cos(x) rounds to 1 for |x| <= 0x1.6a09e667f3bccp-27
        for x in [f64::from_bits(1), -f64::MIN_POSITIVE, 1e-300, -1e-9] {
            assert_eq!(cos(x).to_bits(), 1.0f64.to_bits(), "{x:e}");
        }
        // first input above the threshold rounds to 1 - 2^-53
        let x = f64::from_bits(0x3e46_a09e_667f_3bcd);
        assert_eq!(cos(x).to_bits(), 0x3fef_ffff_ffff_ffff);
    }

    /// Correctly rounded values (mpmath at 2000 bits, rounded to nearest-even).
    #[test]
    fn known_values() {
        let cases: &[(u64, u64)] = &[
            (0x3ff921fb54442d18, 0x3c91a62633145c07), // cos(0x1.921fb54442d18p+0) = 0x1.1a62633145c07p-54
            (0x400921fb54442d18, 0xbff0000000000000), // cos(0x1.921fb54442d18p+1) = -0x1.0000000000000p+0
            (0x401921fb54442d18, 0x3ff0000000000000), // cos(0x1.921fb54442d18p+2) = 0x1.0000000000000p+0
            (0x3ff0000000000000, 0x3fe14a280fb5068c), // cos(0x1.0000000000000p+0) = 0x1.14a280fb5068cp-1
            (0x3fe0000000000000, 0x3fec1528065b7d50), // cos(0x1.0000000000000p-1) = 0x1.c1528065b7d50p-1
            (0x3e50000000000000, 0x3fefffffffffffff), // cos(0x1.0000000000000p-26) = 0x1.fffffffffffffp-1
            (0x3e57137449123ef6, 0x3feffffffffffffe), // cos(0x1.7137449123ef6p-26) = 0x1.ffffffffffffep-1
            (0x3e46a09e667f3bcd, 0x3fefffffffffffff), // cos(0x1.6a09e667f3bcdp-27) = 0x1.fffffffffffffp-1
            (0x3ef0000000000000, 0x3feffffffff00000), // cos(0x1.0000000000000p-16) = 0x1.ffffffff00000p-1
            (0x41dfffffffffffff, 0x3fce70c0e3c38c22), // cos(0x1.fffffffffffffp+30) = 0x1.e70c0e3c38c22p-3
            (0x41e0000000000000, 0x3fce70c2d5131e55), // cos(0x1.0000000000000p+31) = 0x1.e70c2d5131e55p-3
            (0x4480f0cf064dd592, 0x3fe0be2cef01c8f4), // cos(0x1.0f0cf064dd592p+73) = 0x1.0be2cef01c8f4p-1
            (0x7fe0000000000000, 0xbfea719f26c232bf), // cos(0x1.0000000000000p+1023) = -0x1.a719f26c232bfp-1
            (0x7fefffffffffffff, 0xbfefffe62ecfab75), // cos(0x1.fffffffffffffp+1023) = -0x1.fffe62ecfab75p-1
            (0x400005023d32fee5, 0xbfdac6909aedad7b), // cos(0x1.005023d32fee5p+1) = -0x1.ac6909aedad7bp-2
            (0x7fe61a3db8c8d129, 0x3ff0000000000000), // cos(0x1.61a3db8c8d129p+1023) = 0x1.0000000000000p+0
            (0x7526ac5b262ca1ff, 0x3ff0000000000000), // cos(0x1.6ac5b262ca1ffp+851) = 0x1.0000000000000p+0
            (0x3f30009effd4beda, 0x3fefffffeffec1fc), // cos(0x1.0009effd4bedap-12) = 0x1.fffffeffec1fcp-1
            (0x3ff00147eec5cfa5, 0x3fe148001d3ec044), // cos(0x1.00147eec5cfa5p+0) = 0x1.148001d3ec044p-1
            (0x3e88000000000009, 0x3fefffffffffff70), // cos(0x1.8000000000009p-23) = 0x1.fffffffffff70p-1
            (0x4022400000000000, 0xbfee92a7637106ba), // cos(0x1.2400000000000p+3) = -0x1.e92a7637106bap-1
            (0x408f400000000000, 0x3fe1ff026793f1bb), // cos(0x1.f400000000000p+9) = 0x1.1ff026793f1bbp-1
        ];
        for &(x, y) in cases {
            let got = cos(f64::from_bits(x));
            assert_eq!(
                got.to_bits(),
                y,
                "cos({:e}) = {:e}, expected {:e}",
                f64::from_bits(x),
                got,
                f64::from_bits(y)
            );
            // even symmetry
            assert_eq!(cos(-f64::from_bits(x)).to_bits(), y);
        }
    }
}

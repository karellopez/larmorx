// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded two-argument arctangent for binary64 (`atan2`).
//!
//! Port of CORE-MATH `src/binary64/atan2/atan2.c` and its 192-bit arithmetic `tint.h` at
//! commit 040ee482a8ca (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only: the floating-point environment and errno code of the C files is
//! dropped (it sets flags only). Algorithms, tables and the order of every floating-point
//! operation are kept exactly; `__builtin_fma` is [`f64::mul_add`], C's unsigned arithmetic
//! wraps, and hex-float constants are written as their exact binary64 bit patterns.
//!
//! Original licence of atan2.c and tint.h (MIT):
//!
//! Copyright (c) 2024-2025 Paul Zimmermann and Alexei Sibidanov (atan2.c);
//! Copyright (c) 2023-2024 Paul Zimmermann and Alexei Sibidanov (tint.h).
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

#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

/// `f64` from its IEEE-754 bit pattern (constants are stored as exact bit patterns).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

// ------------------------------------------------------------------------------------------
// tint.h: 192-bit floating-point numbers h·2^64·2^64 + m·2^64 + l, with exponent and sign.

/// `tint_t`: the significand `h:m:l` (normalised: the top bit of `h` set, unless zero), the
/// exponent `ex` and the sign `sgn` (0 or 1). `_h` of the C union is `h:m` as one `u128`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tint {
    h: u64,
    m: u64,
    l: u64,
    ex: i64,
    sgn: u64,
}

impl Tint {
    const fn new(h: u64, m: u64, l: u64, ex: i64, sgn: u64) -> Self {
        Tint { h, m, l, ex, sgn }
    }
    #[inline(always)]
    fn hi(&self) -> u128 {
        (u128::from(self.h) << 64) | u128::from(self.m)
    }
    #[inline(always)]
    fn set_hi(&mut self, v: u128) {
        self.h = (v >> 64) as u64;
        self.m = v as u64;
    }
}

const ZERO: Tint = Tint::new(0, 0, 0, -1076, 0);
const ONE: Tint = Tint::new(0x8000000000000000, 0, 0, 1, 0);
const PI: Tint = Tint::new(
    0xc90fdaa22168c234,
    0xc4c6628b80dc1cd1,
    0x29024e088a67cc74,
    2,
    0,
);
const PI2: Tint = Tint::new(
    0xc90fdaa22168c234,
    0xc4c6628b80dc1cd1,
    0x29024e088a67cc74,
    1,
    0,
);

/// `mul_tint`.
#[inline(always)]
fn mul_tint(a: &Tint, b: &Tint) -> Tint {
    let mut r = Tint::new(0, 0, 0, a.ex + b.ex, a.sgn ^ b.sgn);
    let (ah, am, al) = (u128::from(a.h), u128::from(a.m), u128::from(a.l));
    let (bh, bm, bl) = (u128::from(b.h), u128::from(b.m), u128::from(b.l));
    let rh = ah * bh;
    let rm1 = ah * bm;
    let rm2 = am * bh;
    let mut rl1 = ah * bl;
    let rl2 = am * bm;
    let rl3 = al * bh;
    r.h = (rh >> 64) as u64;
    r.m = rh as u64;
    r.l = rm1 as u64;
    let mut h = (rm1 >> 64) as u64;
    r.m = r.m.wrapping_add(h);
    r.h = r.h.wrapping_add(u64::from(r.m < h));
    let mut l = rm2 as u64;
    h = (rm2 >> 64) as u64;
    r.l = r.l.wrapping_add(l);
    let mut cm = u64::from(r.l < l);
    r.m = r.m.wrapping_add(h);
    r.h = r.h.wrapping_add(u64::from(r.m < h));
    rl1 = (rl1 >> 64) + (rl2 >> 64) + (rl3 >> 64);
    l = rl1 as u64;
    cm = cm.wrapping_add((rl1 >> 64) as u64);
    r.l = r.l.wrapping_add(l);
    cm = cm.wrapping_add(u64::from(r.l < l));
    r.m = r.m.wrapping_add(cm);
    r.h = r.h.wrapping_add(u64::from(r.m < cm));
    if (r.h >> 63) == 0 {
        r.h = (r.h << 1) | (r.m >> 63);
        r.m = (r.m << 1) | (r.l >> 63);
        r.l <<= 1;
        r.ex -= 1;
    }
    r
}

#[inline(always)]
fn tint_zero_p(a: &Tint) -> bool {
    a.h == 0
}

/// `cmp_tint_abs`.
#[inline(always)]
fn cmp_tint_abs(a: &Tint, b: &Tint) -> i32 {
    if tint_zero_p(a) {
        return if tint_zero_p(b) { 0 } else { -1 };
    }
    if tint_zero_p(b) {
        return 1;
    }
    let c = a.ex.cmp(&b.ex) as i32;
    if c != 0 {
        return c;
    }
    let c = a.hi().cmp(&b.hi()) as i32;
    if c != 0 {
        return c;
    }
    a.l.cmp(&b.l) as i32
}

/// `rshift(a, b, k)`: the significand shifted right by `k` bits (exponent and sign
/// unchanged from `a`'s previous value, as in C).
#[inline(always)]
fn rshift(b: &Tint, k: i64) -> (u128, u64) {
    let (bh, bl) = (b.hi(), b.l);
    if k == 0 {
        (bh, bl)
    } else if k < 64 {
        (bh >> k, ((bh << (64 - k)) as u64) | (bl >> k))
    } else if k == 64 {
        (bh >> k, bh as u64)
    } else if k < 128 {
        (bh >> k, (bh >> (k - 64)) as u64)
    } else if k < 192 {
        (0, (bh >> (k - 64)) as u64)
    } else {
        (0, 0)
    }
}

/// `lshift(a, b, k)`.
#[inline(always)]
fn lshift(bh: u128, bl: u64, k: i64) -> (u128, u64) {
    if k == 0 {
        (bh, bl)
    } else if k < 64 {
        ((bh << k) | u128::from(bl >> (64 - k)), bl << k)
    } else if k == 64 {
        ((bh << k) | u128::from(bl), 0)
    } else if k < 128 {
        ((bh << k) | (u128::from(bl) << (k - 64)), 0)
    } else if k < 192 {
        (u128::from(bl) << (k - 64), 0)
    } else {
        (0, 0)
    }
}

/// The number of leading zeros of the 192-bit `h:l`, as `add_tint` counts them.
#[inline(always)]
fn clz192(h: u128, l: u64) -> i64 {
    let th = (h >> 64) as u64;
    if th != 0 {
        i64::from(th.leading_zeros())
    } else if h != 0 {
        64 + i64::from((h as u64).leading_zeros())
    } else {
        128 + i64::from(l.leading_zeros())
    }
}

/// `add_tint`.
#[inline(always)]
fn add_tint(a: &Tint, b: &Tint) -> Tint {
    let (mut a, mut b) = (a, b);
    match cmp_tint_abs(a, b) {
        0 => {
            if (a.sgn ^ b.sgn) != 0 {
                return ZERO;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => std::mem::swap(&mut a, &mut b),
        _ => {}
    }
    let mut r = Tint::new(0, 0, 0, 0, 0);
    let sh = a.ex.wrapping_sub(b.ex);
    let (mut th_, mut tl) = rshift(b, sh);
    if (a.sgn ^ b.sgn) != 0 {
        // a subtraction
        let (ah, al) = (a.hi(), a.l);
        tl = al.wrapping_sub(tl);
        th_ = ah.wrapping_sub(th_).wrapping_sub(u128::from(tl > al));
        let ex = clz192(th_, tl);
        if ex <= 1 || sh == 0 {
            let (h, l) = lshift(th_, tl, ex);
            r.set_hi(h);
            r.l = l;
            r.ex = a.ex - ex;
        } else {
            let (bh2, bl2) = lshift(b.hi(), b.l, ex - sh);
            let (rh, rl) = lshift(ah, al, ex);
            let tl2 = rl.wrapping_sub(bl2);
            let th2 = rh.wrapping_sub(bh2).wrapping_sub(u128::from(tl2 > rl));
            let ex1 = clz192(th2, tl2);
            let (h, l) = lshift(th2, tl2, ex1);
            r.set_hi(h);
            r.l = l;
            r.ex = a.ex - (ex + ex1);
        }
    } else {
        // an addition
        let ah = a.hi();
        let al = a.l;
        let mut rl = al.wrapping_add(tl);
        let cl = u64::from(rl < al);
        let mut rh = ah.wrapping_add(th_);
        let mut ch = u64::from(rh < ah);
        rh = rh.wrapping_add(u128::from(cl));
        ch += u64::from(rh < u128::from(cl));
        if ch != 0 {
            r.ex = a.ex + 1;
            rl = ((rh << 63) as u64) | (rl >> 1);
            rh = (u128::from(ch) << 127) | (rh >> 1);
        } else {
            r.ex = a.ex;
        }
        r.set_hi(rh);
        r.l = rl;
    }
    r.sgn = a.sgn;
    r
}

/// `tint_fromd`.
#[inline(always)]
fn tint_fromd(x: f64) -> Tint {
    let u = x.to_bits();
    let sgn = u >> 63;
    let ax = u & 0x7fffffffffffffff;
    let e = (ax >> 52) as i64;
    if e != 0 {
        Tint::new((1u64 << 63) | (ax << 11), 0, 0, e - 0x3fe, sgn)
    } else {
        let e = i64::from(ax.leading_zeros());
        Tint::new(ax << e, 0, 0, -0x3f2 - e, sgn)
    }
}

/// `as_ldexp(x, i)`: the exponent field moved by `i`.
#[inline(always)]
fn as_ldexp(x: f64, i: i64) -> f64 {
    f64::from_bits(x.to_bits().wrapping_add((i as u64) << 52))
}

/// `tint_tod`.
#[inline(always)]
fn tint_tod(a: &Tint, err: u64, y: f64, x: f64) -> f64 {
    if a.ex >= 1025 {
        return if a.sgn != 0 {
            -fb(0x7fe0000000000000) - fb(0x7fe0000000000000)
        } else {
            fb(0x7fe0000000000000) + fb(0x7fe0000000000000)
        };
    }
    if a.ex <= -1074 {
        let tiny = if a.sgn != 0 {
            -fb(0x0000000000000001)
        } else {
            fb(0x0000000000000001)
        };
        if a.ex < -1074 {
            return tiny * 0.5;
        }
        let mid = a.h == (1u64 << 63) && a.m == 0 && a.l == 0;
        return tiny * if mid { 0.5 } else { 0.75 };
    }
    let (mut hh, mut mm, mut ll) = (a.h, a.m, a.l);
    let mut ex = a.ex;
    let mut low = hh & 0x7ff;
    if (mm == 0 || !mm == 0)
        && ((mm == 0 && (low == 0 || low == 0x400) && ll < err)
            || (!mm == 0 && (low == 0x3ff || low == 0x7ff) && !ll < err))
    {
        panic!("CORE-MATH atan2: unexpected worst case y,x = {y:e},{x:e}");
    }
    if ex <= -1022 {
        let sh = -1021 - ex;
        ll = (mm << (64 - sh)) | (ll >> sh) | u64::from(ll > 0);
        mm = (hh << (64 - sh)) | (mm >> sh);
        hh >>= sh;
        low = hh & 0x7ff;
        ex += sh;
    }
    let mut h = (hh >> 11) as f64;
    let l: f64 = if err == 0 {
        0.0
    } else if low < 0x400 {
        0.25
    } else if low > 0x400 {
        0.75
    } else if mm == 0 && ll == 0 {
        0.5
    } else {
        0.75
    };
    let s = if a.sgn != 0 { -1.0 } else { 1.0 };
    h = l.mul_add(s, s * h);
    h *= fb(0x3cb0000000000000);
    h * as_ldexp(1.0, ex - 1)
}

/// `inv_tint`: an approximation of `1/A`.
#[inline(always)]
fn inv_tint(a_in: &Tint) -> Tint {
    let mut a = tint_tod(a_in, 0, 0.0, 0.0);
    let subnormal = a.abs() < fb(0x0010000000000000);
    if subnormal {
        a *= fb(0x4340000000000000);
    }
    let mut r = tint_fromd(1.0 / a);
    if subnormal {
        r.ex += 53;
    }
    let mut q = mul_tint(a_in, &r);
    q.sgn = 1 - q.sgn;
    q = add_tint(&ONE, &q);
    q = mul_tint(&r, &q);
    add_tint(&r, &q)
}

/// `div_tint`: an approximation of `b/a`.
#[inline(always)]
fn div_tint(b: &Tint, a: &Tint) -> Tint {
    let y = inv_tint(a);
    let r = mul_tint(&y, b);
    let mut z = mul_tint(a, &r);
    z.sgn = 1 - z.sgn;
    z = add_tint(b, &z);
    z = mul_tint(&y, &z);
    add_tint(&r, &z)
}

/// `div_tint_d`.
#[inline(always)]
fn div_tint_d(b: f64, a: f64) -> Tint {
    div_tint(&tint_fromd(b), &tint_fromd(a))
}

// ------------------------------------------------------------------------------------------
// atan2.c

const PI_H: f64 = fb(0x400921fb54442d18);
const PI_L: f64 = fb(0x3ca1a62633145c07);
const PI_OVER2_H: f64 = fb(0x3ff921fb54442d18);
const PI_OVER2_L: f64 = fb(0x3c91a62633145c07);

/// `atan2_accurate`: the type [29,29] rational approximation of `atan(z)` in 192-bit
/// arithmetic.
#[inline(never)]
fn atan2_accurate(y: f64, x: f64) -> f64 {
    let t = y / x;
    if t == 0.0 {
        if x > 0.0 {
            return t;
        }
        return if y > 0.0 { PI_H + PI_L } else { -PI_H - PI_L };
    }
    let mut corr = t.mul_add(x, -y);
    if corr == 0.0 && x > 0.0 && t.abs() <= fb(0x3e4d12ed0af1a27f) {
        if y.abs() >= fb(0x06c0000000000000) {
            return t.mul_add(-fb(0x3c90000000000000), t);
        }
        corr = (t * fb(0x4680000000000000)).mul_add(x, -y * fb(0x4680000000000000));
        if corr == 0.0 {
            return t.mul_add(-fb(0x3c90000000000000), t);
        }
    }
    let inv = y.abs() > x.abs();
    let mut z = if inv {
        div_tint_d(x, y)
    } else {
        div_tint_d(y, x)
    };
    if !inv && x > 0.0 && z.ex <= -96 {
        z.l = z.l.wrapping_sub(2);
        z.m = z.m.wrapping_sub(u64::from(z.l < 2));
        z.h = z.h.wrapping_sub(u64::from(z.m < 1));
        return tint_tod(&z, 1, y, x);
    }
    let sz = z.sgn;
    z.sgn = 0;
    let mut p = P[29];
    let mut q = Q[29];
    for i in (0..=28).rev() {
        p = mul_tint(&p, &z);
        q = mul_tint(&q, &z);
        p = add_tint(&p, &P[i]);
        q = add_tint(&q, &Q[i]);
    }
    p = mul_tint(&p, &z);
    z = div_tint(&p, &q);
    let mut err: u64 = 662;
    z.sgn = sz;
    if inv {
        if z.sgn == 0 {
            z.sgn = 1;
            z = add_tint(&PI2, &z);
        } else {
            z = add_tint(&PI2, &z);
            z.sgn = 1;
        }
        err = 524;
    }
    if x < 0.0 {
        if z.sgn == 0 {
            z.sgn = 1;
            z = add_tint(&PI, &z);
            z.sgn = 1;
        } else {
            z = add_tint(&PI, &z);
        }
        err = 266;
    }
    tint_tod(&z, err, y, x)
}

#[inline(always)]
fn fasttwosum(x: f64, y: f64, e: &mut f64) -> f64 {
    let s = x + y;
    let z = s - x;
    *e = y - z;
    s
}

#[inline(always)]
fn fastsum(xh: f64, xl: f64, yh: f64, yl: f64, e: &mut f64) -> f64 {
    let mut sl = 0.0;
    let sh = fasttwosum(xh, yh, &mut sl);
    *e = (xl + yl) + sl;
    sh
}

/// `as_atan2_special`: NaN, infinite and zero arguments.
#[inline(never)]
fn as_atan2_special(y0: f64, x0: f64) -> f64 {
    let (iy, ix) = (y0.to_bits(), x0.to_bits());
    let (aiy, aix) = (iy << 1, ix << 1);
    if aiy >= 0x7ffu64 << 53 || aix >= 0x7ffu64 << 53 {
        if aiy > 0x7ffu64 << 53 || aix > 0x7ffu64 << 53 {
            return y0 + x0;
        }
        if aiy == 0x7ffu64 << 53 && aix == 0x7ffu64 << 53 {
            let k = (ix >> 63) as usize;
            return FINF[k][1].copysign(y0) + FINF[k][0].copysign(y0);
        }
        if aix == 0x7ffu64 << 53 {
            if x0 < 0.0 {
                return PI_H.copysign(y0) + PI_L.copysign(y0);
            }
            return 0.0f64.copysign(y0);
        }
        return PI_OVER2_H.copysign(y0) + PI_OVER2_L.copysign(y0);
    }
    if aiy == 0 || aix == 0 {
        if aiy == 0 && aix == 0 {
            if ix == 0 {
                return y0;
            }
            return if iy == 0 { PI_H + PI_L } else { -PI_H - PI_L };
        }
        if aiy == 0 {
            if x0 > 0.0 {
                return y0 / x0;
            }
            return if (iy >> 63) == 0 {
                PI_H + PI_L
            } else {
                -PI_H - PI_L
            };
        }
        return if y0 > 0.0 {
            PI_OVER2_H + PI_OVER2_L
        } else {
            -PI_OVER2_H - PI_OVER2_L
        };
    }
    0.0
}

/// `cr_atan2(y0, x0)`.
#[inline(always)]
pub(crate) fn atan2(y0: f64, x0: f64) -> f64 {
    const MASK: u64 = 0x7fffffffffffffff;
    let (iy, ix) = (y0.to_bits(), x0.to_bits());
    let aiy = iy & MASK;
    if aiy == 0 || aiy >= 0x7ffu64 << 52 {
        return as_atan2_special(y0, x0);
    }
    let aix = ix & MASK;
    if aix == 0 || aix >= 0x7ffu64 << 52 {
        return as_atan2_special(y0, x0);
    }
    let ax = x0.abs();
    let ay = y0.abs();
    let mut x = ax.max(ay);
    let mut y = ax.min(ay);
    let (sy, sx) = (iy >> 63, ix >> 63);
    let gt = u64::from(aix < aiy);
    let dxy = aix.wrapping_sub(aiy) ^ gt.wrapping_neg();
    if dxy >= 53u64 << 52 {
        return atan2_accurate(y0, x0);
    }
    let sgn = ASGN[(gt ^ sx ^ sy) as usize];
    let kw = ((sx << 2) | (sy << 1) | gt) as usize;
    let jj = (y / x + (2.0 + 1.0 / 128.0)).to_bits();
    let jt = ((jj >> (52 - 7)) & 127) as usize;
    let one_sgn = 1.0f64.copysign(sgn);
    let mut fh = F2[jt][1] * one_sgn;
    let mut fl = F2[jt][0] * one_sgn;
    fh += O[kw][0];
    fl += O[kw][1];
    if x < fb(0x0670000000000000) {
        x *= fb(0x7970000000000000);
        y *= fb(0x7970000000000000);
    }
    if x > fb(0x7fd0000000000000) && jt != 0 {
        x *= fb(0x3fe0000000000000);
        y *= fb(0x3fe0000000000000);
    }
    let t0 = T2[jt];
    let zn = (-t0).mul_add(x, y);
    let zd = t0.mul_add(y, x);
    let mut z = zn / zd;
    let mut z2 = z * z;
    z *= one_sgn;
    let dz = (z * z2) * (B[0] + z2 * (B[1] + z2 * B[2]));
    let mut eps = z.abs() * fb(0x3cc0510000000000) + fb(0x3a50000000000000);
    let rh = fasttwosum(fh, z, &mut z);
    let rl = (fl + dz) + z;
    let mut lb = rh + (rl - eps);
    let mut ub = rh + (rl + eps);
    if lb != ub {
        let mut dh = y * t0;
        let mut dl = y.mul_add(t0, -dh);
        let mut e = 0.0;
        dh = fasttwosum(x, dh, &mut e);
        let rdh = 1.0 / dh;
        dl += e;
        let mut nh = x * t0;
        let mut nl = x.mul_add(t0, -nh);
        let dt = y - nh;
        let y1 = dt + nh;
        if y1 == y {
            nh = fasttwosum(dt, -nl, &mut nl);
        } else {
            nh = fasttwosum(dt, (y - y1) - nl, &mut nl);
        }
        let mut zh = nh * rdh;
        z2 = zh * zh;
        let mut zl = rdh * (dh.mul_add(-zh, nh) + (nl - (nh * rdh) * dl));
        zl += zh * z2 * ((B2[0] + z2 * B2[1]) + (z2 * z2) * (B2[2] + z2 * B2[3]));
        zh *= one_sgn;
        zl *= one_sgn;
        eps = fb(0x3cd4000000000000) * (zh.abs() * z2 + fb(0x3cc0000000000000));
        fh = fastsum(fh, fl, zh, zl, &mut fl);
        lb = fh + (fl - eps);
        ub = fh + (fl + eps);
        if lb != ub {
            return atan2_accurate(y0, x0);
        }
    }
    ub
}

/// `T2` of atan2.c: j/64.
#[rustfmt::skip]
const T2: [f64; 65] = [ fb(0x0000000000000000), fb(0x3f90000000000000), fb(0x3fa0000000000000), fb(0x3fa8000000000000), fb(0x3fb0000000000000), fb(0x3fb4000000000000), fb(0x3fb8000000000000), fb(0x3fbc000000000000), fb(0x3fc0000000000000), fb(0x3fc2000000000000), fb(0x3fc4000000000000), fb(0x3fc6000000000000), fb(0x3fc8000000000000), fb(0x3fca000000000000), fb(0x3fcc000000000000), fb(0x3fce000000000000), fb(0x3fd0000000000000), fb(0x3fd1000000000000), fb(0x3fd2000000000000), fb(0x3fd3000000000000), fb(0x3fd4000000000000), fb(0x3fd5000000000000), fb(0x3fd6000000000000), fb(0x3fd7000000000000), fb(0x3fd8000000000000), fb(0x3fd9000000000000), fb(0x3fda000000000000), fb(0x3fdb000000000000), fb(0x3fdc000000000000), fb(0x3fdd000000000000), fb(0x3fde000000000000), fb(0x3fdf000000000000), fb(0x3fe0000000000000), fb(0x3fe0800000000000), fb(0x3fe1000000000000), fb(0x3fe1800000000000), fb(0x3fe2000000000000), fb(0x3fe2800000000000), fb(0x3fe3000000000000), fb(0x3fe3800000000000), fb(0x3fe4000000000000), fb(0x3fe4800000000000), fb(0x3fe5000000000000), fb(0x3fe5800000000000), fb(0x3fe6000000000000), fb(0x3fe6800000000000), fb(0x3fe7000000000000), fb(0x3fe7800000000000), fb(0x3fe8000000000000), fb(0x3fe8800000000000), fb(0x3fe9000000000000), fb(0x3fe9800000000000), fb(0x3fea000000000000), fb(0x3fea800000000000), fb(0x3feb000000000000), fb(0x3feb800000000000), fb(0x3fec000000000000), fb(0x3fec800000000000), fb(0x3fed000000000000), fb(0x3fed800000000000), fb(0x3fee000000000000), fb(0x3fee800000000000), fb(0x3fef000000000000), fb(0x3fef800000000000), fb(0x3ff0000000000000)];

/// `f2` of atan2.c: atan(j/64), double-double (small term first).
#[rustfmt::skip]
const F2: [[f64; 2]; 65] = [ [fb(0x0000000000000000), fb(0x0000000000000000)], [fb(0xbca95220c39d4dff), fb(0x3f8fff555bbb7300)], [fb(0x3ca2542779d776de), fb(0x3f9ffd55bba97600)], [fb(0xbca6061bbe3de53c), fb(0x3fa7fb818430da40)], [fb(0xbca639269b0da47e), fb(0x3faff55bb72cfe00)], [fb(0xbc84a7663af440f7), fb(0x3fb3f59f0e7c55a0)], [fb(0x3cad1824d59f9e13), fb(0x3fb7ee182602f100)], [fb(0x3c8bef71e5340b31), fb(0x3fbbe39ebe6f07c0)], [fb(0x3cab8cb225e627d0), fb(0x3fbfd5ba9aac2f60)], [fb(0x3cab92de9bac94c2), fb(0x3fc1e1fafb043720)], [fb(0xbc9d3cb89e62dafd), fb(0x3fc3d6eee8c66270)], [fb(0xbca882a55960087a), fb(0x3fc5c9811e3ec270)], [fb(0x3c91347b0b4f881d), fb(0x3fc7b97b4bce5b00)], [fb(0x3ca873d8079ed0d2), fb(0x3fc9a6a8e96c8620)], [fb(0x3c9022f621a5c1cb), fb(0x3fcb90d7529260a0)], [fb(0x3ca9c648d1534598), fb(0x3fcd77d5df205730)], [fb(0xbc94ea9238610a08), fb(0x3fcf5b75f92c80e0)], [fb(0x3ca2c5c8e721970d), fb(0x3fd09dc597d86360)], [fb(0x3c630ca4748b1bf9), fb(0x3fd18bf5a30bf178)], [fb(0xbca20ef9ba6dbf90), fb(0x3fd278372057ef48)], [fb(0x3cae69c5abb498d2), fb(0x3fd362773707ebc8)], [fb(0x3c9a8a86f0ea9311), fb(0x3fd44aa436c2af08)], [fb(0x3c9db5336feef7f0), fb(0x3fd530ad9951cd48)], [fb(0x3c99636a3aa3b840), fb(0x3fd614840309cfe0)], [fb(0x3c81ce2a8c848b74), fb(0x3fd6f61941e4def0)], [fb(0xbc84b1bbd1ea6db3), fb(0x3fd7d5604b63b3f8)], [fb(0xbca4925e8b916e0b), fb(0x3fd8b24d394a1b28)], [fb(0x3c79e6c988fd0a77), fb(0x3fd98cd5454d6b18)], [fb(0xbcaa49bd836a1700), fb(0x3fda64eec3cc2400)], [fb(0xbcaca3cf09c6b5f8), fb(0x3fdb3a911da65c70)], [fb(0xbc7cc1ce70934c34), fb(0x3fdc0db4c94ec9f0)], [fb(0x3c82e982ddf3872a), fb(0x3fdcde53432c1350)], [fb(0xbc82ea406ee84d0f), fb(0x3fddac670561bb50)], [fb(0xbcade35847c81979), fb(0x3fde77eb7f175a38)], [fb(0xbc6a3992dc382a23), fb(0x3fdf40dd0b541418)], [fb(0xbc8b32c949c9d593), fb(0x3fe0039c73c1a40c)], [fb(0xbc7d5b495f6349e6), fb(0x3fe0657e94db30d0)], [fb(0xbcaf34582f6255fe), fb(0x3fe0c6145b5b43dc)], [fb(0x3c9ed42511e3f11d), fb(0x3fe1255d9bfbd2a8)], [fb(0xbc91cef189ff9e7f), fb(0x3fe1835a88be7c14)], [fb(0xbc5928df287a668f), fb(0x3fe1e00babdefeb4)], [fb(0xbcae3bde360c7ddb), fb(0x3fe23b71e2cc9e6c)], [fb(0x3c9bd86313ce4fde), fb(0x3fe2958e59308e30)], [fb(0xbca8e8a85803cc1d), fb(0x3fe2ee628406cbcc)], [fb(0xbc977ef7641c777f), fb(0x3fe345f01cce37bc)], [fb(0x3cab73ef3389d02f), fb(0x3fe39c391cd41718)], [fb(0x3c7ecf8b492644f0), fb(0x3fe3f13fb89e96f4)], [fb(0x3cac1125fd3810c7), fb(0x3fe445065b795b54)], [fb(0x3ca2483350fe548b), fb(0x3fe4978fa3269ee0)], [fb(0x3c84a33dbeb3796c), fb(0x3fe4e8de5bb6ec04)], [fb(0xbca46edd2af69483), fb(0x3fe538f57b890620)], [fb(0xbca2bcb93b18b52a), fb(0x3fe587d81f732fbc)], [fb(0x3c60028e4bc5e7ca), fb(0x3fe5d58987169b18)], [fb(0x3caed487acaf1174), fb(0x3fe6220d115d7b8c)], [fb(0xbca2dd4dfd7d1777), fb(0x3fe66d663923e088)], [fb(0x3c92bfe3cf3b9d79), fb(0x3fe6b798920b3d98)], [fb(0xbc78c34d25aadef6), fb(0x3fe700a7c5784634)], [fb(0xbc9f426acf4d3bdb), fb(0x3fe748978fba8e10)], [fb(0xbcaafe57dd9ff230), fb(0x3fe78f6bbd5d3160)], [fb(0xbc954fbef0e862ab), fb(0x3fe7d528289fa094)], [fb(0x3c990227758b11ba), fb(0x3fe819d0b7158a4c)], [fb(0x3ca16b66e7fc8b8c), fb(0x3fe85d69576cc2c4)], [fb(0xbc855b9a5e177a1b), fb(0x3fe89ff5ff57f1f8)], [fb(0x3cac27cfaa9f7a14), fb(0x3fe8e17aa99cc05c)], [fb(0x3c81a62633145c07), fb(0x3fe921fb54442d18)]];

/// `O` of atan2.c: the quadrant offsets.
#[rustfmt::skip]
const O: [[f64; 2]; 8] = [ [0.0,0.0], [fb(0x3ff921fb54442d18),fb(0x3c91a62633145c07)], [0.0,0.0], [fb(0xbff921fb54442d18),fb(0xbc91a62633145c07)], [fb(0x400921fb54442d18),fb(0x3ca1a62633145c07)], [fb(0x3ff921fb54442d18),fb(0x3c91a62633145c07)], [fb(0xc00921fb54442d18),fb(0xbca1a62633145c07)], [fb(0xbff921fb54442d18),fb(0xbc91a62633145c07)]];

/// `b` of atan2.c.
#[rustfmt::skip]
const B: [f64; 3] = [fb(0xbfd55555555554d2), fb(0x3fc999999860e1ca), fb(0xbfc248ad469844a1)];

/// `b2` of atan2.c.
#[rustfmt::skip]
const B2: [f64; 4] = [fb(0xbfd5555555555555), fb(0x3fc999999999755e), fb(0xbfc24924883596f8), fb(0x3fbc6f7d73531bc2)];

/// `asgn` of atan2.c.
#[rustfmt::skip]
const ASGN: [f64; 2] = [0.0, -0.0];

/// `finf` of atan2.c (both arguments infinite).
#[rustfmt::skip]
const FINF: [[f64; 2]; 2] = [[fb(0x3c80000000000000), fb(0x3fe921fb54442d18)], [fb(0x3c90000000000000), fb(0x4002d97c7f3321d2)]];

/// `P` of atan2.c: the numerator, z·P(z)/Q(z) ≈ atan(z) on [0, 1].
#[rustfmt::skip]
const P: [Tint; 30] = [
    Tint::new(0x82703f8b53112eca, 0x71ce6279d467aa45, 0x19c9b28466b6fc0, -10, 0),
    Tint::new(0xb3cf74b427d53e03, 0xee590b839e751890, 0xf2570b12bcb2381a, -8, 0),
    Tint::new(0xb08498b17b88c39a, 0x11e37821d784d8fa, 0x2795b14b84b9d78c, -6, 0),
    Tint::new(0xfd4baa7bb52b0d83, 0xb25425beb0a04dc3, 0x92008d9fb34b68ff, -5, 0),
    Tint::new(0x96541948b85fb386, 0xe0b815ace425a859, 0xbeca17c69bb4f7a0, -3, 0),
    Tint::new(0x96aaa5d4baffc590, 0xf23db783b6e4ce55, 0x2c54e054d3b69f7e, -2, 0),
    Tint::new(0x83cd41b5fa862b11, 0xad8f9d6e3eb9d833, 0x4ef470d801fa03ca, -1, 0),
    Tint::new(0xcbbc7752197cc106, 0xbc66cfb3edefa6e3, 0x568b2f9e4feb8910, -1, 0),
    Tint::new(0x8cf8f7c585767042, 0xcd92a5b9ac13d8c8, 0x43bd4a023dbe04d, 0, 0),
    Tint::new(0xafdc36d6b36139bf, 0x11df63474e3859d4, 0x42b30b2c089a2b6c, 0, 0),
    Tint::new(0xc6f8fb1463822519, 0x2f9f04cf55c10377, 0x1c44675c6d483606, 0, 0),
    Tint::new(0xcce79f10f754e315, 0x91ce47edcbc8a4ee, 0xb2a6f721ac656b42, 0, 0),
    Tint::new(0xc090384dbd77132e, 0x1dfb4d9a1d595de7, 0xe506ca899a394f70, 0, 0),
    Tint::new(0xa556ac9eee25e867, 0xd84dafa25141d203, 0xf1c7c2a5156b3ec3, 0, 0),
    Tint::new(0x81bd2677ddfcbc38, 0x1df33f27dbb60939, 0xcb4c2c73286a6c29, 0, 0),
    Tint::new(0xb9ed12169d72e8d9, 0xc81d95dcef13c227, 0x974b95796b93c332, -1, 0),
    Tint::new(0xf2df6da875b666fa, 0xc29dd3a69c274c65, 0x83b9260976c8f0c5, -2, 0),
    Tint::new(0x902d34d4db910ed8, 0x6f3ecddfd6af0a9f, 0x8e950d7e70a8e9b2, -2, 0),
    Tint::new(0x9aefa41961ce1207, 0xacd0317666753751, 0x3eaf42a53fcc6534, -3, 0),
    Tint::new(0x95db9977c5e6e263, 0x85367a63fab8a720, 0xfc2b63bd115998d8, -4, 0),
    Tint::new(0x817cb6d8236d5f4d, 0x70c7815f28909d9b, 0xc54e97155d12158, -5, 0),
    Tint::new(0xc5eec4228c352098, 0x789e56be1dee9b67, 0xba25bda81292ccc1, -7, 0),
    Tint::new(0x840c38ccfcb5dc64, 0xb55a44095bf7f312, 0x6668b8792d4ecd08, -8, 0),
    Tint::new(0x97074af7b286b19e, 0xc20aac21f3130f1d, 0x369e9fdbdd48528a, -10, 0),
    Tint::new(0x9054011c32d4b7a5, 0xc68d3c5d525f4af4, 0xeeb3c98615dee188, -12, 0),
    Tint::new(0xdddb4a2eaab2bb15, 0x753761a8f0e9aa8d, 0x3f20620cd3e075bb, -15, 0),
    Tint::new(0x80ef9a1b3e59abe5, 0x18e6afe0651fdf49, 0xd1c0329e6a702d9d, -17, 0),
    Tint::new(0xca16dd49cd30854c, 0xbad8ca379401027c, 0x1fc7f47985486f16, -21, 0),
    Tint::new(0xa0ba01b8742ba801, 0xe6dc14a9516d7f04, 0x1f5303d878905d2f, -25, 0),
    Tint::new(0x9c0440896ce6515c, 0xcf3470fa7f10ac3, 0xa9dc80682c111a9d, -59, 1),
];

/// `Q` of atan2.c: the denominator.
#[rustfmt::skip]
const Q: [Tint; 30] = [
    Tint::new(0x82703f8b53112eca, 0x71ce6279d467aa45, 0x19c9b28466b6fc0, -10, 0),
    Tint::new(0xb3cf74b427d53e03, 0xee590b839e751890, 0xf2570b12bcb238cf, -8, 0),
    Tint::new(0xb33c44af0d43c9e8, 0xf4426f84614701db, 0x979e49dc5b858442, -6, 0),
    Tint::new(0x8264d256f0bf4d61, 0xee21431cc0f29764, 0xce0cc1b593ac9952, -4, 0),
    Tint::new(0x9d97c4947687b2ee, 0xef4087c49dde5229, 0x742bc6e02bfb6df8, -3, 0),
    Tint::new(0xa0f888dc7aab949e, 0xf623f19ac413db23, 0x854744a164f70ecd, -2, 0),
    Tint::new(0x8fd9c7172ab0828e, 0x1a9f72fadafc3dfb, 0xaf96a180ea297178, -1, 0),
    Tint::new(0xe38168b78a51ee15, 0x604a47815c8091eb, 0x9faaa42312e8eb30, -1, 0),
    Tint::new(0xa164ccdbb65ee133, 0xe623433c5e086402, 0xcb228afd335a245f, 0, 0),
    Tint::new(0xced0c0cbe44fac2f, 0xd7c2adfc7c5dda9e, 0xa22b4a311cf5a5b1, 0, 0),
    Tint::new(0xf0e6660438c90371, 0x3dca1a0f58f0d943, 0x8a92b9de68c4488c, 0, 0),
    Tint::new(0x8000000000000000, 0x0, 0x0, 1, 0),
    Tint::new(0xf8ea68f9ad849103, 0xb257e4bfc606967f, 0x818fa2816928c4aa, 0, 0),
    Tint::new(0xddc64eeee3506234, 0x47a33ea40a2b22e4, 0xa6a2c129ace2470a, 0, 0),
    Tint::new(0xb52ddfba2d37c8e2, 0x24551a9b19fa064a, 0x3d78c68b54bc3850, 0, 0),
    Tint::new(0x87ac113d26b683a1, 0x4e97756c38785b0b, 0xf75f0a4506931ba8, 0, 0),
    Tint::new(0xba065070b900f3d5, 0xe49d8f9f238f1eeb, 0x6805d48d67f1dcd9, -1, 0),
    Tint::new(0xe902d2a4bfc22441, 0x945cfe6c83a5b828, 0x519ebb457f8ccca0, -2, 0),
    Tint::new(0x84e405c915f7769b, 0xfaaf34c3c51dd39, 0xa97fa0ae99382c93, -2, 0),
    Tint::new(0x896da46c7e6cebb3, 0xe0afc0b6db3dbbf5, 0x61ccff6e78efff4, -3, 0),
    Tint::new(0x801a0c1a0dfefc92, 0xf8e0a97902e08d47, 0x81898e0b7d8ef0bc, -4, 0),
    Tint::new(0xd59d088930df07db, 0xa8cb00021849fbaf, 0xf80cc91aa0bddf89, -6, 0),
    Tint::new(0x9db56a6984b77231, 0xcf7c2074e0480959, 0x3316ecaa6aa4604e, -7, 0),
    Tint::new(0xcb7450d9d62d2e68, 0x9787b4265977d004, 0xba615cee00a841d7, -9, 0),
    Tint::new(0xe12a52e02b64b23e, 0x97c9139a8ced716c, 0x70a176ad2b5f16d2, -11, 0),
    Tint::new(0xd057b6d0900ed30d, 0x5e04bb17eb879a5e, 0x23ed135b60bea070, -13, 0),
    Tint::new(0x9b1f1ca22fef7ae5, 0x19a687f0a4073eee, 0x5a9ccbc8c14578b, -15, 0),
    Tint::new(0xaeba710f053aac3b, 0x30ea5d6fb116f775, 0x8a9bcfef5698f8f1, -18, 0),
    Tint::new(0x84b9ada076cd007d, 0x13e55006073f76b1, 0x4cf4259968228f3e, -21, 0),
    Tint::new(0xcca4b9046786937d, 0x233c1e691c127c1f, 0xf493e0b7a3de459a, -26, 0),
];

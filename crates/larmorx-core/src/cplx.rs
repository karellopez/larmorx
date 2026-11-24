// SPDX-License-Identifier: Apache-2.0
//! Double-precision complex arithmetic as x86-64's compiled code computes it, bit for bit (NaNs
//! included: x86-64's SSE rules on every platform, `math::x86_nan`):
//!
//! - [`C64::mul`]: the plain product `(ar br - ai bi, ar bi + ai br)`, as GCC expands a complex
//!   multiplication under Fortran's rules (`-fcx-fortran-rules`, gfortran's default) and as the
//!   first, inline step of C's and C++'s (ISO C99 rules); GCC treats a real number promoted to
//!   complex as a complex number with a zero imaginary part (it honours signed zeros), so every
//!   mixed product is this full product.
//! - [`C64::fdiv`]: GCC's "wide" division under Fortran's rules: Smith's algorithm (Smith,
//!   "Algorithm 116: Complex division", CACM 5(8), 1962) branching on `|br| < |bi|`.
//! - [`cxx_mul`], [`divdc3`]: C99's (and C++'s `std::complex<double>`) product and quotient: the
//!   plain product, recomputed by [`muldc3`] (C11 Annex G.5.1's `_Cmultd`) when both parts are
//!   NaN; the quotient by Smith's algorithm with Annex G's recovery of infinities and zeros
//!   (`__divdc3` of the system's libgcc, measured as a black box:
//!   `validation/investigations/complex-runtime/`).
//! - [`cabs`] and [`csqrt`]: the C library's complex absolute value and square root, with the
//!   correctly rounded `hypot` (`math::hypot`, CORE-MATH's) in place of glibc's (the project's
//!   decision D4: `hypot` correctly rounded in both modes; glibc's `hypot` is not correctly
//!   rounded on about 0.5 % of arguments, GLIBC-07). The square root is Kahan's (Kahan, "Branch
//!   Cuts for Complex Elementary Functions", 1987) in the form glibc 2.35's `csqrt` was measured
//!   to take as a black box (its special values, zero parts and scaling; the investigation's
//!   README, "csqrt's form").

use crate::math;
use crate::math::x86_nan::{add_f64, div_f64, mul_f64, sub_f64};

/// A `double _Complex` / Fortran `COMPLEX*16` / `std::complex<double>`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

/// glibc's NaN for a complex function's invalid result (`0x7ff8000000000000`).
const QNAN: f64 = f64::from_bits(0x7ff8_0000_0000_0000);

// named methods, not the operators: each says which of the runtimes' rules it follows
#[allow(clippy::should_implement_trait)]
impl C64 {
    /// `(0, 0)`.
    pub const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    /// `(1, 0)`.
    pub const ONE: C64 = C64 { re: 1.0, im: 0.0 };

    #[inline(always)]
    pub const fn new(re: f64, im: f64) -> C64 {
        C64 { re, im }
    }

    /// A real number promoted to complex: `(x, +0)`.
    #[inline(always)]
    pub const fn real(x: f64) -> C64 {
        C64 { re: x, im: 0.0 }
    }

    /// `self + b`, part by part.
    #[inline(always)]
    pub fn add(self, b: C64) -> C64 {
        C64::new(add_f64(self.re, b.re), add_f64(self.im, b.im))
    }

    /// `self - b`, part by part.
    #[inline(always)]
    pub fn sub(self, b: C64) -> C64 {
        C64::new(sub_f64(self.re, b.re), sub_f64(self.im, b.im))
    }

    /// `-self` (both signs flipped).
    #[inline(always)]
    pub fn neg(self) -> C64 {
        C64::new(-self.re, -self.im)
    }

    /// The conjugate `(re, -im)`.
    #[inline(always)]
    pub fn conj(self) -> C64 {
        C64::new(self.re, -self.im)
    }

    /// The plain product: `(ar br - ai bi, ar bi + ai br)`.
    #[inline(always)]
    pub fn mul(self, b: C64) -> C64 {
        let (ar, ai, br, bi) = (self.re, self.im, b.re, b.im);
        C64::new(
            sub_f64(mul_f64(ar, br), mul_f64(ai, bi)),
            add_f64(mul_f64(ar, bi), mul_f64(ai, br)),
        )
    }

    /// GCC's wide (Smith's) quotient `self / b` under Fortran's rules: for `|br| < |bi|`, `r =
    /// br / bi`, `d = br r + bi`, `((ar r + ai) / d, (ai r - ar) / d)`; otherwise (a NaN
    /// included) `r = bi / br`, `d = bi r + br`, `((ai r + ar) / d, (ai - ar r) / d)`.
    #[inline]
    pub fn fdiv(self, b: C64) -> C64 {
        smith(self.re, self.im, b.re, b.im)
    }

    /// `self / x` for a real `x` promoted to complex (`x + 0i`: [`C64::fdiv`]).
    #[inline]
    pub fn fdiv_real(self, x: f64) -> C64 {
        smith(self.re, self.im, x, 0.0)
    }

    /// Fortran's `z .EQ. (0, 0)`: both parts equal zero (a NaN part is not).
    #[inline(always)]
    pub fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }

    /// Whether a part is NaN.
    #[inline(always)]
    pub fn has_nan(self) -> bool {
        self.re.is_nan() || self.im.is_nan()
    }
}

#[inline]
fn smith(ar: f64, ai: f64, br: f64, bi: f64) -> C64 {
    if br.abs() < bi.abs() {
        let ratio = div_f64(br, bi);
        let div = add_f64(mul_f64(br, ratio), bi);
        let tr = add_f64(mul_f64(ar, ratio), ai);
        let ti = sub_f64(mul_f64(ai, ratio), ar);
        C64::new(div_f64(tr, div), div_f64(ti, div))
    } else {
        let ratio = div_f64(bi, br);
        let div = add_f64(mul_f64(bi, ratio), br);
        let tr = add_f64(mul_f64(ai, ratio), ar);
        let ti = sub_f64(ai, mul_f64(ar, ratio));
        C64::new(div_f64(tr, div), div_f64(ti, div))
    }
}

/// `copysign(isinf(v) ? 1 : 0, v)` (Annex G's boxing of an infinity).
#[inline]
fn boxed(v: f64) -> f64 {
    (if v.is_infinite() { 1.0f64 } else { 0.0 }).copysign(v)
}

/// C11 Annex G.5.1's `_Cmultd` (libgcc's `__muldc3`): the plain product, and where both of its
/// parts are NaN the infinities recovered (an infinite factor boxed to ±1 and the other's NaNs
/// zeroed; or, after an overflow, every NaN zeroed) and the product recomputed times infinity.
pub fn muldc3(a: f64, b: f64, c: f64, d: f64) -> C64 {
    let (ac, bd, ad, bc) = (mul_f64(a, c), mul_f64(b, d), mul_f64(a, d), mul_f64(b, c));
    let mut x = sub_f64(ac, bd);
    let mut y = add_f64(ad, bc);
    if x.is_nan() && y.is_nan() {
        let (mut a, mut b, mut c, mut d) = (a, b, c, d);
        let mut recalc = false;
        let zero_nan = |v: f64| if v.is_nan() { 0.0f64.copysign(v) } else { v };
        if a.is_infinite() || b.is_infinite() {
            a = boxed(a);
            b = boxed(b);
            c = zero_nan(c);
            d = zero_nan(d);
            recalc = true;
        }
        if c.is_infinite() || d.is_infinite() {
            c = boxed(c);
            d = boxed(d);
            a = zero_nan(a);
            b = zero_nan(b);
            recalc = true;
        }
        if !recalc && (ac.is_infinite() || bd.is_infinite() || ad.is_infinite() || bc.is_infinite())
        {
            a = zero_nan(a);
            b = zero_nan(b);
            c = zero_nan(c);
            d = zero_nan(d);
            recalc = true;
        }
        if recalc {
            x = mul_f64(f64::INFINITY, sub_f64(mul_f64(a, c), mul_f64(b, d)));
            y = mul_f64(f64::INFINITY, add_f64(mul_f64(a, d), mul_f64(b, c)));
        }
    }
    C64::new(x, y)
}

/// C99's (C++'s `std::complex<double>`) product as GCC compiles it: the plain product inline,
/// [`muldc3`] when both of its parts are NaN.
#[inline]
pub fn cxx_mul(x: C64, y: C64) -> C64 {
    let p = x.mul(y);
    if p.re.is_nan() && p.im.is_nan() {
        return muldc3(x.re, x.im, y.re, y.im);
    }
    p
}

/// libgcc's `__divdc3(a, b, c, d)` (C99's and C++'s complex quotient): Smith's algorithm, and
/// where both parts are NaN C11 Annex G's recovery: a zero divisor of a number not all NaN gives
/// `copysign(inf, c)` times it; an infinite dividend over a finite divisor gives infinity times
/// the boxed quotient; a finite dividend over an infinite divisor, zero times it.
pub fn divdc3(a: f64, b: f64, c: f64, d: f64) -> C64 {
    let q = smith(a, b, c, d);
    let (mut x, mut y) = (q.re, q.im);
    if x.is_nan() && y.is_nan() {
        if c == 0.0 && d == 0.0 && (!a.is_nan() || !b.is_nan()) {
            let s = f64::INFINITY.copysign(c);
            x = mul_f64(s, a);
            y = mul_f64(s, b);
        } else if (a.is_infinite() || b.is_infinite()) && c.is_finite() && d.is_finite() {
            let (a, b) = (boxed(a), boxed(b));
            x = mul_f64(f64::INFINITY, add_f64(mul_f64(a, c), mul_f64(b, d)));
            y = mul_f64(f64::INFINITY, sub_f64(mul_f64(b, c), mul_f64(a, d)));
        } else if (c.is_infinite() || d.is_infinite()) && a.is_finite() && b.is_finite() {
            let (c, d) = (boxed(c), boxed(d));
            x = mul_f64(0.0, add_f64(mul_f64(a, c), mul_f64(b, d)));
            y = mul_f64(0.0, sub_f64(mul_f64(b, c), mul_f64(a, d)));
        }
    }
    C64::new(x, y)
}

/// C99's (C++'s) quotient `x / y` ([`divdc3`]).
#[inline]
pub fn cxx_div(x: C64, y: C64) -> C64 {
    divdc3(x.re, x.im, y.re, y.im)
}

/// `cabs(z)`: the correctly rounded `hypot(re, im)` (`math::hypot`), with CORE-MATH's own NaN
/// and infinity rules (`hypot(±inf, qNaN) = +inf`; otherwise `|x| + |y|`) in x86-64's SSE NaNs.
pub fn cabs(z: C64) -> f64 {
    let (x, y) = (z.re, z.im);
    let emsk: u64 = 0x7ff << 52;
    if x.to_bits() & emsk == emsk || y.to_bits() & emsk == emsk {
        let (wx, wy, wm) = (x.to_bits() << 1, y.to_bits() << 1, emsk << 1);
        let ninf = (wx == wm) ^ (wy == wm);
        let nqnn = ((wx >> 52) == 0xfff) ^ ((wy >> 52) == 0xfff);
        if ninf && nqnn {
            return f64::INFINITY;
        }
        return add_f64(x.abs(), y.abs());
    }
    math::hypot(x, y)
}

/// `sqrt(x)` as `sqrtsd` computes it, for the non-negative finite numbers [`csqrt`] takes it of.
#[inline(always)]
fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// `csqrt(z)`: Kahan's principal square root with the correctly rounded `hypot`, in the form
/// glibc 2.35's was measured to take: an infinite imaginary part gives `(+inf, y)`; an infinite
/// real part `(+inf, ±0)` (positive) or `(+0, ±inf)` (negative), with glibc's NaN `0x7ff8...` in
/// the other part for a NaN imaginary part; any other NaN `(NaN, NaN)`; `(±0, ±0)` gives `(+0,
/// y)`. Otherwise the parts are scaled by 1/4 when one is above `DBL_MAX / 4` (the result
/// doubled), by 2^108 when both are below `2 DBL_MIN` (the result times 2^-54); then a zero real
/// part gives `r = sqrt(|y| / 2)` and `(r, copysign(r, y))`, a zero imaginary part `(sqrt(x), y)`
/// or `(+0, copysign(sqrt(-x), y))`, and otherwise `t = sqrt((hypot(x, y) + |x|) / 2)` and `(t,
/// (y / t) / 2)` for `x >= 0`, `((|y| / t) / 2, copysign(t, y))` for `x < 0`.
pub fn csqrt(z: C64) -> C64 {
    let (mut x, mut y) = (z.re, z.im);
    if y.is_infinite() {
        return C64::new(f64::INFINITY, y);
    }
    if x.is_infinite() {
        if x < 0.0 {
            let re = if y.is_nan() { QNAN } else { 0.0 };
            return C64::new(re, f64::INFINITY.copysign(y));
        }
        let im = if y.is_nan() { QNAN } else { 0.0f64.copysign(y) };
        return C64::new(f64::INFINITY, im);
    }
    if x.is_nan() || y.is_nan() {
        return C64::new(QNAN, QNAN);
    }
    if x == 0.0 && y == 0.0 {
        return C64::new(0.0, y);
    }
    let mut k = 0i32;
    if x.abs() > f64::MAX / 4.0 || y.abs() > f64::MAX / 4.0 {
        x *= 0.25;
        y *= 0.25;
        k = 1;
    } else if x.abs() < 2.0 * f64::MIN_POSITIVE && y.abs() < 2.0 * f64::MIN_POSITIVE {
        x *= f64::from_bits(((1023 + 108) as u64) << 52);
        y *= f64::from_bits(((1023 + 108) as u64) << 52);
        k = -54;
    }
    let (mut re, mut im);
    if x == 0.0 {
        let r = sqrt(0.5 * y.abs());
        re = r;
        im = r.copysign(y);
    } else if y == 0.0 {
        if x > 0.0 {
            re = sqrt(x);
            im = y;
        } else {
            re = 0.0;
            im = sqrt(-x).copysign(y);
        }
    } else {
        let t = sqrt(0.5 * (math::hypot(x, y) + x.abs()));
        if x >= 0.0 {
            re = t;
            im = 0.5 * (y / t);
        } else {
            re = 0.5 * (y.abs() / t);
            im = t.copysign(y);
        }
    }
    if k == 1 {
        re *= 2.0;
        im *= 2.0;
    } else if k != 0 {
        let s = f64::from_bits(((1023 - 54) as u64) << 52);
        re *= s;
        im *= s;
    }
    C64::new(re, im)
}

/// The complex logarithm the project decided for `clog` (FreeSurfer's matrix logarithm takes
/// `std::log` of a complex eigenvalue; glibc 2.35's `clog` is not correctly rounded and its form
/// is not public: it differs from every simple formula on 0.1 to 15 % of arguments, measured as
/// a black box): the imaginary part the correctly rounded `atan2(y, x)` ([`math::atan2`]), the
/// real part `log |z|` from correctly rounded functions: where `|z|^2 - 1` (each square split
/// exactly into a product and its error, `p + e` with a fused multiply-add, then `(p_x - 1) +
/// p_y` by two error-free sums and the low parts added in order, rounded once) is above `-1/2`
/// and below `1/2`, half the correctly rounded `log1p` of it ([`math::log1p`]: accurate near the
/// unit circle, where an eigenvalue of a near-identity transform lies), otherwise the correctly
/// rounded `log` of the correctly rounded `hypot` (relative error below an ulp and a half there).
/// The special values are C11 Annex G's as glibc's were measured (`clog(+-0 + i y) = -inf + i
/// atan2(y, +-0)`; an infinite part gives `+inf`; a NaN part gives glibc's NaN `0x7ff8...` in
/// both parts, or `+inf` in the real part when the other part is infinite).
pub fn clog(z: C64) -> C64 {
    let (x, y) = (z.re, z.im);
    if x.is_nan() || y.is_nan() {
        let re = if x.is_infinite() || y.is_infinite() {
            f64::INFINITY
        } else {
            QNAN
        };
        return C64::new(re, QNAN);
    }
    let im = math::atan2(y, x);
    if x.is_infinite() || y.is_infinite() {
        return C64::new(f64::INFINITY, im);
    }
    if x == 0.0 && y == 0.0 {
        return C64::new(f64::NEG_INFINITY, im);
    }
    C64::new(clog_re(x, y), im)
}

/// [`clog`]'s real part of a finite `x + i y`, not both zero.
fn clog_re(x: f64, y: f64) -> f64 {
    let px = x * x;
    let ex = x.mul_add(x, -px);
    let py = y * y;
    let ey = y.mul_add(y, -py);
    // two error-free sums: (px - 1) + py
    let (h1, l1) = two_sum(px, -1.0);
    let (h, l2) = two_sum(h1, py);
    let lo = ((l1 + l2) + ex) + ey;
    let d = h + lo;
    if d > -0.5 && d < 0.5 {
        0.5 * math::log1p(d)
    } else {
        math::log(math::hypot(x, y))
    }
}

/// Knuth's error-free sum: `(s, e)` with `s = a + b` rounded and `s + e = a + b` exactly.
#[inline(always)]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    let e = (a - (s - bb)) + (b - bb);
    (s, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// glibc 2.35's `csqrt` on special values (measured as a black box, the investigation's
    /// table) and the zero-part paths.
    #[test]
    fn csqrt_special_values_are_glibcs() {
        let b = |x: u64, y: u64| {
            let r = csqrt(C64::new(f64::from_bits(x), f64::from_bits(y)));
            (r.re.to_bits(), r.im.to_bits())
        };
        let (z, nz, one, mone) = (
            0u64,
            0x8000_0000_0000_0000u64,
            0x3ff0_0000_0000_0000u64,
            0xbff0_0000_0000_0000u64,
        );
        let (inf, minf, nan, snan) = (
            0x7ff0_0000_0000_0000u64,
            0xfff0_0000_0000_0000u64,
            0x7ff8_0000_0000_0000u64,
            0xfff4_0000_0000_0456u64,
        );
        assert_eq!(b(z, nz), (z, nz));
        assert_eq!(b(nz, z), (z, z));
        assert_eq!(b(z, mone), (0x3fe6_a09e_667f_3bcd, 0xbfe6_a09e_667f_3bcd));
        assert_eq!(b(mone, z), (z, one));
        assert_eq!(b(mone, nz), (z, mone));
        assert_eq!(b(one, mone), (0x3ff1_9435_caff_a9f9, 0xbfdd_2031_38f6_c828));
        assert_eq!(
            b(mone, mone),
            (0x3fdd_2031_38f6_c828, 0xbff1_9435_caff_a9f9)
        );
        assert_eq!(b(snan, inf), (inf, inf));
        assert_eq!(b(snan, minf), (inf, minf));
        assert_eq!(b(inf, mone), (inf, nz));
        assert_eq!(b(inf, snan), (inf, nan));
        assert_eq!(b(minf, mone), (z, minf));
        assert_eq!(b(minf, 0xfff8_0000_0000_0000), (nan, minf));
        assert_eq!(b(snan, one), (nan, nan));
        assert_eq!(b(one, snan), (nan, nan));
    }

    /// `cabs`: the correctly rounded hypot, and CORE-MATH's NaN rules in SSE's NaNs.
    #[test]
    fn cabs_is_the_correctly_rounded_hypot() {
        assert_eq!(cabs(C64::new(3.0, -4.0)), 5.0);
        assert_eq!(cabs(C64::new(f64::NEG_INFINITY, f64::NAN)), f64::INFINITY);
        let s = f64::from_bits(0xfff4_0000_0000_0456);
        assert_eq!(cabs(C64::new(s, 1.0)).to_bits(), 0x7ffc_0000_0000_0456);
        assert_eq!(
            cabs(C64::new(s, f64::INFINITY)).to_bits(),
            0x7ffc_0000_0000_0456
        );
    }

    /// The decided `clog`: glibc's special values (measured as a black box) and the two real
    /// parts' paths.
    #[test]
    fn clog_special_values_and_paths() {
        let b = |x: u64, y: u64| {
            let r = clog(C64::new(f64::from_bits(x), f64::from_bits(y)));
            (r.re.to_bits(), r.im.to_bits())
        };
        let (z, nz, one, inf, minf, nan, snan) = (
            0u64,
            0x8000_0000_0000_0000u64,
            0x3ff0_0000_0000_0000u64,
            0x7ff0_0000_0000_0000u64,
            0xfff0_0000_0000_0000u64,
            0x7ff8_0000_0000_0000u64,
            0xfff4_0000_0000_0456u64,
        );
        assert_eq!(b(z, z), (minf, z));
        assert_eq!(b(nz, z), (minf, 0x4009_21fb_5444_2d18));
        assert_eq!(b(nz, nz), (minf, 0xc009_21fb_5444_2d18));
        assert_eq!(b(z, inf), (inf, 0x3ff9_21fb_5444_2d18));
        assert_eq!(b(minf, inf), (inf, 0x4002_d97c_7f33_21d2));
        assert_eq!(b(inf, snan), (inf, nan));
        assert_eq!(b(snan, minf), (inf, nan));
        assert_eq!(b(one, snan), (nan, nan));
        assert_eq!(b(snan, one), (nan, nan));
        assert_eq!(b(one, z), (z, z));
        // near the unit circle (log1p) and far from it (log of hypot)
        let near = clog(C64::new(0.6, 0.8000001));
        assert!(near.re > 7.99e-8 && near.re < 8.01e-8);
        let far = clog(C64::new(3.0, 4.0));
        assert_eq!(far.re, math::log(5.0));
    }

    /// Smith's division and Annex G's recoveries.
    #[test]
    fn division_and_products() {
        let q = divdc3(1.0, 2.0, 3.0, 4.0);
        assert_eq!((q.re, q.im), (0.44, 0.08));
        let q = divdc3(1.0, 2.0, 0.0, 0.0);
        assert_eq!((q.re, q.im), (f64::INFINITY, f64::INFINITY));
        let q = divdc3(f64::INFINITY, 1.0, 1.0, 1.0);
        assert_eq!((q.re, q.im), (f64::INFINITY, f64::NEG_INFINITY));
        let q = divdc3(1.0, 1.0, f64::INFINITY, 1.0);
        assert_eq!((q.re, q.im), (0.0, 0.0));
        let p = cxx_mul(C64::new(f64::INFINITY, f64::NAN), C64::new(1.0, 0.0));
        assert!(p.re.is_infinite() || p.im.is_infinite());
        assert_eq!(
            C64::new(1.0, 2.0).fdiv(C64::new(3.0, 4.0)),
            C64::new(0.44, 0.08)
        );
    }
}

// SPDX-License-Identifier: Apache-2.0
//! The double-precision sine and cosine with the bits of glibc 2.35 on x86-64 (`sin`, `cos`:
//! the FMA build glibc runs on CPUs with FMA and AVX2, the oracles' build; `sincos`: its single
//! build), on every platform.
//!
//! A **clean-room implementation** (Apache-2.0), written only from `specs/glibc-dbl.md` (the
//! method as mathematics: the three kernels of §6, the paths of §4 and §7, the reductions of §8,
//! the constants of §3 as bits, table `R` of §5 by its definition) and black-box runs of the
//! system's functions. glibc's code for these functions (IBM's Accurate Mathematical Library,
//! LGPL-2.1-or-later) was never opened, nor any translation of it. Table `R` and the digits of
//! 2/π are generated from their definitions (`sincos_glibc_tables.rs`, written by
//! `validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py`).
//!
//! They are **not correctly rounded** (GLIBC-07: about 0.1 % of results are one unit in the last
//! place from the exact value); [`super::sin`] and [`super::cos`] are, and are larmorx's
//! default everywhere. These serve `reproduce` only, where an original program's result depends
//! on glibc's last bit (FSL's programs' double `sin` and `cos`).
//!
//! **What `reproduce` gets.** glibc 2.35's bits on every argument tested (the black-box record's
//! 3.2 × 10^9 arguments and 184 special ones, NaN bits included, and billions of fresh arguments
//! against the system's functions: `validation/investigations/cleanroom-glibc-dbl/`), with one
//! theoretical exception the spec documents (§5.3, the user's decision of 2026-11-04): table `R`
//! comes from its definition, which differs from glibc's table in 18 low parts (17 rows), so a
//! result can differ from glibc's where the exact correction of the table-driven step lies within
//! about 2^-107 of a rounding boundary, an estimated one evaluation in about 2^53 of those that
//! read these rows; none is known. glibc's double `tan`, `asin`, `acos` and `atan` stay
//! correctly rounded in `reproduce` too (the user's decision, spec §9; they differ from glibc's
//! in the last bit on 0.05 to 0.25 % of results, spec §10.3).
//!
//! **The arithmetic.** Every operation is one IEEE operation in round to nearest, in the order
//! the spec lists. The spec's `FMA(a, b, c)` has two readings (§1): in `sin` and `cos` it is
//! fused (`f64::mul_add`, correctly rounded on every platform; on x86-64 CPUs with FMA a copy
//! compiled with FMA is chosen at run time, `super::glibc_dispatch!`, the same bits); in
//! `sincos` (and in [`sin_glibc_sse2`], [`cos_glibc_sse2`], glibc's build for CPUs without FMA)
//! it is a rounded product then a rounded sum. Rust never fuses `a * b + c` by itself. The
//! only NaN results are `x / x` for an infinite or NaN `x`, made as x86-64 makes them
//! (`super::x86_nan`): the default NaN `0xfff8000000000000` for an infinity, a NaN argument
//! quieted (sign and payload kept). So the results are the same bits on every platform.

// The spec's expression forms are kept (`bb = fl(fl(t − b) + bb)`).
#![allow(clippy::unreadable_literal, clippy::assign_op_pattern)]

use super::sincos_glibc_tables::{D, R};
use super::x86_nan::div_f64;

const fn b(bits: u64) -> f64 {
    f64::from_bits(bits)
}

// Spec §3: the multiples and parts of π/2.
/// `fl(π/2)`.
const H0: f64 = b(0x3ff921fb54442d18);
/// `fl(π/2 − H0)`.
const H1: f64 = b(0x3c91a62633145c07);
/// `fl(2/π)`.
const G: f64 = b(0x3fe45f306dc9c883);
/// π/2 in parts of 26, 27 and 26 significant bits, then the rest (§3).
const M1: f64 = b(0x3ff921fb58000000);
const M2: f64 = b(0xbe4dde973c000000);
const M3: f64 = b(0xbc8cb3b398000000);
const M4: f64 = b(0xbacd747f23e32ed7);
/// `π/2 − M1` with 23 significant bits.
const M2L: f64 = b(0xbe4dde9740000000);

// Shifters and scales.
/// `1.5 · 2^52`: `fl(fl(v + K52) − K52)` is the integer nearest `v` (ties to even), `|v| < 2^51`.
const K52: f64 = b(0x4338000000000000);
/// `1.5 · 2^54`: the same for the multiples of 4.
const K54: f64 = b(0x4358000000000000);
/// `2^27 + 1`, the splitting constant.
const C27: f64 = 134217729.0;
/// `2^-600`.
const P2M600: f64 = b(0x1a70000000000000);

// The polynomial coefficients (§3), bit for bit.
const T3: f64 = b(0xbfc5555555555555);
const T5: f64 = b(0x3f81111111110ece);
const T7: f64 = b(0xbf2a01a019db08b8);
const T9: f64 = b(0x3ec71de27b9a7ed9);
const T11: f64 = b(0xbe5addffc2fcdf59);
const U3: f64 = b(0xbfc5555555555515);
const U5: f64 = b(0x3f811110e829872f);
const V2: f64 = b(0x3fe0000000000000);
const V4: f64 = b(0xbfa5555555555535);
const V6: f64 = b(0x3f56c16bedd9e239);

/// `τ`, the double nearest 0.126 (§3): below it the table-driven sine is the small-number sine.
const TAU: f64 = b(0x3fc020c49ba5e354);

// The paths' limits on `|x|` (§4).
/// `2^-27`.
const LIM27: f64 = b(0x3e40000000000000);
/// `2^-26`.
const LIM26: f64 = b(0x3e50000000000000);
/// `0.85546875 = 0x1.b6p-1`.
const LIM0: f64 = b(0x3feb600000000000);
/// `L1 = 0x1.368fdp+1`.
const L1: f64 = b(0x400368fd00000000);
/// `L2 = 0x1.921fbp+26`.
const L2: f64 = b(0x419921fb00000000);

/// The spec's `FMA(a, b, c)` in its two readings (§1): fused (`sin`, `cos`) or a rounded product
/// then a rounded sum (`sincos`, glibc's build without FMA).
#[inline(always)]
fn fma<const FUSED: bool>(a: f64, b: f64, c: f64) -> f64 {
    if FUSED { a.mul_add(b, c) } else { a * b + c }
}

/// The integer nearest `v`, ties to even, as a double (`|v| < 2^51`; exact IEEE arithmetic).
#[inline(always)]
fn nearest(v: f64) -> f64 {
    (v + K52) - K52
}

/// Row `n` of table `R`: `(SH, SL, CH, CL)`.
#[inline(always)]
fn row(n: usize) -> (f64, f64, f64, f64) {
    let r = &R[n];
    (b(r[0]), b(r[1]), b(r[2]), b(r[3]))
}

/// `S0(x, d)`, the sine of a small number `x + d` (§6.1, `|x| < τ`).
#[inline(always)]
fn sin_small<const F: bool>(x: f64, d: f64) -> f64 {
    let z = x * x;
    let p = fma::<F>(
        z,
        fma::<F>(z, fma::<F>(z, fma::<F>(z, T11, T9), T7), T5),
        T3,
    );
    let h = 0.5 * d;
    let e = fma::<F>(z, fma::<F>(p, x, -h), d);
    x + e
}

/// `S(x, d)`, the table-driven sine of `x + d` (§6.2).
#[inline(always)]
fn sin_kernel<const F: bool>(x: f64, d: f64) -> f64 {
    if x.abs() < TAU {
        return sin_small::<F>(x, d);
    }
    let d1 = if x > 0.0 { d } else { -d };
    let y = x.abs();
    let nf = nearest(128.0 * y);
    let xr = y - nf * (1.0 / 128.0); // exact
    let (sh, sl, ch, cl) = row(nf as usize);
    let z = xr * xr;
    let p = fma::<F>(z, U5, U3);
    let s = xr + fma::<F>(xr * z, p, d1);
    let q = fma::<F>(z, fma::<F>(z, V6, V4), V2);
    let c = fma::<F>(xr, d1, z * q);
    let rho = fma::<F>(s, ch, fma::<F>(-c, sh, fma::<F>(s, cl, sl)));
    (sh + rho).copysign(x)
}

/// `C(x, d)`, the table-driven cosine of `x + d` (§6.3).
#[inline(always)]
fn cos_kernel<const F: bool>(x: f64, d: f64) -> f64 {
    let d1 = if x < 0.0 { -d } else { d };
    let y = x.abs();
    let nf = nearest(128.0 * y);
    let xr = (y - nf * (1.0 / 128.0)) + d1; // the difference exact, the sum rounded
    let (sh, sl, ch, cl) = row(nf as usize);
    let z = xr * xr;
    let p = fma::<F>(z, U5, U3);
    let s = fma::<F>(xr * z, p, xr);
    let q = fma::<F>(z, fma::<F>(z, V6, V4), V2);
    let c = z * q;
    let rho = fma::<F>(-s, sh, fma::<F>(-c, ch, fma::<F>(-s, sl, cl)));
    ch + rho
}

/// The reduction below `L2` (§8.1): `(n, a, da)` with `x ≈ k π/2 + (a + da)`, `n = k mod 4`.
#[inline(always)]
fn reduce_medium<const F: bool>(x: f64) -> (u32, f64, f64) {
    let t = fma::<F>(x, G, K52);
    let k = t - K52; // an integer, exact
    let n = ((k as i64) & 3) as u32; // the non-negative remainder mod 4
    let mut y = fma::<F>(-k, M1, x);
    y = fma::<F>(-k, M2, y);
    let t2 = fma::<F>(-k, M3, y);
    let e = fma::<F>(-k, M3, y - t2);
    let a = fma::<F>(-k, M4, t2);
    let da = e + fma::<F>(-k, M4, t2 - a);
    (n, a, da)
}

/// `2^e` for `-1022 ≤ e ≤ 1023`, exact.
#[inline(always)]
fn pow2(e: i32) -> f64 {
    f64::from_bits(((1023 + e) as u64) << 52)
}

/// One part `p` of the large reduction (§8.2 step 3): `(b, bb, Σ)`.
#[inline(always)]
fn reduce_part(p: f64) -> (f64, f64, f64) {
    let e = ((p.to_bits() >> 52) & 0x7ff) as i32;
    let j0 = if e >= 450 {
        ((e - 450) / 24) as usize
    } else {
        0
    };
    let mut r = [0.0f64; 6];
    for (i, ri) in r.iter_mut().enumerate() {
        let j = j0 + i;
        *ri = (p * D[j]) * pow2(576 - 24 * j as i32); // both products exact
    }
    let mut sum = 0.0f64;
    for ri in &mut r[..3] {
        let u = nearest(*ri);
        sum += u;
        *ri -= u;
    }
    let mut t = 0.0f64;
    for ri in r.iter().rev() {
        t += *ri;
    }
    let mut bb = (((((r[0] - t) + r[1]) + r[2]) + r[3]) + r[4]) + r[5];
    let u = nearest(t);
    sum += u;
    t -= u;
    let bsum = t + bb;
    bb = (t - bsum) + bb;
    let u = (sum + K54) - K54; // the multiple of 4 nearest Σ
    sum -= u;
    (bsum, bb, sum)
}

/// The reduction from `L2` to the largest double (§8.2; no fused operation in any build).
#[inline(always)]
fn reduce_large(x: f64) -> (u32, f64, f64) {
    let xp = x * P2M600; // exact
    let w = xp * C27;
    let x1 = w - (w - xp);
    let x2 = xp - x1;
    let (b1, bb1, s1) = reduce_part(x1);
    let (b2, bb2, s2) = reduce_part(x2);
    let mut sum = s1 + s2;
    let mut bs = b1 + b2;
    let bb = if b1.abs() > b2.abs() {
        (b1 - bs) + b2
    } else {
        (b2 - bs) + b1
    };
    if bs > 0.5 {
        bs -= 1.0;
        sum += 1.0;
    } else if bs < -0.5 {
        bs += 1.0;
        sum -= 1.0;
    }
    let s = bs + ((bb + bb1) + bb2);
    let t = ((bs - s) + bb) + (bb1 + bb2);
    let w = s * C27;
    let s1 = w - (w - s);
    let s2 = s - s1;
    let hb = s * H0;
    let hbb = ((((s1 * M1) - hb) + (s1 * M2L)) + (s2 * M1)) + (((s2 * M2L) + (s * H1)) + (t * H0));
    let a = hb + hbb;
    let da = (hb - a) + hbb;
    let n = ((sum as i64) & 3) as u32;
    (n, a, da)
}

/// `(n, a, da)` from `L1` (§7.3): §8.1 below `L2`, §8.2 from it.
#[inline(always)]
fn reduce<const F: bool>(x: f64, ax: f64) -> (u32, f64, f64) {
    if ax < L2 {
        reduce_medium::<F>(x)
    } else {
        reduce_large(x)
    }
}

/// `sin(x)` in either reading of `FMA` (§4, §7.1 to §7.3).
#[inline(always)]
fn sin_build<const F: bool>(x: f64) -> f64 {
    let ax = x.abs();
    if ax < LIM26 {
        return x;
    }
    if ax < LIM0 {
        return sin_kernel::<F>(x, 0.0);
    }
    if ax < L1 {
        let t = H0 - ax;
        return cos_kernel::<F>(t, H1).copysign(x);
    }
    if !ax.is_finite() {
        return div_f64(x, x);
    }
    let (n, a, da) = reduce::<F>(x, ax);
    let v = if n & 1 == 1 {
        cos_kernel::<F>(a, da)
    } else {
        sin_kernel::<F>(a, da)
    };
    if n >= 2 { -v } else { v }
}

/// `cos(x)` in either reading of `FMA` (§4, §7.1 to §7.3).
#[inline(always)]
fn cos_build<const F: bool>(x: f64) -> f64 {
    let ax = x.abs();
    if ax < LIM27 {
        return 1.0;
    }
    if ax < LIM0 {
        return cos_kernel::<F>(x, 0.0);
    }
    if ax < L1 {
        let y = H0 - ax;
        let a = y + H1;
        let da = (y - a) + H1;
        return sin_kernel::<F>(a, da);
    }
    if !ax.is_finite() {
        return div_f64(x, x);
    }
    let (n, a, da) = reduce::<F>(x, ax);
    let m = (n + 1) & 3;
    let v = if m & 1 == 1 {
        cos_kernel::<F>(a, da)
    } else {
        sin_kernel::<F>(a, da)
    };
    if m >= 2 { -v } else { v }
}

/// The sine with glibc 2.35's bits (x86-64, the FMA build). See the module's documentation.
#[inline(always)]
#[must_use]
pub fn sin_glibc(x: f64) -> f64 {
    sin_build::<true>(x)
}

/// The cosine with glibc 2.35's bits (x86-64, the FMA build).
#[inline(always)]
#[must_use]
pub fn cos_glibc(x: f64) -> f64 {
    cos_build::<true>(x)
}

/// The sine as glibc 2.35's x86-64 build without FMA computes it (each `FMA` of the spec a
/// rounded product then a rounded sum): glibc's choice on x86-64 CPUs without FMA and AVX2.
#[inline]
#[must_use]
pub fn sin_glibc_sse2(x: f64) -> f64 {
    sin_build::<false>(x)
}

/// The cosine as glibc 2.35's x86-64 build without FMA computes it.
#[inline]
#[must_use]
pub fn cos_glibc_sse2(x: f64) -> f64 {
    cos_build::<false>(x)
}

/// `(sin x, cos x)` with glibc 2.35's bits (x86-64, `sincos`'s single build: every `FMA` of the
/// spec a rounded product then a rounded sum; its own paths, spec §7.4).
#[inline]
#[must_use]
pub fn sincos_glibc(x: f64) -> (f64, f64) {
    const F: bool = false;
    let ax = x.abs();
    if ax < LIM27 {
        return (x, 1.0);
    }
    if ax < LIM0 {
        return (sin_kernel::<F>(x, 0.0), cos_kernel::<F>(x, 0.0));
    }
    if ax < L1 {
        let y = H0 - ax;
        let a = y + H1;
        let da = (y - a) + H1;
        return (cos_kernel::<F>(a, da).copysign(x), sin_kernel::<F>(a, da));
    }
    if !ax.is_finite() {
        let q = div_f64(x, x);
        return (q, q);
    }
    let (n, mut a, mut da) = reduce::<F>(x, ax);
    if n == 1 || n == 2 {
        a = -a;
        da = -da;
    }
    let s = sin_kernel::<F>(a, da);
    let c = cos_kernel::<F>(a, da);
    match n {
        0 => (s, c),
        1 => (c, s),
        2 => (s, -c),
        _ => (-c, s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits2(v: (f64, f64)) -> (u64, u64) {
        (v.0.to_bits(), v.1.to_bits())
    }

    /// Spec §4.1: zeros, infinities, NaNs (x86-64's), subnormals.
    #[test]
    fn special_values() {
        for f in [sin_glibc, sin_glibc_sse2] {
            assert_eq!(f(0.0).to_bits(), 0);
            assert_eq!(f(-0.0).to_bits(), 0x8000_0000_0000_0000);
            assert_eq!(f(5e-324).to_bits(), 1);
            assert_eq!(f(-5e-324).to_bits(), 0x8000_0000_0000_0001);
        }
        for f in [cos_glibc, cos_glibc_sse2] {
            assert_eq!(f(0.0), 1.0);
            assert_eq!(f(-0.0), 1.0);
            assert_eq!(f(5e-324), 1.0);
        }
        assert_eq!(
            bits2(sincos_glibc(-0.0)),
            (0x8000_0000_0000_0000, 1.0f64.to_bits())
        );
        let default_nan = 0xfff8_0000_0000_0000u64;
        for x in [f64::INFINITY, f64::NEG_INFINITY] {
            for f in [sin_glibc, cos_glibc, sin_glibc_sse2, cos_glibc_sse2] {
                assert_eq!(f(x).to_bits(), default_nan);
            }
            assert_eq!(bits2(sincos_glibc(x)), (default_nan, default_nan));
        }
        for (x, q) in [
            (0x7ff4_0000_0000_5678u64, 0x7ffc_0000_0000_5678u64),
            (0xfff8_0000_0000_1234, 0xfff8_0000_0000_1234),
            (0x7ff0_0000_0000_0001, 0x7ff8_0000_0000_0001),
            (0xfff4_0000_0000_5678, 0xfffc_0000_0000_5678),
        ] {
            let x = f64::from_bits(x);
            for f in [sin_glibc, cos_glibc, sin_glibc_sse2, cos_glibc_sse2] {
                assert_eq!(f(x).to_bits(), q);
            }
            assert_eq!(bits2(sincos_glibc(x)), (q, q));
        }
    }

    /// Table `R`'s first row and a check of the definition at one row (`sin` and `cos` of
    /// N/128 as double-doubles: `SH` is the nearest double to the exact value, so the
    /// correctly rounded `sin`/`cos` of the double `N/128` are `SH`/`CH`).
    #[test]
    fn table_r() {
        assert_eq!(R[0], [0, 0, 1.0f64.to_bits(), 0]);
        for (n, r) in R.iter().enumerate() {
            let a = n as f64 / 128.0;
            assert_eq!(super::super::sin(a).to_bits(), r[0], "row {n}");
            assert_eq!(super::super::cos(a).to_bits(), r[2], "row {n}");
            if n > 0 {
                // the low part is below half a unit in the last place of the high part
                for (h, l) in [(r[0], r[1]), (r[2], r[3])] {
                    let (h, l) = (b(h), b(l));
                    assert!(l.abs() <= (h.next_up() - h) / 2.0, "row {n}");
                }
            }
        }
        assert_eq!(D[0], 10680707.0);
        assert_eq!(D[1], 7228996.0);
        assert_eq!(D[74], 343717.0);
    }

    /// `sincos` is `sin` and `cos` of glibc's build without FMA outside `[0.85546875, L1)`
    /// (spec §2), on seeded arguments of every path.
    #[test]
    fn sincos_is_the_sse2_build_outside_the_band() {
        let mut s = 0x2545_f491_4f6c_dd1du64;
        for k in 0..400_000u32 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let u = (s >> 11) as f64 / (1u64 << 53) as f64;
            let x = match k % 4 {
                0 => f64::from_bits(s),
                1 => (u - 0.5) * 20.0,
                2 => (u - 0.5) * 4e8,
                _ => (u - 0.5) * 1.8,
            };
            let ax = x.abs();
            let (sv, cv) = sincos_glibc(x);
            if !(LIM0..L1).contains(&ax) {
                assert_eq!(sv.to_bits(), sin_glibc_sse2(x).to_bits(), "{x:e}");
            }
            assert_eq!(cv.to_bits(), cos_glibc_sse2(x).to_bits(), "{x:e}");
        }
    }

    /// The unit tests' arguments (the verifier's `unit_test_arg`, the same code): for
    /// `k = 0 … 65535`, SplitMix64 of `k` mapped by `k mod 4` to a bit pattern, a uniform value
    /// on [-8, 8), a uniform value on [-2^28, 2^28), a log-uniform magnitude 2^-30 to 2^33. Plain
    /// integer and IEEE arithmetic: the same arguments on every platform.
    fn unit_test_arg(k: u64) -> f64 {
        let mut z = k.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let r = z ^ (z >> 31);
        let u = (r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000);
        match k % 4 {
            0 => f64::from_bits(r),
            1 => u * 16.0 - 8.0,
            2 => u * 536_870_912.0 - 268_435_456.0,
            _ => {
                let e = ((r >> 52) & 0x7ff) % 64;
                f64::from_bits(
                    (r & 0x8000_0000_0000_0000) | ((993 + e) << 52) | (r & 0x000f_ffff_ffff_ffff),
                )
            }
        }
    }

    /// glibc 2.35's results on the unit tests' 65,536 arguments, on every platform: the FNV-1a
    /// hashes of the system's `sin`, `cos` and `sincos` (FMA builds; and with
    /// `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`, its builds without FMA), recorded on Linux
    /// x86-64 by `validation/investigations/cleanroom-glibc-dbl/verify` (`hashes`).
    #[test]
    fn matches_glibc_on_seeded_arguments() {
        let fnv = |h: u64, v: f64| (h ^ v.to_bits()).wrapping_mul(0x0100_0000_01b3);
        let mut h = [0xcbf2_9ce4_8422_2325u64; 6];
        for k in 0..65_536u64 {
            let x = unit_test_arg(k);
            let (s, c) = sincos_glibc(x);
            h[0] = fnv(h[0], super::super::sin_glibc(x));
            h[1] = fnv(h[1], super::super::cos_glibc(x));
            h[2] = fnv(h[2], s);
            h[3] = fnv(h[3], c);
            h[4] = fnv(h[4], sin_glibc_sse2(x));
            h[5] = fnv(h[5], cos_glibc_sse2(x));
        }
        assert_eq!(
            h,
            [
                0xf5c8_7e41_b448_cfe7, // sin (FMA build)
                0x9eb9_47f7_4dd5_8407, // cos (FMA build)
                0x05c5_26c3_ef0e_4c95, // sincos, the sine
                0x7dc5_2ea6_9115_1c2c, // sincos, the cosine
                0x05c5_26c3_ef0e_4c95, // sin, the build without FMA
                0x7dc5_2ea6_9115_1c2c, // cos, the build without FMA
            ]
        );
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Karel Lopez Vilaret. Written for larmorx-gpl (not a translation) and relicensed
// to Apache-2.0 by its author on 2026-10-29, when the AFNI replicas moved into larmorx: the
// arithmetic of C's `long double` on x86-64, which AFNI's nifti2_io.c uses in a few places.
//! `long double` arithmetic as AFNI's x86-64 build does it, for results stored in a double.
//!
//! AFNI's NIfTI library (`nifti2_io.c`) mixes `long double` constants into some double
//! formulas: `0.25l * (r32 - r23) / a`, `r11 + r22 + r33 + 1.0l`, `1.0l - (b*b + c*c + d*d)`,
//! `2.0l * (b*c - a*d) * yd`. On x86-64 Linux, AFNI's reference platform, `long double` is
//! x87's extended format: the operation is rounded to a 64-bit significand, then, when the
//! result is stored in a double, to 53 bits, both to nearest with ties to even. This double
//! rounding gives another double than the plain double operation in the last bit of about one
//! result in 2,000 to 4,000 where the exact result needs more than 64 bits. [`add`], [`mul`]
//! and [`div`] compute the same doubles, exactly, with integer arithmetic. (`long double` is a
//! 128-bit format on aarch64 Linux and plain double on Apple silicon, so AFNI's own results
//! differ there; larmorx gives x86-64's on every platform.)
//!
//! The emulation covers normal operands and results in the double range above 2^-970 and
//! below 2^970; outside it the plain double operation is used (no such value reaches these
//! formulas from a NIfTI header).

/// The significand and exponent of a normal double: `|x| = m·2^e`, `2^52 <= m < 2^53`.
fn parts(x: f64) -> (u64, i32) {
    let bits = x.abs().to_bits();
    let m = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    (m, ((bits >> 52) & 0x7ff) as i32 - 1075)
}

/// The exact value `m·2^e` (`m > 0`, plus something below its last bit when `sticky`) rounded
/// to 64 significant bits, then to 53, both to nearest with ties to even; `None` outside the
/// range covered.
fn round_twice(negative: bool, m: u128, e: i32, sticky: bool) -> Option<f64> {
    debug_assert!(m != 0);
    let width = 128 - m.leading_zeros() as i32;
    let (mut q64, mut exponent) = if width > 64 {
        let cut = width - 64;
        let mut q = m >> cut;
        let rest = m & ((1u128 << cut) - 1);
        let half = 1u128 << (cut - 1);
        if rest > half || (rest == half && (sticky || q & 1 == 1)) {
            q += 1;
        }
        (q, e + cut)
    } else {
        // Exact in 64 bits (a quotient, the only result with a `sticky` part, has more).
        debug_assert!(!sticky);
        (m << (64 - width), e - (64 - width))
    };
    if q64 >> 64 != 0 {
        q64 >>= 1;
        exponent += 1;
    }
    // Then to 53 bits, from the 64-bit value alone.
    let low = q64 & 0x7ff;
    let mut q53 = q64 >> 11;
    if low > 0x400 || (low == 0x400 && q53 & 1 == 1) {
        q53 += 1;
    }
    exponent += 11;
    // q53 <= 2^53 converts exactly, and so does its product by a power of 2 in this range.
    if !(-1022..=970).contains(&exponent) {
        return None;
    }
    let value = q53 as f64 * f64::from_bits(((exponent + 1023) as u64) << 52);
    Some(if negative { -value } else { value })
}

/// `(double)((long double)a + b)`.
pub fn add(a: f64, b: f64) -> f64 {
    if !a.is_normal() || !b.is_normal() {
        return a + b;
    }
    let ((ma, ea), (mb, eb)) = (parts(a), parts(b));
    if (ea - eb).abs() > 72 {
        // The smaller operand lies below both roundings: the result is the larger one, as in
        // double.
        return a + b;
    }
    let low = ea.min(eb);
    let signed = |m: u64, e: i32, x: f64| {
        let v = i128::from(m) << (e - low);
        if x < 0.0 { -v } else { v }
    };
    let sum = signed(ma, ea, a) + signed(mb, eb, b);
    if sum == 0 {
        return a + b;
    }
    round_twice(sum < 0, sum.unsigned_abs(), low, false).unwrap_or(a + b)
}

/// `(double)((long double)a * b)`.
pub fn mul(a: f64, b: f64) -> f64 {
    if !a.is_normal() || !b.is_normal() {
        return a * b;
    }
    let ((ma, ea), (mb, eb)) = (parts(a), parts(b));
    let product = u128::from(ma) * u128::from(mb);
    round_twice((a < 0.0) != (b < 0.0), product, ea + eb, false).unwrap_or(a * b)
}

/// `(double)((long double)n / d)`.
pub fn div(n: f64, d: f64) -> f64 {
    if !n.is_normal() || !d.is_normal() {
        return n / d;
    }
    let ((mn, en), (md, ed)) = (parts(n), parts(d));
    // The quotient scaled by 2^75 (75 or 76 bits) and whether a remainder is left.
    let num = u128::from(mn) << 75;
    let (quotient, remainder) = (num / u128::from(md), num % u128::from(md));
    round_twice(
        (n < 0.0) != (d < 0.0),
        quotient,
        en - ed - 75,
        remainder != 0,
    )
    .unwrap_or(n / d)
}

/// `x < 1.e-7l` for a double `x`: the long double constant is closer to 10^-7 than the
/// double one, which lies below it, so the double `1e-7` itself compares less.
pub fn less_than_1e_minus_7_extended(x: f64) -> bool {
    x <= 1.0e-7
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(op, a, b, (double)((long double)a op b))` from GCC on x86-64, where the double
    /// rounding gives another double than the plain double operation. (The emulation gave
    /// GCC's double for 2 million random sums, products and quotients, 348 of them different
    /// from the plain double operation, and for 3 million quotients of the kind
    /// `dmat44_to_quatern` forms, 723 different.)
    const CASES: &[(char, u64, u64, u64)] = &[
        (
            '/',
            0x3fb199b0014069f8,
            0x3fdc4b99284c5daa,
            0x3fc3e79d963471c6,
        ),
        (
            '/',
            0xbfc6b1d9dd60484e,
            0x3fd7f77189516818,
            0xbfde4d4572af317c,
        ),
        (
            '/',
            0x3fc9dee6548ee900,
            0x3fddc9922970fad7,
            0x3fdbcad9a3560546,
        ),
        (
            '/',
            0x3f77a04319ab1640,
            0x3fd7fb4f681c8721,
            0x3f8f868284906e10,
        ),
        (
            '/',
            0xbf8e46defc5b1a80,
            0x3fe039e8c40fc190,
            0xbf9ddad120a8808a,
        ),
        (
            '+',
            0x3dbc8ca003c739e8,
            0xc01290893ad9c0c0,
            0xc01290893ad94e8e,
        ),
        (
            '+',
            0x3ff0000000000000,
            0xbf152d43efb55002,
            0x3fefff5695e08256,
        ),
        (
            '+',
            0xc2f4acd7e3645701,
            0x3fd9ffec694a76d0,
            0xc2f4acd7e36456fa,
        ),
        (
            '*',
            0x40f690dd98943c77,
            0xc034070d3c240fdc,
            0xc13c3f070a3233b4,
        ),
        (
            '*',
            0xc3c4f1eab8d2af22,
            0xbd0b406f3d8b94f9,
            0x40e1d64ab0d21e04,
        ),
        (
            '*',
            0x3fdf41c88a75757c,
            0x3ff74ce07c4ff0fc,
            0x3fe6c25fa5b60b82,
        ),
    ];

    #[test]
    fn double_rounding_as_on_x86_64() {
        for &(op, a, b, want) in CASES {
            let (a, b) = (f64::from_bits(a), f64::from_bits(b));
            let got = match op {
                '+' => add(a, b),
                '*' => mul(a, b),
                _ => div(a, b),
            };
            assert_eq!(got.to_bits(), want, "{a} {op} {b}");
        }
    }

    #[test]
    fn exact_and_simple_results_are_the_double_ones() {
        for (a, b) in [(1.0, 3.0), (-0.3, 0.7), (0.5, 0.25), (1.0, 1.0), (0.0, 2.0)] {
            assert_eq!(div(a, b), a / b, "{a} / {b}");
            assert_eq!(mul(a, b), a * b, "{a} * {b}");
            assert_eq!(add(a, b), a + b, "{a} + {b}");
        }
        assert_eq!(add(1.0, -1.0), 0.0);
        assert_eq!(add(1.0e300, 1.0e-300), 1.0e300);
        assert!(less_than_1e_minus_7_extended(1.0e-7));
        assert!(!less_than_1e_minus_7_extended(1.0000000000000001e-7));
    }
}

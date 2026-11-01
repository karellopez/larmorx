// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Written for the translation of FSL
// 6.0.7.17; it holds no FSL code.
//! C and C++ number semantics as FSL's x86-64 binaries have them: conversions of floating
//! values to integers, and the text C++ streams and `printf` write for numbers.
//!
//! **Conversions.** C leaves the conversion of an out-of-range or NaN floating value to an
//! integer undefined; on x86-64 GCC emits `cvttss2si`/`cvttsd2si`, which truncate toward zero
//! and give the "integer indefinite" value (the most negative integer of the register's width)
//! for NaN and out-of-range inputs. A conversion to `unsigned int` uses the 64-bit instruction
//! and keeps the low 32 bits. Rust's `as` saturates instead and maps NaN to 0, so code that a
//! NaN or a huge value can reach uses these functions.
//!
//! **Text to numbers.** [`strtod`] (the longest prefix that reads as a number, and where it
//! stops), [`strtof`], [`atof`], [`atoi`] and [`strtol`], in glibc's "C" locale. Decimal
//! conversions are exact (glibc is correctly rounded both ways; so are Rust's `parse` and
//! `format`, ties to even).
//!
//! **Text.** `std::ostream << double` with the default flags writes `printf("%.*g", 6, v)`;
//! `std::fixed` gives `"%.*f"`. glibc prints the exact binary value rounded to nearest, ties
//! to even, as Rust's formatting does; only the shape of the output differs, which
//! [`fmt_g`] and [`fmt_f`] reproduce (exponents with a sign and at least two digits, trailing
//! zeros removed by `%g`, `nan`, `-nan`, `inf`).

/// `(int) x` for a `float` on x86-64 (`cvttss2si`, 32-bit).
#[inline]
pub fn f32_to_i32(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `(int) x` for a `double` on x86-64 (`cvttsd2si`, 32-bit).
#[inline]
pub fn f64_to_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `(int64_t) x` or `(long) x` for a `float` on x86-64 (`cvttss2si`, 64-bit).
#[inline]
pub fn f32_to_i64(x: f32) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// `(int64_t) x` for a `double` on x86-64 (`cvttsd2si`, 64-bit).
#[inline]
pub fn f64_to_i64(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// `(unsigned int) x` for a `float` on x86-64: the 64-bit conversion, low 32 bits kept.
#[inline]
pub fn f32_to_u32(x: f32) -> u32 {
    f32_to_i64(x) as u32
}

/// `(short) x`, `(char) x` and other narrow conversions of a `float` on x86-64: the 32-bit
/// conversion, low bits kept (GCC converts to `int` and stores the low bytes).
#[inline]
pub fn f32_to_i16(x: f32) -> i16 {
    f32_to_i32(x) as i16
}

/// `(char) x` of a `float` on x86-64 (signed `char`; the low byte of the 32-bit conversion).
#[inline]
pub fn f32_to_i8(x: f32) -> i8 {
    f32_to_i32(x) as i8
}

/// C's `isspace` in the "C" locale.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `strtod(s, &end)`: the value and the number of bytes read (0 when no number starts `s`).
pub fn strtod(s: &str) -> (f64, usize) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_space(b[i]) {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let lower = |k: usize, n: usize| -> String {
        b[k..(k + n).min(b.len())]
            .iter()
            .map(|c| c.to_ascii_lowercase() as char)
            .collect()
    };
    let sign = |v: f64| if neg { -v } else { v };
    // inf, infinity
    if lower(i, 3) == "inf" {
        let n = if lower(i, 8) == "infinity" { 8 } else { 3 };
        return (sign(f64::INFINITY), i + n);
    }
    // nan, nan(n-char-sequence)
    if lower(i, 3) == "nan" {
        let mut j = i + 3;
        if j < b.len() && b[j] == b'(' {
            let mut k = j + 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                k += 1;
            }
            if k < b.len() && b[k] == b')' {
                j = k + 1;
            }
        }
        let nan = if neg { -f64::NAN } else { f64::NAN };
        return (nan, j);
    }
    // hexadecimal
    if i + 1 < b.len() && b[i] == b'0' && (b[i + 1] == b'x' || b[i + 1] == b'X') {
        if let Some((v, end)) = hex_float(b, i + 2) {
            return (sign(v), end);
        }
        // "0x" without hex digits reads as 0, ending after the 0
        return (sign(0.0), i + 1);
    }
    // decimal
    let mut j = i;
    let mut digits = 0;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return (0.0, 0);
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        if k < b.len() && b[k].is_ascii_digit() {
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            j = k;
        }
    }
    let text = &s[start..j];
    let text = text.strip_prefix('+').unwrap_or(text);
    let v: f64 = text.parse().unwrap_or(0.0);
    (v, j)
}

/// A hexadecimal significand and optional binary exponent from `b[i..]`, correctly rounded.
fn hex_float(b: &[u8], i: usize) -> Option<(f64, usize)> {
    let mut j = i;
    let mut mant: u128 = 0;
    let mut exp: i64 = 0;
    let mut any = false;
    let mut sticky = false;
    let mut push = |d: u32, frac: bool, mant: &mut u128, exp: &mut i64| {
        if *mant >> 120 == 0 {
            *mant = (*mant << 4) | u128::from(d);
            if frac {
                *exp -= 4;
            }
        } else {
            sticky |= d != 0;
            if !frac {
                *exp += 4;
            }
        }
    };
    while j < b.len() && b[j].is_ascii_hexdigit() {
        push(hex(b[j]), false, &mut mant, &mut exp);
        j += 1;
        any = true;
    }
    if j < b.len() && b[j] == b'.' {
        let mut k = j + 1;
        let mut frac_any = false;
        while k < b.len() && b[k].is_ascii_hexdigit() {
            push(hex(b[k]), true, &mut mant, &mut exp);
            k += 1;
            frac_any = true;
        }
        if any || frac_any {
            j = k;
            any = true;
        }
    }
    if !any {
        return None;
    }
    if j < b.len() && (b[j] == b'p' || b[j] == b'P') {
        let mut k = j + 1;
        let mut eneg = false;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            eneg = b[k] == b'-';
            k += 1;
        }
        if k < b.len() && b[k].is_ascii_digit() {
            let mut e: i64 = 0;
            while k < b.len() && b[k].is_ascii_digit() {
                e = (e * 10 + i64::from(b[k] - b'0')).min(1 << 40);
                k += 1;
            }
            exp += if eneg { -e } else { e };
            j = k;
        }
    }
    Some((compose(mant, exp, sticky), j))
}

fn hex(c: u8) -> u32 {
    (c as char).to_digit(16).unwrap_or(0)
}

/// `mant * 2^exp` rounded to the nearest double (ties to even; `sticky`: bits cut below).
fn compose(mant: u128, exp: i64, sticky: bool) -> f64 {
    if mant == 0 {
        return 0.0;
    }
    let len = 128 - i64::from(mant.leading_zeros());
    let top = exp + len - 1;
    if top > 1023 {
        return f64::INFINITY;
    }
    let lsb = (top - 52).max(-1074);
    let shift = lsb - exp;
    let (mut keep, up) = if shift <= 0 {
        (mant << (-shift).min(127), false)
    } else if shift >= 128 {
        (0, false)
    } else {
        let keep = mant >> shift;
        let rem = mant & ((1u128 << shift) - 1);
        let half = 1u128 << (shift - 1);
        let up = rem > half || (rem == half && (sticky || keep & 1 == 1));
        (keep, up)
    };
    keep += u128::from(up);
    (keep as f64) * 2f64.powi(lsb.clamp(-1074, 1023) as i32)
}

/// `strtof(s, &end)` for a decimal number (what C++'s `>> float` hands it): the value rounded
/// once to `float`, and the bytes read. Other forms (hexadecimal, `inf`, `nan`) go through
/// `strtod`'s value.
pub fn strtof(s: &str) -> (f32, usize) {
    let (v, used) = strtod(s);
    if used == 0 {
        return (0.0, 0);
    }
    let text = s[..used].trim_start_matches(|c: char| c.is_ascii() && is_space(c as u8));
    let decimal = text
        .bytes()
        .all(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.' | b'e' | b'E'));
    match text.parse::<f32>() {
        Ok(f) if decimal => (f, used),
        _ => (v as f32, used),
    }
}

/// `atof(s)`: `strtod` without the end.
pub fn atof(s: &str) -> f64 {
    strtod(s).0
}

/// `atoi(s)` as glibc computes it (`(int) strtol(s, NULL, 10)`: saturating at `long`'s range,
/// then the low 32 bits).
pub fn atoi(s: &str) -> i32 {
    strtol(s, 10).0 as i32
}

/// `strtol(s, &end, base)` for base 10 or 0 (decimal, octal `0`, hexadecimal `0x`): the value,
/// saturated, and the bytes read.
pub fn strtol(s: &str, base: u32) -> (i64, usize) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_space(b[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let mut radix = base;
    if (base == 0 || base == 16)
        && i + 1 < b.len()
        && b[i] == b'0'
        && (b[i + 1] == b'x' || b[i + 1] == b'X')
        && i + 2 < b.len()
        && b[i + 2].is_ascii_hexdigit()
    {
        radix = 16;
        i += 2;
    } else if base == 0 {
        radix = if i < b.len() && b[i] == b'0' { 8 } else { 10 };
    }
    let start = i;
    let mut acc: i128 = 0;
    while i < b.len() {
        let Some(d) = (b[i] as char).to_digit(radix) else {
            break;
        };
        acc = (acc * i128::from(radix) + i128::from(d)).min(i128::from(i64::MAX) + 1);
        i += 1;
    }
    if i == start {
        return (0, 0);
    }
    let v = if neg { -acc } else { acc };
    let v = v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    (v, i)
}

/// `printf("%.*f", prec, v)` (glibc).
pub fn fmt_f(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    format!("{v:.prec$}")
}

/// `printf("%.*e", prec, v)` (glibc): `d.ddde±XX`, at least two exponent digits.
pub fn fmt_e(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    let s = format!("{v:.prec$e}");
    let (mant, exp) = s.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent digits");
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{mant}e{sign}{:02}", exp.abs())
}

/// `printf("%.*g", prec, v)` (glibc), which is also what `std::ostream << v` writes with the
/// stream's default flags and precision `prec`.
pub fn fmt_g(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    let p = prec.max(1);
    // The exponent of the value once rounded to p significant digits.
    let e_form = format!("{v:.prec$e}", prec = p - 1);
    let (_, exp) = e_form.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent digits");
    let body = if x < -4 || x >= p as i32 {
        fmt_e(v, p - 1)
    } else {
        let frac = (p as i32 - 1 - x) as usize;
        format!("{v:.frac$}")
    };
    strip_zeros(&body)
}

/// `%g`'s removal of trailing zeros (and of a bare decimal point), mantissa only.
fn strip_zeros(s: &str) -> String {
    let (mant, exp) = match s.find('e') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    };
    let mant = if mant.contains('.') {
        mant.trim_end_matches('0').trim_end_matches('.')
    } else {
        mant
    };
    format!("{mant}{exp}")
}

/// glibc's text for NaN and infinities (`nan`, `-nan`, `inf`, `-inf`).
fn special(v: f64) -> Option<String> {
    if v.is_nan() {
        Some(if v.is_sign_negative() { "-nan" } else { "nan" }.to_owned())
    } else if v.is_infinite() {
        Some(if v < 0.0 { "-inf" } else { "inf" }.to_owned())
    } else {
        None
    }
}

/// `std::ostream << v` for a `double` or a `float` (promoted) with the default flags and the
/// default precision 6.
pub fn stream(v: f64) -> String {
    fmt_g(v, 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_follow_x86() {
        assert_eq!(f32_to_i32(f32::NAN), i32::MIN);
        assert_eq!(f32_to_i32(3e9), i32::MIN);
        assert_eq!(f32_to_i32(-2.7), -2);
        assert_eq!(f32_to_i64(f32::NAN), i64::MIN);
        assert_eq!(f32_to_u32(f32::NAN), 0);
        assert_eq!(f32_to_u32(-1.0), u32::MAX);
        assert_eq!(f32_to_u32(7.9), 7);
        assert_eq!(f32_to_i16(70000.0), (70000_i32) as i16);
        assert_eq!(f64_to_i32(-2147483648.5), i32::MIN);
        assert_eq!(f64_to_i32(-2147483648.0), i32::MIN);
    }

    #[test]
    fn strtod_prefixes() {
        assert_eq!(strtod("0.20"), (0.2, 4));
        assert_eq!(strtod(" 1e3x"), (1000.0, 4));
        assert_eq!(strtod("-.5"), (-0.5, 3));
        assert_eq!(strtod("1e"), (1.0, 1));
        assert_eq!(strtod("abc"), (0.0, 0));
        assert_eq!(strtod(""), (0.0, 0));
        assert_eq!(strtod("0x1p-2"), (0.25, 6));
        assert_eq!(strtod("0x"), (0.0, 1));
        assert_eq!(strtod("INFINITY").0, f64::INFINITY);
        assert_eq!(strtod("-inf"), (f64::NEG_INFINITY, 4));
        assert!(strtod("nan(12)").0.is_nan());
        assert_eq!(strtod("nan(12)").1, 7);
    }

    #[test]
    fn integers() {
        assert_eq!(atoi(" 42abc"), 42);
        assert_eq!(atoi("-7"), -7);
        assert_eq!(atoi("x"), 0);
        assert_eq!(strtol("0x1F", 0), (31, 4));
        assert_eq!(strtol("017", 0), (15, 3));
    }

    #[test]
    fn printf_shapes() {
        assert_eq!(fmt_g(0.0, 6), "0");
        assert_eq!(fmt_g(-0.0, 6), "-0");
        assert_eq!(fmt_g(1.0, 6), "1");
        assert_eq!(fmt_g(0.1, 6), "0.1");
        assert_eq!(fmt_g(123456.0, 6), "123456");
        assert_eq!(fmt_g(1234567.0, 6), "1.23457e+06");
        assert_eq!(fmt_g(0.0001, 6), "0.0001");
        assert_eq!(fmt_g(0.00001234, 6), "1.234e-05");
        assert_eq!(fmt_g(999999.5, 6), "1e+06");
        assert_eq!(fmt_g(-2.25346789, 6), "-2.25347");
        assert_eq!(fmt_g(1e-300, 6), "1e-300");
        assert_eq!(fmt_g(f64::NAN, 6), "nan");
        assert_eq!(fmt_g(-f64::NAN, 6), "-nan");
        assert_eq!(fmt_g(f64::NEG_INFINITY, 6), "-inf");
        assert_eq!(fmt_f(1.0, 6), "1.000000");
        assert_eq!(fmt_f(-0.0, 6), "-0.000000");
        assert_eq!(fmt_f(-1e-9, 6), "-0.000000");
        assert_eq!(fmt_f(2.5e-7, 6), "0.000000");
        assert_eq!(fmt_f(5e-7, 6), "0.000000");
        assert_eq!(fmt_f(5.000001e-7, 6), "0.000001");
        assert_eq!(fmt_e(12345.678, 3), "1.235e+04");
    }
}

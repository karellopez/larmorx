// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The C library's text conversions as FSL's programs use them (glibc, the "C" locale):
//! `strtod` (the longest prefix that reads as a number, and where it stops), `atof`, `atoi`,
//! and `printf`'s `%g`, `%f` and `%e` (which C++ streams use to print numbers: `cout << x` is
//! `%.6g`, and `fixed` with a precision is `%.<p>f`). Decimal conversions are exact (glibc is
//! correctly rounded both ways; so are Rust's `parse` and `format`, ties to even).

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

/// `printf("%.<prec>e", v)`.
pub fn fmt_e(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return non_finite(v);
    }
    let s = format!("{:.*e}", prec, v);
    // Rust: "1.23456e5", "-1.2e-7"; C: "1.23456e+05", "-1.2e-07"
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// `printf("%.<prec>f", v)`.
pub fn fmt_f(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return non_finite(v);
    }
    format!("{:.*}", prec, v)
}

/// `printf("%.<prec>g", v)` (C++ streams' default formatting with precision `prec`).
pub fn fmt_g(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return non_finite(v);
    }
    let p = if prec == 0 { 1 } else { prec };
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let e_form = format!("{:.*e}", p - 1, v);
    let x: i32 = e_form
        .split_once('e')
        .map_or(0, |(_, e)| e.parse().unwrap_or(0));
    let mut s = if (p as i32) > x && x >= -4 {
        format!("{:.*}", (p as i32 - 1 - x) as usize, v)
    } else {
        fmt_e(v, p - 1)
    };
    // remove trailing zeros of the fraction, and a trailing point
    let (body, exp) = match s.find('e') {
        Some(k) => (s[..k].to_owned(), s[k..].to_owned()),
        None => (s.clone(), String::new()),
    };
    let body = if body.contains('.') {
        body.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        body
    };
    s = body + &exp;
    s
}

/// `cout << v` for a double or float: `%g` with precision 6.
pub fn g6(v: f64) -> String {
    fmt_g(v, 6)
}

fn non_finite(v: f64) -> String {
    if v.is_nan() {
        // glibc prints the sign bit of a NaN
        if v.is_sign_negative() {
            "-nan".into()
        } else {
            "nan".into()
        }
    } else if v > 0.0 {
        "inf".into()
    } else {
        "-inf".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn printf_formats() {
        assert_eq!(g6(65.42545), "65.4254");
        assert_eq!(g6(654.255), "654.255");
        assert_eq!(g6(0.0), "0");
        assert_eq!(g6(1e-5), "1e-05");
        assert_eq!(g6(123456789.0), "1.23457e+08");
        assert_eq!(g6(100000.0), "100000");
        assert_eq!(g6(1000000.0), "1e+06");
        assert_eq!(g6(0.0001), "0.0001");
        assert_eq!(g6(-2.5), "-2.5");
        assert_eq!(fmt_f(1.5, 6), "1.500000");
        assert_eq!(fmt_e(12345.678, 2), "1.23e+04");
        assert_eq!(g6(f64::NAN), "nan");
    }
}

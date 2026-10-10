// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The C library's number conversions as AFNI uses them: `strtod`, `atoi`, `sscanf("%f")`,
//! `sscanf("%d")` and `printf("%g")`.
//!
//! These follow the C standard and glibc's behaviour for decimal numbers, infinities and NaNs.
//! Hexadecimal floating-point numbers (`0x1p3`), which glibc also reads, are not supported:
//! only their leading `0` is read.

/// The length of the longest prefix of `b` that `strtod` reads as a number, after skipping
/// leading white space, and the white space skipped. `None` if there is no number.
fn number_prefix(b: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < b.len() && is_space(b[i]) {
        i += 1;
    }
    let start = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let lower = |k: usize, word: &str| {
        b.len() >= k + word.len() && b[k..k + word.len()].eq_ignore_ascii_case(word.as_bytes())
    };
    if lower(i, "infinity") {
        return Some((start, i + 8 - start));
    }
    if lower(i, "inf") {
        return Some((start, i + 3 - start));
    }
    if lower(i, "nan") {
        let mut j = i + 3;
        // "nan(n-char-sequence)" is one token.
        if j < b.len() && b[j] == b'(' {
            let mut k = j + 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                k += 1;
            }
            if k < b.len() && b[k] == b')' {
                j = k + 1;
            }
        }
        return Some((start, j - start));
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - digits_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - frac;
    }
    if digits == 0 {
        return None;
    }
    // An exponent counts only if it has digits.
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    Some((start, i - start))
}

/// C's `isspace` in the "C" locale.
pub(crate) fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// Parses the text `strtod` matched (Rust's parser accepts the same decimal, `inf`,
/// `infinity` and `nan` forms; a `nan(...)` payload is dropped).
fn parse_matched<T: std::str::FromStr + Copy>(t: &str, nan: T) -> Option<T> {
    let body = t.split('(').next().unwrap_or(t);
    let unsigned = body.trim_start_matches(['+', '-']);
    if unsigned.len() >= 3 && unsigned[..3].eq_ignore_ascii_case("nan") {
        return Some(nan);
    }
    body.parse().ok()
}

/// `strtod(s, &end)`: the value and the number of bytes read (0 and 0 if `s` does not start
/// with a number). Rounded correctly, as glibc does.
pub fn strtod(s: &str) -> (f64, usize) {
    match number_prefix(s.as_bytes()) {
        Some((skip, len)) => {
            let t = &s[skip..skip + len];
            let v = parse_matched(t, f64::NAN).unwrap_or(0.0);
            // glibc's NaN from "-nan" keeps the sign; the sign does not matter to AFNI.
            (v, skip + len)
        }
        None => (0.0, 0),
    }
}

/// `sscanf(s, "%f", &v)`: the float, rounded once from the decimal text (as glibc's
/// `strtof`), or `None` when nothing matches.
pub fn scanf_f32(s: &str) -> Option<f32> {
    let (skip, len) = number_prefix(s.as_bytes())?;
    parse_matched(&s[skip..skip + len], f32::NAN)
}

/// `sscanf(s, "%lf", &v)`: the double, or `None` when nothing matches.
pub fn scanf_f64(s: &str) -> Option<f64> {
    let (skip, len) = number_prefix(s.as_bytes())?;
    parse_matched(&s[skip..skip + len], f64::NAN)
}

/// `atoi(s)` / `sscanf(s, "%d")` on the leading integer: `None` when there is none.
/// Values beyond the `int` range saturate (glibc's `strtol` saturates at the `long` range;
/// AFNI never reaches it).
pub fn scan_int(s: &str) -> Option<(i64, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_space(b[i]) {
        i += 1;
    }
    let start = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let d = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == d {
        return None;
    }
    let v =
        s[start..i]
            .parse::<i64>()
            .unwrap_or(if b[start] == b'-' { i64::MIN } else { i64::MAX });
    Some((v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)), i))
}

/// `printf("%g", x)`: six significant digits, trailing zeros removed, exponent form below
/// 1e-4 and from 1e6.
pub fn fmt_g(x: f64) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan" } else { "nan" }.to_owned();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    // The exponent after rounding to 6 significant digits.
    let sci = format!("{x:.5e}");
    let (mantissa, exp) = sci.split_once('e').expect("exponent form");
    let exp: i32 = exp.parse().expect("an exponent");
    if !(-4..6).contains(&exp) {
        let mantissa = trim_zeros(mantissa);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exp.abs())
    } else {
        let decimals = (5 - exp) as usize;
        trim_zeros(&format!("{x:.decimals$}")).to_owned()
    }
}

fn trim_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strtod_prefixes() {
        assert_eq!(strtod("2s"), (2.0, 1));
        assert_eq!(strtod("2000ms"), (2000.0, 4));
        assert_eq!(strtod(" 1.5e0s"), (1.5, 6));
        assert_eq!(strtod("2e"), (2.0, 1));
        assert_eq!(strtod(".5"), (0.5, 2));
        assert_eq!(strtod("abc"), (0.0, 0));
        assert_eq!(strtod("-"), (0.0, 0));
        assert_eq!(strtod("inf"), (f64::INFINITY, 3));
        assert_eq!(strtod("-Infinity!"), (f64::NEG_INFINITY, 9));
        assert!(strtod("nan(1)x").0.is_nan());
        assert_eq!(strtod("nan(1)x").1, 6);
        assert_eq!(strtod("0x10").1, 1);
    }

    #[test]
    fn scanf_and_ints() {
        assert_eq!(scanf_f32("0.1"), Some(0.1f32));
        assert_eq!(scanf_f32("1.5abc"), Some(1.5));
        assert_eq!(scanf_f32("abc"), None);
        assert_eq!(scan_int("3@2"), Some((3, 1)));
        assert_eq!(scan_int("-7"), Some((-7, 2)));
        assert_eq!(scan_int("1.9"), Some((1, 1)));
        assert_eq!(scan_int("x"), None);
    }

    #[test]
    fn printf_g() {
        for (x, s) in [
            (0.0, "0"),
            (2.5, "2.5"),
            (-0.5, "-0.5"),
            (1.0, "1"),
            (0.66666667, "0.666667"),
            (123456.0, "123456"),
            (1234567.0, "1.23457e+06"),
            (0.0001, "0.0001"),
            (0.00001234, "1.234e-05"),
            (999999.5, "1e+06"),
            (2.0000001, "2"),
        ] {
            assert_eq!(fmt_g(x), s, "{x}");
        }
    }
}

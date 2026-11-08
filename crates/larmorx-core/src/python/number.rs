// SPDX-License-Identifier: Apache-2.0 AND PSF-2.0
//! Python's `float(str)`, `int(str)` (base 10), `repr(float)` and `format(float, '.Nf')`
//! (CPython 3.12.16; module documentation in [`super`]).

/// Python's `str.strip()` whitespace for numbers: the characters `_PyUnicode_TransformDecimal
/// AndSpaceToASCII` turns into a space (Unicode whitespace) and ASCII's `Py_ISSPACE`.
fn is_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\x1c'..='\x1f')
}

/// `_Py_string_to_number_with_underscores`' rule: an underscore only between two digits.
/// Returns the text without its underscores, or `None` when one is misplaced.
fn without_underscores(s: &str) -> Option<String> {
    if !s.contains('_') {
        return Some(s.to_owned());
    }
    let mut out = String::with_capacity(s.len());
    let mut prev = '\0';
    for c in s.chars() {
        if c == '_' {
            if !prev.is_ascii_digit() {
                return None;
            }
        } else {
            if prev == '_' && !c.is_ascii_digit() {
                return None;
            }
            out.push(c);
        }
        prev = c;
    }
    (prev != '_').then_some(out)
}

/// `float(s)` for a `str`: `None` where Python raises `ValueError` ("could not convert string
/// to float"). Unicode decimal digits other than ASCII's are not converted (a documented
/// difference: Python accepts them).
pub fn parse_float(s: &str) -> Option<f64> {
    let s = without_underscores(s)?;
    let t = s.trim_matches(is_space);
    if t.is_empty() {
        return None;
    }
    // Python's grammar (`_Py_dg_strtod` and `_Py_parse_inf_or_nan`) is Rust's: an optional
    // sign, then `inf`, `infinity` or `nan` in any case, or digits with an optional point and
    // exponent; both round correctly. Rust's parser accepts nothing Python refuses.
    t.parse::<f64>().ok()
}

/// `int(s)` (base 10) for a `str`: `None` where Python raises `ValueError` ("invalid literal for
/// int() with base 10", or more than 4,300 digits). Values beyond `i128` saturate (Python's are
/// unbounded).
pub fn parse_int(s: &str) -> Option<i128> {
    let t = s.trim_matches(is_space);
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'+') => (false, &t[1..]),
        Some(b'-') => (true, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() || digits.starts_with('_') {
        return None;
    }
    let d = without_underscores(digits)?;
    if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) || d.len() > 4300 {
        return None;
    }
    let mut v: i128 = 0;
    for b in d.bytes() {
        v = v.saturating_mul(10).saturating_add(i128::from(b - b'0'));
    }
    Some(if neg { v.saturating_neg() } else { v })
}

/// The shortest digits of `x` (finite, not zero is not required) and its decimal point
/// position `decpt` (`x = 0.d1d2... × 10^decpt`), as `_Py_dg_dtoa` mode 0 gives them.
fn shortest_digits(x: f64) -> (String, i32) {
    let e = format!("{:e}", x.abs());
    let (mant, exp) = e.split_once('e').expect("exponent form");
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let exp: i32 = exp.parse().expect("exponent");
    (digits, exp + 1)
}

/// Python's `repr(x)` (and `str(x)`) of a float: the shortest digits that read back as `x`,
/// positional when the decimal exponent is in `[-4, 16)`, else `d.ddde±XX`.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x == 0.0 {
        return format!("{sign}0.0");
    }
    let (digits, decpt) = shortest_digits(x);
    let n = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        let exp = decpt - 1;
        let mant = if n > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        let esign = if exp < 0 { '-' } else { '+' };
        format!("{sign}{mant}e{esign}{:02}", exp.abs())
    } else if decpt <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt >= n {
        format!("{sign}{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let (a, b) = digits.split_at(decpt as usize);
        format!("{sign}{a}.{b}")
    }
}

/// Python's `format(x, f".{prec}f")` (also `f"{x:.{prec}f}"` and `"%.{prec}f" % x`): the
/// exact value rounded to `prec` decimals, ties to even (`_Py_dg_dtoa` mode 3).
pub fn format_fixed(x: f64, prec: usize) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    // Rust rounds the exact binary value to the nearest, ties to even, as Python does.
    format!("{x:.prec$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_parse_as_python() {
        assert_eq!(parse_float(" 0.015 "), Some(0.015));
        assert_eq!(parse_float("1_000.5"), Some(1000.5));
        assert_eq!(parse_float("1__0"), None);
        assert_eq!(parse_float("_1"), None);
        assert_eq!(parse_float("1_"), None);
        assert_eq!(parse_float("1._5"), None);
        assert_eq!(parse_float("-Infinity").map(f64::is_infinite), Some(true));
        assert!(parse_float("nan").is_some_and(f64::is_nan));
        assert_eq!(parse_float("1e"), None);
        assert_eq!(parse_float("1.e5"), Some(1e5));
        assert_eq!(parse_float(".5"), Some(0.5));
        assert_eq!(parse_float("."), None);
        assert_eq!(parse_float("0x10"), None);
        assert_eq!(parse_float(""), None);
        assert_eq!(parse_float("1e500"), Some(f64::INFINITY));
    }

    #[test]
    fn ints_parse_as_python() {
        assert_eq!(parse_int(" 007 "), Some(7));
        assert_eq!(parse_int("-1_000"), Some(-1000));
        assert_eq!(parse_int("+3"), Some(3));
        assert_eq!(parse_int("- 3"), None);
        assert_eq!(parse_int("3.0"), None);
        assert_eq!(parse_int("1__0"), None);
        assert_eq!(parse_int("_1"), None);
        assert_eq!(parse_int(""), None);
    }

    #[test]
    fn reprs_as_python() {
        for (x, r) in [
            (0.1, "0.1"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (-0.0, "-0.0"),
            (123.456, "123.456"),
            (1.5e-300, "1.5e-300"),
            (f64::MAX, "1.7976931348623157e+308"),
            (5e-324, "5e-324"),
            (14.0, "14.0"),
            (66.0, "66.0"),
        ] {
            assert_eq!(float_repr(x), r);
        }
        assert_eq!(format_fixed(0.125, 2), "0.12");
        assert_eq!(format_fixed(0.375, 2), "0.38");
        assert_eq!(format_fixed(2.5, 0), "2");
    }
}

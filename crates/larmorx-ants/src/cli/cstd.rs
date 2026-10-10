// SPDX-License-Identifier: Apache-2.0
//! The C and C++ standard-library conversions that ANTs programs apply to their arguments and
//! their printed values, reproduced so that larmorx reads and prints exactly as they do
//! (glibc and libstdc++, as the ANTs oracle is built on Linux).
//!
//! - [`atof`]: C's `atof` (re-exported from the parser).
//! - [`stoi`]: `std::stoi`, which throws (and so aborts an ANTs program) when there is no
//!   number.
//! - [`from_string_f32`]: ANTs' `from_string<float>` (`istringstream >> std::dec >> value`,
//!   then "is the stream at its end?"), which ImageMath uses to decide whether an operand is a
//!   number or an image file.
//! - [`format_g`]: `std::cout << value` for `float` and `double` (`%g` with 6 significant
//!   digits).

pub use super::parser::atof;

/// `std::stoi(s)`: leading whitespace, an optional sign and decimal digits; anything after is
/// ignored. `None` where `std::stoi` throws (no digits, or out of `int` range).
pub fn stoi(s: &str) -> Option<i32> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let bytes = t.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits_from = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == digits_from {
        return None;
    }
    t[..end]
        .parse::<i64>()
        .ok()
        .and_then(|v| i32::try_from(v).ok())
}

/// What ANTs' `from_string<float>(value, s, std::dec)` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamFloat {
    /// The function's result: whether the stream reached its end, which ANTs takes to mean
    /// "this is a number".
    pub is_number: bool,
    /// The value stored into the target: `None` when the stream had nothing but whitespace
    /// (the target keeps its previous value), `0` when the characters do not form a number,
    /// `±f32::MAX` on overflow (libstdc++'s rules).
    pub value: Option<f32>,
}

/// libstdc++'s `operator>>(float&)` on an `istringstream` of `s`, then `eof()`.
///
/// The characters a float may contain are collected as `num_get::_M_extract_float` does (an
/// optional sign, digits, one decimal point, one exponent after some digits, with its own
/// sign); the stream is at its end only if every character was taken. The collected text is
/// converted as `strtof` does, and must be consumed entirely.
pub fn from_string_f32(s: &str) -> StreamFloat {
    let is_space = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_space(bytes[i]) {
        i += 1;
    }
    if i == bytes.len() {
        // The sentry hits the end while skipping whitespace: eof and fail, value untouched.
        return StreamFloat {
            is_number: true,
            value: None,
        };
    }
    let mut text = String::new();
    if matches!(bytes[i], b'+' | b'-') {
        text.push(bytes[i] as char);
        i += 1;
    }
    let mut mantissa = false;
    let mut dec = false;
    let mut sci = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'.' && !dec && !sci {
            text.push('.');
            dec = true;
        } else if (c == b'e' || c == b'E') && !sci && mantissa {
            text.push('e');
            sci = true;
            if i + 1 < bytes.len() && matches!(bytes[i + 1], b'+' | b'-') {
                i += 1;
                text.push(bytes[i] as char);
            }
        } else if c.is_ascii_digit() {
            text.push(c as char);
            mantissa = true;
        } else {
            break;
        }
        i += 1;
    }
    StreamFloat {
        is_number: i == bytes.len(),
        value: Some(strtof_whole(&text)),
    }
}

/// libstdc++'s `__convert_to_v` for float: `strtof` must consume the whole text (else 0);
/// overflow gives `±FLT_MAX`.
fn strtof_whole(text: &str) -> f32 {
    let b = text.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let mut digits = 0;
    let mut normalized = String::from(&text[..i]);
    while i < b.len() && b[i].is_ascii_digit() {
        normalized.push(b[i] as char);
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        normalized.push('.');
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            normalized.push(b[i] as char);
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if i < b.len() {
        // An exponent: it needs at least one digit after the optional sign.
        let mut j = i + 1;
        if j < b.len() && matches!(b[j], b'+' | b'-') {
            j += 1;
        }
        if j == b.len() || !b[j..].iter().all(u8::is_ascii_digit) {
            return 0.0;
        }
        normalized.push_str(&text[i..]);
    }
    if normalized.ends_with('.') {
        normalized.push('0');
    }
    if normalized.starts_with('.') || normalized.starts_with("+.") || normalized.starts_with("-.") {
        normalized = normalized.replacen('.', "0.", 1);
    }
    match normalized.parse::<f32>() {
        Ok(v) if v.is_infinite() => {
            if v > 0.0 {
                f32::MAX
            } else {
                -f32::MAX
            }
        }
        Ok(v) => v,
        Err(_) => 0.0,
    }
}

/// `std::ostream << value` with the default format: `%g` with 6 significant digits, as glibc
/// prints it (`nan`, `-nan`, `inf`, `-inf`; exponents with a sign and at least two digits).
pub fn format_g(v: f64) -> String {
    format_g_precision(v, 6)
}

/// `%.<precision>g`.
pub fn format_g_precision(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    let p = precision.max(1);
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent");
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if x < -4 || x >= p as i32 {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", strip(mantissa), sign, x.abs())
    } else {
        let decimals = (p as i32 - 1 - x).max(0) as usize;
        strip(&format!("{v:.decimals$}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stoi_follows_the_standard_library() {
        assert_eq!(stoi("3"), Some(3));
        assert_eq!(stoi("  -12abc"), Some(-12));
        assert_eq!(stoi("2.9"), Some(2));
        assert_eq!(stoi("+7"), Some(7));
        assert_eq!(stoi("abc"), None);
        assert_eq!(stoi(""), None);
        assert_eq!(stoi("-"), None);
        assert_eq!(stoi("99999999999"), None);
    }

    #[test]
    fn stream_floats() {
        let f = |s: &str| from_string_f32(s);
        assert_eq!(
            f("2"),
            StreamFloat {
                is_number: true,
                value: Some(2.0)
            }
        );
        assert_eq!(
            f("-0.5"),
            StreamFloat {
                is_number: true,
                value: Some(-0.5)
            }
        );
        assert_eq!(
            f("1e3"),
            StreamFloat {
                is_number: true,
                value: Some(1000.0)
            }
        );
        assert_eq!(
            f(".5"),
            StreamFloat {
                is_number: true,
                value: Some(0.5)
            }
        );
        assert_eq!(
            f("5."),
            StreamFloat {
                is_number: true,
                value: Some(5.0)
            }
        );
        assert_eq!(
            f("1.e2"),
            StreamFloat {
                is_number: true,
                value: Some(100.0)
            }
        );
        assert_eq!(
            f("  7"),
            StreamFloat {
                is_number: true,
                value: Some(7.0)
            }
        );
        // Empty or blank: at the end at once, value untouched.
        assert_eq!(
            f(""),
            StreamFloat {
                is_number: true,
                value: None
            }
        );
        assert_eq!(
            f("   "),
            StreamFloat {
                is_number: true,
                value: None
            }
        );
        // File names stop early.
        assert!(!f("img.nii.gz").is_number);
        assert!(!f("2.nii").is_number);
        assert!(!f("0x10").is_number);
        assert!(!f("nan").is_number);
        assert!(!f("7 ").is_number);
        // Taken entirely but not a number: 0.
        assert_eq!(
            f("1e"),
            StreamFloat {
                is_number: true,
                value: Some(0.0)
            }
        );
        assert_eq!(
            f("-"),
            StreamFloat {
                is_number: true,
                value: Some(0.0)
            }
        );
        assert_eq!(
            f("1e+"),
            StreamFloat {
                is_number: true,
                value: Some(0.0)
            }
        );
        // Overflow: FLT_MAX.
        assert_eq!(
            f("1e40"),
            StreamFloat {
                is_number: true,
                value: Some(f32::MAX)
            }
        );
        assert_eq!(f("-1e40").value, Some(-f32::MAX));
    }

    #[test]
    fn g_format() {
        assert_eq!(format_g(12292.4), "12292.4");
        assert_eq!(format_g(110631.2), "110631");
        assert_eq!(format_g(102.43567), "102.436");
        assert_eq!(format_g(1234567.0), "1.23457e+06");
        assert_eq!(format_g(0.0001), "0.0001");
        assert_eq!(format_g(0.00001234), "1.234e-05");
        assert_eq!(format_g(0.5), "0.5");
        assert_eq!(format_g(100.0), "100");
        assert_eq!(format_g(-3.0), "-3");
        assert_eq!(format_g(f64::NAN), "nan");
        assert_eq!(format_g(-f64::NAN), "-nan");
        assert_eq!(format_g(f64::INFINITY), "inf");
        assert_eq!(format_g(1e100), "1e+100");
        assert_eq!(format_g(999999.5), "1e+06");
    }
}

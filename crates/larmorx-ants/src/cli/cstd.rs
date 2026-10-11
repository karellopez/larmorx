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
//! - [`convert_vector_f32`], [`convert_vector_f64`]: ANTs' `ConvertVector` (`1.5x1x2`).
//! - [`stof`]: `std::stof`, which throws when there is no number.
//! - [`parser_convert_f32`], [`parser_convert_i32`], [`parser_convert_u32`],
//!   [`parser_convert_bool`], [`parser_convert_vector_u32`], [`parser_convert_vector_f32`]:
//!   the strict `Convert<T>` and `ConvertVector<T>` of ANTs' command-line parser, which throw
//!   (and so abort the program) unless the whole text is read.

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
    match stream_text(s) {
        None => StreamFloat {
            is_number: true,
            value: None,
        },
        Some((is_number, text)) => StreamFloat {
            is_number,
            value: Some(strtof_whole(&text)),
        },
    }
}

/// A command-line number read strictly ([`strict_f32`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrictNumber {
    /// A decimal number within `float`'s range.
    Number(f32),
    /// A decimal number beyond `float`'s range (`1e40`).
    OutOfRange,
    /// Not a decimal number.
    NotANumber,
}

/// `s` read strictly as a `float`: optional blanks, an optional sign, decimal digits with at
/// most one point (at least one digit), an optional exponent (`e` or `E`, an optional sign,
/// digits), optional blanks, and nothing else. The value is the correctly rounded `float`
/// (as `strtof` gives it). larmorx's reading of numeric operands where ANTs' own readings
/// take malformed text (ANTS-05).
pub fn strict_f32(s: &str) -> StrictNumber {
    let t = s.trim_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = t.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let mut digits = 0;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
        digits += 1;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return StrictNumber::NotANumber;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let from = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == from {
            return StrictNumber::NotANumber;
        }
    }
    if i != b.len() {
        return StrictNumber::NotANumber;
    }
    // Rust's parser takes this grammar except a point without digits before or after it.
    let text = t.replace(".e", ".0e").replace(".E", ".0E");
    let text = if text.ends_with('.') {
        format!("{text}0")
    } else {
        text
    };
    match text.parse::<f32>() {
        Ok(v) if v.is_finite() => StrictNumber::Number(v),
        Ok(_) => StrictNumber::OutOfRange,
        Err(_) => StrictNumber::NotANumber,
    }
}

/// [`from_string_f32`] for `double` (`istringstream >> double`, converted as `strtod`):
/// `(is_number, value)`, the value `None` when untouched.
pub fn from_string_f64(s: &str) -> (bool, Option<f64>) {
    match stream_text(s) {
        None => (true, None),
        Some((is_number, text)) => (is_number, Some(strtod_whole(&text))),
    }
}

/// The characters `num_get::_M_extract_float` collects from `s` and whether it reached the
/// end; `None` when there is nothing but whitespace (the sentry fails, the value is untouched).
fn stream_text(s: &str) -> Option<(bool, String)> {
    let is_space = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_space(bytes[i]) {
        i += 1;
    }
    if i == bytes.len() {
        return None;
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
    Some((i == bytes.len(), text))
}

/// ANTs' `ConvertVector<float>` (`Examples/antsUtilities.h`): the text split at every `x`,
/// each piece read with `istringstream >> float` into one variable that is reused from piece
/// to piece. A piece with nothing but whitespace leaves the variable as it was, so `1.5x`
/// gives `[1.5, 1.5]`; where the variable was never set (an empty first piece), C++ reads an
/// uninitialised value, and this returns `None`. A piece that is not a number reads as 0.
pub fn convert_vector_f32(s: &str) -> Option<Vec<f32>> {
    let mut value: Option<f32> = None;
    let mut out = Vec::new();
    for piece in s.split('x') {
        if let Some(v) = from_string_f32(piece).value {
            value = Some(v);
        }
        out.push(value?);
    }
    Some(out)
}

/// ANTs' `ConvertVector<double>`: [`convert_vector_f32`] read as `double`.
pub fn convert_vector_f64(s: &str) -> Option<Vec<f64>> {
    let mut value: Option<f64> = None;
    let mut out = Vec::new();
    for piece in s.split('x') {
        if let Some(v) = from_string_f64(piece).1 {
            value = Some(v);
        }
        out.push(value?);
    }
    Some(out)
}

/// `std::stof(s)`: glibc's `strtof` on the longest number at the start of `s` (after
/// whitespace): decimals, `inf`, `infinity`, `nan` (any case) and hexadecimal floats. `None`
/// where `std::stof` throws: no number, or a result out of range (overflow to infinity).
pub fn stof(s: &str) -> Option<f32> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = t.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let negative = b.first() == Some(&b'-');
    let rest = t[i..].to_ascii_lowercase();
    if rest.starts_with("inf") {
        return Some(if negative {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        });
    }
    if rest.starts_with("nan") {
        return Some(if negative { -f32::NAN } else { f32::NAN });
    }
    if rest.starts_with("0x") {
        // Hexadecimal: exact in double for the floats a command line holds.
        let v = super::parser::atof(t) as f32;
        return v.is_finite().then_some(v);
    }
    let from = i;
    let mut digits = 0;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
        digits += 1;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let e = j;
        while b.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        if j > e {
            i = j;
        }
    }
    let mut text = String::from(&t[..from]);
    let body = &t[from..i];
    if body.starts_with('.') {
        text.push('0');
    }
    text.push_str(body);
    let text = text.replace(".e", ".0e").replace(".E", ".0E");
    let text = if text.ends_with('.') {
        format!("{text}0")
    } else {
        text
    };
    let v: f32 = text.parse().ok()?;
    v.is_finite().then_some(v)
}

/// The text `strtof`/`strtod` would consume entirely, in a form Rust's parser reads; `None`
/// where they would not consume it all (libstdc++ then stores 0).
fn normalized_number(text: &str) -> Option<String> {
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
        return None;
    }
    if i < b.len() {
        // An exponent: it needs at least one digit after the optional sign.
        let mut j = i + 1;
        if j < b.len() && matches!(b[j], b'+' | b'-') {
            j += 1;
        }
        if j == b.len() || !b[j..].iter().all(u8::is_ascii_digit) {
            return None;
        }
        normalized.push_str(&text[i..]);
    }
    if normalized.ends_with('.') {
        normalized.push('0');
    }
    if normalized.starts_with('.') || normalized.starts_with("+.") || normalized.starts_with("-.") {
        normalized = normalized.replacen('.', "0.", 1);
    }
    Some(normalized)
}

/// libstdc++'s `__convert_to_v` for float: `strtof` must consume the whole text (else 0);
/// overflow gives `±FLT_MAX`.
fn strtof_whole(text: &str) -> f32 {
    match normalized_number(text).map(|n| n.parse::<f32>()) {
        Some(Ok(v)) if v.is_infinite() => {
            if v > 0.0 {
                f32::MAX
            } else {
                -f32::MAX
            }
        }
        Some(Ok(v)) => v,
        _ => 0.0,
    }
}

/// [`strtof_whole`] for double (`strtod`; overflow gives `±DBL_MAX`).
fn strtod_whole(text: &str) -> f64 {
    match normalized_number(text).map(|n| n.parse::<f64>()) {
        Some(Ok(v)) if v.is_infinite() => {
            if v > 0.0 {
                f64::MAX
            } else {
                -f64::MAX
            }
        }
        Some(Ok(v)) => v,
        _ => 0.0,
    }
}

/// The text `CommandLineParser::Convert` reads: trailing `" \n\r\t"` removed.
fn strip_end(s: &str) -> &str {
    s.trim_end_matches([' ', '\n', '\r', '\t'])
}

/// libstdc++'s integer extraction (`num_get::_M_extract_int`, decimal): leading whitespace,
/// an optional sign, digits. Returns `(negative, magnitude, characters read)`, or `None`
/// without digits or when the magnitude exceeds `limit`.
fn extract_int(s: &str, limit: u64) -> Option<(bool, u64, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let negative = b.get(i) == Some(&b'-');
    if matches!(b.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let from = i;
    let mut value: u64 = 0;
    while let Some(d) = b.get(i).filter(|c| c.is_ascii_digit()) {
        value = value.checked_mul(10)?.checked_add(u64::from(d - b'0'))?;
        if value > limit {
            return None;
        }
        i += 1;
    }
    (i > from).then_some((negative, value, i))
}

/// ANTs' `CommandLineParser::Convert<int>` (`Utilities/antsCommandLineParser.h:120-138`):
/// `istringstream >> int` must succeed and read every character (after trailing whitespace
/// is removed); otherwise ITK throws and ANTs aborts (`None`).
pub fn parser_convert_i32(s: &str) -> Option<i32> {
    let s = strip_end(s);
    let (negative, magnitude, read) = extract_int(s, 1 << 31)?;
    if read != s.len() || (!negative && magnitude > i32::MAX as u64) {
        return None;
    }
    let v = magnitude as i64;
    Some((if negative { -v } else { v }) as i32)
}

/// `Convert<unsigned int>`: as [`parser_convert_i32`]; a minus sign negates modulo 2³²
/// (libstdc++), so `-1` reads as 4294967295.
pub fn parser_convert_u32(s: &str) -> Option<u32> {
    let s = strip_end(s);
    let (negative, magnitude, read) = extract_int(s, u64::from(u32::MAX))?;
    if read != s.len() {
        return None;
    }
    let v = magnitude as u32;
    Some(if negative { v.wrapping_neg() } else { v })
}

/// `Convert<bool>`: an integer, 0 or 1 (`noboolalpha`); anything else throws.
pub fn parser_convert_bool(s: &str) -> Option<bool> {
    match parser_convert_i32(s)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// `Convert<float>`: `istringstream >> float` must read every character and give a finite
/// number (an overflow sets `failbit`, so ANTs throws).
pub fn parser_convert_f32(s: &str) -> Option<f32> {
    let s = strip_end(s);
    let (at_end, text) = stream_text(s)?;
    if !at_end {
        return None;
    }
    let v: f32 = normalized_number(&text)?.parse().ok()?;
    v.is_finite().then_some(v)
}

/// The pieces `ConvertVector` reads: `std::getline(stream, piece, 'x')` on the text with its
/// trailing whitespace removed (no piece after a final `x`; none at all for an empty text).
fn getline_pieces(s: &str) -> Vec<&str> {
    let s = strip_end(s);
    if s.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<&str> = s.split('x').collect();
    if s.ends_with('x') {
        pieces.pop();
    }
    pieces
}

/// `ConvertVector<unsigned int>`: `Convert<unsigned int>` of each piece; `None` if any throws.
pub fn parser_convert_vector_u32(s: &str) -> Option<Vec<u32>> {
    getline_pieces(s)
        .into_iter()
        .map(parser_convert_u32)
        .collect()
}

/// `ConvertVector<float>`: `Convert<float>` of each piece; `None` if any throws.
pub fn parser_convert_vector_f32(s: &str) -> Option<Vec<f32>> {
    getline_pieces(s)
        .into_iter()
        .map(parser_convert_f32)
        .collect()
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
    fn vectors_and_stof() {
        assert_eq!(convert_vector_f32("1.5"), Some(vec![1.5]));
        assert_eq!(convert_vector_f32("1x2.5x3"), Some(vec![1.0, 2.5, 3.0]));
        assert_eq!(convert_vector_f32("1.5x"), Some(vec![1.5, 1.5]));
        assert_eq!(convert_vector_f32("2xabc"), Some(vec![2.0, 0.0]));
        assert_eq!(convert_vector_f32(""), None);
        assert_eq!(convert_vector_f32("x1"), None);
        assert_eq!(stof("0.5"), Some(0.5));
        assert_eq!(stof("  -2e1xyz"), Some(-20.0));
        assert_eq!(stof(".5"), Some(0.5));
        assert_eq!(stof("5."), Some(5.0));
        assert_eq!(stof("1e"), Some(1.0));
        assert_eq!(stof("0x10"), Some(16.0));
        assert_eq!(stof("abc"), None);
        assert_eq!(stof("1e60"), None);
        assert_eq!(stof("0.1"), Some(0.1f32));
        assert_eq!(convert_vector_f64("0.1x2"), Some(vec![0.1, 2.0]));
        assert_eq!(from_string_f64("1e400"), (true, Some(f64::MAX)));
        assert_eq!(from_string_f64(" "), (true, None));
        assert_eq!(from_string_f64("3abc"), (false, Some(3.0)));
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

    #[test]
    fn strict_numbers() {
        use StrictNumber::*;
        assert_eq!(strict_f32("7"), Number(7.0));
        assert_eq!(strict_f32(" 7 "), Number(7.0));
        assert_eq!(strict_f32("-2.5e1"), Number(-25.0));
        assert_eq!(strict_f32(".5"), Number(0.5));
        assert_eq!(strict_f32("5."), Number(5.0));
        assert_eq!(strict_f32("+5.e1"), Number(50.0));
        assert_eq!(strict_f32("0.1"), Number(0.1));
        assert_eq!(strict_f32("1e40"), OutOfRange);
        assert_eq!(strict_f32("-1e39"), OutOfRange);
        for text in [
            "", " ", "-", "1e", "1e+", ".", "1.5x", "abc", "inf", "nan", "0x10", "1 2",
        ] {
            assert_eq!(strict_f32(text), NotANumber, "{text:?}");
        }
    }
}

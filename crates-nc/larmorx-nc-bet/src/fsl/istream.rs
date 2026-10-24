// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! C++'s formatted input (`std::ifstream >> x`, `std::getline`) as GCC's libstdc++ performs it
//! in the "C" locale, for the files FSL's programs read as text (meshes, matrices).
//!
//! What matters for a replica is which characters an extraction consumes, what it stores and
//! when the stream fails: a number is the longest prefix of the accepted characters (an optional
//! sign, digits, one decimal point, an exponent after a digit), converted with `strtod` or as a
//! base-10 integer. An extraction that finds no number stores 0 and fails the stream; one that
//! starts on a failed stream, or finds only white space before the end, fails and stores
//! nothing: the variable keeps whatever it held (in FSL's code, usually never set). Every later
//! extraction on a failed stream does nothing.

use crate::fsl::cstr;

/// What an extraction did to its variable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Got<T> {
    /// A value was read (the stream is still good).
    Value(T),
    /// The extraction ran and failed: C++ stores this value (0, or the largest value of the
    /// type on overflow) and the stream fails.
    Failed(T),
    /// The stream had failed already, or only white space was left: nothing is stored.
    Untouched,
}

impl<T: Copy> Got<T> {
    /// The value an extraction stored, if any.
    pub fn stored(self) -> Option<T> {
        match self {
            Got::Value(v) | Got::Failed(v) => Some(v),
            Got::Untouched => None,
        }
    }

    /// The value, only if it was read.
    pub fn ok(self) -> Option<T> {
        match self {
            Got::Value(v) => Some(v),
            _ => None,
        }
    }
}

/// A text input stream (`std::ifstream` opened on a whole file).
pub struct IStream<'a> {
    data: &'a [u8],
    pos: usize,
    fail: bool,
    eof: bool,
}

/// `isspace` in the "C" locale.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

impl<'a> IStream<'a> {
    pub fn new(data: &'a [u8]) -> IStream<'a> {
        IStream {
            data,
            pos: 0,
            fail: false,
            eof: false,
        }
    }

    /// Whether the stream has failed (`fail()`).
    pub fn failed(&self) -> bool {
        self.fail
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// The sentry of a formatted extraction: fails on a stream that is not good, else skips
    /// white space and fails (with end-of-file) if nothing else is left.
    fn sentry(&mut self) -> bool {
        if self.fail || self.eof {
            self.fail = true;
            return false;
        }
        while let Some(c) = self.peek() {
            if !is_space(c) {
                return true;
            }
            self.pos += 1;
        }
        self.eof = true;
        self.fail = true;
        false
    }

    /// `std::getline(stream, s)`: the characters up to the next newline (consumed, not stored).
    /// `None` (and the stream fails) when it cannot extract anything.
    pub fn getline(&mut self) -> Option<String> {
        if self.fail || self.eof {
            self.fail = true;
            return None;
        }
        let start = self.pos;
        while let Some(c) = self.peek() {
            self.pos += 1;
            if c == b'\n' {
                return Some(String::from_utf8_lossy(&self.data[start..self.pos - 1]).into_owned());
            }
        }
        self.eof = true;
        if self.pos == start {
            self.fail = true;
            return None;
        }
        Some(String::from_utf8_lossy(&self.data[start..]).into_owned())
    }

    /// `stream >> s` for a `std::string`: the next run of characters that are not white space.
    pub fn word(&mut self) -> Got<String> {
        if !self.sentry() {
            return Got::Untouched;
        }
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_space(c) {
                break;
            }
            self.pos += 1;
        }
        if self.pos == self.data.len() {
            self.eof = true;
        }
        Got::Value(String::from_utf8_lossy(&self.data[start..self.pos]).into_owned())
    }

    /// Consumes the next character (end-of-file sets the flag).
    fn advance(&mut self) -> Option<u8> {
        self.pos += 1;
        let c = self.peek();
        if c.is_none() {
            self.eof = true;
        }
        c
    }

    /// The characters `num_get` accumulates for a floating-point number (`_M_extract_float`):
    /// an optional sign, leading zeros as one, digits, one decimal point, and an exponent after
    /// a digit with its optional sign.
    fn float_chars(&mut self) -> String {
        let mut x = String::new();
        let mut c = self.peek();
        if let Some(s @ (b'+' | b'-')) = c {
            x.push(s as char);
            c = self.advance();
        }
        let mut found_mantissa = false;
        while let Some(ch) = c {
            if ch == b'0' {
                if !found_mantissa {
                    x.push('0');
                    found_mantissa = true;
                }
                c = self.advance();
            } else {
                break;
            }
        }
        let (mut found_dec, mut found_sci) = (false, false);
        while let Some(ch) = c {
            if ch.is_ascii_digit() {
                x.push(ch as char);
                found_mantissa = true;
            } else if ch == b'.' && !found_dec && !found_sci {
                x.push('.');
                found_dec = true;
            } else if (ch == b'e' || ch == b'E') && !found_sci && found_mantissa {
                x.push('e');
                found_sci = true;
                c = self.advance();
                match c {
                    Some(s @ (b'+' | b'-')) => x.push(s as char),
                    Some(_) => continue,
                    None => break,
                }
            } else {
                break;
            }
            c = self.advance();
        }
        x
    }

    /// `stream >> d` for a `double`.
    pub fn f64(&mut self) -> Got<f64> {
        if !self.sentry() {
            return Got::Untouched;
        }
        let x = self.float_chars();
        let (v, used) = cstr::strtod(&x);
        if x.is_empty() || used != x.len() {
            self.fail = true;
            return Got::Failed(0.0);
        }
        if v.is_infinite() {
            self.fail = true;
            return Got::Failed(if v > 0.0 { f64::MAX } else { -f64::MAX });
        }
        Got::Value(v)
    }

    /// `stream >> f` for a `float` (`strtof`).
    pub fn f32(&mut self) -> Got<f32> {
        if !self.sentry() {
            return Got::Untouched;
        }
        let x = self.float_chars();
        let (v, used) = cstr::strtof(&x);
        if x.is_empty() || used != x.len() {
            self.fail = true;
            return Got::Failed(0.0);
        }
        if v.is_infinite() {
            self.fail = true;
            return Got::Failed(if v > 0.0 { f32::MAX } else { -f32::MAX });
        }
        Got::Value(v)
    }

    /// `stream >> n` for an `int`: a `long` in base 10 (`_M_extract_int`), then clamped to
    /// `int`'s range (a failure).
    pub fn i32(&mut self) -> Got<i32> {
        if !self.sentry() {
            return Got::Untouched;
        }
        let mut c = self.peek();
        let mut negative = false;
        if let Some(s @ (b'+' | b'-')) = c {
            negative = s == b'-';
            c = self.advance();
        }
        let mut digits = false;
        let mut value: i128 = 0;
        let mut overflow = false;
        while let Some(ch) = c {
            if !ch.is_ascii_digit() {
                break;
            }
            digits = true;
            value = value * 10 + i128::from(ch - b'0');
            if value > i128::from(i64::MAX) + 1 {
                overflow = true;
                value = i128::from(i64::MAX) + 1;
            }
            c = self.advance();
        }
        if !digits {
            self.fail = true;
            return Got::Failed(0);
        }
        let long: i64 = if negative {
            if overflow || value > i128::from(i64::MAX) + 1 {
                self.fail = true;
                i64::MIN
            } else {
                (-value) as i64
            }
        } else if overflow || value > i128::from(i64::MAX) {
            self.fail = true;
            i64::MAX
        } else {
            value as i64
        };
        if long < i64::from(i32::MIN) {
            self.fail = true;
            return Got::Failed(i32::MIN);
        }
        if long > i64::from(i32::MAX) {
            self.fail = true;
            return Got::Failed(i32::MAX);
        }
        if self.fail {
            return Got::Failed(long as i32);
        }
        Got::Value(long as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_words_and_lines() {
        let mut s = IStream::new(
            b"# vtk DataFile Version 3.0\nsurface\n  POINTS 12  float\n1.5 -2e3 007\n",
        );
        assert_eq!(s.getline().as_deref(), Some("# vtk DataFile Version 3.0"));
        assert_eq!(s.getline().as_deref(), Some("surface"));
        assert_eq!(s.word(), Got::Value("POINTS".into()));
        assert_eq!(s.i32(), Got::Value(12));
        assert_eq!(s.word(), Got::Value("float".into()));
        assert_eq!(s.f64(), Got::Value(1.5));
        assert_eq!(s.f64(), Got::Value(-2000.0));
        assert_eq!(s.i32(), Got::Value(7));
        assert_eq!(s.i32(), Got::Untouched);
        assert!(s.failed());
        assert_eq!(s.f64(), Got::Untouched);
    }

    #[test]
    fn failures() {
        let mut s = IStream::new(b"1e x");
        assert_eq!(s.f64(), Got::Failed(0.0));
        assert_eq!(s.f64(), Got::Untouched);
        let mut s = IStream::new(b"nan");
        assert_eq!(s.f64(), Got::Failed(0.0));
        let mut s = IStream::new(b"1e999 2");
        assert_eq!(s.f64(), Got::Failed(f64::MAX));
        let mut s = IStream::new(b"0x10 5");
        assert_eq!(s.f64(), Got::Value(0.0));
        assert_eq!(s.f64(), Got::Failed(0.0));
        let mut s = IStream::new(b"99999999999 1");
        assert_eq!(s.i32(), Got::Failed(i32::MAX));
        let mut s = IStream::new(b"12.5 3");
        assert_eq!(s.i32(), Got::Value(12));
        assert_eq!(s.i32(), Got::Failed(0));
        let mut s = IStream::new(b"   ");
        assert_eq!(s.i32(), Got::Untouched);
        // the last number of a file without a newline is read; the next extraction fails
        let mut s = IStream::new(b"3");
        assert_eq!(s.i32(), Got::Value(3));
        assert_eq!(s.i32(), Got::Untouched);
    }
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C++ streams `lta_convert.cpp` reads and writes its text formats with (`std::ifstream`'s `>>`,
//! `peek`, `getline`; `std::ofstream`'s `<<` with `precision`), as libstdc++ defines them in the
//! "C" locale, the program's own (FreeSurfer's `setLocale` changes only C's `LC_NUMERIC`, which
//! libstdc++'s streams do not read).
//!
//! An input stream ([`CxxIn`]) holds the file's bytes and the stream's state: `eofbit` (the end met
//! while reading) and `failbit` (an extraction that took nothing, or a number that did not
//! convert). Once `failbit` is set every later extraction takes nothing and leaves its variable
//! as it was; `peek` returns the end. A number (`>> float`, `num_get`): after white space, an
//! optional sign, digits with at most one `.`, and an exponent (`e` or `E` after a digit, an
//! optional sign, digits), as far as they go; that text must convert whole (`strtof`, correctly
//! rounded), else the variable becomes 0 with `failbit`; a value beyond `float`'s range becomes
//! `FLT_MAX` (with its sign) with `failbit`. White space before an item that reaches the end sets
//! `eofbit` and `failbit` and leaves the variable as it was.

use larmorx_freesurfer_core::cstd::format_double;

/// `isspace` in the "C" locale: the stream's white space.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `std::ifstream` on a file's bytes.
pub struct CxxIn {
    data: Vec<u8>,
    pos: usize,
    eof: bool,
    fail: bool,
}

impl CxxIn {
    /// A stream on `data` (a file opened for reading).
    pub fn new(data: Vec<u8>) -> CxxIn {
        CxxIn {
            data,
            pos: 0,
            eof: false,
            fail: false,
        }
    }

    /// `eof()`.
    pub fn eof(&self) -> bool {
        self.eof
    }

    /// `fail()` (`failbit`; `badbit` never set here).
    pub fn fail(&self) -> bool {
        self.fail
    }

    fn good(&self) -> bool {
        !self.eof && !self.fail
    }

    /// The sentry of a formatted extraction: `false` (and `failbit`) when the stream is not good;
    /// white space skipped, the end met there setting `eofbit` and `failbit`.
    fn sentry(&mut self) -> bool {
        if !self.good() {
            self.fail = true;
            return false;
        }
        while self.pos < self.data.len() && is_space(self.data[self.pos]) {
            self.pos += 1;
        }
        if self.pos == self.data.len() {
            self.eof = true;
            self.fail = true;
            return false;
        }
        true
    }

    /// `peek()`: the next byte, `None` for the end (which sets `eofbit` when the stream was good).
    pub fn peek(&mut self) -> Option<u8> {
        if !self.good() {
            return None;
        }
        match self.data.get(self.pos) {
            Some(&b) => Some(b),
            None => {
                self.eof = true;
                None
            }
        }
    }

    /// `getline(stream, s)`: the bytes up to the next new line (taken, not stored) or the end
    /// (`eofbit`); nothing taken at all sets `failbit` (and `s` is then empty).
    pub fn getline(&mut self, s: &mut Vec<u8>) {
        if !self.good() {
            self.fail = true;
            return;
        }
        s.clear();
        let mut taken = 0usize;
        loop {
            match self.data.get(self.pos) {
                None => {
                    self.eof = true;
                    break;
                }
                Some(&b'\n') => {
                    self.pos += 1;
                    taken += 1;
                    break;
                }
                Some(&b) => {
                    s.push(b);
                    self.pos += 1;
                    taken += 1;
                }
            }
        }
        if taken == 0 {
            self.fail = true;
        }
    }

    /// `stream >> s` (a word): after white space, the bytes up to the next white space or the
    /// end (`eofbit`). When the sentry fails `s` keeps what it held. Returns whether a word was
    /// read.
    pub fn word(&mut self, s: &mut Vec<u8>) -> bool {
        if !self.sentry() {
            return false;
        }
        s.clear();
        while let Some(&b) = self.data.get(self.pos) {
            if is_space(b) {
                break;
            }
            s.push(b);
            self.pos += 1;
        }
        if self.pos == self.data.len() {
            self.eof = true;
        }
        true
    }

    /// `stream >> v` for a `float`: `Some` of the value stored into the variable (0, or
    /// `FLT_MAX` with its sign, when the text does not convert; `failbit` then), `None` when the
    /// sentry failed (the variable keeps its value).
    pub fn float(&mut self) -> Option<f32> {
        if !self.sentry() {
            return None;
        }
        let d = &self.data;
        let mut i = self.pos;
        let mut x = String::new();
        if let Some(&c) = d.get(i)
            && (c == b'+' || c == b'-')
        {
            x.push(c as char);
            i += 1;
        }
        let mut found_mantissa = false;
        while let Some(&c) = d.get(i) {
            if c != b'0' {
                break;
            }
            if !found_mantissa {
                x.push('0');
                found_mantissa = true;
            }
            i += 1;
        }
        let (mut found_dec, mut found_sci) = (false, false);
        while let Some(&c) = d.get(i) {
            if c.is_ascii_digit() {
                x.push(c as char);
                found_mantissa = true;
            } else if c == b'.' && !found_dec && !found_sci {
                x.push('.');
                found_dec = true;
            } else if (c == b'e' || c == b'E') && !found_sci && found_mantissa {
                x.push('e');
                found_sci = true;
                i += 1;
                match d.get(i) {
                    Some(&s) if s == b'+' || s == b'-' => x.push(s as char),
                    Some(_) => continue,
                    None => break,
                }
            } else {
                break;
            }
            i += 1;
        }
        self.pos = i;
        if self.pos >= self.data.len() {
            self.pos = self.data.len();
            self.eof = true;
        }
        // strtof must take the whole text; an infinity is a failure (`FLT_MAX`)
        match x.parse::<f32>() {
            Ok(v) if v.is_infinite() => {
                self.fail = true;
                Some(if v < 0.0 { -f32::MAX } else { f32::MAX })
            }
            Ok(v) => Some(v),
            Err(_) => {
                self.fail = true;
                Some(0.0)
            }
        }
    }
}

/// `stream << x` for a `double` (or a `float`, widened) with the stream's `precision(p)` and no
/// float field: `%.<p>g`.
pub fn put(x: f64, precision: usize) -> String {
    format_double(&format!("%.{precision}g"), x).expect("a %g format")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats(s: &str, n: usize) -> (Vec<Option<f32>>, bool, bool) {
        let mut c = CxxIn::new(s.as_bytes().to_vec());
        let v = (0..n).map(|_| c.float()).collect();
        (v, c.eof(), c.fail())
    }

    #[test]
    fn numbers_as_libstdcxx_extracts_them() {
        assert_eq!(
            floats("1 2.5\n", 2),
            (vec![Some(1.0), Some(2.5)], false, false)
        );
        // the end met while skipping white space: the variable untouched, eof and fail
        assert_eq!(floats("1\n", 2), (vec![Some(1.0), None], true, true));
        // a number ending at the end of the data sets eof only
        assert_eq!(floats("7", 1), (vec![Some(7.0)], true, false));
        // text that does not convert: 0 and fail; later extractions take nothing
        assert_eq!(floats("x 3", 2), (vec![Some(0.0), None], false, true));
        assert_eq!(floats("1e 3", 2), (vec![Some(0.0), None], false, true));
        assert_eq!(floats("-.5e-1", 1).0, vec![Some(-0.05)]);
        assert_eq!(floats("00012", 1).0, vec![Some(12.0)]);
        assert_eq!(floats("1.5.5", 1).0, vec![Some(1.5)]);
        // beyond float's range: FLT_MAX and fail
        assert_eq!(floats("-1e40", 1), (vec![Some(-f32::MAX)], true, true));
        assert_eq!(floats("1e-50", 1).0, vec![Some(0.0)]);
        assert_eq!(floats("1ex", 1).0, vec![Some(0.0)]);
    }

    #[test]
    fn words_lines_and_peek() {
        let mut c = CxxIn::new(b"#a b\nT: x\n".to_vec());
        let mut s = Vec::new();
        assert_eq!(c.peek(), Some(b'#'));
        c.getline(&mut s);
        assert_eq!(s, b"#a b");
        assert!(c.word(&mut s) && s == b"T:");
        assert!(c.word(&mut s) && s == b"x");
        assert!(!c.eof());
        // the end after white space: the word stays
        assert!(!c.word(&mut s) && s == b"x" && c.eof() && c.fail());
        assert_eq!(c.peek(), None);
        assert_eq!(put(1.0, 17), "1");
        assert_eq!(put(f64::from(0.1f32), 17), "0.10000000149011612");
        assert_eq!(put(f64::from(0.1f32), 7), "0.1");
        assert_eq!(put(-0.0, 6), "-0");
    }
}

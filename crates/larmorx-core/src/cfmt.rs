// SPDX-License-Identifier: Apache-2.0
//! C's `printf` conversions of a `double` as glibc prints them (`%f`, `%e`, `%g` with a
//! precision and a field width), and `std::ostream << double` with its default format (`%g`
//! with 6 significant digits), for replicas whose messages print numbers.
//!
//! Written from the C standard's description of the conversions (ISO C 7.21.6.1) and glibc's
//! documented spellings of the special values (`nan`, `-nan`, `inf`, `-inf`); the decimal
//! digits are the exact value correctly rounded with ties to even, as glibc rounds them in the
//! default rounding mode (Rust's exact formatting rounds the same way;
//! `validation/probes/fs-vnl` compares them with the C library).

/// Where a ported routine's messages go: the C program's standard output (`printf`, `std::cout`)
/// and standard error (`std::cerr`), in the order the routine writes them. The routine only
/// reports; how the program's streams are buffered and interleaved is its caller's (a replica's
/// I/O layer).
pub trait Streams {
    /// Text the original writes to standard output.
    fn out(&mut self, text: &str);
    /// Text the original writes to standard error.
    fn err(&mut self, text: &str);
    /// Bytes the original writes to standard output that need not be UTF-8 (a message that
    /// carries a file's bytes as they are). A stream that can hold bytes writes them unchanged;
    /// the default passes them to [`Streams::out`] with each invalid sequence as U+FFFD.
    fn out_bytes(&mut self, bytes: &[u8]) {
        self.out(&String::from_utf8_lossy(bytes));
    }
    /// Bytes the original writes to standard error that need not be UTF-8 (see
    /// [`Streams::out_bytes`]).
    fn err_bytes(&mut self, bytes: &[u8]) {
        self.err(&String::from_utf8_lossy(bytes));
    }
}

/// [`Streams`] onto two byte writers (a program's standard output and standard error, or
/// buffers): text and bytes written unchanged, write errors ignored as C's `fprintf` callers
/// ignore them.
pub struct Writers<'a> {
    pub out: &'a mut dyn std::io::Write,
    pub err: &'a mut dyn std::io::Write,
}

impl Streams for Writers<'_> {
    fn out(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        let _ = self.err.write_all(text.as_bytes());
    }
    fn out_bytes(&mut self, bytes: &[u8]) {
        let _ = self.out.write_all(bytes);
    }
    fn err_bytes(&mut self, bytes: &[u8]) {
        let _ = self.err.write_all(bytes);
    }
}

/// One stream of [`Streams`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// [`Streams`] that keeps every message, in order (for tests and for callers that print later).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Captured {
    pub messages: Vec<(Stream, String)>,
}

impl Captured {
    /// Everything written to one stream, concatenated.
    pub fn text(&self, stream: Stream) -> String {
        self.messages
            .iter()
            .filter(|(s, _)| *s == stream)
            .map(|(_, t)| t.as_str())
            .collect()
    }
}

impl Streams for Captured {
    fn out(&mut self, text: &str) {
        self.messages.push((Stream::Out, text.to_owned()));
    }
    fn err(&mut self, text: &str) {
        self.messages.push((Stream::Err, text.to_owned()));
    }
}

/// [`Streams`] that drops everything.
#[derive(Clone, Copy, Debug, Default)]
pub struct Silent;

impl Streams for Silent {
    fn out(&mut self, _: &str) {}
    fn err(&mut self, _: &str) {}
}

/// The special values' spellings, or `None` for a finite number.
fn special(v: f64) -> Option<&'static str> {
    if v.is_nan() {
        Some(if v.is_sign_negative() { "-nan" } else { "nan" })
    } else if v.is_infinite() {
        Some(if v < 0.0 { "-inf" } else { "inf" })
    } else {
        None
    }
}

/// Right-justifies `s` in a field of `width` characters (`%<width>...`).
pub fn pad(s: &str, width: usize) -> String {
    format!("{s:>width$}")
}

/// `%.<prec>f`.
pub fn f(v: f64, prec: usize) -> String {
    match special(v) {
        Some(s) => s.to_owned(),
        None => format!("{v:.prec$}"),
    }
}

/// `%.<prec>e`: one digit, the point (unless the precision is 0), `prec` digits, and an
/// exponent with a sign and at least two digits.
pub fn e(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s.to_owned();
    }
    let s = format!("{v:.prec$e}");
    let (mantissa, exp) = s.split_once('e').expect("an exponent");
    let x: i32 = exp.parse().expect("an exponent");
    let sign = if x < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", x.abs())
}

/// `%.<prec>g`: `%e` or `%f` with `prec` significant digits (0 counts as 1), whichever C's rule
/// picks from the exponent after rounding, trailing zeros and a trailing point removed.
pub fn g(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s.to_owned();
    }
    let p = prec.max(1);
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = sci.split_once('e').expect("an exponent");
    let x: i32 = exp.parse().expect("an exponent");
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

/// `%.<prec>Lf` of an x86-64 `long double` (`prec` at most 18): the exact value correctly rounded
/// with ties to even, as glibc prints it; `nan`, `-nan`, `inf`, `-inf`; an encoding the x87 does
/// not support (an unnormal, a pseudo-NaN or pseudo-infinity) prints as `nan` with its sign, as a
/// NaN.
pub fn f80_f(v: crate::x87::F80, prec: usize) -> String {
    assert!(prec <= 18, "f80_f: precision above 18");
    let bits = v.to_bits();
    let m = bits as u64;
    let se = (bits >> 64) as u16;
    let neg = se & 0x8000 != 0;
    let e = (se & 0x7fff) as i32;
    let sign = if neg { "-" } else { "" };
    let int_bit = m >> 63 == 1;
    if e == 0x7fff {
        return if int_bit && m << 1 == 0 {
            format!("{sign}inf")
        } else {
            format!("{sign}nan")
        };
    }
    if e != 0 && !int_bit {
        return format!("{sign}nan");
    }
    // value = m * 2^ex
    let ex = if e == 0 {
        1 - 16383 - 63
    } else {
        e - 16383 - 63
    };
    let scale = 10u128.pow(prec as u32);
    let digits = if ex >= 0 {
        // An integer: m * 2^ex, in decimal, then `prec` zeros.
        let mut limbs: Vec<u32> = vec![m as u32, (m >> 32) as u32];
        for _ in 0..ex {
            let mut carry = 0u32;
            for l in limbs.iter_mut() {
                let x = ((*l as u64) << 1) | carry as u64;
                *l = x as u32;
                carry = (x >> 32) as u32;
            }
            if carry != 0 {
                limbs.push(carry);
            }
        }
        let mut dec = String::new();
        while limbs.iter().any(|&l| l != 0) {
            let mut rem = 0u64;
            for l in limbs.iter_mut().rev() {
                let x = (rem << 32) | *l as u64;
                *l = (x / 1_000_000_000) as u32;
                rem = x % 1_000_000_000;
            }
            while limbs.last() == Some(&0) {
                limbs.pop();
            }
            dec = if limbs.is_empty() {
                format!("{rem}{dec}")
            } else {
                format!("{rem:09}{dec}")
            };
        }
        if dec.is_empty() {
            dec.push('0');
        }
        dec + &"0".repeat(prec)
    } else {
        let n = m as u128 * scale;
        let s = (-ex) as u32;
        let q = if s >= 128 {
            0
        } else {
            let q = n >> s;
            let r = n & ((1u128 << s) - 1);
            let half = 1u128 << (s - 1);
            if r > half || (r == half && q & 1 == 1) {
                q + 1
            } else {
                q
            }
        };
        format!("{:0>width$}", q, width = prec + 1)
    };
    let (ip, fp) = digits.split_at(digits.len() - prec);
    if prec == 0 {
        format!("{sign}{ip}")
    } else {
        format!("{sign}{ip}.{fp}")
    }
}

/// `std::ostream << v` for a `double` (or a `float`, which the stream widens) with the stream's
/// default flags: `%g` with 6 significant digits.
pub fn ostream(v: f64) -> String {
    g(v, 6)
}

/// `std::ostream << std::setw(width) << v`: [`ostream`] right-justified in `width` characters.
pub fn ostream_setw(v: f64, width: usize) -> String {
    pad(&ostream(v), width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_spell_as_glibc() {
        assert_eq!(g(0.0001, 6), "0.0001");
        assert_eq!(g(0.00001, 6), "1e-05");
        assert_eq!(g(123456.0, 6), "123456");
        assert_eq!(g(1234567.0, 6), "1.23457e+06");
        assert_eq!(g(999999.5, 6), "1e+06");
        assert_eq!(g(-0.0, 6), "-0");
        assert_eq!(g(f64::NAN, 6), "nan");
        assert_eq!(g(-f64::NAN, 6), "-nan");
        assert_eq!(e(0.0, 6), "0.000000e+00");
        assert_eq!(e(1e-300, 2), "1.00e-300");
        assert_eq!(e(-2.5, 0), "-2e+00");
        assert_eq!(f(0.125, 2), "0.12");
        assert_eq!(f(f64::NEG_INFINITY, 3), "-inf");
        assert_eq!(pad(&f(1.2346, 3), 10), "     1.235");
        assert_eq!(ostream_setw(0.5, 4), " 0.5");
    }
}

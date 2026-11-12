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
        assert_eq!(pad(&f(3.14159, 3), 10), "     3.142");
        assert_eq!(ostream_setw(0.5, 4), " 0.5");
    }
}

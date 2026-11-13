// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's `scanf` conversions `%f` and `%d` as FreeSurfer's readers meet them (`fscanf`,
//! `sscanf`), larmorx's own code written from the C standard (C11 7.21.6.2, 7.22.1.3) and glibc
//! 2.35's behaviour **measured** as a black box (`validation/probes/fs-maths/scanf_probe.c`: what
//! each input returns, assigns, sets `errno` to and leaves unread), no glibc code.
//!
//! glibc's `%f` differs from the standard's "longest prefix of a matching sequence" in measured
//! ways, all reproduced here: an exponent marker without digits is taken and ignored (`1e`, `1e+`,
//! `100ergs` read 1, 1 and 100, consuming `1e`, `1e+`, `100e`); `0x` without hexadecimal digits is
//! a matching failure unless a point follows (`0x.` reads 0, and then the exponent is not read);
//! `inf` is complete, but an unfinished `infinity` fails (`infin`), reading the character that
//! does not match (as an unfinished `inf` or `nan` does; a number leaves it unread); `nan` never takes a
//! parenthesised payload (`nan(1)` reads NaN and leaves `(1)`); `-nan` is the negative quiet NaN.
//! The value is correctly rounded (round to nearest, ties to even, subnormals included), and
//! `errno` becomes `ERANGE` on overflow and on underflow, which glibc detects after rounding: an
//! inexact result whose magnitude rounded to 24 bits with an unbounded exponent is below `FLT_MIN`.
//!
//! `%d` reads a `long` (64 bits, `strtol`'s range: beyond it `LONG_MAX` or `LONG_MIN` and
//! `ERANGE`) and keeps its low 32 bits (measured: `2147483648` reads -2147483648, `99999999999`
//! reads 1215752191).
//!
//! **`errno` at the end of the input** (measured, [`Call`]): a conversion that meets the end of the
//! input before its item's first character sets `errno` to 0, unless an earlier item of the same
//! call ended at the end of the input (then `errno` is left alone); white-space directives that meet
//! it leave `errno` alone (`fscanf(fp, "%f", ...)` at the end of a file returns `EOF` with `errno`
//! 0; `sscanf("1 2", "%d %d %d", ...)` returns 2 with `errno` unchanged; `"%f %f"` on `"5 "`
//! returns 1 with `errno` 0).

use std::cmp::Ordering;
use std::io::BufRead;

use crate::cstd::is_space;

/// `errno`'s `ERANGE` on Linux.
pub const ERANGE: i32 = 34;

/// A C input stream as `scanf` reads it: one byte of lookahead.
pub trait Stream {
    /// The next byte, unread.
    fn peek(&mut self) -> Option<u8>;
    /// Reads the byte [`Stream::peek`] gave.
    fn bump(&mut self);
}

/// A `FILE *` over a reader.
pub struct Reader<'a>(pub &'a mut dyn BufRead);

impl Stream for Reader<'_> {
    fn peek(&mut self) -> Option<u8> {
        self.0.fill_buf().ok().and_then(|b| b.first().copied())
    }
    fn bump(&mut self) {
        self.0.consume(1);
    }
}

/// A string as `sscanf` reads it (its end is the C string's end).
pub struct Bytes<'a> {
    pub s: &'a [u8],
    pub i: usize,
}

impl<'a> Bytes<'a> {
    pub fn new(s: &'a [u8]) -> Bytes<'a> {
        let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
        Bytes { s: &s[..end], i: 0 }
    }
}

impl Stream for Bytes<'_> {
    fn peek(&mut self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn bump(&mut self) {
        self.i += 1;
    }
}

/// One conversion's outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scan<T> {
    /// The value, and whether it set `errno` to `ERANGE`.
    Value(T, bool),
    /// A matching failure (the characters read stay read).
    Fail,
    /// The input ended before a character of the item (`scanf` returns `EOF` when nothing was
    /// assigned yet).
    Eof,
}

/// A white-space directive (and the white space every conversion but `%c`, `%[` and `%n` skips).
pub fn skip_space(st: &mut dyn Stream) {
    while st.peek().is_some_and(is_space) {
        st.bump();
    }
}

/// The letters of `inf`, `inity` or `nan` (any case); a character that does not match is read
/// all the same (measured: `infix` reads 5 characters, `nax` 3), unlike a number's.
fn take_word(st: &mut dyn Stream, word: &[u8]) -> bool {
    for &w in word {
        match st.peek() {
            Some(c) if c.to_ascii_lowercase() == w => st.bump(),
            Some(_) => {
                st.bump();
                return false;
            }
            None => return false,
        }
    }
    true
}

fn digits(st: &mut dyn Stream, hex: bool, out: &mut Vec<u8>) {
    while let Some(c) = st.peek() {
        let ok = if hex {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        };
        if !ok {
            break;
        }
        out.push(c);
        st.bump();
    }
}

/// An optional exponent after its marker (already read): sign and decimal digits, as an `i64`
/// saturated far beyond any float's range; no digits: 0.
fn exponent(st: &mut dyn Stream) -> i64 {
    let neg = match st.peek() {
        Some(b'-') => {
            st.bump();
            true
        }
        Some(b'+') => {
            st.bump();
            false
        }
        _ => false,
    };
    let mut e: i64 = 0;
    while let Some(c) = st.peek().filter(u8::is_ascii_digit) {
        e = (e * 10 + i64::from(c - b'0')).min(1 << 40);
        st.bump();
    }
    if neg { -e } else { e }
}

/// `%f` into a `float`.
pub fn scan_f32(st: &mut dyn Stream) -> Scan<f32> {
    skip_space(st);
    let Some(first) = st.peek() else {
        return Scan::Eof;
    };
    let neg = first == b'-';
    if first == b'-' || first == b'+' {
        st.bump();
    }
    let signed = |v: f32| if neg { -v } else { v };
    match st.peek().map(|c| c.to_ascii_lowercase()) {
        Some(b'i') => {
            if !take_word(st, b"inf") {
                return Scan::Fail;
            }
            if st.peek().map(|c| c.to_ascii_lowercase()) == Some(b'i') && !take_word(st, b"inity") {
                return Scan::Fail;
            }
            return Scan::Value(signed(f32::INFINITY), false);
        }
        Some(b'n') => {
            if !take_word(st, b"nan") {
                return Scan::Fail;
            }
            let bits = if neg { 0xffc0_0000 } else { 0x7fc0_0000 };
            return Scan::Value(f32::from_bits(bits), false);
        }
        _ => {}
    }
    let mut int = Vec::new();
    if st.peek() == Some(b'0') {
        st.bump();
        if matches!(st.peek(), Some(b'x' | b'X')) {
            st.bump();
            return scan_hex(st, neg);
        }
        int.push(b'0');
    }
    digits(st, false, &mut int);
    let mut frac = Vec::new();
    if st.peek() == Some(b'.') {
        st.bump();
        digits(st, false, &mut frac);
    }
    if int.is_empty() && frac.is_empty() {
        return Scan::Fail;
    }
    let mut e = 0;
    if matches!(st.peek(), Some(b'e' | b'E')) {
        st.bump();
        e = exponent(st);
    }
    let (v, erange) = decimal_to_f32(&int, &frac, e);
    Scan::Value(signed(v), erange)
}

fn scan_hex(st: &mut dyn Stream, neg: bool) -> Scan<f32> {
    let mut int = Vec::new();
    digits(st, true, &mut int);
    let mut frac = Vec::new();
    let mut dot = false;
    if st.peek() == Some(b'.') {
        st.bump();
        dot = true;
        digits(st, true, &mut frac);
    }
    let signed = |v: f32| if neg { -v } else { v };
    if int.is_empty() && frac.is_empty() {
        return if dot {
            Scan::Value(signed(0.0), false)
        } else {
            Scan::Fail
        };
    }
    let mut e = 0;
    if matches!(st.peek(), Some(b'p' | b'P')) {
        st.bump();
        e = exponent(st);
    }
    // The significant hexadecimal digits: the first 30 exactly (120 bits), the rest as a sticky bit.
    let all: Vec<u8> = int.iter().chain(frac.iter()).copied().collect();
    let start = all.iter().position(|&c| c != b'0').unwrap_or(all.len());
    let sig = &all[start..];
    if sig.is_empty() {
        return Scan::Value(signed(0.0), false);
    }
    let keep = sig.len().min(30);
    let mut mant: u128 = 0;
    for &c in &sig[..keep] {
        mant = (mant << 4) | u128::from((c as char).to_digit(16).unwrap());
    }
    let sticky = sig[keep..].iter().any(|&c| c != b'0');
    // value = mant x 2^(4 (digits after the kept ones, to the point) + e)
    let after_point = frac.len() as i64;
    let dropped = (sig.len() - keep) as i64;
    let exp2 = e.saturating_add(4 * (dropped - after_point));
    let (v, inexact, tiny, overflow) = round_binary(mant, exp2, sticky);
    Scan::Value(signed(v), overflow || (inexact && tiny))
}

/// `mant` x 2^`exp2` (with `sticky` set when more nonzero bits follow `mant`'s last one), rounded
/// to the nearest `float` (ties to even): the value, whether it is inexact, whether its magnitude
/// rounded to 24 bits with an unbounded exponent is below `FLT_MIN` (glibc's tininess), and
/// whether it overflowed.
fn round_binary(mant: u128, exp2: i64, sticky: bool) -> (f32, bool, bool, bool) {
    if mant == 0 {
        return (0.0, false, false, false);
    }
    let nbits = 128 - i64::from(mant.leading_zeros());
    let lead = nbits - 1 + exp2; // the value is in [2^lead, 2^(lead + 1))
    if lead > 200 {
        return (f32::INFINITY, true, false, true);
    }
    if lead < -200 {
        return (0.0, true, true, false);
    }
    // Round to an integer number of 2^q units, ties to even.
    let round_at = |q: i64| -> (u128, bool) {
        let shift = q - exp2; // bits of mant below the unit
        if shift <= 0 {
            return (mant << (-shift) as u32, sticky);
        }
        if shift >= 128 {
            return (0, true);
        }
        let s = shift as u32;
        let kept = mant >> s;
        let rem = mant & ((1u128 << s) - 1);
        let half = 1u128 << (s - 1);
        let inexact = rem != 0 || sticky;
        let up = rem > half || (rem == half && (sticky || kept & 1 == 1));
        (kept + u128::from(up), inexact)
    };
    // Tininess: 24 bits with an unbounded exponent.
    let (m24, _) = round_at(lead - 23);
    let lead24 = if m24 >> 24 != 0 { lead + 1 } else { lead };
    let tiny = lead24 < -126;
    if lead >= -126 {
        let (m, inexact) = round_at(lead - 23);
        let (m, lead) = if m >> 24 != 0 {
            (m >> 1, lead + 1)
        } else {
            (m, lead)
        };
        if lead > 127 {
            return (f32::INFINITY, true, tiny, true);
        }
        let bits = (((lead + 127) as u32) << 23) | (m as u32 & 0x7f_ffff);
        return (f32::from_bits(bits), inexact, tiny, false);
    }
    let (k, inexact) = round_at(-149);
    // k <= 2^23: 2^23 is FLT_MIN, which the bits give as well
    (f32::from_bits(k as u32), inexact, tiny, false)
}

/// A decimal `int.frac` x 10^`e` as a `float`: correctly rounded (the standard library's parser
/// is), and whether glibc sets `ERANGE` (overflow, or an inexact result that is tiny after
/// rounding: decided exactly with integers).
fn decimal_to_f32(int: &[u8], frac: &[u8], e: i64) -> (f32, bool) {
    let all: Vec<u8> = int.iter().chain(frac.iter()).copied().collect();
    let start = all.iter().position(|&c| c != b'0').unwrap_or(all.len());
    let end = all
        .iter()
        .rposition(|&c| c != b'0')
        .map_or(start, |p| p + 1);
    if start >= end {
        return (0.0, false);
    }
    let sig = &all[start..end];
    // value = sig x 10^e10
    let e10 = e - frac.len() as i64 + (all.len() - end) as i64;
    let lead10 = e10 + sig.len() as i64; // the value is below 10^lead10
    if lead10 > 50 {
        return (f32::INFINITY, true);
    }
    if lead10 < -60 {
        return (0.0, true);
    }
    let text = format!("{}e{}", std::str::from_utf8(sig).unwrap(), e10);
    let v: f32 = text.parse().expect("a decimal number");
    if v.is_infinite() {
        return (v, true);
    }
    if v != 0.0 && v.abs() > f32::MIN_POSITIVE {
        return (v, false);
    }
    // Exact comparisons with integers: value' = sig x 10^a x 2^s against K x 10^b.
    let (a, b) = (e10.max(0) as u32, (-e10).max(0) as u32);
    let cmp_with = |k: u64, s: u32| -> Ordering {
        let mut l = Big::from_digits(sig);
        l.mul_pow10(a);
        l.shl(s);
        let mut r = Big::from_u64(k);
        r.mul_pow10(b);
        l.cmp(&r)
    };
    // exact when value == v (v = k x 2^-149, a subnormal, 0 or FLT_MIN)
    let k = u64::from(v.to_bits());
    let exact = cmp_with(k, 149) == Ordering::Equal;
    // tiny when value < T = (2^25 - 1) x 2^-151
    let tiny = cmp_with((1 << 25) - 1, 151) == Ordering::Less;
    (v, !exact && tiny)
}

/// A small unsigned integer of 32-bit limbs (least significant first), for exact comparisons.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn from_u64(v: u64) -> Big {
        let mut b = Big(vec![v as u32, (v >> 32) as u32]);
        b.trim();
        b
    }
    fn from_digits(d: &[u8]) -> Big {
        let mut b = Big(Vec::new());
        for &c in d {
            b.mul_add(10, u32::from(c - b'0'));
        }
        b
    }
    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    fn mul_add(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for l in self.0.iter_mut() {
            let t = u64::from(*l) * u64::from(m) + carry;
            *l = t as u32;
            carry = t >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
        self.trim();
    }
    fn mul_pow10(&mut self, n: u32) {
        for _ in 0..n {
            self.mul_add(10, 0);
        }
    }
    fn shl(&mut self, s: u32) {
        let (words, bits) = ((s / 32) as usize, s % 32);
        let mut out = vec![0u32; words];
        let mut carry = 0u32;
        for &l in &self.0 {
            if bits == 0 {
                out.push(l);
            } else {
                out.push((l << bits) | carry);
                carry = l >> (32 - bits);
            }
        }
        if carry != 0 {
            out.push(carry);
        }
        self.0 = out;
        self.trim();
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, o: &Big) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Big {
    fn cmp(&self, o: &Big) -> Ordering {
        self.0
            .len()
            .cmp(&o.0.len())
            .then_with(|| self.0.iter().rev().cmp(o.0.iter().rev()))
    }
}

/// One `scanf` call's conversions with glibc's `errno` rules: `ERANGE` from a conversion, 0 for a
/// conversion that meets the end of the input before its item unless an earlier item of the call
/// ended there.
#[derive(Default)]
pub struct Call {
    latched: bool,
}

impl Call {
    pub fn new() -> Call {
        Call::default()
    }

    fn after<T>(&mut self, st: &mut dyn Stream, r: Scan<T>, errno: &mut i32) -> Option<T> {
        match r {
            Scan::Value(v, erange) => {
                if erange {
                    *errno = ERANGE;
                }
                if st.peek().is_none() {
                    self.latched = true;
                }
                Some(v)
            }
            Scan::Fail => {
                if st.peek().is_none() {
                    self.latched = true;
                }
                None
            }
            Scan::Eof => {
                if !self.latched {
                    *errno = 0;
                }
                None
            }
        }
    }

    /// A `%f` of this call: its value, or `None` for a matching or input failure (the call stops).
    pub fn f32(&mut self, st: &mut dyn Stream, errno: &mut i32) -> Option<f32> {
        let r = scan_f32(st);
        self.after(st, r, errno)
    }

    /// A `%d` of this call.
    pub fn i32(&mut self, st: &mut dyn Stream, errno: &mut i32) -> Option<i32> {
        let r = scan_i32(st);
        self.after(st, r, errno)
    }
}

/// `%d` into an `int`.
pub fn scan_i32(st: &mut dyn Stream) -> Scan<i32> {
    skip_space(st);
    let Some(first) = st.peek() else {
        return Scan::Eof;
    };
    let neg = first == b'-';
    if first == b'-' || first == b'+' {
        st.bump();
    }
    let mut d = Vec::new();
    digits(st, false, &mut d);
    if d.is_empty() {
        return Scan::Fail;
    }
    let mut v: i128 = 0;
    for &c in &d {
        v = (v * 10 + i128::from(c - b'0')).min(i128::from(i64::MAX) + 2);
    }
    let v = if neg { -v } else { v };
    let erange = v > i128::from(i64::MAX) || v < i128::from(i64::MIN);
    let l = v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    Scan::Value(l as i32, erange)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> (Scan<f32>, usize) {
        let mut b = Bytes::new(s.as_bytes());
        let r = scan_f32(&mut b);
        (r, b.i)
    }

    fn bits(s: &str) -> (u32, bool, usize) {
        match f(s) {
            (Scan::Value(v, e), i) => (v.to_bits(), e, i),
            other => panic!("{s}: {other:?}"),
        }
    }

    /// The black-box measurements of glibc 2.35 (`scanf_probe.c`): bits, `ERANGE`, bytes read.
    #[test]
    fn glibc_measurements() {
        let cases: &[(&str, u32, bool, usize)] = &[
            ("1", 0x3f80_0000, false, 1),
            ("-1.5", 0xbfc0_0000, false, 4),
            ("+.5", 0x3f00_0000, false, 3),
            ("5.", 0x40a0_0000, false, 2),
            ("1e", 0x3f80_0000, false, 2),
            ("1e+", 0x3f80_0000, false, 3),
            ("1e-3x", 0x3a83_126f, false, 4),
            ("100ergs", 0x42c8_0000, false, 4),
            ("0x1p3", 0x4100_0000, false, 5),
            ("0x1.8", 0x3fc0_0000, false, 5),
            ("0x.p1", 0x0000_0000, false, 3),
            ("infx", 0x7f80_0000, false, 3),
            ("infinityx", 0x7f80_0000, false, 8),
            ("-inf", 0xff80_0000, false, 4),
            ("-nan", 0xffc0_0000, false, 4),
            ("nan(123)", 0x7fc0_0000, false, 3),
            ("1e40", 0x7f80_0000, true, 4),
            ("1e-50", 0x0000_0000, true, 5),
            ("1e-40", 0x0001_16c2, true, 5),
            ("3.4028235e38", 0x7f7f_ffff, false, 12),
            ("3.40282357e38", 0x7f80_0000, true, 13),
            ("1.17549435e-38", 0x0080_0000, false, 14),
            ("0x1p-149", 0x0000_0001, false, 8),
            ("0x1p-150", 0x0000_0000, true, 8),
            ("0x1.000001p0", 0x3f80_0000, false, 12),
            ("1.2.3", 0x3f99_999a, false, 3),
            ("1ee5", 0x3f80_0000, false, 2),
            ("1e5e5", 0x47c3_5000, false, 3),
            ("0x1.fffffffp127", 0x7f80_0000, true, 15),
            ("0x0.8p-149", 0x0000_0000, true, 10),
            ("0x1.8p-149", 0x0000_0002, true, 10),
            ("0x1.4p-149", 0x0000_0001, true, 10),
            ("1.4e-45", 0x0000_0001, true, 7),
            ("0x123456789abcdef0123p0", 0x6391_a2b4, false, 23),
            ("1e99999999999", 0x7f80_0000, true, 13),
            ("0e99999999999", 0x0000_0000, false, 13),
            ("0x1.fffffep-127", 0x0080_0000, true, 15),
            ("0x1.ffffffp-127", 0x0080_0000, false, 15),
            ("0x1.fffffcp-127", 0x007f_ffff, false, 15),
            (
                "1.40129846432481707092372958328991613128026194187651577175706828388979108268586060148663818836212158203125e-45",
                0x0000_0001,
                false,
                110,
            ),
            ("1.1754942e-38", 0x007f_ffff, true, 13),
            ("0x1.ffffffp127", 0x7f80_0000, true, 14),
            ("0xa.", 0x4120_0000, false, 4),
            ("-0", 0x8000_0000, false, 2),
        ];
        for &(s, b, e, n) in cases {
            assert_eq!(bits(s), (b, e, n), "{s}");
        }
        for (s, n) in [
            (".", 1),
            ("-", 1),
            ("0x", 2),
            ("0xg", 2),
            ("0xp1", 2),
            ("infin", 5),
            ("infinit", 7),
            ("na", 2),
            ("infix", 5),
            ("infinx", 6),
            ("nax", 3),
            ("ix", 2),
            ("e5", 0),
            (".e1", 1),
            ("-0x", 3),
            ("-e", 1),
        ] {
            assert_eq!(f(s), (Scan::Fail, n), "{s}");
        }
        assert_eq!(f("   ").0, Scan::Eof);
        assert_eq!(bits("  \t\n7"), (0x40e0_0000, false, 5));
    }

    /// `eof2.c`'s measurements: `"%f %f"` and `"%d %d"` on each input, `errno` 7 before.
    #[test]
    fn errno_at_the_end_of_the_input() {
        for (input, n, errno) in [
            ("", 0, 0),
            (" ", 0, 0),
            ("5", 1, 7),
            ("5 ", 1, 0),
            ("5  ", 1, 0),
            ("5 6", 2, 7),
        ] {
            for int in [false, true] {
                let mut b = Bytes::new(input.as_bytes());
                let mut call = Call::new();
                let mut e = 7;
                let mut got = 0;
                for _ in 0..2 {
                    let ok = if int {
                        call.i32(&mut b, &mut e).is_some()
                    } else {
                        call.f32(&mut b, &mut e).is_some()
                    };
                    if !ok {
                        break;
                    }
                    got += 1;
                    skip_space(&mut b);
                }
                assert_eq!((got, e), (n, errno), "{input:?} int {int}");
            }
        }
    }

    #[test]
    fn ints_as_glibc() {
        let d = |s: &str| scan_i32(&mut Bytes::new(s.as_bytes()));
        assert_eq!(d("2147483648"), Scan::Value(-2_147_483_648, false));
        assert_eq!(d("99999999999"), Scan::Value(1_215_752_191, false));
        assert_eq!(d("-0"), Scan::Value(0, false));
        assert_eq!(d("+5"), Scan::Value(5, false));
        assert_eq!(d("a"), Scan::Fail);
        assert_eq!(d(""), Scan::Eof);
    }
}

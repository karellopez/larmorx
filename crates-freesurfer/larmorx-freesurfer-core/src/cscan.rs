// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's `scanf` conversions `%f`, `%lf` and `%d` as FreeSurfer's readers meet them (`fscanf`,
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
//! **A whole call and `errno` at the end of the input** (measured, [`Scanf`], [`Call`]): the first
//! time a call meets the end of its input it remembers `errno` as it is then, and every later
//! attempt of the same call to read a character sets `errno` back to that value; an end met while
//! a conversion skips the white space before its item is remembered as 0; a number and `inf` look
//! one character past themselves, a complete `nan` or `infinity` does not (`fscanf(fp, "%f", ...)`
//! at the end of a file returns `EOF` with `errno` 0; `sscanf("1 2", "%d %d %d", ...)` returns 2
//! with `errno` unchanged; `"%f %f"` on `"5 "` returns 1 with `errno` 0; `"%f %d"` on `"1e99"`
//! returns 1 with `errno` as it was before the call, on `"1e99 x"` with `ERANGE`).

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
    /// Whether a read already found the end of the input. By default a look at the next byte; a C
    /// stream says whether its end-of-file flag is set (glibc does not read on after a complete
    /// `nan` or `infinity`: measured). [`Scanf`] and [`Call`] track the end themselves.
    fn hit_end(&mut self) -> bool {
        self.peek().is_none()
    }
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

/// A number's text as `%f` and `%lf` read it (glibc's rules; both conversions read the same
/// characters, only the final rounding differs).
enum Text {
    Inf(bool),
    Nan(bool),
    /// `0x.` with no digit: 0 (the exponent is not read).
    HexZero(bool),
    Hex {
        neg: bool,
        int: Vec<u8>,
        frac: Vec<u8>,
        e: i64,
    },
    Dec {
        neg: bool,
        int: Vec<u8>,
        frac: Vec<u8>,
        e: i64,
    },
    Fail,
    Eof,
}

fn scan_text(st: &mut dyn Stream) -> Text {
    skip_space(st);
    let Some(first) = st.peek() else {
        return Text::Eof;
    };
    let neg = first == b'-';
    if first == b'-' || first == b'+' {
        st.bump();
    }
    match st.peek().map(|c| c.to_ascii_lowercase()) {
        Some(b'i') => {
            if !take_word(st, b"inf") {
                return Text::Fail;
            }
            if st.peek().map(|c| c.to_ascii_lowercase()) == Some(b'i') && !take_word(st, b"inity") {
                return Text::Fail;
            }
            return Text::Inf(neg);
        }
        Some(b'n') => {
            if !take_word(st, b"nan") {
                return Text::Fail;
            }
            return Text::Nan(neg);
        }
        _ => {}
    }
    let mut int = Vec::new();
    if st.peek() == Some(b'0') {
        st.bump();
        if matches!(st.peek(), Some(b'x' | b'X')) {
            st.bump();
            return scan_hex_text(st, neg);
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
        return Text::Fail;
    }
    let mut e = 0;
    if matches!(st.peek(), Some(b'e' | b'E')) {
        st.bump();
        e = exponent(st);
    }
    Text::Dec { neg, int, frac, e }
}

fn scan_hex_text(st: &mut dyn Stream, neg: bool) -> Text {
    let mut int = Vec::new();
    digits(st, true, &mut int);
    let mut frac = Vec::new();
    let mut dot = false;
    if st.peek() == Some(b'.') {
        st.bump();
        dot = true;
        digits(st, true, &mut frac);
    }
    if int.is_empty() && frac.is_empty() {
        return if dot { Text::HexZero(neg) } else { Text::Fail };
    }
    let mut e = 0;
    if matches!(st.peek(), Some(b'p' | b'P')) {
        st.bump();
        e = exponent(st);
    }
    Text::Hex { neg, int, frac, e }
}

/// A binary format's parameters: significand bits (with the leading one), the smallest normal
/// exponent, the largest exponent.
#[derive(Clone, Copy)]
struct Format {
    p: i64,
    emin: i64,
    emax: i64,
}

const F32: Format = Format {
    p: 24,
    emin: -126,
    emax: 127,
};
const F64: Format = Format {
    p: 53,
    emin: -1022,
    emax: 1023,
};

/// `%f` into a `float`.
pub fn scan_f32(st: &mut dyn Stream) -> Scan<f32> {
    let signed = |neg: bool, v: f32| if neg { -v } else { v };
    match scan_text(st) {
        Text::Eof => Scan::Eof,
        Text::Fail => Scan::Fail,
        Text::Inf(neg) => Scan::Value(signed(neg, f32::INFINITY), false),
        Text::Nan(neg) => {
            let bits = if neg { 0xffc0_0000 } else { 0x7fc0_0000 };
            Scan::Value(f32::from_bits(bits), false)
        }
        Text::HexZero(neg) => Scan::Value(signed(neg, 0.0), false),
        Text::Hex { neg, int, frac, e } => {
            let (bits, erange) = hex_bits(&int, &frac, e, F32);
            Scan::Value(signed(neg, f32::from_bits(bits as u32)), erange)
        }
        Text::Dec { neg, int, frac, e } => {
            let (v, erange) = decimal_to_f32(&int, &frac, e);
            Scan::Value(signed(neg, v), erange)
        }
    }
}

/// `%lf` into a `double`: the same characters as `%f` (glibc's scanner is shared), rounded to
/// 53 bits; `ERANGE` on overflow and on an inexact result that is tiny after rounding (below
/// `DBL_MIN` when rounded to 53 bits with an unbounded exponent).
pub fn scan_f64(st: &mut dyn Stream) -> Scan<f64> {
    let signed = |neg: bool, v: f64| if neg { -v } else { v };
    match scan_text(st) {
        Text::Eof => Scan::Eof,
        Text::Fail => Scan::Fail,
        Text::Inf(neg) => Scan::Value(signed(neg, f64::INFINITY), false),
        Text::Nan(neg) => {
            let bits = if neg {
                0xfff8_0000_0000_0000
            } else {
                0x7ff8_0000_0000_0000
            };
            Scan::Value(f64::from_bits(bits), false)
        }
        Text::HexZero(neg) => Scan::Value(signed(neg, 0.0), false),
        Text::Hex { neg, int, frac, e } => {
            let (bits, erange) = hex_bits(&int, &frac, e, F64);
            Scan::Value(signed(neg, f64::from_bits(bits)), erange)
        }
        Text::Dec { neg, int, frac, e } => {
            let (v, erange) = decimal_to_f64(&int, &frac, e);
            Scan::Value(signed(neg, v), erange)
        }
    }
}

/// A hexadecimal significand's digits and binary exponent, rounded in `f` (ties to even): the
/// bits and whether glibc sets `ERANGE`.
fn hex_bits(int: &[u8], frac: &[u8], e: i64, f: Format) -> (u64, bool) {
    // The significant hexadecimal digits: the first 30 exactly (120 bits), the rest as a sticky bit.
    let all: Vec<u8> = int.iter().chain(frac.iter()).copied().collect();
    let start = all.iter().position(|&c| c != b'0').unwrap_or(all.len());
    let sig = &all[start..];
    if sig.is_empty() {
        return (0, false);
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
    let (bits, inexact, tiny, overflow) = round_binary(mant, exp2, sticky, f);
    (bits, overflow || (inexact && tiny))
}

/// `mant` x 2^`exp2` (with `sticky` set when more nonzero bits follow `mant`'s last one), rounded
/// to the nearest number of format `f` (ties to even): its bits, whether it is inexact, whether its
/// magnitude rounded to `p` bits with an unbounded exponent is below the smallest normal number
/// (glibc's tininess), and whether it overflowed.
fn round_binary(mant: u128, exp2: i64, sticky: bool, f: Format) -> (u64, bool, bool, bool) {
    let inf_bits = ((2 * f.emax + 1) as u64) << (f.p - 1);
    if mant == 0 {
        return (0, false, false, false);
    }
    let nbits = 128 - i64::from(mant.leading_zeros());
    let lead = nbits - 1 + exp2; // the value is in [2^lead, 2^(lead + 1))
    if lead > f.emax + 73 {
        return (inf_bits, true, false, true);
    }
    if lead < f.emin - (f.p - 1) - 51 {
        return (0, true, true, false);
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
    // Tininess: p bits with an unbounded exponent.
    let (mp, _) = round_at(lead - (f.p - 1));
    let leadp = if mp >> f.p != 0 { lead + 1 } else { lead };
    let tiny = leadp < f.emin;
    if lead >= f.emin {
        let (m, inexact) = round_at(lead - (f.p - 1));
        let (m, lead) = if m >> f.p != 0 {
            (m >> 1, lead + 1)
        } else {
            (m, lead)
        };
        if lead > f.emax {
            return (inf_bits, true, tiny, true);
        }
        let frac_mask = (1u64 << (f.p - 1)) - 1;
        let bits = (((lead + f.emax) as u64) << (f.p - 1)) | (m as u64 & frac_mask);
        return (bits, inexact, tiny, false);
    }
    let (k, inexact) = round_at(f.emin - (f.p - 1));
    // k <= 2^(p-1): 2^(p-1) is the smallest normal number, which the bits give as well
    (k as u64, inexact, tiny, false)
}

/// A decimal number's significant digits and exponent: `value = sig x 10^e10`, with
/// `10^lead10` above it; `None` for zero.
fn decimal_parts(int: &[u8], frac: &[u8], e: i64) -> Option<(Vec<u8>, i64, i64)> {
    let all: Vec<u8> = int.iter().chain(frac.iter()).copied().collect();
    let start = all.iter().position(|&c| c != b'0').unwrap_or(all.len());
    let end = all
        .iter()
        .rposition(|&c| c != b'0')
        .map_or(start, |p| p + 1);
    if start >= end {
        return None;
    }
    let sig = all[start..end].to_vec();
    let e10 = e - frac.len() as i64 + (all.len() - end) as i64;
    let lead10 = e10 + sig.len() as i64;
    Some((sig, e10, lead10))
}

/// Exact comparison of `sig x 10^e10 x 2^s` with `k x 10^0` (`Ordering` of the first).
fn cmp_exact(sig: &[u8], e10: i64, s: u32, k: u64) -> Ordering {
    let (a, b) = (e10.max(0) as u32, (-e10).max(0) as u32);
    let mut l = Big::from_digits(sig);
    l.mul_pow10(a);
    l.shl(s);
    let mut r = Big::from_u64(k);
    r.mul_pow10(b);
    l.cmp(&r)
}

/// A decimal `int.frac` x 10^`e` as a `float`: correctly rounded (the standard library's parser
/// is), and whether glibc sets `ERANGE` (overflow, or an inexact result that is tiny after
/// rounding: decided exactly with integers).
fn decimal_to_f32(int: &[u8], frac: &[u8], e: i64) -> (f32, bool) {
    let Some((sig, e10, lead10)) = decimal_parts(int, frac, e) else {
        return (0.0, false);
    };
    if lead10 > 50 {
        return (f32::INFINITY, true);
    }
    if lead10 < -60 {
        return (0.0, true);
    }
    let text = format!("{}e{}", std::str::from_utf8(&sig).unwrap(), e10);
    let v: f32 = text.parse().expect("a decimal number");
    if v.is_infinite() {
        return (v, true);
    }
    if v != 0.0 && v.abs() > f32::MIN_POSITIVE {
        return (v, false);
    }
    // exact when value == v (v = k x 2^-149, a subnormal, 0 or FLT_MIN)
    let k = u64::from(v.to_bits());
    let exact = cmp_exact(&sig, e10, 149, k) == Ordering::Equal;
    // tiny when value < T = (2^25 - 1) x 2^-151
    let tiny = cmp_exact(&sig, e10, 151, (1 << 25) - 1) == Ordering::Less;
    (v, !exact && tiny)
}

/// [`decimal_to_f32`] for a `double`: tiny below `(2^54 - 1) x 2^-1076`.
fn decimal_to_f64(int: &[u8], frac: &[u8], e: i64) -> (f64, bool) {
    let Some((sig, e10, lead10)) = decimal_parts(int, frac, e) else {
        return (0.0, false);
    };
    if lead10 > 330 {
        return (f64::INFINITY, true);
    }
    if lead10 < -345 {
        return (0.0, true);
    }
    let text = format!("{}e{}", std::str::from_utf8(&sig).unwrap(), e10);
    let v: f64 = text.parse().expect("a decimal number");
    if v.is_infinite() {
        return (v, true);
    }
    if v != 0.0 && v.abs() > f64::MIN_POSITIVE {
        return (v, false);
    }
    let k = v.to_bits();
    let exact = cmp_exact(&sig, e10, 1074, k) == Ordering::Equal;
    let tiny = cmp_exact(&sig, e10, 1076, (1 << 54) - 1) == Ordering::Less;
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

/// Why a `scanf` call stopped: a character that does not match (a matching failure) or the end
/// of the input (an input failure).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Matching,
    Input,
}

/// One `scanf` call's state for glibc's rules at the end of the input (measured as a black box:
/// `validation/investigations/glibc-scanf-probe/` and `validation/investigations/glibc-stdio-
/// probe/scanf_end.c`, no glibc code):
///
/// - the first time the call meets the end of its input it remembers `errno` as it is then, and
///   every later attempt of the same call to read a character (a conversion's item, a text
///   character, the white space a format ends with) sets `errno` back to that value;
/// - an end met while a conversion skips the white space before its item is remembered as 0;
/// - a conversion meets the end only when it looks past its last character: a number does, and so
///   does `inf` (it may go on as `infinity`); a complete `nan` or `infinity` does not.
#[derive(Clone, Copy, Debug, Default)]
struct EndRule {
    eof: bool,
    saved: i32,
}

impl EndRule {
    fn peek(&mut self, st: &mut dyn Stream, errno_now: i32) -> Option<u8> {
        let c = st.peek();
        if c.is_none() && !self.eof {
            self.eof = true;
            self.saved = errno_now;
        }
        c
    }

    /// The white space before a conversion's item (an end met there remembered as 0); whether an
    /// item character follows. Without one `errno` is the remembered value.
    fn begin(&mut self, st: &mut dyn Stream, errno: &mut i32) -> bool {
        if !self.eof {
            while let Some(c) = self.peek(st, 0) {
                if !is_space(c) {
                    return true;
                }
                st.bump();
            }
        }
        *errno = self.saved;
        false
    }
}

/// The stream as a conversion reads it, with the call's end rule (the errno of the moment is the
/// one before the conversion).
struct Tracked<'b> {
    st: &'b mut dyn Stream,
    end: &'b mut EndRule,
    errno_now: i32,
}

impl Stream for Tracked<'_> {
    fn peek(&mut self) -> Option<u8> {
        self.end.peek(self.st, self.errno_now)
    }
    fn bump(&mut self) {
        self.st.bump();
    }
    fn hit_end(&mut self) -> bool {
        self.st.hit_end()
    }
}

/// One `sscanf` or `fscanf` call, directive by directive, as glibc 2.35 behaves (measured as a
/// black box, [`EndRule`]): text that must match, white space (any amount, none included), `%d`,
/// `%f`, `%lf`, `%s` with a width, and the suppressed forms; the values come from this module's
/// conversions. The call returns ([`Scanf::finish`]) the number of items assigned, or `EOF` (-1)
/// when its input ended before it assigned any (a matching failure returns the count). A number
/// beyond its type's range sets `errno` to `ERANGE`, suppressed or not. A call that stops (a
/// matching or input failure) reads nothing more, so a format's final white space is skipped only
/// by a call that reached it.
pub struct Scanf<'a> {
    st: &'a mut dyn Stream,
    end: EndRule,
    /// A white-space directive is pending (skipped by the next directive that reads).
    skip: bool,
    done: i32,
    stop: Option<Stop>,
}

impl<'a> Scanf<'a> {
    pub fn new(st: &'a mut dyn Stream) -> Scanf<'a> {
        Scanf {
            st,
            end: EndRule::default(),
            skip: false,
            done: 0,
            stop: None,
        }
    }

    /// Whether the call has stopped.
    pub fn stopped(&self) -> bool {
        self.stop.is_some()
    }

    /// The items assigned so far.
    pub fn assigned(&self) -> i32 {
        self.done
    }

    /// The format's text `t`: white space (any amount, none included) and characters that must
    /// match.
    pub fn text(&mut self, errno: &mut i32, t: &[u8]) {
        for &b in t {
            if self.stop.is_some() {
                return;
            }
            if is_space(b) {
                self.skip = true;
                continue;
            }
            if self.end.eof {
                *errno = self.end.saved;
                self.stop = Some(Stop::Input);
                return;
            }
            loop {
                match self.end.peek(self.st, *errno) {
                    None => {
                        self.stop = Some(Stop::Input);
                        return;
                    }
                    Some(c) if self.skip && is_space(c) => self.st.bump(),
                    Some(c) => {
                        self.skip = false;
                        if c == b {
                            self.st.bump();
                        } else {
                            self.stop = Some(Stop::Matching);
                        }
                        break;
                    }
                }
            }
        }
    }

    fn begin(&mut self, errno: &mut i32) -> bool {
        if self.stop.is_some() {
            return false;
        }
        self.skip = false;
        if self.end.begin(self.st, errno) {
            return true;
        }
        self.stop = Some(Stop::Input);
        false
    }

    fn after<T>(&mut self, errno: &mut i32, r: Scan<T>, assign: bool) -> Option<T> {
        match r {
            Scan::Value(v, erange) => {
                if erange {
                    *errno = ERANGE;
                }
                if assign {
                    self.done += 1;
                }
                Some(v)
            }
            Scan::Fail => {
                self.stop = Some(Stop::Matching);
                None
            }
            Scan::Eof => {
                self.stop = Some(Stop::Input);
                None
            }
        }
    }

    fn convert<T>(
        &mut self,
        errno: &mut i32,
        assign: bool,
        f: fn(&mut dyn Stream) -> Scan<T>,
    ) -> Option<T> {
        if !self.begin(errno) {
            return None;
        }
        let r = f(&mut Tracked {
            st: self.st,
            end: &mut self.end,
            errno_now: *errno,
        });
        self.after(errno, r, assign)
    }

    /// `%d` (`assign` false: `%*d`): the value read, or `None` when the call stops.
    pub fn d(&mut self, errno: &mut i32, assign: bool) -> Option<i32> {
        self.convert(errno, assign, scan_i32)
    }

    /// `%f` (or `%e`, `%g`) into a `float` (`assign` false: suppressed).
    pub fn f(&mut self, errno: &mut i32, assign: bool) -> Option<f32> {
        self.convert(errno, assign, scan_f32)
    }

    /// `%lf` into a `double` (`assign` false: suppressed).
    pub fn lf(&mut self, errno: &mut i32, assign: bool) -> Option<f64> {
        self.convert(errno, assign, scan_f64)
    }

    /// `%<width>s` (`assign` false: `%*s`): the characters before white space or the end, at most
    /// `width` (`usize::MAX`: no width).
    pub fn s(&mut self, errno: &mut i32, width: usize, assign: bool) -> Option<Vec<u8>> {
        if !self.begin(errno) {
            return None;
        }
        let mut w = Vec::new();
        while w.len() < width {
            match self.end.peek(self.st, *errno) {
                Some(c) if !is_space(c) => {
                    w.push(c);
                    self.st.bump();
                }
                _ => break,
            }
        }
        if assign {
            self.done += 1;
        }
        Some(w)
    }

    /// The end of the format: its final white space skipped (when the call reached it); the
    /// call's return value.
    pub fn finish(&mut self, errno: &mut i32) -> i32 {
        if self.stop.is_none() && self.skip {
            if self.end.eof {
                *errno = self.end.saved;
            } else {
                while let Some(c) = self.end.peek(self.st, *errno) {
                    if !is_space(c) {
                        break;
                    }
                    self.st.bump();
                }
            }
        }
        if self.stop == Some(Stop::Input) && self.done == 0 {
            -1
        } else {
            self.done
        }
    }
}

/// One `scanf` call's conversions, each given its stream, with [`Scanf`]'s rules at the end of
/// the input (the white space between them read by the caller through [`skip_space`], which is
/// the same as a white-space directive: the next conversion's own skip remembers an end met there
/// as 0). For calls made of conversions only; [`Scanf`] for the others.
#[derive(Default)]
pub struct Call {
    end: EndRule,
}

impl Call {
    pub fn new() -> Call {
        Call::default()
    }

    fn convert<T>(
        &mut self,
        st: &mut dyn Stream,
        errno: &mut i32,
        f: fn(&mut dyn Stream) -> Scan<T>,
    ) -> Option<T> {
        if !self.end.begin(st, errno) {
            return None;
        }
        let r = f(&mut Tracked {
            st,
            end: &mut self.end,
            errno_now: *errno,
        });
        match r {
            Scan::Value(v, erange) => {
                if erange {
                    *errno = ERANGE;
                }
                Some(v)
            }
            Scan::Fail | Scan::Eof => None,
        }
    }

    /// A `%f` of this call: its value, or `None` for a matching or input failure (the call stops).
    pub fn f32(&mut self, st: &mut dyn Stream, errno: &mut i32) -> Option<f32> {
        self.convert(st, errno, scan_f32)
    }

    /// A `%lf` of this call.
    pub fn f64(&mut self, st: &mut dyn Stream, errno: &mut i32) -> Option<f64> {
        self.convert(st, errno, scan_f64)
    }

    /// A `%d` of this call.
    pub fn i32(&mut self, st: &mut dyn Stream, errno: &mut i32) -> Option<i32> {
        self.convert(st, errno, scan_i32)
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

    /// `eof2.c`'s measurements: `"%f %f"` and `"%d %d"` on each input, `errno` 7 before; and
    /// `validation/probes/fs-volume/scanf_word_errno.c`'s for an `ERANGE` at the end (`%f` only:
    /// `"%f %f %f %f"`).
    #[test]
    fn errno_at_the_end_of_the_input() {
        for (input, n, errno, float_only) in [
            ("", 0, 0, false),
            (" ", 0, 0, false),
            ("5", 1, 7, false),
            ("5 ", 1, 0, false),
            ("5  ", 1, 0, false),
            ("5 6", 2, 7, false),
            ("1e40", 1, 7, true),
            ("1e40 ", 1, 0, true),
            ("-1877   1e40", 2, 7, true),
            ("1e-50 3", 2, 34, true),
        ] {
            for int in [false, true] {
                if int && float_only {
                    continue;
                }
                let mut b = Bytes::new(input.as_bytes());
                let mut call = Call::new();
                let mut e = 7;
                let mut got = 0;
                for _ in 0..if float_only { 4 } else { 2 } {
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

    /// `validation/probes/fs-volume/scanf_word_errno.c`'s measurements (track C's relay 15; glibc
    /// 2.35, `errno` 7 before each call): every row of its output, for `"%*s %s"`, `"%*s %f"`,
    /// `"%f %f %f %f"` and `"%f"`, through [`Scanf`] (the return value and `errno` after).
    #[test]
    fn word_and_number_calls_at_the_end_of_the_input() {
        let run = |input: &str, f: &dyn Fn(&mut Scanf, &mut i32)| {
            let mut b = Bytes::new(input.as_bytes());
            let mut sc = Scanf::new(&mut b);
            let mut errno = 7;
            f(&mut sc, &mut errno);
            (sc.finish(&mut errno), errno)
        };
        let word_s = |sc: &mut Scanf, e: &mut i32| {
            sc.s(e, usize::MAX, false);
            sc.text(e, b" ");
            sc.s(e, usize::MAX, true);
        };
        let word_f = |sc: &mut Scanf, e: &mut i32| {
            sc.s(e, usize::MAX, false);
            sc.text(e, b" ");
            sc.f(e, true);
        };
        let ffff = |sc: &mut Scanf, e: &mut i32| {
            for k in 0..4 {
                if k > 0 {
                    sc.text(e, b" ");
                }
                sc.f(e, true);
            }
        };
        let f1 = |sc: &mut Scanf, e: &mut i32| {
            sc.f(e, true);
        };
        // (input, %*s %s, %*s %f): the probe's first table
        for (input, s, f) in [
            ("fscale", (-1, 7), (-1, 7)),
            ("fscale ", (-1, 0), (-1, 0)),
            ("fscale\n", (-1, 0), (-1, 0)),
            ("fscale 1e40", (1, 7), (1, 34)),
            ("fscale 1e40\n", (1, 7), (1, 34)),
            ("subject", (-1, 7), (-1, 7)),
            ("subject ", (-1, 0), (-1, 0)),
            ("subject x", (1, 7), (0, 7)),
            ("subject x\n", (1, 7), (0, 7)),
            ("", (-1, 0), (-1, 0)),
            (" ", (-1, 0), (-1, 0)),
            ("fscale x", (1, 7), (0, 7)),
        ] {
            assert_eq!(run(input, &word_s), s, "%*s %s on {input:?}");
            assert_eq!(run(input, &word_f), f, "%*s %f on {input:?}");
        }
        // (input, %f %f %f %f, %f, %*s %f): the probe's second table
        for (input, four, one, word) in [
            ("-1877   1e40", (2, 7), (1, 7), (1, 34)),
            ("-1877   1e40 ", (2, 0), (1, 7), (1, 34)),
            ("1e40", (1, 7), (1, 34), (-1, 7)),
            ("1e40 ", (1, 0), (1, 34), (-1, 0)),
            ("5 1e40", (2, 7), (1, 7), (1, 34)),
            ("1e-50", (1, 7), (1, 34), (-1, 7)),
            ("1e-50 3", (2, 34), (1, 34), (1, 7)),
            ("fscale 1e40", (0, 7), (0, 7), (1, 34)),
            ("1 2", (2, 7), (1, 7), (1, 7)),
        ] {
            assert_eq!(run(input, &ffff), four, "%f %f %f %f on {input:?}");
            assert_eq!(run(input, &f1), one, "%f on {input:?}");
            assert_eq!(run(input, &word_f), word, "%*s %f on {input:?}");
        }
        // track E's fixed calls (specs/freesurfer-maths.md §18.1; glibc 2.35 measured by
        // validation/probes/fs-maths/scanf_errno_cases.py): white space at the format's end reads,
        // so `"%f "` on `1e40` sets errno back to its value from before the overflow
        let fs = |n: usize, blank: bool| {
            move |sc: &mut Scanf, e: &mut i32| {
                for k in 0..n {
                    if k > 0 {
                        sc.text(e, b" ");
                    }
                    sc.f(e, true);
                }
                if blank {
                    sc.text(e, b" ");
                }
            }
        };
        for (input, n, blank, want) in [
            ("1e40", 1, false, (1, 34)),
            ("1e40", 1, true, (1, 7)),
            ("1e40", 2, false, (1, 7)),
            ("1e40 ", 2, false, (1, 0)),
            ("1 2 ", 3, false, (2, 0)),
            ("1e-50 3", 3, false, (2, 34)),
            ("-1877   1e40", 4, false, (2, 7)),
            ("", 1, false, (-1, 0)),
        ] {
            assert_eq!(
                run(input, &fs(n, blank)),
                want,
                "{n} %f, blank {blank}, on {input:?}"
            );
        }
    }

    /// `validation/investigations/glibc-scanf-probe/scanf_errno.c`'s measurements (track D's: the
    /// surface readers' formats; glibc 2.35, `errno` 5 before each call): the return value and
    /// `errno` after.
    #[test]
    fn whole_calls_at_the_end_of_the_input() {
        let run = |input: &str, f: &dyn Fn(&mut Scanf, &mut i32)| {
            let mut b = Bytes::new(input.as_bytes());
            let mut sc = Scanf::new(&mut b);
            let mut errno = 5;
            f(&mut sc, &mut errno);
            (sc.finish(&mut errno), errno)
        };
        let points = |sc: &mut Scanf, e: &mut i32| {
            sc.text(e, b"POINTS ");
            sc.d(e, true);
            sc.text(e, b" float\n");
        };
        assert_eq!(run("POINTS", &points), (-1, 0));
        assert_eq!(run("POINTS ", &points), (-1, 0));
        assert_eq!(run("POINT", &points), (-1, 5));
        assert_eq!(run("POINTSX", &points), (0, 5));
        assert_eq!(run("POINTS 5", &points), (1, 5));
        assert_eq!(run("POINTS 99999999999 float", &points), (1, 5));
        let curv = |sc: &mut Scanf, e: &mut i32| {
            sc.d(e, false);
            for k in 0..4 {
                sc.text(e, b" ");
                sc.f(e, k == 3);
            }
            sc.text(e, b"\n");
        };
        assert_eq!(run("12", &curv), (-1, 5));
        assert_eq!(run("12 1 2 3", &curv), (-1, 5));
        assert_eq!(run("99999999999 1e99 2 3 4", &curv), (1, 34));
        assert_eq!(run("1 2 3 4 1e99", &curv), (1, 5));
        assert_eq!(run("x 2 3 4 5", &curv), (0, 5));
        let field = |sc: &mut Scanf, e: &mut i32| {
            sc.text(e, b"FIELD ");
            sc.s(e, usize::MAX, false);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b"\n");
        };
        assert_eq!(run("FIELD", &field), (-1, 0));
        assert_eq!(run("FIELD a", &field), (-1, 5));
        assert_eq!(run("FIELD a 3", &field), (1, 5));
        let fd = |sc: &mut Scanf, e: &mut i32| {
            sc.f(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
        };
        for (input, want) in [
            ("1e99", (1, 5)),
            ("1e99 ", (1, 0)),
            ("1e99 x", (1, 34)),
            ("inf", (1, 5)),
            ("nan", (1, 0)),
            ("-nan", (1, 0)),
            ("infinity", (1, 0)),
            ("1", (1, 5)),
            ("nan(1", (1, 5)),
            ("in", (0, 5)),
        ] {
            assert_eq!(run(input, &fd), want, "{input:?}");
        }
        assert_eq!(
            run("", &|sc, e| {
                sc.d(e, true);
            }),
            (-1, 0)
        );
        assert_eq!(
            run("", &|sc, e| {
                sc.text(e, b"x");
                sc.d(e, true);
            }),
            (-1, 5)
        );
        // a call of conversions only, the white space read by the caller
        let mut b = Bytes::new(b"1e99");
        let mut call = Call::new();
        let mut e = 5;
        assert!(call.f32(&mut b, &mut e).is_some());
        assert_eq!(e, ERANGE);
        skip_space(&mut b);
        assert!(call.i32(&mut b, &mut e).is_none());
        assert_eq!(e, 5);
        let mut b = Bytes::new(b"nan");
        let mut call = Call::new();
        let mut e = 5;
        call.f32(&mut b, &mut e);
        assert!(call.i32(&mut b, &mut e).is_none());
        assert_eq!(e, 0);
    }

    fn lf(s: &str) -> (Scan<f64>, usize) {
        let mut b = Bytes::new(s.as_bytes());
        let r = scan_f64(&mut b);
        (r, b.i)
    }

    /// `%lf` measured on glibc 2.35 (`validation/probes/fs-io/scanf_lf.c`): bits, `ERANGE`,
    /// bytes read.
    #[test]
    fn glibc_measurements_double() {
        let cases: &[(&str, u64, bool, usize)] = &[
            ("1", 0x3ff0_0000_0000_0000, false, 1),
            ("0.1", 0x3fb9_9999_9999_999a, false, 3),
            ("-2.5e-3", 0xbf64_7ae1_47ae_147b, false, 7),
            ("1e", 0x3ff0_0000_0000_0000, false, 2),
            ("1e+", 0x3ff0_0000_0000_0000, false, 3),
            ("100ergs", 0x4059_0000_0000_0000, false, 4),
            ("1e400", 0x7ff0_0000_0000_0000, true, 5),
            ("1e-400", 0, true, 6),
            ("4.9e-324", 1, true, 8),
            ("2.2250738585072014e-308", 0x0010_0000_0000_0000, false, 23),
            ("2.2250738585072009e-308", 0x000f_ffff_ffff_ffff, true, 23),
            ("1.7976931348623157e308", 0x7fef_ffff_ffff_ffff, false, 22),
            ("1.7976931348623159e308", 0x7ff0_0000_0000_0000, true, 22),
            ("0x1p-1074", 1, false, 9),
            ("0x1p-1075", 0, true, 9),
            ("0x1.8p-1074", 2, true, 11),
            ("0x1.fffffffffffff8p-1023", 0x0010_0000_0000_0000, false, 24),
            ("0x1.fffffffffffffp1023", 0x7fef_ffff_ffff_ffff, false, 22),
            ("0x1.fffffffffffff8p1023", 0x7ff0_0000_0000_0000, true, 23),
            ("0x.p1", 0, false, 3),
            ("infx", 0x7ff0_0000_0000_0000, false, 3),
            ("-nan", 0xfff8_0000_0000_0000, false, 4),
            ("nan(1)", 0x7ff8_0000_0000_0000, false, 3),
            ("-0", 0x8000_0000_0000_0000, false, 2),
            (
                "123456789012345678901234567890",
                0x45f8_ee90_ff6c_373e,
                false,
                30,
            ),
        ];
        for &(s, b, e, n) in cases {
            match lf(s) {
                (Scan::Value(v, er), i) => assert_eq!((v.to_bits(), er, i), (b, e, n), "{s}"),
                other => panic!("{s}: {other:?}"),
            }
        }
        for (s, n) in [
            (".", 1),
            ("-", 1),
            ("0x", 2),
            ("infin", 5),
            ("nax", 3),
            ("e5", 0),
        ] {
            assert_eq!(lf(s), (Scan::Fail, n), "{s}");
        }
        assert_eq!(lf(" ").0, Scan::Eof);
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

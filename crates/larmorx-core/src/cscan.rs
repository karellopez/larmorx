// SPDX-License-Identifier: Apache-2.0
//! C's `sscanf` conversions `%lf` (a `double`), `%f` (a `float`) and `%d` (an `int`) of one string,
//! and `strtod` and `strtof`,
//! as the C library reads them, for ports of C parsers whose values come from `sscanf` (MINC's
//! `.xfm` reader reads every number with `sscanf(token, "%lf", &d)`).
//!
//! Written from the C standard's description (ISO C 7.21.6.2 for `fscanf`'s conversions, 7.22.1.3
//! for `strtod`, 7.22.1.4 for `strtol`), with the C locale's white space (`isspace`: space, `\t`,
//! `\n`, `\v`, `\f`, `\r`). Where the standard leaves room, the choices are glibc 2.35's, read as a
//! black box: `validation/probes/fs-minc-nifti` compares these functions with the C library's on
//! millions of generated strings. glibc's choices, as measured:
//!
//! - `%lf` reads the longest run of characters that can still begin a number (the sign, the
//!   digits, one decimal point, an exponent letter after a digit and its sign; `0x` and the hex
//!   digits and `p`; `inf`, `infinity`, `nan`), then converts that run with `strtod`; the
//!   conversion succeeds when `strtod` uses a prefix of it, so `"1e"` and `"1e+"` read 1 (ISO C's
//!   own example would reject `"100e"`; glibc does not); `"0x"` with nothing after it that the
//!   run takes reads nothing (`"0x."` reads 0),
//!   and `"nan(...)"` reads the default NaN (the parenthesis is not part of the run).
//! - A run that `strtod` cannot use at all (`"-"`, `"."`, `"in"`) is a matching failure: no value.
//! - `%d` converts its run as `strtol` would and keeps the low 32 bits of the `long` result.
//!
//! Decimal strings are rounded correctly (to nearest, ties to even), as glibc's `strtod` does;
//! hexadecimal strings exactly the same way, subnormals included.

/// `isspace` in the C locale.
pub fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn lower(c: u8) -> u8 {
    c.to_ascii_lowercase()
}

/// Whether `s[i..]` begins with `word` (ASCII, any case).
fn starts_ci(s: &[u8], i: usize, word: &[u8]) -> bool {
    s.len() >= i + word.len()
        && s[i..i + word.len()]
            .iter()
            .zip(word)
            .all(|(a, b)| lower(*a) == *b)
}

/// `strtod(s, &end)`: the value and the number of bytes used (0: no conversion, value 0).
/// Leading white space is skipped (and counted when a number follows).
pub fn strtod(s: &[u8]) -> (f64, usize) {
    let (v, n) = strto(s, Prec::Double);
    (f64::from_bits(v), n)
}

/// `strtof(s, &end)`: as [`strtod`], the value rounded once, correctly, to a `float` (never
/// through a `double`); a NaN's payload goes into the significand's low 22 bits.
pub fn strtof(s: &[u8]) -> (f32, usize) {
    let (v, n) = strto(s, Prec::Single);
    (f32::from_bits(v as u32), n)
}

/// The two IEEE formats `strto` rounds to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prec {
    Single,
    Double,
}

impl Prec {
    /// Significand bits (with the leading one) and the largest exponent.
    fn params(self) -> (i64, i64) {
        match self {
            Prec::Single => (24, 127),
            Prec::Double => (53, 1023),
        }
    }
    fn inf(self) -> u64 {
        let (p, emax) = self.params();
        ((2 * emax + 1) as u64) << (p - 1)
    }
    fn neg(self, bits: u64) -> u64 {
        let (p, emax) = self.params();
        bits | (1u64 << (p - 1 + bit_len(2 * emax + 1)))
    }
}

fn bit_len(v: i64) -> i64 {
    64 - (v as u64).leading_zeros() as i64
}

/// `strtod` or `strtof`: the result's bits.
fn strto(s: &[u8], prec: Prec) -> (u64, usize) {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let sign = |v: u64| if neg { prec.neg(v) } else { v };
    // INF / INFINITY
    if starts_ci(s, i, b"inf") {
        let end = if starts_ci(s, i, b"infinity") {
            i + 8
        } else {
            i + 3
        };
        return (sign(prec.inf()), end);
    }
    // NAN / NAN(n-char-sequence)
    if starts_ci(s, i, b"nan") {
        let mut end = i + 3;
        let mut payload = None;
        if end < s.len() && s[end] == b'(' {
            let mut j = end + 1;
            while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'_') {
                j += 1;
            }
            if j < s.len() && s[j] == b')' {
                payload = Some(&s[end + 1..j]);
                end = j + 1;
            }
        }
        return (sign(nan_with_payload(payload, prec)), end);
    }
    // Hexadecimal: 0x, then hex digits with an optional point, then an optional binary exponent.
    if i + 1 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        if let Some((mant, sticky, exp, end)) = hex_float(s, i + 2) {
            let (p, emax) = prec.params();
            return (sign(compose(mant, sticky, exp, p, emax)), end);
        }
        // "0x" without a hex digit after it: the subject is the "0".
        return (sign(0), i + 1);
    }
    // Decimal.
    let mut j = i;
    let mut digits = 0usize;
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < s.len() && s[j] == b'.' {
        j += 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        let _ = start;
        return (0, 0);
    }
    let mantissa_end = j;
    if j < s.len() && lower(s[j]) == b'e' {
        let mut k = j + 1;
        if k < s.len() && (s[k] == b'+' || s[k] == b'-') {
            k += 1;
        }
        if k < s.len() && s[k].is_ascii_digit() {
            while k < s.len() && s[k].is_ascii_digit() {
                k += 1;
            }
            j = k;
        }
    }
    let _ = mantissa_end;
    // Rust's parser rounds decimal strings correctly; give it the subject without its sign
    // (an exponent too large for its integer is clamped first: the value is then 0 or infinity
    // either way).
    let text = std::str::from_utf8(&s[i..j]).expect("ASCII digits");
    let v = match prec {
        Prec::Double => parse_decimal::<f64>(text).to_bits(),
        Prec::Single => parse_decimal::<f32>(text).to_bits() as u64,
    };
    (sign(v), j)
}

/// The two float types `parse_decimal` rounds to.
trait Decimal: std::str::FromStr {
    const ZERO: Self;
    const INFINITY: Self;
}
impl Decimal for f64 {
    const ZERO: f64 = 0.0;
    const INFINITY: f64 = f64::INFINITY;
}
impl Decimal for f32 {
    const ZERO: f32 = 0.0;
    const INFINITY: f32 = f32::INFINITY;
}

/// A decimal subject (digits, an optional point, an optional exponent; no sign), correctly
/// rounded.
fn parse_decimal<T: Decimal>(text: &str) -> T {
    match text.parse::<T>() {
        Ok(v) => v,
        Err(_) => {
            // An exponent beyond Rust's range: split it off and clamp it.
            let (m, e) = text.split_at(text.find(['e', 'E']).expect("an exponent"));
            let e = &e[1..];
            let neg = e.starts_with('-');
            let digits = e.trim_start_matches(['+', '-']);
            let mant_zero = m.bytes().all(|c| c == b'0' || c == b'.');
            if mant_zero {
                T::ZERO
            } else if neg {
                let _ = digits;
                T::ZERO
            } else {
                T::INFINITY
            }
        }
    }
}

/// glibc's NaN for `strtod` (`strtof`): the default quiet NaN, or with a payload from
/// `nan(n-char-sequence)` read as an unsigned number (decimal, `0x` hexadecimal or `0` octal) into
/// the significand's low 51 (22) bits when it is one.
fn nan_with_payload(payload: Option<&[u8]>, prec: Prec) -> u64 {
    let (p, _) = prec.params();
    let quiet: u64 = prec.inf() | (1u64 << (p - 2));
    let low = (1u64 << (p - 2)) - 1;
    let Some(p) = payload else {
        return quiet;
    };
    let (digits, radix) = if p.len() > 1 && p[0] == b'0' && lower(p[1]) == b'x' {
        (&p[2..], 16)
    } else if p.len() > 1 && p[0] == b'0' {
        (&p[1..], 8)
    } else {
        (p, 10)
    };
    let mut value: u64 = 0;
    for &c in digits {
        let d = match c {
            b'0'..=b'9' => (c - b'0') as u64,
            b'a'..=b'f' => (c - b'a' + 10) as u64,
            b'A'..=b'F' => (c - b'A' + 10) as u64,
            _ => return quiet,
        };
        if d >= radix {
            return quiet;
        }
        value = match value.checked_mul(radix).and_then(|v| v.checked_add(d)) {
            Some(v) => v,
            None => return quiet,
        };
    }
    quiet | (value & low)
}

/// The hexadecimal significand at `s[i..]` (after `0x`), its optional point and binary
/// exponent: the significand's leading 64 bits, whether a nonzero tail was dropped, the binary
/// exponent of the bits kept and the end; `None` when no hex digit follows.
fn hex_float(s: &[u8], mut i: usize) -> Option<(u64, bool, i64, usize)> {
    let hexval = |c: u8| -> Option<u64> {
        match c {
            b'0'..=b'9' => Some((c - b'0') as u64),
            b'a'..=b'f' => Some((c - b'a' + 10) as u64),
            b'A'..=b'F' => Some((c - b'A' + 10) as u64),
            _ => None,
        }
    };
    // The significand: up to 64 significant bits kept, the rest folded into a sticky bit.
    let mut mant: u64 = 0;
    let mut sticky = false;
    let mut exp: i64 = 0; // binary exponent adjustment for the digits kept
    let mut any = false;
    let mut seen_point = false;
    loop {
        if i < s.len() && !seen_point && s[i] == b'.' {
            seen_point = true;
            i += 1;
            continue;
        }
        let Some(d) = s.get(i).copied().and_then(hexval) else {
            break;
        };
        any = true;
        if mant >> 60 == 0 {
            mant = (mant << 4) | d;
            if seen_point {
                exp -= 4;
            }
        } else {
            sticky |= d != 0;
            if !seen_point {
                exp += 4;
            }
        }
        i += 1;
    }
    if !any {
        return None;
    }
    if i < s.len() && lower(s[i]) == b'p' {
        let mut k = i + 1;
        let mut eneg = false;
        if k < s.len() && (s[k] == b'+' || s[k] == b'-') {
            eneg = s[k] == b'-';
            k += 1;
        }
        if k < s.len() && s[k].is_ascii_digit() {
            let mut e: i64 = 0;
            while k < s.len() && s[k].is_ascii_digit() {
                e = (e * 10 + (s[k] - b'0') as i64).min(1 << 40);
                k += 1;
            }
            exp += if eneg { -e } else { e };
            i = k;
        }
    }
    Some((mant, sticky, exp, i))
}

/// `mant × 2^exp` (with `sticky`: a nonzero tail below `mant`'s last bit) rounded to the nearest
/// value of the format with `p` significand bits and largest exponent `emax`, ties to even,
/// subnormals and overflow included: its bits (positive).
fn compose(mant: u64, sticky: bool, exp: i64, p: i64, emax: i64) -> u64 {
    if mant == 0 {
        return 0;
    }
    let emin = 1 - emax;
    let frac = p - 1;
    let inf = ((2 * emax + 1) as u64) << frac;
    // Normalise: the leading bit at position 63.
    let lz = mant.leading_zeros() as i64;
    let m = mant << lz;
    // value = m × 2^(exp - lz), m in [2^63, 2^64).
    let e2 = exp - lz + 63; // the unbiased exponent of the leading bit
    if e2 > emax {
        return inf;
    }
    // Bits to keep: p for normal numbers, fewer for subnormals.
    let keep: i64 = if e2 >= emin { p } else { p - (emin - e2) };
    if keep <= 0 {
        // Below half the smallest subnormal, or exactly at it (round to even: zero), or above.
        if keep == 0 {
            // the value is in [2^(emin-p), 2^(emin-p+1)): half the smallest subnormal or more
            let half = m == 1u64 << 63 && !sticky;
            return if half { 0 } else { 1 };
        }
        return 0;
    }
    let drop = 64 - keep; // bits of m below the kept ones
    let kept = m >> drop;
    let rem = m & ((1u64 << drop) - 1);
    let halfway = 1u64 << (drop - 1);
    let round_up = rem > halfway || (rem == halfway && (sticky || kept & 1 == 1));
    let mut q = kept + round_up as u64;
    let mut e = e2;
    if e2 >= emin {
        if q == 1u64 << p {
            q >>= 1;
            e += 1;
            if e > emax {
                return inf;
            }
        }
        (((e + emax) as u64) << frac) | (q & ((1u64 << frac) - 1))
    } else {
        // Subnormal: q counts units of the smallest subnormal (it may round up to the smallest
        // normal, whose bits are the same number).
        q
    }
}

/// The characters `%lf` reads from `s[i..]` (after the white space): the end of the longest run
/// that can still begin a number, in glibc's way.
fn float_run(s: &[u8], mut i: usize) -> usize {
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    if i < s.len() && lower(s[i]) == b'n' {
        // "nan" (what follows, a parenthesis included, is not read).
        return if starts_ci(s, i, b"nan") {
            i + 3
        } else {
            usize::MAX
        };
    }
    if i < s.len() && lower(s[i]) == b'i' {
        if !starts_ci(s, i, b"inf") {
            return usize::MAX;
        }
        let j = i + 3;
        if j < s.len() && lower(s[j]) == b'i' {
            return if starts_ci(s, j, b"inity") {
                j + 5
            } else {
                usize::MAX
            };
        }
        return j;
    }
    let mut hex = false;
    let mut got_digit = false;
    let mut after_prefix = usize::MAX;
    if i + 1 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        // "0x" alone reads nothing; with anything read after it, strtod decides.
        hex = true;
        i += 2;
        after_prefix = i;
    }
    let mut got_point = false;
    let mut got_e = false;
    while i < s.len() {
        let c = s[i];
        let is_digit = if hex {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        };
        if is_digit && !(got_e && hex && !c.is_ascii_digit()) {
            got_digit = true;
            i += 1;
        } else if c == b'.' && !got_point && !got_e {
            got_point = true;
            i += 1;
        } else if !got_e && got_digit && lower(c) == if hex { b'p' } else { b'e' } {
            got_e = true;
            i += 1;
            if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
                i += 1;
            }
        } else {
            break;
        }
    }
    if hex && i == after_prefix {
        return usize::MAX;
    }
    i
}

/// `sscanf(s, "%lf", &d) == 1`: the value, or `None` (a matching failure or an empty string).
pub fn sscanf_f64(s: &[u8]) -> Option<f64> {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return None;
    }
    let end = float_run(s, i);
    if end == usize::MAX || end == i {
        return None;
    }
    let (v, used) = strtod(&s[i..end]);
    if used == 0 { None } else { Some(v) }
}

/// `sscanf(s, "%f", &f) == 1`: the value, or `None` (as [`sscanf_f64`], converted by [`strtof`]).
pub fn sscanf_f32(s: &[u8]) -> Option<f32> {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return None;
    }
    let end = float_run(s, i);
    if end == usize::MAX || end == i {
        return None;
    }
    let (v, used) = strtof(&s[i..end]);
    if used == 0 { None } else { Some(v) }
}

/// `strtol(s, &end, 10)` on 64-bit `long`: the value (clamped to the `long` range on overflow)
/// and the bytes used (0: no conversion).
pub fn strtol10(s: &[u8]) -> (i64, usize) {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let first = i;
    let mut v: i128 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = (v * 10 + (s[i] - b'0') as i128).min(i64::MAX as i128 + 1);
        i += 1;
    }
    if i == first {
        return (0, 0);
    }
    let v = if neg { -v } else { v };
    (v.clamp(i64::MIN as i128, i64::MAX as i128) as i64, i)
}

/// `sscanf(s, "%d", &i) == 1`: the value, or `None`.
pub fn sscanf_i32(s: &[u8]) -> Option<i32> {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return None;
    }
    let (v, used) = strtol10(&s[i..]);
    if used == 0 { None } else { Some(v as i32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: &str) -> Option<u64> {
        sscanf_f64(s.as_bytes()).map(f64::to_bits)
    }

    #[test]
    fn plain_numbers() {
        assert_eq!(sscanf_f64(b"1.5"), Some(1.5));
        assert_eq!(sscanf_f64(b"  -2e3xyz"), Some(-2000.0));
        assert_eq!(sscanf_f64(b"0.1"), Some(0.1));
        assert_eq!(sscanf_f64(b"1e"), Some(1.0));
        assert_eq!(sscanf_f64(b"1e+"), Some(1.0));
        assert_eq!(sscanf_f64(b"0x"), None);
        assert_eq!(strtod(b"0x").1, 1);
        assert_eq!(sscanf_f64(b"0x1p-1"), Some(0.5));
        assert_eq!(sscanf_f64(b"0x1.8p1"), Some(3.0));
        assert_eq!(sscanf_f64(b"-"), None);
        assert_eq!(sscanf_f64(b"."), None);
        assert_eq!(sscanf_f64(b""), None);
        assert_eq!(sscanf_f64(b"x"), None);
        assert_eq!(sscanf_f64(b"inf"), Some(f64::INFINITY));
        assert_eq!(sscanf_f64(b"-Infinity"), Some(f64::NEG_INFINITY));
        assert!(sscanf_f64(b"nan").unwrap().is_nan());
        assert_eq!(bits("1e400"), Some(f64::INFINITY.to_bits()));
        assert_eq!(bits("1e-400"), Some(0));
        assert_eq!(bits("4.9406564584124654e-324"), Some(1));
        assert_eq!(bits("0x1p-1074"), Some(1));
        assert_eq!(bits("0x1p-1075"), Some(0));
        assert_eq!(bits("0x1.0000000000001p-1075"), Some(1));
        assert_eq!(
            bits("0x1.fffffffffffff8p1023"),
            Some(f64::INFINITY.to_bits())
        );
    }

    #[test]
    fn integers() {
        assert_eq!(sscanf_i32(b"42"), Some(42));
        assert_eq!(sscanf_i32(b"-7x"), Some(-7));
        assert_eq!(sscanf_i32(b"4294967297"), Some(1));
        assert_eq!(sscanf_i32(b"x"), None);
        assert_eq!(sscanf_i32(b"+"), None);
    }

    #[test]
    fn floats_rounded_once() {
        // glibc's strtof (validation/probes/fs-gifti's scan family: 20,000 strings, 0 differ)
        let b = |s: &str| sscanf_f32(s.as_bytes()).map(f32::to_bits);
        assert_eq!(b("1.0"), Some(0x3f80_0000));
        assert_eq!(b(" 1.0x"), Some(0x3f80_0000));
        assert_eq!(b("abc"), None);
        assert_eq!(b(""), None);
        // a value just above a float midpoint that a double would round onto it
        assert_eq!(b("1.00000017881393432617187501"), Some(0x3f80_0002));
        assert_eq!(b("1.000000178813934326171875"), Some(0x3f80_0002));
        assert_eq!(b("1.00000017881393432617187499"), Some(0x3f80_0001));
        assert_eq!(b("3.4028236e38"), Some(0x7f80_0000));
        assert_eq!(b("7e-46"), Some(0)); // below half the smallest subnormal
        assert_eq!(b("7.1e-46"), Some(0x0000_0001));
        assert_eq!(b("1e-46"), Some(0));
        assert_eq!(b("0x1.ffffffp127"), Some(0x7f80_0000));
        assert_eq!(b("0x1p-149"), Some(1));
        assert_eq!(b("-nan"), Some(0xffc0_0000));
        assert_eq!(b("nan(0x7f)"), Some(0x7fc0_0000));
        assert_eq!(b("-Infinity"), Some(0xff80_0000));
        assert_eq!(strtof(b"nan(0x7f)").0.to_bits(), 0x7fc0_007f);
    }
}

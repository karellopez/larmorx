// SPDX-License-Identifier: Apache-2.0
//! C's `sscanf` conversions `%lf` (a `double`) and `%d` (an `int`) of one string, and `strtod`,
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
//!   digits and `p`; `inf`, `infinity`, `nan`, `nan(...)`), then converts that run with `strtod`;
//!   the conversion succeeds when `strtod` uses a prefix of it, so `"1e"` and `"1e+"` read 1 and
//!   `"0x"` reads 0 (ISO C's own example would reject `"100e"`; glibc does not).
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
    s.len() >= i + word.len() && s[i..i + word.len()].iter().zip(word).all(|(a, b)| lower(*a) == *b)
}

/// `strtod(s, &end)`: the value and the number of bytes used (0: no conversion, value 0).
/// Leading white space is skipped (and counted when a number follows).
pub fn strtod(s: &[u8]) -> (f64, usize) {
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
    let sign = |v: f64| if neg { -v } else { v };
    // INF / INFINITY
    if starts_ci(s, i, b"inf") {
        let end = if starts_ci(s, i, b"infinity") { i + 8 } else { i + 3 };
        return (sign(f64::INFINITY), end);
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
        return (sign(nan_with_payload(payload)), end);
    }
    // Hexadecimal: 0x, then hex digits with an optional point, then an optional binary exponent.
    if i + 1 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        if let Some((v, end)) = hex_float(s, i + 2) {
            return (sign(v), end);
        }
        // "0x" without a hex digit after it: the subject is the "0".
        return (sign(0.0), i + 1);
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
        return (0.0, 0);
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
    let v = parse_decimal(text);
    (sign(v), j)
}

/// A decimal subject (digits, an optional point, an optional exponent; no sign), correctly
/// rounded.
fn parse_decimal(text: &str) -> f64 {
    match text.parse::<f64>() {
        Ok(v) => v,
        Err(_) => {
            // An exponent beyond Rust's range: split it off and clamp it.
            let (m, e) = text.split_at(text.find(['e', 'E']).expect("an exponent"));
            let e = &e[1..];
            let neg = e.starts_with('-');
            let digits = e.trim_start_matches(['+', '-']);
            let mant_zero = m.bytes().all(|c| c == b'0' || c == b'.');
            if mant_zero {
                0.0
            } else if neg {
                let _ = digits;
                0.0
            } else {
                f64::INFINITY
            }
        }
    }
}

/// glibc's NaN for `strtod`: the default quiet NaN, or with a payload from `nan(n-char-sequence)`
/// read as an unsigned number (decimal, `0x` hexadecimal or `0` octal) into the significand's low
/// 51 bits when it is one.
fn nan_with_payload(payload: Option<&[u8]>) -> f64 {
    const QUIET: u64 = 0x7ff8_0000_0000_0000;
    let Some(p) = payload else {
        return f64::from_bits(QUIET);
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
            _ => return f64::from_bits(QUIET),
        };
        if d >= radix {
            return f64::from_bits(QUIET);
        }
        value = match value.checked_mul(radix).and_then(|v| v.checked_add(d)) {
            Some(v) => v,
            None => return f64::from_bits(QUIET),
        };
    }
    f64::from_bits(QUIET | (value & ((1u64 << 51) - 1)))
}

/// The hexadecimal significand at `s[i..]` (after `0x`), its optional point and binary
/// exponent: the value and the end, or `None` when no hex digit follows.
fn hex_float(s: &[u8], mut i: usize) -> Option<(f64, usize)> {
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
    Some((compose(mant, sticky, exp), i))
}

/// `mant × 2^exp` (with `sticky`: a nonzero tail below `mant`'s last bit) rounded to the nearest
/// double, ties to even, subnormals and overflow included.
fn compose(mant: u64, sticky: bool, exp: i64) -> f64 {
    if mant == 0 {
        return 0.0;
    }
    // Normalise: the leading bit at position 63.
    let lz = mant.leading_zeros() as i64;
    let m = mant << lz;
    // value = m × 2^(exp - lz), m in [2^63, 2^64).
    let e2 = exp - lz + 63; // the unbiased exponent of the leading bit
    if e2 > 1023 {
        return f64::INFINITY;
    }
    // Bits to keep: 53 for normal numbers, fewer for subnormals.
    let keep: i64 = if e2 >= -1022 { 53 } else { 53 - (-1022 - e2) };
    if keep <= 0 {
        // Below half the smallest subnormal, or exactly at it (round to even: zero), or above.
        if keep == 0 {
            // m × 2^(e2-63) with e2 = -1075: the value is in [2^-1075, 2^-1074).
            let half = m == 1u64 << 63 && !sticky;
            return if half { 0.0 } else { f64::from_bits(1) };
        }
        return 0.0;
    }
    let drop = 64 - keep; // bits of m below the kept ones
    let kept = m >> drop;
    let rem = m & ((1u64 << drop) - 1);
    let halfway = 1u64 << (drop - 1);
    let round_up = rem > halfway || (rem == halfway && (sticky || kept & 1 == 1));
    let round_up = round_up || (rem == halfway && sticky);
    let mut q = kept + round_up as u64;
    let mut e = e2;
    if e2 >= -1022 {
        if q == 1u64 << 53 {
            q >>= 1;
            e += 1;
            if e > 1023 {
                return f64::INFINITY;
            }
        }
        let bits = (((e + 1023) as u64) << 52) | (q & ((1u64 << 52) - 1));
        f64::from_bits(bits)
    } else {
        // Subnormal: q counts units of 2^-1074 (it may round up to the smallest normal).
        f64::from_bits(q)
    }
}

/// The characters `%lf` reads from `s[i..]` (after the white space): the end of the longest run
/// that can still begin a number, in glibc's way.
fn float_run(s: &[u8], mut i: usize) -> usize {
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    if i < s.len() && lower(s[i]) == b'n' {
        // "nan", then an optional "(n-char-sequence)".
        if !starts_ci(s, i, b"nan") {
            return usize::MAX;
        }
        let mut j = i + 3;
        if j < s.len() && s[j] == b'(' {
            let mut k = j + 1;
            while k < s.len() && (s[k].is_ascii_alphanumeric() || s[k] == b'_') {
                k += 1;
            }
            if k < s.len() && s[k] == b')' {
                j = k + 1;
            } else {
                return usize::MAX;
            }
        }
        return j;
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
    if i + 1 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        hex = true;
        i += 2;
        got_digit = true;
    }
    let mut got_point = false;
    let mut got_e = false;
    while i < s.len() {
        let c = s[i];
        let is_digit = if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() };
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
        assert_eq!(sscanf_f64(b"0x"), Some(0.0));
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
        assert_eq!(bits("0x1.fffffffffffff8p1023"), Some(f64::INFINITY.to_bits()));
    }

    #[test]
    fn integers() {
        assert_eq!(sscanf_i32(b"42"), Some(42));
        assert_eq!(sscanf_i32(b"-7x"), Some(-7));
        assert_eq!(sscanf_i32(b"4294967297"), Some(1));
        assert_eq!(sscanf_i32(b"x"), None);
        assert_eq!(sscanf_i32(b"+"), None);
    }
}

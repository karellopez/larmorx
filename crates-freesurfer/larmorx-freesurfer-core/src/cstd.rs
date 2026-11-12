// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's conversions as FreeSurfer's `fs.core` meets them, written from the C
//! standard (C11 7.22.1, 7.21.6) and glibc 2.35's documented behaviour, no glibc code: `atoi`,
//! `atof` (`strtod`'s prefix), `sscanf`'s `%lx`, `printf`'s `%.Nf`, and x86-64's conversion of a
//! double to `int` (`cvttsd2si`). larmorx's own code; candidates for `larmorx-core` with the
//! C-library item (queue item 3.1-08), where every FreeSurfer crate can share them.

/// `isspace` in the C locale: space, `\t`, `\n`, `\v`, `\f`, `\r`.
pub fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `strtol(s, NULL, 10)` on a 64-bit `long`: leading white space, a sign, decimal digits;
/// out of range: `LONG_MAX` or `LONG_MIN`; no digits: 0.
pub fn strtol10(s: &[u8]) -> i64 {
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let neg = match s.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let mut v: i128 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = (v * 10 + i128::from(s[i] - b'0')).min(i128::from(i64::MAX) + 1);
        i += 1;
    }
    let v = if neg { -v } else { v };
    v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

/// glibc's `atoi(s)`: `(int) strtol(s, NULL, 10)`, the `long` cut to its low 32 bits.
pub fn atoi(s: &[u8]) -> i32 {
    strtol10(s) as i32
}

/// The longest prefix `strtod` reads after skipping white space, as its value (0 when there is
/// none): decimal numbers (correctly rounded, as glibc), `inf`, `infinity`, `nan`, `nan(...)`
/// (any case), and hexadecimal ones (`0x1.8p3`).
pub fn atof(s: &[u8]) -> f64 {
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let start = i;
    let neg = matches!(s.get(i), Some(b'-'));
    if matches!(s.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let sign = if neg { -1.0 } else { 1.0 };
    let rest = &s[i..];
    let starts =
        |w: &str| rest.len() >= w.len() && rest[..w.len()].eq_ignore_ascii_case(w.as_bytes());
    if starts("inf") {
        return sign * f64::INFINITY;
    }
    if starts("nan") {
        return if neg { -f64::NAN } else { f64::NAN };
    }
    if (starts("0x"))
        && rest.len() > 2
        && (rest[2].is_ascii_hexdigit()
            || (rest[2] == b'.' && rest.get(3).is_some_and(u8::is_ascii_hexdigit)))
    {
        return sign * hex_float(&rest[2..]);
    }
    // the decimal prefix: digits [. digits] [e [sign] digits]
    let mut j = 0;
    let mut digits = 0;
    while j < rest.len() && rest[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < rest.len() && rest[j] == b'.' {
        j += 1;
        while j < rest.len() && rest[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if j < rest.len() && (rest[j] == b'e' || rest[j] == b'E') {
        let mut k = j + 1;
        if k < rest.len() && (rest[k] == b'+' || rest[k] == b'-') {
            k += 1;
        }
        if k < rest.len() && rest[k].is_ascii_digit() {
            while k < rest.len() && rest[k].is_ascii_digit() {
                k += 1;
            }
            j = k;
        }
    }
    let text = std::str::from_utf8(&s[start..i + j]).unwrap_or("0");
    text.parse::<f64>().unwrap_or(0.0)
}

/// A hexadecimal significand and its optional binary exponent (after `0x`), rounded to the
/// nearest double, ties to even. (Exact up to 32 significant hexadecimal digits; more only
/// matter for ties, and in the subnormal range the last bit may differ.)
fn hex_float(s: &[u8]) -> f64 {
    let mut mant: u128 = 0;
    let mut exp: i64 = 0;
    let mut sticky = false;
    let mut i = 0;
    let mut seen_point = false;
    while i < s.len() {
        let c = s[i];
        if c == b'.' && !seen_point {
            seen_point = true;
        } else if let Some(d) = (c as char).to_digit(16) {
            if mant >> 120 == 0 {
                mant = (mant << 4) | u128::from(d);
                if seen_point {
                    exp -= 4;
                }
            } else {
                sticky |= d != 0;
                if !seen_point {
                    exp += 4;
                }
            }
        } else {
            break;
        }
        i += 1;
    }
    if i < s.len() && (s[i] == b'p' || s[i] == b'P') {
        let mut k = i + 1;
        let neg = matches!(s.get(k), Some(b'-'));
        if matches!(s.get(k), Some(b'-' | b'+')) {
            k += 1;
        }
        if k < s.len() && s[k].is_ascii_digit() {
            let mut e: i64 = 0;
            while k < s.len() && s[k].is_ascii_digit() {
                e = (e * 10 + i64::from(s[k] - b'0')).min(1 << 40);
                k += 1;
            }
            exp += if neg { -e } else { e };
        }
    }
    if mant == 0 {
        return 0.0;
    }
    let m = (mant << 1) | u128::from(sticky);
    let mut x = m as f64 * 0.5;
    // scale by 2^exp in steps that stay exact while the result is normal
    let mut e = exp;
    while e > 0 {
        let step = e.min(1000);
        x *= 2f64.powi(step as i32);
        e -= step;
    }
    while e < 0 {
        let step = (-e).min(1000);
        x /= 2f64.powi(step as i32);
        e += step;
    }
    x
}

/// glibc's `sscanf(s, "0x%lx", &v)`: the literal `0x`, then `%lx` (white space skipped, a sign,
/// an optional `0x` or `0X`, hexadecimal digits; beyond 64 bits `ULONG_MAX`, and a minus sign
/// negates modulo 2^64). `None` when the conversion fails (the variable is left as it was).
pub fn scan_0x_lx(s: &[u8]) -> Option<u64> {
    let s = s.strip_prefix(b"0x")?;
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let neg = matches!(s.get(i), Some(b'-'));
    if matches!(s.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    if s.len() >= i + 3 && s[i] == b'0' && (s[i + 1] | 0x20) == b'x' && s[i + 2].is_ascii_hexdigit()
    {
        i += 2;
    }
    let mut v: u64 = 0;
    let mut any = false;
    let mut overflow = false;
    while i < s.len() {
        let Some(d) = (s[i] as char).to_digit(16) else {
            break;
        };
        any = true;
        match v.checked_mul(16).and_then(|x| x.checked_add(u64::from(d))) {
            Some(x) => v = x,
            None => overflow = true,
        }
        i += 1;
    }
    if !any {
        return None;
    }
    let v = if overflow { u64::MAX } else { v };
    Some(if neg { v.wrapping_neg() } else { v })
}

/// x86-64's conversion of a double to `int` (`cvttsd2si`, what GCC emits for `(int) x`): the
/// value truncated toward zero, or `INT_MIN` when it is out of range or NaN (C leaves those
/// undefined).
pub fn double_to_int_x86(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    }
}

/// `printf("%W.Pf", x)`: `P` decimals, correctly rounded (ties to even on the exact value, as
/// glibc), right-aligned in at least `W` characters; `inf`, `-inf`, `nan`, `-nan` as glibc
/// writes them.
pub fn fmt_f(x: f64, width: usize, prec: usize) -> String {
    let body = if x.is_nan() {
        if x.is_sign_negative() {
            "-nan".to_owned()
        } else {
            "nan".to_owned()
        }
    } else if x.is_infinite() {
        if x < 0.0 {
            "-inf".to_owned()
        } else {
            "inf".to_owned()
        }
    } else {
        format!("{x:.prec$}")
    };
    format!("{body:>width$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_as_glibc() {
        assert_eq!(atoi(b"  -12x"), -12);
        assert_eq!(atoi(b"+7"), 7);
        assert_eq!(atoi(b"x"), 0);
        // strtol clamps to LONG_MAX, whose low 32 bits are -1
        assert_eq!(atoi(b"99999999999999999999"), -1);
        assert_eq!(atoi(b"4294967297"), 1);
        assert_eq!(atoi(b"-99999999999999999999"), 0);
    }

    #[test]
    fn atof_prefixes() {
        assert_eq!(atof(b" 2.5e1x"), 25.0);
        assert_eq!(atof(b"1e"), 1.0);
        assert_eq!(atof(b"-.5"), -0.5);
        assert_eq!(atof(b"abc"), 0.0);
        assert_eq!(atof(b"INFINITY"), f64::INFINITY);
        assert!(atof(b"nan(0x1)").is_nan());
        assert_eq!(atof(b"0x1.8p1"), 3.0);
        assert_eq!(atof(b"0x10"), 16.0);
        assert_eq!(atof(b"0x"), 0.0);
    }

    #[test]
    fn scan_hex() {
        assert_eq!(scan_0x_lx(b"0x10"), Some(16));
        assert_eq!(scan_0x_lx(b"0x 0x1f"), Some(31));
        assert_eq!(scan_0x_lx(b"0X10"), None);
        assert_eq!(scan_0x_lx(b"10"), None);
        assert_eq!(scan_0x_lx(b"0xg"), None);
        assert_eq!(scan_0x_lx(b"0x-1"), Some(u64::MAX));
        assert_eq!(scan_0x_lx(b"0x1ffffffffffffffff"), Some(u64::MAX));
    }

    #[test]
    fn conversions_and_printf() {
        assert_eq!(double_to_int_x86(-2.9), -2);
        assert_eq!(double_to_int_x86(3e9), i32::MIN);
        assert_eq!(double_to_int_x86(f64::NAN), i32::MIN);
        assert_eq!(double_to_int_x86(-2147483648.5), i32::MIN);
        assert_eq!(fmt_f(5.0, 2, 1), "5.0");
        assert_eq!(fmt_f(0.25, 4, 1), " 0.2");
        assert_eq!(fmt_f(-f64::NAN, 2, 1), "-nan");
    }
}

// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Written for the translation of FSL
// 6.0.7.17; it holds no FSL code.
//! C and C++ number semantics as FSL's x86-64 binaries have them: conversions of floating
//! values to integers, and the text C++ streams and `printf` write for numbers.
//!
//! **Conversions.** C leaves the conversion of an out-of-range or NaN floating value to an
//! integer undefined; on x86-64 GCC emits `cvttss2si`/`cvttsd2si`, which truncate toward zero
//! and give the "integer indefinite" value (the most negative integer of the register's width)
//! for NaN and out-of-range inputs. A conversion to `unsigned int` uses the 64-bit instruction
//! and keeps the low 32 bits. Rust's `as` saturates instead and maps NaN to 0, so code that a
//! NaN or a huge value can reach uses these functions.
//!
//! **Text.** `std::ostream << double` with the default flags writes `printf("%.*g", 6, v)`;
//! `std::fixed` gives `"%.*f"`. glibc prints the exact binary value rounded to nearest, ties
//! to even, as Rust's formatting does; only the shape of the output differs, which
//! [`fmt_g`] and [`fmt_f`] reproduce (exponents with a sign and at least two digits, trailing
//! zeros removed by `%g`, `nan`, `-nan`, `inf`).

/// `(int) x` for a `float` on x86-64 (`cvttss2si`, 32-bit).
#[inline]
pub fn f32_to_i32(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `(int) x` for a `double` on x86-64 (`cvttsd2si`, 32-bit).
#[inline]
pub fn f64_to_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `(int64_t) x` or `(long) x` for a `float` on x86-64 (`cvttss2si`, 64-bit).
#[inline]
pub fn f32_to_i64(x: f32) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// `(int64_t) x` for a `double` on x86-64 (`cvttsd2si`, 64-bit).
#[inline]
pub fn f64_to_i64(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// `(unsigned int) x` for a `float` on x86-64: the 64-bit conversion, low 32 bits kept.
#[inline]
pub fn f32_to_u32(x: f32) -> u32 {
    f32_to_i64(x) as u32
}

/// `(short) x`, `(char) x` and other narrow conversions of a `float` on x86-64: the 32-bit
/// conversion, low bits kept (GCC converts to `int` and stores the low bytes).
#[inline]
pub fn f32_to_i16(x: f32) -> i16 {
    f32_to_i32(x) as i16
}

/// `(char) x` of a `float` on x86-64 (signed `char`; the low byte of the 32-bit conversion).
#[inline]
pub fn f32_to_i8(x: f32) -> i8 {
    f32_to_i32(x) as i8
}

/// `printf("%.*f", prec, v)` (glibc).
pub fn fmt_f(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    format!("{v:.prec$}")
}

/// `printf("%.*e", prec, v)` (glibc): `d.ddde±XX`, at least two exponent digits.
pub fn fmt_e(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    let s = format!("{v:.prec$e}");
    let (mant, exp) = s.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent digits");
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{mant}e{sign}{:02}", exp.abs())
}

/// `printf("%.*g", prec, v)` (glibc), which is also what `std::ostream << v` writes with the
/// stream's default flags and precision `prec`.
pub fn fmt_g(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    let p = prec.max(1);
    // The exponent of the value once rounded to p significant digits.
    let e_form = format!("{v:.prec$e}", prec = p - 1);
    let (_, exp) = e_form.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent digits");
    let body = if x < -4 || x >= p as i32 {
        fmt_e(v, p - 1)
    } else {
        let frac = (p as i32 - 1 - x) as usize;
        format!("{v:.frac$}")
    };
    strip_zeros(&body)
}

/// `%g`'s removal of trailing zeros (and of a bare decimal point), mantissa only.
fn strip_zeros(s: &str) -> String {
    let (mant, exp) = match s.find('e') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    };
    let mant = if mant.contains('.') {
        mant.trim_end_matches('0').trim_end_matches('.')
    } else {
        mant
    };
    format!("{mant}{exp}")
}

/// glibc's text for NaN and infinities (`nan`, `-nan`, `inf`, `-inf`).
fn special(v: f64) -> Option<String> {
    if v.is_nan() {
        Some(if v.is_sign_negative() { "-nan" } else { "nan" }.to_owned())
    } else if v.is_infinite() {
        Some(if v < 0.0 { "-inf" } else { "inf" }.to_owned())
    } else {
        None
    }
}

/// `std::ostream << v` for a `double` or a `float` (promoted) with the default flags and the
/// default precision 6.
pub fn stream(v: f64) -> String {
    fmt_g(v, 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_follow_x86() {
        assert_eq!(f32_to_i32(f32::NAN), i32::MIN);
        assert_eq!(f32_to_i32(3e9), i32::MIN);
        assert_eq!(f32_to_i32(-2.7), -2);
        assert_eq!(f32_to_i64(f32::NAN), i64::MIN);
        assert_eq!(f32_to_u32(f32::NAN), 0);
        assert_eq!(f32_to_u32(-1.0), u32::MAX);
        assert_eq!(f32_to_u32(7.9), 7);
        assert_eq!(f32_to_i16(70000.0), (70000_i32) as i16);
        assert_eq!(f64_to_i32(-2147483648.5), i32::MIN);
        assert_eq!(f64_to_i32(-2147483648.0), i32::MIN);
    }

    #[test]
    fn printf_shapes() {
        assert_eq!(fmt_g(0.0, 6), "0");
        assert_eq!(fmt_g(-0.0, 6), "-0");
        assert_eq!(fmt_g(1.0, 6), "1");
        assert_eq!(fmt_g(0.1, 6), "0.1");
        assert_eq!(fmt_g(123456.0, 6), "123456");
        assert_eq!(fmt_g(1234567.0, 6), "1.23457e+06");
        assert_eq!(fmt_g(0.0001, 6), "0.0001");
        assert_eq!(fmt_g(0.00001234, 6), "1.234e-05");
        assert_eq!(fmt_g(999999.5, 6), "1e+06");
        assert_eq!(fmt_g(-3.14159265, 6), "-3.14159");
        assert_eq!(fmt_g(1e-300, 6), "1e-300");
        assert_eq!(fmt_g(f64::NAN, 6), "nan");
        assert_eq!(fmt_g(-f64::NAN, 6), "-nan");
        assert_eq!(fmt_g(f64::NEG_INFINITY, 6), "-inf");
        assert_eq!(fmt_f(1.0, 6), "1.000000");
        assert_eq!(fmt_f(-0.0, 6), "-0.000000");
        assert_eq!(fmt_f(-1e-9, 6), "-0.000000");
        assert_eq!(fmt_f(2.5e-7, 6), "0.000000");
        assert_eq!(fmt_f(5e-7, 6), "0.000000");
        assert_eq!(fmt_f(5.000001e-7, 6), "0.000001");
        assert_eq!(fmt_e(12345.678, 3), "1.235e+04");
    }
}

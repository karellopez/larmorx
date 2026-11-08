// SPDX-License-Identifier: Apache-2.0
//! Numbers as C's `printf` formats them, for AFNI's messages: `%g` (6 significant digits,
//! trailing zeros removed, exponent form below 1e-4 and from 1e6) and `%f` (6 decimals). Rust's
//! formatting rounds the exact binary value to nearest, ties to even, as glibc does.

/// `%g`.
pub fn g(v: f64) -> String {
    g_prec(v, 6)
}

/// `%.<p>g`.
pub fn g_prec(v: f64, p: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let p = p.max(1);
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = sci.split_once('e').expect("an exponent");
    let x: i32 = exp.parse().expect("an exponent");
    if !(-4..p as i32).contains(&x) {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), x.abs())
    } else {
        let decimals = (p as i32 - 1 - x).max(0) as usize;
        strip(&format!("{v:.decimals$}")).to_owned()
    }
}

/// `%f`.
pub fn f(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    format!("{v:.6}")
}

/// `%<w>.<p>g`: right-aligned in `w` characters.
pub fn g_width(v: f64, w: usize, p: usize) -> String {
    format!("{:>w$}", g_prec(v, p))
}

/// `%.<d>f` (C's `nan`, `inf` for those values).
pub fn fixed(v: f64, d: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    format!("{v:.d$}")
}

/// `%<w>.<d>f`.
pub fn fixed_width(v: f64, w: usize, d: usize) -> String {
    format!("{:>w$}", fixed(v, d))
}

/// `%+.<d>f`.
pub fn fixed_signed(v: f64, d: usize) -> String {
    let s = fixed(v, d);
    if s.starts_with('-') || s.starts_with('+') {
        s
    } else {
        format!("+{s}")
    }
}

/// `%.1f`.
pub fn f1(v: f64) -> String {
    fixed(v, 1)
}

/// `%g` of a float32 value (C promotes it to double).
pub fn g32(v: f32) -> String {
    g(f64::from(v))
}

fn strip(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_printf() {
        // Values AFNI 25.2.09's 3dTshift printed (observed).
        assert_eq!(g(2.0), "2");
        assert_eq!(g(0.75), "0.75");
        assert_eq!(g(-0.0), "-0");
        assert_eq!(g32(0.012_345_679), "0.0123457");
        assert_eq!(g32(1.234_567_9), "1.23457");
        assert_eq!(g32(1.234e-5), "1.234e-05");
        assert_eq!(g32(1.999_999), "2");
        assert_eq!(g32(333.333_33), "333.333");
        assert_eq!(g32(0.083_333_336), "0.0833333");
        assert_eq!(g(1234567.0), "1.23457e+06");
        assert_eq!(g(100000.0), "100000");
        assert_eq!(f(f64::from(36.869_896f32)), "36.869896");
        assert_eq!(f(0.5), "0.500000");
        assert_eq!(g_width(1.0, 13, 6), "            1");
        assert_eq!(g_width(-0.0, 13, 6), "           -0");
        assert_eq!(g_prec(1.23456, 4), "1.235");
        assert_eq!(fixed_width(-0.0, 8, 4), " -0.0000");
        assert_eq!(fixed_signed(0.25, 3), "+0.250");
        assert_eq!(fixed_signed(-0.25, 3), "-0.250");
    }
}

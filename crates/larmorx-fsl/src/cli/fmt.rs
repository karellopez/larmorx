// SPDX-License-Identifier: Apache-2.0
//! Numbers as C's `printf` and C++'s default stream formatting print them in FSL's messages:
//! `%g` (6 significant digits, trailing zeros removed, exponent form below 1e-4 and from 1e6)
//! and `%f` (6 decimals). Rust's formatting rounds the exact binary value to nearest, ties to
//! even, as glibc does.

/// `%g`.
pub fn g(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    const P: i32 = 6;
    let sci = format!("{:.*e}", (P - 1) as usize, v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    if !(-4..P).contains(&x) {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), x.abs())
    } else {
        let decimals = (P - 1 - x).max(0) as usize;
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
        // values bet2 -v printed (observed)
        assert_eq!(g(837.7420043945312), "837.742");
        assert_eq!(g(-9.197999954223633), "-9.198");
        assert_eq!(g(84.55382592705053), "84.5538");
        assert_eq!(g(217510.0), "217510");
        assert_eq!(g(1e-5), "1e-05");
        assert_eq!(g(f64::NAN), "nan");
        assert_eq!(f(157.874776), "157.874776");
    }
}

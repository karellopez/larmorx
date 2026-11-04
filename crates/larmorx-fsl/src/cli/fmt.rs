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

/// A double as nlohmann/json writes it (§6.2 of `specs/fslstats.md`): the shortest decimal
/// that reads back to the same double; fixed notation from 1e-4 to below 1e15 (an integral
/// value with `.0`), else `d.ddde+XX` (at least two exponent digits); `null` for NaN and the
/// infinities.
pub fn json(v: f64) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    // the shortest round-trip digits and the decimal exponent: v = 0.d1d2... x 10^n
    let sci = format!("{:e}", v.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    let k = digits.len() as i32;
    let sign = if v < 0.0 { "-" } else { "" };
    let body = if k <= n && n <= 15 {
        format!("{digits}{}.0", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 15 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -4 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let es = if e < 0 { '-' } else { '+' };
        let m = if k == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!("{m}e{es}{:02}", e.abs())
    };
    format!("{sign}{body}")
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
        // nlohmann's forms (specs/fslstats.md §6.2; values fslstats -json printed, observed)
        assert_eq!(json(7.13116826237859), "7.13116826237859");
        assert_eq!(json(f64::from(-1.129_077_f32)), "-1.1290769577026367");
        assert_eq!(json(3150.0), "3150.0");
        assert_eq!(json(16842496.0), "16842496.0");
        assert_eq!(json(1e20), "1e+20");
        assert_eq!(json(1.5e-5), "1.5e-05");
        assert_eq!(json(1e-4), "0.0001");
        assert_eq!(json(1e15), "1e+15");
        assert_eq!(json(123456789012345.0), "123456789012345.0");
        assert_eq!(json(f64::NAN), "null");
    }
}

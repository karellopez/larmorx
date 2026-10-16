// SPDX-License-Identifier: Apache-2.0
//! Numbers as a C++ `std::ostream` prints them (glibc's `printf` conversions), with the
//! stream state ANTs leaves behind: after its first iteration line, `std::cout` stays in
//! `std::scientific` with precision 4, so every later floating-point value it prints is in
//! that format.

/// The formatting state of a `std::ostream` (`std::scientific` or the default, and
/// `precision`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OStream {
    pub scientific: bool,
    pub precision: usize,
}

impl Default for OStream {
    fn default() -> Self {
        OStream {
            scientific: false,
            precision: 6,
        }
    }
}

impl OStream {
    /// `os << v` for a `double` (a `float` is printed as its double value).
    pub fn float(&self, v: f64) -> String {
        if self.scientific {
            format_e(v, self.precision)
        } else {
            format_g(v, self.precision)
        }
    }

    /// `os << "[" << a[0] << ", " << ... << "]"`, as ITK prints its arrays and points.
    pub fn array(&self, values: &[f64]) -> String {
        let parts: Vec<String> = values.iter().map(|&v| self.float(v)).collect();
        format!("[{}]", parts.join(", "))
    }
}

/// `%.{precision}g`.
pub fn format_g(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    let p = precision.max(1);
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent");
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

/// `%.{precision}e`: one digit, the point, `precision` digits, `e`, the sign and at least
/// two exponent digits.
pub fn format_e(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    let sci = format!("{v:.precision$e}");
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let x: i32 = exp.parse().expect("exponent");
    let sign = if x < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", x.abs())
}

/// An ITK array with the default stream format.
pub fn itk_array<R: larmorx_core::ItkReal>(values: &[R]) -> String {
    let v: Vec<f64> = values.iter().map(|x| x.to_f64()).collect();
    OStream::default().array(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_formats() {
        assert_eq!(format_e(-9.602079549428e-01, 12), "-9.602079549428e-01");
        assert_eq!(format_e(f64::MAX, 12), "1.797693134862e+308");
        assert_eq!(format_e(0.020002, 4), "2.0002e-02");
        assert_eq!(format_g(1e-6, 6), "1e-06");
        assert_eq!(format_g(0.1, 6), "0.1");
        assert_eq!(format_g(32.0, 6), "32");
        let s = OStream {
            scientific: true,
            precision: 4,
        };
        assert_eq!(s.array(&[1.0, 0.0]), "[1.0000e+00, 0.0000e+00]");
    }
}

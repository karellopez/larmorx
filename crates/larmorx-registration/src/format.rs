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

/// `printf("%a", v)` as glibc prints it (`0x1.8p+1`, `-0x0p+0`, subnormals as `0x0.8p-1022`).
pub fn hex_float(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.into();
    }
    let bits = v.to_bits();
    let sign = if bits >> 63 == 1 { "-" } else { "" };
    let exp = ((bits >> 52) & 0x7ff) as i64;
    let mut mant = bits & ((1u64 << 52) - 1);
    if exp == 0 && mant == 0 {
        return format!("{sign}0x0p+0");
    }
    let (lead, e) = if exp == 0 {
        (0, -1022)
    } else {
        (1, exp - 1023)
    };
    let mut digits = 13;
    while digits > 0 && mant & 0xf == 0 {
        mant >>= 4;
        digits -= 1;
    }
    let frac = if digits == 0 {
        String::new()
    } else {
        format!(".{mant:0digits$x}")
    };
    let esign = if e < 0 { '-' } else { '+' };
    format!("{sign}0x{lead}{frac}p{esign}{}", e.abs())
}

/// `operator<<` of an `itk::Array<double>` or `<float>`: `[a, b, ...]`, each number by
/// `ConvertNumberToString` (the shortest decimal that reads back to it).
pub fn itk_array<R: larmorx_core::ItkReal>(values: &[R]) -> String {
    let parts: Vec<String> = values
        .iter()
        .map(|x| match R::PRECISION {
            larmorx_core::Precision::Double => larmorx_io::itk_transform::itk_number(x.to_f64()),
            larmorx_core::Precision::Float => {
                larmorx_io::itk_transform::itk_number_f32(x.to_f64() as f32)
            }
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// `operator<<` of an `itk::Matrix<double>` (`vnl_matrix_fixed::print`): each row's entries
/// separated by a blank (the default stream format), each row ended by a new line.
pub fn itk_matrix<const R: usize, const C: usize>(m: &[[f64; C]; R]) -> String {
    let os = OStream::default();
    let mut s = String::new();
    for row in m {
        let parts: Vec<String> = row.iter().map(|&v| os.float(v)).collect();
        s.push_str(&parts.join(" "));
        s.push('\n');
    }
    s
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

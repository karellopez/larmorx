// SPDX-License-Identifier: Apache-2.0
//! C's `printf` of one `double` through a caller's format (`printf(fmt, x)`), as the matrices'
//! printing functions use it (`specs/freesurfer-maths.md` §4, §8.6): the text of the format, `%%`,
//! and the conversions `f F e E g G` with the flags `-`, `+`, space, `#`, `0`, a width and a
//! precision (ISO C 7.21.6.1; the length modifiers `l`, `L`, `h` are read and ignored). The
//! digits are `larmorx_core::cfmt`'s (the exact value correctly rounded, ties to even).

use larmorx_core::cfmt;

/// One conversion's flags, width and precision.
#[derive(Clone, Copy, Default)]
struct Spec {
    left: bool,
    plus: bool,
    space: bool,
    alt: bool,
    zero: bool,
    width: usize,
    prec: Option<usize>,
}

/// `%e`'s body without its sign, for a finite non-negative `v` (`alt`: keep the point).
fn body_e(v: f64, prec: usize, alt: bool) -> String {
    let s = cfmt::e(v, prec);
    if alt && prec == 0 {
        let (m, x) = s.split_once('e').expect("an exponent");
        format!("{m}.e{x}")
    } else {
        s
    }
}

/// `%g`'s body for a finite non-negative `v`.
fn body_g(v: f64, prec: usize, alt: bool) -> String {
    if !alt {
        return cfmt::g(v, prec);
    }
    // `#`: the trailing zeros and the point are kept.
    let p = prec.max(1);
    if v == 0.0 {
        return format!("{:.*}", p - 1, 0.0);
    }
    let sci = format!("{:.*e}", p - 1, v);
    let x: i32 = sci
        .split_once('e')
        .expect("an exponent")
        .1
        .parse()
        .expect("an exponent");
    if x < -4 || x >= p as i32 {
        body_e(v, p - 1, true)
    } else {
        let decimals = (p as i32 - 1 - x).max(0) as usize;
        let s = format!("{v:.decimals$}");
        if s.contains('.') { s } else { format!("{s}.") }
    }
}

/// One converted value with its flags.
fn convert(spec: Spec, conv: u8, v: f64) -> String {
    let upper = conv.is_ascii_uppercase();
    let neg = v.is_sign_negative();
    let a = v.abs();
    let finite = v.is_finite();
    let mut body = if v.is_nan() {
        "nan".to_owned()
    } else if v.is_infinite() {
        "inf".to_owned()
    } else {
        let prec = spec.prec.unwrap_or(6);
        match conv.to_ascii_lowercase() {
            b'f' => {
                let s = cfmt::f(a, prec);
                if spec.alt && prec == 0 {
                    format!("{s}.")
                } else {
                    s
                }
            }
            b'e' => body_e(a, prec, spec.alt),
            _ => body_g(a, prec, spec.alt),
        }
    };
    if upper {
        body = body.to_ascii_uppercase();
    }
    let sign = if neg {
        "-"
    } else if spec.plus {
        "+"
    } else if spec.space {
        " "
    } else {
        ""
    };
    let len = sign.len() + body.len();
    if len >= spec.width {
        return format!("{sign}{body}");
    }
    let pad = spec.width - len;
    if spec.left {
        format!("{sign}{body}{}", " ".repeat(pad))
    } else if spec.zero && finite {
        format!("{sign}{}{body}", "0".repeat(pad))
    } else {
        format!("{}{sign}{body}", " ".repeat(pad))
    }
}

/// `printf(fmt, v)`'s bytes: every conversion of the format takes `v`; a conversion this module
/// does not know is written as it stands.
pub fn sprintf_f64(fmt: &[u8], v: f64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c != b'%' {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if i < fmt.len() && fmt[i] == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        let mut s = Spec::default();
        while i < fmt.len() {
            match fmt[i] {
                b'-' => s.left = true,
                b'+' => s.plus = true,
                b' ' => s.space = true,
                b'#' => s.alt = true,
                b'0' => s.zero = true,
                _ => break,
            }
            i += 1;
        }
        while i < fmt.len() && fmt[i].is_ascii_digit() {
            s.width = s.width * 10 + usize::from(fmt[i] - b'0');
            i += 1;
        }
        if i < fmt.len() && fmt[i] == b'.' {
            i += 1;
            let mut p = 0;
            while i < fmt.len() && fmt[i].is_ascii_digit() {
                p = p * 10 + usize::from(fmt[i] - b'0');
                i += 1;
            }
            s.prec = Some(p);
        }
        while i < fmt.len() && matches!(fmt[i], b'l' | b'L' | b'h') {
            i += 1;
        }
        if i < fmt.len() && matches!(fmt[i], b'f' | b'F' | b'e' | b'E' | b'g' | b'G') {
            out.extend_from_slice(convert(s, fmt[i], v).as_bytes());
            i += 1;
        } else {
            let end = (i + 1).min(fmt.len());
            out.extend_from_slice(&fmt[start..end]);
            i = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sprintf_f64;

    #[test]
    fn forms() {
        let t = |f: &str, v: f64| String::from_utf8(sprintf_f64(f.as_bytes(), v)).unwrap();
        assert_eq!(t("%8.5lf", f64::INFINITY), "     inf");
        assert_eq!(t("%10.3lf", -0.0), "    -0.000");
        assert_eq!(t("%g", 6.925580203527572e28), "6.92558e+28");
        assert_eq!(t("% 2.5f", 1.5), " 1.50000");
        assert_eq!(t("%+f", 2.0), "+2.000000");
        assert_eq!(t("%+4.5f", -0.25), "-0.25000");
        assert_eq!(t("%05.1f", -2.0), "-02.0");
        assert_eq!(t("%-6gx", 1.0), "1     x");
        assert_eq!(t("%#g", 1.0), "1.00000");
        assert_eq!(t("%E", f64::NAN), "NAN");
    }
}

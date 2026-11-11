// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's `str()` of a float array (`numpy/_core/arrayprint.py`: `array2string` with the
//! default print options, `FloatingFormat` in `'maxprec'` mode with precision 8, `_formatArray`
//! with line width 75; numpy 2.5.4, unchanged from 2.3.5 on this path), for the messages a
//! replica prints with numbers in them (tedana's adaptive-mask thresholds, nilearn's affines).
//!
//! The digits are Dragon4's shortest unique digits of the element's own type (`float32` or
//! `float64`), cut at 8 decimals when longer (rounded, ties to even); the elements share one
//! layout: positional, or scientific when a non-zero magnitude is below 0.0001, at or above
//! 1e8, or the largest is more than 1000 times the smallest; padded to a common width. Arrays
//! of more than 1000 elements (numpy's summary with `...`) are not ported.

/// A float numpy prints with its own type's shortest digits.
pub trait PrintFloat: Copy {
    fn to_f64(self) -> f64;
    /// `format!("{:e}")`: the shortest digits in scientific form.
    fn sci(self) -> String;
    /// `format!("{:.p$}")`: the exact value rounded to `p` decimals, ties to even.
    fn fixed(self, p: usize) -> String;
    /// `format!("{:.p$e}")`.
    fn sci_fixed(self, p: usize) -> String;
}

impl PrintFloat for f32 {
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn sci(self) -> String {
        shortest_ties_even(
            format!("{self:e}"),
            |p| format!("{self:.p$e}"),
            |s| s.parse::<f32>().is_ok_and(|v| v == self),
        )
    }
    fn fixed(self, p: usize) -> String {
        format!("{self:.p$}")
    }
    fn sci_fixed(self, p: usize) -> String {
        format!("{self:.p$e}")
    }
}

/// The shortest digits that read back as the value, and among them the closest to it, ties to
/// an even last digit (as numpy's Dragon4 and Python's `repr` choose them): Rust's shortest
/// form `shortest` gives the length; the value correctly rounded to that many digits (`fixed`,
/// exact, ties to even) is taken when it reads back (`reads_back`), which it does unless the
/// value is a power of two whose lower neighbour is closer.
pub fn shortest_ties_even(
    shortest: String,
    fixed: impl Fn(usize) -> String,
    reads_back: impl Fn(&str) -> bool,
) -> String {
    let mant = shortest
        .split_once('e')
        .map_or(shortest.as_str(), |(m, _)| m);
    let n = mant.chars().filter(char::is_ascii_digit).count();
    let r = fixed(n.saturating_sub(1));
    if r != shortest && reads_back(&r) {
        r
    } else {
        shortest
    }
}

impl PrintFloat for f64 {
    fn to_f64(self) -> f64 {
        self
    }
    fn sci(self) -> String {
        shortest_ties_even(
            format!("{self:e}"),
            |p| format!("{self:.p$e}"),
            |s| s.parse::<f64>().is_ok_and(|v| v == self),
        )
    }
    fn fixed(self, p: usize) -> String {
        format!("{self:.p$}")
    }
    fn sci_fixed(self, p: usize) -> String {
        format!("{self:.p$e}")
    }
}

const PRECISION: usize = 8;
const LINE_WIDTH: usize = 75;

/// `dragon4_positional(x, precision=8, unique=True, fractional=True, trim='.')`: the integer
/// and fraction parts (the sign in the integer part).
fn positional<T: PrintFloat>(x: T) -> (String, String) {
    let v = x.to_f64();
    let sign = if v.is_sign_negative() { "-" } else { "" };
    let s = x.sci();
    let (mant, exp) = s.split_once('e').expect("exponent form");
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let exp: i64 = exp.parse().expect("exponent");
    let decpt = exp + 1;
    let n = digits.len() as i64;
    let frac_len = (n - decpt).max(0);
    let (int_part, frac_part) = if v == 0.0 {
        ("0".to_owned(), String::new())
    } else if frac_len as usize > PRECISION {
        // Cut at 8 decimals: the exact value rounded, then trailing zeros trimmed.
        let f = x.fixed(PRECISION);
        let f = f.trim_start_matches('-');
        let (a, b) = f.split_once('.').expect("decimals");
        (a.to_owned(), b.trim_end_matches('0').to_owned())
    } else if decpt <= 0 {
        (
            "0".to_owned(),
            format!("{}{digits}", "0".repeat((-decpt) as usize)),
        )
    } else if decpt >= n {
        (
            format!("{digits}{}", "0".repeat((decpt - n) as usize)),
            String::new(),
        )
    } else {
        let (a, b) = digits.split_at(decpt as usize);
        (a.to_owned(), b.to_owned())
    };
    (format!("{sign}{int_part}"), frac_part)
}

/// `dragon4_scientific(x, precision=8, unique=True, trim='.')`: the integer digit (with the
/// sign), the fraction digits and the exponent (signed, at least two digits).
fn scientific<T: PrintFloat>(x: T) -> (String, String, String) {
    let v = x.to_f64();
    let mut s = x.sci();
    let digits = s
        .split_once('e')
        .expect("e")
        .0
        .chars()
        .filter(char::is_ascii_digit)
        .count();
    if digits > PRECISION + 1 {
        s = x.sci_fixed(PRECISION);
    }
    sci_parts(v, &s, true)
}

/// A scientific form `d.ddde±x` (Rust's) as numpy's integer part (with the sign of `v`),
/// fraction digits (trailing zeros cut when `trim`) and exponent (`±XX`).
fn sci_parts(v: f64, s: &str, trim: bool) -> (String, String, String) {
    let sign = if v.is_sign_negative() { "-" } else { "" };
    let (mant, exp) = s.split_once('e').expect("e");
    let mant = mant.trim_start_matches('-');
    let (a, b) = mant.split_once('.').unwrap_or((mant, ""));
    let b = if trim { b.trim_end_matches('0') } else { b };
    let exp: i64 = exp.parse().expect("exponent");
    let e = format!("{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    (format!("{sign}{a}"), b.to_owned(), e)
}

/// `FloatingFormat` for one array: formats each element to the common layout.
struct Format {
    exp: bool,
    pad_left: usize,
    pad_right: usize,
    precision: usize,
}

impl Format {
    fn new<T: PrintFloat>(a: &[T]) -> Format {
        let finite: Vec<T> = a
            .iter()
            .copied()
            .filter(|x| x.to_f64().is_finite())
            .collect();
        let nz: Vec<f64> = finite
            .iter()
            .map(|x| x.to_f64().abs())
            .filter(|&x| x != 0.0)
            .collect();
        let mut exp = false;
        if !nz.is_empty() {
            let max = nz.iter().copied().fold(f64::MIN, f64::max);
            let min = nz.iter().copied().fold(f64::MAX, f64::min);
            exp = max >= 1e8 || min < 0.0001 || max / min > 1000.0;
        }
        let (mut pad_left, mut pad_right, mut precision) = (0, 0, PRECISION);
        if finite.is_empty() {
        } else if exp {
            let parts: Vec<_> = finite.iter().map(|&x| scientific(x)).collect();
            let exp_size = parts.iter().map(|p| p.2.len()).max().unwrap_or(0) - 1;
            precision = parts.iter().map(|p| p.1.len()).max().unwrap_or(0);
            pad_left = parts.iter().map(|p| p.0.len()).max().unwrap_or(0);
            pad_right = exp_size + 2 + precision;
        } else {
            let parts: Vec<_> = finite.iter().map(|&x| positional(x)).collect();
            pad_left = parts.iter().map(|p| p.0.len()).max().unwrap_or(0);
            pad_right = parts.iter().map(|p| p.1.len()).max().unwrap_or(0);
        }
        if finite.len() != a.len() {
            let neginf = a
                .iter()
                .any(|x| x.to_f64().is_infinite() && x.to_f64() < 0.0);
            let offset = pad_right + 1;
            pad_left = pad_left
                .max(3usize.saturating_sub(offset))
                .max((3 + usize::from(neginf)).saturating_sub(offset));
        }
        Format {
            exp,
            pad_left,
            pad_right,
            precision,
        }
    }

    fn format<T: PrintFloat>(&self, x: T) -> String {
        let v = x.to_f64();
        if !v.is_finite() {
            let r = if v.is_nan() {
                "nan"
            } else if v > 0.0 {
                "inf"
            } else {
                "-inf"
            };
            let w = self.pad_left + self.pad_right + 1;
            return format!("{r:>w$}");
        }
        if self.exp {
            let (mut a, mut b, mut e) = scientific(x);
            if b.len() < self.precision {
                // `min_digits` (numpy's `FloatingFormat` sets it to the array's precision in
                // scientific form): Dragon4 goes on with the element's own digits, the last
                // rounded ties to even, instead of padding with zeros (numpy 2.5.4, observed:
                // `[7.6950021e-02 7.0551876e-04]`, where 7.695002e-02 is the shortest)
                (a, b, e) = sci_parts(x.to_f64(), &x.sci_fixed(self.precision), false);
            }
            let b = format!("{b:0<w$}", w = self.precision);
            format!("{a:>l$}.{b}e{e}", l = self.pad_left)
        } else {
            let (a, b) = positional(x);
            format!("{a:>l$}.{b:<r$}", l = self.pad_left, r = self.pad_right)
        }
    }
}

/// `str(x)` of a numpy `float32` or `float64` scalar: its own type's shortest digits,
/// positional with at least one decimal when `1e-4 <= |x| < 1e16` (or zero), else `d.ddde±XX`.
pub fn scalar_str<T: PrintFloat>(x: T) -> String {
    let v = x.to_f64();
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let sign = if v.is_sign_negative() { "-" } else { "" };
    if v == 0.0 {
        return format!("{sign}0.0");
    }
    let s = x.sci();
    let (mant, exp) = s.split_once('e').expect("exponent form");
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let exp: i64 = exp.parse().expect("exponent");
    let a = v.abs();
    if !(1e-4..1e16).contains(&a) {
        let m = if digits.len() > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        return format!(
            "{sign}{m}e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        );
    }
    let decpt = exp + 1;
    let n = digits.len() as i64;
    if decpt <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt >= n {
        format!("{sign}{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let (i, f) = digits.split_at(decpt as usize);
        format!("{sign}{i}.{f}")
    }
}

/// `_extendLine`.
fn extend_line(s: &mut String, line: &mut String, word: &str, width: usize, prefix: &str) {
    let mut wrap = line.len() + word.len() > width;
    if line.len() <= prefix.len() {
        wrap = false;
    }
    if wrap {
        s.push_str(line.trim_end());
        s.push('\n');
        *line = prefix.to_owned();
    }
    line.push_str(word);
}

fn row<T: PrintFloat>(f: &Format, a: &[T], hanging: &str, width: usize) -> String {
    let elem_width = width - 1;
    let mut s = String::new();
    let mut line = hanging.to_owned();
    for (k, &x) in a.iter().enumerate() {
        extend_line(&mut s, &mut line, &f.format(x), elem_width, hanging);
        if k + 1 < a.len() {
            line.push(' ');
        }
    }
    s.push_str(&line);
    format!("[{}]", &s[hanging.len()..])
}

/// `str(a)` of a one-dimensional `float32` or `float64` array.
pub fn array_str<T: PrintFloat>(a: &[T]) -> String {
    assert!(a.len() <= 1000, "numpy's summarised form is not ported");
    let f = Format::new(a);
    row(&f, a, " ", LINE_WIDTH)
}

/// `str(a)` of a C-ordered two-dimensional array of `rows` rows.
pub fn array_str_2d<T: PrintFloat>(a: &[T], rows: usize) -> String {
    assert!(a.len() <= 1000, "numpy's summarised form is not ported");
    let f = Format::new(a);
    let cols = a.len().checked_div(rows).unwrap_or(0);
    let lines: Vec<String> = (0..rows)
        .map(|r| row(&f, &a[r * cols..(r + 1) * cols], "  ", LINE_WIDTH - 1))
        .collect();
    format!("[{}]", lines.join("\n "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_as_numpy() {
        let a = [180.95544f32, 65.59678, 18.5366];
        assert_eq!(array_str(&a), "[180.95544  65.59678  18.5366 ]");
        assert_eq!(array_str(&[1.0f64, 2.5]), "[1.  2.5]");
        assert_eq!(array_str(&[1e-5f64, 2.5e-5]), "[1.0e-05 2.5e-05]");
        // scientific form: a shorter element shows its own further digits (numpy's
        // `min_digits`), not zeros (numpy 2.5.4's output)
        let t = [7.6950021e-02f32, 7.0551876e-04, 1.2756876e-05];
        assert_eq!(array_str(&t), "[7.6950021e-02 7.0551876e-04 1.2756876e-05]");
        let t = [3.7185407e-01f32, 1.7238496e-03, 6.0828668e-05, 8.7365316e-07, 3.0783408e-08];
        assert_eq!(
            array_str(&t),
            "[3.7185407e-01 1.7238496e-03 6.0828668e-05 8.7365316e-07 3.0783408e-08]"
        );
        assert_eq!(array_str(&[0.5f32, 1.25e-05]), "[5.00e-01 1.25e-05]");
        assert_eq!(array_str(&[0.0f64, -0.0]), "[ 0. -0.]");
        assert_eq!(array_str(&[f64::NAN, 1.0]), "[nan  1.]");
        assert_eq!(array_str(&[0.1f32]), "[0.1]");
        assert_eq!(array_str(&[1.0f64 / 3.0]), "[0.33333333]");
        assert_eq!(scalar_str(221.86421f32), "221.86421");
        assert_eq!(scalar_str(0.0001f32), "1e-04");
        assert_eq!(scalar_str(0.0001f64), "0.0001");
        assert_eq!(scalar_str(3.0f32), "3.0");
        assert_eq!(scalar_str(1e20f32), "1e+20");
        assert_eq!(scalar_str(9999999999999998.0f64), "9999999999999998.0");
        assert_eq!(
            array_str_2d(&[1.0f64, 0.0, 0.0, 1.0], 2),
            "[[1. 0.]\n [0. 1.]]"
        );
    }
}

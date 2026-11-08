// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's element rules that sdcflows' arithmetic goes through, in numpy's operation order
//! (numpy 2.5.4, unchanged from 2.2.6 on these routines; BSD-3-Clause, notice below).
//!
//! - [`floor_divide`]: `a // b` for float64 (`npy_floor_divide`, `npy_divmod` in
//!   `numpy/_core/src/npymath/npy_math_internal.h.src`): the quotient of `a - fmod(a, b)`,
//!   moved to Python's remainder convention and snapped to the nearest integer, not
//!   `floor(a / b)`.
//! - [`allclose_zero`]: `numpy.allclose(x, 0, atol=atol)` (`isclose` with `rtol = 1e-5`).
//! - [`cross`]: `numpy.cross` of two 3-vectors (`numpy/_core/numeric.py`: products into the
//!   output, then the second product subtracted).
//! - [`norm3`]: `numpy.linalg.norm(v)` of a 3-vector along an axis, `sqrt(add.reduce(v * v))`,
//!   the reduction left to right.
//! - [`py_repr`], [`py_format_2e`]: Python's `repr()` and `f"{x:.2e}"` of a float (the messages
//!   sdcflows, SciPy and scikit-learn write with numbers in them).
//!
//! numpy: Copyright (c) 2005-2025, NumPy Developers. All rights reserved. Redistribution and
//! use in source and binary forms, with or without modification, are permitted provided that
//! the following conditions are met: redistributions of source code must retain the above
//! copyright notice, this list of conditions and the following disclaimer; redistributions in
//! binary form must reproduce the above copyright notice, this list of conditions and the
//! following disclaimer in the documentation and/or other materials provided with the
//! distribution; neither the name of the NumPy Developers nor the names of any contributors
//! may be used to endorse or promote products derived from this software without specific
//! prior written permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
//! CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
//! THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//! DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//! INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//! LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
//! BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//! STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
//! USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

/// `a // b` for float64, as numpy computes it (`npy_floor_divide`).
pub fn floor_divide(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return a / b;
    }
    // npy_divmod
    let mut m = a % b; // C fmod: exact, the sign of a
    let mut div = (a - m) / b;
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) {
            m += b;
            div -= 1.0;
        }
    } else {
        m = 0.0f64.copysign(b);
    }
    let _ = m;
    if div != 0.0 {
        let mut floordiv = div.floor();
        if div - floordiv > 0.5 {
            floordiv += 1.0;
        }
        floordiv
    } else {
        0.0f64.copysign(a / b)
    }
}

/// `numpy.allclose(values, 0, atol=atol)`: every `|v - 0| <= atol + 1e-5 * |0|` (a NaN is never
/// close; an infinity is not close to 0).
pub fn allclose_zero(values: &[f64], atol: f64) -> bool {
    values
        .iter()
        .all(|&v| (v - 0.0).abs() <= atol + 1e-5 * 0.0f64.abs())
}

/// `numpy.cross(a, b)` of two 3-vectors.
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let mut cp = [a[1] * b[2], a[2] * b[0], a[0] * b[1]];
    cp[0] -= a[2] * b[1];
    cp[1] -= a[0] * b[2];
    cp[2] -= a[1] * b[0];
    cp
}

/// `numpy.linalg.norm(v)` of a 3-vector: `sqrt((v0² + v1²) + v2²)`.
pub fn norm3(v: [f64; 3]) -> f64 {
    let s = [v[0] * v[0], v[1] * v[1], v[2] * v[2]];
    ((s[0] + s[1]) + s[2]).sqrt()
}

/// sdcflows' test that two grids are aligned: `np.allclose(np.linalg.norm(np.cross(a[:-1, :-1].T,
/// b[:-1, :-1].T), axis=1), 0, atol=1e-3)` (column `i` of one 3×3 matrix against column `i` of
/// the other).
pub fn grids_aligned(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> bool {
    let norms: Vec<f64> = (0..3)
        .map(|i| {
            let ca = [a[0][i], a[1][i], a[2][i]];
            let cb = [b[0][i], b[1][i], b[2][i]];
            norm3(cross(ca, cb))
        })
        .collect();
    allclose_zero(&norms, 1e-3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_divide_follows_numpy() {
        assert_eq!(floor_divide(236.0, 16.0), 14.0);
        assert_eq!(floor_divide(-7.0, 2.0), -4.0);
        assert_eq!(floor_divide(7.0, -2.0), -4.0);
        assert_eq!(floor_divide(0.0, 3.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(floor_divide(-0.0, 3.0).to_bits(), (-0.0f64).to_bits());
        // numpy's quotient of the exact remainder: 1 // 0.1 is 9, not floor(1 / 0.1) = 10.
        assert_eq!(floor_divide(1.0, 0.1), 9.0);
        assert!(floor_divide(1.0, 0.0).is_infinite());
        assert!(floor_divide(f64::NAN, 2.0).is_nan());
    }

    #[test]
    fn aligned_test() {
        let eye = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut scaled = eye;
        scaled[0][0] = 16.0;
        scaled[2][2] = -10.0;
        assert!(grids_aligned(&eye, &scaled));
        let (s, c) = (0.1f64.sin(), 0.1f64.cos());
        let mut rot = eye;
        rot[0][0] = c;
        rot[0][1] = -s;
        rot[1][0] = s;
        rot[1][1] = c;
        assert!(!grids_aligned(&eye, &rot));
    }
}

/// Python's `repr()` of a float (`float_repr_style` "short": the shortest decimal that reads
/// back as the same double, the one nearest the exact value, ties to even; scientific
/// notation when the decimal point would sit 4 or more places left of the first digit or more
/// than 16 places right of it; positional numbers keep a ".0").
pub fn py_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // Rust's shortest round trip gives the digit count; the correctly rounded string of that
    // many digits is the nearest one (Rust may pick another of the same length on a tie).
    let short = format!("{x:e}");
    let n = short
        .split_once('e')
        .map_or(0, |(m, _)| m.chars().filter(char::is_ascii_digit).count());
    let exact = format!("{:.*e}", n.saturating_sub(1), x);
    let chosen = if exact.parse::<f64>() == Ok(x) {
        exact
    } else {
        short
    };
    let (mant, exp) = chosen.split_once('e').unwrap_or((&chosen, "0"));
    let negative = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let e: i32 = exp.parse().unwrap_or(0);
    let decpt = e + 1;
    let body = if decpt <= -4 || decpt > 16 {
        let (first, rest) = digits.split_at(1);
        let mut m = first.to_string();
        if !rest.is_empty() {
            m.push('.');
            m.push_str(rest);
        }
        format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else if decpt <= 0 {
        format!("0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(decpt as usize - digits.len()))
    } else {
        let (a, b) = digits.split_at(decpt as usize);
        format!("{a}.{b}")
    };
    if negative { format!("-{body}") } else { body }
}

/// Python's `f"{x:.2e}"` of a float.
pub fn py_format_2e(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let s = format!("{x:.2e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let (sign, digits) = e.strip_prefix('-').map_or(("+", e), |d| ("-", d));
    format!("{m}e{sign}{digits:0>2}")
}

#[cfg(test)]
mod repr_tests {
    use super::*;

    #[test]
    fn python_repr_and_format() {
        // the float32 791.06524658203125: a tie at 16 digits, Python keeps the even digit
        assert_eq!(py_repr(f64::from(791.065_25_f32)), "791.0652465820312");
        assert_eq!(py_repr(f64::from(1_319.524_5_f32)), "1319.5245361328125");
        assert_eq!(py_repr(85_199_288.0), "85199288.0");
        assert_eq!(py_repr(1e-5), "1e-05");
        assert_eq!(py_repr(0.0001), "0.0001");
        assert_eq!(py_repr(1e16), "1e+16");
        assert_eq!(py_repr(1e15), "1000000000000000.0");
        assert_eq!(py_repr(-1.0), "-1.0");
        assert_eq!(py_repr(-0.5), "-0.5");
        assert_eq!(py_repr(0.1), "0.1");
        assert_eq!(py_repr(1.5e-7), "1.5e-07");
        assert_eq!(py_repr(-0.0), "-0.0");
        assert_eq!(py_repr(f64::NAN), "nan");
        assert_eq!(py_format_2e(12345.0), "1.23e+04");
        assert_eq!(py_format_2e(0.000123), "1.23e-04");
        assert_eq!(py_format_2e(f64::NAN), "nan");
    }
}

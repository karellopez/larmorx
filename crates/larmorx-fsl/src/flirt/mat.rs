// SPDX-License-Identifier: Apache-2.0
//! 4 × 4 matrices as FLIRT computes, reads, writes and prints them (`specs/flirt.md` §2.5, §12).
//!
//! The products are the shared engine's (`larmorx_mri::hmc::rigid::mul`, row by column, left to
//! right); inverses and solves are LU decompositions with partial pivoting (§12), so that a
//! singular matrix is reported as FLIRT reports it (`inv(): matrix is singular`).

pub use larmorx_mri::hmc::rigid::{IDENTITY, Mat4, mul};

/// FLIRT's message for a singular inverse (§2.2).
pub const SINGULAR: &str = "inv(): matrix is singular";

/// The LU decomposition of `a` with partial pivoting (rows exchanged for the largest magnitude
/// in each column, the first on ties): the packed factors and the row order; `None` when a
/// pivot is exactly zero or not finite.
fn lu(a: &Mat4) -> Option<(Mat4, [usize; 4])> {
    let mut m = *a;
    let mut perm = [0, 1, 2, 3];
    for k in 0..4 {
        let mut p = k;
        let mut best = m[k][k].abs();
        for (i, row) in m.iter().enumerate().skip(k + 1) {
            if row[k].abs() > best {
                best = row[k].abs();
                p = i;
            }
        }
        if best == 0.0 || !best.is_finite() {
            return None;
        }
        if p != k {
            m.swap(p, k);
            perm.swap(p, k);
        }
        let pivot = m[k][k];
        for i in k + 1..4 {
            let f = m[i][k] / pivot;
            m[i][k] = f;
            for j in k + 1..4 {
                m[i][j] -= f * m[k][j];
            }
        }
    }
    Some((m, perm))
}

/// The solution `x` of `a·x = b` by LU with partial pivoting; `None` for a singular `a`.
pub fn solve_vec(a: &Mat4, b: [f64; 4]) -> Option<[f64; 4]> {
    let (lu, perm) = lu(a)?;
    Some(substitute(&lu, &perm, b))
}

fn substitute(lu: &Mat4, perm: &[usize; 4], b: [f64; 4]) -> [f64; 4] {
    let mut y = [0.0; 4];
    for i in 0..4 {
        let mut s = b[perm[i]];
        for j in 0..i {
            s -= lu[i][j] * y[j];
        }
        y[i] = s;
    }
    let mut x = [0.0; 4];
    for i in (0..4).rev() {
        let mut s = y[i];
        for j in i + 1..4 {
            s -= lu[i][j] * x[j];
        }
        x[i] = s / lu[i][i];
    }
    x
}

/// `inv(a)` (§12): a diagonal matrix element by element, any other by LU with partial
/// pivoting; `None` when `a` is singular.
pub fn inverse(a: &Mat4) -> Option<Mat4> {
    let diagonal = (0..4).all(|i| (0..4).all(|j| i == j || a[i][j] == 0.0));
    if diagonal {
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            if a[i][i] == 0.0 || !a[i][i].is_finite() {
                return None;
            }
            out[i][i] = 1.0 / a[i][i];
        }
        return Some(out);
    }
    let (lu, perm) = lu(a)?;
    let mut out = [[0.0; 4]; 4];
    for j in 0..4 {
        let mut e = [0.0; 4];
        e[j] = 1.0;
        let col = substitute(&lu, &perm, e);
        for i in 0..4 {
            out[i][j] = col[i];
        }
    }
    Some(out)
}

/// `inv(a)·b` as a solve (§12); `None` for a singular `a`.
pub fn solve(a: &Mat4, b: &Mat4) -> Option<Mat4> {
    let (lu, perm) = lu(a)?;
    let mut out = [[0.0; 4]; 4];
    for j in 0..4 {
        let col = substitute(&lu, &perm, [b[0][j], b[1][j], b[2][j], b[3][j]]);
        for i in 0..4 {
            out[i][j] = col[i];
        }
    }
    Some(out)
}

/// `a·v` (a 4 × 4 times 4-vector product, left to right).
pub fn apply(a: &Mat4, v: [f64; 4]) -> [f64; 4] {
    [0, 1, 2, 3].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2] + a[i][3] * v[3])
}

/// `diag(d, 1)`.
pub fn diagonal(d: [f64; 3]) -> Mat4 {
    larmorx_mri::hmc::rigid::diagonal(d)
}

/// `aᵀ`.
pub fn transpose(a: &Mat4) -> Mat4 {
    let mut t = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            t[i][j] = a[j][i];
        }
    }
    t
}

/// C's `atof`: the longest leading prefix of the word that reads as a number (after leading
/// blanks), 0 when there is none.
pub fn atof(word: &str) -> f64 {
    let s = word.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let b = s.as_bytes();
    // the longest prefix that parses: try the special words, then digits
    let lower = s.to_ascii_lowercase();
    for (name, v) in [
        ("+infinity", f64::INFINITY),
        ("-infinity", f64::NEG_INFINITY),
        ("infinity", f64::INFINITY),
        ("+inf", f64::INFINITY),
        ("-inf", f64::NEG_INFINITY),
        ("inf", f64::INFINITY),
        ("+nan", f64::NAN),
        ("-nan", -f64::NAN),
        ("nan", f64::NAN),
    ] {
        if lower.starts_with(name) {
            return v;
        }
    }
    let mut end = 0;
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits = true;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits = true;
        }
    }
    if digits {
        end = i;
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            let start = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > start {
                end = j;
            }
        }
    }
    if end == 0 {
        return 0.0;
    }
    s[..end].parse::<f64>().unwrap_or(0.0)
}

/// C's `atoi`: the leading integer of the word (after blanks), 0 when there is none; values
/// beyond 32 bits wrap as glibc's conversion of a long does.
pub fn atoi(word: &str) -> i32 {
    let s = word.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let b = s.as_bytes();
    let mut i = 0;
    let neg = if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
        b[0] == b'-'
    } else {
        false
    };
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    let v = if neg { v.saturating_neg() } else { v };
    v as i32
}

/// A number as a C++ stream reads a double from text: the longest prefix of the token that is
/// a number; `None` when the token does not start with one.
fn stream_number(token: &str) -> Option<f64> {
    let lower = token.to_ascii_lowercase();
    let body = lower.trim_start_matches(['+', '-']);
    if body.starts_with("inf") || body.starts_with("nan") {
        return Some(atof(token));
    }
    let b = token.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let start = i;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        i += 1;
    }
    if i == start || !token[start..i].bytes().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(atof(token))
}

/// The rows of numbers of a matrix file as FLIRT reads them (§2.2): the lines that start with
/// a number; the first such line's count of numbers is the number of columns, and a later line
/// with fewer numbers is an error (`Could not find enough numbers in matrix file`).
pub fn parse_matrix_text(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut cols = 0;
    for line in text.lines() {
        let nums: Vec<f64> = line
            .split_whitespace()
            .map_while(stream_number)
            .collect();
        if nums.is_empty() {
            continue;
        }
        if rows.is_empty() {
            cols = nums.len();
        } else if nums.len() < cols {
            return Err("Could not find enough numbers in matrix file".to_owned());
        }
        rows.push(nums[..cols].to_vec());
    }
    Ok(rows)
}

/// A 4 × 4 matrix from the rows of a matrix file; `None` unless there are 4 rows of 4.
pub fn mat4_of(rows: &[Vec<f64>]) -> Option<Mat4> {
    if rows.len() < 4 || rows.iter().take(4).any(|r| r.len() < 4) {
        return None;
    }
    let mut m = [[0.0; 4]; 4];
    for i in 0..4 {
        m[i].copy_from_slice(&rows[i][..4]);
    }
    Some(m)
}

/// C's `%.{p}g`.
pub fn g(v: f64, p: usize) -> String {
    larmorx_mri::hmc::report::general(v, p)
}

/// C's `%.6f` (with `-nan` for a NaN whose sign is set).
pub fn f6(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_owned();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    format!("{v:.6}")
}

/// A matrix file's text (§2.5): every element in `%.10g` followed by two blanks, a newline
/// after each row.
pub fn file_text(m: &Mat4) -> String {
    let mut s = String::new();
    for row in m {
        for &v in row {
            s.push_str(&g(v, 10));
            s.push_str("  ");
        }
        s.push('\n');
    }
    s
}

/// A matrix printed on standard output (§2.5): fixed notation with 6 decimals, one blank after
/// each, a newline per row.
pub fn print_text(m: &Mat4) -> String {
    let mut s = String::new();
    for row in m {
        for &v in row {
            s.push_str(&f6(v));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// The stream state of FLIRT's standard output (§2.5): numbers print as `%g` until the first
/// matrix is printed, then as `%.6f` for good.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stream {
    pub fixed: bool,
}

impl Stream {
    /// A number as the stream prints it now.
    pub fn num(&self, v: f64) -> String {
        if self.fixed { f6(v) } else { g(v, 6) }
    }

    /// A matrix: printed in fixed notation, which stays on.
    pub fn matrix(&mut self, m: &Mat4) -> String {
        self.fixed = true;
        print_text(m)
    }
}

/// The RMS deviation of two matrices over a sphere of radius `r` about the origin (§8.6):
/// `√(tᵀt + (r²/5)·tr(AᵀA))` with `D = M1·M2⁻¹ − I`.
pub fn rms_deviation(m1: &Mat4, m2: &Mat4, r: f64) -> f64 {
    let Some(inv) = inverse(m2) else {
        return f64::INFINITY;
    };
    let mut d = mul(m1, &inv);
    for (i, row) in d.iter_mut().enumerate() {
        row[i] -= 1.0;
    }
    let t = [d[0][3], d[1][3], d[2][3]];
    let mut tr = 0.0;
    for j in 0..3 {
        for i in 0..3 {
            tr += d[i][j] * d[i][j];
        }
    }
    (t[0] * t[0] + t[1] * t[1] + t[2] * t[2] + (r * r / 5.0) * tr).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverses_and_solves() {
        let a: Mat4 = [
            [1.1, 0.02, -0.03, 2.0],
            [-0.01, 0.97, 0.04, -3.0],
            [0.02, -0.05, 1.1, 1.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let i = inverse(&a).unwrap();
        let p = mul(&a, &i);
        for r in 0..4 {
            for c in 0..4 {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((p[r][c] - want).abs() < 1e-14);
            }
        }
        let s = solve(&a, &IDENTITY).unwrap();
        assert_eq!(s, i);
        let mut sing = IDENTITY;
        sing[1][1] = 0.0;
        assert!(inverse(&sing).is_none());
        let mut sing = a;
        sing[2] = sing[1];
        assert!(inverse(&sing).is_none());
    }

    #[test]
    fn c_conversions() {
        assert_eq!(atof("3.5abc"), 3.5);
        assert_eq!(atof("abc"), 0.0);
        assert_eq!(atof("-1e2"), -100.0);
        assert_eq!(atof("1e"), 1.0);
        assert!(atof("nan").is_nan());
        assert_eq!(atoi("12.7"), 12);
        assert_eq!(atoi("-3x"), -3);
        assert_eq!(atoi("x"), 0);
    }

    #[test]
    fn matrix_text() {
        let rows = parse_matrix_text("1 0 0 0\n0 1 0 0\n\n0 0 1 0\n0 0 0 1\n").unwrap();
        assert_eq!(mat4_of(&rows), Some(IDENTITY));
        assert!(parse_matrix_text("1 2 3 4\n5 6\n").is_err());
        assert_eq!(file_text(&IDENTITY).lines().next(), Some("1  0  0  0  "));
        assert_eq!(g(0.9998031425123, 10), "0.9998031425");
        assert_eq!(
            print_text(&IDENTITY).lines().next(),
            Some("1.000000 0.000000 0.000000 0.000000 ")
        );
    }
}

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
    lu_pivots(a).map(|(m, perm, _)| (m, perm))
}

/// [`lu`] with the row interchange of each step (LAPACK's `ipiv`).
fn lu_pivots(a: &Mat4) -> Option<(Mat4, [usize; 4], [usize; 4])> {
    let mut m = *a;
    let mut perm = [0, 1, 2, 3];
    let mut ipiv = [0, 1, 2, 3];
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
        ipiv[k] = p;
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
    Some((m, perm, ipiv))
}

/// The solution `x` of `a·x = b` by LU with partial pivoting; `None` for a singular `a`.
pub fn solve_vec(a: &Mat4, b: [f64; 4]) -> Option<[f64; 4]> {
    let (lu, ipiv) = lu_left_looking(a)?;
    let mut perm = [0, 1, 2, 3];
    for (k, &p) in ipiv.iter().enumerate() {
        perm.swap(k, p);
    }
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

/// `inv(a)` (§12, "the 4 × 4 inverse exactly"): a diagonal matrix by reciprocals, an upper
/// triangle by the triangle's inverse ([`upper_inverse`]), any other by the left-looking LU
/// factorisation with partial pivoting, the inverse of `U`, the solve against `L` and the
/// column interchanges; `None` when `a` is singular. (Observed: so the signs of FSL's zeros,
/// e.g. convert_xfm -inverse's `-0`.)
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
    let upper = (0..4).all(|i| (0..i).all(|j| a[i][j] == 0.0));
    if upper {
        if (0..4).any(|i| a[i][i] == 0.0 || !a[i][i].is_finite()) {
            return None;
        }
        let mut m = *a;
        upper_inverse(&mut m);
        return Some(m);
    }
    let (mut m, ipiv) = lu_left_looking(a)?;
    upper_inverse(&mut m);
    // from the last column to the first: the entries below the diagonal set aside and zeroed,
    // the column replaced by y - A·x with the columns to its right (four rows: the columns in
    // pairs, y - (a_{k+1}·x_{k+1} + a_k·x_k); a last single column y + a_k·(x_k·(-1)))
    for j in (0..4).rev() {
        let mut work = [0.0f64; 4];
        for i in j + 1..4 {
            work[i] = m[i][j];
            m[i][j] = 0.0;
        }
        let mut k = j + 1;
        while k + 1 < 4 {
            for row in m.iter_mut() {
                let p = row[k + 1] * work[k + 1] + row[k] * work[k];
                row[j] -= p;
            }
            k += 2;
        }
        if k < 4 {
            for row in m.iter_mut() {
                row[j] += row[k] * -work[k];
            }
        }
    }
    for j in (0..3).rev() {
        let jp = ipiv[j];
        if jp != j {
            for row in m.iter_mut() {
                row.swap(j, jp);
            }
        }
    }
    Some(m)
}

/// The inverse of the upper triangle of `m` in place (§12): column by column, `d = 1/u_jj`, the
/// column above the diagonal multiplied by the part already inverted (for each earlier column
/// `i` in order, `x_k = x_i·u_ki + x_k` for `k < i`, then `x_i = x_i·u_ii`), then each entry
/// multiplied by `-d` (so a zero becomes `-0`).
fn upper_inverse(m: &mut Mat4) {
    for j in 0..4 {
        m[j][j] = 1.0 / m[j][j];
        let ajj = -m[j][j];
        let mut x = [0.0f64; 4];
        for (i, xi) in x.iter_mut().enumerate().take(j) {
            *xi = m[i][j];
        }
        for i in 0..j {
            let xi = x[i];
            for k in 0..i {
                x[k] += xi * m[k][i];
            }
            x[i] = xi * m[i][i];
        }
        for (i, &xi) in x.iter().enumerate().take(j) {
            m[i][j] = xi * ajj;
        }
    }
}

/// The LU factorisation with partial pivoting, column by column (left-looking, §12): for
/// column `j` the earlier row interchanges; the entries above the diagonal `a_ij - Σ_{k<i} l_ik
/// a_kj` (the sum from 0 in order, then `+ 0`); the entries on and below it `y - A·x` (`t` summed from 0 in
/// order, then `(-1)·t + y`); the pivot the first entry of largest magnitude, the rows
/// interchanged (the columns so far), the entries below it multiplied by the pivot's
/// reciprocal. `None` when a pivot is exactly zero or not finite.
fn lu_left_looking(a: &Mat4) -> Option<(Mat4, [usize; 4])> {
    let mut m = *a;
    let mut ipiv = [0, 1, 2, 3];
    for j in 0..4 {
        for (k, &p) in ipiv.iter().enumerate().take(j) {
            if p != k {
                let t = m[k][j];
                m[k][j] = m[p][j];
                m[p][j] = t;
            }
        }
        for i in 1..j {
            let mut t = 0.0f64;
            for k in 0..i {
                t += m[i][k] * m[k][j];
            }
            m[i][j] = (m[i][j] - t) + 0.0;
        }
        for r in j..4 {
            let mut t = 0.0f64;
            for k in 0..j {
                t += m[r][k] * m[k][j];
            }
            m[r][j] += -t;
        }
        let mut p = j;
        let mut best = m[j][j].abs();
        for (i, row) in m.iter().enumerate().skip(j + 1) {
            if row[j].abs() > best {
                best = row[j].abs();
                p = i;
            }
        }
        if best == 0.0 || !best.is_finite() {
            return None;
        }
        ipiv[j] = p;
        if p != j {
            for c in 0..=j {
                let t = m[j][c];
                m[j][c] = m[p][c];
                m[p][c] = t;
            }
        }
        let r = 1.0 / m[j][j];
        for row in m.iter_mut().skip(j + 1) {
            row[j] *= r;
        }
    }
    Some((m, ipiv))
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

/// One extraction of a double by a C++ stream (libstdc++'s `operator>>`, observed on FSL's
/// matrix files): blanks skipped; the characters that may form a number accumulated (a sign,
/// digits, one decimal point, an exponent after a digit with its sign and digits: no `inf`,
/// `nan` or hexadecimal); the whole accumulated text must be a number, else the value is 0 and
/// the stream fails; a value beyond the double range is ±`f64::MAX` and fails the stream.
enum Extracted {
    /// The end of the text before any character of a number.
    End,
    Value(f64),
    /// The stream failed: this value is stored, every later extraction keeps it.
    Failed(f64),
}

fn extract(b: &[u8], pos: &mut usize) -> Extracted {
    while *pos < b.len() && c_space(b[*pos]) {
        *pos += 1;
    }
    if *pos >= b.len() {
        return Extracted::End;
    }
    let start = *pos;
    let mut i = start;
    if b[i] == b'+' || b[i] == b'-' {
        i += 1;
    }
    let (mut digit, mut point, mut exponent) = (false, false, false);
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            digit = true;
        } else if c == b'.' && !point && !exponent {
            point = true;
        } else if (c == b'e' || c == b'E') && digit && !exponent {
            exponent = true;
            if i + 1 < b.len() && (b[i + 1] == b'+' || b[i + 1] == b'-') {
                i += 1;
            }
        } else {
            break;
        }
        i += 1;
    }
    *pos = i;
    let text = std::str::from_utf8(&b[start..i]).unwrap_or("");
    match text.parse::<f64>() {
        Ok(v) if v.is_infinite() => Extracted::Failed(f64::MAX.copysign(v)),
        Ok(v) if digit && !text.ends_with(['e', 'E', '+', '-']) => Extracted::Value(v),
        _ => Extracted::Failed(0.0),
    }
}

/// C's `isspace` in the C locale.
fn c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Whether a word is all number for C's `strtod` (decimal, hexadecimal, `inf`, `nan`).
fn whole_number(word: &str) -> bool {
    let (_, used) = crate::cli::options::strtod(word);
    used > 0 && used == word.len()
}

/// The rows of numbers of a matrix file as FLIRT's programs read them (§2.2; observed on FSL
/// 6.0.7.23): the lines whose first word is a number for `strtod` (other lines are passed
/// over); the first such line's count of words is the number of columns; each line's values
/// are read as a C++ stream reads doubles ([`extract`]): a word that is not a number reads 0 and
/// the rest of the line repeats the last value (0, or ±`f64::MAX` after a value out of range);
/// a line that ends before its values are read is an error (`Could not find enough numbers in
/// matrix file`).
pub fn parse_matrix_text(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut cols = 0;
    for line in text.split('\n') {
        let mut words = line
            .split(|c: char| c.is_ascii() && c_space(c as u8))
            .filter(|w| !w.is_empty());
        let Some(first) = words.next() else { continue };
        if !whole_number(first) {
            continue;
        }
        if rows.is_empty() {
            cols = 1 + words.count();
        }
        let b = line.as_bytes();
        let mut pos = 0;
        let mut row = Vec::with_capacity(cols);
        let mut kept: Option<f64> = None;
        for _ in 0..cols {
            if let Some(v) = kept {
                row.push(v);
                continue;
            }
            match extract(b, &mut pos) {
                Extracted::End => {
                    return Err("Could not find enough numbers in matrix file".to_owned());
                }
                Extracted::Value(v) => row.push(v),
                Extracted::Failed(v) => {
                    row.push(v);
                    kept = Some(v);
                }
            }
        }
        rows.push(row);
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
        // the solve and LAPACK's inverse agree to rounding (their operation orders differ)
        let s = solve(&a, &IDENTITY).unwrap();
        for r in 0..4 {
            for c in 0..4 {
                assert!((s[r][c] - i[r][c]).abs() < 1e-15);
            }
        }
        // the sign of a zero as LAPACK leaves it (convert_xfm -inverse, observed)
        let z: Mat4 = [
            [0.9905325444, -0.1372781065, 0.0, 1.5],
            [0.1372781065, 0.9905325444, 0.0, 3.25],
            [0.0, 0.0, 1.05, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let zi = inverse(&z).unwrap();
        assert!(zi[2][3] == 0.0 && zi[2][3].is_sign_negative());
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
        // FSL 6.0.7.23's reader (observed through convert_xfm): inf and nan read 0 and fail the
        // line; a value out of range is the largest double, repeated; extra words ignored
        let r = |t: &str| parse_matrix_text(t);
        let rest = "0 1 0 0\n0 0 1 0\n0 0 0 1\n";
        assert_eq!(
            r(&format!("1 inf 0 0\n{rest}")).unwrap()[0],
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(r(&format!("0 0 inf 1\n{rest}")).unwrap()[0], [0.0; 4]);
        assert_eq!(
            r(&format!("1 0x10 0 3\n{rest}")).unwrap()[0],
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            r(&format!("1 2.5e 0 3\n{rest}")).unwrap()[0],
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            r(&format!("1 0 1e400 5\n{rest}")).unwrap()[0],
            [1.0, 0.0, f64::MAX, f64::MAX]
        );
        assert_eq!(r(&format!("0x1p3 1 0 0\n{rest}")).unwrap()[0], [0.0; 4]);
        assert!(r(&format!("1 0 0 0 junk\n{rest}")).is_err());
        assert!(r("1 2 3 4\n0 1 0 \n0 0 1 0\n0 0 0 1\n").is_err());
        assert_eq!(
            r("1 2 3 4\n0 1 nan\n0 0 1 0\n0 0 0 1\n").unwrap()[1],
            [0.0, 1.0, 0.0, 0.0]
        );
        let skipped = r("1 2 3 4\n.x 1 0 0\nx1 0\n# 1 2\n0 0 1 0\n0 0 0 1\n0 1 0 0\n").unwrap();
        assert_eq!(skipped.len(), 4);
        assert_eq!(
            r(&format!("1 2\\t3 4\n{rest}")).unwrap()[0],
            [1.0, 2.0, 0.0]
        );
        assert_eq!(r(&format!("1\\t2 3 4 5\n{rest}")).unwrap().len(), 3);
        assert_eq!(file_text(&IDENTITY).lines().next(), Some("1  0  0  0  "));
        assert_eq!(g(0.9998031425123, 10), "0.9998031425");
        assert_eq!(
            print_text(&IDENTITY).lines().next(),
            Some("1.000000 0.000000 0.000000 0.000000 ")
        );
    }
}

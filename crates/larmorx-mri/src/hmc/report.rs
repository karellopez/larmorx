// SPDX-License-Identifier: Apache-2.0
//! Motion parameters, RMS displacements and the text formats of the `.mat`, `.par` and `.rms`
//! files (`specs/mcflirt.md` §10.2–§10.4).

use super::rigid::{self, IDENTITY, Mat4};
use super::volume::Volume;

/// The RMS deviation between two affine matrices over a sphere of radius `radius` mm centred
/// at `centre` (Jenkinson 1999; §10.4): with `Δ = M1·M2⁻¹ − I`, `A` its linear part and
/// `τ = Δ[:3, 3] + A·centre`, `√(τ·τ + R²/5 · trace(AᵀA))`.
pub fn rms_deviation(m1: &Mat4, m2: &Mat4, centre: [f64; 3], radius: f64) -> f64 {
    let mut d = rigid::mul(m1, &rigid::inverse(m2));
    for (k, row) in d.iter_mut().enumerate().take(3) {
        row[k] -= 1.0;
    }
    let tau: Vec<f64> = (0..3)
        .map(|i| d[i][3] + d[i][0] * centre[0] + d[i][1] * centre[1] + d[i][2] * centre[2])
        .collect();
    let trace: f64 = (0..3)
        .flat_map(|i| (0..3).map(move |j| (i, j)))
        .map(|(i, j)| d[i][j] * d[i][j])
        .sum();
    (tau.iter().map(|t| t * t).sum::<f64>() + radius * radius / 5.0 * trace).sqrt()
}

/// The centre of a volume's field of view in FSL-mm: `0.5·(n − 1)·d` per axis.
pub fn fov_centre(v: &Volume) -> [f64; 3] {
    [0, 1, 2].map(|k| 0.5 * (v.shape[k] as f64 - 1.0) * f64::from(v.voxel_size[k]))
}

/// Absolute RMS displacements (`rms(I, M_t)`) and relative ones (`rms(M_{t−1}, M_t)`), with
/// mcflirt's radius of 80 mm about the reference's field-of-view centre.
pub fn rms_series(matrices: &[Mat4], reference: &Volume) -> (Vec<f64>, Vec<f64>) {
    let c = fov_centre(reference);
    let abs = matrices
        .iter()
        .map(|m| rms_deviation(&IDENTITY, m, c, 80.0))
        .collect();
    let rel = matrices
        .windows(2)
        .map(|w| rms_deviation(&w[0], &w[1], c, 80.0))
        .collect();
    (abs, rel)
}

/// The six motion parameters of every matrix, decomposed about the reference's
/// intensity-weighted centre (§10.3): `rx ry rz` in radians, `tx ty tz` in mm.
pub fn motion_parameters(matrices: &[Mat4], reference: &Volume) -> Vec<[f64; 6]> {
    let c = reference.centre_of_mass();
    matrices
        .iter()
        .map(|m| {
            let p = rigid::decompose(m, c);
            [p[0], p[1], p[2], p[3], p[4], p[5]]
        })
        .collect()
}

/// A number as C's `printf("%.6f")` prints it (`nan` and `-nan` for NaN by sign).
pub fn fixed6(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_owned();
    }
    format!("{v:.6}")
}

/// A number as C++'s default stream output prints it (`%g` with 6 significant digits).
pub fn general6(v: f64) -> String {
    general(v, 6)
}

/// C's `%.{precision}g`.
pub fn general(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_owned();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    let p = precision.max(1);
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if exp < -4 || exp >= p as i32 {
        let m = strip_zeros(mantissa);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{m}e{sign}{:02}", exp.unsigned_abs())
    } else {
        let decimals = (p as i32 - 1 - exp).max(0) as usize;
        strip_zeros(&format!("{v:.decimals$}")).to_owned()
    }
}

fn strip_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// The text of a `.mat` file: four rows, each number with 6 decimals followed by a space.
pub fn mat_text(m: &Mat4) -> String {
    let mut s = String::new();
    for row in m {
        for &v in row {
            s.push_str(&fixed6(v));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// The text of a `.par` file: one line per volume, six numbers each followed by two spaces.
pub fn par_text(params: &[[f64; 6]]) -> String {
    let mut s = String::new();
    for p in params {
        for &v in p {
            s.push_str(&general6(v));
            s.push_str("  ");
        }
        s.push('\n');
    }
    s
}

/// The text of a `.rms` file: one value per line.
pub fn rms_text(values: &[f64]) -> String {
    values.iter().map(|&v| general6(v) + "\n").collect()
}

/// Parses an FSL text matrix (4 rows of 4 numbers; extra lines ignored).
pub fn parse_mat(text: &str) -> Option<Mat4> {
    let numbers: Vec<f64> = text
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if numbers.len() < 16 {
        return None;
    }
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row.copy_from_slice(&numbers[4 * i..4 * i + 4]);
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_formats() {
        assert_eq!(general6(0.0970445123), "0.0970445");
        assert_eq!(general6(-0.0011228), "-0.0011228");
        assert_eq!(general6(1.56775), "1.56775");
        assert_eq!(general6(0.0), "0");
        assert_eq!(general6(-0.0), "-0");
        assert_eq!(general6(1e-5), "1e-05");
        assert_eq!(general6(-2.5e-7), "-2.5e-07");
        assert_eq!(general6(123456789.0), "1.23457e+08");
        assert_eq!(general6(100000.0), "100000");
        assert_eq!(general6(1000000.0), "1e+06");
        assert_eq!(general6(f64::NAN), "nan");
        assert_eq!(general6(-f64::NAN), "-nan");
        assert_eq!(fixed6(-0.0), "-0.000000");
        assert_eq!(fixed6(-1e-9), "-0.000000");
        assert_eq!(fixed6(0.0003271), "0.000327");
        assert_eq!(fixed6(f64::NAN), "nan");
    }

    #[test]
    fn mat_files_round_trip() {
        let mut m = IDENTITY;
        m[0][3] = -0.098268;
        let text = mat_text(&m);
        assert_eq!(
            text.lines().next().unwrap(),
            "1.000000 0.000000 0.000000 -0.098268 "
        );
        assert_eq!(parse_mat(&text), Some(m));
    }

    #[test]
    fn rms_of_a_translation_is_its_length() {
        let mut m = IDENTITY;
        m[0][3] = 3.0;
        m[1][3] = 4.0;
        let v = rms_deviation(&IDENTITY, &m, [10.0, 20.0, 30.0], 80.0);
        assert!((v - 5.0).abs() < 1e-12);
    }
}

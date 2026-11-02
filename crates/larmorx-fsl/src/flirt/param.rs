// SPDX-License-Identifier: Apache-2.0
//! FLIRT's two parametrisations of the rotation (`specs/flirt.md` §3): Euler angles (the shared
//! engine's, `larmorx_mri::hmc::rigid`) and, with `-anglerep quaternion`, the vector part of a
//! unit quaternion. The search always uses Euler angles (§8.1).

use larmorx_mri::hmc::rigid::{self, Params};

use super::cost::Log;
use super::mat::Mat4;

/// The rotation's parameters (`-anglerep`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AngleRep {
    #[default]
    Euler,
    Quaternion,
}

/// The rotation of the quaternion's vector part `(x, y, z)` (§3): `None` (FSL's message) when
/// `1 − x² − y² − z²` is negative.
fn quaternion_rotation(x: f64, y: f64, z: f64) -> Option<[[f64; 3]; 3]> {
    let w2 = (((1.0 - x * x) - y * y) - z * z) as f32;
    if w2 < 0.0 {
        return None;
    }
    let w = w2.sqrt();
    let (x, y, z) = (x as f32, y as f32, z as f32);
    let two = |a: f32, b: f32| (2.0 * a) * b;
    let r = [
        [
            (1.0 - two(y, y)) - two(z, z),
            two(x, y) - two(w, z),
            two(x, z) + two(w, y),
        ],
        [
            two(x, y) + two(w, z),
            (1.0 - two(x, x)) - two(z, z),
            two(y, z) - two(w, x),
        ],
        [
            two(x, z) - two(w, y),
            two(y, z) + two(w, x),
            (1.0 - two(x, x)) - two(y, y),
        ],
    ];
    Some(r.map(|row| row.map(f64::from)))
}

/// The matrix of the first `n` parameters of `p` about `centre` (§3), with `rep`'s rotation.
/// The quaternion's messages go to `log`.
pub fn compose(rep: AngleRep, p: &Params, n: usize, centre: [f64; 3], log: &Log) -> Mat4 {
    if rep == AngleRep::Euler {
        return rigid::compose(p, n, centre);
    }
    if n < 3 {
        log.push(format!("Can only do 3 or more, not {n}"));
    }
    let r = quaternion_rotation(p[0], p[1], p[2]).unwrap_or_else(|| {
        log.push("Parameters do not form a valid axis - greater than unity");
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    });
    let l = rigid::with_scales_skews(&r, p, n);
    let mut m = rigid::IDENTITY;
    let translations = match n {
        0..=3 => 3,
        4..=6 => n - 3,
        _ => 3,
    };
    for i in 0..3 {
        m[i][..3].copy_from_slice(&l[i]);
        let lc = l[i][0] * centre[0] + l[i][1] * centre[1] + l[i][2] * centre[2];
        let mut t = centre[i] - lc;
        if i < translations {
            t += p[3 + i];
        }
        m[i][3] = t;
    }
    m
}

/// The 12 parameters of `m` about `centre` (§3), with `rep`'s rotation: for quaternions the
/// rotation's entries in single precision turned into a quaternion as the NIfTI-1 reference
/// library does it (`bet::quatern`, shared with BET), its vector part.
pub fn decompose(rep: AngleRep, m: &Mat4, centre: [f64; 3]) -> Params {
    let mut p = rigid::decompose(m, centre);
    if rep == AngleRep::Euler || p.iter().any(|v| v.is_nan()) {
        return p;
    }
    let (r, _, _, _) = rigid::decompose_parts(m, centre);
    let rf = r.map(|row| row.map(|v| f64::from(v as f32)));
    let ([b, c, d], _) = crate::bet::quatern::mat_to_quatern(&rf);
    p[0] = b;
    p[1] = c;
    p[2] = d;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternions_round_trip() {
        let log = Log::default();
        let c = [40.0, 50.0, 30.0];
        let p: Params = [
            0.05, -0.02, 0.03, 1.0, -2.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0,
        ];
        let m = compose(AngleRep::Quaternion, &p, 6, c, &log);
        let q = decompose(AngleRep::Quaternion, &m, c);
        for k in 0..6 {
            assert!((p[k] - q[k]).abs() < 1e-5, "{k}: {} {}", p[k], q[k]);
        }
        assert!(log.take().is_empty());
        let bad: Params = [0.9, 0.9, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
        let m = compose(AngleRep::Quaternion, &bad, 6, c, &log);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(
            log.take(),
            ["Parameters do not form a valid axis - greater than unity"]
        );
    }
}

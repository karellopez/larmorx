// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface library's small inline helpers (`fs.surfio`, queue item 3.1-46), translated from
//! FreeSurfer 8.2.0's `include/mrisurf_base.h` (`sign`, `mrisNormalize`, `closeEnough`,
//! `project_point_onto_sphere`; `hasActiveRealmTrees` is [`crate::aaa::has_active_realm_trees`])
//! and `include/mrisurf_sphere_interp.h` (`Vec3` and its operators, `dot`, `cross`).
//!
//! The arithmetic is C's: `float` where FreeSurfer computes in `float` (C++'s `sqrt` and `fabs`
//! of a `float` are the `float` overloads), each operation in the source's order, with SSE's NaN
//! rules (`larmorx_freesurfer_core::sse`).

use larmorx_freesurfer_core::sse::{add, add64, div, mul, mul64, sqrt, sub, sub64};

/// `sign(t)` (`template <typename T> double sign`): 1.0 when `t > 0`, else -1.0 (NaN and 0
/// included).
pub fn sign_f32(t: f32) -> f64 {
    if t > 0.0 { 1.0 } else { -1.0 }
}

/// `sign(t)` for a `double`.
pub fn sign_f64(t: f64) -> f64 {
    if t > 0.0 { 1.0 } else { -1.0 }
}

/// `mrisNormalize(v)`: `d = sqrt(v0² + v1² + v2²)` in `float`; when `d > 0` each component is
/// divided by `d`. Returns `d`.
pub fn mris_normalize(v: &mut [f32; 3]) -> f32 {
    let d = sqrt(add(add(mul(v[0], v[0]), mul(v[1], v[1])), mul(v[2], v[2])));
    if d > 0.0 {
        v[0] = div(v[0], d);
        v[1] = div(v[1], d);
        v[2] = div(v[2], d);
    }
    d
}

/// `closeEnough(a, b)`: 1 when the larger magnitude is below 1e-6, or `|a - b|` is below 1 % of
/// it (the bound in double: `mag * 1.0e-2`); else 0.
pub fn close_enough(a: f32, b: f32) -> i32 {
    let mag = if a.abs() > b.abs() { a.abs() } else { b.abs() };
    if f64::from(mag) < 1.0e-6 {
        return 1;
    }
    if f64::from(sub(a, b).abs()) < f64::from(mag) * 1.0e-2 {
        return 1;
    }
    0
}

/// `project_point_onto_sphere(cx, cy, cz, radius, ...)`: in `float`, `dist = sqrt(cx² + cy² +
/// cz²)`, `d = 0` when `|dist| < FLT_EPSILON` (`FZERO`) else `1 - radius / dist`, and the point
/// `c - d * c`. Returns `NO_ERROR` (0) with the point.
pub fn project_point_onto_sphere(cx: f32, cy: f32, cz: f32, radius: f32) -> (f32, f32, f32) {
    let x2 = mul(cx, cx);
    let y2 = mul(cy, cy);
    let z2 = mul(cz, cz);
    let dist = sqrt(add(add(x2, y2), z2));
    let d = if dist.abs() < f32::EPSILON {
        0.0
    } else {
        sub(1.0, div(radius, dist))
    };
    let dx = mul(d, cx);
    let dy = mul(d, cy);
    let dz = mul(d, cz);
    (sub(cx, dx), sub(cy, dy), sub(cz, dz))
}

/// `Vec3` (`include/mrisurf_sphere_interp.h`): three doubles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3(pub [f64; 3]);

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3([x, y, z])
    }

    /// `a * b` (two vectors): `a0 b0 + a1 b1 + a2 b2` in double, left to right.
    pub fn inner(self, b: Vec3) -> f64 {
        add64(
            add64(mul64(self.0[0], b.0[0]), mul64(self.0[1], b.0[1])),
            mul64(self.0[2], b.0[2]),
        )
    }

    /// `a * s` and `s * a` (`operator*(double, Vec3)` is `vect * scalar`).
    pub fn scale(self, s: f64) -> Vec3 {
        Vec3::new(
            mul64(self.0[0], s),
            mul64(self.0[1], s),
            mul64(self.0[2], s),
        )
    }
}

impl std::ops::Add for Vec3 {
    type Output = Vec3;
    /// `a + b`, component by component.
    fn add(self, b: Vec3) -> Vec3 {
        Vec3::new(
            add64(self.0[0], b.0[0]),
            add64(self.0[1], b.0[1]),
            add64(self.0[2], b.0[2]),
        )
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;
    /// `a - b`.
    fn sub(self, b: Vec3) -> Vec3 {
        Vec3::new(
            sub64(self.0[0], b.0[0]),
            sub64(self.0[1], b.0[1]),
            sub64(self.0[2], b.0[2]),
        )
    }
}

/// `dot(u, v)`: [`Vec3::inner`] in double, returned as `float`.
pub fn dot(u: Vec3, v: Vec3) -> f32 {
    u.inner(v) as f32
}

/// `cross(u, v)`.
pub fn cross(u: Vec3, v: Vec3) -> Vec3 {
    let (a, b) = (u.0, v.0);
    Vec3::new(
        sub64(mul64(a[1], b[2]), mul64(a[2], b[1])),
        sub64(mul64(a[2], b[0]), mul64(a[0], b[2])),
        sub64(mul64(a[0], b[1]), mul64(a[1], b[0])),
    )
}

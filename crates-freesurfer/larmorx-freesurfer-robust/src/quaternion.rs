// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `Quaternion` (queue 3.2-03): the rotations of the robust registration's transformation models,
//! translated from FreeSurfer 8.2.0's `mri_robust_register/Quaternion.h` (its inline functions):
//! the rotation vector and the Euler angles in, the rotation matrix out, the product.
//!
//! `sin`, `cos` and `sqrt` in `double`: the correctly rounded functions by default, glibc's with
//! `reproduce` (`larmorx_core::math::Libm`; GLIBC-07 noted where the two differ); NaNs as x86-64
//! makes them. `mri_robust_template`'s rotation mean (queue 3.2-14) adds `importMatrix`,
//! `getRotAngle` (`acos`: the correctly rounded function in both modes, the project's decision
//! for glibc's `acos`), `getRotAxis` and `getRotVec`. The half rotation and the other averages
//! are not reached by either program.

use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::sse::{DEFAULT_NAN64, add64, div64, mul64, sqrt64, sub64};

/// A quaternion `a + b i + c j + d k`, and whether it is known to be normalised.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub normed: bool,
}

impl Default for Quaternion {
    /// `Quaternion()`: 1, normalised.
    fn default() -> Self {
        Quaternion {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            normed: true,
        }
    }
}

/// A libm result with x86-64's NaN: a NaN argument passed on quieted, else SSE's default NaN.
fn nan_of(r: f64, x: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if x.is_nan() {
        f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
    } else {
        DEFAULT_NAN64
    }
}

impl Quaternion {
    /// `importRotVec(v1, v2, v3)`: the rotation by `|v|` about `v` (`l = sqrt(v1^2 + v2^2 +
    /// v3^2)`; 1 for `l < 0.000001`; else `cos(l / 2)` and `sin(l / 2) v / l`).
    pub fn import_rot_vec(g: &mut Globals, v1: f64, v2: f64, v3: f64) -> Quaternion {
        let libm = Libm::new(g.mode());
        let l = sqrt64(add64(add64(mul64(v1, v1), mul64(v2, v2)), mul64(v3, v3)));
        if l < 0.000001 {
            return Quaternion::default();
        }
        let h = div64(l, 2.0);
        let slh = nan_of(libm.sin(h), h);
        let q = Quaternion {
            a: nan_of(libm.cos(h), h),
            b: div64(mul64(slh, v1), l),
            c: div64(mul64(slh, v2), l),
            d: div64(mul64(slh, v3), l),
            normed: true,
        };
        note_glibc07(g, &libm, "sin and cos");
        q
    }

    /// `importRotVec(alpha, v1, v2, v3)`: the rotation by `alpha` about `v` (`cos(alpha / 2)`
    /// and `sin(alpha / 2) v / |v|`; 1 for `|v| < 0.000001`).
    pub fn import_rot_vec_angle(
        g: &mut Globals,
        alpha: f64,
        v1: f64,
        v2: f64,
        v3: f64,
    ) -> Quaternion {
        let libm = Libm::new(g.mode());
        let l = sqrt64(add64(add64(mul64(v1, v1), mul64(v2, v2)), mul64(v3, v3)));
        if l < 0.000001 {
            return Quaternion::default();
        }
        let h = div64(alpha, 2.0);
        let slh = nan_of(libm.sin(h), h);
        let q = Quaternion {
            a: nan_of(libm.cos(h), h),
            b: div64(mul64(slh, v1), l),
            c: div64(mul64(slh, v2), l),
            d: div64(mul64(slh, v3), l),
            normed: true,
        };
        note_glibc07(g, &libm, "sin and cos");
        q
    }

    /// `importZYXAngles(z1, y2, x3)`: `qx3 * qy2 * qz1`, the rotations by `z1` about z, then `y2`
    /// about y, then `x3` about x.
    pub fn import_zyx_angles(g: &mut Globals, z1: f64, y2: f64, x3: f64) -> Quaternion {
        let qz1 = Quaternion::import_rot_vec_angle(g, z1, 0.0, 0.0, 1.0);
        let qy2 = Quaternion::import_rot_vec_angle(g, y2, 0.0, 1.0, 0.0);
        let qx3 = Quaternion::import_rot_vec_angle(g, x3, 1.0, 0.0, 0.0);
        let mut q = qx3.mul(&qy2).mul(&qz1);
        q.normed = true;
        q
    }

    /// `operator*`: the quaternion product, normalised when both are.
    pub fn mul(&self, v: &Quaternion) -> Quaternion {
        let (a, b, c, d) = (self.a, self.b, self.c, self.d);
        let an = sub64(
            sub64(sub64(mul64(a, v.a), mul64(b, v.b)), mul64(c, v.c)),
            mul64(d, v.d),
        );
        let bn = sub64(
            add64(add64(mul64(a, v.b), mul64(b, v.a)), mul64(c, v.d)),
            mul64(d, v.c),
        );
        let cn = add64(
            add64(sub64(mul64(a, v.c), mul64(b, v.d)), mul64(c, v.a)),
            mul64(d, v.b),
        );
        let dn = add64(
            sub64(add64(mul64(a, v.d), mul64(b, v.c)), mul64(c, v.b)),
            mul64(d, v.a),
        );
        Quaternion {
            a: an,
            b: bn,
            c: cn,
            d: dn,
            normed: self.normed && v.normed,
        }
    }

    /// `importMatrix(a00, ..., a22)`: the quaternion of a rotation matrix (`trace = a00 + a11 +
    /// a22`; above 0.00001 `s = 0.5 / sqrt(trace + 1)`, `a = 0.25 / s` and the differences of the
    /// off-diagonal pairs times `s`; otherwise from the largest diagonal element, `s = 2 sqrt(1 +
    /// that - the other two)`), normalised.
    #[allow(clippy::too_many_arguments)]
    pub fn import_matrix(
        a00: f64,
        a01: f64,
        a02: f64,
        a10: f64,
        a11: f64,
        a12: f64,
        a20: f64,
        a21: f64,
        a22: f64,
    ) -> Quaternion {
        let trace = add64(add64(a00, a11), a22);
        let (a, b, c, d);
        if trace > 0.00001 {
            let s = div64(0.5, sqrt64(add64(trace, 1.0)));
            a = div64(0.25, s);
            b = mul64(sub64(a21, a12), s);
            c = mul64(sub64(a02, a20), s);
            d = mul64(sub64(a10, a01), s);
        } else if a00 > a11 && a00 > a22 {
            let s = mul64(2.0, sqrt64(sub64(sub64(add64(1.0, a00), a11), a22)));
            a = div64(sub64(a21, a12), s);
            b = mul64(0.25, s);
            c = div64(add64(a01, a10), s);
            d = div64(add64(a02, a20), s);
        } else if a11 > a22 {
            let s = mul64(2.0, sqrt64(sub64(sub64(add64(1.0, a11), a00), a22)));
            a = div64(sub64(a02, a20), s);
            b = div64(add64(a01, a10), s);
            c = mul64(0.25, s);
            d = div64(add64(a12, a21), s);
        } else {
            let s = mul64(2.0, sqrt64(sub64(sub64(add64(1.0, a22), a00), a11)));
            a = div64(sub64(a10, a01), s);
            b = div64(add64(a02, a20), s);
            c = div64(add64(a12, a21), s);
            d = mul64(0.25, s);
        }
        Quaternion {
            a,
            b,
            c,
            d,
            normed: true,
        }
    }

    /// `getRotAngle()`: `2 acos(a / |q|)` (`|q|` the square root of `a a + b b + c c + d d`;
    /// `acos` correctly rounded in both modes).
    pub fn rot_angle(&self) -> f64 {
        let (a, b, c, d) = (self.a, self.b, self.c, self.d);
        let l = sqrt64(add64(
            add64(add64(mul64(a, a), mul64(b, b)), mul64(c, c)),
            mul64(d, d),
        ));
        let x = div64(a, l);
        mul64(2.0, nan_of(larmorx_core::math::acos(x), x))
    }

    /// `getRotAxis()`: `(b, c, d)` times `1 / sqrt(b b + c c + d d)`; all three 0 when one is
    /// a NaN (the identity's axis among them).
    pub fn rot_axis(&self) -> [f64; 3] {
        let (b, c, d) = (self.b, self.c, self.d);
        let l = sqrt64(add64(add64(mul64(b, b), mul64(c, c)), mul64(d, d)));
        let l = div64(1.0, l);
        let v = [mul64(b, l), mul64(c, l), mul64(d, l)];
        if v.iter().any(|x| x.is_nan()) {
            return [0.0; 3];
        }
        v
    }

    /// `getRotVec()`: the axis times the angle.
    pub fn rot_vec(&self) -> [f64; 3] {
        let v = self.rot_axis();
        let alpha = self.rot_angle();
        [mul64(v[0], alpha), mul64(v[1], alpha), mul64(v[2], alpha)]
    }

    /// `getNormalized()`: itself when normalised, else each part divided by the norm.
    pub fn normalized(&self) -> Quaternion {
        if self.normed {
            return *self;
        }
        let (a, b, c, d) = (self.a, self.b, self.c, self.d);
        let l = sqrt64(add64(
            add64(add64(mul64(a, a), mul64(b, b)), mul64(c, c)),
            mul64(d, d),
        ));
        Quaternion {
            a: div64(a, l),
            b: div64(b, l),
            c: div64(c, l),
            d: div64(d, l),
            normed: false,
        }
    }

    /// `getRotMatrix3d()`: the rotation matrix of the normalised quaternion, row by row.
    pub fn rot_matrix3d(&self) -> [f64; 9] {
        let q = self.normalized();
        let q11 = mul64(q.a, q.a);
        let q12 = mul64(q.a, q.b);
        let q13 = mul64(q.a, q.c);
        let q14 = mul64(q.a, q.d);
        let q22 = mul64(q.b, q.b);
        let q23 = mul64(q.b, q.c);
        let q24 = mul64(q.b, q.d);
        let q33 = mul64(q.c, q.c);
        let q34 = mul64(q.c, q.d);
        let q44 = mul64(q.d, q.d);
        [
            sub64(sub64(add64(q11, q22), q33), q44),
            mul64(2.0, sub64(q23, q14)),
            mul64(2.0, add64(q24, q13)),
            mul64(2.0, add64(q23, q14)),
            sub64(add64(sub64(q11, q22), q33), q44),
            mul64(2.0, sub64(q34, q12)),
            mul64(2.0, sub64(q24, q13)),
            mul64(2.0, add64(q34, q12)),
            add64(sub64(sub64(q11, q22), q33), q44),
        ]
    }
}

// SPDX-License-Identifier: Apache-2.0
//! The arithmetic of FLIRT's other programs (`specs/flirt.md` §15.1 to §15.4): `avscale`'s
//! decomposition and half transforms, the coordinate programs' mappings, `rmsdiff`'s deviations
//! and `makerot`'s rotation about an axis.
//!
//! Written from the spec and black-box runs of FSL 6.0.7.23's programs (the findings in
//! `docs/findings/fsl-flirt-original.md`, "FLIRT's other programs"), never from FSL's source.

use larmorx_core::math;
use larmorx_mri::hmc::rigid::{self, Params};

use super::cost::Log;
use super::mat::{self, IDENTITY, Mat4};
use super::param::{self, AngleRep};

/// The message of a decomposition whose matrix has a zero scale or a non-finite entry (FSL's
/// linear-algebra library's, observed).
pub const INVERSE_PROBLEM: &str =
    "matrix multiplication: problem with matrix inverse; suggest to use solve() instead";

/// The determinant as an LU decomposition with partial pivoting gives it: the product of the
/// factor's diagonal from the first entry, its sign changed once per row interchange; a zero
/// pivot is passed over (the column is left as it is), so a singular matrix gives `±0`.
pub fn determinant(a: &Mat4) -> f64 {
    let mut m = *a;
    let mut negative = false;
    for k in 0..4 {
        let mut p = k;
        let mut best = m[k][k].abs();
        for (i, row) in m.iter().enumerate().skip(k + 1) {
            if row[k].abs() > best {
                best = row[k].abs();
                p = i;
            }
        }
        if m[p][k] != 0.0 {
            if p != k {
                m.swap(p, k);
                negative = !negative;
            }
            let pivot = m[k][k];
            for i in k + 1..4 {
                m[i][k] /= pivot;
            }
        }
        for i in k + 1..4 {
            for j in k + 1..4 {
                m[i][j] -= m[i][k] * m[k][j];
            }
        }
    }
    let mut v = m[0][0];
    for (i, row) in m.iter().enumerate().skip(1) {
        v *= row[i];
    }
    if negative { -v } else { v }
}

/// The skew matrix of `k = (k_xy, k_xz, k_yz)` (§3: unit upper triangular).
fn skew(k: [f64; 3]) -> Mat4 {
    let mut m = IDENTITY;
    m[0][1] = k[0];
    m[0][2] = k[1];
    m[1][2] = k[2];
    m
}

/// The parameters with their scales and skews in single precision, the rotations and
/// translations in double (observed: avscale's average scale, its half transforms' and inverted
/// eddy matrix's last printed digits on three matrices; the spec does not say).
fn single_scales(mut p: Params) -> Params {
    for v in &mut p[6..12] {
        *v = f64::from(*v as f32);
    }
    p
}

/// What `avscale` prints of a matrix (§15.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Avscale {
    /// The 12 parameters about the centre: rotations (radians), translations, scales, skews.
    pub params: Params,
    /// The first six parameters composed about the origin.
    pub rotation_translation: Mat4,
    /// The mean of the scales, in single precision.
    pub average_scale: f32,
    pub determinant: f64,
}

/// The decomposition of `m` about `centre` (§15.1): `Err` with FSL's message when a scale is 0
/// or an entry of the top three rows is not finite.
pub fn avscale(m: &Mat4, centre: [f64; 3]) -> Result<Avscale, &'static str> {
    if m.iter().take(3).flatten().any(|v| !v.is_finite()) {
        return Err(INVERSE_PROBLEM);
    }
    let (_, s, _, _) = rigid::decompose_parts(m, centre);
    if s.contains(&0.0) {
        return Err(INVERSE_PROBLEM);
    }
    let params = single_scales(rigid::decompose(m, centre));
    let rotation_translation = rigid::compose(&params, 6, [0.0; 3]);
    let average_scale = (((params[6] + params[7]) + params[8]) / 3.0) as f32;
    Ok(Avscale {
        params,
        rotation_translation,
        average_scale,
        determinant: determinant(m),
    })
}

/// The half transforms of `m` (§15.1): the forward half (the matrix's square root) and the
/// backward half (the forward times `m⁻¹`); `Err` with FSL's message when `m` (or the
/// translation's system) is singular.
pub fn half_transforms(m: &Mat4, det: f64) -> Result<(Mat4, Mat4), &'static str> {
    let mp = if det < 0.0 {
        mat::mul(m, &mat::diagonal([-1.0, 1.0, 1.0]))
    } else {
        *m
    };
    let p = single_scales(param::decompose(AngleRep::Quaternion, &mp, [0.0; 3]));
    let norm = ((p[0] * p[0] + p[1] * p[1]) + p[2] * p[2]).sqrt();
    let t = math::asin(norm);
    let f = 1.0 / (2.0 * math::cos(t / 2.0));
    let mut half = rigid::IDENTITY_PARAMS;
    for k in 0..3 {
        half[k] = p[k] * f;
    }
    let r = param::compose(AngleRep::Quaternion, &half, 3, [0.0; 3], &Log::default());
    let k = skew([p[9] / 2.0, p[10] / 2.0, p[11] / 2.0]);
    let s = mat::diagonal([p[6].sqrt(), p[7].sqrt(), p[8].sqrt()]);
    let (Some(si), Some(ki), Some(ri)) = (mat::inverse(&s), mat::inverse(&k), mat::inverse(&r))
    else {
        return Err(mat::SINGULAR);
    };
    let mut a = mat::mul(&mat::mul(&mat::mul(&mp, &si), &ki), &ri);
    for (i, row) in a.iter_mut().enumerate() {
        row[i] += 1.0;
    }
    // the 3 x 3 system, solved within a 4 x 4 one whose last row and column are the identity's
    let mut a3 = IDENTITY;
    for i in 0..3 {
        a3[i][..3].copy_from_slice(&a[i][..3]);
    }
    let Some(tr) = mat::solve_vec(&a3, [m[0][3], m[1][3], m[2][3], 0.0]) else {
        return Err("solve(): solution not found");
    };
    let mut forward = mat::mul(&mat::mul(&r, &k), &s);
    for i in 0..3 {
        forward[i][3] = tr[i];
    }
    let Some(mi) = mat::inverse(m) else {
        return Err(mat::SINGULAR);
    };
    Ok((forward, mat::mul(&forward, &mi)))
}

/// `--inverteddies`' matrices (§15.1): the check `m⁻¹·R·K·S` and the inverted eddy-current
/// matrix `R·K'·S'` (the y scale inverted, the xy and yz skews negated); `None` when `m` is
/// singular.
pub fn inverted_eddies(m: &Mat4, a: &Avscale) -> Option<(Mat4, Mat4)> {
    let p = &a.params;
    let r = &a.rotation_translation;
    let k = skew([p[9], p[10], p[11]]);
    let s = mat::diagonal([p[6], p[7], p[8]]);
    let check = mat::mul(&mat::mul(&mat::mul(&mat::inverse(m)?, r), &k), &s);
    let k2 = skew([-p[9], p[10], -p[11]]);
    let s2 = mat::diagonal([p[6], 1.0 / p[7], p[8]]);
    Some((check, mat::mul(&mat::mul(r, &k2), &s2)))
}

/// The RMS deviation of `m1` and `m2` over a sphere of radius `r` about `centre` (§15.3,
/// `specs/mcflirt.md` §10.4): `√(τᵀτ + (r²/5)·tr(AᵀA))` with `Δ = m1·m2⁻¹ − I`, `A` its 3 × 3
/// part and `τ = Δ`'s translation `+ A·centre`; `None` when `m2` is singular.
pub fn rms_deviation(m1: &Mat4, m2: &Mat4, centre: [f64; 3], r: f64) -> Option<f64> {
    let mut d = mat::mul(m1, &mat::inverse(m2)?);
    for (i, row) in d.iter_mut().enumerate() {
        row[i] -= 1.0;
    }
    let mut tt = 0.0;
    for i in 0..3 {
        let t = d[i][3] + ((d[i][0] * centre[0] + d[i][1] * centre[1]) + d[i][2] * centre[2]);
        tt += t * t;
    }
    let mut tr = 0.0;
    for j in 0..3 {
        for row in d.iter().take(3) {
            tr += row[j] * row[j];
        }
    }
    Some((tt + (r * r / 5.0) * tr).sqrt())
}

/// The largest and the RMS distance `|(m1 − m2)·(x, y, z, 1)|` over the mask's voxels above 0.5
/// (§15.3): the mask's own indices (x fastest) times the reference's voxel sizes, each product
/// in single precision. A NaN distance never replaces the largest; an empty mask's RMS is NaN.
pub fn mask_deviation(
    m1: &Mat4,
    m2: &Mat4,
    mask: &[f32],
    shape: [usize; 3],
    voxel: [f32; 3],
) -> (f64, f64) {
    let mut d = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            d[i][j] = m1[i][j] - m2[i][j];
        }
    }
    let [nx, ny, nz] = shape;
    let (mut largest, mut sum, mut n) = (0.0f64, 0.0f64, 0usize);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if !(mask[i + nx * (j + ny * k)] > 0.5) {
                    continue;
                }
                let p = [
                    f64::from(i as f32 * voxel[0]),
                    f64::from(j as f32 * voxel[1]),
                    f64::from(k as f32 * voxel[2]),
                    1.0,
                ];
                let v = mat::apply(&d, p);
                let dist = (((v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]) + v[3] * v[3]).sqrt();
                if dist > largest {
                    largest = dist;
                }
                sum += dist * dist;
                n += 1;
            }
        }
    }
    let rms = if n == 0 {
        sse::DEFAULT_NAN
    } else {
        (sum / n as f64).sqrt()
    };
    (largest, rms)
}

/// The geometry of an image as the coordinate programs see it (§15.2): its voxel-size diagonal
/// `S`, its NIfTI-to-FSL voxel matrix `N` and its millimetre matrix `V`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Space {
    pub s: Mat4,
    pub n: Mat4,
    pub v: Mat4,
}

impl Space {
    /// An image of `shape` and `voxel` sizes; `form` its sform (else qform) as stored, `None`
    /// when neither is set (the reader's default: the voxel sizes with x reversed about the
    /// field of view, observed); `neurological` whether it is stored so (x reversed in memory,
    /// the form following). `None` when a matrix it needs is singular.
    pub fn of_image(
        shape: [usize; 3],
        voxel: [f32; 3],
        form: Option<Mat4>,
        neurological: bool,
    ) -> Option<Space> {
        let s = mat::diagonal(voxel.map(f64::from));
        let fov = ((shape[0] as f32 - 1.0) * voxel[0]) as f64;
        let mut f = IDENTITY;
        f[0][0] = -1.0;
        f[0][3] = fov;
        let fs = mat::mul(&f, &s);
        let (n, v) = match form {
            // no form: the voxel sizes, x not reversed (observed in millimetres, 2026-11-02)
            None => (IDENTITY, s),
            Some(w) if neurological => {
                let n = mat::solve(&s, &fs)?;
                (n, mat::mul(&w, &n))
            }
            Some(w) => (IDENTITY, w),
        };
        Some(Space { s, n, v })
    }

    /// A destination or standard image read with neither sform nor qform: its world
    /// coordinates its voxel sizes, x not reversed (observed, 2026-11-02), as
    /// [`Space::of_image`] gives them for any image without forms.
    pub fn unset(voxel: [f32; 3]) -> Space {
        let s = mat::diagonal(voxel.map(f64::from));
        Space {
            s,
            n: IDENTITY,
            v: s,
        }
    }

    /// The standard volume of `std2imgcoord` and `img2stdcoord` without `-std`: no sform or
    /// qform, 1 mm voxels (§15.2).
    pub fn empty() -> Space {
        Space {
            s: IDENTITY,
            n: IDENTITY,
            v: IDENTITY,
        }
    }
}

/// `S_t⁻¹·(M·(S_s·u))` (§15.2): FSL voxels of `source` through `m` to FSL voxels of `target`.
fn through(u: [f64; 4], m: &Mat4, source: &Space, target: &Space) -> Option<[f64; 4]> {
    mat::solve_vec(&target.s, mat::apply(m, mat::apply(&source.s, u)))
}

/// `img2imgcoord`'s mapping of `c` (NIfTI voxels, or millimetres with `mm`) from `src` to `dest`
/// through `m`.
pub fn img2img(c: [f64; 3], m: &Mat4, src: &Space, dest: &Space, mm: bool) -> Option<[f64; 4]> {
    let c = [c[0], c[1], c[2], 1.0];
    if mm {
        let u = mat::solve_vec(&src.v, c)?;
        Some(mat::apply(&dest.v, through(u, m, src, dest)?))
    } else {
        mat::solve_vec(&dest.n, through(mat::apply(&src.n, c), m, src, dest)?)
    }
}

/// `std2imgcoord`'s mapping of `c` (the standard image's millimetres) through `m⁻¹` to the
/// input image's millimetres, or its NIfTI voxels with `vox`. `Err` with FSL's message when `m`
/// is singular, `Ok(None)` when another matrix is.
pub fn std2img(
    c: [f64; 3],
    m: &Mat4,
    img: &Space,
    std: &Space,
    vox: bool,
) -> Result<Option<[f64; 4]>, &'static str> {
    let mi = mat::inverse(m).ok_or(mat::SINGULAR)?;
    let Some(u) = mat::solve_vec(&std.v, [c[0], c[1], c[2], 1.0]) else {
        return Ok(None);
    };
    let Some(w) = through(u, &mi, std, img) else {
        return Ok(None);
    };
    Ok(if vox {
        mat::solve_vec(&img.n, w)
    } else {
        Some(mat::apply(&img.v, w))
    })
}

/// `img2stdcoord`'s mapping of `c` (the input image's NIfTI voxels, or millimetres with `mm`)
/// through `m` to the standard image's millimetres.
pub fn img2std(c: [f64; 3], m: &Mat4, img: &Space, std: &Space, mm: bool) -> Option<[f64; 4]> {
    let c = [c[0], c[1], c[2], 1.0];
    let u = if mm {
        mat::solve_vec(&img.v, c)?
    } else {
        mat::apply(&img.n, c)
    };
    Some(mat::apply(&std.v, through(u, m, img, std)?))
}

/// x86-64's rules for NaN operands, written out so that every platform gives the same NaN signs
/// (`makerot`, §15.4): an operation with one NaN operand returns it, with two the first one's;
/// an invalid operation on numbers gives the default NaN, whose sign bit is set.
mod sse {
    pub const DEFAULT_NAN: f64 = f64::from_bits(0xfff8_0000_0000_0000);

    #[inline]
    fn pick(a: f64, b: f64, r: f64) -> f64 {
        if a.is_nan() {
            a
        } else if b.is_nan() {
            b
        } else if r.is_nan() {
            DEFAULT_NAN
        } else {
            r
        }
    }

    pub fn add(a: f64, b: f64) -> f64 {
        pick(a, b, a + b)
    }

    pub fn sub(a: f64, b: f64) -> f64 {
        pick(a, b, a - b)
    }

    pub fn mul(a: f64, b: f64) -> f64 {
        pick(a, b, a * b)
    }

    pub fn div(a: f64, b: f64) -> f64 {
        pick(a, b, a / b)
    }

    pub fn sqrt(a: f64) -> f64 {
        if a.is_nan() {
            a
        } else if a < 0.0 {
            DEFAULT_NAN
        } else {
            a.sqrt()
        }
    }

    /// The length of `v`: the squares summed from the first, the running sum first.
    pub fn norm(v: [f64; 3]) -> f64 {
        sqrt(add(add(mul(v[0], v[0]), mul(v[1], v[1])), mul(v[2], v[2])))
    }

    /// A double rounded to single precision and back, a NaN keeping its sign.
    pub fn to_f32(a: f64) -> f32 {
        if a.is_nan() {
            if a.is_sign_negative() {
                -f32::NAN
            } else {
                f32::NAN
            }
        } else {
            a as f32
        }
    }

    pub fn from_f32(a: f32) -> f64 {
        if a.is_nan() {
            if a.is_sign_negative() {
                -f64::NAN
            } else {
                f64::NAN
            }
        } else {
            f64::from(a)
        }
    }

    /// `a·x` for a 3 × 3 matrix in the order of §15.4: the first row as `x₀·a₀₀`, then
    /// `(x_k·a₀ₖ) + s`; the other rows as `a_r0·x₀`, then `s + a_rk·x_k`.
    pub fn matvec(a: &[[f64; 3]; 3], x: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        let mut s = mul(x[0], a[0][0]);
        for k in 1..3 {
            s = add(mul(x[k], a[0][k]), s);
        }
        out[0] = s;
        for r in 1..3 {
            let mut s = mul(a[r][0], x[0]);
            for k in 1..3 {
                s = add(s, mul(a[r][k], x[k]));
            }
            out[r] = s;
        }
        out
    }
}

/// The length of an axis as `makerot` measures it (zero only for an exactly zero axis).
pub fn axis_length(a: [f64; 3]) -> f64 {
    sse::norm(a)
}

/// `makerot`'s matrix (§15.4): a rotation by `theta` degrees (single precision) about `axis`
/// through `centre`. The axis is divided by its length, or with `fsl76` by the square root of
/// its length as FSL divides it (FSL-76). The axis must not be zero ([`axis_length`]).
pub fn makerot(theta: f32, axis: [f64; 3], centre: [f64; 3], fsl76: bool) -> Mat4 {
    use sse::{div, matvec, mul, norm, sub};
    let len = norm(axis);
    let scale = if fsl76 { sse::sqrt(len) } else { len };
    let theta = sse::from_f32(theta);
    let w = axis.map(|a| div(mul(mul(div(a, scale), theta), std::f64::consts::PI), 180.0));
    let angle = sse::to_f32(norm(w));
    let mut out = IDENTITY;
    if angle < 1e-8 {
        return out;
    }
    let a = sse::from_f32(angle);
    let n = w.map(|v| div(v, a));
    let mut u = [-n[1], n[0], 0.0];
    let nu = norm(u);
    if nu == 0.0 {
        u = [1.0, 0.0, 0.0];
    } else {
        u = u.map(|v| div(v, nu));
    }
    let v = [
        sub(mul(n[1], u[2]), mul(n[2], u[1])),
        sub(mul(n[2], u[0]), mul(n[0], u[2])),
        sub(mul(n[0], u[1]), mul(n[1], u[0])),
    ];
    let nv = norm(v);
    let v = v.map(|x| div(x, nv));
    let (c, s) = if angle.is_finite() {
        (math::cosf(angle), math::sinf(angle))
    } else {
        let d = sse::to_f32(sub(a, a));
        (d, d)
    };
    let (c, s) = (sse::from_f32(c), sse::from_f32(s));
    let b = [[u[0], v[0], n[0]], [u[1], v[1], n[1]], [u[2], v[2], n[2]]];
    let core = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]];
    let column = |m: &[[f64; 3]; 3], j: usize| [m[0][j], m[1][j], m[2][j]];
    let mut br = [[0.0; 3]; 3];
    for j in 0..3 {
        let col = matvec(&b, column(&core, j));
        for i in 0..3 {
            br[i][j] = col[i];
        }
    }
    let mut r = [[0.0; 3]; 3];
    for j in 0..3 {
        let col = matvec(&br, b[j]);
        for i in 0..3 {
            r[i][j] = col[i];
        }
    }
    let mut imr = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            imr[i][j] = sub(if i == j { 1.0 } else { 0.0 }, r[i][j]);
        }
    }
    let t = matvec(&imr, centre);
    for i in 0..3 {
        out[i][..3].copy_from_slice(&r[i]);
        out[i][3] = t[i];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat4, b: &[[f64; 4]; 3], tol: f64) -> bool {
        (0..3).all(|i| (0..4).all(|j| (a[i][j] - b[i][j]).abs() <= tol * b[i][j].abs().max(1e-30)))
    }

    type Case = (f32, [f64; 3], [f64; 3], [[f64; 4]; 3]);

    #[test]
    fn makerot_as_fsl_writes_it() {
        // FSL 6.0.7.23's makerot -o (10 significant digits; observed, FSL-76 reproduced)
        let cases: [Case; 5] = [
            (
                -45.5,
                [0.0, 0.0, 4.0],
                [1.0, 2.0, 3.0],
                [
                    [-0.01745235175, -0.9998477101, 0.0, 3.017147772],
                    [0.9998477101, -0.01745235175, 0.0, 1.035056993],
                    [0.0, 0.0, 1.000000069, -2.083199193e-07],
                ],
            ),
            (
                30.0,
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [
                    [0.5630827959, 0.7473922704, -0.3526224518, 0.1264421753],
                    [-0.6129562133, 0.6639098386, 0.4283788329, 1.562002663],
                    [0.5542765374, -0.02507066169, 0.83195491, -1.083482301],
                ],
            ),
            (
                30.0,
                [f64::from(1e-30f32), 0.0, 1.0],
                [1.0, 2.0, 3.0],
                [
                    [0.8660253882, 0.5, 1.339745565e-31, -0.8660253882],
                    [-0.5, 0.8660253882, 5.000000016e-31, 0.7679492235],
                    [
                        1.339745565e-31,
                        -5.000000016e-31,
                        0.9999999443,
                        1.669652047e-07,
                    ],
                ],
            ),
            (
                30.0,
                [0.0, 1.0, 0.0],
                [1.0, 2.0, 3.0],
                [
                    [0.8660253882, 0.0, -0.5, 1.633974612],
                    [0.0, 0.9999999443, 0.0, 1.113101364e-07],
                    [0.5, 0.0, 0.8660253882, -0.09807616472],
                ],
            ),
            (
                5.75e-7,
                [0.0, 0.0, 1.0],
                [0.0; 3],
                [
                    [1.0, 1.003564343e-08, 0.0, 0.0],
                    [-1.003564343e-08, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 0.9999999593, 0.0],
                ],
            ),
        ];
        for (theta, axis, centre, want) in cases {
            let m = makerot(theta, axis, centre, true);
            assert!(close(&m, &want, 6e-10), "{theta} {axis:?}: {m:?}");
            assert_eq!(mat::g(m[2][3], 10), mat::g(want[2][3], 10));
        }
        // below 1e-8 radians: the identity
        assert_eq!(makerot(5.7e-7, [0.0, 0.0, 1.0], [0.0; 3], true), IDENTITY);
        // FSL-76 corrected: 30 degrees about (0, 0, 4) is 30 degrees
        let m = makerot(30.0, [0.0, 0.0, 4.0], [0.0; 3], false);
        assert!((m[0][0] - f64::from(30f32.to_radians().cos())).abs() < 1e-7);
    }

    #[test]
    fn makerot_nan_signs() {
        // FSL: a NaN axis component (observed: rows 1, 2 `nan`, row 3 `-nan`)
        let m = makerot(30.0, [f64::NAN, 1.0, 1.0], [1.0, 2.0, 3.0], true);
        for j in 0..4 {
            assert!(m[0][j].is_nan() && m[0][j].is_sign_positive());
            assert!(m[1][j].is_nan() && m[1][j].is_sign_positive());
            assert!(m[2][j].is_nan() && m[2][j].is_sign_negative());
        }
        // an infinite angle (observed: rows 1, 2 `-nan`, row 3 `nan`)
        let m = makerot(f32::INFINITY, [0.0, 0.0, 1.0], [1.0, 2.0, 3.0], true);
        for j in 0..4 {
            assert!(m[0][j].is_nan() && m[0][j].is_sign_negative(), "{m:?}");
            assert!(m[1][j].is_nan() && m[1][j].is_sign_negative());
            assert!(m[2][j].is_nan() && m[2][j].is_sign_positive());
        }
    }

    #[test]
    fn determinants() {
        let a: Mat4 = [
            [1.05, 0.02, -0.03, 2.0],
            [-0.01, 0.97, 0.04, -3.0],
            [0.02, -0.05, 1.1, 1.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!((determinant(&a) - 1.123253).abs() < 1e-6);
        let mut s = a;
        s[3] = [0.0; 4];
        assert_eq!(determinant(&s), 0.0);
    }
}

// SPDX-License-Identifier: Apache-2.0
//! applytopup's corrections (FSL 6.0.7.23's `applytopup`, `specs/topup.md` §7, §9.4, §10): the
//! Jacobian modulation (`--method=jac`) and the least-squares restoration (`--method=lsr`) of
//! J. L. R. Andersson, S. Skare and J. Ashburner (NeuroImage 20(2):870-888, 2003), a clean-room
//! original written from the spec, FSL's ApplytopupUsersGuide and black-box runs of FSL's program
//! (`docs/findings/fsl-topup-original.md`, findings 17 to 22).
//!
//! Every image is on the field's grid, x fastest, in FSL's frame (radiological storage
//! mirrored along x as read). Positions are in voxels; the movements act in mm on `index ·
//! voxel size`. Observed (black-box runs, findings 17 to 20):
//! - the interpolation (cubic B-splines or trilinear) is periodic along every axis, as topup's;
//! - an image `i` with phase encoding `p` and read-out time `t` is sampled at `M⁻¹ x + h(x) t
//!   (px, py, 0)` (a phase encoding's z component moves nothing), `M` its movement; a voxel is
//!   valid when that position lies in `[0, n - 1]` along every axis but the phase encoding's
//!   when the encoding is along x or y alone, along every axis otherwise;
//! - `jac`'s Jacobian is `1 + t (px ∂h/∂x + py ∂h/∂y)` (per voxel): the coefficients' own
//!   derivatives, or, for a field image, the derivatives of its cubic spline (coefficients with
//!   mirrored ends, zero beyond the image);
//! - `lsr`'s collections, lines and matrices as §7.4 describes them, a line with an invalid
//!   voxel skipped in every collection (its voxels 0 in the output); the matrices built row by
//!   row by FSL's walk (§7.4, spec writer 2026-11-03), the mapping at a negative index read
//!   circularly by default and as FSL reads it with FSL-133 reproduced.

use rayon::prelude::*;

use super::images::{Grid, HORIZON, interp, pole, prefilter};
use super::model::Scan;
use super::movement::Motion;
use super::spline::{Axis, Op1, apply3};

/// The images' interpolation (`--interp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Trilinear,
    Spline,
}

/// The field on the images' grid: its values (Hz) and its derivatives along each axis (Hz per
/// voxel).
#[derive(Clone, Debug)]
pub struct FieldOnGrid {
    pub dims: [usize; 3],
    pub h: Vec<f64>,
    pub dh: [Vec<f64>; 3],
    /// The voxels whose Jacobian is 0 (FSL-134 reproduced: a field of another shape, its
    /// Jacobian 0 beyond its own grid); empty when none.
    pub jac_zero: Vec<bool>,
}

impl FieldOnGrid {
    /// FSL-134 reproduced (`specs/topup.md` §7.3, spec writer 2026-11-03, observed): this field
    /// of a coefficient file, evaluated on its own grid, laid onto the grid `to` of as many
    /// voxels: the displacement from the values in storage order (the image's voxel of linear
    /// index `L` takes the field's of index `L`), the Jacobian's derivatives at the same indices
    /// `(x, y, z)` (the field's beyond the image's grid dropped, the image's beyond the field's
    /// grid a Jacobian of 0).
    pub fn laid_onto(&self, to: [usize; 3]) -> FieldOnGrid {
        let n = to.iter().product::<usize>();
        let f = self.dims;
        let inside = |v: usize| {
            let x = [v % to[0], (v / to[0]) % to[1], v / (to[0] * to[1])];
            (x[0] < f[0] && x[1] < f[1] && x[2] < f[2]).then(|| x[0] + f[0] * (x[1] + f[1] * x[2]))
        };
        let dh = std::array::from_fn(|a| {
            (0..n)
                .map(|v| inside(v).map_or(0.0, |w| self.dh[a][w]))
                .collect()
        });
        FieldOnGrid {
            dims: to,
            h: self.h[..n].to_vec(),
            dh,
            jac_zero: (0..n).map(|v| inside(v).is_none()).collect(),
        }
    }
}

/// The field of topup's coefficients `coef` (dims `cdims`, x fastest) for a field of `n` voxels
/// with knot spacings `k` and B-splines of `order`, evaluated at its voxels, and whether it is
/// laid with the general quadratic count on an axis where FSL's differs (FSL-145: FSL refuses
/// such a file); `None` when the coefficients' grid is not that layout's. `fsl_only`
/// (FSL-145 reproduced): FSL's own count only.
pub fn coefficient_field(
    coef: &[f64],
    cdims: [usize; 3],
    n: [usize; 3],
    k: [usize; 3],
    order: u8,
    fsl_only: bool,
) -> Option<(FieldOnGrid, bool)> {
    // the layout whose count is the file's (FSL-145: on an axis shorter than two knot spacings
    // FSL writes one or two quadratic coefficients, the default the general count)
    let mut general_on_short = false;
    let axes: [Axis; 3] = std::array::from_fn(|a| {
        let axis = Axis::of_order(n[a], k[a], order);
        let fsl = axis.with_fsl145(true);
        if fsl_only || (axis.ncoef() != cdims[a] && fsl.ncoef() == cdims[a]) {
            fsl
        } else {
            general_on_short |= axis.short_quadratic();
            axis
        }
    });
    if (0..3).any(|a| axes[a].ncoef() != cdims[a]) || coef.len() != cdims.iter().product() {
        return None;
    }
    let ops: [[Op1; 2]; 3] = std::array::from_fn(|a| [axes[a].at_voxels(0), axes[a].at_voxels(1)]);
    let eval = |deriv: Option<usize>| {
        let o: [&Op1; 3] = std::array::from_fn(|a| &ops[a][usize::from(deriv == Some(a))]);
        apply3(coef, cdims, [Some(o[0]), Some(o[1]), Some(o[2])]).0
    };
    Some((
        FieldOnGrid {
            dims: n,
            h: eval(None),
            dh: [eval(Some(0)), eval(Some(1)), eval(Some(2))],
            jac_zero: Vec::new(),
        },
        general_on_short,
    ))
}

/// The cubic B-spline coefficients of samples `s` with mirrored ends, in place (`specs/topup.md`
/// §7.3, spec writer 2026-11-03): §4.5's recursive prefilter with the mirrored start, the forward
/// initial value `Σ_{i<n} z^i f[i]` truncated at `n = min(N, 15)` terms and the backward one
/// `−z / (1 − z²) (2 c⁺[N − 1] − f[N − 1])`, the coefficients `6 c⁻`. For `N ≥ 15` these are the
/// mirrored spline's (`c[-1] = c[1]`, `c[n] = c[n - 2]`) to about 1e-9.
fn mirror_prefilter_line(s: &mut [f64]) {
    let nn = s.len();
    if nn < 2 {
        return;
    }
    let z = pole();
    let n = nn.min(HORIZON);
    let last = s[nn - 1];
    let mut zi = 1.0;
    let mut c0 = 0.0;
    for &v in &s[..n] {
        c0 += zi * v;
        zi *= z;
    }
    s[0] = c0;
    for i in 1..nn {
        s[i] += z * s[i - 1];
    }
    s[nn - 1] = -z / (1.0 - z * z) * (2.0 * s[nn - 1] - last);
    for i in (0..nn - 1).rev() {
        s[i] = z * (s[i + 1] - s[i]);
    }
    for v in s.iter_mut() {
        *v *= 6.0;
    }
}

/// Applies `f` to every line of `data` (dims `d`) along `axis`.
fn for_lines(data: &mut [f64], d: [usize; 3], axis: usize, f: impl Fn(&mut [f64]) + Sync) {
    let stride = [1, d[0], d[0] * d[1]][axis];
    let n = d[axis];
    let plane = d[0] * d[1];
    let starts: Vec<usize> = (0..data.len())
        .filter(|&v| [v % d[0], (v / d[0]) % d[1], v / plane][axis] == 0)
        .collect();
    let lines: Vec<(usize, Vec<f64>)> = starts
        .par_iter()
        .map(|&s0| {
            let mut line: Vec<f64> = (0..n).map(|j| data[s0 + j * stride]).collect();
            f(&mut line);
            (s0, line)
        })
        .collect();
    for (s0, line) in lines {
        for (j, v) in line.into_iter().enumerate() {
            data[s0 + j * stride] = v;
        }
    }
}

/// The field image `h` (Hz, dims `n`) with its derivatives: the cubic spline of `h` with mirrored
/// ends, its derivative along each axis evaluated at the voxels with the coefficients beyond the
/// image taken as zero (observed: FSL's Jacobians of field images, findings 18).
pub fn image_field(h: Vec<f64>, n: [usize; 3]) -> FieldOnGrid {
    image_field_on(h, n, n)
}

/// [`image_field`] on the grid `to` of as many voxels (FSL-134 reproduced, `specs/topup.md`
/// §7.3): the values laid in storage order for the displacement, the spline's derivatives
/// evaluated at the image's voxels `(x, y, z)` with the coefficients beyond the field image's
/// own grid taken as zero.
pub fn image_field_on(h: Vec<f64>, n: [usize; 3], to: [usize; 3]) -> FieldOnGrid {
    let mut c = h.clone();
    for axis in 0..3 {
        for_lines(&mut c, n, axis, mirror_prefilter_line);
    }
    // the coefficients on a grid covering both (zeros beyond the field image's)
    let big: [usize; 3] = std::array::from_fn(|a| n[a].max(to[a]));
    if big != n {
        let mut p = vec![0.0; big.iter().product()];
        for z in 0..n[2] {
            for y in 0..n[1] {
                for x in 0..n[0] {
                    p[x + big[0] * (y + big[1] * z)] = c[x + n[0] * (y + n[1] * z)];
                }
            }
        }
        c = p;
    }
    let dh: [Vec<f64>; 3] = std::array::from_fn(|deriv| {
        let mut d = c.clone();
        for axis in 0..3 {
            let k: [f64; 3] = if axis == deriv {
                [-0.5, 0.0, 0.5]
            } else {
                [1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0]
            };
            for_lines(&mut d, big, axis, |line| {
                let m = line.len();
                let src = line.to_vec();
                for (j, out) in line.iter_mut().enumerate() {
                    let at = |i: isize| {
                        if i < 0 || i as usize >= m {
                            0.0
                        } else {
                            src[i as usize]
                        }
                    };
                    let j = j as isize;
                    *out = k[0] * at(j - 1) + k[1] * at(j) + k[2] * at(j + 1);
                }
            });
        }
        if big == to {
            d
        } else {
            let mut o = Vec::with_capacity(to.iter().product());
            for z in 0..to[2] {
                for y in 0..to[1] {
                    for x in 0..to[0] {
                        o.push(d[x + big[0] * (y + big[1] * z)]);
                    }
                }
            }
            o
        }
    });
    FieldOnGrid {
        dims: to,
        h,
        dh,
        jac_zero: Vec::new(),
    }
}

/// How an image is moved before its correction: its sampling position `P = A x + b` (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampling {
    pub a: [[f64; 3]; 3],
    pub b: [f64; 3],
}

impl Sampling {
    /// topup's movement `p` (tx, ty, tz, rx, ry, rz) about the centre `c` (mm): `R^T (x - c -
    /// t) + c`, the inverse of `--rbmout`'s matrix.
    pub fn of_movement(p: [f64; 6], c: [f64; 3]) -> Sampling {
        let m = Motion::new(p, c);
        Sampling {
            a: m.rt,
            b: m.offset,
        }
    }

    /// The inverse of the 4 × 4 matrix `m` (by rows): a movement given as a matrix; `None` when
    /// it is singular.
    pub fn of_matrix(m: [[f64; 4]; 4]) -> Option<Sampling> {
        let r = [
            [m[0][0], m[0][1], m[0][2]],
            [m[1][0], m[1][1], m[1][2]],
            [m[2][0], m[2][1], m[2][2]],
        ];
        let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
            - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
            + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let cof = |i: usize, j: usize| {
            let (i1, i2) = ((i + 1) % 3, (i + 2) % 3);
            let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
            r[i1][j1] * r[i2][j2] - r[i1][j2] * r[i2][j1]
        };
        let a: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| cof(j, i) / det));
        let t = [m[0][3], m[1][3], m[2][3]];
        let b = std::array::from_fn(|i| -(0..3).map(|k| a[i][k] * t[k]).sum::<f64>());
        Some(Sampling { a, b })
    }

    pub fn is_identity(&self) -> bool {
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        self.a == id && self.b == [0.0; 3]
    }

    #[inline]
    fn apply(&self, x: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            self.a[i][0] * x[0] + self.a[i][1] * x[1] + self.a[i][2] * x[2] + self.b[i]
        })
    }
}

/// One input image: its volumes (each the field's grid) and acquisition.
#[derive(Clone, Debug)]
pub struct Image {
    pub volumes: Vec<Vec<f32>>,
    pub scan: Scan,
    /// Its movement (`None`: none).
    pub sampling: Option<Sampling>,
}

/// A correction's settings.
#[derive(Clone, Debug)]
pub struct Settings {
    pub dims: [usize; 3],
    pub voxel: [f64; 3],
    pub interp: Interp,
    /// FSL-132 reproduced: the first volume of the first image interpolated trilinearly.
    pub first_trilinear: bool,
    /// The Jacobian's terms' scales along x and y (`specs/topup.md` §7.3): a coefficient
    /// field's the image's voxel size over the field's (1 on its own grid); a field image's 1, or
    /// with FSL-134 reproduced FSL's, the image's x voxel size over the field image's.
    pub jac_scale: [f64; 2],
    /// FSL-133 reproduced: the mapping at a negative index read as FSL reads it.
    pub fsl133: bool,
}

/// The output: each volume's values (the images' volume count) and the mask.
#[derive(Clone, Debug)]
pub struct Corrected {
    pub volumes: Vec<Vec<f32>>,
    pub mask: Vec<bool>,
    /// FSL-133: a restored line's matrix read the mapping at a negative index where FSL's read
    /// and the circular one take different field values.
    pub wrap_read_differs: bool,
}

/// The axis an encoding wraps along: the only one of x and y it has a component along.
pub fn wrap_axis(pe: [f64; 3]) -> Option<usize> {
    match (pe[0] != 0.0, pe[1] != 0.0) {
        (true, false) => Some(0),
        (false, true) => Some(1),
        _ => None,
    }
}

/// A volume prepared for interpolation.
fn interpolant(v: &[f32], dims: [usize; 3], linear: bool) -> Grid {
    let g = Grid {
        dims,
        data: v.iter().map(|&x| f64::from(x)).collect(),
        linear,
    };
    if linear { g } else { prefilter(&g) }
}

/// The sampling positions (voxels) of an image and their validity: `M⁻¹ x` plus, when `field`
/// is given, the field's displacement.
fn positions(s: &Settings, img: &Image, field: Option<&FieldOnGrid>) -> (Vec<[f64; 3]>, Vec<bool>) {
    let [nx, ny, _] = s.dims;
    let nvox = s.dims.iter().product::<usize>();
    let wrap = wrap_axis(img.scan.pe);
    let e = [
        img.scan.pe[0] * img.scan.t,
        img.scan.pe[1] * img.scan.t,
        0.0,
    ];
    (0..nvox)
        .into_par_iter()
        .map(|v| {
            let x = [v % nx, (v / nx) % ny, v / (nx * ny)];
            let mut p: [f64; 3] = std::array::from_fn(|a| x[a] as f64);
            if let Some(m) = &img.sampling {
                let q = m.apply(std::array::from_fn(|a| p[a] * s.voxel[a]));
                p = std::array::from_fn(|a| q[a] / s.voxel[a]);
            }
            if let Some(f) = field {
                for a in 0..2 {
                    p[a] += f.h[v] * e[a];
                }
            }
            let valid =
                (0..3).all(|a| Some(a) == wrap || (p[a] >= 0.0 && p[a] <= (s.dims[a] - 1) as f64));
            (p, valid)
        })
        .unzip()
}

/// The values of `volume` at `pos` (`linear`: trilinear, else cubic B-splines).
fn sample(volume: &[f32], dims: [usize; 3], pos: &[[f64; 3]], linear: bool) -> Vec<f64> {
    let g = interpolant(volume, dims, linear);
    pos.par_iter()
        .map(|&p| interp(&g, p, [false; 3]).0)
        .collect()
}

/// `--method=jac` (§7.3): each image moved, displaced and modulated by the Jacobian, averaged
/// over the images volume by volume; 0 outside the mask.
pub fn jac(s: &Settings, images: &[Image], field: &FieldOnGrid) -> Corrected {
    let nvox = s.dims.iter().product::<usize>();
    let n_vols = images.first().map_or(0, |i| i.volumes.len());
    let mut mask = vec![true; nvox];
    let mut sums = vec![vec![0.0f64; nvox]; n_vols];
    for (k, img) in images.iter().enumerate() {
        let (pos, valid) = positions(s, img, Some(field));
        for (m, v) in mask.iter_mut().zip(&valid) {
            *m &= *v;
        }
        let (px, py, t) = (img.scan.pe[0], img.scan.pe[1], img.scan.t);
        let [sx, sy] = s.jac_scale;
        let jac: Vec<f64> = (0..nvox)
            .into_par_iter()
            .map(|v| {
                if field.jac_zero.get(v) == Some(&true) {
                    0.0
                } else {
                    1.0 + t * (px * field.dh[0][v] * sx + py * field.dh[1][v] * sy)
                }
            })
            .collect();
        for (vi, vol) in img.volumes.iter().enumerate() {
            let linear = s.interp == Interp::Trilinear || (s.first_trilinear && k == 0 && vi == 0);
            let f = sample(vol, s.dims, &pos, linear);
            sums[vi]
                .par_iter_mut()
                .zip(f.par_iter().zip(jac.par_iter()))
                .for_each(|(acc, (fv, jv))| *acc += fv * jv);
        }
    }
    let m = images.len().max(1) as f64;
    let volumes = sums
        .into_iter()
        .map(|sum| {
            sum.par_iter()
                .zip(mask.par_iter())
                .map(|(&x, &ok)| if ok { (x / m) as f32 } else { 0.0 })
                .collect()
        })
        .collect();
    Corrected {
        volumes,
        mask,
        wrap_read_differs: false,
    }
}

/// Why the least-squares restoration cannot run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LsrError {
    /// An encoding along both x and y, or along neither.
    Vector,
    /// A collection without two different displacements per Hz.
    Combination,
}

/// The collections of `images` (§7.4): the axis (x or y) and its images, x's first.
pub fn collections(images: &[Image]) -> Result<Vec<(usize, Vec<usize>)>, LsrError> {
    let mut by_axis: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
    for (k, img) in images.iter().enumerate() {
        match wrap_axis(img.scan.pe) {
            Some(a) if img.scan.pe[2] == 0.0 => by_axis[a].push(k),
            _ => return Err(LsrError::Vector),
        }
    }
    let mut out = Vec::new();
    for (axis, members) in by_axis.into_iter().enumerate() {
        if members.is_empty() {
            continue;
        }
        let pts: Vec<f64> = members
            .iter()
            .map(|&k| images[k].scan.pe[axis] * images[k].scan.t)
            .collect();
        if !pts.iter().any(|&p| p != pts[0]) {
            return Err(LsrError::Combination);
        }
        out.push((axis, members));
    }
    Ok(out)
}

/// The resampling matrix of one line (§7.4; Andersson et al. 2003), row-major `n × n`, and
/// whether a read of the mapping at a negative index took a field value FSL's read would not
/// (FSL-133). `K[i][j]` is the length (in undistorted voxels) of `[u₁, u₂] ∩ [j − 1/2, j +
/// 1/2]`, the columns wrapping circularly, where `u₁` and `u₂` are the points at which the
/// mapping `φ(k) = k + pt h_{w(k)}` (linear between the integers) reaches observed voxel `i`'s
/// edges `i ∓ 1/2`, each found by FSL's walk from `⌈x⌉` (`specs/topup.md` §7.4, spec writer
/// 2026-11-03, observed): up while `x > φ(k)` (the last `k` below), `k` itself when `x =
/// φ(k)`, else down while `x < φ(k − 1)`; then `u = k + (x − φ(k)) / (φ(k + 1) − φ(k))`.
/// Where `φ` increases this is the overlap of voxel `j`'s image with voxel `i`; where it folds
/// the walk takes the crossing it meets (FSL's, rule C). `w(k) = k mod n` for `k ≥ 0`; for `k <
/// 0` the circular `n + k`, or with `fsl133` FSL's `(k + 2³²) mod n` (FSL-133).
pub fn resampling_matrix(h: &[f64], pt: f64, fsl133: bool) -> (Vec<f64>, bool) {
    let n = h.len();
    let mut k = vec![0.0; n * n];
    if n == 0 {
        return (k, false);
    }
    let ni = n as i64;
    let differs = std::cell::Cell::new(false);
    let phi = |a: i64| {
        let j = if a >= 0 {
            (a % ni) as usize
        } else {
            let circular = a.rem_euclid(ni) as usize;
            let fsl = (a + (1i64 << 32)).rem_euclid(ni) as usize;
            if h[fsl].to_bits() != h[circular].to_bits() {
                differs.set(true);
            }
            if fsl133 { fsl } else { circular }
        };
        a as f64 + h[j] * pt
    };
    // the walk's reach: φ(k) lies within the largest displacement of k
    let reach = h.iter().fold(0.0f64, |m, &v| m.max((v * pt).abs()));
    let steps = if reach.is_finite() {
        reach.min(1e7) as i64 + 4
    } else {
        4
    };
    let crossing = |x: f64| -> f64 {
        let mut c = x.ceil() as i64;
        let start = c;
        let base = if x > phi(c) {
            while x > phi(c) && c - start <= steps {
                c += 1;
            }
            c - 1
        } else if x == phi(c) {
            c
        } else {
            while x < phi(c - 1) && start - c <= steps {
                c -= 1;
            }
            c - 1
        };
        let (a, b) = (phi(base), phi(base + 1));
        if x == a {
            base as f64
        } else {
            base as f64 + (x - a) / (b - a)
        }
    };
    for i in 0..n {
        let u1 = crossing(i as f64 - 0.5);
        let u2 = crossing(i as f64 + 0.5);
        if !(u2 > u1) {
            continue;
        }
        let first = (u1 + 0.5).floor() as i64;
        let last = (u2 + 0.5).floor() as i64;
        for j in first..=last {
            let ov = u2.min(j as f64 + 0.5) - u1.max(j as f64 - 0.5);
            if ov > 0.0 {
                k[i * n + j.rem_euclid(ni) as usize] += ov;
            }
        }
    }
    (k, differs.get())
}

/// The Cholesky factor (lower, row-major) of the symmetric positive definite `a` (`n × n`);
/// `None` when it is not positive definite.
pub(super) fn cholesky(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if s <= 0.0 || !s.is_finite() {
                    return None;
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    Some(l)
}

pub(super) fn cholesky_solve(l: &[f64], n: usize, b: &mut [f64]) {
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i * n + k] * b[k];
        }
        b[i] = s / l[i * n + i];
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for k in i + 1..n {
            s -= l[k * n + i] * b[k];
        }
        b[i] = s / l[i * n + i];
    }
}

/// The regularisation weight of the restoration (§7.4).
pub const LSR_LAMBDA: f64 = 0.01;

/// `SᵀS` of the line's second-difference matrix (rows `(-1, 2, -1)`, the first `(1, -1)`, the
/// last `(-1, 1)`), row-major `n × n`.
fn second_difference_gram(n: usize) -> Vec<f64> {
    let mut s = vec![0.0; n * n];
    if n >= 2 {
        s[0] = 1.0;
        s[1] = -1.0;
        s[(n - 1) * n + n - 2] = -1.0;
        s[(n - 1) * n + n - 1] = 1.0;
    }
    for i in 1..n.saturating_sub(1) {
        s[i * n + i - 1] = -1.0;
        s[i * n + i] = 2.0;
        s[i * n + i + 1] = -1.0;
    }
    let mut g = vec![0.0; n * n];
    for r in 0..n {
        for i in 0..n {
            let si = s[r * n + i];
            if si == 0.0 {
                continue;
            }
            for j in 0..n {
                g[i * n + j] += si * s[r * n + j];
            }
        }
    }
    g
}

/// Each image moved onto the first's space (no displacement; positions outside the grid
/// interpolated periodically), volume by volume, and the mask: every image's validity.
pub(super) fn moved_images(s: &Settings, images: &[Image]) -> (Vec<Vec<Vec<f64>>>, Vec<bool>) {
    let nvox = s.dims.iter().product::<usize>();
    let mut mask = vec![true; nvox];
    let mut moved: Vec<Vec<Vec<f64>>> = Vec::with_capacity(images.len());
    for (k, img) in images.iter().enumerate() {
        let (pos, valid) = positions(s, img, None);
        for (m, v) in mask.iter_mut().zip(&valid) {
            *m &= *v;
        }
        let identity = img.sampling.is_none_or(|m| m.is_identity());
        moved.push(
            img.volumes
                .iter()
                .enumerate()
                .map(|(vi, vol)| {
                    if identity {
                        vol.iter().map(|&x| f64::from(x)).collect()
                    } else {
                        let linear = s.interp == Interp::Trilinear
                            || (s.first_trilinear && k == 0 && vi == 0);
                        sample(vol, s.dims, &pos, linear)
                    }
                })
                .collect(),
        );
    }
    (moved, mask)
}

/// One line of a restoration: its first voxel, its restored volumes (`None`: skipped) and
/// whether its matrices read the wrap-around where FSL-133's read differs.
type RestoredLine = (usize, Option<Vec<Vec<f64>>>, bool);

/// The lines along `axis` (x or y) of a grid `dims`: their first voxels in storage order (the
/// other in-plane axis fastest, then z) and the stride along a line.
pub(super) fn line_starts(dims: [usize; 3], axis: usize) -> (Vec<usize>, usize) {
    let [nx, ny, nz] = dims;
    let stride = if axis == 0 { 1 } else { nx };
    let starts = (0..nz)
        .flat_map(|z| {
            (0..(if axis == 0 { ny } else { nx })).map(move |o| {
                if axis == 0 {
                    z * nx * ny + o * nx
                } else {
                    z * nx * ny + o
                }
            })
        })
        .collect();
    (starts, stride)
}

/// `--method=lsr` (§7.4): each image moved onto the first's space, then each collection's lines
/// restored and the collections added; a line with an invalid voxel is 0 in the output.
pub fn lsr(s: &Settings, images: &[Image], field: &FieldOnGrid) -> Result<Corrected, LsrError> {
    let colls = collections(images)?;
    let nvox = s.dims.iter().product::<usize>();
    let n_vols = images.first().map_or(0, |i| i.volumes.len());
    let (moved, mask) = moved_images(s, images);
    let mut out = vec![vec![0.0f64; nvox]; n_vols];
    let mut skipped = vec![false; nvox];
    let mut wrap_read_differs = false;
    for (axis, members) in &colls {
        let axis = *axis;
        let n = s.dims[axis];
        let (starts, stride) = line_starts(s.dims, axis);
        let gram = second_difference_gram(n);
        let pts: Vec<f64> = members
            .iter()
            .map(|&k| images[k].scan.pe[axis] * images[k].scan.t)
            .collect();
        let lines: Vec<RestoredLine> = starts
            .par_iter()
            .map(|&s0| {
                if (0..n).any(|j| !mask[s0 + j * stride]) {
                    return (s0, None, false);
                }
                let h: Vec<f64> = (0..n).map(|j| field.h[s0 + j * stride]).collect();
                let mut differs = false;
                let ks: Vec<Vec<f64>> = pts
                    .iter()
                    .map(|&pt| {
                        let (m, d) = resampling_matrix(&h, pt, s.fsl133);
                        differs |= d;
                        m
                    })
                    .collect();
                // A = Σ KᵀK + λ SᵀS
                let mut a: Vec<f64> = gram.iter().map(|g| LSR_LAMBDA * g).collect();
                for kmat in &ks {
                    for r in 0..n {
                        let row = &kmat[r * n..(r + 1) * n];
                        let nz_cols: Vec<usize> = (0..n).filter(|&j| row[j] != 0.0).collect();
                        for &i in &nz_cols {
                            for &j in &nz_cols {
                                a[i * n + j] += row[i] * row[j];
                            }
                        }
                    }
                }
                let Some(l) = cholesky(&a, n) else {
                    return (s0, None, differs);
                };
                let restored = (0..n_vols)
                    .map(|vi| {
                        let mut b = vec![0.0; n];
                        for (kmat, &k) in ks.iter().zip(members) {
                            let y = &moved[k][vi];
                            for r in 0..n {
                                let yr = y[s0 + r * stride];
                                if yr == 0.0 {
                                    continue;
                                }
                                for j in 0..n {
                                    b[j] += kmat[r * n + j] * yr;
                                }
                            }
                        }
                        cholesky_solve(&l, n, &mut b);
                        b
                    })
                    .collect();
                (s0, Some(restored), differs)
            })
            .collect();
        for (s0, r, differs) in lines {
            wrap_read_differs |= differs && r.is_some();
            match r {
                None => {
                    for j in 0..n {
                        skipped[s0 + j * stride] = true;
                    }
                }
                Some(vols) => {
                    for (vi, f) in vols.into_iter().enumerate() {
                        for (j, v) in f.into_iter().enumerate() {
                            out[vi][s0 + j * stride] += v;
                        }
                    }
                }
            }
        }
    }
    let volumes = out
        .into_iter()
        .map(|o| {
            o.iter()
                .zip(&skipped)
                .map(|(&x, &sk)| if sk { 0.0 } else { x as f32 })
                .collect()
        })
        .collect();
    let mask = skipped.iter().map(|&sk| !sk).collect();
    Ok(Corrected {
        volumes,
        mask,
        wrap_read_differs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_field_resamples_each_voxel_onto_itself() {
        let (k, _) = resampling_matrix(&[0.0; 6], 0.05, false);
        for i in 0..6 {
            for j in 0..6 {
                assert_eq!(k[i * 6 + j], if i == j { 1.0 } else { 0.0 });
            }
        }
    }

    #[test]
    fn every_voxel_is_resampled_whole() {
        let h = [3.0, -2.0, 5.0, 8.0, 1.0, -4.0, 0.0, 2.5];
        for pt in [0.05, -0.05, 0.11] {
            let (k, _) = resampling_matrix(&h, pt, false);
            for j in 0..8 {
                let s: f64 = (0..8).map(|i| k[i * 8 + j]).sum();
                assert!((s - 1.0).abs() < 1e-12, "{s}");
            }
        }
    }

    #[test]
    fn fsl133_reads_the_middle_of_a_line_of_30() {
        // the mapping at -1 takes voxel 15's field in FSL's read, voxel 29's circularly
        let mut h = vec![0.0; 30];
        h[15] = 4.0;
        h[29] = -4.0;
        let (a, differs) = resampling_matrix(&h, 0.05, false);
        let (b, same) = resampling_matrix(&h, 0.05, true);
        assert!(differs && same);
        assert!(a.iter().zip(&b).any(|(x, y)| x != y));
        // a power of two wraps the same in both reads
        let h32: Vec<f64> = (0..32).map(|i| (i % 7) as f64 - 3.0).collect();
        let (c, d32) = resampling_matrix(&h32, 0.05, false);
        let (e, _) = resampling_matrix(&h32, 0.05, true);
        assert!(!d32);
        assert_eq!(c, e);
    }

    #[test]
    fn a_field_of_another_shape_is_laid_in_storage_order() {
        let f = FieldOnGrid {
            dims: [4, 2, 1],
            h: (0..8).map(f64::from).collect(),
            dh: std::array::from_fn(|a| (0..8).map(|v| f64::from(v + 10 * a as i32)).collect()),
            jac_zero: Vec::new(),
        };
        let l = f.laid_onto([2, 4, 1]);
        assert_eq!(l.h, f.h);
        // (x, y) = (1, 1) is the field's index 1 + 4 = 5; (0, 2) lies beyond the field's grid
        assert_eq!(l.dh[0][1 + 2], 5.0);
        assert!(l.jac_zero[4] && !l.jac_zero[3]);
    }

    #[test]
    fn mirror_prefilter_interpolates() {
        // from 15 voxels on, the mirrored spline's coefficients (to ~1e-9)
        let s0: Vec<f64> = (0..17).map(|i| ((i * 5 + 1) % 7) as f64 - 2.0).collect();
        let mut c = s0.clone();
        mirror_prefilter_line(&mut c);
        let n = c.len();
        let at = |i: isize| {
            c[if i < 0 {
                1
            } else if i as usize >= n {
                n - 2
            } else {
                i as usize
            }]
        };
        for (j, &v) in s0.iter().enumerate() {
            let j = j as isize;
            let back = (at(j - 1) + 4.0 * at(j) + at(j + 1)) / 6.0;
            assert!((back - v).abs() < 1e-7, "{j}: {back} {v}");
        }
    }

    #[test]
    fn a_matrix_movement_inverts() {
        let p = [1.5, -2.0, 0.5, 0.02, -0.01, 0.03];
        let c = [10.0, 12.0, 8.0];
        let m = super::super::movement::matrix(p, c);
        let a = Sampling::of_matrix(m).unwrap();
        let b = Sampling::of_movement(p, c);
        for i in 0..3 {
            assert!((a.b[i] - b.b[i]).abs() < 1e-12);
            for j in 0..3 {
                assert!((a.a[i][j] - b.a[i][j]).abs() < 1e-12);
            }
        }
    }
}

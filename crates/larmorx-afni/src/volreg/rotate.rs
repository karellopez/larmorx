// SPDX-License-Identifier: Apache-2.0
//! Rotating and shifting a volume (`specs/3dvolreg.md` §4.2, §4.3, §5.1 to §5.5): the rotation
//! matrices, the 180° flip, the four-shear decomposition and its choice among six axis orders,
//! the zero padding, and the shears applied row by row.
//!
//! The decomposition is solved with the stable formulas of §5.3 (the spec does not give
//! AFNI's own evaluation; its last bits are AFNI's, §16), the factor `f` as a cube root of the
//! determinant. Everything after it (the coefficients rounded to float32, the row shifts of
//! [`super::shift`], the pairing of rows, the skip of a negligible shear) follows the spec.
//!
//! The rows of a shear are independent: they are shifted in parallel, by planes (the x- and
//! y-shears' rows lie in z planes; the z-shear's rows are gathered by y slabs into a scratch
//! volume and written back by planes). Each row's result is computed the same way whichever
//! thread computes it, so the result does not depend on the thread count.

use rayon::prelude::*;

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math;

use super::grid::M3;
use super::shift::{self, FourierWork, Method};

/// The sine and cosine of the rotations: correctly rounded by default, glibc 2.35's with
/// `reproduce` (§4.3, GLIBC-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trig {
    Correct,
    Glibc,
}

impl Trig {
    pub fn of(mode: UpstreamDefects) -> Trig {
        if mode.reproduce() {
            Trig::Glibc
        } else {
            Trig::Correct
        }
    }

    /// `(sin x, cos x)` from the separate functions.
    pub fn sin_cos(self, x: f64) -> (f64, f64) {
        match self {
            Trig::Correct => (math::sin(x), math::cos(x)),
            Trig::Glibc => (math::sin_glibc(x), math::cos_glibc(x)),
        }
    }

    /// `(sin x, cos x)` from the combined function (glibc's `sincos`).
    pub fn sincos(self, x: f64) -> (f64, f64) {
        match self {
            Trig::Correct => (math::sin(x), math::cos(x)),
            Trig::Glibc => math::sincos_glibc(x),
        }
    }

    pub fn exp(self, x: f64) -> f64 {
        match self {
            Trig::Correct => math::exp(x),
            Trig::Glibc => math::exp_glibc(x),
        }
    }
}

/// The elementary rotation `E_axis(θ)` of §4.3 from its sine and cosine.
fn elementary(axis: usize, (s, c): (f64, f64)) -> M3 {
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        1 => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
        _ => [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// `A·B`, each entry summed left to right.
pub fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j])
    })
}

/// `A·v`, each entry summed left to right.
pub fn mul_vec(a: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

pub fn transpose(a: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

/// The rotation of three angles (radians) `R(a1, θ1; a2, θ2; a3, θ3) = E_a3·(E_a2·E_a1)`
/// (§4.3): the third pair of sine and cosine from the combined function.
pub fn rot3(axes: [usize; 3], theta: [f64; 3], trig: Trig) -> M3 {
    let e1 = elementary(axes[0], trig.sin_cos(theta[0]));
    let e2 = elementary(axes[1], trig.sin_cos(theta[1]));
    let e3 = elementary(axes[2], trig.sincos(theta[2]));
    mul(&e3, &mul(&e2, &e1))
}

/// A rigid transformation to apply to a volume (§5.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    /// Angles (radians, float32) about the internal axes and a shift (mm along the dataset
    /// axes), applied after the rotation.
    Angles {
        theta: [f32; 3],
        axes: [usize; 3],
        shift: [f32; 3],
    },
    /// A matrix (dataset axes; its nearest orthogonal matrix is used) and a shift (mm).
    Matrix { m: M3, shift: [f64; 3] },
}

/// One shear of §5.3: along `axis`, the coefficients `a`, `b` on the other two axes in index
/// order, and the shift `s`, all in index units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shear {
    pub axis: usize,
    pub a: f64,
    pub b: f64,
    pub s: f64,
}

/// How a transformation is carried out: the flip of §5.2 (the two axes flipped) and the four
/// shears, or `None` when no decomposition was found (AFNI then leaves the volume as it was).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub flip: Option<(usize, usize)>,
    pub shears: Option<[Shear; 4]>,
}

/// The nearest orthogonal matrix (`U·Vᵀ` of the SVD), by the polar decomposition; the matrix
/// itself when it is singular.
fn nearest_orthogonal(m: &M3) -> M3 {
    larmorx_core::linalg::polar_orthogonal(m).unwrap_or(*m)
}

/// The four shears of the transformation `p ↦ q·(p − c) + c + e` (index units), §5.3: the
/// shortcut for pure shifts, the six axis orders with the smallest norm, the retry.
fn decompose(q: &M3, e: [f64; 3], trig: Trig) -> Option<[Shear; 4]> {
    let trace = q[0][0] + q[1][1] + q[2][2];
    let off = q[0][1].abs()
        + q[0][2].abs()
        + q[1][0].abs()
        + q[1][2].abs()
        + q[2][0].abs()
        + q[2][1].abs();
    if trace >= 2.99999 && off / trace < 1.0e-6 {
        let zero = |axis, s| Shear {
            axis,
            a: 0.0,
            b: 0.0,
            s,
        };
        return Some([zero(0, 0.0), zero(1, e[1]), zero(2, e[2]), zero(0, e[0])]);
    }
    if let Some(s) = best_order(q, e) {
        return Some(s);
    }
    let p = rot3([0, 1, 2], [1.09e-6, 1.22e-6, 1.37e-6], trig);
    let q2 = mul(&transpose(&p), &mul(q, &p));
    best_order(&q2, e)
}

const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn best_order(q: &M3, e: [f64; 3]) -> Option<[Shear; 4]> {
    let mut best: Option<(f64, [Shear; 4])> = None;
    for o in ORDERS {
        let qp: M3 = std::array::from_fn(|m| std::array::from_fn(|n| q[o[m]][o[n]]));
        let ep = [e[o[0]], e[o[1]], e[o[2]]];
        let Some((norm, coef)) = solve_full(&qp, ep) else {
            continue;
        };
        if best.as_ref().is_some_and(|(b, _)| norm >= *b) {
            continue;
        }
        let along = [0usize, 1, 2, 0];
        let shears = std::array::from_fn(|m| {
            let (c, s) = coef[m];
            let axis = o[along[m]];
            // The coefficients on the original axes, then the other two in index order.
            let mut on = [0.0f64; 3];
            for (pn, &v) in c.iter().enumerate() {
                on[o[pn]] = v;
            }
            let others: Vec<usize> = (0..3).filter(|&a| a != axis).collect();
            Shear {
                axis,
                a: on[others[0]],
                b: on[others[1]],
                s,
            }
        });
        best = Some((norm, shears));
    }
    best.map(|(_, s)| s)
}

/// The solution of §5.3 step 2 for one axis order (`q` and `e` already permuted): its norm and
/// each shear's coefficients on the permuted axes and shift (X1, Y, Z, X2); `None` when this
/// order fails.
fn solve_full(q: &M3, e: [f64; 3]) -> Option<(f64, [([f64; 3], f64); 4])> {
    let d = super::grid::det(q);
    let (q10, q11, q12) = (q[1][0], q[1][1], q[1][2]);
    let (q20, q21, q22) = (q[2][0], q[2][1], q[2][2]);
    let t = q11 * q20 - q10 * q21;
    if d == 0.0 || q10 == 0.0 || t == 0.0 || !d.is_finite() {
        return None;
    }
    let f = math::cbrt(d);
    let ay = q10;
    let b1 = (q11 - f) / q10;
    let az = t / f;
    let bz = (q20 - az) / q10;
    let c1 = (q22 - f - bz * q12) / az;
    let cy = q12 - q10 * c1;
    let det2 = q10 * q21 - q20 * q11;
    let r0 = q[0][0] - f;
    let r1 = q[0][1] - f * b1;
    let b2 = (r0 * q21 - q20 * r1) / det2;
    let c2 = (q10 * r1 - q11 * r0) / det2;
    let sy = e[1];
    let sz = e[2] - bz * e[1];
    let sx = e[0] - b2 * e[1] - c2 * e[2];
    let norm = [b1, c1, ay, cy, az, bz]
        .iter()
        .fold(0.0f64, |m, v| m.max(v.abs()));
    if !norm.is_finite() {
        return None;
    }
    Some((
        norm,
        [
            ([0.0, b1, c1], 0.0),
            ([ay, 0.0, cy], sy),
            ([az, bz, 0.0], sz),
            ([0.0, b2, c2], sx),
        ],
    ))
}

/// The plan of a transformation for a (padded) grid of `dims` voxels of sizes `sizes` (mm).
pub fn plan(tf: &Transform, sizes: [f32; 3], trig: Trig) -> Plan {
    let (mut q, d) = match *tf {
        Transform::Angles { theta, axes, shift } => {
            let theta = if theta == [0.0; 3] {
                [1.0e-6f32, 1.1e-6, 0.9e-6]
            } else {
                theta
            };
            let neg = theta.map(|t| -f64::from(t));
            (rot3(axes, neg, trig), shift.map(f64::from))
        }
        Transform::Matrix { m, shift } => (nearest_orthogonal(&m), shift),
    };
    // §5.2: the 180° flip.
    let mut flip = None;
    if q[0][0] + q[1][1] + q[2][2] < 1.0 {
        let mut t = 0;
        for a in 1..3 {
            if q[a][a] > q[t][t] {
                t = a;
            }
        }
        let others: Vec<usize> = (0..3).filter(|&a| a != t).collect();
        let (p, r) = (others[0], others[1]);
        if q[p][p] + q[r][r] < -0.02 {
            flip = Some((p, r));
            let mut fm = [[0.0; 3]; 3];
            for (a, row) in fm.iter_mut().enumerate() {
                row[a] = if a == p || a == r { -1.0 } else { 1.0 };
            }
            q = mul(&q, &fm);
        }
    }
    // §4.2: index units.
    let delta = sizes.map(|s| if s == 0.0 { 1.0 } else { f64::from(s) });
    let mut qi = q;
    for m in 0..3 {
        for n in 0..3 {
            if m != n {
                qi[m][n] = q[m][n] * delta[n] / delta[m];
            }
        }
    }
    let di = [0, 1, 2].map(|n| d[n] / delta[n]);
    Plan {
        flip,
        shears: decompose(&qi, di, trig),
    }
}

/// What a resampling needs besides the volume: the method, the padding and the D13 mode.
#[derive(Clone, Copy, Debug)]
pub struct Resampler {
    pub method: Method,
    pub zpad: usize,
    pub trig: Trig,
}

/// The reusable buffers of resamplings on one grid.
#[derive(Default)]
pub struct Work {
    pad: Vec<f32>,
    scratch: Vec<f32>,
}

/// Sets when a resampling found no decomposition (AFNI's "can't compute shear
/// transformation"): the volume was left as it was.
pub static NO_SHEARS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Resampler {
    /// `vol` (a volume of `n` voxels of sizes `sizes`) transformed by `tf` into `out`.
    pub fn apply(
        &self,
        vol: &[f32],
        n: [usize; 3],
        sizes: [f32; 3],
        tf: &Transform,
        out: &mut [f32],
        work: &mut Work,
    ) {
        out.copy_from_slice(vol);
        if n.iter().any(|&d| d < 2) {
            return;
        }
        let p = plan(tf, sizes, self.trig);
        self.apply_plan(&p, n, out, work);
    }

    /// `vol` transformed in place by the plan `p`.
    pub fn apply_plan(&self, p: &Plan, n: [usize; 3], vol: &mut [f32], work: &mut Work) {
        let Some(shears) = &p.shears else {
            NO_SHEARS.store(true, std::sync::atomic::Ordering::Relaxed);
            return;
        };
        let z = self.zpad;
        let dims = [n[0] + 2 * z, n[1] + 2 * z, n[2] + 2 * z];
        let total = dims[0] * dims[1] * dims[2];
        let (buf, scratch) = (&mut work.pad, &mut work.scratch);
        if z > 0 {
            buf.clear();
            buf.resize(total, 0.0);
            for k in 0..n[2] {
                for j in 0..n[1] {
                    let src = &vol[n[0] * (j + n[1] * k)..][..n[0]];
                    let at = z + dims[0] * (j + z + dims[1] * (k + z));
                    buf[at..at + n[0]].copy_from_slice(src);
                }
            }
        } else {
            buf.clear();
            buf.extend_from_slice(vol);
        }
        if let Some((a, b)) = p.flip {
            flip(buf, dims, a, b);
        }
        for sh in shears {
            shear(buf, scratch, dims, sh, self.method, self.trig);
        }
        if z > 0 {
            for k in 0..n[2] {
                for j in 0..n[1] {
                    let at = z + dims[0] * (j + z + dims[1] * (k + z));
                    vol[n[0] * (j + n[1] * k)..][..n[0]].copy_from_slice(&buf[at..at + n[0]]);
                }
            }
        } else {
            vol.copy_from_slice(buf);
        }
    }
}

/// Flips the content along axes `a` and `b` about the grid centre.
fn flip(v: &mut [f32], dims: [usize; 3], a: usize, b: usize) {
    let [nx, ny, nz] = dims;
    let src = v.to_vec();
    let rev = |axis: usize, i: usize| {
        if axis == a || axis == b {
            dims[axis] - 1 - i
        } else {
            i
        }
    };
    v.par_chunks_mut(nx * ny)
        .enumerate()
        .for_each(|(k, plane)| {
            let sk = rev(2, k);
            for j in 0..ny {
                let sj = rev(1, j);
                for i in 0..nx {
                    plane[i + nx * j] = src[rev(0, i) + nx * (sj + ny * sk)];
                }
            }
        });
    let _ = nz;
}

/// The float32 coefficients of a shear and whether it is skipped (§5.5).
fn coefficients(sh: &Shear, dims: [usize; 3]) -> Option<(f32, f32, f32, [f32; 2])> {
    let (a, b, s) = (sh.a as f32, sh.b as f32, sh.s as f32);
    let others: Vec<usize> = (0..3).filter(|&x| x != sh.axis).collect();
    let centre = [0, 1].map(|m| 0.5f32 * (dims[others[m]] as f32 - 1.0));
    let st = (f64::from(a.abs()) * f64::from(centre[0])
        + f64::from(b.abs()) * f64::from(centre[1])
        + f64::from(s.abs())) as f32;
    if st < 1.0e-3 {
        return None;
    }
    Some((a, b, s, [centre[0], st]))
}

/// The per-thread buffers of a shear pass.
struct Lines {
    f: Vec<f32>,
    g: Vec<f32>,
    out: Vec<f32>,
    fourier: Option<FourierWork>,
}

impl Lines {
    fn new(n: usize, l: Option<usize>) -> Lines {
        Lines {
            f: vec![0.0; n],
            g: vec![0.0; n],
            out: vec![0.0; n],
            fourier: l.map(FourierWork::new),
        }
    }
}

/// Shifts the row pair `f` (shift `a1`) and `g` (shift `a2`; `None` for a lone row) in place.
fn shift_rows(
    lines: &mut Lines,
    method: Method,
    trig: Trig,
    two: bool,
    a1: f32,
    a2: f32,
    n: usize,
) {
    let Lines { f, g, out, fourier } = lines;
    if method == Method::Fourier {
        let w = fourier.as_mut().expect("a Fourier plan");
        let beyond = |a: f32| a < -(n as f32) || a > n as f32;
        if beyond(a1) && beyond(a2) {
            f.fill(0.0);
            g.fill(0.0);
            return;
        }
        shift::shift_fourier_pair(w, f, two.then_some(&mut g[..]), a1, a2, trig);
        return;
    }
    shift::shift_poly(method, f, a1, out);
    std::mem::swap(f, out);
    if two {
        shift::shift_poly(method, g, a2, out);
        std::mem::swap(g, out);
    }
}

/// Applies one shear to `v` (a padded volume of `dims`) in place.
fn shear(
    v: &mut Vec<f32>,
    scratch: &mut Vec<f32>,
    dims: [usize; 3],
    sh: &Shear,
    method: Method,
    trig: Trig,
) {
    let Some((a, b, s, [_, st])) = coefficients(sh, dims) else {
        return;
    };
    let axis = sh.axis;
    let others: Vec<usize> = (0..3).filter(|&x| x != axis).collect();
    let (u_axis, w_axis) = (others[0], others[1]);
    let cu = 0.5f32 * (dims[u_axis] as f32 - 1.0);
    let cw = 0.5f32 * (dims[w_axis] as f32 - 1.0);
    let n = dims[axis];
    let l = (method == Method::Fourier).then(|| shift::fourier_length(n, st));
    let [nx, ny, nz] = dims;
    let stride = [1, nx, nx * ny];
    let nu = dims[u_axis];
    // The shift of row (u, w): a·(u − cu) + b·(w − cw) + s in float32.
    let alpha = move |u: usize, w: usize| a * (u as f32 - cu) + b * (w as f32 - cw) + s;
    // The rows of one value of w, in pairs along u (§5.5).
    let rows_of = move |w: usize,
                        lines: &mut Lines,
                        read: &dyn Fn(usize, &mut [f32]),
                        write: &mut dyn FnMut(usize, &[f32])| {
        let mut prev_second = 0.0f32;
        let mut u = 0;
        while u < nu {
            let a0 = alpha(u, w);
            if u + 1 < nu {
                let a1 = a0 + a;
                read(u, &mut lines.f);
                read(u + 1, &mut lines.g);
                shift_rows(lines, method, trig, true, a0, a1, n);
                write(u, &lines.f);
                write(u + 1, &lines.g);
                prev_second = a1;
            } else {
                let second = if axis == 0 { a0 + a } else { prev_second };
                read(u, &mut lines.f);
                shift_rows(lines, method, trig, false, a0, second, n);
                write(u, &lines.f);
            }
            u += 2;
        }
    };
    match axis {
        // x- and y-shears: the rows of each z plane (w = k).
        0 | 1 => {
            let plane = nx * ny;
            v.par_chunks_mut(plane).enumerate().for_each_init(
                || Lines::new(n, l),
                |lines, (k, pl)| {
                    let cell = std::cell::RefCell::new(pl);
                    let read = |u: usize, row: &mut [f32]| {
                        let pl = cell.borrow();
                        if axis == 0 {
                            row.copy_from_slice(&pl[nx * u..nx * u + nx]);
                        } else {
                            for (j, r) in row.iter_mut().enumerate() {
                                *r = pl[u + nx * j];
                            }
                        }
                    };
                    let mut write = |u: usize, row: &[f32]| {
                        let mut pl = cell.borrow_mut();
                        if axis == 0 {
                            pl[nx * u..nx * u + nx].copy_from_slice(row);
                        } else {
                            for (j, &r) in row.iter().enumerate() {
                                pl[u + nx * j] = r;
                            }
                        }
                    };
                    rows_of(k, lines, &read, &mut write);
                },
            );
        }
        // z-shear: rows (i, j) along k; by y slabs into the scratch volume (k fastest), then
        // back by planes.
        _ => {
            scratch.clear();
            scratch.resize(v.len(), 0.0);
            let src: &[f32] = v;
            scratch.par_chunks_mut(nx * nz).enumerate().for_each_init(
                || Lines::new(n, l),
                |lines, (j, slab)| {
                    let cell = std::cell::RefCell::new(slab);
                    let read = |u: usize, row: &mut [f32]| {
                        for (k, r) in row.iter_mut().enumerate() {
                            *r = src[u + stride[1] * j + stride[2] * k];
                        }
                    };
                    let mut write = |u: usize, row: &[f32]| {
                        cell.borrow_mut()[nz * u..nz * u + nz].copy_from_slice(row);
                    };
                    rows_of(j, lines, &read, &mut write);
                },
            );
            let sc: &[f32] = scratch;
            v.par_chunks_mut(nx * ny).enumerate().for_each(|(k, pl)| {
                for j in 0..ny {
                    for i in 0..nx {
                        pl[i + nx * j] = sc[k + nz * (i + nx * j)];
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &M3, b: &M3, tol: f64) -> bool {
        (0..3).all(|i| (0..3).all(|j| (a[i][j] - b[i][j]).abs() < tol))
    }

    /// The shears composed reproduce the matrix (unit diagonals, f = 1).
    #[test]
    fn shears_compose_to_the_rotation() {
        let q = rot3([2, 0, 1], [0.1, -0.05, 0.07], Trig::Correct);
        let e = [1.5, -2.0, 0.25];
        let shears = decompose(&q, e, Trig::Correct).unwrap();
        // Compose the affine maps p -> S p + t, applied in order.
        let mut m: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut t = [0.0f64; 3];
        for sh in &shears {
            let others: Vec<usize> = (0..3).filter(|&x| x != sh.axis).collect();
            let mut s: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            s[sh.axis][others[0]] = sh.a;
            s[sh.axis][others[1]] = sh.b;
            m = mul(&s, &m);
            t = mul_vec(&s, t);
            t[sh.axis] += sh.s;
        }
        assert!(close(&m, &q, 1e-12), "{m:?} {q:?}");
        for i in 0..3 {
            assert!((t[i] - e[i]).abs() < 1e-12);
        }
        // One axis only: every order fails, the retry succeeds.
        let one = rot3([2, 0, 1], [0.0, 0.0, 0.01], Trig::Correct);
        assert!(best_order(&one, e).is_none());
        assert!(decompose(&one, e, Trig::Correct).is_some());
    }

    #[test]
    fn a_shift_moves_the_content() {
        let n = [12, 10, 8];
        let mut vol = vec![0f32; 12 * 10 * 8];
        vol[5 + 12 * (4 + 10 * 3)] = 1.0;
        let tf = Transform::Angles {
            theta: [0.0; 3],
            axes: [2, 0, 1],
            shift: [2.0, 0.0, 0.0],
        };
        let r = Resampler {
            method: Method::Heptic,
            zpad: 2,
            trig: Trig::Correct,
        };
        let mut out = vec![0f32; vol.len()];
        r.apply(&vol, n, [1.0; 3], &tf, &mut out, &mut Work::default());
        let at = 7 + 12 * (4 + 10 * 3);
        assert!((out[at] - 1.0).abs() < 1e-3, "{}", out[at]);
    }
}

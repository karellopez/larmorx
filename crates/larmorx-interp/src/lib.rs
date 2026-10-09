//! Image interpolators for larmorx (PLAN.md §5), ported from ITK v5.4.5.
//!
//! An interpolator evaluates a 3D image at a **continuous index** (voxel coordinates). Each one
//! reproduces the ITK class `antsApplyTransforms` uses for the same name, including its edge
//! handling and the order of its arithmetic:
//!
//! | [`Interpolation`] | ITK class |
//! |---|---|
//! | `Linear` | `LinearInterpolateImageFunction` |
//! | `NearestNeighbor` | `NearestNeighborInterpolateImageFunction` |
//! | `BSpline { order }` | `BSplineInterpolateImageFunction` (+ `BSplineDecompositionImageFilter`) |
//! | `Gaussian { sigma, alpha }` | `GaussianInterpolateImageFunction` |
//! | `MultiLabel { sigma, alpha }` | `LabelImageGaussianInterpolateImageFunction` |
//! | `GenericLabel` | `LabelImageGenericInterpolateImageFunction` (linear) |
//! | `WindowedSinc(window)` | `WindowedSincInterpolateImageFunction` (radius 3, zero outside) |
//!
//! Callers check [`is_inside`] first, as ITK's resampler does: interpolators assume the index
//! lies within half a voxel of the grid.
#![forbid(unsafe_code)]

mod vnl;

use std::collections::BTreeMap;

use larmorx_core::RealElement;
use rayon::prelude::*;

/// A 3D scalar image in Fortran order (`x` fastest), read as f64.
#[derive(Clone, Copy, Debug)]
pub struct Volume<'a, T> {
    pub data: &'a [T],
    pub size: [usize; 3],
}

impl<'a, T: RealElement> Volume<'a, T> {
    pub fn new(data: &'a [T], size: [usize; 3]) -> Self {
        assert_eq!(
            data.len(),
            size.iter().product::<usize>(),
            "volume size mismatch"
        );
        Volume { data, size }
    }

    #[inline]
    fn at(&self, i: usize, j: usize, k: usize) -> f64 {
        self.data[i + self.size[0] * (j + self.size[1] * k)].to_f64()
    }
}

/// Whether a continuous index lies within half a voxel of the grid (ITK's `IsInsideBuffer`).
pub fn is_inside(cidx: [f64; 3], size: [usize; 3]) -> bool {
    (0..3).all(|d| cidx[d] >= -0.5 && cidx[d] < size[d] as f64 - 0.5)
}

/// The window of a windowed-sinc interpolator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Cosine,
    Hamming,
    Welch,
    Lanczos,
    Blackman,
}

/// An interpolation method, with ITK's parameters.
#[derive(Clone, Debug, PartialEq)]
pub enum Interpolation {
    Linear,
    NearestNeighbor,
    /// Order 0 to 5 (ANTs' default is 3).
    BSpline {
        order: u32,
    },
    /// `sigma` in physical units (ANTs' default: the input spacing), `alpha` the cut-off in
    /// sigmas (ANTs' default: 1).
    Gaussian {
        sigma: [f64; 3],
        alpha: f64,
    },
    /// Label-wise Gaussian voting (ANTs' `MultiLabel`; default alpha 4).
    MultiLabel {
        sigma: [f64; 3],
        alpha: f64,
    },
    /// Label-wise linear interpolation (ANTs' `GenericLabel`).
    GenericLabel,
    WindowedSinc(Window),
}

/// The interpolator could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InterpError {
    #[error("B-spline order must be between 0 and 5, not {0}")]
    BSplineOrder(u32),
    #[error("Gaussian sigma and alpha must be positive")]
    GaussianParameters,
}

/// An interpolator prepared for one image.
pub struct Interpolator<'a, T> {
    volume: Volume<'a, T>,
    kind: Kind,
}

enum Kind {
    Linear,
    NearestNeighbor,
    BSpline { order: u32, coefficients: Vec<f64> },
    Gaussian(GaussianParams),
    MultiLabel(GaussianParams),
    GenericLabel,
    WindowedSinc(Window),
}

#[derive(Clone, Copy)]
struct GaussianParams {
    scaling: [f64; 3],
    cutoff: [f64; 3],
}

impl GaussianParams {
    fn new(sigma: [f64; 3], alpha: f64, spacing: [f64; 3]) -> Result<Self, InterpError> {
        if alpha <= 0.0 || sigma.iter().any(|&s| s <= 0.0) {
            return Err(InterpError::GaussianParameters);
        }
        Ok(GaussianParams {
            scaling: std::array::from_fn(|d| {
                1.0 / (std::f64::consts::SQRT_2 * sigma[d] / spacing[d])
            }),
            cutoff: std::array::from_fn(|d| sigma[d] * alpha / spacing[d]),
        })
    }
}

impl<'a, T: RealElement> Interpolator<'a, T> {
    /// Prepares `method` for `volume`; `spacing` is the image spacing (for the Gaussian ones).
    /// B-spline coefficients are computed here (in parallel on the current rayon pool).
    pub fn new(
        volume: Volume<'a, T>,
        method: &Interpolation,
        spacing: [f64; 3],
    ) -> Result<Self, InterpError> {
        let kind = match method {
            Interpolation::Linear => Kind::Linear,
            Interpolation::NearestNeighbor => Kind::NearestNeighbor,
            Interpolation::BSpline { order } => {
                if *order > 5 {
                    return Err(InterpError::BSplineOrder(*order));
                }
                Kind::BSpline {
                    order: *order,
                    coefficients: bspline::coefficients(&volume, *order),
                }
            }
            Interpolation::Gaussian { sigma, alpha } => {
                Kind::Gaussian(GaussianParams::new(*sigma, *alpha, spacing)?)
            }
            Interpolation::MultiLabel { sigma, alpha } => {
                Kind::MultiLabel(GaussianParams::new(*sigma, *alpha, spacing)?)
            }
            Interpolation::GenericLabel => Kind::GenericLabel,
            Interpolation::WindowedSinc(w) => Kind::WindowedSinc(*w),
        };
        Ok(Interpolator { volume, kind })
    }

    /// The value at a continuous index inside the image (see [`is_inside`]).
    pub fn evaluate(&self, cidx: [f64; 3]) -> f64 {
        match &self.kind {
            Kind::Linear => linear(&self.volume, cidx),
            Kind::NearestNeighbor => {
                let idx = cidx.map(|c| (c + 0.5).floor() as usize);
                self.volume.at(idx[0], idx[1], idx[2])
            }
            Kind::BSpline {
                order,
                coefficients,
            } => bspline::evaluate(coefficients, self.volume.size, *order, cidx),
            Kind::Gaussian(p) => gaussian(&self.volume, p, cidx),
            Kind::MultiLabel(p) => multi_label(&self.volume, p, cidx),
            Kind::GenericLabel => generic_label(&self.volume, cidx),
            Kind::WindowedSinc(w) => windowed_sinc(&self.volume, *w, cidx),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Linear

/// ITK's 3D linear interpolation: trilinear, combining x, then y, then z as `a + (b − a)·d`. An
/// axis is skipped when its fractional distance is ≤ 0 or its upper neighbour lies beyond the
/// last voxel (both happen within half a voxel of the edges).
fn linear<T: RealElement>(v: &Volume<'_, T>, cidx: [f64; 3]) -> f64 {
    let mut base = [0usize; 3];
    let mut dist = [0.0f64; 3];
    let mut active = [false; 3];
    for d in 0..3 {
        let b = cidx[d].floor().max(0.0);
        dist[d] = cidx[d] - b;
        base[d] = b as usize;
        active[d] = dist[d] > 0.0 && base[d] + 1 < v.size[d];
    }
    let step = |d: usize, on: usize| if active[d] { base[d] + on } else { base[d] };
    let corner = |x: usize, y: usize, z: usize| v.at(step(0, x), step(1, y), step(2, z));
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    // Reduce along x for each (y, z) corner that is needed, then along y, then along z.
    let ys: &[usize] = if active[1] { &[0, 1] } else { &[0] };
    let zs: &[usize] = if active[2] { &[0, 1] } else { &[0] };
    let mut plane = [[0.0f64; 2]; 2];
    for &z in zs {
        for &y in ys {
            let a = corner(0, y, z);
            plane[z][y] = if active[0] {
                lerp(a, corner(1, y, z), dist[0])
            } else {
                a
            };
        }
    }
    let mut line = [0.0f64; 2];
    for &z in zs {
        line[z] = if active[1] {
            lerp(plane[z][0], plane[z][1], dist[1])
        } else {
            plane[z][0]
        };
    }
    if active[2] {
        lerp(line[0], line[1], dist[2])
    } else {
        line[0]
    }
}

// ------------------------------------------------------------------------------------------------
// Gaussian and label-Gaussian

/// The voxels within the cut-off of `cidx` along one axis, and their Gaussian weights
/// (ITK's `ComputeInterpolationRegion` and `ComputeErrorFunctionArray`).
fn erf_weights(c: f64, size: usize, scaling: f64, cutoff: f64) -> (usize, Vec<f64>) {
    let begin = (c + 0.5 - cutoff).floor().max(0.0) as usize;
    let end = ((c + 0.5 + cutoff).ceil().max(0.0) as usize).min(size);
    let n = end.saturating_sub(begin);
    let mut t = (-0.5 - c + begin as f64) * scaling;
    let mut e_last = vnl::erf(t);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        t += scaling;
        let e_now = vnl::erf(t);
        out.push(e_now - e_last);
        e_last = e_now;
    }
    (begin, out)
}

fn gaussian_region(size: [usize; 3], p: &GaussianParams, cidx: [f64; 3]) -> [(usize, Vec<f64>); 3] {
    std::array::from_fn(|d| erf_weights(cidx[d], size[d], p.scaling[d], p.cutoff[d]))
}

fn gaussian<T: RealElement>(v: &Volume<'_, T>, p: &GaussianParams, cidx: [f64; 3]) -> f64 {
    let [(bx, wx), (by, wy), (bz, wz)] = gaussian_region(v.size, p, cidx);
    let (mut sum_me, mut sum_m) = (0.0f64, 0.0f64);
    for (k, &w2) in wz.iter().enumerate() {
        for (j, &w1) in wy.iter().enumerate() {
            for (i, &w0) in wx.iter().enumerate() {
                let w = w0 * w1 * w2;
                sum_me += v.at(bx + i, by + j, bz + k) * w;
                sum_m += w;
            }
        }
    }
    sum_me / sum_m
}

/// Label voting: the label whose summed Gaussian weight first exceeds the running maximum, in
/// scan order (x fastest), wins.
fn multi_label<T: RealElement>(v: &Volume<'_, T>, p: &GaussianParams, cidx: [f64; 3]) -> f64 {
    let [(bx, wx), (by, wy), (bz, wz)] = gaussian_region(v.size, p, cidx);
    let mut weights: Vec<(f64, f64)> = Vec::new();
    let (mut wmax, mut vmax) = (0.0f64, 0.0f64);
    for (k, &w2) in wz.iter().enumerate() {
        for (j, &w1) in wy.iter().enumerate() {
            for (i, &w0) in wx.iter().enumerate() {
                let w = w0 * w1 * w2;
                let value = v.at(bx + i, by + j, bz + k);
                // Labels compare by value, like ITK's std::map key.
                let total = match weights.iter_mut().find(|(k, _)| *k == value) {
                    Some((_, acc)) => {
                        *acc += w;
                        *acc
                    }
                    None => {
                        weights.push((value, w));
                        w
                    }
                };
                if total > wmax {
                    wmax = total;
                    vmax = value;
                }
            }
        }
    }
    vmax
}

// ------------------------------------------------------------------------------------------------
// Generic label

/// For each label (ascending), the linear interpolation of its indicator image; the label with
/// the strictly highest value wins, starting from label 0 at 0. Only labels present among the
/// neighbours can be non-zero, so only those are evaluated.
fn generic_label<T: RealElement>(v: &Volume<'_, T>, cidx: [f64; 3]) -> f64 {
    let base = cidx.map(|c| c.floor().max(0.0) as usize);
    let mut labels = BTreeMap::new();
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let (i, j, k) = (
                    (base[0] + dx).min(v.size[0] - 1),
                    (base[1] + dy).min(v.size[1] - 1),
                    (base[2] + dz).min(v.size[2] - 1),
                );
                let value = v.at(i, j, k);
                labels.insert(OrderedF64(value), ());
            }
        }
    }
    let (mut best_value, mut best_label) = (0.0f64, 0.0f64);
    for OrderedF64(label) in labels.into_keys() {
        let indicator = Volume {
            data: v.data,
            size: v.size,
        };
        let value = linear_indicator(&indicator, cidx, label);
        if value > best_value {
            best_value = value;
            best_label = label;
        }
    }
    best_label
}

/// [`linear`] applied to the indicator image `value == label`.
fn linear_indicator<T: RealElement>(v: &Volume<'_, T>, cidx: [f64; 3], label: f64) -> f64 {
    let mut base = [0usize; 3];
    let mut dist = [0.0f64; 3];
    let mut active = [false; 3];
    for d in 0..3 {
        let b = cidx[d].floor().max(0.0);
        dist[d] = cidx[d] - b;
        base[d] = b as usize;
        active[d] = dist[d] > 0.0 && base[d] + 1 < v.size[d];
    }
    let step = |d: usize, on: usize| if active[d] { base[d] + on } else { base[d] };
    let corner = |x: usize, y: usize, z: usize| {
        if v.at(step(0, x), step(1, y), step(2, z)) == label {
            1.0
        } else {
            0.0
        }
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let ys: &[usize] = if active[1] { &[0, 1] } else { &[0] };
    let zs: &[usize] = if active[2] { &[0, 1] } else { &[0] };
    let mut plane = [[0.0f64; 2]; 2];
    for &z in zs {
        for &y in ys {
            let a = corner(0, y, z);
            plane[z][y] = if active[0] {
                lerp(a, corner(1, y, z), dist[0])
            } else {
                a
            };
        }
    }
    let mut line = [0.0f64; 2];
    for &z in zs {
        line[z] = if active[1] {
            lerp(plane[z][0], plane[z][1], dist[1])
        } else {
            plane[z][0]
        };
    }
    if active[2] {
        lerp(line[0], line[1], dist[2])
    } else {
        line[0]
    }
}

/// Total order on f64 label values (bit-exact equality, numeric order).
#[derive(Clone, Copy, PartialEq)]
struct OrderedF64(f64);
impl Eq for OrderedF64 {}
impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrderedF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

// ------------------------------------------------------------------------------------------------
// Windowed sinc

const SINC_RADIUS: i64 = 3;

fn window(w: Window, a: f64) -> f64 {
    let r = SINC_RADIUS as f64;
    let pi = std::f64::consts::PI;
    match w {
        Window::Cosine => libm::cos(a * (pi / (2.0 * r))),
        Window::Hamming => 0.54 + 0.46 * libm::cos(a * (pi / r)),
        Window::Welch => 1.0 - a * (1.0 / (r * r)) * a,
        Window::Lanczos => {
            if a == 0.0 {
                1.0
            } else {
                let z = (pi / r) * a;
                libm::sin(z) / z
            }
        }
        Window::Blackman => {
            0.42 + 0.5 * libm::cos(a * (pi / r)) + 0.08 * libm::cos(a * (2.0 * pi / r))
        }
    }
}

fn sinc(x: f64) -> f64 {
    let px = std::f64::consts::PI * x;
    if x == 0.0 { 1.0 } else { libm::sin(px) / px }
}

/// ITK's windowed sinc with radius 3: 6 taps per axis (offsets −2..=3 around `floor(index)`),
/// a delta when the index falls on a voxel, zero outside the image, summed in neighbourhood
/// order (x fastest) with weights multiplied in axis order.
fn windowed_sinc<T: RealElement>(v: &Volume<'_, T>, w: Window, cidx: [f64; 3]) -> f64 {
    let taps = (2 * SINC_RADIUS) as usize;
    let mut base = [0i64; 3];
    let mut weights = [[0.0f64; 6]; 3];
    for d in 0..3 {
        let b = cidx[d].floor();
        let dist = cidx[d] - b;
        base[d] = b as i64;
        if dist == 0.0 {
            for (i, wt) in weights[d].iter_mut().enumerate() {
                *wt = if i as i64 == SINC_RADIUS - 1 {
                    1.0
                } else {
                    0.0
                };
            }
        } else {
            let mut x = dist + SINC_RADIUS as f64;
            for wt in weights[d].iter_mut().take(taps) {
                x -= 1.0;
                *wt = window(w, x) * sinc(x);
            }
        }
    }
    let pixel = |o: [i64; 3]| -> f64 {
        let idx: [i64; 3] = std::array::from_fn(|d| base[d] + o[d]);
        if (0..3).all(|d| idx[d] >= 0 && idx[d] < v.size[d] as i64) {
            v.at(idx[0] as usize, idx[1] as usize, idx[2] as usize)
        } else {
            0.0
        }
    };
    let mut sum = 0.0f64;
    for oz in (1 - SINC_RADIUS)..=SINC_RADIUS {
        for oy in (1 - SINC_RADIUS)..=SINC_RADIUS {
            for ox in (1 - SINC_RADIUS)..=SINC_RADIUS {
                let mut value = pixel([ox, oy, oz]);
                value *= weights[0][(ox + SINC_RADIUS - 1) as usize];
                value *= weights[1][(oy + SINC_RADIUS - 1) as usize];
                value *= weights[2][(oz + SINC_RADIUS - 1) as usize];
                sum += value;
            }
        }
    }
    sum
}

// ------------------------------------------------------------------------------------------------
// B-spline

// The arithmetic follows ITK's expression for expression (e.g. `z2n *= z2n * iz`), so the
// result rounds the same way.
#[allow(clippy::misrefactored_assign_op)]
mod bspline {
    use super::*;

    const TOLERANCE: f64 = 1e-10;

    fn poles(order: u32) -> Vec<f64> {
        match order {
            2 => vec![8f64.sqrt() - 3.0],
            3 => vec![3f64.sqrt() - 2.0],
            4 => vec![
                (664.0 - 438_976f64.sqrt()).sqrt() + 304f64.sqrt() - 19.0,
                (664.0 + 438_976f64.sqrt()).sqrt() - 304f64.sqrt() - 19.0,
            ],
            5 => vec![
                (135.0 / 2.0 - (17745.0f64 / 4.0).sqrt()).sqrt() + (105.0f64 / 4.0).sqrt()
                    - 13.0 / 2.0,
                (135.0 / 2.0 + (17745.0f64 / 4.0).sqrt()).sqrt()
                    - (105.0f64 / 4.0).sqrt()
                    - 13.0 / 2.0,
            ],
            _ => vec![],
        }
    }

    /// ITK's `DataToCoefficients1D` on one line (in place).
    fn line_to_coefficients(c: &mut [f64], poles: &[f64]) {
        let n = c.len();
        if n == 1 {
            return;
        }
        let mut c0 = 1.0;
        for &z in poles {
            c0 = c0 * (1.0 - z) * (1.0 - 1.0 / z);
        }
        for v in c.iter_mut() {
            *v *= c0;
        }
        for &z in poles {
            // Initial causal coefficient.
            let horizon = (libm::log(TOLERANCE) / libm::log(z.abs())).ceil() as usize;
            let mut zn = z;
            if horizon < n {
                let mut sum = c[0];
                for v in c.iter().take(horizon).skip(1) {
                    sum += zn * v;
                    zn *= z;
                }
                c[0] = sum;
            } else {
                let iz = 1.0 / z;
                let mut z2n = libm::pow(z, (n - 1) as f64);
                let mut sum = c[0] + z2n * c[n - 1];
                z2n *= z2n * iz;
                for v in c.iter().take(n - 1).skip(1) {
                    sum += (zn + z2n) * v;
                    zn *= z;
                    z2n *= iz;
                }
                c[0] = sum / (1.0 - zn * zn);
            }
            for i in 1..n {
                c[i] += z * c[i - 1];
            }
            // Initial anti-causal coefficient.
            c[n - 1] = (z / (z * z - 1.0)) * (z * c[n - 2] + c[n - 1]);
            for i in (0..n - 1).rev() {
                c[i] = z * (c[i + 1] - c[i]);
            }
        }
    }

    /// The B-spline coefficients of a volume (ITK's `BSplineDecompositionImageFilter`): the 1D
    /// filter along x, then y, then z, each line independently.
    pub(super) fn coefficients<T: RealElement>(v: &Volume<'_, T>, order: u32) -> Vec<f64> {
        let [nx, ny, nz] = v.size;
        let mut c: Vec<f64> = v.data.iter().map(|x| x.to_f64()).collect();
        let poles = poles(order);
        if poles.is_empty() {
            return c;
        }
        // x lines are contiguous.
        c.par_chunks_mut(nx)
            .for_each(|line| line_to_coefficients(line, &poles));
        // y lines: within each z plane, gather, filter, scatter.
        c.par_chunks_mut(nx * ny).for_each(|plane| {
            let mut line = vec![0.0; ny];
            for i in 0..nx {
                for (j, v) in line.iter_mut().enumerate() {
                    *v = plane[i + nx * j];
                }
                line_to_coefficients(&mut line, &poles);
                for (j, v) in line.iter().enumerate() {
                    plane[i + nx * j] = *v;
                }
            }
        });
        // z lines: across planes; process columns of the (x, y) plane in parallel.
        let plane = nx * ny;
        let columns: Vec<Vec<f64>> = (0..plane)
            .into_par_iter()
            .map(|xy| {
                let mut line: Vec<f64> = (0..nz).map(|k| c[xy + plane * k]).collect();
                line_to_coefficients(&mut line, &poles);
                line
            })
            .collect();
        for (xy, line) in columns.into_iter().enumerate() {
            for (k, v) in line.into_iter().enumerate() {
                c[xy + plane * k] = v;
            }
        }
        c
    }

    fn weights(order: u32, x: f64, first: i64) -> [f64; 6] {
        let mut w = [0.0f64; 6];
        match order {
            0 => w[0] = 1.0,
            1 => {
                let t = x - first as f64;
                w[1] = t;
                w[0] = 1.0 - t;
            }
            2 => {
                let t = x - (first + 1) as f64;
                w[1] = 0.75 - t * t;
                w[2] = 0.5 * (t - w[1] + 1.0);
                w[0] = 1.0 - w[1] - w[2];
            }
            3 => {
                let t = x - (first + 1) as f64;
                w[3] = (1.0 / 6.0) * t * t * t;
                w[0] = (1.0 / 6.0) + 0.5 * t * (t - 1.0) - w[3];
                w[2] = t + w[0] - 2.0 * w[3];
                w[1] = 1.0 - w[0] - w[2] - w[3];
            }
            4 => {
                let t = x - (first + 2) as f64;
                let t2 = t * t;
                let tt = (1.0 / 6.0) * t2;
                w[0] = 0.5 - t;
                w[0] *= w[0];
                w[0] *= (1.0 / 24.0) * w[0];
                let t0 = t * (tt - 11.0 / 24.0);
                let t1 = 19.0 / 96.0 + t2 * (0.25 - tt);
                w[1] = t1 + t0;
                w[3] = t1 - t0;
                w[4] = w[0] + t0 + 0.5 * t;
                w[2] = 1.0 - w[0] - w[1] - w[3] - w[4];
            }
            5 => {
                let mut t = x - (first + 2) as f64;
                let mut t2 = t * t;
                w[5] = (1.0 / 120.0) * t * t2 * t2;
                t2 -= t;
                let t4 = t2 * t2;
                t -= 0.5;
                let tt = t2 * (t2 - 3.0);
                w[0] = (1.0 / 24.0) * (1.0 / 5.0 + t2 + t4) - w[5];
                let mut t0 = (1.0 / 24.0) * (t2 * (t2 - 5.0) + 46.0 / 5.0);
                let mut t1 = (-1.0 / 12.0) * t * (tt + 4.0);
                w[2] = t0 + t1;
                w[3] = t0 - t1;
                t0 = (1.0 / 16.0) * (9.0 / 5.0 - tt);
                t1 = (1.0 / 24.0) * t * (t4 - t2 - 5.0);
                w[1] = t0 + t1;
                w[4] = t0 - t1;
            }
            _ => unreachable!("order checked when building"),
        }
        w
    }

    /// ITK's B-spline evaluation: support from `floor(float(x) + ½·even)`, weights per axis,
    /// mirror boundaries, then the sum over the support with x fastest.
    pub(super) fn evaluate(
        coefficients: &[f64],
        size: [usize; 3],
        order: u32,
        cidx: [f64; 3],
    ) -> f64 {
        let n = order as usize + 1;
        let half: f32 = if order & 1 == 1 { 0.0 } else { 0.5 };
        let mut index = [[0usize; 6]; 3];
        let mut w = [[0.0f64; 6]; 3];
        for d in 0..3 {
            let first = ((cidx[d] as f32) + half).floor() as i64 - i64::from(order / 2);
            w[d] = weights(order, cidx[d], first);
            let end = size[d] as i64 - 1;
            for (k, slot) in index[d].iter_mut().take(n).enumerate() {
                let mut i = first + k as i64;
                if size[d] == 1 {
                    i = 0;
                } else {
                    if i < 0 {
                        i = -i;
                    }
                    if i >= end {
                        i = end - (i - end);
                    }
                }
                *slot = i as usize;
            }
        }
        let (nx, ny) = (size[0], size[1]);
        let mut sum = 0.0f64;
        for kz in 0..n {
            for ky in 0..n {
                for kx in 0..n {
                    let weight = w[0][kx] * w[1][ky] * w[2][kz];
                    sum += weight
                        * coefficients[index[0][kx] + nx * (index[1][ky] + ny * index[2][kz])];
                }
            }
        }
        sum
    }
}

#[cfg(test)]
mod tests;

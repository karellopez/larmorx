// SPDX-License-Identifier: Apache-2.0
//! Atropos' adaptive smoothing: `BSplineScatteredDataPointSetToImageFilter` with several
//! fitting levels and point weights, and `BSplineControlPointImageFilter` (ITK v5.4.5,
//! `Modules/Filtering/ImageGrid`), as `GetSmoothIntensityImageFromPriorImage` runs them.
//!
//! The points are the voxels whose prior probability (or prior label indicator) is at least
//! 0.5 inside the mask, at their physical positions with an identity direction, valued by the
//! intensity and weighted by the probability. They lie on the image grid, so each level is fitted
//! with [`larmorx_image::bspline::fit::GridFitPlan`] (ITK's single-thread order). Between levels
//! the residuals are updated by evaluating the level's lattice at every point
//! (`CollapsePhiLattice`, the last axis first, the parametric coordinate as ITK computes it
//! there), the running sum `psi` is refined (per axis while the axis has levels left) and the
//! next level fitted; the result is `psi + phi`, evaluated on the image grid.

use larmorx_image::bspline::fit::{GridFit, GridFitPlan};
use larmorx_image::bspline::{BSPLINE_EPSILON, Kernel, Lattice, refined_lattice_coefficients};

/// A fitted control-point lattice (`GetPhiLattice`), kept per intensity image and class.
pub type ControlLattice = Lattice;

/// Why the fit failed (an ITK exception).
#[derive(Clone, Debug, PartialEq)]
pub struct SmoothingError(pub String);

/// The grid of the fit (the image's `m_Origin`, `m_Spacing`, `m_Size`) and its settings.
pub struct FitSettings<'a> {
    pub origin: &'a [f64],
    pub spacing: &'a [f64],
    pub size: &'a [usize],
    pub order: usize,
    pub levels: &'a [u32],
    pub control_points: &'a [u32],
}

/// The parametric coordinate `ThreadedGenerateDataForUpdatingResidualValues` gives a point:
/// `float(spans) · float(point − origin) / (float(size − 1) · spacing)`, the product in float,
/// the quotient in double, stored as float, pulled inside `[0, spans)` at the ends.
fn residual_u(
    spans: usize,
    point: f32,
    origin: f64,
    size: usize,
    spacing: f64,
) -> Result<f32, SmoothingError> {
    let s = spans as f32;
    let r = (f64::from(s) / (f64::from((size - 1) as f32) * spacing)) as f32;
    let eps = (f64::from(r) * spacing * f64::from(BSPLINE_EPSILON)) as f32;
    let num = s * ((f64::from(point) - origin) as f32);
    let mut u = (f64::from(num) / (f64::from((size - 1) as f32) * spacing)) as f32;
    if (u - s).abs() <= eps {
        u = s - eps;
    }
    if u < 0.0 && u.abs() <= eps {
        u = 0.0;
    }
    if u < 0.0 || u >= s {
        return Err(SmoothingError(format!(
            "The collapse point component {u} is outside the corresponding parametric domain of \
             [0, {spans})."
        )));
    }
    Ok(u)
}

/// The value of `lattice` (order `order`) at parametric coordinates `u`: `CollapsePhiLattice`
/// along the last axis first, sums in float from 0.
fn collapse_at(lattice: &Lattice, kernel: &Kernel, u: &[f32]) -> f32 {
    let d = lattice.size.len();
    let order = kernel.order();
    let mut current = lattice.data.clone();
    let mut size = lattice.size.clone();
    for axis in (0..d).rev() {
        let first = u[axis] as u32;
        let half = 0.5 * f64::from((order as u32).wrapping_sub(1) as f32);
        let inner: usize = size[..axis].iter().product();
        let mut out = vec![0.0f32; inner];
        for k in 0..=order as u32 {
            let index = first + k;
            let v = (f64::from(u[axis] - index as f32) + half) as f32;
            let w = kernel.evaluate(f64::from(v)) as f32;
            let base = index as usize * inner;
            for (o, &x) in out.iter_mut().zip(&current[base..base + inner]) {
                *o += x * w;
            }
        }
        current = out;
        size.truncate(axis);
    }
    current[0]
}

/// `RefineControlPointLattice` of the scattered-data filter at `level`: axes whose number of
/// levels exceeds `level` are doubled (`2n − order`), the others keep their control points
/// (and ITK still uses the refinement coefficients on them).
fn refine(
    psi: &Lattice,
    order: usize,
    level: u32,
    levels: &[u32],
) -> Result<Lattice, SmoothingError> {
    let d = psi.size.len();
    let coefficients = refined_lattice_coefficients(order)
        .ok_or_else(|| SmoothingError("the refinement SVD did not converge".into()))?;
    let n = order + 1;
    let new_size: Vec<usize> = (0..d)
        .map(|a| {
            if level < levels[a] {
                2 * psi.size[a] - order
            } else {
                psi.size[a]
            }
        })
        .collect();
    let mut refined = Lattice::zeros(&new_size);
    let in_strides = strides(&psi.size);
    let out_strides = strides(&new_size);
    let n_terms = n.pow(d as u32);
    let total: usize = new_size.iter().product();
    let mut idx = vec![0usize; d];
    let mut idx_psi = vec![0usize; d];
    let mut off = vec![0usize; d];
    let mut off_psi = vec![0usize; d];
    let mut linear = 0usize;
    while linear < total {
        let mut rest = linear;
        for a in 0..d {
            idx[a] = rest % new_size[a];
            rest /= new_size[a];
        }
        for a in 0..d {
            idx_psi[a] = if level < levels[a] {
                (0.5 * idx[a] as f64) as usize
            } else {
                idx[a]
            };
        }
        for i in 0..(1usize << d) {
            let mut r = i;
            for o in off.iter_mut() {
                *o = r % 2;
                r /= 2;
            }
            if (0..d).any(|a| idx[a] + off[a] >= new_size[a]) {
                continue;
            }
            let mut sum = 0.0f32;
            for j in 0..n_terms {
                let mut r = j;
                for o in off_psi.iter_mut() {
                    *o = r % n;
                    r /= n;
                }
                if (0..d).any(|a| idx_psi[a] + off_psi[a] >= psi.size[a]) {
                    continue;
                }
                let mut coeff = 1.0f32;
                for a in 0..d {
                    coeff *= coefficients[off[a] * n + off_psi[a]];
                }
                let mut at = 0;
                for a in 0..d {
                    at += (idx_psi[a] + off_psi[a]) * in_strides[a];
                }
                sum += psi.data[at] * coeff;
            }
            let mut at = 0;
            for a in 0..d {
                at += (idx[a] + off[a]) * out_strides[a];
            }
            refined.data[at] = sum;
        }
        // The next index whose components are all even.
        loop {
            linear += 1;
            if linear >= total {
                break;
            }
            let mut rest = linear;
            let mut even = true;
            for a in 0..d {
                if !(rest % new_size[a]).is_multiple_of(2) {
                    even = false;
                }
                rest /= new_size[a];
            }
            if even {
                break;
            }
        }
    }
    Ok(refined)
}

fn strides(size: &[usize]) -> Vec<usize> {
    let mut s = vec![1usize; size.len()];
    for a in 1..size.len() {
        s[a] = s[a - 1] * size[a - 1];
    }
    s
}

/// The multi-level fit of `values` at the grid voxels `voxels` (increasing linear indices),
/// weighted by `weights`: the final control-point lattice (`GetPhiLattice`).
pub fn fit(
    settings: &FitSettings<'_>,
    voxels: &[usize],
    values: &[f32],
    weights: &[f32],
    n_threads: usize,
) -> Result<Lattice, SmoothingError> {
    let d = settings.size.len();
    let order = settings.order;
    if order == 0 {
        return Err(SmoothingError(
            "The spline order in each dimension must be greater than 0".into(),
        ));
    }
    let mut current: Vec<usize> = settings
        .control_points
        .iter()
        .map(|&c| c as usize)
        .collect();
    if current.iter().any(|&c| c < order + 1) {
        return Err(SmoothingError(
            "The number of control points must be greater than the spline order.".into(),
        ));
    }
    // The points' coordinates per axis: the physical position with an identity direction,
    // as float.
    let axis_points: Vec<Vec<f32>> = (0..d)
        .map(|a| {
            (0..settings.size[a])
                .map(|i| (settings.origin[a] + settings.spacing[a] * i as f64) as f32)
                .collect()
        })
        .collect();
    let mut strides_img = vec![1usize; d];
    for a in 1..d {
        strides_img[a] = strides_img[a - 1] * settings.size[a - 1];
    }
    let coords: Vec<Vec<usize>> = voxels
        .iter()
        .map(|&v| {
            (0..d)
                .map(|a| (v / strides_img[a]) % settings.size[a])
                .collect()
        })
        .collect();
    let max_levels = settings.levels.iter().copied().max().unwrap_or(1).max(1);
    let kernel = Kernel::new(order);
    let fit_level = |current: &[usize], residuals: &[f32]| -> Result<Lattice, SmoothingError> {
        let grid = GridFit {
            axis_points: &axis_points,
            origin: settings.origin,
            spacing: settings.spacing,
            size: settings.size,
            control_points: current,
            order,
            double_axes: 0,
        };
        let plan = GridFitPlan::new(&grid, settings.size, voxels, Some(weights), n_threads)
            .map_err(|e| SmoothingError(e.to_string()))?;
        plan.fit(residuals, n_threads)
            .map_err(|e| SmoothingError(e.to_string()))
    };
    let mut residuals = values.to_vec();
    let mut phi = fit_level(&current, &residuals)?;
    if max_levels > 1 {
        let mut psi = Lattice::zeros(&phi.size);
        for level in 1..max_levels {
            // Residuals minus the level's fit at every point.
            let spans: Vec<usize> = phi.size.iter().map(|&s| s - order).collect();
            let mut u = vec![0.0f32; d];
            for (p, c) in coords.iter().enumerate() {
                for a in 0..d {
                    u[a] = residual_u(
                        spans[a],
                        axis_points[a][c[a]],
                        settings.origin[a],
                        settings.size[a],
                        settings.spacing[a],
                    )?;
                }
                residuals[p] -= collapse_at(&phi, &kernel, &u);
            }
            for (s, &p) in psi.data.iter_mut().zip(&phi.data) {
                *s = p + *s;
            }
            psi = refine(&psi, order, level, settings.levels)?;
            for a in 0..d {
                if level < settings.levels[a] {
                    current[a] = 2 * current[a] - order;
                }
            }
            phi = fit_level(&current, &residuals)?;
        }
        for (s, &p) in psi.data.iter_mut().zip(&phi.data) {
            *s = p + *s;
        }
        phi = psi;
    }
    Ok(phi)
}

/// The fitted lattice evaluated on the image grid (`BSplineControlPointImageFilter`, and the
/// scattered-data filter's own reconstruction, which is the same).
pub fn reconstruct(
    lattice: &Lattice,
    order: usize,
    size: &[usize],
    spacing: &[f64],
    n_threads: usize,
) -> Result<Vec<f32>, SmoothingError> {
    larmorx_image::bspline::evaluate(lattice, order, size, spacing, n_threads)
        .map_err(|e| SmoothingError(e.to_string()))
}

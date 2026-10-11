// SPDX-License-Identifier: Apache-2.0
//! `BSplineScatteredDataPointSetToImageFilter` with one fitting level
//! (`ThreadedGenerateDataForFitting` and `AfterThreadedGenerateData`, ITK v5.4.5), for points
//! that lie on an image grid, as N4 fits its residual bias field.
//!
//! **The algorithm** (Lee, Wolberg and Shin, *Scattered data interpolation with multilevel
//! B-splines*, 1997, with weights): each point `n` with value `v`, weight `w` and parametric
//! coordinate `p` adds, to each of the `(order + 1)^D` control points whose B-spline is
//! non-zero at `p`, with `B` the product of the kernel weights and `Σ B²` over all of them:
//! `ω += w·B²` and `δ += v · B³·w / Σ B²`. The control point's value is `δ/ω` (0 where `ω` is
//! almost 0, or the quotient is not finite).
//!
//! **Arithmetic as ITK's.** Parameters are `f32`: `r = spans / ((size − 1)·spacing)`,
//! `p = (point − origin)·r` (the point in `f32`, as ITK's `PointSet` stores it), pulled inside
//! `[0, spans)` at the ends; kernel weights in `f64` multiplied into an `f32` product, axis 0
//! first; sums in `f32`.
//!
//! **Threads.** ITK splits the points into one block per work unit, accumulates each block
//! into its own lattices and adds them up afterwards, so its result depends on the number of
//! threads (`docs/findings/ants-n4.md`). larmorx reproduces ITK's **single-thread** order on
//! any number of threads: every control point receives its contributions in point order. It
//! computes `Σ B²` per point in parallel, then splits the work by control points (the last
//! one or two axes of the lattice), each task visiting, in order, the points that reach its
//! control points.

#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use super::{BSPLINE_EPSILON, Kernel, Lattice};
use crate::FilterError;
use crate::itk_math::almost_equal_f32;
use crate::volume::strides;

/// The domain and lattice of a fit (the filter's `m_Origin`, `m_Spacing`, `m_Size`,
/// `m_NumberOfControlPoints` and spline order), and the coordinates of the grid its points
/// lie on.
#[derive(Clone, Debug)]
pub struct GridFit<'a> {
    /// Per axis: the physical coordinate of each grid coordinate, as `f32` (ITK's point
    /// coordinates).
    pub axis_points: &'a [Vec<f32>],
    pub origin: &'a [f64],
    pub spacing: &'a [f64],
    pub size: &'a [usize],
    pub control_points: &'a [usize],
    pub order: usize,
}

/// Per axis and grid coordinate: the first control point and the `order + 1` kernel weights.
struct AxisTable {
    first: Vec<u32>,
    /// `(order + 1)` weights per coordinate.
    weights: Vec<f64>,
}

fn axis_table(
    points: &[f32],
    origin: f64,
    spacing: f64,
    size: usize,
    spans: usize,
    kernel: &Kernel,
    axis: usize,
) -> Result<AxisTable, FilterError> {
    let order = kernel.order();
    let r = (f64::from(spans as f32) / (f64::from((size - 1) as f32) * spacing)) as f32;
    let eps = (f64::from(r) * spacing * f64::from(BSPLINE_EPSILON)) as f32;
    let s = spans as f32;
    let half = 0.5 * f64::from((order as u32).wrapping_sub(1) as f32);
    let mut first = Vec::with_capacity(points.len());
    let mut weights = Vec::with_capacity(points.len() * (order + 1));
    for &pt in points {
        let mut p = ((f64::from(pt) - origin) * f64::from(r)) as f32;
        if (p - s).abs() <= eps {
            p = s - eps;
        }
        if p < 0.0 && p.abs() <= eps {
            p = 0.0;
        }
        if p < 0.0 || p >= s || p.is_nan() {
            return Err(FilterError::invalid(format!(
                "The reparameterized point component {p} is outside the corresponding \
                 parametric domain of [0, {spans}) (axis {axis})"
            )));
        }
        let base = p as u32;
        first.push(base);
        let frac = p - base as f32;
        for j in 0..=order {
            let u = (f64::from(frac - j as f32) + half) as f32;
            weights.push(kernel.evaluate(f64::from(u)));
        }
    }
    Ok(AxisTable { first, weights })
}

/// The B-spline fit of `values` at the grid voxels `voxels` (linear indices into a grid of
/// `grid_size`, Fortran order, increasing), each with its weight (`None`: 1). Returns the
/// control-point lattice (`phi`).
pub fn fit_on_grid(
    fit: &GridFit<'_>,
    grid_size: &[usize],
    voxels: &[usize],
    values: &[f32],
    weights: Option<&[f32]>,
    n_threads: usize,
) -> Result<Lattice, FilterError> {
    let d = grid_size.len();
    let order = fit.order;
    if order == 0 {
        return Err(FilterError::invalid(
            "The spline order in each dimension must be greater than 0",
        ));
    }
    if fit.axis_points.len() != d
        || fit.origin.len() != d
        || fit.spacing.len() != d
        || fit.size.len() != d
        || fit.control_points.len() != d
    {
        return Err(FilterError::invalid("the fit's dimensions differ"));
    }
    if values.len() != voxels.len() || weights.is_some_and(|w| w.len() != voxels.len()) {
        return Err(FilterError::invalid(
            "The number of weight points and input points must be equal.",
        ));
    }
    if fit.control_points.iter().any(|&c| c < order + 1) {
        return Err(FilterError::invalid(
            "The number of control points must be greater than the spline order.",
        ));
    }
    if fit.size.iter().any(|&s| s < 2) {
        return Err(FilterError::invalid(
            "B-spline fitting needs at least 2 voxels along every axis",
        ));
    }
    let kernel = Kernel::new(order);
    let n = order + 1;
    let mut tables = Vec::with_capacity(d);
    for a in 0..d {
        if fit.axis_points[a].len() != grid_size[a] {
            return Err(FilterError::invalid(
                "the grid coordinates have the wrong length",
            ));
        }
        tables.push(axis_table(
            &fit.axis_points[a],
            fit.origin[a],
            fit.spacing[a],
            fit.size[a],
            fit.control_points[a] - order,
            &kernel,
            a,
        )?);
    }
    let grid_strides = strides(grid_size);
    let coordinate = |voxel: usize, a: usize| (voxel / grid_strides[a]) % grid_size[a];
    let neighbours = n.pow(d as u32);

    // The B-spline product for neighbour digits `j` (axis 0 first) at grid coordinates `x`.
    let product = |x: &[usize], j: &[usize]| -> f32 {
        let mut b = 1.0f32;
        for a in 0..d {
            b = (f64::from(b) * tables[a].weights[x[a] * n + j[a]]) as f32;
        }
        b
    };

    let (omega, delta) = larmorx_core::parallel::with_threads(n_threads, || {
        // Pass 1: Σ B² per point, over its neighbours in Fortran order.
        let w2: Vec<f32> = voxels
            .par_iter()
            .with_min_len(1024)
            .map(|&voxel| {
                let x: Vec<usize> = (0..d).map(|a| coordinate(voxel, a)).collect();
                let mut j = vec![0usize; d];
                let mut sum = 0.0f32;
                for m in 0..neighbours {
                    let mut rest = m;
                    for ja in j.iter_mut() {
                        *ja = rest % n;
                        rest /= n;
                    }
                    let b = product(&x, &j);
                    sum += b * b;
                }
                sum
            })
            .collect();

        // Pass 2: each task owns the control points with given indices along the last one or
        // two axes (a contiguous block, those axes being the slowest) and visits, in point
        // order, the points that reach them.
        let cp = fit.control_points;
        let n_fixed = d.min(2);
        let fixed: Vec<usize> = (d - n_fixed..d).collect();
        let free: usize = cp[..d - n_fixed].iter().product();
        let n_tasks: usize = fixed.iter().map(|&a| cp[a]).product();
        let last = d - 1;
        let base_last: Vec<u32> = voxels
            .iter()
            .map(|&v| tables[last].first[coordinate(v, last)])
            .collect();
        let cp_strides = strides(cp);
        let blocks: Vec<(Vec<f32>, Vec<f32>)> = (0..n_tasks)
            .into_par_iter()
            .map(|task| {
                let mut c_fixed = [0usize; 2];
                let mut rest = task;
                for (k, &a) in fixed.iter().enumerate() {
                    c_fixed[k] = rest % cp[a];
                    rest /= cp[a];
                }
                let c_last = c_fixed[n_fixed - 1] as u32;
                // Points whose first control point along the last axis is in
                // [c_last − order, c_last] (it is non-decreasing in voxel order).
                let lo = base_last.partition_point(|&b| b + (order as u32) < c_last);
                let hi = base_last.partition_point(|&b| b <= c_last);
                let mut omega = vec![0.0f32; free];
                let mut delta = vec![0.0f32; free];
                let mut x = vec![0usize; d];
                let mut j = vec![0usize; d];
                let free_neighbours = n.pow((d - n_fixed) as u32);
                for idx in lo..hi {
                    let voxel = voxels[idx];
                    for (a, xa) in x.iter_mut().enumerate() {
                        *xa = coordinate(voxel, a);
                    }
                    let mut reaches = true;
                    for (k, &a) in fixed.iter().enumerate() {
                        let b = tables[a].first[x[a]] as usize;
                        if c_fixed[k] < b || c_fixed[k] > b + order {
                            reaches = false;
                            break;
                        }
                        j[a] = c_fixed[k] - b;
                    }
                    if !reaches {
                        continue;
                    }
                    let wc = weights.map_or(1.0, |w| w[idx]);
                    let value = values[idx];
                    for m in 0..free_neighbours {
                        let mut rest = m;
                        let mut cell = 0;
                        for a in 0..d - n_fixed {
                            j[a] = rest % n;
                            rest /= n;
                            cell += (tables[a].first[x[a]] as usize + j[a]) * cp_strides[a];
                        }
                        let t = product(&x, &j);
                        omega[cell] += wc * t * t;
                        delta[cell] += value * (t * t * t * wc / w2[idx]);
                    }
                }
                (omega, delta)
            })
            .collect();
        let total: usize = cp.iter().product();
        let mut omega = vec![0.0f32; total];
        let mut delta = vec![0.0f32; total];
        for (task, (o, dl)) in blocks.into_iter().enumerate() {
            let start = task * free;
            omega[start..start + free].copy_from_slice(&o);
            delta[start..start + free].copy_from_slice(&dl);
        }
        (omega, delta)
    })?;

    let data = omega
        .iter()
        .zip(&delta)
        .map(|(&o, &dl)| {
            if almost_equal_f32(o, 0.0) {
                0.0
            } else {
                let p = dl / o;
                if p.is_finite() { p } else { 0.0 }
            }
        })
        .collect();
    Ok(Lattice {
        size: fit.control_points.to_vec(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ITK's single-thread loop, written plainly: the reference for the parallel version.
    fn sequential(
        fit: &GridFit<'_>,
        grid_size: &[usize],
        voxels: &[usize],
        values: &[f32],
        weights: &[f32],
    ) -> Vec<f32> {
        let d = grid_size.len();
        let order = fit.order;
        let n = order + 1;
        let kernel = Kernel::new(order);
        let tables: Vec<AxisTable> = (0..d)
            .map(|a| {
                axis_table(
                    &fit.axis_points[a],
                    fit.origin[a],
                    fit.spacing[a],
                    fit.size[a],
                    fit.control_points[a] - order,
                    &kernel,
                    a,
                )
                .unwrap()
            })
            .collect();
        let gs = strides(grid_size);
        let cs = strides(fit.control_points);
        let total: usize = fit.control_points.iter().product();
        let (mut omega, mut delta) = (vec![0.0f32; total], vec![0.0f32; total]);
        for (i, &v) in voxels.iter().enumerate() {
            let x: Vec<usize> = (0..d).map(|a| (v / gs[a]) % grid_size[a]).collect();
            let mut bs = Vec::new();
            let mut w2 = 0.0f32;
            for m in 0..n.pow(d as u32) {
                let mut rest = m;
                let mut b = 1.0f32;
                let mut cell = 0;
                for a in 0..d {
                    let j = rest % n;
                    rest /= n;
                    b = (f64::from(b) * tables[a].weights[x[a] * n + j]) as f32;
                    cell += (tables[a].first[x[a]] as usize + j) * cs[a];
                }
                bs.push((cell, b));
                w2 += b * b;
            }
            for (cell, t) in bs {
                omega[cell] += weights[i] * t * t;
                delta[cell] += values[i] * (t * t * t * weights[i] / w2);
            }
        }
        omega
            .iter()
            .zip(&delta)
            .map(|(&o, &dl)| {
                if almost_equal_f32(o, 0.0) {
                    0.0
                } else {
                    let p = dl / o;
                    if p.is_finite() { p } else { 0.0 }
                }
            })
            .collect()
    }

    #[test]
    fn parallel_fit_equals_the_sequential_order() {
        let grid_size = [11usize, 9, 7];
        let spacing = [1.5f64, 2.0, 2.5];
        let origin = [-3.0f64, 4.0, 1.0];
        let axis_points: Vec<Vec<f32>> = (0..3)
            .map(|a| {
                (0..grid_size[a])
                    .map(|x| (f64::from(origin[a] as f32) + spacing[a] * x as f64) as f32)
                    .collect()
            })
            .collect();
        let cp = [6usize, 5, 4];
        let fit = GridFit {
            axis_points: &axis_points,
            origin: &origin,
            spacing: &spacing,
            size: &grid_size,
            control_points: &cp,
            order: 3,
        };
        let voxels: Vec<usize> = (0..693).filter(|v| v % 3 != 1).collect();
        let values: Vec<f32> = voxels
            .iter()
            .map(|&v| ((v * 37) % 101) as f32 * 0.01)
            .collect();
        let weights: Vec<f32> = voxels
            .iter()
            .map(|&v| 0.25 + (v % 4) as f32 * 0.25)
            .collect();
        let expected = sequential(&fit, &grid_size, &voxels, &values, &weights);
        for threads in [1, 2, 5] {
            let got =
                fit_on_grid(&fit, &grid_size, &voxels, &values, Some(&weights), threads).unwrap();
            assert_eq!(got.data, expected, "{threads} threads");
        }
    }

    #[test]
    fn points_outside_the_domain_are_errors() {
        let axis_points = vec![vec![0.0f32, 1.0, 2.0, 30.0]];
        let fit = GridFit {
            axis_points: &axis_points,
            origin: &[0.0],
            spacing: &[1.0],
            size: &[4],
            control_points: &[4],
            order: 3,
        };
        assert!(fit_on_grid(&fit, &[4], &[0, 1, 2, 3], &[1.0; 4], None, 1).is_err());
    }
}

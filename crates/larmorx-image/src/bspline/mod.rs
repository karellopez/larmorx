// SPDX-License-Identifier: Apache-2.0
//! ITK's B-spline control-point lattices: fitting scattered data, evaluating a lattice on an
//! image grid, and refining it (ITK v5.4.5, `Modules/Filtering/ImageGrid`):
//!
//! - [`fit`]: `BSplineScatteredDataPointSetToImageFilter` with one fitting level (Lee,
//!   Wolberg and Shin's B-spline approximation, with per-point weights), as N4 runs it. Its
//!   accumulation runs in ITK's single-thread order on any number of threads (see the
//!   module).
//! - [`evaluate`]: `BSplineControlPointImageFilter`'s reconstruction of a lattice on a grid of
//!   `size` voxels. ITK maps output index `i` to the parametric coordinate
//!   `spans · i / (size − 1)`, so the reconstruction depends on the sizes only, not on the
//!   grid's origin or spacing (except for the end-point tolerance).
//! - [`refine`]: `BSplineControlPointImageFilter::RefineControlPointLattice`, which doubles
//!   the lattice's resolution with [`kernel::refined_lattice_coefficients`].
//!
//! Lattices hold one `f32` per control point (`itk::Vector<float, 1>` in N4), in Fortran
//! order. The B-spline parameters are `f32` (`RealType` of both filters) and the kernels
//! `f64`, rounded to `f32` where ITK stores them, as in the C++ code.

pub mod fit;
pub mod kernel;

use rayon::prelude::*;

use crate::FilterError;
use crate::volume::strides;
pub use kernel::{Kernel, refined_lattice_coefficients};

/// `m_BSplineEpsilon` of both filters.
pub const BSPLINE_EPSILON: f32 = 1e-3;

/// A control-point lattice: one value per control point, Fortran order.
#[derive(Clone, Debug, PartialEq)]
pub struct Lattice {
    pub size: Vec<usize>,
    pub data: Vec<f32>,
}

impl Lattice {
    /// A lattice of zeros.
    pub fn zeros(size: &[usize]) -> Self {
        Lattice {
            size: size.to_vec(),
            data: vec![0.0; size.iter().product()],
        }
    }

    pub fn dim(&self) -> usize {
        self.size.len()
    }
}

/// The end-point tolerance `epsilon[i] = r · spacing · m_BSplineEpsilon` with
/// `r = spans / ((size − 1) · spacing)`, stored in float as ITK does.
fn epsilon(spans: usize, size: usize, spacing: f64) -> f32 {
    let r = (f64::from(spans as f32) / (f64::from((size - 1) as f32) * spacing)) as f32;
    (f64::from(r) * spacing * f64::from(BSPLINE_EPSILON)) as f32
}

/// The parametric coordinate ITK's reconstruction gives index `i` of an axis of `size`
/// voxels: `spans · i / (size − 1)` in float, pulled inside `[0, spans)` at the ends.
fn reconstruction_u(
    spans: usize,
    i: usize,
    size: usize,
    eps: f32,
    axis: usize,
) -> Result<f32, FilterError> {
    let s = spans as f32;
    let mut u = s * i as f32 / (size - 1) as f32;
    if (u - s).abs() <= eps {
        u = s - eps;
    }
    if u < 0.0 && u.abs() <= eps {
        u = 0.0;
    }
    if u < 0.0 || u >= s {
        return Err(FilterError::invalid(format!(
            "The collapse point component {u} is outside the corresponding parametric domain \
             of [0, {spans}) (axis {axis})"
        )));
    }
    Ok(u)
}

/// The `order + 1` kernel weights (`f32`) and the first control point for a parametric
/// coordinate `u`, as `CollapsePhiLattice` computes them: control point `floor(u) + k` gets
/// `B(u − (floor(u) + k) + (order − 1)/2)`.
pub(crate) fn collapse_weights(kernel: &Kernel, u: f32) -> (usize, Vec<f32>) {
    let order = kernel.order();
    let first = u as u32; // static_cast<unsigned int>(u)
    let half = 0.5 * f64::from((order as u32).wrapping_sub(1) as f32);
    let weights = (0..=order as u32)
        .map(|k| {
            let index = first + k;
            let v = (f64::from(u - index as f32) + half) as f32;
            kernel.evaluate(f64::from(v)) as f32
        })
        .collect();
    (first as usize, weights)
}

/// `CollapsePhiLattice` along `axis`: `out(…) = Σ_k lattice(…, first + k, …) · w[k]`, summed
/// in float from 0 in `k` order. `lattice` has `size` (axes after `axis` already collapsed to
/// 1); the result has the axes before `axis`.
fn collapse(lattice: &[f32], size: &[usize], axis: usize, first: usize, w: &[f32]) -> Vec<f32> {
    let inner: usize = size[..axis].iter().product();
    let stride = inner;
    let mut out = vec![0.0f32; inner];
    for (k, &wk) in w.iter().enumerate() {
        let base = (first + k) * stride;
        for (o, &v) in out.iter_mut().zip(&lattice[base..base + inner]) {
            *o += v * wk;
        }
    }
    out
}

/// `BSplineControlPointImageFilter` (`DynamicThreadedGenerateData`): the B-spline of `order`
/// with control points `lattice`, evaluated on a grid of `size` voxels with `spacing` (the
/// spacing only enters the end-point tolerance). Returns the values in Fortran order.
///
/// Each voxel collapses the lattice along the last axis first, down to the first, in float;
/// values do not depend on how the grid is split between threads.
pub fn evaluate(
    lattice: &Lattice,
    order: usize,
    size: &[usize],
    spacing: &[f64],
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = lattice.dim();
    if size.len() != d || spacing.len() != d {
        return Err(FilterError::invalid(
            "the grid and the lattice differ in dimension",
        ));
    }
    if order == 0 {
        return Err(FilterError::invalid(
            "The spline order in each dimension must be greater than 0",
        ));
    }
    if size.iter().any(|&n| n < 2) {
        return Err(FilterError::invalid(
            "B-spline reconstruction needs at least 2 voxels along every axis",
        ));
    }
    if lattice.size.iter().any(|&n| n <= order) {
        return Err(FilterError::invalid(
            "The number of control points must be greater than the spline order.",
        ));
    }
    let kernel = Kernel::new(order);
    // Per axis and output coordinate: the first control point and the weights.
    let mut tables: Vec<Vec<(usize, Vec<f32>)>> = Vec::with_capacity(d);
    for a in 0..d {
        let spans = lattice.size[a] - order;
        let eps = epsilon(spans, size[a], spacing[a]);
        let mut axis = Vec::with_capacity(size[a]);
        for i in 0..size[a] {
            let u = reconstruction_u(spans, i, size[a], eps, a)?;
            axis.push(collapse_weights(&kernel, u));
        }
        tables.push(axis);
    }
    // Work unit: one plane of axes 0 and 1 (one row in 2D). The axes above 1 are collapsed
    // once per plane, axis 1 once per row, axis 0 per voxel: the same arithmetic as ITK's
    // cached collapses.
    let n0 = size[0];
    let plane = if d >= 3 { n0 * size[1] } else { n0 };
    let mut out = vec![0.0f32; size.iter().product()];
    let lattice_size = &lattice.size;
    let unit = |p: usize, dst: &mut [f32]| {
        let mut coords = vec![0usize; d];
        let mut rest = p;
        for a in (if d >= 3 { 2 } else { 1 })..d {
            coords[a] = rest % size[a];
            rest /= size[a];
        }
        let mut csize = lattice_size.clone();
        let mut current: Option<Vec<f32>> = None;
        for a in (2..d).rev() {
            let (first, w) = &tables[a][coords[a]];
            let src = current.as_deref().unwrap_or(&lattice.data);
            current = Some(collapse(src, &csize, a, *first, w));
            csize[a] = 1;
        }
        let above1: &[f32] = current.as_deref().unwrap_or(&lattice.data);
        for (y, line) in dst.chunks_mut(n0).enumerate() {
            // In 2D the unit is one row, whose axis-1 coordinate is `p`.
            let y = if d >= 3 { y } else { coords[1] };
            let (first, w) = &tables[1][y];
            let row = collapse(above1, &csize, 1, *first, w);
            for (x, v) in line.iter_mut().enumerate() {
                let (first, w) = &tables[0][x];
                let mut sum = 0.0f32;
                for (k, &wk) in w.iter().enumerate() {
                    sum += row[first + k] * wk;
                }
                *v = sum;
            }
        }
    };
    larmorx_core::parallel::with_threads(n_threads, || {
        out.par_chunks_mut(plane)
            .enumerate()
            .for_each(|(p, dst)| unit(p, dst));
    })?;
    Ok(out)
}

/// `NumberToIndex(number, size)` for a uniform `size` per axis: the digits of `number` in
/// base `base`, axis 0 the least significant.
fn number_to_index(number: usize, base: usize, d: usize, out: &mut [usize]) {
    let mut n = number;
    for o in out.iter_mut().take(d) {
        *o = n % base;
        n /= base;
    }
}

/// `BSplineControlPointImageFilter::RefineControlPointLattice` with two levels on every axis:
/// the lattice of `order` refined to `2·(n − order) + order` control points per axis.
///
/// Each refined control point `2·i + o` (`o` ∈ {0, 1} per axis) is
/// `Σ_j lattice(i + j) · Π_axes c[o][j]` over `j` ∈ {0, …, order}^D in Fortran order (control
/// points past the lattice skipped), summed in float from 0.
pub fn refine(lattice: &Lattice, order: usize) -> Result<Lattice, FilterError> {
    let d = lattice.dim();
    let coefficients = refined_lattice_coefficients(order)
        .ok_or_else(|| FilterError::invalid("the refinement SVD did not converge"))?;
    let n = order + 1;
    let new_size: Vec<usize> = lattice.size.iter().map(|&s| 2 * s - order).collect();
    let mut refined = Lattice::zeros(&new_size);
    let in_strides = strides(&lattice.size);
    let out_strides = strides(&new_size);
    let n_terms = n.pow(d as u32);
    let mut off = vec![0usize; d];
    let mut off_psi = vec![0usize; d];
    // Even indices of the refined lattice, in Fortran order.
    let half_size: Vec<usize> = new_size.iter().map(|&s| s.div_ceil(2)).collect();
    let n_even: usize = half_size.iter().product();
    let mut idx = vec![0usize; d];
    for e in 0..n_even {
        let mut rest = e;
        for a in 0..d {
            idx[a] = 2 * (rest % half_size[a]);
            rest /= half_size[a];
        }
        for i in 0..(1usize << d) {
            number_to_index(i, 2, d, &mut off);
            if (0..d).any(|a| idx[a] + off[a] >= new_size[a]) {
                continue;
            }
            let mut sum = 0.0f32;
            for j in 0..n_terms {
                number_to_index(j, n, d, &mut off_psi);
                if (0..d).any(|a| idx[a] / 2 + off_psi[a] >= lattice.size[a]) {
                    continue;
                }
                let mut coeff = 1.0f32;
                for a in 0..d {
                    coeff *= coefficients[off[a] * n + off_psi[a]];
                }
                let mut at = 0;
                for a in 0..d {
                    at += (idx[a] / 2 + off_psi[a]) * in_strides[a];
                }
                sum += lattice.data[at] * coeff;
            }
            let mut at = 0;
            for a in 0..d {
                at += (idx[a] + off[a]) * out_strides[a];
            }
            refined.data[at] = sum;
        }
    }
    Ok(refined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_lattice_reconstructs_a_constant() {
        let lattice = Lattice {
            size: vec![5, 4],
            data: vec![2.0; 20],
        };
        let out = evaluate(&lattice, 3, &[9, 7], &[1.0, 2.0], 2).unwrap();
        for v in out {
            assert!((v - 2.0).abs() < 1e-5, "{v}");
        }
    }

    #[test]
    fn refinement_keeps_the_function() {
        // A smooth lattice; its refinement must describe nearly the same spline.
        let size = [5usize, 6, 4];
        let data: Vec<f32> = (0..120).map(|i| ((i * 7) % 11) as f32 * 0.1).collect();
        let lattice = Lattice {
            size: size.to_vec(),
            data,
        };
        let fine = refine(&lattice, 3).unwrap();
        assert_eq!(fine.size, [7, 9, 5]);
        let grid = [13usize, 11, 9];
        let a = evaluate(&lattice, 3, &grid, &[1.0; 3], 1).unwrap();
        let b = evaluate(&fine, 3, &grid, &[1.0; 3], 3).unwrap();
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() < 1e-4, "{x} {y}");
        }
    }
}

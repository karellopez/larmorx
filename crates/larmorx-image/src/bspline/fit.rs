// SPDX-License-Identifier: Apache-2.0
//! `BSplineScatteredDataPointSetToImageFilter` with one fitting level
//! (`ThreadedGenerateDataForFitting` and `AfterThreadedGenerateData`, ITK v5.4.5), for points
//! that lie on an image grid, as N4 fits its residual bias field.
//!
//! **The algorithm** (Lee, Wolberg and Shin, *Scattered data interpolation with multilevel
//! B-splines*, 1997, with weights): each point with value `v`, weight `w` and parametric
//! coordinate `p` adds, to each of the `(order + 1)^D` control points whose B-spline is
//! non-zero at `p` (its product of kernel weights `B`), `ω += w·B²` and
//! `δ += v · B³·w / Σ B²`, the sum over all of them. The control point's value is `δ/ω` (0
//! where `ω` is almost 0, or the quotient is not finite).
//!
//! **Arithmetic as ITK's.** Parameters are `f32`: `r = spans / ((size − 1)·spacing)`,
//! `p = (point − origin)·r` (the point in `f32`, as ITK's `PointSet` stores it), pulled inside
//! `[0, spans)` at the ends; kernel weights in `f64` multiplied into an `f32` product, axis 0
//! first; sums in `f32`. This is what the C++ source defines, and what ITK and ANTs compute
//! when built by a correct compiler. GCC 11.4 and 12.3 at `-O3` miscompile it (GCC bug
//! 110182, fixed in 11.5, 12.4 and 13.2): they keep `r` of the first two axes of 2D and 3D
//! fits in double precision. [`GridFit::double_axes`] reproduces that miscompile, only to
//! compare with such a build ([`gcc11_double_axes`], `docs/findings/ants-n4.md`); it is 0
//! everywhere else.
//!
//! **Speed.** Within one fitting level the points, weights and lattice size do not change, so
//! [`GridFitPlan`] computes the kernel weights, `Σ B²`, `ω` and (memory permitting) every
//! factor `B³·w/Σ B²` once; each fit then only accumulates `δ`.
//!
//! **Threads.** ITK splits the points into one block per work unit, accumulates each block
//! into its own lattices and adds them up afterwards, so its result depends on the number of
//! threads (`docs/findings/ants-n4.md`). larmorx reproduces ITK's **single-thread** order on
//! any number of threads: tasks own the control points with given indices along the last one
//! or two axes, and each visits, in point order, the grid slabs whose points reach them.

#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use super::{BSPLINE_EPSILON, Kernel, Lattice};
use crate::FilterError;
use crate::itk_math::almost_equal_f32;
use crate::volume::strides;

/// **Not ITK's behaviour; for comparisons with a miscompiled build only.** How many leading
/// axes ANTs 2.6.5 built by GCC 11.4 at `-O3` (x86-64; the repository's former oracle,
/// `oracles/ants-2.6.5-gcc11-O3`) computes in double precision where the C++ source rounds
/// to float: the first two in 2D and 3D, none in 4D.
///
/// GCC's SLP vectorizer computes those two axes as a pair of doubles, rounds them to a pair
/// of floats and converts them back; GCC's folding of two conversions in a row then removes
/// the round trip, because it compares `TYPE_PRECISION` of vector types (their number of
/// lanes, equal for 2 × float and 2 × double) instead of the element precision (GCC bug
/// 110182, wrong code; fixed in GCC 11.5, 12.4 and 13.2). This drops the rounding of `r[i]`
/// in `ThreadedGenerateDataForFitting` (`divpd` without `cvtpd2ps`, so `p` and `epsilon`
/// use the unrounded quotient) and, in N4's `UpdateBiasFieldEstimate`, the rounding of the
/// origin in `TransformIndexToPhysicalPoint` into a float point. Pass it as
/// [`GridFit::double_axes`] (and `N4Parameters::double_axes`) to reproduce such a build bit
/// for bit; larmorx itself always uses 0 (`docs/findings/ants-n4.md`).
pub fn gcc11_double_axes(dim: usize) -> usize {
    if dim == 2 || dim == 3 { 2 } else { 0 }
}

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
    /// 0: ITK's source, which rounds `r` to float on every axis. A non-zero value keeps `r`
    /// of that many leading axes in double precision, reproducing a miscompiled ITK build
    /// ([`gcc11_double_axes`]); for comparisons with such a build only. The caller computes
    /// `axis_points` to match.
    pub double_axes: usize,
}

/// Per axis and grid coordinate: the first control point and the `order + 1` kernel weights.
struct AxisTable {
    first: Vec<u32>,
    weights: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
fn axis_table(
    points: &[f32],
    origin: f64,
    spacing: f64,
    size: usize,
    spans: usize,
    kernel: &Kernel,
    axis: usize,
    unrounded: bool,
) -> Result<AxisTable, FilterError> {
    let order = kernel.order();
    let r_full = f64::from(spans as f32) / (f64::from((size - 1) as f32) * spacing);
    let r = if unrounded {
        r_full
    } else {
        f64::from(r_full as f32)
    };
    let eps = (r * spacing * f64::from(BSPLINE_EPSILON)) as f32;
    let s = spans as f32;
    let half = 0.5 * f64::from((order as u32).wrapping_sub(1) as f32);
    let mut first = Vec::with_capacity(points.len());
    let mut weights = Vec::with_capacity(points.len() * (order + 1));
    for &pt in points {
        let mut p = ((f64::from(pt) - origin) * r) as f32;
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

/// Memory for the cached factors above which they are recomputed at every fit instead.
const FACTOR_BUDGET_BYTES: usize = 512 << 20;

/// A fit's points, weights and lattice prepared once (one N4 fitting level): fit any number
/// of value sets on them with [`GridFitPlan::fit`].
pub struct GridFitPlan<'a> {
    d: usize,
    /// `order + 1`.
    n: usize,
    /// `n^d`, the control points each point reaches.
    nn: usize,
    grid_size: Vec<usize>,
    voxels: &'a [usize],
    weights: Vec<f32>,
    tables: Vec<AxisTable>,
    control_points: Vec<usize>,
    /// `Σ B²` per point.
    w2: Vec<f32>,
    /// `ω` per control point.
    omega: Vec<f32>,
    /// `B³·w / Σ B²` per point and neighbour (neighbour digits in Fortran order), if cached.
    factors: Option<Vec<f32>>,
    /// Neighbour digits: `digits[m * d + a]`.
    digits: Vec<usize>,
    /// The axes that split the work (the last one or two).
    fixed: Vec<usize>,
    /// The other axes' count of neighbour combinations, `n^(d − fixed)`.
    free_count: usize,
    /// Control points per task block (the product over the free axes).
    block_len: usize,
    /// Per slab (fixed-axis coordinates, first fixed axis fastest): the range of points.
    slab_start: Vec<usize>,
    /// Per point: `Σ_free first_a · stride_a` in the lattice.
    free_offset: Vec<u32>,
    /// Per free-neighbour combination: `Σ_free j_a · stride_a`.
    neighbour_offset: Vec<u32>,
}

impl<'a> GridFitPlan<'a> {
    /// Prepares the fit of values at the grid voxels `voxels` (linear indices into a grid of
    /// `grid_size`, Fortran order, increasing), each with its weight (`None`: 1).
    pub fn new(
        fit: &GridFit<'_>,
        grid_size: &[usize],
        voxels: &'a [usize],
        weights: Option<&[f32]>,
        n_threads: usize,
    ) -> Result<Self, FilterError> {
        let d = grid_size.len();
        let order = fit.order;
        if d == 0 {
            return Err(FilterError::invalid("a fit needs at least one axis"));
        }
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
        if weights.is_some_and(|w| w.len() != voxels.len()) {
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
        if voxels.windows(2).any(|w| w[0] >= w[1])
            || voxels
                .last()
                .is_some_and(|&v| v >= grid_size.iter().product())
        {
            return Err(FilterError::invalid(
                "the points must be distinct grid voxels in increasing order",
            ));
        }
        let kernel = Kernel::new(order);
        let n = order + 1;
        let nn = n.pow(d as u32);
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
                a < fit.double_axes,
            )?);
        }
        let weights = weights.map_or_else(|| vec![1.0; voxels.len()], <[f32]>::to_vec);
        let digits: Vec<usize> = (0..nn)
            .flat_map(|m| {
                let mut rest = m;
                (0..d).map(move |_| {
                    let j = rest % n;
                    rest /= n;
                    j
                })
            })
            .collect();
        // The last two axes split the work in 3D and above, the last one in 2D: with both
        // axes of a 2D lattice fixed every block would be a single control point, a task and
        // an allocation per point (random testing, 2026-10-25: 3 times ANTs' memory and 12
        // times its time on a lattice of 57 million points). Each control point is summed in
        // point order either way, so the result is the same, bit for bit.
        let n_fixed = if d >= 3 { 2 } else { 1 };
        let fixed: Vec<usize> = (d - n_fixed..d).collect();
        let n_free = d - fixed.len();
        let free_count = n.pow(n_free as u32);
        let cp = fit.control_points.to_vec();
        let cp_strides = strides(&cp);
        let block_len: usize = cp[..n_free].iter().product();
        let grid_strides = strides(grid_size);
        // Slabs: points sharing their fixed-axis coordinates (contiguous, the fixed axes being
        // the slowest).
        let slab_stride = grid_strides[fixed[0]];
        let n_slabs: usize = fixed.iter().map(|&a| grid_size[a]).product();
        let mut slab_start = vec![0usize; n_slabs + 1];
        {
            let mut s = 0;
            for (i, &v) in voxels.iter().enumerate() {
                let slab = v / slab_stride;
                while s < slab {
                    s += 1;
                    slab_start[s] = i;
                }
            }
            while s < n_slabs {
                s += 1;
                slab_start[s] = voxels.len();
            }
        }
        let free_offset: Vec<u32> = voxels
            .iter()
            .map(|&v| {
                (0..n_free)
                    .map(|a| {
                        let x = (v / grid_strides[a]) % grid_size[a];
                        tables[a].first[x] as usize * cp_strides[a]
                    })
                    .sum::<usize>() as u32
            })
            .collect();
        let neighbour_offset: Vec<u32> = (0..free_count)
            .map(|m| {
                (0..n_free)
                    .map(|a| digits[m * d + a] * cp_strides[a])
                    .sum::<usize>() as u32
            })
            .collect();
        let mut plan = GridFitPlan {
            d,
            n,
            nn,
            grid_size: grid_size.to_vec(),
            voxels,
            weights,
            tables,
            control_points: cp,
            w2: Vec::new(),
            omega: Vec::new(),
            factors: None,
            digits,
            fixed,
            free_count,
            block_len,
            slab_start,
            free_offset,
            neighbour_offset,
        };
        larmorx_core::parallel::with_threads(n_threads, || -> Result<(), FilterError> {
            plan.w2 = plan.sums_of_squares();
            plan.omega = plan.accumulate(|p, i, t| {
                let _ = p;
                let wc = plan.weights[i];
                wc * t * t
            });
            if voxels.len().saturating_mul(nn).saturating_mul(4) <= FACTOR_BUDGET_BYTES {
                plan.factors = Some(plan.all_factors());
            }
            Ok(())
        })??;
        Ok(plan)
    }

    /// The lattice size.
    pub fn control_points(&self) -> &[usize] {
        &self.control_points
    }

    /// The grid coordinates of point `i`.
    fn coordinates(&self, i: usize, out: &mut [usize]) {
        let v = self.voxels[i];
        let mut rest = v;
        for (a, o) in out.iter_mut().enumerate() {
            *o = rest % self.grid_size[a];
            rest /= self.grid_size[a];
        }
    }

    /// `B` for neighbour `m` of a point at grid coordinates `x`: the kernel weights multiplied
    /// into a float, axis 0 first.
    #[inline]
    fn product(&self, x: &[usize], m: usize) -> f32 {
        let n = self.n;
        let mut b = 1.0f32;
        for a in 0..self.d {
            b = (f64::from(b) * self.tables[a].weights[x[a] * n + self.digits[m * self.d + a]])
                as f32;
        }
        b
    }

    /// `Σ B²` per point, over its neighbours in Fortran order (in parallel over points).
    fn sums_of_squares(&self) -> Vec<f32> {
        (0..self.voxels.len())
            .into_par_iter()
            .with_min_len(1024)
            .map_init(
                || vec![0usize; self.d],
                |x, i| {
                    self.coordinates(i, x);
                    let mut sum = 0.0f32;
                    for m in 0..self.nn {
                        let b = self.product(x, m);
                        sum += b * b;
                    }
                    sum
                },
            )
            .collect()
    }

    /// `B³·w / Σ B²` for every point and neighbour.
    fn all_factors(&self) -> Vec<f32> {
        let nn = self.nn;
        let mut out = vec![0.0f32; self.voxels.len() * nn];
        out.par_chunks_mut(nn)
            .enumerate()
            .with_min_len(256)
            .for_each_init(
                || vec![0usize; self.d],
                |x, (i, dst)| {
                    self.coordinates(i, x);
                    let (wc, w2) = (self.weights[i], self.w2[i]);
                    for (m, f) in dst.iter_mut().enumerate() {
                        let t = self.product(x, m);
                        *f = t * t * t * wc / w2;
                    }
                },
            );
        out
    }

    /// The lattice `Σ_points contribution(point, index, B)` over the control points each
    /// point reaches, each control point summed in point order (ITK's single-thread order),
    /// in parallel over blocks of control points.
    fn accumulate<F>(&self, contribution: F) -> Vec<f32>
    where
        F: Fn(&Self, usize, f32) -> f32 + Sync,
    {
        self.accumulate_with(|i, m, x| contribution(self, i, self.product(x, m)), true)
    }

    /// The core of [`GridFitPlan::accumulate`]: `term(point, neighbour, coordinates)` added to
    /// each control point. `needs_coordinates`: whether `term` reads the coordinates.
    fn accumulate_with<T>(&self, term: T, needs_coordinates: bool) -> Vec<f32>
    where
        T: Fn(usize, usize, &[usize]) -> f32 + Sync,
    {
        let d = self.d;
        let order = self.n - 1;
        let cp = &self.control_points;
        let n_fixed = self.fixed.len();
        let n_tasks: usize = self.fixed.iter().map(|&a| cp[a]).product();
        let blocks: Vec<Vec<f32>> = (0..n_tasks)
            .into_par_iter()
            .map(|task| {
                let mut block = vec![0.0f32; self.block_len];
                // The task's control-point indices along the fixed axes, and the coordinate
                // ranges of the points that reach them.
                let mut c = [0usize; 2];
                let mut ranges = [(0usize, 0usize); 2];
                let mut rest = task;
                for (k, &a) in self.fixed.iter().enumerate() {
                    c[k] = rest % cp[a];
                    rest /= cp[a];
                    let first = &self.tables[a].first;
                    let lo = first.partition_point(|&b| (b as usize) + order < c[k]);
                    let hi = first.partition_point(|&b| (b as usize) <= c[k]);
                    ranges[k] = (lo, hi);
                }
                let mut x = vec![0usize; d];
                let size0 = self.grid_size[self.fixed[0]];
                let (r1lo, r1hi) = if n_fixed == 2 { ranges[1] } else { (0, 1) };
                for x1 in r1lo..r1hi {
                    for x0 in ranges[0].0..ranges[0].1 {
                        let slab = if n_fixed == 2 { x0 + size0 * x1 } else { x0 };
                        let mut m_fixed = c[0] - self.tables[self.fixed[0]].first[x0] as usize;
                        if n_fixed == 2 {
                            m_fixed +=
                                self.n * (c[1] - self.tables[self.fixed[1]].first[x1] as usize);
                        }
                        for i in self.slab_start[slab]..self.slab_start[slab + 1] {
                            if needs_coordinates {
                                self.coordinates(i, &mut x);
                            }
                            let base = self.free_offset[i] as usize;
                            for mf in 0..self.free_count {
                                let m = mf + self.free_count * m_fixed;
                                let cell = base + self.neighbour_offset[mf] as usize;
                                block[cell] += term(i, m, &x);
                            }
                        }
                    }
                }
                block
            })
            .collect();
        let mut out = Vec::with_capacity(n_tasks * self.block_len);
        for b in blocks {
            out.extend_from_slice(&b);
        }
        out
    }

    /// The B-spline fit of `values` (one per point): the control-point lattice (`phi`).
    pub fn fit(&self, values: &[f32], n_threads: usize) -> Result<Lattice, FilterError> {
        if values.len() != self.voxels.len() {
            return Err(FilterError::SizeMismatch {
                what: "the values",
                actual: values.len(),
                expected: self.voxels.len(),
            });
        }
        let nn = self.nn;
        let delta = larmorx_core::parallel::with_threads(n_threads, || match &self.factors {
            Some(f) => self.accumulate_with(|i, m, _| values[i] * f[i * nn + m], false),
            None => self.accumulate_with(
                |i, m, x| {
                    let t = self.product(x, m);
                    values[i] * (t * t * t * self.weights[i] / self.w2[i])
                },
                true,
            ),
        })?;
        let data = self
            .omega
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
            size: self.control_points.clone(),
            data,
        })
    }
}

/// The B-spline fit of `values` at the grid voxels `voxels` (linear indices into a grid of
/// `grid_size`, Fortran order, increasing), each with its weight (`None`: 1): a
/// [`GridFitPlan`] used once.
pub fn fit_on_grid(
    fit: &GridFit<'_>,
    grid_size: &[usize],
    voxels: &[usize],
    values: &[f32],
    weights: Option<&[f32]>,
    n_threads: usize,
) -> Result<Lattice, FilterError> {
    if values.len() != voxels.len() {
        return Err(FilterError::invalid(
            "The number of weight points and input points must be equal.",
        ));
    }
    GridFitPlan::new(fit, grid_size, voxels, weights, n_threads)?.fit(values, n_threads)
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
                    a < fit.double_axes,
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

    type Case = (Vec<usize>, Vec<f64>, Vec<f64>, Vec<Vec<f32>>, Vec<usize>);

    fn case(dim: usize) -> Case {
        let all_size = [11usize, 9, 7, 5];
        let all_spacing = [1.5f64, 2.0, 2.5, 3.0];
        let all_origin = [-3.0f64, 4.0, 1.0, 0.5];
        let all_cp = [6usize, 5, 4, 4];
        let size = all_size[..dim].to_vec();
        let spacing = all_spacing[..dim].to_vec();
        let origin = all_origin[..dim].to_vec();
        let axis_points = (0..dim)
            .map(|a| {
                (0..size[a])
                    .map(|x| (f64::from(origin[a] as f32) + spacing[a] * x as f64) as f32)
                    .collect()
            })
            .collect();
        (size, spacing, origin, axis_points, all_cp[..dim].to_vec())
    }

    #[test]
    fn parallel_fit_equals_the_sequential_order_in_2_to_4_dimensions() {
        for dim in 2..=4 {
            let (size, spacing, origin, axis_points, cp) = case(dim);
            let total: usize = size.iter().product();
            for unrounded in [0, gcc11_double_axes(dim)] {
                let fit = GridFit {
                    axis_points: &axis_points,
                    origin: &origin,
                    spacing: &spacing,
                    size: &size,
                    control_points: &cp,
                    order: 3,
                    double_axes: unrounded,
                };
                let voxels: Vec<usize> = (0..total).filter(|v| v % 3 != 1).collect();
                let values: Vec<f32> = voxels
                    .iter()
                    .map(|&v| ((v * 37) % 101) as f32 * 0.01)
                    .collect();
                let weights: Vec<f32> = voxels
                    .iter()
                    .map(|&v| 0.25 + (v % 4) as f32 * 0.25)
                    .collect();
                let expected = sequential(&fit, &size, &voxels, &values, &weights);
                for threads in [1, 2, 5] {
                    let got = fit_on_grid(&fit, &size, &voxels, &values, Some(&weights), threads)
                        .unwrap();
                    assert_eq!(got.data, expected, "{dim}D, {threads} threads");
                }
                // Without cached factors.
                let mut plan = GridFitPlan::new(&fit, &size, &voxels, Some(&weights), 3).unwrap();
                plan.factors = None;
                assert_eq!(plan.fit(&values, 2).unwrap().data, expected);
            }
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
            double_axes: 0,
        };
        assert!(fit_on_grid(&fit, &[4], &[0, 1, 2, 3], &[1.0; 4], None, 1).is_err());
    }
}

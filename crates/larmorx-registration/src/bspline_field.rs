// SPDX-License-Identifier: Apache-2.0
//! B-spline approximation of displacement fields (ITK v5.4.5,
//! `Modules/Filtering/DisplacementField/include/itkDisplacementFieldToBSplineImageFilter.hxx`,
//! `Modules/Filtering/ImageGrid/include/itkBSplineScatteredDataPointSetToImageFilter.hxx`), as
//! `BSplineSmoothingOnUpdateDisplacementFieldTransform` and `BSplineSyNImageRegistrationMethod`
//! run it: one fitting level, no closed dimension, no inverse estimate.
//!
//! - [`field_to_bspline`]: `DisplacementFieldToBSplineImageFilter` with
//!   `SetUseInputFieldToDefineTheBSplineDomain(true)`: every voxel of the field is a point at
//!   its parametric position (its physical point in `float`, mapped back to a continuous index
//!   in double and onto the B-spline domain without its direction, in `float`), weighted by an
//!   optional confidence image; with `EnforceStationaryBoundary` the boundary voxels are points
//!   of displacement 0 and weight 1e10.
//! - [`points_to_bspline`]: the same filter given a point set and a domain
//!   (`SetBSplineDomainFromImage`), as the B-spline SyN method fits a point-set metric's
//!   derivative.
//! - [`fit`] and [`reconstruct`]: `BSplineScatteredDataPointSetToImageFilter` for one level:
//!   the control-point lattice of Lee, Wolberg and Shin's approximation (`ω += w·B²`,
//!   `δ += v·B³·w/ΣB²`, `φ = δ/ω`), and the B-spline of that lattice evaluated on the domain's
//!   grid (`CollapsePhiLattice`, the last axis first).
//!
//! **Arithmetic as ITK's.** The filter's `RealType` is `float`: the parametric coordinates, the
//! kernel products `B`, `ΣB²`, the weights and `ω` are floats; the data (`Vector<R, D>`) are
//! scaled by float factors in `R`, and summed in `R`. The kernels are
//! [`larmorx_image::bspline::Kernel`] (`BSplineKernelFunction` in double, stored in float).
//!
//! **Threads.** ITK splits the points among its work units, accumulates each block into its
//! own lattices and adds them up, so its result depends on the thread count. larmorx gives
//! ITK's single-thread result on any number of threads: each task owns the control points with
//! one index along the last axis and adds, in point order, the contributions of the points that
//! reach them.

use larmorx_image::bspline::Kernel;
use larmorx_image::itk_math::almost_equal_f32;
use rayon::prelude::*;

use crate::field::Field;
use crate::format::format_g;
use crate::{Image, ItkException, Real, Space};

const SCATTERED: &str =
    "Modules/Filtering/ImageGrid/include/itkBSplineScatteredDataPointSetToImageFilter.hxx";
const SCATTERED_CLASS: &str = "BSplineScatteredDataPointSetToImageFilter";
const TO_BSPLINE: &str =
    "Modules/Filtering/DisplacementField/include/itkDisplacementFieldToBSplineImageFilter.hxx";
const TO_BSPLINE_CLASS: &str = "DisplacementFieldToBSplineImageFilter";

/// `m_BSplineEpsilon`.
const BSPLINE_EPSILON: f32 = 1e-3;

/// The weight of a boundary point (`boundaryWeight = 1.0e10`, a float).
const BOUNDARY_WEIGHT: f32 = 1.0e10;

/// A control-point lattice of displacement vectors (`PointDataImageType`), x fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct Lattice<R, const D: usize> {
    pub size: [usize; D],
    pub data: Vec<[R; D]>,
}

/// The scattered data of a fit: points (`Point<float, D>`, the B-spline domain's parametric
/// positions), their displacements and weights (float).
#[derive(Clone, Debug, Default)]
pub struct Scattered<R, const D: usize> {
    pub points: Vec<[f32; D]>,
    pub data: Vec<[R; D]>,
    pub weights: Vec<f32>,
}

/// The fit's domain: `m_Origin`, `m_Spacing`, `m_Size` of the filter (its direction only
/// places the output).
#[derive(Clone, Copy, Debug)]
pub struct FitDomain<const D: usize> {
    pub origin: [f64; D],
    pub spacing: [f64; D],
    pub size: [usize; D],
}

impl<const D: usize> FitDomain<D> {
    /// The domain of an image's grid.
    pub fn of(space: &Space<D>) -> Self {
        FitDomain {
            origin: space.origin,
            spacing: space.spacing,
            size: space.size,
        }
    }
}

fn scattered_error(line: u32, description: impl Into<String>) -> ItkException {
    ItkException::new(SCATTERED, line, SCATTERED_CLASS, description)
}

/// `r[i]` and `epsilon[i]` of an axis: `float(spans) / (float(size − 1)·spacing)` stored in
/// float, and `r·spacing·m_BSplineEpsilon` stored in float.
fn r_and_epsilon(spans: usize, size: usize, spacing: f64) -> (f32, f32) {
    let r = (f64::from(spans as f32) / (f64::from((size - 1) as f32) * spacing)) as f32;
    let eps = (f64::from(r) * spacing * f64::from(BSPLINE_EPSILON)) as f32;
    (r, eps)
}

/// The kernel weights of the `order + 1` control points from `first = uint(p)` on: control
/// point `first + j` gets `B(float(float(p − first) − j) + (order − 1)/2)`, stored in float.
#[inline]
fn fit_weights(kernel: &Kernel, order: usize, half: f64, p: f32, out: &mut [f64]) -> usize {
    let first = p as u32;
    let frac = p - first as f32;
    for (j, o) in out.iter_mut().enumerate().take(order + 1) {
        let u = (f64::from(frac - j as f32) + half) as f32;
        *o = kernel.evaluate(f64::from(u));
    }
    first as usize
}

/// The checks of `GenerateData` before the fit.
fn check<const D: usize>(
    domain: &FitDomain<D>,
    control_points: &[u32; D],
    order: u32,
) -> Result<(), ItkException> {
    if order == 0 {
        return Err(scattered_error(
            88,
            "The spline order in each dimension must be greater than 0",
        ));
    }
    if domain.size.contains(&0) {
        return Err(scattered_error(198, "Size must be specified."));
    }
    if control_points.iter().any(|&c| c < order + 1) {
        return Err(scattered_error(
            220,
            "The number of control points must be greater than the spline order.",
        ));
    }
    Ok(())
}

/// A point's parametric coordinates `p` (float), checked as ITK does.
#[allow(clippy::too_many_arguments)]
fn parametric<const D: usize>(
    point: &[f32; D],
    domain: &FitDomain<D>,
    r: &[f32; D],
    eps: &[f32; D],
    spans: &[usize; D],
) -> Result<[f32; D], ItkException> {
    let mut p = [0f32; D];
    for i in 0..D {
        let s = spans[i] as f32;
        let mut v = ((f64::from(point[i]) - domain.origin[i]) * f64::from(r[i])) as f32;
        if (v - s).abs() <= eps[i] {
            v = s - eps[i];
        }
        if v < 0.0 && v.abs() <= eps[i] {
            v = 0.0;
        }
        if v < 0.0 || v >= s || v.is_nan() {
            return Err(scattered_error(
                477,
                format!(
                    "The reparameterized point component {} is outside the corresponding parametric domain of [0, {}).",
                    format_g(f64::from(v), 6),
                    spans[i]
                ),
            ));
        }
        p[i] = v;
    }
    Ok(p)
}

/// What one point contributes: its first control point per axis, the kernel weights per
/// axis, and `ΣB²`.
struct PointWeights<const D: usize> {
    first: [usize; D],
    /// `(order + 1)` weights per axis, axis by axis.
    weights: Vec<f64>,
    w2: f32,
}

/// `B` for the neighbour of digits `digits` (axis 0 first): the kernel weights multiplied
/// into a float, axis 0 first, each product computed in double.
#[inline]
fn product<const D: usize>(w: &[f64], n: usize, digits: &[usize; D]) -> f32 {
    let mut b = 1.0f32;
    for a in 0..D {
        b = (f64::from(b) * w[a * n + digits[a]]) as f32;
    }
    b
}

/// The neighbour digits of `m` (axis 0 the least significant, base `n`).
#[inline]
fn digits_of<const D: usize>(m: usize, n: usize) -> [usize; D] {
    let mut rest = m;
    std::array::from_fn(|_| {
        let d = rest % n;
        rest /= n;
        d
    })
}

/// `ThreadedGenerateDataForFitting` and `AfterThreadedGenerateData` for one level: the
/// control-point lattice `φ` of `control_points` per axis.
pub fn fit<R: Real, const D: usize>(
    domain: &FitDomain<D>,
    control_points: &[u32; D],
    order: u32,
    input: &Scattered<R, D>,
) -> Result<Lattice<R, D>, ItkException> {
    check(domain, control_points, order)?;
    let ord = order as usize;
    let n = ord + 1;
    let nn = n.pow(D as u32);
    let kernel = Kernel::new(ord);
    let half = 0.5 * f64::from(order.wrapping_sub(1) as f32);
    let cps: [usize; D] = std::array::from_fn(|i| control_points[i] as usize);
    let spans: [usize; D] = std::array::from_fn(|i| cps[i] - ord);
    let mut r = [0f32; D];
    let mut eps = [0f32; D];
    for i in 0..D {
        (r[i], eps[i]) = r_and_epsilon(spans[i], domain.size[i], domain.spacing[i]);
    }
    // Each point's parametric position, kernel weights and ΣB² (in parallel; the first point
    // outside the domain is ITK's exception).
    let per_point: Vec<Result<PointWeights<D>, ItkException>> = input
        .points
        .par_iter()
        .with_min_len(256)
        .map(|pt| {
            let p = parametric(pt, domain, &r, &eps, &spans)?;
            let mut weights = vec![0f64; D * n];
            let first: [usize; D] = std::array::from_fn(|a| {
                fit_weights(&kernel, ord, half, p[a], &mut weights[a * n..(a + 1) * n])
            });
            let mut w2 = 0f32;
            for m in 0..nn {
                let b = product(&weights, n, &digits_of::<D>(m, n));
                w2 += b * b;
            }
            Ok(PointWeights { first, weights, w2 })
        })
        .collect();
    let mut points = Vec::with_capacity(per_point.len());
    for p in per_point {
        points.push(p?);
    }
    let strides: [usize; D] = {
        let mut s = [1usize; D];
        for k in 1..D {
            s[k] = s[k - 1] * cps[k - 1];
        }
        s
    };
    let last = D - 1;
    let slab: usize = strides[last];
    // Points by their first control point along the last axis, in point order.
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); cps[last]];
    for (i, p) in points.iter().enumerate() {
        buckets[p.first[last]].push(i as u32);
    }
    // One task per control-point index along the last axis: the points that reach it, in point
    // order (a merge of the buckets c − order ..= c), each adding its contributions to the
    // control points of that index.
    let slabs: Vec<(Vec<f32>, Vec<[R; D]>)> = (0..cps[last])
        .into_par_iter()
        .map(|c| {
            let mut omega = vec![0f32; slab];
            let mut delta = vec![[R::ZERO; D]; slab];
            let lo = c.saturating_sub(ord);
            let mut heads: Vec<(usize, usize)> = (lo..=c).map(|b| (b, 0usize)).collect();
            loop {
                // The next point among the buckets (smallest index).
                let mut best: Option<(usize, u32)> = None;
                for (k, &(b, pos)) in heads.iter().enumerate() {
                    if let Some(&i) = buckets[b].get(pos)
                        && best.is_none_or(|(_, j)| i < j)
                    {
                        best = Some((k, i));
                    }
                }
                let Some((k, i)) = best else { break };
                heads[k].1 += 1;
                let i = i as usize;
                let pw = &points[i];
                let j_last = c - pw.first[last];
                let wc = input.weights[i];
                let v = input.data[i];
                // The neighbours whose last-axis digit is j_last, in ItW order.
                let inner = nn / n;
                for mi in 0..inner {
                    let m = mi + inner * j_last;
                    let digits = digits_of::<D>(m, n);
                    let t = product(&pw.weights, n, &digits);
                    let mut cell = 0usize;
                    for a in 0..last {
                        cell += (pw.first[a] + digits[a]) * strides[a];
                    }
                    omega[cell] += wc * t * t;
                    let factor = t * t * t * wc / pw.w2;
                    let f = R::from_f64(f64::from(factor));
                    let d = &mut delta[cell];
                    for k in 0..D {
                        d[k] = d[k] + scale(v[k], f);
                    }
                }
            }
            (omega, delta)
        })
        .collect();
    let mut data = Vec::with_capacity(slab * cps[last]);
    for (omega, delta) in slabs {
        for (o, dl) in omega.iter().zip(&delta) {
            let mut p = [R::ZERO; D];
            if !almost_equal_f32(*o, 0.0) {
                for k in 0..D {
                    let q = divide(dl[k], *o);
                    p[k] = if q.to_f64().is_finite() { q } else { R::ZERO };
                }
            }
            data.push(p);
        }
    }
    Ok(Lattice { size: cps, data })
}

/// `R · float` (`Vector<R>::operator*=(float)`): in double for `R` = double, in float for
/// float.
#[inline]
fn scale<R: Real>(v: R, f: R) -> R {
    v * f
}

/// `Vector<R> / float`.
#[inline]
fn divide<R: Real>(v: R, o: f32) -> R {
    v / R::from_f64(f64::from(o))
}

/// The collapse weights of an output coordinate `u` along an axis: the first control point
/// and `B(float(u − float(first + k)) + (order − 1)/2)` in float.
fn collapse_weights(kernel: &Kernel, order: usize, half: f64, u: f32) -> (usize, Vec<f32>) {
    let first = u as u32;
    let w = (0..=order as u32)
        .map(|k| {
            let index = first + k;
            let v = (f64::from(u - index as f32) + half) as f32;
            kernel.evaluate(f64::from(v)) as f32
        })
        .collect();
    (first as usize, w)
}

/// `ThreadedGenerateDataForReconstruction`: the B-spline of `lattice` evaluated on `space`'s
/// grid (output index `i` at `U = spans·i/(size − 1)` in float, pulled inside `[0, spans)`),
/// each voxel collapsing the lattice along the last axis first; sums in `R` from 0.
pub fn reconstruct<R: Real, const D: usize>(
    lattice: &Lattice<R, D>,
    order: u32,
    space: &Space<D>,
) -> Result<Field<R, D>, ItkException> {
    let ord = order as usize;
    let kernel = Kernel::new(ord);
    let half = 0.5 * f64::from(order.wrapping_sub(1) as f32);
    // Per axis and output coordinate: the first control point and the weights.
    let mut tables: Vec<Vec<(usize, Vec<f32>)>> = Vec::with_capacity(D);
    for a in 0..D {
        let spans = lattice.size[a] - ord;
        let size = space.size[a];
        let (_, eps) = r_and_epsilon(spans, size, space.spacing[a]);
        let s = spans as f32;
        let mut axis = Vec::with_capacity(size);
        for i in 0..size {
            let mut u = s * i as f32 / (size - 1) as f32;
            if (u - s).abs() <= eps {
                u = s - eps;
            }
            if u < 0.0 && u.abs() <= eps {
                u = 0.0;
            }
            if u < 0.0 || u >= s || u.is_nan() {
                return Err(scattered_error(
                    630,
                    format!(
                        "The collapse point component {} is outside the corresponding parametric domain of [0, {spans}).",
                        format_g(f64::from(u), 6)
                    ),
                ));
            }
            axis.push(collapse_weights(&kernel, ord, half, u));
        }
        tables.push(axis);
    }
    let lsize = lattice.size;
    // Collapse `src` (axes after `axis` already of size 1) along `axis`.
    let collapse = |src: &[[R; D]], axis: usize, first: usize, w: &[f32]| -> Vec<[R; D]> {
        let inner: usize = lsize[..axis].iter().product();
        let mut out = vec![[R::ZERO; D]; inner];
        for (k, &wk) in w.iter().enumerate() {
            let base = (first + k) * inner;
            let b = R::from_f64(f64::from(wk));
            for (o, v) in out.iter_mut().zip(&src[base..base + inner]) {
                for c in 0..D {
                    o[c] = o[c] + v[c] * b;
                }
            }
        }
        out
    };
    let n0 = space.size[0];
    let n_voxels = space.len();
    let lines = n_voxels / n0.max(1);
    let rows: Vec<Vec<[R; D]>> = (0..lines)
        .into_par_iter()
        .map(|line| {
            let idx = space.index_of(line * n0);
            let mut current: Option<Vec<[R; D]>> = None;
            for a in (1..D).rev() {
                let i = (idx[a] - space.start[a]) as usize;
                let (first, w) = &tables[a][i];
                let src = current.as_deref().unwrap_or(&lattice.data);
                current = Some(collapse(src, a, *first, w));
            }
            let row = current.unwrap_or_else(|| lattice.data.clone());
            (0..n0)
                .map(|x| {
                    let (first, w) = &tables[0][x];
                    let mut sum = [R::ZERO; D];
                    for (k, &wk) in w.iter().enumerate() {
                        let b = R::from_f64(f64::from(wk));
                        let v = &row[first + k];
                        for c in 0..D {
                            sum[c] = sum[c] + v[c] * b;
                        }
                    }
                    sum
                })
                .collect()
        })
        .collect();
    let data: Vec<[R; D]> = rows.into_iter().flatten().collect();
    let out_space = Space::new(
        space.size,
        [0; D],
        space.spacing,
        space.origin,
        space.direction,
    )?;
    Ok(Image::new(out_space, data))
}

/// `TransformIndexToPhysicalPoint` into a `Point<float>`: each coordinate starts at the origin
/// (in float) and adds `M[i][j]·index[j]` in double, stored in float.
#[inline]
fn index_to_point_f32<const D: usize>(space: &Space<D>, idx: &[i64; D]) -> [f32; D] {
    std::array::from_fn(|i| {
        let mut v = space.origin[i] as f32;
        for j in 0..D {
            v = (f64::from(v) + space.index_to_point[i][j] * idx[j] as f64) as f32;
        }
        v
    })
}

/// The parametric point of a physical point on the B-spline domain `space`: its continuous
/// index (double), then `TransformContinuousIndexToPhysicalPoint` into a `Point<float>` on the
/// domain without its direction (`sum` in float from 0 of `spacing·index`, plus the origin).
#[inline]
fn parametric_point<const D: usize>(space: &Space<D>, p: &[f32; D]) -> [f32; D] {
    let pd: [f64; D] = std::array::from_fn(|k| f64::from(p[k]));
    let c: [f64; D] = space.point_to_cindex(&pd);
    std::array::from_fn(|r| {
        let mut sum = 0f32;
        for k in 0..D {
            let m = if k == r { space.spacing[r] } else { 0.0 };
            sum = (f64::from(sum) + m * c[k]) as f32;
        }
        (f64::from(sum) + space.origin[r]) as f32
    })
}

/// Whether `idx` is on the stationary boundary of a domain of `size` voxels from index 0.
#[inline]
fn on_boundary<const D: usize>(idx: &[i64; D], start: &[i64; D], size: &[usize; D]) -> bool {
    (0..D).any(|d| idx[d] == start[d] || idx[d] == start[d] + size[d] as i64 - 1)
}

/// `DisplacementFieldToBSplineImageFilter` with the input field defining the B-spline domain:
/// the field smoothed by a B-spline of `order` with `control_points` per axis (one fitting
/// level). `confidence` (`SetConfidenceImage`, on the field's grid) leaves out the voxels where
/// it is not positive and weights the others by it; with `stationary_boundary` the boundary
/// voxels are fixed at 0.
pub fn field_to_bspline<R: Real, const D: usize>(
    field: &Field<R, D>,
    confidence: Option<&Image<f64, D>>,
    control_points: &[u32; D],
    order: u32,
    stationary_boundary: bool,
) -> Result<Field<R, D>, ItkException> {
    let (domain_space, input) = field_scattered(field, confidence, stationary_boundary)?;
    run(&domain_space, control_points, order, &input)
}

/// The B-spline domain and the scattered data `DisplacementFieldToBSplineImageFilter` builds
/// from a field that defines the domain (see [`field_to_bspline`]).
pub fn field_scattered<R: Real, const D: usize>(
    field: &Field<R, D>,
    confidence: Option<&Image<f64, D>>,
    stationary_boundary: bool,
) -> Result<(Space<D>, Scattered<R, D>), ItkException> {
    let space = &field.space;
    let start = [0i64; D];
    // The B-spline domain: the field's geometry (bsplinePhysicalDomainField) with region
    // index 0.
    let domain_space = Space::new(
        space.size,
        start,
        space.spacing,
        space.origin,
        space.direction,
    )?;
    // Per voxel: its parametric point, its vector and its weight, if it is a point.
    type Point<R, const D: usize> = ([f32; D], [R; D], f32);
    let selected: Vec<Option<Point<R, D>>> = (0..space.len())
        .into_par_iter()
        .with_min_len(1024)
        .map(|o| {
            let idx = space.index_of(o);
            let boundary = stationary_boundary && on_boundary(&idx, &start, &space.size);
            let conf = confidence.map(|c| c.data[o]);
            if let Some(c) = conf
                && c <= 0.0
                && !boundary
            {
                return None;
            }
            let mut weight = 1.0f32;
            if let Some(c) = conf
                && c > 0.0
            {
                weight = c as f32;
            }
            let physical = index_to_point_f32(space, &idx);
            let parametric = parametric_point(&domain_space, &physical);
            let mut data = field.data[o];
            if boundary {
                data = [R::ZERO; D];
                weight = BOUNDARY_WEIGHT;
            }
            Some((parametric, data, weight))
        })
        .collect();
    let mut input = Scattered {
        points: Vec::with_capacity(selected.len()),
        data: Vec::with_capacity(selected.len()),
        weights: Vec::with_capacity(selected.len()),
    };
    for (p, d, w) in selected.into_iter().flatten() {
        input.points.push(p);
        input.data.push(d);
        input.weights.push(w);
    }
    Ok((domain_space, input))
}

/// `DisplacementFieldToBSplineImageFilter` given a point set and a B-spline domain
/// (`SetBSplineDomainFromImage(field)`): each point inside the domain (and, with a stationary
/// boundary, more than half a voxel inside it) at its parametric position; with
/// `stationary_boundary` the domain's boundary voxels are points of displacement 0 and weight
/// 1e10, listed first.
pub fn points_to_bspline<R: Real, const D: usize>(
    domain: &Space<D>,
    points: &[[f32; D]],
    data: &[[R; D]],
    control_points: &[u32; D],
    order: u32,
    stationary_boundary: bool,
) -> Result<Field<R, D>, ItkException> {
    let start = [0i64; D];
    let domain_space = Space::new(
        domain.size,
        start,
        domain.spacing,
        domain.origin,
        domain.direction,
    )?;
    // The parametric domain (the domain without its direction).
    let identity: [[f64; D]; D] =
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }));
    let parametric_space = Space::new(domain.size, start, domain.spacing, domain.origin, identity)?;
    let mut input = Scattered::default();
    if stationary_boundary {
        for o in 0..domain_space.len() {
            let idx = domain_space.index_of(o);
            if on_boundary(&idx, &start, &domain.size) {
                input
                    .points
                    .push(index_to_point_f32(&parametric_space, &idx));
                input.data.push([R::ZERO; D]);
                input.weights.push(BOUNDARY_WEIGHT);
            }
        }
    }
    for (p, d) in points.iter().zip(data) {
        let pd: [f64; D] = std::array::from_fn(|k| f64::from(p[k]));
        let c: [f64; D] = domain_space.point_to_cindex(&pd);
        // ImageRegion::IsInside(ContinuousIndex): start − 0.5 ≤ c ≤ start + size − 0.5.
        let mut inside = (0..D).all(|i| {
            c[i] >= start[i] as f64 - 0.5 && c[i] <= (start[i] + domain.size[i] as i64) as f64 - 0.5
        });
        if inside && stationary_boundary {
            for i in 0..D {
                if c[i] < start[i] as f64 + 0.5
                    || c[i] > (start[i] + domain.size[i] as i64 - 1) as f64 - 0.5
                {
                    inside = false;
                    break;
                }
            }
        }
        if inside {
            let q: [f32; D] = std::array::from_fn(|r| {
                let mut sum = 0f32;
                for k in 0..D {
                    let m = if k == r { domain.spacing[r] } else { 0.0 };
                    sum = (f64::from(sum) + m * c[k]) as f32;
                }
                (f64::from(sum) + domain.origin[r]) as f32
            });
            input.points.push(q);
            input.data.push(*d);
            input.weights.push(1.0);
        }
    }
    run(&domain_space, control_points, order, &input)
}

/// The fit and its reconstruction on the domain.
fn run<R: Real, const D: usize>(
    domain: &Space<D>,
    control_points: &[u32; D],
    order: u32,
    input: &Scattered<R, D>,
) -> Result<Field<R, D>, ItkException> {
    if input.points.is_empty() {
        return Err(ItkException::new(
            TO_BSPLINE,
            359,
            TO_BSPLINE_CLASS,
            "No points were found.  Check that one or both inputs (displacement field/point set) are set.",
        ));
    }
    let lattice = fit(&FitDomain::of(domain), control_points, order, input)?;
    reconstruct(&lattice, order, domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ITK's single-thread loop, written plainly: the reference for [`fit`].
    fn sequential(
        domain: &FitDomain<3>,
        cps: &[u32; 3],
        order: u32,
        input: &Scattered<f64, 3>,
    ) -> Vec<[f64; 3]> {
        let ord = order as usize;
        let n = ord + 1;
        let kernel = Kernel::new(ord);
        let half = 0.5 * f64::from(order.wrapping_sub(1) as f32);
        let c: [usize; 3] = std::array::from_fn(|i| cps[i] as usize);
        let spans: [usize; 3] = std::array::from_fn(|i| c[i] - ord);
        let mut r = [0f32; 3];
        let mut eps = [0f32; 3];
        for i in 0..3 {
            (r[i], eps[i]) = r_and_epsilon(spans[i], domain.size[i], domain.spacing[i]);
        }
        let total = c[0] * c[1] * c[2];
        let mut omega = vec![0f32; total];
        let mut delta = vec![[0f64; 3]; total];
        for (i, pt) in input.points.iter().enumerate() {
            let p = parametric(pt, domain, &r, &eps, &spans).unwrap();
            let mut w = vec![0f64; 3 * n];
            let first: [usize; 3] =
                std::array::from_fn(|a| fit_weights(&kernel, ord, half, p[a], &mut w[a * n..]));
            let mut w2 = 0f32;
            let mut bs = Vec::new();
            for m in 0..n * n * n {
                let dg = digits_of::<3>(m, n);
                let b = product(&w, n, &dg);
                w2 += b * b;
                bs.push((dg, b));
            }
            for (dg, t) in bs {
                let cell =
                    (first[0] + dg[0]) + c[0] * ((first[1] + dg[1]) + c[1] * (first[2] + dg[2]));
                let wc = input.weights[i];
                omega[cell] += wc * t * t;
                let f = f64::from(t * t * t * wc / w2);
                for k in 0..3 {
                    delta[cell][k] += input.data[i][k] * f;
                }
            }
        }
        omega
            .iter()
            .zip(&delta)
            .map(|(&o, d)| {
                if almost_equal_f32(o, 0.0) {
                    [0.0; 3]
                } else {
                    std::array::from_fn(|k| {
                        let q = d[k] / f64::from(o);
                        if q.is_finite() { q } else { 0.0 }
                    })
                }
            })
            .collect()
    }

    #[test]
    fn the_parallel_fit_is_the_single_thread_fit() {
        let space = Space::new(
            [11, 9, 7],
            [0; 3],
            [1.5, 2.0, 2.5],
            [-3.0, 4.0, 1.0],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap();
        let data: Vec<[f64; 3]> = (0..space.len())
            .map(|o| {
                let v = ((o * 37) % 101) as f64 * 0.01;
                [v, -v * 0.5, v * v]
            })
            .collect();
        let field = Image::new(space.clone(), data);
        let conf: Vec<f64> = (0..space.len()).map(|o| (o % 4) as f64 * 0.5).collect();
        let conf = Image::new(space.clone(), conf);
        for (cps, order) in [([5u32, 6, 4], 3u32), ([4, 4, 3], 2), ([3, 3, 3], 1)] {
            for c in [None, Some(&conf)] {
                for stationary in [false, true] {
                    // The scattered data as field_to_bspline builds them.
                    let mut input = Scattered::default();
                    for o in 0..space.len() {
                        let idx = space.index_of(o);
                        let boundary = stationary && on_boundary(&idx, &[0; 3], &space.size);
                        let cv = c.map(|c| c.data[o]);
                        if cv.is_some_and(|v| v <= 0.0) && !boundary {
                            continue;
                        }
                        let mut w = cv.map_or(1.0, |v| if v > 0.0 { v as f32 } else { 1.0 });
                        let mut d = field.data[o];
                        if boundary {
                            d = [0.0; 3];
                            w = BOUNDARY_WEIGHT;
                        }
                        input
                            .points
                            .push(parametric_point(&space, &index_to_point_f32(&space, &idx)));
                        input.data.push(d);
                        input.weights.push(w);
                    }
                    let domain = FitDomain::of(&space);
                    let expected = sequential(&domain, &cps, order, &input);
                    for threads in [1, 3] {
                        let got = larmorx_core::parallel::with_threads(threads, || {
                            fit(&domain, &cps, order, &input).unwrap()
                        })
                        .unwrap();
                        assert_eq!(got.data, expected, "{cps:?} {order} {threads}");
                    }
                    let smooth = field_to_bspline(&field, c, &cps, order, stationary).unwrap();
                    assert_eq!(smooth.space.size, space.size);
                }
            }
        }
    }

    #[test]
    fn a_constant_field_stays_proportional_and_the_boundary_is_pinned() {
        let space = Space::new(
            [12, 10, 8],
            [0; 3],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap();
        // One level of Lee, Wolberg and Shin's approximation does not reproduce a constant
        // exactly, but it is linear in the data: the components keep their ratios.
        let field = Image::new(space.clone(), vec![[1.0f64, 2.0, -0.5]; space.len()]);
        let s = field_to_bspline(&field, None, &[8, 8, 7], 3, false).unwrap();
        for v in &s.data {
            assert!((v[1] - 2.0 * v[0]).abs() < 1e-12 && (v[2] + 0.5 * v[0]).abs() < 1e-12);
            assert!(v[0] > 0.5 && v[0] < 2.0, "{v:?}");
        }
        let s = field_to_bspline(&field, None, &[8, 8, 7], 3, true).unwrap();
        assert!(s.data[0][0].abs() < 1e-3);
    }

    #[test]
    fn too_few_control_points_are_refused() {
        let space = Space::new(
            [6, 6],
            [0; 2],
            [1.0, 1.0],
            [0.0, 0.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap();
        let field = Image::new(space.clone(), vec![[0.0f64; 2]; space.len()]);
        let e = field_to_bspline(&field, None, &[3, 4], 3, true).unwrap_err();
        assert_eq!(
            e.description,
            "The number of control points must be greater than the spline order."
        );
    }
}

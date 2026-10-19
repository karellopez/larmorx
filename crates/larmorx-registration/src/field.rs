// SPDX-License-Identifier: Apache-2.0
//! Displacement fields and the filters SyN applies to them (ITK v5.4.5,
//! `Modules/Filtering/DisplacementField`, `Modules/Core/ImageFunction`,
//! `Modules/Registration/Common`, `Modules/Registration/RegistrationMethodsv4`).
//!
//! A [`Field`] is an `Image<Vector<R, D>, D>`: one displacement (physical units, ITK's LPS
//! axes) per voxel, voxel `x` fastest, components interleaved. Every routine follows the ITK
//! class it ports operation for operation, in the class's types:
//!
//! - [`vector_linear`][]: `VectorLinearInterpolateImageFunction<Field, R>` (the `2^D`
//!   neighbours in ITK's order, overlaps in `R`, sums in double, stopping once the overlaps
//!   reach 1);
//! - [`transform_point`][]: `DisplacementFieldTransform<R, D>::TransformPoint`;
//! - [`compose`][]: `ComposeDisplacementFieldsImageFilter`;
//! - [`invert`][]: `InvertDisplacementFieldImageFilter` (the fixed-point iteration with its
//!   tolerances and boundary condition);
//! - [`gaussian_smooth`][]: `SyNImageRegistrationMethod::GaussianSmoothDisplacementField`
//!   (`GaussianOperator` along each axis through `VectorNeighborhoodOperatorImageFilter`, the
//!   boundary pinned and the result blended for variances below 0.5);
//! - [`scale_update`][]: `SyNImageRegistrationMethod::ScaleUpdateField`;
//! - [`resample_to`][]: `DisplacementFieldTransformParametersAdaptor::AdaptTransformParameters`
//!   (`ResampleImageFilter` with an identity transform and the linear interpolator).
//!
//! Each per-voxel step runs in parallel; every sum ITK accumulates over voxels is accumulated
//! in a single ITK work unit's raster order, so the result does not depend on the thread
//! count.

use std::sync::Arc;

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::interp::at_cindex_with;
use crate::{Image, Real, Space};

/// A displacement field: `Image<Vector<R, D>, D>`.
pub type Field<R, const D: usize> = Image<[R; D], D>;

/// A field of zero vectors on `space` (`AllocateInitialized`).
pub fn zero_field<R: Real, const D: usize>(space: &Space<D>) -> Field<R, D> {
    Image::new(space.clone(), vec![[R::ZERO; D]; space.len()])
}

/// `VectorLinearInterpolateImageFunction<Field, R>::EvaluateAtContinuousIndex(c)`: the base
/// index `Math::Floor` (or the exact floor, ITK-01, as `floor` picks), the distances in `R`;
/// for each of the `2^D` neighbours (bit `d` of the counter: the upper neighbour along `d`,
/// clamped to the region's end; else the lower, clamped to its start), the overlap in `R`
/// (`overlap *= 1.0 − distance` computed in double and stored in `R`); where the overlap is
/// not 0, each component adds `overlap · value` (a product in `R`) to a double sum, and the
/// loop ends once the overlaps add up to exactly 1.
#[inline]
pub fn vector_linear<R: Real, const D: usize>(
    field: &Field<R, D>,
    c: &[R; D],
    floor: &IndexFloor,
) -> [f64; D] {
    let space = &field.space;
    let mut base = [0i64; D];
    let mut distance = [R::ZERO; D];
    for d in 0..D {
        base[d] = floor.pick(c[d].itk_floor(), c[d].to_f64());
        distance[d] = c[d] - R::from_i64(base[d]);
    }
    let strides = space.strides();
    let mut output = [0.0f64; D];
    let mut total = 0.0f64;
    for counter in 0..(1usize << D) {
        let mut overlap = R::ONE;
        let mut upper = counter;
        let mut offset = 0usize;
        for d in 0..D {
            let index = if upper & 1 == 1 {
                overlap = overlap * distance[d];
                (base[d] + 1).min(space.start[d] + space.size[d] as i64 - 1)
            } else {
                overlap = R::from_f64(overlap.to_f64() * (1.0 - distance[d].to_f64()));
                base[d].max(space.start[d])
            };
            offset += (index - space.start[d]) as usize * strides[d];
            upper >>= 1;
        }
        if overlap != R::ZERO {
            let v = &field.data[offset];
            for k in 0..D {
                output[k] += (overlap * v[k]).to_f64();
            }
            total += overlap.to_f64();
        }
        if total == 1.0 {
            break;
        }
    }
    output
}

/// `DisplacementFieldTransform<R, D>::TransformPoint(p)`: inside the field's buffer (ITK's
/// `IsInsideBuffer` on the continuous index of `R`), `p + d(p)`, each coordinate computed in
/// double and stored in `R`; outside, `p` unchanged.
#[inline]
pub fn transform_point<R: Real, const D: usize>(
    field: &Field<R, D>,
    p: &[R; D],
    floor: &IndexFloor,
) -> [R; D] {
    let pf: [f64; D] = std::array::from_fn(|k| p[k].to_f64());
    let c: [R; D] = field.space.point_to_cindex(&pf);
    if !field.space.is_inside_buffer(&c) {
        return *p;
    }
    let d = vector_linear(field, &c, floor);
    std::array::from_fn(|k| R::from_f64(pf[k] + d[k]))
}

/// `ComposeDisplacementFieldsImageFilter` (`SetDisplacementField(displacement)`,
/// `SetWarpingField(warping)`): on the displacement field's grid, for each voxel `x` (its
/// physical point from the warping field's geometry), `x₂ = x + w(x)` in double; where `x₂`
/// (as a point of `R`, the interpolator's coordinate type) is inside the displacement field,
/// `u = d(x₂)` interpolated (double), else 0; the output `(x₂ + u) − x`, stored in `R`.
pub fn compose<R: Real, const D: usize>(
    displacement: &Field<R, D>,
    warping: &Field<R, D>,
    floor: &IndexFloor,
) -> Field<R, D> {
    let out_space = displacement.space.clone();
    let data: Vec<[R; D]> = (0..out_space.len())
        .into_par_iter()
        .map(|o| {
            let idx = out_space.index_of(o);
            let w = warping.at(&idx);
            let p1 = warping.space.index_to_point(&idx);
            let p2: [f64; D] = std::array::from_fn(|k| p1[k] + w[k].to_f64());
            // The interpolator's PointType is Point<R>: x₂ converted.
            let p2r: [f64; D] = std::array::from_fn(|k| R::from_f64(p2[k]).to_f64());
            let c: [R; D] = displacement.space.point_to_cindex(&p2r);
            let u = if displacement.space.is_inside_buffer(&c) {
                vector_linear(displacement, &c, floor)
            } else {
                [0.0; D]
            };
            std::array::from_fn(|k| R::from_f64((p2[k] + u[k]) - p1[k]))
        })
        .collect();
    Image::new(out_space, data)
}

/// `InvertDisplacementFieldImageFilter`'s settings as `SyNImageRegistrationMethod` sets them.
#[derive(Clone, Copy, Debug)]
pub struct InvertSettings<R> {
    pub maximum_iterations: u32,
    pub mean_error_tolerance: R,
    pub max_error_tolerance: R,
    pub enforce_boundary_condition: bool,
}

impl<R: Real> InvertSettings<R> {
    /// `SyNImageRegistrationMethod::InvertDisplacementField`: 20 iterations, mean error
    /// 0.001, max error 0.1, boundary pinned (the filter's default).
    pub fn syn() -> Self {
        InvertSettings {
            maximum_iterations: 20,
            mean_error_tolerance: R::from_f64(0.001),
            max_error_tolerance: R::from_f64(0.1),
            enforce_boundary_condition: true,
        }
    }
}

/// Whether `idx` is on the field's boundary as `GaussianSmoothDisplacementField` and the
/// inverse filter test it: an axis at its start, or at `size − start − 1` (the last index
/// when the region starts at 0, as every field here does).
#[inline]
fn on_boundary<const D: usize>(space: &Space<D>, idx: &[i64; D]) -> bool {
    (0..D).any(|d| idx[d] == space.start[d] || idx[d] == space.size[d] as i64 - space.start[d] - 1)
}

/// `InvertDisplacementFieldImageFilter::GenerateData` for `field`, starting from `estimate`
/// (a copy of it; zeros without one), on the estimate's grid (the field's without one).
///
/// Each iteration composes the field with the current inverse ([`compose`], the inverse as
/// the warping field), takes the negated result `e` and its norm in voxels
/// `|e / spacing|` (summed in `R`, the root in `R`), their mean (a sum over the voxels in
/// raster order, divided by their number) and maximum; then every voxel moves by
/// `ε·e`, `e` first scaled down to `ε·max/|e|` where its norm exceeds `ε·max`, with
/// `ε = 0.75` at the first iteration and 0.5 after, and the boundary set to 0. It stops after
/// `maximum_iterations`, or when the maximum is at most the max-error tolerance or the mean
/// at most the mean-error tolerance (tested before each iteration).
pub fn invert<R: Real, const D: usize>(
    field: &Field<R, D>,
    estimate: Option<&Field<R, D>>,
    settings: &InvertSettings<R>,
    floor: &IndexFloor,
) -> Field<R, D> {
    let mut inverse = match estimate {
        Some(e) => e.clone(),
        None => zero_field(&field.space),
    };
    let space = inverse.space.clone();
    let n = space.len();
    let inverse_spacing: [R; D] =
        std::array::from_fn(|d| R::from_f64(1.0 / field.space.spacing[d]));
    let mut max_error = R::MAX;
    let mut mean_error = R::MAX;
    let mut iteration = 0u32;
    loop {
        let go = iteration < settings.maximum_iterations
            && max_error > settings.max_error_tolerance
            && mean_error > settings.mean_error_tolerance;
        iteration += 1;
        if !go {
            break;
        }
        let composed = compose(field, &inverse, floor);
        // The negated composition and its norm in voxels.
        let norms: Vec<R> = composed
            .data
            .par_iter()
            .map(|e| {
                let mut s = R::ZERO;
                for d in 0..D {
                    let v = e[d] * inverse_spacing[d];
                    s = s + v * v;
                }
                s.sqrt()
            })
            .collect();
        let mut local_mean = R::ZERO;
        let mut local_max = R::ZERO;
        for &s in &norms {
            local_mean = local_mean + s;
            if local_max < s {
                local_max = s;
            }
        }
        mean_error = R::ZERO + local_mean;
        max_error = R::ZERO;
        if max_error < local_max {
            max_error = local_max;
        }
        mean_error = mean_error / R::from_i64(n as i64);
        let epsilon = if iteration == 1 {
            R::from_f64(0.75)
        } else {
            R::from_f64(0.5)
        };
        let bound = epsilon * max_error;
        inverse
            .data
            .par_iter_mut()
            .enumerate()
            .for_each(|(o, inv)| {
                let e = composed.data[o];
                let mut update: [R; D] = std::array::from_fn(|d| -e[d]);
                let scaled = norms[o];
                if scaled > bound {
                    let factor = epsilon * max_error / scaled;
                    for u in &mut update {
                        *u = *u * factor;
                    }
                }
                *inv = std::array::from_fn(|d| inv[d] + update[d] * epsilon);
                if settings.enforce_boundary_condition && on_boundary(&space, &space.index_of(o)) {
                    *inv = [R::ZERO; D];
                }
            });
    }
    inverse
}

/// `GaussianOperator<R, D>` along an axis: [`larmorx_image::discrete_gaussian::gaussian_kernel`]
/// (variance in voxels², maximum error, maximum width), stored in `R`.
fn gaussian_operator<R: Real>(variance: R, maximum_error: f64, maximum_width: usize) -> Vec<R> {
    larmorx_image::discrete_gaussian::gaussian_kernel(
        variance.to_f64(),
        maximum_error,
        maximum_width,
    )
    .into_iter()
    .map(R::from_f64)
    .collect()
}

/// `VectorNeighborhoodOperatorImageFilter` with a directional operator along `axis`
/// (zero-flux Neumann boundary: the nearest voxel of the region): each component the inner
/// product of the kernel with the voxels along the axis, summed in `R` from the kernel's first
/// tap.
fn convolve_axis<R: Real, const D: usize>(
    field: &Field<R, D>,
    axis: usize,
    kernel: &[R],
) -> Field<R, D> {
    let space = &field.space;
    let radius = (kernel.len() / 2) as i64;
    let stride = space.strides()[axis];
    let size = space.size[axis] as i64;
    let data: Vec<[R; D]> = (0..space.len())
        .into_par_iter()
        .map(|o| {
            let i = ((o / stride) % size as usize) as i64;
            let line_start = o - i as usize * stride;
            let mut sum = [R::ZERO; D];
            for (k, &c) in kernel.iter().enumerate() {
                let j = (i + k as i64 - radius).clamp(0, size - 1) as usize;
                let v = &field.data[line_start + j * stride];
                for d in 0..D {
                    sum[d] = sum[d] + c * v[d];
                }
            }
            sum
        })
        .collect();
    Image::new(space.clone(), data)
}

/// `SyNImageRegistrationMethod::GaussianSmoothDisplacementField(field, variance)`: a copy for
/// a variance ≤ 0; else the field smoothed along each axis in turn by `GaussianOperator`
/// (maximum error 0.001, maximum width the field's size along the axis), then every boundary
/// voxel set to 0 and the others blended `s·w₁ + f·w₂`, with `w₁ = 1 − variance/0.5` below a
/// variance of 0.5 (1 above) and `w₂ = 1 − w₁`.
pub fn gaussian_smooth<R: Real, const D: usize>(field: &Field<R, D>, variance: R) -> Field<R, D> {
    if variance <= R::ZERO {
        return field.clone();
    }
    let mut smooth = field.clone();
    for axis in 0..D {
        let kernel = gaussian_operator(variance, 0.001, field.space.size[axis]);
        smooth = convolve_axis(&smooth, axis, &kernel);
    }
    let mut weight1 = R::ONE;
    if variance < R::from_f64(0.5) {
        weight1 = R::from_f64(1.0 - 1.0 * (variance.to_f64() / 0.5));
    }
    let weight2 = R::ONE - weight1;
    let space = field.space.clone();
    smooth.data.par_iter_mut().enumerate().for_each(|(o, s)| {
        if on_boundary(&space, &space.index_of(o)) {
            *s = [R::ZERO; D];
        } else {
            let f = field.data[o];
            *s = std::array::from_fn(|d| s[d] * weight1 + f[d] * weight2);
        }
    });
    smooth
}

/// `SyNImageRegistrationMethod::ScaleUpdateField(field)`: the largest norm in voxels
/// (`Σ (v/spacing)²` with each quotient in double, accumulated in `R`, the root in `R`), and
/// every vector multiplied by `learning_rate / max` (by `learning_rate` if the maximum is not
/// positive), in `R`.
pub fn scale_update<R: Real, const D: usize>(field: &Field<R, D>, learning_rate: R) -> Field<R, D> {
    let spacing = field.space.spacing;
    let norms: Vec<R> = field
        .data
        .par_iter()
        .map(|v| {
            let mut local = R::ZERO;
            for d in 0..D {
                let q = v[d].to_f64() / spacing[d];
                local = R::from_f64(local.to_f64() + q * q);
            }
            local.sqrt()
        })
        .collect();
    let mut max_norm = -R::MAX;
    for &n in &norms {
        if n > max_norm {
            max_norm = n;
        }
    }
    let mut scale = learning_rate;
    if max_norm > R::ZERO {
        scale = scale / max_norm;
    }
    let data = field
        .data
        .par_iter()
        .map(|v| std::array::from_fn(|d| v[d] * scale))
        .collect();
    Image::new(field.space.clone(), data)
}

/// `CastComponentWithBoundsChecking` of a double component into `R` (clamped to the float
/// range for float components).
#[inline]
fn cast_component<R: Real>(v: f64) -> R {
    if R::PRECISION == larmorx_core::Precision::Double {
        return R::from_f64(v);
    }
    let max = R::MAX.to_f64();
    if v <= -max {
        -R::MAX
    } else if v >= max {
        R::MAX
    } else {
        R::from_f64(v)
    }
}

/// `ResampleImageFilter<Field, Field, R>` with an identity transform and
/// `LinearInterpolateImageFunction<Field, R>` onto `output` (start index 0, as
/// `SetSize` gives): the scan-line path (`LinearThreadedGenerateData`), each component
/// interpolated as the scalar interpolator does it, cast with bounds checking; zero vectors
/// outside the input.
pub fn resample_to<R: Real, const D: usize>(
    field: &Field<R, D>,
    output: &Space<D>,
    floor: &IndexFloor,
) -> Field<R, D> {
    let n0 = output.size[0];
    if n0 == 0 || output.is_empty() {
        return Image::new(output.clone(), Vec::new());
    }
    let first = output.start[0];
    let size0 = n0 as f64;
    let transform_index = |idx: &[i64; D]| -> [R; D] {
        let p = output.index_to_point(idx);
        let pr: [f64; D] = std::array::from_fn(|k| R::from_f64(p[k]).to_f64());
        field.space.point_to_cindex::<R>(&pr)
    };
    let input = &field.space;
    let lines = output.len() / n0;
    let rows: Vec<Vec<[R; D]>> = (0..lines)
        .into_par_iter()
        .map(|line| {
            let mut index = output.index_of(line * n0);
            index[0] = first;
            let start = transform_index(&index);
            index[0] = first + n0 as i64;
            let end = transform_index(&index);
            let vector: [R; D] = std::array::from_fn(|k| end[k] - start[k]);
            (0..n0)
                .map(|x| {
                    let alpha = x as f64 / size0;
                    let c: [R; D] = std::array::from_fn(|k| {
                        R::from_f64(start[k].to_f64() + alpha * vector[k].to_f64())
                    });
                    if !input.is_inside_buffer(&c) {
                        return [R::ZERO; D];
                    }
                    std::array::from_fn(|k| {
                        cast_component(at_cindex_with(input, &c, floor, |o| {
                            field.data[o][k].to_f64()
                        }))
                    })
                })
                .collect()
        })
        .collect();
    Image::new(output.clone(), rows.into_iter().flatten().collect())
}

/// `DisplacementFieldTransform<R, D>` as SyN and ANTs use it: the displacement field and,
/// for an invertible transform, its inverse field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldXfm<R, const D: usize> {
    pub field: Arc<Field<R, D>>,
    pub inverse: Option<Arc<Field<R, D>>>,
    /// The floor of the interpolator's base index: the exact one (ITK-01 corrected), or
    /// ITK's.
    pub exact_floor: bool,
}

impl<R: Real, const D: usize> FieldXfm<R, D> {
    pub fn new(field: Field<R, D>, inverse: Option<Field<R, D>>, exact_floor: bool) -> Self {
        FieldXfm {
            field: Arc::new(field),
            inverse: inverse.map(Arc::new),
            exact_floor,
        }
    }

    /// The interpolator's floor.
    #[inline]
    pub fn floor(&self) -> IndexFloor {
        if self.exact_floor {
            IndexFloor::new(larmorx_core::defects::UpstreamDefects::Fix)
        } else {
            IndexFloor::itk()
        }
    }

    /// `GetInverseTransform()`: the fields swapped, or `None` without an inverse field
    /// (`GetInverse` returns false).
    pub fn inverse_transform(&self) -> Option<Self> {
        self.inverse.as_ref().map(|inv| FieldXfm {
            field: Arc::clone(inv),
            inverse: Some(Arc::clone(&self.field)),
            exact_floor: self.exact_floor,
        })
    }

    /// `TransformPoint`.
    #[inline]
    pub fn transform_point(&self, p: &[R; D]) -> [R; D] {
        transform_point(&self.field, p, &self.floor())
    }

    /// `GetFixedParameters()`: size, origin, spacing, direction (row by row).
    pub fn fixed_parameters(&self) -> Vec<f64> {
        let s = &self.field.space;
        let mut out: Vec<f64> = s.size.iter().map(|&n| n as f64).collect();
        out.extend_from_slice(&s.origin);
        out.extend_from_slice(&s.spacing);
        out.extend(s.direction.iter().flatten());
        out
    }

    /// `ComputeJacobianWithRespectToPosition(point)`: at the voxel nearest `p`
    /// (`TransformPhysicalPointToIndex`), the identity plus the field's gradient by fourth-order
    /// central differences along each index axis (`(f₋₂ − 8f₋₁ + 8f₊₁ − f₊₂)/(12·spacing)`,
    /// the outer neighbours clamped to the region), each component's gradient turned by the
    /// field's direction; the identity on and outside the region's border, or where a
    /// difference is not finite.
    pub fn jacobian_position(&self, p: &[R; D]) -> [[R; D]; D] {
        self.jacobian_position_internal(p, false)
    }

    /// `ComputeInverseJacobianWithRespectToPosition(point)`: as
    /// [`FieldXfm::jacobian_position`] with each difference negated (`dPixSign = −1`), the
    /// first-order approximation `I − ∇u` of the inverse.
    pub fn inverse_jacobian_position(&self, p: &[R; D]) -> [[R; D]; D] {
        self.jacobian_position_internal(p, true)
    }

    /// `ComputeJacobianWithRespectToPositionInternal(index, jacobian, doInverseJacobian)`.
    fn jacobian_position_internal(&self, p: &[R; D], inverse: bool) -> [[R; D]; D] {
        let sign = if inverse { -R::ONE } else { R::ONE };
        let identity: [[R; D]; D] =
            std::array::from_fn(|i| std::array::from_fn(|j| if i == j { R::ONE } else { R::ZERO }));
        let field = &self.field;
        let s = &field.space;
        let pf: [f64; D] = std::array::from_fn(|k| p[k].to_f64());
        let index = s.point_to_index(&pf);
        for i in 0..D {
            let upper = s.start[i] + s.size[i] as i64 - 1;
            if index[i] <= s.start[i] || index[i] >= upper {
                return identity;
            }
        }
        let mut jacobian = [[R::ZERO; D]; D];
        let space_r = R::ONE;
        for col in 0..D {
            let mut dif = [index; 4];
            dif[0][col] -= 2;
            dif[1][col] -= 1;
            dif[2][col] += 1;
            dif[3][col] += 2;
            let upper = s.start[col] + s.size[col] as i64 - 1;
            dif[0][col] = dif[0][col].max(s.start[col]);
            dif[3][col] = dif[3][col].min(upper);
            let pix: [[R; D]; 4] = std::array::from_fn(|i| field.at(&dif[i]));
            // (pixDisp[0] - pixDisp[1]*8.0 + pixDisp[2]*8.0 - pixDisp[3]) / (12.0*space*spacing)
            // * dPixSign: `Vector<R>` times or over a double computes in double and stores in
            // `R`; the sums and the sign are in `R`.
            let denom = 12.0 * space_r.to_f64() * s.spacing[col];
            for row in 0..D {
                let a = pix[0][row] - R::from_f64(pix[1][row].to_f64() * 8.0);
                let b = a + R::from_f64(pix[2][row].to_f64() * 8.0);
                let c = b - pix[3][row];
                let d = R::from_f64(c.to_f64() / denom) * sign;
                jacobian[row][col] = d;
                if !d.to_f64().is_finite() {
                    return identity;
                }
            }
        }
        for row in jacobian.iter_mut() {
            let local: [f64; D] = std::array::from_fn(|k| row[k].to_f64());
            let physical = s.local_to_physical_vector(&local);
            *row = std::array::from_fn(|k| R::from_f64(physical[k]));
        }
        for (i, row) in jacobian.iter_mut().enumerate() {
            row[i] = R::from_f64(row[i].to_f64() + 1.0);
        }
        jacobian
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space2(nx: usize, ny: usize, spacing: f64) -> Space<2> {
        Space::new(
            [nx, ny],
            [0, 0],
            [spacing, spacing],
            [1.0, -2.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap()
    }

    #[test]
    fn interpolation_reproduces_voxel_values_and_is_linear() {
        let space = space2(5, 4, 2.0);
        let data: Vec<[f64; 2]> = (0..space.len())
            .map(|o| {
                let idx = space.index_of(o);
                [idx[0] as f64, 2.0 * idx[1] as f64]
            })
            .collect();
        let f = Image::new(space, data);
        let floor = IndexFloor::itk();
        assert_eq!(vector_linear(&f, &[2.0, 1.0], &floor), [2.0, 2.0]);
        assert_eq!(vector_linear(&f, &[2.5, 1.5], &floor), [2.5, 3.0]);
        // Clamped beyond the last voxel.
        assert_eq!(vector_linear(&f, &[4.4, 0.0], &floor), [4.0, 0.0]);
    }

    #[test]
    fn identity_composition_and_inverse_of_zero() {
        let space = space2(6, 6, 1.0);
        let z = zero_field::<f64, 2>(&space);
        let floor = IndexFloor::itk();
        assert_eq!(compose(&z, &z, &floor), z);
        let inv = invert(&z, None, &InvertSettings::syn(), &floor);
        assert_eq!(inv, z);
    }

    #[test]
    fn smoothing_pins_the_boundary() {
        let space = space2(7, 7, 1.0);
        let f = Image::new(space.clone(), vec![[1.0f64, -1.0]; space.len()]);
        let s = gaussian_smooth(&f, 3.0);
        assert_eq!(s.at(&[0, 3]), [0.0, 0.0]);
        let c = s.at(&[3, 3]);
        assert!((c[0] - 1.0).abs() < 1e-12 && (c[1] + 1.0).abs() < 1e-12);
        assert_eq!(gaussian_smooth(&f, 0.0), f);
    }

    #[test]
    fn scaling_normalises_the_largest_vector() {
        let space = space2(3, 1, 2.0);
        let f = Image::new(space, vec![[4.0f64, 0.0], [0.0, 2.0], [0.0, 0.0]]);
        let s = scale_update(&f, 0.5);
        assert_eq!(s.data[0], [1.0, 0.0]);
        assert_eq!(s.data[1], [0.0, 0.5]);
    }
}

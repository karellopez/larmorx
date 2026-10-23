// SPDX-License-Identifier: Apache-2.0
//! The exponential (stationary velocity field) transforms of ANTs' `Exponential` and
//! `BSplineExponential` stages (ITK v5.4.5, `Modules/Filtering/DisplacementField`:
//! `itkExponentialDisplacementFieldImageFilter.hxx`, `itkConstantVelocityFieldTransform.hxx`,
//! `itkGaussianExponentialDiffeomorphicTransform.hxx`,
//! `itkBSplineExponentialDiffeomorphicTransform.hxx`; `Modules/Filtering/ImageGrid`:
//! `itkWarpVectorImageFilter.hxx`; `Modules/Core/ImageFunction`:
//! `itkVectorLinearInterpolateNearestNeighborExtrapolateImageFunction.hxx`;
//! `Modules/Registration/Common`: `itkConstantVelocityFieldTransformParametersAdaptor.hxx`,
//! `itkGaussianExponentialDiffeomorphicTransformParametersAdaptor.hxx`,
//! `itkBSplineExponentialDiffeomorphicTransformParametersAdaptor.hxx`).
//!
//! A constant velocity field `v` defines the displacement `exp(v)`, computed by scaling and
//! squaring ([`exponentiate`]): `v / 2^n`, then `n` times `u ← u + u∘(x + u)` (the field warped
//! by itself, linearly interpolated, extrapolated with the nearest voxel). Its inverse is
//! `exp(−v)`, the same with `−v / 2^n`. The transform's update ([`update`]) smooths the update
//! field (a Gaussian, or a B-spline approximation), adds it to the velocity field, smooths the
//! velocity field the same way and integrates again.
//!
//! Every voxel is computed on its own, so the result does not depend on the thread count.

use std::sync::Arc;

use larmorx_core::{IndexFloor, ItkReal as _};
use rayon::prelude::*;

use crate::field::{Field, gaussian_smooth, resample_to};
use crate::{Image, ItkException, Real, Space};

/// How a constant velocity field is integrated (`ConstantVelocityFieldTransform`'s
/// `m_CalculateNumberOfIntegrationStepsAutomatically` and `m_NumberOfIntegrationSteps`): ANTs
/// asks for the automatic number when the stage's `numberOfIntegrationSteps` is 0 (or not
/// given), else that number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Integration {
    pub automatic: bool,
    pub steps: u32,
}

impl Integration {
    /// ANTs' setting for a stage's `numberOfIntegrationSteps`.
    pub fn for_stage(steps: u32) -> Self {
        if steps == 0 {
            Integration {
                automatic: true,
                // ConstantVelocityFieldTransform's default.
                steps: 10,
            }
        } else {
            Integration {
                automatic: false,
                steps,
            }
        }
    }
}

/// `ExponentialDisplacementFieldImageFilter`'s automatic number of iterations: the largest
/// squared norm of the field's vectors (each in double, summed in double), divided by the
/// smallest spacing squared, gives `2 + ½·log(max)/ln 2` (0's smallest positive double for a
/// zero field), whose integer part plus one is the number of iterations, at most `maximum`;
/// a negative value gives 0. An infinite norm makes the conversion to an integer undefined in
/// C++: x86-64 gives 0 (`reproduce`); larmorx clamps it to the maximum.
pub fn automatic_iterations<R: Real, const D: usize>(
    field: &Field<R, D>,
    maximum: u32,
    reproduce: bool,
) -> u32 {
    let mut maxnorm2 = 0.0f64;
    for v in &field.data {
        let mut norm2 = 0.0f64;
        for c in v {
            let value = c.to_f64();
            norm2 += value * value;
        }
        if norm2 > maxnorm2 {
            maxnorm2 = norm2;
        }
    }
    let mut minpixelspacing = field.space.spacing[0];
    for &s in &field.space.spacing[1..] {
        if s < minpixelspacing {
            minpixelspacing = s;
        }
    }
    maxnorm2 /= minpixelspacing * minpixelspacing;
    let numiterfloat = if maxnorm2 > 0.0 {
        2.0 + 0.5 * larmorx_core::math::log(maxnorm2) / std::f64::consts::LN_2
    } else {
        f64::MIN_POSITIVE
    };
    if numiterfloat >= 0.0 {
        let x = numiterfloat + 1.0;
        let n = if x < 4_294_967_296.0 {
            x as u32
        } else if reproduce {
            // cvttsd2si's 64-bit indefinite value, of which the low 32 bits are 0.
            0
        } else {
            u32::MAX
        };
        n.min(maximum)
    } else {
        0
    }
}

/// `VectorLinearInterpolateNearestNeighborExtrapolateImageFunction<Field, double>` at a
/// continuous index (double): per axis the base index `Math::Floor` (the exact floor where
/// `floor` corrects ITK-01), moved to the region's first or last index with a distance of 0
/// when outside it; then the `2^D` neighbours (no clamping: an upper neighbour past the end has
/// an overlap of 0), each overlap in double, the values added in double, until the overlaps add
/// up to exactly 1.
#[inline]
pub fn vector_linear_extrapolate<R: Real, const D: usize>(
    field: &Field<R, D>,
    c: &[f64; D],
    floor: &IndexFloor,
) -> [f64; D] {
    let space = &field.space;
    let mut base = [0i64; D];
    let mut distance = [0.0f64; D];
    for d in 0..D {
        let start = space.start[d];
        let end = start + space.size[d] as i64 - 1;
        let b = floor.pick(c[d].itk_floor(), c[d]);
        if b >= start {
            if b < end {
                base[d] = b;
                distance[d] = c[d] - b as f64;
            } else {
                base[d] = end;
                distance[d] = 0.0;
            }
        } else {
            base[d] = start;
            distance[d] = 0.0;
        }
    }
    let strides = space.strides();
    let mut output = [0.0f64; D];
    let mut total = 0.0f64;
    for counter in 0..(1usize << D) {
        let mut overlap = 1.0f64;
        let mut upper = counter;
        let mut index = [0i64; D];
        for d in 0..D {
            if upper & 1 == 1 {
                index[d] = base[d] + 1;
                overlap *= distance[d];
            } else {
                index[d] = base[d];
                overlap *= 1.0 - distance[d];
            }
            upper >>= 1;
        }
        if overlap != 0.0 {
            let mut offset = 0usize;
            for d in 0..D {
                offset += (index[d] - space.start[d]) as usize * strides[d];
            }
            let v = &field.data[offset];
            for k in 0..D {
                output[k] += overlap * v[k].to_f64();
            }
            total += overlap;
        }
        if total == 1.0 {
            break;
        }
    }
    output
}

/// One squaring of `ExponentialDisplacementFieldImageFilter`: `WarpVectorImageFilter` of the
/// field by itself (each voxel's physical point plus its vector, interpolated by
/// [`vector_linear_extrapolate`], stored in `R`), added to the field in place
/// (`AddImageFilter`).
fn square<R: Real, const D: usize>(field: &Field<R, D>, floor: &IndexFloor) -> Field<R, D> {
    let space = &field.space;
    let data: Vec<[R; D]> = (0..space.len())
        .into_par_iter()
        .with_min_len(256)
        .map(|o| {
            let idx = space.index_of(o);
            let mut point = space.index_to_point(&idx);
            let u = field.data[o];
            for j in 0..D {
                point[j] += u[j].to_f64();
            }
            let c: [f64; D] = space.point_to_cindex(&point);
            let w = vector_linear_extrapolate(field, &c, floor);
            std::array::from_fn(|k| u[k] + R::from_f64(w[k]))
        })
        .collect();
    Image::new(space.clone(), data)
}

/// `ExponentialDisplacementFieldImageFilter` (`ComputeInverse` given by `inverse`): the
/// number of iterations `n` (automatic, or `maximum`), the velocity divided by `±2^n` (each
/// component's quotient in double, stored in `R`; for `n` = 0 a copy, or the field negated),
/// then `n` squarings.
pub fn exponentiate<R: Real, const D: usize>(
    velocity: &Field<R, D>,
    integration: Integration,
    inverse: bool,
    floor: &IndexFloor,
    reproduce: bool,
) -> Field<R, D> {
    let numiter = if integration.automatic {
        // m_MaximumNumberOfIterations keeps its default, 20.
        automatic_iterations(velocity, 20, reproduce)
    } else {
        integration.steps
    };
    let divisor = if numiter == 0 {
        if inverse { -1.0 } else { 1.0 }
    } else {
        let d = f64::from(1u32 << numiter.min(31));
        if inverse { -d } else { d }
    };
    let mut field = Image::new(
        velocity.space.clone(),
        velocity
            .data
            .par_iter()
            .map(|v| std::array::from_fn(|k| R::from_f64(v[k].to_f64() / divisor)))
            .collect(),
    );
    for _ in 0..numiter {
        field = square(&field, floor);
    }
    field
}

/// `ConstantVelocityFieldTransform::IntegrateVelocityField` with the time bounds `[0, 1]`: the
/// displacement field `exp(v)` and its inverse `exp(−v)`.
pub fn integrate<R: Real, const D: usize>(
    velocity: &Field<R, D>,
    integration: Integration,
    floor: &IndexFloor,
    reproduce: bool,
) -> (Field<R, D>, Field<R, D>) {
    let forward = exponentiate(velocity, integration, false, floor, reproduce);
    let inverse = exponentiate(velocity, integration, true, floor, reproduce);
    (forward, inverse)
}

/// How an exponential transform smooths (`GaussianExponentialDiffeomorphicTransform`,
/// `BSplineExponentialDiffeomorphicTransform`).
#[derive(Clone, Debug, PartialEq)]
pub enum ExponentialSmoothing {
    /// Variances in voxels² of the update field and of the velocity field (each skipped when
    /// ≤ 0).
    Gaussian {
        update_variance: f64,
        velocity_variance: f64,
    },
    /// The B-spline approximations' order and numbers of control points per axis (a field is
    /// smoothed only when every axis has more control points than the order).
    BSpline {
        order: u32,
        update_control_points: Vec<u32>,
        velocity_control_points: Vec<u32>,
    },
}

/// The state of an exponential transform: its velocity field and how it is integrated and
/// smoothed.
#[derive(Clone, Debug, PartialEq)]
pub struct Exponential<R, const D: usize> {
    pub velocity: Arc<Field<R, D>>,
    pub integration: Integration,
    pub smoothing: ExponentialSmoothing,
}

impl<R: Real, const D: usize> Exponential<R, D> {
    /// `GetNameOfClass()`.
    pub fn class(&self) -> &'static str {
        match self.smoothing {
            ExponentialSmoothing::Gaussian { .. } => "GaussianExponentialDiffeomorphicTransform",
            ExponentialSmoothing::BSpline { .. } => "BSplineExponentialDiffeomorphicTransform",
        }
    }

    /// `UpdateTransformParameters(update, factor)`: the update field smoothed (the Gaussian in
    /// place, in the caller's array, as ITK smooths the imported buffer; the B-spline into a
    /// new field), times `factor`, added to the velocity field, which is then smoothed alike;
    /// returns the new velocity field. The caller integrates it.
    pub fn update(&mut self, update: &mut [R], factor: R) -> Result<(), ItkException> {
        let space = self.velocity.space.clone();
        let n = space.len();
        if update.len() < D * n {
            return Err(ItkException::new(
                "Modules/Filtering/DisplacementField/include/itkGaussianExponentialDiffeomorphicTransform.hxx",
                64,
                self.class(),
                format!(
                    "Parameter update size, {}, must  be same as transform parameter size, {}\n",
                    update.len(),
                    D * n
                ),
            ));
        }
        let as_field = |u: &[R]| -> Field<R, D> {
            Image::new(space.clone(), u.as_chunks::<D>().0[..n].to_vec())
        };
        let update_field: Field<R, D> = match &self.smoothing {
            ExponentialSmoothing::Gaussian {
                update_variance, ..
            } => {
                if *update_variance > 0.0 {
                    let smoothed =
                        gaussian_smooth(&as_field(update), R::from_f64(*update_variance));
                    for (u, v) in update.as_chunks_mut::<D>().0.iter_mut().zip(&smoothed.data) {
                        *u = *v;
                    }
                    smoothed
                } else {
                    as_field(update)
                }
            }
            ExponentialSmoothing::BSpline {
                order,
                update_control_points,
                ..
            } => {
                let field = as_field(update);
                if update_control_points.iter().all(|&c| c > *order) {
                    let cps: [u32; D] = std::array::from_fn(|d| update_control_points[d]);
                    crate::bspline_field::field_to_bspline(&field, None, &cps, *order, true)?
                } else {
                    field
                }
            }
        };
        // MultiplyImageFilter by the constant factor, then AddImageFilter.
        let data: Vec<[R; D]> = self
            .velocity
            .data
            .iter()
            .zip(&update_field.data)
            .map(|(v, u)| std::array::from_fn(|k| v[k] + u[k] * factor))
            .collect();
        let updated = Image::new(space.clone(), data);
        let smoothed = match &self.smoothing {
            ExponentialSmoothing::Gaussian {
                velocity_variance, ..
            } => {
                if *velocity_variance > 0.0 {
                    gaussian_smooth(&updated, R::from_f64(*velocity_variance))
                } else {
                    updated
                }
            }
            ExponentialSmoothing::BSpline {
                order,
                velocity_control_points,
                ..
            } => {
                if velocity_control_points.iter().all(|&c| c > *order) {
                    let cps: [u32; D] = std::array::from_fn(|d| velocity_control_points[d]);
                    crate::bspline_field::field_to_bspline(&updated, None, &cps, *order, true)?
                } else {
                    updated
                }
            }
        };
        self.velocity = Arc::new(smoothed);
        Ok(())
    }

    /// `ConstantVelocityFieldTransformParametersAdaptor::AdaptTransformParameters`: the velocity
    /// field resampled onto `space` (`ResampleImageFilter` with an identity transform and the
    /// linear interpolator, from index 0) unless the transform's fixed parameters (its field's
    /// geometry) already equal the required ones. Returns whether it resampled (the caller then
    /// integrates).
    pub fn adapt(&mut self, space: &Space<D>, floor: &IndexFloor) -> bool {
        let s = &self.velocity.space;
        if s.size == space.size
            && s.origin == space.origin
            && s.spacing == space.spacing
            && s.direction == space.direction
        {
            return false;
        }
        let target = Space::new(
            space.size,
            [0; D],
            space.spacing,
            space.origin,
            space.direction,
        )
        .expect("the virtual domain's geometry");
        self.velocity = Arc::new(resample_to(&self.velocity, &target, floor));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space3(n: usize, spacing: f64) -> Space<3> {
        Space::new(
            [n, n + 1, n - 1],
            [0; 3],
            [spacing; 3],
            [-3.0, 2.0, 1.0],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap()
    }

    #[test]
    fn a_zero_field_takes_one_iteration_and_stays_zero() {
        let space = space3(6, 2.0);
        let v = crate::field::zero_field::<f64, 3>(&space);
        assert_eq!(automatic_iterations(&v, 20, false), 1);
        let (f, i) = integrate(&v, Integration::for_stage(0), &IndexFloor::itk(), false);
        assert!(
            f.data
                .iter()
                .chain(&i.data)
                .all(|x| x.iter().all(|&c| c == 0.0))
        );
    }

    #[test]
    fn a_uniform_velocity_gives_its_translation_inside() {
        // A constant velocity is a translation: exp(v) = v away from the edges, where the
        // extrapolation keeps the field constant anyway.
        let space = space3(8, 1.0);
        let v = Image::new(space.clone(), vec![[0.75f64, -0.25, 0.5]; space.len()]);
        for steps in [0, 3, 6] {
            let (f, inv) = integrate(&v, Integration::for_stage(steps), &IndexFloor::itk(), false);
            for (a, b) in f.data.iter().zip(&inv.data) {
                for k in 0..3 {
                    assert!((a[k] - v.data[0][k]).abs() < 1e-12, "{a:?}");
                    assert!((b[k] + v.data[0][k]).abs() < 1e-12, "{b:?}");
                }
            }
        }
    }

    #[test]
    fn exp_and_its_inverse_compose_to_the_identity_for_a_smooth_field() {
        let space = space3(14, 1.0);
        let data: Vec<[f64; 3]> = (0..space.len())
            .map(|o| {
                let i = space.index_of(o);
                let x = i[0] as f64 / 13.0;
                let y = i[1] as f64 / 14.0;
                [0.6 * x * (1.0 - x), 0.4 * y * (1.0 - y) * x, -0.3 * x * y]
            })
            .collect();
        let v = Image::new(space.clone(), data);
        let floor = IndexFloor::itk();
        let (f, inv) = integrate(&v, Integration::for_stage(0), &floor, false);
        let composed = crate::field::compose(&f, &inv, &floor);
        let worst = composed
            .data
            .iter()
            .flat_map(|c| c.iter())
            .fold(0.0f64, |m, &c| m.max(c.abs()));
        assert!(worst < 0.02, "{worst}");
    }

    #[test]
    fn the_automatic_count_follows_the_norm_and_the_spacing() {
        let space = space3(5, 2.0);
        // |v|² / spacing² = 16 / 4 = 4: 2 + ½·log2(4) = 3, plus one: 4.
        let v = Image::new(space.clone(), vec![[4.0f64, 0.0, 0.0]; space.len()]);
        assert_eq!(automatic_iterations(&v, 20, false), 4);
        // Small vectors: a negative count, no iteration.
        let v = Image::new(space.clone(), vec![[1e-4f64, 0.0, 0.0]; space.len()]);
        assert_eq!(automatic_iterations(&v, 20, false), 0);
        let v = Image::new(space.clone(), vec![[f64::INFINITY, 0.0, 0.0]; space.len()]);
        assert_eq!(automatic_iterations(&v, 20, true), 0);
        assert_eq!(automatic_iterations(&v, 20, false), 20);
    }
}

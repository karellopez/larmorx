// SPDX-License-Identifier: Apache-2.0
//! `DemonsImageToImageMetricv4` (ITK v5.4.5, `itkDemonsImageToImageMetricv4.hxx`,
//! `itkDemonsImageToImageMetricv4GetValueAndDerivativeThreader.hxx`): ANTs' `Demons`, for
//! displacement field transforms only.
//!
//! The gradient comes from the **fixed** image (the metric's default source). With
//! `s = f − m` and `K` the mean of the fixed image's squared spacings (`m_Normalizer`), a
//! point's value is `s²` and its derivative `s·∇f / (s²/K + |∇f|²)`, or 0 where `|s|` is below
//! 0.001 or the denominator below 1e-9; everything in `R`. The value is the mean of `s²` over
//! the valid points; the derivatives are stored per voxel (local support).

use larmorx_core::IndexFloor;

use super::mean_squares::{BaseReduction, Reduced};
use super::{GradientSource, ImageMetricv4};
use crate::transform::Composite;
use crate::{ItkException, Real};

/// `Initialize`: a moving transform that is not a displacement field throws; else the
/// normaliser `Σ spacing² / D` of the fixed image (each square in double, accumulated in `R`).
pub fn normalizer<R: Real, const D: usize>(m: &ImageMetricv4<R, D>) -> Result<R, ItkException> {
    if !m.local_support {
        return Err(ItkException::new(
            "Modules/Registration/Metricsv4/include/itkDemonsImageToImageMetricv4.hxx",
            68,
            "DemonsImageToImageMetricv4",
            "The moving transform must be a displacement field transform",
        ));
    }
    let mut k = R::ZERO;
    for &s in &m.fixed.space().spacing {
        k = R::from_f64(k.to_f64() + s * s);
    }
    Ok(k / R::from_i64(D as i64))
}

/// `GetValueAndDerivative` (`compute_derivative` true) or `GetValue`.
pub fn evaluate<R: Real, const D: usize>(
    m: &ImageMetricv4<R, D>,
    transform: &Composite<R, D>,
    compute_derivative: bool,
    floor: &IndexFloor,
) -> Result<Reduced<R>, ItkException> {
    let normalizer = normalizer(m)?;
    let np = m.local_parameters(transform);
    let gradients = GradientSource {
        fixed: true,
        moving: false,
    };
    let intensity_threshold = R::from_f64(0.001);
    let denominator_threshold = R::from_f64(1e-9);
    let process = |i: usize| -> Option<(R, Vec<R>, Option<usize>)> {
        let s = m.sample(i, transform, gradients, compute_derivative, floor)?;
        let speed = s.fixed_value - s.moving_value;
        let sqr_speed = speed * speed;
        let mut local = Vec::new();
        if compute_derivative {
            let mut magnitude = R::ZERO;
            for j in 0..D {
                magnitude = magnitude + s.fixed_gradient[j] * s.fixed_gradient[j];
            }
            let denominator = sqr_speed / normalizer + magnitude;
            local = if speed.abs() < intensity_threshold || denominator < denominator_threshold {
                vec![R::ZERO; np]
            } else {
                (0..np)
                    .map(|p| speed * s.fixed_gradient[p] / denominator)
                    .collect()
            };
        }
        Some((sqr_speed, local, m.voxel_of(&s.virtual_index)))
    };
    let mut reduction = BaseReduction::for_metric(m, transform, compute_derivative);
    m.for_each_sample(process, |(v, l, voxel)| {
        reduction.add_point(v, &l, voxel);
        Ok(())
    })?;
    Ok(reduction.finish(compute_derivative, np))
}

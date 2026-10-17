// SPDX-License-Identifier: Apache-2.0
//! `CorrelationImageToImageMetricv4` (ITK v5.4.5, `itkCorrelationImageToImageMetricv4.hxx`,
//! `itkCorrelationImageToImageMetricv4HelperThreader.hxx`,
//! `itkCorrelationImageToImageMetricv4GetValueAndDerivativeThreader.hxx`): ANTs' `GC`.
//!
//! Two passes over the domain. The first (`InitializeForIteration`'s helper) averages the
//! fixed and moving values of the valid points; the second accumulates, with `f` and `m`
//! the values less their averages, `Σf·m`, `Σf²`, `Σm²` and, per parameter, `Σf·g` and
//! `Σm·g` where `g` is the moving gradient projected on the transform's Jacobian. The value
//! is `−(Σf·m)² / (Σm²·Σf²)` (computed in double), the derivative
//! `2·Σfm/(Σf²·Σm²)·(Σf·g − Σfm/Σm²·Σm·g)` (in `R`). Every sum is in `R`, in the order one
//! ITK work unit visits the points.

use larmorx_core::IndexFloor;

use super::{GradientSource, ImageMetricv4, no_valid_points_warning};
use crate::transform::Composite;
use crate::{ItkException, Real};

const CLASS: &str = "CorrelationImageToImageMetricv4";

/// The averages of the first pass, and the warning ITK prints when no point is valid.
fn averages<R: Real, const D: usize>(
    m: &ImageMetricv4<R, D>,
    transform: &Composite<R, D>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<(R, R), ItkException> {
    let none = GradientSource {
        fixed: false,
        moving: false,
    };
    let mut fix_sum = R::ZERO;
    let mut mov_sum = R::ZERO;
    let mut valid: usize = 0;
    m.for_each_sample(
        |i| {
            m.sample(i, transform, none, false, floor)
                .map(|s| (s.fixed_value, s.moving_value))
        },
        |(f, mv)| {
            fix_sum = fix_sum + f;
            mov_sum = mov_sum + mv;
            valid += 1;
            Ok(())
        },
    )?;
    if valid == 0 {
        warnings.push(
            "WARNING: In Modules/Registration/Metricsv4/include/itkCorrelationImageToImageMetricv4HelperThreader.hxx, line 77\n\
             CorrelationImageToImageMetricv4HelperThreader (0x0): collected only zero points\n\n"
                .to_string(),
        );
        return Ok((R::ZERO, R::ZERO));
    }
    let n = R::from_i64(valid as i64);
    Ok(((R::ZERO + fix_sum) / n, (R::ZERO + mov_sum) / n))
}

/// `GetValue` (`compute_derivative` false) or `GetValueAndDerivative`.
pub fn evaluate<R: Real, const D: usize>(
    m: &ImageMetricv4<R, D>,
    transform: &Composite<R, D>,
    compute_derivative: bool,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<(R, Vec<R>), ItkException> {
    let np = transform.number_of_parameters();
    let (average_fix, average_mov) = averages(m, transform, floor, warnings)?;
    let gradients = GradientSource {
        fixed: false,
        moving: true,
    };
    let process = |i: usize| -> Option<(R, R, Vec<R>)> {
        let s = m.sample(i, transform, gradients, compute_derivative, floor)?;
        let f1 = s.fixed_value - average_fix;
        let m1 = s.moving_value - average_mov;
        let mut projected = Vec::new();
        if compute_derivative {
            let mut jac = vec![R::ZERO; D * np];
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            transform.jacobian(&vp, &mut jac);
            projected = (0..np)
                .map(|par| {
                    let mut sum = R::ZERO;
                    for dim in 0..D {
                        sum = sum + s.moving_gradient[dim] * jac[dim * np + par];
                    }
                    sum
                })
                .collect();
        }
        Some((f1, m1, projected))
    };
    let (mut f2, mut m2, mut fm) = (R::ZERO, R::ZERO, R::ZERO);
    let mut fdm = vec![R::ZERO; if compute_derivative { np } else { 0 }];
    let mut mdm = fdm.clone();
    let mut valid: usize = 0;
    m.for_each_sample(process, |(f1, m1, projected)| {
        f2 = f2 + f1 * f1;
        m2 = m2 + m1 * m1;
        fm = fm + f1 * m1;
        for (par, &sum) in projected.iter().enumerate() {
            fdm[par] = fdm[par] + f1 * sum;
            mdm[par] = mdm[par] + m1 * sum;
        }
        valid += 1;
        Ok(())
    })?;
    let mut derivative = vec![R::ZERO; if compute_derivative { np } else { 0 }];
    // VerifyNumberOfValidPoints.
    if valid == 0 {
        warnings.push(no_valid_points_warning(
            "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx",
            544,
            CLASS,
        ));
        return Ok((R::MAX, derivative));
    }
    // The work unit's sums added to zero.
    let (fm, f2, m2) = (R::ZERO + fm, R::ZERO + f2, R::ZERO + m2);
    let m2f2 = m2 * f2;
    if m2f2 <= R::EPSILON {
        return Ok((R::ZERO, derivative));
    }
    let value = R::from_f64(-fm.to_f64() * fm.to_f64() / m2f2.to_f64());
    if compute_derivative {
        let fc = R::from_f64(2.0);
        let a = fc * fm / (f2 * m2);
        let b = fm / m2;
        for par in 0..np {
            let (fd, md) = (R::ZERO + fdm[par], R::ZERO + mdm[par]);
            derivative[par] = R::ZERO + a * (fd - b * md);
        }
    }
    Ok((value, derivative))
}

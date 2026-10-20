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

/// `GetValueAndDerivative` with a moving transform of local support (a displacement field:
/// SyN's identity field, or the field a stage optimises). ITK accumulates every point's
/// `f·g` and `m·g` into the derivative's first `D` entries (its per-thread sums are sized for
/// all the parameters but indexed by the local parameter) and scales the whole vector, so
/// only the first voxel gets a derivative (ITK-30, reproduced with `correct` false);
/// corrected, each voxel gets its own point's terms. `g` is the moving gradient projected on
/// the transform's Jacobian (`D` local parameters).
pub fn local_derivative<R: Real, const D: usize>(
    metric: &ImageMetricv4<R, D>,
    transform: &Composite<R, D>,
    correct: bool,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<(R, Vec<R>), ItkException> {
    let total = D * metric.virtual_space.len();
    let (average_fix, average_mov) = averages(metric, transform, floor, warnings)?;
    let gradients = GradientSource {
        fixed: false,
        moving: true,
    };
    let (mut f2, mut m2, mut fm) = (R::ZERO, R::ZERO, R::ZERO);
    let mut fdm = vec![R::ZERO; total];
    let mut mdm = vec![R::ZERO; total];
    let mut count = 0usize;
    metric.for_each_sample(
        |i| {
            let s = metric.sample(i, transform, gradients, true, floor)?;
            let f1 = s.fixed_value - average_fix;
            let m1 = s.moving_value - average_mov;
            // Σ_dim ∇m[dim]·J[dim][par].
            let mut jac = [R::ZERO; 16];
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            metric.moving_jacobian(transform, &vp, &mut jac[..D * D]);
            let projected: [R; D] = std::array::from_fn(|par| {
                let mut sum = R::ZERO;
                for dim in 0..D {
                    sum = sum + s.moving_gradient[dim] * jac[dim * D + par];
                }
                sum
            });
            Some((f1, m1, projected, metric.voxel_of(&s.virtual_index)))
        },
        |(f1, m1, projected, voxel)| {
            f2 = f2 + f1 * f1;
            m2 = m2 + m1 * m1;
            fm = fm + f1 * m1;
            let base = if correct {
                match voxel {
                    Some(v) => v * D,
                    None => return Ok(()),
                }
            } else {
                0
            };
            for par in 0..D {
                fdm[base + par] = fdm[base + par] + f1 * projected[par];
                mdm[base + par] = mdm[base + par] + m1 * projected[par];
            }
            count += 1;
            Ok(())
        },
    )?;
    let mut derivative = vec![R::ZERO; total];
    if count == 0 {
        warnings.push(no_valid_points_warning(
            "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx",
            544,
            CLASS,
        ));
        return Ok((R::MAX, derivative));
    }
    let (fm, f2, m2) = (R::ZERO + fm, R::ZERO + f2, R::ZERO + m2);
    let m2f2 = m2 * f2;
    if m2f2 <= R::EPSILON {
        // m_Value keeps its previous value; ANTs' SyN reads it as the value of this call.
        return Ok((R::ZERO, derivative));
    }
    let value = R::from_f64(-fm.to_f64() * fm.to_f64() / m2f2.to_f64());
    // m_DerivativeResult += fc * fm / (f2 * m2) * (fdm - fm / m2 * mdm), element by element.
    let fc = R::from_f64(2.0);
    let a = fc * fm / (f2 * m2);
    let b = fm / m2;
    for i in 0..total {
        let fd = R::ZERO + fdm[i];
        let md = R::ZERO + mdm[i];
        derivative[i] = derivative[i] + a * (fd - b * md);
    }
    Ok((value, derivative))
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

// SPDX-License-Identifier: Apache-2.0
//! `MeanSquaresImageToImageMetricv4` (ITK v5.4.5,
//! `itkMeanSquaresImageToImageMetricv4GetValueAndDerivativeThreader.hxx`) and the reduction
//! of `ImageToImageMetricv4GetValueAndDerivativeThreaderBase` that it and other metrics
//! share ([`BaseReduction`]).

use larmorx_core::IndexFloor;

use super::{GradientSource, ImageMetricv4};
use crate::transform::Composite;
use crate::{CompensatedSum, ItkException, Real};

/// `ImageToImageMetricv4GetValueAndDerivativeThreaderBase` for one work unit: the measure
/// summed in `R`, each derivative component in a compensated sum, then
/// `AfterThreadedExecution`'s averages.
pub struct BaseReduction<R> {
    pub valid: usize,
    pub measure: R,
    pub derivatives: Vec<CompensatedSum>,
}

/// The outcome of a reduction: the value and derivative, or ITK's "no valid points" case
/// (value `max`, derivative 0, with a warning).
pub struct Reduced<R> {
    pub value: R,
    pub derivative: Vec<R>,
    pub no_valid_points: bool,
}

impl<R: Real> BaseReduction<R> {
    pub fn new(np: usize, compute_derivative: bool) -> Self {
        BaseReduction {
            valid: 0,
            measure: R::ZERO,
            derivatives: if compute_derivative {
                vec![CompensatedSum::default(); np]
            } else {
                Vec::new()
            },
        }
    }

    /// One valid point: `NumberOfValidPoints++`, `Measure += value`, and its local
    /// derivative into the compensated sums.
    #[inline]
    pub fn add(&mut self, value: R, local: &[R]) {
        self.valid += 1;
        self.measure = self.measure + value;
        for (s, &d) in self.derivatives.iter_mut().zip(local) {
            s.add(d.to_f64());
        }
    }

    /// `AfterThreadedExecution`: the derivative of the work unit added to a compensated sum
    /// (the per-unit sum converted to `R` on the way) and to the zeroed result; then, if any
    /// point was valid, the measure and the derivative divided by their number.
    pub fn finish(self, compute_derivative: bool, np: usize) -> Reduced<R> {
        let mut derivative = if compute_derivative {
            vec![R::ZERO; np]
        } else {
            Vec::new()
        };
        if compute_derivative {
            for (p, unit) in self.derivatives.iter().enumerate() {
                let mut sum = CompensatedSum::default();
                sum.add(R::from_f64(unit.sum()).to_f64());
                derivative[p] = R::from_f64(derivative[p].to_f64() + sum.sum());
            }
        }
        if self.valid == 0 {
            return Reduced {
                value: R::MAX,
                derivative: vec![R::ZERO; derivative.len()],
                no_valid_points: true,
            };
        }
        let n = R::from_i64(self.valid as i64);
        let value = (R::ZERO + self.measure) / n;
        for d in derivative.iter_mut() {
            *d = *d / n;
        }
        Reduced {
            value,
            derivative,
            no_valid_points: false,
        }
    }
}

/// `MeanSquaresImageToImageMetricv4`: the mean of `(fixed − moving)²`; the local derivative
/// `Σ_dim 2·diff·J[dim][p]·∇m[dim]`, each term in double, accumulated in `R`.
pub fn evaluate<R: Real, const D: usize>(
    m: &ImageMetricv4<R, D>,
    transform: &Composite<R, D>,
    compute_derivative: bool,
    floor: &IndexFloor,
) -> Result<Reduced<R>, ItkException> {
    let np = transform.number_of_parameters();
    let gradients = GradientSource {
        fixed: false,
        moving: true,
    };
    let process = |i: usize| -> Option<(R, Vec<R>)> {
        let s = m.sample(i, transform, gradients, compute_derivative, floor)?;
        let diff = s.fixed_value - s.moving_value;
        let value = R::ZERO + diff * diff;
        let mut local = Vec::new();
        if compute_derivative {
            let mut jac = vec![R::ZERO; D * np];
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            transform.jacobian(&vp, &mut jac);
            local = (0..np)
                .map(|par| {
                    let mut l = R::ZERO;
                    for dim in 0..D {
                        let t = 2.0
                            * diff.to_f64()
                            * jac[dim * np + par].to_f64()
                            * s.moving_gradient[dim].to_f64();
                        l = R::from_f64(l.to_f64() + t);
                    }
                    l
                })
                .collect();
        }
        Some((value, local))
    };
    let mut reduction = BaseReduction::new(np, compute_derivative);
    m.for_each_sample(process, |(v, l)| {
        reduction.add(v, &l);
        Ok(())
    })?;
    Ok(reduction.finish(compute_derivative, np))
}

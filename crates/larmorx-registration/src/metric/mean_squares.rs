// SPDX-License-Identifier: Apache-2.0
//! `MeanSquaresImageToImageMetricv4` (ITK v5.4.5,
//! `itkMeanSquaresImageToImageMetricv4GetValueAndDerivativeThreader.hxx`) and the reduction
//! of `ImageToImageMetricv4GetValueAndDerivativeThreaderBase` that it and other metrics
//! share ([`BaseReduction`]).

use larmorx_core::IndexFloor;

use super::{GradientSource, ImageMetricv4};
use crate::transform::Composite;
use crate::{CompensatedSum, CompensatedSums, ItkException, Real};

/// `ImageToImageMetricv4GetValueAndDerivativeThreaderBase` for one work unit: the measure
/// summed in `R`; for a transform of global support each derivative component in a
/// compensated sum, then `AfterThreadedExecution`'s averages; for a transform of local
/// support ([`BaseReduction::new_local`]) each point's derivative added (in `R`) to its
/// voxel's entries of the whole derivative, which is not averaged.
pub struct BaseReduction<R> {
    pub valid: usize,
    pub measure: R,
    pub derivatives: CompensatedSums,
    /// Local support: the derivative over the virtual domain and the number of local
    /// parameters (`D`).
    local: Option<(Vec<R>, usize)>,
}

/// The outcome of a reduction: the value and derivative, or ITK's "no valid points" case
/// (value `max`, derivative 0, with a warning).
pub struct Reduced<R> {
    pub value: R,
    pub derivative: Vec<R>,
    pub no_valid_points: bool,
    /// Whether a point's derivative asked the moving transform for its Jacobian (every valid
    /// point does, except in the CC metric, which skips points of a flat neighbourhood): only
    /// then can a transform without a Jacobian with respect to position throw (ITK-35).
    pub jacobian_used: bool,
}

impl<R: Real> BaseReduction<R> {
    pub fn new(np: usize, compute_derivative: bool) -> Self {
        BaseReduction {
            valid: 0,
            measure: R::ZERO,
            derivatives: CompensatedSums::new(if compute_derivative { np } else { 0 }),
            local: None,
        }
    }

    /// A reduction for a transform of local support: `total` derivative entries (zeros),
    /// `local` per voxel.
    pub fn new_local(total: usize, local: usize, compute_derivative: bool) -> Self {
        BaseReduction {
            valid: 0,
            measure: R::ZERO,
            derivatives: CompensatedSums::new(0),
            local: Some((
                if compute_derivative {
                    vec![R::ZERO; total]
                } else {
                    Vec::new()
                },
                local,
            )),
        }
    }

    /// The reduction `m` needs: [`BaseReduction::new_local`] with local support, else
    /// [`BaseReduction::new`].
    pub fn for_metric<const D: usize>(
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        compute_derivative: bool,
    ) -> Self {
        if m.local_support {
            Self::new_local(m.number_of_parameters(transform), D, compute_derivative)
        } else {
            Self::new(m.local_parameters(transform), compute_derivative)
        }
    }

    /// One valid point: `NumberOfValidPoints++`, `Measure += value`, and its local
    /// derivative into the compensated sums (global support) or into the entries of its
    /// virtual voxel `voxel` (local support; `None`: outside the virtual region).
    #[inline]
    pub fn add_point(&mut self, value: R, local: &[R], voxel: Option<usize>) {
        self.valid += 1;
        self.measure = self.measure + value;
        match &mut self.local {
            None => {
                if !local.is_empty() {
                    self.derivatives.add_dense(local);
                }
            }
            Some((derivative, n)) => {
                if let Some(v) = voxel
                    && !derivative.is_empty()
                {
                    for (i, &d) in local.iter().enumerate().take(*n) {
                        let cell = &mut derivative[v * *n + i];
                        *cell = *cell + d;
                    }
                }
            }
        }
    }

    /// [`BaseReduction::add_point`] with the derivative's parameters: every one (`cols`
    /// `None`), or the columns `cols` of a B-spline transform's (the others zero).
    #[inline]
    pub fn add_point_cols(
        &mut self,
        value: R,
        local: &[R],
        cols: Option<&[usize]>,
        voxel: Option<usize>,
    ) {
        match cols {
            Some(cols) if self.local.is_none() => {
                self.valid += 1;
                self.measure = self.measure + value;
                if !self.derivatives.sums.is_empty() {
                    self.derivatives.add_sparse(cols, local);
                }
            }
            _ => self.add_point(value, local, voxel),
        }
    }

    /// [`BaseReduction::add_point`] for a transform of global support.
    #[inline]
    pub fn add(&mut self, value: R, local: &[R]) {
        self.add_point(value, local, None);
    }

    /// `AfterThreadedExecution`: for global support, the derivative of the work unit added to
    /// a compensated sum (the per-unit sum converted to `R` on the way) and to the zeroed
    /// result; then, if any point was valid, the measure divided by their number, and the
    /// derivative too for global support.
    pub fn finish(mut self, compute_derivative: bool, np: usize) -> Reduced<R> {
        self.derivatives.flush();
        if let Some((derivative, _)) = self.local {
            if self.valid == 0 {
                return Reduced {
                    value: R::MAX,
                    derivative: vec![R::ZERO; derivative.len()],
                    no_valid_points: true,
                    jacobian_used: true,
                };
            }
            let n = R::from_i64(self.valid as i64);
            return Reduced {
                value: (R::ZERO + self.measure) / n,
                derivative,
                no_valid_points: false,
                jacobian_used: true,
            };
        }
        let mut derivative = if compute_derivative {
            vec![R::ZERO; np]
        } else {
            Vec::new()
        };
        if compute_derivative {
            for (p, unit) in self.derivatives.sums.iter().enumerate() {
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
                jacobian_used: true,
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
            jacobian_used: true,
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
    let np = m.local_parameters(transform);
    let gradients = GradientSource {
        fixed: false,
        moving: true,
    };
    type Point<R> = (R, Vec<R>, Option<usize>, Option<Vec<usize>>);
    let process = |i: usize| -> Option<Point<R>> {
        let s = m.sample(i, transform, gradients, compute_derivative, floor)?;
        let diff = s.fixed_value - s.moving_value;
        let value = R::ZERO + diff * diff;
        let mut local = Vec::new();
        let mut cols = None;
        if compute_derivative {
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            let jac = m.jacobian_columns(transform, &vp);
            local = (0..jac.n)
                .map(|par| {
                    let mut l = R::ZERO;
                    for dim in 0..D {
                        let t = 2.0
                            * diff.to_f64()
                            * jac.at(dim, par).to_f64()
                            * s.moving_gradient[dim].to_f64();
                        l = R::from_f64(l.to_f64() + t);
                    }
                    l
                })
                .collect();
            cols = jac.cols;
        }
        let voxel = if m.local_support {
            m.voxel_of(&s.virtual_index)
        } else {
            None
        };
        Some((value, local, voxel, cols))
    };
    let mut reduction = BaseReduction::for_metric(m, transform, compute_derivative);
    m.for_each_sample(process, |(v, l, voxel, cols)| {
        reduction.add_point_cols(v, &l, cols.as_deref(), voxel);
        Ok(())
    })?;
    Ok(reduction.finish(compute_derivative, np))
}

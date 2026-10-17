// SPDX-License-Identifier: Apache-2.0
//! `MattesMutualInformationImageToImageMetricv4` (ITK v5.4.5,
//! `itkMattesMutualInformationImageToImageMetricv4.hxx` and its
//! `GetValueAndDerivativeThreader.hxx`), for transforms of global support.
//!
//! The joint histogram has `bins × bins` bins: the fixed value falls in one bin (a box Parzen
//! window), the moving value spreads over four with cubic B-spline weights. The value is
//! `−Σ p(f,m)·log(p(f,m) / (p(f)·p(m)))`; the derivative accumulates, for each point, the
//! moving image gradient projected on the transform's Jacobian times the B-spline
//! derivative into `bins × bins × parameters` cells, which are then weighted by the
//! logarithm of each bin's ratio. All in `R` (`PDFValueType`), in a single ITK thread's
//! order.

use crate::CompensatedSum;
use larmorx_core::IndexFloor;

use super::{GradientSource, ImageMetricv4};
use crate::transform::Composite;
use crate::{ItkException, Real};

const FILE: &str =
    "Modules/Registration/Metricsv4/include/itkMattesMutualInformationImageToImageMetricv4.hxx";
const CLASS: &str = "MattesMutualInformationImageToImageMetricv4";

/// `BSplineKernelFunction<3>::FastEvaluate`.
#[inline]
pub fn cubic<R: Real>(u: R) -> R {
    let a = u.abs();
    let c = |v: f64| R::from_f64(v);
    if a < c(1.0) {
        let s = a * a;
        (c(4.0) - c(6.0) * s + c(3.0) * s * a) / c(6.0)
    } else if a < c(2.0) {
        let s = a * a;
        (c(8.0) - c(12.0) * a + c(6.0) * s - s * a) / c(6.0)
    } else {
        R::ZERO
    }
}

/// `BSplineDerivativeKernelFunction<3>::FastEvaluate`.
#[inline]
pub fn cubic_derivative<R: Real>(u: R) -> R {
    let c = |v: f64| R::from_f64(v);
    if u >= R::ZERO && u < c(1.0) {
        c(-2.0) * u + c(1.5) * u * u
    } else if u > c(-1.0) && u < R::ZERO {
        c(-2.0) * u - c(1.5) * u * u
    } else if u >= c(1.0) && u < c(2.0) {
        c(-2.0) + c(2.0) * u - c(0.5) * u * u
    } else if u > c(-2.0) && u <= c(-1.0) {
        c(2.0) + c(2.0) * u + c(0.5) * u * u
    } else {
        R::ZERO
    }
}

/// The metric's state after `Initialize` (the intensity ranges and bin sizes).
#[derive(Clone, Debug)]
pub struct Mattes<R> {
    pub bins: usize,
    pub fixed_true_min: R,
    pub fixed_true_max: R,
    pub moving_true_min: R,
    pub moving_true_max: R,
    pub fixed_bin_size: R,
    pub fixed_normalized_min: R,
    pub moving_bin_size: R,
    pub moving_normalized_min: R,
}

/// One point's contribution, computed in parallel.
struct Contribution<R> {
    fixed_index: usize,
    /// `pdfMovingIndex`, the first of the four moving bins.
    moving_index: usize,
    /// The four B-spline weights and, with the derivative, their derivatives.
    weights: [R; 4],
    derivatives: [R; 4],
    /// `Σ_dim J[dim][mu]·∇m[dim]` for each parameter `mu`.
    inner: Vec<R>,
}

impl<R: Real> Mattes<R> {
    /// `Initialize`: the fixed and moving intensity ranges over the analysis region (the
    /// sampled points, or the masked or whole images) and the bin sizes, padded by two bins
    /// on each side.
    pub fn initialize<const D: usize>(
        bins: usize,
        m: &ImageMetricv4<R, D>,
    ) -> Result<Self, ItkException> {
        let mut fmin = R::MAX;
        let mut fmax = -R::MAX;
        let take = |v: R, lo: &mut R, hi: &mut R| {
            *lo = if *lo < v { *lo } else { v };
            *hi = if *hi > v { *hi } else { v };
        };
        let fixed = &m.fixed;
        match &m.samples {
            Some(samples) => {
                for p in samples.iter() {
                    let local: [R; D] = std::array::from_fn(|k| R::from_f64(p[k]));
                    let q = match &m.fixed_transform {
                        Some(t) => t.transform_point(&local),
                        None => local,
                    };
                    let q: [f64; D] = std::array::from_fn(|k| q[k].to_f64());
                    let idx = fixed.space.point_to_index(&q);
                    if fixed.space.contains(&idx) {
                        let v = fixed.at(&idx);
                        take(v, &mut fmin, &mut fmax);
                    }
                }
            }
            None => match &m.fixed_mask {
                Some(mask) => {
                    for (o, &v) in fixed.data.iter().enumerate() {
                        let p = fixed.space.index_to_point(&fixed.space.index_of(o));
                        if mask.is_inside(&p) {
                            take(v, &mut fmin, &mut fmax);
                        }
                    }
                }
                None => {
                    for &v in &fixed.data {
                        take(v, &mut fmin, &mut fmax);
                    }
                }
            },
        }
        let mut mmin = R::MAX;
        let mut mmax = -R::MAX;
        let moving = &m.moving;
        match &m.moving_mask {
            Some(mask) => {
                for (o, &v) in moving.data.iter().enumerate() {
                    let p = moving.space.index_to_point(&moving.space.index_of(o));
                    if mask.is_inside(&p) {
                        take(v, &mut mmin, &mut mmax);
                    }
                }
            }
            None => {
                for &v in &moving.data {
                    take(v, &mut mmin, &mut mmax);
                }
            }
        }
        let padding = R::from_f64(2.0);
        let denominator = R::from_i64(bins as i64 - 4);
        let fixed_bin_size = (fmax - fmin) / denominator;
        let moving_bin_size = (mmax - mmin) / denominator;
        Ok(Mattes {
            bins,
            fixed_true_min: fmin,
            fixed_true_max: fmax,
            moving_true_min: mmin,
            moving_true_max: mmax,
            fixed_bin_size,
            fixed_normalized_min: fmin / fixed_bin_size - padding,
            moving_bin_size,
            moving_normalized_min: mmin / moving_bin_size - padding,
        })
    }

    /// `static_cast<OffsetValueType>(term)` clamped to `[2, bins − 3]`.
    #[inline]
    fn parzen_index(&self, term: R) -> usize {
        let t = term.to_f64();
        // C++'s conversion truncates toward zero; out-of-range and NaN values are clamped
        // like the bounds they exceed (on x86-64 they give LONG_MIN, which clamps to 2).
        let mut i = if t.is_nan() || t <= -9.2e18 || t >= 9.2e18 {
            i64::MIN
        } else {
            t as i64
        };
        if i < 2 {
            i = 2;
        } else {
            let n = self.bins as i64 - 3;
            if i > n {
                i = n;
            }
        }
        i as usize
    }

    /// `GetValue` (`compute_derivative` false) or `GetValueAndDerivative`: the value and the
    /// derivative (empty without it).
    pub fn evaluate<const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        compute_derivative: bool,
        floor: &IndexFloor,
    ) -> Result<(R, Vec<R>), ItkException> {
        let bins = self.bins;
        let np = transform.number_of_parameters();
        let gradients = GradientSource {
            fixed: false,
            moving: true,
        };
        let mut fixed_marginal = vec![R::ZERO; bins];
        let mut joint = vec![R::ZERO; bins * bins];
        let mut joint_derivatives = if compute_derivative {
            vec![R::ZERO; bins * bins * np]
        } else {
            Vec::new()
        };
        let mut valid: usize = 0;
        let process = |i: usize| -> Option<Contribution<R>> {
            let s = m.sample(i, transform, gradients, compute_derivative, floor)?;
            let mv = s.moving_value;
            if mv < self.moving_true_min || mv > self.moving_true_max {
                return None;
            }
            let term = mv / self.moving_bin_size - self.moving_normalized_min;
            let parzen = self.parzen_index(term);
            let moving_index = parzen - 1;
            let fterm = s.fixed_value / self.fixed_bin_size - self.fixed_normalized_min;
            let fixed_index = self.parzen_index(fterm);
            let mut arg = R::from_i64(moving_index as i64) - term;
            let mut weights = [R::ZERO; 4];
            let mut derivatives = [R::ZERO; 4];
            for b in 0..4 {
                weights[b] = cubic(arg);
                if compute_derivative {
                    derivatives[b] = cubic_derivative(arg);
                }
                arg = R::from_f64(arg.to_f64() + 1.0);
            }
            let mut inner = Vec::new();
            if compute_derivative {
                let mut jac = vec![R::ZERO; D * np];
                let local: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
                transform.jacobian(&local, &mut jac);
                inner = (0..np)
                    .map(|mu| {
                        let mut ip = R::ZERO;
                        for dim in 0..D {
                            ip = ip + jac[dim * np + mu] * s.moving_gradient[dim];
                        }
                        ip
                    })
                    .collect();
            }
            Some(Contribution {
                fixed_index,
                moving_index,
                weights,
                derivatives,
                inner,
            })
        };
        m.for_each_sample(process, |c| {
            fixed_marginal[c.fixed_index] = fixed_marginal[c.fixed_index] + R::ONE;
            let row = c.fixed_index * bins;
            for b in 0..4 {
                let bin = row + c.moving_index + b;
                joint[bin] = joint[bin] + c.weights[b];
                if compute_derivative {
                    let offset = c.fixed_index * np * bins + (c.moving_index + b) * np;
                    for mu in 0..np {
                        let cell = &mut joint_derivatives[offset + mu];
                        *cell = *cell + c.inner[mu] * c.derivatives[b];
                    }
                }
            }
            valid += 1;
            Ok(())
        })?;
        // GetValueCommonAfterThreadedExecution.
        let mut sum = CompensatedSum::default();
        for &v in &joint {
            sum.add(v.to_f64());
        }
        let joint_sum = R::from_f64(sum.sum());
        if compute_derivative {
            let n_factor =
                R::from_f64(-1.0 / (self.moving_bin_size * R::from_i64(valid as i64)).to_f64());
            for v in joint_derivatives.iter_mut() {
                *v = *v * n_factor;
            }
        }
        // ComputeResults.
        if valid == 0 {
            return Err(ItkException::new(
                FILE,
                307,
                CLASS,
                "All samples map outside moving image buffer. The images do not sufficiently \
                 overlap. They need to be initialized to have more overlap before this metric \
                 will work. For instance, you can align the image centers by translation.\n",
            ));
        }
        if joint_sum < R::EPSILON {
            return Err(ItkException::new(
                FILE,
                315,
                CLASS,
                "Joint PDF summed to zero",
            ));
        }
        let normalization = R::from_f64(1.0 / joint_sum.to_f64());
        for v in joint.iter_mut() {
            *v = *v * normalization;
        }
        for v in fixed_marginal.iter_mut() {
            *v = *v * normalization;
        }
        let mut moving_marginal = vec![R::ZERO; bins];
        for (col, mm) in moving_marginal.iter_mut().enumerate() {
            let mut s = R::ZERO;
            let mut i = col;
            while i < bins * bins {
                s = s + joint[i];
                i += bins;
            }
            *mm = s;
        }
        let close = R::EPSILON;
        let mut derivative = if compute_derivative {
            vec![R::ZERO; np]
        } else {
            Vec::new()
        };
        let mut total = R::ZERO;
        for f in 0..bins {
            let fm = fixed_marginal[f];
            if fm > close {
                let log_fm = fm.ln();
                for mi in 0..bins {
                    let mm = moving_marginal[mi];
                    let j = joint[f * bins + mi];
                    if mm > close && j > close {
                        let ratio = (j / mm).ln();
                        total = total + j * (ratio - log_fm);
                        if compute_derivative {
                            let offset = f * np * bins + mi * np;
                            for p in 0..np {
                                derivative[p] =
                                    derivative[p] + joint_derivatives[offset + p] * ratio;
                            }
                        }
                    }
                }
            }
        }
        Ok((R::from_f64(-total.to_f64()), derivative))
    }
}

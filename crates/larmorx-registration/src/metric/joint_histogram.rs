// SPDX-License-Identifier: Apache-2.0
//! `JointHistogramMutualInformationImageToImageMetricv4` (ITK v5.4.5,
//! `itkJointHistogramMutualInformationImageToImageMetricv4.hxx`,
//! `itkJointHistogramMutualInformationComputeJointPDFThreaderBase.hxx`,
//! `itkJointHistogramMutualInformationGetValueAndDerivativeThreader.hxx`): ANTs' `MI2`
//! (ANTs' `MI` is Mattes').
//!
//! Each evaluation first builds the joint histogram: every valid point's fixed and moving
//! values, scaled to `[0, 1]` by the images' extremes, fall in one bin of a `bins × bins`
//! image whose bins are `1/(bins − 5)` wide with two bins of padding on each side; the counts
//! divided by their total are smoothed by `DiscreteGaussianImageFilter` (variance 1 voxel²,
//! ANTs' setting) and summed along each axis into the marginals. The value is
//! `−Σ p·log(p / (px·py)) / log 2`; the derivative weights each point's moving gradient,
//! projected on the transform's Jacobian, by finite differences of the joint and marginal
//! densities, interpolated linearly.
//!
//! **ITK's marginals are swapped (ITK-18).** The marginal ITK calls the fixed image's sums the
//! joint density over its first index, which is the fixed bin: it is the moving image's
//! marginal, and the other way round. The value and the derivative use them under their
//! names. `correct_marginals` (the default, D13) gives each image its own marginal;
//! without it larmorx computes what ITK's source defines.

use std::sync::atomic::{AtomicU64, Ordering};

use larmorx_core::IndexFloor;

use super::mean_squares::BaseReduction;
use super::{GradientSource, ImageMetricv4, no_valid_points_warning};
use crate::interp::Linear;
use crate::transform::Composite;
use crate::{CompensatedSum, Image, ItkException, Real, Space};

const CLASS: &str = "JointHistogramMutualInformationImageToImageMetricv4";

/// The metric after `Initialize()` for one level.
#[derive(Debug)]
pub struct JointHistogramMi<R> {
    bins: usize,
    fixed_min: R,
    fixed_max: R,
    moving_min: R,
    moving_max: R,
    /// `m_Log2`, `log(2)` stored in `R`.
    log2: R,
    /// `m_JointPDFSpacing[0]`.
    spacing: f64,
    joint_space: Space<2>,
    marginal_space: Space<1>,
    /// The kernel of the joint density's smoothing.
    kernel: Vec<f64>,
    /// Whether each image gets its own marginal (ITK-18 corrected).
    correct_marginals: bool,
    /// `m_Value` (as f64 bits), which `GetValue` returns unchanged when no point is valid.
    value: AtomicU64,
}

/// The joint density and its marginals for the current parameters (`InitializeForIteration`).
struct Densities<R> {
    joint: Vec<R>,
    /// What ITK stores as the fixed image's marginal (the sums over the fixed bins).
    fixed_marginal: Vec<R>,
    /// What ITK stores as the moving image's marginal (the sums over the moving bins).
    moving_marginal: Vec<R>,
    /// `m_JointHistogramTotalCount`.
    count: usize,
}

/// The extremes of an image's voxels inside its mask (all voxels without one), as
/// `Initialize` scans them; `(max, nonpositive min)` of `R` when none is inside.
fn extremes<R: Real, const D: usize>(
    image: &Image<R, D>,
    mask: Option<&crate::mask::Mask<D>>,
) -> (R, R) {
    let mut lo = R::MAX;
    let mut hi = -R::MAX;
    for (o, &v) in image.data.iter().enumerate() {
        if let Some(m) = mask {
            let p = image.space.index_to_point(&image.space.index_of(o));
            if !m.is_inside(&p) {
                continue;
            }
        }
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    (lo, hi)
}

/// `DiscreteGaussianImageFilter` on the joint density: one pass per axis, the last axis
/// first, each tap's product summed in double from the kernel's first tap and stored in `R`,
/// the nearest edge bin outside.
fn smooth<R: Real>(data: &[R], bins: usize, kernel: &[f64]) -> Vec<R> {
    let r = (kernel.len() / 2) as isize;
    let last = bins as isize - 1;
    let mut current = data.to_vec();
    for axis in [1usize, 0] {
        let mut out = vec![R::ZERO; bins * bins];
        for j in 0..bins {
            for i in 0..bins {
                let mut sum = 0.0f64;
                for (k, &c) in kernel.iter().enumerate() {
                    let (x, y) = if axis == 0 {
                        ((i as isize + k as isize - r).clamp(0, last) as usize, j)
                    } else {
                        (i, (j as isize + k as isize - r).clamp(0, last) as usize)
                    };
                    sum += c * current[x + y * bins].to_f64();
                }
                out[i + j * bins] = R::from_f64(sum);
            }
        }
        current = out;
    }
    current
}

impl<R: Real> JointHistogramMi<R> {
    /// `SetNumberOfHistogramBins(bins)`, `SetVarianceForJointPDFSmoothing(1.0)`, then
    /// `Initialize()`: the extremes of the level's images inside their masks, the densities'
    /// geometry.
    pub fn initialize<const D: usize>(
        bins: usize,
        m: &ImageMetricv4<R, D>,
        correct_marginals: bool,
    ) -> Result<Self, ItkException> {
        let (fixed_min, fixed_max) = extremes(&m.fixed, m.fixed_mask.as_ref());
        let (moving_min, moving_max) = extremes(&m.moving, m.moving_mask.as_ref());
        // spacing = 1 / (bins − 2·2 − 1), in R; origin = −2 spacings, in double.
        let padding = R::from_f64(2.0);
        let denominator = R::from_i64(bins as i64) - padding * R::from_f64(2.0) - R::ONE;
        let spacing = (R::ONE / denominator).to_f64();
        let origin = -(spacing * padding.to_f64());
        let identity2 = [[1.0, 0.0], [0.0, 1.0]];
        let joint_space = Space::new(
            [bins, bins],
            [0, 0],
            [spacing, spacing],
            [origin, origin],
            identity2,
        )?;
        let marginal_space = Space::new([bins], [0], [spacing], [origin], [[1.0]])?;
        let kernel = larmorx_image::discrete_gaussian::gaussian_kernel(1.0, f64::from(0.01f32), 32);
        Ok(JointHistogramMi {
            bins,
            fixed_min,
            fixed_max,
            moving_min,
            moving_max,
            log2: R::from_f64(std::f64::consts::LN_2),
            spacing,
            joint_space,
            marginal_space,
            kernel,
            correct_marginals,
            value: AtomicU64::new(0.0f64.to_bits()),
        })
    }

    fn last_value(&self) -> R {
        R::from_f64(f64::from_bits(self.value.load(Ordering::Relaxed)))
    }

    fn set_value(&self, v: R) {
        self.value.store(v.to_f64().to_bits(), Ordering::Relaxed);
    }

    /// `ComputeJointPDFPoint`: the values scaled by the extremes, in `R`.
    fn joint_point(&self, fixed: R, moving: R) -> [f64; 2] {
        let a = (fixed - self.fixed_min) / (self.fixed_max - self.fixed_min);
        let b = (moving - self.moving_min) / (self.moving_max - self.moving_min);
        [a.to_f64(), b.to_f64()]
    }

    /// `InitializeForIteration`: the joint histogram of the valid points, normalised,
    /// smoothed, and its marginals.
    fn densities<const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        floor: &IndexFloor,
    ) -> Result<Densities<R>, ItkException> {
        let bins = self.bins;
        let none = GradientSource {
            fixed: false,
            moving: false,
        };
        let mut counts = vec![0u64; bins * bins];
        let mut count: usize = 0;
        m.for_each_sample(
            |i| {
                let s = m.sample(i, transform, none, false, floor)?;
                let index = self
                    .joint_space
                    .point_to_index(&self.joint_point(s.fixed_value, s.moving_value));
                self.joint_space
                    .contains(&index)
                    .then(|| index[0] as usize + index[1] as usize * bins)
            },
            |bin| {
                counts[bin] += 1;
                count += 1;
                Ok(())
            },
        )?;
        let mut joint = vec![R::ZERO; bins * bins];
        if count > 0 {
            let total = R::from_i64(count as i64);
            for (p, &c) in joint.iter_mut().zip(&counts) {
                *p = R::from_i64(c as i64) / total;
            }
            joint = smooth(&joint, bins, &self.kernel);
        } else {
            // FillBuffer(0), then the smoothing of zeros.
            joint = smooth(&joint, bins, &self.kernel);
        }
        // Lines along the first index (the fixed bin) for each second index, then lines
        // along the second index for each first index.
        let mut fixed_marginal = vec![R::ZERO; bins];
        let mut moving_marginal = vec![R::ZERO; bins];
        for (k, out) in fixed_marginal.iter_mut().enumerate() {
            let mut sum = CompensatedSum::default();
            for i in 0..bins {
                sum.add(joint[i + k * bins].to_f64());
            }
            *out = R::from_f64(sum.sum());
        }
        for (k, out) in moving_marginal.iter_mut().enumerate() {
            let mut sum = CompensatedSum::default();
            for j in 0..bins {
                sum.add(joint[k + j * bins].to_f64());
            }
            *out = R::from_f64(sum.sum());
        }
        if self.correct_marginals {
            std::mem::swap(&mut fixed_marginal, &mut moving_marginal);
        }
        Ok(Densities {
            joint,
            fixed_marginal,
            moving_marginal,
            count,
        })
    }

    /// `ComputeValue`: `−Σ pxy·log(pxy / (px·py)) / log 2`, the terms in `R`, their sum
    /// compensated.
    fn compute_value(&self, d: &Densities<R>) -> R {
        let bins = self.bins;
        let eps = R::EPSILON;
        let mut total = CompensatedSum::default();
        for ii in 0..bins {
            let px = d.fixed_marginal[ii];
            for jj in 0..bins {
                let py = d.moving_marginal[jj];
                let denom = px * py;
                let pxy = d.joint[ii + jj * bins];
                let mut local = R::ZERO;
                if denom.abs() > eps && pxy / denom > eps {
                    local = pxy * (pxy / denom).ln();
                }
                total.add(local.to_f64());
            }
        }
        R::from_f64(-total.sum() / self.log2.to_f64())
    }

    /// A finite difference of a density along `axis` at `point`, over one bin centred on it,
    /// both ends clamped to `[spacing, 1]` (`ComputeJointPDFDerivative`,
    /// `ComputeMovingImageMarginalPDFDerivative`).
    fn derivative_along<const N: usize>(
        &self,
        interp: &Linear<'_, f64, N>,
        point: &[f64; N],
        axis: usize,
        floor: &IndexFloor,
    ) -> R {
        let offset = R::from_f64(0.5 * self.spacing).to_f64();
        let eps = R::from_f64(self.spacing).to_f64();
        let mut left = *point;
        left[axis] -= offset;
        let mut right = *point;
        right[axis] += offset;
        if left[axis] < eps {
            left[axis] = eps;
        }
        if right[axis] < eps {
            right[axis] = eps;
        }
        if left[axis] > 1.0 {
            left[axis] = 1.0;
        }
        if right[axis] > 1.0 {
            right[axis] = 1.0;
        }
        let delta = R::from_f64(right[axis] - left[axis]);
        if delta > R::ZERO {
            let deriv = R::from_f64(interp.evaluate(&right, floor) - interp.evaluate(&left, floor));
            return deriv / delta;
        }
        R::ZERO
    }

    /// `GetValue` (`compute_derivative` false) or `GetValueAndDerivative`.
    pub fn evaluate<const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        compute_derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        let d = self.densities(m, transform, floor)?;
        let np = m.local_parameters(transform);
        if !compute_derivative {
            // GetValue: the histogram's count is the number of valid points; with none,
            // ITK returns m_Value unchanged.
            if d.count == 0 {
                warnings.push(no_valid_points_warning(
                    "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx",
                    544,
                    CLASS,
                ));
                return Ok((self.last_value(), Vec::new()));
            }
            let v = self.compute_value(&d);
            self.set_value(v);
            return Ok((v, Vec::new()));
        }
        let as_f64 = |v: &[R]| -> Vec<f64> { v.iter().map(|x| x.to_f64()).collect() };
        let joint_image = Image::new(self.joint_space.clone(), as_f64(&d.joint));
        let moving_marginal_image =
            Image::new(self.marginal_space.clone(), as_f64(&d.moving_marginal));
        let joint_interp = Linear::new(&joint_image);
        let marginal_interp = Linear::new(&moving_marginal_image);
        let gradients = GradientSource {
            fixed: false,
            moving: true,
        };
        let eps = R::from_f64(1.0e-16);
        type Point<R> = (Vec<R>, Option<usize>, Option<Vec<usize>>);
        let process = |i: usize| -> Option<Point<R>> {
            let s = m.sample(i, transform, gradients, true, floor)?;
            if s.moving_value < self.moving_min || s.moving_value > self.moving_max {
                return None;
            }
            let point = self.joint_point(s.fixed_value, s.moving_value);
            if !joint_interp.is_inside(&point) {
                return None;
            }
            let joint_value = R::from_f64(joint_interp.evaluate(&point, floor));
            let d_joint = self.derivative_along(&joint_interp, &point, 1, floor);
            let mind = [point[1]];
            let moving_value = R::from_f64(marginal_interp.evaluate(&mind, floor));
            let d_moving = self.derivative_along(&marginal_interp, &mind, 0, floor);
            let scaling = if joint_value > eps && moving_value > eps {
                let ratio = joint_value.ln() - moving_value.ln();
                let term1 = d_joint * ratio;
                let term2 = self.log2 * d_moving * joint_value / moving_value;
                term2 - term1
            } else {
                R::ZERO
            };
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            let jac = m.jacobian_columns(transform, &vp);
            let voxel = if m.local_support {
                m.voxel_of(&s.virtual_index)
            } else {
                None
            };
            Some((
                (0..jac.n)
                    .map(|par| {
                        let mut sum = R::ZERO;
                        for dim in 0..D {
                            sum = sum + scaling * jac.at(dim, par) * s.moving_gradient[dim];
                        }
                        sum
                    })
                    .collect(),
                voxel,
                jac.cols,
            ))
        };
        let mut reduction = BaseReduction::for_metric(m, transform, true);
        m.for_each_sample_of(
            m.sample_width(transform),
            process,
            |(local, voxel, cols)| {
                reduction.add_point_cols(R::ZERO, &local, cols.as_deref(), voxel);
                Ok(())
            },
        )?;
        let reduced = reduction.finish(true, np);
        if reduced.no_valid_points {
            warnings.push(no_valid_points_warning(
                "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx",
                544,
                CLASS,
            ));
            self.set_value(reduced.value);
            return Ok((reduced.value, reduced.derivative));
        }
        let v = self.compute_value(&d);
        self.set_value(v);
        Ok((v, reduced.derivative))
    }
}

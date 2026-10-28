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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// Whether the threader's joint histogram images exist: `BeforeThreadedExecution`
    /// creates them at the metric object's first evaluation and keeps them while the number
    /// of work units and the bins stay the same (a stage, at one thread). A level's
    /// `Initialize` keeps them: [`Mattes::share_joint_pdf`] gives every level of a stage the
    /// stage's flag.
    pub joint_pdf: Arc<AtomicBool>,
}

/// One point's contribution, computed in parallel.
struct Contribution<R> {
    fixed_index: usize,
    /// `pdfMovingIndex`, the first of the four moving bins.
    moving_index: usize,
    /// The four B-spline weights and, with the derivative, their derivatives.
    weights: [R; 4],
    derivatives: [R; 4],
    /// `Σ_dim J[dim][mu]·∇m[dim]` for each parameter `mu` (or each of `cols`).
    inner: Vec<R>,
    /// The parameters of `inner` (a B-spline transform's; the others' are zero).
    cols: Option<Vec<usize>>,
}

impl<R: Real> Mattes<R> {
    /// `Initialize`: the fixed and moving intensity ranges over the analysis region (the
    /// sampled points, or the masked or whole images) and the bin sizes, padded by two bins
    /// on each side.
    pub fn initialize<const D: usize>(
        bins: usize,
        m: &ImageMetricv4<R, D>,
    ) -> Result<Self, ItkException> {
        // NumericTraits<PixelType>::max() and NonpositiveMin(): the pixel type's range.
        let pixel_max = if m.pixel_f32 {
            R::from_f64(f64::from(f32::MAX))
        } else {
            R::MAX
        };
        let mut fmin = pixel_max;
        let mut fmax = -pixel_max;
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
                    let idx = fixed.space().point_to_index(&q);
                    if fixed.space().contains(&idx) {
                        let v = fixed.at(&idx);
                        take(v, &mut fmin, &mut fmax);
                    }
                }
            }
            None => match &m.fixed_mask {
                Some(mask) => {
                    let space = fixed.space();
                    fixed.for_each_value(|o, v| {
                        let p = space.index_to_point(&space.index_of(o));
                        if mask.is_inside(&p) {
                            take(v, &mut fmin, &mut fmax);
                        }
                    });
                }
                None => fixed.for_each_value(|_, v| take(v, &mut fmin, &mut fmax)),
            },
        }
        let mut mmin = pixel_max;
        let mut mmax = -pixel_max;
        let moving = &m.moving;
        match &m.moving_mask {
            Some(mask) => {
                let space = moving.space();
                moving.for_each_value(|o, v| {
                    let p = space.index_to_point(&space.index_of(o));
                    if mask.is_inside(&p) {
                        take(v, &mut mmin, &mut mmax);
                    }
                });
            }
            None => moving.for_each_value(|_, v| take(v, &mut mmin, &mut mmax)),
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
            joint_pdf: Arc::default(),
        })
    }

    /// The metric object's joint histogram images, shared by the levels (and the calls) of
    /// one stage.
    pub fn share_joint_pdf(&mut self, flag: &Arc<AtomicBool>) {
        self.joint_pdf = Arc::clone(flag);
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

    /// The threader's `BeforeThreadedExecution` allocates the joint histogram as an image
    /// whose spacing is the two bin sizes; `ImageBase::SetSpacing` (ITK built with
    /// `ITK_LEGACY_REMOVE`, as ANTs builds it) throws for a zero or negative spacing: an
    /// analysis region of one intensity, or of none (an empty mask). The image is then left
    /// without a buffer, so a later evaluation of the same metric crashes in ITK
    /// (`FillBuffer`); this is the first evaluation's exception (from the default spacing
    /// `[1, 1]`).
    ///
    /// When the images are created, `SetSpacing` inverts `Direction · diag(spacing)`
    /// (`ComputeIndexToPhysicalPointMatrices`, `Matrix::GetInverse`: vnl's SVD): a NaN or
    /// infinite bin size (an image with NaN or infinite values) makes LINPACK fail, and vnl
    /// prints its warning on standard error, once per image (one per work unit: once at one
    /// thread); a determinant of 0 throws (found by differential random testing,
    /// 2026-10-25).
    ///
    /// ITK-100: once the images exist (a later level of the stage), `SetSpacing` is not called
    /// again and a bin size of 0 or less is used silently (the metric 0); `reproduce`
    /// (`outside.alias`) does so, the default gives the first level's exception.
    pub fn check_joint_pdf_spacing(
        &self,
        outside: &super::OutsideDerivatives,
        warnings: &mut Vec<String>,
    ) -> Result<(), ItkException> {
        let spacing = [self.fixed_bin_size.to_f64(), self.moving_bin_size.to_f64()];
        let o = crate::format::OStream::default();
        let to = format!("[{}, {}]", o.float(spacing[0]), o.float(spacing[1]));
        if self.joint_pdf.load(Ordering::Relaxed) {
            if spacing.iter().all(|&v| v > 0.0 || v.is_nan()) {
                return Ok(());
            }
            outside.count_stale_bins();
            if outside.alias {
                return Ok(());
            }
        }
        for &v in &spacing {
            if v == 0.0 {
                return Err(ItkException::new(
                    "Modules/Core/Common/include/itkImageBase.hxx",
                    77,
                    "Image",
                    format!(
                        "Zero-valued spacing is not supported and may result in undefined behavior.\nRefusing to change spacing from [1, 1] to {to}"
                    ),
                ));
            }
            if v < 0.0 {
                return Err(ItkException::new(
                    "Modules/Core/Common/include/itkImageBase.hxx",
                    90,
                    "Image",
                    format!(
                        "Negative spacing is not supported and may result in undefined behavior.\nRefusing to change spacing from [1, 1] to {to}"
                    ),
                ));
            }
        }
        if !self.joint_pdf.swap(true, Ordering::Relaxed) {
            // Direction (the identity) · diag(spacing), summed as vnl_matrix_fixed multiplies.
            let [s0, s1] = spacing;
            let m = [
                1.0 * s0 + 0.0 * 0.0,
                1.0 * 0.0 + 0.0 * s1,
                0.0 * s0 + 1.0 * 0.0,
                0.0 * 0.0 + 1.0 * s1,
            ];
            if m[0] * m[3] - m[1] * m[2] == 0.0 {
                return Err(ItkException::new(
                    "Modules/Core/Common/include/itkMatrix.h",
                    256,
                    "",
                    "Singular matrix. Determinant is 0.",
                ));
            }
            if let Err(info) = larmorx_core::vnl_svd::vnl_svd_info(&m, 2, 2) {
                warnings.push(larmorx_core::vnl_svd::suspicious_svdc(&m, 2, 2, info));
            }
        }
        Ok(())
    }

    /// `GetValueAndDerivative` for a transform of local support (SyN's identity field): each
    /// point's four B-spline derivative contributions `Σ_dim J[dim][mu]·∇m[dim] · β′` are kept
    /// per moving bin at its voxel (`m_LocalDerivativeByParzenBin`), with the joint
    /// histogram's row-and-column index of its first bin (`m_JointPdfIndex1DArray`); after the
    /// histogram, each entry of the derivative subtracts, for each of the four bins,
    /// its contribution times `log(p(f,m)/p(m)) / (binSize·N)` of that bin (`m_PRatioArray`, 0
    /// for empty bins). Points outside the virtual region (ITK writes outside its arrays)
    /// keep only their histogram contribution.
    pub fn evaluate_local<const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        self.check_joint_pdf_spacing(&m.outside, warnings)?;
        let bins = self.bins;
        let total = D * m.virtual_space.len();
        let gradients = GradientSource {
            fixed: false,
            moving: true,
        };
        let mut fixed_marginal = vec![R::ZERO; bins];
        let mut joint = vec![R::ZERO; bins * bins];
        let mut pdf_index = vec![0usize; total];
        let mut by_bin: [Vec<R>; 4] = std::array::from_fn(|_| vec![R::ZERO; total]);
        let mut valid: usize = 0;
        let process = |i: usize| -> Option<(Contribution<R>, Option<usize>)> {
            let s = m.sample(i, transform, gradients, true, floor)?;
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
                derivatives[b] = cubic_derivative(arg);
                arg = R::from_f64(arg.to_f64() + 1.0);
            }
            let mut jac = vec![R::ZERO; D * D];
            let local: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
            m.moving_jacobian(transform, &local, &mut jac);
            let inner = (0..D)
                .map(|mu| {
                    let mut ip = R::ZERO;
                    for dim in 0..D {
                        ip = ip + jac[dim * D + mu] * s.moving_gradient[dim];
                    }
                    ip
                })
                .collect();
            Some((
                Contribution {
                    fixed_index,
                    moving_index,
                    weights,
                    derivatives,
                    inner,
                    cols: None,
                },
                m.voxel_of(&s.virtual_index),
            ))
        };
        m.for_each_sample(process, |(c, voxel)| {
            fixed_marginal[c.fixed_index] = fixed_marginal[c.fixed_index] + R::ONE;
            let row = c.fixed_index * bins;
            if let Some(v) = voxel {
                for i in 0..D {
                    pdf_index[v * D + i] = c.moving_index + row;
                }
            }
            for b in 0..4 {
                let bin = row + c.moving_index + b;
                joint[bin] = joint[bin] + c.weights[b];
                if let Some(v) = voxel {
                    for mu in 0..D {
                        let cell = &mut by_bin[b][v * D + mu];
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
        let n_factor =
            R::from_f64(1.0 / (self.moving_bin_size * R::from_i64(valid as i64)).to_f64());
        let mut p_ratio = vec![R::ZERO; bins * bins];
        let mut total_value = R::ZERO;
        for f in 0..bins {
            let fm = fixed_marginal[f];
            if fm > close {
                let log_fm = fm.ln();
                for mi in 0..bins {
                    let mm = moving_marginal[mi];
                    let j = joint[f * bins + mi];
                    if mm > close && j > close {
                        let ratio = (j / mm).ln();
                        total_value = total_value + j * (ratio - log_fm);
                        p_ratio[mi + f * bins] = ratio * n_factor;
                    }
                }
            }
        }
        let mut derivative = vec![R::ZERO; total];
        for (i, d) in derivative.iter_mut().enumerate() {
            for b in 0..4 {
                *d = *d - by_bin[b][i] * p_ratio[pdf_index[i] + b];
            }
        }
        Ok((R::from_f64(-total_value.to_f64()), derivative))
    }

    /// `GetValue` (`compute_derivative` false) or `GetValueAndDerivative`: the value and the
    /// derivative (empty without it).
    pub fn evaluate<const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        compute_derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        if m.local_support && compute_derivative {
            return self.evaluate_local(m, transform, floor, warnings);
        }
        self.check_joint_pdf_spacing(&m.outside, warnings)?;
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
            let mut cols = None;
            if compute_derivative {
                let local: [R; D] = std::array::from_fn(|k| R::from_f64(s.virtual_point[k]));
                let jac = m.jacobian_columns(transform, &local);
                inner = (0..jac.n)
                    .map(|mu| {
                        let mut ip = R::ZERO;
                        for dim in 0..D {
                            ip = ip + jac.at(dim, mu) * s.moving_gradient[dim];
                        }
                        ip
                    })
                    .collect();
                cols = jac.cols;
            }
            Some(Contribution {
                fixed_index,
                moving_index,
                weights,
                derivatives,
                inner,
                cols,
            })
        };
        let width = if compute_derivative {
            m.sample_width(transform)
        } else {
            0
        };
        m.for_each_sample_of(width, process, |c| {
            fixed_marginal[c.fixed_index] = fixed_marginal[c.fixed_index] + R::ONE;
            let row = c.fixed_index * bins;
            for b in 0..4 {
                let bin = row + c.moving_index + b;
                joint[bin] = joint[bin] + c.weights[b];
                if compute_derivative {
                    // Plain sums: a B-spline transform's zero columns add nothing.
                    let offset = c.fixed_index * np * bins + (c.moving_index + b) * np;
                    for (k, &ip) in c.inner.iter().enumerate() {
                        let mu = c.cols.as_ref().map_or(k, |cols| cols[k]);
                        let cell = &mut joint_derivatives[offset + mu];
                        *cell = *cell + ip * c.derivatives[b];
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

#[cfg(test)]
mod tests {
    use super::*;

    /// ITK-100 (random testing, 2026-10-25, `ants-registration:20261025:884`): the joint
    /// histogram images of a stage's first level are kept, so a later level's bin size of 0
    /// never reaches `SetSpacing`'s exception in ITK; `reproduce` uses it as ITK does, the
    /// default gives the first level's exception; both count it for the notice.
    #[test]
    fn a_later_level_zero_bin_size_is_used_as_itk_uses_it() {
        let state = |bin: f64, created: bool| Mattes::<f64> {
            bins: 8,
            fixed_true_min: 1.0,
            fixed_true_max: 1.0,
            moving_true_min: 0.0,
            moving_true_max: 1.0,
            fixed_bin_size: bin,
            fixed_normalized_min: 1.0 / bin - 2.0,
            moving_bin_size: 0.25,
            moving_normalized_min: -2.0,
            joint_pdf: Arc::new(AtomicBool::new(created)),
        };
        let mut w = Vec::new();
        // The first level: ITK's exception in both modes.
        for alias in [false, true] {
            let outside = crate::metric::OutsideDerivatives::new(alias);
            let e = state(0.0, false)
                .check_joint_pdf_spacing(&outside, &mut w)
                .unwrap_err();
            assert_eq!(e.line, 77);
            assert_eq!(outside.stale_bins(), 0);
        }
        // A later level: used in reproduce, the exception by default; counted in both.
        let reproduce = crate::metric::OutsideDerivatives::new(true);
        assert!(
            state(0.0, true)
                .check_joint_pdf_spacing(&reproduce, &mut w)
                .is_ok()
        );
        assert_eq!(reproduce.stale_bins(), 1);
        let fix = crate::metric::OutsideDerivatives::new(false);
        assert!(
            state(0.0, true)
                .check_joint_pdf_spacing(&fix, &mut w)
                .is_err()
        );
        assert_eq!(fix.stale_bins(), 1);
        // A positive bin size at a later level: nothing to name.
        assert!(
            state(0.5, true)
                .check_joint_pdf_spacing(&fix, &mut w)
                .is_ok()
        );
        assert_eq!(fix.stale_bins(), 1);
        assert!(w.is_empty());
    }
}

// SPDX-License-Identifier: Apache-2.0
//! The Gaussian models of Atropos: ITK's `GaussianMembershipFunction` (ITK v5.4.5,
//! `itkGaussianMembershipFunction.hxx`), estimated as `WeightedCovarianceSampleFilter` and
//! `WeightedMeanSampleFilter` estimate it, and ANTs' `GaussianListSampleFunction` and
//! `PartialVolumeGaussianListSampleFunction` (ANTs v2.6.5, `ImageSegmentation/`).
//!
//! Everything is in `f64` as in ITK: the weighted mean is a compensated (Kahan) sum of
//! `x · w` divided by the plain sum of the weights; the covariance sums `(w · d_r) · d_c` over
//! the lower triangle and multiplies by `1 / (Σw − Σw² / Σw)`; the inverse and the determinant
//! come from vnl's SVD (`vnl_matrix_inverse`).

use larmorx_core::vnl_svd::{vnl_inverse, vnl_svd};

/// Kahan summation (`itk::CompensatedSummation<double>`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Kahan {
    sum: f64,
    compensation: f64,
}

impl Kahan {
    #[inline]
    pub(crate) fn add(&mut self, element: f64) {
        let compensated = element - self.compensation;
        let t = self.sum + compensated;
        self.compensation = (t - self.sum) - compensated;
        self.sum = t;
    }

    pub(crate) fn sum(&self) -> f64 {
        self.sum
    }

    pub(crate) fn compensation(&self) -> f64 {
        self.compensation
    }
}

/// Why an estimate failed (an ITK exception: Atropos stops).
#[derive(Clone, Debug, PartialEq)]
pub struct EstimateError {
    /// The ITK source file and line, and the class.
    pub file: &'static str,
    pub line: u32,
    pub class: &'static str,
    pub description: String,
}

/// `itk::Statistics::GaussianMembershipFunction<Array<float>>`.
#[derive(Clone, Debug, PartialEq)]
pub struct GaussianMembership {
    pub mean: Vec<f64>,
    /// Row-major `k × k`.
    pub covariance: Vec<f64>,
    pub inverse: Vec<f64>,
    pub prefactor: f64,
    pub nonsingular: bool,
}

impl Default for GaussianMembership {
    /// The constructor: measurement size 0, the univariate prefactor `1/√(2π)`.
    fn default() -> Self {
        GaussianMembership {
            mean: Vec::new(),
            covariance: Vec::new(),
            inverse: Vec::new(),
            prefactor: 1.0 / (2.0 * std::f64::consts::PI).sqrt(),
            nonsingular: true,
        }
    }
}

/// `vnl_matrix_inverse<double>(m).determinant_magnitude()`: the product of the singular values.
pub(crate) fn determinant_magnitude(m: &[f64], k: usize) -> f64 {
    match vnl_svd(m, k, k) {
        Some((_, w, _)) => {
            let mut p = w[0];
            for &x in &w[1..] {
                p *= x;
            }
            p
        }
        None => f64::NAN,
    }
}

impl GaussianMembership {
    pub fn size(&self) -> usize {
        self.mean.len()
    }

    /// `SetMean` then `SetCovariance` (which does nothing when the covariance is unchanged).
    pub fn set(&mut self, mean: Vec<f64>, covariance: Vec<f64>) -> Result<(), EstimateError> {
        let k = mean.len();
        self.mean = mean;
        if self.covariance == covariance {
            return Ok(());
        }
        self.covariance = covariance;
        let det = determinant_magnitude(&self.covariance, k);
        if det < 0.0 {
            return Err(EstimateError {
                file: "Modules/Numerics/Statistics/include/itkGaussianMembershipFunction.hxx",
                line: 120,
                class: "GaussianMembershipFunction",
                description: "det( m_Covariance ) < 0".into(),
            });
        }
        self.nonsingular = det > 1.0e-6;
        if self.nonsingular {
            self.inverse =
                vnl_inverse(&self.covariance, k).unwrap_or_else(|| vec![f64::NAN; k * k]);
            let root = (2.0 * std::f64::consts::PI).sqrt();
            self.prefactor = 1.0 / (det.sqrt() * larmorx_core::math::pow(root, k as f64));
        } else {
            let large = larmorx_core::math::pow(f64::MAX, 1.0 / 3.0) / k as f64;
            self.inverse = (0..k * k)
                .map(|i| if i % (k + 1) == 0 { large } else { 0.0 })
                .collect();
            self.prefactor = 1.0;
        }
        Ok(())
    }

    /// `Evaluate`: `prefactor · exp(−½ dᵀ Σ⁻¹ d)`, `d = x − μ` in `f64`.
    #[inline]
    pub fn evaluate(&self, x: &[f32]) -> f64 {
        let k = self.mean.len();
        let mut temp = 0.0f64;
        for r in 0..k {
            let mut rowdot = 0.0f64;
            for c in 0..k {
                rowdot += self.inverse[r * k + c] * (f64::from(x[c]) - self.mean[c]);
            }
            temp += rowdot * (f64::from(x[r]) - self.mean[r]);
        }
        self.prefactor * larmorx_core::math::exp(-0.5 * temp)
    }
}

/// `WeightedMeanSampleFilter::ComputeMeanWithWeights` on `n × m` samples.
pub(crate) fn weighted_mean(
    samples: &[f32],
    m: usize,
    weights: &[f64],
) -> Result<Vec<f64>, EstimateError> {
    let mut sum = vec![Kahan::default(); m];
    let mut total = 0.0f64;
    for (i, &w) in weights.iter().enumerate() {
        total += w;
        let x = &samples[i * m..(i + 1) * m];
        for d in 0..m {
            sum[d].add(f64::from(x[d]) * w);
        }
    }
    if total > f64::EPSILON {
        Ok(sum.iter().map(|s| s.sum() / total).collect())
    } else {
        Err(EstimateError {
            file: "Modules/Numerics/Statistics/include/itkWeightedMeanSampleFilter.hxx",
            line: 117,
            class: "WeightedMeanSampleFilter",
            description: format!(
                "Total weight was too close to zero. Value = {}",
                crate::cli::cstd::format_g(total)
            ),
        })
    }
}

/// `WeightedCovarianceSampleFilter::ComputeCovarianceMatrixWithWeights`: the weighted mean and
/// the covariance (row-major).
pub(crate) fn weighted_covariance(
    samples: &[f32],
    m: usize,
    weights: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), EstimateError> {
    let mean = weighted_mean(samples, m, weights)?;
    let mut out = vec![0.0f64; m * m];
    let mut total = 0.0f64;
    let mut total_sq = 0.0f64;
    let mut diff = vec![0.0f64; m];
    for (i, &w) in weights.iter().enumerate() {
        total += w;
        total_sq += w * w;
        let x = &samples[i * m..(i + 1) * m];
        for d in 0..m {
            diff[d] = f64::from(x[d]) - mean[d];
        }
        for r in 0..m {
            for c in 0..=r {
                out[r * m + c] += w * diff[r] * diff[c];
            }
        }
    }
    for r in 1..m {
        for c in 0..r {
            out[c * m + r] = out[r * m + c];
        }
    }
    let norm = total - total_sq / total;
    if norm > f64::EPSILON {
        let inv = 1.0 / norm;
        for v in out.iter_mut() {
            *v *= inv;
        }
        Ok((mean, out))
    } else {
        Err(EstimateError {
            file: "Modules/Numerics/Statistics/include/itkWeightedCovarianceSampleFilter.hxx",
            line: 248,
            class: "WeightedCovarianceSampleFilter",
            description: format!(
                "Normalization factor was too close to zero. Value = {}",
                crate::cli::cstd::format_g(norm)
            ),
        })
    }
}

/// ANTs' `GaussianListSampleFunction`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GaussianModel {
    pub gaussian: GaussianMembership,
}

/// What `SetInputListSample` said besides its result: ITK's warning for a sample of at most one
/// element (the model then keeps its previous parameters).
pub const SMALL_SAMPLE: &str =
    "The input list sample has <= 1 element.  Function evaluations will be equal to 0.";

impl GaussianModel {
    /// `SetInputListSample` with the weights set before: the weighted estimate, then ANTs'
    /// check of the covariance's determinant. `Ok(Some(warning))` for a sample of at most one
    /// element.
    pub fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = weights.len();
        if n <= 1 {
            return Ok(Some(SMALL_SAMPLE));
        }
        let (mean, cov) = weighted_covariance(samples, m, weights)?;
        self.gaussian.set(mean, cov)?;
        let det = determinant_magnitude(&self.gaussian.covariance, m);
        if det < 0.0 {
            return Err(EstimateError {
                file: "ImageSegmentation/antsGaussianListSampleFunction.hxx",
                line: 96,
                class: "GaussianListSampleFunction",
                description: "Determinant of the covariance < 0".into(),
            });
        } else if det < 1.0e-6 {
            return Err(EstimateError {
                file: "ImageSegmentation/antsGaussianListSampleFunction.hxx",
                line: 100,
                class: "GaussianListSampleFunction",
                description: format!(
                    "Covariance is singular (determinant = {} < 1.0e-6)",
                    crate::cli::cstd::format_g(det)
                ),
            });
        }
        Ok(None)
    }

    /// `Evaluate`, returned as `float`.
    #[inline]
    pub fn evaluate(&self, x: &[f32]) -> f32 {
        self.gaussian.evaluate(x) as f32
    }
}

/// ANTs' `PartialVolumeGaussianListSampleFunction`: the mean of the two classes' weighted
/// means, and a diagonal covariance `m₀²/12 − m₀m₁/6 + m₁²/12`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartialVolumeModel {
    pub means: [Vec<f64>; 2],
    pub calculated: [bool; 2],
    pub gaussian: GaussianMembership,
}

impl PartialVolumeModel {
    /// `SetIndexedInputListSample(d, sample)` with the weights set before.
    pub fn set_sample(
        &mut self,
        d: usize,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = samples.len() / m.max(1);
        let mut warning = None;
        if n > 1 {
            // The weighted mean (weights of the sample's size), else the plain mean.
            self.means[d] = if weights.len() == n {
                weighted_mean(samples, m, weights)?
            } else {
                weighted_mean(samples, m, &vec![1.0; n])?
            };
        } else {
            warning = Some("The input list sample has <= 1 element.");
        }
        self.calculated[d] = true;
        if self.calculated[0] && self.calculated[1] {
            if self.means[0].len() != self.means[1].len() {
                return Err(EstimateError {
                    file: "ImageSegmentation/antsPartialVolumeGaussianListSampleFunction.hxx",
                    line: 126,
                    class: "PartialVolumeGaussianListSampleFunction",
                    description: "Mean sizes are unequal.".into(),
                });
            }
            let k = self.means[0].len();
            let mut mean = vec![0.0; k];
            let mut cov = vec![0.0; k * k];
            for i in 0..k {
                cov[i * k + i] = 1.0;
            }
            for i in 0..k {
                let (a, b) = (self.means[0][i], self.means[1][i]);
                mean[i] = 0.5 * (a + b);
                cov[i * k + i] = 1.0 / 12.0 * (a * a) + -1.0 / 6.0 * a * b + 1.0 / 12.0 * (b * b);
            }
            self.gaussian.set(mean, cov)?;
        }
        Ok(warning)
    }

    #[inline]
    pub fn evaluate(&self, x: &[f32]) -> f32 {
        if self.calculated[0] && self.calculated[1] {
            self.gaussian.evaluate(x) as f32
        } else {
            0.0
        }
    }
}

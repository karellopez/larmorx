// SPDX-License-Identifier: Apache-2.0
//! Atropos' likelihood models (`-k`): ANTs' list-sample functions (ANTs v2.6.5,
//! `ImageSegmentation/ants*ListSampleFunction.hxx`).

use super::gaussian::{EstimateError, GaussianModel, PartialVolumeModel};

/// The model chosen with `-k`, with its parameters.
#[derive(Clone, Debug, PartialEq)]
pub enum LikelihoodModel {
    Gaussian,
    /// `HistogramParzenWindows[sigma, bins]` (1, 32).
    HistogramParzenWindows { sigma: f32, bins: u32 },
    /// `ManifoldParzenWindows[pointSetSigma, evaluationK, covarianceK, kernelSigma]`
    /// (1, 50, 0, 1).
    ManifoldParzenWindows {
        regularization_sigma: f32,
        evaluation_k: u32,
        covariance_k: u32,
        kernel_sigma: f32,
    },
    /// `JointShapeAndOrientationProbability[shapeSigma, shapeBins, orientationSigma,
    /// orientationBins]` (2, 64, 1, 32).
    JointShapeAndOrientationProbability {
        shape_sigma: f32,
        shape_bins: u32,
        orientation_sigma: f32,
        orientation_bins: u32,
    },
    LogEuclideanGaussian,
}

/// One class's likelihood function.
#[derive(Clone, Debug)]
pub enum Likelihood {
    Gaussian(GaussianModel),
    PartialVolume(PartialVolumeModel),
}

impl Likelihood {
    /// The tissue class model for `model`.
    pub fn new(model: &LikelihoodModel) -> Self {
        match model {
            LikelihoodModel::Gaussian => Likelihood::Gaussian(GaussianModel::default()),
            _ => Likelihood::Gaussian(GaussianModel::default()),
        }
    }

    /// `SetInputListSample(sample)` with `weights` set before (`n × m` samples, row-major).
    /// `Ok(Some(warning))` for ITK's warning on a sample of at most one element.
    pub fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        match self {
            Likelihood::Gaussian(g) => g.set_sample(samples, m, weights),
            Likelihood::PartialVolume(p) => p.set_sample(0, samples, m, weights),
        }
    }

    /// `SetIndexedInputListSample(d, sample)` (partial-volume models).
    pub fn set_indexed_sample(
        &mut self,
        d: usize,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        match self {
            Likelihood::PartialVolume(p) => p.set_sample(d, samples, m, weights),
            other if d == 0 => other.set_sample(samples, m, weights),
            _ => Ok(None),
        }
    }

    /// `Evaluate(measurement)`.
    #[inline]
    pub fn evaluate(&self, x: &[f32]) -> f32 {
        match self {
            Likelihood::Gaussian(g) => g.evaluate(x),
            Likelihood::PartialVolume(p) => p.evaluate(x),
        }
    }
}

/// `Print(os, indent)` of a model: the header (`Name (0x0)`, the address as `0x0`) and its
/// `PrintSelf` one indent further.
pub fn print(model: &Likelihood, indent: &str) -> String {
    let next = format!("{indent}  ");
    match model {
        Likelihood::Gaussian(g) => format!(
            "{indent}GaussianListSampleFunction (0x0)\n{next}{}",
            gaussian_text(&g.gaussian)
        ),
        Likelihood::PartialVolume(p) => format!(
            "{indent}PartialVolumeGaussianListSampleFunction (0x0)\n{next}{}",
            gaussian_text(&p.gaussian)
        ),
    }
}

/// `mean = [..], covariance = [a, b; c, d]` as the Gaussian functions print it: the mean
/// with ITK's `ConvertNumberToString`, the covariance with the stream's default format.
fn gaussian_text(g: &super::gaussian::GaussianMembership) -> String {
    let mean: Vec<String> = g
        .mean
        .iter()
        .map(|&x| larmorx_io::itk_transform::itk_number(x))
        .collect();
    let k = g.mean.len();
    let mut s = format!("mean = [{}], covariance = [", mean.join(", "));
    if k == 0 || g.covariance.len() != k * k {
        s.push_str("]\n");
        return s;
    }
    for r in 0..k {
        for c in 0..k - 1 {
            s.push_str(&crate::cli::cstd::format_g(g.covariance[r * k + c]));
            s.push_str(", ");
        }
        s.push_str(&crate::cli::cstd::format_g(g.covariance[r * k + k - 1]));
        s.push_str(if r == k - 1 { "]\n" } else { "; " });
    }
    s
}

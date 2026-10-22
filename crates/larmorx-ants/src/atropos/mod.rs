// SPDX-License-Identifier: Apache-2.0
//! `Atropos`, ANTs' finite-mixture segmentation with an MRF prior (ANTs v2.6.5,
//! `ImageSegmentation/antsAtroposSegmentationImageFilter.hxx` and the list-sample functions and
//! filters beside it, on ITK v5.4.5), ported operation for operation.
//!
//! [`Atropos`] is the filter (`AtroposSegmentationImageFilter<Image<float, D>, Image<unsigned
//! int, D>>`): build it with the intensity images, the mask and [`AtroposOptions`], run
//! [`Atropos::run`] (`GenerateData`), then ask for the outputs as `Atropos.cxx` does
//! ([`Atropos::posterior`], [`Atropos::likelihood`], [`Atropos::distance_prior`],
//! [`Atropos::smooth_intensity`], [`Atropos::icm_codes`]). The command line is
//! [`crate::cli::atropos`]; what ANTs really computes, and where larmorx corrects it (D13), is
//! in `docs/findings/ants-atropos.md`.
//!
//! **Determinism.** ANTs computes the posteriors in parallel (each voxel on its own) and the
//! rest sequentially; larmorx does the same work in parallel where it is per voxel (the
//! posteriors, the likelihoods, the ICM updates of one code, whose random draws are made first
//! in ANTs' order) and keeps every sum in ANTs' order, so its result does not depend on the
//! number of threads.

// Index loops, `-1.0 * x` and the SLATEC tables' literals mirror the C++ and Fortran sources
// (`-1.0 * x` and `-x` are the same bits; the literals round to the same doubles); the
// likelihood models are one per class, so their sizes do not matter.
#![allow(
    clippy::needless_range_loop,
    clippy::neg_multiply,
    clippy::excessive_precision,
    clippy::large_enum_variant
)]

mod fast_marching;
mod filter;
pub mod gaussian;
pub mod models;
pub mod outliers;
mod smoothing;
pub mod tdist;

use std::collections::BTreeMap;

use larmorx_core::defects::UpstreamDefects;

pub use filter::{Atropos, AtroposError, IterationRecord};
pub use models::{Likelihood, LikelihoodModel};
pub use outliers::OutlierHandling;

/// How the first labelling is made (`-i`).
#[derive(Clone, Debug, PartialEq)]
pub enum Initialization {
    /// `Random[K]`: a label drawn for every voxel.
    Random,
    /// `Otsu[K]`: Otsu's `K − 1` thresholds of a 200-bin histogram of the first image.
    Otsu,
    /// `KMeans[K, centres]`: k-means on the first image (from the given centres, or centres
    /// spread evenly over its range), then on every image.
    KMeans { centers: Option<Vec<f32>> },
    /// `PriorProbabilityImages[K, images, weight, threshold]`: one probability image per class
    /// (voxels in Fortran order, on the intensity images' grid).
    PriorProbabilityImages { images: Vec<Vec<f32>> },
    /// `PriorLabelImage[K, image, weight]`.
    PriorLabelImage { image: Vec<u32> },
}

impl Initialization {
    pub fn uses_priors(&self) -> bool {
        matches!(
            self,
            Initialization::PriorProbabilityImages { .. } | Initialization::PriorLabelImage { .. }
        )
    }
}

/// The posterior formulation (`-p`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Formulation {
    /// `prior^w · (likelihood · mrf)^(1 − w)`.
    #[default]
    Socrates,
    /// Socrates, with the probabilities set to 1 where the prior label or probability is 1.
    Plato,
    /// Socrates with the distance prior multiplied into the spatial prior.
    Aristotle,
    /// A sigmoid of the spatial prior times `likelihood · mrf`.
    Sigmoid,
}

/// Every setting of the filter, with ANTs' defaults in `Default`.
#[derive(Clone, Debug, PartialEq)]
pub struct AtroposOptions {
    /// Tissue classes (at least 2; ITK clamps from below).
    pub classes: u32,
    pub initialization: Initialization,
    /// The likelihood model of the tissue classes (`-k`; Gaussian by default).
    pub likelihood: LikelihoodModel,
    pub formulation: Formulation,
    pub use_mixture_model_proportions: bool,
    pub initial_annealing_temperature: f32,
    pub annealing_rate: f32,
    pub minimum_annealing_temperature: f32,
    pub max_iterations: u32,
    pub convergence_threshold: f32,
    /// `-m [smoothingFactor, radius]` (0.3 by default).
    pub mrf_smoothing: f32,
    /// `-m [coefficientImage, radius]`: a smoothing factor per voxel.
    pub mrf_coefficients: Option<Vec<f32>>,
    /// The MRF radius per axis (1 by default).
    pub mrf_radius: Vec<u32>,
    /// `--icm [asynchronous, iterations]`.
    pub asynchronous: bool,
    pub max_icm_iterations: u32,
    /// The random generator's seed (`--use-random-seed 0`: 19650218).
    pub seed: u32,
    /// `--use-euclidean-distance`.
    pub euclidean_distance: bool,
    /// `--label-propagation`: per class, `(lambda, boundary probability)`.
    pub label_parameters: BTreeMap<u32, (f32, f32)>,
    /// `--minimize-memory-usage`.
    pub minimize_memory: bool,
    /// `--winsorize-outliers`.
    pub outliers: Option<OutlierHandling>,
    /// Accepted partial-volume label sets (`-s`), in the order they were added.
    pub partial_volume_sets: Vec<Vec<u32>>,
    /// The partial-volume likelihood functions ANTs created (one per `-s`, accepted or not).
    pub partial_volume_functions: usize,
    pub use_partial_volume_likelihoods: bool,
    /// `-b [levels, mesh, order]`.
    pub spline_order: u32,
    pub levels: Vec<u32>,
    pub control_points: Vec<u32>,
    /// Per intensity image, the adaptive smoothing weight (`-a [image, weight]`).
    pub adaptive_smoothing: Vec<f32>,
    /// The prior weight (`PriorProbabilityImages[..., w]`, `PriorLabelImage[..., w]`).
    pub prior_weight: f32,
    /// The prior probability threshold.
    pub probability_threshold: f32,
    /// Every class's prior probability image is one image (ANTS-100: ANTs' classes all hold the
    /// caster's output, a vector image's last component, and its in-place normalization applies
    /// to that one image once per class). Only for reproducing ANTs.
    pub aliased_priors: bool,
    pub upstream_defects: UpstreamDefects,
}

impl AtroposOptions {
    /// ANTs' defaults for `dim` dimensions.
    pub fn new(dim: usize) -> Self {
        AtroposOptions {
            classes: 3,
            initialization: Initialization::KMeans { centers: None },
            likelihood: LikelihoodModel::Gaussian,
            formulation: Formulation::Socrates,
            use_mixture_model_proportions: true,
            initial_annealing_temperature: 1.0,
            annealing_rate: 1.0,
            minimum_annealing_temperature: 0.1,
            max_iterations: 5,
            convergence_threshold: 0.001,
            mrf_smoothing: 0.3,
            mrf_coefficients: None,
            mrf_radius: vec![1; dim],
            asynchronous: true,
            max_icm_iterations: 1,
            seed: 19_650_218,
            euclidean_distance: false,
            label_parameters: BTreeMap::new(),
            minimize_memory: false,
            outliers: None,
            partial_volume_sets: Vec::new(),
            partial_volume_functions: 0,
            use_partial_volume_likelihoods: false,
            spline_order: 3,
            levels: vec![6; dim],
            control_points: vec![4; dim],
            adaptive_smoothing: Vec::new(),
            prior_weight: 0.0,
            probability_threshold: 0.0,
            aliased_priors: false,
            upstream_defects: UpstreamDefects::Fix,
        }
    }

    /// The number of partial-volume classes the filter uses.
    pub fn partial_volume_classes(&self) -> usize {
        if self.use_partial_volume_likelihoods {
            self.partial_volume_sets.len()
        } else {
            0
        }
    }
}

/// The seed `--use-random-seed 0` sets (`19650218`, MT19937's reference seed).
pub const FIXED_SEED: u32 = 19_650_218;

/// The seed larmorx uses where ANTs seeds from the clock (`--use-random-seed 1` or no `-r`):
/// ITK's generator constructor's seed (D13 (b): no clock).
pub const CLOCK_STAND_IN_SEED: u32 = 121_212;

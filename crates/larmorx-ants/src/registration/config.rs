// SPDX-License-Identifier: Apache-2.0
//! The configuration of a registration as ANTs builds it from its command line
//! (`antsRegistrationTemplateHeader.h`, `DoRegistration`), in the form the stages take it.
//!
//! Values keep the types ANTs reads them as: a learning rate, a metric weight, a sampling
//! percentage and the smoothing sigmas are read as `float`; a convergence threshold and the
//! point-set parameters as the computation type (`f64` holding a `float` under `--float 1`).

use std::path::PathBuf;

use larmorx_core::Precision;

use super::{MetricKind, Sampling, XfrmMethod};

/// A stage's transform and its parameters (`RegistrationHelper::TransformMethod`), as given.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformSpec {
    pub method: XfrmMethod,
    /// The gradient step (`Convert<float>` of the first parameter).
    pub gradient_step: f32,
    /// The remaining parameters, as written (read by the deformable transforms in A5).
    pub parameters: Vec<String>,
    /// `SyN`/`GaussianDisplacementField`/`Exponential`: update field variance (voxels²).
    pub update_field_variance: f64,
    /// `SyN`/`GaussianDisplacementField`: total field variance.
    pub total_field_variance: f64,
    /// `TimeVaryingVelocityField`: the time variances, and the number of time indices.
    pub update_field_time_sigma: f64,
    pub total_field_time_sigma: f64,
    pub number_of_time_indices: u32,
    /// `TimeVaryingBSplineVelocityField`: samples of time (default 4).
    pub number_of_time_point_samples: u32,
    /// `Exponential`: velocity field variance.
    pub velocity_field_variance: f64,
    /// B-spline transforms: spline order (default 3), mesh sizes, integration steps.
    pub spline_order: u32,
    pub mesh_size: Vec<u32>,
    pub total_mesh_size: Vec<u32>,
    pub number_of_integration_steps: u32,
}

impl TransformSpec {
    /// A transform with only its gradient step (the linear ones).
    pub fn linear(method: XfrmMethod, gradient_step: f32) -> Self {
        TransformSpec {
            method,
            gradient_step,
            parameters: Vec::new(),
            update_field_variance: 0.0,
            total_field_variance: 0.0,
            update_field_time_sigma: 0.0,
            total_field_time_sigma: 0.0,
            number_of_time_indices: 0,
            number_of_time_point_samples: 4,
            velocity_field_variance: 0.0,
            spline_order: 3,
            mesh_size: Vec::new(),
            total_mesh_size: Vec::new(),
            number_of_integration_steps: 0,
        }
    }
}

/// An image or a point set a metric reads.
#[derive(Clone, Debug, PartialEq)]
pub enum MetricInput {
    Image(PathBuf),
    /// A labelled point set (ICP, PSE, JHCT): a file, read as `ReadLabeledPointSet` reads it.
    LabeledPoints(PathBuf),
    /// An intensity point set (IGDM): an image and its mask.
    IntensityPoints {
        image: PathBuf,
        mask: PathBuf,
    },
}

/// One metric of a stage (`RegistrationHelper::Metric`), as ANTs's `AddMetric` receives it.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricSpec {
    pub kind: MetricKind,
    pub fixed: MetricInput,
    pub moving: MetricInput,
    /// The stage (0 = the first on the command line).
    pub stage: usize,
    /// `metricWeight` (`Convert<float>`, default 1).
    pub weight: f32,
    pub sampling: Sampling,
    /// `numberOfBins` for Mattes and MI (default 32).
    pub number_of_bins: u32,
    /// `radius` for CC (default 4).
    pub radius: u32,
    /// `useGradientFilter` (default false).
    pub use_gradient_filter: bool,
    /// The sampling percentage (`Convert<float>` for image metrics; default 1).
    pub sampling_percentage: f64,
    // Point-set metrics.
    pub use_boundary_points_only: bool,
    pub point_set_sigma: f64,
    pub evaluation_k_neighborhood: u32,
    pub alpha: f64,
    pub use_anisotropic_covariances: bool,
    pub intensity_distance_sigma: f64,
    pub euclidean_distance_sigma: f64,
    pub neighborhood_radius: Vec<u32>,
    pub gradient_point_set_sigma: f64,
}

/// One registration stage as the helper runs it.
#[derive(Clone, Debug, PartialEq)]
pub struct StageSpec {
    pub transform: TransformSpec,
    pub metrics: Vec<MetricSpec>,
    /// Iterations per level (their number is the number of levels).
    pub iterations: Vec<u32>,
    /// `convergenceThreshold` (default 1e-6), in the computation type.
    pub convergence_threshold: f64,
    /// `convergenceWindowSize` (default 10, at least 2).
    pub convergence_window_size: u32,
    pub shrink_factors: Vec<u32>,
    /// Smoothing sigmas per level (`float`; one value is repeated for every level).
    pub smoothing_sigmas: Vec<f32>,
    /// `mm` (true) or `vox`/no unit (false).
    pub sigmas_in_physical_units: bool,
    /// `--restrict-deformation` weights for this stage, if given.
    pub restrict_deformation: Option<Vec<f64>>,
}

/// An initial transform (`-r`, `-q`, `-k`): a file, maybe inverted, the identity, or one
/// derived from the images.
#[derive(Clone, Debug, PartialEq)]
pub enum InitialTransformSpec {
    /// `identity` or `Identity`: a `MatrixOffsetTransformBase` identity.
    Identity,
    /// A transform file, inverted when `invert`.
    File { path: PathBuf, invert: bool },
    /// `[fixedImage,movingImage,feature]`: 0 image centres, 1 centres of mass, 2 origins
    /// (`CenteredTransformInitializer`, a rigid transform).
    FromImages {
        fixed: PathBuf,
        moving: PathBuf,
        feature: u16,
    },
}

/// Masks of a stage: fixed and moving, each a file or none (`NULL`, `NOMASK` or a file that
/// does not exist).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StageMasks {
    pub fixed: Option<PathBuf>,
    pub moving: Option<PathBuf>,
}

/// Everything `antsRegistration` was told, in the order ANTs uses it.
#[derive(Clone, Debug, PartialEq)]
pub struct RegistrationConfig {
    pub dimension: usize,
    pub precision: Precision,
    pub verbose: bool,
    pub stages: Vec<StageSpec>,
    pub output_prefix: String,
    pub output_warped: Option<String>,
    pub output_inverse_warped: Option<String>,
    pub initial_moving: Vec<InitialTransformSpec>,
    pub initial_fixed: Vec<InitialTransformSpec>,
    pub restore_state: Vec<InitialTransformSpec>,
    pub save_state: Option<String>,
    /// Masks: one entry for all stages, or one per stage (in stage order), as given.
    pub fixed_masks: Vec<Option<PathBuf>>,
    pub moving_masks: Vec<Option<PathBuf>>,
    pub winsorize: bool,
    pub lower_quantile: f32,
    pub upper_quantile: f32,
    pub use_histogram_matching: bool,
    pub collapse_output_transforms: bool,
    pub write_composite_transform: bool,
    pub initialize_transforms_per_stage: bool,
    pub minc: bool,
    /// `--random-seed` or `ANTS_RANDOM_SEED`; 0 means none (ITK seeds from its own counter).
    pub random_seed: i32,
    pub print_similarity_measure_interval: u32,
    pub write_interval_volumes: u32,
    /// `-n`, as given (lower-cased name and parameters); default `linear`.
    pub interpolation: (String, Vec<String>),
}

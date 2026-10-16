// SPDX-License-Identifier: Apache-2.0
//! ANTs' `RegistrationHelper` (ANTs v2.6.5, `Examples/itkantsRegistrationHelper.h/.hxx`): the
//! stages, their metrics and settings, and `DoRegistration`, which preprocesses each metric's
//! images, runs ITK's registration method for each stage
//! ([`larmorx_registration::method`]) and accumulates the transforms, logging what ANTs
//! logs with `--verbose 1` (`Logger()`, `PrintState`, the iteration observers).
//!
//! The linear stages are ported; the deformable ones are A5's and are refused before
//! `DoRegistration` runs.

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects, push_once};
use larmorx_registration::format::OStream;
use larmorx_registration::mask::Mask;
use larmorx_registration::method::{
    self, LevelProblem, MetricKind as MethodMetric, SamplingStrategy, Stage, StageMetric,
};
use larmorx_registration::optimizer::{ConjugateGradientLineSearch, IterationState, Observer};
use larmorx_registration::transform::{Composite, Kind, LinearXfm, Xfm};
use larmorx_registration::{Image, ItkException, Real};

use super::{MetricKind, XfrmMethod};
use crate::image::Pixel;

/// The computation types of a registration (`double`, or `float` with `--float 1`).
pub trait Compute: Real + Pixel {
    /// `typeid(R).name()` in ITK's printed class names (`double`, `float`).
    const ITK_NAME: &'static str;

    /// `static_cast<R>(v)` (ITK's conversion, as `ItkReal::from_f64`).
    #[inline]
    fn r(v: f64) -> Self {
        <Self as larmorx_core::ItkReal>::from_f64(v)
    }

    /// The image to write.
    fn output(image: crate::AntsImage<Self>) -> crate::image::OutputImage;
}

impl Compute for f64 {
    const ITK_NAME: &'static str = "double";
    fn output(image: crate::AntsImage<f64>) -> crate::image::OutputImage {
        image.into()
    }
}

impl Compute for f32 {
    const ITK_NAME: &'static str = "float";
    fn output(image: crate::AntsImage<f32>) -> crate::image::OutputImage {
        image.into()
    }
}

/// The two output streams of a run and the state of `std::cout`.
pub struct Log<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// `--verbose`: `Logger()` writes to `std::cout`; otherwise it writes nowhere.
    pub verbose: bool,
    /// `std::cout`'s format (it keeps ANTs' `std::scientific` after the first iteration line).
    pub cout: OStream,
}

impl<'a> Log<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write, verbose: bool) -> Self {
        Log {
            out,
            err,
            verbose,
            cout: OStream::default(),
        }
    }

    /// `Logger() << s`.
    pub fn logger(&mut self, s: &str) {
        if self.verbose {
            let _ = self.out.write_all(s.as_bytes());
        }
    }

    /// `std::cout << s`, verbose or not.
    pub fn cout(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }

    /// `std::cerr << s`.
    pub fn cerr(&mut self, s: &str) {
        let _ = self.err.write_all(s.as_bytes());
    }

    /// A floating value as the logger's stream prints it now.
    pub fn f(&self, v: f64) -> String {
        self.cout.float(v)
    }
}

/// The metric's sampling strategy (`RegistrationHelper::SamplingStrategy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampling {
    None,
    Regular,
    Random,
}

/// `RegistrationHelper::Metric`.
#[derive(Clone, Debug)]
pub struct Metric<R, const D: usize> {
    /// `None` for an unknown name (`IllegalMetric`).
    pub kind: Option<MetricKind>,
    pub fixed: Option<Arc<Image<R, D>>>,
    pub moving: Option<Arc<Image<R, D>>>,
    pub stage: usize,
    pub weighting: R,
    pub sampling: Sampling,
    pub number_of_bins: i32,
    pub radius: u32,
    pub use_gradient_filter: bool,
    pub use_boundary_points_only: bool,
    pub point_set_sigma: R,
    pub evaluation_k_neighborhood: R,
    pub alpha: R,
    pub use_anisotropic_covariances: bool,
    pub sampling_percentage: R,
    pub intensity_distance_sigma: R,
    pub euclidean_distance_sigma: R,
}

/// `RegistrationHelper::TransformMethod` (the fields `PrintState` shows).
#[derive(Clone, Debug)]
pub struct TransformMethod<R> {
    pub method: XfrmMethod,
    pub gradient_step: R,
    pub update_field_variance: R,
    pub total_field_variance: R,
    pub update_field_time_sigma: R,
    pub total_field_time_sigma: R,
    pub number_of_time_indices: u32,
}

impl<R: Real> TransformMethod<R> {
    /// A linear stage's method (`Add*Transform(gradientStep)`).
    pub fn linear(method: XfrmMethod, gradient_step: R) -> Self {
        TransformMethod {
            method,
            gradient_step,
            update_field_variance: R::ZERO,
            total_field_variance: R::ZERO,
            update_field_time_sigma: R::ZERO,
            total_field_time_sigma: R::ZERO,
            number_of_time_indices: 0,
        }
    }
}

/// The registration as ANTs' helper holds it.
pub struct RegistrationHelper<R, const D: usize> {
    pub metrics: Vec<Metric<R, D>>,
    pub transforms: Vec<TransformMethod<R>>,
    pub output_prefix: String,
    pub iterations: Vec<Vec<u32>>,
    pub convergence_thresholds: Vec<R>,
    pub convergence_window_sizes: Vec<u32>,
    pub smoothing_sigmas: Vec<Vec<f32>>,
    pub sigmas_in_physical_units: Vec<bool>,
    pub restrict_deformation: Vec<Vec<R>>,
    pub shrink_factors: Vec<Vec<u32>>,
    pub use_histogram_matching: bool,
    pub winsorize: bool,
    pub lower_quantile: R,
    pub upper_quantile: R,
    /// `SetRegistrationRandomSeed` (0: none; larmorx then uses [`RegistrationHelper::default_seed`]).
    pub random_seed: i32,
    /// The seed used where ANTs would seed from the clock.
    pub default_seed: i32,
    pub print_similarity_measure_interval: u32,
    pub write_interval_volumes: u32,
    pub initialize_transforms_per_stage: bool,
    /// `m_CompositeTransform`: the initial moving transforms, then each stage's result.
    pub composite: Option<Vec<Xfm<R, D>>>,
    /// `m_FixedInitialTransform`.
    pub fixed_initial: Option<Vec<Xfm<R, D>>>,
    pub fixed_masks: Vec<Option<Mask<D>>>,
    pub moving_masks: Vec<Option<Mask<D>>>,
    pub n_threads: usize,
    /// Warnings ITK printed to `std::cerr` (`itkWarningMacro`).
    pub warnings: Vec<String>,
    /// The D13 mode (ANTS-49: a linear stage that throws).
    pub upstream_defects: UpstreamDefects,
    /// The defects that changed this run, or would have.
    pub notices: Vec<Notice>,
}

impl<R: Compute, const D: usize> Default for RegistrationHelper<R, D> {
    fn default() -> Self {
        RegistrationHelper {
            metrics: Vec::new(),
            transforms: Vec::new(),
            output_prefix: String::new(),
            iterations: Vec::new(),
            convergence_thresholds: Vec::new(),
            convergence_window_sizes: Vec::new(),
            smoothing_sigmas: Vec::new(),
            sigmas_in_physical_units: Vec::new(),
            restrict_deformation: Vec::new(),
            shrink_factors: Vec::new(),
            use_histogram_matching: true,
            winsorize: false,
            lower_quantile: R::ZERO,
            upper_quantile: R::ONE,
            random_seed: 0,
            default_seed: DEFAULT_SEED,
            print_similarity_measure_interval: 0,
            write_interval_volumes: 0,
            initialize_transforms_per_stage: false,
            composite: None,
            fixed_initial: None,
            fixed_masks: Vec::new(),
            moving_masks: Vec::new(),
            n_threads: 1,
            warnings: Vec::new(),
            upstream_defects: UpstreamDefects::Fix,
            notices: Vec::new(),
        }
    }
}

/// `InitializeWithPreviousLinearTransform`: the stage's transform `output` (class name without
/// `Transform`: `name`) set from the composite's last transform where ANTs knows how
/// (translation to translation, rigid or affine; rigid to rigid or affine; affine to affine);
/// `false`, after ANTs' messages, otherwise.
fn initialize_with_previous<R: Compute, const D: usize>(
    log: &mut Log<'_>,
    previous: Option<&Xfm<R, D>>,
    name: &str,
    output: &mut Xfm<R, D>,
) -> bool {
    let Some(previous) = previous else {
        log.logger("ERROR: INITIALIZATION RETURNS FALSE. Previous Linear Transform is Null\n");
        return false;
    };
    let previous_class = previous.class();
    log.logger(&format!(
        "Try to initialize the current {name} from previous {previous_class}.\n"
    ));
    let failed = |log: &mut Log<'_>| {
        log.logger("WARNING: Initialization Failed\n");
        false
    };
    let rigid = matches!(previous_class, "Euler3DTransform" | "Euler2DTransform");
    match (name, output) {
        ("Translation", Xfm::Translation { offset }) => match previous {
            Xfm::Translation { offset: o } if previous_class == "TranslationTransform" => {
                *offset = *o;
                true
            }
            _ => failed(log),
        },
        ("Euler2D" | "Euler3D", Xfm::Linear(out)) => {
            *out = LinearXfm::new(out.kind.clone());
            match previous {
                Xfm::Translation { offset } => {
                    out.set_offset(*offset);
                    true
                }
                Xfm::Linear(prev) if rigid => {
                    // SetFixedParameters (the centre, and Euler3D's ZYX flag), SetParameters.
                    let fixed = previous.fixed_parameters();
                    if let larmorx_registration::transform::Kind::Euler3D { zyx } = &mut out.kind {
                        *zyx = fixed.get(3).is_some_and(|&v| v != 0.0);
                    }
                    out.set_center(std::array::from_fn(|i| R::r(fixed[i])));
                    out.set_parameters(&prev.parameters()).is_ok()
                }
                _ => failed(log),
            }
        }
        ("Affine", Xfm::Linear(out)) => {
            *out = LinearXfm::new(out.kind.clone());
            match previous {
                Xfm::Translation { offset } => {
                    out.set_offset(*offset);
                    true
                }
                Xfm::Linear(prev) if rigid => {
                    out.set_center(prev.center);
                    out.set_matrix(prev.matrix);
                    out.set_translation(prev.translation);
                    true
                }
                Xfm::Linear(prev) if previous_class == "AffineTransform" => {
                    let fixed = previous.fixed_parameters();
                    out.set_center(std::array::from_fn(|i| R::r(fixed[i])));
                    out.set_parameters(&prev.parameters()).is_ok()
                }
                _ => failed(log),
            }
        }
        _ => failed(log),
    }
}

/// The seed larmorx uses where ANTs, given no seed, seeds its sampling from the clock (its
/// result then changes from run to run): the Mersenne twister's own default seed in ITK.
pub const DEFAULT_SEED: i32 = 121_212;

/// Why `DoRegistration` failed: what ANTs printed has been logged; the exit status is 1.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct RegistrationFailed(pub String);

/// `ShrinkFactorsPerDimension` (`CalculateShrinkFactorsPerDimension`): the factor for the
/// axis of smallest spacing, and for each other axis the factor (up to it, preferring the
/// smaller on ties) whose shrunk spacing is closest to that axis's.
pub fn shrink_factors_per_dimension<const D: usize>(factor: u32, spacing: &[f64; D]) -> [usize; D] {
    let mut min_spacing = spacing[0];
    let mut min_index = 0;
    for (n, &s) in spacing.iter().enumerate().skip(1) {
        if min_spacing > s {
            min_spacing = s;
            min_index = n;
        }
    }
    let mut out = [0usize; D];
    out[min_index] = factor as usize;
    let new_min = spacing[min_index] * f64::from(factor);
    for n in 0..D {
        if out[n] == 0 {
            let mut best = (spacing[n] * f64::from(factor) - new_min).abs();
            let mut best_factor = factor;
            let mut f = factor;
            while f > 1 {
                f -= 1;
                let d = (spacing[n] * f64::from(f) - new_min).abs();
                if d <= best {
                    best = d;
                    best_factor = f;
                }
            }
            out[n] = best_factor as usize;
        }
    }
    out
}

/// ITK's `Print` of an image (`PrintState`'s "Fixed image = ..."): object addresses and
/// modification times are not reproducible and are printed as 0.
pub fn itk_image_print<R: Compute, const D: usize>(image: &Image<R, D>) -> String {
    let fmt = OStream::default();
    let ints = |v: &[i64]| -> String {
        format!(
            "[{}]",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let s = &image.space;
    let size: Vec<i64> = s.size.iter().map(|&n| n as i64).collect();
    let region = |name: &str| -> String {
        format!(
            "  {name}: \n    Dimension: {D}\n    Index: {}\n    Size: {}\n",
            ints(&s.start),
            ints(&size)
        )
    };
    let matrix = |m: &[[f64; D]; D]| -> String {
        let mut out = String::new();
        for row in m {
            let r: Vec<String> = row.iter().map(|&v| fmt.float(v)).collect();
            out.push_str(&r.join(" "));
            out.push('\n');
        }
        out
    };
    let mut out = String::new();
    out.push_str("Image (0x0)\n");
    out.push_str(&format!(
        "  RTTI typeinfo:   itk::Image<{}, {D}u>\n",
        R::ITK_NAME
    ));
    out.push_str("  Reference Count: 2\n  Modified Time: 0\n  Debug: Off\n  Object Name: \n");
    out.push_str("  Observers: \n    none\n  Source: (none)\n  Source output name: (none)\n");
    out.push_str("  Release Data: Off\n  Data Released: False\n  Global Release Data: Off\n");
    out.push_str("  PipelineMTime: 0\n  UpdateMTime: 0\n  RealTimeStamp: 0 seconds \n");
    out.push_str(&region("LargestPossibleRegion"));
    out.push_str(&region("BufferedRegion"));
    out.push_str(&region("RequestedRegion"));
    out.push_str(&format!("  Spacing: {}\n", fmt.array(&s.spacing)));
    out.push_str(&format!("  Origin: {}\n", fmt.array(&s.origin)));
    out.push_str(&format!("  Direction: \n{}\n", matrix(&s.direction)));
    out.push_str(&format!(
        "  IndexToPointMatrix: \n{}\n",
        matrix(&s.index_to_point)
    ));
    out.push_str(&format!(
        "  PointToIndexMatrix: \n{}\n",
        matrix(&s.point_to_index)
    ));
    out.push_str(&format!(
        "  Inverse Direction: \n{}\n",
        matrix(&s.inverse_direction)
    ));
    out.push_str("  PixelContainer: \n    ImportImageContainer (0x0)\n");
    out.push_str(&format!(
        "      RTTI typeinfo:   itk::ImportImageContainer<unsigned long, {}>\n",
        R::ITK_NAME
    ));
    out.push_str("      Reference Count: 1\n      Modified Time: 0\n      Debug: Off\n");
    out.push_str("      Object Name: \n      Observers: \n        none\n      Pointer: 0x0\n");
    out.push_str(&format!(
        "      Container manages memory: true\n      Size: {}\n      Capacity: {}\n\n",
        s.len(),
        s.len()
    ));
    out
}

/// ANTs' iteration observer of a linear stage (`antsRegistrationOptimizerCommandIterationUpdate`).
struct IterationObserver<'l, 'a, R> {
    log: &'l mut Log<'a>,
    iterations: Vec<u32>,
    current_level: usize,
    clock: Instant,
    last: f64,
    _r: std::marker::PhantomData<R>,
}

impl<'p, R: Compute, const D: usize> Observer<R, LevelProblem<'p, R, D>>
    for IterationObserver<'_, '_, R>
{
    fn iteration(
        &mut self,
        state: &IterationState<R>,
        number_of_iterations: &mut usize,
        _problem: &mut LevelProblem<'p, R, D>,
    ) -> Result<(), ItkException> {
        let current = state.iteration + 1;
        if current == 1 {
            *number_of_iterations = self.iterations[self.current_level] as usize;
            self.current_level += 1;
            self.log.logger(
                "DIAGNOSTIC,Iteration,metricValue,convergenceValue,ITERATION_TIME_INDEX,SINCE_LAST\n",
            );
        }
        let now = self.clock.elapsed().as_secs_f64();
        self.log.logger(" ");
        let sci12 = OStream {
            scientific: true,
            precision: 12,
        };
        let sci4 = OStream {
            scientific: true,
            precision: 4,
        };
        let line = format!(
            "2DIAGNOSTIC, {:>5}, {}, {}, {}, {}, \n",
            current,
            sci12.float(state.value.to_f64()),
            sci12.float(state.convergence.to_f64()),
            sci4.float(now),
            sci4.float(now - self.last)
        );
        self.log.logger(&line);
        if self.log.verbose {
            self.log.cout = sci4;
        }
        self.last = now;
        Ok(())
    }
}

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// `PrintState()`.
    fn print_state(&self, log: &mut Log<'_>) {
        let mut s = format!(
            "Dimension = {D}\nNumber of stages = {}\nUse histogram matching = {}\nWinsorize image intensities = {}\n  Lower quantile = {}\n  Upper quantile = {}\n\n\n",
            self.transforms.len(),
            self.use_histogram_matching,
            self.winsorize,
            log.f(self.lower_quantile.to_f64()),
            log.f(self.upper_quantile.to_f64()),
        );
        for i in 0..self.transforms.len() {
            s.push_str(&format!("Stage {} State\n", i + 1));
            let m = &self.metrics[i];
            let t = &self.transforms[i];
            let name = m.kind.map_or("Illegal", |k| k.as_str());
            if !m.kind.is_some_and(|k| k.is_point_set()) {
                let image = |i: &Option<Arc<Image<R, D>>>| match i {
                    Some(i) => itk_image_print(i),
                    None => "(none)\n".into(),
                };
                s.push_str(&format!(
                    "   Image metric = {name}\n     Fixed image = {}     Moving image = {}     Weighting = {}\n     Sampling strategy = {}\n     Number of bins = {}\n     Radius = {}\n     Sampling percentage  = {}\n",
                    image(&m.fixed),
                    image(&m.moving),
                    log.f(m.weighting.to_f64()),
                    match m.sampling {
                        Sampling::Random => "random",
                        Sampling::Regular => "regular",
                        Sampling::None => "none",
                    },
                    m.number_of_bins,
                    m.radius,
                    log.f(m.sampling_percentage.to_f64()),
                ));
            }
            s.push_str(&format!(
                "   Transform = {}\n     Gradient step = {}\n     Update field sigma (voxel space) = {}\n     Total field sigma (voxel space) = {}\n     Update field time sigma = {}\n     Total field time sigma  = {}\n     Number of time indices = {}\n     Number of time point samples = {}\n",
                t.method.as_str(),
                log.f(t.gradient_step.to_f64()),
                log.f(t.update_field_variance.to_f64()),
                log.f(t.total_field_variance.to_f64()),
                log.f(t.update_field_time_sigma.to_f64()),
                log.f(t.total_field_time_sigma.to_f64()),
                t.number_of_time_indices,
                t.number_of_time_indices,
            ));
        }
        log.logger(&s);
    }

    /// `ValidateParameters()`.
    fn validate(&self, log: &mut Log<'_>) -> Result<(), RegistrationFailed> {
        let n = self.transforms.len();
        let fail = |log: &mut Log<'_>, msg: &str| {
            log.logger(&format!("{msg}\n"));
            Err(RegistrationFailed(msg.into()))
        };
        if n == 0 {
            return fail(log, "No transformations are specified.");
        }
        if self.iterations.len() != n {
            return fail(
                log,
                "The number of iteration sets specified does not match the number of stages.",
            );
        }
        if self.shrink_factors.len() != n {
            return fail(
                log,
                "The number of shrinkFactors specified does not match the number of stages.",
            );
        }
        if self.smoothing_sigmas.len() != n {
            return fail(
                log,
                "The number of smoothing sigma sets specified does not match the number of stages.",
            );
        }
        if self.sigmas_in_physical_units.len() != n {
            return fail(
                log,
                "The number of smoothing sigma in physical units bool values does not match the number of stages.",
            );
        }
        for m in &self.metrics {
            if !m.kind.is_some_and(|k| k.is_point_set())
                && (m.fixed.is_none() || m.moving.is_none())
            {
                return fail(log, "The image metric has no fixed and/or moving image.");
            }
        }
        if self.fixed_masks.len() > 1 && self.fixed_masks.len() != n {
            return fail(
                log,
                "The number of fixed masks must be equal to 1 (use the mask for all stages) or the number of fixed masks must be equal to the number of stages.",
            );
        }
        if self.moving_masks.len() > 1 && self.moving_masks.len() != n {
            return fail(
                log,
                "The number of moving masks must be equal to 1 (i.e., use the mask for all stages) or the number of moving masks must be equal to the number of stages.",
            );
        }
        Ok(())
    }

    /// `DoRegistration()`.
    pub fn do_registration(
        &mut self,
        log: &mut Log<'_>,
        floor: &IndexFloor,
    ) -> Result<(), RegistrationFailed> {
        let total = Instant::now();
        self.validate(log)?;
        self.print_state(log);
        let n_stages = self.transforms.len();
        log.logger(&format!("Registration using {n_stages} total stages.\n"));
        if self.composite.is_none() {
            self.composite = Some(Vec::new());
        }
        if self.fixed_initial.is_none() {
            self.fixed_initial = Some(Vec::new());
        }
        for stage in 0..n_stages {
            let timer = Instant::now();
            log.logger(&format!("\nStage {stage}\n"));
            let iterations = self.iterations[stage].clone();
            let its: Vec<String> = iterations.iter().map(|i| i.to_string()).collect();
            log.logger(&format!("  iterations = {}\n", its.join("x")));
            let threshold = self.convergence_thresholds[stage];
            log.logger(&format!(
                "  convergence threshold = {}\n",
                log.f(threshold.to_f64())
            ));
            let window = self.convergence_window_sizes[stage];
            log.logger(&format!("  convergence window size = {window}\n"));
            let levels = iterations.len();
            log.logger(&format!("  number of levels = {levels}\n"));
            let fixed_mask = match self.fixed_masks.len() {
                0 => None,
                1 => self.fixed_masks[0].clone(),
                _ => self.fixed_masks[stage].clone(),
            };
            let moving_mask = match self.moving_masks.len() {
                0 => None,
                1 => self.moving_masks[0].clone(),
                _ => self.moving_masks[stage].clone(),
            };
            let stage_metrics: Vec<Metric<R, D>> = self
                .metrics
                .iter()
                .filter(|m| m.stage == n_stages - stage - 1)
                .cloned()
                .collect();
            let mut prepared: Vec<StageMetric<R, D>> = Vec::new();
            for m in &stage_metrics {
                let weight = log.f(m.weighting.to_f64());
                let gradient = u8::from(m.use_gradient_filter);
                let kind = match m.kind {
                    Some(MetricKind::Cc) => {
                        log.logger(&format!(
                            "  using the CC metric (radius = {}, weight = {weight}, use gradient filter = {gradient})\n",
                            m.radius
                        ));
                        MethodMetric::Cc {
                            radius: m.radius as usize,
                        }
                    }
                    Some(MetricKind::Mattes) => {
                        log.logger(&format!(
                            "  using the Mattes MI metric (number of bins = {}, weight = {weight}, use gradient filter = {gradient})\n",
                            m.number_of_bins
                        ));
                        MethodMetric::Mattes {
                            bins: m.number_of_bins.max(0) as usize,
                        }
                    }
                    Some(MetricKind::Mi) => {
                        log.logger(&format!(
                            "  using the joint histogram MI metric (number of bins = {}, weight = {weight}, use gradient filter = {gradient})\n",
                            m.number_of_bins
                        ));
                        let correct_marginals = self.upstream_defects.fix();
                        push_once(
                            &mut self.notices,
                            Notice::new(
                                &defects::ITK_18,
                                self.upstream_defects,
                                if correct_marginals {
                                    "each image's marginal density is its own; ANTs swaps them"
                                } else {
                                    "the marginal densities are swapped, as in ANTs"
                                },
                            ),
                        );
                        MethodMetric::Mi {
                            bins: m.number_of_bins.max(0) as usize,
                            correct_marginals,
                        }
                    }
                    Some(MetricKind::MeanSquares) => {
                        log.logger(&format!(
                            "  using the MeanSquares metric ( weight = {weight}, use gradient filter = {gradient})\n"
                        ));
                        MethodMetric::MeanSquares
                    }
                    Some(MetricKind::Demons) => {
                        log.logger(&format!(
                            "  using the Demons metric ( weight = {weight}, use gradient filter = {gradient})\n"
                        ));
                        MethodMetric::Demons
                    }
                    Some(MetricKind::Gc) => {
                        log.logger(&format!(
                            "  using the global correlation metric ( weight = {weight}, use gradient filter = {gradient})\n"
                        ));
                        MethodMetric::Gc
                    }
                    Some(k) => {
                        log.logger(&format!(
                            "  using the {} metric (weight = {weight})\n",
                            k.as_str()
                        ));
                        return self.unsupported(log, "the point-set metrics");
                    }
                    None => {
                        log.logger("ERROR: Unrecognized metric. \n");
                        return Err(RegistrationFailed("ERROR: Unrecognized metric. ".into()));
                    }
                };
                let (Some(fixed), Some(moving)) = (&m.fixed, &m.moving) else {
                    unreachable!("validated");
                };
                let mut preprocessing = String::new();
                if self.winsorize {
                    preprocessing.push_str("  preprocessing:  winsorizing the image intensities\n");
                }
                let lq = self.lower_quantile.to_f64() as f32;
                let uq = self.upper_quantile.to_f64() as f32;
                let pf = larmorx_registration::preprocess::preprocess(&fixed.data, lq, uq, None);
                let mut pm =
                    larmorx_registration::preprocess::preprocess(&moving.data, lq, uq, None);
                if self.use_histogram_matching {
                    preprocessing.push_str("  preprocessing:  histogram matching the images\n");
                    pm = larmorx_registration::preprocess::preprocess(
                        &moving.data,
                        lq,
                        uq,
                        Some(&pf),
                    );
                }
                log.logger(&preprocessing);
                prepared.push(StageMetric {
                    kind,
                    fixed: Arc::new(fixed.with_data(pf)),
                    moving: Arc::new(moving.with_data(pm)),
                    fixed_mask: fixed_mask.clone(),
                    moving_mask: moving_mask.clone(),
                    use_gradient_filter: m.use_gradient_filter,
                    weight: m.weighting,
                });
            }
            let factors = &self.shrink_factors[stage];
            if factors.len() != levels {
                let msg = format!(
                    "\n\n\nERROR:  The number of shrink factors does not match the number of levels.\nShrink Factors: {}\nNumber Of Levels: {levels}\n\n\n\n",
                    factors.len()
                );
                log.logger(&msg);
                return Err(RegistrationFailed(
                    "The number of shrink factors does not match the number of levels.".into(),
                ));
            }
            let virtual_spacing = prepared[0].fixed.space.spacing;
            let mut per_level = Vec::new();
            for (n, &f) in factors.iter().enumerate() {
                let s = shrink_factors_per_dimension(f, &virtual_spacing);
                let shown: Vec<String> = s.iter().map(|v| v.to_string()).collect();
                log.logger(&format!(
                    "  Shrink factors (level {} out of {levels}): [{}]\n",
                    n + 1,
                    shown.join(", ")
                ));
                per_level.push(s);
            }
            let sigmas = &self.smoothing_sigmas[stage];
            if sigmas.len() != levels {
                log.logger(
                    "ERROR:  The number of smoothing sigmas does not match the number of levels.\n",
                );
                return Err(RegistrationFailed(
                    "The number of smoothing sigmas does not match the number of levels.".into(),
                ));
            }
            let sigmas_r: Vec<R> = sigmas.iter().map(|&s| R::r(f64::from(s))).collect();
            // `itk::Array<R>`'s `operator<<` prints with `ConvertNumberToString` (the
            // shortest round trip), whatever the stream's format.
            let shown: Vec<String> = sigmas_r
                .iter()
                .map(|s| match R::PRECISION {
                    larmorx_core::Precision::Double => {
                        larmorx_io::itk_transform::itk_number(s.to_f64())
                    }
                    larmorx_core::Precision::Float => {
                        larmorx_io::itk_transform::itk_number_f32(s.to_f64() as f32)
                    }
                })
                .collect();
            log.logger(&format!(
                "  smoothing sigmas per level: [{}]\n",
                shown.join(", ")
            ));
            let m0 = &stage_metrics[0];
            let percentage = m0.sampling_percentage.to_f64() as f32;
            let sampling = match m0.sampling {
                Sampling::Random => {
                    log.logger(&format!(
                        "  random sampling (percentage = {})\n",
                        log.f(f64::from(percentage))
                    ));
                    SamplingStrategy::Random
                }
                Sampling::Regular => {
                    log.logger(&format!(
                        "  regular sampling (percentage = {})\n",
                        log.f(f64::from(percentage))
                    ));
                    SamplingStrategy::Regular
                }
                Sampling::None => {
                    log.logger("  Using default NONE metricSamplingStrategy \n");
                    SamplingStrategy::None
                }
            };
            let learning_rate = self.transforms[stage].gradient_step;
            let method = self.transforms[stage].method;
            let mut output = match self.new_transform(method) {
                Some(t) => t,
                None => return self.unsupported(log, &format!("{} stages", method.as_str())),
            };
            let class = output.class();
            let n_parameters = output.number_of_parameters();
            let percentage_r = R::r(f64::from(percentage));
            if let Err(e) = method::check_sampling_percentage_levels(percentage_r.to_f64(), levels)
            {
                return Err(self.exception_in_main(log, &e));
            }
            let weights = match self.restrict_deformation.get(stage) {
                Some(w) if w.len() == n_parameters => w.clone(),
                _ => Vec::new(),
            };
            let mut optimizer = ConjugateGradientLineSearch {
                learning_rate,
                maximum_step_size: learning_rate,
                number_of_iterations: iterations.first().copied().unwrap_or(0) as usize,
                minimum_convergence_value: threshold,
                convergence_window_size: window as usize,
                lower_limit: R::ZERO,
                upper_limit: R::r(2.0),
                epsilon: R::r(0.2),
                maximum_line_search_iterations: 20,
                weights,
                estimate_learning_rate_at_each_iteration: true,
                estimate_learning_rate_once: false,
            };
            // PrepareRegistrationMethod: with --initialize-transforms-per-stage, the stage starts
            // from the last transform of the composite when it can, and that transform leaves
            // the composite.
            if self.initialize_transforms_per_stage {
                let queue = self.composite.as_mut().expect("set above");
                if !queue.is_empty() {
                    log.logger(&format!(
                        "Current number of transforms in the composite transform: {}\n",
                        queue.len()
                    ));
                    for (i, t) in queue.iter().enumerate() {
                        log.logger(&format!("{}) {}\n", i + 1, t.class()));
                    }
                    let name = class.replacen("Transform", "", 1);
                    if initialize_with_previous(log, queue.last(), &name, &mut output) {
                        log.logger("Registration process is run using direct initialization!\n");
                        queue.pop();
                    }
                }
            }
            let composite = self.composite.as_ref().expect("set above").clone();
            let fixed_initial = self.fixed_initial.as_ref().expect("set above").clone();
            let seed = if self.random_seed != 0 {
                self.random_seed
            } else {
                self.default_seed
            };
            let stage_spec = Stage {
                metrics: prepared,
                shrink_factors: per_level,
                smoothing_sigmas: sigmas_r,
                sigmas_in_physical_units: self.sigmas_in_physical_units[stage],
                sampling,
                sampling_percentage: percentage_r,
                seed,
                fixed_initial: (!fixed_initial.is_empty()).then(|| Composite::new(fixed_initial)),
                moving_initial: composite.clone(),
                initialize_center: true,
                n_threads: self.n_threads,
            };
            log.logger(&format!("\n*** Running {class} registration ***\n\n"));
            let mut observer = IterationObserver::<R> {
                log,
                iterations: iterations.clone(),
                current_level: 0,
                clock: Instant::now(),
                last: 0.0,
                _r: std::marker::PhantomData,
            };
            let mut warnings = Vec::new();
            let result = method::run_stage(
                &stage_spec,
                output,
                &mut optimizer,
                &mut observer,
                floor,
                &mut warnings,
            );
            for w in &warnings {
                log.cerr(w);
            }
            self.warnings.extend(warnings);
            match result {
                Ok((t, _)) => {
                    self.composite.as_mut().expect("set above").push(t);
                }
                Err(e) => {
                    // AddLinearTransformToCompositeTransform logs the exception and returns
                    // EXIT_FAILURE, which DoRegistration ignores (ANTS-49).
                    log.logger(&format!("Exception caught: {}\n", e.print()));
                    if self.upstream_defects.fix() {
                        push_once(
                            &mut self.notices,
                            Notice::new(
                                &defects::ANTS_49,
                                self.upstream_defects,
                                format!(
                                    "stage {stage} failed: an error; ANTs drops the stage and goes on"
                                ),
                            ),
                        );
                        let msg = format!(
                            "larmorx: antsRegistration: stage {stage} failed (the exception above); \
                             {}=reproduce drops the stage and goes on, as ANTs does (ANTS-49)\n",
                            defects::ENV
                        );
                        log.cerr(&msg);
                        return Err(RegistrationFailed(e.description));
                    }
                    push_once(
                        &mut self.notices,
                        Notice::new(
                            &defects::ANTS_49,
                            self.upstream_defects,
                            format!("stage {stage} failed and was dropped, as in ANTs"),
                        ),
                    );
                }
            }
            let elapsed = timer.elapsed().as_secs_f64();
            log.logger(&format!(
                "  Elapsed time (stage {stage}): {}\n\n",
                log.f(elapsed)
            ));
        }
        let elapsed = total.elapsed().as_secs_f64();
        log.logger(&format!("\nTotal elapsed time: {}\n", log.f(elapsed)));
        Ok(())
    }

    /// The transform a linear stage optimises, as its traits choose it.
    fn new_transform(&self, method: XfrmMethod) -> Option<Xfm<R, D>> {
        let linear = |kind: Kind| Some(Xfm::Linear(LinearXfm::new(kind)));
        match (method, D) {
            (XfrmMethod::Translation, _) => Some(Xfm::Translation {
                offset: [R::ZERO; D],
            }),
            (XfrmMethod::Rigid, 3) => linear(Kind::Euler3D { zyx: false }),
            (XfrmMethod::Rigid, 2) => linear(Kind::Euler2D),
            (XfrmMethod::Similarity, 2) => linear(Kind::Similarity2D),
            (XfrmMethod::Similarity, 3) => linear(Kind::Similarity3D),
            (XfrmMethod::CompositeAffine, 2) => linear(Kind::AntsCenteredAffine2D),
            (XfrmMethod::CompositeAffine, 3) => linear(Kind::AntsAffine3D),
            (XfrmMethod::Affine, _) => linear(Kind::Affine),
            (XfrmMethod::Rigid | XfrmMethod::Similarity | XfrmMethod::CompositeAffine, 4) => {
                linear(Kind::Affine)
            }
            _ => None,
        }
    }

    /// A part of `antsRegistration` larmorx does not port yet.
    fn unsupported<T>(&self, log: &mut Log<'_>, what: &str) -> Result<T, RegistrationFailed> {
        let msg = format!("larmorx: antsRegistration: {what} are not supported yet\n");
        log.cerr(&msg);
        Err(RegistrationFailed(msg))
    }

    /// An ITK exception that `main` catches: "Exception Object caught: " and its print.
    fn exception_in_main(&self, log: &mut Log<'_>, e: &ItkException) -> RegistrationFailed {
        log.cerr(&format!("Exception Object caught: \n{}\n", e.print()));
        RegistrationFailed(e.description.clone())
    }
}

/// D13 and other state a run reports.
pub fn notices_for(_notices: &mut Notices) {}

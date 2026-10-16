// SPDX-License-Identifier: Apache-2.0
//! `ImageRegistrationMethodv4` for one linear stage (ITK v5.4.5,
//! `Modules/Registration/RegistrationMethodsv4/include/itkImageRegistrationMethodv4.hxx`).
//!
//! Each level shrinks the virtual domain (`ShrinkImageFilter`'s output geometry), smooths
//! the fixed and moving images (`SmoothingRecursiveGaussianImageFilter`, sigmas in voxels
//! scaled by each image's spacing unless given in millimetres), samples the virtual domain
//! when asked (`SetMetricSamplePoints`), initialises the metric and runs the optimizer.
//! The moving transform is a composite of the initial transforms and the transform being
//! optimised, whose centre starts at the last matrix-offset initial transform's
//! (`InitializeCenterOfLinearOutputTransform`).

use std::sync::Arc;

use larmorx_core::IndexFloor;
use larmorx_image::ItkGrid;
use larmorx_image::shrink::shrink_plan;

use crate::mask::Mask;
use crate::metric::joint_histogram::JointHistogramMi;
use crate::metric::mattes::Mattes;
use crate::metric::neighborhood_correlation::NeighborhoodCorrelation;
use crate::metric::{ImageMetricv4, correlation, mean_squares, no_valid_points_warning};
use crate::optimizer::{ConjugateGradientLineSearch, Observer, Outcome, Problem, scales};
use crate::rng::{MersenneTwister, RandomIterator};
use crate::transform::{Composite, Xfm};
use crate::{Image, ItkException, Real, Space};

const FILE: &str =
    "Modules/Registration/RegistrationMethodsv4/include/itkImageRegistrationMethodv4.hxx";
const CLASS: &str = "ImageRegistrationMethodv4";

/// The metric sampling strategy (`MetricSamplingStrategyEnum`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplingStrategy {
    None,
    Regular,
    Random,
}

/// The image metric of a stage and its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetricKind {
    /// Mattes mutual information with this many histogram bins.
    Mattes {
        bins: usize,
    },
    MeanSquares,
    /// Joint-histogram mutual information (ANTs' `MI`) with this many bins; ITK's swapped
    /// marginals corrected or not (ITK-18).
    Mi {
        bins: usize,
        correct_marginals: bool,
    },
    /// Global correlation (`CorrelationImageToImageMetricv4`, ANTs' `GC`).
    Gc,
    /// Neighbourhood cross-correlation of this radius (ANTs' `CC`).
    Cc {
        radius: usize,
    },
    /// `DemonsImageToImageMetricv4`, which needs a displacement field: a linear stage throws
    /// when it initialises the metric.
    Demons,
}

/// One image metric of a stage: its images (preprocessed), masks and settings.
#[derive(Clone, Debug)]
pub struct StageMetric<R, const D: usize> {
    pub kind: MetricKind,
    pub fixed: Arc<Image<R, D>>,
    pub moving: Arc<Image<R, D>>,
    pub fixed_mask: Option<Mask<D>>,
    pub moving_mask: Option<Mask<D>>,
    pub use_gradient_filter: bool,
    /// The metric's weight in a stage of several metrics.
    pub weight: R,
}

/// A stage as `ImageRegistrationMethodv4` runs it.
#[derive(Clone, Debug)]
pub struct Stage<R, const D: usize> {
    /// The stage's image metrics (`ObjectToObjectMultiMetricv4` when there are several).
    pub metrics: Vec<StageMetric<R, D>>,
    /// Shrink factors per level and axis.
    pub shrink_factors: Vec<[usize; D]>,
    /// Smoothing sigma per level (`SmoothingSigmasPerLevel`, as `R`).
    pub smoothing_sigmas: Vec<R>,
    pub sigmas_in_physical_units: bool,
    pub sampling: SamplingStrategy,
    pub sampling_percentage: R,
    /// `MetricSamplingReinitializeSeed(seed)`: the first seed of the stage's generators.
    pub seed: i32,
    /// The fixed initial transform (none: identity).
    pub fixed_initial: Option<Composite<R, D>>,
    /// The moving initial transforms (flattened, identity transforms dropped by the caller).
    pub moving_initial: Vec<Xfm<R, D>>,
    /// Whether the output transform's centre starts at the initial transforms' (on in
    /// ANTs).
    pub initialize_center: bool,
    pub n_threads: usize,
}

/// A metric, initialised for a level.
enum LevelMetric<R> {
    Mattes(Mattes<R>),
    MeanSquares,
    Mi(Box<JointHistogramMi<R>>),
    Gc,
    Cc(NeighborhoodCorrelation),
}

const VERIFY_FILE: &str = "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx";

impl<R: Real> LevelMetric<R> {
    /// `Initialize()` of the level's metric (Demons throws for a linear transform).
    fn initialize<const D: usize>(
        kind: MetricKind,
        metric: &ImageMetricv4<R, D>,
    ) -> Result<Self, ItkException> {
        Ok(match kind {
            MetricKind::Mattes { bins } => LevelMetric::Mattes(Mattes::initialize(bins, metric)?),
            MetricKind::MeanSquares => LevelMetric::MeanSquares,
            MetricKind::Mi {
                bins,
                correct_marginals,
            } => LevelMetric::Mi(Box::new(JointHistogramMi::initialize(
                bins,
                metric,
                correct_marginals,
            )?)),
            MetricKind::Gc => LevelMetric::Gc,
            MetricKind::Cc { radius } => LevelMetric::Cc(NeighborhoodCorrelation { radius }),
            MetricKind::Demons => {
                return Err(ItkException::new(
                    "Modules/Registration/Metricsv4/include/itkDemonsImageToImageMetricv4.hxx",
                    68,
                    "DemonsImageToImageMetricv4",
                    "The moving transform must be a displacement field transform",
                ));
            }
        })
    }

    /// `GetValue` or `GetValueAndDerivative`.
    fn evaluate<const D: usize>(
        &self,
        metric: &ImageMetricv4<R, D>,
        moving: &Composite<R, D>,
        derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        let reduced = |r: mean_squares::Reduced<R>, class: &str, warnings: &mut Vec<String>| {
            if r.no_valid_points {
                warnings.push(no_valid_points_warning(VERIFY_FILE, 544, class));
            }
            (r.value, r.derivative)
        };
        match self {
            LevelMetric::Mattes(m) => m.evaluate(metric, moving, derivative, floor),
            LevelMetric::MeanSquares => {
                let r = mean_squares::evaluate(metric, moving, derivative, floor)?;
                Ok(reduced(r, "MeanSquaresImageToImageMetricv4", warnings))
            }
            LevelMetric::Mi(m) => m.evaluate(metric, moving, derivative, floor, warnings),
            LevelMetric::Gc => correlation::evaluate(metric, moving, derivative, floor, warnings),
            LevelMetric::Cc(m) => {
                let r = m.evaluate(metric, moving, derivative, floor)?;
                Ok(reduced(
                    r,
                    "ANTSNeighborhoodCorrelationImageToImageMetricv4",
                    warnings,
                ))
            }
        }
    }
}

/// `vnl_vector<R>::magnitude()`: the squares summed in `R`, the root in double.
fn magnitude<R: Real>(v: &[R]) -> R {
    let mut sum = R::ZERO;
    for &x in v {
        sum = sum + x * x;
    }
    R::from_f64(sum.to_f64().sqrt())
}

/// The optimizer's view of a level: the metrics and the moving composite.
pub struct LevelProblem<'a, R, const D: usize> {
    /// Each metric with its level state; the first is the one the scales estimator uses.
    metrics: Vec<(ImageMetricv4<R, D>, LevelMetric<R>)>,
    /// The weights of a stage of several metrics.
    weights: Vec<R>,
    pub moving: Composite<R, D>,
    corners: Vec<[f64; D]>,
    floor: &'a IndexFloor,
    /// Warnings ITK's output window would print (`itkWarningMacro`).
    pub warnings: Vec<String>,
}

impl<R: Real, const D: usize> LevelProblem<'_, R, D> {
    /// The first metric's inputs (the virtual domain is common to all).
    pub fn metric(&self) -> &ImageMetricv4<R, D> {
        &self.metrics[0].0
    }

    /// `GetValue`/`GetValueAndDerivative`; with several metrics
    /// `ObjectToObjectMultiMetricv4`'s: the first metric's value, and the derivative
    /// `Σ w_j·d_j/|d_j|` times the mean of the magnitudes `|d_j|`.
    fn evaluate(&mut self, derivative: bool) -> Result<(R, Vec<R>), ItkException> {
        if self.metrics.len() == 1 {
            let (m, state) = &self.metrics[0];
            return state.evaluate(m, &self.moving, derivative, self.floor, &mut self.warnings);
        }
        let np = self.moving.number_of_parameters();
        let mut first = R::ZERO;
        let mut result = vec![R::ZERO; if derivative { np } else { 0 }];
        let mut total = R::ZERO;
        for (j, (m, state)) in self.metrics.iter().enumerate() {
            let (v, d) =
                state.evaluate(m, &self.moving, derivative, self.floor, &mut self.warnings)?;
            if j == 0 {
                first = v;
            }
            if derivative {
                let mag = magnitude(&d);
                total = total + mag;
                let weight_over = if mag > R::EPSILON {
                    self.weights[j] / mag
                } else {
                    R::ZERO
                };
                for (r, &x) in result.iter_mut().zip(&d) {
                    *r = *r + x * weight_over;
                }
            }
        }
        if derivative {
            let mean = total / R::from_i64(self.metrics.len() as i64);
            for r in &mut result {
                *r = *r * mean;
            }
        }
        Ok((first, result))
    }
}

impl<R: Real, const D: usize> Problem<R> for LevelProblem<'_, R, D> {
    fn value_and_derivative(&mut self) -> Result<(R, Vec<R>), ItkException> {
        self.evaluate(true)
    }

    fn value(&mut self) -> Result<R, ItkException> {
        Ok(self.evaluate(false)?.0)
    }

    fn parameters(&self) -> Vec<R> {
        self.moving.active().parameters()
    }

    fn set_parameters(&mut self, p: &[R]) -> Result<(), ItkException> {
        self.moving.active_mut().set_parameters(p)
    }

    fn update(&mut self, update: &[R]) -> Result<(), ItkException> {
        self.moving.active_mut().update(update, R::ONE)
    }

    fn estimate_scales(&mut self) -> Result<Vec<R>, ItkException> {
        scales::estimate_scales(&mut self.moving, &self.corners)
    }

    fn estimate_step_scale(&mut self, step: &[R]) -> Result<R, ItkException> {
        scales::estimate_step_scale(&mut self.moving, &self.corners, step)
    }

    fn estimate_maximum_step(&self) -> R {
        scales::maximum_step(&self.metrics[0].0.virtual_space)
    }
}

/// The shrunk virtual domain of a level (`ShrinkImageFilter::GenerateOutputInformation`).
pub fn shrink_space<const D: usize>(
    space: &Space<D>,
    factors: &[usize; D],
) -> Result<Space<D>, ItkException> {
    let grid = ItkGrid {
        size: space.size.to_vec(),
        start: space.start.to_vec(),
        spacing: space.spacing.to_vec(),
        origin: space.origin.to_vec(),
        direction: space.direction.iter().map(|r| r.to_vec()).collect(),
    };
    let plan = shrink_plan(&grid, factors)
        .map_err(|e| ItkException::new(FILE, 470, "ShrinkImageFilter", e.to_string()))?;
    let o = plan.output;
    Space::new(
        std::array::from_fn(|k| o.size[k]),
        std::array::from_fn(|k| o.start[k]),
        std::array::from_fn(|k| o.spacing[k]),
        std::array::from_fn(|k| o.origin[k]),
        space.direction,
    )
}

/// `SetMetricSamplePoints` for one metric: the regularly or randomly chosen voxels of the
/// virtual domain, each moved by a normal variate times a third of the spacing, kept if
/// inside the fixed mask. `seed` is the generators' next seed (`m_CurrentRandomSeed`).
pub fn sample_points<R: Real, const D: usize>(
    space: &Space<D>,
    strategy: SamplingStrategy,
    percentage: R,
    fixed_mask: Option<&Mask<D>>,
    seed: &mut i32,
) -> Vec<[f64; D]> {
    let third: [f64; D] = std::array::from_fn(|k| space.spacing[k] / 3.0);
    let mut randomizer = MersenneTwister::from_int(*seed);
    *seed = seed.wrapping_add(1);
    let mut points = Vec::new();
    // The sample points are `Point<float>` (`PointSet`'s default coordinate type): the
    // index's physical point is accumulated in float (`TransformIndexToPhysicalPoint<float>`,
    // each `+=` computed in double and stored), the perturbation added the same way.
    let mut take = |idx: &[i64; D], randomizer: &mut MersenneTwister| {
        let mut p: [f64; D] = std::array::from_fn(|i| {
            let mut v = space.origin[i] as f32;
            for j in 0..D {
                v = (f64::from(v) + space.index_to_point[i][j] * idx[j] as f64) as f32;
            }
            f64::from(v)
        });
        for d in 0..D {
            p[d] = ((p[d] + randomizer.normal() * third[d]) as f32) as f64;
        }
        if fixed_mask.is_none_or(|m| m.is_inside(&p)) {
            points.push(p);
        }
    };
    match strategy {
        SamplingStrategy::Regular => {
            let step = (1.0 / percentage.to_f64()).ceil() as u64;
            let mut count = step;
            for o in 0..space.len() {
                if count == step {
                    count = 0;
                    take(&space.index_of(o), &mut randomizer);
                }
                count += 1;
            }
        }
        SamplingStrategy::Random => {
            let total = space.len() as u64;
            // static_cast<unsigned long>(static_cast<float>(total) * percentage), the product
            // in R.
            let n = (R::from_f64(f64::from(total as f32)) * percentage).to_f64() as u64;
            let mut it = RandomIterator::new(space.size, space.start, *seed);
            *seed = seed.wrapping_add(1);
            for idx in it.samples(n as usize) {
                take(&idx, &mut randomizer);
            }
        }
        SamplingStrategy::None => {}
    }
    points
}

/// `ComputeMovingImageGradientFilterImage`: `GradientRecursiveGaussianImageFilter` with the
/// largest spacing as sigma, normalised across scale, each vector then turned by the image's
/// direction (`TransformLocalVectorToPhysicalVector`, each row summed from 0 in double).
fn gradient_image<R: Real, const D: usize>(
    image: &Image<R, D>,
    n_threads: usize,
) -> Result<Image<[f64; D], D>, ItkException> {
    let space = &image.space;
    let mut sigma = 0.0f64;
    for &s in &space.spacing {
        if s > sigma {
            sigma = s;
        }
    }
    let components = R::gradient(&image.data, &space.size, &space.spacing, sigma, n_threads)
        .map_err(|e| {
            ItkException::new(
                "Modules/Filtering/ImageFilterBase/include/itkRecursiveSeparableImageFilter.hxx",
                223,
                "RecursiveGaussianImageFilter",
                e.to_string(),
            )
        })?;
    let data = (0..space.len())
        .map(|o| {
            let local: [f64; D] = std::array::from_fn(|k| components[k][o]);
            space.local_to_physical_vector(&local)
        })
        .collect();
    Ok(Image::new(space.clone(), data))
}

/// Runs a stage: returns the optimised transform and each level's outcome.
pub fn run_stage<R: Real, const D: usize, O>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut ConjugateGradientLineSearch<R>,
    observer: &mut O,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<(Xfm<R, D>, Vec<Outcome<R>>), ItkException>
where
    O: for<'a> Observer<R, LevelProblem<'a, R, D>>,
{
    let mut output = output;
    // InitializeCenterOfLinearOutputTransform: the centre of the last matrix-offset
    // transform among the initial transforms.
    if stage.initialize_center
        && let Xfm::Linear(out) = &mut output
        && let Some(c) = stage.moving_initial.iter().rev().find_map(|t| match t {
            Xfm::Linear(l) => Some(l.center),
            _ => None,
        })
    {
        out.set_center(c);
    }
    let virtual_base = stage.metrics[0].fixed.space.clone();
    let multi = stage.metrics.len() > 1;
    // ObjectToObjectMultiMetricv4's weights, normalised to sum 1 at each Initialize.
    let mut weights: Vec<R> = stage.metrics.iter().map(|m| m.weight).collect();
    let mut current_seed = stage.seed;
    let mut outcomes = Vec::new();
    for (level, factors) in stage.shrink_factors.iter().enumerate() {
        let virtual_space = shrink_space(&virtual_base, factors)?;
        let sigma = stage.smoothing_sigmas[level];
        let smooth = |image: &Arc<Image<R, D>>| -> Result<Arc<Image<R, D>>, ItkException> {
            if sigma <= R::ZERO {
                return Ok(Arc::clone(image));
            }
            let sigmas: Vec<f64> = (0..D)
                .map(|i| {
                    let s = sigma.to_f64();
                    if stage.sigmas_in_physical_units {
                        s
                    } else {
                        s * image.space.spacing[i]
                    }
                })
                .collect();
            let data = R::smooth(
                &image.data,
                &image.space.size,
                &image.space.spacing,
                &sigmas,
                stage.n_threads,
            )
            .map_err(|e| {
                ItkException::new(
                    "Modules/Filtering/Smoothing/include/itkSmoothingRecursiveGaussianImageFilter.hxx",
                    214,
                    "SmoothingRecursiveGaussianImageFilter",
                    e.to_string(),
                )
            })?;
            Ok(Arc::new(image.with_data(data)))
        };
        let mut metrics = Vec::with_capacity(stage.metrics.len());
        for sm in &stage.metrics {
            metrics.push(ImageMetricv4 {
                fixed: smooth(&sm.fixed)?,
                moving: smooth(&sm.moving)?,
                fixed_mask: sm.fixed_mask.clone(),
                moving_mask: sm.moving_mask.clone(),
                fixed_transform: stage.fixed_initial.clone(),
                virtual_space: virtual_space.clone(),
                samples: None,
                fixed_gradient_image: None,
                moving_gradient_image: None,
            });
        }
        // SetMetricSamplePoints: one set per metric, each from the first metric's virtual
        // domain and fixed mask, with the next seeds.
        if stage.sampling != SamplingStrategy::None {
            let first_mask = stage.metrics[0].fixed_mask.clone();
            for m in &mut metrics {
                m.samples = Some(Arc::new(sample_points(
                    &virtual_space,
                    stage.sampling,
                    stage.sampling_percentage,
                    first_mask.as_ref(),
                    &mut current_seed,
                )));
            }
        }
        // m_Metric->Initialize().
        if multi {
            let mut sum = R::ZERO;
            for &w in &weights {
                sum = sum + w;
            }
            if sum <= R::EPSILON {
                return Err(ItkException::new(
                    "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
                    143,
                    "ObjectToObjectMultiMetricv4",
                    format!(
                        "The derivative weights are too small: {}",
                        crate::format::itk_array(&weights)
                    ),
                ));
            }
            for w in &mut weights {
                *w = *w / sum;
            }
        }
        let mut states = Vec::with_capacity(metrics.len());
        for (m, sm) in metrics.iter_mut().zip(&stage.metrics) {
            let initialized = (|| {
                if let Some(s) = &m.samples
                    && s.is_empty()
                {
                    return Err(ItkException::new(
                        "Modules/Registration/Metricsv4/include/itkImageToImageMetricv4.hxx",
                        268,
                        "ImageToImageMetricv4",
                        "VirtualSampledPointSet must have 1 or more points.",
                    ));
                }
                // Demons checks its transform before ImageToImageMetricv4::Initialize.
                if sm.kind == MetricKind::Demons {
                    return LevelMetric::initialize(sm.kind, m);
                }
                if sm.use_gradient_filter {
                    m.moving_gradient_image =
                        Some(Arc::new(gradient_image(&m.moving, stage.n_threads)?));
                }
                LevelMetric::initialize(sm.kind, m)
            })();
            match initialized {
                Ok(state) => states.push(state),
                Err(e) if multi => {
                    return Err(e.wrapped(
                        "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
                        226,
                        "Caught exception initializing metric: \n",
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        let mut transforms = stage.moving_initial.clone();
        transforms.push(output.clone());
        let corners = scales::corners(&virtual_space);
        let mut problem = LevelProblem {
            metrics: metrics.into_iter().zip(states).collect(),
            weights: weights.clone(),
            moving: Composite::new(transforms),
            corners,
            floor,
            warnings: Vec::new(),
        };
        let outcome = optimizer.start(&mut problem, observer);
        warnings.append(&mut problem.warnings);
        outcomes.push(outcome?);
        output = problem.moving.active().clone();
    }
    Ok((output, outcomes))
}

/// `SetMetricSamplingPercentage(p)` after `SetNumberOfLevels(levels)`: the per-level
/// percentages, all 1 until then, are checked only when they change.
pub fn check_sampling_percentage_levels(p: f64, levels: usize) -> Result<(), ItkException> {
    if levels == 0 || p == 1.0 {
        return Ok(());
    }
    check_sampling_percentage(p)
}

/// `SetMetricSamplingPercentagePerLevel`'s check: a percentage outside `(0, 1]` throws.
pub fn check_sampling_percentage(p: f64) -> Result<(), ItkException> {
    if p <= 0.0 || p > 1.0 {
        return Err(ItkException::new(
            FILE,
            1324,
            CLASS,
            "sampling percentage outside expected (0,1] range",
        ));
    }
    Ok(())
}

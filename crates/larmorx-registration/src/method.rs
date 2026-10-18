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
use crate::pointset::{PointSetEvents, PointSetMetric};
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
pub struct ImageStageMetric<R, const D: usize> {
    pub kind: MetricKind,
    pub fixed: Arc<Image<R, D>>,
    pub moving: Arc<Image<R, D>>,
    pub fixed_mask: Option<Mask<D>>,
    pub moving_mask: Option<Mask<D>>,
    pub use_gradient_filter: bool,
    /// The metric's weight in a stage of several metrics.
    pub weight: R,
}

/// One point-set metric of a stage: the metric (which keeps state from level to level) and
/// the stage's fixed mask, whose grid is the metric's virtual domain (`SetVirtualDomainFromImage`
/// of the mask cast to the virtual image type; the mask's values are not used).
#[derive(Clone, Debug)]
pub struct PointSetStageMetric<R, const D: usize> {
    pub metric: PointSetMetric<R, D>,
    pub fixed_mask: Option<Mask<D>>,
    pub weight: R,
}

/// A metric of a stage.
#[derive(Clone, Debug)]
pub enum StageMetric<R, const D: usize> {
    Image(ImageStageMetric<R, D>),
    PointSet(Box<PointSetStageMetric<R, D>>),
}

impl<R: Real, const D: usize> StageMetric<R, D> {
    /// The metric's weight.
    pub fn weight(&self) -> R {
        match self {
            StageMetric::Image(m) => m.weight,
            StageMetric::PointSet(m) => m.weight,
        }
    }

    /// The metric's own virtual domain, as `antsRegistration` sets it: an image metric's fixed
    /// image; a point-set metric's fixed mask, if the stage has one.
    pub fn virtual_domain(&self) -> Option<&Space<D>> {
        match self {
            StageMetric::Image(m) => Some(&m.fixed.space),
            StageMetric::PointSet(m) => m.fixed_mask.as_ref().map(|k| &k.image.space),
        }
    }
}

/// The description of the error larmorx raises where ANTs crashes (the caller refuses with
/// its message): a point-set metric in a stage that samples its metrics
/// (`SetMetricSamplePoints` casts every metric to an image metric), a stage of point-set
/// metrics without a virtual domain.
pub const CRASH_FILE: &str = "larmorx: ANTs crashes here";

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
pub enum LevelMetric<R> {
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

/// A metric of a level: an image metric with its level state, or a point-set metric.
pub enum LevelEntry<R, const D: usize> {
    Image(ImageMetricv4<R, D>, LevelMetric<R>),
    PointSet(Box<PointSetMetric<R, D>>),
}

/// The optimizer's view of a level: the metrics and the moving composite.
pub struct LevelProblem<'a, R, const D: usize> {
    /// Each metric with its level state; the first is the one the scales estimator uses.
    metrics: Vec<LevelEntry<R, D>>,
    /// The weights of a stage of several metrics.
    weights: Vec<R>,
    pub moving: Composite<R, D>,
    /// The scales estimator's samples: the corners of the virtual domain, or the first
    /// metric's fixed points when it is a point-set metric (`SetVirtualDomainPointSet`).
    samples: Vec<[f64; D]>,
    /// Whether `samples` are the first metric's fixed points (`VirtualDomainPointSetSampling`).
    point_set_samples: bool,
    /// The level's virtual domain.
    virtual_space: Space<D>,
    floor: &'a IndexFloor,
    /// Warnings ITK's output window would print (`itkWarningMacro`).
    pub warnings: Vec<String>,
}

impl<'a, R: Real, const D: usize> LevelProblem<'a, R, D> {
    /// The problem of one image metric outside a registration method (`antsAI`,
    /// `antsAffineInitializer`): `metric` with its sample points and gradient images already
    /// set, initialised here (`Initialize()`), and the moving transform `moving`, whose scales
    /// are estimated at the corners of the metric's virtual domain.
    pub fn single(
        metric: ImageMetricv4<R, D>,
        kind: MetricKind,
        moving: Composite<R, D>,
        floor: &'a IndexFloor,
    ) -> Result<Self, ItkException> {
        if let Some(s) = &metric.samples
            && s.is_empty()
        {
            return Err(ItkException::new(
                "Modules/Registration/Metricsv4/include/itkImageToImageMetricv4.hxx",
                268,
                "ImageToImageMetricv4",
                "VirtualSampledPointSet must have 1 or more points.",
            ));
        }
        let state = LevelMetric::initialize(kind, &metric)?;
        let samples = scales::corners(&metric.virtual_space);
        let virtual_space = metric.virtual_space.clone();
        Ok(LevelProblem {
            metrics: vec![LevelEntry::Image(metric, state)],
            weights: vec![R::ONE],
            moving,
            samples,
            point_set_samples: false,
            virtual_space,
            floor,
            warnings: Vec::new(),
        })
    }

    /// The first metric's inputs, if it is an image metric.
    pub fn metric(&self) -> Option<&ImageMetricv4<R, D>> {
        match &self.metrics[0] {
            LevelEntry::Image(m, _) => Some(m),
            LevelEntry::PointSet(_) => None,
        }
    }

    /// One metric's value (and derivative).
    fn evaluate_one(
        entry: &mut LevelEntry<R, D>,
        moving: &Composite<R, D>,
        derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        match entry {
            LevelEntry::Image(m, state) => state.evaluate(m, moving, derivative, floor, warnings),
            LevelEntry::PointSet(p) => {
                if derivative {
                    let v = p.value_and_derivative(moving, warnings)?;
                    Ok((v.value, v.derivative))
                } else {
                    Ok((p.value(moving, warnings)?, Vec::new()))
                }
            }
        }
    }

    /// `GetValue`/`GetValueAndDerivative`; with several metrics
    /// `ObjectToObjectMultiMetricv4`'s: the first metric's value, and the derivative
    /// `Σ w_j·d_j/|d_j|` times the mean of the magnitudes `|d_j|`.
    fn evaluate(&mut self, derivative: bool) -> Result<(R, Vec<R>), ItkException> {
        if self.metrics.len() == 1 {
            return Self::evaluate_one(
                &mut self.metrics[0],
                &self.moving,
                derivative,
                self.floor,
                &mut self.warnings,
            );
        }
        let np = self.moving.number_of_parameters();
        let mut first = R::ZERO;
        let mut result = vec![R::ZERO; if derivative { np } else { 0 }];
        let mut total = R::ZERO;
        for (j, entry) in self.metrics.iter_mut().enumerate() {
            let (v, d) = Self::evaluate_one(
                entry,
                &self.moving,
                derivative,
                self.floor,
                &mut self.warnings,
            )?;
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

impl<R: Real, const D: usize> LevelProblem<'_, R, D> {
    /// `SampleVirtualDomainWithPointSet`'s check: an empty point set throws.
    fn check_samples(&self) -> Result<(), ItkException> {
        if self.point_set_samples && self.samples.is_empty() {
            return Err(ItkException::new(
                "Modules/Numerics/Optimizersv4/include/itkRegistrationParameterScalesEstimator.hxx",
                609,
                "RegistrationParameterScalesFromPhysicalShift",
                "The virtual domain point set has no points.",
            ));
        }
        Ok(())
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
        self.check_samples()?;
        scales::estimate_scales(&mut self.moving, &self.samples)
    }

    fn estimate_step_scale(&mut self, step: &[R]) -> Result<R, ItkException> {
        self.check_samples()?;
        scales::estimate_step_scale(&mut self.moving, &self.samples, step)
    }

    fn estimate_maximum_step(&self) -> R {
        scales::maximum_step(&self.virtual_space)
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
pub fn gradient_image<R: Real, const D: usize>(
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

/// The observer's full-scale similarity (`UpdateFullScaleMetricValue`):
/// `ANTSNeighborhoodCorrelationImageToImageMetricv4` of radius 4 between the original
/// (unprocessed, full-resolution) images, on the fixed image's grid, every voxel, no mask, no
/// gradient filter, with the level's fixed and moving transforms. The "no valid points"
/// warning goes to `warnings`.
pub fn full_scale_cc<R: Real, const D: usize>(
    fixed: &Arc<Image<R, D>>,
    moving: &Arc<Image<R, D>>,
    fixed_transform: Option<Composite<R, D>>,
    moving_transform: &Composite<R, D>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<R, ItkException> {
    let metric = ImageMetricv4 {
        fixed: Arc::clone(fixed),
        moving: Arc::clone(moving),
        fixed_mask: None,
        moving_mask: None,
        fixed_transform,
        virtual_space: fixed.space.clone(),
        samples: None,
        fixed_gradient_image: None,
        moving_gradient_image: None,
        pixel_f32: false,
    };
    let r =
        NeighborhoodCorrelation { radius: 4 }.evaluate(&metric, moving_transform, false, floor)?;
    if r.no_valid_points {
        warnings.push(no_valid_points_warning(
            VERIFY_FILE,
            544,
            "ANTSNeighborhoodCorrelationImageToImageMetricv4",
        ));
    }
    Ok(r.value)
}

impl<R: Real, const D: usize> Stage<R, D> {
    /// The virtual domain the stage's levels shrink: the first metric's own (an image metric's
    /// fixed image, a point-set metric's fixed mask), else the first image metric's fixed
    /// image (`GetCurrentLevelVirtualDomainImage`, then `GetFixedImage(firstImageMetric)`);
    /// `None` for point-set metrics without a fixed mask (ANTs crashes).
    pub fn virtual_base(&self) -> Option<Space<D>> {
        if let Some(s) = self.metrics.first()?.virtual_domain() {
            return Some(s.clone());
        }
        self.metrics.iter().find_map(|m| match m {
            StageMetric::Image(i) => Some(i.fixed.space.clone()),
            StageMetric::PointSet(_) => None,
        })
    }
}

fn crash(description: impl Into<String>) -> ItkException {
    ItkException::new(CRASH_FILE, 0, "", description)
}

/// Runs a stage: returns the optimised transform and each level's outcome.
pub fn run_stage<R: Real, const D: usize, O>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut ConjugateGradientLineSearch<R>,
    observer: &mut O,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    events: &mut PointSetEvents,
) -> Result<(Xfm<R, D>, Vec<Outcome<R>>), ItkException>
where
    O: for<'a> Observer<R, LevelProblem<'a, R, D>>,
{
    let result = run_levels(stage, output, optimizer, observer, floor, warnings);
    for p in &result.1 {
        events.merge(&p.events);
    }
    result.0
}

/// A stage's result (its transform and per-level outcomes), and its point-set metrics as they
/// end.
type StageRun<R, const D: usize> = (
    Result<(Xfm<R, D>, Vec<Outcome<R>>), ItkException>,
    Vec<Box<PointSetMetric<R, D>>>,
);

/// The levels of [`run_stage`]; also the stage's point-set metrics as they end.
fn run_levels<R: Real, const D: usize, O>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut ConjugateGradientLineSearch<R>,
    observer: &mut O,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> StageRun<R, D>
where
    O: for<'a> Observer<R, LevelProblem<'a, R, D>>,
{
    // The point-set metrics live through the stage (ITK keeps one metric object).
    let mut point_sets: Vec<Option<Box<PointSetMetric<R, D>>>> = stage
        .metrics
        .iter()
        .map(|m| match m {
            StageMetric::PointSet(p) => Some(Box::new(p.metric.clone())),
            StageMetric::Image(_) => None,
        })
        .collect();
    let result = levels(
        stage,
        output,
        optimizer,
        observer,
        floor,
        warnings,
        &mut point_sets,
    );
    (result, point_sets.into_iter().flatten().collect())
}

#[allow(clippy::too_many_arguments)]
fn levels<R: Real, const D: usize, O>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut ConjugateGradientLineSearch<R>,
    observer: &mut O,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    point_sets: &mut [Option<Box<PointSetMetric<R, D>>>],
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
    let Some(virtual_base) = stage.virtual_base() else {
        return Err(crash(
            "a stage of point-set metrics without a fixed mask has no virtual domain (ANTs reads its spacing from a null image)",
        ));
    };
    let multi = stage.metrics.len() > 1;
    // ObjectToObjectMultiMetricv4's weights, normalised to sum 1 at each Initialize.
    let mut weights: Vec<R> = stage.metrics.iter().map(|m| m.weight()).collect();
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
        let mut images: Vec<Option<ImageMetricv4<R, D>>> = Vec::with_capacity(stage.metrics.len());
        for sm in &stage.metrics {
            images.push(match sm {
                StageMetric::Image(sm) => Some(ImageMetricv4 {
                    fixed: smooth(&sm.fixed)?,
                    moving: smooth(&sm.moving)?,
                    fixed_mask: sm.fixed_mask.clone(),
                    moving_mask: sm.moving_mask.clone(),
                    fixed_transform: stage.fixed_initial.clone(),
                    virtual_space: virtual_space.clone(),
                    samples: None,
                    fixed_gradient_image: None,
                    moving_gradient_image: None,
                    pixel_f32: false,
                }),
                StageMetric::PointSet(_) => None,
            });
        }
        // SetMetricSamplePoints: one set per metric, each from the first metric's virtual
        // domain and fixed mask, with the next seeds; every metric is cast to an image metric.
        if stage.sampling != SamplingStrategy::None {
            let StageMetric::Image(first) = &stage.metrics[0] else {
                return Err(ItkException::new(
                    FILE,
                    if multi { 924 } else { 938 },
                    CLASS,
                    "Invalid metric conversion.",
                ));
            };
            let first_mask = first.fixed_mask.clone();
            for m in &mut images {
                let points = sample_points(
                    &virtual_space,
                    stage.sampling,
                    stage.sampling_percentage,
                    first_mask.as_ref(),
                    &mut current_seed,
                );
                match m {
                    Some(m) => m.samples = Some(Arc::new(points)),
                    None => {
                        return Err(crash(
                            "a stage that samples its metrics (Regular or Random) with a point-set metric: ANTs gives the sample points to a null image metric and crashes",
                        ));
                    }
                }
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
        let mut transforms = stage.moving_initial.clone();
        transforms.push(output.clone());
        let moving = Composite::new(transforms);
        let mut entries = Vec::with_capacity(images.len());
        let mut level_warnings = Vec::new();
        for (j, (m, sm)) in images.into_iter().zip(&stage.metrics).enumerate() {
            let initialized = match (m, sm) {
                (Some(mut m), StageMetric::Image(sm)) => (|| {
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
                        return LevelMetric::initialize(sm.kind, &m)
                            .map(|s| LevelEntry::Image(m, s));
                    }
                    if sm.use_gradient_filter {
                        m.moving_gradient_image =
                            Some(Arc::new(gradient_image(&m.moving, stage.n_threads)?));
                    }
                    let state = LevelMetric::initialize(sm.kind, &m)?;
                    Ok(LevelEntry::Image(m, state))
                })(),
                (_, StageMetric::PointSet(_)) => {
                    let mut p = point_sets[j].take().expect("kept between levels");
                    let r = p.initialize(
                        Some(virtual_space.clone()),
                        stage.fixed_initial.as_ref(),
                        &moving,
                        &mut level_warnings,
                    );
                    match r {
                        Ok(()) => Ok(LevelEntry::PointSet(p)),
                        Err(e) => {
                            point_sets[j] = Some(p);
                            Err(e)
                        }
                    }
                }
                (None, StageMetric::Image(_)) => unreachable!("built above"),
            };
            match initialized {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warnings.append(&mut level_warnings);
                    for (k, entry) in entries.into_iter().enumerate() {
                        if let LevelEntry::PointSet(p) = entry {
                            point_sets[k] = Some(p);
                        }
                    }
                    if multi {
                        return Err(e.wrapped(
                            "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
                            226,
                            "Caught exception initializing metric: \n",
                        ));
                    }
                    return Err(e);
                }
            }
        }
        // The scales estimator's samples: the first metric's fixed points when it is a
        // point-set metric (SetVirtualDomainPointSet), else the domain's corners.
        let samples = match &entries[0] {
            LevelEntry::PointSet(p) => p
                .inputs
                .fixed_points()
                .iter()
                .map(|q| std::array::from_fn(|k| f64::from(q[k])))
                .collect(),
            LevelEntry::Image(..) => scales::corners(&virtual_space),
        };
        let point_set_samples = matches!(entries[0], LevelEntry::PointSet(_));
        let mut problem = LevelProblem {
            metrics: entries,
            weights: weights.clone(),
            moving,
            samples,
            point_set_samples,
            virtual_space: virtual_space.clone(),
            floor,
            warnings: level_warnings,
        };
        let outcome = optimizer.start(&mut problem, observer);
        warnings.append(&mut problem.warnings);
        output = problem.moving.active().clone();
        for (j, entry) in problem.metrics.into_iter().enumerate() {
            if let LevelEntry::PointSet(p) = entry {
                point_sets[j] = Some(p);
            }
        }
        outcomes.push(outcome?);
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

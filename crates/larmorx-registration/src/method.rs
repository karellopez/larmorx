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
use crate::optimizer::{LevelOptimizer, Observer, Outcome, Problem, scales};
use crate::pointset::{PointSetEvents, PointSetMetric};
use crate::rng::{MersenneTwister, RandomIterator};
use crate::transform::{Composite, Xfm};
use crate::{Image, ItkException, Real, Space};

pub(crate) const FILE: &str =
    "Modules/Registration/RegistrationMethodsv4/include/itkImageRegistrationMethodv4.hxx";
pub(crate) const CLASS: &str = "ImageRegistrationMethodv4";

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

/// ITK's exception for a sampled metric left without points: thrown in
/// `ImageToImageMetricv4::GetValueAndDerivativeExecute` (`itkImageToImageMetricv4.hxx:263`),
/// in the joint-histogram metric's `InitializeForIteration` (line 233) for `MI`; the class
/// printed is the metric's own (random testing, 2026-10-25: larmorx printed line 268 and the
/// base class).
pub(crate) fn empty_sample_set(kind: MetricKind) -> ItkException {
    const BASE: &str = "Modules/Registration/Metricsv4/include/itkImageToImageMetricv4.hxx";
    let (file, line, class) = match kind {
        MetricKind::Mi { .. } => (
            "Modules/Registration/Metricsv4/include/itkJointHistogramMutualInformationImageToImageMetricv4.hxx",
            233,
            "JointHistogramMutualInformationImageToImageMetricv4",
        ),
        MetricKind::Mattes { .. } => (BASE, 263, "MattesMutualInformationImageToImageMetricv4"),
        MetricKind::MeanSquares => (BASE, 263, "MeanSquaresImageToImageMetricv4"),
        MetricKind::Gc => (BASE, 263, "CorrelationImageToImageMetricv4"),
        MetricKind::Cc { .. } => (BASE, 263, "ANTSNeighborhoodCorrelationImageToImageMetricv4"),
        MetricKind::Demons => (BASE, 263, "DemonsImageToImageMetricv4"),
    };
    ItkException::new(
        file,
        line,
        class,
        "VirtualSampledPointSet must have 1 or more points.",
    )
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
    /// A B-spline displacement field's meshes per level (`BSplineDisplacementField`); `None`
    /// for other transforms.
    pub field_meshes: Option<FieldMeshes>,
    /// A B-spline transform's mesh at the base level and physical dimensions
    /// (`BSplineTransformParametersAdaptor` per level); `None` for other transforms.
    pub bspline_mesh: Option<BSplineMesh<D>>,
    /// ITK-30 corrected: GC's derivative for a displacement field is each voxel's own.
    pub correct_gc_derivative: bool,
    /// ITK-35 corrected: a B-spline transform applied after the stage's transform has its
    /// Jacobian with respect to position (ITK throws at the first derivative).
    pub correct_bspline_jacobian: bool,
    /// ITK-97 and ITK-99: the derivatives of sample points outside the virtual region,
    /// aliased as ITK does (`reproduce`) or dropped, and their counts.
    pub outside: crate::metric::OutsideDerivatives,
    pub n_threads: usize,
}

/// The adaptors of a B-spline displacement field stage
/// (`BSplineSmoothingOnUpdateDisplacementFieldTransformParametersAdaptor`): at level `l` the
/// meshes are `mesh << (l + shift)` (ANTs: `shift` 1), the numbers of control points the mesh
/// plus the order where the mesh is positive, else 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMeshes {
    pub order: u32,
    pub update_mesh: Vec<u32>,
    pub total_mesh: Vec<u32>,
    pub shift: u32,
}

impl FieldMeshes {
    /// The control points of `mesh` at `level`.
    pub fn control_points(&self, mesh: &[u32], level: usize) -> Vec<u32> {
        mesh.iter()
            .map(|&m| {
                let m = m.wrapping_shl(level as u32 + self.shift);
                if m > 0 { m + self.order } else { 0 }
            })
            .collect()
    }
}

/// The adaptors of a `BSpline` stage (`BSplineTransformParametersAdaptor`): at level `l` the
/// mesh `mesh << l`, on the level's virtual domain (its origin and direction) with the stage's
/// physical dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineMesh<const D: usize> {
    pub mesh: [usize; D],
    pub physical: [f64; D],
}

/// A metric, initialised for a level.
pub enum LevelMetric<R> {
    Mattes(Mattes<R>),
    MeanSquares,
    Mi(Box<JointHistogramMi<R>>),
    /// GC; with a transform of local support, ITK-30 corrected or not.
    Gc {
        correct_local: bool,
    },
    Cc(NeighborhoodCorrelation),
    /// Demons (a displacement field only).
    Demons,
}

const VERIFY_FILE: &str = "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx";

impl<R: Real> LevelMetric<R> {
    /// `Initialize()` of the level's metric (Demons throws for a linear transform).
    pub(crate) fn initialize<const D: usize>(
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
            MetricKind::Gc => LevelMetric::Gc {
                correct_local: true,
            },
            MetricKind::Cc { radius } => LevelMetric::Cc(NeighborhoodCorrelation { radius }),
            MetricKind::Demons => {
                crate::metric::demons::normalizer(metric)?;
                LevelMetric::Demons
            }
        })
    }

    /// A Mattes metric's joint histogram images, kept by the metric object across the levels
    /// and calls of a stage (`flag` is the stage's).
    pub(crate) fn share_joint_pdf(&mut self, flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) {
        if let LevelMetric::Mattes(m) = self {
            m.share_joint_pdf(flag);
        }
    }

    /// `GetValue` or `GetValueAndDerivative`.
    pub(crate) fn evaluate<const D: usize>(
        &self,
        metric: &ImageMetricv4<R, D>,
        moving: &Composite<R, D>,
        derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        self.evaluate_jacobian(metric, moving, derivative, floor, warnings)
            .map(|(v, d, _)| (v, d))
    }

    /// [`LevelMetric::evaluate`], and whether a point's derivative used the moving
    /// transform's Jacobian ([`Reduced::jacobian_used`](mean_squares::Reduced)).
    pub(crate) fn evaluate_jacobian<const D: usize>(
        &self,
        metric: &ImageMetricv4<R, D>,
        moving: &Composite<R, D>,
        derivative: bool,
        floor: &IndexFloor,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>, bool), ItkException> {
        if let LevelMetric::Cc(m) = self {
            let r = m.evaluate(metric, moving, derivative, floor)?;
            let used = r.jacobian_used;
            if r.no_valid_points {
                warnings.push(no_valid_points_warning(
                    VERIFY_FILE,
                    544,
                    "ANTSNeighborhoodCorrelationImageToImageMetricv4",
                ));
            }
            return Ok((r.value, r.derivative, used));
        }
        // Demons' derivative is the image gradient's own (`ProcessPoint` never asks the moving
        // transform for a Jacobian): a B-spline transform after the active one never throws
        // there (ITK-35; found by differential random testing, 2026-10-25).
        let used = !matches!(self, LevelMetric::Demons);
        self.evaluate_inner(metric, moving, derivative, floor, warnings)
            .map(|(v, d)| (v, d, used))
    }

    fn evaluate_inner<const D: usize>(
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
            LevelMetric::Mattes(m) => m.evaluate(metric, moving, derivative, floor, warnings),
            LevelMetric::MeanSquares => {
                let r = mean_squares::evaluate(metric, moving, derivative, floor)?;
                Ok(reduced(r, "MeanSquaresImageToImageMetricv4", warnings))
            }
            LevelMetric::Mi(m) => m.evaluate(metric, moving, derivative, floor, warnings),
            LevelMetric::Gc { correct_local } if metric.local_support && derivative => {
                correlation::local_derivative(metric, moving, *correct_local, floor, warnings)
            }
            LevelMetric::Gc { .. } => {
                correlation::evaluate(metric, moving, derivative, floor, warnings)
            }
            LevelMetric::Demons => {
                let r = crate::metric::demons::evaluate(metric, moving, derivative, floor)?;
                Ok(reduced(r, "DemonsImageToImageMetricv4", warnings))
            }
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
pub(crate) fn magnitude<R: Real>(v: &[R]) -> R {
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
    /// ITK-35 corrected: B-spline transforms after the active one have a Jacobian with
    /// respect to position.
    bspline_jacobian: bool,
    /// Warnings ITK's output window would print (`itkWarningMacro`).
    pub warnings: Vec<String>,
    /// The call's thread count: the transform's updates and the scales run in a pool of that
    /// size (`None`: the caller's pool).
    n_threads: Option<usize>,
}

impl<'a, R: Real, const D: usize> LevelProblem<'a, R, D> {
    fn estimate_scales_inner(&mut self, no_shift: &mut bool) -> Result<Vec<R>, ItkException> {
        self.check_samples()?;
        if self.moving.active().is_bspline() {
            // CentralRegionSampling (VirtualDomainPointSetSampling with a point-set metric
            // first); every parameter.
            let samples = if self.point_set_samples {
                self.samples.clone()
            } else {
                scales::central_region(&self.virtual_space)
            };
            return scales::estimate_bspline_scales(&mut self.moving, &samples, no_shift);
        }
        if self.moving.active().has_local_support() {
            // CentralRegionSampling (VirtualDomainPointSetSampling with a point-set metric
            // first), the parameters of the central voxel (the first point's).
            let (samples, index) = if self.point_set_samples {
                let p = self.samples[0];
                (self.samples.clone(), self.virtual_space.point_to_index(&p))
            } else {
                (
                    scales::central_region(&self.virtual_space),
                    scales::central_index(&self.virtual_space),
                )
            };
            if !self.virtual_space.contains(&index) {
                return Err(ItkException::new(
                    "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx",
                    330,
                    "ObjectToObjectMetric",
                    " point is not inside virtual domain. Cannot compute offset. ",
                ));
            }
            let offset = self.virtual_space.offset(&index) * D;
            return scales::estimate_local_scales(&mut self.moving, &samples, offset, no_shift);
        }
        scales::estimate_scales_checked(&mut self.moving, &self.samples, no_shift)
    }

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
            return Err(empty_sample_set(kind));
        }
        let state = LevelMetric::initialize(kind, &metric)?;
        let samples = scales::corners(&metric.virtual_space);
        let virtual_space = metric.virtual_space.clone();
        let n_threads = metric.n_threads;
        Ok(LevelProblem {
            metrics: vec![LevelEntry::Image(metric, state)],
            weights: vec![R::ONE],
            moving,
            samples,
            point_set_samples: false,
            virtual_space,
            floor,
            bspline_jacobian: false,
            warnings: Vec::new(),
            n_threads,
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
        bspline_jacobian: bool,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        // A B-spline transform after the active one throws at the first point whose
        // derivative is computed (any valid point); ITK-35 corrected, it has its Jacobian.
        let jacobian_error = if derivative && !bspline_jacobian {
            moving.position_jacobian_error()
        } else {
            None
        };
        let before = warnings.len();
        let mut jacobian_used = true;
        let result = match entry {
            LevelEntry::Image(m, state) => state
                .evaluate_jacobian(m, moving, derivative, floor, warnings)
                .map(|(v, d, used)| {
                    jacobian_used = used;
                    (v, d)
                }),
            LevelEntry::PointSet(p) => {
                if derivative {
                    p.value_and_derivative(moving, warnings)
                        .map(|v| (v.value, v.derivative))
                } else {
                    Ok((p.value(moving, warnings)?, Vec::new()))
                }
            }
        };
        let Some(e) = jacobian_error else {
            return result;
        };
        if result.is_ok() && !jacobian_used {
            return result;
        }
        let no_valid = warnings[before..]
            .iter()
            .any(|w| w.contains("No valid points were found"));
        if let (LevelEntry::Image(_, LevelMetric::Mattes(_)), Err(m)) = (&*entry, &result)
            && m.description.starts_with("All samples map outside")
        {
            return result;
        }
        if no_valid {
            return result;
        }
        warnings.truncate(before);
        const BASE: &str = "Modules/Registration/Metricsv4/include/itkImageToImageMetricv4GetValueAndDerivativeThreaderBase.hxx";
        const PROCESS: &str = "Exception in GetValueAndDerivativeProcessPoint:\n";
        Err(match entry {
            LevelEntry::Image(m, state) => match state {
                LevelMetric::Gc { .. } => e.wrapped(
                    "Modules/Registration/Metricsv4/include/itkCorrelationImageToImageMetricv4GetValueAndDerivativeThreader.hxx",
                    250,
                    PROCESS,
                ),
                LevelMetric::Cc(_) => e.wrapped(
                    "Modules/Registration/Metricsv4/include/itkANTSNeighborhoodCorrelationImageToImageMetricv4GetValueAndDerivativeThreader.hxx",
                    if m.samples.is_some() { 628 } else { 82 },
                    "Caught exception: \n",
                ),
                _ => e.wrapped(BASE, 259, PROCESS),
            },
            LevelEntry::PointSet(_) => e,
        })
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
                self.bspline_jacobian,
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
                self.bspline_jacobian,
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

    fn estimate_step_scale_inner(&mut self, step: &mut [R]) -> Result<R, ItkException> {
        self.check_samples()?;
        if self.moving.active().has_local_support() || self.moving.active().is_bspline() {
            // FullDomainSampling (the point set with a point-set metric first).
            let samples = if self.point_set_samples {
                self.samples.clone()
            } else {
                scales::full_domain(&self.virtual_space)
            };
            return scales::estimate_local_step_scale(&mut self.moving, &samples, step);
        }
        scales::estimate_step_scale(&mut self.moving, &self.samples, step)
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

    fn update(&mut self, update: &mut [R]) -> Result<(), ItkException> {
        let moving = &mut self.moving;
        crate::metric::in_pool(self.n_threads, || {
            moving.active_mut().update_in_place(update, R::ONE)
        })?
    }

    fn number_of_local_parameters(&self) -> usize {
        self.moving.number_of_local_parameters()
    }

    fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    fn estimate_scales(&mut self) -> Result<Vec<R>, ItkException> {
        // ITK warns when no parameter moves a voxel (found by differential random testing,
        // 2026-10-25: antsAI's start points outside the images).
        let mut no_shift = false;
        let n_threads = self.n_threads;
        let scales =
            crate::metric::in_pool(n_threads, || self.estimate_scales_inner(&mut no_shift))
                .and_then(|r| r);
        if no_shift {
            self.warnings.push(scales::NO_SHIFT_WARNING.to_owned());
        }
        scales
    }

    fn estimate_step_scale(&mut self, step: &mut [R]) -> Result<R, ItkException> {
        let n_threads = self.n_threads;
        crate::metric::in_pool(n_threads, || self.estimate_step_scale_inner(step))?
    }

    fn estimate_maximum_step(&self) -> R {
        scales::maximum_step(&self.virtual_space)
    }
}

/// A level's smoothed image (`InitializeRegistrationAtEachLevel`):
/// `SmoothingRecursiveGaussianImageFilter` with the level's sigma times each axis's spacing
/// (or in millimetres); the image itself for a sigma ≤ 0.
pub fn smooth_for_level<R: Real, const D: usize>(
    image: &Arc<Image<R, D>>,
    sigma: R,
    physical: bool,
    n_threads: usize,
) -> Result<Arc<Image<R, D>>, ItkException> {
    if sigma <= R::ZERO {
        return Ok(Arc::clone(image));
    }
    let sigmas: Vec<f64> = (0..D)
        .map(|i| {
            let s = sigma.to_f64();
            if physical {
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
        n_threads,
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
    let mut sigma = 0.0f64;
    for &s in &image.space.spacing {
        if s > sigma {
            sigma = s;
        }
    }
    gradient_image_with(image, sigma, true, n_threads)
}

/// `GradientRecursiveGaussianImageFilter` of `sigma` (physical units) with
/// `UseImageDirection` on: each vector turned by the image's direction.
pub fn gradient_image_with<R: Real, const D: usize>(
    image: &Image<R, D>,
    sigma: f64,
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<Image<[f64; D], D>, ItkException> {
    let space = &image.space;
    let components = R::gradient(
        &image.data,
        &space.size,
        &space.spacing,
        sigma,
        normalize_across_scale,
        n_threads,
    )
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
    n_threads: usize,
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
        local_support: false,
        outside: Default::default(),
        n_threads: Some(n_threads),
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
pub fn run_stage<R: Real, const D: usize, O, Opt: LevelOptimizer<R>>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut Opt,
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
fn run_levels<R: Real, const D: usize, O, Opt: LevelOptimizer<R>>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut Opt,
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
fn levels<R: Real, const D: usize, O, Opt: LevelOptimizer<R>>(
    stage: &Stage<R, D>,
    output: Xfm<R, D>,
    optimizer: &mut Opt,
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
    // Each metric object's joint histogram images (Mattes), kept across the levels.
    let joint_pdfs: Vec<std::sync::Arc<std::sync::atomic::AtomicBool>> =
        stage.metrics.iter().map(|_| Default::default()).collect();
    for (level, factors) in stage.shrink_factors.iter().enumerate() {
        let virtual_space = shrink_space(&virtual_base, factors)?;
        // The level's adaptor: a displacement field resampled onto the level's virtual domain
        // (DisplacementFieldTransformParametersAdaptor), a B-spline field given the level's
        // numbers of control points.
        if let Xfm::Field(f) = &mut output {
            let adapted = crate::metric::in_pool(Some(stage.n_threads), || {
                crate::field::adapt(f, &virtual_space, floor)
            })?;
            *f = adapted;
            if let (
                Some(meshes),
                Some(crate::field::FieldSmoothing::BSpline {
                    update_control_points,
                    total_control_points,
                    ..
                }),
            ) = (&stage.field_meshes, &mut f.smoothing)
            {
                *update_control_points = meshes.control_points(&meshes.update_mesh, level);
                *total_control_points = meshes.control_points(&meshes.total_mesh, level);
            }
            // BSplineExponentialDiffeomorphicTransformParametersAdaptor: the level's numbers of
            // control points for the update and the velocity fields.
            if let (Some(meshes), Some(crate::field::FieldSmoothing::Exponential(e))) =
                (&stage.field_meshes, &mut f.smoothing)
                && let crate::exponential::ExponentialSmoothing::BSpline {
                    update_control_points,
                    velocity_control_points,
                    ..
                } = &mut e.smoothing
            {
                *update_control_points = meshes.control_points(&meshes.update_mesh, level);
                *velocity_control_points = meshes.control_points(&meshes.total_mesh, level);
            }
        }
        // A B-spline transform's adaptor: the level's mesh on the level's virtual domain.
        if let (Xfm::BSpline(b), Some(m)) = (&mut output, &stage.bspline_mesh) {
            let mesh: [usize; D] = std::array::from_fn(|d| m.mesh[d] << level);
            let required = crate::transform::bspline::required_fixed(
                &mesh,
                &virtual_space.origin,
                &m.physical,
                &virtual_space.direction,
            );
            crate::metric::in_pool(Some(stage.n_threads), || {
                crate::transform::bspline::adapt(b, &required, floor)
            })??;
        }
        let local_support = output.has_local_support();
        let sigma = stage.smoothing_sigmas[level];
        let smooth = |image: &Arc<Image<R, D>>| -> Result<Arc<Image<R, D>>, ItkException> {
            smooth_for_level(
                image,
                sigma,
                stage.sigmas_in_physical_units,
                stage.n_threads,
            )
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
                    local_support,
                    outside: stage.outside.clone(),
                    n_threads: Some(stage.n_threads),
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
                        return Err(empty_sample_set(sm.kind));
                    }
                    // Demons checks its transform before ImageToImageMetricv4::Initialize, and
                    // takes its gradient from the fixed image.
                    if sm.kind == MetricKind::Demons {
                        crate::metric::demons::normalizer(&m)?;
                        if sm.use_gradient_filter {
                            m.fixed_gradient_image =
                                Some(Arc::new(gradient_image(&m.fixed, stage.n_threads)?));
                        }
                        return LevelMetric::initialize(sm.kind, &m).map(|mut s| {
                            s.share_joint_pdf(&joint_pdfs[j]);
                            LevelEntry::Image(m, s)
                        });
                    }
                    if sm.use_gradient_filter {
                        m.moving_gradient_image =
                            Some(Arc::new(gradient_image(&m.moving, stage.n_threads)?));
                    }
                    let mut state = LevelMetric::initialize(sm.kind, &m)?;
                    state.share_joint_pdf(&joint_pdfs[j]);
                    if let LevelMetric::Gc { correct_local } = &mut state {
                        *correct_local = stage.correct_gc_derivative;
                    }
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
            // A moving composite that is not linear (a displacement field before the stage's
            // transform): `RandomSampling` of 1000 virtual voxels (`SizeOfSmallDomain`). ITK's
            // iterator is seeded from the clock (ANTS-72); larmorx from the stage's seed.
            LevelEntry::Image(..) if moving.transforms.iter().any(|t| !t.is_linear()) => {
                let mut it =
                    RandomIterator::new(virtual_space.size, virtual_space.start, stage.seed);
                it.samples(1000)
                    .iter()
                    .map(|idx| virtual_space.index_to_point(idx))
                    .collect()
            }
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
            bspline_jacobian: stage.correct_bspline_jacobian,
            warnings: level_warnings,
            n_threads: Some(stage.n_threads),
        };
        let outcome = optimizer.start_level(&mut problem, observer);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::bspline::BSplineXfm;
    use crate::transform::{Kind, LinearXfm};

    /// A smooth blob on a grid of `n`³ voxels of 1.5 mm from `origin`, centred at `c` (mm).
    fn blob(n: usize, origin: f64, c: [f64; 3]) -> Image<f64, 3> {
        let space = Space::new(
            [n; 3],
            [0; 3],
            [1.5; 3],
            [origin; 3],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap();
        let data = (0..space.len())
            .map(|o| {
                let p = space.index_to_point(&space.index_of(o));
                let r2: f64 = (0..3).map(|k| ((p[k] - c[k]) / 6.0).powi(2)).sum();
                let s2: f64 = (0..3).map(|k| ((p[k] - c[k] - 3.0) / 4.0).powi(2)).sum();
                100.0 * (-0.5 * r2).exp() + 40.0 * (-0.5 * s2).exp()
            })
            .collect();
        Image::new(space, data)
    }

    /// The metric's parallel loops run in a pool of the call's thread count, never on rayon's
    /// global pool (sized by the machine: the data panel's re-validation, 2026-10-26); with
    /// `None` they run in the caller's pool.
    #[test]
    fn metric_samples_run_in_the_calls_pool() {
        let fixed = Arc::new(blob(40, 0.0, [20.0; 3]));
        let pool_of = |n_threads: Option<usize>| {
            let metric = ImageMetricv4 {
                fixed: Arc::clone(&fixed),
                moving: Arc::clone(&fixed),
                fixed_mask: None,
                moving_mask: None,
                fixed_transform: None,
                virtual_space: fixed.space.clone(),
                samples: None,
                fixed_gradient_image: None,
                moving_gradient_image: None,
                pixel_f32: false,
                local_support: false,
                outside: Default::default(),
                n_threads,
            };
            let mut seen = std::collections::BTreeSet::new();
            metric
                .for_each_sample(
                    |_| {
                        Some((
                            rayon::current_num_threads(),
                            std::thread::current().name().map(str::to_string),
                        ))
                    },
                    |x| {
                        seen.insert(x);
                        Ok(())
                    },
                )
                .unwrap();
            seen
        };
        for n in [1, 3] {
            let seen = pool_of(Some(n));
            assert!(!seen.is_empty());
            for (threads, name) in &seen {
                assert_eq!(*threads, n);
                let name = name.as_deref().unwrap_or("");
                assert!(name.starts_with(&format!("larmorx-{n}-")), "{name}");
            }
        }
        // `None`: the pool the caller installed.
        let seen = larmorx_core::parallel::with_threads(2, || pool_of(None)).unwrap();
        assert!(seen.iter().all(|(t, name)| {
            *t == 2 && name.as_deref().is_some_and(|s| s.starts_with("larmorx-2-"))
        }));
    }

    /// ITK-97 and ITK-99 (differential random testing, 2026-10-25): the parameter offset of a
    /// virtual index outside the region, as ITK computes it (the region's linear offset): a
    /// voxel of the array (aliased, kept only with `alias`) or none (past the array).
    #[test]
    fn outside_indices_alias_as_in_itk() {
        let fixed = Arc::new(blob(4, 0.0, [3.0; 3]));
        for alias in [false, true] {
            let metric = ImageMetricv4 {
                fixed: Arc::clone(&fixed),
                moving: Arc::clone(&fixed),
                fixed_mask: None,
                moving_mask: None,
                fixed_transform: None,
                virtual_space: fixed.space.clone(),
                samples: None,
                fixed_gradient_image: None,
                moving_gradient_image: None,
                pixel_f32: false,
                local_support: true,
                outside: crate::metric::OutsideDerivatives::new(alias),
                n_threads: Some(1),
            };
            assert_eq!(metric.voxel_of(&[1, 2, 3]), Some(1 + 4 * 2 + 16 * 3));
            // One past the end of the first axis: the next row's first voxel.
            assert_eq!(metric.voxel_of(&[4, 0, 0]), alias.then_some(4));
            // Before the first voxel, or past the last plane: outside the array.
            assert_eq!(metric.voxel_of(&[-1, 0, 0]), None);
            assert_eq!(metric.voxel_of(&[0, 0, 4]), None);
            assert_eq!((metric.outside.aliased(), metric.outside.beyond()), (1, 2));
        }
    }

    /// The derivative of MeanSquares through a composite against central finite differences
    /// of its value: the best scalar `c` with `derivative ≈ c·fd` and the relative residual.
    fn derivative_check(composite: Composite<f64, 3>) -> (f64, f64) {
        // The moving image extends far beyond the fixed one, so no point leaves it (a point
        // that does changes the number of points and makes the value jump).
        let fixed = Arc::new(blob(24, 0.0, [17.0, 17.0, 17.0]));
        let moving = Arc::new(blob(40, -12.0, [18.5, 16.0, 17.5]));
        let metric = ImageMetricv4 {
            fixed: Arc::clone(&fixed),
            moving,
            fixed_mask: None,
            moving_mask: None,
            fixed_transform: None,
            virtual_space: fixed.space.clone(),
            samples: None,
            fixed_gradient_image: None,
            moving_gradient_image: None,
            pixel_f32: false,
            local_support: false,
            outside: Default::default(),
            n_threads: Some(1),
        };
        let floor = IndexFloor::new(larmorx_core::defects::UpstreamDefects::Fix);
        let mut problem =
            LevelProblem::single(metric, MetricKind::MeanSquares, composite, &floor).unwrap();
        problem.bspline_jacobian = true;
        let (_, derivative) = problem.value_and_derivative().unwrap();
        let theta = problem.parameters();
        let mut fd = vec![0.0; theta.len()];
        for k in 0..theta.len() {
            let h = 1e-5;
            let mut q = theta.clone();
            q[k] += h;
            problem.set_parameters(&q).unwrap();
            let plus = problem.value().unwrap();
            q[k] -= 2.0 * h;
            problem.set_parameters(&q).unwrap();
            let minus = problem.value().unwrap();
            fd[k] = (plus - minus) / (2.0 * h);
        }
        problem.set_parameters(&theta).unwrap();
        problem.set_parameters(&theta).unwrap();
        let dot: f64 = derivative.iter().zip(&fd).map(|(a, b)| a * b).sum();
        let ff: f64 = fd.iter().map(|b| b * b).sum();
        let c = dot / ff;
        let res: f64 = derivative
            .iter()
            .zip(&fd)
            .map(|(a, b)| (a - c * b).powi(2))
            .sum::<f64>()
            .sqrt();
        let norm: f64 = derivative.iter().map(|a| a * a).sum::<f64>().sqrt();
        (c, res / norm)
    }

    /// ITK-35 corrected: an affine transform optimised under a B-spline transform applied after
    /// it gets the metric derivative of the composite, as accurate as without the B-spline
    /// transform (MeanSquares' derivative is minus the value's gradient; the residual, a few
    /// per cent, is the central-difference image gradient against the exact derivative of the
    /// linear interpolation the finite differences see: about 2 % without the B-spline
    /// transform, 4 % through it).
    #[test]
    fn the_derivative_through_a_bspline_transform_matches_finite_differences() {
        let mut affine = LinearXfm::<f64, 3>::new(Kind::Affine);
        affine.set_matrix([
            [1.02, 0.03, -0.01],
            [-0.02, 0.98, 0.02],
            [0.01, -0.03, 1.01],
        ]);
        affine.set_offset([0.4, -0.6, 0.3]);
        let plain = derivative_check(Composite::new(vec![Xfm::Linear(affine.clone())]));
        let mut b = BSplineXfm::<f64, 3>::new_default();
        b.set_domain_origin([0.0; 3]).unwrap();
        b.set_domain_physical([34.5; 3]).unwrap();
        b.set_domain_mesh([2, 2, 2]).unwrap();
        let mut rng = crate::rng::MersenneTwister::from_int(9);
        for c in &mut b.coefficients {
            *c = 3.0 * (rng.closed() - 0.5);
        }
        let chained = derivative_check(Composite::new(vec![Xfm::BSpline(b), Xfm::Linear(affine)]));
        eprintln!("plain {plain:?}, through the B-spline transform {chained:?}");
        for (c, residual) in [plain, chained] {
            assert!((c + 1.0).abs() < 0.02, "scale {c}");
            assert!(residual < 0.05, "residual {residual}");
        }
        // As accurate as the affine transform alone (within a factor of 2 of its residual).
        assert!(
            chained.1 < 2.0 * plain.1.max(1e-3),
            "{chained:?} vs {plain:?}"
        );
    }
}

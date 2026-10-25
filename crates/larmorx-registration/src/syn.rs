// SPDX-License-Identifier: Apache-2.0
//! `SyNImageRegistrationMethod` (ITK v5.4.5,
//! `Modules/Registration/RegistrationMethodsv4/include/itkSyNImageRegistrationMethod.hxx`), as
//! ANTs v2.6.5's `antsRegistration` runs it (`itkantsRegistrationHelper.hxx`, `case SyN`):
//! symmetric normalisation, a fixed-to-middle and a moving-to-middle displacement field updated
//! by gradient steps, with `DownsampleImagesForMetricDerivatives` on and
//! `AverageMidPointGradients` off.
//!
//! Per level, the images are smoothed and the virtual domain shrunk as for a linear stage
//! ([`crate::method`]); the two fields start at zero on the level's domain (or from a
//! restored state), or are resampled onto it from the previous level
//! (`DisplacementFieldTransformParametersAdaptor`). Each iteration:
//!
//! 1. the fixed images are resampled onto the virtual domain through the fixed initial
//!    transform and the inverse of the fixed-to-middle field, the moving images through the
//!    moving initial transforms and the inverse of the moving-to-middle field (masks with the
//!    nearest neighbour);
//! 2. the metric's derivative over the virtual domain is computed twice, once with the
//!    resampled fixed images as the metric's fixed images (the update of the fixed-to-middle
//!    field) and once with the roles swapped (the moving-to-middle field), each with identity
//!    displacement field transforms ([`ImageMetricv4::local_support`]); several metrics are
//!    combined by `ObjectToObjectMultiMetricv4`, whose weights are normalised again at each
//!    `Initialize` (twice per iteration);
//! 3. each update is multiplied by the optimizer weights (`-g`), smoothed
//!    ([`crate::field::gaussian_smooth`], the update variance), scaled to the learning rate
//!    ([`crate::field::scale_update`]), composed with its field and smoothed again (the total
//!    variance); each total field is then inverted twice ([`crate::field::invert`]): its inverse,
//!    from the previous inverse, and the inverse of that, from the total field;
//! 4. the metric value is the mean of the two evaluations' values; the window convergence
//!    monitor stops the level when its value falls below the threshold.
//!
//! At the end the output transform's field is the moving-to-middle inverse composed with the
//! fixed-to-middle field, its inverse the fixed-to-middle inverse composed with the
//! moving-to-middle field.

use std::sync::Arc;

use larmorx_core::IndexFloor;

use crate::convergence::WindowConvergence;
use crate::field::{
    Field, FieldXfm, InvertSettings, adapt, compose, gaussian_smooth, invert, scale_update,
    zero_field,
};
use crate::mask::Mask;
use crate::method::{
    CRASH_FILE, LevelMetric, MetricKind, SamplingStrategy, StageMetric, gradient_image, magnitude,
    sample_points, shrink_space, smooth_for_level,
};
use crate::metric::{ImageMetricv4, demons, no_valid_points_warning};
use crate::pointset::metric::{PointSetEvents, PointSetMetric, TangentCall};
use crate::resample::{resample_mask_nearest, resample_nonlinear_with};
use crate::transform::{Composite, Xfm};
use crate::{Image, ItkException, Real, Space};

const VERIFY_FILE: &str = "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx";

/// A SyN stage as `SyNImageRegistrationMethod` runs it.
#[derive(Clone, Debug)]
pub struct SynStage<R, const D: usize> {
    /// The stage's metrics (`ObjectToObjectMultiMetricv4` when there are several).
    pub metrics: Vec<StageMetric<R, D>>,
    /// Shrink factors per level and axis.
    pub shrink_factors: Vec<[usize; D]>,
    /// Smoothing sigma per level (as `R`).
    pub smoothing_sigmas: Vec<R>,
    pub sigmas_in_physical_units: bool,
    pub sampling: SamplingStrategy,
    pub sampling_percentage: R,
    /// `MetricSamplingReinitializeSeed(seed)`.
    pub seed: i32,
    /// The fixed initial transform (none: ITK passes a null transform).
    pub fixed_initial: Option<Composite<R, D>>,
    /// The moving initial transforms (the registration's composite, flattened; may be empty).
    pub moving_initial: Vec<Xfm<R, D>>,
    /// `SetLearningRate` (the gradient step).
    pub learning_rate: R,
    /// Iterations per level.
    pub iterations: Vec<u32>,
    pub convergence_threshold: R,
    pub convergence_window: usize,
    /// `SetGaussianSmoothingVarianceForTheUpdateField` / `...TotalField` (voxels²).
    pub update_variance: R,
    pub total_variance: R,
    /// `SetOptimizerWeights` (`-g`), set only when it has `D` values.
    pub optimizer_weights: Option<Vec<R>>,
    /// Restored fixed-to-middle and moving-to-middle transforms (`--restore-state`, or the
    /// previous SyN stage with `--initialize-transforms-per-stage`).
    pub restored: Option<(FieldXfm<R, D>, FieldXfm<R, D>)>,
    /// The fields' interpolator floor (ITK-01 corrected or not).
    pub exact_floor: bool,
    /// ITK-30: compute GC's derivative per voxel (corrected) or, as ITK, all of it in the
    /// first voxel's entries.
    pub correct_gc_derivative: bool,
    /// ITK-31: a role without a mask has none (corrected) or keeps the previous call's mask.
    pub correct_stale_masks: bool,
    /// ITK-33: a point-set metric among several metrics is evaluated through the fixed and
    /// moving composites (corrected) or, as ITK, through identity displacement fields.
    pub correct_point_set_transforms: bool,
    /// `BSplineSyNImageRegistrationMethod`: the update and total fields smoothed by B-spline
    /// approximations instead of Gaussians (the variances are then not used).
    pub bspline: Option<BSplineSynSmoothing<D>>,
    pub n_threads: usize,
}

/// The B-spline smoothing of a B-spline SyN stage (`BSplineSyN[step,update,total,order]`): the
/// mesh sizes at the base level, doubled at each level (`meshSize << level`), and the spline
/// order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BSplineSynSmoothing<const D: usize> {
    pub order: u32,
    pub update_mesh: [u32; D],
    pub total_mesh: [u32; D],
}

impl<const D: usize> BSplineSynSmoothing<D> {
    /// `BSplineSmoothingOnUpdateDisplacementFieldTransformParametersAdaptor::SetMeshSize...`
    /// of a level: `mesh << level` plus the order where it is positive, else 0.
    pub fn control_points(mesh: &[u32; D], level: usize, order: u32) -> [u32; D] {
        std::array::from_fn(|d| {
            let m = mesh[d].wrapping_shl(level as u32);
            if m > 0 { m + order } else { 0 }
        })
    }
}

/// `BSplineSyNImageRegistrationMethod::BSplineSmoothDisplacementField`: a copy when a number
/// of control points is 0, else the field's B-spline approximation (one fitting level, the
/// boundary stationary) weighted by `mask`.
pub fn bspline_smooth<R: Real, const D: usize>(
    field: &Field<R, D>,
    control_points: &[u32; D],
    order: u32,
    mask: Option<&Image<f64, D>>,
) -> Result<Field<R, D>, ItkException> {
    if control_points.contains(&0) {
        return Ok(field.clone());
    }
    crate::bspline_field::field_to_bspline(field, mask, control_points, order, true)
}

/// The weighted mask of `BSplineSyNImageRegistrationMethod::ComputeUpdateField`: the first
/// metric's mask resampled onto the virtual domain through `transform`
/// (`ResampleImageFilter<Image<uchar>, Image<double>, R>`, its default linear interpolator,
/// default 0).
fn weighted_mask<R: Real, const D: usize>(
    mask: &Mask<D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    floor: &IndexFloor,
) -> Image<f64, D> {
    let image = &mask.image;
    let data = resample_nonlinear_with(&image.space, output, transform, 0.0f64, |c| {
        crate::interp::at_cindex_with(&image.space, c, floor, |o| f64::from(image.data[o]))
    });
    let space = Space::new(
        output.size,
        [0; D],
        output.spacing,
        output.origin,
        output.direction,
    )
    .expect("the virtual domain's geometry");
    Image::new(space, data)
}

/// What the iteration observer sees after each iteration (`IterationEvent`).
pub struct SynIteration<'a, R, const D: usize> {
    /// The level (0-based, `GetCurrentLevel`).
    pub level: usize,
    /// The iteration (1-based, `GetCurrentIteration`).
    pub iteration: usize,
    /// `GetCurrentMetricValue`.
    pub metric_value: R,
    /// `GetCurrentConvergenceValue`.
    pub convergence_value: R,
    pub fixed_to_middle: &'a FieldXfm<R, D>,
    pub moving_to_middle: &'a FieldXfm<R, D>,
}

/// ANTs' observer of a SyN stage (`antsDisplacementAndVelocityFieldRegistrationCommandIterationUpdate`).
pub trait SynObserver<R, const D: usize> {
    fn iteration(&mut self, state: &SynIteration<'_, R, D>) -> Result<(), ItkException>;
}

/// An observer that does nothing.
pub struct NoObserver;

impl<R, const D: usize> SynObserver<R, D> for NoObserver {
    fn iteration(&mut self, _: &SynIteration<'_, R, D>) -> Result<(), ItkException> {
        Ok(())
    }
}

/// The stage's result: the output transform (the two fields composed) and the
/// fixed-to-middle and moving-to-middle transforms (the registration state).
#[derive(Clone, Debug)]
pub struct SynOutput<R, const D: usize> {
    pub output: FieldXfm<R, D>,
    pub fixed_to_middle: FieldXfm<R, D>,
    pub moving_to_middle: FieldXfm<R, D>,
    /// ITK-31's condition was met: a metric with only one of its two masks evaluated more
    /// than once in a level.
    pub stale_masks: bool,
    /// ITK-33's condition was met: a point-set metric among several metrics was evaluated.
    pub identity_point_sets: bool,
}

impl<R: Real, const D: usize> SynStage<R, D> {
    /// The virtual domain the levels shrink (as [`crate::method::Stage::virtual_base`]).
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

/// `ResampleImageFilter<Image<R>, Image<R>, R>` with the linear interpolator and default 0
/// onto the virtual domain through a composite holding a displacement field.
pub(crate) fn resample_image<R: Real, const D: usize>(
    image: &Image<R, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    floor: &IndexFloor,
) -> Image<R, D> {
    let data = resample_nonlinear_with(&image.space, output, transform, R::ZERO, |c| {
        R::from_f64(crate::interp::at_cindex_with(&image.space, c, floor, |o| {
            image.data[o].to_f64()
        }))
    });
    // The float output is clamped to the float range (CastPixelWithBoundsChecking); a
    // linear interpolation of float values never leaves it.
    Image::new(output.clone(), data)
}

/// One image metric's images and masks resampled onto the virtual domain in an iteration.
pub(crate) struct Resampled<R, const D: usize> {
    pub(crate) fixed: Arc<Image<R, D>>,
    pub(crate) moving: Arc<Image<R, D>>,
    pub(crate) fixed_mask: Option<Mask<D>>,
    pub(crate) moving_mask: Option<Mask<D>>,
    /// The gradient images (`useGradientFilter`), computed on demand.
    pub(crate) fixed_gradient: Option<Arc<Image<[f64; D], D>>>,
    pub(crate) moving_gradient: Option<Arc<Image<[f64; D], D>>>,
}

/// The stage's per-level image metric: its kind and its level images.
pub(crate) struct LevelImageMetric<R, const D: usize> {
    pub(crate) kind: MetricKind,
    pub(crate) fixed: Arc<Image<R, D>>,
    pub(crate) moving: Arc<Image<R, D>>,
    pub(crate) fixed_mask: Option<Mask<D>>,
    pub(crate) moving_mask: Option<Mask<D>>,
    pub(crate) use_gradient_filter: bool,
    pub(crate) samples: Option<Arc<Vec<[f64; D]>>>,
}

/// An image metric after `Initialize` with SyN's identity transforms.
struct Initialized<R, const D: usize> {
    kind: MetricKind,
    metric: ImageMetricv4<R, D>,
    /// The level state of the metrics `LevelMetric` holds (not Demons and GC).
    state: Option<LevelMetric<R>>,
}

/// `Initialize()` of one image metric on the resampled images.
#[allow(clippy::too_many_arguments)]
fn initialize_metric<R: Real, const D: usize>(
    kind: MetricKind,
    fixed: &Arc<Image<R, D>>,
    moving: &Arc<Image<R, D>>,
    fixed_mask: &Option<Mask<D>>,
    moving_mask: &Option<Mask<D>>,
    fixed_gradient: Option<Arc<Image<[f64; D], D>>>,
    moving_gradient: Option<Arc<Image<[f64; D], D>>>,
    virtual_space: &Space<D>,
    samples: &Option<Arc<Vec<[f64; D]>>>,
    fixed_transform: Option<Composite<R, D>>,
) -> Result<Initialized<R, D>, ItkException> {
    let metric = ImageMetricv4 {
        fixed: Arc::clone(fixed),
        moving: Arc::clone(moving),
        fixed_mask: fixed_mask.clone(),
        moving_mask: moving_mask.clone(),
        fixed_transform,
        virtual_space: virtual_space.clone(),
        samples: samples.clone(),
        fixed_gradient_image: fixed_gradient,
        moving_gradient_image: moving_gradient,
        pixel_f32: false,
        local_support: true,
        outside: Default::default(),
    };
    let state = match kind {
        MetricKind::Demons => {
            demons::normalizer(&metric)?;
            None
        }
        MetricKind::Gc => None,
        _ => Some(LevelMetric::initialize(kind, &metric)?),
    };
    Ok(Initialized {
        kind,
        metric,
        state,
    })
}

/// `GetValueAndDerivative` of an initialised image metric: the value and the derivative over
/// the virtual domain (`D` per voxel).
fn metric_derivative<R: Real, const D: usize>(
    m: &Initialized<R, D>,
    correct_gc: bool,
    moving: &Composite<R, D>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<(R, Vec<R>), ItkException> {
    if let Some(s) = &m.metric.samples
        && s.is_empty()
    {
        return Err(crate::method::empty_sample_set(m.kind));
    }
    match (&m.state, m.kind) {
        (Some(state), _) => state.evaluate(&m.metric, moving, true, floor, warnings),
        (None, MetricKind::Gc) => crate::metric::correlation::local_derivative(
            &m.metric, moving, correct_gc, floor, warnings,
        ),
        (None, _) => {
            let r = demons::evaluate(&m.metric, moving, true, floor)?;
            if r.no_valid_points {
                warnings.push(no_valid_points_warning(
                    VERIFY_FILE,
                    544,
                    "DemonsImageToImageMetricv4",
                ));
            }
            Ok((r.value, r.derivative))
        }
    }
}

/// The multi-metric's weights, normalised at each `Initialize` (`m_MetricWeights / sum`).
pub(crate) fn normalise_weights<R: Real>(weights: &mut [R]) -> Result<(), ItkException> {
    let mut sum = R::ZERO;
    for &w in weights.iter() {
        sum = sum + w;
    }
    if sum <= R::EPSILON {
        return Err(ItkException::new(
            "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
            143,
            "ObjectToObjectMultiMetricv4",
            format!(
                "The derivative weights are too small: {}",
                crate::format::itk_array(weights)
            ),
        ));
    }
    for w in weights.iter_mut() {
        *w = *w / sum;
    }
    Ok(())
}

/// What a gradient call takes from its stage.
#[derive(Clone, Copy)]
pub(crate) struct CallSettings<'a, R> {
    pub n_threads: usize,
    /// ITK-30 corrected.
    pub correct_gc_derivative: bool,
    /// `SetOptimizerWeights` (`-g`), set only when it has `D` values.
    pub optimizer_weights: Option<&'a [R]>,
}

impl<'a, R: Real> CallSettings<'a, R> {
    fn of<const D: usize>(stage: &'a SynStage<R, D>) -> Self {
        CallSettings {
            n_threads: stage.n_threads,
            correct_gc_derivative: stage.correct_gc_derivative,
            optimizer_weights: stage.optimizer_weights.as_deref(),
        }
    }
}

/// The metric and its level state (`m_Metric`) for one call of `ComputeMetricGradientField`:
/// `swapped` gives the moving images the role of the fixed ones (the moving-to-middle
/// update).
pub(crate) struct GradientCall<'a, R, const D: usize> {
    /// The image metrics (`None` where the stage has a point-set metric).
    pub(crate) metrics: &'a [Option<LevelImageMetric<R, D>>],
    pub(crate) resampled: &'a mut [Option<Resampled<R, D>>],
    /// The point-set metrics (`None` where the stage has an image metric).
    pub(crate) point_sets: &'a mut [Option<PointSetMetric<R, D>>],
    /// The iteration's fixed and moving composites.
    pub(crate) fixed_composite: &'a Composite<R, D>,
    pub(crate) moving_composite: &'a Composite<R, D>,
    /// The identity displacement field transform ITK gives every metric of a multi-metric
    /// (ITK-33 reproduced), or `None`.
    pub(crate) identity: Option<&'a Composite<R, D>>,
    pub(crate) virtual_space: &'a Space<D>,
    pub(crate) weights: &'a mut Vec<R>,
    pub(crate) multi: bool,
    pub(crate) settings: CallSettings<'a, R>,
    pub(crate) floor: &'a IndexFloor,
    /// Each metric's fixed and moving masks as the metric object holds them between calls:
    /// ITK sets a resampled mask only where the stage has one, so a role without a mask keeps
    /// the mask the previous call set (ITK-31, reproduced when `stale_masks`).
    pub(crate) masks: &'a mut [(Option<Mask<D>>, Option<Mask<D>>)],
    pub(crate) stale_masks: bool,
    /// Each metric object's joint histogram images (Mattes): created at its first evaluation
    /// in the stage.
    pub(crate) joint_pdfs: &'a [Arc<std::sync::atomic::AtomicBool>],
    /// The fixed and moving transforms the image metrics are evaluated through, on top of
    /// their resampled images (ITK-36 reproduced: a time-varying B-spline velocity field's
    /// multi-metric whose last metric is a point-set metric); `None`: the identity.
    pub(crate) through: Option<(&'a Composite<R, D>, &'a Composite<R, D>)>,
}

impl<R: Real, const D: usize> GradientCall<'_, R, D> {
    /// `ComputeMetricGradientField`: the metric's value and its derivative over the virtual
    /// domain as a field, multiplied by the optimizer weights.
    pub(crate) fn gradient_field(
        &mut self,
        swapped: bool,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Field<R, D>), ItkException> {
        // m_Metric->Initialize(): the multi-metric normalises its weights first.
        if self.multi {
            normalise_weights(self.weights)?;
        }
        let n = self.metrics.len();
        let mut initialized = Vec::with_capacity(n);
        for j in 0..n {
            let Some(lm) = &self.metrics[j] else {
                // A point-set metric: SetFixedObject/SetMovingObject (exchanged in the swapped
                // call), the transforms, tangent space; Initialize.
                let (ft, mt) = match self.identity {
                    Some(identity) => (identity, identity),
                    None if swapped => (self.moving_composite, self.fixed_composite),
                    None => (self.fixed_composite, self.moving_composite),
                };
                let call = TangentCall {
                    swapped,
                    fixed_transform: ft,
                    moving_transform: mt,
                    virtual_space: self.virtual_space,
                    sparse: true,
                };
                let p = self.point_sets[j].as_mut().expect("a point-set metric");
                match p.tangent_initialize(&call, warnings) {
                    Ok(()) => initialized.push(None),
                    Err(e) if self.multi => {
                        return Err(e.wrapped(
                            "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
                            226,
                            "Caught exception initializing metric: \n",
                        ));
                    }
                    Err(e) => return Err(e),
                }
                continue;
            };
            let r = self.resampled[j].as_mut().expect("an image metric");
            if lm.use_gradient_filter {
                // Initialize computes the gradient image of the image whose gradient the
                // metric uses: its moving image (Demons: its fixed image). In the swapped call
                // the metric's fixed image is the resampled moving image.
                let uses_moving = lm.kind != MetricKind::Demons;
                if uses_moving == swapped {
                    if r.fixed_gradient.is_none() {
                        r.fixed_gradient =
                            Some(Arc::new(gradient_image(&r.fixed, self.settings.n_threads)?));
                    }
                } else if r.moving_gradient.is_none() {
                    r.moving_gradient = Some(Arc::new(gradient_image(
                        &r.moving,
                        self.settings.n_threads,
                    )?));
                }
            }
            let (fi, mi, fm, mm, fg, mg) = if swapped {
                (
                    &r.moving,
                    &r.fixed,
                    &r.moving_mask,
                    &r.fixed_mask,
                    r.moving_gradient.clone(),
                    r.fixed_gradient.clone(),
                )
            } else {
                (
                    &r.fixed,
                    &r.moving,
                    &r.fixed_mask,
                    &r.moving_mask,
                    r.fixed_gradient.clone(),
                    r.moving_gradient.clone(),
                )
            };
            // SetFixedImageMask / SetMovingImageMask only for the roles that have a mask.
            let state = &mut self.masks[j];
            if fm.is_some() || !self.stale_masks {
                state.0 = fm.clone();
            }
            if mm.is_some() || !self.stale_masks {
                state.1 = mm.clone();
            }
            let (fm, mm) = (&state.0, &state.1);
            let (fg, mg) = if lm.use_gradient_filter {
                (fg, mg)
            } else {
                (None, None)
            };
            let result = initialize_metric(
                lm.kind,
                fi,
                mi,
                fm,
                mm,
                fg,
                mg,
                self.virtual_space,
                &lm.samples,
                self.through.map(|(f, _)| f.clone()),
            );
            match result {
                Ok(mut m) => {
                    if let Some(s) = &mut m.state {
                        s.share_joint_pdf(&self.joint_pdfs[j]);
                    }
                    initialized.push(Some(m));
                }
                Err(e) if self.multi => {
                    return Err(e.wrapped(
                        "Modules/Registration/Metricsv4/include/itkObjectToObjectMultiMetricv4.hxx",
                        226,
                        "Caught exception initializing metric: \n",
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        let mut values = Vec::with_capacity(n);
        let mut derivatives = Vec::with_capacity(n);
        for (j, m) in initialized.iter().enumerate() {
            let (v, d) = match m {
                Some(m) => {
                    let identity = Composite::new(Vec::new());
                    let moving = self.through.map_or(&identity, |(_, m)| m);
                    metric_derivative(
                        m,
                        self.settings.correct_gc_derivative,
                        moving,
                        self.floor,
                        warnings,
                    )?
                }
                None => self.point_sets[j]
                    .as_mut()
                    .expect("a point-set metric")
                    .tangent_evaluate(warnings)?,
            };
            values.push(v);
            derivatives.push(d);
        }
        let total = D * self.virtual_space.len();
        let (value, mut derivative) = if self.multi {
            // ObjectToObjectMultiMetricv4::GetValueAndDerivative.
            let mut result = vec![R::ZERO; total];
            let mut total_magnitude = R::ZERO;
            for (j, d) in derivatives.iter().enumerate() {
                let mag = magnitude(d);
                total_magnitude = total_magnitude + mag;
                let weight_over = if mag > R::EPSILON {
                    self.weights[j] / mag
                } else {
                    R::ZERO
                };
                for (r, &x) in result.iter_mut().zip(d) {
                    *r = *r + x * weight_over;
                }
            }
            let mean = total_magnitude / R::from_i64(n as i64);
            for r in &mut result {
                *r = *r * mean;
            }
            (values[0], result)
        } else {
            (values[0], derivatives.pop().expect("one metric"))
        };
        if let Some(w) = self.settings.optimizer_weights
            && w.len() == D
            && !weights_are_identity(w)
        {
            for chunk in derivative.as_chunks_mut::<D>().0 {
                for d in 0..D {
                    chunk[d] = chunk[d] * w[d];
                }
            }
        }
        let data = derivative.as_chunks::<D>().0.to_vec();
        Ok((value, Image::new(self.virtual_space.clone(), data)))
    }
}

impl<R: Real, const D: usize> GradientCall<'_, R, D> {
    /// `BSplineSyNImageRegistrationMethod::ComputeUpdateField` for a stage whose only metric is
    /// a point-set metric: the metric in tangent space with each fixed point's derivative kept
    /// at the point (`SetStoreDerivativeAsSparseFieldForLocalSupportTransforms(false)`), times
    /// the optimizer weights, and the B-spline fitted to the fixed points' virtual positions
    /// (`SetBSplineDomainFromImage` of the virtual domain); a zero field without fixed points
    /// or with 0 control points.
    pub(crate) fn bspline_point_update(
        &mut self,
        swapped: bool,
        control_points: &[u32; D],
        order: u32,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Field<R, D>), ItkException> {
        let (ft, mt) = if swapped {
            (self.moving_composite, self.fixed_composite)
        } else {
            (self.fixed_composite, self.moving_composite)
        };
        let call = TangentCall {
            swapped,
            fixed_transform: ft,
            moving_transform: mt,
            virtual_space: self.virtual_space,
            sparse: false,
        };
        let p = self.point_sets[0].as_mut().expect("a point-set metric");
        p.tangent_initialize(&call, warnings)?;
        let (value, mut derivative) = p.tangent_evaluate(warnings)?;
        if let Some(w) = self.settings.optimizer_weights
            && w.len() == D
            && !weights_are_identity(w)
        {
            for chunk in derivative.as_chunks_mut::<D>().0 {
                for d in 0..D {
                    chunk[d] = chunk[d] * w[d];
                }
            }
        }
        let points = p.tangent_fixed_points();
        let zero = zero_field(self.virtual_space);
        if points.is_empty() || control_points.contains(&0) {
            return Ok((value, zero));
        }
        let data: Vec<[R; D]> = derivative.as_chunks::<D>().0.to_vec();
        let field = crate::bspline_field::points_to_bspline(
            self.virtual_space,
            &points,
            &data,
            control_points,
            order,
            true,
        )?;
        Ok((value, field))
    }
}

/// `m_OptimizerWeightsAreIdentity`: every weight within 1e-4 of 1.
pub fn weights_are_identity<R: Real>(w: &[R]) -> bool {
    let tolerance = R::from_f64(1e-4);
    w.iter().all(|&x| (R::ONE - x).abs() <= tolerance)
}

/// Runs a SyN stage: the output transform and the state, or ITK's exception.
pub fn run_syn<R: Real, const D: usize>(
    stage: &SynStage<R, D>,
    observer: &mut dyn SynObserver<R, D>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    events: &mut PointSetEvents,
) -> Result<SynOutput<R, D>, ItkException> {
    // The point-set metrics live through the stage (ITK keeps one metric object).
    let mut point_sets: Vec<Option<PointSetMetric<R, D>>> = stage
        .metrics
        .iter()
        .map(|m| match m {
            StageMetric::PointSet(p) => Some(p.metric.clone()),
            StageMetric::Image(_) => None,
        })
        .collect();
    let result = run_syn_levels(stage, observer, floor, warnings, &mut point_sets);
    for p in point_sets.iter().flatten() {
        events.merge(&p.events);
    }
    result
}

fn run_syn_levels<R: Real, const D: usize>(
    stage: &SynStage<R, D>,
    observer: &mut dyn SynObserver<R, D>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    point_sets: &mut [Option<PointSetMetric<R, D>>],
) -> Result<SynOutput<R, D>, ItkException> {
    let Some(virtual_base) = stage.virtual_base() else {
        return Err(crash(
            "a stage of point-set metrics without a fixed mask has no virtual domain (ANTs reads its spacing from a null image)",
        ));
    };
    let multi = stage.metrics.len() > 1;
    let has_point_sets = point_sets.iter().any(Option::is_some);
    // A single point-set metric (`POINT_SET_METRIC`): B-spline SyN's own path.
    let point_set_only = !multi && has_point_sets;
    let mut identity_point_sets = false;
    let mut weights: Vec<R> = stage.metrics.iter().map(|m| m.weight()).collect();
    let mut current_seed = stage.seed;
    let invert_settings = InvertSettings::<R>::syn();
    let mut state: Option<(FieldXfm<R, D>, FieldXfm<R, D>)> = None;
    let mut stale_masks = false;
    // Each metric object's joint histogram images (Mattes), kept across the levels and calls
    // of the stage.
    let joint_pdfs: Vec<Arc<std::sync::atomic::AtomicBool>> =
        stage.metrics.iter().map(|_| Default::default()).collect();
    for (level, factors) in stage.shrink_factors.iter().enumerate() {
        let virtual_space = shrink_space(&virtual_base, factors)?;
        let sigma = stage.smoothing_sigmas[level];
        let mut level_metrics = Vec::with_capacity(stage.metrics.len());
        for sm in &stage.metrics {
            let StageMetric::Image(sm) = sm else {
                level_metrics.push(None);
                continue;
            };
            level_metrics.push(Some(LevelImageMetric {
                kind: sm.kind,
                fixed: smooth_for_level(
                    &sm.fixed,
                    sigma,
                    stage.sigmas_in_physical_units,
                    stage.n_threads,
                )?,
                moving: smooth_for_level(
                    &sm.moving,
                    sigma,
                    stage.sigmas_in_physical_units,
                    stage.n_threads,
                )?,
                fixed_mask: sm.fixed_mask.clone(),
                moving_mask: sm.moving_mask.clone(),
                use_gradient_filter: sm.use_gradient_filter,
                samples: None,
            }));
        }
        // SetMetricSamplePoints: one set per metric from the first metric's fixed mask, every
        // metric cast to an image metric.
        if stage.sampling != SamplingStrategy::None {
            let Some(first) = &level_metrics[0] else {
                return Err(ItkException::new(
                    crate::method::FILE,
                    if multi { 924 } else { 938 },
                    crate::method::CLASS,
                    "Invalid metric conversion.",
                ));
            };
            let first_mask = first.fixed_mask.clone();
            for lm in &mut level_metrics {
                let points = sample_points(
                    &virtual_space,
                    stage.sampling,
                    stage.sampling_percentage,
                    first_mask.as_ref(),
                    &mut current_seed,
                );
                match lm {
                    Some(lm) => lm.samples = Some(Arc::new(points)),
                    None => {
                        return Err(crash(
                            "a stage that samples its metrics (Regular or Random) with a point-set metric: ANTs gives the sample points to a null image metric and crashes",
                        ));
                    }
                }
            }
        }
        // SyN's InitializeRegistrationAtEachLevel.
        let (mut f2m, mut m2m) = match state.take() {
            None => match &stage.restored {
                None => (
                    FieldXfm::new(
                        zero_field(&virtual_space),
                        Some(zero_field(&virtual_space)),
                        stage.exact_floor,
                    ),
                    FieldXfm::new(
                        zero_field(&virtual_space),
                        Some(zero_field(&virtual_space)),
                        stage.exact_floor,
                    ),
                ),
                Some((f, m)) => {
                    if f.inverse.is_none() || m.inverse.is_none() {
                        return Err(ItkException::new(
                            "Modules/Registration/RegistrationMethodsv4/include/itkSyNImageRegistrationMethod.hxx",
                            114,
                            "SyNImageRegistrationMethod",
                            "Invalid state restoration.",
                        ));
                    }
                    (
                        pooled(stage.n_threads, || adapt(f, &virtual_space, floor))?,
                        pooled(stage.n_threads, || adapt(m, &virtual_space, floor))?,
                    )
                }
            },
            Some((f, m)) => (
                pooled(stage.n_threads, || adapt(&f, &virtual_space, floor))?,
                pooled(stage.n_threads, || adapt(&m, &virtual_space, floor))?,
            ),
        };
        // The metric's masks as InitializeRegistrationAtEachLevel sets them (the stage's own).
        let mut mask_state: Vec<(Option<Mask<D>>, Option<Mask<D>>)> = level_metrics
            .iter()
            .map(|lm| match lm {
                Some(lm) => (lm.fixed_mask.clone(), lm.moving_mask.clone()),
                None => (None, None),
            })
            .collect();
        // StartOptimization.
        let mut convergence = WindowConvergence::new(stage.convergence_window);
        let n_iterations = stage.iterations.get(level).copied().unwrap_or(0) as usize;
        if n_iterations > 1
            && level_metrics
                .iter()
                .flatten()
                .any(|lm| lm.fixed_mask.is_some() != lm.moving_mask.is_some())
        {
            stale_masks = true;
        }
        if multi && has_point_sets && n_iterations > 0 {
            identity_point_sets = true;
        }
        // ITK-33 reproduced: the identity displacement field transform of the level.
        let identity =
            (multi && has_point_sets && !stage.correct_point_set_transforms).then(|| {
                Composite::new(vec![Xfm::Field(FieldXfm::new(
                    zero_field(&virtual_space),
                    Some(zero_field(&virtual_space)),
                    stage.exact_floor,
                ))])
            });
        let mut iteration = 0usize;
        let mut converged = false;
        while iteration < n_iterations && !converged {
            iteration += 1;
            // Each total field inverted from the previous inverse, and that inverse inverted
            // again from the total field.
            let finish = |fixed_total: Field<R, D>,
                          moving_total: Field<R, D>,
                          moving_value: R,
                          fixed_value: R| {
                let fixed_inverse = invert(
                    &fixed_total,
                    f2m.inverse.as_deref(),
                    &invert_settings,
                    floor,
                );
                let fixed_field =
                    invert(&fixed_inverse, Some(&fixed_total), &invert_settings, floor);
                let moving_inverse = invert(
                    &moving_total,
                    m2m.inverse.as_deref(),
                    &invert_settings,
                    floor,
                );
                let moving_field = invert(
                    &moving_inverse,
                    Some(&moving_total),
                    &invert_settings,
                    floor,
                );
                (
                    FieldXfm::new(fixed_field, Some(fixed_inverse), stage.exact_floor),
                    FieldXfm::new(moving_field, Some(moving_inverse), stage.exact_floor),
                    moving_value,
                    fixed_value,
                )
            };
            let step = || -> Result<_, ItkException> {
                let mut fixed_composite: Vec<Xfm<R, D>> = stage
                    .fixed_initial
                    .as_ref()
                    .map(|c| c.transforms.clone())
                    .unwrap_or_default();
                fixed_composite.push(Xfm::Field(
                    f2m.inverse_transform().expect("the fields have inverses"),
                ));
                let fixed_composite = Composite::new(fixed_composite);
                let mut moving_composite = stage.moving_initial.clone();
                moving_composite.push(Xfm::Field(
                    m2m.inverse_transform().expect("the fields have inverses"),
                ));
                let moving_composite = Composite::new(moving_composite);
                let mut resampled: Vec<Option<Resampled<R, D>>> = level_metrics
                    .iter()
                    .map(|lm| {
                        lm.as_ref().map(|lm| Resampled {
                            fixed: Arc::new(resample_image(
                                &lm.fixed,
                                &virtual_space,
                                &fixed_composite,
                                floor,
                            )),
                            moving: Arc::new(resample_image(
                                &lm.moving,
                                &virtual_space,
                                &moving_composite,
                                floor,
                            )),
                            fixed_mask: lm.fixed_mask.as_ref().map(|m| {
                                Mask::new(resample_mask_nearest(
                                    &m.image,
                                    &virtual_space,
                                    &fixed_composite,
                                ))
                            }),
                            moving_mask: lm.moving_mask.as_ref().map(|m| {
                                Mask::new(resample_mask_nearest(
                                    &m.image,
                                    &virtual_space,
                                    &moving_composite,
                                ))
                            }),
                            fixed_gradient: None,
                            moving_gradient: None,
                        })
                    })
                    .collect();
                let mut call = GradientCall {
                    metrics: &level_metrics,
                    resampled: &mut resampled,
                    point_sets: &mut *point_sets,
                    fixed_composite: &fixed_composite,
                    moving_composite: &moving_composite,
                    identity: identity.as_ref(),
                    virtual_space: &virtual_space,
                    weights: &mut weights,
                    multi,
                    settings: CallSettings::of(stage),
                    floor,
                    masks: &mut mask_state,
                    stale_masks: !stage.correct_stale_masks,
                    joint_pdfs: &joint_pdfs,
                    through: None,
                };
                // B-spline SyN with a single point-set metric: the B-spline fitted to the
                // points' derivatives.
                if let Some(b) = &stage.bspline
                    && point_set_only
                {
                    let cps = BSplineSynSmoothing::control_points(&b.update_mesh, level, b.order);
                    let (moving_value, update) =
                        call.bspline_point_update(false, &cps, b.order, warnings)?;
                    let fixed_update = scale_update(&update, stage.learning_rate);
                    let (fixed_value, update) =
                        call.bspline_point_update(true, &cps, b.order, warnings)?;
                    let moving_update = scale_update(&update, stage.learning_rate);
                    let tcp = BSplineSynSmoothing::control_points(&b.total_mesh, level, b.order);
                    let fixed_total = bspline_smooth(
                        &compose(&fixed_update, &f2m.field, floor),
                        &tcp,
                        b.order,
                        None,
                    )?;
                    let moving_total = bspline_smooth(
                        &compose(&moving_update, &m2m.field, floor),
                        &tcp,
                        b.order,
                        None,
                    )?;
                    return Ok(finish(fixed_total, moving_total, moving_value, fixed_value));
                }
                // The fixed-to-middle update (the metric value is the "moving" one), then the
                // moving-to-middle update.
                let (moving_value, fixed_gradient) = call.gradient_field(false, warnings)?;
                let smooth_update = |gradient: &Field<R, D>,
                                     mask: Option<&Mask<D>>,
                                     composite: &Composite<R, D>|
                 -> Result<Field<R, D>, ItkException> {
                    Ok(match &stage.bspline {
                        None => gaussian_smooth(gradient, stage.update_variance),
                        Some(b) => {
                            let weighted =
                                mask.map(|m| weighted_mask(m, &virtual_space, composite, floor));
                            bspline_smooth(
                                gradient,
                                &BSplineSynSmoothing::control_points(
                                    &b.update_mesh,
                                    level,
                                    b.order,
                                ),
                                b.order,
                                weighted.as_ref(),
                            )?
                        }
                    })
                };
                let smooth_total = |total: &Field<R, D>| -> Result<Field<R, D>, ItkException> {
                    Ok(match &stage.bspline {
                        None => gaussian_smooth(total, stage.total_variance),
                        Some(b) => bspline_smooth(
                            total,
                            &BSplineSynSmoothing::control_points(&b.total_mesh, level, b.order),
                            b.order,
                            None,
                        )?,
                    })
                };
                // The first metric's masks (BSplineSyN's weighted masks).
                let (first_fixed_mask, first_moving_mask) = match &level_metrics[0] {
                    Some(lm) => (lm.fixed_mask.as_ref(), lm.moving_mask.as_ref()),
                    None => (None, None),
                };
                let fixed_update = scale_update(
                    &smooth_update(&fixed_gradient, first_fixed_mask, &fixed_composite)?,
                    stage.learning_rate,
                );
                let (fixed_value, moving_gradient) = call.gradient_field(true, warnings)?;
                let moving_update = scale_update(
                    &smooth_update(&moving_gradient, first_moving_mask, &moving_composite)?,
                    stage.learning_rate,
                );
                let fixed_total = smooth_total(&compose(&fixed_update, &f2m.field, floor))?;
                let moving_total = smooth_total(&compose(&moving_update, &m2m.field, floor))?;
                Ok(finish(fixed_total, moving_total, moving_value, fixed_value))
            };
            // The iteration's work runs in a pool of the stage's thread count.
            let (new_f2m, new_m2m, moving_value, fixed_value) = pooled(stage.n_threads, step)??;
            f2m = new_f2m;
            m2m = new_m2m;
            let metric_value = R::from_f64(0.5 * (moving_value.to_f64() + fixed_value.to_f64()));
            convergence.add(metric_value.to_f64());
            let convergence_value = R::from_f64(convergence.value());
            if convergence_value < stage.convergence_threshold {
                converged = true;
            }
            observer.iteration(&SynIteration {
                level,
                iteration,
                metric_value,
                convergence_value,
                fixed_to_middle: &f2m,
                moving_to_middle: &m2m,
            })?;
        }
        state = Some((f2m, m2m));
    }
    let (f2m, m2m) = state.expect("at least one level");
    let output = pooled(stage.n_threads, || output_transform(&f2m, &m2m, floor))?;
    Ok(SynOutput {
        output,
        fixed_to_middle: f2m,
        moving_to_middle: m2m,
        stale_masks,
        identity_point_sets,
    })
}

/// `f` run in a pool of `n_threads` threads (0: every logical CPU).
pub(crate) fn pooled<T: Send>(
    n_threads: usize,
    f: impl FnOnce() -> T + Send,
) -> Result<T, ItkException> {
    larmorx_core::parallel::with_threads(n_threads, f).map_err(|e| {
        ItkException::new(
            "Modules/Core/Common/src/itkMultiThreaderBase.cxx",
            0,
            "MultiThreaderBase",
            e.to_string(),
        )
    })
}

/// The output transform of `GenerateData`: the moving-to-middle inverse composed with the
/// fixed-to-middle field (warping), and the fixed-to-middle inverse composed with the
/// moving-to-middle field for its inverse.
pub fn output_transform<R: Real, const D: usize>(
    f2m: &FieldXfm<R, D>,
    m2m: &FieldXfm<R, D>,
    floor: &IndexFloor,
) -> FieldXfm<R, D> {
    let m_inv = m2m.inverse.as_ref().expect("an inverse");
    let f_inv = f2m.inverse.as_ref().expect("an inverse");
    FieldXfm::new(
        compose(m_inv, &f2m.field, floor),
        Some(compose(f_inv, &m2m.field, floor)),
        f2m.exact_floor,
    )
}

/// The observer's full-scale similarity of a SyN stage (`UpdateFullScaleMetricValue` with
/// `DownsampleImagesForMetricDerivatives` on): the original fixed image resampled onto its own
/// grid through `fixed` (the fixed initial transform and the inverse of the fixed-to-middle
/// field), the original moving image onto the fixed grid through `moving` (the moving
/// initial transforms and the inverse of the moving-to-middle field), both linearly with
/// default 0; then `ANTSNeighborhoodCorrelationImageToImageMetricv4` of radius 4 between them
/// with identity transforms, every voxel of the fixed grid, its value. The "no valid points"
/// warning goes to `warnings`.
pub fn full_scale_cc<R: Real, const D: usize>(
    original_fixed: &Image<R, D>,
    original_moving: &Image<R, D>,
    fixed: &Composite<R, D>,
    moving: &Composite<R, D>,
    floor: &IndexFloor,
    n_threads: usize,
    warnings: &mut Vec<String>,
) -> Result<R, ItkException> {
    let grid = &original_fixed.space;
    let r = pooled(n_threads, || {
        let f = resample_image(original_fixed, grid, fixed, floor);
        let m = resample_image(original_moving, grid, moving, floor);
        let metric = ImageMetricv4 {
            fixed: Arc::new(f),
            moving: Arc::new(m),
            fixed_mask: None,
            moving_mask: None,
            fixed_transform: None,
            virtual_space: grid.clone(),
            samples: None,
            fixed_gradient_image: None,
            moving_gradient_image: None,
            pixel_f32: false,
            local_support: true,
            outside: Default::default(),
        };
        crate::metric::neighborhood_correlation::NeighborhoodCorrelation { radius: 4 }.evaluate(
            &metric,
            &Composite::new(Vec::new()),
            false,
            floor,
        )
    })??;
    if r.no_valid_points {
        warnings.push(no_valid_points_warning(
            VERIFY_FILE,
            544,
            "ANTSNeighborhoodCorrelationImageToImageMetricv4",
        ));
    }
    Ok(r.value)
}

/// The resampler of SyN's images, for the helper's other uses.
pub fn resample_linear_nonlinear<R: Real, const D: usize>(
    image: &Image<R, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    floor: &IndexFloor,
) -> Image<R, D> {
    resample_image(image, output, transform, floor)
}

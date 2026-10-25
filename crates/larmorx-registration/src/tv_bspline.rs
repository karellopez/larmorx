// SPDX-License-Identifier: Apache-2.0
//! ANTs' `TimeVaryingBSplineVelocityField` stage (ITK v5.4.5,
//! `Modules/Registration/RegistrationMethodsv4/include/itkTimeVaryingBSplineVelocityFieldImageRegistrationMethod.hxx`,
//! `Modules/Filtering/DisplacementField/include/itkTimeVaryingBSplineVelocityFieldTransform.hxx`,
//! `Modules/Registration/Common/include/itkTimeVaryingBSplineVelocityFieldTransformParametersAdaptor.hxx`),
//! as `antsRegistration` runs it (`itkantsRegistrationHelper.hxx`,
//! `case TimeVaryingBSplineVelocityField`).
//!
//! The velocity field is a B-spline in space and time: a lattice of control points in
//! `D + 1` dimensions ([`crate::bspline_field::Lattice`]), sampled on the level's virtual domain
//! at `numberOfTimePointSamples` times (`BSplineControlPointImageFilter`,
//! [`crate::bspline_field::reconstruct`]) and integrated like a time-varying velocity field
//! ([`crate::time_varying::integrate`]). Each iteration evaluates the metric's derivative at
//! every time sample (the images resampled through the flows from that time back to 0 and on
//! to 1, as for `TimeVaryingVelocityField`), scales each time sample's derivatives to a largest
//! norm of 1, averages them with the previous iteration's, fits one B-spline in space and time
//! to all of them (`BSplineScatteredDataPointSetToImageFilter`, one level, the domain's
//! boundary pinned with weight 10¹⁰, a fixed mask as the weights), and adds that lattice, times
//! the learning rate, to the velocity's.
//!
//! **Point-set metrics.** A single point-set metric gives its tangent-space derivative at the
//! fixed points' virtual positions, fitted with the domain's boundary points. Among several
//! metrics (an image metric first), each point-set metric's derivative is added at its nearest
//! virtual voxel, as in SyN.
//!
//! **ITK's defects, as decided** (D13, `upstream_defects`): ITK-36 (the multi-metric's
//! transforms are those the last metric set: point-set metrics before an image metric see
//! identity fields, image metrics after a point-set metric are warped twice), ITK-37 (at the
//! first time sample the fixed flow is zero but its inverse is the previous integration's; at
//! the last, the moving flow's inverse likewise).

use std::sync::Arc;

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::bspline_field::{FitDomain, Lattice, Scattered, fit, reconstruct};
use crate::convergence::WindowConvergence;
use crate::field::{Field, FieldSmoothing, FieldXfm, zero_field};
use crate::mask::Mask;
use crate::method::{
    CRASH_FILE, SamplingStrategy, StageMetric, sample_points, shrink_space, smooth_for_level,
};
use crate::pointset::metric::{PointSetEvents, PointSetMetric, TangentCall};
use crate::resample::{Interpolator, resample_linear, resample_nonlinear_with};
use crate::syn::{
    CallSettings, GradientCall, LevelImageMetric, Resampled, pooled, resample_image,
    weights_are_identity,
};
use crate::time_varying::{
    TvField, TvIteration, TvObserver, VelocityOutput, integrate, spatial_space,
};
use crate::transform::{Composite, Xfm};
use crate::{Image, ItkException, Matrix, Real, Space};

/// The weight of a boundary point (`m_BoundaryWeight`, 1e10, a float in the weights' container).
const BOUNDARY_WEIGHT: f32 = 1.0e10;

/// A `TimeVaryingBSplineVelocityField` stage.
#[derive(Clone, Debug)]
pub struct TvbStage<R, const D: usize> {
    pub metrics: Vec<StageMetric<R, D>>,
    pub shrink_factors: Vec<[usize; D]>,
    pub smoothing_sigmas: Vec<R>,
    pub sigmas_in_physical_units: bool,
    pub sampling: SamplingStrategy,
    pub sampling_percentage: R,
    pub seed: i32,
    pub fixed_initial: Option<Composite<R, D>>,
    pub moving_initial: Vec<Xfm<R, D>>,
    pub learning_rate: R,
    pub iterations: Vec<u32>,
    pub convergence_threshold: R,
    pub convergence_window: usize,
    /// The mesh at the base level, `D + 1` values (space, then time).
    pub mesh: Vec<u32>,
    /// `numberOfTimePointSamples`.
    pub time_samples: u32,
    pub order: u32,
    pub optimizer_weights: Option<Vec<R>>,
    /// The transform domain: the helper's virtual domain image (its origin, spacing, size and
    /// direction).
    pub domain: Space<D>,
    pub exact_floor: bool,
    pub correct_gc_derivative: bool,
    /// ITK-36 corrected: every metric of a multi-metric with its own transforms.
    pub correct_multi_metric_transforms: bool,
    /// ITK-37 corrected: the flows' inverses are zero where the flows are.
    pub correct_stale_inverses: bool,
    /// ITK-40 corrected: the metric's masks resampled with their images.
    pub correct_masks: bool,
    /// ITK-38 corrected: a single point-set metric's points (and the domain's boundary points)
    /// placed at their parametric positions (the virtual domain without its direction), as
    /// the image metrics' are; ITK places them at their physical positions.
    pub correct_point_positions: bool,
    pub n_threads: usize,
}

/// What a `TimeVaryingBSplineVelocityField` stage ends with.
#[derive(Clone, Debug)]
pub struct TvbOutput<R, const D: usize> {
    pub output: FieldXfm<R, D>,
    /// ITK-36's condition met (a multi-metric with a point-set metric evaluated).
    pub multi_metric_transforms: bool,
    /// ITK-37's condition met (a point-set metric evaluated at the first or last time sample).
    pub stale_inverses: bool,
    /// ITK-38's condition met (a single point-set metric on a virtual domain whose direction is
    /// not the identity).
    pub physical_points: bool,
    /// ITK-40's condition met (a mask evaluated after an iteration).
    pub masks_not_moved: bool,
}

fn crash(description: impl Into<String>) -> ItkException {
    ItkException::new(CRASH_FILE, 0, "", description)
}

const METHOD_FILE: &str = "Modules/Registration/RegistrationMethodsv4/include/itkTimeVaryingBSplineVelocityFieldImageRegistrationMethod.hxx";
const METHOD_CLASS: &str = "TimeVaryingBSplineVelocityFieldImageRegistrationMethod";

/// `TimeVaryingBSplineVelocityFieldTransformParametersAdaptor::UpdateRequiredFixedParameters`:
/// the lattice of a transform domain (origin, spacing and size of `E` axes, the mesh, the
/// direction) as the adaptor computes it with its spline order of 3: `mesh + 3` control points
/// per axis, spaced `(size − 1)·spacing / mesh`, the origin one control-point spacing before the
/// domain's (turned by the direction).
pub fn lattice_space<const E: usize>(
    origin: &[f64; E],
    spacing: &[f64; E],
    size: &[usize; E],
    mesh: &[u32; E],
    direction: &Matrix<f64, E>,
) -> Result<Space<E>, ItkException> {
    const ORDER: u32 = 3;
    let mut lattice_size = [0usize; E];
    let mut lattice_spacing = [0f64; E];
    let mut offset = [0f64; E];
    for i in 0..E {
        lattice_size[i] = (f64::from(mesh[i]) + f64::from(ORDER)) as usize;
        let physical = (size[i] as f64 - 1.0) * spacing[i];
        let grid = physical / f64::from(mesh[i]);
        lattice_spacing[i] = grid;
        offset[i] = -0.5 * grid * f64::from(ORDER - 1);
    }
    let mut lattice_origin = [0f64; E];
    for r in 0..E {
        let mut sum = 0.0;
        for c in 0..E {
            sum += direction[r][c] * offset[c];
        }
        lattice_origin[r] = sum + origin[r];
    }
    Space::new(
        lattice_size,
        [0; E],
        lattice_spacing,
        lattice_origin,
        *direction,
    )
}

/// `TimeVaryingBSplineVelocityFieldTransform` as the stage drives it.
#[derive(Clone, Debug)]
struct TvbTransform<R, const D: usize, const E: usize> {
    /// The control-point lattice (the transform's parameters).
    lattice: TvField<R, D, E>,
    order: u32,
    /// The sampled velocity field's geometry (`m_VelocityFieldOrigin`, ...).
    sampled: Space<E>,
    lower: R,
    upper: R,
    steps: u32,
    field: Field<R, D>,
    inverse: Option<Field<R, D>>,
}

impl<R: Real, const D: usize, const E: usize> TvbTransform<R, D, E> {
    /// The velocity field sampled from the lattice (`BSplineControlPointImageFilter`).
    fn sample(&self) -> Result<TvField<R, D, E>, ItkException> {
        let lattice = Lattice {
            size: self.lattice.space.size,
            data: self.lattice.data.clone(),
        };
        let mut v = reconstruct(&lattice, self.order, &self.sampled)?;
        // The output keeps the sampled field's region from index 0 and its geometry.
        v.space = Space::new(
            self.sampled.size,
            [0; E],
            self.sampled.spacing,
            self.sampled.origin,
            self.sampled.direction,
        )?;
        Ok(v)
    }

    /// `IntegrateVelocityField()`: the flow from the lower to the upper bound and back
    /// (`inverse` false skips the backward flow where nothing reads it).
    fn integrate(&mut self, floor: &IndexFloor, inverse: bool) -> Result<(), ItkException> {
        let v = self.sample()?;
        self.field = integrate(
            &v,
            self.lower.to_f64(),
            self.upper.to_f64(),
            self.steps,
            floor,
        );
        self.inverse = if inverse {
            Some(integrate(
                &v,
                self.upper.to_f64(),
                self.lower.to_f64(),
                self.steps,
                floor,
            ))
        } else {
            None
        };
        Ok(())
    }

    /// The adaptor of a level: the lattice upsampled onto `required` (each component's
    /// coefficients evaluated by `BSplineResampleImageFunction` (cubic) at the new control
    /// points, then `BSplineDecompositionImageFilter` of the transform's order), the transform
    /// domain as the sampled field, the time bounds [0, 1].
    fn adapt(
        &mut self,
        required: &Space<E>,
        domain: &Space<E>,
        floor: &IndexFloor,
    ) -> Result<(), ItkException> {
        let identity: Composite<R, E> = Composite::new(Vec::new());
        let n = required.len();
        let mut components: Vec<Vec<R>> = Vec::with_capacity(D);
        for j in 0..D {
            let image = Image::new(
                self.lattice.space.clone(),
                self.lattice.data.iter().map(|v| v[j]).collect(),
            );
            let up = resample_linear(
                &image,
                required,
                &identity,
                &Interpolator::Other(larmorx_interp::nd::InterpolationNd::BSplineCoefficients {
                    order: 3,
                }),
                R::ZERO,
                floor,
            )
            .map_err(|e| {
                ItkException::new(
                    "Modules/Registration/Common/include/itkTimeVaryingBSplineVelocityFieldTransformParametersAdaptor.hxx",
                    0,
                    "TimeVaryingBSplineVelocityFieldTransformParametersAdaptor",
                    e,
                )
            })?;
            components.push(crate::transform::bspline::decompose(
                &up,
                &required.size,
                self.order,
            ));
        }
        let data: Vec<[R; D]> = (0..n)
            .map(|o| std::array::from_fn(|j| components[j][o]))
            .collect();
        self.lattice = Image::new(required.clone(), data);
        self.sampled = domain.clone();
        self.lower = R::ZERO;
        self.upper = R::ONE;
        Ok(())
    }
}

/// The weighted mask of `AttachMetricGradientPointSetAtSpecificTimePoint`: the first metric's
/// fixed mask resampled onto the virtual domain through the fixed composite
/// (`ResampleImageFilter<Image<uchar>, Image<float>, R>`, linear, default 0), stored in float.
fn weighted_mask<R: Real, const D: usize>(
    mask: &Mask<D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    floor: &IndexFloor,
) -> Vec<f32> {
    let image = &mask.image;
    let target = Space::new(
        output.size,
        [0; D],
        output.spacing,
        output.origin,
        output.direction,
    )
    .expect("the virtual domain's geometry");
    resample_nonlinear_with(&image.space, &target, transform, 0.0f64, |c| {
        crate::interp::at_cindex_with(&image.space, c, floor, |o| f64::from(image.data[o]))
    })
    .into_iter()
    .map(|v| v as f32)
    .collect()
}

/// Runs a `TimeVaryingBSplineVelocityField` stage.
pub fn run_tvbsvf<R: Real, const D: usize>(
    stage: &TvbStage<R, D>,
    observer: &mut dyn TvObserver<R>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    events: &mut PointSetEvents,
) -> Result<TvbOutput<R, D>, ItkException> {
    match D {
        2 => run_e::<R, D, 3>(stage, observer, floor, warnings, events),
        3 => run_e::<R, D, 4>(stage, observer, floor, warnings, events),
        4 => run_e::<R, D, 5>(stage, observer, floor, warnings, events),
        _ => Err(crash(format!("{D} dimensions"))),
    }
}

fn run_e<R: Real, const D: usize, const E: usize>(
    stage: &TvbStage<R, D>,
    observer: &mut dyn TvObserver<R>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    events: &mut PointSetEvents,
) -> Result<TvbOutput<R, D>, ItkException> {
    assert_eq!(E, D + 1);
    let mut point_sets: Vec<Option<PointSetMetric<R, D>>> = stage
        .metrics
        .iter()
        .map(|m| match m {
            StageMetric::PointSet(p) => Some(p.metric.clone()),
            StageMetric::Image(_) => None,
        })
        .collect();
    let result = run_levels::<R, D, E>(stage, observer, floor, warnings, &mut point_sets);
    for p in point_sets.iter().flatten() {
        events.merge(&p.events);
    }
    result
}

/// The transform domain's `E`-dimensional geometry: the domain image in space, time from 0 with
/// spacing 1 and `n` samples, the direction `direction` (the domain's, or the identity).
fn domain_space<const D: usize, const E: usize>(
    domain: &Space<D>,
    n: usize,
    identity_direction: bool,
) -> Result<Space<E>, ItkException> {
    let mut s = crate::time_varying::space_time::<D, E>(domain, n, [0; D])?;
    if identity_direction {
        let mut direction = [[0f64; E]; E];
        for (i, row) in direction.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        s = Space::new(s.size, s.start, s.spacing, s.origin, direction)?;
    }
    Ok(s)
}

#[allow(clippy::too_many_lines)]
fn run_levels<R: Real, const D: usize, const E: usize>(
    stage: &TvbStage<R, D>,
    observer: &mut dyn TvObserver<R>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
    point_sets: &mut [Option<PointSetMetric<R, D>>],
) -> Result<TvbOutput<R, D>, ItkException> {
    let Some(virtual_base) = stage
        .metrics
        .first()
        .and_then(|m| m.virtual_domain().cloned())
    else {
        return Err(crash(
            "a stage of point-set metrics without a fixed mask has no virtual domain (ANTs reads its spacing from a null image)",
        ));
    };
    let multi = stage.metrics.len() > 1;
    let has_point_sets = point_sets.iter().any(Option::is_some);
    let point_set_only = !multi && has_point_sets;
    let samples = stage.time_samples as usize;
    let mesh0: [u32; E] = std::array::from_fn(|i| stage.mesh[i]);
    // The initial lattice (the helper's initial adaptor, with the domain's direction).
    let domain_full = crate::time_varying::space_time::<D, E>(&stage.domain, 2, [0; D])?;
    let initial = lattice_space::<E>(
        &domain_full.origin,
        &domain_full.spacing,
        &domain_full.size,
        &mesh0,
        &domain_full.direction,
    )?;
    let sampled0 = domain_space::<D, E>(&virtual_base, samples, false)?;
    let mut transform = TvbTransform::<R, D, E> {
        lattice: Image::new(initial.clone(), vec![[R::ZERO; D]; initial.len()]),
        order: stage.order,
        sampled: sampled0,
        lower: R::ZERO,
        upper: R::ONE,
        steps: 10,
        field: zero_field(&stage.domain),
        inverse: None,
    };
    let mut weights: Vec<R> = stage.metrics.iter().map(|m| m.weight()).collect();
    let mut current_seed = stage.seed;
    let settings = CallSettings {
        n_threads: stage.n_threads,
        correct_gc_derivative: stage.correct_gc_derivative,
        optimizer_weights: stage.optimizer_weights.as_deref(),
    };
    let mut multi_metric_transforms = false;
    let mut stale_inverses = false;
    let mut physical_points = false;
    let mut masks_not_moved = false;
    let need_inverses = has_point_sets;
    let mut mesh = mesh0;
    // Each metric object's joint histogram images (Mattes), kept across the levels and calls
    // of the stage.
    let joint_pdfs: Vec<Arc<std::sync::atomic::AtomicBool>> =
        stage.metrics.iter().map(|_| Default::default()).collect();
    for (level, factors) in stage.shrink_factors.iter().enumerate() {
        let virtual_space = shrink_space(&virtual_base, factors)?;
        // The level's adaptor: the transform domain (the helper's virtual domain, time [0, 1]
        // in 2 samples), the level's mesh, the identity direction (ANTs sets no direction on
        // these adaptors).
        let required_domain = domain_space::<D, E>(&stage.domain, 2, true)?;
        let required = lattice_space::<E>(
            &required_domain.origin,
            &required_domain.spacing,
            &required_domain.size,
            &mesh,
            &required_domain.direction,
        )?;
        {
            let t = &mut transform;
            pooled(stage.n_threads, || {
                t.adapt(&required, &required_domain, floor)
            })??;
        }
        for m in mesh.iter_mut() {
            *m = m.wrapping_shl(1);
        }
        // The level's images and sample points.
        let sigma = stage.smoothing_sigmas[level];
        let mut level_metrics: Vec<Option<LevelImageMetric<R, D>>> =
            Vec::with_capacity(stage.metrics.len());
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
        // StartOptimization: the first metric's fixed mask (a multi-metric's first metric must
        // be an image metric).
        if multi && level_metrics[0].is_none() {
            return Err(ItkException::new(
                METHOD_FILE,
                113,
                METHOD_CLASS,
                "ERROR: Invalid conversion from the multi metric queue.",
            ));
        }
        let fixed_mask = level_metrics[0].as_ref().and_then(|m| m.fixed_mask.clone());
        let steps = samples as u32 + 2;
        // The sampled velocity field on the level's virtual domain, then integrated.
        transform.sampled = domain_space::<D, E>(&virtual_space, samples, false)?;
        {
            let t = &mut transform;
            pooled(stage.n_threads, || t.integrate(floor, true))??;
        }
        let n_iterations = stage.iterations.get(level).copied().unwrap_or(0) as usize;
        let mut convergence = WindowConvergence::new(stage.convergence_window);
        let mut previous: Option<Vec<[R; D]>> = None;
        let mut masks: Vec<(Option<Mask<D>>, Option<Mask<D>>)> = level_metrics
            .iter()
            .map(|lm| match lm {
                Some(lm) => (lm.fixed_mask.clone(), lm.moving_mask.clone()),
                None => (None, None),
            })
            .collect();
        let lattice_size = transform.lattice.space.size;
        let cps: [u32; E] = std::array::from_fn(|i| lattice_size[i] as u32);
        let mut fit_origin = [0f64; E];
        let mut fit_spacing = [0f64; E];
        let mut fit_size = [0usize; E];
        for d in 0..D {
            fit_origin[d] = virtual_space.origin[d];
            fit_spacing[d] = virtual_space.spacing[d];
            fit_size[d] = virtual_space.size[d];
        }
        fit_origin[D] = 0.0;
        fit_spacing[D] = 0.1;
        fit_size[D] = 11;
        let fit_domain = FitDomain {
            origin: fit_origin,
            spacing: fit_spacing,
            size: fit_size,
        };
        // The parametric domain of the image path: the virtual domain without its direction.
        let parametric = {
            let mut identity = [[0f64; D]; D];
            for (i, row) in identity.iter_mut().enumerate() {
                row[i] = 1.0;
            }
            Space::new(
                virtual_space.size,
                virtual_space.start,
                virtual_space.spacing,
                virtual_space.origin,
                identity,
            )?
        };
        let mut iteration = 0usize;
        let mut converged = false;
        while iteration < n_iterations && !converged {
            iteration += 1;
            // GetMetricDerivativePointSetForAllTimePoints.
            let mut scattered = Scattered::<R, E, D>::default();
            let mut value_sum = R::ZERO;
            for tp in 0..samples {
                let t = if samples > 1 {
                    R::from_i64(tp as i64) / R::from_i64(samples as i64 - 1)
                } else {
                    R::ZERO
                };
                // The fixed flow, from t back to 0 (zero at the first sample: the field is
                // filled with zeros, its inverse is the previous integration's, ITK-37).
                if tp == 0 {
                    transform.field = zero_field(&transform.field.space);
                    if transform.inverse.is_some() && need_inverses {
                        stale_inverses = true;
                    }
                    if stage.correct_stale_inverses {
                        transform.inverse = Some(zero_field(&transform.field.space));
                    }
                } else {
                    transform.lower = t;
                    transform.upper = R::ZERO;
                    transform.steps = steps;
                    let tr = &mut transform;
                    pooled(stage.n_threads, || tr.integrate(floor, need_inverses))??;
                }
                let fixed_field = FieldXfm::new(
                    transform.field.clone(),
                    transform.inverse.clone(),
                    stage.exact_floor,
                );
                let mut fixed: Vec<Xfm<R, D>> = stage
                    .fixed_initial
                    .as_ref()
                    .map(|c| c.transforms.clone())
                    .unwrap_or_default();
                fixed.push(Xfm::Field(fixed_field));
                let fixed_composite = Composite::new(fixed);
                // The moving flow, from t to 1 (zero at the last sample, its inverse ITK-37's).
                if tp == samples - 1 {
                    transform.field = zero_field(&transform.field.space);
                    if transform.inverse.is_some() && need_inverses {
                        stale_inverses = true;
                    }
                    if stage.correct_stale_inverses {
                        transform.inverse = Some(zero_field(&transform.field.space));
                    }
                } else {
                    transform.lower = t;
                    transform.upper = R::ONE;
                    transform.steps = steps;
                    let tr = &mut transform;
                    pooled(stage.n_threads, || tr.integrate(floor, need_inverses))??;
                }
                let moving_field = FieldXfm::new(
                    transform.field.clone(),
                    transform.inverse.clone(),
                    stage.exact_floor,
                );
                let mut moving = stage.moving_initial.clone();
                moving.push(Xfm::Field(moving_field));
                let moving_composite = Composite::new(moving);
                let first_new = scattered.points.len();
                let time = t.to_f64() as f32;
                if point_set_only {
                    // The point-set metric alone: each fixed point's derivative at its virtual
                    // position, then the boundary of the fixed flow's domain, pinned.
                    let p = point_sets[0].as_mut().expect("a point-set metric");
                    let call = TangentCall {
                        swapped: false,
                        fixed_transform: &fixed_composite,
                        moving_transform: &moving_composite,
                        virtual_space: &virtual_space,
                        sparse: false,
                    };
                    let (value, mut derivative) =
                        pooled(stage.n_threads, || -> Result<_, ItkException> {
                            p.tangent_initialize(&call, warnings)?;
                            p.tangent_evaluate(warnings)
                        })??;
                    value_sum = value_sum + value;
                    if let Some(w) = settings.optimizer_weights
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
                    let oblique = virtual_space.direction != parametric.direction;
                    if oblique {
                        physical_points = true;
                    }
                    let correct = stage.correct_point_positions && oblique;
                    for (k, q) in points.iter().enumerate() {
                        let mut st = [0f32; E];
                        if correct {
                            let qd: [f64; D] = std::array::from_fn(|d| f64::from(q[d]));
                            let c: [f64; D] = virtual_space.point_to_cindex(&qd);
                            let r = parametric.cindex_to_point(&c);
                            for d in 0..D {
                                st[d] = r[d] as f32;
                            }
                        } else {
                            st[..D].copy_from_slice(q);
                        }
                        st[D] = time;
                        scattered.points.push(st);
                        scattered
                            .data
                            .push(std::array::from_fn(|d| derivative[k * D + d]));
                        scattered.weights.push(1.0);
                    }
                    let fs = &transform.field.space;
                    for o in 0..fs.len() {
                        let idx = fs.index_of(o);
                        let boundary = (0..D).any(|d| {
                            idx[d] == fs.start[d] || idx[d] == fs.start[d] + fs.size[d] as i64 - 1
                        });
                        if boundary {
                            let q = if correct {
                                let c: [f64; D] = std::array::from_fn(|d| idx[d] as f64);
                                parametric.cindex_to_point(&c)
                            } else {
                                fs.index_to_point(&idx)
                            };
                            let mut st = [0f32; E];
                            for d in 0..D {
                                st[d] = q[d] as f32;
                            }
                            st[D] = time;
                            scattered.points.push(st);
                            scattered.data.push([R::ZERO; D]);
                            scattered.weights.push(BOUNDARY_WEIGHT);
                        }
                    }
                } else {
                    // AttachMetricGradientPointSetAtSpecificTimePoint.
                    // ITK-40's notice: whether moving the masks changes them.
                    if !masks_not_moved {
                        masks_not_moved = level_metrics.iter().flatten().any(|lm| {
                            crate::time_varying::masks_move(
                                &lm.fixed_mask,
                                &lm.moving_mask,
                                &virtual_space,
                                &fixed_composite,
                                &moving_composite,
                            )
                        });
                    }
                    let mut resampled: Vec<Option<Resampled<R, D>>> =
                        Vec::with_capacity(level_metrics.len());
                    for lm in &level_metrics {
                        resampled.push(lm.as_ref().map(|lm| {
                            let (fixed_mask, moving_mask) = crate::time_varying::evaluation_masks(
                                &lm.fixed_mask,
                                &lm.moving_mask,
                                stage.correct_masks,
                                &virtual_space,
                                &fixed_composite,
                                &moving_composite,
                            );
                            Resampled {
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
                                fixed_mask,
                                moving_mask,
                                fixed_gradient: None,
                                moving_gradient: None,
                            }
                        }));
                    }
                    // ITK-36: the transforms the last metric set apply to every metric.
                    let last_is_point_set =
                        matches!(stage.metrics.last(), Some(StageMetric::PointSet(_)));
                    let identity_field = Composite::new(vec![Xfm::Field(FieldXfm::new(
                        zero_field(&virtual_space),
                        Some(zero_field(&virtual_space)),
                        stage.exact_floor,
                    ))]);
                    let reproduce_itk_36 =
                        multi && has_point_sets && !stage.correct_multi_metric_transforms;
                    if multi && has_point_sets {
                        multi_metric_transforms = true;
                    }
                    let identity =
                        (reproduce_itk_36 && !last_is_point_set).then_some(&identity_field);
                    let images_through = (reproduce_itk_36 && last_is_point_set)
                        .then_some((&fixed_composite, &moving_composite));
                    let mut call = GradientCall {
                        metrics: &level_metrics,
                        resampled: &mut resampled,
                        point_sets: &mut *point_sets,
                        fixed_composite: &fixed_composite,
                        moving_composite: &moving_composite,
                        identity,
                        virtual_space: &virtual_space,
                        weights: &mut weights,
                        multi,
                        settings,
                        floor,
                        masks: &mut masks,
                        stale_masks: false,
                        joint_pdfs: &joint_pdfs,
                        through: images_through,
                        outside: &crate::metric::OutsideDerivatives::default(),
                    };
                    let (value, derivative) = call.gradient_field(false, warnings)?;
                    value_sum = value_sum + value;
                    let mask_weights = fixed_mask
                        .as_ref()
                        .map(|m| weighted_mask(m, &virtual_space, &fixed_composite, floor));
                    let vs = &virtual_space;
                    for o in 0..vs.len() {
                        let idx = vs.index_of(o);
                        let boundary = (0..D).any(|d| {
                            idx[d] == vs.start[d] || idx[d] == vs.start[d] + vs.size[d] as i64 - 1
                        });
                        let weight: f32 = if boundary {
                            BOUNDARY_WEIGHT
                        } else if let Some(w) = &mask_weights {
                            R::from_f64(f64::from(w[o])).to_f64() as f32
                        } else {
                            1.0
                        };
                        let c: [f64; D] = std::array::from_fn(|d| idx[d] as f64);
                        let q = parametric.cindex_to_point(&c);
                        let mut st = [0f32; E];
                        for d in 0..D {
                            st[d] = q[d] as f32;
                        }
                        st[D] = time;
                        scattered.points.push(st);
                        scattered.data.push(derivative.data[o]);
                        scattered.weights.push(weight);
                    }
                }
                // This time sample's vectors scaled to a largest norm of 1.
                let mut max_squared = R::ZERO;
                for v in &scattered.data[first_new..] {
                    let mut s = 0.0f64;
                    for c in v {
                        let x = c.to_f64();
                        s += x * x;
                    }
                    let s = R::from_f64(s);
                    if s > max_squared {
                        max_squared = s;
                    }
                }
                if max_squared > R::ZERO {
                    let factor = R::from_f64(1.0 / max_squared.sqrt().to_f64());
                    for v in &mut scattered.data[first_new..] {
                        for c in v.iter_mut() {
                            *c = factor * *c;
                        }
                    }
                }
            }
            // Averaging with the previous iteration's vectors (the previous ones as they were
            // before their own averaging).
            match &mut previous {
                Some(prev) => {
                    if prev.len() != scattered.data.len() {
                        return Err(ItkException::new(
                            METHOD_FILE,
                            177,
                            METHOD_CLASS,
                            "The number of points is not the same between iterations.",
                        ));
                    }
                    let half = 0.5f64;
                    for (v, p) in scattered.data.iter_mut().zip(prev.iter_mut()) {
                        let raw = *v;
                        *v = std::array::from_fn(|d| R::from_f64((v[d] + p[d]).to_f64() * half));
                        *p = raw;
                    }
                }
                None => previous = Some(scattered.data.clone()),
            }
            // The B-spline update lattice in space and time.
            let update = pooled(stage.n_threads, || {
                fit(&fit_domain, &cps, stage.order, &scattered)
            })??;
            // TimeVaryingBSplineVelocityFieldTransform::UpdateTransformParameters: the lattice
            // plus the update times the learning rate, then integrated.
            let lr = stage.learning_rate;
            for (v, u) in transform.lattice.data.iter_mut().zip(&update.data) {
                for d in 0..D {
                    v[d] = v[d] + u[d] * lr;
                }
            }
            if need_inverses {
                // The point-set metrics read the next iteration's stale inverse (ITK-37) from
                // this integration.
                let tr = &mut transform;
                pooled(stage.n_threads, || tr.integrate(floor, true))??;
            }
            let metric_value = value_sum / R::from_i64(samples as i64);
            convergence.add(metric_value.to_f64());
            let convergence_value = R::from_f64(convergence.value());
            if convergence_value < stage.convergence_threshold {
                converged = true;
            }
            if converged || iteration >= n_iterations {
                transform.lower = R::ZERO;
                transform.upper = R::ONE;
                transform.steps = steps;
                let tr = &mut transform;
                pooled(stage.n_threads, || tr.integrate(floor, true))??;
            }
            observer.iteration(&TvIteration {
                level,
                iteration,
                metric_value,
                convergence_value,
            })?;
        }
    }
    let lattice_output = VelocityOutput::of(true, &transform.lattice);
    let inverse = transform.inverse.clone();
    let mut out = FieldXfm::new(transform.field, inverse, stage.exact_floor);
    out.smoothing = Some(FieldSmoothing::TimeVarying(Arc::new(lattice_output)));
    let _ = spatial_space::<D, E>;
    let _ = (0..1).into_par_iter();
    Ok(TvbOutput {
        output: out,
        multi_metric_transforms,
        stale_inverses,
        physical_points,
        masks_not_moved,
    })
}

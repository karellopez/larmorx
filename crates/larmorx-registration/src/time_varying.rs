// SPDX-License-Identifier: Apache-2.0
//! Time-varying velocity fields and ANTs' `TimeVaryingVelocityField` stage (ITK v5.4.5,
//! `Modules/Filtering/DisplacementField`: `itkTimeVaryingVelocityFieldIntegrationImageFilter.hxx`,
//! `itkVelocityFieldTransform.hxx`, `itkTimeVaryingVelocityFieldTransform.hxx`,
//! `itkGaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform.hxx`;
//! `Modules/Registration/Common`: `itkTimeVaryingVelocityFieldTransformParametersAdaptor.hxx`;
//! `Modules/Registration/RegistrationMethodsv4`:
//! `itkTimeVaryingVelocityFieldImageRegistrationMethodv4.hxx`), as ANTs v2.6.5's
//! `antsRegistration` runs it (`itkantsRegistrationHelper.hxx`, `case TimeVaryingVelocityField`).
//!
//! A time-varying velocity field is an image of `D`-component vectors in `D + 1` dimensions
//! (space, then time): [`TvField`], an [`Image`] of `[R; D]` on a `Space<E>` with `E = D + 1`.
//! Its displacement field is the flow of the velocity from one time bound to the other,
//! integrated per voxel by fourth-order Runge–Kutta ([`integrate`]) with the velocity linearly
//! interpolated in space and time.
//!
//! **The stage** ([`run_tvvf`]): per level the velocity field is resampled onto the level's
//! domain (keeping its number of time points); each iteration, for every time point `t` the
//! fixed images are resampled through the flow from `t` back to 0 and the moving images through
//! the moving initial transforms and the flow from `t` to 1, the metric's derivative over the
//! virtual domain (identity transforms, one vector per voxel) is scaled to a largest norm of 1
//! and stored at that time point; the update, averaged with the previous iteration's, is
//! smoothed (Gaussian in space and time), added to the velocity field times the learning rate,
//! and the total smoothed; the level ends with the flow from 0 to 1 and its inverse.
//!
//! Every voxel's integration is computed on its own and every sum ITK accumulates over voxels
//! in a single work unit's order, so the result does not depend on the thread count.

use std::sync::Arc;

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::convergence::WindowConvergence;
use crate::field::{Field, FieldSmoothing, FieldXfm, convolve_axis, gaussian_operator, zero_field};
use crate::mask::Mask;
use crate::method::{
    CRASH_FILE, SamplingStrategy, StageMetric, sample_points, shrink_space, smooth_for_level,
};
use crate::pointset::metric::PointSetMetric;
use crate::syn::{CallSettings, GradientCall, LevelImageMetric, Resampled, pooled, resample_image};
use crate::transform::{Composite, Xfm};
use crate::{Image, ItkException, Matrix, Real, Space};

/// A time-varying velocity field: `D`-component vectors on a `D + 1`-dimensional grid (`E`).
pub type TvField<R, const D: usize, const E: usize> = Image<[R; D], E>;

/// What a time-varying velocity field transform ends its stage with, kept for the outputs
/// (`VelocityField.nii.gz` with `-z 0`) and the transform's class: the velocity field (for a
/// B-spline velocity field, its control-point lattice) in `D + 1` dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct VelocityOutput<R> {
    /// `TimeVaryingBSplineVelocityFieldTransform` (else
    /// `GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform`).
    pub bspline: bool,
    pub size: Vec<usize>,
    pub spacing: Vec<f64>,
    pub origin: Vec<f64>,
    /// Row by row.
    pub direction: Vec<f64>,
    /// The vectors, `size.len() − 1` components each, interleaved, voxel `x` fastest.
    pub data: Vec<R>,
    /// Whether the transform ever integrated its velocity field: ITK sets the displacement
    /// field's interpolator only there, so a transform that never did (no iteration, no
    /// resampling) crashes when it maps a point.
    pub integrated: bool,
}

impl<R: Real> VelocityOutput<R> {
    /// `GetNameOfClass()`.
    pub fn class(&self) -> &'static str {
        if self.bspline {
            "TimeVaryingBSplineVelocityFieldTransform"
        } else {
            "GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform"
        }
    }

    /// `GetNumberOfParameters()`: the velocity field's (or lattice's) components.
    pub fn number_of_parameters(&self) -> usize {
        self.data.len()
    }

    /// The output of a velocity field.
    pub fn of<const D: usize, const E: usize>(bspline: bool, v: &TvField<R, D, E>) -> Self {
        let s = &v.space;
        VelocityOutput {
            bspline,
            size: s.size.to_vec(),
            spacing: s.spacing.to_vec(),
            origin: s.origin.to_vec(),
            direction: s.direction.iter().flatten().copied().collect(),
            data: v.data.iter().flatten().copied().collect(),
            integrated: true,
        }
    }
}

/// The spatial part of a velocity field's geometry: the first `D` axes (size, spacing, origin,
/// direction), from index 0 (`TimeVaryingVelocityFieldIntegrationImageFilter`'s output).
pub fn spatial_space<const D: usize, const E: usize>(s: &Space<E>) -> Space<D> {
    let direction: Matrix<f64, D> =
        std::array::from_fn(|i| std::array::from_fn(|j| s.direction[i][j]));
    Space::new(
        std::array::from_fn(|k| s.size[k]),
        [0; D],
        std::array::from_fn(|k| s.spacing[k]),
        std::array::from_fn(|k| s.origin[k]),
        direction,
    )
    .expect("a velocity field's spatial geometry")
}

/// The space-time geometry of a spatial grid with `n` time points (origin 0, spacing 1 along
/// time, the direction extended by 1), from index `start` in space and 0 in time.
pub fn space_time<const D: usize, const E: usize>(
    s: &Space<D>,
    n: usize,
    start: [i64; D],
) -> Result<Space<E>, ItkException> {
    let mut size = [n; E];
    let mut index = [0i64; E];
    let mut spacing = [1.0f64; E];
    let mut origin = [0.0f64; E];
    let mut direction = [[0.0f64; E]; E];
    for (i, row) in direction.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for i in 0..D {
        size[i] = s.size[i];
        index[i] = start[i];
        spacing[i] = s.spacing[i];
        origin[i] = s.origin[i];
        for j in 0..D {
            direction[i][j] = s.direction[i][j];
        }
    }
    Space::new(size, index, spacing, origin, direction)
}

/// `TimeVaryingVelocityFieldIntegrationImageFilter` with the time bounds `lower` and `upper`
/// (the transform's, in `R`, as doubles), `steps` integration steps, `TimeBoundsAsRates` on
/// and no initial diffeomorphism: for each voxel of the velocity field's spatial grid
/// ([`spatial_space`]), fourth-order Runge–Kutta from its physical point. A zero field when the
/// bounds are equal or `steps` is 0.
pub fn integrate<R: Real, const D: usize, const E: usize>(
    velocity: &TvField<R, D, E>,
    lower: f64,
    upper: f64,
    steps: u32,
    floor: &IndexFloor,
) -> Field<R, D> {
    debug_assert_eq!(E, D + 1);
    let out = spatial_space::<D, E>(&velocity.space);
    if lower == upper || steps == 0 {
        return zero_field(&out);
    }
    let vs = &velocity.space;
    // The time axis mapped onto [0, 1]: its origin and the time of the last index.
    let time_origin = vs.origin[D];
    let mut last = [0i64; E];
    for d in 0..E {
        last[d] = vs.start[d] + vs.size[d] as i64 - 1;
    }
    let time_end = vs.index_to_point(&last)[D];
    let time_scale = time_end - time_origin;
    let delta_time = (upper - lower) / f64::from(steps);
    let delta_time_in_image = time_scale * delta_time;
    let first_time = time_origin + lower * time_scale;
    let evaluate = |x: &[f64; E]| -> [R; D] {
        // The interpolator's `PointType` is `Point<R>`: the space-time point is converted
        // before its continuous index is computed.
        let xr: [f64; E] = std::array::from_fn(|k| R::from_f64(x[k]).to_f64());
        let c: [R; E] = vs.point_to_cindex(&xr);
        if !vs.is_inside_buffer(&c) {
            return [R::ZERO; D];
        }
        let v = crate::field::vector_linear_nd(velocity, &c, floor);
        std::array::from_fn(|k| R::from_f64(v[k]))
    };
    let data: Vec<[R; D]> = (0..out.len())
        .into_par_iter()
        .with_min_len(64)
        .map(|o| {
            let p = out.index_to_point(&out.index_of(o));
            let mut displacement = [R::ZERO; D];
            let mut time = first_time;
            for _ in 0..steps {
                let mut x1 = [0.0f64; E];
                for d in 0..D {
                    x1[d] = p[d] + displacement[d].to_f64();
                }
                let mut x2 = x1;
                let mut x3 = x1;
                let mut x4 = x1;
                x1[D] = time;
                x2[D] = time + 0.5 * delta_time_in_image;
                x3[D] = time + 0.5 * delta_time_in_image;
                x4[D] = time + delta_time_in_image;
                let f1 = evaluate(&x1);
                for j in 0..D {
                    x2[j] += f1[j].to_f64() * delta_time * 0.5;
                }
                let f2 = evaluate(&x2);
                for j in 0..D {
                    x3[j] += f2[j].to_f64() * delta_time * 0.5;
                }
                let f3 = evaluate(&x3);
                for j in 0..D {
                    x4[j] += f3[j].to_f64() * delta_time;
                }
                let f4 = evaluate(&x4);
                for j in 0..D {
                    x1[j] += delta_time / 6.0
                        * (f1[j].to_f64()
                            + 2.0 * f2[j].to_f64()
                            + 2.0 * f3[j].to_f64()
                            + f4[j].to_f64());
                    displacement[j] = R::from_f64(x1[j] - p[j]);
                }
                time += delta_time_in_image;
            }
            displacement
        })
        .collect();
    Image::new(out, data)
}

/// `GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform::GaussianSmoothTimeVaryingVelocityField`:
/// the field itself when both variances are ≤ 0; else smoothed along each axis whose variance
/// is positive (the spatial variance for space, the temporal one for time; `GaussianOperator`
/// with maximum error 0.001 and the axis's size as maximum width, zero-flux boundary), then
/// every voxel on a spatial boundary set to 0 and the others blended `s·w₁ + f·w₂` with
/// `w₁ = 1 − spatial/0.5` below a spatial variance of 0.5 (1 above), `w₂ = 1 − w₁`.
pub fn gaussian_smooth<R: Real, const D: usize, const E: usize>(
    field: &TvField<R, D, E>,
    spatial: R,
    temporal: R,
) -> TvField<R, D, E> {
    if spatial <= R::ZERO && temporal <= R::ZERO {
        return field.clone();
    }
    let mut smooth = field.clone();
    for axis in 0..E {
        let variance = if axis < D { spatial } else { temporal };
        if variance > R::ZERO {
            let kernel = gaussian_operator(variance, 0.001, field.space.size[axis]);
            smooth = convolve_axis(&smooth, axis, &kernel);
        }
    }
    let mut weight1 = R::ONE;
    if spatial < R::from_f64(0.5) {
        weight1 = R::from_f64(1.0 - 1.0 * (spatial.to_f64() / 0.5));
    }
    let weight2 = R::ONE - weight1;
    let space = field.space.clone();
    smooth.data.par_iter_mut().enumerate().for_each(|(o, s)| {
        let idx = space.index_of(o);
        let boundary = (0..D).any(|d| {
            idx[d] == space.start[d] || idx[d] == space.size[d] as i64 - space.start[d] - 1
        });
        if boundary {
            *s = [R::ZERO; D];
        } else {
            let f = field.data[o];
            *s = std::array::from_fn(|d| s[d] * weight1 + f[d] * weight2);
        }
    });
    smooth
}

/// `GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform` as the stage drives it.
#[derive(Clone, Debug)]
struct TvTransform<R, const D: usize, const E: usize> {
    velocity: TvField<R, D, E>,
    /// `m_LowerTimeBound`, `m_UpperTimeBound` (`ScalarType`).
    lower: R,
    upper: R,
    /// `m_NumberOfIntegrationSteps` (10 until the method sets it).
    steps: u32,
    field: Field<R, D>,
    inverse: Option<Field<R, D>>,
    update_spatial: R,
    update_temporal: R,
    total_spatial: R,
    total_temporal: R,
}

impl<R: Real, const D: usize, const E: usize> TvTransform<R, D, E> {
    /// `IntegrateVelocityField()`: the flow from the lower to the upper bound and back;
    /// `inverse` false skips the backward flow where nothing reads it.
    fn integrate(&mut self, floor: &IndexFloor, inverse: bool) {
        self.field = integrate(
            &self.velocity,
            self.lower.to_f64(),
            self.upper.to_f64(),
            self.steps,
            floor,
        );
        self.inverse = inverse.then(|| {
            integrate(
                &self.velocity,
                self.upper.to_f64(),
                self.lower.to_f64(),
                self.steps,
                floor,
            )
        });
    }

    /// `UpdateTransformParameters(update, factor)`: the update smoothed (both variances of
    /// the update field ≤ 0 skip it), added to the velocity field (times `factor` unless it is
    /// 1), the total smoothed alike.
    fn update(&mut self, update: &[R], factor: R) -> Result<(), ItkException> {
        let n = D * self.velocity.data.len();
        if update.len() != n {
            return Err(ItkException::new(
                "Modules/Core/Transform/include/itkTransform.hxx",
                76,
                "GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform",
                format!(
                    "Parameter update size, {}, must  be same as transform parameter size, {n}\n",
                    update.len()
                ),
            ));
        }
        let update_field = Image::new(
            self.velocity.space.clone(),
            update.as_chunks::<D>().0.to_vec(),
        );
        let update_field =
            gaussian_smooth(&update_field, self.update_spatial, self.update_temporal);
        for (v, u) in self.velocity.data.iter_mut().zip(&update_field.data) {
            for d in 0..D {
                v[d] = if factor == R::ONE {
                    v[d] + u[d]
                } else {
                    v[d] + u[d] * factor
                };
            }
        }
        self.velocity = gaussian_smooth(&self.velocity, self.total_spatial, self.total_temporal);
        // ITK integrates the velocity field here (twice: VelocityFieldTransform's update, then
        // this class's), with the bounds of the method's last integration; the method reads
        // the fields only after integrating again at the end of the level, so the integration
        // is left to it.
        Ok(())
    }
}

/// A `TimeVaryingVelocityField` stage as `TimeVaryingVelocityFieldImageRegistrationMethodv4`
/// runs it under ANTs.
#[derive(Clone, Debug)]
pub struct TvvfStage<R, const D: usize> {
    pub metrics: Vec<StageMetric<R, D>>,
    pub shrink_factors: Vec<[usize; D]>,
    pub smoothing_sigmas: Vec<R>,
    pub sigmas_in_physical_units: bool,
    pub sampling: SamplingStrategy,
    pub sampling_percentage: R,
    /// The first seed of the metric sampling's generators (ANTs never reinitialises it for
    /// this stage: its generators are seeded from the clock; larmorx uses the registration's
    /// seed).
    pub seed: i32,
    /// The moving initial transforms (the registration's composite, flattened).
    pub moving_initial: Vec<Xfm<R, D>>,
    /// The fixed initial transforms (`-q`), which ITK's method does not use (ITK-39).
    pub fixed_initial: Option<Composite<R, D>>,
    pub learning_rate: R,
    pub iterations: Vec<u32>,
    pub convergence_threshold: R,
    pub convergence_window: usize,
    /// ITK-39 corrected: the fixed images resampled through the fixed initial transforms too.
    pub correct_fixed_initial: bool,
    /// ITK-40 corrected: the metric's masks resampled with their images (nearest neighbour), as
    /// SyN resamples them.
    pub correct_masks: bool,
    /// `numberOfTimeIndices`: the velocity field's time points.
    pub time_indices: u32,
    pub update_variance: R,
    pub update_time_variance: R,
    pub total_variance: R,
    pub total_time_variance: R,
    /// `SetOptimizerWeights` (`-g` with `D` values).
    pub optimizer_weights: Option<Vec<R>>,
    /// The grid the velocity and displacement fields start on: the stage's first preprocessed
    /// fixed image (its region's start index and geometry).
    pub field_space: Space<D>,
    pub exact_floor: bool,
    /// ITK-30 corrected.
    pub correct_gc_derivative: bool,
    pub n_threads: usize,
}

/// What a time-varying velocity field stage ends with.
#[derive(Clone, Debug)]
pub struct TvvfOutput<R, const D: usize> {
    pub output: FieldXfm<R, D>,
    /// ITK-39's condition met: a fixed initial transform.
    pub fixed_initial_ignored: bool,
    /// ITK-40's condition met: a mask evaluated after an iteration.
    pub masks_not_moved: bool,
}

/// The masks of one image metric as the metric holds them in an evaluation: the stage's
/// masks as they are (ITK-40 reproduced), or resampled onto the virtual domain through the
/// fixed and moving composites with the nearest neighbour (corrected).
pub(crate) fn evaluation_masks<R: Real, const D: usize>(
    fixed_mask: &Option<Mask<D>>,
    moving_mask: &Option<Mask<D>>,
    correct: bool,
    virtual_space: &Space<D>,
    fixed: &Composite<R, D>,
    moving: &Composite<R, D>,
) -> (Option<Mask<D>>, Option<Mask<D>>) {
    if !correct {
        return (fixed_mask.clone(), moving_mask.clone());
    }
    (
        fixed_mask.as_ref().map(|m| {
            Mask::new(crate::resample::resample_mask_nearest(
                &m.image,
                virtual_space,
                fixed,
            ))
        }),
        moving_mask.as_ref().map(|m| {
            Mask::new(crate::resample::resample_mask_nearest(
                &m.image,
                virtual_space,
                moving,
            ))
        }),
    )
}

/// Whether moving the masks with the images (ITK-40 corrected) changes which virtual voxels
/// they hold: the stage's masks as the metric tests them at each voxel of the virtual domain,
/// against their nearest-neighbour resamplings through the fixed and moving composites.
pub(crate) fn masks_move<R: Real, const D: usize>(
    fixed_mask: &Option<Mask<D>>,
    moving_mask: &Option<Mask<D>>,
    virtual_space: &Space<D>,
    fixed: &Composite<R, D>,
    moving: &Composite<R, D>,
) -> bool {
    let differs = |m: &Mask<D>, t: &Composite<R, D>| {
        let moved = crate::resample::resample_mask_nearest(&m.image, virtual_space, t);
        (0..virtual_space.len()).any(|o| {
            let p = virtual_space.index_to_point(&virtual_space.index_of(o));
            m.is_inside(&p) != (moved.data[o] != 0)
        })
    };
    fixed_mask.as_ref().is_some_and(|m| differs(m, fixed))
        || moving_mask.as_ref().is_some_and(|m| differs(m, moving))
}

/// What the observer sees after each iteration (`IterationEvent` of the method).
pub struct TvIteration<R> {
    /// The level (0-based).
    pub level: usize,
    /// `GetCurrentIteration()` (1-based).
    pub iteration: usize,
    pub metric_value: R,
    pub convergence_value: R,
}

/// ANTs' observer of a time-varying velocity field stage (`antsRegistrationCommandIterationUpdate`).
pub trait TvObserver<R> {
    fn iteration(&mut self, state: &TvIteration<R>) -> Result<(), ItkException>;
}

fn crash(description: impl Into<String>) -> ItkException {
    ItkException::new(CRASH_FILE, 0, "", description)
}

/// `ResampleImageFilter::VerifyPreconditions` given no input (a point-set metric's image).
fn no_input() -> ItkException {
    ItkException::new(
        "Modules/Core/Common/src/itkProcessObject.cxx",
        1339,
        "ResampleImageFilter",
        "Input Primary is required but not set.",
    )
}

/// Runs a `TimeVaryingVelocityField` stage: its output transform (the flow from 0 to 1, its
/// inverse, and the velocity field for the outputs), or ITK's exception.
pub fn run_tvvf<R: Real, const D: usize>(
    stage: &TvvfStage<R, D>,
    observer: &mut dyn TvObserver<R>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<TvvfOutput<R, D>, ItkException> {
    match D {
        2 => run_tvvf_e::<R, D, 3>(stage, observer, floor, warnings),
        3 => run_tvvf_e::<R, D, 4>(stage, observer, floor, warnings),
        4 => run_tvvf_e::<R, D, 5>(stage, observer, floor, warnings),
        _ => Err(crash(format!("{D} dimensions"))),
    }
}

fn run_tvvf_e<R: Real, const D: usize, const E: usize>(
    stage: &TvvfStage<R, D>,
    observer: &mut dyn TvObserver<R>,
    floor: &IndexFloor,
    warnings: &mut Vec<String>,
) -> Result<TvvfOutput<R, D>, ItkException> {
    assert_eq!(E, D + 1);
    // The first metric must be an image metric (ANTs reads the first preprocessed fixed
    // image's geometry).
    let virtual_base = match stage.metrics.first() {
        Some(StageMetric::Image(m)) => m.fixed.space.clone(),
        _ => {
            return Err(crash(
                "a TimeVaryingVelocityField stage whose first metric is a point-set metric: ANTs allocates the velocity field on the null image of its first metric",
            ));
        }
    };
    let n_time = stage.time_indices as usize;
    if n_time == 0 {
        return Err(crash(
            "a TimeVaryingVelocityField stage with 0 time indices: ANTs divides the velocity field's voxels by 0",
        ));
    }
    let velocity_space = space_time::<D, E>(&stage.field_space, n_time, stage.field_space.start)?;
    let field_space = stage.field_space.clone();
    let mut transform = TvTransform::<R, D, E> {
        velocity: Image::new(
            velocity_space.clone(),
            vec![[R::ZERO; D]; velocity_space.len()],
        ),
        lower: R::ZERO,
        upper: R::ONE,
        steps: 10,
        field: zero_field(&field_space),
        inverse: Some(zero_field(&field_space)),
        update_spatial: stage.update_variance,
        update_temporal: stage.update_time_variance,
        total_spatial: stage.total_variance,
        total_temporal: stage.total_time_variance,
    };
    let multi = stage.metrics.len() > 1;
    let mut weights: Vec<R> = stage.metrics.iter().map(|m| m.weight()).collect();
    let mut current_seed = stage.seed;
    let fixed_initial_ignored = stage.fixed_initial.is_some();
    let mut integrated = false;
    let mut masks_not_moved = false;
    let settings = CallSettings {
        n_threads: stage.n_threads,
        correct_gc_derivative: stage.correct_gc_derivative,
        optimizer_weights: stage.optimizer_weights.as_deref(),
    };
    // Each metric object's joint histogram images (Mattes), kept across the levels and calls
    // of the stage.
    let joint_pdfs: Vec<Arc<std::sync::atomic::AtomicBool>> =
        stage.metrics.iter().map(|_| Default::default()).collect();
    for (level, factors) in stage.shrink_factors.iter().enumerate() {
        let virtual_space = shrink_space(&virtual_base, factors)?;
        // TimeVaryingVelocityFieldTransformParametersAdaptor.
        let required = space_time::<D, E>(&virtual_space, n_time, [0; D])?;
        {
            let s = &transform.velocity.space;
            let same = s.size == required.size
                && s.origin == required.origin
                && s.spacing == required.spacing
                && s.direction == required.direction;
            if !same {
                let v = pooled(stage.n_threads, || {
                    crate::field::resample_to(&transform.velocity, &required, floor)
                })?;
                transform.velocity = v;
                transform.lower = R::ZERO;
                transform.upper = R::ONE;
                let t = &mut transform;
                pooled(stage.n_threads, || t.integrate(floor, true))?;
                integrated = true;
            }
        }
        // The level's images, masks and sample points.
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
            let first_mask = level_metrics[0].as_ref().and_then(|m| m.fixed_mask.clone());
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
        // StartOptimization.
        let steps = n_time as u32 + 2;
        let n_iterations = stage.iterations.get(level).copied().unwrap_or(0) as usize;
        let per_time = transform.velocity.data.len() / n_time.max(1);
        let total = D * transform.velocity.data.len();
        let mut last_update = vec![R::ZERO; total];
        let mut convergence = WindowConvergence::new(stage.convergence_window);
        let mut point_sets: Vec<Option<PointSetMetric<R, D>>> =
            stage.metrics.iter().map(|_| None).collect();
        let mut masks: Vec<(Option<Mask<D>>, Option<Mask<D>>)> = level_metrics
            .iter()
            .map(|lm| match lm {
                Some(lm) => (lm.fixed_mask.clone(), lm.moving_mask.clone()),
                None => (None, None),
            })
            .collect();
        let mut iteration = 0usize;
        let mut converged = false;
        while iteration < n_iterations && !converged {
            iteration += 1;
            // ITK-40's notice: whether moving the masks changes them (checked until it does).
            let check_masks = !masks_not_moved;
            let step = || -> Result<(Vec<R>, R, bool), ItkException> {
                let mut update = vec![R::ZERO; total];
                let mut value_sum = R::ZERO;
                let mut masks_differ = false;
                for tp in 0..n_time {
                    let t = if n_time > 1 {
                        R::from_i64(tp as i64) / R::from_i64(n_time as i64 - 1)
                    } else {
                        R::ZERO
                    };
                    // The fixed transform: the flow from t back to 0 (zero at t = 0).
                    let fixed_field = if tp == 0 {
                        zero_field(&transform.field.space)
                    } else {
                        integrate(&transform.velocity, t.to_f64(), 0.0, steps, floor)
                    };
                    // The moving transform: the flow from t to 1 (zero at the last time point).
                    let moving_field = if tp == n_time - 1 {
                        zero_field(&transform.field.space)
                    } else {
                        integrate(&transform.velocity, t.to_f64(), 1.0, steps, floor)
                    };
                    let mut fixed: Vec<Xfm<R, D>> = match &stage.fixed_initial {
                        Some(c) if stage.correct_fixed_initial => c.transforms.clone(),
                        _ => Vec::new(),
                    };
                    fixed.push(Xfm::Field(FieldXfm::new(
                        fixed_field,
                        None,
                        stage.exact_floor,
                    )));
                    let fixed_composite = Composite::new(fixed);
                    let mut moving = stage.moving_initial.clone();
                    moving.push(Xfm::Field(FieldXfm::new(
                        moving_field,
                        None,
                        stage.exact_floor,
                    )));
                    let moving_composite = Composite::new(moving);
                    let mut resampled: Vec<Option<Resampled<R, D>>> =
                        Vec::with_capacity(level_metrics.len());
                    for lm in &level_metrics {
                        let Some(lm) = lm else {
                            return Err(no_input());
                        };
                        if check_masks && !masks_differ {
                            masks_differ = masks_move(
                                &lm.fixed_mask,
                                &lm.moving_mask,
                                &virtual_space,
                                &fixed_composite,
                                &moving_composite,
                            );
                        }
                        let (fixed_mask, moving_mask) = evaluation_masks(
                            &lm.fixed_mask,
                            &lm.moving_mask,
                            stage.correct_masks,
                            &virtual_space,
                            &fixed_composite,
                            &moving_composite,
                        );
                        resampled.push(Some(Resampled {
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
                        }));
                    }
                    let mut call = GradientCall {
                        metrics: &level_metrics,
                        resampled: &mut resampled,
                        point_sets: &mut point_sets,
                        fixed_composite: &fixed_composite,
                        moving_composite: &moving_composite,
                        identity: None,
                        virtual_space: &virtual_space,
                        weights: &mut weights,
                        multi,
                        settings,
                        floor,
                        masks: &mut masks,
                        stale_masks: false,
                        joint_pdfs: &joint_pdfs,
                        through: None,
                    };
                    let (value, derivative) = call.gradient_field(false, warnings)?;
                    value_sum = value_sum + value;
                    // The largest norm (VectorMagnitudeImageFilter, StatisticsImageFilter).
                    let mut max_norm = -R::MAX;
                    for v in &derivative.data {
                        let mut s = 0.0f64;
                        for c in v {
                            let x = c.to_f64();
                            s += x * x;
                        }
                        let norm = R::from_f64(s.sqrt());
                        if norm > max_norm {
                            max_norm = norm;
                        }
                    }
                    if max_norm <= R::ZERO {
                        max_norm = R::ONE;
                    }
                    let scale = R::from_f64(1.0 / max_norm.to_f64());
                    let offset = tp * per_time * D;
                    for (k, v) in derivative.data.iter().enumerate() {
                        for d in 0..D {
                            update[offset + k * D + d] = v[d] * scale;
                        }
                    }
                }
                Ok((update, value_sum, masks_differ))
            };
            let (mut update, value_sum, masks_differ) = pooled(stage.n_threads, step)??;
            masks_not_moved |= masks_differ;
            // Averaging with the last update reduces oscillations.
            let half = R::from_f64(0.5);
            for (u, l) in update.iter_mut().zip(&last_update) {
                *u = (*u + *l) * half;
            }
            last_update.clone_from(&update);
            {
                let t = &mut transform;
                let lr = stage.learning_rate;
                pooled(stage.n_threads, || t.update(&update, lr))??;
            }
            let metric_value = value_sum / R::from_i64(n_time as i64);
            convergence.add(metric_value.to_f64());
            let convergence_value = R::from_f64(convergence.value());
            if convergence_value < stage.convergence_threshold {
                converged = true;
            }
            if converged || iteration >= n_iterations {
                transform.lower = R::ZERO;
                transform.upper = R::ONE;
                transform.steps = steps;
                let t = &mut transform;
                pooled(stage.n_threads, || t.integrate(floor, true))?;
                integrated = true;
            } else {
                // The method's last integration of the iteration (the flow from the last time
                // point back to 0) leaves these bounds.
                if n_time > 1 {
                    transform.lower = R::ONE;
                    transform.upper = R::ZERO;
                }
                transform.steps = steps;
            }
            observer.iteration(&TvIteration {
                level,
                iteration,
                metric_value,
                convergence_value,
            })?;
        }
    }
    let mut output = VelocityOutput::of(false, &transform.velocity);
    output.integrated = integrated;
    let mut out = FieldXfm::new(transform.field, transform.inverse, stage.exact_floor);
    out.smoothing = Some(FieldSmoothing::TimeVarying(Arc::new(output)));
    Ok(TvvfOutput {
        output: out,
        fixed_initial_ignored,
        masks_not_moved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid2() -> Space<2> {
        Space::new(
            [9, 8],
            [0, 0],
            [1.5, 2.0],
            [-4.0, 3.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap()
    }

    #[test]
    fn a_constant_velocity_flows_into_a_translation_and_back() {
        let s = grid2();
        let st = space_time::<2, 3>(&s, 4, [0, 0]).unwrap();
        let v: TvField<f64, 2, 3> = Image::new(st.clone(), vec![[0.3, -0.2]; st.len()]);
        let floor = IndexFloor::itk();
        let f = integrate(&v, 0.0, 1.0, 6, &floor);
        let b = integrate(&v, 1.0, 0.0, 6, &floor);
        for (x, y) in f.data.iter().zip(&b.data) {
            assert!(
                (x[0] - 0.3).abs() < 1e-12 && (x[1] + 0.2).abs() < 1e-12,
                "{x:?}"
            );
            assert!(
                (y[0] + 0.3).abs() < 1e-12 && (y[1] - 0.2).abs() < 1e-12,
                "{y:?}"
            );
        }
        // Equal bounds or no step: a zero field.
        assert!(
            integrate(&v, 0.5, 0.5, 6, &floor)
                .data
                .iter()
                .all(|x| *x == [0.0, 0.0])
        );
        assert!(
            integrate(&v, 0.0, 1.0, 0, &floor)
                .data
                .iter()
                .all(|x| *x == [0.0, 0.0])
        );
    }

    #[test]
    fn the_flow_follows_the_time_axis() {
        // v(t) = (t, 0) in the field's time axis [0, 3] mapped onto [0, 1]: the displacement
        // after [0, 1] is ∫ 3t' dt' ... linear in time: RK4 integrates it exactly.
        let s = grid2();
        let st = space_time::<2, 3>(&s, 4, [0, 0]).unwrap();
        let data: Vec<[f64; 2]> = (0..st.len())
            .map(|o| [st.index_of(o)[2] as f64, 0.0])
            .collect();
        let v: TvField<f64, 2, 3> = Image::new(st, data);
        let f = integrate(&v, 0.0, 1.0, 4, &IndexFloor::itk());
        // The velocity at parametric time τ is 3τ: the displacement 1.5, where the path stays
        // inside the grid (voxels whose path leaves it get no velocity from there on).
        for (o, x) in f.data.iter().enumerate() {
            if f.space.index_of(o)[0] < 8 {
                assert!((x[0] - 1.5).abs() < 1e-12, "{x:?}");
            } else {
                assert!(x[0] < 1.5, "{x:?}");
            }
        }
    }

    #[test]
    fn smoothing_pins_the_spatial_boundary_only() {
        let s = grid2();
        let st = space_time::<2, 3>(&s, 3, [0, 0]).unwrap();
        let v: TvField<f64, 2, 3> = Image::new(st.clone(), vec![[1.0, 2.0]; st.len()]);
        let sm = gaussian_smooth(&v, 1.0, 0.5);
        for o in 0..st.len() {
            let i = st.index_of(o);
            let edge = i[0] == 0 || i[0] == 8 || i[1] == 0 || i[1] == 7;
            if edge {
                assert_eq!(sm.data[o], [0.0, 0.0]);
            } else {
                assert!((sm.data[o][0] - 1.0).abs() < 1e-12);
            }
        }
        assert_eq!(gaussian_smooth(&v, 0.0, 0.0), v);
    }
}

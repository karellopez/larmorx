// SPDX-License-Identifier: Apache-2.0
//! The SyN stage of ANTs' `RegistrationHelper::DoRegistration` (ANTs v2.6.5,
//! `Examples/itkantsRegistrationHelper.hxx`, `case SyN`) and its iteration observer
//! (`Examples/antsDisplacementAndVelocityFieldRegistrationCommandIterationUpdate.h`).
//!
//! The stage runs ITK's `SyNImageRegistrationMethod` ([`larmorx_registration::syn`]) from the
//! registration's composite (or, with `--initialize-transforms-per-stage`, from the
//! registration state that a previous SyN stage or `--restore-state` left), then appends the
//! output displacement field transform to the composite and records the new registration
//! state: the composite before the stage, then the fixed-to-middle and moving-to-middle
//! transforms.

use std::sync::Arc;
use std::time::Instant;

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notice, push_once};
use larmorx_registration::format::OStream;
use larmorx_registration::method::{self, SamplingStrategy, StageMetric};
use larmorx_registration::pointset::metric::PointSetEvents;
use larmorx_registration::syn::{
    SynIteration, SynObserver, SynStage, full_scale_cc, output_transform, run_syn,
};
use larmorx_registration::transform::{Composite, Xfm};
use larmorx_registration::{Image, ItkException};

use super::MetricKind;
use super::helper::{Compute, IntervalSink, Log, RegistrationFailed, RegistrationHelper};

/// What `DoRegistration` prepared for the stage before its `switch`.
pub(crate) struct SynInputs<R, const D: usize> {
    pub stage: usize,
    pub iterations: Vec<u32>,
    pub threshold: R,
    pub window: u32,
    pub levels: usize,
    /// The stage's metric kinds (`None`: a metric larmorx's parser did not name); IGDM first
    /// is refused.
    pub kinds: Vec<Option<MetricKind>>,
    pub prepared: Vec<StageMetric<R, D>>,
    pub shrink_factors: Vec<[usize; D]>,
    pub sigmas: Vec<R>,
    pub sampling: SamplingStrategy,
    pub percentage: R,
}

/// A pair of images: the fixed and the moving one.
type ImagePair<R, const D: usize> = (Arc<Image<R, D>>, Arc<Image<R, D>>);

/// ANTs' observer of a SyN stage.
struct SynLog<'l, 'a, 's, 'k, R, const D: usize> {
    log: &'l mut Log<'a>,
    iterations: Vec<u32>,
    clock: Instant,
    last: f64,
    /// `--print-similarity-measure-interval` (0: off).
    full_scale_interval: u32,
    /// The first metric's original images (`SetOrigFixedImage`, `SetOrigMovingImage`).
    originals: Option<ImagePair<R, D>>,
    /// The stage's metric is an image metric (ANTs' observer casts it to one).
    single_image_metric: bool,
    /// `--write-interval-volumes` (0: off), the stage and the output prefix.
    write_interval: u32,
    stage: usize,
    prefix: String,
    sink: Option<&'s mut (dyn IntervalSink<R, D> + 'k)>,
    fixed_initial: Option<Composite<R, D>>,
    moving_initial: Vec<Xfm<R, D>>,
    /// ANTS-71 reproduced: the full-scale similarity's crash.
    reproduce: bool,
    floor: &'s IndexFloor,
    n_threads: usize,
    warnings: Vec<String>,
    /// The full-scale similarity met ANTS-71's condition.
    ants_71: bool,
}

impl<R: Compute, const D: usize> SynObserver<R, D> for SynLog<'_, '_, '_, '_, R, D> {
    fn iteration(&mut self, s: &SynIteration<'_, R, D>) -> Result<(), ItkException> {
        let current = s.iteration as u32;
        if current == 1 {
            if self.full_scale_interval != 0 {
                self.log.logger(&format!(
                    "XXDIAGNOSTIC,Iteration,metricValue,convergenceValue,ITERATION_TIME_INDEX,SINCE_LAST,FullScaleCCInterval={}\n",
                    self.full_scale_interval
                ));
            } else {
                self.log.logger(
                    "XXDIAGNOSTIC,Iteration,metricValue,convergenceValue,ITERATION_TIME_INDEX,SINCE_LAST\n",
                );
            }
        }
        let now = self.clock.elapsed().as_secs_f64();
        let last_iteration = self.iterations.get(s.level).copied().unwrap_or(0);
        let due = |interval: u32| {
            interval != 0
                && (current == 1 || current.is_multiple_of(interval) || current == last_iteration)
        };
        let mut full_scale_value = R::ZERO;
        if due(self.full_scale_interval) {
            let missing = self.fixed_initial.is_none()
                || self.moving_initial.is_empty()
                || !self.single_image_metric;
            if missing {
                self.ants_71 = true;
                if self.reproduce {
                    return Err(ItkException::new(
                        method::CRASH_FILE,
                        0,
                        "",
                        "--print-similarity-measure-interval in a SyN stage without an initial fixed transform (-q), without a moving transform before the stage, or with a metric that is not a single image metric: ANTs' observer adds a null transform to the full-scale similarity's composites or reads the metric through a null pointer (ANTS-71)",
                    ));
                }
            }
            let Some((fixed, moving)) = &self.originals else {
                return Err(ItkException::new(
                    method::CRASH_FILE,
                    0,
                    "",
                    "--print-similarity-measure-interval with a point-set metric first: ANTs' observer computes the full-scale similarity of null images",
                ));
            };
            let mut fixed_composite = self
                .fixed_initial
                .as_ref()
                .map(|c| c.transforms.clone())
                .unwrap_or_default();
            fixed_composite.push(Xfm::Field(
                s.fixed_to_middle
                    .inverse_transform()
                    .expect("SyN's fields have inverses"),
            ));
            let mut moving_composite = self.moving_initial.clone();
            moving_composite.push(Xfm::Field(
                s.moving_to_middle
                    .inverse_transform()
                    .expect("SyN's fields have inverses"),
            ));
            let mut printed = Vec::new();
            full_scale_value = full_scale_cc(
                fixed,
                moving,
                &Composite::new(fixed_composite),
                &Composite::new(moving_composite),
                self.floor,
                self.n_threads,
                &mut printed,
            )?;
            for w in printed {
                self.log.cerr(&w);
                self.warnings.push(w);
            }
        }
        if due(self.write_interval) {
            let name = format!(
                "{}Stage{}_level{}_Iter{:04}.nii.gz",
                self.prefix,
                self.stage + 1,
                s.level + 1,
                current
            );
            self.log.cout(&format!("*{name}\n"));
            if self.originals.is_none() {
                return Err(ItkException::new(
                    method::CRASH_FILE,
                    0,
                    "",
                    "--write-interval-volumes with a point-set metric first: ANTs' observer resamples a null image",
                ));
            }
            if let Some(sink) = self.sink.as_deref_mut() {
                let mut queue = self.moving_initial.clone();
                queue.push(Xfm::Field(output_transform(
                    s.fixed_to_middle,
                    s.moving_to_middle,
                    self.floor,
                )));
                sink.write_interval_volume(self.log, &name, &queue);
            }
        } else {
            self.log.logger(" ");
        }
        let sci12 = OStream {
            scientific: true,
            precision: 12,
        };
        let sci4 = OStream {
            scientific: true,
            precision: 4,
        };
        let ss = self.log.cout.precision;
        let mut line = format!(
            "1DIAGNOSTIC, {:>5}, {}, {}, {}, {}, ",
            current,
            sci12.float(s.metric_value.to_f64()),
            sci12.float(s.convergence_value.to_f64()),
            sci4.float(now),
            sci4.float(now - self.last)
        );
        if self.full_scale_interval != 0 && full_scale_value.to_f64().abs() > 1e-7 {
            line.push_str(&sci12.float(full_scale_value.to_f64()));
        }
        line.push('\n');
        self.log.logger(&line);
        // setprecision(ss) and unsetf(fixed | scientific) on the log stream.
        if self.log.verbose {
            self.log.cout = OStream {
                scientific: false,
                precision: ss,
            };
        }
        self.last = now;
        Ok(())
    }
}

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// The notices of a SyN or B-spline SyN stage's outcome: ITK-33 (point-set metrics among
    /// several metrics) and ITK-31 (stale masks).
    pub(crate) fn syn_notices(&mut self, stage: usize, identity_point_sets: bool, stale_masks: bool) {
        let mode = self.upstream_defects;
        if identity_point_sets {
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ITK_33,
                    mode,
                    if mode.fix() {
                        format!(
                            "stage {stage}: the point-set metrics among several metrics are evaluated through the initial transforms and the deformation; ITK gives them identity transforms"
                        )
                    } else {
                        format!(
                            "stage {stage}: the point-set metrics among several metrics are evaluated with identity transforms, as in ITK (they never see the initial transforms or the deformation)"
                        )
                    },
                ),
            );
        }
        if stale_masks {
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ITK_31,
                    mode,
                    if mode.fix() {
                        format!(
                            "stage {stage} has only one of its two masks: the other role has none; ITK keeps the previous evaluation's resampled mask there"
                        )
                    } else {
                        format!(
                            "stage {stage} has only one of its two masks: the other role keeps the previous evaluation's resampled mask, as in ITK"
                        )
                    },
                ),
            );
        }
    }

    /// The `case SyN` of `DoRegistration`.
    pub(crate) fn syn_stage<'s, 'k>(
        &mut self,
        log: &mut Log<'_>,
        floor: &IndexFloor,
        sink: Option<&'s mut (dyn IntervalSink<R, D> + 'k)>,
        x: SynInputs<R, D>,
    ) -> Result<(), RegistrationFailed> {
        if x.kinds.first().copied().flatten() == Some(MetricKind::Igdm) {
            log.logger(
                "Intensity point set metric is not implemented yet for the specified transform.\n",
            );
            return Err(RegistrationFailed(
                "Intensity point set metric is not implemented yet for the specified transform."
                    .into(),
            ));
        }
        let stage = x.stage;
        let method = &self.transforms[stage];
        let learning_rate = method.gradient_step;
        let update_variance = method.update_field_variance;
        let total_variance = method.total_field_variance;
        let optimizer_weights = match self.restrict_deformation.get(stage) {
            Some(w) if w.len() == D => Some(w.clone()),
            _ => None,
        };
        // --initialize-transforms-per-stage: start from the registration state.
        let mut restored = None;
        let mut syn_initialized = false;
        let mut moving_initial: Vec<Xfm<R, D>> = Vec::new();
        if self.initialize_transforms_per_stage
            && let Some(state) = self.registration_state.as_mut()
        {
            let n = state.len();
            let pair = if n >= 2 {
                match (&state[n - 2], &state[n - 1]) {
                    (Xfm::Field(f), Xfm::Field(m))
                        if f.inverse.is_some() && m.inverse.is_some() =>
                    {
                        Some((f.clone(), m.clone()))
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some(pair) = pair {
                log.logger(
                    "Current SyN transform is directly initialized from the previous stage.\n",
                );
                restored = Some(pair);
                state.truncate(n - 2);
            }
            if !state.is_empty() {
                moving_initial = state.clone();
            }
            syn_initialized = true;
            let composite = self.composite.as_mut().expect("set by DoRegistration");
            composite.pop();
            self.origins.truncate(composite.len());
        }
        let composite = self.composite.clone().expect("set by DoRegistration");
        if !composite.is_empty() && !syn_initialized {
            moving_initial = composite.clone();
        }
        let fixed_initial = self
            .fixed_initial
            .as_ref()
            .filter(|f| !f.is_empty())
            .map(|f| Composite::new(f.clone()));
        if let Err(e) = method::check_sampling_percentage_levels(x.percentage.to_f64(), x.levels) {
            return Err(self.exception_in_main(log, &e));
        }
        let seed = if self.random_seed != 0 {
            self.random_seed
        } else {
            self.default_seed
        };
        log.logger(&format!(
            "\n*** Running SyN registration (varianceForUpdateField = {}, varianceForTotalField = {}) ***\n\n",
            log.f(update_variance.to_f64()),
            log.f(total_variance.to_f64())
        ));
        // ANTS-61: the SyN method holds labelled point sets only, so an IGDM metric gets no
        // input: the inputs end at the last metric that has one.
        let mut failed_early = None;
        let igdm: Vec<bool> = x
            .kinds
            .iter()
            .map(|k| *k == Some(MetricKind::Igdm))
            .collect();
        if igdm.iter().any(|&w| w) {
            let pairs = igdm.iter().rposition(|&w| !w).map_or(0, |i| i + 1);
            let mismatch = pairs != igdm.len();
            let mode = self.upstream_defects;
            if mode.fix() {
                push_once(
                    &mut self.notices,
                    Notice::new(
                        &defects::ANTS_61,
                        mode,
                        format!(
                            "stage {stage} runs with all its metrics; ANTs gives IGDM no input and {}",
                            if mismatch {
                                "throws (the stage is dropped)"
                            } else {
                                "crashes"
                            }
                        ),
                    ),
                );
            } else if mismatch {
                push_once(
                    &mut self.notices,
                    Notice::new(
                        &defects::ANTS_61,
                        mode,
                        format!(
                            "stage {stage}: a point-set metric without an input, ITK's exception, as in ANTs"
                        ),
                    ),
                );
                failed_early = Some(ItkException::new(
                    "Modules/Registration/RegistrationMethodsv4/include/itkImageRegistrationMethodv4.hxx",
                    306,
                    "SyNImageRegistrationMethod",
                    "Mismatch between number of image pairs and the number of metrics.",
                ));
            } else {
                return Err(self.crash(
                    log,
                    "a SyN stage that lists IGDM before another metric: ANTs gives IGDM no point sets",
                ));
            }
        }
        let correct_gc = self.upstream_defects.fix();
        let has_gc = x
            .prepared
            .iter()
            .any(|m| matches!(m, StageMetric::Image(i) if i.kind == method::MetricKind::Gc));
        if has_gc {
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ITK_30,
                    self.upstream_defects,
                    if correct_gc {
                        format!(
                            "stage {stage}: GC's derivative is each voxel's own; ITK adds all of it to the first voxel"
                        )
                    } else {
                        format!(
                            "stage {stage}: GC's derivative is all in the first voxel, as in ITK"
                        )
                    },
                ),
            );
        }
        let single_image_metric =
            x.prepared.len() == 1 && matches!(x.prepared[0], StageMetric::Image(_));
        let syn = SynStage {
            metrics: x.prepared,
            shrink_factors: x.shrink_factors,
            smoothing_sigmas: x.sigmas,
            sigmas_in_physical_units: self.sigmas_in_physical_units[stage],
            sampling: x.sampling,
            sampling_percentage: x.percentage,
            seed,
            fixed_initial: fixed_initial.clone(),
            moving_initial: moving_initial.clone(),
            learning_rate,
            iterations: x.iterations.clone(),
            convergence_threshold: x.threshold,
            convergence_window: x.window as usize,
            update_variance,
            total_variance,
            optimizer_weights,
            restored,
            exact_floor: self.upstream_defects.fix(),
            correct_gc_derivative: correct_gc,
            correct_stale_masks: self.upstream_defects.fix(),
            correct_point_set_transforms: self.upstream_defects.fix(),
            bspline: None,
            n_threads: self.n_threads,
        };
        let originals = match (&self.metrics[0].fixed, &self.metrics[0].moving) {
            (Some(f), Some(m)) => Some((Arc::clone(f), Arc::clone(m))),
            _ => None,
        };
        let mut observer = SynLog::<R, D> {
            log,
            iterations: x.iterations,
            clock: Instant::now(),
            last: 0.0,
            full_scale_interval: self.print_similarity_measure_interval,
            originals,
            single_image_metric,
            write_interval: self.write_interval_volumes,
            stage,
            prefix: self.output_prefix.clone(),
            sink,
            fixed_initial,
            moving_initial,
            reproduce: !self.upstream_defects.fix(),
            floor,
            n_threads: self.n_threads,
            warnings: Vec::new(),
            ants_71: false,
        };
        let mut warnings = Vec::new();
        let mut events = PointSetEvents::default();
        let result = match failed_early {
            Some(e) => Err(e),
            None => run_syn(&syn, &mut observer, floor, &mut warnings, &mut events),
        };
        let observed = std::mem::take(&mut observer.warnings);
        let ants_71 = observer.ants_71;
        for w in &warnings {
            log.cerr(w);
        }
        self.warnings.extend(observed);
        self.warnings.extend(warnings);
        self.point_set_notices(&events);
        if ants_71 {
            let mode = self.upstream_defects;
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ANTS_71,
                    mode,
                    if mode.fix() {
                        format!(
                            "stage {stage}: the full-scale similarity is computed with the missing transforms as the identity; ANTs crashes"
                        )
                    } else {
                        format!(
                            "stage {stage}: ANTs crashes computing the full-scale similarity; larmorx stops there"
                        )
                    },
                ),
            );
        }
        match result {
            Ok(out) => {
                self.syn_notices(stage, out.identity_point_sets, out.stale_masks);
                // m_RegistrationState: the composite as it is, then the two transforms.
                let composite = self.composite.as_mut().expect("set by DoRegistration");
                let mut state = composite.clone();
                state.push(Xfm::Field(out.fixed_to_middle));
                state.push(Xfm::Field(out.moving_to_middle));
                self.registration_state = Some(state);
                composite.push(Xfm::Field(out.output));
                self.origins.push(Some(stage));
                Ok(())
            }
            Err(e) if e.file == method::CRASH_FILE => Err(self.crash(log, &e.description)),
            Err(e) => {
                log.logger(&format!("Exception caught: {}\n", e.print()));
                Err(RegistrationFailed(e.description))
            }
        }
    }
}

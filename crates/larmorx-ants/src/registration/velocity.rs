// SPDX-License-Identifier: Apache-2.0
//! The velocity-field stages of ANTs' `RegistrationHelper::DoRegistration` (ANTs v2.6.5,
//! `Examples/itkantsRegistrationHelper.hxx`, `case TimeVaryingVelocityField` and
//! `case TimeVaryingBSplineVelocityField`), observed by `antsRegistrationCommandIterationUpdate`
//! (`XDIAGNOSTIC`/`WDIAGNOSTIC` lines).
//!
//! The time-varying velocity field stage runs ITK's
//! `TimeVaryingVelocityFieldImageRegistrationMethodv4` ([`larmorx_registration::time_varying`])
//! from the registration's composite. It does not go through `PrepareRegistrationMethod`: no
//! `--initialize-transforms-per-stage` listing, and its metric sampling is never reseeded. Its
//! output joins the composite as a displacement field with its inverse.

use std::time::Instant;

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notice, push_once};
use larmorx_registration::ItkException;
use larmorx_registration::method::{self, StageMetric};
use larmorx_registration::pointset::metric::PointSetEvents;
use larmorx_registration::time_varying::{TvIteration, TvObserver, TvvfStage, run_tvvf};
use larmorx_registration::transform::{Composite, Xfm};
use larmorx_registration::tv_bspline::{TvbStage, run_tvbsvf};

use super::bspline_syn::MethodLog;
use super::helper::{Compute, Log, RegistrationFailed, RegistrationHelper};
use super::syn::SynInputs;

impl<R: Compute> TvObserver<R> for MethodLog<'_, '_> {
    fn iteration(&mut self, s: &TvIteration<R>) -> Result<(), ItkException> {
        MethodLog::iteration(
            self,
            s.iteration,
            s.metric_value.to_f64(),
            s.convergence_value.to_f64(),
        );
        Ok(())
    }
}

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// The `case TimeVaryingVelocityField` of `DoRegistration`.
    pub(crate) fn tvvf_stage(
        &mut self,
        log: &mut Log<'_>,
        floor: &IndexFloor,
        x: SynInputs<R, D>,
    ) -> Result<(), RegistrationFailed> {
        let stage = x.stage;
        // The velocity field is allocated on the first preprocessed fixed image, before
        // anything else: a point-set metric first has none.
        let field_space = match x.prepared.first() {
            Some(StageMetric::Image(m)) => m.fixed.space.clone(),
            _ => {
                return Err(self.crash(
                    log,
                    "a TimeVaryingVelocityField stage whose first metric is a point-set metric: ANTs allocates the velocity field on the null image of its first metric",
                ));
            }
        };
        let t = &self.transforms[stage];
        let optimizer_weights = match self.restrict_deformation.get(stage) {
            Some(w) if w.len() == D => Some(w.clone()),
            _ => None,
        };
        // SetMetricSamplingPercentage outside the try block.
        if let Err(e) = method::check_sampling_percentage_levels(x.percentage.to_f64(), x.levels) {
            return Err(self.exception_in_main(log, &e));
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
                        format!("stage {stage}: GC's derivative is all in the first voxel, as in ITK")
                    },
                ),
            );
        }
        let seed = if self.random_seed != 0 {
            self.random_seed
        } else {
            self.default_seed
        };
        let spec = TvvfStage {
            metrics: x.prepared,
            shrink_factors: x.shrink_factors,
            smoothing_sigmas: x.sigmas,
            sigmas_in_physical_units: self.sigmas_in_physical_units[stage],
            sampling: x.sampling,
            sampling_percentage: x.percentage,
            seed,
            moving_initial: self.composite.clone().expect("set by DoRegistration"),
            fixed_initial: self
                .fixed_initial
                .as_ref()
                .filter(|f| !f.is_empty())
                .map(|f| Composite::new(f.clone())),
            correct_fixed_initial: self.upstream_defects.fix(),
            correct_masks: self.upstream_defects.fix(),
            learning_rate: t.gradient_step,
            iterations: x.iterations.clone(),
            convergence_threshold: x.threshold,
            convergence_window: x.window as usize,
            time_indices: t.number_of_time_indices,
            update_variance: t.update_field_variance,
            update_time_variance: t.update_field_time_sigma,
            total_variance: t.total_field_variance,
            total_time_variance: t.total_field_time_sigma,
            optimizer_weights,
            field_space,
            exact_floor: self.upstream_defects.fix(),
            correct_gc_derivative: correct_gc,
            n_threads: self.n_threads,
        };
        log.logger(&format!(
            "\n*** Running time-varying velocity field registration (varianceForUpdateField = {}, varianceForTotalField = {}, varianceForUpdateFieldTime = {}, varianceForTotalFieldTime = {}) ***\n\n",
            log.f(t.update_field_variance.to_f64()),
            log.f(t.total_field_variance.to_f64()),
            log.f(t.update_field_time_sigma.to_f64()),
            log.f(t.total_field_time_sigma.to_f64()),
        ));
        let mut observer = MethodLog {
            log,
            clock: Instant::now(),
            last: 0.0,
        };
        let mut warnings = Vec::new();
        let result = run_tvvf(&spec, &mut observer, floor, &mut warnings);
        let log = observer.log;
        for w in &warnings {
            log.cerr(w);
        }
        self.warnings.extend(warnings);
        match result {
            Ok(out) => {
                let fix = self.upstream_defects.fix();
                if out.fixed_initial_ignored {
                    push_once(
                        &mut self.notices,
                        Notice::new(
                            &defects::ITK_39,
                            self.upstream_defects,
                            if fix {
                                format!("stage {stage}: the fixed images are resampled through the fixed initial transforms (-q) and the flow; ITK's TimeVaryingVelocityField method ignores -q")
                            } else {
                                format!("stage {stage}: the fixed initial transforms (-q) are ignored, as in ITK")
                            },
                        ),
                    );
                }
                self.mask_notice(stage, out.masks_not_moved);
                let composite = self.composite.as_mut().expect("set by DoRegistration");
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

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// The `case TimeVaryingBSplineVelocityField` of `DoRegistration`.
    pub(crate) fn tvbsvf_stage(
        &mut self,
        log: &mut Log<'_>,
        floor: &IndexFloor,
        x: SynInputs<R, D>,
    ) -> Result<(), RegistrationFailed> {
        let stage = x.stage;
        let mesh = self.transforms[stage].velocity_field_mesh.clone();
        if mesh.len() != D + 1 {
            log.logger(&format!(
                "The transform domain mesh size does not have the correct number of elements.For image dimension = {D}, you need {}elements. \n",
                D + 1
            ));
            return Err(RegistrationFailed(
                "The transform domain mesh size does not have the correct number of elements."
                    .into(),
            ));
        }
        let Some(domain) = x.prepared.first().and_then(|m| m.virtual_domain().cloned()) else {
            return Err(self.crash(
                log,
                "a stage of point-set metrics needs a fixed mask (-x) to define its virtual domain: ANTs reads the spacing of a null image",
            ));
        };
        let t = &self.transforms[stage];
        let (learning_rate, samples, order) =
            (t.gradient_step, t.number_of_time_point_samples, t.spline_order);
        let optimizer_weights = match self.restrict_deformation.get(stage) {
            Some(w) if w.len() == D => Some(w.clone()),
            _ => None,
        };
        // PrepareRegistrationMethod: the sampling percentage (outside the try block), the
        // --initialize-transforms-per-stage attempt, the point sets' kinds.
        if let Err(e) = method::check_sampling_percentage_levels(x.percentage.to_f64(), x.levels) {
            return Err(self.exception_in_main(log, &e));
        }
        self.list_for_initialization(log, "TimeVaryingBSplineVelocityField");
        let failed_early = self.point_set_kinds(
            log,
            stage,
            &x.kinds,
            "TimeVaryingBSplineVelocityFieldImageRegistrationMethod",
        )?;
        let moving_initial = self.composite.clone().expect("set by DoRegistration");
        let fixed_initial = self
            .fixed_initial
            .as_ref()
            .filter(|f| !f.is_empty())
            .map(|f| Composite::new(f.clone()));
        let seed = if self.random_seed != 0 {
            self.random_seed
        } else {
            self.default_seed
        };
        let fix = self.upstream_defects.fix();
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
                    if fix {
                        format!(
                            "stage {stage}: GC's derivative is each voxel's own; ITK adds all of it to the first voxel"
                        )
                    } else {
                        format!("stage {stage}: GC's derivative is all in the first voxel, as in ITK")
                    },
                ),
            );
        }
        let spec = TvbStage {
            metrics: x.prepared,
            shrink_factors: x.shrink_factors,
            smoothing_sigmas: x.sigmas,
            sigmas_in_physical_units: self.sigmas_in_physical_units[stage],
            sampling: x.sampling,
            sampling_percentage: x.percentage,
            seed,
            fixed_initial,
            moving_initial,
            learning_rate,
            iterations: x.iterations.clone(),
            convergence_threshold: x.threshold,
            convergence_window: x.window as usize,
            mesh: mesh.clone(),
            time_samples: samples,
            order,
            optimizer_weights,
            domain,
            exact_floor: fix,
            correct_gc_derivative: fix,
            correct_multi_metric_transforms: fix,
            correct_stale_inverses: fix,
            correct_point_positions: fix,
            correct_masks: fix,
            n_threads: self.n_threads,
        };
        log.logger(&format!(
            "\n*** Running time-varying b-spline velocity field registration (initial mesh size = {}) ***\n\n",
            super::bspline_syn::print_array(&mesh)
        ));
        let mut observer = MethodLog {
            log,
            clock: Instant::now(),
            last: 0.0,
        };
        let mut warnings = Vec::new();
        let mut events = PointSetEvents::default();
        let result = match failed_early {
            Some(e) => Err(e),
            None => run_tvbsvf(&spec, &mut observer, floor, &mut warnings, &mut events),
        };
        let log = observer.log;
        for w in &warnings {
            log.cerr(w);
        }
        self.warnings.extend(warnings);
        self.point_set_notices(&events);
        match result {
            Ok(out) => {
                if out.multi_metric_transforms {
                    push_once(
                        &mut self.notices,
                        Notice::new(
                            &defects::ITK_36,
                            self.upstream_defects,
                            if fix {
                                format!(
                                    "stage {stage}: each metric of the multi-metric is evaluated through its own transforms; ITK gives every metric the transforms the last one set (point-set metrics before an image metric see identity fields, image metrics after a point-set metric are warped twice)"
                                )
                            } else {
                                format!(
                                    "stage {stage}: every metric of the multi-metric is evaluated through the transforms the last one set, as in ITK"
                                )
                            },
                        ),
                    );
                }
                self.mask_notice(stage, out.masks_not_moved);
                if out.physical_points {
                    push_once(
                        &mut self.notices,
                        Notice::new(
                            &defects::ITK_38,
                            self.upstream_defects,
                            if fix {
                                format!(
                                    "stage {stage}: the point-set metric's points are fitted at their positions in the B-spline's parametric domain; ITK fits them at their physical positions, outside that domain when the virtual domain's direction is not the identity (and throws when one falls outside it)"
                                )
                            } else {
                                format!(
                                    "stage {stage}: the point-set metric's points are fitted at their physical positions in the B-spline's parametric domain, as in ITK"
                                )
                            },
                        ),
                    );
                }
                if out.stale_inverses {
                    push_once(
                        &mut self.notices,
                        Notice::new(
                            &defects::ITK_37,
                            self.upstream_defects,
                            if fix {
                                format!(
                                    "stage {stage}: at the first and last time samples the zero flows have zero inverses; ITK keeps the previous integration's inverse, which moves the point sets"
                                )
                            } else {
                                format!(
                                    "stage {stage}: at the first and last time samples the zero flows keep the previous integration's inverse, as in ITK"
                                )
                            },
                        ),
                    );
                }
                let composite = self.composite.as_mut().expect("set by DoRegistration");
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

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// ITK-40's notice: the time-varying velocity field methods' metrics keep the stage's masks
    /// while their images are resampled through the flows.
    fn mask_notice(&mut self, stage: usize, met: bool) {
        if !met {
            return;
        }
        let fix = self.upstream_defects.fix();
        push_once(
            &mut self.notices,
            Notice::new(
                &defects::ITK_40,
                self.upstream_defects,
                if fix {
                    format!("stage {stage}: the masks are resampled with their images through the flows (nearest neighbour), as SyN resamples them; ITK's time-varying velocity field methods keep them where they were")
                } else {
                    format!("stage {stage}: the masks stay where they were while the images are resampled through the flows, as in ITK")
                },
            ),
        );
    }
}

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
use larmorx_registration::time_varying::{TvIteration, TvObserver, TvvfStage, run_tvvf};
use larmorx_registration::transform::Xfm;

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
            Ok(output) => {
                let composite = self.composite.as_mut().expect("set by DoRegistration");
                composite.push(Xfm::Field(output));
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

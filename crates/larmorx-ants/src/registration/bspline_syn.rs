// SPDX-License-Identifier: Apache-2.0
//! The B-spline SyN stage of ANTs' `RegistrationHelper::DoRegistration` (ANTs v2.6.5,
//! `Examples/itkantsRegistrationHelper.hxx`, `case BSplineSyN`) and its iteration observer
//! (`Examples/antsRegistrationCommandIterationUpdate.h`).
//!
//! The stage runs ITK's `BSplineSyNImageRegistrationMethod` ([`larmorx_registration::syn`] with
//! [`BSplineSynSmoothing`]) from the registration's composite. Unlike a SyN stage it goes
//! through `PrepareRegistrationMethod` (so `--initialize-transforms-per-stage` only lists the
//! composite and fails to initialise it), accepts IGDM as its first metric (its registration
//! method then holds intensity point sets), never reads or writes the registration state, and
//! its observer prints `XDIAGNOSTIC`/`WDIAGNOSTIC` lines.

use std::time::Instant;

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notice, push_once};
use larmorx_registration::format::OStream;
use larmorx_registration::method::{self, StageMetric};
use larmorx_registration::pointset::metric::PointSetEvents;
use larmorx_registration::syn::{
    BSplineSynSmoothing, SynIteration, SynObserver, SynStage, run_syn,
};
use larmorx_registration::transform::{Composite, Xfm};
use larmorx_registration::{ItkException, Real};

use super::MetricKind;
use super::helper::{
    Compute, Log, RegistrationFailed, RegistrationHelper, initialize_with_previous,
};
use super::syn::SynInputs;

/// ANTs' observer of the stages that run an `ImageRegistrationMethodv4` subclass with their
/// own optimisation loop (`antsRegistrationCommandIterationUpdate`): at each iteration
/// (`IterationEvent` of the method) the header at the first, then a `WDIAGNOSTIC` line. The
/// log stream stays in scientific notation with precision 4 afterwards.
pub(crate) struct MethodLog<'l, 'a> {
    pub log: &'l mut Log<'a>,
    pub clock: Instant,
    pub last: f64,
}

impl MethodLog<'_, '_> {
    /// One `IterationEvent` (`GetCurrentIteration`, `GetCurrentMetricValue`,
    /// `GetCurrentConvergenceValue`).
    pub fn iteration(&mut self, iteration: usize, metric: f64, convergence: f64) {
        if iteration == 1 {
            self.log
                .logger("XDIAGNOSTIC,Iteration,metricValue,convergenceValue,ITERATION_TIME_INDEX,SINCE_LAST\n");
        }
        let now = self.clock.elapsed().as_secs_f64();
        let sci12 = OStream {
            scientific: true,
            precision: 12,
        };
        let sci4 = OStream {
            scientific: true,
            precision: 4,
        };
        self.log.logger(&format!(
            "WDIAGNOSTIC, {:>5}, {}, {}, {}, {}, \n",
            iteration,
            sci12.float(metric),
            sci12.float(convergence),
            sci4.float(now),
            sci4.float(now - self.last)
        ));
        if self.log.verbose {
            self.log.cout = sci4;
        }
        self.last = now;
    }
}

impl<R: Real, const D: usize> SynObserver<R, D> for MethodLog<'_, '_> {
    fn iteration(&mut self, s: &SynIteration<'_, R, D>) -> Result<(), ItkException> {
        MethodLog::iteration(
            self,
            s.iteration,
            s.metric_value.to_f64(),
            s.convergence_value.to_f64(),
        );
        Ok(())
    }
}

/// An `ArrayType` (`FixedArray<unsigned int>`) as `operator<<` prints it.
pub(crate) fn print_array(values: &[u32]) -> String {
    let v: Vec<String> = values.iter().map(|x| x.to_string()).collect();
    format!("[{}]", v.join(", "))
}

impl<R: Compute, const D: usize> RegistrationHelper<R, D> {
    /// `PrepareRegistrationMethod`'s `--initialize-transforms-per-stage` part for a stage whose
    /// transform is not linear (`name`: its class without `Transform`): the composite listed,
    /// then ANTs' failed attempt to initialise it from the composite's last transform.
    pub(crate) fn list_for_initialization(&self, log: &mut Log<'_>, name: &str) {
        if !self.initialize_transforms_per_stage {
            return;
        }
        let queue = self.composite.as_ref().expect("set by DoRegistration");
        if queue.is_empty() {
            return;
        }
        log.logger(&format!(
            "Current number of transforms in the composite transform: {}\n",
            queue.len()
        ));
        for (i, t) in queue.iter().enumerate() {
            log.logger(&format!("{}) {}\n", i + 1, t.class()));
        }
        let mut dummy = Xfm::Identity;
        initialize_with_previous(log, queue.last(), name, &mut dummy);
    }

    /// ANTS-61 for a stage that goes through `PrepareRegistrationMethod`: its registration
    /// method holds the intensity point sets when the first metric is IGDM, the labelled ones
    /// otherwise, and a point-set metric of the other kind gets no input. Returns ITK's
    /// exception when ANTs throws (reproduced), or stops where ANTs crashes.
    pub(crate) fn point_set_kinds(
        &mut self,
        log: &mut Log<'_>,
        stage: usize,
        kinds: &[Option<MetricKind>],
        class: &'static str,
    ) -> Result<Option<ItkException>, RegistrationFailed> {
        let intensity_kind = kinds.first().copied().flatten() == Some(MetricKind::Igdm);
        let wrong: Vec<bool> = kinds
            .iter()
            .map(|k| {
                k.is_some_and(|k| k.is_point_set())
                    && ((*k == Some(MetricKind::Igdm)) != intensity_kind)
            })
            .collect();
        if !wrong.iter().any(|&w| w) {
            return Ok(None);
        }
        let pairs = wrong.iter().rposition(|&w| !w).map_or(0, |i| i + 1);
        let mismatch = pairs != kinds.len();
        if self.upstream_defects.fix() {
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ANTS_61,
                    self.upstream_defects,
                    format!(
                        "stage {stage} runs with all its metrics; ANTs gives IGDM, or the labelled point sets, no input and {}",
                        if mismatch {
                            "throws (the stage is dropped)"
                        } else {
                            "crashes"
                        }
                    ),
                ),
            );
            Ok(None)
        } else if mismatch {
            push_once(
                &mut self.notices,
                Notice::new(
                    &defects::ANTS_61,
                    self.upstream_defects,
                    format!(
                        "stage {stage}: a point-set metric without an input, ITK's exception, as in ANTs"
                    ),
                ),
            );
            Ok(Some(ItkException::new(
                "Modules/Registration/RegistrationMethodsv4/include/itkImageRegistrationMethodv4.hxx",
                306,
                class,
                "Mismatch between number of image pairs and the number of metrics.",
            )))
        } else {
            Err(self.crash(
                log,
                "a stage that combines IGDM with ICP, PSE or JHCT, or lists it after an image metric: ANTs casts a point-set metric to the other kind's type",
            ))
        }
    }

    /// The `case BSplineSyN` of `DoRegistration`.
    pub(crate) fn bspline_syn_stage(
        &mut self,
        log: &mut Log<'_>,
        floor: &IndexFloor,
        x: SynInputs<R, D>,
    ) -> Result<(), RegistrationFailed> {
        let stage = x.stage;
        let method = &self.transforms[stage];
        let learning_rate = method.gradient_step;
        let order = method.spline_order;
        if method.update_field_mesh.len() != D || method.total_field_mesh.len() != D {
            log.logger("ERROR:  The mesh size(s) don't match the ImageDimension.\n");
            return Err(RegistrationFailed(
                "The mesh size(s) don't match the ImageDimension.".into(),
            ));
        }
        let update_mesh: [u32; D] = std::array::from_fn(|d| method.update_field_mesh[d]);
        let total_mesh: [u32; D] = std::array::from_fn(|d| method.total_field_mesh[d]);
        let optimizer_weights = match self.restrict_deformation.get(stage) {
            Some(w) if w.len() == D => Some(w.clone()),
            _ => None,
        };
        // PrepareRegistrationMethod: the sampling percentage (outside the try block), the
        // --initialize-transforms-per-stage attempt, the initial transforms.
        if let Err(e) = method::check_sampling_percentage_levels(x.percentage.to_f64(), x.levels) {
            return Err(self.exception_in_main(log, &e));
        }
        self.list_for_initialization(log, "BSplineSmoothingOnUpdateDisplacementField");
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
        let failed_early =
            self.point_set_kinds(log, stage, &x.kinds, "BSplineSyNImageRegistrationMethod")?;
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
        let syn = SynStage {
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
            update_variance: R::ZERO,
            total_variance: R::ZERO,
            optimizer_weights,
            restored: None,
            exact_floor: self.upstream_defects.fix(),
            correct_gc_derivative: correct_gc,
            correct_stale_masks: self.upstream_defects.fix(),
            correct_point_set_transforms: self.upstream_defects.fix(),
            bspline: Some(BSplineSynSmoothing {
                order,
                update_mesh,
                total_mesh,
            }),
            outside: larmorx_registration::metric::OutsideDerivatives::new(
                !self.upstream_defects.fix(),
            ),
            n_threads: self.n_threads,
        };
        log.logger(&format!(
            "\n*** Running B-spline SyN registration (updateMeshSizeAtBaseLevel = {}, totalMeshSizeAtBaseLevel = {}) ***\n\n",
            print_array(&update_mesh),
            print_array(&total_mesh)
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
            None => run_syn(&syn, &mut observer, floor, &mut warnings, &mut events),
        };
        super::helper::outside_notices(
            &mut self.notices,
            self.upstream_defects,
            stage,
            &syn.outside,
        );
        let log = observer.log;
        for w in &warnings {
            log.cerr(w);
        }
        self.warnings.extend(warnings);
        self.point_set_notices(&events);
        match result {
            Ok(out) => {
                self.syn_notices(stage, out.identity_point_sets, out.stale_masks);
                // The output is a BSplineSmoothingOnUpdateDisplacementFieldTransform (its class
                // shows in later stages' --initialize-transforms-per-stage listings).
                let mut output = out.output;
                output.smoothing = Some(larmorx_registration::field::FieldSmoothing::BSpline {
                    order: self.transforms[stage].spline_order,
                    update_control_points: Vec::new(),
                    total_control_points: Vec::new(),
                });
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

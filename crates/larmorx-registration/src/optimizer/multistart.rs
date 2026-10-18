// SPDX-License-Identifier: Apache-2.0
//! `MultiStartOptimizerv4` (ITK v5.4.5,
//! `Modules/Numerics/Optimizersv4/include/itkMultiStartOptimizerv4.hxx`): a local optimizer
//! run from each parameter set of a list, the best (lowest) final metric value kept.
//!
//! Per start point (`ResumeOptimization`): the metric's parameters are set to it, the local
//! optimizer runs (if there is one) and its result replaces the start point in the list, and
//! the metric is evaluated once more. An ITK exception in a start point is caught and becomes
//! a warning; the metric value then stays the previous start point's (or 0 before the first),
//! which still takes part in the search for the minimum, as in ITK. At the end the metric's
//! parameters are set to the best start point's.
//!
//! ITK runs one local optimizer object for every start point, so whatever state it keeps from
//! one run to the next carries over: without a scales estimator
//! (`antsAffineInitializer`), the learning rate the golden-section line search leaves at the
//! end of one start point is where the next one begins (ITK-21). [`Corrections`] holds
//! larmorx's corrections of ITK's behaviour (D13).

use super::{ConjugateGradientLineSearch, Observer, Problem};
use crate::{ItkException, Real};

const FILE: &str = "Modules/Numerics/Optimizersv4/include/itkMultiStartOptimizerv4.hxx";

/// The events an observer of the multi-start optimizer receives (ANTs'
/// `antsCommandIterationUpdate<MultiStartOptimizerv4>`).
pub trait MultiStartObserver<R> {
    /// `StartEvent`, with `GetNumberOfIterations()` (the number of start points).
    fn start(&mut self, _number_of_iterations: usize) {}
    /// `IterationEvent`: `GetCurrentIteration()` and `GetValue()` (the current metric value).
    fn iteration(&mut self, _current_iteration: usize, _value: R) {}
    /// `EndEvent`: the best index and the list of metric values.
    fn end(&mut self, _best_index: usize, _metric_values: &[R]) {}
}

/// An observer that does nothing.
pub struct Silent;
impl<R> MultiStartObserver<R> for Silent {}

/// The local optimizer's iterations go unobserved.
struct NoLocalObserver;
impl<R: Real, P: ?Sized> Observer<R, P> for NoLocalObserver {
    fn iteration(
        &mut self,
        _state: &super::IterationState<R>,
        _number_of_iterations: &mut usize,
        _problem: &mut P,
    ) -> Result<(), ItkException> {
        Ok(())
    }
}

/// What the multi-start optimizer ends with.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiStartOutcome<R> {
    /// The list of parameters, each start point replaced by the local optimizer's result.
    pub parameters: Vec<Vec<R>>,
    /// `GetMetricValuesList()`: one value per start point that did not throw.
    pub metric_values: Vec<R>,
    /// `GetBestParametersIndex()`.
    pub best_index: usize,
    /// `m_MinimumMetricValue`.
    pub minimum: R,
    /// Whether each start point threw.
    pub failed: Vec<bool>,
    /// The exception of each start point that threw, with its number.
    pub errors: Vec<(usize, ItkException)>,
    /// The start point at which ITK crashes: after a start point whose exception leaves the
    /// metric unusable ([`leaves_metric_unusable`]), the next evaluation crashes. The loop
    /// stops there, before the end event; `parameters` and `best_index` are then partial.
    pub crashed_at: Option<usize>,
}

/// Whether an exception leaves ITK's metric unusable: `ImageBase::SetSpacing` refusing the
/// Mattes joint histogram's spacing in `BeforeThreadedExecution` leaves that histogram
/// without a buffer, and the next evaluation crashes in `FillBuffer`.
pub fn leaves_metric_unusable(e: &ItkException) -> bool {
    e.file == "Modules/Core/Common/include/itkImageBase.hxx"
}

impl<R: Real> MultiStartOutcome<R> {
    /// `GetBestParameters()`: `None` for an empty list (ITK reads past the end of its list).
    pub fn best_parameters(&self) -> Option<&[R]> {
        self.parameters.get(self.best_index).map(Vec::as_slice)
    }
}

/// larmorx's corrections of `MultiStartOptimizerv4` (D13); `Default` is ITK's behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Corrections {
    /// ITK-19: a start point that threw takes no part in the search for the minimum. ITK
    /// compares the previous start point's value (0 before the first) instead, so a first
    /// start point that throws becomes the best one unless another one's value is below 0.
    pub skip_failed: bool,
    /// ITK-21: every start point's local optimization begins at the learning rate the local
    /// optimizer has when [`optimize`] is called (its configured rate). ITK starts each one
    /// from the rate the previous start point's line search left, so each start point's
    /// result depends on the one before it; with a scales estimator the rate is estimated
    /// again at each start, and this changes nothing.
    pub restart_learning_rate: bool,
}

/// `StartOptimization()` over `parameters`. `local` is the local optimizer (`None`: each
/// start point is only evaluated). `scales` are the multi-start optimizer's own scales
/// (`SetScales`; empty when not set), which ITK only checks. `warnings` receives ITK's
/// warning text for each start point that threw. `corrections`: larmorx's corrections of
/// ITK's behaviour ([`Corrections::default`] is ITK's).
pub fn optimize<R: Real, P: Problem<R> + ?Sized, O: MultiStartObserver<R> + ?Sized>(
    parameters: Vec<Vec<R>>,
    mut local: Option<&mut ConjugateGradientLineSearch<R>>,
    scales: &[R],
    problem: &mut P,
    observer: &mut O,
    warnings: &mut Vec<String>,
    corrections: Corrections,
) -> Result<MultiStartOutcome<R>, ItkException> {
    let skip_failed = corrections.skip_failed;
    let configured_rate = local.as_deref().map(|o| o.learning_rate);
    let mut list = parameters;
    let n = list.len();
    let mut values = Vec::new();
    let mut best = 0usize;
    let mut minimum = R::MAX;
    let mut failures = Vec::new();
    let mut errors = Vec::new();
    if n == 0 {
        return Ok(MultiStartOutcome {
            parameters: list,
            metric_values: values,
            best_index: best,
            minimum,
            failed: failures,
            errors,
            crashed_at: None,
        });
    }
    // ObjectToObjectOptimizerBase::StartOptimization: the scales, if set, must match.
    let np = problem.parameters().len();
    if !scales.is_empty() {
        if scales.len() != np {
            return Err(ItkException::new(
                "Modules/Numerics/Optimizersv4/src/itkObjectToObjectOptimizerBase.cxx",
                100,
                "MultiStartOptimizerv4Template",
                format!(
                    "Size of scales ({}) must equal number of local parameters ({np}).",
                    scales.len()
                ),
            ));
        }
        for &s in scales {
            if s <= R::EPSILON {
                return Err(ItkException::new(
                    "Modules/Numerics/Optimizersv4/src/itkObjectToObjectOptimizerBase.cxx",
                    114,
                    "MultiStartOptimizerv4Template",
                    format!(
                        "m_Scales values must be > epsilon.{}",
                        crate::format::itk_array(scales)
                    ),
                ));
            }
        }
    }
    observer.start(n);
    let mut current_value = R::ZERO;
    for current in 0..n {
        let attempt = (|| -> Result<R, ItkException> {
            problem.set_parameters(&list[current])?;
            if let Some(optimizer) = local.as_deref_mut() {
                if corrections.restart_learning_rate
                    && let Some(rate) = configured_rate
                {
                    optimizer.learning_rate = rate;
                }
                optimizer.start(problem, &mut NoLocalObserver)?;
                list[current] = problem.parameters();
            }
            problem.value()
        })();
        let failed = match attempt {
            Ok(v) => {
                current_value = v;
                values.push(v);
                false
            }
            Err(e) => {
                errors.push((current, e));
                warnings.push(format!(
                    "WARNING: In {FILE}, line 187\nMultiStartOptimizerv4Template (0x0): An exception occurred in sub-optimization number {current}.  If too many of these occur, you may need to set a different set of initial parameters.\n\n"
                ));
                true
            }
        };
        failures.push(failed);
        if !(failed && skip_failed) && current_value < minimum {
            minimum = current_value;
            best = current;
        }
        observer.iteration(current, current_value);
        if failed
            && current + 1 < n
            && errors
                .last()
                .is_some_and(|(_, e)| leaves_metric_unusable(e))
        {
            return Ok(MultiStartOutcome {
                parameters: list,
                metric_values: values,
                best_index: best,
                minimum,
                failed: failures,
                errors,
                crashed_at: Some(current + 1),
            });
        }
    }
    // StopOptimization: the best parameters into the metric.
    problem.set_parameters(&list[best])?;
    observer.end(best, &values);
    Ok(MultiStartOutcome {
        parameters: list,
        metric_values: values,
        best_index: best,
        minimum,
        failed: failures,
        errors,
        crashed_at: None,
    })
}

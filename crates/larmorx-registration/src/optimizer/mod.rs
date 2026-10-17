// SPDX-License-Identifier: Apache-2.0
//! `ConjugateGradientLineSearchOptimizerv4` and its bases (ITK v5.4.5,
//! `Modules/Numerics/Optimizersv4`: `itkConjugateGradientLineSearchOptimizerv4.hxx`,
//! `itkGradientDescentLineSearchOptimizerv4.hxx`, `itkGradientDescentOptimizerv4.hxx`,
//! `itkGradientDescentOptimizerBasev4.hxx`, `itkObjectToObjectOptimizerBase.cxx`), as ANTs
//! configures it for a linear stage: line search over `[0, 2]` times the learning rate with
//! ε 0.2, the learning rate estimated from the scales at the first iteration of each level,
//! Polak–Ribière conjugation, the best parameters returned at the end.
//!
//! The optimizer is one object across a stage's levels, as in ANTs: its learning rate and
//! number of iterations carry over from one level to the next.

pub mod multistart;
pub mod scales;

use crate::convergence::WindowConvergence;
use crate::{ItkException, Real};

/// What the optimizer optimises: the metric with the moving transform.
pub trait Problem<R: Real> {
    /// `GetValueAndDerivative` at the current parameters.
    fn value_and_derivative(&mut self) -> Result<(R, Vec<R>), ItkException>;
    /// `GetValue` at the current parameters.
    fn value(&mut self) -> Result<R, ItkException>;
    /// `GetParameters()` (of the optimised transform).
    fn parameters(&self) -> Vec<R>;
    /// `SetParameters`.
    fn set_parameters(&mut self, p: &[R]) -> Result<(), ItkException>;
    /// `UpdateTransformParameters(update, 1)`.
    fn update(&mut self, update: &[R]) -> Result<(), ItkException>;
    /// `RegistrationParameterScalesFromPhysicalShift::EstimateScales`.
    fn estimate_scales(&mut self) -> Result<Vec<R>, ItkException>;
    /// `EstimateStepScale(step)`.
    fn estimate_step_scale(&mut self, step: &[R]) -> Result<R, ItkException>;
    /// `EstimateMaximumStepSize()`: the smallest virtual spacing.
    fn estimate_maximum_step(&self) -> R;
}

/// Why an optimisation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopCondition {
    MaximumNumberOfIterations,
    ConvergenceCheckerPassed,
}

/// What an iteration observer sees (`IterationEvent`).
pub struct IterationState<R> {
    /// `GetCurrentIteration()` (0 for the first iteration of a level).
    pub iteration: usize,
    /// `GetValue()`: the metric value at the start of the iteration.
    pub value: R,
    /// `GetConvergenceValue()`.
    pub convergence: R,
    /// `GetLearningRate()`: the step the line search chose.
    pub learning_rate: R,
    /// `GetGradient()`: the step just taken (the conjugate direction times the learning
    /// rate).
    pub step: Vec<R>,
}

/// The iteration observer (ANTs' `antsRegistrationOptimizerCommandIterationUpdate`): it may
/// change the number of iterations (`SetNumberOfIterations`).
pub trait Observer<R: Real, P: ?Sized> {
    fn iteration(
        &mut self,
        state: &IterationState<R>,
        number_of_iterations: &mut usize,
        problem: &mut P,
    ) -> Result<(), ItkException>;
}

/// The optimizer's settings and the state it keeps between levels.
#[derive(Clone, Debug)]
pub struct ConjugateGradientLineSearch<R> {
    pub learning_rate: R,
    pub maximum_step_size: R,
    pub number_of_iterations: usize,
    pub minimum_convergence_value: R,
    pub convergence_window_size: usize,
    pub lower_limit: R,
    pub upper_limit: R,
    pub epsilon: R,
    pub maximum_line_search_iterations: usize,
    /// `SetWeights` (`--restrict-deformation`), empty for none.
    pub weights: Vec<R>,
    /// Estimate the learning rate at each iteration (ANTs: true; only iteration 0 is
    /// estimated by the line-search optimizers anyway).
    pub estimate_learning_rate_at_each_iteration: bool,
    pub estimate_learning_rate_once: bool,
    /// Scales set with `SetScales` and no scales estimator (`antsAffineInitializer`): `None`
    /// estimates them (`RegistrationParameterScalesFromPhysicalShift`, as ANTs' registrations
    /// and `antsAI` set it). Without an estimator neither the scales nor the learning rate are
    /// estimated, so the learning rate a run ends with is the next run's first.
    pub scales: Option<Vec<R>>,
}

/// The state of one level's optimisation.
struct Run<R> {
    gradient: Vec<R>,
    conjugate: Vec<R>,
    last_gradient: Vec<R>,
    scales: Vec<R>,
    scales_identity: bool,
    weights_identity: bool,
    current_iteration: usize,
    value: R,
    convergence: R,
    best_value: R,
    best_parameters: Vec<R>,
    line_search_iterations: usize,
    monitor: WindowConvergence,
}

/// What a level's optimisation ended with.
#[derive(Clone, Debug)]
pub struct Outcome<R> {
    pub stop: StopCondition,
    pub iterations: usize,
    pub value: R,
    pub convergence: R,
}

impl<R: Real> ConjugateGradientLineSearch<R> {
    /// `StartOptimization()` for one level: scales, convergence monitor, then the iterations.
    pub fn start<P: Problem<R> + ?Sized, O: Observer<R, P> + ?Sized>(
        &mut self,
        problem: &mut P,
        observer: &mut O,
    ) -> Result<Outcome<R>, ItkException> {
        let np = problem.parameters().len();
        if self.scales.is_none()
            && self.estimate_learning_rate_once
            && self.estimate_learning_rate_at_each_iteration
        {
            return Err(ItkException::new(
                "Modules/Numerics/Optimizersv4/include/itkGradientDescentOptimizerBasev4.hxx",
                99,
                "ConjugateGradientLineSearchOptimizerv4Template",
                "Both m_DoEstimateLearningRateOnce and m_DoEstimateLearningRateAtEachIteration are enabled. Not allowed. ",
            ));
        }
        // GradientDescentOptimizerBasev4::StartOptimization: the scales and, without a
        // maximum step, its estimate; then ObjectToObjectOptimizerBase estimates the scales
        // again and checks them.
        let scales = match &self.scales {
            Some(s) => s.clone(),
            None => {
                let _ = problem.estimate_scales()?;
                if self.maximum_step_size <= R::EPSILON {
                    self.maximum_step_size = problem.estimate_maximum_step();
                }
                problem.estimate_scales()?
            }
        };
        if scales.len() != np {
            return Err(ItkException::new(
                "Modules/Numerics/Optimizersv4/src/itkObjectToObjectOptimizerBase.cxx",
                100,
                "ConjugateGradientLineSearchOptimizerv4Template",
                format!(
                    "Size of scales ({}) must equal number of local parameters ({np}).",
                    scales.len()
                ),
            ));
        }
        let mut scales_identity = true;
        for &s in &scales {
            if s <= R::EPSILON {
                return Err(ItkException::new(
                    "Modules/Numerics/Optimizersv4/src/itkObjectToObjectOptimizerBase.cxx",
                    114,
                    "ConjugateGradientLineSearchOptimizerv4Template",
                    format!(
                        "m_Scales values must be > epsilon.{}",
                        crate::format::itk_array(&scales)
                    ),
                ));
            }
        }
        for &s in &scales {
            if (R::ONE - s).abs() > R::from_f64(0.01) {
                scales_identity = false;
                break;
            }
        }
        let mut weights_identity = true;
        if !self.weights.is_empty() {
            if self.weights.len() != np {
                return Err(ItkException::new(
                    "Modules/Numerics/Optimizersv4/src/itkObjectToObjectOptimizerBase.cxx",
                    137,
                    "ConjugateGradientLineSearchOptimizerv4Template",
                    format!(
                        "Size of weights ({}) must equal number of local parameters ({np}).",
                        self.weights.len()
                    ),
                ));
            }
            for &w in &self.weights {
                if (R::ONE - w).abs() > R::from_f64(1e-4) {
                    weights_identity = false;
                    break;
                }
            }
        }
        let mut run = Run {
            gradient: vec![R::ZERO; np],
            conjugate: vec![R::ZERO; np],
            last_gradient: vec![R::ZERO; np],
            scales,
            scales_identity,
            weights_identity,
            current_iteration: 0,
            value: R::ZERO,
            convergence: R::MAX,
            best_value: R::MAX,
            best_parameters: problem.parameters(),
            line_search_iterations: 0,
            monitor: WindowConvergence::new(self.convergence_window_size),
        };
        self.resume(problem, observer, &mut run)
    }

    /// `ResumeOptimization()`.
    fn resume<P: Problem<R> + ?Sized, O: Observer<R, P> + ?Sized>(
        &mut self,
        problem: &mut P,
        observer: &mut O,
        run: &mut Run<R>,
    ) -> Result<Outcome<R>, ItkException> {
        loop {
            if run.current_iteration >= self.number_of_iterations {
                self.stop(problem, run)?;
                return Ok(Outcome {
                    stop: StopCondition::MaximumNumberOfIterations,
                    iterations: run.current_iteration,
                    value: run.value,
                    convergence: run.convergence,
                });
            }
            let (value, gradient) = problem.value_and_derivative()?;
            run.value = value;
            run.gradient = gradient;
            run.monitor.add(value.to_f64());
            run.convergence = R::from_f64(run.monitor.value());
            if run.convergence <= self.minimum_convergence_value {
                self.stop(problem, run)?;
                return Ok(Outcome {
                    stop: StopCondition::ConvergenceCheckerPassed,
                    iterations: run.current_iteration,
                    value: run.value,
                    convergence: run.convergence,
                });
            }
            self.advance(problem, observer, run)?;
            if run.value < run.best_value {
                run.best_value = run.value;
                run.best_parameters = problem.parameters();
            }
            run.current_iteration += 1;
        }
    }

    /// `StopOptimization()` (`ReturnBestParametersAndValue` on): the best parameters back
    /// into the transform.
    fn stop<P: Problem<R> + ?Sized>(
        &self,
        problem: &mut P,
        run: &mut Run<R>,
    ) -> Result<(), ItkException> {
        problem.set_parameters(&run.best_parameters)?;
        run.value = run.best_value;
        Ok(())
    }

    /// `ModifyGradientByScales()`.
    fn modify_by_scales(&self, run: &mut Run<R>) {
        if (run.scales_identity && run.weights_identity) || run.gradient.is_empty() {
            return;
        }
        let n = run.scales.len();
        let factor: Vec<R> = (0..n)
            .map(|i| {
                if run.weights_identity {
                    R::ONE / run.scales[i]
                } else {
                    self.weights[i] / run.scales[i]
                }
            })
            .collect();
        for (j, g) in run.gradient.iter_mut().enumerate() {
            *g = *g * factor[j % n];
        }
    }

    /// `EstimateLearningRate()`.
    fn estimate_learning_rate<P: Problem<R> + ?Sized>(
        &mut self,
        problem: &mut P,
        run: &Run<R>,
    ) -> Result<(), ItkException> {
        if self.scales.is_none()
            && (self.estimate_learning_rate_at_each_iteration
                || (self.estimate_learning_rate_once && run.current_iteration == 0))
        {
            let step_scale = problem.estimate_step_scale(&run.gradient)?;
            self.learning_rate = if step_scale <= R::EPSILON {
                R::ONE
            } else {
                self.maximum_step_size / step_scale
            };
        }
        Ok(())
    }

    /// `AdvanceOneStep()` of the conjugate-gradient line search.
    fn advance<P: Problem<R> + ?Sized, O: Observer<R, P> + ?Sized>(
        &mut self,
        problem: &mut P,
        observer: &mut O,
        run: &mut Run<R>,
    ) -> Result<(), ItkException> {
        self.modify_by_scales(run);
        if run.current_iteration == 0 {
            self.estimate_learning_rate(problem, run)?;
        }
        let inner = |a: &[R], b: &[R]| -> R {
            let mut s = R::ZERO;
            for (x, y) in a.iter().zip(b) {
                s = s + *x * *y;
            }
            s
        };
        let mut gamma = R::ZERO;
        let denominator = inner(&run.last_gradient, &run.last_gradient);
        if denominator > R::EPSILON {
            let difference: Vec<R> = run
                .gradient
                .iter()
                .zip(&run.last_gradient)
                .map(|(g, l)| *g - *l)
                .collect();
            gamma = inner(&difference, &run.gradient) / denominator;
        }
        if gamma < R::ZERO || gamma > R::from_f64(5.0) {
            gamma = R::ZERO;
        }
        run.last_gradient = run.gradient.clone();
        run.conjugate = run
            .gradient
            .iter()
            .zip(&run.conjugate)
            .map(|(g, c)| *g + *c * gamma)
            .collect();
        run.gradient = run.conjugate.clone();
        run.line_search_iterations = 0;
        let lr = self.learning_rate;
        self.learning_rate = self.golden_section(
            problem,
            run,
            lr * self.lower_limit,
            lr,
            lr * self.upper_limit,
            R::MAX,
        )?;
        let step: Vec<R> = run
            .gradient
            .iter()
            .map(|g| *g * self.learning_rate)
            .collect();
        run.gradient = step;
        problem.update(&run.gradient)?;
        let state = IterationState {
            iteration: run.current_iteration,
            value: run.value,
            convergence: run.convergence,
            learning_rate: self.learning_rate,
            step: run.gradient.clone(),
        };
        observer.iteration(&state, &mut self.number_of_iterations, problem)
    }

    /// `GoldenSectionSearch(a, b, c, metricb)`.
    fn golden_section<P: Problem<R> + ?Sized>(
        &mut self,
        problem: &mut P,
        run: &mut Run<R>,
        a: R,
        b: R,
        c: R,
        metricb: R,
    ) -> Result<R, ItkException> {
        let two = R::from_f64(2.0);
        if run.line_search_iterations > self.maximum_line_search_iterations {
            return Ok((c + a) / two);
        }
        run.line_search_iterations += 1;
        // ITK's literal (`m_Phi = 1.618034`), not the exact golden ratio.
        #[allow(clippy::approx_constant)]
        let phi = R::from_f64(1.618034);
        let resphi = R::from_f64(2.0) - phi;
        let x = if c - b > b - a {
            b + resphi * (c - b)
        } else {
            b - resphi * (b - a)
        };
        if (c - a).abs() < self.epsilon * (b.abs() + x.abs()) {
            return Ok((c + a) / two);
        }
        let mut metricb = metricb;
        let metricx = {
            let base_lr = self.learning_rate;
            let base_gradient = run.gradient.clone();
            let base_parameters = problem.parameters();
            self.learning_rate = x;
            let step: Vec<R> = run
                .gradient
                .iter()
                .map(|g| *g * self.learning_rate)
                .collect();
            problem.update(&step)?;
            let mx = problem.value()?;
            problem.set_parameters(&base_parameters)?;
            run.gradient = base_gradient.clone();
            if metricb == R::MAX {
                self.learning_rate = b;
                let step: Vec<R> = run
                    .gradient
                    .iter()
                    .map(|g| *g * self.learning_rate)
                    .collect();
                problem.update(&step)?;
                metricb = problem.value()?;
                problem.set_parameters(&base_parameters)?;
                run.gradient = base_gradient;
                self.learning_rate = base_lr;
            }
            mx
        };
        if metricx < metricb {
            if c - b > b - a {
                self.golden_section(problem, run, b, x, c, metricx)
            } else {
                self.golden_section(problem, run, a, x, b, metricx)
            }
        } else if c - b > b - a {
            self.golden_section(problem, run, a, b, x, metricb)
        } else if metricx == R::MAX {
            self.golden_section(problem, run, a, x, b, metricx)
        } else {
            self.golden_section(problem, run, x, b, c, metricb)
        }
    }
}

// SPDX-License-Identifier: Apache-2.0
//! `WindowConvergenceMonitoringFunction` (ITK v5.4.5,
//! `Modules/Numerics/Optimizersv4/include/itkWindowConvergenceMonitoringFunction.hxx`): the
//! optimizers' convergence value is minus the slope, at the end of the window, of a linear
//! B-spline fitted to the last `window` metric values, each divided by the sum of the
//! absolute values of **all** the values added so far.
//!
//! The fit is `BSplineScatteredDataPointSetToImageFilter` with one level, spline order 1 and
//! two control points, run on one work unit (`itkBSplineScatteredDataPointSetToImageFilter.hxx`:
//! `ThreadedGenerateDataForFitting`, `AfterThreadedGenerateData`); its `RealType` is `float`
//! and the point coordinates are `float` (`PointSet`'s default coordinate type), while the
//! data are double. The slope is `BSplineControlPointImageFunction::EvaluateGradientAtParametricPoint`
//! at 1 (`itkBSplineControlPointImageFunction.hxx`), with the end point moved inside by the
//! filter's ε, where the order-1 kernel's derivative is −1 and +1.

use std::collections::VecDeque;

/// `m_BSplineEpsilon` (`float`).
const BSPLINE_EPSILON: f32 = 1e-3;

/// The monitor of one optimisation (one level).
#[derive(Clone, Debug)]
pub struct WindowConvergence {
    window: usize,
    values: VecDeque<f64>,
    total_energy: f64,
}

impl WindowConvergence {
    /// `SetWindowSize(window)`.
    pub fn new(window: usize) -> Self {
        WindowConvergence {
            window,
            values: VecDeque::new(),
            total_energy: 0.0,
        }
    }

    /// `AddEnergyValue(value)`: the value joins the window (the oldest leaves when it is
    /// full) and its absolute value is added to the total energy, which is never reduced.
    pub fn add(&mut self, value: f64) {
        self.values.push_back(value);
        if self.values.len() > self.window {
            self.values.pop_front();
        }
        self.total_energy += value.abs();
    }

    /// `GetConvergenceValue()`: `NumericTraits<double>::max()` until the window is full.
    pub fn value(&self) -> f64 {
        if self.values.len() < self.window {
            return f64::MAX;
        }
        let w = self.window;
        // r = (spans / ((size − 1)·spacing)) and ε = r·spacing·m_BSplineEpsilon, with
        // size 11 and spacing 0.1 (double), stored in float.
        let spacing = 0.1f64;
        let r = (1.0f32 as f64 / (10.0f32 as f64 * spacing)) as f32;
        let epsilon = (f64::from(r) * spacing * f64::from(BSPLINE_EPSILON)) as f32;
        let mut omega = [0.0f32; 2];
        let mut delta = [0.0f64; 2];
        for (n, &energy) in self.values.iter().enumerate() {
            let t = n as f32 / (w - 1) as f32;
            let mut p = (f64::from(t) * f64::from(r)) as f32;
            if (p - 1.0f32).abs() <= epsilon {
                p = 1.0f32 - epsilon;
            }
            if p < 0.0 && p.abs() <= epsilon {
                p = 0.0;
            }
            // `p` is in [0, 1): one span, control points 0 and 1.
            let base = p as u32;
            let mut weights = [0.0f32; 2];
            let mut w2 = 0.0f32;
            for (j, weight) in weights.iter_mut().enumerate() {
                let u = (p - base as f32) - j as f32;
                let kernel = {
                    let a = f64::from(u).abs();
                    if a < 1.0 { 1.0 - a } else { 0.0 }
                };
                *weight = (1.0f64 * kernel) as f32;
                w2 += *weight * *weight;
            }
            let data = energy / self.total_energy;
            for j in 0..2 {
                let idx = j + base as usize;
                let t = weights[j];
                omega[idx] += t * t;
                delta[idx] += data * f64::from(t * t * t * 1.0f32 / w2);
            }
        }
        let phi: [f64; 2] = std::array::from_fn(|k| {
            if omega[k] == 0.0 {
                return 0.0;
            }
            let v = delta[k] / f64::from(omega[k]);
            if v.is_finite() { v } else { 0.0 }
        });
        // The gradient at the parametric point 1 − ε: the derivative of the order-1 kernel
        // is −1 for control point 0 and +1 for control point 1.
        let mut gradient = 0.0f64;
        gradient += -phi[0];
        gradient += phi[1];
        -gradient
    }
}

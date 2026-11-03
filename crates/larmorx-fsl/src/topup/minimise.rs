// SPDX-License-Identifier: Apache-2.0
//! The minimisers of one level (`specs/topup.md` §4.7): Levenberg-Marquardt on the
//! Gauss-Newton Hessian, and Møller's scaled conjugate gradients.
//!
//! Observed (black-box runs with `-v` and `--debug`, `docs/findings/fsl-topup-original.md`):
//! the Levenberg-Marquardt step is the preconditioned conjugate-gradient solution (from zero,
//! diagonal preconditioner, `|r| / |b| < 1e-3`) of the damped system, and FSL's fields after
//! the first step agree with it to 6e-5 Hz; the scaled conjugate gradients take one cost and two
//! gradients per accepted iteration, and their cost sequence is reproduced to 5 digits by the
//! textbook algorithm (Møller 1993, in Bishop's formulation) with `σ = 0.01` and `λ₁ = 0.1`.

use super::model::{Eval, Level, dot};
use super::stencil::pcg;

/// What a minimiser reports: each cost evaluation.
pub trait Report {
    fn evaluated(&mut self, level: &Level, e: &Eval);
}

impl<F: FnMut(&Level, &Eval)> Report for F {
    fn evaluated(&mut self, level: &Level, e: &Eval) {
        self(level, e)
    }
}

const CG_TOLERANCE: f64 = 1e-3;
const CG_MAX_ITER: usize = 500;

/// Levenberg-Marquardt from `c`, at most `miter` accepted steps.
pub fn levenberg_marquardt(
    level: &Level,
    mut c: Vec<f64>,
    miter: usize,
    report: &mut dyn Report,
) -> Vec<f64> {
    let mut ev = level.eval(&c);
    report.evaluated(level, &ev);
    if miter == 0 {
        return c;
    }
    let mut g = level.gradient(&ev, &c);
    let mut h = level.hessian(&ev);
    let mut damping = 0.1;
    let mut accepted = 0;
    loop {
        let hd = h.damped(damping);
        let b: Vec<f64> = g.iter().map(|x| -x).collect();
        let step = pcg(&hd, &b, CG_TOLERANCE, CG_MAX_ITER);
        let trial: Vec<f64> = c.iter().zip(&step).map(|(a, s)| a + s).collect();
        let et = level.eval(&trial);
        report.evaluated(level, &et);
        if et.cost < ev.cost {
            let converged = 2.0 * (et.cost - ev.cost).abs()
                <= 1e-8 * (et.cost.abs() + ev.cost.abs() + f64::EPSILON);
            c = trial;
            ev = et;
            damping /= 10.0;
            accepted += 1;
            if converged || accepted >= miter {
                break;
            }
            g = level.gradient(&ev, &c);
            h = level.hessian(&ev);
        } else {
            damping *= 10.0;
            if damping > 1e20 {
                break;
            }
        }
    }
    c
}

/// Scaled conjugate gradients from `c`, at most `miter` accepted steps.
pub fn scaled_conjugate_gradients(
    level: &Level,
    mut x: Vec<f64>,
    miter: usize,
    report: &mut dyn Report,
) -> Vec<f64> {
    const SIGMA0: f64 = 0.01;
    const BETA_MIN: f64 = 1e-15;
    const BETA_MAX: f64 = 1e100;
    let n = x.len();
    let ev = level.eval(&x);
    report.evaluated(level, &ev);
    if miter == 0 {
        return x;
    }
    let mut fold = ev.cost;
    let mut gnew = level.gradient(&ev, &x);
    let mut gold;
    let mut d: Vec<f64> = gnew.iter().map(|v| -v).collect();
    let mut success = true;
    let mut nsuccess = 0usize;
    let mut beta = 0.1;
    let mut accepted = 0;
    let (mut mu, mut kappa, mut gamma) = (0.0, 0.0, 0.0);
    while accepted < miter {
        if success {
            mu = dot(&d, &gnew);
            if mu >= 0.0 {
                d = gnew.iter().map(|v| -v).collect();
                mu = dot(&d, &gnew);
            }
            kappa = dot(&d, &d);
            if kappa < f64::EPSILON {
                break;
            }
            let sigma = SIGMA0 / kappa.sqrt();
            let xp: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + sigma * b).collect();
            let ep = level.eval(&xp);
            let gplus = level.gradient(&ep, &xp);
            let diff: Vec<f64> = gplus.iter().zip(&gnew).map(|(a, b)| a - b).collect();
            gamma = dot(&d, &diff) / sigma;
        }
        let mut delta = gamma + beta * kappa;
        if delta <= 0.0 {
            delta = beta * kappa;
            beta -= gamma / kappa;
        }
        let alpha = -mu / delta;
        let xnew: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + alpha * b).collect();
        let en = level.eval(&xnew);
        report.evaluated(level, &en);
        let fnew = en.cost;
        let ratio = 2.0 * (fnew - fold) / (alpha * mu);
        if ratio >= 0.0 && fnew.is_finite() {
            success = true;
            nsuccess += 1;
            accepted += 1;
            x = xnew;
            fold = fnew;
            gold = std::mem::take(&mut gnew);
            gnew = level.gradient(&en, &x);
            // the relative gradient test
            let scale = fnew.abs().max(1.0);
            let test = gnew
                .iter()
                .zip(&x)
                .map(|(g, p)| g.abs() * p.abs().max(1.0) / scale)
                .fold(0.0f64, f64::max);
            if test < 1e-8 {
                break;
            }
            if nsuccess == n {
                d = gnew.iter().map(|v| -v).collect();
                nsuccess = 0;
            } else {
                let diff: Vec<f64> = gold.iter().zip(&gnew).map(|(a, b)| a - b).collect();
                let g2 = dot(&diff, &gnew) / mu;
                d = d.iter().zip(&gnew).map(|(dv, gv)| g2 * dv - gv).collect();
            }
        } else {
            success = false;
        }
        if ratio < 0.25 {
            beta = (4.0 * beta).min(BETA_MAX);
        }
        if ratio > 0.75 {
            beta = (0.5 * beta).max(BETA_MIN);
        }
    }
    x
}

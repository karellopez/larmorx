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
use super::stencil::{Hessian, pcg};

/// What a minimiser reports: each cost evaluation (numbered as FSL's `--debug` files are: the
/// iteration and the attempt within it), each gradient and each Hessian (observed, findings 9).
pub trait Report {
    fn evaluated(&mut self, level: &Level, e: &Eval, iter: usize, attempt: usize);
    fn gradient(&mut self, _level: &Level, _e: &Eval, _g: &[f64], _iter: usize) {}
    fn hessian(&mut self, _level: &Level, _e: &Eval, _h: &Hessian, _iter: usize) {}
}

const CG_TOLERANCE: f64 = 1e-3;
const CG_MAX_ITER: usize = 500;

/// Levenberg-Marquardt from `c` (the coefficients and the free movement parameters), at most
/// `miter` accepted steps.
pub fn levenberg_marquardt(
    level: &Level,
    mut c: Vec<f64>,
    miter: usize,
    report: &mut dyn Report,
) -> Vec<f64> {
    let mut ev = level.eval(&c);
    report.evaluated(level, &ev, 0, 1);
    if miter == 0 {
        return c;
    }
    let mut iter = 1;
    let mut attempt = 0;
    let mut g = level.gradient(&ev, &c);
    report.gradient(level, &ev, &g, iter);
    let mut h = level.hessian(&ev);
    if level.single {
        h.to_single();
    }
    report.hessian(level, &ev, &h, iter);
    let mut damping = 0.1;
    let mut accepted = 0;
    loop {
        // a parameter the data and the regulariser leave free (a zero diagonal: λ = 0 and a
        // coefficient whose support lies outside the mask) ends the level without a step
        // (observed: FSL's default run on the half-resolution pair prints the first cost only
        // and returns a zero field)
        if h.diagonal().iter().any(|d| !(*d > 0.0)) {
            break;
        }
        let hd = h.damped(damping);
        let b: Vec<f64> = g.iter().map(|x| -x).collect();
        let step = pcg(&hd, &b, CG_TOLERANCE, CG_MAX_ITER);
        let trial: Vec<f64> = c.iter().zip(&step).map(|(a, s)| a + s).collect();
        let et = level.eval(&trial);
        attempt += 1;
        report.evaluated(level, &et, iter, attempt);
        if et.cost < ev.cost {
            let converged = 2.0 * (et.cost - ev.cost).abs()
                <= 1e-8 * (et.cost.abs() + ev.cost.abs() + f64::EPSILON);
            c = trial;
            ev = et;
            damping /= 10.0;
            accepted += 1;
            iter += 1;
            attempt = 0;
            if converged || accepted >= miter {
                break;
            }
            g = level.gradient(&ev, &c);
            report.gradient(level, &ev, &g, iter);
            h = level.hessian(&ev);
            if level.single {
                h.to_single();
            }
            report.hessian(level, &ev, &h, iter);
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
    report.evaluated(level, &ev, 0, 1);
    if miter == 0 {
        return x;
    }
    // FSL's numbering: a counter of the gradients; an evaluation takes the counter's value
    let mut counter = 1;
    let (mut last, mut attempt) = (0, 1);
    let mut fold = ev.cost;
    let mut gnew = level.gradient(&ev, &x);
    report.gradient(level, &ev, &gnew, counter);
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
            counter += 1;
            report.gradient(level, &ep, &gplus, counter);
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
        attempt = if counter == last { attempt + 1 } else { 1 };
        last = counter;
        report.evaluated(level, &en, counter, attempt);
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
            counter += 1;
            report.gradient(level, &en, &gnew, counter);
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

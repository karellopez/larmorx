// SPDX-License-Identifier: Apache-2.0
//! The minimisers of one level (`specs/topup.md` §4.7): Levenberg-Marquardt on the
//! Gauss-Newton Hessian, and Møller's scaled conjugate gradients.
//!
//! Observed (black-box runs with `-v` and `--debug`, `docs/findings/fsl-topup-original.md`):
//! the Levenberg-Marquardt step is the preconditioned conjugate-gradient solution (from zero,
//! diagonal preconditioner, `|r| / |b| < 1e-3`) of the damped system, and FSL's fields after
//! the first step agree with it to 6e-5 Hz; the scaled conjugate gradients take one cost and two
//! gradients per accepted iteration, and their cost sequence is reproduced to 5 digits by the
//! textbook algorithm (Møller 1993, in Bishop's formulation) with `σ = 0.01` and `λ₁ = 0.1`;
//! the gradient at `x + σd` takes the regulariser's weight of the current point (FSL evaluates
//! no cost there). `--miter` counts every iteration of the scaled conjugate gradients, the
//! rejected ones too, and a level whose last iteration is rejected ends at that iteration's
//! point in FSL (FSL-242; observed with `-v`, random testing's topup-trial-06854 and -11566 of
//! 2026-11-05: one SCG iteration whose step raises the cost evaluates once, and the next level
//! starts from the worse point).

use super::model::{Eval, Level, dot};
use super::stencil::{Hessian, pcg_damped};

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
        if h.diagonal().iter().any(|d| *d == 0.0 || !d.is_finite()) {
            break;
        }
        let b: Vec<f64> = g.iter().map(|x| -x).collect();
        let step = pcg_damped(&h, damping, &b, CG_TOLERANCE, CG_MAX_ITER, level.pool());
        let trial: Vec<f64> = c.iter().zip(&step).map(|(a, s)| a + s).collect();
        let et = level.eval(&trial);
        attempt += 1;
        report.evaluated(level, &et, iter, attempt);
        if et.cost < ev.cost {
            let converged = 2.0 * (et.cost - ev.cost).abs()
                <= 1e-8 * (et.cost.abs() + ev.cost.abs() + f64::EPSILON);
            c = trial;
            level.recycle(std::mem::replace(&mut ev, et));
            damping /= 10.0;
            accepted += 1;
            iter += 1;
            attempt = 0;
            if converged || accepted >= miter {
                break;
            }
            g = level.gradient(&ev, &c);
            report.gradient(level, &ev, &g, iter);
            level.pool().give(std::mem::take(&mut h.cc.data));
            h = level.hessian(&ev);
            if level.single {
                h.to_single();
            }
            report.hessian(level, &ev, &h, iter);
        } else {
            level.recycle(et);
            damping *= 10.0;
            if damping > 1e20 {
                break;
            }
        }
    }
    c
}

/// What a level of scaled conjugate gradients ended with.
#[derive(Debug)]
pub struct ScgEnd {
    /// The parameters it ends at.
    pub x: Vec<f64>,
    /// Its last iteration's step was rejected (its cost not below the current point's): FSL
    /// ends at that rejected point (FSL-242), the default at the current one; the two differ.
    pub last_rejected: bool,
}

/// Scaled conjugate gradients from `c`, `miter` iterations (each counted, accepted or not:
/// observed). When the last one is rejected, `fsl242` (`reproduce`) ends at its point, as FSL;
/// else at the current point (the lowest cost found).
pub fn scaled_conjugate_gradients(
    level: &Level,
    mut x: Vec<f64>,
    miter: usize,
    fsl242: bool,
    report: &mut dyn Report,
) -> ScgEnd {
    const SIGMA0: f64 = 0.01;
    const BETA_MIN: f64 = 1e-15;
    const BETA_MAX: f64 = 1e100;
    let n = x.len();
    let ev = level.eval(&x);
    report.evaluated(level, &ev, 0, 1);
    if miter == 0 {
        return ScgEnd {
            x,
            last_rejected: false,
        };
    }
    // FSL's numbering: a counter of the gradients; an evaluation takes the counter's value
    let mut counter = 1;
    let (mut last, mut attempt) = (0, 1);
    let mut fold = ev.cost;
    let mut cur_ssd = ev.ssd;
    let mut gnew = level.gradient(&ev, &x);
    report.gradient(level, &ev, &gnew, counter);
    let mut d: Vec<f64> = gnew.iter().map(|v| -v).collect();
    // the trial points and the gradients' difference in buffers reused across iterations (the
    // same arithmetic, element by element)
    let mut xp = vec![0.0; n];
    let mut xnew = vec![0.0; n];
    let mut diff = vec![0.0; n];
    let mut success = true;
    let mut nsuccess = 0usize;
    let mut beta = 0.1;
    let mut iterations = 0;
    let mut last_rejected = false;
    let (mut mu, mut kappa, mut gamma) = (0.0, 0.0, 0.0);
    while iterations < miter {
        iterations += 1;
        if success {
            mu = dot(&d, &gnew);
            if mu >= 0.0 {
                for (dv, g) in d.iter_mut().zip(&gnew) {
                    *dv = -g;
                }
                mu = dot(&d, &gnew);
            }
            kappa = dot(&d, &d);
            if kappa < f64::EPSILON {
                break;
            }
            let sigma = SIGMA0 / kappa.sqrt();
            for ((o, a), b) in xp.iter_mut().zip(&x).zip(&d) {
                *o = a + sigma * b;
            }
            let mut ep = level.eval(&xp);
            // the regulariser's weight λ' = λ · SSD (`--ssqlambda=1`) of the current point: FSL
            // evaluates no cost at x + σd, so its λ' stays the last cost's (observed: the first
            // step of a level whose start has Reg > 0, findings 34)
            ep.ssd = cur_ssd;
            let gplus = level.gradient(&ep, &xp);
            counter += 1;
            report.gradient(level, &ep, &gplus, counter);
            level.recycle(ep);
            for ((o, a), b) in diff.iter_mut().zip(&gplus).zip(&gnew) {
                *o = a - b;
            }
            level.pool().give(gplus);
            gamma = dot(&d, &diff) / sigma;
        }
        let mut delta = gamma + beta * kappa;
        if delta <= 0.0 {
            delta = beta * kappa;
            beta -= gamma / kappa;
        }
        let alpha = -mu / delta;
        for ((o, a), b) in xnew.iter_mut().zip(&x).zip(&d) {
            *o = a + alpha * b;
        }
        let en = level.eval(&xnew);
        attempt = if counter == last { attempt + 1 } else { 1 };
        last = counter;
        report.evaluated(level, &en, counter, attempt);
        let fnew = en.cost;
        let ratio = 2.0 * (fnew - fold) / (alpha * mu);
        if ratio >= 0.0 && fnew.is_finite() {
            success = true;
            last_rejected = false;
            nsuccess += 1;
            std::mem::swap(&mut x, &mut xnew);
            fold = fnew;
            cur_ssd = en.ssd;
            let gold = std::mem::take(&mut gnew);
            gnew = level.gradient(&en, &x);
            counter += 1;
            report.gradient(level, &en, &gnew, counter);
            level.recycle(en);
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
                for (dv, g) in d.iter_mut().zip(&gnew) {
                    *dv = -g;
                }
                nsuccess = 0;
            } else {
                for ((o, a), b) in diff.iter_mut().zip(&gold).zip(&gnew) {
                    *o = a - b;
                }
                let g2 = dot(&diff, &gnew) / mu;
                for (dv, gv) in d.iter_mut().zip(&gnew) {
                    *dv = g2 * *dv - gv;
                }
            }
            level.pool().give(gold);
        } else {
            success = false;
            last_rejected = true;
            level.recycle(en);
        }
        if ratio < 0.25 {
            beta = (4.0 * beta).min(BETA_MAX);
        }
        if ratio > 0.75 {
            beta = (0.5 * beta).max(BETA_MIN);
        }
    }
    // the loop ends after `miter` iterations (or at the gradient test, after an accepted step)
    if last_rejected && fsl242 {
        std::mem::swap(&mut x, &mut xnew);
    }
    ScgEnd { x, last_rejected }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The costs a minimiser evaluates.
    #[derive(Default)]
    struct Costs(Vec<f64>);

    impl Report for Costs {
        fn evaluated(&mut self, _level: &Level, e: &Eval, _iter: usize, _attempt: usize) {
            self.0.push(e.cost);
        }
    }

    #[test]
    fn every_scg_iteration_counts_and_a_rejected_last_one_is_fsl242() {
        // observed (FSL with -v): `--miter=k` evaluates the level's start and k steps, rejected
        // ones included; FSL ends a level whose last step is rejected at that step's point
        let (level, start) = super::super::model::tests::level(false);
        let mut rejected = 0;
        // rough starts (the helper's field scaled), where the quadratic model can fail
        for (scale, miter) in [1.0, 300.0, 3000.0, 30000.0]
            .into_iter()
            .flat_map(|s| (0..12).map(move |m| (s, m)))
        {
            let start: Vec<f64> = start.iter().map(|c| scale * c).collect();
            let mut fix = Costs::default();
            let a = scaled_conjugate_gradients(&level, start.clone(), miter, false, &mut fix);
            let mut rep = Costs::default();
            let b = scaled_conjugate_gradients(&level, start.clone(), miter, true, &mut rep);
            assert_eq!(fix.0.len(), 1 + miter, "miter {miter}");
            assert_eq!(fix.0, rep.0, "the same path in both modes");
            assert_eq!(a.last_rejected, b.last_rejected);
            let cost = |x: &[f64]| level.eval(x).cost;
            if a.last_rejected {
                rejected += 1;
                // the default keeps the lowest cost, FSL's the rejected step's (higher)
                assert!(cost(&b.x) > cost(&a.x), "miter {miter}");
                assert_eq!(cost(&b.x).to_bits(), fix.0[miter].to_bits());
            } else {
                assert_eq!(a.x, b.x, "miter {miter}");
            }
            let lowest = fix.0.iter().copied().fold(f64::INFINITY, f64::min);
            assert_eq!(cost(&a.x).to_bits(), lowest.to_bits(), "miter {miter}");
        }
        assert!(rejected > 0, "no rejected last step: the test sees nothing");
    }
}

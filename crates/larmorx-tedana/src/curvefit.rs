// SPDX-License-Identifier: Apache-2.0
//! The monoexponential curve fit (`specs/t2smap.md` §7.3, §7.4, §13.5, §16).
//!
//! tedana fits every voxel of a group with SciPy's `curve_fit` of S(TE) = S0 · exp(−TE / T2*)
//! to the voxel's n · T_u samples, bounded (S0 ≥ the smallest sample, T2* ≥ 0), so with SciPy's
//! trust-region reflective method (Branch, Coleman and Li 1999; SciPy's documentation of
//! `least_squares`), forward-difference derivatives and the tolerances 1e-8, at most 200
//! evaluations. The original runs the same method (Coleman-Li scaling, the exact trust-region
//! subproblem from the singular values of the scaled and augmented Jacobian, the step truncated
//! at the bounds, its reflection and the scaled anti-gradient, the radius rules, the three
//! tolerances and the evaluation budget, as `larmorx_optim`'s port of SciPy's method keeps them)
//! with the two changes §16 allows:
//!
//! - **the model's own derivatives** instead of forward differences (∂/∂S0 = e, ∂/∂T2* =
//!   S0 · e · TE / T2*², e = exp(−TE / T2*));
//! - **the problem reduced to the n distinct echo times**: with μ_e the voxel's mean of echo e
//!   over its T_u samples and C the samples' sum of squares about those means, the cost is
//!   ½ (T_u Σ_e (p_e − μ_e)² + C), the gradient T_u Σ_e j_e (p_e − μ_e) and JᵀJ = T_u Σ_e j_e j_eᵀ
//!   exactly, so every evaluation costs n exponentials instead of n · T_u (200 times fewer on
//!   fMRIPrep's calls), and a step's reduction of the cost is computed from the n differences
//!   without the cancellation of two large sums.
//!
//! The singular values come from one-sided Jacobi rotations of the explicit (n + 2) × 2 matrix
//! (the same values SciPy's SVD of the m + 2 rows gives, in exact arithmetic). A start is
//! refused as SciPy refuses it (§7.3's failures and their order); where SciPy's rounding would
//! raise in the middle of a fit (a truncated step a hair outside the trust region), the
//! original takes the step on the region's boundary instead: such a fit completes (§17.3's
//! criterion (b) judges it). The covariance is §7.3's, from the final Jacobian.

use larmorx_core::math;
use rayon::prelude::*;

use crate::fit::Maps;
use crate::input::Data;

/// SciPy's default tolerances (§7.3).
pub const FTOL: f64 = 1e-8;
pub const XTOL: f64 = 1e-8;
pub const GTOL: f64 = 1e-8;
/// `max_nfev`: 100 per parameter.
pub const MAX_NFEV: usize = 200;
const EPS: f64 = f64::EPSILON;

/// Why a voxel's fit failed (§7.3, in the order SciPy checks): tedana keeps the log-linear
/// values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// A sample that is not finite (`array must not contain infs or NaNs`).
    NotFinite,
    /// The start outside the bounds, or NaN (`Initial guess is outside of provided bounds`).
    Start,
    /// The residuals at the start not finite.
    Residuals,
    /// A Jacobian that is not finite (SciPy's forward differences at an infinite T2* start;
    /// its SVD refuses them).
    Jacobian,
    /// No convergence within [`MAX_NFEV`] evaluations (`Optimal parameters not found`).
    MaxNfev,
}

/// A completed fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub s0: f64,
    pub t2s: f64,
    /// The covariance P[0, 0] (S0's variance), P[1, 1] (T2*'s), P[0, 1] (§7.3).
    pub pcov: [f64; 3],
    /// SciPy's `OptimizeWarning` (the covariance could not be estimated: +∞).
    pub warn: bool,
    /// Residual evaluations and SciPy's status (1 gtol, 2 ftol, 3 xtol, 4 ftol and xtol).
    pub nfev: usize,
    pub status: i32,
}

/// A voxel's samples of one fit, reduced to what the cost needs (§16).
#[derive(Clone, Debug, Default)]
pub struct Samples {
    /// The echo times (ms) of echoes 1..n.
    pub te: Vec<f64>,
    /// Per echo, the mean of the samples.
    pub mu: Vec<f64>,
    /// Samples per echo.
    pub tu: usize,
    /// Σ_e Σ_t (y_et − μ_e)².
    pub c: f64,
    pub ymin: f64,
    pub ymax: f64,
    /// Every sample finite.
    pub finite: bool,
}

impl Samples {
    /// Fills from echoes 1..n's samples: `echo(e)` yields echo e's (float32, widened).
    pub fn fill<I: Iterator<Item = f32> + Clone>(
        &mut self,
        tes: &[f64],
        n: usize,
        echo: impl Fn(usize) -> I,
    ) {
        self.te.clear();
        self.te.extend_from_slice(&tes[..n]);
        self.mu.clear();
        self.c = 0.0;
        self.ymin = f64::INFINITY;
        self.ymax = f64::NEG_INFINITY;
        self.finite = true;
        self.tu = 0;
        for e in 0..n {
            let mut sum = 0.0f64;
            let mut k = 0usize;
            for x in echo(e) {
                let y = f64::from(x);
                if !y.is_finite() {
                    self.finite = false;
                }
                sum += y;
                self.ymin = self.ymin.min(y);
                self.ymax = self.ymax.max(y);
                k += 1;
            }
            self.tu = k;
            self.mu.push(if k > 0 { sum / k as f64 } else { 0.0 });
        }
        if !self.finite {
            return;
        }
        for e in 0..n {
            let mu = self.mu[e];
            self.c += echo(e)
                .map(|x| {
                    let d = f64::from(x) - mu;
                    d * d
                })
                .sum::<f64>();
        }
    }

    fn n(&self) -> usize {
        self.te.len()
    }

    /// The number of residuals, m = n · T_u.
    pub fn m(&self) -> usize {
        self.n() * self.tu
    }
}

/// Scratch buffers of one thread.
#[derive(Clone, Debug, Default)]
pub struct Work {
    /// At the current point: e_k = exp(−TE_k / T2*), the reduced residuals p_k − μ_k, ∂p_k/∂T2*.
    e: Vec<f64>,
    r: Vec<f64>,
    j1: Vec<f64>,
    /// At a trial point.
    en: Vec<f64>,
    rn: Vec<f64>,
    /// The columns of a matrix being rotated.
    ca: Vec<f64>,
    cb: Vec<f64>,
}

impl Work {
    fn size(&mut self, n: usize) {
        for v in [
            &mut self.e,
            &mut self.r,
            &mut self.j1,
            &mut self.en,
            &mut self.rn,
        ] {
            v.clear();
            v.resize(n, 0.0);
        }
    }
}

/// The model at `x` (§7.3): e_k and the reduced residuals into `e`, `r`; false when a residual
/// of a sample would not be finite (p − y is monotone in y, so its extremes decide).
fn evaluate(s: &Samples, x: [f64; 2], exp: fn(f64) -> f64, e: &mut [f64], r: &mut [f64]) -> bool {
    let mut ok = true;
    for k in 0..s.n() {
        let ek = exp(-s.te[k] / x[1]);
        let p = x[0] * ek;
        e[k] = ek;
        r[k] = p - s.mu[k];
        ok &= p.is_finite() && (p - s.ymin).is_finite() && (p - s.ymax).is_finite();
    }
    ok
}

/// ∂p_k/∂T2* at `x` (0 where the exponential underflows), false when one is not finite.
fn derivative(s: &Samples, x: [f64; 2], e: &[f64], j1: &mut [f64]) -> bool {
    let mut ok = true;
    for k in 0..s.n() {
        j1[k] = if e[k] == 0.0 {
            0.0
        } else {
            x[0] * e[k] * (s.te[k] / x[1]) / x[1]
        };
        ok &= j1[k].is_finite() && e[k].is_finite();
    }
    ok
}

fn norm(v: [f64; 2]) -> f64 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// numpy's `maximum` and `minimum` (NaN propagates).
fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn np_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// Python's `max(a, b)` and `min(a, b)` of two numbers (the first unless the second compares
/// greater, or less).
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The bounds: lb = (the smallest sample, 0), ub = +∞ (§7.3).
#[derive(Clone, Copy, Debug)]
struct Bounds {
    lb: [f64; 2],
}

impl Bounds {
    fn contains(&self, x: [f64; 2]) -> bool {
        (0..2).all(|i| x[i] >= self.lb[i] && x[i] <= f64::INFINITY)
    }

    /// Coleman-Li's scaling vector v and its derivative's sign dv (the upper bounds are
    /// infinite, so only g > 0 scales).
    fn cl_scaling(&self, x: [f64; 2], g: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        let mut v = [1.0; 2];
        let mut dv = [0.0; 2];
        for i in 0..2 {
            if g[i] > 0.0 {
                v[i] = x[i] - self.lb[i];
                dv[i] = 1.0;
            }
        }
        (v, dv)
    }

    /// A point strictly inside: with `rstep` > 0 one within rstep · max(1, |lb|) of a lower
    /// bound moves to that distance; with 0, one on or below it moves to the next number above.
    fn strictly_feasible(&self, x: [f64; 2], rstep: f64) -> [f64; 2] {
        let mut out = x;
        for i in 0..2 {
            let lb = self.lb[i];
            if rstep == 0.0 {
                if x[i] <= lb {
                    out[i] = lb.next_up();
                }
            } else {
                let threshold = rstep * py_max(1.0, lb.abs());
                let lower = x[i] - lb;
                let upper = f64::INFINITY - x[i];
                if lower <= np_min(upper, threshold) {
                    out[i] = lb + threshold;
                }
            }
        }
        out
    }

    /// The step length along `s` from `x` to the nearest bound, and which bounds it hits.
    fn step_to_bound(&self, x: [f64; 2], s: [f64; 2]) -> (f64, [i8; 2]) {
        let mut steps = [f64::INFINITY; 2];
        for i in 0..2 {
            if s[i] != 0.0 {
                steps[i] = np_max((self.lb[i] - x[i]) / s[i], (f64::INFINITY - x[i]) / s[i]);
            }
        }
        // numpy's `min` (a NaN wins)
        let min = if steps[0].is_nan() || steps[1].is_nan() {
            f64::NAN
        } else {
            steps[0].min(steps[1])
        };
        let mut hits = [0i8; 2];
        for i in 0..2 {
            if steps[i] == min {
                hits[i] = if s[i] > 0.0 {
                    1
                } else if s[i] < 0.0 {
                    -1
                } else {
                    0
                };
            }
        }
        (min, hits)
    }
}

/// The parameters t at which ‖x + t s‖ = Δ, ascending; a zero direction gives (0, 0), a point
/// a rounding outside the region is taken as on its boundary.
fn intersect_trust_region(x: [f64; 2], s: [f64; 2], delta: f64) -> (f64, f64) {
    let a = dot(s, s);
    if a == 0.0 {
        return (0.0, 0.0);
    }
    let b = dot(x, s);
    let c = (dot(x, x) - delta * delta).min(0.0);
    let d = (b * b - a * c).sqrt();
    let q = -(b + d.copysign(b));
    let t1 = q / a;
    let t2 = c / q;
    if t1 < t2 { (t1, t2) } else { (t2, t1) }
}

/// The minimum of a t² + b t + c over [lb, ub] among the ends and the interior extremum.
fn minimize_quadratic_1d(a: f64, b: f64, lb: f64, ub: f64, c: f64) -> (f64, f64) {
    let mut ts = [lb, ub, f64::NAN];
    let mut k = 2;
    if a != 0.0 {
        let extremum = -0.5 * b / a;
        if lb < extremum && extremum < ub {
            ts[2] = extremum;
            k = 3;
        }
    }
    // numpy's argmin: the first minimum, a NaN first
    let mut best = 0;
    let mut best_y = ts[0] * (a * ts[0] + b) + c;
    for (i, &t) in ts.iter().enumerate().take(k).skip(1) {
        let y = t * (a * t + b) + c;
        if !best_y.is_nan() && (y.is_nan() || y < best_y) {
            best = i;
            best_y = y;
        }
    }
    (ts[best], best_y)
}

/// The radius update and the ratio of the actual to the predicted reduction.
fn update_tr_radius(
    delta: f64,
    actual: f64,
    predicted: f64,
    step_norm: f64,
    bound_hit: bool,
) -> (f64, f64) {
    let ratio = if predicted > 0.0 {
        actual / predicted
    } else if predicted == actual && actual == 0.0 {
        1.0
    } else {
        0.0
    };
    let delta = if ratio < 0.25 {
        0.25 * step_norm
    } else if ratio > 0.75 && bound_hit {
        delta * 2.0
    } else {
        delta
    };
    (delta, ratio)
}

/// SciPy's status for a step: 2 ftol, 3 xtol, 4 both.
fn check_termination(df: f64, f: f64, dx_norm: f64, x_norm: f64, ratio: f64) -> Option<i32> {
    let ftol_ok = df < FTOL * f && ratio > 0.25;
    let xtol_ok = dx_norm < XTOL * (XTOL + x_norm);
    match (ftol_ok, xtol_ok) {
        (true, true) => Some(4),
        (true, false) => Some(2),
        (false, true) => Some(3),
        (false, false) => None,
    }
}

/// The singular values (descending) and the right singular vectors (`v[i][j]`: row i, column
/// j) of the k × 2 matrix of columns `a`, `b`, by one-sided Jacobi rotations (columns rotated
/// in place).
fn svd2(a: &mut [f64], b: &mut [f64]) -> ([f64; 2], [[f64; 2]; 2]) {
    let mut v = [[1.0, 0.0], [0.0, 1.0]];
    let sums = |a: &[f64], b: &[f64]| {
        let (mut aa, mut bb, mut ab) = (0.0f64, 0.0f64, 0.0f64);
        for (x, y) in a.iter().zip(b.iter()) {
            aa += x * x;
            bb += y * y;
            ab += x * y;
        }
        (aa, bb, ab)
    };
    let (mut alpha, mut beta, mut gamma) = sums(a, b);
    for _ in 0..4 {
        if gamma == 0.0 || gamma.abs() <= EPS * alpha.sqrt() * beta.sqrt() || !gamma.is_finite() {
            break;
        }
        let zeta = (beta - alpha) / (2.0 * gamma);
        let t = zeta.signum() / (zeta.abs() + 1.0f64.hypot(zeta));
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = c * t;
        for (x, y) in a.iter_mut().zip(b.iter_mut()) {
            let (p, q) = (*x, *y);
            *x = c * p - s * q;
            *y = s * p + c * q;
        }
        for row in v.iter_mut() {
            let (p, q) = (row[0], row[1]);
            row[0] = c * p - s * q;
            row[1] = s * p + c * q;
        }
        (alpha, beta, gamma) = sums(a, b);
    }
    let (s0, s1) = (alpha.sqrt(), beta.sqrt());
    if s1 > s0 {
        ([s1, s0], [[v[0][1], v[0][0]], [v[1][1], v[1][0]]])
    } else {
        ([s0, s1], v)
    }
}

/// One iteration's quadratic model: the scaled Jacobian's rows (n of them, each standing for
/// T_u equal rows), the diagonal term and the scaled gradient.
struct Model<'a> {
    e: &'a [f64],
    j1: &'a [f64],
    tu: f64,
    d: [f64; 2],
    diag_h: [f64; 2],
    g_h: [f64; 2],
}

impl Model<'_> {
    /// (J_h s)_k: one row of the scaled Jacobian times s.
    fn row(&self, k: usize, s: [f64; 2]) -> f64 {
        self.d[0] * self.e[k] * s[0] + self.d[1] * self.j1[k] * s[1]
    }

    /// ‖J_h s‖².
    fn jh_sq(&self, s: [f64; 2]) -> f64 {
        self.tu
            * (0..self.e.len())
                .map(|k| {
                    let r = self.row(k, s);
                    r * r
                })
                .sum::<f64>()
    }

    /// (J_h u) · (J_h v).
    fn jh_dot(&self, u: [f64; 2], v: [f64; 2]) -> f64 {
        self.tu
            * (0..self.e.len())
                .map(|k| self.row(k, u) * self.row(k, v))
                .sum::<f64>()
    }

    fn diag_dot(&self, a: [f64; 2], b: [f64; 2]) -> f64 {
        a[0] * self.diag_h[0] * b[0] + a[1] * self.diag_h[1] * b[1]
    }

    /// ½ sᵀ(J_hᵀJ_h + diag) s + g_h · s.
    fn evaluate_quadratic(&self, s: [f64; 2]) -> f64 {
        0.5 * (self.jh_sq(s) + self.diag_dot(s, s)) + dot(s, self.g_h)
    }

    /// The coefficients (a, b, c) of the quadratic along s from s0 (c = 0 without s0).
    fn quadratic_1d(&self, s: [f64; 2], s0: Option<[f64; 2]>) -> (f64, f64, f64) {
        let a = 0.5 * (self.jh_sq(s) + self.diag_dot(s, s));
        let mut b = dot(self.g_h, s);
        let Some(s0) = s0 else {
            return (a, b, 0.0);
        };
        b += self.jh_dot(s0, s);
        let mut c = 0.5 * self.jh_sq(s0) + dot(self.g_h, s0);
        b += self.diag_dot(s0, s);
        c += 0.5 * self.diag_dot(s0, s0);
        (a, b, c)
    }
}

/// The trust-region subproblem from the singular values (§7.3's exact solver): the step in
/// the scaled variables and the Levenberg-Marquardt parameter.
fn solve_trust_region(
    m: usize,
    suf: [f64; 2],
    s: [f64; 2],
    v: &[[f64; 2]; 2],
    delta: f64,
    initial_alpha: f64,
) -> ([f64; 2], f64) {
    let v_dot = |w: [f64; 2]| -> [f64; 2] {
        [
            v[0][0] * w[0] + v[0][1] * w[1],
            v[1][0] * w[0] + v[1][1] * w[1],
        ]
    };
    let phi_and_derivative = |alpha: f64| {
        let denom = [s[0] * s[0] + alpha, s[1] * s[1] + alpha];
        let p_norm = norm([suf[0] / denom[0], suf[1] / denom[1]]);
        let phi = p_norm - delta;
        let q = suf[0] * suf[0] / (denom[0] * denom[0] * denom[0])
            + suf[1] * suf[1] / (denom[1] * denom[1] * denom[1]);
        (phi, -q / p_norm)
    };
    let full_rank = s[1] > EPS * m as f64 * s[0];
    if full_rank {
        let q = v_dot([suf[0] / (s[0] * s[0]), suf[1] / (s[1] * s[1])]);
        let p = [-q[0], -q[1]];
        if norm(p) <= delta {
            return (p, 0.0);
        }
    }
    let mut alpha_upper = norm(suf) / delta;
    let mut alpha_lower = if full_rank {
        let (phi, phi_prime) = phi_and_derivative(0.0);
        -phi / phi_prime
    } else {
        0.0
    };
    let reset = |lower: f64, upper: f64| py_max(0.001 * upper, (lower * upper).sqrt());
    let mut alpha = if !full_rank && initial_alpha == 0.0 {
        reset(alpha_lower, alpha_upper)
    } else {
        initial_alpha
    };
    for _ in 0..10 {
        if alpha < alpha_lower || alpha > alpha_upper {
            alpha = reset(alpha_lower, alpha_upper);
        }
        let (phi, phi_prime) = phi_and_derivative(alpha);
        if phi < 0.0 {
            alpha_upper = alpha;
        }
        let ratio = phi / phi_prime;
        alpha_lower = py_max(alpha_lower, alpha - ratio);
        alpha -= (phi + delta) * ratio / delta;
        if phi.abs() < 0.01 * delta {
            break;
        }
    }
    let q = v_dot([
        suf[0] / (s[0] * s[0] + alpha),
        suf[1] / (s[1] * s[1] + alpha),
    ]);
    let p = [-q[0], -q[1]];
    let k = delta / norm(p);
    ([p[0] * k, p[1] * k], alpha)
}

/// The step among the truncated trust-region step, its reflection at the bound and the scaled
/// anti-gradient: (step, step in the scaled variables, predicted reduction).
fn select_step(
    bounds: &Bounds,
    model: &Model<'_>,
    x: [f64; 2],
    mut p: [f64; 2],
    mut p_h: [f64; 2],
    delta: f64,
    theta: f64,
) -> ([f64; 2], [f64; 2], f64) {
    let d = model.d;
    if bounds.contains([x[0] + p[0], x[1] + p[1]]) {
        return (p, p_h, -model.evaluate_quadratic(p_h));
    }
    let (p_stride, hits) = bounds.step_to_bound(x, p);
    let mut r_h = p_h;
    for i in 0..2 {
        if hits[i] != 0 {
            r_h[i] = -r_h[i];
        }
    }
    let mut r = [d[0] * r_h[0], d[1] * r_h[1]];
    p = [p[0] * p_stride, p[1] * p_stride];
    p_h = [p_h[0] * p_stride, p_h[1] * p_stride];
    let x_on_bound = [x[0] + p[0], x[1] + p[1]];
    let (_, to_tr) = intersect_trust_region(p_h, r_h, delta);
    let (to_bound, _) = bounds.step_to_bound(x_on_bound, r);
    let r_stride = py_min(to_bound, to_tr);
    let (r_stride_l, r_stride_u) = if r_stride > 0.0 {
        let u = if r_stride == to_bound {
            theta * to_bound
        } else {
            to_tr
        };
        ((1.0 - theta) * p_stride / r_stride, u)
    } else {
        (0.0, -1.0)
    };
    let r_value = if r_stride_l <= r_stride_u {
        let (a, b, c) = model.quadratic_1d(r_h, Some(p_h));
        let (rs, rv) = minimize_quadratic_1d(a, b, r_stride_l, r_stride_u, c);
        r_h = [r_h[0] * rs + p_h[0], r_h[1] * rs + p_h[1]];
        r = [r_h[0] * d[0], r_h[1] * d[1]];
        rv
    } else {
        f64::INFINITY
    };
    p = [p[0] * theta, p[1] * theta];
    p_h = [p_h[0] * theta, p_h[1] * theta];
    let p_value = model.evaluate_quadratic(p_h);
    let ag_h = [-model.g_h[0], -model.g_h[1]];
    let ag = [d[0] * ag_h[0], d[1] * ag_h[1]];
    let to_tr = delta / norm(ag_h);
    let (to_bound, _) = bounds.step_to_bound(x, ag);
    let ag_stride = if to_bound < to_tr {
        theta * to_bound
    } else {
        to_tr
    };
    let (a, b, _) = model.quadratic_1d(ag_h, None);
    let (ag_stride, ag_value) = minimize_quadratic_1d(a, b, 0.0, ag_stride, 0.0);
    let ag_h = [ag_h[0] * ag_stride, ag_h[1] * ag_stride];
    let ag = [ag[0] * ag_stride, ag[1] * ag_stride];
    if p_value < r_value && p_value < ag_value {
        (p, p_h, -p_value)
    } else if r_value < p_value && r_value < ag_value {
        (r, r_h, -r_value)
    } else {
        (ag, ag_h, -ag_value)
    }
}

/// The gradient Jᵀr at the current point.
fn gradient(s: &Samples, e: &[f64], j1: &[f64], r: &[f64]) -> [f64; 2] {
    let tu = s.tu as f64;
    let (mut g0, mut g1) = (0.0f64, 0.0f64);
    for k in 0..s.n() {
        g0 += e[k] * r[k];
        g1 += j1[k] * r[k];
    }
    [tu * g0, tu * g1]
}

/// Half the sum of squared residuals.
fn half_cost(s: &Samples, r: &[f64]) -> f64 {
    0.5 * (s.tu as f64 * r.iter().map(|x| x * x).sum::<f64>() + s.c)
}

/// One voxel's bounded fit from `start` = (S0, T2*), with `exp` the model's exponential
/// (§13.3: correctly rounded by default, glibc 2.35's in `reproduce`).
pub fn fit(
    s: &Samples,
    start: [f64; 2],
    exp: fn(f64) -> f64,
    w: &mut Work,
) -> Result<Fit, Failure> {
    if !s.finite {
        return Err(Failure::NotFinite);
    }
    let n = s.n();
    let m = s.m();
    let bounds = Bounds { lb: [s.ymin, 0.0] };
    // (the bounds are strictly ordered: the smallest sample is finite)
    if !bounds.contains(start) {
        return Err(Failure::Start);
    }
    let x0 = bounds.strictly_feasible(start, 1e-10);
    w.size(n);
    if !evaluate(s, x0, exp, &mut w.e, &mut w.r) {
        return Err(Failure::Residuals);
    }
    if !x0[1].is_finite() || !derivative(s, x0, &w.e, &mut w.j1) {
        return Err(Failure::Jacobian);
    }
    let tu = s.tu as f64;
    let mut x = x0;
    let mut nfev = 1usize;
    let mut cost = half_cost(s, &w.r);
    let mut g = gradient(s, &w.e, &w.j1, &w.r);
    let (v0, _) = bounds.cl_scaling(x, g);
    let mut delta = norm([x0[0] / v0[0].sqrt(), x0[1] / v0[1].sqrt()]);
    if delta == 0.0 {
        delta = 1.0;
    }
    let mut alpha = 0.0f64;
    let mut status: Option<i32> = None;
    loop {
        let (v, dv) = bounds.cl_scaling(x, g);
        let g_norm = np_max((g[0] * v[0]).abs(), (g[1] * v[1]).abs());
        if g_norm < GTOL {
            status = Some(1);
        }
        if status.is_some() || nfev == MAX_NFEV {
            break;
        }
        let d = [v[0].sqrt(), v[1].sqrt()];
        let diag_h = [g[0] * dv[0], g[1] * dv[1]];
        let g_h = [d[0] * g[0], d[1] * g[1]];
        // the singular values of [J·diag(d); diag(√diag_h)]
        let st = tu.sqrt();
        w.ca.clear();
        w.cb.clear();
        for k in 0..n {
            w.ca.push(st * d[0] * w.e[k]);
            w.cb.push(st * d[1] * w.j1[k]);
        }
        w.ca.extend([diag_h[0].sqrt(), 0.0]);
        w.cb.extend([0.0, diag_h[1].sqrt()]);
        let (sv, vv) = svd2(&mut w.ca, &mut w.cb);
        let suf = [
            vv[0][0] * g_h[0] + vv[1][0] * g_h[1],
            vv[0][1] * g_h[0] + vv[1][1] * g_h[1],
        ];
        let theta = py_max(0.995, 1.0 - g_norm);
        let model = Model {
            e: &w.e,
            j1: &w.j1,
            tu,
            d,
            diag_h,
            g_h,
        };
        let mut actual = -1.0f64;
        let mut x_new = x;
        let mut cost_new = cost;
        while actual <= 0.0 && nfev < MAX_NFEV {
            let (p_h, new_alpha) = solve_trust_region(m, suf, sv, &vv, delta, alpha);
            alpha = new_alpha;
            let p = [d[0] * p_h[0], d[1] * p_h[1]];
            let (step, step_h, predicted) = select_step(&bounds, &model, x, p, p_h, delta, theta);
            x_new = bounds.strictly_feasible([x[0] + step[0], x[1] + step[1]], 0.0);
            let finite = evaluate(s, x_new, exp, &mut w.en, &mut w.rn);
            nfev += 1;
            let step_h_norm = norm(step_h);
            if !finite {
                delta = 0.25 * step_h_norm;
                continue;
            }
            cost_new = half_cost(s, &w.rn);
            // the reduction from the n residual differences (the within-echo sum cancels)
            actual = 0.5
                * tu
                * w.r
                    .iter()
                    .zip(&w.rn)
                    .map(|(a, b)| (a - b) * (a + b))
                    .sum::<f64>();
            let (delta_new, ratio) = update_tr_radius(
                delta,
                actual,
                predicted,
                step_h_norm,
                step_h_norm > 0.95 * delta,
            );
            status = check_termination(actual, cost, norm(step), norm(x), ratio);
            if status.is_some() {
                break;
            }
            alpha *= delta / delta_new;
            delta = delta_new;
        }
        if actual > 0.0 {
            x = x_new;
            std::mem::swap(&mut w.e, &mut w.en);
            std::mem::swap(&mut w.r, &mut w.rn);
            cost = cost_new;
            if !derivative(s, x, &w.e, &mut w.j1) {
                return Err(Failure::Jacobian);
            }
            g = gradient(s, &w.e, &w.j1, &w.r);
        }
    }
    let Some(status) = status else {
        return Err(Failure::MaxNfev);
    };
    // the covariance (§7.3): the pseudo-inverse of JᵀJ from the final Jacobian's SVD
    let st = tu.sqrt();
    w.ca.clear();
    w.cb.clear();
    for k in 0..n {
        w.ca.push(st * w.e[k]);
        w.cb.push(st * w.j1[k]);
    }
    let (sv, vv) = svd2(&mut w.ca, &mut w.cb);
    let threshold = EPS * m.max(2) as f64 * sv[0];
    let mut p = [0.0f64; 3];
    for (k, &sk) in sv.iter().enumerate() {
        if sk > threshold {
            let s2 = sk * sk;
            p[0] += vv[0][k] / s2 * vv[0][k];
            p[1] += vv[1][k] / s2 * vv[1][k];
            p[2] += vv[0][k] / s2 * vv[1][k];
        }
    }
    let mut warn = false;
    if p.iter().any(|x| x.is_nan()) || m <= 2 {
        p = [f64::INFINITY; 3];
        warn = true;
    } else {
        let s_sq = 2.0 * cost / (m - 2) as f64;
        for x in p.iter_mut() {
            *x *= s_sq;
        }
    }
    Ok(Fit {
        s0: x[0],
        t2s: x[1],
        pcov: p,
        warn,
        nfev,
        status,
    })
}

/// The exponential of the model (§13.3).
pub fn model_exp(reproduce: bool) -> fn(f64) -> f64 {
    if reproduce {
        math::exp_glibc
    } else {
        math::exp
    }
}

/// The curve fit's results beside the maps (§7.5), `[v * cols + t]` over the mask's voxels.
#[derive(Clone, Debug)]
pub struct Curve {
    /// 1 where the fit failed (every fit voxel, count-1 voxels included: TED-08 decides what
    /// is written).
    pub failed: Vec<u8>,
    /// P[0, 0], P[1, 1], P[0, 1]; 0 where the fit failed or no fit ran.
    pub pcov: Vec<[f64; 3]>,
    /// The fit raised SciPy's `OptimizeWarning`.
    pub warn: Vec<bool>,
}

/// The curve fit of every fit voxel (§7.3; every volume's own in `ts`, §7.4) from the
/// log-linear `maps`, which receive the fitted S0 and T2* (their `s0_other` NaN: no longer a
/// log-linear value). Voxels are independent: fitted in parallel, in fixed chunks.
pub fn run(
    data: &Data,
    count: &[u8],
    tes: &[f64],
    volumes: Option<&[usize]>,
    maps: &mut Maps,
    reproduce: bool,
) -> Curve {
    let cols = maps.cols;
    // `ts` fits each volume alone (its columns are the volumes; with one volume both modes read
    // the same single sample per echo)
    let ts = cols > 1;
    let n_vox = data.n_vox;
    let mut curve = Curve {
        failed: vec![0; n_vox * cols],
        pcov: vec![[0.0; 3]; n_vox * cols],
        warn: vec![false; n_vox * cols],
    };
    let exp = model_exp(reproduce);
    (
        maps.t2s.par_chunks_mut(cols),
        maps.s0.par_chunks_mut(cols),
        maps.s0_other.par_chunks_mut(cols),
        curve.failed.par_chunks_mut(cols),
        curve.pcov.par_chunks_mut(cols),
        curve.warn.par_chunks_mut(cols),
    )
        .into_par_iter()
        .enumerate()
        .with_min_len(64)
        .for_each_init(
            || (Samples::default(), Work::default()),
            |(samples, work), (v, (t2s, s0, s0_other, failed, pcov, warn))| {
                if count[v] == 0 {
                    return;
                }
                let n = usize::from(count[v]).max(2);
                for t in 0..cols {
                    if ts {
                        samples.fill(tes, n, |e| std::iter::once(data.series(v, e)[t]));
                    } else {
                        match volumes {
                            None => samples.fill(tes, n, |e| data.series(v, e).iter().copied()),
                            Some(vols) => samples.fill(tes, n, |e| {
                                let s = data.series(v, e);
                                vols.iter().map(move |&tt| s[tt])
                            }),
                        }
                    }
                    match fit(samples, [s0[t], t2s[t]], exp, work) {
                        Ok(f) => {
                            s0[t] = f.s0;
                            t2s[t] = f.t2s;
                            s0_other[t] = f64::NAN;
                            pcov[t] = f.pcov;
                            warn[t] = f.warn;
                        }
                        Err(_) => failed[t] = 1,
                    }
                }
            },
        );
    curve
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(tes: &[f64], y: &[Vec<f32>]) -> Samples {
        let mut s = Samples::default();
        s.fill(tes, tes.len(), |e| y[e].iter().copied());
        s
    }

    #[test]
    fn exact_decay_is_recovered() {
        let tes = [14.0, 29.96, 45.92];
        let y: Vec<Vec<f32>> = tes
            .iter()
            .map(|&t| vec![(1000.0 * (-t / 31.0f64).exp()) as f32; 5])
            .collect();
        let s = samples(&tes, &y);
        let f = fit(&s, [900.0, 25.0], math::exp, &mut Work::default()).expect("a fit");
        assert!(
            (f.s0 - 1000.0).abs() < 1e-3 && (f.t2s - 31.0).abs() < 1e-4,
            "{f:?}"
        );
        assert!(!f.warn && f.status > 0);
    }

    #[test]
    fn starts_as_scipy_refuses_them() {
        let tes = [14.0, 29.96, 45.92];
        let y: Vec<Vec<f32>> = vec![vec![900.0, 905.0], vec![560.0, 555.0], vec![300.0, 310.0]];
        let s = samples(&tes, &y);
        let w = &mut Work::default();
        assert_eq!(
            fit(&s, [1000.0, f64::INFINITY], math::exp, w),
            Err(Failure::Jacobian)
        );
        assert_eq!(
            fit(&s, [1000.0, f64::NEG_INFINITY], math::exp, w),
            Err(Failure::Start)
        );
        assert_eq!(
            fit(&s, [1000.0, f64::NAN], math::exp, w),
            Err(Failure::Start)
        );
        assert_eq!(fit(&s, [f64::NAN, 30.0], math::exp, w), Err(Failure::Start));
        assert_eq!(fit(&s, [299.0, 30.0], math::exp, w), Err(Failure::Start));
        assert_eq!(
            fit(&s, [f64::INFINITY, 30.0], math::exp, w),
            Err(Failure::Residuals)
        );
        // a start of 0 (or −0) is moved to 1e-10 ms, where every derivative is 0: done at once
        for z in [0.0, -0.0, 1e-300] {
            let f = fit(&s, [1000.0, z], math::exp, w).expect("a fit");
            assert_eq!((f.s0, f.t2s, f.status), (1000.0, 1e-10, 1));
            assert_eq!(f.pcov, [0.0; 3]);
        }
        let mut bad = y.clone();
        bad[1][0] = f32::INFINITY;
        assert_eq!(
            fit(&samples(&tes, &bad), [1000.0, 30.0], math::exp, w),
            Err(Failure::NotFinite)
        );
    }

    #[test]
    fn two_samples_warn() {
        let tes = [14.0, 29.96];
        let y: Vec<Vec<f32>> = vec![vec![900.0], vec![560.0]];
        let f = fit(
            &samples(&tes, &y),
            [950.0, 25.0],
            math::exp,
            &mut Work::default(),
        )
        .expect("a fit");
        assert!(f.warn && f.pcov == [f64::INFINITY; 3]);
    }

    /// A seeded generator (xorshift64*) and Gaussian noise (Box-Muller).
    struct Rng(u64);

    impl Rng {
        fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        }

        fn normal(&mut self) -> f64 {
            let (u, v) = (self.uniform(), self.uniform());
            (-2.0 * math::log(u)).sqrt() * math::cos(std::f64::consts::TAU * v)
        }
    }

    /// The log-linear start (§7.2) in closed form.
    fn loglin(tes: &[f64], y: &[Vec<f32>]) -> [f64; 2] {
        let (mut sx, mut sy, mut sxx, mut sxy, mut m) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for (e, ys) in y.iter().enumerate() {
            for &v in ys {
                let (u, l) = (-tes[e], f64::from(math::logf(v.abs() + 1.0)));
                sx += u;
                sy += l;
                sxx += u * u;
                sxy += u * l;
                m += 1.0;
            }
        }
        let b1 = (sxy - sx * sy / m) / (sxx - sx * sx / m);
        let b0 = (sy - b1 * sx) / m;
        [math::exp(b0), 1.0 / b1]
    }

    /// The original's fit against SciPy's `curve_fit` (its bit-exact port, SciPy 1.18.1, forward
    /// differences) on noisy random decays, judged by §17.3's curve-fit row: every voxel by (a),
    /// or (b) the original's objective at least as good, or (c) a start at the feasibility
    /// boundary; (b) or (c) for at most 3 % of the voxels; the same failures.
    #[test]
    fn agrees_with_scipy_within_the_thresholds() {
        use larmorx_core::defects::UpstreamDefects;
        use larmorx_core::math::Libm;
        let libm = Libm::quiet(UpstreamDefects::Reproduce);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut w = Work::default();
        let (mut n_a, mut n_b, mut n_c, mut n_fail, mut n_vox) = (0, 0, 0, 0, 0);
        for case in 0..1500 {
            let (tes, tu): (Vec<f64>, usize) = match case % 3 {
                0 => (vec![14.0, 29.96, 45.92], 20),
                1 => (vec![12.0, 28.0, 44.0, 60.0], 8),
                _ => (vec![13.7, 30.0], 1 + case % 5),
            };
            let s0 = 200.0 + 2800.0 * rng.uniform();
            let t2s = 10.0 + 110.0 * rng.uniform();
            let sd = s0 * (0.005 + 0.3 * rng.uniform());
            // a few rising and flat voxels
            let rate = if case % 17 == 0 { -0.004 } else { 1.0 / t2s };
            let y: Vec<Vec<f32>> = tes
                .iter()
                .map(|&te| {
                    (0..tu)
                        .map(|_| (s0 * math::exp(-te * rate) + sd * rng.normal()) as f32)
                        .collect()
                })
                .collect();
            let start = loglin(&tes, &y);
            let mut s = Samples::default();
            s.fill(&tes, tes.len(), |e| y[e].iter().copied());
            let xdata: Vec<f64> = tes
                .iter()
                .flat_map(|&t| std::iter::repeat_n(t, tu))
                .collect();
            let ydata: Vec<f64> = y.iter().flatten().map(|&v| f64::from(v)).collect();
            let mut model = |xd: &[f64], p: &[f64; 2], out: &mut [f64]| {
                for (o, &te) in out.iter_mut().zip(xd) {
                    *o = p[0] * math::exp_glibc(-te / p[1]);
                }
            };
            let lb = [ydata.iter().copied().fold(f64::INFINITY, f64::min), 0.0];
            let reference = larmorx_optim::curve_fit2(
                &mut model,
                &xdata,
                &ydata,
                start,
                lb,
                [f64::INFINITY; 2],
                larmorx_optim::Scipy::V1_18_1,
                &libm,
            );
            let ours = fit(&s, start, math::exp_glibc, &mut w);
            n_vox += 1;
            let objective = |p: [f64; 2]| -> f64 {
                xdata
                    .iter()
                    .zip(&ydata)
                    .map(|(&te, &y)| {
                        let r = p[0] * math::exp(-te / p[1]) - y;
                        r * r
                    })
                    .sum()
            };
            let near_start = (1.0 / start[1]).abs() * 1000.0 <= 1e-9
                || (start[0] - lb[0]).abs() <= 1e-6 * lb[0].abs();
            let (ref_p, our_p) = match (&reference, &ours) {
                (Err(_), Err(_)) => {
                    n_fail += 1;
                    continue;
                }
                (Ok(r), Ok(f)) => ([r.popt[0], r.popt[1]], [f.s0, f.t2s]),
                (Ok(r), Err(_)) => ([r.popt[0], r.popt[1]], start),
                (Err(_), Ok(f)) => (start, [f.s0, f.t2s]),
            };
            // (a): the written float32 values in seconds, rates within 1e-6 relative + 1e-9 s⁻¹
            let rate_of = |t: f64| 1.0 / f64::from((t / 1000.0) as f32);
            let (ra, rb) = (rate_of(ref_p[1]), rate_of(our_p[1]));
            let (sa, sb) = (f64::from(ref_p[0] as f32), f64::from(our_p[0] as f32));
            if (ra - rb).abs() <= 1e-6 * ra.abs() + 1e-9 && (sa - sb).abs() <= 1e-6 * sa.abs() {
                n_a += 1;
            } else if objective(our_p)
                <= objective(ref_p) * (1.0 + 1e-6)
                    + 1e-12 * ydata.iter().map(|y| y * y).sum::<f64>()
            {
                n_b += 1;
            } else if near_start {
                n_c += 1;
            } else {
                panic!(
                    "case {case}: SciPy {reference:?}, the original {ours:?} from {start:?} (lb {lb:?})"
                );
            }
        }
        let share = (n_b + n_c) as f64 / n_vox as f64;
        eprintln!("(a) {n_a}, (b) {n_b}, (c) {n_c}, both failed {n_fail} of {n_vox}");
        assert!(
            share <= 0.03,
            "(a) {n_a}, (b) {n_b}, (c) {n_c}, failed {n_fail}"
        );
        assert!(
            n_a > 1000 && n_fail > 0,
            "(a) {n_a}, (b) {n_b}, (c) {n_c}, failed {n_fail}"
        );
    }

    #[test]
    fn svd_of_two_columns() {
        let mut a = vec![3.0, 0.0, 4.0];
        let mut b = vec![1.0, 2.0, 0.5];
        let (s, v) = svd2(&mut a, &mut b);
        // the Gram matrix's eigenvalues: [[25, 5], [5, 5.25]]
        let (t, det): (f64, f64) = (25.0 + 5.25, 25.0 * 5.25 - 25.0);
        let l0 = 0.5 * (t + (t * t - 4.0 * det).sqrt());
        assert!(
            (s[0] * s[0] - l0).abs() < 1e-12 && (s[0] * s[1] * s[0] * s[1] - det).abs() < 1e-10
        );
        // V is orthogonal
        assert!((v[0][0] * v[0][1] + v[1][0] * v[1][1]).abs() < 1e-15);
    }
}

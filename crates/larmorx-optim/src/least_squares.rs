// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `scipy.optimize.least_squares(fun, x0, jac='2-point', bounds=(lb, ub), method='trf')` for
//! two parameters, with SciPy's defaults (`ftol = xtol = gtol = 1e-8`, `x_scale = 1`, the linear
//! loss, `tr_solver='exact'`, `max_nfev = 100 · 2`), as SciPy 1.18.1 and 1.15.3 compute it with
//! numpy's and their own OpenBLAS on x86-64 (crate documentation; notices in `lib.rs`).
//!
//! The port keeps SciPy's operation order everywhere: the trust-region reflective iteration of
//! `_lsq/trf.py`'s `trf_bounds` (Coleman-Li scaling, the SVD of the augmented Jacobian, the
//! trust-region subproblem of `solve_lsq_trust_region`, `select_step`'s choice among the
//! truncated step, its reflection and the scaled anti-gradient, `make_strictly_feasible`,
//! `update_tr_radius`, `check_termination`), the forward differences of
//! `_numdiff.approx_derivative` with `_adjust_scheme_to_bounds`, numpy's `dot` and `norm` on
//! OpenBLAS's kernels, numpy's NaN rules for `maximum`, `minimum` and `argmin`, Python's `max`
//! and `min` on two numbers, and C's `pow` where numpy calls it.

use crate::Scipy;
use larmorx_core::math::Libm;
use larmorx_transform::numpy_linalg::blas::{self, Layout};
use larmorx_transform::numpy_linalg::{GesddError, LapackRelease, gesdd_s2};

/// `np.finfo(float).eps`.
const EPS: f64 = f64::EPSILON;
/// `_eps_for_method(float64, float64, '2-point')`: `EPS ** 0.5`, exactly 2^-26.
const RSTEP: f64 = 1.490_116_119_384_765_6e-8;
/// SciPy's default tolerances.
const FTOL: f64 = 1e-8;
const XTOL: f64 = 1e-8;
const GTOL: f64 = 1e-8;

/// What `least_squares` returns (the fields `curve_fit` uses, and the counts).
#[derive(Clone, Debug, PartialEq)]
pub struct LeastSquares {
    /// The solution.
    pub x: [f64; 2],
    /// Half the sum of squared residuals at `x`.
    pub cost: f64,
    /// The residuals at `x` (`m` values).
    pub fun: Vec<f64>,
    /// The Jacobian at `x`, `m × 2`, column-major (`jac[k + i·m]` is `∂f_k/∂x_i`).
    pub jac: Vec<f64>,
    /// The gradient `Jᵀf` at `x`.
    pub grad: [f64; 2],
    /// The first-order optimality measure of the last iteration (`‖g·v‖∞`).
    pub optimality: f64,
    /// Residual evaluations counted by the method (the initial one included; the Jacobian's
    /// differences are not counted).
    pub nfev: usize,
    /// Jacobian evaluations.
    pub njev: usize,
    /// SciPy's status: 0 (the evaluation budget is exhausted), 1 (`gtol`), 2 (`ftol`),
    /// 3 (`xtol`), 4 (`ftol` and `xtol`).
    pub status: i32,
}

/// Why `least_squares` raised (every one a `ValueError` in SciPy; [`GesddError::DidNotConverge`]
/// is numpy's `LinAlgError`, a subclass of it).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LeastSquaresError {
    #[error("Each lower bound must be strictly less than each upper bound.")]
    BoundsOrder,
    #[error("Initial guess is outside of provided bounds")]
    OutsideBounds,
    #[error("Residuals are not finite in the initial point.")]
    ResidualsNotFinite,
    /// `intersect_trust_region` with a zero direction.
    #[error("`s` is zero.")]
    SZero,
    /// `intersect_trust_region` from a point outside the trust region (a rounding of the
    /// truncated step).
    #[error("`x` is not within the trust region.")]
    NotWithinTrustRegion,
    /// `scipy.linalg.svd` of the augmented Jacobian: a value that is not finite, or no
    /// convergence.
    #[error(transparent)]
    Svd(#[from] GesddError),
}

impl LeastSquaresError {
    /// The Python exception's class and message.
    pub fn python(&self) -> (&'static str, String) {
        match self {
            LeastSquaresError::Svd(GesddError::DidNotConverge) => ("LinAlgError", self.to_string()),
            _ => ("ValueError", self.to_string()),
        }
    }
}

/// `least_squares(..., method='trf')` (module documentation): `fun(x, f)` writes the `m`
/// residuals at `x` into `f`; `x0`, `lb` and `ub` as SciPy takes them (`lb[i] < ub[i]`, `x0`
/// within). `libm` gives C's `pow` (`reproduce`: glibc's).
pub fn least_squares_trf2(
    fun: &mut dyn FnMut(&[f64; 2], &mut [f64]),
    m: usize,
    x0: [f64; 2],
    lb: [f64; 2],
    ub: [f64; 2],
    scipy: Scipy,
    libm: &Libm,
) -> Result<LeastSquares, LeastSquaresError> {
    if (0..2).any(|i| lb[i] >= ub[i]) {
        return Err(LeastSquaresError::BoundsOrder);
    }
    if !in_bounds(&x0, &lb, &ub) {
        return Err(LeastSquaresError::OutsideBounds);
    }
    let x0 = make_strictly_feasible(&x0, &lb, &ub, 1e-10);
    let mut f0 = vec![0.0f64; m];
    fun(&x0, &mut f0);
    // SciPy 1.18.1's VectorFunction takes the Jacobian at x0 before the check below, SciPy
    // 1.15.3 after it; neither can raise, so the order does not matter.
    if !f0.iter().all(|v| v.is_finite()) {
        return Err(LeastSquaresError::ResidualsNotFinite);
    }
    let j0 = approx_jacobian(fun, &x0, &f0, &lb, &ub);
    Trf {
        m,
        lb,
        ub,
        scipy,
        libm,
    }
    .run(fun, x0, f0, j0)
}

/// The fixed context of one call.
struct Trf<'a> {
    m: usize,
    lb: [f64; 2],
    ub: [f64; 2],
    scipy: Scipy,
    libm: &'a Libm,
}

impl Trf<'_> {
    /// `trf_bounds` with `x_scale = 1`, the linear loss and the exact solver.
    fn run(
        &self,
        fun: &mut dyn FnMut(&[f64; 2], &mut [f64]),
        x0: [f64; 2],
        f0: Vec<f64>,
        j0: Vec<f64>,
    ) -> Result<LeastSquares, LeastSquaresError> {
        let m = self.m;
        let (lb, ub) = (&self.lb, &self.ub);
        let mut x = x0;
        let mut f = f0;
        let mut nfev = 1usize;
        let mut jac = j0;
        let mut njev = 1usize;
        let mut cost = 0.5 * blas::dot(&f, &f);
        let mut g = grad(&jac, &f, m);
        // scale = scale_inv = 1 (x_scale = 1.0 resized to x0's shape).
        let (mut v, _dv) = cl_scaling_vector(&x, &g, lb, ub);
        let mut delta = norm(&[x0[0] / v[0].sqrt(), x0[1] / v[1].sqrt()]);
        if delta == 0.0 {
            delta = 1.0;
        }
        // g_norm = norm(g * v, ord=np.inf) here too; the loop recomputes it before any use.
        let mut g_norm;
        let max_nfev = 2 * 100;
        let mut alpha = 0.0f64;
        let mut termination_status: Option<i32> = None;
        let mut f_new = vec![0.0f64; m];
        // J_augmented (m + 2) × 2, column-major as LAPACK receives it; J_h = J_augmented[:m],
        // row-major as numpy holds it.
        let mut j_aug = vec![0.0f64; (m + 2) * 2];
        let mut j_h = vec![0.0f64; 2 * m];
        let mut f_aug = vec![0.0f64; m + 2];
        loop {
            let dv;
            (v, dv) = cl_scaling_vector(&x, &g, lb, ub);
            g_norm = norm_inf(&[g[0] * v[0], g[1] * v[1]]);
            if g_norm < GTOL {
                termination_status = Some(1);
            }
            if termination_status.is_some() || nfev == max_nfev {
                break;
            }
            // v[dv != 0] *= scale_inv (1); d = v**0.5 * scale; diag_h = g * dv * scale.
            let d = [v[0].sqrt(), v[1].sqrt()];
            let diag_h = [g[0] * dv[0], g[1] * dv[1]];
            let g_h = [d[0] * g[0], d[1] * g[1]];
            f_aug[..m].copy_from_slice(&f);
            f_aug[m] = 0.0;
            f_aug[m + 1] = 0.0;
            for k in 0..m {
                let (a0, a1) = (jac[k] * d[0], jac[k + m] * d[1]);
                j_h[2 * k] = a0;
                j_h[2 * k + 1] = a1;
                j_aug[k] = a0;
                j_aug[k + (m + 2)] = a1;
            }
            // J_augmented[m:] = np.diag(diag_h**0.5).
            j_aug[m] = diag_h[0].sqrt();
            j_aug[m + 1] = 0.0;
            j_aug[m + (m + 2)] = 0.0;
            j_aug[m + 1 + (m + 2)] = diag_h[1].sqrt();
            let svd = gesdd_s2(&j_aug, m + 2, LapackRelease::V3_12_0)?;
            let s = svd.s;
            let uf = self.ut_dot(&svd.u, m + 2, &f_aug);
            // theta controls step back step ratio from the bounds.
            let theta = py_max(0.995, 1.0 - g_norm);
            let mut actual_reduction = -1.0f64;
            let mut cost_new = cost;
            let mut x_new = x;
            while actual_reduction <= 0.0 && nfev < max_nfev {
                let (p_h, new_alpha) = self.solve_lsq_trust_region(&uf, &s, &svd.vt, delta, alpha);
                alpha = new_alpha;
                let p = [d[0] * p_h[0], d[1] * p_h[1]];
                let (step, step_h, predicted_reduction) =
                    self.select_step(&x, &j_h, &diag_h, &g_h, p, p_h, &d, delta, theta)?;
                x_new = make_strictly_feasible(&[x[0] + step[0], x[1] + step[1]], lb, ub, 0.0);
                fun(&x_new, &mut f_new);
                nfev += 1;
                let step_h_norm = norm(&step_h);
                if !f_new.iter().all(|v| v.is_finite()) {
                    delta = 0.25 * step_h_norm;
                    continue;
                }
                cost_new = 0.5 * blas::dot(&f_new, &f_new);
                actual_reduction = cost - cost_new;
                let (delta_new, ratio) = update_tr_radius(
                    delta,
                    actual_reduction,
                    predicted_reduction,
                    step_h_norm,
                    step_h_norm > 0.95 * delta,
                );
                let step_norm = norm(&step);
                termination_status = check_termination(
                    actual_reduction,
                    cost,
                    step_norm,
                    norm(&x),
                    ratio,
                    FTOL,
                    XTOL,
                );
                if termination_status.is_some() {
                    break;
                }
                alpha *= delta / delta_new;
                delta = delta_new;
            }
            if actual_reduction > 0.0 {
                x = x_new;
                std::mem::swap(&mut f, &mut f_new);
                cost = cost_new;
                jac = approx_jacobian(fun, &x, &f, lb, ub);
                njev += 1;
                g = grad(&jac, &f, m);
            }
        }
        Ok(LeastSquares {
            x,
            cost,
            fun: f,
            jac,
            grad: g,
            optimality: g_norm,
            nfev,
            njev,
            status: termination_status.unwrap_or(0),
        })
    }

    /// `U.T.dot(f_augmented)` for the `rows × 2` factor `u` (column-major): SciPy 1.15.3's
    /// Fortran-ordered `U` makes `U.T` C-contiguous (`dgemv_t`), SciPy 1.18.1's C-ordered `U`
    /// makes it F-contiguous (`dgemv_n`).
    fn ut_dot(&self, u: &[f64], rows: usize, x: &[f64]) -> [f64; 2] {
        let y = match self.scipy {
            Scipy::V1_15_3 => blas::matvec(u, 2, rows, Layout::C, x),
            Scipy::V1_18_1 => {
                let mut rm = vec![0.0f64; 2 * rows];
                for i in 0..rows {
                    rm[2 * i] = u[i];
                    rm[2 * i + 1] = u[i + rows];
                }
                blas::matvec(&rm, 2, rows, Layout::F, x)
            }
        };
        [y[0], y[1]]
    }

    /// `V.dot(x)` with `V = VT.T` (`vt` column-major): C-contiguous for SciPy 1.15.3's
    /// Fortran-ordered `VT`, F-contiguous for SciPy 1.18.1's C-ordered one.
    fn v_dot(&self, vt: &[f64; 4], x: &[f64; 2]) -> [f64; 2] {
        let y = match self.scipy {
            // V row-major: V(i, j) = VT(j, i) = vt[j + 2i], the column-major buffer itself.
            Scipy::V1_15_3 => blas::matvec(vt, 2, 2, Layout::C, x),
            // V column-major: V(i, j) at i + 2j = VT(j, i), VT's row-major buffer.
            Scipy::V1_18_1 => {
                let rm = [vt[0], vt[2], vt[1], vt[3]];
                blas::matvec(&rm, 2, 2, Layout::F, x)
            }
        };
        [y[0], y[1]]
    }

    /// `solve_lsq_trust_region(n=2, m, uf, s, V, Delta, initial_alpha=alpha)` (rtol 0.01, ten
    /// iterations at most): `(p, alpha)`.
    fn solve_lsq_trust_region(
        &self,
        uf: &[f64; 2],
        s: &[f64; 2],
        vt: &[f64; 4],
        delta: f64,
        initial_alpha: f64,
    ) -> ([f64; 2], f64) {
        let libm = self.libm;
        let m = self.m;
        let phi_and_derivative = |alpha: f64, suf: &[f64; 2]| {
            let denom = [s[0] * s[0] + alpha, s[1] * s[1] + alpha];
            let p_norm = norm(&[suf[0] / denom[0], suf[1] / denom[1]]);
            let phi = p_norm - delta;
            let q = [
                (suf[0] * suf[0]) / libm.pow(denom[0], 3.0),
                (suf[1] * suf[1]) / libm.pow(denom[1], 3.0),
            ];
            let phi_prime = -sum2(&q) / p_norm;
            (phi, phi_prime)
        };
        let suf = [s[0] * uf[0], s[1] * uf[1]];
        // m >= n always here (m >= 2 residuals).
        let full_rank = if m >= 2 {
            let threshold = EPS * m as f64 * s[0];
            s[1] > threshold
        } else {
            false
        };
        if full_rank {
            let q = self.v_dot(vt, &[uf[0] / s[0], uf[1] / s[1]]);
            let p = [-q[0], -q[1]];
            if norm(&p) <= delta {
                return (p, 0.0);
            }
        }
        let mut alpha_upper = norm(&suf) / delta;
        let mut alpha_lower = if full_rank {
            let (phi, phi_prime) = phi_and_derivative(0.0, &suf);
            -phi / phi_prime
        } else {
            0.0
        };
        let reset = |lower: f64, upper: f64| py_max(0.001 * upper, libm.pow(lower * upper, 0.5));
        let mut alpha = if !full_rank && initial_alpha == 0.0 {
            reset(alpha_lower, alpha_upper)
        } else {
            initial_alpha
        };
        for _ in 0..10 {
            if alpha < alpha_lower || alpha > alpha_upper {
                alpha = reset(alpha_lower, alpha_upper);
            }
            let (phi, phi_prime) = phi_and_derivative(alpha, &suf);
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
        let q = self.v_dot(
            vt,
            &[
                suf[0] / (s[0] * s[0] + alpha),
                suf[1] / (s[1] * s[1] + alpha),
            ],
        );
        let p = [-q[0], -q[1]];
        let k = delta / norm(&p);
        ([p[0] * k, p[1] * k], alpha)
    }

    /// `select_step(x, J_h, diag_h, g_h, p, p_h, d, Delta, lb, ub, theta)`: `(step, step_h,
    /// predicted reduction)`.
    #[allow(clippy::too_many_arguments)]
    fn select_step(
        &self,
        x: &[f64; 2],
        j_h: &[f64],
        diag_h: &[f64; 2],
        g_h: &[f64; 2],
        mut p: [f64; 2],
        mut p_h: [f64; 2],
        d: &[f64; 2],
        delta: f64,
        theta: f64,
    ) -> Result<([f64; 2], [f64; 2], f64), LeastSquaresError> {
        let (lb, ub) = (&self.lb, &self.ub);
        if in_bounds(&[x[0] + p[0], x[1] + p[1]], lb, ub) {
            let p_value = self.evaluate_quadratic(j_h, g_h, &p_h, diag_h);
            return Ok((p, p_h, -p_value));
        }
        let (p_stride, hits) = step_size_to_bound(x, &p, lb, ub);
        // Compute the reflected direction.
        let mut r_h = p_h;
        for i in 0..2 {
            if hits[i] != 0 {
                r_h[i] *= -1.0;
            }
        }
        let mut r = [d[0] * r_h[0], d[1] * r_h[1]];
        // Restrict trust-region step, such that it hits the bound.
        p = [p[0] * p_stride, p[1] * p_stride];
        p_h = [p_h[0] * p_stride, p_h[1] * p_stride];
        let x_on_bound = [x[0] + p[0], x[1] + p[1]];
        let (_, to_tr) = intersect_trust_region(&p_h, &r_h, delta, self.libm)?;
        let (to_bound, _) = step_size_to_bound(&x_on_bound, &r, lb, ub);
        let r_stride = py_min(to_bound, to_tr);
        let (r_stride_l, r_stride_u) = if r_stride > 0.0 {
            let l = (1.0 - theta) * p_stride / r_stride;
            let u = if r_stride == to_bound {
                theta * to_bound
            } else {
                to_tr
            };
            (l, u)
        } else {
            (0.0, -1.0)
        };
        let r_value;
        if r_stride_l <= r_stride_u {
            let (a, b, c) = self.build_quadratic_1d(j_h, g_h, &r_h, Some(&p_h), diag_h);
            let (rs, rv) = minimize_quadratic_1d(a, b, r_stride_l, r_stride_u, Some(c));
            r_h = [r_h[0] * rs, r_h[1] * rs];
            r_h = [r_h[0] + p_h[0], r_h[1] + p_h[1]];
            r = [r_h[0] * d[0], r_h[1] * d[1]];
            r_value = rv;
        } else {
            r_value = f64::INFINITY;
        }
        // Now correct p_h to make it strictly interior.
        p = [p[0] * theta, p[1] * theta];
        p_h = [p_h[0] * theta, p_h[1] * theta];
        let p_value = self.evaluate_quadratic(j_h, g_h, &p_h, diag_h);
        let ag_h = [-g_h[0], -g_h[1]];
        let ag = [d[0] * ag_h[0], d[1] * ag_h[1]];
        let to_tr = delta / norm(&ag_h);
        let (to_bound, _) = step_size_to_bound(x, &ag, lb, ub);
        let ag_stride = if to_bound < to_tr {
            theta * to_bound
        } else {
            to_tr
        };
        let (a, b, _) = self.build_quadratic_1d(j_h, g_h, &ag_h, None, diag_h);
        let (ag_stride, ag_value) = minimize_quadratic_1d(a, b, 0.0, ag_stride, None);
        let ag_h = [ag_h[0] * ag_stride, ag_h[1] * ag_stride];
        let ag = [ag[0] * ag_stride, ag[1] * ag_stride];
        Ok(if p_value < r_value && p_value < ag_value {
            (p, p_h, -p_value)
        } else if r_value < p_value && r_value < ag_value {
            (r, r_h, -r_value)
        } else {
            (ag, ag_h, -ag_value)
        })
    }

    /// `J_h.dot(s)` for the row-major `m × 2` `J_h` (`dgemv_t` with two rows).
    fn jh_dot(&self, j_h: &[f64], s: &[f64; 2]) -> Vec<f64> {
        blas::matvec(j_h, self.m, 2, Layout::C, s)
    }

    /// `evaluate_quadratic(J, g, s, diag)` for one vector `s`: `0.5 · q + l`.
    fn evaluate_quadratic(&self, j_h: &[f64], g: &[f64; 2], s: &[f64; 2], diag: &[f64; 2]) -> f64 {
        let js = self.jh_dot(j_h, s);
        let mut q = blas::dot(&js, &js);
        q += blas::dot(&[s[0] * diag[0], s[1] * diag[1]], s);
        let l = blas::dot(s, g);
        0.5 * q + l
    }

    /// `build_quadratic_1d(J, g, s, diag=diag, s0=s0)`: `(a, b, c)` (`c = 0` without `s0`).
    fn build_quadratic_1d(
        &self,
        j_h: &[f64],
        g: &[f64; 2],
        s: &[f64; 2],
        s0: Option<&[f64; 2]>,
        diag: &[f64; 2],
    ) -> (f64, f64, f64) {
        let v = self.jh_dot(j_h, s);
        let mut a = blas::dot(&v, &v);
        a += blas::dot(&[s[0] * diag[0], s[1] * diag[1]], s);
        a *= 0.5;
        let mut b = blas::dot(g, s);
        let Some(s0) = s0 else {
            return (a, b, 0.0);
        };
        let u = self.jh_dot(j_h, s0);
        b += blas::dot(&u, &v);
        let mut c = 0.5 * blas::dot(&u, &u) + blas::dot(g, s0);
        let s0d = [s0[0] * diag[0], s0[1] * diag[1]];
        b += blas::dot(&s0d, s);
        c += 0.5 * blas::dot(&s0d, s0);
        (a, b, c)
    }
}

/// `compute_grad(J, f)` = `J.T.dot(f)`: `J.T` is C-contiguous (the transposed buffer of
/// `approx_derivative`), so `dgemv_t` over the `m × 2` column-major `J`.
fn grad(jac: &[f64], f: &[f64], m: usize) -> [f64; 2] {
    let y = blas::matvec(jac, 2, m, Layout::C, f);
    [y[0], y[1]]
}

/// `approx_derivative(fun, x, method='2-point', f0=f0, bounds=(lb, ub))`: the `m × 2`
/// column-major Jacobian.
fn approx_jacobian(
    fun: &mut dyn FnMut(&[f64; 2], &mut [f64]),
    x: &[f64; 2],
    f0: &[f64],
    lb: &[f64; 2],
    ub: &[f64; 2],
) -> Vec<f64> {
    let m = f0.len();
    // _compute_absolute_step: rstep · sign(x) · max(1, |x|), sign -1 where not x >= 0.
    let mut h = [0.0f64; 2];
    for i in 0..2 {
        let sign = if x[i] >= 0.0 { 1.0 } else { -1.0 };
        h[i] = (RSTEP * sign) * np_maximum(1.0, x[i].abs());
    }
    // _adjust_scheme_to_bounds(x, h, 1, '1-sided', lb, ub).
    let unbounded = (0..2).all(|i| lb[i] == f64::NEG_INFINITY && ub[i] == f64::INFINITY);
    if !unbounded {
        for i in 0..2 {
            let h_total = h[i];
            let lower_dist = x[i] - lb[i];
            let upper_dist = ub[i] - x[i];
            let xt = x[i] + h_total;
            let violated = xt < lb[i] || xt > ub[i];
            let fitting = h_total.abs() <= np_maximum(lower_dist, upper_dist);
            if violated && fitting {
                h[i] *= -1.0;
            }
            if upper_dist >= lower_dist && !fitting {
                h[i] = upper_dist;
            }
            if upper_dist < lower_dist && !fitting {
                h[i] = -lower_dist;
            }
        }
    }
    // _dense_difference, '2-point'.
    let mut jac = vec![0.0f64; 2 * m];
    let mut f1 = vec![0.0f64; m];
    for i in 0..2 {
        let mut x1 = *x;
        x1[i] = x[i] + h[i];
        fun(&x1, &mut f1);
        let dx = (x[i] + h[i]) - x[i];
        for k in 0..m {
            jac[k + i * m] = (f1[k] - f0[k]) / dx;
        }
    }
    jac
}

/// `numpy.linalg.norm(v)` of two values: `sqrt(dot(v, v))`.
fn norm(v: &[f64; 2]) -> f64 {
    blas::dot(v, v).sqrt()
}

/// `numpy.linalg.norm(v, ord=np.inf)`: `abs(v).max()`, a NaN wins.
fn norm_inf(v: &[f64; 2]) -> f64 {
    np_max_reduce(v[0].abs(), v[1].abs())
}

/// `np.sum` of two values (`0 + ((-0 + a) + b)`).
fn sum2(a: &[f64; 2]) -> f64 {
    larmorx_core::numpy::reduce::add_reduce(&a[..])
}

/// numpy's `maximum(a, b)` (`a >= b` or `a` NaN: `a`; else `b`).
fn np_maximum(a: f64, b: f64) -> f64 {
    if a >= b || a.is_nan() { a } else { b }
}

/// numpy's `minimum(a, b)` (`a <= b` or `a` NaN: `a`; else `b`).
fn np_minimum(a: f64, b: f64) -> f64 {
    if a <= b || a.is_nan() { a } else { b }
}

/// `np.max` of two values (`maximum.reduce`: a NaN wins).
fn np_max_reduce(a: f64, b: f64) -> f64 {
    np_maximum(a, b)
}

/// Python's `max(a, b)`: `b` only when `b > a`.
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Python's `min(a, b)`: `b` only when `b < a`.
fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `in_bounds(x, lb, ub)`.
fn in_bounds(x: &[f64; 2], lb: &[f64; 2], ub: &[f64; 2]) -> bool {
    (0..2).all(|i| x[i] >= lb[i] && x[i] <= ub[i])
}

/// `CL_scaling_vector(x, g, lb, ub)`: `(v, dv)`.
fn cl_scaling_vector(
    x: &[f64; 2],
    g: &[f64; 2],
    lb: &[f64; 2],
    ub: &[f64; 2],
) -> ([f64; 2], [f64; 2]) {
    let mut v = [1.0f64; 2];
    let mut dv = [0.0f64; 2];
    for i in 0..2 {
        if g[i] < 0.0 && ub[i].is_finite() {
            v[i] = ub[i] - x[i];
            dv[i] = -1.0;
        }
    }
    for i in 0..2 {
        if g[i] > 0.0 && lb[i].is_finite() {
            v[i] = x[i] - lb[i];
            dv[i] = 1.0;
        }
    }
    (v, dv)
}

/// `find_active_constraints(x, lb, ub, rtol)`: -1 at an active lower bound, 1 at an upper.
fn find_active_constraints(x: &[f64; 2], lb: &[f64; 2], ub: &[f64; 2], rtol: f64) -> [i8; 2] {
    let mut active = [0i8; 2];
    if rtol == 0.0 {
        for i in 0..2 {
            if x[i] <= lb[i] {
                active[i] = -1;
            }
        }
        for i in 0..2 {
            if x[i] >= ub[i] {
                active[i] = 1;
            }
        }
        return active;
    }
    for i in 0..2 {
        let lower_dist = x[i] - lb[i];
        let upper_dist = ub[i] - x[i];
        let lower_threshold = rtol * np_maximum(1.0, lb[i].abs());
        let upper_threshold = rtol * np_maximum(1.0, ub[i].abs());
        if lb[i].is_finite() && lower_dist <= np_minimum(upper_dist, lower_threshold) {
            active[i] = -1;
        }
        if ub[i].is_finite() && upper_dist <= np_minimum(lower_dist, upper_threshold) {
            active[i] = 1;
        }
    }
    active
}

/// `make_strictly_feasible(x, lb, ub, rstep)`.
fn make_strictly_feasible(x: &[f64; 2], lb: &[f64; 2], ub: &[f64; 2], rstep: f64) -> [f64; 2] {
    let mut x_new = *x;
    let active = find_active_constraints(x, lb, ub, rstep);
    for i in 0..2 {
        if rstep == 0.0 {
            if active[i] == -1 {
                x_new[i] = next_after(lb[i], ub[i]);
            } else if active[i] == 1 {
                x_new[i] = next_after(ub[i], lb[i]);
            }
        } else if active[i] == -1 {
            x_new[i] = lb[i] + rstep * np_maximum(1.0, lb[i].abs());
        } else if active[i] == 1 {
            x_new[i] = ub[i] - rstep * np_maximum(1.0, ub[i].abs());
        }
    }
    for i in 0..2 {
        if x_new[i] < lb[i] || x_new[i] > ub[i] {
            x_new[i] = 0.5 * (lb[i] + ub[i]);
        }
    }
    x_new
}

/// C's `nextafter(from, to)`.
fn next_after(from: f64, to: f64) -> f64 {
    if from.is_nan() || to.is_nan() {
        return from + to;
    }
    if from == to {
        return to;
    }
    if from == 0.0 {
        let tiny = f64::from_bits(1);
        return if to > 0.0 { tiny } else { -tiny };
    }
    let bits = from.to_bits();
    let up = (to > from) == (from > 0.0);
    f64::from_bits(if up { bits + 1 } else { bits - 1 })
}

/// `step_size_to_bound(x, s, lb, ub)`: the smallest step along `s` to a bound, and which
/// components reach it (`np.equal(steps, min_step) * np.sign(s).astype(int)`).
fn step_size_to_bound(x: &[f64; 2], s: &[f64; 2], lb: &[f64; 2], ub: &[f64; 2]) -> (f64, [i64; 2]) {
    let mut steps = [f64::INFINITY; 2];
    for i in 0..2 {
        if s[i] != 0.0 {
            steps[i] = np_maximum((lb[i] - x[i]) / s[i], (ub[i] - x[i]) / s[i]);
        }
    }
    // np.min: minimum.reduce, a NaN wins.
    let min_step = np_minimum(steps[0], steps[1]);
    let mut hits = [0i64; 2];
    for i in 0..2 {
        if steps[i] == min_step {
            // np.sign(NaN) is NaN, which astype(int) makes INT64_MIN on x86-64.
            hits[i] = if s[i].is_nan() {
                i64::MIN
            } else if s[i] > 0.0 {
                1
            } else if s[i] < 0.0 {
                -1
            } else {
                0
            };
        }
    }
    (min_step, hits)
}

/// `intersect_trust_region(x, s, Delta)`: the roots `t` of `‖x + s·t‖² = Delta²`, ordered.
fn intersect_trust_region(
    x: &[f64; 2],
    s: &[f64; 2],
    delta: f64,
    libm: &Libm,
) -> Result<(f64, f64), LeastSquaresError> {
    let a = blas::dot(s, s);
    if a == 0.0 {
        return Err(LeastSquaresError::SZero);
    }
    let b = blas::dot(x, s);
    let c = blas::dot(x, x) - libm.pow(delta, 2.0);
    if c > 0.0 {
        return Err(LeastSquaresError::NotWithinTrustRegion);
    }
    let d = (b * b - a * c).sqrt();
    let q = -(b + d.copysign(b));
    let t1 = q / a;
    let t2 = c / q;
    Ok(if t1 < t2 { (t1, t2) } else { (t2, t1) })
}

/// `minimize_quadratic_1d(a, b, lb, ub, c)` on `[lb, ub]`: `(t, y)` (`c` absent: numpy adds the
/// Python integer 0, i.e. `+0.0`).
fn minimize_quadratic_1d(a: f64, b: f64, lb: f64, ub: f64, c: Option<f64>) -> (f64, f64) {
    let mut t = vec![lb, ub];
    if a != 0.0 {
        let extremum = -0.5 * b / a;
        if lb < extremum && extremum < ub {
            t.push(extremum);
        }
    }
    let c = c.unwrap_or(0.0);
    let y: Vec<f64> = t.iter().map(|&ti| ti * (a * ti + b) + c).collect();
    // np.argmin: the first minimum, or the first NaN (numpy's `!(x >= min)` test).
    let mut best = 0usize;
    let mut mp = y[0];
    if !mp.is_nan() {
        for (i, &yi) in y.iter().enumerate().skip(1) {
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(yi >= mp) {
                mp = yi;
                best = i;
                if mp.is_nan() {
                    break;
                }
            }
        }
    }
    (t[best], y[best])
}

/// `update_tr_radius(Delta, actual_reduction, predicted_reduction, step_norm, bound_hit)`:
/// `(Delta, ratio)`.
fn update_tr_radius(
    delta: f64,
    actual_reduction: f64,
    predicted_reduction: f64,
    step_norm: f64,
    bound_hit: bool,
) -> (f64, f64) {
    let ratio = if predicted_reduction > 0.0 {
        actual_reduction / predicted_reduction
    } else if predicted_reduction == actual_reduction && actual_reduction == 0.0 {
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

/// `check_termination(dF, F, dx_norm, x_norm, ratio, ftol, xtol)`.
fn check_termination(
    d_f: f64,
    f: f64,
    dx_norm: f64,
    x_norm: f64,
    ratio: f64,
    ftol: f64,
    xtol: f64,
) -> Option<i32> {
    let ftol_satisfied = d_f < ftol * f && ratio > 0.25;
    let xtol_satisfied = dx_norm < xtol * (xtol + x_norm);
    match (ftol_satisfied, xtol_satisfied) {
        (true, true) => Some(4),
        (true, false) => Some(2),
        (false, true) => Some(3),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exp_fit(scipy: Scipy) -> LeastSquares {
        let tes = [10.0, 20.0, 30.0, 40.0];
        let y: Vec<f64> = tes
            .iter()
            .map(|&t: &f64| 1000.0 * (-t / 30.0f64).exp() + if t > 25.0 { 1.0 } else { -1.0 })
            .collect();
        let libm = Libm::quiet(larmorx_core::defects::UpstreamDefects::Reproduce);
        let mut fun = |p: &[f64; 2], f: &mut [f64]| {
            for k in 0..4 {
                f[k] = p[0] * libm.exp(-tes[k] / p[1]) - y[k];
            }
        };
        least_squares_trf2(
            &mut fun,
            4,
            [900.0, 25.0],
            [700.0, 0.0],
            [f64::INFINITY; 2],
            scipy,
            &libm,
        )
        .unwrap()
    }

    #[test]
    fn fits_a_monoexponential() {
        for scipy in [Scipy::V1_18_1, Scipy::V1_15_3] {
            let r = exp_fit(scipy);
            assert!(r.status > 0, "{r:?}");
            assert!(
                (r.x[0] - 1000.0).abs() < 5.0 && (r.x[1] - 30.0).abs() < 0.5,
                "{r:?}"
            );
        }
    }

    #[test]
    fn bounds_and_start_checks() {
        let libm = Libm::default();
        let mut fun = |p: &[f64; 2], f: &mut [f64]| f.iter_mut().for_each(|v| *v = p[0]);
        let r = least_squares_trf2(
            &mut fun,
            3,
            [1.0, 1.0],
            [2.0, 0.0],
            [3.0, 5.0],
            Scipy::V1_18_1,
            &libm,
        );
        assert_eq!(r, Err(LeastSquaresError::OutsideBounds));
        let r = least_squares_trf2(
            &mut fun,
            3,
            [1.0, 1.0],
            [2.0, 0.0],
            [2.0, 5.0],
            Scipy::V1_18_1,
            &libm,
        );
        assert_eq!(r, Err(LeastSquaresError::BoundsOrder));
        assert_eq!(next_after(0.0, 1.0), f64::from_bits(1));
        assert_eq!(next_after(f64::INFINITY, 0.0), f64::MAX);
        assert_eq!(next_after(1.0, 2.0), 1.0 + f64::EPSILON);
    }
}

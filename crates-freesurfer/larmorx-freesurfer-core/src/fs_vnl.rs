// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's minimizers over VXL (`fs.maths`, queue item 3.1-23), translated from FreeSurfer
//! 8.2.0's `utils/fs_cost_function.cpp`, `utils/fs_powell.cpp`, `utils/fs_lbfgs.cpp`,
//! `utils/fs_lbfgs_subject.cpp`, `utils/fs_lbfgs_observer.cpp` and their headers in
//! `include/fs_vnl/`:
//!
//! - [`FsFunctions`]: the C function pointers FreeSurfer's programs hand to the minimizers (the
//!   cost function `float (*)(float[])`, its gradient `void (*)(float[], float[])`, the global
//!   `powell_iteration_func`, `fs_lbfgs_observer`'s step function and user callback), every
//!   parameter array 1-based as Numerical Recipes' code indexes it;
//! - [`FsCostFunction`] (`fs_cost_function`): a `vnl_cost_function` over those pointers, converting
//!   VNL's `double` parameters to a `float` array and back;
//! - [`FsPowell`] (`fs_powell`): VNL's Powell minimizer copied by FreeSurfer to take (and return)
//!   the initial directions, with its `printf` lines, the per-iteration callback and its own return
//!   codes (FS-35);
//! - [`FsLbfgs`] (`fs_lbfgs`) with [`FsLbfgsObserver`] (`fs_lbfgs_subject`, `fs_lbfgs_observer`):
//!   VNL's L-BFGS driver copied by FreeSurfer to keep and report the best point (FS-37).
//!
//! The VXL routines they call are the main package's ports of ITK 4.13.2's VNL (queue item 3.1-07):
//! `larmorx_core::vnl_minimize` (`vnl_nonlinear_minimizer`, `vnl_brent`, `vnl_bracket_minimum`,
//! `vnl_brent_minimizer`) and `larmorx_core::vnl_lbfgs` (netlib's `lbfgs_` as VXL translated it);
//! this crate contains no VXL code. Every expression is evaluated in the C types and order.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-maths.md`):
//! - **FS-35** (kept in both modes, rule C): `fs_powell` returns `FAILED_USER_REQUEST` where VNL's
//!   Powell returns `FAILED_TOO_MANY_ITERATIONS` (its iteration limit), so `OpenPowell`'s exit and
//!   `OpenPowell2`'s message for too many iterations never happen ([`crate::numerics`]).
//! - **FS-37** (corrected by default): `fs_lbfgs` records the best point only from the second
//!   evaluation on, and reports the last value evaluated. Where no evaluation improved on the
//!   starting point, a successful return empties the parameters (FreeSurfer's `OpenDFPMin` then
//!   reads past them and crashes) and a failed one leaves them at the line search's last trial
//!   point with the previous run's end value; elsewhere the end value is the last one evaluated,
//!   not the best one's. By default the starting point is the best until a smaller value, and
//!   every return gives the best point and its value; `reproduce` gives FreeSurfer's (but the
//!   crash: there the starting point).

use std::io::Write;

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::FS_37;
use larmorx_core::vnl_lbfgs::{LbfgsGlobal, lbfgs};
use larmorx_core::vnl_minimize::{Brent, CostFunction, NonlinearMinimizer, ReturnCode};

use crate::diag;
use crate::globals::Globals;

/// The C function pointers a FreeSurfer program hands to the minimizers. Every parameter array
/// is 1-based (Numerical Recipes' indexing): the values are at `[1..=n]`, and `[0]`, which
/// FreeSurfer never sets (a stack array's first float; `calloc`'s 0 for the iteration
/// callback), is 0 here. A function receives a copy of the parameters: what it writes into
/// them is lost, as in C.
pub trait FsFunctions {
    /// The cost function (`float (*)(float[])`).
    fn function(&mut self, p: &[f32]) -> f32;

    /// The gradient (`void (*)(float p[], float g[])`): writes `g[1..=n]` (`g` holds `n + 1`
    /// floats, all 0 on entry; `g[0]` is never read). Only [`FsLbfgs`] calls it; FreeSurfer's
    /// callers of `OpenDFPMin` always supply one (the default leaves the gradient 0).
    fn gradient(&mut self, _p: &[f32], _g: &mut [f32]) {}

    /// `powell_iteration_func(p, nparms)`, the global [`FsPowell`] calls at the start of every
    /// iteration when a program has set it (`oct_register_mosaic` does; FreeSurfer's default is
    /// `NULL`, which this no-op default stands for).
    fn powell_iteration(&mut self, _p: &[f32], _nparms: i32) {}

    /// `fs_lbfgs_observer`'s step function (`void (*)(int itno, float sse, void *parms, float
    /// *p)`, its `parms` being the implementer's own state), called at each new best point.
    fn step(&mut self, _itno: i32, _sse: f32, _p: &[f32]) {}

    /// `fs_lbfgs_observer`'s user callback (`void (*)(float[])`), called after the step function.
    fn user_callback(&mut self, _p: &[f32]) {}
}

/// A plain cost function (a closure) as [`FsFunctions`]: no gradient, no callbacks.
impl<F: FnMut(&[f32]) -> f32> FsFunctions for F {
    fn function(&mut self, p: &[f32]) -> f32 {
        self(p)
    }
}

/// `copyFromVNLToFloat` (and `ConvertFromVNLDoubleToFloat`'s vector form): `x` as floats at
/// `out[1..=n]`, `out[0] = 0`.
fn to_float_1based(x: &[f64], out: &mut Vec<f32>) {
    out.clear();
    out.push(0.0);
    out.extend(x.iter().map(|&v| v as f32));
}

/// `fs_cost_function`: a `vnl_cost_function` over a FreeSurfer program's function pointers.
pub struct FsCostFunction<'a> {
    funcs: &'a mut dyn FsFunctions,
    /// `vnl_cost_function::dim` (`get_number_of_unknowns`): set by the three-argument
    /// constructor, 0 with the one-argument one (`fs_powell` never reads it).
    pub dim: i32,
    p: Vec<f32>,
}

impl<'a> FsCostFunction<'a> {
    /// `fs_cost_function(function)`.
    pub fn new(funcs: &'a mut dyn FsFunctions) -> Self {
        FsCostFunction {
            funcs,
            dim: 0,
            p: Vec::new(),
        }
    }

    /// `fs_cost_function(function, functionGradient, numberOfUnknowns)`.
    pub fn with_gradient(funcs: &'a mut dyn FsFunctions, number_of_unknowns: i32) -> Self {
        FsCostFunction {
            funcs,
            dim: number_of_unknowns,
            p: Vec::new(),
        }
    }

    /// The program's functions (for the observer's callbacks).
    pub fn funcs(&mut self) -> &mut dyn FsFunctions {
        &mut *self.funcs
    }

    /// `f(x)`: the cost function at `x` converted to `float` (1-based), widened to `double`.
    pub fn f(&mut self, x: &[f64]) -> f64 {
        to_float_1based(x, &mut self.p);
        f64::from(self.funcs.function(&self.p))
    }

    /// `gradf(x, gradient)`: the gradient function at `x` (as `float`, 1-based), its
    /// `result[1..=n]` widened into `gradient`.
    pub fn gradf(&mut self, x: &[f64], gradient: &mut [f64]) {
        to_float_1based(x, &mut self.p);
        let mut result = vec![0.0f32; x.len() + 1];
        self.funcs.gradient(&self.p, &mut result);
        for (gi, &r) in gradient.iter_mut().zip(&result[1..]) {
            *gi = f64::from(r);
        }
    }

    /// `vnl_cost_function::compute(x, &f, &g)`: `f`, then `gradf`.
    pub fn compute(&mut self, x: &[f64], g: &mut [f64]) -> f64 {
        let f = self.f(x);
        self.gradf(x, g);
        f
    }
}

impl CostFunction for FsCostFunction<'_> {
    fn f(&mut self, x: &[f64]) -> f64 {
        FsCostFunction::f(self, x)
    }
}

/// The C streams as `larmorx_core::cfmt`'s [`Streams`] (VNL's messages).
struct Std<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Streams for Std<'_> {
    fn out(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        let _ = self.err.write_all(text.as_bytes());
    }
}

/// `fs_powell`: Powell's direction-set method with `vnl_brent` line searches, from FreeSurfer's
/// copy of `vnl_powell`.
#[derive(Clone, Debug, PartialEq)]
pub struct FsPowell {
    /// `vnl_nonlinear_minimizer`'s settings and counters (`ftol`, `maxfev`, `verbose`, ...).
    pub base: NonlinearMinimizer,
    /// `linmin_xtol_` (`set_linmin_xtol`; default 1e-4).
    pub linmin_xtol: f64,
    /// `initial_step_` (`set_initial_step`; default 1.0).
    pub initial_step: f64,
}

impl Default for FsPowell {
    fn default() -> Self {
        Self::new()
    }
}

/// `printf("%f", v)`.
fn pf(v: f64) -> String {
    cfmt::f(v, 6)
}

/// `printf("%g", v)`.
fn pg(v: f64) -> String {
    cfmt::g(v, 6)
}

impl FsPowell {
    /// `fs_powell(functor)`: `linmin_xtol_` 1e-4, `initial_step_` 1.0, VNL's defaults.
    pub fn new() -> Self {
        FsPowell {
            base: NonlinearMinimizer::new(),
            linmin_xtol: 1e-4,
            initial_step: 1.0,
        }
    }

    /// `fs_powell::minimize(p, xi)`: minimizes `functor` from `p` (updated in place) along the
    /// directions `xi`, the columns of an `n x n` matrix stored row by row (updated in place and
    /// returned, as FreeSurfer's; a caller without directions passes the identity, as
    /// `minimize` allocates). Prints its header (`fs_powell::minimize`, `nparams`, `maxfev`,
    /// `ftol`, `linmin_xtol_`) and one `powell nthiter` line per iteration on standard output,
    /// and with `verbose` its trace (standard output; `report_iter`'s line on standard error).
    /// Returns `CONVERGED_FTOL`, or **FS-35** `FAILED_USER_REQUEST` when `maxfev` iterations
    /// pass (VNL's Powell: `FAILED_TOO_MANY_ITERATIONS`; kept in both modes).
    pub fn minimize(
        &mut self,
        functor: &mut FsCostFunction,
        p: &mut [f64],
        xi: &mut [f64],
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> ReturnCode {
        let n = p.len();
        let verbose = self.base.verbose;
        let mut ptt = vec![0.0f64; n];
        let mut xit = vec![0.0f64; n];
        let mut fret = functor.f(p);
        self.base.report_eval(fret);
        let mut pt = p.to_vec();
        let _ = writeln!(out, "fs_powell::minimize");
        let _ = writeln!(out, "  nparams {n}");
        let _ = writeln!(out, "  maxfev {}", self.base.maxfev);
        let _ = writeln!(out, "  ftol   {}", pf(self.base.ftol));
        let _ = writeln!(out, "  linmin_xtol_   {}", pf(self.linmin_xtol));
        let maxfev = self.base.maxfev as u32;
        let mut float_parms = Vec::with_capacity(n + 1);
        while self.base.num_iterations < maxfev {
            let fp = fret;
            let mut ibig = 0usize;
            let mut del = 0.0f64;
            let _ = writeln!(
                out,
                "  powell nthiter {}: fret = {}",
                self.base.num_iterations as i32,
                pf(fret)
            );
            // powell_iteration_func: a calloc'ed float copy (1-based); a no-op when unset.
            to_float_1based(p, &mut float_parms);
            functor.funcs().powell_iteration(&float_parms, n as i32);
            for i in 0..n {
                if verbose {
                    let _ = writeln!(
                        out,
                        "    param {:>3}  nthiter {:>2}",
                        i as i32, self.base.num_iterations as i32
                    );
                }
                for j in 0..n {
                    xit[j] = xi[j * n + i];
                }
                let fptt = fret;
                let (xx, fr) = self.line_minimize(functor, p, &xit, self.initial_step, true, out);
                fret = fr;
                if verbose {
                    let _ = writeln!(out, "  min xx={}, fret={}", pg(xx), pg(fret));
                }
                uninit(p, &xit, xx);
                if (fptt - fret).abs() > del {
                    del = (fptt - fret).abs();
                    ibig = i;
                }
                if verbose {
                    let _ = writeln!(out, "  i = {}  fp={} fret={}", i as i32, pf(fp), pf(fret));
                }
            }
            if verbose {
                let _ = writeln!(
                    out,
                    "  niters {}  fp={} fret={}",
                    self.base.num_iterations as i32,
                    pf(fp),
                    pf(fret)
                );
                let _ = writeln!(
                    out,
                    "  check: {} <= {}",
                    pf(2.0 * (fp - fret).abs()),
                    pf(self.base.ftol * (fp.abs() + fret.abs()))
                );
            }
            if 2.0 * (fp - fret).abs() <= self.base.ftol * (fp.abs() + fret.abs()) {
                if verbose {
                    let _ = writeln!(out, "  converged");
                }
                return ReturnCode::ConvergedFtol;
            }
            if self.base.num_iterations == maxfev {
                return ReturnCode::FailedUserRequest;
            }
            for j in 0..n {
                ptt[j] = 2.0 * p[j] - pt[j];
                xit[j] = p[j] - pt[j];
                pt[j] = p[j];
            }
            let fptt = functor.f(&ptt);
            self.base.report_eval(fret);
            if fptt < fp {
                let t =
                    2.0 * (fp - 2.0 * fret + fptt) * sqr(fp - fret - del) - del * sqr(fp - fptt);
                if t < 0.0 {
                    let (xx, fr) = self.line_minimize(functor, p, &xit, 1.0, false, out);
                    fret = fr;
                    uninit(p, &xit, xx);
                    for j in 0..n {
                        xi[j * n + ibig] = xi[j * n + n - 1];
                        xi[j * n + n - 1] = xit[j];
                    }
                }
            }
            let mut s = Std {
                out: &mut *out,
                err: &mut *err,
            };
            self.base.report_iter(&mut s);
        }
        if verbose {
            let _ = writeln!(out, "  niters {}", self.base.num_iterations as i32);
        }
        ReturnCode::FailedUserRequest
    }

    /// One line minimization along `dx` from `x0` (`fs_powell_1dfun` with `vnl_brent`:
    /// `bracket_minimum(&ax, &xx, &bx)` from `ax = bx = 0`, `xx = step`, then
    /// `minimize_given_bounds(bx, xx, ax, linmin_xtol_, &xx)`); every evaluation is reported.
    /// `trace`: the verbose `bracketing` lines (the first search of an iteration's loop only).
    /// Returns `(xx, fret)`.
    fn line_minimize(
        &mut self,
        functor: &mut FsCostFunction,
        x0: &[f64],
        dx: &[f64],
        step: f64,
        trace: bool,
        out: &mut dyn Write,
    ) -> (f64, f64) {
        let n = x0.len();
        let verbose = self.base.verbose && trace;
        let linmin_xtol = self.linmin_xtol;
        let base = &mut self.base;
        let mut tmpx = vec![0.0f64; n];
        let mut f1d = |x: &[f64]| {
            let lambda = x[0];
            for i in 0..n {
                tmpx[i] = x0[i] + lambda * dx[i];
            }
            let e = functor.f(&tmpx);
            base.report_eval(e);
            e
        };
        let mut brent = Brent::new();
        let (ax, xx, bx) = (0.0f64, step, 0.0f64);
        if verbose {
            let _ = writeln!(out, "  bracketing");
        }
        let ((ax, xx, bx), _) = brent.bracket_minimum(&mut f1d, ax, xx, bx);
        if verbose {
            let _ = writeln!(out, "  bracket ax={} xx={} bx={}", pg(ax), pg(xx), pg(bx));
        }
        let (fret, xx) = brent.minimize_given_bounds(&mut f1d, bx, xx, ax, linmin_xtol);
        (xx, fret)
    }
}

/// `fs_powell_1dfun::uninit(xx, p)` with `x0_ = p`: `p[i] = x0[i] + xx * dx[i]`.
fn uninit(p: &mut [f64], dx: &[f64], xx: f64) {
    for (pi, &d) in p.iter_mut().zip(dx) {
        *pi += xx * d;
    }
}

/// Two vectors equal bit for bit.
fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[inline]
fn sqr(x: f64) -> f64 {
    x * x
}

/// `fs_lbfgs_observer` (with `fs_lbfgs_subject`'s one observer): counts the best points
/// reported and hands each, as floats (1-based), to the program's step function and user
/// callback ([`FsFunctions::step`], [`FsFunctions::user_callback`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsLbfgsObserver {
    /// `mNumberOfOptimalUpdates` (`getNumberOfOptimalUpdates`).
    pub number_of_optimal_updates: i32,
}

impl FsLbfgsObserver {
    /// `update(bestF, bestX)`: the step function with the count so far and `(float)bestF`, then
    /// the user callback; then the count goes up.
    pub fn update(&mut self, funcs: &mut dyn FsFunctions, best_f: f64, best_x: &[f64]) {
        let mut current = Vec::with_capacity(best_x.len() + 1);
        to_float_1based(best_x, &mut current);
        funcs.step(self.number_of_optimal_updates, best_f as f32, &current);
        funcs.user_callback(&current);
        self.number_of_optimal_updates += 1;
    }
}

/// `fs_lbfgs`: the L-BFGS driver FreeSurfer copied from `vnl_lbfgs`, reporting each new best
/// point to its observer.
#[derive(Clone, Debug, PartialEq)]
pub struct FsLbfgs {
    /// `vnl_nonlinear_minimizer`'s settings and counters (`gtol` is the gradient tolerance;
    /// `xtol` and `ftol` are not used).
    pub base: NonlinearMinimizer,
    /// `memory` (default 5): the corrections kept.
    pub memory: i32,
    /// `line_search_accuracy` (default 0.9): the line search's `gtol`.
    pub line_search_accuracy: f64,
    /// `default_step_length` (default 1.0): the line search's `stpinit`.
    pub default_step_length: f64,
}

impl Default for FsLbfgs {
    fn default() -> Self {
        Self::new()
    }
}

impl FsLbfgs {
    /// `fs_lbfgs(f)` with `init_parameters()`.
    pub fn new() -> Self {
        FsLbfgs {
            base: NonlinearMinimizer::new(),
            memory: 5,
            line_search_accuracy: 0.9,
            default_step_length: 1.0,
        }
    }

    /// `fs_lbfgs::minimize(x)`: L-BFGS from `x` (replaced by the best point found) by reverse
    /// communication with netlib's `lbfgs_` (`larmorx_core::vnl_lbfgs`: no caller diagonal, `xtol`
    /// 1e-16, `eps = gtol`; `IPRINT` (1, 1) under `DIAG_VERBOSE`, else (1, 0) with `trace` and
    /// (-1, 0) without), each better point reported to `observer`. Resets the iteration and
    /// evaluation counters (not the end error); stops with `true` when `lbfgs_` converges, with
    /// `false` on its error or after `maxfev` evaluations (`failure_code` `FAILED_USER_REQUEST`).
    /// With `verbose` it writes `vnl_lbfgs`' header and with `verbose` but not `trace` its
    /// progress on standard error. `check_derivatives_` (FreeSurfer never sets it) is not ported.
    /// **FS-37:** see the module's documentation.
    pub fn minimize(
        &mut self,
        g: &mut Globals,
        f: &mut FsCostFunction,
        mut observer: Option<&mut FsLbfgsObserver>,
        x: &mut Vec<f64>,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> bool {
        let reproduce = g.notices.reproduce();
        let mode = g.notices.mode();
        let n = i64::from(f.dim);
        let m = i64::from(self.memory);
        let nu = n.max(0) as usize;
        let mut gl = LbfgsGlobal::new();
        let mut gvec = vec![0.0f64; nu];
        let mut diagv = vec![0.0f64; nu];
        let wlen = (n * (2 * m + 1) + 2 * m).max(0) as usize;
        let mut w = vec![0.0f64; wlen];
        if self.base.verbose {
            let mag = x.iter().fold(0.0f64, |s, &v| s + v * v).sqrt();
            let _ = writeln!(
                err,
                "vnl_lbfgs: n = {n}, memory = {m}, Workspace = {wlen}[ {} MB], ErrorScale = {}, xnorm = {}",
                cfmt::ostream(wlen as f64 / 128.0 / 1024.0),
                cfmt::ostream(1.0),
                cfmt::ostream(mag)
            );
        }
        let we_trace = self.base.verbose && !self.base.trace;
        if we_trace {
            let _ = write!(err, "vnl_lbfgs: ");
        }
        // The best point: FreeSurfer records one only when an evaluation improves on the first
        // (FS-37); the default starts from the first point. Both see the same values.
        let start = x.clone();
        let mut best_f = 0.0f64;
        let mut best_x: Vec<f64> = x.clone();
        let mut improved = false;
        let mut last_f;
        let ok;
        self.base.num_evaluations = 0;
        self.base.num_iterations = 0;
        let mut iflag: i64 = 0;
        loop {
            let eps = self.base.gtol;
            let local_xtol = 1e-16f64;
            gl.gtol = self.line_search_accuracy;
            gl.stpinit = self.default_step_length;
            let fv = f.compute(x, &mut gvec);
            last_f = fv;
            if self.base.num_evaluations == 0 {
                self.base.start_error = fv;
                best_f = fv;
            } else if fv < best_f {
                best_x.clone_from(x);
                best_f = fv;
                improved = true;
                if let Some(o) = observer.as_deref_mut() {
                    o.update(f.funcs(), best_f, &best_x);
                }
            }
            let mut iprint = [if self.base.trace { 1 } else { -1 }, 0];
            if diag::verbose_on(g) {
                iprint = [1, 1];
            }
            let mut s = Std {
                out: &mut *out,
                err: &mut *err,
            };
            let call = lbfgs(
                n,
                m,
                x,
                fv,
                &gvec,
                false,
                &mut diagv,
                iprint,
                eps,
                local_xtol,
                &mut w,
                &mut iflag,
                &mut gl,
                mode,
                &mut s,
            );
            g.notices.extend(call.notices);
            self.base.num_iterations = self.base.num_iterations.wrapping_add(1);
            if we_trace {
                let _ = write!(err, "{iflag}:{} ", cfmt::ostream(fv));
            }
            if iflag == 0 {
                ok = true;
                break;
            }
            if iflag < 0 {
                ok = false;
                break;
            }
            self.base.num_evaluations += 1;
            if self.base.num_evaluations > i64::from(self.base.maxfev as i32) {
                self.base.failure_code = ReturnCode::FailedUserRequest;
                ok = false;
                break;
            }
        }
        // FreeSurfer's return (`end_error_ = f; x = best_x`; on a failure only when a best point
        // was recorded, else x stays at the last trial point and end_error_ unchanged): with no
        // best point a successful return empties x (`None`: OpenDFPMin then crashes).
        let fs_x: Option<Vec<f64>> = if improved {
            Some(best_x.clone())
        } else if ok {
            None
        } else {
            Some(x.clone())
        };
        let fs_end = if ok || improved {
            last_f
        } else {
            self.base.end_error
        };
        let differs = fs_x.as_deref().is_none_or(|v| !same_bits(v, &best_x))
            || fs_end.to_bits() != best_f.to_bits();
        if differs {
            let detail = match (reproduce, fs_x.is_none()) {
                (false, _) => {
                    "fs_lbfgs returns its best point and that point's value (FreeSurfer: the last value evaluated, and without an improvement on the starting point no point, or the line search's last trial point)"
                }
                (true, true) => {
                    "fs_lbfgs converged without improving on its starting point: FreeSurfer empties the parameters (OpenDFPMin then crashes); the starting point is kept"
                }
                (true, false) => {
                    "fs_lbfgs returns the last value evaluated and, without an improvement on its starting point, the line search's last trial point (FreeSurfer's)"
                }
            };
            g.notices.add(&FS_37, detail);
        }
        if reproduce {
            *x = fs_x.unwrap_or(start);
            self.base.end_error = fs_end;
        } else {
            *x = best_x;
            self.base.end_error = best_f;
        }
        if we_trace {
            let _ = writeln!(err, "done");
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    struct Quad {
        evals: usize,
        steps: Vec<(i32, f32)>,
    }

    impl FsFunctions for Quad {
        fn function(&mut self, p: &[f32]) -> f32 {
            self.evals += 1;
            let (a, b) = (p[1] - 1.0, p[2] + 2.0);
            a * a + 3.0 * b * b + 0.5 * a * b
        }
        fn gradient(&mut self, p: &[f32], g: &mut [f32]) {
            let (a, b) = (p[1] - 1.0, p[2] + 2.0);
            g[1] = 2.0 * a + 0.5 * b;
            g[2] = 6.0 * b + 0.5 * a;
        }
        fn step(&mut self, itno: i32, sse: f32, _p: &[f32]) {
            self.steps.push((itno, sse));
        }
    }

    #[test]
    fn powell_minimizes_and_prints_its_header() {
        let mut q = Quad {
            evals: 0,
            steps: Vec::new(),
        };
        let mut cf = FsCostFunction::new(&mut q);
        let mut pw = FsPowell::new();
        let mut p = vec![0.0, 0.0];
        let mut xi = vec![1.0, 0.0, 0.0, 1.0];
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = pw.minimize(&mut cf, &mut p, &mut xi, &mut o, &mut e);
        assert_eq!(code, ReturnCode::ConvergedFtol);
        assert!((p[0] - 1.0).abs() < 1e-3 && (p[1] + 2.0).abs() < 1e-3, "{p:?}");
        let text = String::from_utf8(o).unwrap();
        assert!(text.starts_with(
            "fs_powell::minimize\n  nparams 2\n  maxfev 2000\n  ftol   0.000000\n  linmin_xtol_   0.000100\n  powell nthiter 0: fret = 12.000000\n"
        ));
        assert!(e.is_empty());
    }

    #[test]
    fn lbfgs_reports_best_points() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let mut q = Quad {
                evals: 0,
                steps: Vec::new(),
            };
            let mut cf = FsCostFunction::with_gradient(&mut q, 2);
            let mut lb = FsLbfgs::new();
            let mut obs = FsLbfgsObserver::default();
            let mut x = vec![0.0, 0.0];
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let ok = lb.minimize(&mut g, &mut cf, Some(&mut obs), &mut x, &mut o, &mut e);
            assert!(ok);
            assert!((x[0] - 1.0).abs() < 1e-3 && (x[1] + 2.0).abs() < 1e-3, "{x:?}");
            assert!(obs.number_of_optimal_updates > 0);
            assert_eq!(q.steps.len() as i32, obs.number_of_optimal_updates);
        }
    }
}

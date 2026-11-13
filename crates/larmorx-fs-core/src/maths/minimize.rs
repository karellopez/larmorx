// SPDX-License-Identifier: Apache-2.0
//! The minimizers (spec §13): Powell's method with given directions (`OpenPowell`,
//! `OpenPowell2`) and the L-BFGS driver behind `OpenDFPMin`, over FreeSurfer's 1-based float
//! parameters.
//!
//! The cost functions are the caller's, as FreeSurfer's programs hand them over: they take the
//! parameters at `p[1..=n]` of a float slice (`p[0]` is 0 here: FreeSurfer never sets it, and the
//! functions never read it) and return a float; a gradient function writes `g[1..=n]`. The
//! minimizers run in double precision on VNL's routines as the main package ports them
//! (`larmorx_core::vnl_minimize`'s bracketing, Brent minimizer and counters,
//! `larmorx_core::vnl_lbfgs`'s `lbfgs`); every stored result of an operation follows x86-64's NaN
//! rules (`crate::sse`), so the points handed to the cost functions are the official build's on
//! every platform.
//!
//! Defects (their two modes in the spec): FS-34 (no directions), FS-35 and FS-36 (kept in both
//! modes), FS-37 (the L-BFGS passes' best point and end value), FS-38 (`OpenDFPMin`'s stop test),
//! FS-39 (the evaluation limit); VNL's own ITK-106 (L-BFGS's printout) and ITK-114 (Brent's
//! endless loop) come with the main package's ports.

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::{FS_34, FS_37, FS_38, FS_39};
use larmorx_core::vnl_lbfgs::{LbfgsGlobal, lbfgs};
use larmorx_core::vnl_minimize::{Brent, NonlinearMinimizer, ReturnCode};

use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse::{add64, mul64, narrow, sub64, widen};

/// A cost function: FreeSurfer's `float (*)(float *p)`, the parameters at `p[1..=n]`.
pub type CostFn<'a> = dyn FnMut(&[f32]) -> f32 + 'a;
/// A gradient function: FreeSurfer's `void (*)(float *p, float *g)`, writing `g[1..=n]`.
pub type GradientFn<'a> = dyn FnMut(&[f32], &mut [f32]) + 'a;
/// Powell's iteration callback (`powell_iteration_func`): a float copy of the parameters at
/// `[1..=n]` (`[0]` = 0) and `n`.
pub type IterationFn<'a> = dyn FnMut(&[f32], i32) + 'a;
/// `OpenDFPMin`'s step function: the observer's count so far, the best value (float) and a float
/// copy of the best point at `[1..=n]` (FreeSurfer's `void *` data is the closure's own).
pub type StepFn<'a> = dyn FnMut(i32, f32, &[f32]) + 'a;
/// `OpenDFPMin`'s user callback: a float copy of the best point at `[1..=n]`.
pub type UserFn<'a> = dyn FnMut(&[f32]) + 'a;

/// The process's streams for VNL's printing routines.
struct ProcStreams<'a>(&'a mut Process);

impl Streams for ProcStreams<'_> {
    fn out(&mut self, text: &str) {
        self.0.put_out(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        self.0.put_err(text.as_bytes());
    }
}

/// Element `i` of a caller's array (0 outside it, §1).
fn at(a: &[f32], i: usize) -> f32 {
    a.get(i).copied().unwrap_or(0.0)
}

/// Sets element `i` of a caller's array (dropped outside it, §1).
fn put(a: &mut [f32], i: usize, v: f32) {
    if let Some(x) = a.get_mut(i) {
        *x = v;
    }
}

/// The bridge's value at `x` (§13.1): `x` rounded to float into `p[1..=n]` (`p` reused: a fresh
/// copy for each call, as FreeSurfer's), the function called, its result widened.
fn value_at(func: &mut CostFn, p: &mut [f32], x: &[f64]) -> f64 {
    p[0] = 0.0;
    for (i, &v) in x.iter().enumerate() {
        p[i + 1] = narrow(v);
    }
    widen(func(p))
}

/// The settings of Powell's method (§13.2; VNL's defaults by [`PowellSettings::default`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowellSettings {
    /// The relative tolerance on the cost (`ftol`, 1e-10).
    pub ftol: f64,
    /// The iteration bound (`maxfev`, 2000; the loop compares with it converted to an unsigned
    /// 32-bit number).
    pub maxfev: i64,
    /// The line minimizations' tolerance (1e-4).
    pub linmin_xtol: f64,
    /// The line minimizations' first step (1.0).
    pub initial_step: f64,
    pub verbose: bool,
    pub trace: bool,
}

impl Default for PowellSettings {
    fn default() -> Self {
        PowellSettings {
            ftol: 1e-8 * 0.01,
            maxfev: 2000,
            linmin_xtol: 1e-4,
            initial_step: 1.0,
            verbose: false,
            trace: false,
        }
    }
}

/// What Powell's method ended with: its return code and VNL's counters (the iteration count,
/// the evaluations, the start and end values).
#[derive(Clone, Debug, PartialEq)]
pub struct PowellRun {
    pub code: ReturnCode,
    pub counters: NonlinearMinimizer,
}

impl Process {
    /// One line minimization along `d` from `x0` with the first step `step` (§13.2): VNL's
    /// bracketing from 0 and `step`, then its Brent minimizer to `xtol`; every evaluation
    /// reported. `verbose` prints the bracketing lines. Returns `(xx, fret)`.
    #[allow(clippy::too_many_arguments)]
    fn line_minimize(
        &mut self,
        base: &mut NonlinearMinimizer,
        cost: &mut dyn FnMut(&[f64]) -> f64,
        x0: &[f64],
        d: &[f64],
        step: f64,
        xtol: f64,
        verbose: bool,
    ) -> (f64, f64) {
        let n = x0.len();
        let mut tmp = vec![0.0f64; n];
        let mut f1d = |l: &[f64]| {
            let lambda = l[0];
            for i in 0..n {
                tmp[i] = add64(x0[i], mul64(lambda, d[i]));
            }
            let e = cost(&tmp);
            base.report_eval(e);
            e
        };
        if verbose {
            self.put_out(b"  bracketing\n");
        }
        let mut brent = Brent::new();
        // vnl_brent's bracket_minimum(&ax, &xx, &bx) with ax = 0, xx = step, bx = 0: it brackets
        // from bx and xx, and gives ax the upper end, xx the interior point, bx the lower end.
        let ((ax, xx, bx), _) = brent.bracket_minimum(&mut f1d, 0.0, step, 0.0);
        if verbose {
            let t = format!(
                "  bracket ax={} xx={} bx={}\n",
                cfmt::g(ax, 6),
                cfmt::g(xx, 6),
                cfmt::g(bx, 6)
            );
            self.put_out(t.as_bytes());
        }
        let (fret, xmin) = brent.minimize_given_bounds(&mut f1d, bx, xx, ax, xtol);
        for nt in &brent.inner.notices {
            self.note(nt.defect, nt.detail.clone());
        }
        (xmin, fret)
    }

    /// Powell's method with given directions (§13.2): minimizes `cost` from `p` (n doubles,
    /// updated) along the columns of `xi` (n x n, row-major: element `(r, c)` at `r n + c`,
    /// updated). `iteration` is the program's iteration callback, if it set one.
    pub fn fs_powell(
        &mut self,
        cost: &mut dyn FnMut(&[f64]) -> f64,
        p: &mut [f64],
        xi: &mut [f64],
        s: &PowellSettings,
        mut iteration: Option<&mut IterationFn>,
    ) -> PowellRun {
        let n = p.len();
        let mut base = NonlinearMinimizer::new();
        base.ftol = s.ftol;
        base.maxfev = s.maxfev;
        base.verbose = s.verbose;
        base.trace = s.trace;
        let mut fret = cost(p);
        base.report_eval(fret);
        let mut pt = p.to_vec();
        let mut ptt = vec![0.0f64; n];
        let mut xit = vec![0.0f64; n];
        let head = format!(
            "fs_powell::minimize\n  nparams {n}\n  maxfev {}\n  ftol   {}\n  linmin_xtol_   {}\n",
            s.maxfev,
            cfmt::f(s.ftol, 6),
            cfmt::f(s.linmin_xtol, 6)
        );
        self.put_out(head.as_bytes());
        let bound = s.maxfev as u32;
        let verbose = s.verbose;
        while base.num_iterations < bound {
            let count = base.num_iterations;
            let fp = fret;
            let mut ibig = 0usize;
            let mut del = 0.0f64;
            let t = format!("  powell nthiter {count}: fret = {}\n", cfmt::f(fret, 6));
            self.put_out(t.as_bytes());
            if let Some(cb) = iteration.as_deref_mut() {
                let mut copy = vec![0.0f32; n + 1];
                for (i, &v) in p.iter().enumerate() {
                    copy[i + 1] = narrow(v);
                }
                cb(&copy, n as i32);
            }
            for i in 0..n {
                if verbose {
                    let t = format!("    param {i:3}  nthiter {count:2}\n");
                    self.put_out(t.as_bytes());
                }
                for j in 0..n {
                    xit[j] = xi[j * n + i];
                }
                let fptt = fret;
                let (xx, fr) = self.line_minimize(
                    &mut base,
                    cost,
                    p,
                    &xit,
                    s.initial_step,
                    s.linmin_xtol,
                    verbose,
                );
                fret = fr;
                if verbose {
                    let t = format!("  min xx={}, fret={}\n", cfmt::g(xx, 6), cfmt::g(fret, 6));
                    self.put_out(t.as_bytes());
                }
                for (k, pk) in p.iter_mut().enumerate() {
                    *pk = add64(*pk, mul64(xx, xit[k]));
                }
                if (fptt - fret).abs() > del {
                    del = sub64(fptt, fret).abs();
                    ibig = i;
                }
                if verbose {
                    let t = format!(
                        "  i = {i}  fp={} fret={}\n",
                        cfmt::f(fp, 6),
                        cfmt::f(fret, 6)
                    );
                    self.put_out(t.as_bytes());
                }
            }
            let lhs = mul64(2.0, sub64(fp, fret).abs());
            let rhs = mul64(s.ftol, add64(fp.abs(), fret.abs()));
            if verbose {
                let t = format!(
                    "  niters {count}  fp={} fret={}\n  check: {} <= {}\n",
                    cfmt::f(fp, 6),
                    cfmt::f(fret, 6),
                    cfmt::f(lhs, 6),
                    cfmt::f(rhs, 6)
                );
                self.put_out(t.as_bytes());
            }
            if lhs <= rhs {
                if verbose {
                    self.put_out(b"  converged\n");
                }
                return PowellRun {
                    code: ReturnCode::ConvergedFtol,
                    counters: base,
                };
            }
            for j in 0..n {
                ptt[j] = sub64(mul64(2.0, p[j]), pt[j]);
                xit[j] = sub64(p[j], pt[j]);
                pt[j] = p[j];
            }
            let fptt = cost(&ptt);
            base.report_eval(fret);
            if fptt < fp {
                let t =
                    2.0 * (fp - 2.0 * fret + fptt) * sqr(fp - fret - del) - del * sqr(fp - fptt);
                if t < 0.0 {
                    let (xx, fr) =
                        self.line_minimize(&mut base, cost, p, &xit, 1.0, s.linmin_xtol, false);
                    fret = fr;
                    for (k, pk) in p.iter_mut().enumerate() {
                        *pk = add64(*pk, mul64(xx, xit[k]));
                    }
                    for j in 0..n {
                        xi[j * n + ibig] = xi[j * n + n - 1];
                        xi[j * n + n - 1] = xit[j];
                    }
                }
            }
            base.report_iter(&mut ProcStreams(self));
        }
        if verbose {
            let t = format!("  niters {}\n", base.num_iterations);
            self.put_out(t.as_bytes());
        }
        // FS-35 (kept in both modes, not noted): VNL gives FAILED_TOO_MANY_ITERATIONS here.
        PowellRun {
            code: ReturnCode::FailedUserRequest,
            counters: base,
        }
    }

    /// Powell's start and end for `OpenPowell` and `OpenPowell2`: the widened parameters and
    /// directions (the identity without them), the run, then the count, the end value, the
    /// parameters and the directions written back (FS-34 without directions).
    #[allow(clippy::too_many_arguments)]
    fn powell_open(
        &mut self,
        p: &mut [f32],
        xi: Option<&mut [f32]>,
        n: i32,
        iter: &mut i32,
        fret: &mut f32,
        func: &mut CostFn,
        s: &PowellSettings,
        iteration: Option<&mut IterationFn>,
    ) -> ReturnCode {
        let nn = n.max(0) as usize;
        let mut x: Vec<f64> = (1..=nn).map(|i| widen(at(p, i))).collect();
        let mut dirs = vec![0.0f64; nn * nn];
        match &xi {
            Some(m) => {
                for (k, d) in dirs.iter_mut().enumerate() {
                    *d = widen(at(m, k));
                }
            }
            None => {
                for i in 0..nn {
                    dirs[i * nn + i] = 1.0;
                }
            }
        }
        let mut pbuf = vec![0.0f32; nn + 1];
        let mut cost = |v: &[f64]| value_at(func, &mut pbuf, v);
        let run = self.fs_powell(&mut cost, &mut x, &mut dirs, s, iteration);
        *iter = run.counters.num_iterations as i32;
        *fret = narrow(run.counters.end_error);
        for (i, &v) in x.iter().enumerate() {
            put(p, i + 1, narrow(v));
        }
        match xi {
            Some(m) => {
                for (k, &d) in dirs.iter().enumerate() {
                    put(m, k, narrow(d));
                }
            }
            None => {
                let what = if self.mode().reproduce() {
                    "Powell's directions, absent, not written (FreeSurfer crashes writing them)"
                } else {
                    "Powell's directions, absent, not written"
                };
                self.note(&FS_34, what);
            }
        }
        run.code
    }

    /// `OpenPowell` (§13.4): Powell's method with VNL's defaults from `p[1..=n]` along the
    /// directions `xi` (n x n row-major, element `(r, c)` = FreeSurfer's `xi[r + 1][c + 1]`;
    /// none: the identity), writing the iteration count, the end value, the parameters and the
    /// directions back. `tol` sets VNL's x tolerance, which Powell never reads (FS-36, kept).
    #[allow(clippy::too_many_arguments)]
    pub fn open_powell(
        &mut self,
        p: &mut [f32],
        xi: Option<&mut [f32]>,
        n: i32,
        tol: f32,
        iter: &mut i32,
        fret: &mut f32,
        func: &mut CostFn,
        iteration: Option<&mut IterationFn>,
    ) -> Result<(), Stop> {
        let _ = tol; // FS-36: the x tolerance, never read by Powell
        let s = PowellSettings::default();
        let code = self.powell_open(p, xi, n, iter, fret, func, &s, iteration);
        match code {
            ReturnCode::FailedTooManyIterations => {
                self.error_exit(codes::BAD_PARM, b"powell exceeding maximum iterations.")
            }
            ReturnCode::ErrorFailure
            | ReturnCode::ErrorDodgyInput
            | ReturnCode::FailedFtolTooSmall
            | ReturnCode::FailedXtolTooSmall
            | ReturnCode::FailedGtolTooSmall => self.error_exit(codes::BAD_PARM, b"powell error."),
            _ => Ok(()),
        }
    }

    /// `OpenPowell2` (§13.5): Powell's method with `ftol`, the line tolerance `linmin_xtol` and
    /// `maxfev` = `maxiter`, verbose (and tracing) when `Gdiag_no > 0`; the count, end value,
    /// parameters and directions always written back. Returns 1 for too many iterations or an
    /// error (never: FS-35), else 0.
    #[allow(clippy::too_many_arguments)]
    pub fn open_powell2(
        &mut self,
        p: &mut [f32],
        xi: Option<&mut [f32]>,
        n: i32,
        ftol: f32,
        linmin_xtol: f32,
        maxiter: i32,
        iter: &mut i32,
        fret: &mut f32,
        func: &mut CostFn,
        iteration: Option<&mut IterationFn>,
    ) -> i32 {
        let verbose = self.gdiag_no > 0;
        let s = PowellSettings {
            ftol: widen(ftol),
            maxfev: i64::from(maxiter),
            linmin_xtol: widen(linmin_xtol),
            initial_step: 1.0,
            verbose,
            trace: verbose,
        };
        let code = self.powell_open(p, xi, n, iter, fret, func, &s, iteration);
        match code {
            ReturnCode::FailedTooManyIterations => {
                self.put_out(b"powell exceeded maximum iterations\n");
                1
            }
            ReturnCode::ErrorFailure
            | ReturnCode::ErrorDodgyInput
            | ReturnCode::FailedFtolTooSmall
            | ReturnCode::FailedXtolTooSmall
            | ReturnCode::FailedGtolTooSmall => {
                let t = format!("powell error {}\n", code.value());
                self.put_out(t.as_bytes());
                1
            }
            _ => 0,
        }
    }
}

#[inline]
fn sqr(x: f64) -> f64 {
    x * x
}

/// The L-BFGS driver's settings and VNL's counters (§13.3).
#[derive(Clone, Debug, PartialEq)]
pub struct LbfgsDriver {
    /// The number of unknowns (the bridge's `n`).
    pub n: usize,
    /// The memory `m` (5).
    pub memory: i64,
    /// The line search's accuracy (0.9).
    pub line_search_accuracy: f64,
    /// The line search's first step (1.0).
    pub default_step: f64,
    /// The gradient tolerance (1e-5).
    pub gtol: f64,
    /// The evaluation limit (2000).
    pub maxfev: i64,
    pub verbose: bool,
    pub trace: bool,
    pub num_evaluations: i64,
    pub num_iterations: i64,
    pub start_error: f64,
    pub end_error: f64,
    pub failure_code: ReturnCode,
}

/// How one L-BFGS pass ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LbfgsPass {
    /// Whether it succeeded (`lbfgs` returned 0).
    pub ok: bool,
    /// Whether it succeeded without any improvement on its first value (FreeSurfer's empty
    /// parameters, FS-37).
    pub empty: bool,
}

impl LbfgsDriver {
    /// The driver's settings for `n` unknowns (VNL's defaults), its counters 0.
    pub fn new(n: usize) -> LbfgsDriver {
        LbfgsDriver {
            n,
            memory: 5,
            line_search_accuracy: 0.9,
            default_step: 1.0,
            gtol: 1e-5,
            maxfev: 2000,
            verbose: false,
            trace: false,
            num_evaluations: 0,
            num_iterations: 0,
            start_error: 0.0,
            end_error: 0.0,
            failure_code: ReturnCode::ErrorFailure,
        }
    }

    /// One pass from `x` (updated): `compute` gives the value and the gradient at a point,
    /// `observer` is told each new best value and point. The point and the end value it leaves
    /// follow FS-37 (§13.3): the best point and the best value by default, FreeSurfer's with
    /// `reproduce` (its empty parameters there the starting point).
    pub fn minimize(
        &mut self,
        pr: &mut Process,
        compute: &mut dyn FnMut(&[f64], &mut [f64]) -> f64,
        x: &mut [f64],
        observer: &mut dyn FnMut(f64, &[f64]),
    ) -> LbfgsPass {
        let n = self.n;
        let m = self.memory;
        let wlen = (n as i64) * (2 * m + 1) + 2 * m;
        let mut w = vec![0.0f64; wlen.max(0) as usize];
        let mut diag = vec![0.0f64; n];
        let mut g = vec![0.0f64; n];
        self.num_evaluations = 0;
        self.num_iterations = 0;
        if self.verbose {
            let xnorm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
            let t = format!(
                "vnl_lbfgs: n = {n}, memory = {m}, Workspace = {wlen}[ {} MB], ErrorScale = 1, xnorm = {}\n",
                cfmt::g(wlen as f64 / 128.0 / 1024.0, 6),
                cfmt::g(xnorm, 6)
            );
            pr.put_err(t.as_bytes());
        }
        let chatty = self.verbose && !self.trace;
        if chatty {
            pr.put_err(b"vnl_lbfgs: ");
        }
        let iprint = if pr.gdiag & DIAG_VERBOSE != 0 {
            [1, 1]
        } else if self.trace {
            [1, 0]
        } else {
            [-1, 0]
        };
        let mode = pr.mode();
        let start = x.to_vec();
        let mut gl = LbfgsGlobal::new();
        let mut iflag = 0i64;
        let mut best: Option<Vec<f64>> = None;
        let mut best_f = 0.0f64;
        let mut last_f;
        let ok;
        loop {
            gl.gtol = self.line_search_accuracy;
            gl.stpinit = self.default_step;
            let f = compute(x, &mut g);
            last_f = f;
            if self.num_evaluations == 0 {
                self.start_error = f;
                best_f = f;
            } else if f < best_f {
                best = Some(x.to_vec());
                best_f = f;
                observer(best_f, x);
            }
            let call = lbfgs(
                n as i64,
                m,
                x,
                f,
                &g,
                false,
                &mut diag,
                iprint,
                self.gtol,
                1e-16,
                &mut w,
                &mut iflag,
                &mut gl,
                mode,
                &mut ProcStreams(pr),
            );
            for nt in &call.notices {
                pr.note(nt.defect, nt.detail.clone());
            }
            self.num_iterations += 1;
            if chatty {
                let t = format!("{iflag}:{} ", cfmt::g(f, 6));
                pr.put_err(t.as_bytes());
            }
            if iflag == 0 {
                ok = true;
                break;
            }
            if iflag < 0 {
                ok = false;
                break;
            }
            self.num_evaluations += 1;
            if self.num_evaluations > i64::from(self.maxfev as i32) {
                self.failure_code = ReturnCode::FailedUserRequest;
                ok = false;
                break;
            }
        }
        if chatty {
            pr.put_err(b"done\n");
        }
        // FS-37: FreeSurfer's returns, and the default's (the start the best until a smaller
        // value; the best point and value at every return).
        let fs: (Option<Vec<f64>>, f64) = match (&best, ok) {
            (Some(b), _) => (Some(b.clone()), last_f),
            (None, true) => (None, last_f),
            (None, false) => (Some(x.to_vec()), self.end_error),
        };
        let empty = ok && best.is_none();
        let def_x = best.clone().unwrap_or_else(|| start.clone());
        let fs_x = fs.0.clone().unwrap_or_else(|| start.clone());
        let same = fs.1.to_bits() == best_f.to_bits()
            && fs_x
                .iter()
                .zip(&def_x)
                .all(|(a, b)| a.to_bits() == b.to_bits());
        if !same || empty {
            let what = match (mode.reproduce(), empty) {
                (true, true) => {
                    "a successful L-BFGS pass without improvement: FreeSurfer's empty parameters (a crash) replaced by the starting point"
                }
                (true, false) => {
                    "an L-BFGS pass's end value and point as FreeSurfer gives them (the last value; without improvement the line search's last trial point)"
                }
                (false, _) => {
                    "an L-BFGS pass gave its best point and best value (the starting point the best until a smaller value)"
                }
            };
            pr.note(&FS_37, what);
        }
        if mode.reproduce() {
            x.copy_from_slice(&fs_x);
            self.end_error = fs.1;
        } else {
            x.copy_from_slice(&def_x);
            self.end_error = best_f;
        }
        LbfgsPass { ok, empty }
    }
}

impl Process {
    /// `OpenDFPMin` (§13.6): L-BFGS passes (memory 7, line-search accuracy 0.1, `gtol` = `tol`)
    /// from `p[1..=n]` while the last pass succeeded and the stop test (FS-38) says continue;
    /// `fret` is the caller's value in and the end value out. `step` and `user` are told every
    /// new best point (one observer for the whole call). Returns 0, or the failure's code (-1:
    /// the line search failed; 9: the evaluation limit, FS-39).
    #[allow(clippy::too_many_arguments)]
    pub fn open_dfp_min(
        &mut self,
        p: &mut [f32],
        n: i32,
        tol: f32,
        iter: &mut i32,
        fret: &mut f32,
        func: &mut CostFn,
        dfunc: &mut GradientFn,
        mut step: Option<&mut StepFn>,
        mut user: Option<&mut UserFn>,
    ) -> Result<i32, Stop> {
        let nn = n.max(0) as usize;
        let mut x: Vec<f64> = (1..=nn).map(|i| widen(at(p, i))).collect();
        let mut driver = LbfgsDriver::new(nn);
        driver.memory = 7;
        driver.line_search_accuracy = 0.1;
        driver.gtol = widen(tol);
        let mut pbuf = vec![0.0f32; nn + 1];
        let mut gbuf = vec![0.0f32; nn + 1];
        let mut compute = |v: &[f64], g: &mut [f64]| -> f64 {
            let f = value_at(func, &mut pbuf, v);
            gbuf.iter_mut().for_each(|e| *e = 0.0);
            dfunc(&pbuf, &mut gbuf);
            for (i, gi) in g.iter_mut().enumerate() {
                *gi = widen(gbuf[i + 1]);
            }
            f
        };
        let mut count = 0i32;
        let mut observer = |best_f: f64, bx: &[f64]| {
            let mut copy = vec![0.0f32; nn + 1];
            for (i, &v) in bx.iter().enumerate() {
                copy[i + 1] = narrow(v);
            }
            if let Some(s) = step.as_deref_mut() {
                s(count, narrow(best_f), &copy);
            }
            if let Some(u) = user.as_deref_mut() {
                u(&copy);
            }
            count += 1;
        };
        let reproduce = self.mode().reproduce();
        let first = *fret;
        let mut previous = *fret;
        let ok;
        loop {
            let pass = driver.minimize(self, &mut compute, &mut x, &mut observer);
            if !pass.ok {
                ok = false;
                break;
            }
            if pass.empty && reproduce {
                // FreeSurfer crashes here (FS-37): the starting point, and the passes stop.
                ok = true;
                break;
            }
            let current = narrow(driver.end_error);
            let fs_stop = (first - current) / first <= tol;
            // A NaN stops (the negated comparison is the test's NaN rule).
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            let def_stop = !((previous - current) / previous > tol);
            if fs_stop != def_stop {
                let what = if reproduce {
                    "OpenDFPMin's stop test compared with the caller's starting value, not the previous pass's"
                } else {
                    "OpenDFPMin's stop test compared with the previous pass's value"
                };
                self.note(&FS_38, what);
            }
            if if reproduce { fs_stop } else { def_stop } {
                ok = true;
                break;
            }
            previous = current;
        }
        let write =
            |iter: &mut i32, fret: &mut f32, p: &mut [f32], count: i32, end: f64, x: &[f64]| {
                *iter = count;
                *fret = narrow(end);
                for (i, &v) in x.iter().enumerate() {
                    put(p, i + 1, narrow(v));
                }
            };
        if ok {
            write(iter, fret, p, count, driver.end_error, &x);
            return Ok(0);
        }
        let code = driver.failure_code;
        match code {
            ReturnCode::ErrorFailure => {
                write(iter, fret, p, count, driver.end_error, &x);
                Ok(-1)
            }
            ReturnCode::ErrorDodgyInput => {
                self.error_exit(
                    codes::BAD_PARM,
                    b"quasi-Newton minimization (lbfsg) dodgy input",
                )?;
                Ok(code.value())
            }
            ReturnCode::FailedFtolTooSmall
            | ReturnCode::FailedXtolTooSmall
            | ReturnCode::FailedGtolTooSmall => {
                self.error_exit(
                    codes::BAD_PARM,
                    b"quasi-Newton minimization (lbfsg) tolerances too small",
                )?;
                Ok(code.value())
            }
            ReturnCode::FailedUserRequest => {
                if reproduce {
                    self.note(
                        &FS_39,
                        "the L-BFGS evaluation limit: nothing written (the parameters, count and value kept)",
                    );
                } else {
                    self.note(
                        &FS_39,
                        "the L-BFGS evaluation limit: the parameters, count and value written",
                    );
                    write(iter, fret, p, count, driver.end_error, &x);
                }
                Ok(code.value())
            }
            _ => Ok(code.value()),
        }
    }
}

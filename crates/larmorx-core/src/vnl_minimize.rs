// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's derivative-free minimizers, as FreeSurfer 8.2.0 calls them through ITK 4.13.2's VNL:
//!
//! - [`fit_parabola`], [`bracket_minimum`]: Tim Cootes' bracketing of a 1-D minimum
//!   (`vnl_bracket_minimum`, with `vnl_fit_parabola`'s parabolic step);
//! - [`BrentMinimizer`]: Brent's 1-D minimization on a bracket (`vnl_brent_minimizer`:
//!   `minimize_given_bounds`, `minimize_given_bounds_and_one_f`, `minimize_given_bounds_and_all_f`,
//!   `minimize`), and [`Brent`], the deprecated `vnl_brent` wrapper that FreeSurfer's
//!   `fs_powell` uses (`bracket_minimum` with its reversed arguments, `minimize_given_bounds`
//!   with a tolerance);
//! - [`NonlinearMinimizer`]: the state and counters of `vnl_nonlinear_minimizer` (tolerances,
//!   evaluation and iteration counts, `report_eval`, `report_iter` with its verbose line) and its
//!   [`ReturnCode`]s;
//! - [`Powell`]: `vnl_powell::minimize` (Powell's direction-set method on the new Brent
//!   minimizer), as `mri_robust_register`'s Powell registration calls it.
//!
//! Every expression is evaluated in double precision in the C++ order (no fused multiply-add:
//! the official binaries are built for baseline x86-64), so the evaluated points and the result
//! are bit-identical to VNL's on every platform; `validation/probes/fs-vnl` compares them with
//! ITK 4.13.2's own VNL built by GCC 8.4.0 with FreeSurfer's flags. The cost function is the
//! caller's (`vnl_cost_function::f`); it receives the same points in the same order.
//!
//! Ported from VXL as bundled with ITK v4.13.2 (BSD licence; the same code in v5.4.5 but for
//! `nullptr` and export macros, `docs/freesurfer/third-party.md` §10):
//! `core/vnl/algo/vnl_bracket_minimum.cxx`, `vnl_fit_parabola.h`, `vnl_brent_minimizer.cxx`,
//! `vnl_brent.cxx`, `vnl_powell.{h,cxx}`, `core/vnl/vnl_nonlinear_minimizer.{h,cxx}`.

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK v4.13.2):
//
//   Copyright 2000-2013 VXL Contributors
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions
//   are met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the names of the copyright holders nor the names of their
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//   FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//   COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//   INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//   HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//   STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//   OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::cfmt::{self, Streams};

/// `vnl_cost_function::f`: the function a minimizer evaluates, at a point of its dimension.
pub trait CostFunction {
    fn f(&mut self, x: &[f64]) -> f64;
}

impl<F: FnMut(&[f64]) -> f64> CostFunction for F {
    fn f(&mut self, x: &[f64]) -> f64 {
        self(x)
    }
}

/// `vnl_nonlinear_minimizer::ReturnCodes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ReturnCode {
    ErrorFailure = -1,
    ErrorDodgyInput = 0,
    ConvergedFtol = 1,
    ConvergedXtol = 2,
    ConvergedXftol = 3,
    ConvergedGtol = 4,
    /// `FAILED_TOO_MANY_ITERATIONS` (also spelled `TOO_MANY_ITERATIONS`).
    FailedTooManyIterations = 5,
    FailedFtolTooSmall = 6,
    FailedXtolTooSmall = 7,
    FailedGtolTooSmall = 8,
    FailedUserRequest = 9,
}

impl ReturnCode {
    /// The enumerator's integer value.
    pub fn value(self) -> i32 {
        self as i32
    }
}

/// `vnl_fit_parabola(xa, xb, xc, fa, fb, fc, p, q)`: the parabola through the three points has
/// its turning point at `xb + p / q`. Returns `(p, q)`.
#[inline]
pub fn fit_parabola(xa: f64, xb: f64, xc: f64, fa: f64, fb: f64, fc: f64) -> (f64, f64) {
    let x1 = xa - xb;
    let f1 = fa - fb;
    let x2 = xc - xb;
    let f2 = fc - fb;
    let p = x2 * x2 * f1 - x1 * x1 * f2;
    let q = 2.0 * (x2 * f1 - x1 * f2);
    (p, q)
}

/// A bracket `a < b < c` with `f(a) > f(b) < f(c)` (when [`bracket_minimum`] succeeds) and the
/// function's values there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bracket {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub fa: f64,
    pub fb: f64,
    pub fc: f64,
}

const GOLDEN_RATIO: f64 = 1.618_033_988_749_894_848;
const COMPL_GOLD: f64 = 0.381_966_011_250_105_152;

/// `vnl_bracket_minimum(fn, a, b, c, fa, fb, fc)`: from the starting points `a` and `b` (`b = a +
/// 1` if they are equal), steps downhill by the golden ratio, with parabolic steps where they help,
/// until the function rises again. Every evaluation is `f(&[x])`, as VNL's wrapper passes a
/// vector of one element. Loops for as long as the function keeps falling, as VNL does.
pub fn bracket_minimum(f: &mut dyn CostFunction, a: f64, b: f64) -> Bracket {
    const EPS: f64 = 1e-7;
    const EPS_SQR: f64 = 1e-14;
    let mut eval = |x: f64| f.f(&[x]);
    let (mut a, mut b) = (a, b);
    if b == a {
        b = a + 1.0;
    }
    let mut fa = eval(a);
    let mut fb = eval(b);
    // Arrange that fb <= fa.
    if fb > fa {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = b + GOLDEN_RATIO * (b - a);
    let mut fc = eval(c);
    let finish = |mut br: Bracket| {
        if br.a > br.c {
            std::mem::swap(&mut br.a, &mut br.c);
            std::mem::swap(&mut br.fa, &mut br.fc);
        }
        br
    };
    while fc < fb {
        let (p, mut q) = fit_parabola(a, b, c, fa, fb, fc);
        if q >= 0.0 && q < EPS_SQR {
            q = EPS_SQR;
        } else if q < 0.0 && q + EPS_SQR > 0.0 {
            q = -1.0 * EPS_SQR;
        }
        let mut du = p / q;
        let tol = EPS * (1.0 + cpp_max(b.abs(), c.abs()));
        if du >= 0.0 && du < tol {
            du = tol;
        } else if du < 0.0 && du + tol > 0.0 {
            du = -1.0 * tol;
        }
        let mut u = b + du;
        if (u - c) < tol && (u - c) >= 0.0 {
            u += tol;
        } else if (c - u) < tol && (c - u) >= 0.0 {
            u -= tol;
        }
        let u_limit = b + 100.0 * (c - b);
        let fu;
        if (u - b) * (c - u) > 0.0 {
            let fu1 = eval(u);
            if fu1 < fc {
                // The bracket is (b, u, c).
                return finish(Bracket {
                    a: b,
                    b: u,
                    c,
                    fa: fb,
                    fb: fu1,
                    fc,
                });
            } else if fu1 > fb {
                // The bracket is (a, b, u).
                return finish(Bracket {
                    a,
                    b,
                    c: u,
                    fa,
                    fb,
                    fc: fu1,
                });
            }
            u = c + GOLDEN_RATIO * (c - b);
            fu = eval(u);
        } else if (u - c) * (u_limit - u) > 0.0 {
            let fu1 = eval(u);
            if fu1 > fc {
                // The bracket is (b, c, u).
                return finish(Bracket {
                    a: b,
                    b: c,
                    c: u,
                    fa: fb,
                    fb: fc,
                    fc: fu1,
                });
            }
            fu = fu1;
        } else if (u_limit - c) * (u - u_limit) >= 0.0 {
            u = u_limit;
            fu = eval(u);
        } else {
            u = c + GOLDEN_RATIO * (c - b);
            fu = eval(u);
        }
        a = b;
        b = c;
        c = u;
        fa = fb;
        fb = fc;
        fc = fu;
    }
    finish(Bracket { a, b, c, fa, fb, fc })
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
#[inline]
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
#[inline]
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `vnl_brent_minimizer`: Brent's minimization of a 1-D function on a bracket, to the
/// tolerance `xtol` (`set_x_tolerance`, default `1e-6`) plus `1e-8·|x|`.
#[derive(Clone, Debug, PartialEq)]
pub struct BrentMinimizer {
    /// `xtol` (set by `set_x_tolerance`, which also sets `epsfcn`; only `xtol` is read here).
    pub xtol: f64,
    /// `f_at_last_minimum()`.
    pub f_at_last_minimum: f64,
}

impl Default for BrentMinimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl BrentMinimizer {
    /// The constructor: `set_x_tolerance(1e-6)`. `f_at_last_minimum` is not initialised by VNL;
    /// it is 0 here until a minimization sets it.
    pub fn new() -> Self {
        BrentMinimizer {
            xtol: 1e-6,
            f_at_last_minimum: 0.0,
        }
    }

    /// `minimize_given_bounds(ax, bx, cx)`: evaluates `f(bx)`, then
    /// [`BrentMinimizer::minimize_given_bounds_and_one_f`]. The bracket's order is not checked
    /// (VNL's assertions are compiled out by `NDEBUG`).
    pub fn minimize_given_bounds(&mut self, f: &mut dyn CostFunction, ax: f64, bx: f64, cx: f64) -> f64 {
        let fb = f.f(&[bx]);
        self.minimize_given_bounds_and_one_f(f, ax, bx, cx, fb)
    }

    /// `minimize_given_bounds_and_one_f(ax, bx, cx, fb)`: the minimum's `x`; its value in
    /// `f_at_last_minimum`.
    pub fn minimize_given_bounds_and_one_f(
        &mut self,
        f: &mut dyn CostFunction,
        ax: f64,
        bx: f64,
        cx: f64,
        fb: f64,
    ) -> f64 {
        let x = bx;
        let (fx, fw, fv) = (fb, fb, fb);
        self.brent_loop(f, ax, cx, x, fx, x, fw, x, fv, 0.0, 0.0)
    }

    /// `minimize_given_bounds_and_all_f(ax, bx, cx, fa, fb, fc)`: the second and third best
    /// points start at the bracket's ends, and the last two steps at the bracket's halves.
    #[allow(clippy::too_many_arguments)]
    pub fn minimize_given_bounds_and_all_f(
        &mut self,
        f: &mut dyn CostFunction,
        ax: f64,
        bx: f64,
        cx: f64,
        fa: f64,
        fb: f64,
        fc: f64,
    ) -> f64 {
        let (w, fw, v, fv) = if fa < fc {
            (ax, fa, cx, fc)
        } else {
            (cx, fc, ax, fa)
        };
        let d = cpp_min(bx - ax, cx - bx);
        let e = cpp_max(bx - ax, cx - bx);
        self.brent_loop(f, ax, cx, bx, fb, w, fw, v, fv, d, e)
    }

    /// `minimize(x)`: brackets from `x - 1` and `x` ([`bracket_minimum`]; VNL passes `x + 1` as
    /// the bracket's third point, which the bracketing overwrites), then
    /// [`BrentMinimizer::minimize_given_bounds_and_all_f`].
    pub fn minimize(&mut self, f: &mut dyn CostFunction, x: f64) -> f64 {
        let br = bracket_minimum(f, x - 1.0, x);
        self.minimize_given_bounds_and_all_f(f, br.a, br.b, br.c, br.fa, br.fb, br.fc)
    }

    /// The loop both `minimize_given_bounds_and_*` functions share, statement by statement.
    #[allow(clippy::too_many_arguments)]
    fn brent_loop(
        &mut self,
        f: &mut dyn CostFunction,
        mut ax: f64,
        mut cx: f64,
        mut x: f64,
        mut fx: f64,
        mut w: f64,
        mut fw: f64,
        mut v: f64,
        mut fv: f64,
        mut d: f64,
        mut e: f64,
    ) -> f64 {
        const EPS: f64 = 1e-8;
        let xtol = self.xtol;
        let mut m = 0.5 * (ax + cx);
        let mut tol = EPS * x.abs() + xtol;
        let mut tol2 = 2.0 * tol;
        while (x - m).abs() > (tol2 - 0.5 * (cx - ax)) {
            let (mut p, mut q, mut r) = (0.0f64, 0.0f64, 0.0f64);
            if e.abs() > tol {
                // Fit a parabola.
                r = (x - w) * (fx - fv);
                q = (x - v) * (fx - fw);
                p = (x - v) * q - (x - w) * r;
                q = 2.0 * (q - r);
                if q > 0.0 {
                    p *= -1.0;
                } else {
                    q *= -1.0;
                }
                r = e;
                e = d;
            }
            if p.abs() < (0.5 * q * r).abs() && p > (q * (ax - x)) && p < (q * (cx - x)) {
                // Parabolic interpolation.
                d = p / q;
                let u = x + d;
                if u - ax < tol2 || cx - u < tol2 {
                    d = if x < m { tol } else { -tol };
                }
            } else {
                // Golden section step.
                e = (if x < m { cx } else { ax }) - x;
                d = COMPL_GOLD * e;
            }
            let u = if d.abs() >= tol {
                x + d
            } else if d > 0.0 {
                x + tol
            } else {
                x - tol
            };
            let fu = f.f(&[u]);
            if fu <= fx {
                if u < x {
                    cx = x;
                } else {
                    ax = x;
                }
                v = w;
                fv = fw;
                w = x;
                fw = fx;
                x = u;
                fx = fu;
            } else {
                if u < x {
                    ax = u;
                } else {
                    cx = u;
                }
                if fu <= fw || w == x {
                    v = w;
                    fv = fw;
                    w = u;
                    fw = fu;
                } else if fu <= fv || v == x || v == w {
                    v = u;
                    fv = fu;
                }
            }
            m = 0.5 * (ax + cx);
            tol = EPS * x.abs() + xtol;
            tol2 = 2.0 * tol;
        }
        self.f_at_last_minimum = fx;
        x
    }
}

/// `vnl_brent` (deprecated in VNL, a wrapper of [`BrentMinimizer`]), as FreeSurfer's `fs_powell`
/// calls it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brent {
    pub inner: BrentMinimizer,
    /// `ftol`, inherited from `vnl_nonlinear_minimizer` (`1e-10`), which `vnl_brent::minimize`
    /// passes as the tolerance.
    pub ftol: f64,
}

impl Brent {
    pub fn new() -> Self {
        Brent {
            inner: BrentMinimizer::new(),
            ftol: 1e-8 * 0.01,
        }
    }

    /// `bracket_minimum(&ax, &bx, &cx)`: VNL calls `vnl_bracket_minimum(f, *cx, *bx, *ax, ...)`,
    /// so the bracketing starts from `cx` and `bx`, and the result comes back with `cx` as its
    /// low end and `ax` as its high end. Returns the new `(ax, bx, cx)` and `(fa, fb, fc)`.
    pub fn bracket_minimum(
        &mut self,
        f: &mut dyn CostFunction,
        _ax: f64,
        bx: f64,
        cx: f64,
    ) -> ((f64, f64, f64), (f64, f64, f64)) {
        let br = bracket_minimum(f, cx, bx);
        ((br.c, br.b, br.a), (br.fc, br.fb, br.fa))
    }

    /// `minimize_given_bounds(ax, bx, cx, tol, &xmin)`: `set_x_tolerance(tol)`, then
    /// [`BrentMinimizer::minimize_given_bounds`]. Returns `(f at the minimum, xmin)`.
    pub fn minimize_given_bounds(
        &mut self,
        f: &mut dyn CostFunction,
        ax: f64,
        bx: f64,
        cx: f64,
        tol: f64,
    ) -> (f64, f64) {
        self.inner.xtol = tol;
        let xmin = self.inner.minimize_given_bounds(f, ax, bx, cx);
        (self.inner.f_at_last_minimum, xmin)
    }

    /// `minimize_given_bounds_and_1st_f(ax, bx, fb, cx, tol, &xmin)`. Returns `(f at the
    /// minimum, xmin)`.
    pub fn minimize_given_bounds_and_1st_f(
        &mut self,
        f: &mut dyn CostFunction,
        ax: f64,
        bx: f64,
        fb: f64,
        cx: f64,
        tol: f64,
    ) -> (f64, f64) {
        self.inner.xtol = tol;
        let xmin = self.inner.minimize_given_bounds_and_one_f(f, ax, bx, cx, fb);
        (self.inner.f_at_last_minimum, xmin)
    }

    /// `minimize(x)`: brackets from `x - 1` (as `ax`), `x + 1` (as `bx`) and `0` (as `cx`) with
    /// [`Brent::bracket_minimum`] (so from `0` and `x + 1`), then
    /// [`Brent::minimize_given_bounds`] on `(cx, bx, ax)` with the tolerance `ftol`. Returns the
    /// minimum's `x`.
    pub fn minimize(&mut self, f: &mut dyn CostFunction, x: f64) -> f64 {
        let ((ax, xx, bx), _) = self.bracket_minimum(f, x - 1.0, x + 1.0, 0.0);
        let ftol = self.ftol;
        self.minimize_given_bounds(f, bx, xx, ax, ftol).1
    }
}

/// `vnl_nonlinear_minimizer`'s settings, counters and reports.
#[derive(Clone, Debug, PartialEq)]
pub struct NonlinearMinimizer {
    pub xtol: f64,
    pub maxfev: i64,
    pub ftol: f64,
    pub gtol: f64,
    pub epsfcn: f64,
    pub num_iterations: u32,
    pub num_evaluations: i64,
    pub start_error: f64,
    pub end_error: f64,
    pub trace: bool,
    pub verbose: bool,
    pub check_derivatives: i32,
    pub failure_code: ReturnCode,
}

impl Default for NonlinearMinimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl NonlinearMinimizer {
    /// The constructor's values: `xtol` 1e-8, `maxfev` 2000, `ftol = xtol·0.01`, `gtol` 1e-5,
    /// `epsfcn = xtol·0.001`, the counters 0, `failure_code` `ERROR_FAILURE`.
    pub fn new() -> Self {
        let xtol = 1e-8;
        NonlinearMinimizer {
            xtol,
            maxfev: 2000,
            ftol: xtol * 0.01,
            gtol: 1e-5,
            epsfcn: xtol * 0.001,
            num_iterations: 0,
            num_evaluations: 0,
            start_error: 0.0,
            end_error: 0.0,
            trace: false,
            verbose: false,
            check_derivatives: 0,
            failure_code: ReturnCode::ErrorFailure,
        }
    }

    /// `set_x_tolerance(v)`: also sets `epsfcn = v·0.001`.
    pub fn set_x_tolerance(&mut self, v: f64) {
        self.xtol = v;
        self.epsfcn = v * 0.001;
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.num_iterations = 0;
        self.num_evaluations = 0;
        self.start_error = 0.0;
        self.end_error = 0.0;
    }

    /// `report_eval(f)`: the first value starts both errors; a smaller one lowers `end_error`.
    pub fn report_eval(&mut self, f: f64) {
        if self.num_evaluations == 0 {
            self.start_error = f;
            self.end_error = f;
        }
        if f < self.end_error {
            self.end_error = f;
        }
        self.num_evaluations += 1;
    }

    /// `report_iter()`: counts the iteration and, when verbose, writes `Iter %4u, Eval %4ld:
    /// Best F = %10g` (stream widths) to standard error. Returns `false` (the base class never
    /// asks to stop).
    pub fn report_iter(&mut self, out: &mut dyn Streams) -> bool {
        self.num_iterations = self.num_iterations.wrapping_add(1);
        if self.verbose {
            out.err(&format!(
                "Iter {:>4}, Eval {:>4}: Best F = {}\n",
                self.num_iterations,
                self.num_evaluations,
                cfmt::ostream_setw(self.end_error, 10)
            ));
        }
        false
    }

    /// `obj_value_reduced()`.
    pub fn obj_value_reduced(&self) -> bool {
        self.failure_code != ReturnCode::ErrorFailure
            && self.failure_code != ReturnCode::ErrorDodgyInput
            && self.end_error < self.start_error
    }
}

/// `vnl_powell`: Powell's direction-set minimization with Brent line searches
/// ([`bracket_minimum`] from 0 and `initial_step`, then [`BrentMinimizer`] at `linmin_xtol`).
#[derive(Clone, Debug, PartialEq)]
pub struct Powell {
    pub base: NonlinearMinimizer,
    /// `linmin_xtol_` (default 1e-4).
    pub linmin_xtol: f64,
    /// `initial_step_` (default 1.0).
    pub initial_step: f64,
}

impl Default for Powell {
    fn default() -> Self {
        Self::new()
    }
}

impl Powell {
    pub fn new() -> Self {
        Powell {
            base: NonlinearMinimizer::new(),
            linmin_xtol: 1e-4,
            initial_step: 1.0,
        }
    }

    /// `vnl_powell::minimize(p)`: minimizes from `p` (updated in place) and returns the
    /// [`ReturnCode`]. The counters continue from earlier calls (VNL does not reset them).
    pub fn minimize(&mut self, functor: &mut dyn CostFunction, p: &mut [f64], out: &mut dyn Streams) -> ReturnCode {
        let n = p.len();
        // xi: the identity, row-major (xi[j][i] is row j, column i).
        let mut xi = vec![0.0f64; n * n];
        for i in 0..n {
            xi[i * n + i] = 1.0;
        }
        let mut ptt = vec![0.0f64; n];
        let mut xit = vec![0.0f64; n];
        let mut fret = functor.f(p);
        self.base.report_eval(fret);
        let mut pt = p.to_vec();
        let maxfev = self.base.maxfev as u32;
        while self.base.num_iterations < maxfev {
            let fp = fret;
            let mut ibig = 0usize;
            let mut del = 0.0f64;
            for i in 0..n {
                for j in 0..n {
                    xit[j] = xi[j * n + i];
                }
                let fptt = fret;
                let (xx, fr) = self.line_minimize(functor, p, &xit, self.initial_step);
                fret = fr;
                for (k, pk) in p.iter_mut().enumerate() {
                    // f1d.uninit(xx, p) with the x0 of this line (p before the update).
                    *pk += xx * xit[k];
                }
                if (fptt - fret).abs() > del {
                    del = (fptt - fret).abs();
                    ibig = i;
                }
            }
            if 2.0 * (fp - fret).abs() <= self.base.ftol * (fp.abs() + fret.abs()) {
                return ReturnCode::ConvergedFtol;
            }
            if self.base.num_iterations == maxfev {
                return ReturnCode::FailedTooManyIterations;
            }
            for j in 0..n {
                ptt[j] = 2.0 * p[j] - pt[j];
                xit[j] = p[j] - pt[j];
                pt[j] = p[j];
            }
            let fptt = functor.f(&ptt);
            self.base.report_eval(fret);
            if fptt < fp {
                let t = 2.0 * (fp - 2.0 * fret + fptt) * sqr(fp - fret - del) - del * sqr(fp - fptt);
                if t < 0.0 {
                    let (xx, fr) = self.line_minimize(functor, p, &xit, 1.0);
                    fret = fr;
                    for (k, pk) in p.iter_mut().enumerate() {
                        *pk += xx * xit[k];
                    }
                    for j in 0..n {
                        xi[j * n + ibig] = xi[j * n + n - 1];
                        xi[j * n + n - 1] = xit[j];
                    }
                }
            }
            if self.base.report_iter(out) {
                return ReturnCode::FailedUserRequest;
            }
        }
        ReturnCode::FailedTooManyIterations
    }

    /// One line minimization along `dx` from `x0` (`vnl_powell_1dfun` with
    /// `vnl_bracket_minimum(f1d, 0, xx, bx)` and `vnl_brent_minimizer` at `linmin_xtol`); each
    /// evaluation is reported. Returns `(xx, fret)`.
    fn line_minimize(&mut self, functor: &mut dyn CostFunction, x0: &[f64], dx: &[f64], step: f64) -> (f64, f64) {
        let n = x0.len();
        let mut tmpx = vec![0.0f64; n];
        let linmin_xtol = self.linmin_xtol;
        let base = &mut self.base;
        let mut f1d = |x: &[f64]| {
            let lambda = x[0];
            for i in 0..n {
                tmpx[i] = x0[i] + lambda * dx[i];
            }
            let e = functor.f(&tmpx);
            base.report_eval(e);
            e
        };
        let br = bracket_minimum(&mut f1d, 0.0, step);
        let mut brent = BrentMinimizer::new();
        brent.xtol = linmin_xtol;
        let xx = brent.minimize_given_bounds(&mut f1d, br.a, br.b, br.c);
        (xx, brent.f_at_last_minimum)
    }
}

#[inline]
fn sqr(x: f64) -> f64 {
    x * x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfmt::Silent;

    #[test]
    fn brent_finds_a_parabolas_minimum() {
        let mut f = |x: &[f64]| (x[0] - 0.3) * (x[0] - 0.3) + 2.0;
        let br = bracket_minimum(&mut f, 0.0, 1.0);
        assert!(br.a < br.b && br.b < br.c && br.fb <= br.fa && br.fb <= br.fc);
        let mut b = BrentMinimizer::new();
        let x = b.minimize_given_bounds(&mut f, br.a, br.b, br.c);
        assert!((x - 0.3).abs() < 1e-5);
        assert_eq!(b.f_at_last_minimum, f(&[x]));
    }

    #[test]
    fn powell_minimizes_a_quadratic() {
        let mut f = |x: &[f64]| {
            let (a, b) = (x[0] - 1.0, x[1] + 2.0);
            a * a + 3.0 * b * b + 0.5 * a * b
        };
        let mut p = [0.0, 0.0];
        let mut powell = Powell::new();
        let code = powell.minimize(&mut f, &mut p, &mut Silent);
        assert_eq!(code, ReturnCode::ConvergedFtol);
        assert!((p[0] - 1.0).abs() < 1e-3 && (p[1] + 2.0).abs() < 1e-3, "{p:?}");
    }
}

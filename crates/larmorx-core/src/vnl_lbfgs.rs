// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Jorge Nocedal's limited-memory BFGS (1990) with Moré and Thuente's line search, as VXL's
//! `v3p/netlib/opt/lbfgs.c` (an f2c translation, edited by VXL to keep its state in a struct)
//! computes it: `v3p_netlib_lbfgs_`, the routine VNL's `vnl_lbfgs` (and other callers) drive by
//! reverse communication.
//!
//! [`lbfgs`] is one call of the routine: the caller evaluates `f` and `g` at `x`, calls, and
//! acts on `iflag` (1: evaluate again at the new `x`; 2: give the diagonal `diag`; 0: done;
//! negative: an error). [`LbfgsGlobal`] holds the line search's state between calls (VXL's
//! `v3p_netlib_lbfgs_global_t`, [`LbfgsGlobal::new`] its `v3p_netlib_lbfgs_init`). The routine
//! prints with `printf`: its messages go to [`Streams::out`], byte for byte (`lb1`'s table,
//! `write50`'s vectors, the errors, which have no final newline).
//!
//! Every operation is in double precision in the C order; the dot products and updates are the
//! reference BLAS `ddot` (unrolled by five) and `daxpy`, with x86-64's NaN made explicit there and
//! in `write50`'s division (`math::x86_nan`: a diverging run prints `-nan` on every platform). The f2c macros
//! `abs`, `min` and `max` are kept with their C semantics (`x >= 0 ? x : -x`, `a <= b ? a : b`,
//! `a >= b ? a : b`), and `mcstep`'s `(float).66` is the float `0.66f` widened to double.
//!
//! Four defects of VXL's translation (all in ITK 4.13.2 and 5.4.5):
//! - **ITK-103** (rule C, measured): in the line search's first stage, when the function
//!   decreased but not enough, Nocedal's Fortran passes the *modified* function values `FXM`,
//!   `FYM` to `MCSTEP`; VXL's `mcstep_` reads its struct's `fx`, `fy` instead (the unmodified
//!   values), so later steps of that search are computed from inconsistent values. Measured on
//!   200,000 smooth problems at two settings (VNL's defaults, and memory 7, line-search accuracy
//!   0.1, `xtol` 1e-16), the correction changed no run
//!   (`validation/probes/fs-vnl --only itk103`), so VXL's behaviour is kept in both modes and
//!   Nocedal's is on request ([`LbfgsGlobal::nocedal_first_stage`], with [`UpstreamDefects::Fix`]),
//!   named where it changes the search's state.
//! - **ITK-104**: `ys` is a local of the C function (`SAVE`d in the Fortran), reset to 0 on
//!   every call, so a caller that supplies the diagonal (`diagco`, `iflag = 2`) gets `1 / 0`.
//!   `Fix` keeps it across calls; `Reproduce` divides by 0. VNL's own driver never supplies it.
//! - **ITK-105** (printing only): `write50` takes the largest value with its sign (a negative
//!   one stops the scale's search) and divides by 0 for a vector of zeros. `Fix` scales by the
//!   largest magnitude and prints a zero vector unscaled; `Reproduce` prints as VXL.
//! - **ITK-106** (printing only): `lb1_` decrements its `x` and `g` pointers for Fortran's
//!   indexing and passes the decremented pointers to `write50`, which indexes from 0: the vectors
//!   are printed from the double *before* each array (memory the routine does not own) and their
//!   last element is dropped. `Fix` prints the vectors; `Reproduce` prints VXL's window with the
//!   value before the array as 0 (memory contents are never reproduced, D13).
//!
//! Ported from VXL as bundled with ITK v4.13.2 (BSD licence; netlib's Fortran by J. Nocedal,
//! `lbfgs.f`, translated by f2c 20050501 and edited by VXL): `v3p/netlib/opt/lbfgs.{c,h}`,
//! `v3p/netlib/blas/{ddot,daxpy}.c`. The Fortran (`lbfgs.f`, kept beside it) is the reference
//! for ITK-103 and ITK-104.

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
//
// The L-BFGS routine is netlib software (J. Nocedal), distributed by VXL as above.

// Fortran's 1-based indices are kept (`w[k]` is `W(k)`), so the port reads line by line
// against lbfgs.c.
#![allow(clippy::needless_range_loop)]
// The expressions are kept as the C writes them (`-1.0 * x` is not `-x` for a NaN; the
// constants are VXL's spellings; the negated comparisons are the f2c loops' NaN behaviour).
#![allow(
    clippy::neg_multiply,
    clippy::manual_range_contains,
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::assign_op_pattern,
    clippy::collapsible_if,
    clippy::needless_late_init,
    clippy::too_many_arguments
)]

use crate::cfmt::{self, Streams};
use crate::defects::{ITK_103, ITK_104, ITK_105, ITK_106, Notice, UpstreamDefects};
use crate::math;
use crate::math::x86_nan::{add_f64, div_f64, mul_f64};

/// The reference BLAS `ddot` (unrolled by five) with SSE's NaN rules explicit (`math::x86_nan`): an
/// L-BFGS run that diverges makes NaNs here (`inf * 0`, `inf - inf`), and the printed iterates
/// show their sign (x86-64's `-nan`).
fn ddot(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let m = n % 5;
    let mut t = 0.0f64;
    for i in 0..m {
        t = add_f64(t, mul_f64(x[i], y[i]));
    }
    let mut i = m;
    while i + 5 <= n {
        t = add_f64(
            add_f64(
                add_f64(
                    add_f64(add_f64(t, mul_f64(x[i], y[i])), mul_f64(x[i + 1], y[i + 1])),
                    mul_f64(x[i + 2], y[i + 2]),
                ),
                mul_f64(x[i + 3], y[i + 3]),
            ),
            mul_f64(x[i + 4], y[i + 4]),
        );
        i += 5;
    }
    t
}

/// The reference BLAS `daxpy`, `y += a·x`, with SSE's NaN rules explicit.
fn daxpy(a: f64, x: &[f64], y: &mut [f64]) {
    if a == 0.0 {
        return;
    }
    for (yi, &xi) in y.iter_mut().zip(x) {
        *yi = add_f64(*yi, mul_f64(a, xi));
    }
}

/// f2c's `abs(x)`: `x >= 0 ? x : -x` (keeps `-0`, flips a NaN's sign).
#[inline]
fn fabs_m(x: f64) -> f64 {
    if x >= 0.0 { x } else { -x }
}

/// f2c's `min(a, b)`: `a <= b ? a : b`.
#[inline]
fn min_m(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/// f2c's `max(a, b)`: `a >= b ? a : b`.
#[inline]
fn max_m(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `(float).66` in `mcstep_`: the float nearest 0.66, widened to double.
const P66F: f64 = 0.66f32 as f64;

/// `v3p_netlib_lbfgs_global_t`: the parameters (`mp`, `lp`, `gtol`, `stpmin`, `stpmax`,
/// `stpinit`) and the state the routine keeps between calls (`private_*`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LbfgsGlobal {
    pub mp: i64,
    pub lp: i64,
    pub gtol: f64,
    pub stpmin: f64,
    pub stpmax: f64,
    pub stpinit: f64,
    pub info: i64,
    pub infoc: i64,
    pub nfev: i64,
    pub maxfev: i64,
    pub stp: f64,
    pub stp1: f64,
    pub beta: f64,
    pub ftol: f64,
    pub gnorm: f64,
    pub xnorm: f64,
    pub inmc: i64,
    pub iscn: i64,
    pub iycn: i64,
    pub iter: i64,
    pub nfun: i64,
    pub ispt: i64,
    pub iypt: i64,
    pub bound: i64,
    pub point: i64,
    pub finish: bool,
    pub dg: f64,
    pub fm: f64,
    pub fx: f64,
    pub fy: f64,
    pub dgm: f64,
    pub dgx: f64,
    pub dgy: f64,
    pub fxm: f64,
    pub fym: f64,
    pub stx: f64,
    pub sty: f64,
    pub dgxm: f64,
    pub dgym: f64,
    pub finit: f64,
    pub width: f64,
    pub stmin: f64,
    pub stmax: f64,
    pub stage1: bool,
    pub width1: f64,
    pub ftest1: f64,
    pub brackt: bool,
    pub dginit: f64,
    pub dgtest: f64,
    /// `ys` kept between calls (ITK-104's correction; not in VXL's struct).
    pub ys: f64,
    /// Nocedal's first stage (`MCSTEP` with `FXM`, `FYM`) instead of VXL's, with
    /// [`UpstreamDefects::Fix`] (ITK-103; not in VXL's struct). Default `false`: VXL's, which the
    /// correction did not change in the measurement (rule C).
    pub nocedal_first_stage: bool,
}

impl LbfgsGlobal {
    /// `v3p_netlib_lbfgs_init`: `mp = lp = 6`, `gtol = 0.9`, `stpmin = 1e-20`, `stpmax = 1e20`,
    /// `stpinit = 1`; the state (uninitialised in C, written before it is read) is 0.
    pub fn new() -> Self {
        LbfgsGlobal {
            mp: 6,
            lp: 6,
            gtol: 0.9,
            stpmin: 1e-20,
            stpmax: 1e20,
            stpinit: 1.0,
            ..Default::default()
        }
    }
}

/// What one call of [`lbfgs`] found: the notices of the defects that changed (or, reproduced,
/// would have changed) this call's arithmetic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LbfgsCall {
    pub notices: Vec<Notice>,
}

/// One call of `v3p_netlib_lbfgs_(n, m, x, f, g, diagco, diag, iprint, eps, xtol, w, iflag,
/// global)`. `x`, `g` and `diag` hold `n` values, `w` at least `n·(2m + 1) + 2m`; `iprint` is
/// `IPRINT(1..2)`. `x`, `diag`, `w`, `iflag` and `global` are updated as the C routine updates
/// them.
pub fn lbfgs(
    n: i64,
    m: i64,
    x: &mut [f64],
    f: f64,
    g: &[f64],
    diagco: bool,
    diag: &mut [f64],
    iprint: [i64; 2],
    eps: f64,
    xtol: f64,
    w: &mut [f64],
    iflag: &mut i64,
    gl: &mut LbfgsGlobal,
    mode: UpstreamDefects,
    out: &mut dyn Streams,
) -> LbfgsCall {
    let mut call = LbfgsCall::default();
    let one = 1.0f64;
    let zero = 0.0f64;
    // ys is a local initialised to 0 in VXL's C (ITK-104); the Fortran keeps it.
    let mut ys = match mode {
        UpstreamDefects::Fix => gl.ys,
        UpstreamDefects::Reproduce => 0.0,
    };
    let mut npt: i64 = 0;
    let nu = n.max(0) as usize;
    // 1-based views.
    macro_rules! wi {
        ($k:expr) => {
            w[($k) as usize - 1]
        };
    }
    let dot = |w: &[f64], a: i64, b: i64| -> f64 {
        // ddot_(n, &w[a], 1, &w[b], 1), a and b 1-based.
        ddot(
            &w[a as usize - 1..a as usize - 1 + nu],
            &w[b as usize - 1..b as usize - 1 + nu],
        )
    };

    enum Go {
        L10,
        L80,
        L100,
        L172,
    }
    let mut go = match *iflag {
        1 => Go::L172,
        2 => Go::L100,
        _ => Go::L10,
    };
    // Entered for the caller's diagonal (L100 directly): where ITK-104 matters.
    let mut entered_for_diag = *iflag == 2;
    loop {
        match go {
            Go::L10 => {
                gl.iter = 0;
                if n <= 0 || m <= 0 {
                    *iflag = -3;
                    if gl.lp > 0 {
                        out.out(" IFLAG= -3 IMPROPER INPUT PARAMETERS (N OR M ARE NOT POSITIVE)");
                    }
                    break;
                }
                if gl.gtol <= 1e-4 {
                    if gl.lp > 0 {
                        out.out("  GTOL IS LESS THAN OR EQUAL TO 1.D-04");
                        out.out("  IT HAS BEEN RESET TO 9.D-01");
                    }
                    gl.gtol = 0.9;
                }
                gl.nfun = 1;
                gl.point = 0;
                gl.finish = false;
                if diagco {
                    if let Some(i) = (1..=n).find(|&i| diag[i as usize - 1] <= zero) {
                        return l195(iflag, gl, i, out, call);
                    }
                } else {
                    for d in diag.iter_mut().take(nu) {
                        *d = 1.0;
                    }
                }
                gl.ispt = n + (m << 1);
                gl.iypt = gl.ispt + n * m;
                for i in 1..=n {
                    wi!(gl.ispt + i) = -g[i as usize - 1] * diag[i as usize - 1];
                }
                gl.gnorm = ddot(&g[..nu], &g[..nu]).sqrt();
                gl.stp1 = one / gl.gnorm;
                gl.ftol = 1e-4;
                gl.maxfev = 20;
                if iprint[0] >= 0 {
                    lb1(iprint, n, m, x, f, g, gl, mode, out, &mut call);
                }
                go = Go::L80;
                // Fall into L80 below on the next turn.
                continue;
            }
            Go::L80 => {
                gl.iter += 1;
                gl.info = 0;
                gl.bound = gl.iter - 1;
                if gl.iter != 1 {
                    if gl.iter > m {
                        gl.bound = m;
                    }
                    ys = dot(w, gl.iypt + npt + 1, gl.ispt + npt + 1);
                    if !diagco {
                        let yy = dot(w, gl.iypt + npt + 1, gl.iypt + npt + 1);
                        for d in diag.iter_mut().take(nu) {
                            *d = ys / yy;
                        }
                    } else {
                        *iflag = 2;
                        gl.ys = ys;
                        return call;
                    }
                    go = Go::L100;
                    continue;
                }
                // iter == 1: goto L165.
                l165(n, g, w, gl);
                go = Go::L172;
                continue;
            }
            Go::L100 => {
                if entered_for_diag && gl.ys != 0.0 {
                    // VXL's C has reset ys to 0 on this entry: 1 / ys is infinite.
                    let detail = match mode {
                        UpstreamDefects::Fix => {
                            "ys kept from the previous call (VXL's C resets it to 0 and divides by 0)"
                        }
                        UpstreamDefects::Reproduce => {
                            "ys reset to 0 on the caller's diagonal: 1/ys is infinite"
                        }
                    };
                    crate::defects::push_once(
                        &mut call.notices,
                        Notice::new(&ITK_104, mode, detail),
                    );
                }
                entered_for_diag = false;
                if diagco {
                    if let Some(i) = (1..=n).find(|&i| diag[i as usize - 1] <= zero) {
                        return l195(iflag, gl, i, out, call);
                    }
                }
                let mut cp = gl.point;
                if gl.point == 0 {
                    cp = m;
                }
                wi!(n + cp) = one / ys;
                for i in 1..=n {
                    wi!(i) = -g[i as usize - 1];
                }
                cp = gl.point;
                for _ in 1..=gl.bound {
                    cp -= 1;
                    if cp == -1 {
                        cp = m - 1;
                    }
                    let sq = dot(w, gl.ispt + cp * n + 1, 1);
                    gl.inmc = n + m + cp + 1;
                    gl.iycn = gl.iypt + cp * n;
                    wi!(gl.inmc) = wi!(n + cp + 1) * sq;
                    let d1 = -wi!(gl.inmc);
                    axpy_w(w, nu, d1, gl.iycn + 1, 1);
                }
                for i in 1..=n {
                    wi!(i) = diag[i as usize - 1] * wi!(i);
                }
                for _ in 1..=gl.bound {
                    let yr = dot(w, gl.iypt + cp * n + 1, 1);
                    gl.beta = wi!(n + cp + 1) * yr;
                    gl.inmc = n + m + cp + 1;
                    gl.beta = wi!(gl.inmc) - gl.beta;
                    gl.iscn = gl.ispt + cp * n;
                    let beta = gl.beta;
                    axpy_w(w, nu, beta, gl.iscn + 1, 1);
                    cp += 1;
                    if cp == m {
                        cp = 0;
                    }
                }
                for i in 1..=n {
                    wi!(gl.ispt + gl.point * n + i) = wi!(i);
                }
                l165(n, g, w, gl);
                go = Go::L172;
                continue;
            }
            Go::L172 => {
                let s0 = (gl.ispt + gl.point * n) as usize; // s = &w[ispt + point*n + 1]
                mcsrch(
                    n,
                    x,
                    f,
                    g,
                    &w[s0..s0 + nu],
                    xtol,
                    diag,
                    gl,
                    mode,
                    out,
                    &mut call,
                );
                if gl.info == -1 {
                    *iflag = 1;
                    gl.ys = ys;
                    return call;
                }
                if gl.info != 1 {
                    // L190.
                    *iflag = -1;
                    if gl.lp > 0 {
                        out.out(&format!(
                            " IFLAG= -1  LINE SEARCH FAILED. SEE DOCUMENTATION OF ROUTINE MCSRCH ERROR RETURN OF LINE SEARCH: INFO= {} POSSIBLE CAUSES: FUNCTION OR GRADIENT ARE INCORRECT OR INCORRECT TOLERANCES",
                            gl.info
                        ));
                    }
                    break;
                }
                gl.nfun += gl.nfev;
                npt = gl.point * n;
                for i in 1..=n {
                    wi!(gl.ispt + npt + i) = gl.stp * wi!(gl.ispt + npt + i);
                    wi!(gl.iypt + npt + i) = g[i as usize - 1] - wi!(i);
                }
                gl.point += 1;
                if gl.point == m {
                    gl.point = 0;
                }
                gl.gnorm = ddot(&g[..nu], &g[..nu]).sqrt();
                gl.xnorm = ddot(&x[..nu], &x[..nu]).sqrt();
                gl.xnorm = max_m(1.0, gl.xnorm);
                if gl.gnorm / gl.xnorm <= eps {
                    gl.finish = true;
                }
                if iprint[0] >= 0 {
                    lb1(iprint, n, m, x, f, g, gl, mode, out, &mut call);
                }
                if gl.finish {
                    *iflag = 0;
                    break;
                }
                go = Go::L80;
                continue;
            }
        }
    }
    gl.ys = ys;
    call
}

/// `daxpy_(n, &a, &w[x], 1, &w[y], 1)` within `w` (1-based starts; the ranges never overlap in
/// lbfgs: `y` is the first `n` entries, `x` lies in the s or y rows).
fn axpy_w(w: &mut [f64], n: usize, a: f64, x: i64, y: i64) {
    let (x0, y0) = (x as usize - 1, y as usize - 1);
    debug_assert!(y0 + n <= x0 || x0 + n <= y0);
    let (lo, hi) = w.split_at_mut(x0.max(y0));
    if y0 < x0 {
        daxpy(a, &hi[..n], &mut lo[y0..y0 + n]);
    } else {
        // Not reached by lbfgs; kept for completeness.
        let xs = lo[x0..x0 + n].to_vec();
        daxpy(a, &xs, &mut hi[..n]);
    }
}

/// L165: the step's start (`stpinit`, or `1 / |g|` at the first iteration) and `W(1..n) = g`.
fn l165(n: i64, g: &[f64], w: &mut [f64], gl: &mut LbfgsGlobal) {
    gl.nfev = 0;
    gl.stp = gl.stpinit;
    if gl.iter == 1 {
        gl.stp = gl.stp1;
    }
    let nu = n as usize;
    w[..nu].copy_from_slice(&g[..nu]);
}

/// L195: a non-positive diagonal element.
fn l195(
    iflag: &mut i64,
    gl: &mut LbfgsGlobal,
    i: i64,
    out: &mut dyn Streams,
    call: LbfgsCall,
) -> LbfgsCall {
    *iflag = -2;
    if gl.lp > 0 {
        out.out(&format!(
            "IFLAG= -2 THE {i}-TH DIAGONAL ELEMENT OF THE INVERSE HESSIAN APPROXIMATION IS NOT POSITIVE"
        ));
    }
    call
}

/// `write50(v, n)`: a scale line (`"  %e x\n"`, when the scale is not 1), then the values over
/// the scale as `" %10.5f"`, 15 to a line.
fn write50(v: &[f64], mode: UpstreamDefects, out: &mut dyn Streams, call: &mut LbfgsCall) {
    let cols = 15;
    let mut vmax = 0.0f64;
    for &vi in v {
        if vi.abs() > vmax {
            vmax = vi;
        }
    }
    let scale_of = |vmax: f64| {
        let s = math::log_glibc(vmax.abs()) / math::log_glibc(10.0);
        math::pow_glibc(10.0, s.ceil() - 1.0)
    };
    let vxl = scale_of(vmax);
    let fixed = {
        let vabs = v
            .iter()
            .fold(0.0f64, |a, &b| if b.abs() > a { b.abs() } else { a });
        if vabs == 0.0 { 1.0 } else { scale_of(vabs) }
    };
    let vmaxscale = match mode {
        UpstreamDefects::Fix => fixed,
        UpstreamDefects::Reproduce => vxl,
    };
    if fixed.to_bits() != vxl.to_bits() {
        crate::defects::push_once(
            &mut call.notices,
            Notice::new(
                &ITK_105,
                mode,
                "the printed vector's scale (VXL's write50 takes the largest value with its sign, and divides by 0 for a vector of zeros)",
            ),
        );
    }
    if vmaxscale != 1.0 {
        out.out(&format!("  {} x\n", cfmt::e(vmaxscale, 6)));
    }
    let mut line = String::new();
    for (i, &vi) in v.iter().enumerate() {
        if i > 0 && i % cols == 0 {
            line.push('\n');
        }
        line.push(' ');
        // a vector of zeros divides 0 by 0 (ITK-105): x86-64's -nan
        line.push_str(&cfmt::pad(&cfmt::f(div_f64(vi, vmaxscale), 5), 10));
    }
    line.push('\n');
    out.out(&line);
}

/// `write50` as `lb1_` calls it (ITK-106): VXL passes the array's pointer decremented by one.
fn write50_lb1(v: &[f64], mode: UpstreamDefects, out: &mut dyn Streams, call: &mut LbfgsCall) {
    let detail = match mode {
        UpstreamDefects::Fix => {
            "the printed vector (VXL's lb1 prints from the value before the array and drops the last)"
        }
        UpstreamDefects::Reproduce => {
            "the printed vector starts with the value before the array (memory VXL does not own, printed as 0) and drops the last"
        }
    };
    if !v.is_empty() {
        crate::defects::push_once(&mut call.notices, Notice::new(&ITK_106, mode, detail));
    }
    match mode {
        UpstreamDefects::Fix => write50(v, mode, out, call),
        UpstreamDefects::Reproduce => {
            let mut window = Vec::with_capacity(v.len());
            if !v.is_empty() {
                window.push(0.0);
                window.extend_from_slice(&v[..v.len() - 1]);
            }
            write50(&window, mode, out, call);
        }
    }
}

/// `lb1_`: the table of iterations (`IPRINT(1)` iterations apart, `IPRINT(2)` its detail).
fn lb1(
    iprint: [i64; 2],
    n: i64,
    m: i64,
    x: &[f64],
    f: f64,
    g: &[f64],
    gl: &LbfgsGlobal,
    mode: UpstreamDefects,
    out: &mut dyn Streams,
    call: &mut LbfgsCall,
) {
    let nu = n as usize;
    let row = |gl: &LbfgsGlobal| {
        format!(
            "{:>4} {:>4}    {}  {}  {}\n",
            gl.iter,
            gl.nfun,
            cfmt::pad(&cfmt::f(f, 3), 10),
            cfmt::pad(&cfmt::f(gl.gnorm, 3), 10),
            cfmt::pad(&cfmt::f(gl.stp, 3), 10)
        )
    };
    const HEAD: &str = "   I   NFN    FUNC        GNORM       STEPLENGTH\n";
    if gl.iter == 0 {
        out.out("*************************************************\n");
        out.out(&format!(
            "  N={n}   NUMBER OF CORRECTIONS={m}       INITIAL VALUES"
        ));
        out.out(&format!(
            " F= {}   GNORM= {}\n",
            cfmt::g(f, 6),
            cfmt::g(gl.gnorm, 6)
        ));
        if iprint[1] >= 1 {
            out.out(" VECTOR X= ");
            write50_lb1(&x[..nu], mode, out, call);
            out.out(" GRADIENT VECTOR G= ");
            write50_lb1(&g[..nu], mode, out, call);
        }
        out.out("*************************************************\n");
        out.out(HEAD);
    } else {
        if iprint[0] == 0 && (gl.iter != 1 && !gl.finish) {
            return;
        }
        if iprint[0] != 0 {
            if (gl.iter - 1) % iprint[0] == 0 || gl.finish {
                if iprint[1] > 1 && gl.iter > 1 {
                    out.out(HEAD);
                }
                out.out(&row(gl));
            } else {
                return;
            }
        } else {
            if iprint[1] > 1 && gl.finish {
                out.out(HEAD);
            }
            out.out(&row(gl));
        }
        if iprint[1] == 2 || iprint[1] == 3 {
            if gl.finish {
                out.out(" FINAL POINT X= ");
            } else {
                out.out(" VECTOR X= ");
            }
            write50_lb1(&x[..nu], mode, out, call);
            if iprint[1] == 3 {
                out.out(" GRADIENT VECTOR G= ");
                write50_lb1(&g[..nu], mode, out, call);
            }
        }
        if gl.finish {
            out.out(" THE MINIMIZATION TERMINATED WITHOUT DETECTING ERRORS.\n");
        }
    }
}

/// `mcsrch_`: Moré and Thuente's line search along `s` from `wa` (a copy of `x` at the line's
/// start), by reverse communication through `info` (`-1`: evaluate at the new `x`).
fn mcsrch(
    n: i64,
    x: &mut [f64],
    f: f64,
    g: &[f64],
    s: &[f64],
    xtol: f64,
    wa: &mut [f64],
    gl: &mut LbfgsGlobal,
    mode: UpstreamDefects,
    out: &mut dyn Streams,
    call: &mut LbfgsCall,
) {
    let p5 = 0.5f64;
    let p66 = 0.66f64;
    let xtrapf = 4.0f64;
    let zero = 0.0f64;
    let nu = n.max(0) as usize;
    if gl.info != -1 {
        gl.infoc = 1;
        if n <= 0
            || gl.stp <= zero
            || gl.ftol < zero
            || gl.gtol < zero
            || xtol < zero
            || gl.stpmin < zero
            || gl.stpmax < gl.stpmin
            || gl.maxfev <= 0
        {
            return;
        }
        gl.dginit = zero;
        for j in 0..nu {
            gl.dginit += g[j] * s[j];
        }
        if gl.dginit >= zero {
            // The only message mcsrch prints (lbfgs then reports INFO= 0).
            out.out("  THE SEARCH DIRECTION IS NOT A DESCENT DIRECTION\n");
            return;
        }
        gl.brackt = false;
        gl.stage1 = true;
        gl.nfev = 0;
        gl.finit = f;
        gl.dgtest = gl.ftol * gl.dginit;
        gl.width = gl.stpmax - gl.stpmin;
        gl.width1 = gl.width / p5;
        wa[..nu].copy_from_slice(&x[..nu]);
        gl.stx = zero;
        gl.fx = gl.finit;
        gl.dgx = gl.dginit;
        gl.sty = zero;
        gl.fy = gl.finit;
        gl.dgy = gl.dginit;
    } else {
        // L45.
        gl.info = 0;
        gl.nfev += 1;
        gl.dg = zero;
        for j in 0..nu {
            gl.dg += g[j] * s[j];
        }
        gl.ftest1 = gl.finit + gl.stp * gl.dgtest;
        if (gl.brackt && (gl.stp <= gl.stmin || gl.stp >= gl.stmax)) || gl.infoc == 0 {
            gl.info = 6;
        }
        if gl.stp == gl.stpmax && f <= gl.ftest1 && gl.dg <= gl.dgtest {
            gl.info = 5;
        }
        if gl.stp == gl.stpmin && (f > gl.ftest1 || gl.dg >= gl.dgtest) {
            gl.info = 4;
        }
        if gl.nfev >= gl.maxfev {
            gl.info = 3;
        }
        if gl.brackt && gl.stmax - gl.stmin <= xtol * gl.stmax {
            gl.info = 2;
        }
        if f <= gl.ftest1 && fabs_m(gl.dg) <= gl.gtol * (-gl.dginit) {
            gl.info = 1;
        }
        if gl.info != 0 {
            return;
        }
        if gl.stage1 && f <= gl.ftest1 && gl.dg >= min_m(gl.ftol, gl.gtol) * gl.dginit {
            gl.stage1 = false;
        }
        if gl.stage1 && f <= gl.fx && f > gl.ftest1 {
            gl.fm = f - gl.stp * gl.dgtest;
            gl.fxm = gl.fx - gl.stx * gl.dgtest;
            gl.fym = gl.fy - gl.sty * gl.dgtest;
            gl.dgm = gl.dg - gl.dgtest;
            gl.dgxm = gl.dgx - gl.dgtest;
            gl.dgym = gl.dgy - gl.dgtest;
            // ITK-103: VXL's mcstep_ reads the struct's fx, fy; the Fortran passes FXM, FYM.
            let run = |gl: &mut LbfgsGlobal, vxl: bool| {
                let (mut dx, mut dy) = (gl.dgxm, gl.dgym);
                let (fm, dgm, stmin, stmax) = (gl.fm, gl.dgm, gl.stmin, gl.stmax);
                if vxl {
                    let (mut fx, mut fy) = (gl.fx, gl.fy);
                    mcstep(
                        gl, &mut fx, &mut dx, &mut fy, &mut dy, fm, dgm, stmin, stmax,
                    );
                    gl.fx = fx;
                    gl.fy = fy;
                } else {
                    let (mut fxm, mut fym) = (gl.fxm, gl.fym);
                    mcstep(
                        gl, &mut fxm, &mut dx, &mut fym, &mut dy, fm, dgm, stmin, stmax,
                    );
                    gl.fxm = fxm;
                    gl.fym = fym;
                }
                gl.dgxm = dx;
                gl.dgym = dy;
                gl.fx = gl.fxm + gl.stx * gl.dgtest;
                gl.fy = gl.fym + gl.sty * gl.dgtest;
                gl.dgx = gl.dgxm + gl.dgtest;
                gl.dgy = gl.dgym + gl.dgtest;
            };
            let nocedal = gl.nocedal_first_stage && mode == UpstreamDefects::Fix;
            if !nocedal {
                run(gl, true);
            } else {
                let mut other = gl.clone();
                run(gl, false);
                run(&mut other, true);
                if !same_state(gl, &other) {
                    crate::defects::push_once(
                        &mut call.notices,
                        Notice::new(
                            &ITK_103,
                            mode,
                            "the line search's first stage stepped with the modified function values, as Nocedal's Fortran (VXL's translation uses the unmodified ones)",
                        ),
                    );
                }
            }
        } else {
            let (mut fx, mut fy, mut dx, mut dy) = (gl.fx, gl.fy, gl.dgx, gl.dgy);
            let (dg, stmin, stmax) = (gl.dg, gl.stmin, gl.stmax);
            mcstep(gl, &mut fx, &mut dx, &mut fy, &mut dy, f, dg, stmin, stmax);
            gl.fx = fx;
            gl.fy = fy;
            gl.dgx = dx;
            gl.dgy = dy;
        }
        if gl.brackt {
            if fabs_m(gl.sty - gl.stx) >= p66 * gl.width1 {
                gl.stp = gl.stx + p5 * (gl.sty - gl.stx);
            }
            gl.width1 = gl.width;
            gl.width = fabs_m(gl.sty - gl.stx);
        }
    }
    // L30.
    if gl.brackt {
        gl.stmin = min_m(gl.stx, gl.sty);
        gl.stmax = max_m(gl.stx, gl.sty);
    } else {
        gl.stmin = gl.stx;
        gl.stmax = gl.stp + xtrapf * (gl.stp - gl.stx);
    }
    gl.stp = max_m(gl.stp, gl.stpmin);
    gl.stp = min_m(gl.stp, gl.stpmax);
    if (gl.brackt && (gl.stp <= gl.stmin || gl.stp >= gl.stmax))
        || gl.nfev >= gl.maxfev - 1
        || gl.infoc == 0
        || (gl.brackt && gl.stmax - gl.stmin <= xtol * gl.stmax)
    {
        gl.stp = gl.stx;
    }
    for j in 0..nu {
        x[j] = wa[j] + gl.stp * s[j];
    }
    gl.info = -1;
}

/// Every field of two states, bit for bit.
fn same_state(a: &LbfgsGlobal, b: &LbfgsGlobal) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

/// `mcstep_`: the safeguarded cubic or quadratic step of the line search. `stx`, `sty`, `stp`,
/// `brackt` and `infoc` are the struct's; `fx`, `fy` (VXL: the struct's; the Fortran: the
/// caller's), `dx`, `dy` are updated through their references.
#[allow(clippy::many_single_char_names)]
fn mcstep(
    gl: &mut LbfgsGlobal,
    fx: &mut f64,
    dx: &mut f64,
    fy: &mut f64,
    dy: &mut f64,
    fp: f64,
    dp: f64,
    stpmin: f64,
    stpmax: f64,
) {
    gl.infoc = 0;
    let (stx, sty, stp) = (gl.stx, gl.sty, gl.stp);
    if (gl.brackt && (stp <= min_m(stx, sty) || stp >= max_m(stx, sty)))
        || *dx * (stp - stx) >= 0.0
        || stpmax < stpmin
    {
        return;
    }
    let sgnd = dp * (*dx / fabs_m(*dx));
    let mcbound;
    let stpf;
    if fp > *fx {
        gl.infoc = 1;
        mcbound = true;
        let theta = (*fx - fp) * 3.0 / (stp - stx) + *dx + dp;
        let s = max_m(max_m(fabs_m(theta), fabs_m(*dx)), fabs_m(dp));
        let d1 = theta / s;
        let mut gamma = s * (d1 * d1 - *dx / s * (dp / s)).sqrt();
        if stp < stx {
            gamma = -gamma;
        }
        let p = gamma - *dx + theta;
        let q = gamma - *dx + gamma + dp;
        let r = p / q;
        let stpc = stx + r * (stp - stx);
        let stpq = stx + *dx / ((*fx - fp) / (stp - stx) + *dx) / 2.0 * (stp - stx);
        if fabs_m(stpc - stx) < fabs_m(stpq - stx) {
            stpf = stpc;
        } else {
            stpf = stpc + (stpq - stpc) / 2.0;
        }
        gl.brackt = true;
    } else if sgnd < 0.0 {
        gl.infoc = 2;
        mcbound = false;
        let theta = (*fx - fp) * 3.0 / (stp - stx) + *dx + dp;
        let s = max_m(max_m(fabs_m(theta), fabs_m(*dx)), fabs_m(dp));
        let d1 = theta / s;
        let mut gamma = s * (d1 * d1 - *dx / s * (dp / s)).sqrt();
        if stp > stx {
            gamma = -gamma;
        }
        let p = gamma - dp + theta;
        let q = gamma - dp + gamma + *dx;
        let r = p / q;
        let stpc = stp + r * (stx - stp);
        let stpq = stp + dp / (dp - *dx) * (stx - stp);
        if fabs_m(stpc - stp) > fabs_m(stpq - stp) {
            stpf = stpc;
        } else {
            stpf = stpq;
        }
        gl.brackt = true;
    } else if fabs_m(dp) < fabs_m(*dx) {
        gl.infoc = 3;
        mcbound = true;
        let theta = (*fx - fp) * 3.0 / (stp - stx) + *dx + dp;
        let s = max_m(max_m(fabs_m(theta), fabs_m(*dx)), fabs_m(dp));
        let d3 = theta / s;
        let d2 = d3 * d3 - *dx / s * (dp / s);
        let mut gamma = s * max_m(0.0, d2).sqrt();
        if stp > stx {
            gamma = -gamma;
        }
        let p = gamma - dp + theta;
        let q = gamma + (*dx - dp) + gamma;
        let r = p / q;
        let stpc = if r < 0.0 && gamma != 0.0 {
            stp + r * (stx - stp)
        } else if stp > stx {
            stpmax
        } else {
            stpmin
        };
        let stpq = stp + dp / (dp - *dx) * (stx - stp);
        if gl.brackt {
            if fabs_m(stp - stpc) < fabs_m(stp - stpq) {
                stpf = stpc;
            } else {
                stpf = stpq;
            }
        } else if fabs_m(stp - stpc) > fabs_m(stp - stpq) {
            stpf = stpc;
        } else {
            stpf = stpq;
        }
    } else {
        gl.infoc = 4;
        mcbound = false;
        if gl.brackt {
            let theta = (fp - *fy) * 3.0 / (sty - stp) + *dy + dp;
            let s = max_m(max_m(fabs_m(theta), fabs_m(*dy)), fabs_m(dp));
            let d1 = theta / s;
            let mut gamma = s * (d1 * d1 - *dy / s * (dp / s)).sqrt();
            if stp > sty {
                gamma = -gamma;
            }
            let p = gamma - dp + theta;
            let q = gamma - dp + gamma + *dy;
            let r = p / q;
            let stpc = stp + r * (sty - stp);
            stpf = stpc;
        } else if stp > stx {
            stpf = stpmax;
        } else {
            stpf = stpmin;
        }
    }
    if fp > *fx {
        gl.sty = stp;
        *fy = fp;
        *dy = dp;
    } else {
        if sgnd < 0.0 {
            gl.sty = gl.stx;
            *fy = *fx;
            *dy = *dx;
        }
        gl.stx = stp;
        *fx = fp;
        *dx = dp;
    }
    let mut stpf = min_m(stpmax, stpf);
    stpf = max_m(stpmin, stpf);
    gl.stp = stpf;
    if gl.brackt && mcbound {
        let d1 = gl.stx + (gl.sty - gl.stx) * P66F;
        if gl.sty > gl.stx {
            gl.stp = min_m(d1, gl.stp);
        } else {
            gl.stp = max_m(d1, gl.stp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfmt::{Captured, Stream};

    /// ITK-105's vector of zeros: with `reproduce` VXL's `write50` scales by `10^ceil(log10(0)) /
    /// 10 = 0` and divides 0 by 0, which x86-64 prints as `-nan` (SSE's default NaN). The port
    /// prints `-nan` on every platform (plain Rust arithmetic on AArch64 would print `nan`; this
    /// test passes under `nan_trap.c`'s AArch64 emulation, `NAN_TRAP_ARM=1`); `fix` scales by 1.
    #[test]
    fn write50_of_zeros_prints_x86_64s_nan() {
        let v = [0.0, -0.0, 0.0];
        let mut out = Captured::default();
        let mut call = LbfgsCall::default();
        write50(&v, UpstreamDefects::Reproduce, &mut out, &mut call);
        assert_eq!(
            out.text(Stream::Out),
            "  0.000000e+00 x\n       -nan       -nan       -nan\n"
        );
        assert_eq!(call.notices.len(), 1);
        let mut out = Captured::default();
        let mut call = LbfgsCall::default();
        write50(&v, UpstreamDefects::Fix, &mut out, &mut call);
        assert_eq!(out.text(Stream::Out), "    0.00000   -0.00000    0.00000\n");
        assert_eq!(call.notices.len(), 1);
    }
}

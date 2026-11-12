// SPDX-License-Identifier: Apache-2.0
//! netlib's `lbfgs_` (VXL's translation) against `larmorx_core::vnl_lbfgs::lbfgs` under
//! `reproduce`, driven by reverse communication as `fs_lbfgs` drives it, in lock step: after every
//! call `x`, `diag`, `w`, `iflag`, the whole state and the printed text, bit for bit. Then the
//! port's correction of ITK-103 is measured against VXL's behaviour on the same problems.

use std::ffi::{c_long, c_void};

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math;
use larmorx_core::vnl_lbfgs::{LbfgsGlobal, lbfgs};
use larmorx_core::x87::cases::SplitMix64;

use crate::{Lib, Tally};

/// A seeded smooth test problem with its gradient (some with a wrong gradient, to reach the line
/// search's failures).
struct LProblem {
    kind: u8,
    n: usize,
    a: Vec<f64>,
    b: Vec<f64>,
    c: Vec<f64>,
    bad_gradient: bool,
}

impl LProblem {
    fn new(rng: &mut SplitMix64, n: usize) -> LProblem {
        let u = |rng: &mut SplitMix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let kind = rng.below(5) as u8;
        // A = R'R + shift: symmetric positive definite, scaled.
        let r: Vec<f64> = (0..n * n).map(|_| u(rng) - 0.5).collect();
        let mut a = vec![0.0; n * n];
        let shift = 10f64.powi(rng.range(-3, 1) as i32);
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                for k in 0..n {
                    s += r[k * n + i] * r[k * n + j];
                }
                a[i * n + j] = s + if i == j { shift } else { 0.0 };
            }
        }
        let scale = 10f64.powi(rng.range(-2, 3) as i32);
        let b = (0..n).map(|_| (u(rng) - 0.5) * scale).collect();
        let c = (0..n).map(|_| (u(rng) - 0.5) * 4.0).collect();
        LProblem {
            kind,
            n,
            a,
            b,
            c,
            bad_gradient: rng.below(12) == 0,
        }
    }

    fn fg(&self, x: &[f64], g: &mut [f64]) -> f64 {
        let n = self.n;
        let mut f = 0.0;
        match self.kind {
            0 => {
                // 0.5 x'Ax - b'x
                for i in 0..n {
                    let mut ax = 0.0;
                    for j in 0..n {
                        ax += self.a[i * n + j] * x[j];
                    }
                    f += 0.5 * x[i] * ax - self.b[i] * x[i];
                    g[i] = ax - self.b[i];
                }
            }
            1 => {
                // Extended Rosenbrock (pairs; an odd last variable gets a quadratic).
                g.iter_mut().for_each(|v| *v = 0.0);
                let mut i = 0;
                while i + 1 < n {
                    let t1 = x[i + 1] - x[i] * x[i];
                    let t2 = 1.0 - x[i];
                    f += 100.0 * t1 * t1 + t2 * t2;
                    g[i] += -400.0 * x[i] * t1 - 2.0 * t2;
                    g[i + 1] += 200.0 * t1;
                    i += 2;
                }
                if n % 2 == 1 {
                    let d = x[n - 1] - self.c[n - 1];
                    f += d * d;
                    g[n - 1] += 2.0 * d;
                }
            }
            2 => {
                // Pseudo-Huber around c, scaled by b.
                for i in 0..n {
                    let d = x[i] - self.c[i];
                    let s = 1.0 + self.b[i].abs();
                    let q = (1.0 + d * d).sqrt();
                    f += s * (q - 1.0);
                    g[i] = s * d / q;
                }
            }
            3 => {
                // Quartic with a coupling.
                for i in 0..n {
                    let d = x[i] - self.c[i];
                    f += d * d * d * d + 0.5 * d * d;
                    g[i] = 4.0 * d * d * d + d;
                }
                for i in 0..n.saturating_sub(1) {
                    f += 0.3 * x[i] * x[i + 1];
                    g[i] += 0.3 * x[i + 1];
                    g[i + 1] += 0.3 * x[i];
                }
            }
            _ => {
                // log(1 + |x - c|^2) plus a small quadratic (glibc's log, a callback both sides share).
                let mut s = 1.0;
                for i in 0..n {
                    let d = x[i] - self.c[i];
                    s += d * d;
                }
                f = math::log_glibc(s);
                for i in 0..n {
                    let d = x[i] - self.c[i];
                    f += 0.01 * x[i] * x[i];
                    g[i] = 2.0 * d / s + 0.02 * x[i];
                }
            }
        }
        if self.bad_gradient {
            g[0] = -g[0] * 3.0;
        }
        f
    }
}

fn state_rust(gl: &LbfgsGlobal) -> (Vec<i64>, Vec<f64>) {
    let ints = vec![
        gl.mp,
        gl.lp,
        gl.info,
        gl.infoc,
        gl.nfev,
        gl.maxfev,
        gl.inmc,
        gl.iscn,
        gl.iycn,
        gl.iter,
        gl.nfun,
        gl.ispt,
        gl.iypt,
        gl.bound,
        gl.point,
        i64::from(gl.finish),
        i64::from(gl.stage1),
        i64::from(gl.brackt),
    ];
    let dbls = vec![
        gl.gtol, gl.stpmin, gl.stpmax, gl.stpinit, gl.stp, gl.stp1, gl.beta, gl.ftol, gl.gnorm, gl.xnorm, gl.dg,
        gl.fm, gl.fx, gl.fy, gl.dgm, gl.dgx, gl.dgy, gl.fxm, gl.fym, gl.stx, gl.sty, gl.dgxm, gl.dgym, gl.finit,
        gl.width, gl.stmin, gl.stmax, gl.width1, gl.ftest1, gl.dginit, gl.dgtest,
    ];
    (ints, dbls)
}

/// One minimization run by the port alone: `(final f, function evaluations, last iflag, any
/// ITK-103 notice)`.
fn port_run(p: &LProblem, x0: &[f64], m: i64, gtol: f64, eps: f64, xtol: f64, nocedal: bool) -> (f64, u32, i64, bool) {
    let n = p.n;
    let mut x = x0.to_vec();
    let mut g = vec![0.0; n];
    let mut diag = vec![0.0; n];
    let mut w = vec![0.0; n * (2 * m as usize + 1) + 2 * m as usize];
    let mut gl = LbfgsGlobal::new();
    gl.gtol = gtol;
    gl.nocedal_first_stage = nocedal;
    let mode = UpstreamDefects::Fix;
    let mut iflag = 0i64;
    let mut evals = 0u32;
    let mut noticed = false;
    let mut f = 0.0;
    let mut quiet = larmorx_core::cfmt::Silent;
    while evals < 500 {
        f = p.fg(&x, &mut g);
        evals += 1;
        let call = lbfgs(n as i64, m, &mut x, f, &g, false, &mut diag, [-1, 0], eps, xtol, &mut w, &mut iflag, &mut gl, mode, &mut quiet);
        noticed |= call.notices.iter().any(|nt| nt.defect.id == "ITK-103");
        if iflag <= 0 {
            break;
        }
    }
    (f, evals, iflag, noticed)
}

/// ITK-103's measurement alone (the port only, no original): `problems` problems at FreeSurfer's
/// `OpenDFPMin` settings (memory 7, line-search accuracy 0.1, `xtol` 1e-16, `diagco` false) and at
/// VNL's defaults (memory 5, `gtol` 0.9), each run with Nocedal's first stage and with VXL's.
pub fn measure_itk103(problems: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x0103_5eed);
    for (label, m, gtol) in [("FreeSurfer's OpenDFPMin (m 7, gtol 0.1)", 7i64, 0.1), ("VNL's defaults (m 5, gtol 0.9)", 5, 0.9)] {
        let (mut changed, mut lower_f, mut higher_f, mut fewer, mut more, mut fail_fix, mut fail_vxl) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        let (mut evals_fix, mut evals_vxl) = (0u64, 0u64);
        for _ in 0..problems {
            t.cases += 1;
            let n = 1 + rng.below(30) as usize;
            let mut p = LProblem::new(&mut rng, n);
            p.bad_gradient = false;
            let x0: Vec<f64> = (0..n).map(|_| (rng.range(-3000, 3000) as f64) / 997.0).collect();
            let eps = [1e-5, 1e-4, 1e-6][rng.below(3) as usize];
            let fix = port_run(&p, &x0, m, gtol, eps, 1e-16, true);
            let vxl = port_run(&p, &x0, m, gtol, eps, 1e-16, false);
            evals_fix += u64::from(fix.1);
            evals_vxl += u64::from(vxl.1);
            if fix.0.to_bits() != vxl.0.to_bits() || fix.1 != vxl.1 || fix.2 != vxl.2 {
                changed += 1;
                lower_f += u64::from(fix.0 < vxl.0);
                higher_f += u64::from(fix.0 > vxl.0);
                fewer += u64::from(fix.1 < vxl.1);
                more += u64::from(fix.1 > vxl.1);
            }
            fail_fix += u64::from(fix.2 < 0);
            fail_vxl += u64::from(vxl.2 < 0);
        }
        t.notes.push(format!(
            "{label}: {problems} problems, the correction changed {changed} (final f lower {lower_f}, higher {higher_f}; fewer evaluations {fewer}, more {more}); evaluations in all {evals_fix} corrected, {evals_vxl} VXL; line-search failures {fail_fix} corrected, {fail_vxl} VXL"
        ));
    }
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x1bf9_5eed);
    let mut capture = crate::ffi::Capture::new(&std::env::temp_dir());
    let mut calls = 0u64;
    // ITK-103's measurement: cases where the correction changed the run, and how.
    let (mut changed, mut lower_f, mut higher_f, mut fewer, mut more) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut fail_fix, mut fail_vxl) = (0u64, 0u64);
    for case in 0..(3_000 * scale) {
        t.cases += 1;
        let n = 1 + rng.below(12) as usize;
        let p = LProblem::new(&mut rng, n);
        let m = [7, 1, 3, 5, 9][rng.below(5) as usize] as i64;
        let gtol = [0.1, 0.9, 1e-5, 0.5][rng.below(4) as usize];
        let stpinit = [1.0, 1.0, 0.5, 2.0][rng.below(4) as usize];
        let eps = [1e-5, 1e-3, 1e-8, 0.0][rng.below(4) as usize];
        let xtol = [1e-16, 1e-10][rng.below(2) as usize];
        let diagco = rng.below(15) == 0;
        let iprint: [i64; 2] = match rng.below(12) {
            0 => [1, 1],
            1 => [0, 0],
            2 => [1, 2],
            3 => [2, 3],
            4 => [0, 2],
            _ => [-1, 0],
        };
        let lp = if rng.below(10) == 0 { 0 } else { 6 };
        let x0: Vec<f64> = (0..n).map(|_| (rng.range(-3000, 3000) as f64) / 997.0).collect();
        let wlen = n * (2 * m as usize + 1) + 2 * m as usize;
        // The original's x and g with a 0 before each array: VXL's lb1 prints from there (ITK-106),
        // which the port's reproduce prints as 0.
        let mut xcb = vec![0.0; n + 1];
        xcb[1..].copy_from_slice(&x0);
        let mut gcb = vec![0.0; n + 1];
        let mut xr = x0.clone();
        let mut gr = vec![0.0; n];
        let (mut dc, mut dr) = (vec![0.0; n], vec![0.0; n]);
        let (mut wc, mut wr) = (vec![0.0; wlen], vec![0.0; wlen]);
        // SAFETY: the shim's state object, freed below.
        let gc_state = unsafe { (lib.lbfgs_new)() };
        unsafe { (lib.lbfgs_set)(gc_state, gtol, stpinit, lp) };
        let mut gl = LbfgsGlobal::new();
        gl.gtol = gtol;
        gl.stpinit = stpinit;
        gl.lp = lp;
        let (mut ifc, mut ifr): (c_long, i64) = (0, 0);
        let mut fc = 0.0;
        let mut fr = 0.0;
        for call in 0..300 {
            if ifc != 2 {
                fc = p.fg(&xcb[1..], &mut gcb[1..]);
                fr = p.fg(&xr, &mut gr);
            } else {
                for i in 0..n {
                    dc[i] = 1.0 / (1.0 + i as f64);
                    dr[i] = dc[i];
                }
            }
            let mut ip = [iprint[0] as c_long, iprint[1] as c_long];
            let (_, out_c, _) = capture.run(lib, || {
                // SAFETY: the arrays hold n, n, n and wlen values; gc_state is the shim's.
                unsafe {
                    (lib.lbfgs)(
                        n as c_long, m as c_long, xcb[1..].as_mut_ptr(), fc, gcb[1..].as_mut_ptr(), c_long::from(diagco),
                        dc.as_mut_ptr(), ip.as_mut_ptr(), eps, xtol, wc.as_mut_ptr(), &mut ifc, gc_state,
                    )
                }
            });
            let mut msgs = Captured::default();
            let res = lbfgs(
                n as i64, m, &mut xr, fr, &gr, diagco, &mut dr, iprint, eps, xtol, &mut wr, &mut ifr, &mut gl,
                UpstreamDefects::Reproduce, &mut msgs,
            );
            calls += 1;
            let _ = &res;
            let w = || format!("case {case} call {call} n={n} m={m} gtol={gtol} kind={} diagco={diagco}", p.kind);
            let mut ok = t.f64s(&|| format!("{} x", w()), &xr, &xcb[1..]);
            ok &= t.bits(&|| format!("{} iflag", w()), ifr as u64, ifc as u64);
            ok &= t.f64s(&|| format!("{} diag", w()), &dr, &dc);
            ok &= t.f64s(&|| format!("{} w", w()), &wr, &wc);
            let (mut ints, mut dbls) = (vec![0 as c_long; 18], vec![0.0f64; 31]);
            // SAFETY: arrays of the shim's 18 and 31 fields.
            unsafe { (lib.lbfgs_state)(gc_state, ints.as_mut_ptr(), dbls.as_mut_ptr()) };
            let (ri, rd) = state_rust(&gl);
            for (k, (a, b)) in ri.iter().zip(&ints).enumerate() {
                ok &= t.bits(&|| format!("{} state int {k}", w()), *a as u64, *b as u64);
            }
            ok &= t.f64s(&|| format!("{} state", w()), &rd, &dbls);
            ok &= t.text(&|| format!("{} stdout", w()), &msgs.text(Stream::Out), &out_c);
            if !ok || ifc <= 0 {
                break;
            }
        }
        // SAFETY: freeing the shim's state.
        unsafe { (lib.lbfgs_free)(gc_state as *mut c_void) };

        // ITK-103: the port's correction against VXL's behaviour, the port alone.
        let fix = port_run(&p, &x0, m, gtol, eps, xtol, true);
        let vxl = port_run(&p, &x0, m, gtol, eps, xtol, false);
        if fix.0.to_bits() != vxl.0.to_bits() || fix.1 != vxl.1 || fix.2 != vxl.2 {
            changed += 1;
            if fix.0 < vxl.0 {
                lower_f += 1;
            } else if fix.0 > vxl.0 {
                higher_f += 1;
            }
            if fix.1 < vxl.1 {
                fewer += 1;
            } else if fix.1 > vxl.1 {
                more += 1;
            }
        }
        fail_fix += u64::from(fix.2 < 0);
        fail_vxl += u64::from(vxl.2 < 0);
    }
    t.notes.push(format!("{calls} lbfgs calls in lock step (reproduce)"));
    t.notes.push(format!(
        "ITK-103 measured on {} problems (Nocedal's first stage against VXL's): the correction changed {changed} runs (final f lower in {lower_f}, higher in {higher_f}; fewer evaluations in {fewer}, more in {more}); line-search failures: corrected {fail_fix}, VXL {fail_vxl}",
        t.cases
    ));
}

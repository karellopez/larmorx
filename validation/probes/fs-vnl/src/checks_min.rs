// SPDX-License-Identifier: Apache-2.0
//! `vnl_bracket_minimum`, `vnl_brent_minimizer`, the deprecated `vnl_brent` and `vnl_powell`:
//! the same cost functions (Rust callbacks) through VNL and through the port; the points
//! evaluated, in order, and the results, bit for bit.

use std::ffi::{c_int, c_long, c_void};

use larmorx_core::cfmt::Captured;
use larmorx_core::math;
use larmorx_core::vnl_minimize::{self, Brent, BrentMinimizer, CostFunction, Powell};
use larmorx_core::x87::cases::SplitMix64;

use crate::{Lib, Tally};

/// A seeded test function of `n` variables: a sum over rotated coordinates of 1-D shapes
/// (smooth, with kinks, flat steps, plateaus, oscillations, rare NaN), recording every point it
/// is called at.
pub struct Problem {
    kinds: Vec<u8>,
    centres: Vec<f64>,
    scales: Vec<f64>,
    rot: Vec<f64>,
    n: usize,
    pub trace: Vec<f64>,
}

impl Problem {
    pub fn new(rng: &mut SplitMix64, n: usize) -> Problem {
        let u = |rng: &mut SplitMix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let kinds = (0..n).map(|_| rng.below(9) as u8).collect();
        let centres = (0..n).map(|_| (u(rng) - 0.5) * 10f64.powi(rng.range(-2, 3) as i32)).collect();
        let scales = (0..n).map(|_| 10f64.powi(rng.range(-3, 3) as i32) * (0.5 + u(rng))).collect();
        let mut rot = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                rot[i * n + j] = if i == j { 1.0 } else { (u(rng) - 0.5) * 0.6 };
            }
        }
        Problem {
            kinds,
            centres,
            scales,
            rot,
            n,
            trace: Vec::new(),
        }
    }

    pub fn eval(&mut self, x: &[f64]) -> f64 {
        self.trace.extend_from_slice(x);
        let mut total = 0.0;
        for i in 0..self.n {
            let mut y = 0.0;
            for j in 0..self.n {
                y += self.rot[i * self.n + j] * x[j];
            }
            let d = y - self.centres[i];
            let s = self.scales[i];
            total += match self.kinds[i] {
                0 => s * d * d,
                1 => s * d * d * d * d + 0.1 * d * d,
                2 => s * d.abs(),
                3 => s * d * d + 1e-3 * math::cos(37.0 * d),
                4 => s * (d * 8.0).floor().abs(),
                5 => s * (d * d).min(1.0),
                6 => s * d * d * (1.0 + 0.5 * math::sin(d)),
                7 => {
                    if d > 1e3 {
                        f64::NAN
                    } else {
                        s * d * d - 0.01 * d
                    }
                }
                _ => s * (1.0 - math::exp(-d * d)),
            };
        }
        total
    }
}

impl CostFunction for Problem {
    fn f(&mut self, x: &[f64]) -> f64 {
        self.eval(x)
    }
}

pub unsafe extern "C" fn cost(ctx: *mut c_void, x: *const f64, n: c_int) -> f64 {
    // SAFETY: ctx is a live &mut Problem and x holds n values (the shim's callback contract).
    let (p, xs) = unsafe { (&mut *(ctx.cast::<Problem>()), std::slice::from_raw_parts(x, n as usize)) };
    p.eval(xs)
}

fn ctx(p: &mut Problem) -> *mut c_void {
    (p as *mut Problem).cast()
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x0b5e_ed01);
    let mut capture = crate::ffi::Capture::new(&std::env::temp_dir());
    for case in 0..(6_000 * scale) {
        t.cases += 1;
        let seed = rng.next_u64();
        let fresh = |n: usize| Problem::new(&mut SplitMix64::new(seed), n);
        let mut r = SplitMix64::new(seed ^ 0x77);
        let a = (r.range(-4000, 4000) as f64) / 1000.0;
        let b = if r.below(8) == 0 { a } else { a + (r.range(-3000, 3000) as f64) / 997.0 };
        let xtol = [1e-6, 1e-4, 1e-8, 1e-3, 1e-12][r.below(5) as usize];
        let what = |s: &str| format!("case {case} {s} a={a} b={b} xtol={xtol}");

        // vnl_bracket_minimum.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        let mut out6 = [0.0f64; 6];
        // SAFETY: the shim's signature; pc lives through the call.
        unsafe { (lib.bracket)(cost, ctx(&mut pc), a, b, out6.as_mut_ptr()) };
        let br = vnl_minimize::bracket_minimum(&mut pr, a, b);
        t.f64s(&|| what("bracket"), &[br.a, br.b, br.c, br.fa, br.fb, br.fc], &out6);
        t.f64s(&|| what("bracket trace"), &pr.trace, &pc.trace);

        // vnl_brent_minimizer's entry points on that bracket (and minimize from a).
        for which in 0..4 {
            let (mut pc, mut pr) = (fresh(1), fresh(1));
            let args = [br.a, br.b, br.c, br.fa, br.fb, br.fc];
            let args = if which == 3 { [a, 0.0, 0.0, 0.0, 0.0, 0.0] } else { args };
            let mut fmin = 0.0;
            // SAFETY: as above.
            let xc = unsafe { (lib.brent_minimizer)(cost, ctx(&mut pc), which, xtol, args.as_ptr(), &mut fmin) };
            let mut bm = BrentMinimizer::new();
            bm.xtol = xtol;
            let xr = match which {
                0 => bm.minimize_given_bounds(&mut pr, args[0], args[1], args[2]),
                1 => bm.minimize_given_bounds_and_one_f(&mut pr, args[0], args[1], args[2], args[3]),
                2 => bm.minimize_given_bounds_and_all_f(&mut pr, args[0], args[1], args[2], args[3], args[4], args[5]),
                _ => bm.minimize(&mut pr, args[0]),
            };
            t.f64s(&|| what(&format!("brent {which}")), &[xr, bm.f_at_last_minimum], &[xc, fmin]);
            t.f64s(&|| what(&format!("brent {which} trace")), &pr.trace, &pc.trace);
        }

        // fs_powell's step with the deprecated vnl_brent: bracket from (0, step), then the bounds.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        let step = [1.0, 0.1, 2.5, a][r.below(4) as usize];
        let mut io5 = [0.0, step, 0.0, 0.0, 0.0];
        // SAFETY: as above.
        unsafe { (lib.vnl_brent_powell_step)(cost, ctx(&mut pc), xtol, io5.as_mut_ptr()) };
        let mut brent = Brent::new();
        let ((ax, xx, bx), _) = brent.bracket_minimum(&mut pr, 0.0, step, 0.0);
        let (fret, xmin) = brent.minimize_given_bounds(&mut pr, bx, xx, ax, xtol);
        t.f64s(&|| what("vnl_brent step"), &[ax, xx, bx, xmin, fret], &io5);
        t.f64s(&|| what("vnl_brent step trace"), &pr.trace, &pc.trace);

        // vnl_brent::minimize.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        // SAFETY: as above.
        let xc = unsafe { (lib.vnl_brent_minimize)(cost, ctx(&mut pc), a) };
        let xr = Brent::new().minimize(&mut pr, a);
        t.f64s(&|| what("vnl_brent minimize"), &[xr], &[xc]);
        t.f64s(&|| what("vnl_brent minimize trace"), &pr.trace, &pc.trace);

        // vnl_powell in 1 to 6 dimensions (a few verbose, with the printed lines compared).
        if case % 3 == 0 {
            let n = 1 + r.below(6) as usize;
            let (mut pc, mut pr) = (fresh(n), fresh(n));
            let p0: Vec<f64> = (0..n).map(|_| (r.range(-3000, 3000) as f64) / 1000.0).collect();
            let linmin = [1e-4, 1e-3, 1e-6][r.below(3) as usize];
            let step = [1.0, 0.5, 2.0][r.below(3) as usize];
            let ftol = [0.0, 1e-6, 1e-3][r.below(3) as usize];
            let maxfev = [2000, 3, 10][r.below(3) as usize];
            let verbose = r.below(10) == 0;
            let mut pcv = p0.clone();
            let (mut iters, mut evals, mut se, mut ee): (c_int, c_long, f64, f64) = (0, 0, 0.0, 0.0);
            let (code, _o, err) = capture.run(lib, || {
                // SAFETY: as above; pcv holds n values.
                unsafe {
                    (lib.powell)(
                        cost, ctx(&mut pc), n as c_int, pcv.as_mut_ptr(), linmin, step, ftol, maxfev,
                        c_int::from(verbose), &mut iters, &mut evals, &mut se, &mut ee,
                    )
                }
            });
            let mut pw = Powell::new();
            pw.linmin_xtol = linmin;
            pw.initial_step = step;
            if ftol > 0.0 {
                pw.base.ftol = ftol;
            }
            pw.base.maxfev = maxfev as i64;
            pw.base.verbose = verbose;
            let mut prv = p0.clone();
            let mut msgs = Captured::default();
            let rc = pw.minimize(&mut pr, &mut prv, &mut msgs);
            let w = || what(&format!("powell n={n} linmin={linmin} step={step} ftol={ftol} maxfev={maxfev}"));
            t.f64s(&w, &prv, &pcv);
            t.bits(&w, rc.value() as u64, code as u64);
            t.bits(&w, u64::from(pw.base.num_iterations), iters as u64);
            t.bits(&w, pw.base.num_evaluations as u64, evals as u64);
            t.f64s(&w, &[pw.base.start_error, pw.base.end_error], &[se, ee]);
            t.f64s(&|| format!("{} trace", w()), &pr.trace, &pc.trace);
            t.text(&|| format!("{} stderr", w()), &msgs.text(larmorx_core::cfmt::Stream::Err), &err);
        }
    }
}

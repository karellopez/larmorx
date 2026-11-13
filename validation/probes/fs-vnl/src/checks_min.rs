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

/// A seeded cost of `n` variables that falls without end along most lines (linear, quadratic,
/// cubic, exponential, square-root descents), or jumps to an infinity or to NaNs of its own
/// (quiet NaNs whose sign and payload come from the call count), recording every point it is
/// called at (as a digest of their bits and a count: a Brent search that overflows can creep
/// through 10^8 points before it ends): the minimizers then run into infinities and make NaNs of
/// them (`inf - inf`, `0 * inf`, `inf / inf`), whose bits are compared exactly.
pub struct Falling {
    kind: u8,
    dir: Vec<f64>,
    centre: f64,
    scale: f64,
    threshold: f64,
    pub calls: u64,
    digest: u64,
    /// Points with a NaN coordinate.
    pub nan_points: u64,
    /// `FS_VNL_UNBOUNDED_CAP`: after this many calls the cost is NaN (for runs under `nan_trap.c`,
    /// where every NaN made from numbers costs two signals and a search that overflows can creep
    /// through 10^8 points); 0, the default, for none.
    cap: u64,
}

impl Falling {
    pub fn new(rng: &mut SplitMix64, n: usize) -> Falling {
        let u = |rng: &mut SplitMix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        Falling {
            kind: rng.below(10) as u8,
            dir: (0..n).map(|i| if i == 0 { 1.0 } else { (u(rng) - 0.5) * 0.8 }).collect(),
            centre: (u(rng) - 0.5) * 10f64.powi(rng.range(-2, 3) as i32),
            scale: 10f64.powi(rng.range(-3, 4) as i32) * (0.5 + u(rng)) * if rng.below(2) == 0 { 1.0 } else { -1.0 },
            threshold: 10f64.powi(rng.range(0, 40) as i32),
            calls: 0,
            digest: 0xcbf2_9ce4_8422_2325,
            nan_points: 0,
            cap: std::env::var("FS_VNL_UNBOUNDED_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
        }
    }

    pub fn eval(&mut self, x: &[f64]) -> f64 {
        self.calls += 1;
        if self.calls > 4_000_000_000 {
            println!("HANG: a falling cost called more than 4,000,000,000 times (kind {})", self.kind);
            std::process::exit(2);
        }
        for xi in x {
            self.digest = (self.digest ^ xi.to_bits()).wrapping_mul(0x0100_0000_01b3);
        }
        self.nan_points += u64::from(x.iter().any(|v| v.is_nan()));
        if self.cap > 0 && self.calls > self.cap {
            return f64::NAN;
        }
        let mut y = 0.0;
        for (xi, di) in x.iter().zip(&self.dir) {
            y += xi * di;
        }
        let d = y - self.centre;
        let s = self.scale;
        let own_nan = f64::from_bits(
            0x7ff8_0000_0000_0000 | (self.calls & 0xffff) | if self.calls & 1 == 1 { 1 << 63 } else { 0 },
        );
        match self.kind {
            0 => -s * d,
            1 => -s.abs() * d * d,
            2 => s * d * d * d,
            3 => -s.abs() * math::exp(d * s.signum()),
            4 => -s.abs() * d.abs().sqrt(),
            5 => {
                if d.abs() > self.threshold {
                    f64::NEG_INFINITY
                } else {
                    -s * d
                }
            }
            6 => {
                if d.abs() > self.threshold {
                    own_nan
                } else {
                    -s * d
                }
            }
            7 => {
                if d.abs() > self.threshold {
                    f64::INFINITY
                } else {
                    -s.abs() * d * d
                }
            }
            8 => {
                // Falls to one side, a NaN of its own for a NaN point.
                if d.is_nan() { own_nan } else { -s * d }
            }
            _ => -s * d + 1e-3 * math::cos(d),
        }
    }
}

impl Falling {
    /// The points it was called at: their count and digest.
    pub fn trace(&self) -> [f64; 2] {
        [f64::from_bits(self.calls), f64::from_bits(self.digest)]
    }
}

impl CostFunction for Falling {
    fn f(&mut self, x: &[f64]) -> f64 {
        self.eval(x)
    }
}

pub unsafe extern "C" fn falling_cost(ctx: *mut c_void, x: *const f64, n: c_int) -> f64 {
    // SAFETY: ctx is a live &mut Falling and x holds n values (the shim's callback contract).
    let (p, xs) = unsafe { (&mut *(ctx.cast::<Falling>()), std::slice::from_raw_parts(x, n as usize)) };
    p.eval(xs)
}

/// An infinity, a NaN (either sign, with or without a payload) or a value near overflow.
fn special(r: &mut SplitMix64) -> f64 {
    const S: [u64; 8] = [
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0000,
        0xfff8_0000_0000_0000,
        0x7ff8_0000_0000_0042,
        0xfff8_0000_0000_0777,
        0x7fe0_0000_0000_0000,
        0xffe1_0000_0000_0000,
    ];
    f64::from_bits(S[r.below(8) as usize])
}

fn w_case(case: u64, kind: u8, n: usize, maxfev: c_int) -> String {
    format!("unbounded case {case} kind {kind} powell n={n} maxfev={maxfev}")
}

fn fctx(p: &mut Falling) -> *mut c_void {
    (p as *mut Falling).cast()
}

/// The minimizers on [`Falling`] costs: every value and every point bit for bit, NaNs included
/// (the tally is strict). The port runs first: where it stops a loop VNL never leaves (ITK-114),
/// VNL is not called (it would not return) and the case is counted.
pub fn run_unbounded(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x0b5e_ed99);
    let mut capture = crate::ffi::Capture::new(&std::env::temp_dir());
    let (mut nan_values, mut nan_points, mut itk114, mut calls) = (0u64, 0u64, 0u64, 0u64);
    // FS_VNL_DEBUG=1: each call on standard error before it runs (to find a call that does not return).
    let dbg = std::env::var_os("FS_VNL_DEBUG").is_some();
    let hangs = |notices: &[larmorx_core::defects::Notice]| notices.iter().any(|n| n.defect.id == "ITK-114");
    for case in 0..(2_000 * scale) {
        t.cases += 1;
        let seed = rng.next_u64();
        let fresh = |n: usize| Falling::new(&mut SplitMix64::new(seed), n);
        let mut r = SplitMix64::new(seed ^ 0x99);
        let mut a = (r.range(-4000, 4000) as f64) / 1000.0 * 10f64.powi(r.range(0, 6) as i32);
        let mut b = if r.below(8) == 0 { a } else { a + (r.range(-3000, 3000) as f64) / 997.0 };
        // Starting points at infinities, NaNs of both signs and with payloads, or near overflow.
        if r.below(6) == 0 {
            a = special(&mut r);
        }
        if r.below(6) == 0 {
            b = if r.below(3) == 0 { a } else { special(&mut r) };
        }
        let xtol = [1e-6, 1e-4, 1e-8, 1e-3][r.below(4) as usize];
        let kind = fresh(1).kind;
        let what = |s: &str| format!("unbounded case {case} kind {kind} {s} a={a} b={b} xtol={xtol}");
        let mut count = |v: &[f64]| nan_values += v.iter().filter(|x| x.is_nan()).count() as u64;

        if dbg {
            eprintln!("case {case} kind {kind} a={a:e} ({:#x}) b={b:e} ({:#x}) xtol={xtol}", a.to_bits(), b.to_bits());
        }
        // vnl_bracket_minimum.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        let br = vnl_minimize::bracket_minimum(&mut pr, a, b);
        let mut out6 = [0.0f64; 6];
        // SAFETY: the shim's signature; pc lives through the call.
        unsafe { (lib.bracket)(falling_cost, fctx(&mut pc), a, b, out6.as_mut_ptr()) };
        t.f64s(&|| what("bracket"), &[br.a, br.b, br.c, br.fa, br.fb, br.fc], &out6);
        t.f64s(&|| what("bracket trace"), &pr.trace(), &pc.trace());
        nan_points += pc.nan_points;
                count(&out6);
        calls += 2;

        for which in 0..4 {
            let (mut pc, mut pr) = (fresh(1), fresh(1));
            let args = [br.a, br.b, br.c, br.fa, br.fb, br.fc];
            let args = if which == 3 { [a, 0.0, 0.0, 0.0, 0.0, 0.0] } else { args };
            let mut bm = BrentMinimizer::new();
            bm.xtol = xtol;
            if dbg {
                eprintln!("  brent {which} {args:?}");
            }
            let xr = match which {
                0 => bm.minimize_given_bounds(&mut pr, args[0], args[1], args[2]),
                1 => bm.minimize_given_bounds_and_one_f(&mut pr, args[0], args[1], args[2], args[3]),
                2 => bm.minimize_given_bounds_and_all_f(&mut pr, args[0], args[1], args[2], args[3], args[4], args[5]),
                _ => bm.minimize(&mut pr, args[0]),
            };
            calls += 1;
            if hangs(&bm.notices) {
                itk114 += 1;
                continue;
            }
            let mut fmin = 0.0;
            // SAFETY: as above.
            let xc = unsafe { (lib.brent_minimizer)(falling_cost, fctx(&mut pc), which, xtol, args.as_ptr(), &mut fmin) };
            t.f64s(&|| what(&format!("brent {which}")), &[xr, bm.f_at_last_minimum], &[xc, fmin]);
            t.f64s(&|| what(&format!("brent {which} trace")), &pr.trace(), &pc.trace());
            nan_points += pc.nan_points;
                        count(&[xc, fmin]);
        }

        // fs_powell's step with the deprecated vnl_brent.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        let step = [1.0, 0.1, 2.5, a][r.below(4) as usize];
        let mut brent = Brent::new();
        if dbg {
            eprintln!("  vnl_brent step {step:e}");
        }
        let ((ax, xx, bx), _) = brent.bracket_minimum(&mut pr, 0.0, step, 0.0);
        let (fret, xmin) = brent.minimize_given_bounds(&mut pr, bx, xx, ax, xtol);
        calls += 1;
        if hangs(&brent.inner.notices) {
            itk114 += 1;
        } else {
            let mut io5 = [0.0, step, 0.0, 0.0, 0.0];
            // SAFETY: as above.
            unsafe { (lib.vnl_brent_powell_step)(falling_cost, fctx(&mut pc), xtol, io5.as_mut_ptr()) };
            t.f64s(&|| what("vnl_brent step"), &[ax, xx, bx, xmin, fret], &io5);
            t.f64s(&|| what("vnl_brent step trace"), &pr.trace(), &pc.trace());
            nan_points += pc.nan_points;
                    }

        // vnl_brent::minimize.
        let (mut pc, mut pr) = (fresh(1), fresh(1));
        let mut brent = Brent::new();
        if dbg {
            eprintln!("  vnl_brent minimize");
        }
        let xr = brent.minimize(&mut pr, a);
        calls += 1;
        if hangs(&brent.inner.notices) {
            itk114 += 1;
        } else {
            // SAFETY: as above.
            let xc = unsafe { (lib.vnl_brent_minimize)(falling_cost, fctx(&mut pc), a) };
            t.f64s(&|| what("vnl_brent minimize"), &[xr], &[xc]);
            t.f64s(&|| what("vnl_brent minimize trace"), &pr.trace(), &pc.trace());
            nan_points += pc.nan_points;
                    }

        // vnl_powell in 1 to 4 dimensions.
        if case % 2 == 0 {
            let n = 1 + r.below(4) as usize;
            let (mut pc, mut pr) = (fresh(n), fresh(n));
            let mut p0: Vec<f64> = (0..n).map(|_| (r.range(-3000, 3000) as f64) / 1000.0).collect();
            if r.below(4) == 0 {
                let k = r.below(n as u64) as usize;
                p0[k] = special(&mut r);
            }
            let linmin = [1e-4, 1e-3, 1e-6][r.below(3) as usize];
            let step = [1.0, 0.5, 2.0][r.below(3) as usize];
            let maxfev = [2000, 3, 10, 40][r.below(4) as usize];
            let verbose = r.below(6) == 0;
            let mut pw = Powell::new();
            pw.linmin_xtol = linmin;
            pw.initial_step = step;
            pw.base.maxfev = maxfev as i64;
            pw.base.verbose = verbose;
            let mut prv = p0.clone();
            let mut msgs = Captured::default();
            if dbg {
                eprintln!("  {} from {p0:?}", w_case(case, kind, n, maxfev));
            }
            let rc = pw.minimize(&mut pr, &mut prv, &mut msgs);
            calls += 1;
            if hangs(&pw.notices) {
                itk114 += 1;
                continue;
            }
            if dbg {
                eprintln!("  the port: {rc:?}, {} evaluations; now VNL", pr.calls);
            }
            let mut pcv = p0.clone();
            let (mut iters, mut evals, mut se, mut ee): (c_int, c_long, f64, f64) = (0, 0, 0.0, 0.0);
            let (code, _o, err) = capture.run(lib, || {
                // SAFETY: as above; pcv holds n values.
                unsafe {
                    (lib.powell)(
                        falling_cost, fctx(&mut pc), n as c_int, pcv.as_mut_ptr(), linmin, step, 0.0, maxfev,
                        c_int::from(verbose), &mut iters, &mut evals, &mut se, &mut ee,
                    )
                }
            });
            let w = || what(&format!("powell n={n} linmin={linmin} step={step} maxfev={maxfev}"));
            t.f64s(&w, &prv, &pcv);
            t.bits(&w, rc.value() as u64, code as u64);
            t.bits(&w, u64::from(pw.base.num_iterations), iters as u64);
            t.bits(&w, pw.base.num_evaluations as u64, evals as u64);
            t.f64s(&w, &[pw.base.start_error, pw.base.end_error], &[se, ee]);
            t.f64s(&|| format!("{} trace", w()), &pr.trace(), &pc.trace());
            nan_points += pc.nan_points;
            t.text(&|| format!("{} stderr", w()), &msgs.text(larmorx_core::cfmt::Stream::Err), &err);
                    }
    }
    t.notes.push(format!(
        "{calls} minimizations; {itk114} stopped by the port where VNL never returns (ITK-114), VNL not called; \
         the original's NaN results {nan_values}, points with a NaN coordinate {nan_points}"
    ));
}

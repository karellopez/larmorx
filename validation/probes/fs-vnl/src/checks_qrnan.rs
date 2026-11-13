// SPDX-License-Identifier: Apache-2.0
//! Family `qrnan`: LINPACK's QR as `vnl_qr<float>` and `vnl_qr<double>` compute it (ITK 4.13.2's
//! netlib and VNL as the unit oracle built them) on NaNs and other special entries, every value
//! compared bit for bit, NaNs included (their signs and payloads come from the operand order of
//! each `+` and `*`, which GCC chooses per statement and per unrolled or vectorized position).
//!
//! 1. The BLAS kernels the QR calls (`sdot`, `ddot`, `saxpy`, `daxpy`, `sscal`, `dscal`), called
//!    directly, on NaNs with distinct payloads: for every length up to 300, the order in which the
//!    kernel passes its NaN operands on (a dot product: all its operands NaN, then the winner
//!    replaced by 1, until none is left, its arrays at four alignments; `axpy`: every subset of
//!    `a`, `x`, `y` NaN; `scal`: `a` and `x` NaN). Equal orders mean the same first operand in
//!    every operation.
//! 2. The same kernels and `snrm2`/`dnrm2` on random vectors of special values (NaNs of both
//!    signs with random payloads, quiet and signalling, infinities, zeros, overflowing and
//!    underflowing magnitudes), lengths 0 to 300, `axpy` also on two columns of one array.
//! 3. The QR of matrices of 1 to 12 rows and columns with such entries: `Q()`, `R()`,
//!    `determinant()`, `QtB`, `solve` (and its message), `inverse()`; and `vnl_determinant<T>` of
//!    5 to 12 rows (the QR determinant, `balance` false).

use std::ffi::{c_int, c_long};

use larmorx_core::itk_real::ItkReal;
use larmorx_core::vnl_fixed::{self, cases};
use larmorx_core::vnl_qr::{self, Qr, RANK_DEFICIENT};
use larmorx_core::x87::cases::SplitMix64;

use crate::ffi::Capture;
use crate::{Lib, Tally};

/// The two element types, with the official kernels and VNL's QR entry points of each.
trait Elem: ItkReal + PartialEq {
    const NAME: &'static str;
    fn nan(tag: u32) -> Self;
    /// The tag of a NaN made by [`Elem::nan`] (sign and quiet bit ignored); `None` otherwise.
    fn tag(self) -> Option<u32>;
    fn bits64(self) -> u64;
    fn special(rng: &mut SplitMix64) -> Self;
    fn square(rng: &mut SplitMix64, n: usize) -> Vec<Self>;
    fn dot(lib: &Lib, x: &[Self], y: &[Self]) -> Self;
    fn axpy(lib: &Lib, a: Self, x: *const Self, y: *mut Self, n: usize);
    fn scal(lib: &Lib, a: Self, x: &mut [Self]);
    fn nrm2(lib: &Lib, x: &[Self]) -> Self;
    fn qr(lib: &Lib, r: usize, c: usize, m: &[Self], q: &mut [Self], rr: &mut [Self]) -> Self;
    fn qtb(lib: &Lib, r: usize, c: usize, m: &[Self], b: &[Self], out: &mut [Self]);
    fn solve(lib: &Lib, r: usize, c: usize, m: &[Self], b: &[Self], out: &mut [Self]);
    fn inverse(lib: &Lib, n: usize, m: &[Self], out: &mut [Self]);
    fn determinant(lib: &Lib, n: usize, m: &[Self]) -> Self;
}

macro_rules! elem {
    ($t:ty, $name:literal, $quiet:expr, $mask:expr, $bits:ty, $special:path, $square:path,
     $dot:ident, $axpy:ident, $scal:ident, $nrm2:ident, $qr:ident, $qtb:ident, $solve:ident,
     $inverse:ident, $det:ident) => {
        impl Elem for $t {
            const NAME: &'static str = $name;
            fn nan(tag: u32) -> Self {
                <$t>::from_bits($quiet | tag as $bits)
            }
            fn tag(self) -> Option<u32> {
                self.is_nan().then(|| (self.to_bits() & $mask) as u32)
            }
            fn bits64(self) -> u64 {
                self.to_bits() as u64
            }
            fn special(rng: &mut SplitMix64) -> Self {
                $special(rng)
            }
            fn square(rng: &mut SplitMix64, n: usize) -> Vec<Self> {
                $square(rng, n)
            }
            fn dot(lib: &Lib, x: &[Self], y: &[Self]) -> Self {
                let (n, one) = (x.len() as c_long, 1 as c_long);
                // SAFETY: n elements in each array, unit strides (netlib's interface).
                unsafe { (lib.$dot)(&n, x.as_ptr(), &one, y.as_ptr(), &one) as $t }
            }
            fn axpy(lib: &Lib, a: Self, x: *const Self, y: *mut Self, n: usize) {
                let (n, one) = (n as c_long, 1 as c_long);
                // SAFETY: the caller passes n elements at x and at y.
                unsafe { (lib.$axpy)(&n, &a, x, &one, y, &one) };
            }
            fn scal(lib: &Lib, a: Self, x: &mut [Self]) {
                let (n, one) = (x.len() as c_long, 1 as c_long);
                // SAFETY: n elements, unit stride.
                unsafe { (lib.$scal)(&n, &a, x.as_mut_ptr(), &one) };
            }
            fn nrm2(lib: &Lib, x: &[Self]) -> Self {
                let (n, one) = (x.len() as c_long, 1 as c_long);
                // SAFETY: n elements, unit stride.
                unsafe { (lib.$nrm2)(&n, x.as_ptr(), &one) as $t }
            }
            fn qr(lib: &Lib, r: usize, c: usize, m: &[Self], q: &mut [Self], rr: &mut [Self]) -> Self {
                // SAFETY: r x c in, r x r and r x c out (the shim's contract).
                unsafe { (lib.$qr)(r as c_int, c as c_int, m.as_ptr(), q.as_mut_ptr(), rr.as_mut_ptr()) }
            }
            fn qtb(lib: &Lib, r: usize, c: usize, m: &[Self], b: &[Self], out: &mut [Self]) {
                // SAFETY: r x c and r in, r out.
                unsafe { (lib.$qtb)(r as c_int, c as c_int, m.as_ptr(), b.as_ptr(), out.as_mut_ptr()) };
            }
            fn solve(lib: &Lib, r: usize, c: usize, m: &[Self], b: &[Self], out: &mut [Self]) {
                // SAFETY: r x c and r in, c out.
                unsafe { (lib.$solve)(r as c_int, c as c_int, m.as_ptr(), b.as_ptr(), out.as_mut_ptr()) };
            }
            fn inverse(lib: &Lib, n: usize, m: &[Self], out: &mut [Self]) {
                // SAFETY: n x n in and out.
                unsafe { (lib.$inverse)(n as c_int, m.as_ptr(), out.as_mut_ptr()) };
            }
            fn determinant(lib: &Lib, n: usize, m: &[Self]) -> Self {
                // SAFETY: n x n in; balance false.
                unsafe { (lib.$det)(n as c_int, m.as_ptr(), 0) }
            }
        }
    };
}

elem!(f32, "f32", 0x7fc0_0000u32, 0x003f_ffffu32, u32, cases::special_f32, cases::matrix_f32,
      k_sdot, k_saxpy, k_sscal, k_snrm2, qr_f32, qr_qtb_f32, qr_solve_f32, qr_inverse_f32, determinant_f32);
elem!(f64, "f64", 0x7ff8_0000_0000_0000u64, 0x0007_ffff_ffff_ffffu64, u64, cases::special_f64, cases::matrix_f64,
      k_ddot, k_daxpy, k_dscal, k_dnrm2, qr_f64, qr_qtb_f64, qr_solve_f64, qr_inverse_f64, determinant_f64);

fn cmp<T: Elem>(t: &mut Tally, what: &dyn Fn() -> String, ours: &[T], theirs: &[T]) {
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        t.bits(&|| format!("{} [{i}]", what()), a.bits64(), b.bits64());
    }
    if ours.len() != theirs.len() {
        t.bits(&|| format!("{} length", what()), ours.len() as u64, theirs.len() as u64);
    }
}

/// Part 1: the order in which each kernel passes NaN operands on, every length to `max_n`.
fn orders<T: Elem>(lib: &Lib, max_n: usize, t: &mut Tally) {
    for n in 1..=max_n {
        t.cases += 1;
        // the dot product's 2n operands: x[i] tagged 1 + i, y[i] tagged 1 + n + i
        let mut live = vec![true; 2 * n];
        let (mut ours, mut theirs) = (Vec::new(), Vec::new());
        // the arrays start at four alignments (each element count from n)
        let (ox, oy) = (n % 4, (n / 4) % 4);
        for _ in 0..2 * n {
            let v = |k: usize, tag: u32| if live[k] { T::nan(tag) } else { T::ONE };
            let xs: Vec<T> = (0..ox).map(|_| T::ONE).chain((0..n).map(|i| v(i, 1 + i as u32))).collect();
            let ys: Vec<T> = (0..oy).map(|_| T::ONE).chain((0..n).map(|i| v(n + i, (1 + n + i) as u32))).collect();
            let (x, y) = (&xs[ox..], &ys[oy..]);
            let (a, b) = (vnl_qr::dot_t(x, y).tag(), T::dot(lib, x, y).tag());
            ours.push(a.unwrap_or(0));
            theirs.push(b.unwrap_or(0));
            match b {
                Some(k) if (1..=2 * n as u32).contains(&k) => live[k as usize - 1] = false,
                _ => break,
            }
        }
        t.bits(&|| format!("{} dot order n={n}: port {ours:?} original {theirs:?}", T::NAME), 0, u64::from(ours != theirs));
        // axpy: every subset of {a, x, y} NaN
        for s in 1..8u32 {
            let a = if s & 1 != 0 { T::nan(0x3000) } else { T::from_f64(2.0) };
            let x: Vec<T> = (0..n).map(|i| if s & 2 != 0 { T::nan(1 + i as u32) } else { T::from_f64(3.0) }).collect();
            let y0: Vec<T> = (0..n).map(|i| if s & 4 != 0 { T::nan(0x1000 + i as u32) } else { T::from_f64(5.0) }).collect();
            let mut ours = y0.clone();
            vnl_qr::axpy_t(a, &x, &mut ours);
            let mut theirs = y0.clone();
            T::axpy(lib, a, x.as_ptr(), theirs.as_mut_ptr(), n);
            cmp(t, &|| format!("{} axpy n={n} NaN set {s}", T::NAME), &ours, &theirs);
        }
        // scal: a and x NaN
        let a = T::nan(0x3000);
        let x0: Vec<T> = (0..n).map(|i| T::nan(1 + i as u32)).collect();
        let mut ours = x0.clone();
        vnl_qr::scal_t(a, &mut ours);
        let mut theirs = x0;
        T::scal(lib, a, &mut theirs);
        cmp(t, &|| format!("{} scal n={n}", T::NAME), &ours, &theirs);
    }
}

/// A vector of `n` entries, some or all of them special.
fn vector<T: Elem>(rng: &mut SplitMix64, n: usize) -> Vec<T> {
    let dense = rng.below(3);
    (0..n)
        .map(|_| {
            if dense == 0 || rng.below(4) == 0 {
                T::special(rng)
            } else {
                T::from_f64(rng.range(-4000, 4000) as f64 / 64.0)
            }
        })
        .collect()
}

/// Part 2: the kernels on random special vectors.
fn kernels<T: Elem>(lib: &Lib, count: u64, rng: &mut SplitMix64, t: &mut Tally) {
    for k in 0..count {
        t.cases += 1;
        let n = if k % 4 == 0 { rng.below(301) as usize } else { rng.below(41) as usize };
        let (x, y) = (vector::<T>(rng, n), vector::<T>(rng, n));
        let a = match rng.below(4) {
            0 => T::special(rng),
            1 => T::ZERO,
            _ => T::from_f64(rng.range(-64, 64) as f64 / 8.0),
        };
        let w = |f: &str| format!("{} {f} n={n} case {k}", T::NAME);
        cmp(t, &|| w("dot"), &[vnl_qr::dot_t(&x, &y)], &[T::dot(lib, &x, &y)]);
        cmp(t, &|| w("nrm2"), &[vnl_qr::nrm2_t(&x)], &[T::nrm2(lib, &x)]);
        let mut ours = y.clone();
        vnl_qr::axpy_t(a, &x, &mut ours);
        let mut theirs = y.clone();
        T::axpy(lib, a, x.as_ptr(), theirs.as_mut_ptr(), n);
        cmp(t, &|| w("axpy"), &ours, &theirs);
        // two columns of one array, as LINPACK's QR passes them
        let mut ours: Vec<T> = x.iter().chain(&y).copied().collect();
        let mut theirs = ours.clone();
        {
            let (xs, ys) = ours.split_at_mut(n);
            vnl_qr::axpy_t(a, xs, ys);
        }
        let p = theirs.as_mut_ptr();
        // SAFETY: the array holds 2n elements; x is its first half, y its second.
        T::axpy(lib, a, p, unsafe { p.add(n) }, n);
        cmp(t, &|| w("axpy in one array"), &ours, &theirs);
        let mut ours = x.clone();
        vnl_qr::scal_t(a, &mut ours);
        let mut theirs = x;
        T::scal(lib, a, &mut theirs);
        cmp(t, &|| w("scal"), &ours, &theirs);
    }
}

/// Part 3: the QR of matrices with special entries.
fn qr<T: Elem>(lib: &Lib, count: u64, rng: &mut SplitMix64, capture: &mut Capture, t: &mut Tally) -> u64 {
    let mut skipped = 0u64;
    for k in 0..count {
        t.cases += 1;
        let (r, c) = (1 + rng.below(12) as usize, 1 + rng.below(12) as usize);
        let big = T::square(rng, r.max(c));
        let m: Vec<T> = (0..r * c).map(|i| big[(i / c) * r.max(c) + i % c]).collect();
        let b = vector::<T>(rng, r);
        let w = |f: &str| format!("{} qr {r}x{c} {f} case {k}", T::NAME);
        let qr = Qr::new(&m, r, c);
        let (mut q, mut rr) = (vec![T::ZERO; r * r], vec![T::ZERO; r * c]);
        let det = T::qr(lib, r, c, &m, &mut q, &mut rr);
        cmp(t, &|| w("Q"), &qr.q(), &q);
        cmp(t, &|| w("R"), &qr.r(), &rr);
        cmp(t, &|| w("determinant"), &[qr.determinant()], &[det]);
        let mut qtb = vec![T::ZERO; r];
        T::qtb(lib, r, c, &m, &b, &mut qtb);
        cmp(t, &|| w("QtB"), &qr.qtb(&b), &qtb);
        if r >= c {
            let mut x = vec![T::ZERO; c];
            let (_, _, err) = capture.run(lib, || T::solve(lib, r, c, &m, &b, &mut x));
            let (xr, info) = qr.solve(&b);
            if r == 1 && info == 1 {
                skipped += 1; // VNL never writes the solution (ITK-113)
            } else {
                cmp(t, &|| w("solve"), &xr, &x);
            }
            let msg = if info > 0 { format!("{RANK_DEFICIENT}{info}\n") } else { String::new() };
            let theirs = err.find("Modules/").map_or(err.as_str(), |i| &err[i..]);
            t.text(&|| w("solve message"), &msg, theirs);
        }
        if r == c {
            let mut inv = vec![T::ZERO; r * r];
            let _ = capture.run(lib, || T::inverse(lib, r, &m, &mut inv));
            let (ours, infos) = qr.inverse();
            if r == 1 && infos[0] == 1 {
                skipped += 1;
            } else {
                cmp(t, &|| w("inverse"), &ours, &inv);
            }
        }
        // vnl_determinant<T>(M) above 4 x 4: the QR determinant
        let n = 5 + rng.below(8) as usize;
        let sq = T::square(rng, n);
        cmp(t, &|| format!("{} vnl_determinant n={n} case {k}", T::NAME), &[vnl_fixed::determinant(&sq, n)], &[T::determinant(lib, n, &sq)]);
    }
    skipped
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut capture = Capture::new(&std::env::temp_dir());
    let mut rng = SplitMix64::new(0x9a7a_0e5d_1a5e);
    orders::<f32>(lib, 300, t);
    orders::<f64>(lib, 300, t);
    kernels::<f32>(lib, 20_000 * scale, &mut rng, t);
    kernels::<f64>(lib, 20_000 * scale, &mut rng, t);
    let s32 = qr::<f32>(lib, 20_000 * scale, &mut rng, &mut capture, t);
    let s64 = qr::<f64>(lib, 20_000 * scale, &mut rng, &mut capture, t);
    t.notes.push(format!(
        "kernel orders for every length 1 to 300; {} one-row solves or inverses with a zero entry not compared (ITK-113)",
        s32 + s64
    ));
}

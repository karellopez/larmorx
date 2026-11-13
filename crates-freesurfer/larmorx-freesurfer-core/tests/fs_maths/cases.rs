// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.maths probe's cases and the port's side of them (shared by the example
//! `fs_maths_probe` and the test `fs_maths_official`). A case is one line: a command and its
//! inputs, every float and double as the hexadecimal of its bits; its output is one line in the
//! same form. `validation/probes/fs-maths/fsmaths.cpp` runs the same lines through FreeSurfer
//! 8.2.0's official code (the unit oracle's build); `check.py` compares the two.
//!
//! The cases come from a seeded generator of integers and exact divisions only (no
//! transcendental function), so they are the same on every platform. They stay inside what
//! FreeSurfer's code addresses within its arrays, except where FreeSurfer's own searches read
//! past them (FS-22): the port notes those, and their lines are compared as `SKIP`.

// The minimizers' problems keep the C expressions (`s = s + ...`) and its 1-based loops.
#![allow(dead_code, clippy::needless_range_loop, clippy::assign_op_pattern)]

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::affine::{self, AffineMatrix, AffineVector};
use larmorx_freesurfer_core::dmatrix::{self, DMatrix};
use larmorx_freesurfer_core::fs_vnl::FsFunctions;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{self as h, Histogram};
use larmorx_freesurfer_core::matrix::{self as m, MATRIX_COMPLEX, MATRIX_REAL, Matrix};
use larmorx_freesurfer_core::matrix_decomp as md;
use larmorx_freesurfer_core::matrix_io as mio;
use larmorx_freesurfer_core::matrix_ops::{self as mo, Region};
use larmorx_freesurfer_core::numerics::{self as nu, NrMatrix, NrVector};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::{pdf, prime};

/// splitmix64.
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }

    /// `k / d` for `k` in `[lo d, hi d]`, in `float`.
    fn f(&mut self, lo: i64, hi: i64, d: i64) -> f32 {
        self.int(lo * d, hi * d) as f32 / d as f32
    }

    fn d(&mut self, lo: i64, hi: i64, d: i64) -> f64 {
        self.int(lo * d, hi * d) as f64 / d as f64
    }
}

/// A generated histogram: what the line carries.
#[derive(Clone)]
struct Gh {
    bin_size: f32,
    min: f32,
    max: f32,
    bins: Vec<f32>,
    counts: Vec<f32>,
}

impl Gh {
    fn n(&self) -> i64 {
        self.bins.len() as i64
    }
}

fn hist(g: &mut Gen, nbins: usize, nonneg: bool) -> Gh {
    let mut bins = Vec::with_capacity(nbins);
    let bin_size;
    match g.below(3) {
        0 => {
            bin_size = 1.0f32;
            let mn = g.int(-20, 260) as f32;
            for b in 0..nbins {
                bins.push(mn + b as f32);
            }
        }
        1 => {
            bin_size = g.int(1, 16) as f32 / 4.0;
            let mn = g.int(-400, 400) as f32 / 8.0;
            for b in 0..nbins {
                bins.push(mn + bin_size * b as f32);
            }
        }
        _ => {
            bin_size = g.int(1, 100) as f32 / 10.0;
            let mut v = g.int(-100, 100) as f32 / 4.0;
            for _ in 0..nbins {
                bins.push(v);
                v += g.int(1, 40) as f32 / 10.0;
            }
        }
    }
    let mut counts = vec![0.0f32; nbins];
    let kind = if nonneg {
        g.pick(&[0, 1, 2, 5])
    } else {
        g.below(6)
    };
    match kind {
        0 => {
            for _ in 0..g.int(1, 4) {
                let b = g.below(nbins as u64) as usize;
                counts[b] = g.int(1, 1000) as f32;
            }
        }
        1 => {
            for _ in 0..g.int(1, 2) {
                let c0 = g.int(0, nbins as i64 - 1);
                let a = g.int(10, 5000);
                let s = g.int(1, 50);
                for (b, c) in counts.iter_mut().enumerate() {
                    let d = b as i64 - c0;
                    let v = a - d * d * s;
                    if v > 0 {
                        *c += v as f32;
                    }
                }
            }
        }
        2 => {
            for c in counts.iter_mut() {
                *c = g.int(0, 100_000) as f32 / 100.0;
            }
        }
        3 => {}
        4 => {
            for c in counts.iter_mut() {
                *c = g.int(-50, 100) as f32;
            }
        }
        _ => {
            let mut v = g.int(0, 20) as f32;
            for c in counts.iter_mut() {
                if g.below(3) == 0 {
                    v = g.int(0, 20) as f32;
                }
                *c = v;
            }
        }
    }
    let mn = bins[0];
    let mx = bins[nbins - 1];
    Gh {
        bin_size,
        min: mn,
        max: mx,
        bins,
        counts,
    }
}

fn nbins(g: &mut Gen) -> usize {
    g.pick(&[2, 3, 4, 5, 7, 8, 12, 16, 25, 32, 64, 100, 128, 256])
}

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn put_gh(s: &mut Vec<String>, h: &Gh) {
    s.push(h.n().to_string());
    s.push(fx(h.bin_size));
    s.push(fx(h.min));
    s.push(fx(h.max));
    s.extend(h.bins.iter().map(|&v| fx(v)));
    s.extend(h.counts.iter().map(|&v| fx(v)));
}

fn put_mat(s: &mut Vec<String>, g: &mut Gen, rows: i64, cols: i64, complex: bool) {
    s.push(rows.to_string());
    s.push(cols.to_string());
    let n = rows * cols * if complex { 2 } else { 1 };
    for _ in 0..n {
        let v = if g.below(8) == 0 {
            0.0
        } else {
            g.f(-1000, 1000, 64)
        };
        s.push(fx(v));
    }
}

/// A matrix's values of one kind (seeded integers and exact divisions only): ordinary, all zero,
/// tiny (around MatrixIsZero's 1e-11), or with an infinity or a NaN.
fn put_mat_kind(s: &mut Vec<String>, g: &mut Gen, rows: i64, cols: i64, complex: bool) {
    s.push(rows.to_string());
    s.push(cols.to_string());
    let n = rows * cols * if complex { 2 } else { 1 };
    let kind = g.below(10);
    for _ in 0..n {
        let v = match kind {
            0 => 0.0f32,
            1 => g.pick(&[0.0f32, 1e-12, -5e-12, 2e-11, -1e-10]),
            2 => g.pick(&[f32::INFINITY, f32::NAN, -f32::INFINITY, 1.0, -0.0]),
            _ => {
                if g.below(8) == 0 {
                    0.0
                } else {
                    g.f(-1000, 1000, 64)
                }
            }
        };
        s.push(fx(v));
    }
}

/// The float index (from `data[0]`) of every float the region copies write in a `drows` x
/// `dcols` destination of type `dt`, or `None` when the C code would not write (an error first).
#[allow(clippy::too_many_arguments)]
fn region_writes(
    srows: i64,
    scols: i64,
    drows: i64,
    dcols: i64,
    dt: i64,
    (sr, sc, rows, cols, dr, dc): (i64, i64, i64, i64, i64, i64),
) -> Option<Vec<i64>> {
    if dc < 1 || dr < 1 {
        return None;
    }
    let (er, ec) = (sr + rows - 1, sc + cols - 1);
    if sr < 1 || sr > srows || sc < 1 || sc > scols || er > srows || ec > scols {
        return None;
    }
    if rows > drows || cols > dcols {
        return None;
    }
    let mut w = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            let (r, c) = (dr + i, dc + j);
            if dt == 1 {
                w.push((r - 1) * dcols + c - 1);
            } else {
                let b = 2 * (r - 1) * dcols + 2 * c - 2;
                w.push(b);
                w.push(b + 1);
            }
        }
    }
    Some(w)
}

const COMMANDS: &[&str] = &[
    "smooth",
    "smoothip",
    "smoothinto",
    "lastpeak",
    "firstpeak",
    "valley",
    "lastpeakreg",
    "highest",
    "addsample",
    "prevvalley",
    "nextvalley",
    "nextpeak",
    "prevpeak",
    "startpeak",
    "endpeak",
    "currentpeak",
    "fillholes",
    "totalreg",
    "clearzero",
    "findbin",
    "correlate",
    "linfit",
    "linscale",
    "threshsum",
    "makepdf",
    "bins",
    "count",
    "init",
    "initgiven",
    "median",
    "cdf",
    "robust",
    "abs",
    "entropy",
    "soap",
    "fwhm",
    "sumnorm",
    "cumsum",
    "clearbins",
    "addsub",
    "clearcounts",
    "copy",
    "plot",
    "mmd",
    "mmdc",
    "affine",
    "dmat",
    "dmmul",
    "dliststats",
    "mmf",
    "melt",
    "mcplx",
    "mregion",
    "mr2c",
    "mrot",
    "mvec",
    "v3angle",
    "mgauss",
    "msim",
    "mprint",
    "primes",
    "pdfg",
    "minv",
    "mqr",
    "meig",
    "msvd",
    "msvdinv",
    "mpinv",
    "mrdtxt",
    "mrdasc",
    "mrdbin",
    "mwr",
    "powell",
    "powell2",
    "dfpmin",
];

/// The minimizers' problems (`fsmaths.cpp`'s `Problem`, the same float expressions): 0 a
/// quadratic, 1 Rosenbrock's, 2 the quadratic rounded down to a multiple of `1/q` (its gradient
/// the quadratic's), 3 weighted absolute values (bounded below; every problem is, so no
/// minimizer runs into infinities). Every call is hashed (FNV-1a over the bits) and counted.
struct Problem {
    kind: i32,
    n: usize,
    f0: f32,
    b: f32,
    q: f32,
    a: Vec<f32>,
    c: Vec<f32>,
    iter_set: bool,
    step_set: bool,
    user_set: bool,
    hash: u64,
    nf: i32,
    ng: i32,
    ni: i32,
    ns: i32,
    nu: i32,
}

impl Problem {
    fn fnv(&mut self, v: u32) {
        for k in 0..4 {
            self.hash ^= u64::from((v >> (8 * k)) & 0xff);
            self.hash = self.hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    fn hashp(&mut self, p: &[f32], from: usize) {
        for i in from..=self.n {
            self.fnv(p[i].to_bits());
        }
    }
}

impl FsFunctions for Problem {
    fn function(&mut self, p: &[f32]) -> f32 {
        let n = self.n;
        let mut s = self.f0;
        if self.kind == 1 {
            for i in 1..n {
                let t = p[i + 1] - p[i] * p[i];
                let u = 1.0f32 - p[i];
                s = s + 100.0f32 * t * t + u * u;
            }
        } else {
            for i in 1..=n {
                let d = p[i] - self.c[i];
                if self.kind == 3 {
                    s = s + self.a[i] * d.abs();
                } else {
                    s = s + self.a[i] * d * d;
                }
            }
            if self.kind != 3 {
                for i in 1..n {
                    s = s + self.b * ((p[i] - self.c[i]) * (p[i + 1] - self.c[i + 1]));
                }
            }
            if self.kind == 2 {
                s = (s * self.q).floor() / self.q;
            }
        }
        self.nf += 1;
        self.fnv(1);
        self.hashp(p, 1);
        self.fnv(s.to_bits());
        s
    }
    fn gradient(&mut self, p: &[f32], g: &mut [f32]) {
        let n = self.n;
        for gi in g.iter_mut().take(n + 1).skip(1) {
            *gi = 0.0;
        }
        if self.kind == 1 {
            for i in 1..n {
                let t = p[i + 1] - p[i] * p[i];
                let u = 1.0f32 - p[i];
                g[i] = g[i] + (-400.0f32 * t * p[i] - 2.0f32 * u);
                g[i + 1] = g[i + 1] + 200.0f32 * t;
            }
        } else {
            for i in 1..=n {
                let d = p[i] - self.c[i];
                let mut gi = if self.kind == 3 {
                    self.a[i]
                        * if d > 0.0 {
                            1.0f32
                        } else if d < 0.0 {
                            -1.0
                        } else {
                            0.0
                        }
                } else {
                    2.0f32 * self.a[i] * d
                };
                if i > 1 && self.kind != 3 {
                    gi = gi + self.b * (p[i - 1] - self.c[i - 1]);
                }
                if i < n && self.kind != 3 {
                    gi = gi + self.b * (p[i + 1] - self.c[i + 1]);
                }
                g[i] = gi;
            }
        }
        self.ng += 1;
        self.fnv(2);
        self.hashp(p, 1);
        self.hashp(g, 1);
    }
    fn powell_iteration(&mut self, p: &[f32], nparms: i32) {
        if !self.iter_set {
            return;
        }
        self.ni += 1;
        self.fnv(3);
        self.fnv(nparms as u32);
        self.hashp(p, 0);
    }
    fn step(&mut self, itno: i32, sse: f32, p: &[f32]) {
        if !self.step_set {
            return;
        }
        self.ns += 1;
        self.fnv(4);
        self.fnv(itno as u32);
        self.fnv(sse.to_bits());
        self.hashp(p, 1);
    }
    fn user_callback(&mut self, p: &[f32]) {
        if !self.user_set {
            return;
        }
        self.nu += 1;
        self.fnv(5);
        self.hashp(p, 1);
    }
}

/// A problem's tokens: kind n f0 b q a[1..n] c[1..n] (n at most 12; a positive definite
/// quadratic: |b| < the smallest a / 2).
fn put_problem(s: &mut Vec<String>, g: &mut Gen, kind: i64, n: i64) {
    s.push(kind.to_string());
    s.push(n.to_string());
    let f0 = g.int(0, 4000) as f32 / 4.0;
    s.push(fx(g.pick(&[0.0f32, 1.0, f0])));
    s.push(fx(g.int(-6, 6) as f32 / 32.0));
    s.push(fx(g.pick(&[1.0f32, 4.0, 64.0, 4096.0])));
    for _ in 0..n {
        s.push(fx(g.int(4, 32) as f32 / 8.0));
    }
    for _ in 0..n {
        s.push(fx(g.int(-32, 32) as f32 / 8.0));
    }
}

/// A minimizer case: the problem, the starting point (never the quadratics' centre on its first
/// coordinate: a zero gradient makes L-BFGS divide by 0), the directions for Powell.
fn minimizer_case(g: &mut Gen, cmd: &str) -> String {
    let kind = g.pick(&[0i64, 0, 1, 2, 2, 3]);
    let n = match cmd {
        "dfpmin" => g.pick(&[2i64, 3, 4, 6, 12]),
        _ => g.pick(&[1i64, 2, 3, 4, 6]),
    };
    let n = if kind == 1 { n.max(2) } else { n };
    let mut s = vec![cmd.to_owned()];
    put_problem(&mut s, g, kind, n);
    let c1 = f32::from_bits(u32::from_str_radix(&s[6 + n as usize], 16).unwrap());
    for i in 0..n {
        let mut v = g.int(-64, 64) as f32 / 8.0;
        if i == 0 && v == c1 {
            v += 0.5;
        }
        s.push(fx(v));
    }
    match cmd {
        "dfpmin" => {
            s.push(fx(g.pick(&[1e-2f32, 1e-4, 1e-6])));
            s.push(g.pick(&[0, 0, 0, 1, 2]).to_string());
            s.push(if g.below(8) == 0 { "1" } else { "0" }.to_owned());
            s.push(g.below(2).to_string());
            s.push(g.below(2).to_string());
        }
        _ => {
            let dkind = g.below(4);
            for r in 0..n {
                for k in 0..n {
                    let v = match dkind {
                        0 | 1 => {
                            if r == k {
                                1.0f32
                            } else {
                                0.0
                            }
                        }
                        2 => {
                            if r == k {
                                g.int(1, 4) as f32 / 2.0
                            } else {
                                0.0
                            }
                        }
                        _ => {
                            if r == k {
                                1.0
                            } else if r > k {
                                g.int(-4, 4) as f32 / 8.0
                            } else {
                                0.0
                            }
                        }
                    };
                    s.push(fx(v));
                }
            }
            if cmd == "powell" {
                s.push(fx(g.pick(&[1e-4f32, 1e-2, 1e-6])));
            } else {
                s.push(fx(g.pick(&[1e-1f32, 1e-2, 1e-3, 1e-4, 1e-6, 1e-8])));
                s.push(fx(g.pick(&[1e-1f32, 1e-2, 1e-3])));
                s.push(g.pick(&[0, 1, 2, 4, 36, 36]).to_string());
                s.push(if g.below(6) == 0 { "1" } else { "0" }.to_owned());
            }
            s.push(g.below(2).to_string());
        }
    }
    s.join(" ")
}

/// The special doubles of the double matrices' cases: infinities, quiet NaNs of both signs
/// with and without a payload, a signalling NaN.
const SPECIAL64: [f64; 6] = [
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::from_bits(0x7ff8_0000_0000_0000),
    f64::from_bits(0xfff8_0000_0000_0005),
    f64::from_bits(0x7ff8_0000_0000_0123),
    f64::from_bits(0x7ff0_0000_0000_0042),
];

/// One case line of command `cmd`.
/// A number as text, in one of the forms FreeSurfer's files hold or a reader may meet: integers,
/// fixed and exponent forms, `%+4.5f`'s, and (rarely) the C library's special and edge forms.
fn num_token(g: &mut Gen) -> String {
    match g.below(40) {
        0 => g
            .pick(&[
                "inf",
                "-inf",
                "nan",
                "-nan",
                "INFINITY",
                "1e40",
                "-1e40",
                "1e-40",
                "1e-50",
                "0x1p-149",
                "0x1.8p3",
                "1e",
                "100ergs",
                "5.",
                ".5",
                "+.5",
                "1.17549435e-38",
                "3.40282357e38",
                "0x.",
                "1e+",
            ])
            .to_owned(),
        1 => g
            .pick(&["abc", ".", "-", "1,5", "nan(1)", "infin", "0x", "e5"])
            .to_owned(),
        k => {
            let v = g.f(-1000, 1000, 64);
            match k % 4 {
                0 => g.int(-1000, 1000).to_string(),
                1 => format!("{v:.3}"),
                2 => format!("{v:+.5}"),
                _ => format!("{v:e}"),
            }
        }
    }
}

fn hex_text(t: &[u8]) -> String {
    if t.is_empty() {
        return "-".to_owned();
    }
    t.iter().map(|b| format!("{b:02x}")).collect()
}

/// A matrix text file: rows of numbers with varied separators and line ends.
fn matrix_text(g: &mut Gen, rows: i64, cols: i64) -> String {
    let mut text = String::new();
    for _ in 0..rows {
        for c in 0..cols {
            if c > 0 || g.below(6) == 0 {
                text.push_str(g.pick(&[" ", "  ", "\t", " \t "]));
            }
            text.push_str(&num_token(g));
        }
        text.push_str(g.pick(&["\n", "\n", "\r\n", " \n", "\n\n"]));
    }
    text
}

fn case(g: &mut Gen, cmd: &str) -> String {
    let mut s = vec![cmd.to_owned()];
    let nb = nbins(g);
    let wsize = g.pick(&[1, 3, 5, 7, 9]);
    let pct = g.int(0, 10) as f32 / 20.0;
    match cmd {
        "smooth" | "smoothip" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            let r = g.int(1, 40) as f32 / 4.0;
            let sigma = g.pick(&[0.5f32, 1.0, 2.0, 3.25, r]);
            s.push(fx(sigma));
        }
        "smoothinto" => {
            let h = hist(g, nb, false);
            let nd = nbins(g);
            let d = hist(g, nd, false);
            put_gh(&mut s, &h);
            put_gh(&mut s, &d);
            s.push(fx(g.int(1, 20) as f32 / 4.0));
        }
        "lastpeak" | "firstpeak" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(wsize.to_string());
            s.push(fx(pct));
        }
        "valley" | "lastpeakreg" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            s.push(wsize.to_string());
            if cmd == "lastpeakreg" {
                s.push(fx(pct));
            }
            let lo = h.min.floor() as i64 - 5;
            let hi = h.max.ceil() as i64 + 5;
            s.push(g.int(lo, hi).to_string());
            s.push(g.int(lo, hi).to_string());
        }
        "highest" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(-3, nb as i64 + 3).to_string());
            s.push(g.int(-3, nb as i64 + 3).to_string());
        }
        "addsample" => {
            let h = hist(g, nb, true);
            put_gh(&mut s, &h);
            let n = g.int(1, 12);
            s.push(n.to_string());
            for _ in 0..n {
                let v = if g.below(3) == 0 {
                    h.bins[g.below(nb as u64) as usize]
                } else {
                    let span = (h.max - h.min).abs() + 4.0 * h.bin_size;
                    h.min - 2.0 * h.bin_size + span * (g.int(0, 1000) as f32 / 1000.0)
                };
                s.push(fx(v));
            }
            if g.below(2) == 0 {
                s.push(fx(0.0));
                s.push(fx(0.0));
            } else {
                let a = g.f(-50, 50, 4);
                s.push(fx(a));
                s.push(fx(a + g.f(1, 300, 4)));
            }
        }
        "prevvalley" | "nextvalley" => {
            put_gh(&mut s, &hist(g, nb, false));
            let b0 = if g.below(10) == 0 {
                g.pick(&[-1, nb as i64])
            } else {
                g.int(0, nb as i64 - 1)
            };
            s.push(b0.to_string());
        }
        "nextpeak" | "prevpeak" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(-1, nb as i64).to_string());
            s.push(g.int(0, 4).to_string());
        }
        "startpeak" | "endpeak" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(0, nb as i64 - 1).to_string());
            s.push(fx(pct));
        }
        "currentpeak" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(0, nb as i64 - 1).to_string());
            s.push(wsize.to_string());
            s.push(fx(pct));
        }
        "fillholes" | "median" | "cdf" | "abs" | "entropy" | "sumnorm" | "cumsum"
        | "clearcounts" | "copy" | "plot" => {
            put_gh(&mut s, &hist(g, nb, false));
        }
        "totalreg" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(0, nb as i64 - 1).to_string());
            s.push(g.int(0, nb as i64 - 1).to_string());
        }
        "clearzero" => {
            let mut h = hist(g, nb, false);
            if h.bins[nb - 1] < 0.0 {
                let shift = -h.bins[nb - 1] + 1.0;
                for b in h.bins.iter_mut() {
                    *b += shift;
                }
            }
            put_gh(&mut s, &h);
        }
        "findbin" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            let v = if g.below(3) == 0 {
                h.counts[g.below(nb as u64) as usize]
            } else {
                h.min - 3.0 + (h.max - h.min + 6.0) * (g.int(0, 1000) as f32 / 1000.0)
            };
            s.push(fx(v));
        }
        "correlate" | "threshsum" | "addsub" => {
            let a = hist(g, nb, false);
            let mut b = hist(g, nb, false);
            if cmd == "correlate" && g.below(2) == 0 {
                b.bins = a.bins.clone();
                b.bin_size = a.bin_size;
            }
            put_gh(&mut s, &a);
            put_gh(&mut s, &b);
            if cmd == "threshsum" {
                s.push(fx(g.f(0, 50, 2)));
            }
        }
        "linfit" => {
            let nb = g.pick(&[4usize, 8, 16, 32]);
            let a = hist(g, nb, true);
            let b = hist(g, nb, true);
            put_gh(&mut s, &a);
            put_gh(&mut s, &b);
            let amin = g.d(8, 10, 10) / 10.0;
            s.push(dx(amin));
            s.push(dx(amin + g.d(1, 3, 10) / 10.0));
            let bmin = -g.d(0, 2, 2);
            s.push(dx(bmin));
            s.push(dx(bmin + g.d(0, 2, 2)));
        }
        "linscale" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(fx(g.f(-4, 4, 8)));
            s.push(fx(g.f(-100, 100, 4)));
        }
        "makepdf" => {
            put_gh(&mut s, &hist(g, nb, false));
        }
        "bins" | "init" => {
            s.push(nb.to_string());
            let mn = g.d(-500, 500, 4);
            s.push(dx(mn));
            let mx = if g.below(6) == 0 {
                mn - g.d(0, 10, 2)
            } else {
                mn + g.d(1, 1000, 8)
            };
            s.push(dx(mx));
        }
        "count" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            let n = g.int(1, 16);
            s.push(n.to_string());
            for _ in 0..n {
                let v = f64::from(h.min) - 5.0 + f64::from(h.max - h.min + 10.0) * g.d(0, 1, 1000);
                s.push(dx(v));
            }
        }
        "initgiven" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            s.push(g.int(2, nb as i64).to_string());
            let mn = g.d(-100, 100, 2);
            s.push(dx(mn));
            s.push(dx(mn + g.d(-10, 300, 2)));
        }
        "robust" => {
            let nb = g.pick(&[8usize, 16, 32, 64, 128, 256]);
            let mut h = hist(g, nb, true);
            if h.counts.iter().all(|&c| c <= 0.0) {
                h.counts[nb / 2] = 10.0;
            }
            put_gh(&mut s, &h);
            s.push(dx(g.pick(&[0.1, 0.25, 0.5])));
        }
        "soap" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(1, 20).to_string());
        }
        "fwhm" => {
            put_gh(&mut s, &hist(g, nb, false));
            s.push(g.int(0, nb as i64 - 1).to_string());
        }
        "clearbins" => {
            let h = hist(g, nb, false);
            put_gh(&mut s, &h);
            let lo = h.min.floor() as i64 - 2;
            let hi = h.max.ceil() as i64 + 2;
            s.push(g.int(lo, hi).to_string());
            s.push(g.int(lo, hi).to_string());
        }
        "mmd" => {
            let (r, k, c) = (g.int(1, 6), g.int(1, 6), g.int(1, 6));
            put_mat(&mut s, g, r, k, false);
            put_mat(&mut s, g, k, c, false);
        }
        "mmdc" => {
            let (t1, t2) = g.pick(&[(1, 2), (2, 1), (2, 2)]);
            s.push(t1.to_string());
            s.push(t2.to_string());
            let (r, k, c) = (g.int(1, 4), g.int(1, 4), g.int(1, 4));
            put_mat(&mut s, g, r, k, t1 == 2);
            put_mat(&mut s, g, k, c, t2 == 2);
        }
        "affine" => {
            put_mat(&mut s, g, 4, 4, false);
            put_mat(&mut s, g, 4, 4, false);
            for _ in 0..3 {
                s.push(fx(g.f(-300, 300, 16)));
            }
            s.push(fx(g.pick(&[1.0f32, 0.0, 0.5])));
        }
        "dmat" => {
            let (r, c) = (g.int(1, 5), g.int(1, 5));
            // a third of the cases with NaNs (of both signs, payloads, a signalling one) and
            // infinities among the numbers and the factors
            let special = g.below(3) == 0;
            for _ in 0..2 {
                s.push(r.to_string());
                s.push(c.to_string());
                for _ in 0..r * c {
                    let v = if special && g.below(4) == 0 {
                        g.pick(&SPECIAL64)
                    } else if g.below(6) == 0 {
                        g.pick(&[0.0, -0.0])
                    } else {
                        g.d(-1000, 1000, 1024)
                    };
                    s.push(dx(v));
                }
            }
            let r = g.d(-10, 10, 64);
            let sp = g.pick(&SPECIAL64);
            let v = if special && g.below(3) == 0 { sp } else { r };
            s.push(dx(g.pick(&[0.0, 1.0, -1.0, 2.5, v])));
            let r = g.d(-10, 10, 64);
            let sp = g.pick(&SPECIAL64);
            let v = if special && g.below(3) == 0 { sp } else { r };
            s.push(dx(g.pick(&[0.0, 1.0, -1.0, 0.25, v])));
        }
        "dmmul" => {
            let (r, k, c) = (g.int(1, 6), g.int(1, 6), g.int(1, 6));
            let n = g.int(1, 12);
            let special = g.below(3) == 0;
            for (rows, cols) in [(r, k), (k, c), (n, 1), (n, 1)] {
                s.push(rows.to_string());
                s.push(cols.to_string());
                for _ in 0..rows * cols {
                    let big = g.pick(&[1.0, 1e8, 1e-6, 3.0]);
                    let v = if special && g.below(4) == 0 {
                        g.pick(&SPECIAL64)
                    } else if special && g.below(4) == 0 {
                        0.0
                    } else {
                        g.d(-1000, 1000, 1 << 20) * big
                    };
                    s.push(dx(v));
                }
            }
        }
        "dliststats" => {
            let n = g.int(1, 40);
            s.push(n.to_string());
            let base = g.pick(&[0.0, 1e6, -250.0, 1e12]);
            let special = g.below(4) == 0;
            for _ in 0..n {
                if special && g.below(5) == 0 {
                    s.push(dx(g.pick(&SPECIAL64)));
                    continue;
                }
                s.push(dx(base + g.d(-100, 100, 1 << 16)));
            }
        }
        "mmf" => {
            let (t1, t2) = g.pick(&[(1, 1), (1, 1), (1, 2), (2, 1), (2, 2)]);
            s.push(t1.to_string());
            s.push(t2.to_string());
            let (r, k, c) = (g.int(1, 6), g.int(1, 6), g.int(1, 6));
            put_mat(&mut s, g, r, k, t1 == 2);
            put_mat(&mut s, g, k, c, t2 == 2);
        }
        "melt" => {
            let (r, c) = (g.int(1, 5), g.int(1, 5));
            let (r, c) = if g.below(3) == 0 { (r, r) } else { (r, c) };
            put_mat_kind(&mut s, g, r, c, false);
            put_mat(&mut s, g, r, c, false);
            let x = g.f(-10, 10, 64);
            s.push(fx(g.pick(&[0.0f32, 1.0, -1.0, 0.5, x])));
            s.push(g.int(0, r.min(c)).to_string());
            let k1 = g.int(1, 2 * r * c + 2);
            s.push(k1.to_string());
            s.push(g.int(1, 6).to_string());
        }
        "mcplx" => {
            let (r, c) = (g.int(1, 4), g.int(1, 4));
            put_mat(&mut s, g, r, c, true);
            let x = g.f(-10, 10, 64);
            s.push(fx(g.pick(&[0.0f32, 1.0, -2.0, x])));
        }
        "mregion" => loop {
            let mut t = vec![cmd.to_owned()];
            let which = g.int(0, 2);
            let st = g.int(1, 2);
            let (srows, scols) = (g.int(1, 5), g.int(1, 5));
            let given = g.below(3) != 0;
            let dt = g.int(1, 2);
            let (drows, dcols) = (g.int(1, 6), g.int(1, 6));
            let rows = g.int(0, srows);
            let cols = g.int(0, scols);
            let sr = g.int(0, srows + 1);
            let sc = g.int(0, scols + 1);
            let (dr, dc) = if given {
                (g.int(0, 4), g.int(0, 4))
            } else if g.below(10) == 0 {
                (1, 0)
            } else {
                (1, 1)
            };
            let (gr, gc, gt) = if given {
                (drows, dcols, dt)
            } else {
                (rows, cols, st)
            };
            let n = gr * gc * if gt == 2 { 2 } else { 1 };
            if let Some(w) = region_writes(srows, scols, gr, gc, gt, (sr, sc, rows, cols, dr, dc))
                && w.iter().any(|&i| i < 0 || i >= n)
            {
                continue;
            }
            t.push(which.to_string());
            t.push(st.to_string());
            put_mat(&mut t, g, srows, scols, st == 2);
            t.push(i64::from(given).to_string());
            if given {
                t.push(dt.to_string());
                put_mat(&mut t, g, drows, dcols, dt == 2);
            }
            for v in [sr, sc, rows, cols, dr, dc] {
                t.push(v.to_string());
            }
            return t.join(" ");
        },
        "mr2c" => {
            let (r, c) = (g.int(1, 5), g.int(1, 5));
            put_mat(&mut s, g, r, c, false);
            put_mat(&mut s, g, r, c, false);
            put_mat(&mut s, g, r, c, true);
        }
        "mrot" => {
            let n = g.int(3, 5);
            s.push(n.to_string());
            s.push(fx(g.f(-7, 7, 4096)));
            s.push(g.int(0, 3).to_string());
            put_mat(&mut s, g, n, n, false);
        }
        "mvec" => {
            let n = g.int(3, 6);
            put_mat(&mut s, g, n, 1, false);
            let n2 = g.int(n, 6);
            put_mat(&mut s, g, n2, 1, false);
            let n3 = g.int(3, 6);
            put_mat(&mut s, g, n3, 1, false);
            let x = g.f(0, 2000, 64);
            s.push(fx(g.pick(&[0.0f32, 1.0, x])));
        }
        "v3angle" => {
            let n = g.int(3, 5);
            let near = g.below(3) == 0;
            s.push(n.to_string());
            s.push("1".to_owned());
            let a: Vec<f32> = (0..n).map(|_| g.f(-100, 100, 64)).collect();
            for &v in &a {
                s.push(fx(v));
            }
            s.push(n.to_string());
            s.push("1".to_owned());
            for &v in &a {
                // a nearly parallel vector, or another one
                let w = if near {
                    v + g.f(-1, 1, 64) / 64.0
                } else {
                    g.f(-100, 100, 64)
                };
                s.push(fx(w));
            }
        }
        "mgauss" => {
            s.push(g.int(0, 12).to_string());
            s.push(fx(g.f(0, 4, 8)));
            s.push(fx(g.f(0, 8, 8)));
            s.push(fx(g.f(0, 1, 16)));
            s.push(g.int(0, 2).to_string());
        }
        "msim" => {
            let (n, k) = (g.int(1, 5), g.int(1, 5));
            put_mat(&mut s, g, n, n, false);
            put_mat(&mut s, g, k, n, false);
        }
        "mprint" => {
            let t = if g.below(4) == 0 { 2 } else { 1 };
            s.push(t.to_string());
            let (r, c) = (g.int(0, 4), g.int(0, 4));
            put_mat_kind(&mut s, g, r, c, t == 2);
            s.push(g.int(0, 8).to_string());
        }
        "primes" => {
            s.push(g.int(1, 3000).to_string());
            let n = match g.below(4) {
                0 => g.pick(&[
                    1, 2, 3, 4, 6, 8, 97, 1024, 163842, 40962, 10242, 65536, 99991,
                ]),
                1 => g.int(1, 64),
                _ => g.int(1, 400_000),
            };
            s.push(n.to_string());
            s.push(g.int(0, 120).to_string());
        }
        "pdfg" => {
            s.push(g.int(-2_000_000_000, 2_000_000_000).to_string());
            s.push(g.int(1, 12).to_string());
        }
        "minv" => {
            let t = if g.below(4) == 0 { 2 } else { 1 };
            let n = if t == 2 { g.int(1, 3) } else { g.int(1, 7) };
            let mut m = vec![cmd.to_owned(), t.to_string()];
            put_mat(&mut m, g, n, n, t == 2);
            // a complex matrix must be invertible: FreeSurfer crashes on a singular one (FS-33),
            // so no zero element and no repeated row; a real one is sometimes singular (the first
            // row repeated)
            if t == 2 {
                for v in m.iter_mut().skip(4) {
                    if v == "00000000" {
                        *v = fx(1.0);
                    }
                }
                // a diagonally dominant real part keeps [A -C; C A] well conditioned
                for i in 0..n as usize {
                    m[4 + 2 * (i * n as usize + i)] = fx(4096.0 + i as f32);
                }
            }
            if t == 1 && g.below(6) == 0 && n > 1 {
                let w = (if t == 2 { 2 } else { 1 } * n) as usize;
                for j in 0..w {
                    m[4 + w + j] = m[4 + j].clone();
                }
            }
            return m.join(" ");
        }
        "mqr" => {
            let (r, c) = (g.int(1, 6), g.int(1, 6));
            put_mat(&mut s, g, r, c, false);
        }
        "meig" => {
            // 0, 1 symmetric; 2 symmetric but one element (FreeSurfer's exact test: the
            // non-symmetric path); 3 any; 4 symmetric with infinities and NaNs (only the helper's
            // symmetric path: a NaN fails the symmetry test, and the non-symmetric path never
            // returns on such a matrix, ITK-112); 5 upper triangular (real eigenvalues)
            let n = g.int(1, 6) as usize;
            let kind = g.below(6);
            let cols = if g.below(10) == 0 { n + 1 } else { n };
            let mut a = vec![0.0f32; n * cols];
            for v in a.iter_mut() {
                *v = if g.below(8) == 0 {
                    0.0
                } else {
                    g.f(-1000, 1000, 64)
                };
            }
            if kind == 4 {
                for v in a.iter_mut() {
                    if g.below(3) == 0 {
                        *v = g.pick(&[f32::INFINITY, -f32::INFINITY, f32::NAN, 1.0, -0.0]);
                    }
                }
            }
            if cols == n {
                for i in 0..n {
                    for j in 0..i {
                        match kind {
                            3 => {}
                            5 => a[i * n + j] = 0.0,
                            _ => a[i * n + j] = a[j * n + i],
                        }
                    }
                }
                if kind == 2 && n > 1 {
                    a[n - 1] += 1.0 / 64.0;
                }
            }
            s.push(i64::from(kind != 4).to_string());
            s.push(n.to_string());
            s.push(cols.to_string());
            s.extend(a.iter().map(|&v| fx(v)));
        }
        "msvd" => {
            let (r, c) = (g.int(1, 6), g.int(1, 6));
            put_mat_kind(&mut s, g, r, c, false);
        }
        "msvdinv" | "mpinv" => {
            if cmd == "mpinv" {
                s.push(g.int(0, 4).to_string());
            }
            let (r, c) = if g.below(3) != 0 {
                let n = g.int(1, 5);
                (n, n)
            } else {
                (g.int(1, 5), g.int(1, 5))
            };
            let at = s.len();
            put_mat_kind(&mut s, g, r, c, false);
            // sometimes singular: the second row a copy of the first
            if r > 1 && g.below(4) == 0 {
                for j in 0..c as usize {
                    s[at + 2 + c as usize + j] = s[at + 2 + j].clone();
                }
            }
        }
        "mrdtxt" => {
            let (r, c) = (g.int(1, 5), g.int(1, 5));
            let mut text = matrix_text(g, r, c);
            match g.below(10) {
                0 => text = String::new(),
                1 => text = format!("  \t\n{text}"),
                2 => {
                    while text.ends_with('\n') {
                        text.pop();
                    }
                }
                3 => {
                    // a first line longer than fgets' 998 bytes
                    let long: String = (0..250).map(|i| format!("{} ", i % 10)).collect();
                    text = format!("{long}\n{text}");
                }
                _ => {}
            }
            s.push(hex_text(text.as_bytes()));
        }
        "mrdasc" => {
            let t = g.pick(&[1, 1, 1, 2, 2, 3]);
            let (r, c) = (g.int(0, 4), g.int(0, 4));
            let given = i64::from(g.below(3) == 0);
            let (gt, gr, gc) = if g.below(4) == 0 {
                (g.pick(&[1, 2]), g.int(0, 4), g.int(0, 4))
            } else {
                (t, r, c)
            };
            let mut text = String::new();
            for _ in 0..g.below(3) {
                text.push_str(g.pick(&["# a comment\n", "\n", "   \n", "  # x 1 2\n"]));
            }
            text.push_str(&match g.below(12) {
                0 => "1 2\n".to_owned(),
                1 => "x 1 1\n".to_owned(),
                2 => format!("{t} {r} {c} 9 # trailing\n"),
                _ => format!("{t}{}{r} {c}\n", g.pick(&[" ", "\t", "  "])),
            });
            let per = if t == 2 { 2 } else { 1 };
            text.push_str(&matrix_text(g, r, c * per));
            if g.below(8) == 0 {
                // the file ends early
                let cut = g.below(text.len() as u64 + 1) as usize;
                text.truncate(cut);
            }
            s.extend([given, gt, gr, gc].iter().map(i64::to_string));
            s.push(hex_text(text.as_bytes()));
        }
        "mrdbin" => {
            let t = g.pick(&[1, 1, 1, 2, 2, 3]);
            let (r, c) = (g.int(0, 4), g.int(0, 4));
            let given = i64::from(g.below(3) == 0);
            let (gt, gr, gc) = if g.below(4) == 0 {
                (g.pick(&[1, 2]), g.int(0, 4), g.int(0, 4))
            } else {
                (t, r, c)
            };
            let mut b = Vec::new();
            for v in [t, r, c] {
                b.extend_from_slice(&(v as i32).to_be_bytes());
            }
            let per = if t == 2 { 2 } else { 1 };
            for _ in 0..r * c * per {
                let v = if g.below(10) == 0 {
                    g.pick(&[
                        f64::INFINITY,
                        f64::NAN,
                        1e300,
                        1e-300,
                        -0.0,
                        3.4028235677973366e38,
                    ])
                } else {
                    g.d(-1000, 1000, 64)
                };
                b.extend_from_slice(&v.to_be_bytes());
            }
            if g.below(8) == 0 {
                let cut = g.below(b.len() as u64 + 1) as usize;
                b.truncate(cut);
            }
            s.extend([given, gt, gr, gc].iter().map(i64::to_string));
            s.push(hex_text(&b));
        }
        "mwr" => {
            let t = if g.below(4) == 0 { 2 } else { 1 };
            s.push(t.to_string());
            let (r, c) = (g.int(0, 4), g.int(0, 4));
            put_mat_kind(&mut s, g, r, c, t == 2);
        }
        "powell" | "powell2" | "dfpmin" => return minimizer_case(g, cmd),
        _ => unreachable!("{cmd}"),
    }
    s.join(" ")
}

/// `n` cases from `seed`, every command in turn.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    (0..n)
        .map(|i| {
            let cmd = COMMANDS[i % COMMANDS.len()];
            case(&mut g, cmd)
        })
        .collect()
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let v = self.t[self.i];
        self.i += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.next().parse().unwrap()
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).unwrap())
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.next(), 16).unwrap())
    }
    fn h(&mut self) -> Histogram {
        let n = self.i();
        let mut gl = Globals::new(UpstreamDefects::Fix);
        let mut h = h::histo_alloc(&mut gl, &mut std::io::sink(), n).unwrap();
        h.bin_size = self.f();
        h.min = self.f();
        h.max = self.f();
        for b in 0..n as usize {
            h.bins[b] = self.f();
        }
        for b in 0..n as usize {
            h.counts[b] = self.f();
        }
        h
    }
    fn m(&mut self, gl: &mut Globals, mtype: i16) -> Matrix {
        let (rows, cols) = (self.i(), self.i());
        let mut mm = m::matrix_alloc(
            gl,
            &mut std::io::sink(),
            &mut std::io::sink(),
            rows,
            cols,
            mtype,
        )
        .unwrap()
        .unwrap();
        for v in mm.data_mut().iter_mut() {
            *v = self.f();
        }
        mm
    }
    fn problem(&mut self) -> Problem {
        let kind = self.i();
        let n = self.i() as usize;
        let (f0, b, q) = (self.f(), self.f(), self.f());
        let mut a = vec![0.0f32; n + 1];
        let mut c = vec![0.0f32; n + 1];
        for v in a.iter_mut().skip(1) {
            *v = self.f();
        }
        for v in c.iter_mut().skip(1) {
            *v = self.f();
        }
        Problem {
            kind,
            n,
            f0,
            b,
            q,
            a,
            c,
            iter_set: false,
            step_set: false,
            user_set: false,
            hash: 0xcbf2_9ce4_8422_2325,
            nf: 0,
            ng: 0,
            ni: 0,
            ns: 0,
            nu: 0,
        }
    }
    fn dm(&mut self) -> DMatrix {
        let (rows, cols) = (self.i(), self.i());
        let mut d = DMatrix::zeros(rows, cols);
        for v in d.data_mut().iter_mut() {
            *v = self.d();
        }
        d
    }
}

#[derive(Default)]
struct Out(Vec<String>);

impl Out {
    fn i(&mut self, v: i32) {
        self.0.push(v.to_string());
    }
    fn f(&mut self, v: f32) {
        self.0.push(fx(v));
    }
    fn d(&mut self, v: f64) {
        self.0.push(dx(v));
    }
    fn h(&mut self, h: &Histogram) {
        self.i(h.nbins);
        self.i(h.max_bins);
        self.f(h.bin_size);
        self.f(h.min);
        self.f(h.max);
        let n = h.nbins.max(0) as usize;
        for &b in &h.bins[..n] {
            self.f(b);
        }
        for &c in &h.counts[..n] {
            self.f(c);
        }
    }
    fn mraw(&mut self, mm: &Matrix) {
        self.i(mm.rows);
        self.i(mm.cols);
        let n = mm.nfloats() + 2;
        for &v in &mm.store()[..n] {
            self.f(v);
        }
    }
    fn mn(&mut self, r: &Ret<Matrix>, given: Option<&Matrix>) {
        match (r, given) {
            (Ret::New(mm), _) => self.mraw(mm),
            (Ret::Given, Some(mm)) => self.mraw(mm),
            _ => self.0.push("NULL".to_owned()),
        }
    }
    fn mt(&mut self, gl: &mut Globals, t: &mut Toks) -> Matrix {
        let ty = t.i() as i16;
        t.m(gl, ty)
    }
    fn dm(&mut self, d: Option<&DMatrix>) {
        match d {
            None => self.0.push("NULL".to_owned()),
            Some(d) => {
                self.i(d.rows);
                self.i(d.cols);
                for &v in d.data() {
                    self.d(v);
                }
            }
        }
    }
    fn counts(&mut self, pr: &Problem) {
        for v in [pr.nf, pr.ng, pr.ni, pr.ns, pr.nu] {
            self.i(v);
        }
        self.0.push(format!("{:016x}", pr.hash));
    }
    fn cap(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            self.0.push("-".to_owned());
        } else {
            self.0
                .push(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        }
    }
    fn file(&mut self, p: &Path) {
        match std::fs::read(p) {
            Ok(bytes) => {
                let mut s = String::with_capacity(2 * bytes.len());
                for b in bytes {
                    s.push_str(&format!("{b:02x}"));
                }
                self.0.push(s);
            }
            Err(_) => self.0.push("NOFILE".to_owned()),
        }
    }
}

fn new_h(r: Ret<Histogram>) -> Histogram {
    r.unwrap_new()
}

/// The port's output line for `line` in `mode`, and the IDs of the defects it noted. `tmp`: a
/// scratch file for the commands that write files.
/// Whether FreeSurfer's code crashes on this case (`reproduce` notes it, and the probe's line is
/// `CRASH`): FS-44, FS-45, FS-46.
fn crashed(g: &Globals, mode: UpstreamDefects) -> bool {
    mode == UpstreamDefects::Reproduce
        && g.notices.as_slice().iter().any(|x| {
            matches!(x.defect.id, "FS-44" | "FS-45" | "FS-46") && x.detail.contains("crashes")
        })
}

pub fn run_line(line: &str, mode: UpstreamDefects, tmp: &Path) -> (String, Vec<&'static str>) {
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = Out::default();
    let mut gl = Globals::new(mode);
    let g = &mut gl;
    let (mut so, mut se) = (std::io::sink(), std::io::sink());
    let cmd = t.next();
    match cmd {
        "smooth" => {
            let hh = t.h();
            let s = t.f();
            o.h(&new_h(h::histo_smooth(g, &hh, None, s)));
        }
        "smoothip" => {
            let mut hh = t.h();
            let s = t.f();
            h::histo_smooth_in_place(g, &mut hh, s);
            o.h(&hh);
        }
        "smoothinto" => {
            let hh = t.h();
            let mut d = t.h();
            let s = t.f();
            h::histo_smooth(g, &hh, Some(&mut d), s);
            o.h(&d);
        }
        "lastpeak" => {
            let hh = t.h();
            let w = t.i();
            o.i(h::histo_find_last_peak(&hh, w, t.f()));
        }
        "firstpeak" => {
            let hh = t.h();
            let w = t.i();
            o.i(h::histo_find_first_peak(&hh, w, t.f()));
        }
        "valley" => {
            let hh = t.h();
            let (w, i0, i1) = (t.i(), t.i(), t.i());
            o.i(h::histo_find_valley(g, &hh, w, i0, i1));
        }
        "lastpeakreg" => {
            let hh = t.h();
            let w = t.i();
            let p = t.f();
            let (i0, i1) = (t.i(), t.i());
            o.i(h::histo_find_last_peak_in_region(&hh, w, p, i0, i1));
        }
        "highest" => {
            let hh = t.h();
            let (b0, b1) = (t.i(), t.i());
            o.i(h::histo_find_highest_peak_in_region(&hh, b0, b1));
        }
        "addsample" => {
            let mut hh = t.h();
            let n = t.i();
            let v: Vec<f32> = (0..n).map(|_| t.f()).collect();
            let (bmin, bmax) = (t.f(), t.f());
            for x in v {
                o.i(h::histo_add_sample(g, &mut hh, x, bmin, bmax));
            }
            o.h(&hh);
        }
        "prevvalley" => {
            let hh = t.h();
            o.i(h::histo_find_previous_valley(g, &hh, t.i()));
        }
        "nextvalley" => {
            let hh = t.h();
            o.i(h::histo_find_next_valley(g, &hh, t.i()));
        }
        "nextpeak" => {
            let hh = t.h();
            let (b0, w) = (t.i(), t.i());
            o.i(h::histo_find_next_peak(g, &hh, b0, w));
        }
        "prevpeak" => {
            let hh = t.h();
            let (b0, w) = (t.i(), t.i());
            o.i(h::histo_find_previous_peak(g, &hh, b0, w));
        }
        "startpeak" => {
            let hh = t.h();
            let b0 = t.i();
            o.i(h::histo_find_start_of_peak(g, &hh, b0, t.f()));
        }
        "endpeak" => {
            let hh = t.h();
            let b0 = t.i();
            o.i(h::histo_find_end_of_peak(g, &hh, b0, t.f()));
        }
        "currentpeak" => {
            let hh = t.h();
            let (b0, w) = (t.i(), t.i());
            o.i(h::histo_find_current_peak(&hh, b0, w, t.f()));
        }
        "fillholes" => {
            let mut hh = t.h();
            h::histo_fill_holes(g, &mut hh);
            o.h(&hh);
        }
        "totalreg" => {
            let hh = t.h();
            let (b0, b1) = (t.i(), t.i());
            o.f(h::histo_total_in_region(g, &hh, b0, b1));
            o.f(h::histo_total(g, &hh));
        }
        "clearzero" => {
            let mut hh = t.h();
            h::histo_clear_zero_bin(g, &mut hh);
            o.h(&hh);
        }
        "findbin" => {
            let hh = t.h();
            let v = t.f();
            o.i(h::histo_find_bin(g, &hh, v));
            o.d(h::histo_get_count(g, &hh, v));
            o.i(h::histo_find_bin_with_count(g, &hh, v));
        }
        "correlate" => {
            let (a, b) = (t.h(), t.h());
            o.d(h::histo_correlate(g, &mut se, &a, &b).unwrap());
        }
        "linfit" => {
            let (a, b) = (t.h(), t.h());
            let (amin, amax, bmin, bmax) = (t.d(), t.d(), t.d(), t.d());
            let (c, pa, pb) = h::histo_find_linear_fit(g, &a, &b, amin, amax, bmin, bmax);
            o.d(c);
            o.f(pa);
            o.f(pb);
        }
        "linscale" => {
            let mut hh = t.h();
            let (s, off) = (t.f(), t.f());
            o.h(&new_h(h::histo_linear_scale(g, &hh, None, s, off)));
            h::histo_linear_scale_in_place(&mut hh, s, off);
            o.h(&hh);
        }
        "threshsum" => {
            let (a, b) = (t.h(), t.h());
            o.f(h::histo_thresh_sum(g, &a, &b, t.f()));
        }
        "makepdf" => {
            let mut hh = t.h();
            o.h(&new_h(h::histo_make_pdf(g, &hh, None)));
            h::histo_make_pdf_in_place(&mut hh);
            o.h(&hh);
        }
        "bins" => {
            let n = t.i();
            let (mn, mx) = (t.d(), t.d());
            o.h(&h::histo_bins(g, &mut se, n, mn, mx).unwrap());
        }
        "count" => {
            let mut hh = t.h();
            let n = t.i();
            let v: Vec<f64> = (0..n).map(|_| t.d()).collect();
            for &x in &v {
                o.i(h::histo_val_to_bin(&hh, x));
            }
            h::histo_count(g, &mut hh, &v);
            o.h(&hh);
        }
        "init" => {
            let n = t.i();
            let (mn, mx) = (t.d(), t.d());
            o.h(&new_h(h::histo_init(g, &mut se, None, n, mn, mx).unwrap()));
        }
        "initgiven" => {
            let mut hh = t.h();
            let n = t.i();
            let (mn, mx) = (t.d(), t.d());
            h::histo_init(g, &mut se, Some(&mut hh), n, mn, mx).unwrap();
            o.h(&hh);
        }
        "median" => {
            let hh = t.h();
            o.d(h::histo_find_median(&hh));
        }
        "cdf" => {
            let hh = t.h();
            o.h(&new_h(h::histo_make_cdf(g, &mut se, &hh, None).unwrap()));
            o.h(&new_h(
                h::histo_make_reverse_cdf(g, &mut se, &hh, None).unwrap(),
            ));
        }
        "robust" => {
            let mut hh = t.h();
            let p = t.d();
            let (status, off, sig) =
                h::histo_robust_gaussian_fit(g, &mut se, None, &mut hh, p).unwrap();
            o.i(status);
            o.d(off.unwrap_or(-1.0));
            o.d(sig.unwrap_or(-1.0));
            o.f(hh.bin_size);
        }
        "abs" => {
            let hh = t.h();
            o.h(&new_h(h::histo_abs(g, &mut se, &hh, None).unwrap()));
        }
        "entropy" => {
            let hh = t.h();
            o.d(h::histo_get_entropy(g, &hh));
        }
        "soap" => {
            let mut hh = t.h();
            let n = t.i();
            o.h(&new_h(h::histo_soap_bubble_zeros(g, &hh, None, n)));
            h::histo_soap_bubble_zeros_in_place(g, &mut hh, n);
            o.h(&hh);
        }
        "fwhm" => {
            let hh = t.h();
            o.f(h::histo_compute_fwhm(g, &hh, t.i()));
        }
        "sumnorm" => {
            let mut hh = t.h();
            h::histo_sum_norm(&mut hh);
            o.h(&hh);
        }
        "cumsum" => {
            let mut hh = t.h();
            o.h(&new_h(h::histo_cumsum(g, &hh, None)));
            h::histo_cumsum_in_place(&mut hh);
            o.h(&hh);
        }
        "clearbins" => {
            let mut hh = t.h();
            let (a, b) = (t.i(), t.i());
            o.h(&new_h(h::histo_clear_bins(g, &hh, None, a, b)));
            h::histo_clear_bins_in_place(&mut hh, a, b);
            o.h(&hh);
        }
        "addsub" => {
            let (mut a, b) = (t.h(), t.h());
            o.h(&new_h(h::histo_add(g, &a, Some(&b), None)));
            o.h(&new_h(h::histo_subtract(g, &a, &b, None)));
            o.h(&new_h(h::histo_add(g, &a, None, None)));
            let a1 = a.clone();
            h::histo_add(g, &a1, Some(&b), Some(&mut a));
            o.h(&a);
        }
        "clearcounts" => {
            let mut hh = t.h();
            o.h(&new_h(h::histo_clear_counts(g, &hh, None)));
            h::histo_clear_counts_in_place(g, &mut hh);
            o.h(&hh);
        }
        "copy" => {
            let mut hh = t.h();
            o.h(&new_h(h::histo_copy(g, &hh, None)));
            o.h(&new_h(h::histo_clear(&hh, None)));
            h::histo_clear_in_place(&mut hh);
            o.h(&hh);
        }
        "plot" => {
            let hh = t.h();
            let _ = std::fs::remove_file(tmp);
            o.i(h::histo_plot(&hh, None, tmp.to_str().unwrap()));
            o.file(tmp);
            let _ = std::fs::remove_file(tmp);
            o.i(h::histo_write_txt(
                &hh,
                &mut so,
                None,
                tmp.to_str().unwrap(),
            ));
            o.file(tmp);
        }
        "mmd" => {
            let a = t.m(g, MATRIX_REAL);
            let b = t.m(g, MATRIX_REAL);
            let c = m::matrix_multiply_d(g, &mut so, &mut se, Some(&a), Some(&b), None)
                .unwrap()
                .unwrap_new();
            o.mraw(&c);
            let d = m::matrix_copy(g, &mut so, &mut se, Some(&a), None)
                .unwrap()
                .unwrap_new();
            o.mraw(&d);
        }
        "mmdc" => {
            let (t1, t2) = (t.i() as i16, t.i() as i16);
            let a = t.m(g, t1);
            let b = t.m(g, t2);
            let mut c = m::matrix_multiply_d(g, &mut so, &mut se, Some(&a), Some(&b), None)
                .unwrap()
                .unwrap_new();
            o.mraw(&c);
            let a2 = m::matrix_copy(g, &mut so, &mut se, Some(&a), None)
                .unwrap()
                .unwrap_new();
            m::matrix_multiply_d(g, &mut so, &mut se, Some(&a2), Some(&b), Some(&mut c)).unwrap();
            o.mraw(&c);
            let a3 = m::matrix_copy(g, &mut so, &mut se, Some(&a), None)
                .unwrap()
                .unwrap_new();
            o.mraw(&a3);
            let _ = MATRIX_COMPLEX;
        }
        "affine" => {
            let a = t.m(g, MATRIX_REAL);
            let b = t.m(g, MATRIX_REAL);
            let (mut am, mut bm) = (AffineMatrix::default(), AffineMatrix::default());
            affine::set_affine_matrix(&mut se, &mut am, &a).unwrap();
            affine::set_affine_matrix(&mut se, &mut bm, &b).unwrap();
            let mut x = AffineVector::default();
            let (x0, x1, x2) = (t.f(), t.f(), t.f());
            affine::set_affine_vector(&mut x, x0, x1, x2);
            x.vec[3] = t.f();
            let y = affine::affine_mv(&am, &x);
            for v in y.vec {
                o.f(v);
            }
            let c = affine::affine_mm(&am, &bm);
            for v in c.mat {
                o.f(v);
            }
            let mut mm = Matrix::real(4, 4);
            affine::get_affine_matrix(&mut se, &mut mm, &c).unwrap();
            o.mraw(&mm);
            let (ix, iy, iz) = affine::get_floor_affine_vector(g, &y);
            o.i(ix);
            o.i(iy);
            o.i(iz);
        }
        "dmat" => {
            let (a, b) = (t.dm(), t.dm());
            let (v1, v2) = (t.d(), t.d());
            let r = |x: Ret<DMatrix>| x.into_new();
            let x = r(
                dmatrix::dmatrix_add_mul(g, &mut so, &mut se, &a, Some(&b), v1, v2, None).unwrap(),
            );
            o.dm(x.as_ref());
            let x = r(dmatrix::dmatrix_add(g, &mut so, &mut se, &a, &b, None).unwrap());
            o.dm(x.as_ref());
            let x = r(dmatrix::dmatrix_subtract(g, &mut so, &mut se, &a, &b, None).unwrap());
            o.dm(x.as_ref());
            let x = r(dmatrix::dmatrix_scalar_mul(g, &mut so, &mut se, &a, v1, None).unwrap());
            o.dm(x.as_ref());
            let x = r(dmatrix::dmatrix_transpose(g, &mut so, &mut se, &a, None).unwrap());
            o.dm(x.as_ref());
            let x = r(dmatrix::dmatrix_copy(g, &mut so, &mut se, Some(&a), None).unwrap());
            o.dm(x.as_ref());
            o.d(dmatrix::dmatrix_max_abs(&mut so, Some(&a)));
        }
        "dmmul" => {
            let (a, b, u, v) = (t.dm(), t.dm(), t.dm(), t.dm());
            let x = dmatrix::dmatrix_multiply(g, &mut so, &mut se, Some(&a), Some(&b), None)
                .unwrap()
                .into_new();
            o.dm(x.as_ref());
            o.d(dmatrix::dvector_dot(&mut so, Some(&u), Some(&v)));
        }
        "dliststats" => {
            let n = t.i();
            let v: Vec<f64> = (0..n).map(|_| t.d()).collect();
            let st = larmorx_freesurfer_core::utils::dlist_stats(&mut so, Some(&v), n).unwrap();
            for x in st {
                o.d(x);
            }
        }
        "mmf" => {
            let (t1, t2) = (t.i() as i16, t.i() as i16);
            let a = t.m(g, t1);
            let b = t.m(g, t2);
            let r = m::matrix_multiply(g, &mut so, &mut se, Some(&a), Some(&b), None).unwrap();
            o.mn(&r, None);
            if let Ret::New(mut c) = r {
                let a2 = m::matrix_copy(g, &mut so, &mut se, Some(&a), None)
                    .unwrap()
                    .unwrap_new();
                let r2 = m::matrix_multiply(g, &mut so, &mut se, Some(&a2), Some(&b), Some(&mut c))
                    .unwrap();
                o.mn(&r2, Some(&c));
            } else {
                o.0.push("NULL".to_owned());
            }
        }
        "melt" => {
            let a = t.m(g, MATRIX_REAL);
            let b = t.m(g, MATRIX_REAL);
            let sv = t.f();
            let (k, k1, k2) = (t.i(), t.i(), t.i());
            let (r, c) = (a.rows, a.cols);
            let x = mo::matrix_transpose(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_add(g, &mut so, &mut se, &a, &b, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_subtract(g, &mut so, &mut se, &a, &b, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_scalar_mul(g, &mut so, &mut se, &a, sv, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_square_elts(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let mut cl = a.clone();
            mo::matrix_clear(g, &mut cl);
            o.mraw(&cl);
            let x = mo::matrix_identity(g, &mut so, &mut se, r, None).unwrap();
            o.mn(&x, None);
            let mut tt = a.clone();
            let x = mo::matrix_identity(g, &mut so, &mut se, k, Some(&mut tt)).unwrap();
            o.mn(&x, Some(&tt));
            let x = mo::matrix_diag(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            o.i(mo::matrix_is_symmetric(g, &mut se, &a));
            if r == c {
                let at = mo::matrix_transpose(g, &mut so, &mut se, &a, None)
                    .unwrap()
                    .unwrap_new();
                let sym = mo::matrix_add(g, &mut so, &mut se, &a, &at, None)
                    .unwrap()
                    .unwrap_new();
                o.i(mo::matrix_is_symmetric(g, &mut se, &sym));
            }
            o.f(mo::matrix_trace(g, &mut se, &a));
            o.d(mo::matrix_sum_elts(g, &mut se, &a));
            o.i(mo::matrix_is_zero(g, &mut se, &a));
            o.i(mo::matrix_check(g, &mut se, &a));
            let x = mo::matrix_const_val(g, &mut so, &mut se, sv, r, c, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_normalize_col_scale(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_normalize_col(g, &mut so, &mut se, &a, None, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_reshape(g, &mut so, &mut se, &a, None, c, r).unwrap();
            o.mn(&x, None);
            let mut d = Matrix::real(k1, k2);
            mo::matrix_reshape(g, &mut so, &mut se, &a, Some(&mut d), 0, 0).unwrap();
            o.mraw(&d);
        }
        "mcplx" => {
            let a = t.m(g, MATRIX_COMPLEX);
            let sv = t.f();
            let x = mo::matrix_transpose(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_add(g, &mut so, &mut se, &a, &a, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_scalar_mul(g, &mut so, &mut se, &a, sv, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_square_elts(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let mut cl = a.clone();
            mo::matrix_clear(g, &mut cl);
            o.mraw(&cl);
            o.f(mo::matrix_trace(g, &mut se, &a));
            o.d(mo::matrix_sum_elts(g, &mut se, &a));
            o.i(mo::matrix_is_zero(g, &mut se, &a));
            let x = mo::matrix_diag(g, &mut so, &mut se, &a, None).unwrap();
            o.mn(&x, None);
            let mut cp = m::matrix_copy(g, &mut so, &mut se, Some(&a), None)
                .unwrap()
                .unwrap_new();
            let x = mo::matrix_const_val(g, &mut so, &mut se, sv, 0, 0, Some(&mut cp)).unwrap();
            o.mn(&x, Some(&cp));
        }
        "mregion" => {
            let which = match t.i() {
                0 => Region::All,
                1 => Region::Real,
                _ => Region::Imag,
            };
            let src = o.mt(g, &mut t);
            let given = t.i() != 0;
            let mut dst = if given { Some(o.mt(g, &mut t)) } else { None };
            let v: Vec<i32> = (0..6).map(|_| t.i()).collect();
            let r = mo::matrix_copy_region(
                g,
                &mut so,
                &mut se,
                which,
                &src,
                dst.as_mut(),
                v[0],
                v[1],
                v[2],
                v[3],
                v[4],
                v[5],
            )
            .unwrap();
            o.mn(&r, dst.as_ref());
            if let Some(d) = &dst {
                o.mraw(d);
            }
        }
        "mr2c" => {
            let a = t.m(g, MATRIX_REAL);
            let b = t.m(g, MATRIX_REAL);
            let mut c = t.m(g, MATRIX_COMPLEX);
            let x = mo::matrix_real_to_complex(g, &mut so, &mut se, &a, &b, None).unwrap();
            o.mn(&x, None);
            let x = mo::matrix_real_to_complex(g, &mut so, &mut se, &a, &b, Some(&mut c)).unwrap();
            o.mn(&x, Some(&c));
        }
        "mrot" => {
            let n = t.i();
            let angle = t.f();
            let which = t.i();
            let x = mo::matrix_alloc_rotation(g, &mut so, &mut se, n, angle, which).unwrap();
            o.mn(&x, None);
            let mut mm = t.m(g, MATRIX_REAL);
            let x =
                mo::matrix_realloc_rotation(g, &mut so, &mut se, n, angle, which, Some(&mut mm))
                    .unwrap();
            o.mn(&x, Some(&mm));
        }
        "mvec" => {
            let v1 = t.m(g, MATRIX_REAL);
            let v2 = t.m(g, MATRIX_REAL);
            let v3 = t.m(g, MATRIX_REAL);
            let norm = t.f();
            o.f(mo::vector_len(g, &mut se, &v1));
            o.f(mo::vector_dot(g, &mut se, &v1, &v2));
            o.f(mo::vector_distance(g, &mut se, &v1, &v2));
            let x = mo::vector_cross_product(g, &mut so, &mut se, &v1, &v2, None).unwrap();
            o.mn(&x, None);
            o.f(mo::vector_triple_product(g, &mut se, &v1, &v2, &v3));
            let (xyz, len) = mo::xyz_normalized_load(v1.relt(1, 1), v1.relt(2, 1), v1.relt(3, 1));
            o.f(xyz.x);
            o.f(xyz.y);
            o.f(xyz.z);
            o.f(len);
            o.f(mo::xyz_approx_angle_known_length(
                &xyz,
                v2.relt(1, 1),
                v2.relt(2, 1),
                v2.relt(3, 1),
                norm,
            ));
        }
        "v3angle" => {
            let v1 = t.m(g, MATRIX_REAL);
            let v2 = t.m(g, MATRIX_REAL);
            o.d(mo::vector3_angle(g, &mut se, &v1, &v2));
        }
        "mgauss" => {
            let len = t.i();
            let (s1, s2, w1) = (t.f(), t.f(), t.f());
            let norm = t.i();
            let x = mo::gaussian_matrix(g, &mut so, &mut se, len, s1, norm, None).unwrap();
            o.mn(&x, None);
            let x = mo::gaussian_matrix2(g, &mut so, &mut se, len, s1, s2, w1, norm, None).unwrap();
            o.mn(&x, None);
        }
        "msim" => {
            let a = t.m(g, MATRIX_REAL);
            let b = t.m(g, MATRIX_REAL);
            let x = mo::matrix_similarity_transform(g, &mut so, &mut se, &a, &b, None).unwrap();
            o.mn(&x, None);
        }
        "mprint" => {
            let a = o.mt(g, &mut t);
            let prec = t.i();
            let mut f = Vec::new();
            o.i(mo::matrix_print(
                g,
                Some(&mut f),
                &mut so,
                &mut se,
                Some(&a),
                prec,
            ));
            o.i(mo::matrix_print_fmt(
                g,
                Some(&mut f),
                &mut so,
                &mut se,
                "%10f",
                Some(&a),
            ));
            let _ = std::fs::write(tmp, &f);
            o.file(tmp);
            let _ = std::fs::remove_file(tmp);
        }
        "primes" => {
            let (nmax, n, p) = (t.i(), t.i(), t.i());
            let pr = prime::get_primes(&mut so, nmax).unwrap();
            o.i(pr.len() as i32);
            o.0.push(pr.iter().map(|&x| i64::from(x)).sum::<i64>().to_string());
            if let Some(&last) = pr.last() {
                o.i(last);
            }
            let f = prime::get_prime_factors(&mut so, n).unwrap();
            o.i(f.len() as i32);
            for &x in &f {
                o.i(x);
            }
            o.i(prime::get_closest_prime_factor(g, &mut so, n, p, false).unwrap());
            o.i(prime::get_closest_prime_factor(g, &mut so, n, p, true).unwrap());
        }
        "pdfg" => {
            let seed = t.i();
            let k = t.i();
            g.drand48.srand48(i64::from(seed));
            for _ in 0..k {
                o.d(pdf::pdf_gaussian(g));
            }
        }
        "minv" => {
            let a = o.mt(g, &mut t);
            let x = mo::matrix_inverse(g, &mut so, &mut se, Some(&a), None).unwrap();
            o.mn(&x, None);
            let mut out = m::matrix_alloc(g, &mut so, &mut se, a.rows, a.cols, a.mtype)
                .unwrap()
                .unwrap();
            let r = mo::matrix_inverse(g, &mut so, &mut se, Some(&a), Some(&mut out)).unwrap();
            o.0.push(if r.is_null() { "NULL" } else { "OK" }.to_owned());
            o.mraw(&out);
            o.f(mo::matrix_determinant(g, &mut se, &a));
        }
        "mqr" => {
            let a = t.m(g, MATRIX_REAL);
            let mut q = Matrix::real(a.rows, a.rows);
            let mut r = Matrix::real(a.rows, a.cols);
            o.i(mo::matrix_qr_decomposition(
                g,
                &mut so,
                &mut se,
                Some(&a),
                Some(&mut q),
                Some(&mut r),
            ));
            o.mraw(&q);
            o.mraw(&r);
        }
        "meig" => {
            let full = t.i();
            let a = t.m(g, MATRIX_REAL);
            let n = a.rows.max(1) as usize;
            let mut ev = vec![7.0f32; n];
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            if full != 0 {
                let r = md::matrix_eigen_system(g, &mut co, &mut ce, &a, &mut ev, None).unwrap();
                o.mn(&r, None);
                ev.iter().for_each(|&x| o.f(x));
                let mut out = Matrix::real(a.rows, a.rows);
                ev.fill(7.0);
                let r = md::matrix_eigen_system(g, &mut co, &mut ce, &a, &mut ev, Some(&mut out))
                    .unwrap();
                o.0.push(if r.is_null() { "NULL" } else { "OK" }.to_owned());
                o.mraw(&out);
                ev.iter().for_each(|&x| o.f(x));
                ev.fill(7.0);
                let r = md::matrix_calculate_eigen_system_helper(
                    g, &mut co, &mut ce, &a, &mut ev, None, false,
                )
                .unwrap();
                o.mn(&r, None);
                ev.iter().for_each(|&x| o.f(x));
            }
            ev.fill(7.0);
            let r = md::matrix_calculate_eigen_system_helper(
                g, &mut co, &mut ce, &a, &mut ev, None, true,
            )
            .unwrap();
            o.mn(&r, None);
            ev.iter().for_each(|&x| o.f(x));
            o.cap(&co);
            o.cap(&ce);
            // FreeSurfer never returns (the probe's child is stopped by its alarm)
            if mode == UpstreamDefects::Reproduce
                && g.notices.contains(&larmorx_core::defects::ITK_112)
            {
                o.0 = vec!["TIMEOUT".to_owned()];
            }
        }
        "msvd" => {
            let a = t.m(g, MATRIX_REAL);
            let (rows, cols) = (a.rows, a.cols);
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            let mut ev = vec![7.0f32; rows.max(1) as usize];
            o.f(md::matrix_svd_eigen_values(g, &mut co, &mut ce, &a, &mut ev).unwrap());
            ev[..rows as usize].iter().for_each(|&x| o.f(x));
            o.f(md::matrix_condition_number(g, &mut co, &mut ce, &a).unwrap());
            o.f(md::matrix_ns_condition_number(g, &mut co, &mut ce, &a).unwrap());
            let mut u = m::matrix_copy(g, &mut co, &mut ce, Some(&a), None)
                .unwrap()
                .unwrap_new();
            let mut z = Matrix::real(cols, 1);
            let r = md::matrix_svd(g, &mut co, &mut ce, &mut u, &mut z, None).unwrap();
            o.mn(&r, None);
            o.mraw(&u);
            o.mraw(&z);
            let mut u2 = m::matrix_copy(g, &mut co, &mut ce, Some(&a), None)
                .unwrap()
                .unwrap_new();
            let mut z2 = Matrix::real(cols, 1);
            let mut v2 = Matrix::real(cols, cols);
            let r = md::matrix_svd(g, &mut co, &mut ce, &mut u2, &mut z2, Some(&mut v2)).unwrap();
            o.0.push(if r.is_null() { "NULL" } else { "OK" }.to_owned());
            o.mraw(&v2);
            o.mraw(&u2);
            o.mraw(&z2);
            o.cap(&co);
            o.cap(&ce);
        }
        "msvdinv" => {
            let a = t.m(g, MATRIX_REAL);
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            let r = md::matrix_svd_inverse(g, &mut co, &mut ce, &a, None).unwrap();
            if crashed(g, mode) {
                o.0.push("CRASH".to_owned());
            } else {
                o.mn(&r, None);
                let mut out = Matrix::real(a.cols, a.rows);
                let r = md::matrix_svd_inverse(g, &mut co, &mut ce, &a, Some(&mut out)).unwrap();
                o.0.push(if r.is_null() { "NULL" } else { "OK" }.to_owned());
                o.mraw(&out);
                o.cap(&co);
                o.cap(&ce);
            }
        }
        "mpinv" => {
            let which = t.i();
            let a = t.m(g, MATRIX_REAL);
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            let mut out = match which {
                1 | 4 => Some(Matrix::real(a.cols, a.rows)),
                2 => Some(Matrix::real(a.rows, a.cols)),
                _ => None,
            };
            let r = if which < 3 {
                md::matrix_pseudo_inverse(g, &mut co, &mut ce, &a, out.as_mut()).unwrap()
            } else {
                md::matrix_svd_pseudo_inverse(g, &mut co, &mut ce, &a, out.as_mut()).unwrap()
            };
            if crashed(g, mode) {
                o.0.push("CRASH".to_owned());
            } else {
                match r {
                    Ret::Given => {
                        o.0.push("GIVEN".to_owned());
                        o.mraw(out.as_ref().unwrap());
                    }
                    r => {
                        o.mn(&r, None);
                        if let Some(x) = out.as_ref().filter(|_| which != 2) {
                            o.mraw(x);
                        }
                    }
                }
                o.cap(&co);
                o.cap(&ce);
            }
        }
        "mrdtxt" | "mrdasc" | "mrdbin" => {
            let dir = tmp.parent().unwrap_or(Path::new("."));
            let file = dir.join("fsmaths-file.tmp");
            let (mut given, mut gt, mut gr, mut gc) = (0, 1, 0, 0);
            if cmd != "mrdtxt" {
                (given, gt, gr, gc) = (t.i(), t.i(), t.i(), t.i());
            }
            let hex = t.next();
            let bytes: Vec<u8> = if hex == "-" {
                Vec::new()
            } else {
                (0..hex.len() / 2)
                    .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
                    .collect()
            };
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            if cmd == "mrdtxt" {
                std::fs::write(&file, &bytes).unwrap();
                let r = mio::matrix_read_txt(g, &mut co, &mut ce, Some(dir), "fsmaths-file.tmp")
                    .unwrap();
                o.mn(&r, None);
                o.i(g.errno);
                let _ = std::fs::remove_file(&file);
            } else {
                let mut om = if given != 0 {
                    m::matrix_alloc(g, &mut co, &mut ce, gr, gc, gt as i16).unwrap()
                } else {
                    None
                };
                g.errno = 0;
                let mut rd: &[u8] = &bytes;
                let r = if cmd == "mrdasc" {
                    mio::matrix_ascii_read_from(g, &mut co, &mut ce, &mut rd, om.as_mut()).unwrap()
                } else {
                    mio::matrix_read_from(g, &mut co, &mut ce, &mut rd, om.as_mut()).unwrap()
                };
                match r {
                    Ret::Given => {
                        o.0.push("GIVEN".to_owned());
                        o.mraw(om.as_ref().unwrap());
                    }
                    r => {
                        o.mn(&r, None);
                        if let Some(x) = om.as_ref() {
                            o.mraw(x);
                        }
                    }
                }
                o.i(g.errno);
                o.i((bytes.len() - rd.len()) as i32);
            }
            o.cap(&co);
            o.cap(&ce);
        }
        "mwr" => {
            let a = o.mt(g, &mut t);
            let dir = tmp.parent().unwrap_or(Path::new("."));
            let file = dir.join("fsmaths-file.tmp");
            let mut buf = Vec::new();
            o.i(mio::matrix_ascii_write_into(g, &mut buf, &a));
            std::fs::write(&file, &buf).unwrap();
            o.file(&file);
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            o.i(mio::matrix_write_txt(
                g,
                &mut ce,
                Some(dir),
                "fsmaths-file.tmp",
                &a,
            ));
            o.cap(&co);
            co.clear();
            o.cap(&ce);
            o.file(&file);
            let _ = std::fs::remove_file(&file);
        }
        "powell" | "powell2" | "dfpmin" => {
            let mut pr = t.problem();
            let n = pr.n;
            let mut p = NrVector::new(1, n as i64);
            for i in 1..=n as i64 {
                p[i] = t.f();
            }
            let (mut co, mut ce) = (Vec::new(), Vec::new());
            if cmd == "dfpmin" {
                let tol = t.f();
                let init = t.i();
                let verbose = t.i();
                pr.step_set = t.i() != 0;
                pr.user_set = t.i() != 0;
                let f = pr.function(p.as_slice());
                let mut fret = match init {
                    0 => f,
                    1 => 0.0,
                    _ => 2.0 * f,
                };
                let mut iter = -7;
                let gdiag = g.gdiag;
                if verbose != 0 {
                    g.gdiag |= larmorx_freesurfer_core::diag::DIAG_VERBOSE;
                }
                let r = nu::open_dfp_min(
                    g,
                    &mut co,
                    &mut ce,
                    p.as_mut_slice(),
                    n as i32,
                    tol,
                    &mut iter,
                    &mut fret,
                    &mut pr,
                )
                .unwrap();
                g.gdiag = gdiag;
                if mode == UpstreamDefects::Reproduce
                    && g.notices
                        .as_slice()
                        .iter()
                        .any(|x| x.defect.id == "FS-37" && x.detail.contains("crashes"))
                {
                    o.0.push("CRASH".to_owned());
                } else {
                    o.i(r);
                    o.i(iter);
                    o.f(fret);
                    for i in 1..=n as i64 {
                        o.f(p[i]);
                    }
                    o.counts(&pr);
                    o.cap(&co);
                    o.cap(&ce);
                }
            } else {
                let mut xi = NrMatrix::new(1, n as i64, 1, n as i64);
                for r in 1..=n as i64 {
                    for k in 1..=n as i64 {
                        xi[(r, k)] = t.f();
                    }
                }
                let mut iter = -7;
                let mut fret = -7.0f32;
                if cmd == "powell" {
                    let tol = t.f();
                    pr.iter_set = t.i() != 0;
                    nu::open_powell(
                        g,
                        &mut co,
                        &mut ce,
                        p.as_mut_slice(),
                        Some(&mut xi),
                        n as i32,
                        tol,
                        &mut iter,
                        &mut fret,
                        &mut pr,
                    )
                    .unwrap();
                } else {
                    let (ftol, lin) = (t.f(), t.f());
                    let (maxit, gno) = (t.i(), t.i());
                    pr.iter_set = t.i() != 0;
                    let save = g.gdiag_no;
                    g.gdiag_no = gno;
                    let r = nu::open_powell2(
                        g,
                        &mut co,
                        &mut ce,
                        p.as_mut_slice(),
                        Some(&mut xi),
                        n as i32,
                        ftol,
                        lin,
                        maxit,
                        &mut iter,
                        &mut fret,
                        &mut pr,
                    );
                    g.gdiag_no = save;
                    o.i(r);
                }
                o.i(iter);
                o.f(fret);
                for i in 1..=n as i64 {
                    o.f(p[i]);
                }
                for r in 1..=n as i64 {
                    for k in 1..=n as i64 {
                        o.f(xi[(r, k)]);
                    }
                }
                o.counts(&pr);
                o.cap(&co);
                o.cap(&ce);
            }
        }
        _ => panic!("unknown case {cmd}"),
    }
    let _ = so.flush();
    let ids = gl.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (o.0.join(" "), ids)
}

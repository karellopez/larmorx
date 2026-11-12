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

#![allow(dead_code)]

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::affine::{self, AffineMatrix, AffineVector};
use larmorx_freesurfer_core::dmatrix::{self, DMatrix};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{self as h, Histogram};
use larmorx_freesurfer_core::matrix::{self as m, MATRIX_COMPLEX, MATRIX_REAL, Matrix};
use larmorx_freesurfer_core::ret::Ret;

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
];

/// One case line of command `cmd`.
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
            for _ in 0..2 {
                s.push(r.to_string());
                s.push(c.to_string());
                for _ in 0..r * c {
                    let v = if g.below(6) == 0 {
                        g.pick(&[0.0, -0.0])
                    } else {
                        g.d(-1000, 1000, 1024)
                    };
                    s.push(dx(v));
                }
            }
            let r = g.d(-10, 10, 64);
            s.push(dx(g.pick(&[0.0, 1.0, -1.0, 2.5, r])));
            let r = g.d(-10, 10, 64);
            s.push(dx(g.pick(&[0.0, 1.0, -1.0, 0.25, r])));
        }
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
        _ => panic!("unknown case {cmd}"),
    }
    let _ = so.flush();
    let ids = gl.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (o.0.join(" "), ids)
}

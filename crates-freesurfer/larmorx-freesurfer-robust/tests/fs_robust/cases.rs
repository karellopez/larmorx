// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.robust probe's cases (`validation/probes/fs-robust/`): the generator (seeded) and the
//! port's answers, one line per case (`<result> | <text>`, the result as `fsrobust.cpp` prints it,
//! the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the
//! program). The volumes are fs-volume's (`get_vol`), with voxels painted after the fill.
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::vnl_matrix::{VnlCMatrix, VnlMatrix};
use larmorx_freesurfer_core::affine::AffineMatrix;
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{self as mm, Mri};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, reinit_cache};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_robust::quaternion::Quaternion;
use larmorx_freesurfer_robust::real::{Mat, Real};
use larmorx_freesurfer_robust::regression::{
    Regression, get_sigma_mad, get_sqrt_tukey_dia_weights,
};
use larmorx_freesurfer_robust::regstep::{Cost, RegistrationStep, StepParams};
use larmorx_freesurfer_robust::robust_gauss as rg;
use larmorx_freesurfer_robust::transformation::{Kind, Transformation};
use larmorx_freesurfer_robust::{costs, mymatrix as mx, mymri};
use larmorx_freesurfer_volume::arith::Pair;

/// The cases' random numbers (SplitMix64).
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
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

const SPECIAL: [f32; 6] = [
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    1e30,
    -0.0,
    1e-40,
];

const SPECIAL64: [u64; 8] = [
    0x7ff8_0000_0000_0000,
    0x7ff4_0000_0000_0123,
    0xfff8_0000_0000_0007,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
    0x8000_0000_0000_0000,
    0x0000_0000_0000_0001,
    0x7fef_ffff_ffff_ffff,
];

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn num(g: &mut Gen, lo: i64, hi: i64, d: i64) -> f32 {
    if g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.int(lo * d, hi * d) as f32 / d as f32
}

fn cosine(g: &mut Gen) -> f32 {
    if g.chance(60) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[-1.0, 0.0, 1.0, 0.6, 0.8, -0.6, -0.8, 0.70710677, 0.1, 0.995])
}

fn size(g: &mut Gen) -> f32 {
    if g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[1.0, 0.5, 2.0, 1.25, 0.9375, 3.0, 0.0, -1.0, 1.1, 0.8])
}

const NAMES: [&str; 4] = ["-", "vol.mgz", "orig/001.mgz", "a"];

/// `set_geom`'s tokens (fs-volume's).
fn geom(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.pick(&[0, 1, 1, 1]).to_string());
    for _ in 0..3 {
        t.push(g.pick(&[1, 2, 3, 4, 6, 256]).to_string());
    }
    for _ in 0..3 {
        t.push(fx(size(g)));
    }
    t.push(g.int(0, 1).to_string());
    // usually a scanner's direction cosines (an identity or a flip), sometimes drawn
    if g.chance(3) {
        for _ in 0..9 {
            t.push(fx(cosine(g)));
        }
    } else {
        let m = g.pick(&[
            [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0],
        ]);
        for v in m {
            t.push(fx(v));
        }
    }
    for _ in 0..3 {
        t.push(fx(num(g, -120, 120, 8)));
    }
    for _ in 0..3 {
        t.push(fx(0.0));
    }
    t.push(g.pick(&NAMES).to_owned());
}

const FTAB: [u32; 28] = [
    0x00000000, 0x80000000, 0x3f800000, 0xbf800000, 0x3f000000, 0x3fc00000, 0x40200000, 0xc0200000,
    0x437f0000, 0x437f8000, 0x43800000, 0xc3962000, 0x477fff9a, 0x501502f9, 0xd01502f9, 0x7f7fc99e,
    0x000116c2, 0x7f800000, 0xff800000, 0x7fc00000, 0x7fa12345, 0xffc00001, 0x40490fd0, 0x24e69595,
    0x42fecccd, 0x3dcccccd, 0x41200000, 0xc1a00000,
];

/// A volume's shape in the cases.
#[derive(Clone, Copy)]
struct Shape {
    w: i64,
    h: i64,
    d: i64,
    ty: i64,
    nf: i64,
}

/// Types: mostly float and bytes (what the registration uses), sometimes short, unsigned short,
/// int and long.
fn shape(g: &mut Gen) -> Shape {
    let ty = if g.chance(6) {
        g.pick(&[1, 4, 10, 2])
    } else {
        g.pick(&[3, 3, 0, 0])
    };
    Shape {
        w: g.int(1, 7),
        h: g.int(1, 7),
        d: if g.chance(5) { 1 } else { g.int(2, 6) },
        ty,
        nf: if g.chance(6) { 2 } else { 1 },
    }
}

/// An ordinary voxel value of a registration's image (non-negative, mostly).
fn intensity(g: &mut Gen) -> f32 {
    if g.chance(50) {
        return g.pick(&SPECIAL);
    }
    if g.chance(4) {
        return 0.0;
    }
    if g.chance(20) {
        return -(g.int(1, 50) as f32) / 4.0;
    }
    g.int(0, 1020) as f32 / 4.0
}

/// `get_vol`'s tokens: the fill (fs-volume's), then painted voxels: none, a few, or every voxel
/// (an ordinary image, most often for float volumes, whose fill holds NaNs and infinities).
fn vol_of(g: &mut Gen, t: &mut Vec<String>, s: Shape) {
    for v in [s.w, s.h, s.d, s.ty, s.nf, 0, g.int(0, 255)] {
        t.push(v.to_string());
    }
    geom(g, t);
    t.push(g.int(0, 3).to_string());
    t.push(g.int(0, 5).to_string());
    for _ in 0..6 {
        t.push(fx(num(g, -100, 100, 4)));
    }
    t.push(fx(g.pick(&[1.0, 1.5, 0.0, 2.0])));
    t.push(fx(g.pick(&[1.0, 0.9375, 0.0])));
    for _ in 0..3 {
        t.push(fx(g.pick(&[0.0, 2000.0, 30.5])));
    }
    t.push(dx(g.pick(&[0.0, 0.17453292519943295])));
    t.push(g.int(0, 3).to_string());
    t.push(g.pick(&[0i64, 1, 1]).to_string());
    let all = if s.ty == 3 { !g.chance(5) } else { g.chance(3) };
    if all {
        t.push((s.w * s.h * s.d * s.nf).to_string());
        for f in 0..s.nf {
            for z in 0..s.d {
                for y in 0..s.h {
                    for x in 0..s.w {
                        for v in [x, y, z, f] {
                            t.push(v.to_string());
                        }
                        t.push(fx(intensity(g)));
                    }
                }
            }
        }
    } else {
        let n = g.int(0, 3);
        t.push(n.to_string());
        for _ in 0..n {
            for v in [
                g.int(0, s.w - 1),
                g.int(0, s.h - 1),
                g.int(0, s.d - 1),
                g.int(0, s.nf - 1),
            ] {
                t.push(v.to_string());
            }
            t.push(fx(intensity(g)));
        }
    }
}

fn vol(g: &mut Gen, t: &mut Vec<String>) -> Shape {
    let s = shape(g);
    vol_of(g, t, s);
    s
}

/// A double of a transform's entry: a rotation's, a scale's, a translation's, or special.
fn d64(g: &mut Gen) -> f64 {
    if g.chance(60) {
        return f64::from_bits(g.pick(&SPECIAL64));
    }
    if g.chance(4) {
        return g.int(-8, 8) as f64 / 4.0;
    }
    (g.unit() - 0.5) * 4.0
}

/// A 4 x 4 transform: a rotation (from a random unit quaternion) times a scale, a translation, the
/// last row (0, 0, 0, 1); sometimes perturbed, sometimes any 16 numbers.
fn m44(g: &mut Gen) -> [[f64; 4]; 4] {
    if g.chance(12) {
        return std::array::from_fn(|_| std::array::from_fn(|_| d64(g)));
    }
    let q: [f64; 4] = std::array::from_fn(|_| g.unit() - 0.5);
    let n = (q.iter().map(|x| x * x).sum::<f64>()).sqrt();
    let [a, b, c, d] = q.map(|x| x / n);
    let r = [
        [
            a * a + b * b - c * c - d * d,
            2.0 * (b * c - a * d),
            2.0 * (b * d + a * c),
        ],
        [
            2.0 * (b * c + a * d),
            a * a - b * b + c * c - d * d,
            2.0 * (c * d - a * b),
        ],
        [
            2.0 * (b * d - a * c),
            2.0 * (c * d + a * b),
            a * a - b * b - c * c + d * d,
        ],
    ];
    let small = g.chance(2);
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        let s = if g.chance(3) {
            1.0 + (g.unit() - 0.5) * 0.4
        } else {
            1.0
        };
        for j in 0..3 {
            m[i][j] = if small {
                // near the identity (a registration's step)
                if i == j { 1.0 } else { 0.0 }
            } else {
                r[i][j]
            } * s;
        }
        m[i][3] = (g.unit() - 0.5) * if g.chance(2) { 4.0 } else { 200.0 };
    }
    if small {
        // a small rotation
        let e = (g.unit() - 0.5) * 0.1;
        m[0][1] += e;
        m[1][0] -= e;
    }
    m[3][3] = 1.0;
    if g.chance(15) {
        let (i, j) = (g.int(0, 3) as usize, g.int(0, 3) as usize);
        m[i][j] = d64(g);
    }
    m
}

fn push_m44(t: &mut Vec<String>, m: &[[f64; 4]; 4]) {
    for row in m {
        for &v in row {
            t.push(dx(v));
        }
    }
}

/// A `vnl_matrix<double>` of the case: rows cols, then the numbers.
fn vm(g: &mut Gen, t: &mut Vec<String>, rows: i64, cols: i64) {
    t.push(rows.to_string());
    t.push(cols.to_string());
    let m = m44(g);
    for r in 0..rows {
        for c in 0..cols {
            let v = if r < 4 && c < 4 {
                m[r as usize][c as usize]
            } else {
                d64(g)
            };
            t.push(dx(v));
        }
    }
}

/// The commands, in the order the generator cycles through them.
pub const COMMANDS: [&str; 21] = [
    "vscale",
    "prefilter",
    "derfilter",
    "rand",
    "subsample",
    "blur",
    "partials",
    "rightsize",
    "bg",
    "ltx",
    "centroid",
    "orient",
    "v2m",
    "vv2m",
    "m2v",
    "getvnl",
    "isdiag",
    "polar",
    "polar2",
    "adist",
    "rdist",
];
/// The commands added after the first ones (cycled with them).
pub const MORE: [&str; 2] = ["fdiff", "rlog"];

pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let all: Vec<&str> = COMMANDS.iter().chain(MORE.iter()).copied().collect();
    let mut lines = Vec::with_capacity(n);
    for k in 0..n {
        let cmd = all[k % all.len()];
        let mut t = vec![cmd.to_owned()];
        gen_case(&mut g, &mut t, cmd);
        lines.push(t.join(" "));
    }
    lines
}

fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "vscale" => {
            vol(g, t);
            t.push(g.int(0, 1).to_string());
            let s = if g.chance(10) {
                f64::from_bits(g.pick(&SPECIAL64))
            } else {
                g.pick(&[1.0, 0.5, 2.0, 1.25, 255.0 / 250.0, 0.987654321, -1.0, 1e10])
            };
            let b = if g.chance(10) {
                f64::from_bits(g.pick(&SPECIAL64))
            } else {
                g.pick(&[0.0, 0.0, 3.5, -10.0, 1e-3])
            };
            t.push(dx(s));
            t.push(dx(b));
        }
        "prefilter" | "derfilter" => {}
        "rand" => {
            t.push(
                g.pick(&[0, 1, 50, 100, 101, 102, 202, 1000, 2147483647])
                    .to_string(),
            );
        }
        "subsample" => {
            vol(g, t);
            t.push(g.int(0, 1).to_string());
            t.push(g.pick(&[-1, 0, 0, 7, 95, 100]).to_string());
        }
        "blur" | "partials" | "bg" | "orient" => {
            vol(g, t);
            if cmd == "blur" {
                t.push(g.int(0, 1).to_string());
            }
        }
        "rightsize" => {
            vol(g, t);
            t.push(fx(g.pick(&[1.0, 0.5, 2.0, 0.9375, 1.1, 0.0, 3.0])));
            t.push(g.int(0, 1).to_string());
        }
        "ltx" => {
            let s = vol(g, t);
            if g.chance(3) {
                t.push("1".into());
                let o = shape(g);
                vol_of(g, t, Shape { ty: s.ty, ..o });
            } else {
                t.push("0".into());
            }
            let m = m44(g);
            push_m44(t, &m);
        }
        "centroid" => {
            vol(g, t);
            let ov = if g.chance(8) {
                f64::from_bits(g.pick(&SPECIAL64))
            } else {
                g.pick(&[0.0, 0.0, 255.0, 100.0, -1.0, 2.5])
            };
            t.push(dx(ov));
        }
        "v2m" | "isdiag" | "polar" | "polar2" | "fdiff" => {
            let n = if cmd == "v2m" || cmd == "fdiff" {
                g.pick(&[1, 3, 4, 4, 2])
            } else {
                g.pick(&[3, 4, 4, 2])
            };
            let c = if cmd == "v2m" || cmd == "fdiff" {
                g.pick(&[n, n, 1, 3])
            } else {
                n
            };
            vm(g, t, n, c);
            if cmd == "isdiag" {
                t.push(dx(g.pick(&[1e-10, 0.0, 1e-30, 1.0, 1e-3])));
            }
            if cmd == "fdiff" {
                let m = m44(g);
                for r in 0..n {
                    for k in 0..c {
                        let v = if r < 4 && k < 4 && g.chance(2) {
                            m[r as usize][k as usize]
                        } else {
                            d64(g)
                        };
                        t.push(dx(v));
                    }
                }
            }
        }
        "vv2m" => {
            let n = g.int(1, 5);
            t.push(n.to_string());
            for _ in 0..n {
                t.push(dx(d64(g)));
            }
        }
        "m2v" => {
            let (r, c) = (g.int(1, 4), g.int(1, 4));
            t.push(r.to_string());
            t.push(c.to_string());
            for _ in 0..r * c {
                t.push(fx(num(g, -100, 100, 8)));
            }
        }
        "getvnl" => {
            let n = g.pick(&[9, 9, 12, 16, 4, 10]);
            t.push(n.to_string());
            for _ in 0..n {
                t.push(dx(d64(g)));
            }
            t.push(g.pick(&[3, 3, 4, 2, 1, 5]).to_string());
        }
        "adist" | "rdist" => {
            let a = m44(g);
            push_m44(t, &a);
            let hasb = !g.chance(4);
            t.push(i64::from(hasb).to_string());
            if hasb {
                // usually close to a (consecutive iterations)
                let mut b = if g.chance(2) { a } else { m44(g) };
                if g.chance(2) {
                    b[0][3] += 0.01;
                    b[1][0] += 1e-4;
                }
                push_m44(t, &b);
            }
            if cmd == "adist" {
                t.push(dx(g.pick(&[100.0, 50.0, 1.0, 0.0])));
            }
        }
        "rlog" => {
            let a = m44(g);
            push_m44(t, &a);
        }
        _ => unreachable!("{cmd}"),
    }
}

// ------------------------------------------------------------------------------------------
// The matrix square root's cases (3.2-02 part 2: `schur`, `sqrtm`), a stream of their own

/// The square root's commands, alternating.
pub const SQRTM: [&str; 2] = ["schur", "sqrtm"];

/// A 3 x 3 rotation from a random unit quaternion, sometimes scaled.
fn rot3(g: &mut Gen) -> [[f64; 3]; 3] {
    let q: [f64; 4] = std::array::from_fn(|_| g.unit() - 0.5);
    let n = (q.iter().map(|x| x * x).sum::<f64>()).sqrt();
    let [a, b, c, d] = q.map(|x| x / n);
    let mut r = [
        [
            a * a + b * b - c * c - d * d,
            2.0 * (b * c - a * d),
            2.0 * (b * d + a * c),
        ],
        [
            2.0 * (b * c + a * d),
            a * a - b * b + c * c - d * d,
            2.0 * (c * d - a * b),
        ],
        [
            2.0 * (b * d - a * c),
            2.0 * (c * d + a * b),
            a * a - b * b - c * c + d * d,
        ],
    ];
    if g.chance(4) {
        let s = 1.0 + (g.unit() - 0.5) * 0.4;
        for row in &mut r {
            for v in row {
                *v *= s;
            }
        }
    }
    r
}

/// A square matrix for the square root: the registration's transforms and rotations, and the
/// structures LAPACK's paths turn on (zero patterns its balancing permutes, Jordan blocks,
/// rotations by a right or a straight angle, negative and zero eigenvalues, tiny and huge
/// scales, subnormal and special entries, other sizes).
fn sq_matrix(g: &mut Gen) -> (usize, Vec<f64>) {
    let kind = g.below(20);
    let (n, mut m): (usize, Vec<f64>) = match kind {
        0..=5 => (4, m44(g).iter().flatten().copied().collect()),
        6..=8 => (3, rot3(g).iter().flatten().copied().collect()),
        9 => {
            // a rotation by a right or a straight angle, maybe with a translation
            let (c, s) = g.pick(&[(0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 0.0)]);
            let mut m = vec![
                c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ];
            if g.chance(2) {
                m[3] = (g.unit() - 0.5) * 20.0;
                m[7] = (g.unit() - 0.5) * 20.0;
            }
            (4, m)
        }
        10 => {
            // a Jordan block, maybe defective in a larger matrix
            let n = g.pick(&[2usize, 3, 4]);
            let l = g.pick(&[1.0, 2.0, 0.5, -1.0, 0.0]);
            let mut m = vec![0.0; n * n];
            for i in 0..n {
                m[i * n + i] = l;
                if i + 1 < n {
                    m[i * n + i + 1] = g.pick(&[1.0, 1e-8, 0.0]);
                }
            }
            (n, m)
        }
        11 => {
            // diagonal or triangular, with zeros the balancing permutes
            let n = g.pick(&[2usize, 3, 4, 5]);
            let mut m = vec![0.0; n * n];
            let tri = g.below(3);
            for i in 0..n {
                for j in 0..n {
                    let keep = i == j || (tri == 1 && j > i) || (tri == 2 && j < i);
                    if keep && !g.chance(5) {
                        m[i * n + j] = if i == j {
                            g.pick(&[1.0, 2.0, 0.25, 9.0, -4.0, 1.5, 0.0])
                        } else {
                            (g.unit() - 0.5) * 4.0
                        };
                    }
                }
            }
            (n, m)
        }
        12 => {
            // a sparse matrix: zero rows and columns isolate eigenvalues
            let n = g.pick(&[3usize, 4, 5, 6]);
            let mut m = vec![0.0; n * n];
            for v in m.iter_mut() {
                if g.chance(2) {
                    *v = (g.unit() - 0.5) * 4.0;
                }
            }
            for i in 0..n {
                if m[i * n + i] == 0.0 {
                    m[i * n + i] = 1.0 + g.unit();
                }
            }
            (n, m)
        }
        13 => {
            // another size, dense
            let n = g.pick(&[1usize, 2, 5, 6, 8, 12]);
            let mut m = vec![0.0; n * n];
            for (k, v) in m.iter_mut().enumerate() {
                *v = (g.unit() - 0.5) * 2.0 + if k % (n + 1) == 0 { 2.0 } else { 0.0 };
            }
            (n, m)
        }
        14 => {
            // tiny or huge: LAPACK scales the matrix (below 2^-459, above 2^459)
            let f = g.pick(&[1e-150, 1e-300, 1e150, 1e300, 1e-200, 4e-320]);
            let mut m: Vec<f64> = m44(g).iter().flatten().copied().collect();
            for v in m.iter_mut() {
                *v *= f;
            }
            (4, m)
        }
        15 => {
            // a column of tiny numbers below the diagonal (the reflectors' rescaling) or of
            // numbers below 2^-511 (the norm's small sums)
            let n = g.pick(&[3usize, 4]);
            let mut m = vec![0.0; n * n];
            for i in 0..n {
                m[i * n + i] = 1.0 + g.unit();
            }
            let t = g.pick(&[1e-300, 1e-160, 1e-310, 5e-324, 1e-200]);
            for i in 1..n {
                m[i * n] = t * (1.0 + g.unit());
            }
            if g.chance(2) {
                m[1] = 0.5;
            }
            (n, m)
        }
        16 => {
            // a negative eigenvalue (the root is complex) or a singular matrix
            let mut m: Vec<f64> = m44(g).iter().flatten().copied().collect();
            if g.chance(2) {
                for v in m.iter_mut().take(4) {
                    *v = -*v;
                }
            } else {
                for k in 0..4 {
                    m[4 + k] = m[k];
                }
            }
            (4, m)
        }
        17 => (0, Vec::new()),
        18 => {
            // the identity, the zero matrix, a scalar multiple
            let n = g.pick(&[1usize, 3, 4]);
            let v = g.pick(&[1.0, 0.0, 4.0, -0.0, 1e-20]);
            let mut m = vec![0.0; n * n];
            for i in 0..n {
                m[i * n + i] = v;
            }
            (n, m)
        }
        _ => {
            // any numbers, special ones among them
            let n = g.pick(&[2usize, 3, 4]);
            (n, (0..n * n).map(|_| d64(g)).collect())
        }
    };
    if g.chance(30) && n > 0 {
        let k = g.below((n * n) as u64) as usize;
        m[k] = f64::from_bits(g.pick(&SPECIAL64));
    }
    (n, m)
}

/// The matrix square root's cases of `seed`.
pub fn generate_sqrtm(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x5157_524d);
    (0..n)
        .map(|k| {
            let (m, v) = sq_matrix(&mut g);
            let mut t = vec![SQRTM[k % 2].to_owned(), m.to_string(), m.to_string()];
            t.extend(v.iter().map(|&x| dx(x)));
            t.join(" ")
        })
        .collect()
}

// ------------------------------------------------------------------------------------------
// The matrix logarithm's and exponential's cases (3.2-15, relay 51: `logm`, `expm`, `geomean`), a
// stream of their own

/// The logarithm's commands, cycled.
pub const LOGM: [&str; 3] = ["logm", "expm", "geomean"];

/// A rotation from the unit quaternion `(1, w v)` normalised (`v` a random direction, `w` sets
/// the angle: about `2 w` radians for a small `w`; square roots only).
fn rot_w(g: &mut Gen, w: f64) -> [[f64; 3]; 3] {
    let v: [f64; 3] = std::array::from_fn(|_| g.unit() - 0.5);
    let nv = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
    let q = [1.0, w * v[0] / nv, w * v[1] / nv, w * v[2] / nv];
    let n = q.iter().map(|x| x * x).sum::<f64>().sqrt();
    let [a, b, c, d] = q.map(|x| x / n);
    [
        [
            a * a + b * b - c * c - d * d,
            2.0 * (b * c - a * d),
            2.0 * (b * d + a * c),
        ],
        [
            2.0 * (b * c + a * d),
            a * a - b * b + c * c - d * d,
            2.0 * (c * d - a * b),
        ],
        [
            2.0 * (b * d - a * c),
            2.0 * (c * d + a * b),
            a * a - b * b - c * c + d * d,
        ],
    ]
}

fn mul33(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

/// A 3 x 3 linear part of a transform: a rotation (small, large, or near a half turn) times a
/// scale (isotropic, anisotropic with one axis set apart: two groups of eigenvalues) and a shear.
fn lin3(g: &mut Gen) -> [[f64; 3]; 3] {
    let w = g.pick(&[0.0, 0.001, 0.01, 0.03, 0.05, 0.1, 0.3, 0.7, 2.0, 30.0]);
    let r = rot_w(g, w);
    let mut s = [[0.0; 3]; 3];
    match g.below(4) {
        0 => {
            for (i, row) in s.iter_mut().enumerate() {
                row[i] = 1.0;
            }
        }
        1 => {
            let f = 1.0 + (g.unit() - 0.5) * 0.2;
            for (i, row) in s.iter_mut().enumerate() {
                row[i] = f;
            }
        }
        2 => {
            // one axis apart by more than 0.1: a pair beside a single eigenvalue
            let k = g.below(3) as usize;
            for (i, row) in s.iter_mut().enumerate() {
                row[i] = if i == k {
                    g.pick(&[1.3, 0.8, 1.6, 0.6])
                } else {
                    1.0 + (g.unit() - 0.5) * 0.02
                };
            }
        }
        _ => {
            for (i, row) in s.iter_mut().enumerate() {
                row[i] = 1.0 + (g.unit() - 0.5) * 0.4;
            }
        }
    }
    if g.chance(3) {
        let (i, j) = (g.below(3) as usize, g.below(3) as usize);
        if i != j {
            s[i][j] = (g.unit() - 0.5) * 0.3;
        }
    }
    if g.chance(2) {
        mul33(&r, &s)
    } else {
        mul33(&s, &r)
    }
}

/// A square matrix for the logarithm: [`lin3`]'s mostly, and the structures its paths turn on
/// (diagonal with separate or equal values, Jordan blocks, a negative or zero eigenvalue, a
/// half-turn rotation, other sizes, tiny and huge scales, special entries).
fn log_matrix(g: &mut Gen) -> (usize, Vec<f64>) {
    let kind = g.below(16);
    let (n, mut m): (usize, Vec<f64>) = match kind {
        0..=6 => (3, lin3(g).iter().flatten().copied().collect()),
        7 => {
            let n = g.pick(&[2usize, 3, 4]);
            let mut m = vec![0.0; n * n];
            for i in 0..n {
                m[i * n + i] = g.pick(&[1.0, 1.05, 2.0, 0.5, 0.95, 1.2, 3.0]);
                for j in i + 1..n {
                    if g.chance(2) {
                        m[i * n + j] = (g.unit() - 0.5) * 0.5;
                    }
                }
            }
            (n, m)
        }
        8 => {
            // a Jordan block (one eigenvalue, defective)
            let n = g.pick(&[2usize, 3]);
            let l = g.pick(&[1.0, 1.1, 0.7]);
            let mut m = vec![0.0; n * n];
            for i in 0..n {
                m[i * n + i] = l;
                if i + 1 < n {
                    m[i * n + i + 1] = g.pick(&[1.0, 0.1, 1e-8]);
                }
            }
            (n, m)
        }
        9 => {
            // a negative or zero eigenvalue (FS-1099), a half turn
            let mut m: Vec<f64> = lin3(g).iter().flatten().copied().collect();
            match g.below(3) {
                0 => {
                    for v in m.iter_mut().take(3) {
                        *v = -*v;
                    }
                }
                1 => {
                    for k in 0..3 {
                        m[3 + k] = m[k];
                    }
                }
                _ => {
                    m = vec![-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0];
                    m[1] = g.pick(&[0.0, 1e-3, 0.1]);
                }
            }
            (3, m)
        }
        10 => {
            // another size: a perturbed identity, or a 4 x 4 transform
            let n = g.pick(&[1usize, 2, 4, 5]);
            if n == 4 && g.chance(2) {
                return (4, m44(g).iter().flatten().copied().collect());
            }
            let mut m = vec![0.0; n * n];
            for (k, v) in m.iter_mut().enumerate() {
                *v = (g.unit() - 0.5) * 0.3 + if k % (n + 1) == 0 { 1.0 } else { 0.0 };
            }
            (n, m)
        }
        11 => {
            let f = g.pick(&[1e-3, 1e3, 1e-150, 1e150, 4.0, 0.25]);
            let m: Vec<f64> = lin3(g).iter().flatten().map(|&x| x * f).collect();
            (3, m)
        }
        12 => {
            // the identity, a multiple of it
            let v = g.pick(&[1.0, 2.0, 0.5]);
            let mut m = vec![0.0; 9];
            for i in 0..3 {
                m[i * 3 + i] = v;
            }
            (3, m)
        }
        13 => (3, (0..9).map(|_| d64(g)).collect()),
        _ => (3, rot3(g).iter().flatten().copied().collect()),
    };
    if g.chance(40) && n > 0 {
        let k = g.below((n * n) as u64) as usize;
        m[k] = f64::from_bits(g.pick(&SPECIAL64));
    }
    (n, m)
}

/// A square matrix for the exponential: a logarithm-like small matrix scaled to each of the
/// Pade degrees' ranges and to the scaling and squaring, and other sizes and special entries.
fn exp_matrix(g: &mut Gen) -> (usize, Vec<f64>) {
    let n = g.pick(&[3usize, 3, 3, 2, 4, 1, 5]);
    let mut m: Vec<f64> = (0..n * n).map(|_| g.unit() - 0.5).collect();
    if g.chance(3) && n == 3 {
        // skew-symmetric (a rotation's logarithm) plus a small symmetric part
        for i in 0..3 {
            m[i * 3 + i] = (g.unit() - 0.5) * 0.1;
            for j in i + 1..3 {
                let a = m[i * 3 + j];
                m[j * 3 + i] = -a + (g.unit() - 0.5) * 0.05;
            }
        }
    }
    let f = g.pick(&[
        0.0, 1e-3, 0.01, 0.05, 0.2, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 20.0, 100.0, 1000.0,
    ]);
    for v in m.iter_mut() {
        *v *= f;
    }
    if g.chance(40) {
        let k = g.below((n * n) as u64) as usize;
        m[k] = f64::from_bits(g.pick(&SPECIAL64));
    }
    (n, m)
}

fn push_vm(t: &mut Vec<String>, n: usize, v: &[f64]) {
    t.push(n.to_string());
    t.push(n.to_string());
    t.extend(v.iter().map(|&x| dx(x)));
}

/// The logarithm's cases of `seed`.
pub fn generate_logm(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x4c4f_474d);
    (0..n)
        .map(|k| {
            let cmd = LOGM[k % 3];
            let mut t = vec![cmd.to_owned()];
            match cmd {
                "logm" => {
                    let (m, v) = log_matrix(&mut g);
                    push_vm(&mut t, m, &v);
                }
                "expm" => {
                    let (m, v) = exp_matrix(&mut g);
                    push_vm(&mut t, m, &v);
                }
                _ => {
                    // the template's mean space: the identity first, then the others' linear parts
                    let k = g.int(1, 4) as usize;
                    t.push(k.to_string());
                    for i in 0..k {
                        let v: Vec<f64> = if i == 0 && !g.chance(4) {
                            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
                        } else if g.chance(20) {
                            log_matrix(&mut g).1.into_iter().chain(std::iter::repeat(0.0)).take(9).collect()
                        } else {
                            lin3(&mut g).iter().flatten().copied().collect()
                        };
                        push_vm(&mut t, 3, &v);
                    }
                }
            }
            t.join(" ")
        })
        .collect()
}

// ------------------------------------------------------------------------------------------
// The registration step's cases (3.2-03 part 1: `rgauss`, `regress`, `trans`, `quat`, `rstep`),
// a stream of their own

/// The registration step's commands, cycled.
pub const REGSTEP: [&str; 5] = ["rgauss", "regress", "trans", "quat", "rstep"];

/// A number of the registration's lists: values with ties, zeros, outliers, sometimes special.
fn rnum(g: &mut Gen) -> f64 {
    if g.chance(45) {
        return f64::from_bits(g.pick(&SPECIAL64));
    }
    match g.below(6) {
        0 => g.int(-4, 4) as f64,
        1 => 0.0,
        2 => (g.unit() - 0.5) * 200.0,
        3 => g.int(-1000, 1000) as f64 / 16.0,
        4 => (g.unit() - 0.5) * 1e-4,
        _ => (g.unit() - 0.5) * 8.0,
    }
}

/// A number token of the case's precision (`dbl`: 16 hex digits, else 8 of a float).
fn tok(dbl: bool, v: f64) -> String {
    if dbl { dx(v) } else { fx(v as f32) }
}

/// A float token whose special value keeps its own bits (a signalling NaN is not quieted).
fn tokf_special(g: &mut Gen) -> String {
    fx(g.pick(&SPECIAL))
}

fn gen_rgauss(g: &mut Gen, t: &mut Vec<String>) {
    let dbl = g.chance(2);
    t.push(i64::from(dbl).to_string());
    let op = g.pick(&[
        "median", "median", "medianI", "mad", "mad", "kth", "qsel", "kthI", "qselI",
    ]);
    t.push(op.to_owned());
    let sel = op.starts_with('k') || op.starts_with('q');
    let n = if sel {
        g.int(1, 13)
    } else if g.chance(25) {
        0
    } else {
        g.int(1, 14)
    };
    t.push(n.to_string());
    let pool: Vec<f64> = (0..4).map(|_| rnum(g)).collect();
    for _ in 0..n {
        if !dbl && g.chance(60) {
            t.push(tokf_special(g));
            continue;
        }
        let v = if g.chance(3) { g.pick(&pool) } else { rnum(g) };
        t.push(tok(dbl, v));
    }
    if op == "mad" {
        let d = if g.chance(3) { rnum(g) } else { 1.4826 };
        t.push(tok(dbl, d));
    } else if sel {
        t.push(g.int(1, n).to_string());
    }
}

fn gen_regress(g: &mut Gen, t: &mut Vec<String>) {
    let dbl = g.chance(2);
    t.push(i64::from(dbl).to_string());
    let method = g.pick(&[
        "rob", "rob", "rob", "rob", "ls", "ls", "wls", "wlsf", "sigma", "tukey",
    ]);
    t.push(method.to_owned());
    t.push(g.pick(&[0, 1, 1]).to_string());
    t.push(g.pick(&[0, 1, 2, 2]).to_string());
    let sat = if g.chance(4) {
        g.pick(&[2.0, 6.0, 0.5, -1.0, 1e-3, 100.0, f64::INFINITY])
    } else {
        4.685
    };
    t.push(dx(sat));
    t.push(dx(g.pick(&[1.4826, 1.4826, 2.0])));
    let has_a = !(matches!(method, "rob" | "ls") && g.chance(5));
    t.push(i64::from(has_a).to_string());
    let cols = if has_a { g.int(1, 7) } else { 1 };
    let rows = if g.chance(12) {
        g.int(1, cols.max(2))
    } else {
        g.int(cols, 48)
    };
    t.push(rows.to_string());
    t.push(cols.to_string());
    // a system: A's columns gradient-like, b = A p + noise, some outliers
    let ptrue: Vec<f64> = (0..cols).map(|_| (g.unit() - 0.5) * 4.0).collect();
    let zero_col = if g.chance(8) { g.int(0, cols - 1) } else { -1 };
    let mut a = vec![0.0f64; (rows * cols) as usize];
    for r in 0..rows {
        for c in 0..cols {
            let v = if c == zero_col {
                0.0
            } else if g.chance(120) {
                f64::from_bits(g.pick(&SPECIAL64))
            } else {
                match g.below(4) {
                    0 => g.int(-50, 50) as f64 / 4.0,
                    1 => (g.unit() - 0.5) * 100.0,
                    2 => (r as f64) * (g.unit() - 0.5),
                    _ => (g.unit() - 0.5) * 2.0,
                }
            };
            a[(r * cols + c) as usize] = v;
        }
    }
    if has_a {
        for &v in &a {
            t.push(tok(dbl, v));
        }
    }
    let identical = g.chance(12);
    for r in 0..rows {
        let mut v = if has_a {
            (0..cols)
                .map(|c| a[(r * cols + c) as usize] * ptrue[c as usize])
                .sum::<f64>()
        } else {
            ptrue[0] * 10.0
        };
        if !identical {
            v += (g.unit() - 0.5) * 0.5;
            if g.chance(6) {
                v += (g.unit() - 0.5) * 400.0;
            }
            if g.chance(150) {
                v = f64::from_bits(g.pick(&SPECIAL64));
            }
        }
        t.push(tok(dbl, v));
    }
    match method {
        "rob" => {
            let wsize = if has_a {
                g.pick(&[0, rows, rows])
            } else {
                g.pick(&[rows, rows, rows + 2, 0])
            };
            t.push(wsize.to_string());
            for _ in 0..wsize {
                t.push(tok(dbl, g.unit()));
            }
        }
        "wls" | "wlsf" | "tukey" => {
            if method != "tukey" {
                for _ in 0..rows {
                    let w = if g.chance(5) { 0.0 } else { g.unit() };
                    t.push(tok(dbl, w));
                }
            }
        }
        "sigma" => {
            let d = if g.chance(3) { rnum(g) } else { 1.4826 };
            t.push(tok(dbl, d));
        }
        _ => {}
    }
}

/// A transformation parameter: an angle, a translation, a scale or a shear, sometimes special.
fn tparam(g: &mut Gen) -> f64 {
    if g.chance(40) {
        return f64::from_bits(g.pick(&SPECIAL64));
    }
    match g.below(5) {
        0 => (g.unit() - 0.5) * 0.4,
        1 => (g.unit() - 0.5) * 40.0,
        2 => 1.0 + (g.unit() - 0.5) * 0.2,
        3 => g.pick(&[0.0, -0.0, 1.0, std::f64::consts::PI, 1e-8, 1e-7]),
        _ => (g.unit() - 0.5) * 8.0,
    }
}

fn gen_trans(g: &mut Gen, t: &mut Vec<String>) {
    let code = g.int(0, 14);
    let dof = [6i64, 6, 4, 4, 3, 3, 2, 12, 12, 7, 7, 6, 6, 3, 0][code as usize];
    let ptype = g.int(0, 2);
    let np = match g.below(10) {
        0 => dof + 1,
        1 if dof > 0 => dof - 1,
        _ => dof,
    };
    for v in [code, ptype, np] {
        t.push(v.to_string());
    }
    for _ in 0..np {
        let v = tparam(g);
        t.push(if ptype == 1 { fx(v as f32) } else { dx(v) });
    }
    t.push(i64::from(g.chance(8)).to_string());
    for _ in 0..3 {
        t.push(g.pick(&[0i64, 1, 2, 7, 31, 128, 255, 300]).to_string());
        let f = if g.chance(25) {
            g.pick(&SPECIAL)
        } else {
            ((g.unit() - 0.5) * 60.0) as f32
        };
        t.push(fx(f));
    }
}

fn gen_quat(g: &mut Gen, t: &mut Vec<String>) {
    let op = g.pick(&["rv", "rva", "zyx"]);
    t.push(op.to_owned());
    let k = if op == "rva" { 4 } else { 3 };
    for _ in 0..k {
        let v = if g.chance(6) {
            g.pick(&[0.0, 1e-7, 1e-6, -1e-6, std::f64::consts::PI])
        } else {
            tparam(g)
        };
        t.push(dx(v));
    }
}

/// `get_vol`'s tokens for an image of the registration step: a smooth blob (`centre` and
/// `scale`), a background of `bg` outside a radius, noise, a few outliers, sometimes a NaN.
#[allow(clippy::too_many_arguments)]
fn reg_vol(
    g: &mut Gen,
    t: &mut Vec<String>,
    s: Shape,
    centre: [f64; 3],
    scale: f64,
    bg: f64,
    nan: bool,
) {
    for v in [s.w, s.h, s.d, s.ty, s.nf, 0, g.int(0, 255)] {
        t.push(v.to_string());
    }
    geom(g, t);
    t.push("0".to_owned());
    t.push((s.d - 1).to_string());
    for _ in 0..6 {
        t.push(fx(0.0));
    }
    t.push(fx(1.0));
    t.push(fx(1.0));
    for _ in 0..3 {
        t.push(fx(0.0));
    }
    t.push(dx(0.0));
    t.push("0".to_owned());
    t.push("0".to_owned());
    t.push((s.w * s.h * s.d * s.nf).to_string());
    let r2 = (s.w * s.w + s.h * s.h + s.d * s.d) as f64 / 6.0;
    for f in 0..s.nf {
        for z in 0..s.d {
            for y in 0..s.h {
                for x in 0..s.w {
                    for v in [x, y, z, f] {
                        t.push(v.to_string());
                    }
                    let (dx_, dy_, dz_) = (
                        x as f64 - centre[0],
                        y as f64 - centre[1],
                        z as f64 - centre[2],
                    );
                    let d2 = dx_ * dx_ + dy_ * dy_ + dz_ * dz_;
                    let mut v = if d2 > r2 * 1.6 {
                        bg
                    } else {
                        scale * (200.0 - 150.0 * d2 / r2) + 20.0 * f as f64 + (g.unit() - 0.5) * 2.0
                    };
                    if g.chance(30) {
                        v = g.int(0, 255) as f64;
                    }
                    if nan && g.chance(25) {
                        v = f64::NAN;
                    }
                    let v = if s.ty == 0 {
                        v.clamp(0.0, 255.0).round()
                    } else {
                        (v * 4.0).round() / 4.0
                    };
                    t.push(fx(v as f32));
                }
            }
        }
    }
}

fn gen_rstep(g: &mut Gen, t: &mut Vec<String>) {
    let dbl = g.chance(4);
    let d = if g.chance(4) { 1 } else { g.int(3, 7) };
    let codes: &[i64] = if d == 1 {
        &[6, 4, 4, 2, 0, 0, 14, 1, 3, 5]
    } else {
        &[13, 11, 11, 11, 7, 7, 14, 9, 8, 10, 12]
    };
    let code = g.pick(codes);
    let costfun = g.pick(&[1, 1, 1, 1, 0, 2]);
    let iscale = code == 14 || g.chance(3);
    let ss = if g.chance(4) { g.int(1, 4) } else { -1 };
    let calls = if g.chance(5) { 2 } else { 1 };
    t.push(i64::from(dbl).to_string());
    t.push(g.pick(&[1, 1, 1, 0]).to_string());
    t.push(costfun.to_string());
    t.push(code.to_string());
    let sat = if g.chance(3) {
        g.pick(&[-1.0, 2.5, 6.0, 9.0])
    } else {
        4.685
    };
    t.push(dx(sat));
    t.push(i64::from(iscale).to_string());
    t.push(ss.to_string());
    t.push(g.pick(&[0, 1, 2, 2]).to_string());
    t.push(calls.to_string());
    let s = Shape {
        w: g.int(3, 10),
        h: g.int(3, 10),
        d,
        ty: g.pick(&[3, 3, 0]),
        nf: if g.chance(8) { 2 } else { 1 },
    };
    for _ in 0..calls {
        let c = [
            s.w as f64 / 2.0 + (g.unit() - 0.5) * 2.0,
            s.h as f64 / 2.0 + (g.unit() - 0.5) * 2.0,
            s.d as f64 / 2.0 + (g.unit() - 0.5) * 2.0,
        ];
        let shift = [
            (g.unit() - 0.5) * 1.5,
            (g.unit() - 0.5) * 1.5,
            if s.d == 1 {
                0.0
            } else {
                (g.unit() - 0.5) * 1.5
            },
        ];
        let bg = g.pick(&[0.0, 0.0, 0.0, 10.0]);
        let nan = s.ty == 3 && g.chance(10);
        let scale_t = if iscale {
            1.0 + (g.unit() - 0.5) * 0.3
        } else {
            1.0
        };
        reg_vol(g, t, s, c, 1.0, bg, nan);
        let ct = [c[0] + shift[0], c[1] + shift[1], c[2] + shift[2]];
        reg_vol(g, t, s, ct, scale_t, bg, nan);
    }
}

/// The registration step's cases of `seed`.
pub fn generate_regstep(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x5245_4753);
    (0..n)
        .map(|k| {
            let cmd = REGSTEP[k % REGSTEP.len()];
            let mut t = vec![cmd.to_owned()];
            match cmd {
                "rgauss" => gen_rgauss(&mut g, &mut t),
                "regress" => gen_regress(&mut g, &mut t),
                "trans" => gen_trans(&mut g, &mut t),
                "quat" => gen_quat(&mut g, &mut t),
                _ => gen_rstep(&mut g, &mut t),
            }
            t.join(" ")
        })
        .collect()
}

// ---- the registration's cases (3.2-03 part 2, relay 43)

/// The registration's command.
pub const REG: [&str; 1] = ["rreg"];

/// A blob picture's tokens (`fsrobust.cpp`'s `get_blob`): the extent and type, the geometry,
/// the blobs, the background, the texture and the voxel transform (row by row).
#[allow(clippy::too_many_arguments)]
fn blob_toks(
    t: &mut Vec<String>,
    dims: [i64; 3],
    ty: i64,
    sizes: [f32; 3],
    cosines: [f32; 9],
    c_ras: [f32; 3],
    blobs: &[[f64; 5]],
    bg: f64,
    tex: f64,
    a: &[f64; 12],
) {
    for v in dims {
        t.push(v.to_string());
    }
    t.push(ty.to_string());
    for v in sizes.iter().chain(&cosines).chain(&c_ras) {
        t.push(fx(*v));
    }
    t.push(blobs.len().to_string());
    for b in blobs {
        for &v in b {
            t.push(dx(v));
        }
    }
    t.push(dx(bg));
    t.push(dx(tex));
    for &v in a {
        t.push(dx(v));
    }
}

/// A small rotation-like voxel transform about `c` (`[[1, -a, b], [a, 1, -c], [-b, c, 1]]`
/// times `k`, rational entries only) and a shift.
fn moved(g: &mut Gen, centre: [f64; 3], two_d: bool, big: bool) -> [f64; 12] {
    let r = |g: &mut Gen, m: f64| ((g.unit() - 0.5) * 2.0 * m * 64.0).round() / 64.0;
    let m = if big { 0.15 } else { 0.06 };
    let (a, b, c) = if two_d {
        (r(g, m), 0.0, 0.0)
    } else {
        (r(g, m), r(g, m), r(g, m))
    };
    let k = if g.chance(4) { 1.0 + r(g, 0.05) } else { 1.0 };
    let rot = [[k, -a * k, b * k], [a * k, k, -c * k], [-b * k, c * k, k]];
    let mut sh = [r(g, 2.0), r(g, 2.0), if two_d { 0.0 } else { r(g, 2.0) }];
    if big {
        for v in &mut sh {
            *v *= 2.0;
        }
    }
    let mut out = [0.0; 12];
    for i in 0..3 {
        out[4 * i] = rot[i][0];
        out[4 * i + 1] = rot[i][1];
        out[4 * i + 2] = rot[i][2];
        // p = R (x - c) + c + shift
        out[4 * i + 3] = centre[i]
            - (rot[i][0] * centre[0] + rot[i][1] * centre[1] + rot[i][2] * centre[2])
            + sh[i];
    }
    out
}

fn gen_rreg(g: &mut Gen, t: &mut Vec<String>) {
    let model = g.pick(&[0i64, 0, 0, 0, 1, 1, 2, 4, 3]);
    let two_d = g.chance(8);
    let dbl = g.chance(5);
    let iscale = model == 4 || g.chance(3);
    let sym = !g.chance(5);
    let sat = g.pick(&[4.685, 4.685, 6.0, 9.0, 3.0, -1.0]);
    let wlimit = g.pick(&[0.16, 0.16, 0.16, 0.08, 0.3]);
    let satit = g.chance(6);
    let nomulti = !satit && g.chance(6);
    let iterate = g.pick(&[5i64, 5, 3, 1, 8]);
    let epsit = g.pick(&[0.01, 0.01, 0.05, 0.001]);
    let highit = g.pick(&[-1i64, -1, -1, -1, 0, 1, 2]);
    let minsize = g.pick(&[-1i64, -1, -1, -1, 16, 18]);
    let maxsize = g.pick(&[-1i64, -1, -1, -1, 20, 28]);
    let ss = g.pick(&[-1i64, -1, -1, -1, 16, 18]);
    let inittrans = !g.chance(4);
    let initorient = g.chance(6);
    let initscaling = g.chance(12);
    let verbose = g.pick(&[1i64, 1, 1, 2, 0]);
    let keeptype = g.chance(2);
    for v in [i64::from(dbl), model, i64::from(iscale), i64::from(sym)] {
        t.push(v.to_string());
    }
    t.push(dx(sat));
    t.push(dx(wlimit));
    for v in [i64::from(satit), i64::from(nomulti), iterate] {
        t.push(v.to_string());
    }
    t.push(dx(epsit));
    for v in [
        highit,
        minsize,
        maxsize,
        ss,
        i64::from(inittrans),
        i64::from(initorient),
        i64::from(initscaling),
        verbose,
        i64::from(keeptype),
    ] {
        t.push(v.to_string());
    }
    if g.chance(6) {
        t.push("1".to_owned());
        let m = moved(g, [10.0, 10.0, 10.0], two_d, false);
        for i in 0..3 {
            for j in 0..4 {
                t.push(dx(m[4 * i + j]));
            }
        }
        for v in [0.0, 0.0, 0.0, 1.0] {
            t.push(dx(v));
        }
    } else {
        t.push("0".to_owned());
    }
    t.push(dx(if g.chance(5) { 1.125 } else { 1.0 }));
    // the movable and the destination
    let side = |g: &mut Gen| g.pick(&[16i64, 16, 18, 20, 20, 22, 24, 28, 33]);
    let dims = [side(g), side(g), if two_d { 1 } else { side(g) }];
    let ty = g.pick(&[3i64, 3, 0, 0, 4]);
    let sizes = g.pick(&[
        [1.0f32, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.5],
        [0.9375, 0.9375, 1.25],
        [2.0, 2.0, 2.0],
    ]);
    let ident = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    // LIA (a coronal volume): x -> -R, y -> -S, z -> +A
    let lia = [-1.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
    // 45 degrees about S (reorientSource's conflict between two columns) and a sagittal volume
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let rot45 = [h, h, 0.0, -h, h, 0.0, 0.0, 0.0, 1.0];
    let sag = [0.0f32, -1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0];
    let orient = |g: &mut Gen| match g.below(12) {
        0 | 1 => lia,
        2 => rot45,
        3 => sag,
        _ => ident,
    };
    let cos_m = orient(g);
    let c_ras = [
        g.pick(&[0.0f32, 0.0, 1.5, -2.25]),
        g.pick(&[0.0f32, 0.0, 3.0]),
        g.pick(&[0.0f32, -1.0]),
    ];
    let ctr = [
        (dims[0] - 1) as f64 / 2.0,
        (dims[1] - 1) as f64 / 2.0,
        (dims[2] - 1) as f64 / 2.0,
    ];
    let hi = if ty == 0 { 70.0 } else { 140.0 };
    let nb = g.int(2, 3) as usize;
    let mut blobs = Vec::new();
    for _ in 0..nb {
        let off = |g: &mut Gen, n: i64| ((g.unit() - 0.5) * (n as f64) * 0.5 * 16.0).round() / 16.0;
        let r = (dims[0].min(dims[1]) as f64 / 4.0).round();
        blobs.push([
            ctr[0] + off(g, dims[0]),
            ctr[1] + off(g, dims[1]),
            if two_d { 0.0 } else { ctr[2] + off(g, dims[2]) },
            (hi * (0.5 + g.unit() * 0.5) * 16.0).round() / 16.0,
            r * r,
        ]);
    }
    let bg = g.pick(&[0.0, 0.0, 0.0, 0.0, 10.0]);
    let tex = g.pick(&[0.0, 1.0, 2.5]);
    let mut id12 = [0.0; 12];
    id12[0] = 1.0;
    id12[5] = 1.0;
    id12[10] = 1.0;
    blob_toks(t, dims, ty, sizes, cos_m, c_ras, &blobs, bg, tex, &id12);
    // the destination: the same picture moved (and scaled in intensity with iscale)
    let same = !g.chance(4);
    let dims_t = if same {
        dims
    } else {
        [side(g), side(g), if two_d { 1 } else { side(g) }]
    };
    let ty_t = if g.chance(5) {
        g.pick(&[3i64, 0, 4])
    } else {
        ty
    };
    let sizes_t = if same || g.chance(2) {
        sizes
    } else {
        [1.0, 1.0, 1.0]
    };
    let cos_t = if g.chance(6) { orient(g) } else { cos_m };
    let big = g.chance(5);
    let a = moved(g, ctr, two_d, big);
    let k = if iscale {
        g.pick(&[1.0, 1.0625, 0.9375, 1.125])
    } else {
        1.0
    };
    let blobs_t: Vec<[f64; 5]> = blobs
        .iter()
        .map(|b| [b[0], b[1], b[2], b[3] * k, b[4]])
        .collect();
    blob_toks(
        t,
        dims_t,
        ty_t,
        sizes_t,
        cos_t,
        c_ras,
        &blobs_t,
        bg * k,
        tex,
        &a,
    );
}

/// The registration's cases of `seed`.
pub fn generate_reg(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x5252_4547);
    (0..n)
        .map(|_| {
            let mut t = vec!["rreg".to_owned()];
            gen_rreg(&mut g, &mut t);
            t.join(" ")
        })
        .collect()
}

// ------------------------------------------------------------------------------------------
// The answers

/// One buffer both streams write into, in order.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn i(&mut self) -> i32 {
        self.next().parse().expect("an int")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("a float"))
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.next(), 16).expect("a double"))
    }
    fn s(&mut self) -> String {
        let s = self.next();
        if s == "-" {
            String::new()
        } else {
            s.to_owned()
        }
    }
}

#[derive(Default)]
struct Out(String);

impl Out {
    fn i(&mut self, v: i64) {
        self.0 += &format!("{v} ");
    }
    fn f(&mut self, v: f32) {
        self.0 += &format!("{:08x} ", v.to_bits());
    }
    fn d(&mut self, v: f64) {
        self.0 += &format!("{:016x} ", v.to_bits());
    }
    fn hex(&mut self, b: &[u8]) {
        if b.is_empty() {
            self.0 += "- ";
            return;
        }
        for c in b {
            self.0 += &format!("{c:02x}");
        }
        self.0 += " ";
    }
    fn m(&mut self, m: Option<&Matrix>) {
        let Some(m) = m else {
            self.0 += "NULL ";
            return;
        };
        self.i(i64::from(m.rows));
        self.i(i64::from(m.cols));
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                self.f(m.relt(r, c));
            }
        }
    }
    fn vm(&mut self, m: &VnlMatrix) {
        self.i(m.rows as i64);
        self.i(m.cols as i64);
        for &v in &m.data {
            self.d(v);
        }
    }
    fn cm(&mut self, m: &VnlCMatrix) {
        self.i(m.rows as i64);
        self.i(m.cols as i64);
        for z in &m.data {
            self.d(z.re);
            self.d(z.im);
        }
    }
    fn geom(&mut self, v: &VolGeom) {
        for x in [v.width, v.height, v.depth] {
            self.i(i64::from(x));
        }
        for x in [v.xsize, v.ysize, v.zsize] {
            self.f(x);
        }
        self.i(i64::from(v.ras_good_flag));
        for x in [
            v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s,
            v.s_r, v.s_a, v.s_s,
        ] {
            self.f(x);
        }
        self.hex(v.fname.c_bytes());
    }
    fn am(&mut self, a: Option<&AffineMatrix>) {
        match a {
            None => self.0 += "NULL ",
            Some(a) => {
                for v in a.mat {
                    self.f(v);
                }
            }
        }
    }
    fn vol(&mut self, m: Option<&Mri>) {
        let Some(m) = m else {
            self.0 += "NULL ";
            return;
        };
        for v in [
            m.vg.width,
            m.vg.height,
            m.vg.depth,
            m.nframes,
            m.mtype,
            m.imnr0,
            m.imnr1,
        ] {
            self.i(i64::from(v));
        }
        for v in [
            m.xstart, m.xend, m.ystart, m.yend, m.zstart, m.zend, m.thick, m.ps, m.tr, m.te, m.ti,
        ] {
            self.f(v);
        }
        self.d(m.flip_angle);
        self.i(i64::from(m.dof));
        self.d(m.outside_val);
        self.geom(&m.vg);
        self.am(m.vg.i_to_r.as_ref());
        self.m(m.vg.r_to_i.as_ref());
        match &m.chunk {
            Some(c) => self.hex(c),
            None => self.0 += "nochunk ",
        }
    }
}

struct Ctx {
    g: Globals,
    a: Shared,
    b: Shared,
    /// A double least-squares solve of a matrix with an infinite or NaN entry (its NaN bits
    /// are compared as one NaN: `SVDNAN`).
    svdnan: bool,
}

fn set_geom(t: &mut Toks, v: &mut VolGeom) {
    v.valid = t.i();
    v.width = t.i();
    v.height = t.i();
    v.depth = t.i();
    v.xsize = t.f();
    v.ysize = t.f();
    v.zsize = t.f();
    v.ras_good_flag = t.i();
    v.x_r = t.f();
    v.x_a = t.f();
    v.x_s = t.f();
    v.y_r = t.f();
    v.y_a = t.f();
    v.y_s = t.f();
    v.z_r = t.f();
    v.z_a = t.f();
    v.z_s = t.f();
    v.c_r = t.f();
    v.c_a = t.f();
    v.c_s = t.f();
    v.s_r = t.f();
    v.s_a = t.f();
    v.s_s = t.f();
    v.fname = CName::new(t.s().as_bytes());
}

fn get_vol(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let (w, h, d, ty, nf, hdr) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
    let seed = t.i() as u32 as u64;
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let mut m = if hdr != 0 {
        mm::mri_alloc_header(g, a, b, w, h, d, ty, nf)?
    } else {
        mm::mri_alloc_sequence(g, a, b, w, h, d, ty, nf)?
    };
    if let Some(c) = m.chunk.as_mut() {
        if ty == mm::MRI_FLOAT {
            for i in 0..c.len() / 4 {
                let v = FTAB[((i as u64 * 7 + seed) % FTAB.len() as u64) as usize];
                c[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
            }
        } else {
            for (k, x) in c.iter_mut().enumerate() {
                let k = k as u64;
                *x = ((k * 37 + seed * 101 + (k >> 3) * 13) & 0xff) as u8;
            }
        }
    }
    set_geom(t, &mut m.vg);
    m.vg.width = w;
    m.vg.height = h;
    m.vg.depth = d;
    m.imnr0 = t.i();
    m.imnr1 = t.i();
    m.xstart = t.f();
    m.xend = t.f();
    m.ystart = t.f();
    m.yend = t.f();
    m.zstart = t.f();
    m.zend = t.f();
    m.thick = t.f();
    m.ps = t.f();
    m.tr = t.f();
    m.te = t.f();
    m.ti = t.f();
    m.flip_angle = t.d();
    m.dof = t.i();
    match t.i() {
        1 => {
            reinit_cache(g, a, b, &mut m.vg)?;
        }
        2 => {
            m.vg.i_to_r = None;
            m.vg.r_to_i = None;
        }
        _ => {}
    }
    let n = t.i();
    for _ in 0..n {
        let (x, y, z, f) = (t.i(), t.i(), t.i(), t.i());
        let v = t.f();
        mri_set_vox_val(g, b, &mut m, x, y, z, f, v, 0)?;
    }
    Ok(m)
}

fn get_vm(t: &mut Toks) -> VnlMatrix {
    let (r, c) = (t.i() as usize, t.i() as usize);
    let mut m = VnlMatrix::zeros(r, c);
    for v in m.data.iter_mut() {
        *v = t.d();
    }
    m
}

fn get_m4(t: &mut Toks) -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for row in &mut m {
        for v in row {
            *v = t.d();
        }
    }
    m
}

fn escape(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            s += &format!("\\x{c:02x}");
        } else {
            s.push(c as char);
        }
    }
    s
}

/// The port's line for a case line, and the defects it noted.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(32 << 20)
            .spawn_scoped(s, || run_line_here(line, mode))
            .expect("a thread for the case")
            .join()
            .expect("the case's thread")
    })
}

fn run_line_here(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let shared = Shared::default();
    let mut cx = Ctx {
        g: Globals::new(mode),
        a: shared.clone(),
        b: shared.clone(),
        svdnan: false,
    };
    cx.g.progname = "fsrobust".to_owned();
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = Out::default();
    let res = case(&mut cx, &mut t, &mut o);
    let text = escape(&shared.0.borrow());
    let result = match res {
        Ok(()) => o.0.trim_end().to_owned(),
        Err(f) => format!("EXIT {}", f.status()),
    };
    let mut ids: Vec<&'static str> =
        cx.g.notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id)
            .collect();
    if cx.svdnan {
        ids.push("SVDNAN");
    }
    ids.sort_unstable();
    ids.dedup();
    (format!("{result} | {text}"), ids)
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let cmd = t.next();
    match cmd {
        "vscale" => {
            let mut src = get_vol(cx, t)?;
            let same = t.i() != 0;
            let (s, b) = (t.d(), t.d());
            let (g, a, e) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if same {
                let r = mymri::mri_valscale(g, a, e, Pair::Same(&mut src), s, b)?;
                o.i(1);
                debug_assert!(matches!(r, Ret::Given));
                o.vol(Some(&src));
            } else {
                let r = mymri::mri_valscale(g, a, e, Pair::New(&src), s, b)?;
                o.i(0);
                o.vol(r.into_new().as_ref());
            }
        }
        "prefilter" => {
            let m = mymri::get_prefilter(&mut cx.g, &mut cx.a, &mut cx.b)?;
            o.vol(Some(&m));
        }
        "derfilter" => {
            let m = mymri::get_derfilter(&mut cx.g, &mut cx.a, &mut cx.b)?;
            o.vol(Some(&m));
        }
        "rand" => {
            let mut p = t.i();
            let v = mymri::get_rand(&mut p);
            o.d(v);
            o.i(i64::from(p));
        }
        "subsample" => {
            let src = get_vol(cx, t)?;
            let (fix, randpos) = (t.i() != 0, t.i());
            let r = mymri::sub_sample(&mut cx.g, &mut cx.a, &mut cx.b, &src, None, fix, randpos)?;
            o.vol(r.into_new().as_ref());
        }
        "blur" => {
            let mut src = get_vol(cx, t)?;
            let same = t.i() != 0;
            let (g, a, e) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if same {
                let r = mymri::get_blur(g, a, e, Pair::Same(&mut src))?;
                o.i(i64::from(!r.is_null()));
                o.vol(if r.is_null() { None } else { Some(&src) });
            } else {
                let r = mymri::get_blur(g, a, e, Pair::New(&src))?;
                o.i(0);
                o.vol(r.into_new().as_ref());
            }
        }
        "partials" => {
            let src = get_vol(cx, t)?;
            let p = mymri::get_partials(&mut cx.g, &mut cx.a, &mut cx.b, &src)?;
            o.i(1);
            o.vol(Some(&p.fx));
            o.vol(Some(&p.fy));
            o.vol(p.fz.as_ref());
            o.vol(Some(&p.blur));
        }
        "rightsize" => {
            let src = get_vol(cx, t)?;
            let (cs, conform) = (t.f(), t.i() != 0);
            for v in mymri::find_right_size(&mut cx.g, &src, cs, conform) {
                o.i(i64::from(v));
            }
        }
        "bg" => {
            let src = get_vol(cx, t)?;
            let v = mymri::get_background(&mut cx.g, &mut cx.b, &src)?;
            o.f(v);
        }
        "ltx" => {
            let src = get_vol(cx, t)?;
            let given = t.i() != 0;
            let mut dst = if given { Some(get_vol(cx, t)?) } else { None };
            let m = get_m4(t);
            let (g, a, e) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = mymri::mri_linear_transform(g, a, e, &src, dst.as_mut(), &m)?;
            match r {
                Ret::Given => {
                    o.i(1);
                    o.vol(dst.as_ref());
                }
                Ret::New(m) => {
                    o.i(0);
                    o.vol(Some(&m));
                }
                Ret::Null => {
                    o.i(0);
                    o.vol(None);
                }
            }
        }
        "centroid" => {
            let mut src = get_vol(cx, t)?;
            src.outside_val = t.d();
            for v in costs::centroid(&mut cx.g, &mut cx.b, &src)? {
                o.d(v);
            }
        }
        "orient" => {
            let src = get_vol(cx, t)?;
            for row in costs::orientation(&mut cx.g, &mut cx.b, &src)? {
                for v in row {
                    o.d(v);
                }
            }
        }
        "v2m" => {
            let m = get_vm(t);
            let r = mx::convert_vnl2matrix(&mut cx.g, &mut cx.a, &mut cx.b, &m)?;
            o.m(Some(&r));
        }
        "vv2m" => {
            let n = t.i() as usize;
            let v: Vec<f64> = (0..n).map(|_| t.d()).collect();
            let r = mx::convert_vnl_vec2matrix(&mut cx.g, &mut cx.a, &mut cx.b, &v)?;
            o.m(Some(&r));
        }
        "m2v" => {
            let (r, c) = (t.i(), t.i());
            let mut m = matrix_alloc(&mut cx.g, &mut cx.a, &mut cx.b, r, c, MATRIX_REAL)?
                .expect("a matrix");
            for i in 1..=r {
                for j in 1..=c {
                    m.set_relt(i, j, t.f());
                }
            }
            o.vm(&mx::convert_matrix2vnl(&m));
        }
        "getvnl" => {
            let n = t.i() as usize;
            let d: Vec<f64> = (0..n).map(|_| t.d()).collect();
            let r = t.i() as usize;
            o.vm(&mx::get_vnl_matrix(&d, r));
        }
        "isdiag" => {
            let m = get_vm(t);
            let eps = t.d();
            o.i(i64::from(mx::is_diag(&m, eps)));
        }
        "polar" => {
            let a = get_vm(t);
            let (r, s) = mx::polar_decomposition(&mut cx.g, &mut cx.b, &a)?;
            o.vm(&r);
            o.vm(&s);
        }
        "polar2" => {
            let a = get_vm(t);
            let (r, s, d) = mx::polar2_decomposition(&mut cx.g, &mut cx.b, &a)?;
            o.vm(&r);
            o.vm(&s);
            for v in d {
                o.d(v);
            }
        }
        "adist" => {
            let a = VnlMatrix::from_4x4(&get_m4(t));
            let b = if t.i() != 0 {
                Some(VnlMatrix::from_4x4(&get_m4(t)))
            } else {
                None
            };
            let r = t.d();
            o.d(mx::affine_trans_dist_sq(&a, b.as_ref(), r));
        }
        "rdist" => {
            let a = VnlMatrix::from_4x4(&get_m4(t));
            let b = if t.i() != 0 {
                Some(VnlMatrix::from_4x4(&get_m4(t)))
            } else {
                None
            };
            o.d(mx::rigid_trans_dist_sq(
                &mut cx.g,
                &mut cx.b,
                &a,
                b.as_ref(),
            )?);
        }
        "fdiff" => {
            let a = get_vm(t);
            let mut b = VnlMatrix::zeros(a.rows, a.cols);
            for v in b.data.iter_mut() {
                *v = t.d();
            }
            o.d(mx::get_frobenius_diff(&a, &b));
        }
        "rlog" => {
            let m = get_m4(t);
            o.d(mx::rot_matrix_log_norm(&mut cx.b, &m));
        }
        "schur" => {
            let m = get_vm(t);
            let (u, tm) = mx::schur_complex(&mut cx.g, &mut cx.a, &mut cx.b, &m)?;
            o.cm(&u);
            o.cm(&tm);
        }
        "sqrtm" => {
            let m = get_vm(t);
            let r = mx::matrix_sqrt(&mut cx.g, &mut cx.a, &mut cx.b, &m)?;
            o.vm(&r);
        }
        "logm" => {
            let m = get_vm(t);
            let r = larmorx_freesurfer_robust::matrixlog::matrix_log(
                &mut cx.g, &mut cx.a, &mut cx.b, &m, 100,
            )?;
            o.vm(&r);
        }
        "expm" => {
            let m = get_vm(t);
            let r = larmorx_freesurfer_robust::matrixlog::matrix_exp(&mut cx.g, &mut cx.b, &m)?;
            o.vm(&r);
        }
        "geomean" => {
            let k = t.i() as usize;
            let vm: Vec<[[f64; 3]; 3]> = (0..k)
                .map(|_| {
                    let m = get_vm(t);
                    std::array::from_fn(|i| std::array::from_fn(|j| m.at(i, j)))
                })
                .collect();
            let r = larmorx_freesurfer_robust::matrixlog::geometric_mean3(
                &mut cx.g, &mut cx.a, &mut cx.b, &vm,
            )?;
            let m = VnlMatrix {
                rows: 3,
                cols: 3,
                data: r.iter().flatten().copied().collect(),
            };
            o.vm(&m);
        }
        "rgauss" => {
            if t.i() != 0 {
                rgauss::<f64>(cx, t, o);
            } else {
                rgauss::<f32>(cx, t, o);
            }
        }
        "regress" => {
            let mut svdnan = false;
            let r = if t.i() != 0 {
                regress::<f64>(cx, t, o, &mut svdnan)
            } else {
                regress::<f32>(cx, t, o, &mut svdnan)
            };
            if svdnan {
                cx.svdnan = true;
            }
            r?;
        }
        "trans" => trans_case(cx, t, o)?,
        "quat" => {
            let g = &mut cx.g;
            let op = t.next();
            let q = match op {
                "rv" => {
                    let (a, b, c) = (t.d(), t.d(), t.d());
                    Quaternion::import_rot_vec(g, a, b, c)
                }
                "rva" => {
                    let (al, a, b, c) = (t.d(), t.d(), t.d(), t.d());
                    Quaternion::import_rot_vec_angle(g, al, a, b, c)
                }
                _ => {
                    let (a, b, c) = (t.d(), t.d(), t.d());
                    Quaternion::import_zyx_angles(g, a, b, c)
                }
            };
            for v in [q.a, q.b, q.c, q.d] {
                o.d(v);
            }
            for v in q.rot_matrix3d() {
                o.d(v);
            }
        }
        "rstep" => {
            if t.i() != 0 {
                rstep::<f64>(cx, t, o)?;
            } else {
                rstep::<f32>(cx, t, o)?;
            }
        }
        "rreg" => rreg(cx, t, o)?,
        _ => panic!("unknown command {cmd}"),
    }
    Ok(())
}

// ---- the registration step's answers (3.2-03 part 1)

/// A number of the case's type.
fn tv<T: Real>(t: &mut Toks) -> T {
    if T::SINGLE {
        T::of_f32(t.f())
    } else {
        T::narrow(t.d())
    }
}

impl Out {
    fn r<T: Real>(&mut self, v: T) {
        self.0 += &v.hex();
        self.0 += " ";
    }
    fn rv<T: Real>(&mut self, v: &[T]) {
        self.i(v.len() as i64);
        for &x in v {
            self.r(x);
        }
    }
    fn m4(&mut self, m: &[[f64; 4]; 4]) {
        for row in m {
            for &v in row {
                self.d(v);
            }
        }
    }
}

fn rgauss<T: Real>(cx: &mut Ctx, t: &mut Toks, o: &mut Out) {
    let op = t.next();
    let n = t.i() as usize;
    let mut a: Vec<T> = (0..n).map(|_| tv::<T>(t)).collect();
    let g = &mut cx.g;
    match op {
        "median" => o.r(rg::median(g, &mut a)),
        "medianI" => {
            let (v, p) = rg::median_i(g, &mut a);
            o.r(v);
            o.r(p);
        }
        "mad" => {
            let d = tv::<T>(t);
            o.r(rg::mad(g, &mut a, d));
        }
        _ => {
            let k = t.i() as usize;
            match op {
                "kth" => o.r(rg::kth_smallest(&mut a, k)),
                "qsel" => o.r(rg::quick_select(&mut a, k)),
                "kthI" => {
                    let (v, p) = rg::kth_smallest_i(&mut a, k);
                    o.r(v);
                    o.i(i64::from(p));
                }
                _ => {
                    let (v, p) = rg::quick_select_i(&mut a, k);
                    o.r(v);
                    o.i(i64::from(p));
                }
            }
        }
    }
    for &x in &a {
        o.r(x);
    }
}

fn regress<T: Real>(
    cx: &mut Ctx,
    t: &mut Toks,
    o: &mut Out,
    svdnan: &mut bool,
) -> Result<(), Fatal> {
    let method = t.next();
    let (fsvd, verbose) = (t.i() != 0, t.i());
    let (sat, sig) = (t.d(), t.d());
    let (has_a, rows, cols) = (t.i() != 0, t.i() as usize, t.i() as usize);
    let mut a = Mat::<T>::zeros(0, 0);
    if has_a {
        a = Mat::zeros(rows, cols);
        for v in a.data.iter_mut() {
            *v = tv::<T>(t);
        }
    }
    let mut b: Vec<T> = (0..rows).map(|_| tv::<T>(t)).collect();
    let (g, out, err) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let (last, bout) = {
        let mut r = if has_a {
            Regression::new(&a, &mut b)
        } else {
            Regression::new_b(&mut b)
        };
        r.set_verbose(verbose);
        r.set_float_svd(fsvd);
        match method {
            "rob" => {
                let wsize = t.i() as usize;
                let mut w: Vec<T> = (0..wsize).map(|_| tv::<T>(t)).collect();
                let p = r.get_robust_est_w(g, out, err, &mut w, sat, sig)?;
                o.rv(&p);
                o.rv(&w);
            }
            "ls" => {
                let p = r.get_ls_est(g, err)?;
                o.rv(&p);
                if !T::SINGLE && has_a && !a.is_finite() {
                    // the double SVD's NaN bits are an open item (FS-968's): compared with every
                    // NaN read as one (check.py's NANBITS)
                    *svdnan = true;
                }
            }
            "wls" | "wlsf" => {
                let w: Vec<T> = (0..rows).map(|_| tv::<T>(t)).collect();
                let p = if method == "wls" {
                    r.get_weighted_ls_est(g, err, &w)?
                } else {
                    r.get_weighted_ls_est_float(g, err, &w)?
                };
                o.rv(&p);
            }
            "sigma" => {
                let d = tv::<T>(t);
                let bb = r.b().to_vec();
                o.r(get_sigma_mad(g, &bb, d));
            }
            _ => {
                let mut w = vec![T::ZERO; rows];
                let bb = r.b().to_vec();
                get_sqrt_tukey_dia_weights(&bb, &mut w, sat);
                o.rv(&w);
            }
        }
        ((r.lasterror, r.lastweight, r.lastzero), r.b().to_vec())
    };
    o.d(last.0);
    o.d(last.1);
    o.d(last.2);
    o.rv(&bout);
    Ok(())
}

fn trans_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let code = t.i() as usize;
    let ptype = t.i();
    let np = t.i() as usize;
    let mut tr = Transformation::new(Kind::ALL[code]);
    if ptype == 1 {
        let v: Vec<f32> = (0..np).map(|_| t.f()).collect();
        tr.set_parameters(&mut cx.g, &v, true);
    } else {
        let v: Vec<f64> = (0..np).map(|_| t.d()).collect();
        tr.set_parameters(&mut cx.g, &v, ptype == 0);
    }
    if t.i() != 0 {
        tr.set_identity();
    }
    let x = t.i() as u32;
    let fxv = t.f();
    let y = t.i() as u32;
    let fyv = t.f();
    let z = t.i() as u32;
    let fzv = t.f();
    o.i(tr.dof() as i64);
    o.rv(&tr.parameters);
    o.rv(&tr.steps());
    let m = tr.matrix(&mut cx.g, &mut cx.b)?;
    o.m4(&m);
    let gr = tr.gradient(&mut cx.b, x, fxv, y, fyv, z, fzv)?;
    o.rv(&gr);
    Ok(())
}

fn rstep<T: Real>(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let fsvd = t.i() != 0;
    let costfun = Cost::of(t.i()).expect("a cost function");
    let code = t.i() as usize;
    let sat = t.d();
    let iscale = t.i() != 0;
    let ss = t.i();
    let verbose = t.i();
    let calls = t.i();
    let params = StepParams {
        sat,
        iscale,
        transonly: false,
        rigid: true,
        isoscale: false,
        costfun,
        subsamplesize: ss,
        debug: 0,
        verbose,
        iscalefinal: 1.0,
    };
    let mut trans = Transformation::new(Kind::ALL[code]);
    let mut step = RegistrationStep::<T>::new(params);
    step.set_float_svd(fsvd);
    for _ in 0..calls {
        let mut ms = get_vol(cx, t)?;
        let mut mt = get_vol(cx, t)?;
        let (g, out, err) = (&mut cx.g, &mut cx.a, &mut cx.b);
        let (m, second) =
            step.compute_registration_step(g, out, err, Some(&mut trans), &mut ms, &mut mt)?;
        o.m4(&m);
        o.d(second);
        if costfun == Cost::Rob {
            o.d(step.wcheck);
            o.d(step.wchecksqrt);
        }
        o.d(step.zeroweights);
        o.rv(&step.pvec);
        o.vol(step.mri_weights.as_ref());
        o.vol(step.mri_indexing.as_ref());
    }
    Ok(())
}

// ---- the registration's answers (3.2-03 part 2)

/// `fsrobust.cpp`'s `get_blob`.
fn get_blob(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let (w, h, d, ty) = (t.i(), t.i(), t.i(), t.i());
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let mut m = mm::mri_alloc(g, a, b, w, h, d, ty)?;
    let v = &mut m.vg;
    v.xsize = t.f();
    v.ysize = t.f();
    v.zsize = t.f();
    v.x_r = t.f();
    v.x_a = t.f();
    v.x_s = t.f();
    v.y_r = t.f();
    v.y_a = t.f();
    v.y_s = t.f();
    v.z_r = t.f();
    v.z_a = t.f();
    v.z_s = t.f();
    v.c_r = t.f();
    v.c_a = t.f();
    v.c_s = t.f();
    v.ras_good_flag = 1;
    reinit_cache(g, a, b, &mut m.vg)?;
    let nb = t.i() as usize;
    let bl: Vec<f64> = (0..5 * nb).map(|_| t.d()).collect();
    let (bg, tex) = (t.d(), t.d());
    let am: Vec<f64> = (0..12).map(|_| t.d()).collect();
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let (xf, yf, zf) = (f64::from(x), f64::from(y), f64::from(z));
                let px = am[0] * xf + am[1] * yf + am[2] * zf + am[3];
                let py = am[4] * xf + am[5] * yf + am[6] * zf + am[7];
                let pz = am[8] * xf + am[9] * yf + am[10] * zf + am[11];
                let mut v = bg;
                for k in 0..nb {
                    let (dx_, dy_, dz_) = (px - bl[5 * k], py - bl[5 * k + 1], pz - bl[5 * k + 2]);
                    let d2 = dx_ * dx_ + dy_ * dy_ + dz_ * dz_;
                    v += bl[5 * k + 3] * bl[5 * k + 4] / (bl[5 * k + 4] + d2);
                }
                let ti = ((px * 3.0) as i32 + (py * 5.0) as i32 + (pz * 7.0) as i32) % 7;
                v += tex * f64::from(ti);
                if ty != mm::MRI_FLOAT {
                    let hi = if ty == mm::MRI_UCHAR { 255.0 } else { 32767.0 };
                    v = if v < 0.0 {
                        0.0
                    } else if v > hi {
                        hi
                    } else {
                        v
                    };
                    v = f64::from((v + 0.5) as i32);
                }
                mri_set_vox_val(g, b, &mut m, x, y, z, 0, v as f32, 0)?;
            }
        }
    }
    Ok(m)
}

impl Out {
    /// `fsrobust.cpp`'s `put_vol_hash`.
    fn vol_hash(&mut self, m: Option<&Mri>) {
        let Some(m) = m else {
            self.0 += "NULL ";
            return;
        };
        for v in [m.vg.width, m.vg.height, m.vg.depth, m.nframes, m.mtype] {
            self.i(i64::from(v));
        }
        self.f(m.vg.xsize);
        self.f(m.vg.ysize);
        self.f(m.vg.zsize);
        self.d(m.outside_val);
        self.am(m.vg.i_to_r.as_ref());
        match &m.chunk {
            Some(c) => {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for &x in c {
                    h ^= u64::from(x);
                    h = h.wrapping_mul(0x0100_0000_01b3);
                }
                self.0 += &format!("fnv {h:016x} {} ", c.len());
            }
            None => self.0 += "nochunk ",
        }
    }
    /// `put_vm4`: a 4 x 4 `vnl_matrix`, `EMPTY` for another size.
    fn vm4(&mut self, m: Option<&VnlMatrix>) {
        match m {
            Some(m) if m.rows == 4 && m.cols == 4 => {
                for i in 0..4 {
                    for j in 0..4 {
                        self.d(m.at(i, j));
                    }
                }
            }
            _ => self.0 += "EMPTY ",
        }
    }
}

fn rreg(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    use larmorx_freesurfer_core::run::Environment;
    use larmorx_freesurfer_robust::registration::{Io, RegRobust};
    let (dbl, model, iscale, sym) = (t.i() != 0, t.i(), t.i() != 0, t.i() != 0);
    let (sat, wlimit) = (t.d(), t.d());
    let (satit, nomulti, iterate) = (t.i() != 0, t.i() != 0, t.i());
    let epsit = t.d();
    let (highit, minsize, maxsize, ss) = (t.i(), t.i(), t.i(), t.i());
    let (inittrans, initorient, initscaling, verbose) = (t.i() != 0, t.i() != 0, t.i() != 0, t.i());
    let keeptype = t.i() != 0;
    let minit = if t.i() != 0 {
        let mut m = VnlMatrix::zeros(4, 4);
        for r in 0..4 {
            for c in 0..4 {
                *m.at_mut(r, c) = t.d();
            }
        }
        Some(m)
    } else {
        None
    };
    let iscaleinit = t.d();
    let mov = get_blob(cx, t)?;
    let dst = get_blob(cx, t)?;
    let mut r = RegRobust::new();
    if model != 1 {
        r.set_rigid();
    } else {
        r.set_affine();
    }
    if model == 2 {
        r.set_transonly();
    }
    if model == 3 {
        r.set_isoscale();
    }
    r.iscale = iscale;
    if model == 4 {
        r.set_iscale_only();
    }
    r.set_verbose(verbose);
    r.set_debug(0);
    r.highit = highit;
    r.inittransform = inittrans;
    r.initorient = initorient;
    r.initscaling = initscaling;
    r.doubleprec = dbl;
    r.symmetry = sym;
    r.costfun = Cost::Rob;
    r.sat = sat;
    r.wlimit = wlimit;
    r.set_name("reg");
    if ss > 0 {
        r.subsamplesize = ss;
    }
    r.minsize = minsize;
    r.maxsize = maxsize;
    if minit.is_some() {
        r.minit = minit;
    }
    r.iscaleinit = iscaleinit;
    let env = Environment::from_pairs(Vec::<(String, String)>::new());
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let mut io = Io {
        out: a,
        err: b,
        env: &env,
        dir: None,
        input: None,
    };
    r.set_source_and_target(g, &mut io, &mov, &dst, keeptype)?;
    drop(mov);
    drop(dst);
    o.vol_hash(r.mri_source.as_ref());
    o.vol_hash(r.mri_target.as_ref());
    o.vm4(r.rsrc.as_ref());
    o.vm4(r.rtrg.as_ref());
    o.i(i64::from(r.resample));
    if satit {
        o.d(r.find_saturation(g, &mut io)?);
        r.compute_multires_registration(g, &mut io, 0, iterate, epsit)?;
    } else if nomulti {
        r.compute_iterative_registration(g, &mut io, iterate, epsit)?;
    } else {
        r.compute_multires_registration(g, &mut io, 0, iterate, epsit)?;
    }
    o.m4(&r.get_final_vox2vox(io.out));
    o.d(r.get_final_iscale());
    o.i(i64::from(r.get_converged()));
    o.d(r.sat);
    if r.mov2weights.is_some() {
        o.d(r.wcheck);
    } else {
        o.0 += "nowcheck ";
    }
    o.vm4(r.mfinal.as_ref());
    if r.mov2weights.is_some() {
        let (h1, h2) = r.get_half_way_maps(g, io.err)?;
        o.m4(&h1);
        o.m4(&h2);
    } else {
        o.0 += "nohalfway ";
    }
    o.vol_hash(r.mri_weights.as_ref());
    o.vol_hash(r.mri_hweights.as_ref());
    if r.centroid_s.len() == 3 && r.centroid_t.len() == 3 {
        for v in r.get_centroid_s() {
            o.d(v);
        }
        for v in r.get_centroid_t() {
            o.d(v);
        }
        for v in r.get_centroid_s_in_t(io.out) {
            o.d(v);
        }
    } else {
        o.0 += "nocentroids ";
    }
    Ok(())
}

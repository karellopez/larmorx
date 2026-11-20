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
        _ => panic!("unknown command {cmd}"),
    }
    Ok(())
}

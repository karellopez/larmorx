// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.mri probe's cases (`validation/probes/fs-mri/`): a seeded generator of case lines, and
//! the port's answer to each, printed as `fsmri` prints FreeSurfer's (`<result> | <text>`, the text
//! FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program).
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::affine::AffineMatrix;
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry, ctab_deep_copy, ctab_free};
use larmorx_freesurfer_mri::mri::{self as m, Mri};
use larmorx_freesurfer_mri::tkreg as tk;
use larmorx_freesurfer_mri::vol_geom::{self as vgm, VolGeom};
use larmorx_freesurfer_mri::voxel;

/// The cases' random numbers; `finite` keeps the special values (NaN, infinities, 1e30) out of
/// the numbers drawn.
pub struct Gen(u64, bool);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed, false)
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
}

const SPECIAL: [f32; 6] = [
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    1e30,
    -0.0,
    1e-40,
];

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn num(g: &mut Gen, lo: i64, hi: i64, d: i64) -> f32 {
    if !g.1 && g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.int(lo * d, hi * d) as f32 / d as f32
}

fn cosine(g: &mut Gen) -> f32 {
    if !g.1 && g.chance(60) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[-1.0, 0.0, 1.0, 0.6, 0.8, -0.6, -0.8, 0.70710677, 0.1, 0.995])
}

fn size(g: &mut Gen) -> f32 {
    if !g.1 && g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[1.0, 0.5, 2.0, 1.25, 0.9375, 3.0, 0.0, -1.0, 1.1, 0.8])
}

/// `set_geom`'s tokens.
fn geom(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.int(0, 1).to_string());
    for _ in 0..3 {
        t.push(g.int(1, 6).to_string());
    }
    for _ in 0..3 {
        t.push(fx(size(g)));
    }
    t.push(g.int(0, 1).to_string());
    for _ in 0..9 {
        t.push(fx(cosine(g)));
    }
    for _ in 0..3 {
        t.push(fx(num(g, -120, 120, 8)));
    }
    for _ in 0..3 {
        let s = if g.chance(3) {
            g.pick(&[0.1, 1e-6, 2e-5, -0.3, 0.5])
        } else {
            0.0
        };
        t.push(fx(s));
    }
    t.push(g.pick(&["-", "vol.mgz", "orig/001.mgz"]).to_owned());
}

/// `get_mri`'s tokens: a small volume.
fn vol(g: &mut Gen, t: &mut Vec<String>, odd: bool) {
    let types = [0, 1, 2, 3, 4, 6, 7, 9, 10];
    let (lo, nlo) = if odd { (-1, -1) } else { (1, 1) };
    for _ in 0..3 {
        let v = if odd && g.chance(6) {
            g.int(lo, 0)
        } else {
            g.int(1, 4)
        };
        t.push(v.to_string());
    }
    let ty = if odd && g.chance(5) {
        g.pick(&[5, 8, 11, -1])
    } else {
        g.pick(&types)
    };
    t.push(ty.to_string());
    let nf = if odd && g.chance(6) {
        g.int(nlo, 0)
    } else {
        g.int(1, 2)
    };
    t.push(nf.to_string());
    t.push(i64::from(g.chance(5)).to_string());
}

fn matrix(g: &mut Gen, t: &mut Vec<String>) {
    t.push("4".into());
    t.push("4".into());
    for _ in 0..16 {
        t.push(fx(num(g, -50, 50, 4)));
    }
}

fn opt_matrix(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(2) {
        t.push("1".into());
        matrix(g, t);
    } else {
        t.push("0".into());
    }
}

fn word(g: &mut Gen) -> String {
    g.pick(&["-", "bert", "T1", "aseg", "mgz", "a_b"])
        .to_owned()
}

fn ctab(g: &mut Gen, t: &mut Vec<String>, depth: u32) {
    let n = g.int(0, 4);
    t.push(n.to_string());
    t.push(word(g));
    t.push(g.int(0, 3).to_string());
    t.push(g.pick(&[0, 0, 1, 1000]).to_string());
    t.push(word(g));
    for _ in 0..n {
        if g.chance(4) {
            t.push("0".into());
            continue;
        }
        t.push("1".into());
        t.push(word(g));
        for _ in 0..4 {
            t.push(g.int(0, 255).to_string());
        }
        for _ in 0..4 {
            t.push(fx(g.int(0, 255) as f32 / 255.0));
        }
        t.push(g.int(0, 5).to_string());
        t.push(g.pick(&[0, 0, 17]).to_string());
    }
    if depth == 0 {
        if g.chance(3) {
            t.push("1".into());
            ctab(g, t, 1);
        } else {
            t.push("0".into());
        }
    }
}

fn header(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.int(0, 2).to_string());
    t.push(g.int(-1, 5).to_string());
    t.push(g.int(0, 3).to_string());
    t.push(dx(f64::from(num(g, -10, 10, 4))));
    for _ in 0..3 {
        t.push(fx(size(g)));
    }
    for _ in 0..3 {
        t.push(g.int(0, 4).to_string());
    }
    for _ in 0..10 {
        t.push(fx(num(g, -20, 20, 2)));
    }
    t.push(dx(f64::from(num(g, 0, 3, 8))));
    for _ in 0..3 {
        t.push(fx(num(g, 0, 3000, 1)));
    }
    t.push(word(g));
    for _ in 0..9 {
        t.push(fx(cosine(g)));
    }
    for _ in 0..3 {
        t.push(fx(num(g, -120, 120, 8)));
    }
    t.push(g.int(0, 1).to_string());
    t.push(g.int(0, 2).to_string());
    for _ in 0..5 {
        t.push(word(g));
    }
    let n = g.int(0, 3);
    t.push(n.to_string());
    for _ in 0..n {
        t.push(word(g).replace('-', "x"));
    }
    opt_matrix(g, t);
    opt_matrix(g, t);
    opt_matrix(g, t);
    if g.chance(3) {
        t.push("1".into());
        ctab(g, t, 0);
    } else {
        t.push("0".into());
    }
}

fn voxval(g: &mut Gen) -> f32 {
    if g.chance(6) {
        return g.pick(&[
            3e9,
            -3e9,
            2147483520.0,
            2.1474836e9,
            70000.0,
            -40000.0,
            1e19,
        ]);
    }
    if g.chance(10) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[
        300.0, -5.0, 2.5, -2.5, 0.49999997, 255.5, 65535.4, 7.0, -32768.6,
    ])
}

fn vox(g: &mut Gen, t: &mut Vec<String>) {
    let (w, h, d) = (g.int(1, 3), g.int(1, 3), g.int(1, 2));
    let ty = g.pick(&[0, 1, 2, 3, 4, 6, 7, 9, 10]);
    let nf = g.int(1, 2);
    let hdr = g.chance(25);
    for v in [w, h, d, ty, nf, i64::from(hdr)] {
        t.push(v.to_string());
    }
    t.push(g.int(0, 250).to_string());
    t.push(dx(g.pick(&[0.0, -1.0, 7.5])));
    let nops = g.int(1, 4);
    t.push(nops.to_string());
    for _ in 0..nops {
        let op = g.pick(&["set", "set", "get", "get", "dx", "dy", "dz"]);
        t.push(op.into());
        let far = g.chance(12);
        let c = if far {
            w * h * d * nf + g.int(0, 3)
        } else {
            g.int(-1, w)
        };
        let r = g.int(-1, h);
        let s = g.int(-1, d);
        let f = if g.chance(4) {
            g.int(-1, nf)
        } else {
            g.int(0, nf - 1)
        };
        for v in [c, r, s, f] {
            t.push(v.to_string());
        }
        let mode = if g.chance(20) { 2 } else { g.int(0, 1) };
        t.push(mode.to_string());
        if op == "set" {
            t.push(fx(voxval(g)));
        }
    }
}

/// A registration-like matrix: mostly 4 x 4 (small entries, sometimes singular), now and then
/// of another size.
fn reg_matrix(g: &mut Gen, t: &mut Vec<String>) {
    let (rows, cols) = if g.chance(12) {
        g.pick(&[(3, 4), (4, 3), (4, 1), (1, 1)])
    } else {
        (4, 4)
    };
    t.push(rows.to_string());
    t.push(cols.to_string());
    let zero = g.chance(15);
    for r in 1..=rows {
        for c in 1..=cols {
            let v = if zero {
                0.0
            } else if r == 4 && rows == 4 && cols == 4 {
                if c == 4 { 1.0 } else { 0.0 }
            } else if r == c && !g.chance(4) {
                g.pick(&[1.0, 0.98, 1.02, -1.0, 2.0])
            } else {
                num(g, -40, 40, 8)
            };
            t.push(fx(v));
        }
    }
}

fn opt_reg_matrix(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(6) {
        t.push("0".into());
    } else {
        t.push("1".into());
        reg_matrix(g, t);
    }
}

fn point(g: &mut Gen) -> f64 {
    if g.chance(30) {
        return f64::from(g.pick(&SPECIAL));
    }
    g.int(-2000, 2000) as f64 / 16.0
}

/// `vgeq`'s second geometry: the first one's tokens, mostly with one field changed.
fn perturbed(g: &mut Gen, a: &[String]) -> Vec<String> {
    let mut b = a.to_vec();
    if g.chance(4) {
        return b;
    }
    let i = g.below(23) as usize;
    match i {
        0..=3 | 7 => b[i] = (b[i].parse::<i64>().unwrap() + 1).to_string(),
        _ => {
            let v = f32::from_bits(u32::from_str_radix(&b[i], 16).unwrap());
            let d = g.pick(&[1e-8f32, 1e-7, 1e-6, 1e-4, 1e-3, 0.01, 1.0]);
            let w = if g.chance(20) {
                f32::NAN
            } else if g.chance(2) {
                v + d
            } else {
                v - d
            };
            b[i] = fx(w);
        }
    }
    b
}

/// `n` case lines from `seed`.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let cmds = [
        "alloc",
        "allocvg",
        "fromvg",
        "xfm",
        "setv2r",
        "p0",
        "reinit",
        "vox",
        "vox",
        "vox",
        "copyhdr",
        "pulse",
        "ctab",
        "vgcopy",
        "vgassign",
        "usevg",
        "vgtomri",
        "vgfrommri",
        "intent",
        "sizeof",
        "tkreg",
        "ras2s",
        "tk2nat",
        "nat2tk",
        "v2v",
        "tkv2v",
        "fixtk",
        "fsl2tk",
        "tk2fsl",
        "crs1",
        "sphinx",
        "v2w",
        "v2w",
        "codes",
        "vgprint",
        "vgeq",
        "vgeq",
        "shearless",
        "tomat",
        "copy",
        "copy",
        "copy",
        "shape",
    ];
    (0..n)
        .map(|i| {
            let cmd = cmds[i % cmds.len()];
            let mut t = vec![cmd.to_owned()];
            let g = &mut g;
            match cmd {
                "alloc" => vol(g, &mut t, true),
                "allocvg" | "fromvg" => {
                    geom(g, &mut t);
                    t.push(g.pick(&[0, 1, 2, 3, 4, 6, 7, 9, 10]).to_string());
                    t.push(g.int(1, 2).to_string());
                    t.push(g.int(0, 1).to_string());
                }
                "xfm" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                    t.push(g.int(0, 1).to_string());
                }
                "setv2r" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    matrix(g, &mut t);
                }
                "p0" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    for _ in 0..3 {
                        t.push(dx(f64::from(num(g, -150, 150, 16))));
                    }
                }
                "reinit" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                }
                "vox" => vox(g, &mut t),
                "copyhdr" => {
                    vol(g, &mut t, false);
                    header(g, &mut t);
                    if g.chance(2) {
                        t.push("1".into());
                        vol(g, &mut t, false);
                        header(g, &mut t);
                    } else {
                        t.push("0".into());
                    }
                }
                "pulse" => {
                    t.push(dx(f64::from(num(g, 0, 3, 8))));
                    for _ in 0..3 {
                        t.push(fx(num(g, 0, 3000, 1)));
                    }
                }
                "ctab" => ctab(g, &mut t, 0),
                "vgcopy" => {
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                }
                "vgassign" => {
                    geom(g, &mut t);
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                }
                "usevg" | "vgtomri" => {
                    geom(g, &mut t);
                    vol(g, &mut t, false);
                }
                "vgfrommri" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    geom(g, &mut t);
                }
                "intent" => {
                    t.push(g.int(-2, 6).to_string());
                    t.push(
                        g.pick(&[
                            "MRI",
                            "LABEL",
                            "SHAPE",
                            "WARP",
                            "WARP_INV",
                            "TIME_SERIES",
                            "x",
                        ])
                        .to_owned(),
                    );
                }
                "tkreg" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                    t.push(g.int(0, 1).to_string());
                }
                "ras2s" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                }
                "tk2nat" | "nat2tk" | "v2v" | "tkv2v" => {
                    // MRItkReg2Native's failed inverse in place prints VXL's float inverse's NaNs,
                    // whose bits the port does not reproduce yet (larmorx_core's vnl_fixed): its
                    // cases are drawn finite
                    g.1 = cmd == "tk2nat";
                    for _ in 0..2 {
                        vol(g, &mut t, false);
                        geom(g, &mut t);
                    }
                    opt_reg_matrix(g, &mut t);
                    g.1 = false;
                }
                "fixtk" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 100_000).to_string());
                    opt_reg_matrix(g, &mut t);
                }
                "fsl2tk" | "tk2fsl" => {
                    // the NIFTI branch prints VXL's float determinants: drawn finite, as tk2nat
                    g.1 = true;
                    t.push(
                        g.pick(&[
                            "-",
                            "NIFTI",
                            "NIFTI_GZ",
                            "NIFTI_PAIR",
                            "ANALYZE",
                            "NIFTI_GZ",
                        ])
                        .to_owned(),
                    );
                    for _ in 0..2 {
                        vol(g, &mut t, false);
                        geom(g, &mut t);
                    }
                    opt_reg_matrix(g, &mut t);
                    g.1 = false;
                }
                "crs1" => {
                    let k = g.int(0, 2);
                    t.push(k.to_string());
                    if k > 0 {
                        let (r, c) = if k == 1 {
                            (4, 4)
                        } else {
                            g.pick(&[(3, 4), (4, 2)])
                        };
                        t.push(r.to_string());
                        t.push(c.to_string());
                        for _ in 0..r * c {
                            t.push(fx(num(g, -5, 5, 2)));
                        }
                    }
                }
                "sphinx" => geom(g, &mut t),
                "v2w" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                    t.push(g.int(0, 2).to_string());
                    let n = g.int(1, 3);
                    t.push(n.to_string());
                    for _ in 0..n {
                        for _ in 0..3 {
                            t.push(dx(point(g)));
                        }
                        t.push(g.int(0, 1).to_string());
                    }
                }
                "codes" => {
                    t.push(
                        g.pick(&[
                            "nearest",
                            "NEA",
                            "ne",
                            "tri",
                            "TLI",
                            "trilinear",
                            "sinc",
                            "Sin",
                            "cubic",
                            "cub",
                            "x",
                            "nearestx",
                            "cu",
                        ])
                        .to_owned(),
                    );
                    t.push(g.int(-1, 7).to_string());
                    t.push(
                        g.pick(&[
                            "uchar", "UCHAR", "short", "ushrt", "int", "long", "float", "double",
                            "uchar2", "Float", "in",
                        ])
                        .to_owned(),
                    );
                    t.push(g.int(-1, 11).to_string());
                }
                "vgprint" => {
                    geom(g, &mut t);
                    t.push(g.int(0, 1).to_string());
                    t.push(g.int(0, 1).to_string());
                }
                "vgeq" => {
                    let mut a = Vec::new();
                    geom(g, &mut a);
                    let b = perturbed(g, &a);
                    t.extend(a);
                    t.extend(b);
                    t.push(dx(g.pick(&[
                        f64::from(f32::EPSILON),
                        1e-3,
                        0.0,
                        1e-6,
                        10.0,
                    ])));
                    t.push(g.int(0, 1).to_string());
                    opt_matrix(g, &mut t);
                }
                "shearless" => {
                    vol(g, &mut t, false);
                    geom(g, &mut t);
                }
                "tomat" => {
                    geom(g, &mut t);
                    if g.chance(2) {
                        t.push("1".into());
                        matrix(g, &mut t);
                    } else {
                        t.push("0".into());
                    }
                }
                "copy" => {
                    let types = [0, 1, 2, 3, 4, 6, 7, 9, 10];
                    let (w, h, d, nf) = (g.int(1, 3), g.int(1, 3), g.int(1, 2), g.int(1, 2));
                    let st = g.pick(&types);
                    for v in [w, h, d, st, nf, i64::from(g.chance(8))] {
                        t.push(v.to_string());
                    }
                    t.push(g.int(0, 250).to_string());
                    header(g, &mut t);
                    if g.chance(4) {
                        t.push("0".into());
                    } else {
                        t.push("1".into());
                        let dt = if g.chance(3) { st } else { g.pick(&types) };
                        let ext = if g.chance(6) {
                            [g.int(1, 4), g.int(1, 4), g.int(1, 3), g.int(1, 3)]
                        } else {
                            [w, h, d, nf]
                        };
                        for v in [ext[0], ext[1], ext[2], dt, ext[3], i64::from(g.chance(20))] {
                            t.push(v.to_string());
                        }
                        t.push(g.int(0, 250).to_string());
                        t.push(g.int(0, 3).to_string());
                    }
                }
                "shape" => {
                    let a: Vec<i64> = (0..4).map(|_| g.int(0, 3)).collect();
                    let b: Vec<i64> = if g.chance(2) {
                        a.clone()
                    } else {
                        (0..4).map(|_| g.int(0, 3)).collect()
                    };
                    for v in a.iter().chain(b.iter()) {
                        t.push(v.to_string());
                    }
                }
                _ => t.push(g.int(-1, 11).to_string()),
            }
            t.join(" ")
        })
        .collect()
}

// ------------------------------------------------------------------ the port's side

/// One buffer both streams write into, in order (FreeSurfer's standard output and error go to
/// one file in `fsmri`).
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
    fn s(&mut self, v: &str) {
        self.0 += if v.is_empty() { "-" } else { v };
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
    fn geom(&mut self, v: &VolGeom, with_valid: bool) {
        if with_valid {
            self.i(i64::from(v.valid));
        }
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
        self.s(&v.fname.to_string_lossy());
        self.am(v.i_to_r.as_ref());
        self.m(v.r_to_i.as_ref());
        self.m(v.register_mat.as_ref());
    }
    fn header(&mut self, x: &Mri) {
        for v in [x.vg.width, x.vg.height, x.vg.depth, x.nframes, x.mtype] {
            self.i(i64::from(v));
        }
        self.i(x.bytes_per_vox as i64);
        for v in [
            x.bytes_total,
            x.vox_per_row,
            x.vox_per_slice,
            x.vox_per_vol,
            x.vox_total,
        ] {
            self.i(v as i64);
        }
        self.i(i64::from(x.imnr0));
        self.i(i64::from(x.imnr1));
        for v in [
            x.xstart, x.ystart, x.zstart, x.xend, x.yend, x.zend, x.fov, x.thick, x.ps,
        ] {
            self.f(v);
        }
        self.i(i64::from(x.scale));
        for v in [x.roi.dx, x.roi.dy, x.roi.dz] {
            self.i(i64::from(v));
        }
        for v in [x.version, x.intent, x.dof, x.brightness, x.yinvert, x.ptype] {
            self.i(i64::from(v));
        }
        self.d(x.outside_val);
        self.d(x.flip_angle);
        for v in [x.tr, x.te, x.ti, x.location] {
            self.f(v);
        }
        self.i(x.cmdlines.len() as i64);
        for c in &x.cmdlines {
            self.s(c);
        }
        for v in [
            &x.subject_name,
            &x.path_to_t1,
            &x.fname_format,
            &x.gdf_image_stem,
            &x.transform_fname,
        ] {
            self.s(v);
        }
        self.m(x.auto_align.as_ref());
        self.m(x.orig_ras2vox.as_ref());
        self.i(i64::from(x.free_transform));
        self.geom(&x.vg, false);
    }
    fn indices(&mut self, x: &Mri) {
        let (Some(xi), Some(yi), Some(zi)) = (&x.xi, &x.yi, &x.zi) else {
            self.0 += "noindex ";
            return;
        };
        for (t, lim) in [(xi, x.vg.width), (yi, x.vg.height), (zi, x.vg.depth)] {
            for i in [-500, -2, -1, 0, 1, lim - 1, lim, lim + 1, lim + 499] {
                self.i(i64::from(Mri::index(t, i)));
            }
        }
    }
    fn frames(&mut self, x: &Mri) {
        for fr in &x.frames {
            self.i(i64::from(fr.ftype));
            self.f(fr.te);
            self.f(fr.tr);
            self.i(i64::from(fr.label));
            self.i(i64::from(fr.dof));
            self.f(fr.thresh);
            self.m(fr.m_ras2vox.as_ref());
        }
    }
    fn chunk(&mut self, x: &Mri) {
        match &x.chunk {
            Some(c) if x.ischunked => {
                self.0 += "chunk:";
                for b in c {
                    self.0 += &format!("{b:02x}");
                }
                self.0 += " ";
            }
            _ => self.0 += "nochunk ",
        }
    }
    fn ctab(&mut self, ct: Option<&ColorTable>) {
        let Some(ct) = ct else {
            self.0 += "NULL ";
            return;
        };
        self.i(i64::from(ct.nentries()));
        self.s(&String::from_utf8_lossy(&ct.fname));
        self.i(i64::from(ct.version));
        self.i(i64::from(ct.idbase));
        self.s(&ct.tissue_type_schema);
        for e in &ct.entries {
            match e {
                None => self.0 += "0 ",
                Some(e) => {
                    self.0 += "1 ";
                    self.s(&String::from_utf8_lossy(&e.name));
                    for v in [e.ri, e.gi, e.bi, e.ai] {
                        self.i(i64::from(v));
                    }
                    for v in [e.rf, e.gf, e.bf, e.af] {
                        self.f(v);
                    }
                    self.i(i64::from(e.tissue_type));
                }
            }
        }
        self.ctab(ct.ctab_tissue_type.as_deref());
    }
}

struct Ctx {
    g: Globals,
    a: Shared,
    b: Shared,
}

impl Ctx {
    fn set_geom(&mut self, t: &mut Toks, v: &mut VolGeom) {
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
    fn mri(&mut self, t: &mut Toks) -> Result<Mri, Fatal> {
        let (w, h, d, ty, nf, hdr) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
        let (g, a, b) = (&mut self.g, &mut self.a, &mut self.b);
        if hdr != 0 {
            m::mri_alloc_header(g, a, b, w, h, d, ty, nf)
        } else {
            m::mri_alloc_sequence(g, a, b, w, h, d, ty, nf)
        }
    }
    fn matrix(&mut self, t: &mut Toks) -> Matrix {
        let (rows, cols) = (t.i(), t.i());
        let mut x = Matrix::real(rows, cols);
        for r in 1..=rows {
            for c in 1..=cols {
                x.set_relt(r, c, t.f());
            }
        }
        x
    }
    fn opt_matrix(&mut self, t: &mut Toks) -> Option<Matrix> {
        (t.i() != 0).then(|| self.matrix(t))
    }
    fn ctab(&mut self, t: &mut Toks, depth: u32) -> ColorTable {
        let n = t.i();
        let mut ct = ColorTable {
            fname: t.s().into_bytes(),
            version: t.i(),
            idbase: t.i(),
            tissue_type_schema: t.s(),
            ..ColorTable::default()
        };
        for _ in 0..n {
            if t.i() == 0 {
                ct.entries.push(None);
                continue;
            }
            ct.entries.push(Some(Box::new(ColorTableEntry {
                name: t.s().into_bytes(),
                ri: t.i(),
                gi: t.i(),
                bi: t.i(),
                ai: t.i(),
                rf: t.f(),
                gf: t.f(),
                bf: t.f(),
                af: t.f(),
                tissue_type: t.i(),
                count: t.i(),
            })));
        }
        if depth == 0 && t.i() != 0 {
            ct.ctab_tissue_type = Some(Box::new(self.ctab(t, 1)));
        }
        ct
    }
    fn header(&mut self, t: &mut Toks, x: &mut Mri) {
        x.version = t.i();
        x.intent = t.i();
        x.dof = t.i();
        x.mean = t.d();
        x.vg.xsize = t.f();
        x.vg.ysize = t.f();
        x.vg.zsize = t.f();
        x.imnr0 = t.i();
        x.imnr1 = t.i();
        x.ptype = t.i();
        x.fov = t.f();
        x.thick = t.f();
        x.ps = t.f();
        x.location = t.f();
        x.xstart = t.f();
        x.xend = t.f();
        x.ystart = t.f();
        x.yend = t.f();
        x.zstart = t.f();
        x.zend = t.f();
        x.flip_angle = t.d();
        x.tr = t.f();
        x.te = t.f();
        x.ti = t.f();
        x.vg.fname = CName::new(t.s().as_bytes());
        let v = &mut x.vg;
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
        v.ras_good_flag = t.i();
        x.brightness = t.i();
        x.subject_name = t.s();
        x.path_to_t1 = t.s();
        x.fname_format = t.s();
        x.gdf_image_stem = t.s();
        x.transform_fname = t.s();
        let n = t.i();
        x.cmdlines = (0..n).map(|_| t.s()).collect();
        if let Some(r) = self.opt_matrix(t) {
            x.vg.register_mat = Some(r);
        }
        if let Some(a) = self.opt_matrix(t) {
            x.auto_align = Some(a);
        }
        if let Some(o) = self.opt_matrix(t) {
            x.orig_ras2vox = Some(o);
        }
        if t.i() != 0 {
            x.ct = Some(self.ctab(t, 0));
        }
    }
}

/// `fill_chunk`: byte `i` of the voxels `(37 i + seed) mod 256`.
fn fill(x: &mut Mri, seed: u32) {
    if let (Some(c), true) = (x.chunk.as_mut(), x.ischunked) {
        for (i, b) in c.iter_mut().enumerate() {
            *b = ((i as u32).wrapping_mul(37).wrapping_add(seed) & 0xff) as u8;
        }
    }
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    match t.next() {
        "alloc" => {
            let x = cx.mri(t)?;
            o.header(&x);
            o.indices(&x);
            o.frames(&x);
            o.chunk(&x);
            let mut x = Some(x);
            m::mri_free(&mut cx.g, &mut cx.b, &mut x);
            o.i(i64::from(x.is_none()));
        }
        "allocvg" | "fromvg" => {
            let fromvg = t.t[0] == "fromvg";
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            let (ty, nf, hdr) = (t.i(), t.i(), t.i() != 0);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let x = if fromvg {
                m::mri_alloc_from_vol_geom(g, a, b, &v, ty, nf, hdr)?
            } else {
                Mri::from_vol_geom(g, a, b, &v, ty, nf, hdr)?
            };
            o.header(&x);
            if !fromvg {
                o.indices(&x);
                o.frames(&x);
            }
            o.chunk(&x);
        }
        "xfm" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let (base, shear) = (t.i(), t.i() != 0);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let v = &x.vg;
            let r = vgm::xfm_crs2xyz(g, a, b, v, base, shear)?;
            o.m(Some(&r));
            for k in 0..3 {
                let r = match k {
                    0 => vgm::matrix_of_direction_cosines(g, a, b, v, None)?,
                    1 => vgm::matrix_of_voxel_sizes(g, a, b, v, None)?,
                    _ => vgm::matrix_of_translations(g, a, b, v, None)?,
                };
                o.m(r.into_new().as_ref());
            }
            o.m(Some(&vgm::extract_i_to_r(g, a, b, v)?));
            o.m(vgm::extract_r_to_i(g, a, b, v)?.as_ref());
        }
        "setv2r" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let mm = cx.matrix(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(vgm::set_vox2ras_from_matrix(
                g, a, b, &mut x.vg, &mm,
            )?));
            o.geom(&x.vg, true);
        }
        "p0" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let (r0, a0, s0) = (t.d(), t.d(), t.d());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(vgm::p0_to_cras(g, a, b, &mut x.vg, r0, a0, s0)?));
            o.geom(&x.vg, true);
        }
        "reinit" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(vgm::reinit_cache(g, a, b, &mut x.vg)?));
            o.geom(&x.vg, true);
        }
        "vox" => {
            let mut x = cx.mri(t)?;
            fill(&mut x, t.i() as u32);
            x.outside_val = t.d();
            let nops = t.i();
            for _ in 0..nops {
                let op = t.next();
                let (c, r, s, f, mode) = (t.i(), t.i(), t.i(), t.i(), t.i());
                let g = &mut cx.g;
                let b = &mut cx.b;
                match op {
                    "set" => {
                        let v = t.f();
                        o.i(i64::from(voxel::mri_set_vox_val(
                            g, b, &mut x, c, r, s, f, v, mode,
                        )?));
                    }
                    "get" => o.f(voxel::mri_get_vox_val(g, b, &x, c, r, s, f, mode)?),
                    "dx" => o.f(voxel::mri_get_vox_dx(g, b, &x, c, r, s, f)?),
                    "dy" => o.f(voxel::mri_get_vox_dy(g, b, &x, c, r, s, f)?),
                    _ => o.f(voxel::mri_get_vox_dz(g, b, &x, c, r, s, f)?),
                }
            }
            o.chunk(&x);
        }
        "copyhdr" => {
            let mut src = cx.mri(t)?;
            cx.header(t, &mut src);
            let mut dst = if t.i() != 0 {
                let mut d = cx.mri(t)?;
                cx.header(t, &mut d);
                Some(d)
            } else {
                None
            };
            let given = dst.is_some();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = m::mri_copy_header(g, a, b, &src, dst.as_mut())?;
            let res = match r {
                Ret::New(n) => n,
                _ => dst.expect("the given volume"),
            };
            o.i(i64::from(given));
            o.header(&res);
            o.ctab(res.ct.as_ref());
        }
        "pulse" => {
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let mut x = m::mri_alloc_header(g, a, b, 1, 1, 1, m::MRI_UCHAR, 1)?;
            let mut y = m::mri_alloc_header(g, a, b, 1, 1, 1, m::MRI_UCHAR, 1)?;
            x.flip_angle = t.d();
            x.tr = t.f();
            x.te = t.f();
            x.ti = t.f();
            o.i(i64::from(m::mri_copy_pulse_parameters(&x, &mut y)));
            o.d(y.flip_angle);
            o.f(y.tr);
            o.f(y.te);
            o.f(y.ti);
        }
        "ctab" => {
            let ct = cx.ctab(t, 0);
            let mut cp = ctab_deep_copy(&mut cx.g, &mut cx.b, Some(&ct));
            o.ctab(cp.as_ref());
            o.i(i64::from(ctab_free(&mut cx.g, &mut cx.b, &mut cp)));
            o.i(i64::from(cp.is_none()));
        }
        "vgcopy" => {
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            if t.i() != 0 {
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                let tmp = m::mri_alloc_header(g, a, b, 2, 2, 2, m::MRI_UCHAR, 1)?;
                v.i_to_r = tmp.vg.i_to_r;
                v.r_to_i = tmp.vg.r_to_i.clone();
            }
            let c = v.clone();
            o.geom(&c, true);
        }
        "vgassign" => {
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            let mut d = cx.mri(t)?;
            cx.set_geom(t, &mut d.vg);
            if t.i() != 0 {
                v.i_to_r = d.vg.i_to_r;
                v.r_to_i = d.vg.r_to_i.clone();
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                vgm::reinit_cache(g, a, b, &mut v)?;
            }
            d.vg.assign(&mut cx.g, &v);
            o.geom(&d.vg, true);
        }
        "usevg" | "vgtomri" => {
            let to = t.t[0] == "vgtomri";
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            let mut x = cx.mri(t)?;
            if to {
                o.i(i64::from(m::mri_copy_vol_geom_to_mri(&mut x, &v)));
            } else {
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                m::use_vol_geom_to_mri(g, a, b, &v, &mut x.vg)?;
            }
            o.geom(&x.vg, false);
        }
        "vgfrommri" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            o.i(i64::from(m::mri_copy_vol_geom_from_mri(&x, &mut v)));
            o.geom(&v, true);
        }
        "tkreg" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let (base, shear) = (t.i(), t.i() != 0);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let v = &x.vg;
            o.m(Some(&tk::xfm_crs2xyz_tkreg(g, a, b, v)?));
            o.m(Some(&tk::xfm_crs2xyz_fsl(g, a, b, v)?));
            o.m(Some(&tk::get_vox2ras(g, a, b, v, base, shear)?));
            o.m(tk::get_ras2vox(g, a, b, v, base, shear)?.as_ref());
            o.m(Some(&tk::get_vox2tkreg_ras(g, a, b, v)?));
            o.m(tk::get_tkreg_ras2vox(g, a, b, v)?.as_ref());
            o.m(tk::get_ras2tkreg_ras(g, a, b, v)?.as_ref());
            o.m(tk::get_tkreg_ras2ras(g, a, b, v)?.as_ref());
        }
        "ras2s" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let given = t.i() != 0;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if given {
                let mut gm = Matrix::real(4, 4);
                let r = tk::ras_from_surface_ras(g, a, b, &x.vg, Some(&mut gm))?;
                let ok = matches!(r, Ret::Given);
                o.i(i64::from(ok));
                o.m(ok.then_some(&gm));
            } else {
                let r = tk::ras_from_surface_ras(g, a, b, &x.vg, None)?;
                o.i(0);
                o.m(r.into_new().as_ref());
            }
            o.m(tk::vg_tkreg2ras(g, a, b, &x.vg, None)?.into_new().as_ref());
        }
        "tk2nat" | "nat2tk" | "v2v" | "tkv2v" => {
            let cmd = t.t[0];
            let mut ra = cx.mri(t)?;
            cx.set_geom(t, &mut ra.vg);
            let mut rb = cx.mri(t)?;
            cx.set_geom(t, &mut rb.vg);
            let r = cx.opt_matrix(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let res = match cmd {
                "tk2nat" => Some(tk::tkreg2native(g, a, b, &ra.vg, &rb.vg, r.as_ref())?),
                "nat2tk" => Some(tk::tkreg_mtx(g, a, b, &ra.vg, &rb.vg, r.as_ref())?),
                "v2v" => tk::tkreg_mtx_from_vox2vox(g, a, b, &ra.vg, &rb.vg, r.as_ref())?,
                _ => tk::vox_to_vox_from_tkreg_mtx(g, a, b, &ra.vg, &rb.vg, r.as_ref())?,
            };
            o.m(res.as_ref());
        }
        "fixtk" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let seed = t.i();
            let r = cx.opt_matrix(t);
            cx.g.drand48.srand48(i64::from(seed));
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.m(tk::fix_tkreg(g, a, b, &x.vg, r.as_ref())?.as_ref());
            o.f(cx.g.drand48.drand48() as f32);
        }
        "fsl2tk" | "tk2fsl" => {
            let cmd = t.t[0];
            let ty = t.next();
            let env = if ty == "-" {
                Environment::default()
            } else {
                Environment::from_pairs([("FSLOUTPUTTYPE", ty)])
            };
            let mut ra = cx.mri(t)?;
            cx.set_geom(t, &mut ra.vg);
            let mut rb = cx.mri(t)?;
            cx.set_geom(t, &mut rb.vg);
            let r = cx.opt_matrix(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let res = if cmd == "fsl2tk" {
                tk::fsl2tkreg(g, a, b, &env, &ra.vg, &rb.vg, r.as_ref())?
            } else {
                tk::tkreg2fsl(g, a, b, &env, &ra.vg, &rb.vg, r.as_ref())?
            };
            o.m(res.as_ref());
        }
        "crs1" => {
            let k = t.i();
            let mut q = (k != 0).then(|| cx.matrix(t));
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match q.as_mut() {
                None => {
                    let r = tk::mtx_crs1_to_crs0(g, a, b, None)?;
                    o.i(0);
                    o.m(r.into_new().as_ref());
                }
                Some(q) => {
                    let r = tk::mtx_crs1_to_crs0(g, a, b, Some(q))?;
                    let ok = matches!(r, Ret::Given);
                    o.i(i64::from(ok));
                    o.m(ok.then_some(&*q));
                }
            }
        }
        "sphinx" => {
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            o.i(i64::from(tk::hfs2sphinx(&mut v)));
            o.geom(&v, true);
        }
        "v2w" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let cache = t.i();
            if cache == 1 {
                x.vg.i_to_r = None;
                x.vg.r_to_i = None;
            }
            if cache == 2 {
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                vgm::reinit_cache(g, a, b, &mut x.vg)?;
            }
            let n = t.i();
            for _ in 0..n {
                let (px, py, pz) = (t.d(), t.d(), t.d());
                let to_world = t.i() != 0;
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                let (u, v, w) = if to_world {
                    vgm::voxel_to_world(g, a, b, &mut x.vg, px, py, pz)?
                } else {
                    let r = vgm::world_to_voxel(g, a, b, &mut x.vg, px, py, pz)?;
                    let (i, j, k) = vgm::world_to_voxel_index(g, a, b, &mut x.vg, px, py, pz)?;
                    o.i(i64::from(i));
                    o.i(i64::from(j));
                    o.i(i64::from(k));
                    r
                };
                o.d(u);
                o.d(v);
                o.d(w);
            }
            o.geom(&x.vg, false);
        }
        "codes" => {
            let a = t.next();
            let ca = t.i();
            let b = t.next();
            let cb = t.i();
            o.i(i64::from(m::mri_interp_code(a)));
            o.0 += m::mri_interp_string(ca).unwrap_or("NULL");
            o.0 += " ";
            o.i(i64::from(m::mri_precision_code(b)));
            o.0 += m::mri_precision_string(cb).unwrap_or("NULL");
            o.0 += " ";
        }
        "vgprint" => {
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            let (nocheck, pre) = (t.i() != 0, t.i() != 0);
            vgm::vgprint(&mut cx.a, &v, nocheck);
            vgm::geomprint(&mut cx.a, &v, pre.then_some("geom x 7:\n"));
            o.i(0);
        }
        "vgeq" => {
            let mut va = VolGeom::default();
            cx.set_geom(t, &mut va);
            let mut vb = VolGeom::default();
            cx.set_geom(t, &mut vb);
            let thresh = t.d();
            let debug = t.i() != 0;
            let sform = cx.opt_matrix(t);
            cx.g.vg_is_equal_threshold = thresh;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(vgm::vg_equal(g, &va, &vb)));
            o.i(i64::from(vgm::is_not_equal_thresh(&va, &vb, thresh)));
            o.i(i64::from(vgm::checkgeom(g, a, b, &va, &vb, thresh, debug)?));
            o.i(i64::from(vgm::checkvox2ras(
                g,
                a,
                b,
                &va,
                &vb,
                thresh,
                debug,
                sform.as_ref(),
            )?));
        }
        "shearless" => {
            let mut x = cx.mri(t)?;
            cx.set_geom(t, &mut x.vg);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            vgm::shearless_components(g, a, b, &mut x.vg)?;
            o.geom(&x.vg, false);
        }
        "tomat" => {
            let mut v = VolGeom::default();
            cx.set_geom(t, &mut v);
            let mut given = (t.i() != 0).then(|| cx.matrix(t));
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match given.as_mut() {
                Some(q) => {
                    let r = vgm::to_matrix(g, a, b, &v, Some(q))?;
                    o.i(i64::from(matches!(r, Ret::Given)));
                    o.m(Some(&*q));
                }
                None => {
                    let r = vgm::to_matrix(g, a, b, &v, None)?;
                    o.i(0);
                    o.m(r.into_new().as_ref());
                }
            }
        }
        "copy" => {
            let mut src = cx.mri(t)?;
            fill(&mut src, t.i() as u32);
            cx.header(t, &mut src);
            let mut dst = if t.i() != 0 {
                let mut d = cx.mri(t)?;
                fill(&mut d, t.i() as u32);
                d.ptype = t.i();
                Some(d)
            } else {
                None
            };
            let given = dst.is_some();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = mri_copy(g, a, b, &src, dst.as_mut())?;
            let res = match r {
                Ret::New(n) => n,
                _ => dst.expect("the given volume"),
            };
            o.i(i64::from(given));
            o.header(&res);
            o.chunk(&res);
        }
        "shape" => {
            let v: Vec<i32> = (0..8).map(|_| t.i()).collect();
            let sa = m::Shape::new(&mut cx.a, &v[0..4]);
            let sb = m::Shape::new(&mut cx.a, &v[4..8]);
            o.i(i64::from(sa == sb));
            o.i(i64::from(sa != sb));
        }
        "intent" => {
            let code = t.i();
            o.s(m::intent_name(code));
            o.i(i64::from(m::intent_code(t.next())));
        }
        "sizeof" => {
            let s = m::mri_sizeof(t.i());
            o.i(if s == usize::MAX { -1 } else { s as i64 });
        }
        c => panic!("unknown command {c}"),
    }
    Ok(())
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
    let shared = Shared::default();
    let mut cx = Ctx {
        g: Globals::new(mode),
        a: shared.clone(),
        b: shared.clone(),
    };
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

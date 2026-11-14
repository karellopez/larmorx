// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.volume probe's cases (`validation/probes/fs-volume/`): a seeded generator of case lines,
//! and the port's answer to each, printed as `fsvolume` prints FreeSurfer's (`<result> | <text>`,
//! the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program).
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::affine::AffineMatrix;
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix as mx;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::image as im;
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{self as mm, Mri, MriRegion};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_volume::arith::{self as ar, Pair, Tri};
use larmorx_freesurfer_volume::cma as cm;
use larmorx_freesurfer_volume::combine as cb;
use larmorx_freesurfer_volume::conform as cf;
use larmorx_freesurfer_volume::coords as co;
use larmorx_freesurfer_volume::deriv as dv;
use larmorx_freesurfer_volume::frames as fr;
use larmorx_freesurfer_volume::image as img;
use larmorx_freesurfer_volume::labels as lb;
use larmorx_freesurfer_volume::ltaread as lr;
use larmorx_freesurfer_volume::mriset as ms;
use larmorx_freesurfer_volume::mrivoxel as mv;
use larmorx_freesurfer_volume::order as od;
use larmorx_freesurfer_volume::orient as orn;
use larmorx_freesurfer_volume::pca as pc;
use larmorx_freesurfer_volume::region as rg;
use larmorx_freesurfer_volume::resample as rsm;
use larmorx_freesurfer_volume::resize as rs;
use larmorx_freesurfer_volume::rotate as rt;
use larmorx_freesurfer_volume::sample as sm;
use larmorx_freesurfer_volume::segs as sg;
use larmorx_freesurfer_volume::seq as sq;
use larmorx_freesurfer_volume::skull as sk;
use larmorx_freesurfer_volume::stats as st;
use larmorx_freesurfer_volume::talairachex as tx;
use larmorx_freesurfer_volume::vgeom;
use larmorx_freesurfer_volume::vol2vol as v2;
use larmorx_freesurfer_volume::voxops as vx;
use larmorx_freesurfer_volume::xform as xf;

/// The cases' random numbers.
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
fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
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

const NAMES: [&str; 6] = [
    "-",
    "vol.mgz",
    "orig/001.mgz",
    "/data/sub-01/mri/orig.mgz",
    "unknown",
    "a",
];

/// `set_geom`'s tokens.
fn geom(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.pick(&[0, 1, 1, 1, 2, -1]).to_string());
    for _ in 0..3 {
        t.push(g.pick(&[1, 2, 3, 4, 6, 256, 0, -2]).to_string());
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
    t.push(g.pick(&NAMES).to_owned());
}

/// A number's text in a volume-geometry line.
fn numtext(g: &mut Gen, int: bool) -> String {
    if g.chance(6) {
        return g
            .pick(&[
                "nan",
                "-nan",
                "inf",
                "-inf",
                "infinity",
                "INF",
                "1e40",
                "-1e-50",
                "0x1.8p3",
                "1e",
                "abc",
                "+5",
                "1.5e+00x",
                "2147483648",
                "99999999999",
                "-0",
                ".5",
                "5.",
                "1,5",
                "nan(1)",
                "0x",
                "--1",
                "3e-39",
                "1e-46",
            ])
            .to_owned();
    }
    if int {
        return g.pick(&[1, 2, 64, 128, 256, 0, -1, 311]).to_string();
    }
    let v = num(g, -200, 200, 64);
    match g.below(3) {
        0 => larmorx_core::cfmt::e(f64::from(v), 15),
        1 => format!("{v}"),
        _ => larmorx_core::cfmt::f(f64::from(v), 6),
    }
}

fn long_word(g: &mut Gen, n: usize) -> String {
    let c = g.pick(&["a", "x", "valid", "Z9"]);
    let mut s = String::new();
    while s.len() < n {
        s += c;
    }
    s.truncate(n);
    s
}

/// The line of key `k` (0 valid .. 7 cras), as writeVolGeom writes it or perturbed.
fn key_line(g: &mut Gen, k: usize) -> String {
    const KEYS: [&str; 8] = [
        "valid",
        "filename",
        "volume",
        "voxelsize",
        "xras  ",
        "yras  ",
        "zras  ",
        "cras  ",
    ];
    let mut key = KEYS[k].to_owned();
    let mut eq = "=".to_owned();
    if g.chance(12) {
        eq = g
            .pick(&["==", "=1", ":", "is", "", "=valid", "="])
            .to_owned();
    }
    if g.chance(15) {
        key = g
            .pick(&["Valid", "valid=", "xras=", "VOLUME", "cras:", "filenames"])
            .to_owned();
    }
    let body = match k {
        0 => {
            let v = numtext(g, true);
            let tail = g.pick(&[
                "  # volume info valid",
                "  # volume info invalid",
                "",
                " extra",
            ]);
            format!("{v}{tail}")
        }
        1 => {
            if g.chance(10) {
                String::new()
            } else if g.chance(12) {
                let n = g.pick(&[1022, 1023, 1024, 1030]);
                long_word(g, n)
            } else {
                let n = g.pick(&NAMES[1..]);
                let extra = if g.chance(6) { " second word" } else { "" };
                format!("{n}{extra}")
            }
        }
        2 => {
            let n = if g.chance(8) { g.int(0, 4) } else { 3 };
            (0..n)
                .map(|_| numtext(g, true))
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => {
            let n = if g.chance(8) { g.int(0, 4) } else { 3 };
            let sep = if g.chance(10) { "\t" } else { " " };
            (0..n)
                .map(|_| numtext(g, false))
                .collect::<Vec<_>>()
                .join(sep)
        }
    };
    let lead = if g.chance(15) { "  " } else { "" };
    let sp = if g.chance(15) { "\t" } else { " " };
    let end = if g.chance(12) { "\r\n" } else { "\n" };
    if eq.is_empty() {
        format!("{lead}{key}{sp}{body}{end}")
    } else {
        format!("{lead}{key}{sp}{eq} {body}{end}")
    }
}

/// An odd line between the block's lines.
fn odd_line(g: &mut Gen) -> Vec<u8> {
    match g.below(11) {
        0 => b"\n".to_vec(),
        1 => b"   \t\n".to_vec(),
        2 => b"dst volume info\n".to_vec(),
        3 => b"type      = 1 # LINEAR_RAS_TO_RAS\n".to_vec(),
        4 => {
            let n = g.pick(&[63, 64, 65, 100, 1030]);
            format!("{} = 1\n", long_word(g, n)).into_bytes()
        }
        5 => b"valid\n".to_vec(),
        6 => b"\0valid = 1\n".to_vec(),
        7 => b"filename =\n".to_vec(),
        8 => format!("cras = 1 2 {}\n", long_word(g, 1100)).into_bytes(),
        9 => b"# comment\n".to_vec(),
        _ => b"xras = 1 \0 2 3\n".to_vec(),
    }
}

/// The bytes of a volume-geometry block, well formed or perturbed, with what follows it.
fn vg_file(g: &mut Gen) -> Vec<u8> {
    let mut f = Vec::new();
    if g.chance(12) {
        f.extend_from_slice(g.pick(&[&b"\n"[..], b"  \n", b"\0\n"]));
    }
    if g.chance(30) {
        return f;
    }
    let mut order: Vec<usize> = (0..8).collect();
    if g.chance(6) {
        for i in (1..order.len()).rev() {
            let j = g.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
    }
    let perturb = g.chance(2);
    for &k in &order {
        if perturb && g.chance(14) {
            continue;
        }
        if perturb && g.chance(10) {
            f.extend(odd_line(g));
        }
        f.extend(key_line(g, k).into_bytes());
        if perturb && g.chance(20) {
            f.extend(key_line(g, k).into_bytes());
        }
    }
    match g.below(6) {
        0 => {}
        1 => f.extend_from_slice(b"dst volume info\nvalid = 1  # volume info valid\n"),
        2 => f.extend_from_slice(b"\x00\x00\x00\x14\x00\x00\x00\x01"),
        3 => f.extend_from_slice(b"subject bert\nfscale 0.1\n"),
        4 => {
            // no final newline
            f.pop();
        }
        _ => f.extend(odd_line(g)),
    }
    f
}

/// The float voxels the cases draw from (`FTAB` in `fsvolume.cpp`).
const FTAB: [u32; 28] = [
    0x00000000, 0x80000000, 0x3f800000, 0xbf800000, 0x3f000000, 0x3fc00000, 0x40200000, 0xc0200000,
    0x437f0000, 0x437f8000, 0x43800000, 0xc3962000, 0x477fff9a, 0x501502f9, 0xd01502f9, 0x7f7fc99e,
    0x000116c2, 0x7f800000, 0xff800000, 0x7fc00000, 0x7fa12345, 0xffc00001, 0x40490fd0, 0x24e69595,
    0x42fecccd, 0x3dcccccd, 0x41200000, 0xc1a00000,
];

/// A volume's shape in the cases: extent, type, frames, header only.
#[derive(Clone, Copy)]
struct Shape {
    w: i64,
    h: i64,
    d: i64,
    ty: i64,
    nf: i64,
    hdr: bool,
    /// Whether the cached matrices may be dropped.
    drop: bool,
}

fn shape(g: &mut Gen) -> Shape {
    let ty = if g.chance(10) {
        g.pick(&[9, 7, 2])
    } else {
        g.pick(&[0, 0, 1, 3, 3, 4, 10, 2])
    };
    let odd = g.chance(25);
    Shape {
        w: g.int(1, 4),
        h: g.int(1, 4),
        d: g.int(1, 3),
        ty,
        nf: if odd { g.pick(&[0, 3]) } else { g.int(1, 2) },
        hdr: g.chance(40),
        drop: false,
    }
}

/// A second volume: usually the first's shape, sometimes another.
fn like(g: &mut Gen, s: Shape) -> Shape {
    if g.chance(5) {
        let mut o = shape(g);
        if g.chance(2) {
            o.ty = s.ty;
        }
        o
    } else if g.chance(6) {
        Shape {
            ty: g.pick(&[0, 1, 3, 4, 10, 2]),
            ..s
        }
    } else {
        Shape { hdr: false, ..s }
    }
}

/// `get_vol`'s tokens.
fn vol_of(g: &mut Gen, t: &mut Vec<String>, s: Shape) {
    for v in [s.w, s.h, s.d, s.ty, s.nf, i64::from(s.hdr), g.int(0, 255)] {
        t.push(v.to_string());
    }
    geom(g, t);
    // `valid` from geom is the second token of the geometry: fine for an MRI
    t.push(g.int(-2, 3).to_string());
    t.push(g.int(-2, 5).to_string());
    for _ in 0..6 {
        t.push(fx(num(g, -100, 100, 4)));
    }
    t.push(fx(g.pick(&[1.0, 1.5, 0.0, 2.0])));
    t.push(fx(g.pick(&[1.0, 0.9375, 0.0])));
    for _ in 0..3 {
        t.push(fx(g.pick(&[0.0, 2000.0, 30.5, 1e-3])));
    }
    t.push(dx(g.pick(&[0.0, 0.17453292519943295, 1.5])));
    t.push(g.int(0, 3).to_string());
    // the cached matrices dropped (2) only where no MRIcopyHeader copies them: FreeSurfer's
    // AffineMatrixCopy of a source without one crashes (the probe's finding for fs.mri)
    let modes: &[i64] = if s.drop { &[0, 1, 1, 2] } else { &[0, 1, 1] };
    t.push(g.pick(modes).to_string());
}

fn vol(g: &mut Gen, t: &mut Vec<String>) -> Shape {
    let s = shape(g);
    vol_of(g, t, s);
    s
}

fn fval(g: &mut Gen) -> f32 {
    if g.chance(10) {
        return f32::from_bits(g.pick(&[0x7fc00000, 0x7fa12345, 0xffc00001]));
    }
    if g.chance(8) {
        return f32::from_bits(g.pick(&FTAB));
    }
    g.pick(&[0.0, 1.0, 2.0, 0.5, -1.0, 100.0, 127.5, 255.0, 3.0, 0.25])
}

/// A matrix's tokens (`fsvolume.cpp`'s `get_list`): rows, cols, the elements row by row (a
/// voxel-to-RAS-like matrix most of the time).
fn mat_toks(g: &mut Gen, t: &mut Vec<String>, rows: i64, cols: i64) {
    t.push(rows.to_string());
    t.push(cols.to_string());
    let like = g.chance(2);
    for r in 0..rows {
        for c in 0..cols {
            let v = if like && !g.chance(12) {
                if r == 3 {
                    if c == 3 { 1.0 } else { 0.0 }
                } else if c == 3 {
                    num(g, -128, 128, 4)
                } else if r == c {
                    g.pick(&[1.0f32, -1.0, 2.0, 0.5, 1.5, 0.0])
                } else {
                    g.pick(&[0.0f32, 0.0, 0.0, 0.25, -0.5, 1.0])
                }
            } else {
                num(g, -9, 9, 4)
            };
            t.push(fx(v));
        }
    }
}

/// An image's tokens (`fsvolume.cpp`'s `get_img`): rows cols format frames seed xsize ysize.
fn img_toks(g: &mut Gen, t: &mut Vec<String>, rows: i64, cols: i64, format: i64, nf: i64) {
    for v in [rows, cols, format, nf, g.int(0, 255)] {
        t.push(v.to_string());
    }
    for _ in 0..2 {
        t.push(fx(g.pick(&[1.0f32, 1.0, 0.0, 2.5, 1e-20, -0.0, 0.9375])));
    }
}

fn frame_of(g: &mut Gen, s: Shape) -> i64 {
    if g.chance(10) {
        g.pick(&[-1, s.nf, s.nf + 1])
    } else {
        g.int(0, (s.nf - 1).max(0))
    }
}

pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let mut lines = Vec::with_capacity(n);
    let cmds = COMMANDS;
    for i in 0..n {
        let cmd = cmds[i % cmds.len()];
        let mut t: Vec<String> = vec![cmd.to_owned()];
        match cmd {
            "vgwrite" | "vgprint0" | "vgi2r" | "vgr2i" | "tkrv2r" => geom(&mut g, &mut t),
            "vgread" => {
                geom(&mut g, &mut t);
                t.push(hex(&vg_file(&mut g)));
            }
            "getvg" | "copyvg" => {
                geom(&mut g, &mut t);
                geom(&mut g, &mut t);
            }
            "vgeq0" => {
                geom(&mut g, &mut t);
                let a = t[1..].to_vec();
                if g.chance(2) {
                    // the second geometry: the first, perturbed
                    let mut b = a.clone();
                    for _ in 0..g.int(0, 3) {
                        let k = g.below(b.len() as u64 - 1) as usize;
                        if (4..=6).contains(&k) || (8..=22).contains(&k) {
                            let v = f32::from_bits(u32::from_str_radix(&b[k], 16).unwrap());
                            let d = g.pick(&[1e-8f32, 1e-6, 1e-3, 0.5]);
                            b[k] = fx(v + d);
                        } else if k < 4 {
                            b[k] = g.int(0, 4).to_string();
                        }
                    }
                    t.extend(b);
                } else {
                    geom(&mut g, &mut t);
                }
                t.push(dx(g.pick(&[f64::from(f32::EPSILON), 1e-6, 1e-3, 0.0, 1.0])));
                t.push(dx(g.pick(&[f64::from(f32::EPSILON), 1e-4, 0.0])));
            }
            "clone" => {
                let s = vol(&mut g, &mut t);
                if g.chance(3) {
                    t.push("1".into());
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                } else {
                    t.push("0".into());
                }
            }
            "clonesp" => {
                vol(&mut g, &mut t);
                t.push(g.pick(&[-1, -1, 0, 3, 4]).to_string());
                t.push(g.pick(&[-1, -1, 1, 2]).to_string());
            }
            "clear" | "vrange" => {
                vol(&mut g, &mut t);
            }
            "vrangef" => {
                let s = vol(&mut g, &mut t);
                t.push(frame_of(&mut g, s).to_string());
            }
            "vrangereg" => {
                let s = vol(&mut g, &mut t);
                for k in [s.w, s.h, s.d] {
                    t.push(g.int(-2, k).to_string());
                }
                for k in [s.w, s.h, s.d] {
                    t.push(g.int(-1, k + 2).to_string());
                }
            }
            "lrange" => {
                let s = vol(&mut g, &mut t);
                let mut o = like(&mut g, s);
                if g.chance(2) {
                    o.ty = 0;
                }
                vol_of(&mut g, &mut t, o);
                t.push(g.int(0, 40).to_string());
            }
            "match" => {
                let s = vol(&mut g, &mut t);
                let o = like(&mut g, s);
                vol_of(&mut g, &mut t, o);
                for k in [s.w, s.h, s.d] {
                    t.push(if g.chance(2) { 0 } else { g.int(1, k + 1) }.to_string());
                }
                t.push(i64::from(g.chance(6)).to_string());
            }
            "cpframe" | "cpframes" | "linscale" | "smul" | "smulf" | "thresh" | "uthresh"
            | "invc" | "binz" | "abs" | "pos" | "neg" => {
                let mut s = vol(&mut g, &mut t);
                let mode = g.pick(&[0, 0, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                    s.nf = s.nf.min(o.nf);
                }
                match cmd {
                    "cpframe" => {
                        t.push(frame_of(&mut g, s).to_string());
                        t.push(frame_of(&mut g, s).to_string());
                    }
                    "cpframes" => {
                        let a = frame_of(&mut g, s);
                        t.push(a.to_string());
                        t.push((a + g.int(-1, 1)).to_string());
                        t.push(frame_of(&mut g, s).to_string());
                    }
                    "linscale" => {
                        t.push(fx(fval(&mut g)));
                        t.push(fx(fval(&mut g)));
                        t.push(g.int(0, 1).to_string());
                    }
                    "smul" | "thresh" | "uthresh" | "invc" => t.push(fx(fval(&mut g))),
                    "smulf" => {
                        t.push(fx(fval(&mut g)));
                        t.push(frame_of(&mut g, s).to_string());
                    }
                    "binz" => {
                        for _ in 0..3 {
                            t.push(fx(fval(&mut g)));
                        }
                    }
                    _ => {}
                }
            }
            "mul2" | "sub2" | "add2" | "div2" | "sam" => {
                let s = vol(&mut g, &mut t);
                let mut o = like(&mut g, s);
                if (cmd == "div2" || cmd == "sam") && g.chance(2) {
                    o.ty = 0;
                }
                vol_of(&mut g, &mut t, o);
                let mode = g.pick(&[0, 0, 1, 2, 3]);
                t.push(mode.to_string());
                if mode == 1 {
                    let mut p = like(&mut g, s);
                    if cmd == "div2" && g.chance(2) {
                        p.ty = 0;
                    }
                    vol_of(&mut g, &mut t, p);
                }
                if cmd == "sam" {
                    t.push(fx(fval(&mut g)));
                }
            }
            "extract" | "cras" => {
                let mut s = shape(&mut g);
                s.drop = cmd == "cras";
                vol_of(&mut g, &mut t, s);
                if cmd == "extract" {
                    if g.chance(3) {
                        t.push("1".into());
                        let mut o = shape(&mut g);
                        if g.chance(4) {
                            o.ty = s.ty;
                            o.nf = s.nf;
                        }
                        vol_of(&mut g, &mut t, o);
                    } else {
                        t.push("0".into());
                    }
                } else {
                    let o = shape(&mut g);
                    vol_of(&mut g, &mut t, o);
                }
                for k in [s.w, s.h, s.d] {
                    t.push(
                        if g.chance(10) {
                            k + g.int(0, 1)
                        } else {
                            g.int(-1, k - 1)
                        }
                        .to_string(),
                    );
                }
                if cmd == "extract" {
                    for k in [s.w, s.h, s.d] {
                        let n = if g.chance(8) {
                            g.int(-1, 0)
                        } else {
                            g.int(1, k + 1)
                        };
                        t.push(n.to_string());
                    }
                }
                for _ in 0..3 {
                    t.push(g.int(-1, 2).to_string());
                }
            }
            "avg" => {
                let s = vol(&mut g, &mut t);
                let mode = g.pick(&[0, 1, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                }
                t.push(g.pick(&[0, 1, 2, 7, -1, -2, 2147483647]).to_string());
            }
            "dsign" => {
                t.push(dx(g.pick(&[
                    1.0,
                    -1.0,
                    0.0,
                    2.5,
                    f64::NAN,
                    1e300,
                    -0.0,
                    0.1,
                ])));
            }
            "sras" | "spt" | "ostr" | "vtal" => {
                let mut s = shape(&mut g);
                s.drop = true;
                vol_of(&mut g, &mut t, s);
                match cmd {
                    "sras" => t.push(g.int(0, 3).to_string()),
                    "spt" => {
                        t.push(g.int(0, 2).to_string());
                        for _ in 0..3 {
                            t.push(dx(coord(&mut g)));
                        }
                    }
                    "vtal" => {
                        let lt = g.chance(2);
                        t.push(i64::from(lt).to_string());
                        if lt {
                            for i in 0..16 {
                                let v = if g.chance(30) {
                                    g.pick(&[f64::NAN, f64::INFINITY, -0.0, 1e300])
                                } else if i >= 12 {
                                    g.pick(&[0.0, 0.0, 0.0, 1.0, 0.5, 2.0])
                                } else {
                                    g.pick(&[1.0, 0.0, -1.0, 0.98, 1.1, 0.05, -12.5, 30.0])
                                };
                                t.push(dx(if i == 15 && v == 0.0 && g.chance(2) {
                                    1.0
                                } else {
                                    v
                                }));
                            }
                        }
                        for _ in 0..3 {
                            t.push(dx(coord(&mut g)));
                        }
                    }
                    _ => {}
                }
            }
            "com" | "pca" => {
                vol(&mut g, &mut t);
                t.push(g.pick(&[0, 0, 1, 10, 100, 200, 255]).to_string());
            }
            "cmph" => {
                let mut a = Vec::new();
                vol(&mut g, &mut a);
                let ea = hdr_extra(&mut g);
                t.extend(a.iter().cloned());
                t.extend(ea.iter().cloned());
                let mode = g.pick(&[0, 1, 1, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    if g.chance(4) {
                        vol(&mut g, &mut t);
                        t.extend(hdr_extra(&mut g));
                    } else {
                        let mut b = a.clone();
                        let mut eb = ea.clone();
                        for _ in 0..g.int(0, 2) {
                            perturb(&mut g, &mut b, &mut eb);
                        }
                        t.extend(b);
                        t.extend(eb);
                    }
                }
            }
            "dump" => {
                vol(&mut g, &mut t);
                t.push(fx(num(&mut g, -300, 300, 4)));
                t.push(g.int(-1, 3).to_string());
                for _ in 0..3 {
                    t.push(g.pick(&NAMES).to_owned());
                }
                let reg = g.chance(3);
                t.push(i64::from(reg).to_string());
                if reg {
                    for _ in 0..16 {
                        t.push(fx(num(&mut g, -50, 50, 8)));
                    }
                }
            }
            "stri" | "styp" | "sfrm" | "sfty" | "scub" | "scubf" | "ssinc" | "ssincf" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 3;
                }
                if !g.chance(4) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                t.push(dx(g.pick(&[
                    0.0,
                    0.0,
                    0.0,
                    -5.5,
                    f64::from_bits(0x7ff8_0000_0000_1234),
                    1e300,
                ])));
                let n = 4;
                t.push(n.to_string());
                for _ in 0..n {
                    for k in [s.w, s.h, s.d] {
                        t.push(dx(scoord(&mut g, k)));
                    }
                    t.push(frame_of(&mut g, s).to_string());
                    let aux = if cmd.starts_with("ssinc") {
                        if g.chance(40) {
                            g.pick(&[70, 1000, -1])
                        } else {
                            g.int(0, 6)
                        }
                    } else {
                        g.int(-1, 7)
                    };
                    t.push(aux.to_string());
                }
            }
            "trans" | "rot" => {
                let mut s = shape(&mut g);
                if g.chance(2) {
                    s.ty = 0;
                }
                if !g.chance(6) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                let mode = g.pick(&[0, 0, 0, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                }
                if cmd == "trans" {
                    for _ in 0..3 {
                        t.push(dx(g.pick(&[
                            0.0,
                            1.0,
                            -1.0,
                            0.5,
                            -0.5,
                            1.25,
                            2.0,
                            -2.75,
                            0.3,
                            1e-8,
                            f64::NAN,
                            1e10,
                        ])));
                    }
                } else {
                    let a = g.pick(&[
                        0.0f32,
                        0.3,
                        -0.7,
                        std::f32::consts::FRAC_PI_2,
                        std::f32::consts::PI,
                        0.05,
                        2.0,
                    ]);
                    let (c, sn) = (a.cos(), a.sin());
                    let base = match g.below(4) {
                        0 => [1.0, 0.0, 0.0, 0.0, c, sn, 0.0, -sn, c],
                        1 => [c, 0.0, -sn, 0.0, 1.0, 0.0, sn, 0.0, c],
                        2 => [c, sn, 0.0, -sn, c, 0.0, 0.0, 0.0, 1.0],
                        _ => {
                            let mut m = [0.0f32; 9];
                            for v in m.iter_mut() {
                                *v = g.pick(&[0.0, 1.0, -1.0, 0.5, 2.0, 0.25]);
                            }
                            m
                        }
                    };
                    for v in base {
                        t.push(fx(if g.chance(60) { f32::NAN } else { v }));
                    }
                    let o = g.chance(2);
                    t.push(i64::from(o).to_string());
                    if o {
                        for _ in 0..3 {
                            t.push(fx(g.pick(&[0.0, 1.0, 1.5, 2.0, -1.0, 0.75])));
                        }
                    }
                }
            }
            "rotx" | "roty" | "rotz" => {
                let mut s = shape(&mut g);
                if g.chance(2) {
                    s.ty = 0;
                }
                if !g.chance(6) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                let d = g.chance(4);
                t.push(i64::from(d).to_string());
                if d {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                }
                t.push(fx(g.pick(&[
                    0.0f32,
                    0.1,
                    0.5,
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::PI,
                    -0.7,
                    1e-6,
                    f32::NAN,
                    f32::INFINITY,
                ])));
            }
            "skull" => {
                let s = Shape {
                    w: g.int(6, 48),
                    h: g.int(6, 48),
                    d: g.int(1, 14),
                    ty: g.pick(&[0, 0, 0, 3, 4, 1]),
                    nf: 1,
                    hdr: false,
                    drop: false,
                };
                vol_of(&mut g, &mut t, s);
                let nb = g.int(0, 3);
                t.push(nb.to_string());
                for b in 0..nb {
                    let (w, h, d) = (s.w, s.h, s.d);
                    let full = b == 0 && g.chance(2);
                    let (x0, y0, z0) = if full {
                        (0, 0, 0)
                    } else {
                        (g.int(0, w - 1), g.int(0, h - 1), g.int(0, d - 1))
                    };
                    let (x1, y1, z1) = if full {
                        (w - 1, h - 1, d - 1)
                    } else {
                        (g.int(x0, w - 1), g.int(y0, h - 1), g.int(z0, d - 1))
                    };
                    for v in [x0, y0, z0, x1, y1, z1] {
                        t.push(v.to_string());
                    }
                    let v = if full {
                        g.pick(&[0.0f32, 0.0, 5.0, 2.0])
                    } else if g.chance(20) {
                        g.pick(&[f32::INFINITY, f32::NAN, 1e30])
                    } else {
                        g.pick(&[100.0f32, 200.0, 255.0, 60.0, 0.0, 30.0])
                    };
                    t.push(fx(v));
                }
                t.push(g.pick(&[0, 10, 50, 100, 128, 300, -5, 20]).to_string());
            }
            "tftype" => {
                let n = 6;
                t.push(n.to_string());
                for _ in 0..n {
                    let mut name = g
                        .pick(&["talairach", "a/b.c/reg", "x", "", "dir.d/sub", "nii.gz"])
                        .to_owned();
                    for _ in 0..g.int(0, 2) {
                        name += g.pick(&[
                            ".xfm", ".XFM", ".lta", ".Lta", ".fslmat", ".m3z", ".M3D", ".mgz",
                            ".nii", ".nii.gz", ".oct", ".dat", ".reg", ".txt", "#3", "@xfm",
                            "@lta", "#a@b", ".", "@", ".gz",
                        ]);
                    }
                    if g.chance(60) {
                        name += &"z".repeat(600);
                    }
                    t.push(if name.is_empty() {
                        "-".to_owned()
                    } else {
                        hex(name.as_bytes())
                    });
                }
            }
            "ltaread" | "tiv" => {
                let kind = g.below(4);
                let ext = match kind {
                    0 => g.pick(&[".xfm", ".XFM", ".xfm#1", ".lta@xfm"]),
                    1 => g.pick(&[".lta", ".LTA", "", ".oct", ".m3z"]),
                    2 => g.pick(&[".fslmat", ".FSLMAT"]),
                    _ => g.pick(&["/identity.nofile", ".xfm", ".lta", ".fslmat"]),
                };
                let text = match kind {
                    0 => xfm_text(&mut g),
                    1 => lta_text(&mut g),
                    2 => {
                        let mut s = String::new();
                        for _ in 0..g.int(2, 4) {
                            s += &num_row(&mut g);
                        }
                        s
                    }
                    _ => String::new(),
                };
                let mut bytes = text.into_bytes();
                if kind < 3 && g.chance(6) {
                    let cut = g.below(bytes.len() as u64 + 1) as usize;
                    bytes.truncate(cut);
                }
                t.push(hex(ext.as_bytes()));
                t.push(hex(&bytes));
                t.push(i64::from(kind < 3 && !g.chance(30)).to_string());
                // USE_AVERAGE305: 0 unset, 1 `true`, 2 set and empty
                t.push(g.below(3).to_string());
                if cmd == "tiv" {
                    t.push(dx(g.pick(&[2150.0, 1755.0, 2889.2, 0.0, -1.0])));
                }
            }
            "cmalab" => {
                let n = 8;
                t.push(n.to_string());
                for _ in 0..n {
                    let v = if g.chance(4) {
                        g.int(-3, 300)
                    } else {
                        g.pick(&CMA_LABELS)
                    };
                    t.push(v.to_string());
                    let f = if g.chance(3) {
                        fval(&mut g)
                    } else {
                        v as f32 + g.pick(&[0.0, 0.0, 0.5, -0.25, 1e-3])
                    };
                    t.push(fx(f));
                }
            }
            "cmavol" | "fixrib" => {
                let s = seg_vol(&mut g, &mut t);
                let o = like(&mut g, s);
                let o = Shape {
                    hdr: o.hdr && g.chance(4),
                    ..o
                };
                vol_of(&mut g, &mut t, o);
                paint(
                    &mut g,
                    &mut t,
                    o,
                    &[0.0, 3.0, 42.0, 2.0, 41.0, 1.0, 255.0, -1.0, 0.5],
                );
                if cmd == "cmavol" {
                    t.push(g.pick(&[1, 2, 1, 2, 0, 3]).to_string());
                } else {
                    let mode = g.pick(&[0, 1]);
                    t.push(mode.to_string());
                    if mode == 1 {
                        // MRIcopy's destination: the segmentation's shape (another one is
                        // MRIcopy's FS-163, where FreeSurfer writes outside it)
                        vol_of(&mut g, &mut t, Shape { hdr: false, ..s });
                    }
                }
            }
            "lrswap" => {
                seg_vol(&mut g, &mut t);
            }
            "lrseg" => {
                seg_vol(&mut g, &mut t);
                let rows = g.pick(&[1, 2, 3, 3, 4]);
                let rows2 = if g.chance(8) { rows + 1 } else { rows };
                for r in [rows, rows2] {
                    let cols = if g.chance(5) { g.pick(&[0, 2]) } else { 1 };
                    t.push(r.to_string());
                    t.push(cols.to_string());
                    for _ in 0..r * cols {
                        t.push(fx(if g.chance(10) {
                            fval(&mut g)
                        } else {
                            g.pick(&CMA_LABELS) as f32
                        }));
                    }
                }
            }
            "ohe" => {
                seg_vol(&mut g, &mut t);
                let n = g.pick(&[0, 1, 2, 3, 4]);
                t.push(n.to_string());
                for _ in 0..n {
                    t.push(g.pick(&CMA_LABELS[..12]).to_string());
                }
                t.push(g.pick(&[0, 1, 1, 2]).to_string());
            }
            "bvc" => {
                t.push(i64::from(!g.chance(10)).to_string());
                for _ in 0..16 {
                    t.push(dx(if g.chance(10) {
                        f64::from_bits(g.pick(&[
                            0x7ff8_0000_0000_0000,
                            0xfff8_0000_0000_0000,
                            0x7ff0_0000_0000_0000,
                            0x8000_0000_0000_0000,
                        ]))
                    } else {
                        g.int(-1000, 2_000_000) as f64 / g.pick(&[1.0, 3.0, 7.0, 1e6, 0.001])
                    }));
                }
            }
            "bvr" => {
                t.push("1".to_owned());
                t.push(hex(&brainvol_text(&mut g)));
            }
            "setres" => {
                vol(&mut g, &mut t);
                for _ in 0..3 {
                    t.push(fx(size(&mut g)));
                }
            }
            "vfill" => {
                vol(&mut g, &mut t);
                t.push(fx(fval(&mut g)));
            }
            "fillfg" => {
                let mut s = shape(&mut g);
                if !g.chance(4) {
                    s.ty = 0;
                }
                if !g.chance(8) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                let mode = g.pick(&[0, 0, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                }
                for k in [s.w, s.h, s.d] {
                    t.push(g.int(-1, k).to_string());
                }
                t.push(g.pick(&[0, 1, 50, 128, 200, 256, -1]).to_string());
                t.push(g.pick(&[0, 1, 128, 255, 255, 256, -1, 300]).to_string());
            }
            "explane" => {
                let s = vol(&mut g, &mut t);
                if g.chance(3) {
                    t.push("1".into());
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                } else {
                    t.push("0".into());
                }
                t.push(g.pick(&[0, 1, 2, 0, 1, 2, 3, -1]).to_string());
                t.push(g.int(-1, 4).to_string());
            }
            "fillplane" => {
                let s = vol(&mut g, &mut t);
                let o = like(&mut g, s);
                vol_of(&mut g, &mut t, o);
                t.push(g.pick(&[0, 1, 2, 0, 1, 2, 3, -1]).to_string());
                t.push(g.int(-1, 4).to_string());
                t.push(g.pick(&[0, 1, 255, -5, 70000]).to_string());
            }
            "ups" => {
                let mut s = shape(&mut g);
                if !g.chance(6) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                let k = g.int(0, 2);
                let n = if g.chance(12) {
                    g.pick(&[0, -1])
                } else {
                    g.pick(&[1, 2, 2, 3])
                };
                if g.chance(4) {
                    t.push("1".into());
                    let mut o = like(&mut g, s);
                    if g.chance(2) {
                        let f = if k == 1 { 2 } else { n.max(1) };
                        o.w = s.w * f;
                        o.h = s.h * f;
                        o.d = s.d * f;
                    }
                    vol_of(&mut g, &mut t, o);
                } else {
                    t.push("0".into());
                }
                t.push(k.to_string());
                t.push(n.to_string());
            }
            "dns2" | "dnsn" => {
                let mut s = shape(&mut g);
                if !g.chance(6) {
                    s.hdr = false;
                }
                // a NaN factor makes FreeSurfer's extent -2147483648 with reproduce (FS-317), whose
                // allocation error fs.mri does not model yet (told track A): not drawn
                let fs: [f32; 3] = if cmd == "dnsn" {
                    let f = |g: &mut Gen| -> f32 {
                        if g.chance(30) {
                            -1.0
                        } else {
                            g.pick(&[1.0, 2.0, 2.0, 3.0, 0.5, 1.5, 2.5, 4.0])
                        }
                    };
                    [f(&mut g), f(&mut g), f(&mut g)]
                } else {
                    [2.0; 3]
                };
                // extents the factors divide, mostly (FreeSurfer's windows read past the source
                // otherwise: FS-52, FS-53)
                let ext = |g: &mut Gen, f: f32, hi: i64| -> i64 {
                    if f >= 1.0 && f.fract() == 0.0 && !g.chance(3) {
                        f as i64 * g.int(1, 2)
                    } else {
                        g.int(1, hi)
                    }
                };
                s.w = ext(&mut g, fs[0], 5);
                s.h = ext(&mut g, fs[1], 5);
                s.d = ext(&mut g, fs[2], 4);
                vol_of(&mut g, &mut t, s);
                if g.chance(4) {
                    t.push("1".into());
                    let mut o = like(&mut g, s);
                    if g.chance(2) {
                        let e = |n: i64, f: f32| ((n as f32 / f).ceil() as i64).max(0);
                        o.w = e(s.w, fs[0]);
                        o.h = e(s.h, fs[1]);
                        o.d = e(s.d, fs[2]);
                        o.nf = s.nf;
                        if cmd == "dns2" && !g.chance(5) {
                            o.ty = s.ty;
                        }
                    }
                    vol_of(&mut g, &mut t, o);
                } else {
                    t.push("0".into());
                }
                if cmd == "dnsn" {
                    for f in fs {
                        t.push(fx(f));
                    }
                    t.push(g.int(0, 1).to_string());
                }
            }
            "mft" => {
                let s = vol(&mut g, &mut t);
                t.push(frame_of(&mut g, s).to_string());
                t.push(fx(fval(&mut g)));
            }
            "lim" => {
                vol(&mut g, &mut t);
                t.push(fx(fval(&mut g)));
                t.push(fx(fval(&mut g)));
            }
            "chtype" => {
                let mut s = shape(&mut g);
                if !g.chance(6) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                t.push(g.int(0, 1).to_string());
                t.push(g.pick(&[0, 0, 1, 2, 3, 3, 4, 10]).to_string());
                t.push(fx(g.pick(&[0.0, 0.0, 0.01, 0.1, 0.5, 1.0, 2.0, f32::NAN])));
                t.push(fx(g.pick(&[0.0, 0.999, 0.9, 1.0, 0.5, 1.5])));
                t.push(g.pick(&[0, 0, 0, 1, 2]).to_string());
                t.push(g.int(0, 1).to_string());
            }
            "coef" | "hsinc" => {
                let n = 6;
                t.push(n.to_string());
                for _ in 0..n {
                    let x = if g.chance(8) {
                        g.pick(&[f64::NAN, f64::INFINITY, 1e-6, -1e-6, 0.0, -0.0, 1e300])
                    } else {
                        g.int(-40, 40) as f64 / g.pick(&[1.0, 4.0, 7.0, 10.0, 3.0])
                            + g.pick(&[0.0, 0.0, 1e-9, 0.123456789])
                    };
                    t.push(dx(x));
                    if cmd == "coef" {
                        t.push(g.int(-1, 4).to_string());
                    } else {
                        t.push(dx(g.pick(&[10.0, 10.0, 2.0, 0.0, 1.0, 12.0, -4.0])));
                    }
                }
            }
            "splat" | "sseq" | "sxyz" | "tkern" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 3;
                }
                if !g.chance(4) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                if cmd == "sseq" || cmd == "sxyz" {
                    t.push(dx(g.pick(&[
                        0.0,
                        0.0,
                        -5.5,
                        f64::from_bits(0x7ff8_0000_0000_1234),
                        1e300,
                    ])));
                }
                let n = 4;
                t.push(n.to_string());
                for _ in 0..n {
                    if cmd == "sxyz" {
                        for k in [s.w, s.h, s.d] {
                            t.push(g.int(-2, k + 1).to_string());
                        }
                        let fb = g.int(0, s.nf.max(0));
                        t.push(fb.to_string());
                        t.push(g.int(fb - 1, s.nf + 1).to_string());
                        t.push(g.int(0, 1).to_string());
                        continue;
                    }
                    for k in [s.w, s.h, s.d] {
                        t.push(dx(scoord(&mut g, k)));
                    }
                    match cmd {
                        "splat" => {
                            t.push(frame_of(&mut g, s).max(0).to_string());
                            t.push(dx(if g.chance(6) {
                                g.pick(&[
                                    f64::NAN,
                                    f64::from_bits(0x7ff4_0000_0000_0abc),
                                    f64::from_bits(0xfff8_0000_0000_0001),
                                    f64::INFINITY,
                                    -f64::INFINITY,
                                    1e10,
                                    -3e9,
                                ])
                            } else {
                                g.pick(&[
                                    0.5, 1.0, 100.0, 255.7, -3.25, 40000.0, 70000.0, -0.0, 2.5e9,
                                ])
                            }));
                        }
                        "sseq" => {
                            let first = g.int(0, s.nf.max(0));
                            t.push(first.to_string());
                            t.push(g.int(first - 1, s.nf + 1).to_string());
                            t.push(
                                if g.chance(30) {
                                    5
                                } else {
                                    g.pick(&[-1, -1, -1, 1, 0, 0, 2, 3, 4, 6])
                                }
                                .to_string(),
                            );
                        }
                        _ => {}
                    }
                }
            }
            "deriv" | "grad" | "nbrs" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 3;
                }
                if !g.chance(4) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                if cmd == "nbrs" {
                    let n = 3;
                    t.push(n.to_string());
                    for _ in 0..n {
                        let kind = g.int(0, 7);
                        t.push(kind.to_string());
                        let edge = g.chance(4);
                        for (k, m) in [(s.w, 3), (s.h, 2), (s.d, 2)] {
                            let v = if edge {
                                g.int(-m, k + m - 1)
                            } else {
                                g.int(0, (k - 1).max(0))
                            };
                            t.push(v.to_string());
                        }
                        let a = match kind {
                            2 => g.int(1, 5),
                            4 => frame_of(&mut g, s).max(0),
                            _ => g.pick(&[0, 1, 2, 100, 128, 255, -1, 3]),
                        };
                        t.push(a.to_string());
                        t.push(g.pick(&[0, 1, 2, 100, 255, -1]).to_string());
                        t.push(fx(fval(&mut g)));
                        t.push(fx(fval(&mut g)));
                    }
                } else {
                    t.push(dx(g.pick(&[
                        1.0,
                        1.0,
                        -1.0,
                        0.5,
                        f64::from_bits(0x7ff8_0000_0000_0042),
                    ])));
                    let n = 4;
                    t.push(n.to_string());
                    for _ in 0..n {
                        for k in [s.w, s.h, s.d] {
                            t.push(dx(scoord(&mut g, k)));
                        }
                        t.push(g.int(0, 1).to_string());
                        if cmd == "grad" {
                            t.push(frame_of(&mut g, s).max(0).to_string());
                            continue;
                        }
                        for _ in 0..3 {
                            t.push(dx(if g.chance(12) {
                                g.pick(&[f64::NAN, f64::INFINITY, 1e10, -1e10])
                            } else {
                                g.pick(&[
                                    0.0,
                                    1.0,
                                    -1.0,
                                    0.5,
                                    std::f64::consts::FRAC_1_SQRT_2,
                                    -0.25,
                                    2.0,
                                ])
                            }));
                        }
                        t.push(dx(if g.chance(150) {
                            g.pick(&[1e308, f64::INFINITY])
                        } else {
                            g.pick(&[
                                0.0,
                                0.0,
                                1e-8,
                                0.5,
                                1.0,
                                2.0,
                                3.3,
                                -1.0,
                                f64::NAN,
                                1e300,
                                0.2,
                            ])
                        }));
                    }
                }
            }
            "rsfill" => {
                // 1 case in 12: an 8 x 8 x 8 source voting into one template voxel that holds it
                // all (counts past 255)
                let special = g.chance(12);
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 3;
                }
                if !g.chance(10) {
                    s.hdr = false;
                }
                if special {
                    s = Shape {
                        w: 8,
                        h: 8,
                        d: 8,
                        ty: g.pick(&[3, 3, 0]),
                        nf: g.int(1, 2),
                        hdr: false,
                        drop: false,
                    };
                }
                let at = t.len();
                vol_of(&mut g, &mut t, s);
                let mut o = like(&mut g, s);
                o.hdr = g.chance(5);
                if g.chance(4) {
                    o.nf = g.int(0, 3);
                }
                if special {
                    (o.w, o.h, o.d, o.nf) = (1, 1, 1, 1);
                }
                let at2 = t.len();
                vol_of(&mut g, &mut t, o);
                // the geometries' tokens (after w h d type frames hdr seed): 4-6 the voxel sizes,
                // 7 ras_good_flag, 8-16 the cosines, 17-19 the centre, 20-22 s_ras
                let (gs, gt) = (at + 7, at2 + 7);
                let tf = |tok: &str| f32::from_bits(u32::from_str_radix(tok, 16).expect("a float"));
                if special {
                    for (base, size, c) in [(gs, 1.0f32, 0.0f32), (gt, 8.0, 4.0)] {
                        for k in 4..=6 {
                            t[base + k] = fx(size);
                        }
                        t[base + 7] = "1".into();
                        for k in 0..9 {
                            t[base + 8 + k] = fx(if k % 4 == 0 { 1.0 } else { 0.0 });
                        }
                        for k in 17..=19 {
                            t[base + k] = fx(c);
                        }
                    }
                } else if !g.chance(3) {
                    // the template on the source's grid, its voxels scaled and its centre moved;
                    // mostly a source with a good RAS and voxel sizes that make a matrix
                    if !g.chance(4) {
                        t[gs + 7] = "1".into();
                        for k in 4..=6 {
                            t[gs + k] = fx(g.pick(&[1.0f32, 1.0, 0.5, 2.0, 1.5, 0.9375]));
                        }
                    }
                    for k in 4..=22 {
                        t[gt + k] = t[gs + k].clone();
                    }
                    let f = g.pick(&[1.0f32, 1.0, 2.0, 0.5, 1.5]);
                    for k in 4..=6 {
                        t[gt + k] = fx(tf(&t[gs + k]) * f);
                    }
                    for k in 17..=19 {
                        t[gt + k] = fx(tf(&t[gs + k]) + g.pick(&[0.0f32, 0.0, 0.5, -1.0, 2.25]));
                    }
                }
                let ty = if special {
                    6
                } else {
                    g.pick(&[0, 1, 1, 1, 2, 3, 4, 6, 6, 7])
                };
                t.push(ty.to_string());
                t.push(fx(fval(&mut g)));
                t.push(i64::from(g.chance(5)).to_string());
                t.push(i64::from(g.chance(12)).to_string());
            }
            "repval" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 0;
                }
                if !g.chance(8) {
                    s.hdr = false;
                }
                let at = t.len();
                vol_of(&mut g, &mut t, s);
                let mode = g.pick(&[0, 0, 1, 2]);
                t.push(mode.to_string());
                if mode == 1 {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                }
                if g.chance(3) {
                    // MRIreplaceValuesUchar: often the source's first byte
                    t.push("1".into());
                    let seed: u64 = t[at + 6].parse().expect("a seed");
                    let first = (seed * 101) & 0xff;
                    t.push(
                        g.pick(&[first as i64, first as i64, 0, 7, 255, 300, -1])
                            .to_string(),
                    );
                    t.push(g.pick(&[0i64, 1, 128, 255, 256, -5]).to_string());
                } else {
                    t.push("0".into());
                    let v = if g.chance(2) {
                        f32::from_bits(g.pick(&FTAB))
                    } else {
                        fval(&mut g)
                    };
                    t.push(fx(v));
                    t.push(fx(fval(&mut g)));
                }
            }
            "stats" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = g.pick(&[2, 3, 3]);
                }
                if g.chance(4) {
                    s.nf = g.int(1, 3);
                }
                if !g.chance(12) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                t.push(i64::from(g.chance(3)).to_string());
            }
            "type2s" => t.push(g.int(-1, 12).to_string()),
            "cropbb" => {
                vol(&mut g, &mut t);
                for _ in 0..3 {
                    t.push(g.int(-3, 5).to_string());
                }
                for _ in 0..3 {
                    t.push(g.pick(&[0i64, 1, 2, 4, 9, -1, 2147483647]).to_string());
                }
            }
            "crass" => {
                let mut s = shape(&mut g);
                (s.w, s.h, s.d) = (g.int(1, 9), g.int(1, 9), g.int(1, 6));
                s.hdr = g.chance(3);
                s.drop = true;
                vol_of(&mut g, &mut t, s);
                let mut o = shape(&mut g);
                (o.w, o.h, o.d) = (g.int(1, 4), g.int(1, 4), g.int(1, 3));
                o.hdr = true;
                vol_of(&mut g, &mut t, o);
                t.push(i64::from(g.chance(6)).to_string());
            }
            "chkostr" | "o2dc" => {
                if cmd == "o2dc" {
                    let mut s = shape(&mut g);
                    s.hdr = true;
                    vol_of(&mut g, &mut t, s);
                }
                let letters = b"LRAPISlrapisXb ";
                let len = if g.chance(12) {
                    g.pick(&[1usize, 2, 4, 5])
                } else {
                    3
                };
                let mut o: Vec<u8> = (0..len)
                    .map(|_| letters[g.below(letters.len() as u64) as usize])
                    .collect();
                if g.chance(2) {
                    // a valid string, sometimes
                    let axes = [*b"LR", *b"PA", *b"IS"];
                    let mut order = [0usize, 1, 2];
                    for k in (1..3).rev() {
                        order.swap(k, g.below(k as u64 + 1) as usize);
                    }
                    o = order
                        .iter()
                        .map(|&a| axes[a][g.below(2) as usize])
                        .collect();
                    if g.chance(4) {
                        o[0] = o[0].to_ascii_lowercase();
                    }
                }
                if g.chance(30) {
                    o[0] = 0xe9;
                }
                t.push(hex(&o));
            }
            "s44" => {
                let (r, c) = if g.chance(5) {
                    g.pick(&[(0, 0), (3, 4), (4, 3), (5, 5), (1, 16)])
                } else {
                    (4, 4)
                };
                t.push(r.to_string());
                t.push(c.to_string());
                t.push(i64::from(g.chance(6)).to_string());
                for _ in 0..16 {
                    t.push(fx(num(&mut g, -9, 9, 4)));
                }
            }
            "vx2rx" | "v2v" => {
                let s = vol(&mut g, &mut t);
                if cmd == "v2v" {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                } else {
                    let d = g.chance(2);
                    t.push(i64::from(d).to_string());
                    if d {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                    }
                    let mv = !g.chance(15);
                    t.push(i64::from(mv).to_string());
                    if mv {
                        let (r, c) = if g.chance(10) { (3, 4) } else { (4, 4) };
                        mat_toks(&mut g, &mut t, r, c);
                    }
                    let mr = g.chance(3);
                    t.push(i64::from(mr).to_string());
                    if mr {
                        let (r, c) = if g.chance(8) { (4, 3) } else { (4, 4) };
                        mat_toks(&mut g, &mut t, r, c);
                    }
                }
            }
            "setv2r" => {
                let mut s = shape(&mut g);
                if !g.chance(8) {
                    s.hdr = true;
                }
                vol_of(&mut g, &mut t, s);
                let e = g.chance(4);
                t.push(i64::from(e).to_string());
                let txt: &[u8] =
                    g.pick(&[&b"0.5"[..], b"1e-9", b"abc", b"1000", b" 0.01x", b"nan"]);
                t.push(hex(txt));
                let (r, c) = if g.chance(12) { (5, 6) } else { (4, 4) };
                mat_toks(&mut g, &mut t, r, c);
            }
            "toimgv" | "toimg" => {
                let mut s = shape(&mut g);
                (s.w, s.h, s.d) = (g.int(1, 4), g.int(1, 4), g.int(1, 3));
                if !g.chance(15) {
                    s.hdr = false;
                }
                if cmd == "toimg" && !g.chance(6) {
                    s.ty = g.pick(&[0, 1, 3]);
                }
                let at = t.len();
                vol_of(&mut g, &mut t, s);
                if !g.chance(6) {
                    // a principal direction: the cosines a signed permutation
                    let base = at + 7;
                    let mut order = [0usize, 1, 2];
                    for k in (1..3).rev() {
                        order.swap(k, g.below(k as u64 + 1) as usize);
                    }
                    for col in 0..3 {
                        for row in 0..3 {
                            let v = if order[col] == row {
                                g.pick(&[1.0f32, -1.0])
                            } else {
                                0.0
                            };
                            t[base + 8 + 3 * col + row] = fx(v);
                        }
                    }
                }
                let given = g.chance(4);
                t.push(i64::from(given).to_string());
                if given {
                    let format = if cmd == "toimg" {
                        g.pick(&[0i64, 2, 3, 0, 1])
                    } else {
                        g.pick(&[0i64, 0, 0, 3])
                    };
                    let (r, c) = if g.chance(3) {
                        (g.int(1, 4), g.int(1, 4))
                    } else if cmd == "toimg" {
                        (s.h, s.w)
                    } else {
                        g.pick(&[(s.h, s.w), (s.d, s.w), (s.h, s.d), (s.d, s.h)])
                    };
                    img_toks(&mut g, &mut t, r, c, format, 1);
                }
                t.push(g.pick(&[-1i64, -1, 0, 1, 2, 5, -7]).to_string());
                if cmd == "toimgv" {
                    t.push(g.pick(&[0i64, 1, 2, 0, 1, 2, 3, -1]).to_string());
                    t.push(g.pick(&[0i64, 0, 0, 1, s.nf]).to_string());
                }
            }
            "img2mri" => {
                let format = if g.chance(20) {
                    g.pick(&[6i64, 4])
                } else {
                    g.pick(&[0i64, 1, 2, 3, 35])
                };
                let nf = if g.chance(6) {
                    g.pick(&[0i64, 2, 3])
                } else {
                    1
                };
                let (r, c) = (g.int(1, 4), g.int(1, 4));
                img_toks(&mut g, &mut t, r, c, format, nf);
            }
            "smpar" | "erasebp" | "nbhd" | "nearnz" | "lmeans" | "mkpos" | "mlbv" | "lnbhd"
            | "linbhd" | "lbbox" | "fillbox" | "mskz" | "clipwm" => {
                let labels = [0.0f32, 1.0, 2.0, 3.0, 3.0, 7.0, 2.0, -1.0, 250.0];
                let mut s = shape(&mut g);
                if !g.chance(12) {
                    s.hdr = false;
                }
                if cmd == "smpar" {
                    if !g.chance(5) {
                        s.ty = 4;
                    }
                    (s.w, s.h, s.d) = (g.int(1, 5), g.int(1, 5), g.int(1, 4));
                }
                if cmd == "clipwm" && g.chance(2) {
                    s.ty = 0;
                }
                vol_of(&mut g, &mut t, s);
                let painted = !matches!(cmd, "erasebp" | "mkpos" | "fillbox" | "mskz" | "lmeans");
                if painted {
                    paint(&mut g, &mut t, s, &labels);
                }
                let wsz = |g: &mut Gen| g.pick(&[3i64, 3, 1, 5, 0, -1, 2]);
                let pt = |g: &mut Gen, s: Shape| {
                    if g.chance(10) {
                        [g.int(-3, 6), g.int(-3, 6), g.int(-3, 6)]
                    } else {
                        [g.int(0, s.w - 1), g.int(0, s.h - 1), g.int(0, s.d - 1)]
                    }
                };
                let label = |g: &mut Gen| g.pick(&[0i64, 1, 2, 3, 7, -1, 250, 5]);
                match cmd {
                    "smpar" => t.push(g.pick(&[1i64, 2, 3, 4, 8, 13, 26, 27, 0]).to_string()),
                    "erasebp" => t.push(g.pick(&[0i64, 1, 1, 2, 5, -1]).to_string()),
                    "nbhd" => {
                        t.push(g.int(0, 3).to_string());
                        t.push(wsz(&mut g).to_string());
                        for v in pt(&mut g, s) {
                            t.push(v.to_string());
                        }
                        t.push(frame_of(&mut g, s).to_string());
                        t.push(fx(fval(&mut g)));
                    }
                    "nearnz" => {
                        t.push(wsz(&mut g).to_string());
                        for v in pt(&mut g, s) {
                            let d = if g.chance(3) {
                                0.0
                            } else {
                                g.pick(&[0.25f64, -0.4, 0.5, 0.49])
                            };
                            t.push(dx(v as f64 + d));
                        }
                        t.push(i64::from(!g.chance(4)).to_string());
                    }
                    "lmeans" => {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        paint(&mut g, &mut t, o, &labels);
                        t.push(label(&mut g).to_string());
                        t.push(g.int(0, 4).to_string());
                        for v in pt(&mut g, s) {
                            t.push(v.to_string());
                        }
                        t.push(g.pick(&[0i64, 1, 1, 2, 5, -1]).to_string());
                        t.push(i64::from(!g.chance(4)).to_string());
                    }
                    "mkpos" | "mlbv" => {
                        let d = g.chance(4);
                        t.push(i64::from(d).to_string());
                        if d {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                        if cmd == "mlbv" {
                            t.push(label(&mut g).to_string());
                            t.push(g.pick(&[1i64, 255, 9]).to_string());
                            t.push(i64::from(g.chance(2)).to_string());
                        }
                    }
                    "lnbhd" => {
                        let v = g.chance(2);
                        t.push(i64::from(v).to_string());
                        if v {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                        for v in pt(&mut g, s) {
                            t.push(v.to_string());
                        }
                        t.push(g.pick(&[1i64, 1, 0, 2]).to_string());
                        t.push(g.pick(&[8i64, 4, 1, 256]).to_string());
                    }
                    "linbhd" => {
                        for v in pt(&mut g, s) {
                            t.push(v.to_string());
                        }
                        t.push(g.pick(&[1i64, 1, 0, 2]).to_string());
                        t.push(label(&mut g).to_string());
                        t.push(g.pick(&[-1i64, -1, 0, 1, 2, 3]).to_string());
                    }
                    "lbbox" => t.push(label(&mut g).to_string()),
                    "fillbox" => {
                        for _ in 0..3 {
                            t.push(g.int(-2, 4).to_string());
                        }
                        for _ in 0..3 {
                            t.push(g.pick(&[0i64, 1, 2, 3, 9, -1]).to_string());
                        }
                        t.push(fx(fval(&mut g)));
                    }
                    "mskz" => {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        paint(&mut g, &mut t, o, &labels);
                        let mode = g.pick(&[0i64, 0, 1, 2]);
                        t.push(mode.to_string());
                        if mode == 1 {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                    }
                    _ => {
                        // clipwm: the white-matter volume
                        let mut o = like(&mut g, s);
                        if g.chance(2) {
                            o.ty = 0;
                        }
                        vol_of(&mut g, &mut t, o);
                        paint(&mut g, &mut t, o, &[0.0, 5.0, 4.0, 110.0, 250.0]);
                    }
                }
            }
            "lxf" | "rsmat" => {
                let mut s = shape(&mut g);
                if g.chance(3) {
                    s.ty = 3;
                }
                if !g.chance(6) {
                    s.hdr = false;
                }
                vol_of(&mut g, &mut t, s);
                if cmd == "rsmat" {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                    t.push(i64::from(g.chance(8)).to_string());
                    lines.push(t.join(" "));
                    continue;
                }
                t.push(dx(g.pick(&[
                    0.0,
                    0.0,
                    0.0,
                    5.0,
                    -2.5,
                    1e-9,
                    f64::from_bits(0x7ff8_0000_0000_0077),
                ])));
                if g.chance(4) {
                    t.push("1".into());
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                } else {
                    t.push("0".into());
                }
                t.push(g.int(0, 5).to_string());
                t.push(g.pick(&[0, 0, 1, 1, 1, 2, 3, 4]).to_string());
                if g.chance(30) {
                    t.push("0".into());
                } else {
                    t.push("1".into());
                    let kind = g.int(0, 5);
                    for r in 0..4 {
                        for c in 0..4 {
                            let v: f32 = match (kind, r, c) {
                                (_, 3, 3) => 1.0,
                                (_, 3, _) => 0.0,
                                (0, r, c) if r == c => 1.0,
                                (0, _, _) => 0.0,
                                (1, r, c) if r == c => g.pick(&[1.0, 2.0, 0.5, -1.0, 1.5]),
                                (1, _, 3) => g.pick(&[0.0, 1.0, -0.5, 0.25, 2.0]),
                                (1, _, _) => 0.0,
                                (2, 2, _) if g.chance(3) => 0.0,
                                (4, _, _) if g.chance(20) => {
                                    f32::from_bits(g.pick(&[0x7fc00000, 0x7f800000]))
                                }
                                _ => num(&mut g, -20, 20, 10),
                            };
                            t.push(fx(v));
                        }
                    }
                }
            }
            "pvlab" => {
                // a segmentation of a few labels, an intensity volume, the outputs, a border
                let mut s = shape(&mut g);
                if !g.chance(12) {
                    s.hdr = false;
                }
                if g.chance(3) {
                    (s.w, s.h, s.d) = (g.int(2, 6), g.int(2, 6), g.int(2, 5));
                }
                vol_of(&mut g, &mut t, s);
                let labels: &[f32] = &[0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 7.0, -1.0, 250.0, 1.0, -5.0];
                fill_paint(&mut g, &mut t, s, &[&[0.0, 0.0, 1.0, 2.0]], &[labels]);
                let v = !g.chance(12);
                t.push(i64::from(v).to_string());
                if v {
                    let mut o = like(&mut g, s);
                    if !g.chance(4) {
                        o.ty = 3;
                    }
                    vol_of(&mut g, &mut t, o);
                    let ints: &[f32] =
                        &[10.0, 20.0, 30.0, 40.0, 50.0, 100.0, 0.0, 25.5, -5.0, 35.0];
                    fill_paint(&mut g, &mut t, o, &[&[20.0, 40.0, 10.0, 100.0]], &[ints]);
                }
                t.push(
                    g.pick(&[0i64, 1, 1, 2, 2, 3, 7, -1, 250, 5, -5])
                        .to_string(),
                );
                t.push(
                    g.pick(&[8i64, 8, 8, 256, 256, 20000, 4, 3, 1, 0, -1, -3, 1 << 30])
                        .to_string(),
                );
                for _ in 0..2 {
                    // the mixing coefficients, the neighbour labels
                    let d = g.chance(2);
                    t.push(i64::from(d).to_string());
                    if d {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                    }
                }
                let b = g.chance(3);
                t.push(i64::from(b).to_string());
                if b {
                    // frame 0 the border flags, the later frames the neighbours' labels
                    let mut o = like(&mut g, s);
                    o.nf = g.int(1, 4);
                    o.ty = g.pick(&[1, 3, 0]);
                    vol_of(&mut g, &mut t, o);
                    let flags: &[f32] = &[0.0, 1.0, 1.0, 2.0];
                    fill_paint(&mut g, &mut t, o, &[&[0.0, 1.0], labels], &[flags, labels]);
                }
            }
            "randn" | "const" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(3);
                if cmd == "randn" && !g.chance(5) {
                    s.ty = 3;
                }
                let given = g.chance(2);
                let mut dims = [s.w, s.h, s.d, s.nf];
                if !given || g.chance(6) {
                    dims = [g.int(0, 4), g.int(1, 4), g.int(1, 3), g.int(1, 2)];
                }
                for v in dims {
                    t.push(v.to_string());
                }
                if cmd == "randn" {
                    t.push(fx(g.pick(&[0.0f32, 1.0, 100.0, -5.5, 1e30])));
                    t.push(fx(g.pick(&[1.0f32, 0.0, 2.5, 1e-3, 10.0])));
                } else {
                    let v = if g.chance(3) {
                        g.pick(&[300.0f32, -40000.0, 70000.0, -1.0, 255.5, 3e9])
                    } else {
                        fval(&mut g)
                    };
                    t.push(fx(v));
                    t.push(fx(0.0));
                }
                t.push(g.int(1, 1000).to_string());
                t.push(i64::from(given).to_string());
                if given {
                    vol_of(&mut g, &mut t, s);
                }
            }
            "nseq" | "nfr" | "sort" | "revsl" | "cdtype" | "nonmax" | "xrpad" | "ordidx"
            | "vperm" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(3);
                if matches!(cmd, "nseq" | "nfr" | "sort") && g.chance(2) {
                    s.nf = g.int(1, if cmd == "sort" { 8 } else { 4 });
                }
                if matches!(cmd, "nonmax" | "xrpad") && g.chance(2) {
                    (s.w, s.h, s.d) = (g.int(2, 6), g.int(2, 6), g.int(2, 5));
                }
                if cmd == "ordidx" && g.chance(10) {
                    (s.w, s.h, s.d) = (g.int(250, 300), 1, 1);
                }
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[
                    0.0, 1.0, 2.0, 3.5, -1.0, 100.0, 255.0, 300.0, -2.0, 7.0, 40.0,
                ];
                fill_paint(&mut g, &mut t, s, &[&[0.0, 1.0, 10.0, 2.5]], &[vals]);
                match cmd {
                    "nseq" | "nfr" => {
                        t.push(g.int(0, 2).to_string());
                        t.push(fx(g.pick(&[1.0f32, 100.0, 0.0, -2.0, 1e-30, 1e6])));
                    }
                    "sort" => {
                        let m = g.chance(3);
                        t.push(i64::from(m).to_string());
                        if m {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                            let mv: &[f32] = &[0.0, 1.0, 0.5, 0.25, 0.75];
                            fill_paint(&mut g, &mut t, o, &[mv], &[mv]);
                        }
                        let d = g.chance(3);
                        t.push(i64::from(d).to_string());
                        if d {
                            let mut o = like(&mut g, s);
                            if g.chance(3) {
                                o.nf = s.nf;
                            }
                            vol_of(&mut g, &mut t, o);
                        }
                    }
                    "revsl" => {
                        let nf = s.nf.max(1);
                        for cols in [1i64, 3] {
                            let b = g.chance(3);
                            t.push(i64::from(b).to_string());
                            if b {
                                let rows = g.pick(&[nf, nf, nf + 1, (nf - 1).max(1)]);
                                mat_toks(&mut g, &mut t, rows, cols);
                            }
                        }
                        t.push(g.int(0, 3).to_string());
                        let d = g.chance(3);
                        t.push(i64::from(d).to_string());
                        if d {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                    }
                    "cdtype" => t.push(g.pick(&[0i64, 1, 3, 4, 10, 2, s.ty]).to_string()),
                    "nonmax" => {
                        let d = g.chance(3);
                        t.push(i64::from(d).to_string());
                        if d {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                        t.push(fx(g.pick(&[0.0f32, 1.0, -1.0, 10.0, 50.0, -200.0])));
                        t.push(g.pick(&[1i64, 1, -1, 0, 2]).to_string());
                    }
                    "xrpad" => {
                        let r = !g.chance(5);
                        t.push(i64::from(r).to_string());
                        if r {
                            for _ in 0..3 {
                                t.push(g.int(0, 2).to_string());
                            }
                            for _ in 0..3 {
                                t.push(g.pick(&[1i64, 2, 3, 2, 4]).to_string());
                            }
                        }
                        t.push(g.pick(&[0i64, 1, 2, 1]).to_string());
                    }
                    _ => t.push(g.int(1, 1000).to_string()),
                }
            }
            "rms" | "vfrac" | "cdt" | "diffv" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(3);
                if cmd == "rms" && g.chance(2) {
                    s.nf = g.int(1, 4);
                }
                if cmd == "vfrac" {
                    s.nf = 1;
                }
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[0.0, 1.0, 2.0, -1.0, -3.0, 3.5, 10.0, -0.5, 100.0];
                match cmd {
                    "rms" => {
                        fill_paint(&mut g, &mut t, s, &[&[1.0, 2.0, -3.0]], &[vals]);
                        let mut o = like(&mut g, s);
                        if g.chance(2) {
                            o.ty = 3;
                        }
                        vol_of(&mut g, &mut t, o);
                    }
                    "vfrac" => {
                        // the vox2vox matrix: identity, a scaling or shift, sometimes singular
                        let k = g.int(0, 3);
                        t.push("4".into());
                        t.push("4".into());
                        for r in 0..4 {
                            for c in 0..4 {
                                let v: f32 = match (k, r, c) {
                                    (_, 3, 3) => 1.0,
                                    (_, 3, _) => 0.0,
                                    (3, _, _) => 0.0,
                                    (0, r, c) if r == c => 1.0,
                                    (1, r, c) if r == c => g.pick(&[0.5, 2.0, 1.0]),
                                    (2, r, c) if r == c => 1.0,
                                    (2, _, 3) => g.pick(&[0.0, 1.0, -1.0, 0.4]),
                                    _ => 0.0,
                                };
                                t.push(fx(v));
                            }
                        }
                        let mut sg = like(&mut g, s);
                        sg.nf = 1;
                        vol_of(&mut g, &mut t, sg);
                        let labs: &[f32] = &[0.0, 1.0, 2.0, 3.0, 1.0, 2.0, -1.0];
                        fill_paint(&mut g, &mut t, sg, &[labs], &[labs]);
                        let mut fr = like(&mut g, s);
                        fr.nf = g.int(1, 3);
                        if g.chance(2) {
                            fr.ty = 3;
                        }
                        vol_of(&mut g, &mut t, fr);
                        fill_paint(&mut g, &mut t, fr, &[&[0.0]], &[&[0.0, 1.0]]);
                    }
                    "cdt" => {
                        fill_paint(&mut g, &mut t, s, &[vals], &[vals]);
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        fill_paint(&mut g, &mut t, o, &[vals], &[vals]);
                        let d = g.chance(3);
                        t.push(i64::from(d).to_string());
                        if d {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                    }
                    _ => {
                        // diffv
                        fill_paint(&mut g, &mut t, s, &[&[1.0, 0.0]], &[vals]);
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        fill_paint(&mut g, &mut t, o, &[&[1.0, 0.0]], &[vals]);
                        let mv: &[f32] = &[0.0, 1.0, 0.5, 0.25];
                        for k in 0..2 {
                            let m = g.chance(3);
                            t.push(i64::from(m).to_string());
                            if m {
                                let o = like(&mut g, s);
                                vol_of(&mut g, &mut t, o);
                                fill_paint(&mut g, &mut t, o, &[mv], &[mv]);
                            }
                            if k == 0 {
                                t.push(dx(g.pick(&[0.0, 0.5, 1.0, 3.0, -1.0])));
                            }
                        }
                        let mode = g.int(0, 2);
                        t.push(mode.to_string());
                        if mode == 1 {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                        t.push(i64::from(g.chance(3)).to_string());
                        let extra = g.pick(&[0i64, 0, 1, 2]);
                        t.push(extra.to_string());
                        for _ in 0..extra {
                            t.push(dx(g.pick(&[
                                0.0,
                                1.0,
                                -1.0,
                                s.w as f64,
                                (s.w - 1) as f64,
                                0.5,
                            ])));
                            t.push(dx(g.pick(&[0.0, 1.0, s.h as f64, (s.h - 1) as f64])));
                            t.push(dx(g.pick(&[0.0, s.d as f64, (s.d - 1) as f64])));
                            t.push(dx(g.pick(&[0.0, 7.0])));
                            t.push(dx(g.pick(&[9.0, -2.5, 1e40])));
                        }
                    }
                }
            }
            "fpow" | "mbin" | "reshape" | "mmax" | "mconst" | "fbin" | "vote" | "msq" | "crop"
            | "revs" | "cutend" | "segids" | "hcos" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(3);
                if matches!(cmd, "vote" | "fbin" | "fpow") && g.chance(2) {
                    s.nf = g.int(1, 6);
                }
                if matches!(cmd, "fpow") && g.chance(2) {
                    s.ty = 3;
                }
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[0.0, 1.0, 2.0, 3.0, -1.0, 4.0, 0.5, 2.0, 7.0, 100.0];
                fill_paint(&mut g, &mut t, s, &[&[0.0, 1.0, 2.0, 4.0]], &[vals]);
                let opt = |g: &mut Gen, t: &mut Vec<String>, s: Shape| {
                    let d = g.chance(3);
                    t.push(i64::from(d).to_string());
                    if d {
                        let o = like(g, s);
                        vol_of(g, t, o);
                    }
                };
                let mask = |g: &mut Gen, t: &mut Vec<String>, s: Shape| {
                    let m = g.chance(3);
                    t.push(i64::from(m).to_string());
                    if m {
                        let o = like(g, s);
                        vol_of(g, t, o);
                        let mv: &[f32] = &[0.0, 1.0, 0.5, 0.75, 0.25];
                        fill_paint(g, t, o, &[mv], &[mv]);
                    }
                };
                match cmd {
                    "fpow" => {
                        let n = s.nf.max(1);
                        t.push(n.to_string());
                        for _ in 0..n {
                            t.push(fx(
                                g.pick(&[1.0f32, 0.5, 2.0, 3.0, -1.0, 0.0, 0.25, 1.000001])
                            ));
                        }
                    }
                    "mbin" => {
                        let mode = g.int(0, 2);
                        t.push(mode.to_string());
                        if mode == 1 {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                        }
                        t.push(fx(g.pick(&[0.0f32, 1.0, 0.5, -1.0, 2.5])));
                        let ht = g.chance(4) || !g.chance(3);
                        t.push(i64::from(ht).to_string());
                        if ht {
                            let w = g.pick(&[
                                "positive", "neg", "ABS", "Absolute", "po", "x", "negative",
                            ]);
                            t.push(hex(w.as_bytes()));
                        }
                        t.push(i64::from(g.chance(2)).to_string());
                    }
                    "reshape" => {
                        let n = s.w * s.h * s.d * s.nf.max(1);
                        let sh = if g.chance(5) {
                            [g.int(1, 3), g.int(1, 3), g.int(1, 2), g.int(1, 2)]
                        } else if g.chance(2) {
                            [n, 1, 1, 1]
                        } else {
                            [1, n, 1, 1]
                        };
                        for v in sh {
                            t.push(v.to_string());
                        }
                    }
                    "mmax" => {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        fill_paint(&mut g, &mut t, o, &[vals], &[vals]);
                        opt(&mut g, &mut t, s);
                    }
                    "mconst" => {
                        t.push(dx(g.pick(&[2.0, -1.5, 0.0, 1e300, 0.1])));
                        t.push(i64::from(g.chance(2)).to_string());
                        opt(&mut g, &mut t, s);
                    }
                    "fbin" => {
                        t.push(dx(g.pick(&[0.0, 0.5, 1.0, 3.0, -1.0])));
                        mask(&mut g, &mut t, s);
                    }
                    "vote" => {
                        mask(&mut g, &mut t, s);
                        let d = g.chance(4);
                        t.push(i64::from(d).to_string());
                        if d {
                            let mut o = like(&mut g, s);
                            if g.chance(2) {
                                (o.nf, o.ty) = (2, s.ty);
                            }
                            vol_of(&mut g, &mut t, o);
                        }
                        t.push(i64::from(g.chance(2)).to_string());
                    }
                    "msq" => {
                        t.push(g.int(0, 2).to_string());
                        mask(&mut g, &mut t, s);
                        opt(&mut g, &mut t, s);
                    }
                    "crop" => {
                        for k in 0..2 {
                            let _ = k;
                            t.push(g.int(-1, s.w).to_string());
                            t.push(g.int(-1, s.h).to_string());
                            t.push(g.int(0, s.d).to_string());
                        }
                    }
                    "revs" | "hcos" => {
                        if cmd == "hcos" {
                            t.push(dx(g.pick(&[0.5, 1.0, 0.0, 2.0, -1.0])));
                        }
                        opt(&mut g, &mut t, s);
                    }
                    "cutend" => t.push(g.pick(&[0i64, 1, 1, 2, -1]).to_string()),
                    _ => {
                        // segids
                        t.push(frame_of(&mut g, s).to_string());
                        t.push(i64::from(g.chance(2)).to_string());
                    }
                }
            }
            "vv" | "vvsm" | "vvtk" | "vvk" | "vvd" | "maskup" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(4);
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[0.0, 1.0, 2.0, 3.0, -1.0, 4.0, 0.5, 100.0, 7.0];
                if cmd != "vvd" {
                    fill_paint(&mut g, &mut t, s, &[&[0.0, 1.0, 2.0, 5.0]], &[vals]);
                }
                match cmd {
                    "vv" | "vvsm" | "vvtk" => {
                        t.push(dx(g.pick(&[0.0, 0.0, -1.0, 7.5, 1e30])));
                        let mut o = like(&mut g, s);
                        if !g.chance(8) {
                            o.nf = s.nf;
                        }
                        o.hdr = o.hdr && g.chance(4);
                        vol_of(&mut g, &mut t, o);
                        fill_paint(&mut g, &mut t, o, &[&[0.0, 9.0]], &[vals]);
                        m44(&mut g, &mut t);
                        t.push(g.pick(&[0, 0, 1, 1, 1, 2, 3, 7]).to_string());
                        t.push(fx(g.pick(&[0.0f32, 3.0, 1.0, 2.5, -1.0])));
                        t.push(i64::from(g.chance(8)).to_string());
                        t.push(i64::from(g.chance(8)).to_string());
                        if cmd != "vv" {
                            let has = !g.chance(5);
                            t.push(i64::from(has).to_string());
                            if has {
                                let mut v = like(&mut g, s);
                                if g.chance(8) {
                                    v.ty = g.pick(&[9, 7, 2]);
                                }
                                v.hdr = v.hdr && g.chance(4);
                                vol_of(&mut g, &mut t, v);
                                let sh: &[f32] = &[0.0, 0.5, -1.0, 1.0, 0.25, 2.0, -0.75];
                                fill_paint(&mut g, &mut t, v, &[sh], &[sh]);
                                t.push(dx(g.pick(&[0.0, 0.0, 1.0, -2.0])));
                            }
                            let l = g.chance(3);
                            t.push(i64::from(l).to_string());
                            if l {
                                t.push("0".into());
                                m44_elems(&mut g, &mut t);
                            }
                            t.push(g.pick(&[1, -1, 2, -2, 3, -3, 1, 2, 0, 4]).to_string());
                        }
                    }
                    "vvk" => {
                        let mut o = like(&mut g, s);
                        o.nf = if g.chance(6) { g.int(1, 9) } else { 8 };
                        o.hdr = o.hdr && g.chance(4);
                        vol_of(&mut g, &mut t, o);
                        fill_paint(&mut g, &mut t, o, &[&[0.0, 9.0]], &[vals]);
                        m44(&mut g, &mut t);
                    }
                    "vvd" => {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        m44(&mut g, &mut t);
                    }
                    _ => {
                        let m = g.chance(2);
                        t.push(i64::from(m).to_string());
                        if m {
                            let o = like(&mut g, s);
                            vol_of(&mut g, &mut t, o);
                            let mv: &[f32] = &[0.0, 1.0, 0.0, 2.0];
                            fill_paint(&mut g, &mut t, o, &[mv], &[mv]);
                        }
                        t.push(g.pick(&[1, 2, 0, 3, 2]).to_string());
                        t.push(g.pick(&[0, 1, 2, -1, 0]).to_string());
                        t.push(i64::from(g.chance(2)).to_string());
                        t.push(i64::from(g.chance(8)).to_string());
                    }
                }
            }
            "cov" | "mpca" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(4);
                if g.chance(2) {
                    s.nf = g.int(1, 5);
                }
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[0.0, 1.0, 2.0, 3.0, -1.0, 4.0, 0.5, 100.0, -7.0];
                fill_paint(&mut g, &mut t, s, &[&[0.0, 1.0, 2.0, 5.0]], &[vals]);
                let m = g.chance(2);
                t.push(i64::from(m).to_string());
                if m {
                    let o = like(&mut g, s);
                    vol_of(&mut g, &mut t, o);
                    let mv: &[f32] = &[0.0, 1.0, 0.5, 0.75, 1.0];
                    fill_paint(&mut g, &mut t, o, &[mv], &[mv]);
                }
            }
            "pcast" => {
                let n = g.int(1, 6);
                t.push(n.to_string());
                for _ in 0..n {
                    let v = if g.chance(10) {
                        fval(&mut g)
                    } else {
                        g.pick(&[0.0f32, 1.0, 2.5, 10.0, 1e-3, -1.0, 1e20, 3.3, 0.7])
                    };
                    t.push(fx(v));
                }
                t.push(g.pick(&[0, 1, 1, 2]).to_string());
            }
            "fdr" => {
                let n = if g.chance(4) { g.int(2, 3) } else { 1 };
                t.push(n.to_string());
                let pv: &[f32] = &[
                    0.001, 0.01, 0.04, 0.05, 0.2, 0.5, 0.9, 1.0, 2.0, 3.0, -2.0, -0.5, 0.0, 1.3,
                ];
                for _ in 0..n {
                    let mut s = shape(&mut g);
                    s.hdr = s.hdr && g.chance(4);
                    if !g.chance(8) {
                        s.ty = 3;
                    }
                    vol_of(&mut g, &mut t, s);
                    fill_paint(&mut g, &mut t, s, &[pv], &[pv]);
                    t.push(frame_of(&mut g, s).to_string());
                    let m = g.chance(3);
                    t.push(i64::from(m).to_string());
                    if m {
                        let o = like(&mut g, s);
                        vol_of(&mut g, &mut t, o);
                        let mv: &[f32] = &[0.0, 1.0, 0.5, 1.0];
                        fill_paint(&mut g, &mut t, o, &[mv], &[mv]);
                    }
                    let mode = g.pick(&[0, 1, 1, 2]);
                    t.push(mode.to_string());
                    if mode == 1 {
                        let mut o = like(&mut g, s);
                        if !g.chance(8) {
                            o.ty = 3;
                        }
                        o.hdr = o.hdr && g.chance(4);
                        vol_of(&mut g, &mut t, o);
                    }
                }
                t.push(dx(g.pick(&[0.05, 0.01, 0.2, 0.0, 1.0, 0.5])));
                t.push(g.pick(&[0, 1, -1, 0, 2]).to_string());
                t.push(i64::from(g.chance(2)).to_string());
                t.push(i64::from(g.chance(5)).to_string());
                t.push(i64::from(!g.chance(4)).to_string());
                t.push(i64::from(!g.chance(4)).to_string());
            }
            "rgbb" | "vxn" => {
                let mut s = shape(&mut g);
                s.hdr = s.hdr && g.chance(6);
                if (cmd == "rgbb" && g.chance(2)) || (cmd == "vxn" && !g.chance(4)) {
                    (s.w, s.h, s.d) = (g.int(1, 9), g.int(1, 9), g.int(1, 6));
                }
                vol_of(&mut g, &mut t, s);
                let vals: &[f32] = &[0.0, 0.0, 1.0, 2.0, 3.0, -1.0, 4.0, 0.5, 100.0, 1e-9];
                fill_paint(&mut g, &mut t, s, &[&[0.0, 1.0, 2.0]], &[vals]);
                if cmd == "rgbb" {
                    let kind = g.int(0, 2);
                    t.push(kind.to_string());
                    let pads = if kind == 1 { 6 } else { 1 };
                    for _ in 0..pads {
                        t.push(g.pick(&[0, 1, 2, -1, 3, 0]).to_string());
                    }
                } else {
                    let n = g.int(1, 4);
                    t.push(n.to_string());
                    for _ in 0..n {
                        let ws = g.pick(&[3, 3, 1, 5, 7, 8, 2, -3, 0, 4]);
                        let wh = (ws / 2).max(0);
                        // inside, the window clear of the far faces (FS-485), or anywhere
                        let inner = !g.chance(4);
                        for e in [s.w, s.h, s.d] {
                            let hi = if inner { (e - 1 - wh).max(0) } else { e };
                            t.push(g.int(if inner { 0 } else { -1 }, hi).to_string());
                        }
                        t.push(ws.to_string());
                        t.push(frame_of(&mut g, s).to_string());
                    }
                }
            }
            "talcras" | "talmat" | "talpt" | "talvol" => r22_gen(&mut g, &mut t, cmd),
            "dilate" | "dilu" | "msegs" | "sstat" | "hseg" => r22_gen_segs(&mut g, &mut t, cmd),
            "conform" => r23_gen_conform(&mut g, &mut t),
            "union" | "isect" | "mand" | "mxor" => r23_gen_set(&mut g, &mut t, cmd),
            c if R23_SET.contains(&c) => r23_gen_set(&mut g, &mut t, c),
            c if R23_MASK.contains(&c) => r23_gen_mask(&mut g, &mut t, c),
            c if R24_HISTO.contains(&c) => r24_gen_histo(&mut g, &mut t, c),
            c if R25_LTA.contains(&c) => r25_gen_lta(&mut g, &mut t, c),
            c if R25_RES.contains(&c) => r25_gen_res(&mut g, &mut t, c),
            c if R25_MRI2.contains(&c) => r25_gen_mri2(&mut g, &mut t, c),
            c => panic!("no generator for {c}"),
        }
        lines.push(t.join(" "));
    }
    lines
}

/// `talairachex.cpp`'s cases (relay 22): see `fsvolume.cpp`'s `talcras`.
fn r22_gen(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let kind = match cmd {
        "talcras" => 0,
        "talmat" => g.int(0, 3),
        "talpt" => g.int(0, 4),
        _ => g.int(0, 2),
    };
    if cmd != "talcras" {
        t.push(kind.to_string());
    }
    if cmd == "talpt" && kind == 4 {
        let nan = g.chance(3);
        if nan {
            for _ in 0..16 {
                t.push(fx(fval(g)));
            }
        } else {
            m44_elems(g, t);
        }
    } else {
        let mut s = shape(g);
        s.hdr = s.hdr && g.chance(6);
        vol_of(g, t, s);
        if cmd == "talvol" {
            let vals: &[f32] = &[0.0, 1.0, 2.0, 3.0, -1.0, 4.0, 0.5, 100.0, 7.0];
            fill_paint(g, t, s, &[&[0.0, 1.0, 2.0, 5.0]], &[vals]);
            // the second volume: the Talairach volume (kinds 0, 1) or the target (kind 2)
            let given = if kind == 2 { !g.chance(8) } else { g.chance(2) };
            t.push(i64::from(given).to_string());
            if given {
                let mut o = like(g, s);
                o.hdr = false;
                vol_of(g, t, o);
            }
        }
        let has = !g.chance(4);
        t.push(i64::from(has).to_string());
        if has {
            let n = g.pick(&[1i64, 1, 1, 1, 1, 2, 0]);
            t.push(n.to_string());
            // LINEAR_RAS_TO_RAS mostly
            t.push(g.pick(&[1i64, 1, 1, 1, 0, 2]).to_string());
            if n >= 1 {
                if g.chance(12) {
                    for _ in 0..16 {
                        t.push(fx(fval(g)));
                    }
                } else {
                    m44_elems(g, t);
                }
                geom(g, t);
            }
        }
        t.push(i64::from(g.chance(4)).to_string());
        t.push(i64::from(g.chance(6)).to_string());
        t.push(i64::from(g.chance(4)).to_string());
        if cmd == "talvol" && kind == 0 {
            // not SAMPLE_CUBIC_BSPLINE (5): fs.bspline is not ported yet
            t.push(g.pick(&[0i64, 1, 1, 2, 3, 4]).to_string());
        }
    }
    if cmd == "talpt" {
        let n = g.int(1, 3);
        t.push(n.to_string());
        for _ in 0..n {
            for _ in 0..3 {
                let v = if g.chance(12) {
                    f64::from_bits(g.pick(&[0x7ff8_0000_0000_0123u64, 0xfff8_0000_0000_0007]))
                } else {
                    coord(g)
                };
                t.push(dx(v));
            }
        }
    }
}

/// The cases of `MRIdilate`, `MRIdilateUchar`, `MRImatchSegs`, `MRIsegStats` and `HISTOseg`
/// (relay 22, for fs.filter).
fn r22_gen_segs(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let mut s = shape(g);
    s.hdr = s.hdr && g.chance(6);
    if cmd == "dilu" && !g.chance(8) {
        s.ty = 0;
    }
    if g.chance(2) {
        (s.w, s.h, s.d) = (g.int(1, 6), g.int(1, 6), g.int(1, 4));
    }
    vol_of(g, t, s);
    let labels: &[f32] = &[0.0, 0.0, 2.0, 3.0, 41.0, 1.0, -1.0, 0.5, 255.0, 300.0];
    fill_paint(g, t, s, &[&[0.0, 0.0, 1.0, 2.0, 41.0]], &[labels]);
    match cmd {
        "dilate" | "dilu" | "msegs" => {
            let mode = g.pick(&[0, 0, 1, 2]);
            t.push(mode.to_string());
            if mode == 1 {
                let mut o = like(g, s);
                o.hdr = o.hdr && g.chance(4);
                vol_of(g, t, o);
            }
            if cmd == "msegs" {
                let n = g.pick(&[1i64, 2, 3, 0, 1]);
                t.push(n.to_string());
                for _ in 0..n {
                    t.push(g.pick(&[0i64, 2, 3, 41, 1, 255, -1, 300]).to_string());
                }
                t.push(g.pick(&[1i64, 1, 255, 0, -3, 70000]).to_string());
            }
        }
        _ => {
            let mut v = like(g, s);
            v.hdr = v.hdr && g.chance(6);
            vol_of(g, t, v);
            let vals: &[f32] = &[0.0, 1.0, 2.5, -3.0, 100.0, 7.25, 1e-3, 1e6];
            fill_paint(g, t, v, &[&[0.0, 5.0, 2.0]], &[vals]);
            t.push(g.pick(&[0i64, 2, 3, 41, 1, 0, 2]).to_string());
            if cmd == "sstat" {
                t.push(frame_of(g, v).to_string());
            } else {
                let (bmin, bmax) = g.pick(&[(0.0, 10.0), (-5.0, 5.0), (0.0, 100.0), (2.0, 1.0)]);
                t.push(dx(bmin));
                t.push(dx(bmax));
                t.push(dx(g.pick(&[1.0, 0.5, 2.0, 3.0, -1.0, 1.0])));
            }
        }
    }
}

/// Labels the segmentation cases draw from: cma.h's (left and right), the parcellations', a few
/// unknown ones.
const CMA_LABELS: [i64; 48] = [
    0, 2, 3, 41, 42, 4, 43, 10, 49, 17, 53, 30, 62, 5, 7, 8, 11, 12, 13, 14, 15, 16, 18, 24, 26,
    28, 31, 46, 47, 50, 51, 52, 54, 58, 60, 63, 72, 77, 78, 79, 80, 85, 251, 255, 1001, 2035, 5001,
    9999,
];

/// A segmentation's voxels painted: `n`, then `x y z value` of frame 0 (values from `vals`, or
/// odd ones).
fn paint(g: &mut Gen, t: &mut Vec<String>, s: Shape, vals: &[f32]) {
    let n = g.int(0, (s.w * s.h * s.d).min(24));
    t.push(n.to_string());
    for _ in 0..n {
        t.push(g.int(0, s.w - 1).to_string());
        t.push(g.int(0, s.h - 1).to_string());
        t.push(g.int(0, s.d - 1).to_string());
        t.push(fx(if g.chance(12) { fval(g) } else { g.pick(vals) }));
    }
}

/// `fsvolume.cpp`'s `fill_paint`'s tokens: `k` frames filled (each with a value of `fills[f]`, the
/// last list for later frames), then `n` voxels painted (`x y z frame value`, a value of
/// `vals[frame]` or an odd one); none for a volume without voxels or frames.
fn fill_paint(g: &mut Gen, t: &mut Vec<String>, s: Shape, fills: &[&[f32]], vals: &[&[f32]]) {
    let has = !s.hdr && s.nf > 0;
    let k = if has && !g.chance(5) {
        g.int(1, s.nf)
    } else {
        0
    };
    t.push(k.to_string());
    for f in 0..k {
        let l = fills[(f as usize).min(fills.len() - 1)];
        t.push(fx(g.pick(l)));
    }
    let n = if has {
        g.int(0, (s.w * s.h * s.d).min(24))
    } else {
        0
    };
    t.push(n.to_string());
    for _ in 0..n {
        t.push(g.int(0, s.w - 1).to_string());
        t.push(g.int(0, s.h - 1).to_string());
        t.push(g.int(0, s.d - 1).to_string());
        let f = if g.chance(4) { g.int(0, s.nf - 1) } else { 0 };
        t.push(f.to_string());
        let l = vals[(f as usize).min(vals.len() - 1)];
        t.push(fx(if g.chance(12) { fval(g) } else { g.pick(l) }));
    }
}

/// A 4 x 4 matrix's tokens (`fsvolume.cpp`'s `get_m44`): 0 for `NULL`, else 1 and the elements.
fn m44(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(4) {
        t.push("0".into());
        return;
    }
    t.push("1".into());
    m44_elems(g, t);
}

/// A voxel-to-voxel-like 4 x 4 matrix's elements row by row (sometimes anything).
fn m44_elems(g: &mut Gen, t: &mut Vec<String>) {
    let wild = g.chance(10);
    for r in 0..4 {
        for c in 0..4 {
            let v = if wild {
                num(g, -4, 4, 4)
            } else if r == 3 {
                if c == 3 { 1.0 } else { 0.0 }
            } else if c == 3 {
                g.pick(&[0.0f32, 0.0, 0.5, -0.5, 1.0, -1.0, 0.25, 1.5])
            } else if r == c {
                g.pick(&[1.0f32, 1.0, 1.0, 0.5, 2.0, -1.0, 0.9])
            } else {
                g.pick(&[0.0f32, 0.0, 0.0, 0.0, 0.25, -0.1])
            };
            t.push(fx(v));
        }
    }
}

/// A segmentation volume (rarely header only) painted with labels.
fn seg_vol(g: &mut Gen, t: &mut Vec<String>) -> Shape {
    let s = shape(g);
    let s = Shape {
        hdr: s.hdr && g.chance(4),
        ..s
    };
    vol_of(g, t, s);
    let vals: Vec<f32> = CMA_LABELS.iter().map(|&l| l as f32).collect();
    paint(g, t, s, &vals);
    s
}

/// A `brainvol.stats` text: `CacheBrainVolumeStats`'s lines, some changed (numbers in other forms,
/// words missing, lines dropped or doubled, white space at the ends, CR line ends).
fn brainvol_text(g: &mut Gen) -> Vec<u8> {
    let names = [
        "BrainSeg, BrainSegVol",
        "BrainSegNotVent, BrainSegVolNotVent",
        "SupraTentorial, SupraTentorialVol",
        "SupraTentorialNotVent, SupraTentorialVolNotVent",
        "SubCortGray, SubCortGrayVol",
        "lhCortex, lhCortexVol",
        "rhCortex, rhCortexVol",
        "Cortex, CortexVol",
        "TotalGray, TotalGrayVol",
        "lhCerebralWhiteMatter, lhCerebralWhiteMatterVol",
        "rhCerebralWhiteMatter, rhCerebralWhiteMatterVol",
        "CerebralWhiteMatter, CerebralWhiteMatterVol",
        "Mask, MaskVol",
        "SupraTentorialNotVentVox, SupraTentorialVolNotVentVox",
        "BrainSegNotVentSurf, BrainSegVolNotVentSurf",
        "VentricleChoroidVol, VentricleChoroidVol",
    ];
    let mut s = String::new();
    for name in names {
        if g.chance(30) {
            continue;
        }
        let num = if g.chance(4) {
            g.pick(&[
                "1e5", "-3.5", "+7", ".5", "5.", "0x10", "1e", "1e+", "1e400", "1e-400", "abc", "",
                "007.25", "1.5.3", "inf", "nan", "4e2x", "-", "12",
            ])
            .to_owned()
        } else {
            format!("{:.12}", g.int(0, 2_000_000) as f64 / 7.0)
        };
        let mut line = format!("# Measure {name}, a description, {num}, mm^3");
        if g.chance(8) {
            line = format!("# Measure {name}, no number here");
        }
        if g.chance(15) {
            line += g.pick(&[" ", "\r", "\t ", "  "]);
        }
        s += &line;
        s += "\n";
        if g.chance(20) {
            s += &line;
            s += "\n";
        }
    }
    if g.chance(10) {
        s.pop();
    }
    s.into_bytes()
}

/// A row of four numbers as a transform file writes them (sometimes odd).
fn num_row(g: &mut Gen) -> String {
    let mut s = String::new();
    if g.chance(8) {
        s += "  ";
    }
    for i in 0..4 {
        let v = if g.chance(25) {
            g.pick(&["nan", "1e40", "-inf", "abc", "0x1p3", ""])
                .to_owned()
        } else {
            format!(
                "{}",
                g.int(-2000, 2000) as f64 / g.pick(&[1.0, 7.0, 1000.0, 3.0])
            )
        };
        s += &v;
        if i < 3 {
            s += g.pick(&[" ", " ", "\t", "  "]);
        }
    }
    s += g.pick(&["\n", "\n", ";\n", " # c\n", "\n\n"]);
    s
}

/// A MINC `.xfm` text: its header lines (the volume information in several forms), its rows.
fn xfm_text(g: &mut Gen) -> String {
    let mut s = String::from(if g.chance(15) {
        "MNI Transform Fil\n"
    } else {
        "MNI Transform File\n"
    });
    s += g.pick(&[
        "% avi2talxfm\n",
        "%Generated by lta_convert src none dst none\n",
        "%Generated by x src average_305.mnc dst nofile.mnc\n",
        "%Volume: a.mnc\n%Volume: average_305.mnc\n",
        "% minctracc -clobber o.mnc average_305_8.mnc t.xfm\n",
        "% minctracc only.mnc\n",
        "%Generated by x src\n",
        "",
        "%a\n%b\n%c\n",
    ]);
    if g.chance(3) {
        s += "\n";
    }
    if !g.chance(8) {
        s += "Transform_Type = Linear;\n";
    }
    if !g.chance(8) {
        s += "Linear_Transform = \n";
    }
    for _ in 0..g.int(2, 3) {
        s += &num_row(g);
    }
    if g.chance(15) {
        s += "%tail\n";
    }
    s
}

/// A FreeSurfer `.lta` text: header, transforms, volume information, extras.
fn lta_text(g: &mut Gen) -> String {
    let nx = g.pick(&[1, 1, 1, 2, 0]);
    let mut s = String::new();
    if !g.chance(12) {
        s += &format!("type      = {}\n", g.pick(&[0, 1, 1, 14]));
    }
    if !g.chance(15) {
        s += &format!("nxforms   = {nx}\n");
    }
    for _ in 0..nx {
        s += "mean      = 0.0000 0.0000 0.0000\n";
        s += g.pick(&[
            "sigma     = 1.0000\n",
            "sigma     = 10000.0\n",
            "sigma = x\n",
        ]);
        s += g.pick(&["1 4 4\n", "1 4 4\n", "1 3 3\n"]);
        for _ in 0..4 {
            s += &num_row(g);
        }
    }
    for k in 0..nx {
        if g.chance(4) {
            continue;
        }
        for which in ["src", "dst"] {
            s += &format!("{which} volume info\n");
            s += &format!("valid = {}  # volume info valid\n", g.int(0, 1));
            s += g.pick(&["filename = /a/b.mgz\n", "filename = \n"]);
            s += &format!("volume = {} 256 {}\n", 128 + k, g.int(1, 300));
            s += "voxelsize = 1.000000000000000e+00 1.000000000000000e+00 1.200000047683716e+00\n";
            s += "xras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\n";
            s += "yras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00\n";
            s += "zras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00\n";
            s += "cras   = 1.500000000000000e+00 -2.000000000000000e+01 3.000000000000000e+00\n";
        }
    }
    if g.chance(2) {
        s += g.pick(&["subject bert\n", "subject\n", "subject a b\n"]);
    }
    if g.chance(2) {
        s += g.pick(&["fscale 0.1\n", "fscale x\n", "fscale 1e50\n"]);
    }
    s
}

/// A sampler's coordinate on an axis of `n` voxels: inside, at the edges, whole or within
/// `FLT_EPSILON` of whole, outside, NaN and infinite.
fn scoord(g: &mut Gen, n: i64) -> f64 {
    if g.chance(25) {
        return g.pick(&[f64::NAN, f64::INFINITY, -f64::INFINITY, 1e10, -0.0, 3e9]);
    }
    let k = g.int(-1, n) as f64;
    if g.chance(6) {
        return k;
    }
    if g.chance(10) {
        return k + g.pick(&[1e-8, -1e-8, 1e-7, 1.25e-7, -3e-7]);
    }
    if g.chance(5) {
        return n as f64 - 1.0 + g.pick(&[0.2, 0.4, 0.49, 0.5, 0.51, 0.6, 0.75]);
    }
    if g.chance(8) {
        return g.pick(&[-0.3, -0.49, -0.5, -0.51, -0.75]);
    }
    k + g.pick(&[0.5, 0.25, 0.75, 0.1, 0.9, 0.3, 0.6, 0.45, 0.55, 0.999])
}

/// A point's coordinate in the cases.
fn coord(g: &mut Gen) -> f64 {
    if g.chance(30) {
        return g.pick(&[f64::NAN, f64::INFINITY, -f64::INFINITY, 1e10, -0.0]);
    }
    g.pick(&[0.0, 1.0, -1.0, 2.5, 128.0, -30.25, 0.5, 3.0, 17.75])
}

/// `cmph`'s header fields beyond the volume's: field of view, location, `ptype`.
fn hdr_extra(g: &mut Gen) -> Vec<String> {
    vec![
        fx(g.pick(&[0.0, 256.0, 240.0, 1e-9])),
        fx(g.pick(&[0.0, 1.5, -2.0])),
        g.pick(&[0, 0, 1, 2]).to_string(),
    ]
}

/// One small change to a copy of a volume's tokens or its header fields (a float nudged, an
/// integer changed).
fn perturb(g: &mut Gen, v: &mut [String], e: &mut [String]) {
    if g.chance(4) {
        let k = g.below(3) as usize;
        e[k] = if k == 2 {
            g.int(0, 3).to_string()
        } else {
            let x = f32::from_bits(u32::from_str_radix(&e[k], 16).unwrap());
            fx(x + g.pick(&[1e-8f32, 1e-6, 1e-3, 0.5]))
        };
        return;
    }
    // the floats of the geometry (tokens 11 to 13 sizes, 15 to 29 cosines and centre) and the
    // header (33 to 43), the flip angle (44, a double), ras_good_flag (14)
    let k = g.pick(&[11, 12, 13, 14, 15, 18, 24, 26, 33, 36, 38, 41, 42, 43, 44]);
    v[k] = match k {
        14 => g.int(0, 1).to_string(),
        44 => {
            let x = f64::from_bits(u64::from_str_radix(&v[k], 16).unwrap());
            dx(x + g.pick(&[1e-9, 1e-7, 1e-3]))
        }
        _ => {
            let x = f32::from_bits(u32::from_str_radix(&v[k], 16).unwrap());
            fx(x + g.pick(&[1e-8f32, 1e-6, 1e-3, 0.5]))
        }
    };
}

/// The probe's commands, in the order the generator cycles through them.
pub const COMMANDS: [&str; 240] = [
    "vgwrite",
    "vgread",
    "vgread",
    "vgprint0",
    "getvg",
    "copyvg",
    "vgi2r",
    "vgr2i",
    "tkrv2r",
    "vgeq0",
    "clone",
    "clonesp",
    "clear",
    "match",
    "cpframe",
    "cpframes",
    "linscale",
    "smul",
    "smulf",
    "thresh",
    "uthresh",
    "invc",
    "binz",
    "abs",
    "pos",
    "neg",
    "mul2",
    "sub2",
    "add2",
    "div2",
    "sam",
    "vrange",
    "vrangef",
    "vrangereg",
    "lrange",
    "extract",
    "cras",
    "avg",
    "dsign",
    "sras",
    "spt",
    "ostr",
    "vtal",
    "com",
    "pca",
    "cmph",
    "dump",
    "stri",
    "styp",
    "sfrm",
    "sfty",
    "scub",
    "scubf",
    "ssinc",
    "ssincf",
    "coef",
    "hsinc",
    "trans",
    "rot",
    "rotx",
    "roty",
    "rotz",
    "skull",
    "tftype",
    "ltaread",
    "cmalab",
    "cmavol",
    "lrswap",
    "fixrib",
    "lrseg",
    "ohe",
    "bvc",
    "bvr",
    "setres",
    "vfill",
    "fillfg",
    "explane",
    "fillplane",
    "ups",
    "dns2",
    "dnsn",
    "mft",
    "lim",
    "chtype",
    "splat",
    "sseq",
    "sxyz",
    "tkern",
    "deriv",
    "grad",
    "nbrs",
    "lxf",
    "rsmat",
    "rsfill",
    "repval",
    "stats",
    "type2s",
    "cropbb",
    "crass",
    "chkostr",
    "o2dc",
    "s44",
    "vx2rx",
    "v2v",
    "setv2r",
    "toimgv",
    "toimg",
    "img2mri",
    "smpar",
    "erasebp",
    "nbhd",
    "nearnz",
    "lmeans",
    "mkpos",
    "mlbv",
    "lnbhd",
    "linbhd",
    "lbbox",
    "fillbox",
    "mskz",
    "clipwm",
    "pvlab",
    "randn",
    "const",
    "nseq",
    "nfr",
    "sort",
    "revsl",
    "cdtype",
    "nonmax",
    "xrpad",
    "ordidx",
    "vperm",
    "rms",
    "vfrac",
    "cdt",
    "diffv",
    "tiv",
    "fpow",
    "mbin",
    "reshape",
    "mmax",
    "mconst",
    "fbin",
    "vote",
    "msq",
    "crop",
    "revs",
    "cutend",
    "segids",
    "hcos",
    "vv",
    "vvsm",
    "vvk",
    "vvd",
    "maskup",
    "cov",
    "mpca",
    "pcast",
    "fdr",
    "rgbb",
    "vxn",
    "talcras",
    "talmat",
    "talpt",
    "talvol",
    "dilate",
    "dilu",
    "msegs",
    "sstat",
    "hseg",
    "conform",
    "union",
    "isect",
    "mand",
    "mxor",
    "compl",
    "edges",
    "erodenn",
    "erode",
    "erode2d",
    "erodeseg",
    "dilseg",
    "dtl",
    "dtli",
    "dlu",
    "dlab",
    "openn",
    "mopen",
    "mclose",
    "dil6",
    "rlist",
    "rvonly",
    "mdiff",
    "mmask",
    "tmask",
    "mnot",
    "slv",
    "b2c",
    "nmask",
    "von",
    "vinl",
    "linv",
    "cent",
    "pthr",
    "cnbhd",
    "grow",
    "onfg",
    "offbg",
    "cplab",
    "cplv",
    "hreg",
    "hvox",
    "hthr",
    "hist",
    "geq",
    "heq",
    "hlr",
    "hlab",
    "hsegm",
    "ltafill",
    "ltainv",
    "ltachg",
    "ltacat",
    "ltared",
    "ltasrc",
    "ltaprt",
    "a2v",
    "a2vmu",
    "spmw",
    "icode",
    "vvtk",
    "sbound",
    "idxno",
    "ssrob",
    "sfavg",
    "zcat",
    "aedit",
    "shdim",
    "sborder",
];

/// One buffer both streams write into, in order (FreeSurfer's standard output and error go to
/// one file in `fsvolume`).
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
    fn hex(&mut self) -> Vec<u8> {
        let s = self.next();
        if s == "-" {
            return Vec::new();
        }
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
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
        self.0 += &hex(b);
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
    fn geom(&mut self, v: &VolGeom) {
        self.geom_v(v, true);
    }
    /// `with_valid`: an MRI's `valid` is never set by its constructors (memory contents).
    fn geom_v(&mut self, v: &VolGeom, with_valid: bool) {
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
        self.hex(v.fname.c_bytes());
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

fn geom_of(t: &mut Toks) -> VolGeom {
    let mut v = VolGeom::default();
    set_geom(t, &mut v);
    v
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
    // the geometry's fields but its extent (the volume's own)
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
    Ok(m)
}

impl Out {
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
        self.vol_with(m, false);
    }
    /// The volume; with `hash`, its voxels' FNV-1a (64 bits) and byte count instead of their hex
    /// (`fsvolume.cpp`'s `put_vol(m, true)`: a conformed volume's 256^3 voxels).
    fn vol_with(&mut self, m: Option<&Mri>, hash: bool) {
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
        self.geom_v(&m.vg, false);
        self.am(m.vg.i_to_r.as_ref());
        self.m(m.vg.r_to_i.as_ref());
        match &m.chunk {
            Some(c) if hash => {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for &x in c {
                    h ^= u64::from(x);
                    h = h.wrapping_mul(0x0100_0000_01b3);
                }
                self.0 += &format!("fnv {h:016x} {} ", c.len());
            }
            Some(c) => self.hex(c),
            None => self.0 += "nochunk ",
        }
    }
    /// `r != NULL && r == dst`, then the volume returned.
    fn ret(&mut self, r: &Ret<Mri>, given: Option<&Mri>) {
        self.i(i64::from(matches!(r, Ret::Given)));
        match r {
            Ret::New(m) => self.vol(Some(m)),
            Ret::Given => self.vol(given),
            Ret::Null => self.vol(None),
        }
    }
}

/// An image of the case (`fsvolume.cpp`'s `get_img`).
fn get_img(cx: &mut Ctx, t: &mut Toks) -> Result<im::Image, Fatal> {
    let (rows, cols, format, nf) = (t.i(), t.i(), t.i(), t.i());
    let seed = t.i() as u32 as u64;
    let mut i = im::image_alloc(&mut cx.g, &mut cx.b, rows, cols, format, nf, 0)?;
    if let Some(b) = i.image.as_mut() {
        if format == im::PFFLOAT {
            for k in 0..b.len() / 4 {
                let v = FTAB[((k as u64 * 7 + seed) % FTAB.len() as u64) as usize];
                b[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
            }
        } else {
            for (k, x) in b.iter_mut().enumerate() {
                let k = k as u64;
                *x = ((k * 37 + seed * 101 + (k >> 3) * 13) & 0xff) as u8;
            }
        }
    }
    i.xsize = t.f();
    i.ysize = t.f();
    Ok(i)
}

impl Out {
    fn img(&mut self, i: Option<&im::Image>) {
        let Some(i) = i else {
            self.0 += "NULL ";
            return;
        };
        for v in [
            i.rows,
            i.cols,
            i.orows,
            i.ocols,
            i.num_frame,
            i.pixel_format,
            i.sizepix,
        ] {
            self.i(i64::from(v));
        }
        self.i(i.sizeimage as i64);
        self.f(i.xsize);
        self.f(i.ysize);
        match &i.image {
            Some(b) => {
                let n = (i.sizeimage as usize).saturating_mul(i.num_frame.max(0) as usize);
                self.hex(&b[..n.min(b.len())]);
            }
            None => self.0 += "nopix ",
        }
    }
}

/// The destination of `f(src, dst, ...)`: 0 `NULL`, 1 a volume of the case, 2 the source.
/// A volume of the case with its painted voxels (`fsvolume.cpp`'s `paint`).
fn painted(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let mut m = get_vol(cx, t)?;
    let n = t.i();
    for _ in 0..n {
        let (x, y, z, v) = (t.i(), t.i(), t.i(), t.f());
        larmorx_freesurfer_mri::voxel::mri_set_vox_val(
            &mut cx.g, &mut cx.b, &mut m, x, y, z, 0, v, 0,
        )?;
    }
    Ok(m)
}

/// `fsvolume.cpp`'s `fill_paint`: frames filled (`k`, then each frame's value), then voxels
/// painted (`n`, then `x y z frame value`), through `MRIsetVoxVal`.
fn fill_painted(cx: &mut Ctx, t: &mut Toks, m: &mut Mri) -> Result<(), Fatal> {
    use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
    let k = t.i();
    for f in 0..k {
        let v = t.f();
        for x in 0..m.vg.width {
            for y in 0..m.vg.height {
                for z in 0..m.vg.depth {
                    mri_set_vox_val(&mut cx.g, &mut cx.b, m, x, y, z, f, v, 0)?;
                }
            }
        }
    }
    let n = t.i();
    for _ in 0..n {
        let (x, y, z, f, v) = (t.i(), t.i(), t.i(), t.i(), t.f());
        mri_set_vox_val(&mut cx.g, &mut cx.b, m, x, y, z, f, v, 0)?;
    }
    Ok(())
}

/// A label list of the case: rows, cols, the numbers row by row (`fsvolume.cpp`'s `get_list`).
fn get_list(t: &mut Toks) -> Matrix {
    let (rows, cols) = (t.i(), t.i());
    let mut m = Matrix::real(rows, cols);
    for r in 1..=rows {
        for c in 1..=cols {
            let v = t.f();
            m.set_relt(r, c, v);
        }
    }
    m
}

/// A 4 x 4 matrix of the case or `None` (`fsvolume.cpp`'s `get_m44`).
fn get_m44(t: &mut Toks) -> Option<Matrix> {
    if t.i() == 0 {
        return None;
    }
    let mut m = Matrix::real(4, 4);
    for r in 1..=4 {
        for c in 1..=4 {
            let v = t.f();
            m.set_relt(r, c, v);
        }
    }
    Some(m)
}

impl Out {
    /// `fsvolume.cpp`'s `put_lta`.
    fn lta(&mut self, l: Option<&larmorx_freesurfer_mri::lta::Lta>) {
        let Some(l) = l else {
            self.0 += "NULL ";
            return;
        };
        self.i(i64::from(l.ltype));
        self.i(i64::from(l.xforms[0].ltype));
        self.m(l.xforms[0].m_l.as_ref());
        self.geom(&l.xforms[0].src);
        self.geom(&l.xforms[0].dst);
    }
}

/// `cma.h`'s `IS_*` tests of a label as 0/1 characters, in its order (`fsvolume.cpp`'s `cma_bits`).
fn cma_bits<T: cm::Label>(v: T) -> String {
    let mut b: String = [
        cm::is_future_wmsa(v),
        cm::is_lh_layer(v),
        cm::is_cc(v),
        cm::is_rh_layer(v),
        cm::is_layer(v),
        cm::is_unknown(v),
        cm::is_brain(v),
        cm::is_wm(v),
        cm::is_cerebral_wm(v),
        cm::is_hypo(v),
        cm::is_wmsa(v),
        cm::is_wmh(v),
        cm::is_thalamus(v),
        cm::is_gm(v),
        cm::is_ventral_dc(v),
        cm::is_cerebellar_wm(v),
        cm::is_cerebellar_gm(v),
        cm::is_cerebellum(v),
        cm::is_choroid(v),
        cm::is_hippo(v),
        cm::is_amygdala(v),
        cm::is_mtl(v),
        cm::is_cortex(v),
        cm::is_lat_vent(v),
        cm::is_csf(v),
        cm::is_inf_lat_vent(v),
        cm::is_ventricle(v),
        cm::is_caudate(v),
        cm::is_putamen(v),
        cm::is_claustrum(v),
        cm::is_accumbens(v),
        cm::is_pallidum(v),
        cm::label_with_no_topology_constraint(v),
        cm::is_fimbria(v),
        cm::is_gray_matter(v),
        cm::is_white_matter(v),
        cm::is_fluid(v),
        cm::is_lh_class(v),
        cm::is_rh_class(v),
        cm::is_gray_class(v),
        cm::is_white_class(v),
        cm::is_csf_class(v),
    ]
    .iter()
    .map(|&x| if x { '1' } else { '0' })
    .collect();
    for c in [cm::CSF_CLASS, cm::GM_CLASS, cm::WM_CLASS, cm::OTHER_CLASS] {
        b.push(if cm::is_class(v, c) { '1' } else { '0' });
    }
    b
}

fn dst_mode(cx: &mut Ctx, t: &mut Toks) -> Result<(i32, Option<Mri>), Fatal> {
    let mode = t.i();
    let d = if mode == 1 {
        Some(get_vol(cx, t)?)
    } else {
        None
    };
    Ok((mode, d))
}

fn pair<'a>(mode: i32, src: &'a mut Mri, dst: &'a mut Option<Mri>) -> Pair<'a> {
    match mode {
        1 => Pair::Given(src, dst.as_mut().expect("a destination")),
        2 => Pair::Same(src),
        _ => Pair::New(src),
    }
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let cmd = t.next();
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    match cmd {
        "vgwrite" => {
            let vg = geom_of(t);
            let mut fp = CFile::memory_writer();
            vgeom::write_vol_geom(&mut fp, &vg);
            o.hex(&fp.into_bytes());
        }
        "vgread" => {
            let mut vg = geom_of(t);
            let bytes = t.hex();
            let mut fp = CFile::from_bytes(bytes);
            g.errno = 0;
            vgeom::read_vol_geom(g, b, &mut fp, &mut vg);
            let e = g.errno;
            o.geom(&vg);
            o.i(fp.tell());
            o.i(i64::from(fp.feof()));
            o.i(i64::from(fp.ferror()));
            o.i(i64::from(e));
            let mut rest = Vec::new();
            while rest.len() < 64 {
                let c = fp.getc();
                if c < 0 {
                    break;
                }
                rest.push(c as u8);
            }
            o.hex(&rest);
        }
        "vgprint0" => {
            let vg = geom_of(t);
            vgeom::vg_print(b, &vg);
            o.i(0);
        }
        "getvg" | "copyvg" => {
            let src = geom_of(t);
            let mut dst = geom_of(t);
            if cmd == "getvg" {
                vgeom::get_vol_geom(&src, &mut dst);
            } else {
                vgeom::copy_vol_geom(&src, &mut dst);
            }
            o.geom(&dst);
        }
        "vgi2r" => {
            let vg = geom_of(t);
            let m = vgeom::vg_i_to_r(g, a, b, &vg)?;
            o.m(Some(&m));
        }
        "vgr2i" => {
            let vg = geom_of(t);
            let m = vgeom::vg_r_to_i(g, a, b, &vg)?;
            o.m(m.as_ref());
        }
        "tkrv2r" => {
            let vg = geom_of(t);
            let m = vgeom::tkr_vox2ras_from_vol_geom(g, a, b, &vg)?;
            o.m(Some(&m));
        }
        "vgeq0" => {
            let va = geom_of(t);
            let vb = geom_of(t);
            let thresh = t.d();
            g.vg_is_equal_threshold = t.d();
            o.i(i64::from(vgeom::vg_is_not_equal_thresh(&va, &vb, thresh)));
            o.i(i64::from(vgeom::vg_is_equal(g, &va, &vb)));
        }
        "clone" => {
            let src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = ar::mri_clone(g, a, b, &src, dst.as_mut())?;
            o.ret(&r, dst.as_ref());
        }
        "clonesp" => {
            let src = get_vol(cx, t)?;
            let (ty, nf) = (t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = ar::mri_clone_by_space(g, a, b, &src, ty, nf)?;
            o.vol(Some(&r));
        }
        "clear" => {
            let mut m = get_vol(cx, t)?;
            let r = ar::mri_clear(&mut cx.g, &mut cx.b, &mut m)?;
            o.i(i64::from(r));
            o.vol(Some(&m));
        }
        "match" => {
            let va = get_vol(cx, t)?;
            let vb = get_vol(cx, t)?;
            let (w, h, d, nocheck) = (t.i(), t.i(), t.i(), t.i());
            o.i(i64::from(ar::mri_match(&va, &vb)));
            o.i(i64::from(ar::mri_match_dimensions(&va, &vb)));
            let chk = if nocheck != 0 { None } else { Some(&vb) };
            o.i(i64::from(ar::mri_check_size(&va, chk, w, h, d)));
        }
        "cpframe" | "cpframes" | "linscale" | "smul" | "smulf" | "thresh" | "uthresh" | "invc"
        | "binz" | "abs" | "pos" | "neg" => {
            let mut src = get_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = match cmd {
                "cpframe" => {
                    let (sf, df) = (t.i(), t.i());
                    ar::mri_copy_frame(g, a, b, p, sf, df)?
                }
                "cpframes" => {
                    let (s0, s1, d0) = (t.i(), t.i(), t.i());
                    ar::mri_copy_frames(g, a, b, p, s0, s1, d0)?
                }
                "linscale" => {
                    let (sc, off, nz) = (t.f(), t.f(), t.i());
                    ar::mri_linear_scale(g, a, b, p, sc, off, nz != 0)?
                }
                "smul" => ar::mri_scalar_mul(g, a, b, p, t.f())?,
                "smulf" => {
                    let sc = t.f();
                    ar::mri_scalar_mul_frame(g, a, b, p, sc, t.i())?
                }
                "thresh" => ar::mri_threshold(g, a, b, p, t.f())?,
                "uthresh" => ar::mri_upper_threshold_all_frames(g, a, b, p, t.f())?,
                "invc" => ar::mri_invert_contrast(g, a, b, p, t.f())?,
                "binz" => {
                    let (th, lo, hi) = (t.f(), t.f(), t.f());
                    ar::mri_binarize(g, a, b, p, th, lo, hi)?
                }
                "abs" => ar::mri_abs(g, a, b, p)?,
                "pos" => ar::mri_pos(g, a, b, p)?,
                _ => ar::mri_neg(g, a, b, p)?,
            };
            let given = if mode == 2 { Some(&src) } else { dst.as_ref() };
            o.ret(&r, given);
        }
        "mul2" | "sub2" | "add2" | "div2" | "sam" => {
            let mut v1 = get_vol(cx, t)?;
            let mut v2 = get_vol(cx, t)?;
            let mode = t.i();
            let mut d3 = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let scale = if cmd == "sam" { t.f() } else { 0.0 };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let tri = match mode {
                1 => Tri::Given(&v1, &v2, d3.as_mut().expect("a destination")),
                2 => Tri::First(&mut v1, &v2),
                3 => Tri::Second(&v1, &mut v2),
                _ => Tri::New(&v1, &v2),
            };
            let r = match cmd {
                "mul2" => ar::mri_multiply(g, a, b, tri)?,
                "sub2" => ar::mri_subtract(g, a, b, tri)?,
                "add2" => ar::mri_add(g, a, b, tri)?,
                "div2" => ar::mri_divide(g, a, b, tri)?,
                _ => ar::mri_scale_and_multiply(g, a, b, tri, scale)?,
            };
            let given = match mode {
                1 => d3.as_ref(),
                2 => Some(&v1),
                3 => Some(&v2),
                _ => None,
            };
            o.ret(&r, given);
        }
        "vrange" => {
            let m = get_vol(cx, t)?;
            let (r, lo, hi) = ar::mri_val_range(&mut cx.g, &mut cx.b, &m)?;
            o.i(i64::from(r));
            o.f(lo);
            o.f(hi);
        }
        "vrangef" => {
            let m = get_vol(cx, t)?;
            let f = t.i();
            let (r, lh) = ar::mri_val_range_frame(&mut cx.g, &mut cx.b, &m, f)?;
            let (lo, hi) = lh.unwrap_or((-7.0, -7.0));
            o.i(i64::from(r));
            o.f(lo);
            o.f(hi);
        }
        "vrangereg" => {
            let m = get_vol(cx, t)?;
            let reg = MriRegion {
                x: t.i(),
                y: t.i(),
                z: t.i(),
                dx: t.i(),
                dy: t.i(),
                dz: t.i(),
            };
            let (r, lo, hi) = ar::mri_val_range_region(&mut cx.g, &mut cx.b, &m, &reg)?;
            o.i(i64::from(r));
            o.f(lo);
            o.f(hi);
        }
        "lrange" => {
            let m = get_vol(cx, t)?;
            let l = get_vol(cx, t)?;
            let label = t.i();
            let (r, lo, hi) = ar::mri_label_val_range(&mut cx.g, &mut cx.b, &m, &l, label)?;
            o.i(i64::from(r));
            o.f(lo);
            o.f(hi);
        }
        "extract" => {
            let mut src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let p0 = [t.i(), t.i(), t.i()];
            let sz = [t.i(), t.i(), t.i()];
            let p1 = [t.i(), t.i(), t.i()];
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = ar::mri_extract_into(g, a, b, &mut src, dst.as_mut(), p0, sz, p1)?;
            o.ret(&r, dst.as_ref());
            o.am(src.vg.i_to_r.as_ref());
        }
        "cras" => {
            let mut src = get_vol(cx, t)?;
            let dst = get_vol(cx, t)?;
            let v: Vec<i32> = (0..6).map(|_| t.i()).collect();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let (r, aa, ss) = ar::mri_calc_cras_for_extracted_volume(
                g, a, b, &mut src, &dst, v[0], v[1], v[2], v[3], v[4], v[5],
            )?;
            o.d(r);
            o.d(aa);
            o.d(ss);
            o.am(src.vg.i_to_r.as_ref());
            o.m(src.vg.r_to_i.as_ref());
        }
        "avg" => {
            let mut src = get_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let dof = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = ar::mri_average(g, a, b, p, dof)?;
            let given = if mode == 2 { Some(&src) } else { dst.as_ref() };
            o.ret(&r, given);
        }
        "stri" | "styp" | "sfrm" | "sfty" | "scub" | "scubf" | "ssinc" | "ssincf" => {
            samplers(cx, t, o, cmd)?;
        }
        "trans" | "rot" => {
            let mut src = get_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "trans" {
                let (dx, dy, dz) = (t.d(), t.d(), t.d());
                let p = pair(mode, &mut src, &mut dst);
                rt::mri_translate(g, a, b, p, dx, dy, dz)?
            } else {
                let mut mr = mx::matrix_alloc(g, a, b, 3, 3, mx::MATRIX_REAL)?.expect("3 x 3");
                for i in 1..=3 {
                    for j in 1..=3 {
                        mr.set_relt(i, j, t.f());
                    }
                }
                let mo = if t.i() != 0 {
                    let mut m = mx::matrix_alloc(g, a, b, 3, 1, mx::MATRIX_REAL)?.expect("3 x 1");
                    for i in 1..=3 {
                        m.set_relt(i, 1, t.f());
                    }
                    Some(m)
                } else {
                    None
                };
                let p = pair(mode, &mut src, &mut dst);
                rt::mri_rotate_i(g, a, b, p, &mr, mo.as_ref())?
            };
            let given = if mode == 2 { Some(&src) } else { dst.as_ref() };
            o.ret(&r, given);
        }
        "rotx" | "roty" | "rotz" => {
            let src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let angle = t.f();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match cmd {
                "rotx" => rt::mri_rotate_x_i(g, a, b, &src, dst.as_mut(), angle)?,
                "roty" => rt::mri_rotate_y_i(g, a, b, &src, dst.as_mut(), angle)?,
                _ => rt::mri_rotate_z_i(g, a, b, &src, dst.as_mut(), angle)?,
            };
            o.i(0);
            o.vol(r.as_ref());
            o.vol(dst.as_ref());
        }
        "skull" => {
            let mut m = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let nb = t.i();
            for _ in 0..nb {
                let (x0, y0, z0, x1, y1, z1) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
                let v = t.f();
                for z in z0..=z1 {
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            larmorx_freesurfer_mri::voxel::mri_set_vox_val(
                                g, b, &mut m, x, y, z, 0, v, 0,
                            )?;
                        }
                    }
                }
            }
            let th = t.i();
            let mut bx = MriRegion {
                x: -7,
                y: -7,
                z: -7,
                dx: -7,
                dy: -7,
                dz: -7,
            };
            let r = sk::find_approximate_skull_bounding_box(g, a, b, &m, th, &mut bx)?;
            o.i(i64::from(r));
            for v in [bx.x, bx.y, bx.z, bx.dx, bx.dy, bx.dz] {
                o.i(i64::from(v));
            }
        }
        "tftype" => {
            let n = t.i();
            for _ in 0..n {
                let name = t.hex();
                o.i(i64::from(lr::transform_file_name_type(g, &name)));
            }
        }
        "ltaread" => {
            let ext = String::from_utf8(t.hex()).expect("an ASCII name");
            let bytes = t.hex();
            let have = t.i() != 0;
            let env = match t.i() {
                0 => Environment::default(),
                1 => Environment::from_pairs([("USE_AVERAGE305", "true")]),
                _ => Environment::from_pairs([("USE_AVERAGE305", "")]),
            };
            // a folder of its own per call: the tests run cases in several threads at once
            static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("larmorx-fsvol-lta-{}-{call}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch folder");
            let name = format!("fsvol-lta{ext}");
            let path = dir.join(&name);
            let _ = std::fs::remove_file(&path);
            if have {
                std::fs::write(&path, &bytes).expect("the case's file");
            }
            g.errno = 0;
            let r = lr::lta_read_ex(g, a, b, Some(&dir), &name, &mut NoIo(env));
            let _ = std::fs::remove_dir_all(&dir);
            let r = r?;
            o.i(i64::from(g.errno));
            match r {
                None => o.0 += "NULL ",
                Some(lta) => {
                    o.i(i64::from(lta.ltype));
                    o.i(i64::from(lta.num_xforms));
                    for lt in &lta.xforms {
                        for v in [lt.x0, lt.y0, lt.z0, lt.sigma] {
                            o.f(v);
                        }
                        o.m(lt.m_l.as_ref());
                        o.i(i64::from(lt.ltype));
                        o.geom(&lt.src);
                        o.geom(&lt.dst);
                    }
                    o.hex(lta.subject.c_bytes());
                    o.f(lta.fscale);
                }
            }
        }
        "cmalab" => {
            let n = t.i();
            for _ in 0..n {
                let v = t.i();
                let f = t.f();
                o.0 += &format!(
                    "{} {} {} ",
                    cma_bits(v),
                    cma_bits(f),
                    cma_bits(f64::from(f))
                );
                o.i(i64::from(cm::is_sub_cortical_gray(v)));
                o.i(i64::from(cm::mri_aseg_contra_lat_label(&mut cx.a, v)));
            }
        }
        "cmavol" => {
            let aseg = painted(cx, t)?;
            let ribbon = painted(cx, t)?;
            let hemi = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.d(cm::supra_tentorial_vol_correction(g, b, &aseg, &ribbon)?);
            o.d(cm::cortical_gm_vol_correction(
                g, a, b, &aseg, &ribbon, hemi,
            )?);
        }
        "lrswap" => {
            let aseg = painted(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = cm::mri_lrswap_aseg(g, a, b, &aseg)?;
            o.vol(r.as_ref());
        }
        "fixrib" => {
            let aseg = painted(cx, t)?;
            let ribbon = painted(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = cm::mri_fix_aseg_with_ribbon(g, a, b, &aseg, &ribbon, dst.as_mut())?;
            let _ = mode;
            o.ret(&r, dst.as_ref());
        }
        "lrseg" => {
            let seg = painted(cx, t)?;
            let lh = get_list(t);
            let rh = get_list(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = cm::mri_lrswap_seg(g, a, b, &seg, &lh, &rh)?;
            o.vol(r.as_ref());
        }
        "ohe" => {
            let seg = painted(cx, t)?;
            let n = t.i();
            let list: Vec<i32> = (0..n).map(|_| t.i()).collect();
            let force1 = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = cm::mri_one_hot_encode(g, a, b, &seg, &list, force1)?;
            o.vol(r.as_ref());
        }
        "bvc" | "bvr" => {
            // a folder of its own per call: the tests run cases in several threads at once
            static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("larmorx-fsvol-bv-{}-{call}", std::process::id()));
            let subj = "fsvol-subj";
            let stats_dir = dir.join(subj).join("stats");
            let _ = std::fs::remove_dir_all(dir.join(subj));
            if t.i() != 0 {
                std::fs::create_dir_all(&stats_dir).expect("the subject's stats folder");
            }
            std::fs::create_dir_all(&dir).expect("a scratch folder");
            let file = stats_dir.join("brainvol.stats");
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "bvc" {
                let mut stats = [0.0f64; 16];
                for v in &mut stats {
                    *v = t.d();
                }
                cm::cache_brain_volume_stats(g, b, Some(&dir), &stats, subj, ".").map(|()| {
                    o.hex(&std::fs::read(&file).unwrap_or_default());
                })
            } else {
                std::fs::write(&file, t.hex()).expect("the case's file");
                let mut compute = |_: &mut Globals, _: &mut dyn Write, _: &mut dyn Write| {
                    panic!("ComputeBrainVolumeStats is not reached by the cases")
                };
                cm::read_cached_brain_volume_stats(g, a, b, Some(&dir), subj, ".", &mut compute)
                    .map(|stats| {
                        for v in stats {
                            o.d(v);
                        }
                    })
            };
            let _ = std::fs::remove_dir_all(&dir);
            r?;
        }
        "setres" => {
            let mut m = get_vol(cx, t)?;
            let (x, y, z) = (t.f(), t.f(), t.f());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(rs::mri_set_resolution(g, a, b, &mut m, x, y, z)?));
            o.vol(Some(&m));
        }
        "vfill" => {
            let mut m = get_vol(cx, t)?;
            let v = t.f();
            o.i(i64::from(rs::mri_value_fill(
                &mut cx.g, &mut cx.b, &mut m, v,
            )?));
            o.vol(Some(&m));
        }
        "fillfg" => {
            let mut src = get_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (sx, sy, sz, th, fv) = (t.i(), t.i(), t.i(), t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let (r, npix) = rs::mri_fill_fg(g, a, b, p, sx, sy, sz, th, fv)?;
            o.i(i64::from(npix));
            let given = match mode {
                1 => dst.as_ref(),
                2 => Some(&src),
                _ => None,
            };
            o.ret(&r, given);
        }
        "explane" => {
            let src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (or, at) = (t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = rs::mri_extract_plane(g, a, b, &src, dst.as_mut(), or, at)?;
            o.ret(&r, dst.as_ref());
        }
        "fillplane" => {
            let mask = get_vol(cx, t)?;
            let mut dst = get_vol(cx, t)?;
            let (or, at, fv) = (t.i(), t.i(), t.i());
            rs::mri_fill_plane(&mut cx.g, &mut cx.b, &mask, &mut dst, or, at, fv)?;
            o.i(1);
            o.vol(Some(&dst));
        }
        "ups" => {
            let mut src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (k, n) = (t.i(), t.i());
            let env = Environment::default();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match k {
                0 => rs::mri_upsample_n(g, a, b, &src, dst.as_mut(), n)?,
                1 => rs::mri_upsample2(g, a, b, &src, dst.as_mut())?,
                _ => rs::mri_upsample_n_conserve(g, a, b, &env, &mut src, dst.as_mut(), n)?,
            };
            o.ret(&r, dst.as_ref());
        }
        "dns2" | "dnsn" => {
            let src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "dns2" {
                rs::mri_downsample2(g, a, b, &src, dst.as_mut())?
            } else {
                let (fc, fr, fs, keep) = (t.f(), t.f(), t.f(), t.i());
                rs::mri_downsample_n(g, a, b, &src, dst.as_mut(), fc, fr, fs, keep)?
            };
            o.ret(&r, dst.as_ref());
        }
        "mft" => {
            let m = get_vol(cx, t)?;
            let (f, th) = (t.i(), t.f());
            o.d(rs::mri_mean_frame_thresh(&mut cx.g, &mut cx.b, &m, f, th));
        }
        "lim" => {
            let m = get_vol(cx, t)?;
            let (mut lo, mut hi) = (t.f(), t.f());
            o.i(i64::from(rs::mri_limits(
                &mut cx.g, &mut cx.b, &m, &mut lo, &mut hi,
            )?));
            o.f(lo);
            o.f(hi);
        }
        "chtype" => {
            let mut m = get_vol(cx, t)?;
            let (seq, dt, lo, hi, ns) = (t.i(), t.i(), t.f(), t.f(), t.i());
            let env = if t.i() != 0 {
                Environment::from_pairs([("FS_FORCE_BIN_CHECK", "1")])
            } else {
                Environment::default()
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if seq != 0 {
                rs::mri_seq_change_type(g, a, b, &env, &mut m, dt, lo, hi, ns)?
            } else {
                rs::mri_change_type(g, a, b, &env, &m, dt, lo, hi, ns)?
            };
            o.vol(r.as_ref());
            o.vol(Some(&m));
        }
        "coef" | "hsinc" => {
            let n = t.i();
            let libm = larmorx_core::math::Libm::quiet(g.mode());
            for _ in 0..n {
                let x = t.d();
                if cmd == "coef" {
                    let it = t.i();
                    o.d(sm::cubic_coeff(g, b, x, it));
                } else {
                    let fw = t.d();
                    o.d(sm::ham_sinc(&libm, x, fw));
                }
            }
        }
        "dsign" => {
            let v = t.d();
            o.d(co::set_sample_volume_derivative_sign(g, a, v));
        }
        "sras" => {
            let mut m = get_vol(cx, t)?;
            let k = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match k {
                0 => Some(co::surface_ras_from_voxel(g, a, b, &mut m.vg)?),
                1 => co::voxel_from_surface_ras(g, a, b, &mut m.vg)?,
                2 => co::surface_ras_from_ras(g, a, b, &m.vg)?,
                _ => co::get_surface_ras_to_voxel_matrix(g, a, b, &mut m.vg)?,
            };
            o.m(r.as_ref());
            o.am(m.vg.i_to_r.as_ref());
            o.m(m.vg.r_to_i.as_ref());
        }
        "spt" => {
            let mut m = get_vol(cx, t)?;
            let k = t.i();
            let (x, y, z) = (t.d(), t.d(), t.d());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let (r, p) = match k {
                0 => co::ras_to_surface_ras(g, a, b, &m.vg, x, y, z)?,
                1 => co::voxel_to_surface_ras(g, a, b, &mut m.vg, x, y, z)?,
                _ => co::surface_ras_to_voxel(g, a, b, &mut m.vg, x, y, z)?,
            };
            o.i(i64::from(r));
            for v in p {
                o.d(v);
            }
            o.am(m.vg.i_to_r.as_ref());
            o.m(m.vg.r_to_i.as_ref());
        }
        "ostr" => {
            let m = get_vol(cx, t)?;
            let (r, s3) = co::dircos_to_orientation_string(&m.vg);
            o.i(i64::from(r));
            o.hex(&[s3[0], s3[1], s3[2], 0]);
            o.0 += co::slice_direction_name(&m.vg);
            o.0 += " ";
            o.i(i64::from(co::get_slice_direction(&m.vg)));
        }
        "vtal" => {
            let mut m = get_vol(cx, t)?;
            if t.i() != 0 {
                let mut tr = [[0.0f64; 4]; 4];
                for row in tr.iter_mut() {
                    for v in row.iter_mut() {
                        *v = t.d();
                    }
                }
                m.linear_transform = Some(tr);
            }
            let (x, y, z) = (t.d(), t.d(), t.d());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let (r, p) = co::voxel_to_talairach(g, a, b, &mut m, x, y, z)?;
            o.i(i64::from(r));
            for v in p.unwrap_or([-7.0; 3]) {
                o.d(v);
            }
            o.am(m.vg.i_to_r.as_ref());
            o.m(m.vg.r_to_i.as_ref());
        }
        "com" | "pca" => {
            let m = get_vol(cx, t)?;
            let th = t.i() as u8;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if cmd == "com" {
                let (r, means) = co::center_of_mass(g, b, &m, th)?;
                o.i(i64::from(r));
                for v in means {
                    o.d(v);
                }
            } else {
                let mut ev = mx::matrix_alloc(g, a, b, 3, 3, mx::MATRIX_REAL)?.expect("3 x 3");
                let mut evalues = [-7.0f32; 3];
                let (r, means, _) =
                    co::principle_components(g, a, b, &m, Some(&mut ev), &mut evalues, th)?;
                o.i(i64::from(r));
                for v in means {
                    o.d(v);
                }
                for v in evalues {
                    o.f(v);
                }
                o.m(Some(&ev));
            }
        }
        "cmph" => {
            let mut va = get_vol(cx, t)?;
            hdr_fields(t, &mut va);
            let mode = t.i();
            let vb = if mode == 1 {
                let mut v = get_vol(cx, t)?;
                hdr_fields(t, &mut v);
                Some(v)
            } else {
                None
            };
            let b2 = match mode {
                1 => vb.as_ref(),
                2 => Some(&va),
                _ => None,
            };
            o.i(i64::from(co::compare_headers(Some(&va), b2)));
            o.i(i64::from(co::compare_headers(b2, Some(&va))));
        }
        "dump" => {
            let mut m = get_vol(cx, t)?;
            m.fov = t.f();
            m.brightness = t.i();
            m.subject_name = t.s();
            m.path_to_t1 = t.s();
            m.fname_format = t.s();
            if t.i() != 0 {
                let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                let mut r = mx::matrix_alloc(g, a, b, 4, 4, mx::MATRIX_REAL)?.expect("4 x 4");
                for i in 1..=4 {
                    for j in 1..=4 {
                        r.set_relt(i, j, t.f());
                    }
                }
                m.vg.register_mat = Some(r);
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let mut fp = a.clone();
            let r = co::mri_dump(g, a, b, &m, &mut fp)?;
            o.i(i64::from(r));
        }
        "splat" => {
            let mut m = get_vol(cx, t)?;
            let n = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            for i in 0..n {
                let (x, y, z, frame, val) = (t.d(), t.d(), t.d(), t.i(), t.d());
                let r = if frame == 0 && i % 2 == 1 {
                    sq::interpolate_into_volume(g, b, &mut m, x, y, z, val)
                } else {
                    sq::interpolate_into_volume_frame(g, b, &mut m, x, y, z, frame, val)
                };
                o.i(i64::from(r));
            }
            o.vol(Some(&m));
        }
        "sseq" | "sxyz" => {
            let mut m = get_vol(cx, t)?;
            m.outside_val = t.d();
            let n = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            for _ in 0..n {
                let (r, v) = if cmd == "sseq" {
                    let (x, y, z, first, last, ty) = (t.d(), t.d(), t.d(), t.i(), t.i(), t.i());
                    let mut v = vec![-7.0f32; (last + 1).max(1) as usize];
                    let r = if ty < 0 {
                        sq::sample_seq_volume(g, b, &m, x, y, z, &mut v, first, last)?
                    } else {
                        sq::sample_seq_volume_type(g, a, b, &m, x, y, z, &mut v, first, last, ty)?
                    };
                    (r, v)
                } else {
                    let (x, y, z, fb, fe, k) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
                    let mut v = vec![-7.0f32; (fe - fb).max(1) as usize];
                    let r = if k != 0 {
                        sq::sample_volume_frame_xyz_int_n_range_floats(
                            g, b, &m, x, y, z, fb, fe, &mut v,
                        )?
                    } else {
                        sq::sample_volume_frame_type_xyz_int_n_range_nearest_floats(
                            g, b, &m, x, y, z, fb, fe, &mut v,
                        )?
                    };
                    (r, v)
                };
                o.i(i64::from(r));
                for f in v {
                    o.f(f);
                }
            }
        }
        "tkern" => {
            let m = get_vol(cx, t)?;
            let n = t.i();
            for _ in 0..n {
                let (x, y, z) = (t.d(), t.d(), t.d());
                o.i(1);
                for w in sq::trilin_kernel(&m, x, y, z) {
                    o.d(w);
                }
            }
        }
        "deriv" | "grad" => {
            let m = get_vol(cx, t)?;
            cx.g.sample_volume_derivative_sign = t.d();
            let n = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            for _ in 0..n {
                let (x, y, z, kind) = (t.d(), t.d(), t.d(), t.i());
                if cmd == "deriv" {
                    let (ddx, ddy, ddz, sigma) = (t.d(), t.d(), t.d(), t.d());
                    let mut mag = -7.0;
                    let r = if kind != 0 {
                        dv::sample_volume_derivative_scale(
                            g, b, &m, x, y, z, ddx, ddy, ddz, &mut mag, sigma,
                        )?
                    } else {
                        dv::sample_volume_derivative(g, b, &m, x, y, z, ddx, ddy, ddz, &mut mag)
                    };
                    o.i(i64::from(r));
                    o.d(mag);
                } else {
                    let frame = t.i();
                    let (mut p, mut q, mut w) = (-7.0, -7.0, -7.0);
                    let r = if kind != 0 {
                        dv::sample_volume_gradient_frame(
                            g, b, &m, x, y, z, &mut p, &mut q, &mut w, frame,
                        )
                    } else {
                        dv::sample_volume_gradient(g, b, &m, x, y, z, &mut p, &mut q, &mut w)
                    };
                    o.i(i64::from(r));
                    o.d(p);
                    o.d(q);
                    o.d(w);
                }
            }
        }
        "nbrs" => {
            let m = get_vol(cx, t)?;
            let n = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            for _ in 0..n {
                let (kind, x, y, z, a1, b1) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
                let (lo, hi) = (t.f(), t.f());
                let r = match kind {
                    0 => dv::neighbors_on(g, b, &m, x, y, z, a1)?,
                    1 => dv::neighbors_on_3x3(g, b, &m, x, y, z, a1)?,
                    2 => dv::neighbors_in_window(g, b, &m, x, y, z, a1, b1)?,
                    3 => dv::neighbors(g, b, &m, x, y, z, a1)?,
                    4 => dv::neighbors_in_range(g, b, &m, x, y, z, a1, lo, hi)?,
                    5 => dv::neighbors_3x3(g, b, &m, x, y, z, a1)?,
                    6 => dv::neighbors_off(g, &m, x, y, z, a1),
                    _ => dv::neighbors_off_3x3(g, b, &m, x, y, z, a1)?,
                };
                o.i(i64::from(r));
            }
        }
        "lxf" => {
            let mut src = get_vol(cx, t)?;
            src.outside_val = t.d();
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (kind, interp) = (t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let ma = if t.i() != 0 {
                let mut m = mx::matrix_alloc(g, a, b, 4, 4, mx::MATRIX_REAL)?.expect("4 x 4");
                for r in 1..=4 {
                    for c in 1..=4 {
                        m.set_relt(r, c, t.f());
                    }
                }
                Some(m)
            } else {
                None
            };
            let (ma, d) = (ma.as_ref(), dst.as_mut());
            let r = match kind {
                0 => xf::mri_linear_transform_interp(g, a, b, &src, d, ma, interp)?,
                1 => xf::mri_linear_transform(g, a, b, &src, d, ma)?,
                2 => xf::mri_inverse_linear_transform(g, a, b, &src, d, ma)?,
                3 => xf::mri_apply_ras_linear_transform_interp(g, a, b, &src, d, ma, interp)?,
                4 => xf::mri_apply_ras_linear_transform(g, a, b, &src, d, ma)?,
                _ => xf::mri_apply_ras_inverse_linear_transform(g, a, b, &src, d, ma)?,
            };
            o.ret(&r, dst.as_ref());
        }
        "rsfill" => {
            let src = get_vol(cx, t)?;
            let tmpl = get_vol(cx, t)?;
            let (ty, fill, plain) = (t.i(), t.f(), t.i());
            if t.i() != 0 {
                cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_SHOW
                    | larmorx_freesurfer_core::diag::DIAG_VERBOSE;
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if plain != 0 {
                rsm::mri_resample(g, a, b, &src, &tmpl, ty)?
            } else {
                rsm::mri_resample_fill(g, a, b, &src, &tmpl, ty, fill)?
            };
            o.vol(r.as_ref());
        }
        "repval" => {
            let mut src = get_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let uchar = t.i() != 0;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = if uchar {
                let (x, y) = (t.i() as u8, t.i() as u8);
                ar::mri_replace_values_uchar(g, a, b, p, x, y)?
            } else {
                let (x, y) = (t.f(), t.f());
                ar::mri_replace_values(g, a, b, p, x, y)?
            };
            let given = match mode {
                1 => dst.as_ref(),
                2 => Some(&src),
                _ => None,
            };
            o.ret(&r, given);
        }
        "stats" => {
            let m = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if t.i() != 0 {
                let r = st::mri_print_stats(g, a, b, &m)?;
                o.i(i64::from(r));
            } else {
                let q = f32::from_bits(0x7fa5_a5a5);
                let mut s = st::Stats {
                    min: q,
                    max: q,
                    n_voxels: -7,
                    mean: q,
                    std: q,
                };
                o.i(i64::from(st::mri_stats(g, &m, &mut s)));
                o.i(i64::from(s.n_voxels));
                for v in [s.min, s.max, s.mean, s.std] {
                    o.f(v);
                }
            }
        }
        "type2s" => {
            o.0 += st::mri_type2str(t.i());
            o.0 += " ";
        }
        "cropbb" => {
            let m = get_vol(cx, t)?;
            let mut r = MriRegion {
                x: t.i(),
                y: t.i(),
                z: t.i(),
                dx: t.i(),
                dy: t.i(),
                dz: t.i(),
            };
            o.i(i64::from(st::mri_crop_bounding_box(&m, &mut r)));
            for v in [r.x, r.y, r.z, r.dx, r.dy, r.dz] {
                o.i(i64::from(v));
            }
        }
        "crass" => {
            let mut src = get_vol(cx, t)?;
            let dst = get_vol(cx, t)?;
            if t.i() != 0 {
                cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_SHOW
                    | larmorx_freesurfer_core::diag::DIAG_VERBOSE;
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let (r, aa, ss) = st::mri_calc_cras_for_sampled_volume(g, a, b, &mut src, &dst)?;
            for v in [r, aa, ss] {
                o.d(v);
            }
            o.am(src.vg.i_to_r.as_ref());
            o.m(src.vg.r_to_i.as_ref());
        }
        "chkostr" => {
            let s = t.hex();
            match orn::check_orientation_string(g, &s) {
                Some(e) => o.hex(&e),
                None => o.0 += "NULL ",
            }
        }
        "o2dc" => {
            let mut m = get_vol(cx, t)?;
            let s = t.hex();
            let (g, a) = (&mut cx.g, &mut cx.a);
            o.i(i64::from(orn::orientation_string_to_dircos(
                g, a, &mut m, &s,
            )));
            o.geom_v(&m.vg, false);
        }
        "s44" => {
            let (rows, cols, cplx) = (t.i(), t.i(), t.i());
            let ty = if cplx != 0 {
                mx::MATRIX_COMPLEX
            } else {
                mx::MATRIX_REAL
            };
            let mut m = if rows > 0 {
                mx::matrix_alloc(g, a, b, rows, cols, ty)?
            } else {
                None
            };
            let mut v = [0.0f32; 16];
            for x in v.iter_mut() {
                *x = t.f();
            }
            o.i(i64::from(orn::stuff_four_by_four(g, b, m.as_mut(), &v)));
            match (&m, cplx != 0) {
                (Some(m), true) => {
                    o.i(i64::from(m.rows));
                    o.i(i64::from(m.cols));
                    for r in 1..=m.rows {
                        for c in 1..=m.cols {
                            let (re, im) = m.celt(r, c);
                            o.f(re);
                            o.f(im);
                        }
                    }
                }
                _ => o.m(m.as_ref()),
            }
        }
        "vx2rx" => {
            let src = get_vol(cx, t)?;
            let dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let mv = (t.i() != 0).then(|| get_list(t));
            let mut mr = (t.i() != 0).then(|| get_list(t));
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = orn::mri_voxel_xform_to_ras_xform(
                g,
                a,
                b,
                &src,
                dst.as_ref(),
                mv.as_ref(),
                mr.as_mut(),
            )?;
            o.i(i64::from(matches!(r, Ret::Given)));
            match r {
                Ret::New(m) => o.m(Some(&m)),
                Ret::Given => o.m(mr.as_ref()),
                Ret::Null => o.m(None),
            }
        }
        "v2v" => {
            let src = get_vol(cx, t)?;
            let dst = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = orn::mri_get_voxel_to_voxel_xform(g, a, b, &src, &dst)?;
            o.m(r.as_ref());
        }
        "setv2r" => {
            let mut m = get_vol(cx, t)?;
            let e = t.i() != 0;
            let ev = t.hex();
            let env = if e {
                Environment::from_pairs([(
                    "FS_SetVoxToRasXform_Change_VoxSize",
                    String::from_utf8(ev).expect("ASCII"),
                )])
            } else {
                Environment::default()
            };
            let x = get_list(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            g.errno = 0;
            let r = orn::mri_set_voxel_to_ras_xform(g, a, b, &env, &mut m, &x)?;
            o.i(i64::from(r));
            o.i(i64::from(cx.g.errno));
            o.vol(Some(&m));
        }
        "toimgv" | "toimg" => {
            let m = get_vol(cx, t)?;
            let mut given = if t.i() != 0 {
                Some(get_img(cx, t)?)
            } else {
                None
            };
            let slice = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = if cmd == "toimgv" {
                let (view, frame) = (t.i(), t.i());
                img::mri_to_image_view(g, b, &m, given.as_mut(), slice, view, frame)?
            } else {
                img::mri_to_image(g, b, &m, given.as_mut(), slice)?
            };
            if cmd == "toimg" {
                o.i(i64::from(matches!(r, Ret::Given)));
            }
            match r {
                Ret::New(i) => o.img(Some(&i)),
                Ret::Given => o.img(given.as_ref()),
                Ret::Null => o.img(None),
            }
        }
        "img2mri" => {
            let mut im = get_img(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let m = img::image_to_mri(g, a, b, &mut im)?;
            o.vol(Some(&m));
            o.f(im.xsize);
            o.f(im.ysize);
        }
        "smpar" => {
            let m = painted(cx, t)?;
            let count = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = lb::mri_smooth_parcellation(g, a, b, &m, count)?;
            o.vol(r.as_ref());
        }
        "erasebp" => {
            let mut m = get_vol(cx, t)?;
            let border = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            o.i(i64::from(lb::mri_erase_border_planes(
                g, b, &mut m, border,
            )?));
            o.vol(Some(&m));
        }
        "nbhd" => {
            let m = painted(cx, t)?;
            let (which, wsize, x, y, z, frame, thresh) =
                (t.i(), t.i(), t.i(), t.i(), t.i(), t.i(), t.f());
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = match which {
                0 => lb::mri_max_in_nbhd(g, b, &m, wsize, x, y, z, frame)?,
                1 => lb::mri_count_nonzero_in_nbhd(g, b, &m, wsize, x, y, z)?,
                2 => lb::mri_are_nonzero_in_nbhd(g, b, &m, wsize, x, y, z)?,
                _ => lb::mri_count_thresh_in_nbhd(g, b, &m, wsize, x, y, z, thresh)?,
            };
            o.i(i64::from(r));
        }
        "nearnz" => {
            let m = painted(cx, t)?;
            let (wsize, xr, yr, zr, want) = (t.i(), t.d(), t.d(), t.d(), t.i() != 0);
            let mut p = (-9, -9, -9);
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = lb::mri_find_nearest_nonzero_location(
                g,
                b,
                &m,
                wsize,
                xr,
                yr,
                zr,
                want.then_some(&mut p),
            )?;
            o.f(r);
            for v in [p.0, p.1, p.2] {
                o.i(i64::from(v));
            }
        }
        "lmeans" => {
            let src = get_vol(cx, t)?;
            let lab = painted(cx, t)?;
            let (label, which, x0, y0, z0, whalf, want) =
                (t.i(), t.i(), t.i(), t.i(), t.i(), t.i(), t.i() != 0);
            let mut sd = -7.0f64;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match which {
                0 => {
                    let v = lb::mri_mean_in_label_multispectral(g, a, b, &src, &lab, label)?;
                    o.m(v.as_ref());
                }
                1 => o.d(lb::mri_mean_in_label(g, b, &src, &lab, label)?),
                2 => {
                    let p = want.then_some(&mut sd);
                    o.d(lb::mri_mean_and_std_in_label(g, b, &src, &lab, label, p)?);
                    o.d(sd);
                }
                3 => {
                    let p = want.then_some(&mut sd);
                    o.d(lb::mri_mean_in_label_in_region(
                        g, b, &src, &lab, label, x0, y0, z0, whalf, p,
                    )?);
                    o.d(sd);
                }
                _ => o.d(lb::mri_max_in_label_in_region(
                    g, b, &src, &lab, label, x0, y0, z0, whalf,
                )?),
            }
        }
        "mkpos" | "mlbv" => {
            let src = if cmd == "mlbv" {
                painted(cx, t)?
            } else {
                get_vol(cx, t)?
            };
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "mkpos" {
                lb::mri_make_positive(g, a, b, &src, dst.as_mut())?
            } else {
                let (label, mark, six) = (t.i(), t.i(), t.i() != 0);
                lb::mri_mark_label_border_voxels(g, a, b, &src, dst.as_mut(), label, mark, six)?
            };
            o.ret(&r, dst.as_ref());
        }
        "lnbhd" => {
            let lab = painted(cx, t)?;
            let vals = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (x, y, z, whalf, maxl) = (t.i(), t.i(), t.i(), t.i(), t.i());
            let mut counts = vec![-5i32; maxl as usize];
            let mut means = vec![-5.0f32; maxl as usize];
            let (g, b) = (&mut cx.g, &mut cx.b);
            let v = vals.as_ref().map(|m| (m, means.as_mut_slice()));
            let r = lb::mri_compute_label_nbhd(g, b, &lab, v, x, y, z, &mut counts, whalf)?;
            o.i(i64::from(r));
            for c in &counts {
                o.i(i64::from(*c));
            }
            if vals.is_some() {
                for f in &means {
                    o.f(*f);
                }
            }
        }
        "linbhd" => {
            let m = painted(cx, t)?;
            let (x, y, z, whalf, label, which) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = if which < 0 {
                lb::mri_labels_in_nbhd(g, b, &m, x, y, z, whalf, label)?
            } else {
                lb::mri_labels_in_planar_nbhd(g, b, &m, x, y, z, whalf, label, which)?
            };
            o.i(i64::from(r));
        }
        "lbbox" => {
            let m = painted(cx, t)?;
            let label = t.i();
            let mut r = MriRegion {
                x: 0,
                y: 0,
                z: 0,
                dx: 0,
                dy: 0,
                dz: 0,
            };
            let (g, b) = (&mut cx.g, &mut cx.b);
            o.i(i64::from(lb::mri_label_bounding_box(
                g, b, &m, label, &mut r,
            )?));
            for v in [r.x, r.y, r.z, r.dx, r.dy, r.dz] {
                o.i(i64::from(v));
            }
        }
        "fillbox" => {
            let mut m = get_vol(cx, t)?;
            let r = MriRegion {
                x: t.i(),
                y: t.i(),
                z: t.i(),
                dx: t.i(),
                dy: t.i(),
                dz: t.i(),
            };
            let v = t.f();
            let (g, b) = (&mut cx.g, &mut cx.b);
            o.i(i64::from(lb::mri_fill_box(g, b, &mut m, &r, v)?));
            o.vol(Some(&m));
        }
        "mskz" => {
            let mut src = get_vol(cx, t)?;
            let mask = painted(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = lb::mri_mask_zero(g, a, b, p, &mask)?;
            let given = match mode {
                1 => dst.as_ref(),
                2 => Some(&src),
                _ => None,
            };
            o.ret(&r, given);
        }
        "clipwm" => {
            let mut t1 = painted(cx, t)?;
            let wm = painted(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(lb::mri_clip_bright_wm(g, a, b, &mut t1, &wm)?));
            o.vol(Some(&t1));
        }
        "pvlab" => {
            let mut lab = get_vol(cx, t)?;
            fill_painted(cx, t, &mut lab)?;
            let vals = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let (label, maxl) = (t.i(), t.i());
            let mut mix = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let mut nbr = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let sb = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = lb::mri_voxels_in_label_with_partial_volume_effects(
                g,
                a,
                b,
                &lab,
                vals.as_ref(),
                label,
                mix.as_mut(),
                nbr.as_mut(),
                maxl,
                sb.as_ref(),
            )?;
            o.f(r);
            o.vol(mix.as_ref());
            o.vol(nbr.as_ref());
        }
        "randn" | "const" => {
            let dims = [t.i(), t.i(), t.i(), t.i()];
            let (a, b, seed) = (t.f(), t.f(), t.i());
            let mut m = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            larmorx_freesurfer_core::random::set_random_seed(&mut cx.g, i64::from(seed));
            let (g, a_, b_) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "randn" {
                fr::mri_randn(g, a_, b_, dims, a, b, m.as_mut())?
            } else {
                fr::mri_const(g, a_, b_, dims, a, m.as_mut())?
            };
            o.ret(&r, m.as_ref());
        }
        "nseq" | "nfr" => {
            let mut m = get_vol(cx, t)?;
            fill_painted(cx, t, &mut m)?;
            let (which, target) = (t.i(), t.f());
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = match (cmd, which) {
                ("nseq", _) => fr::mri_normalize_sequence(g, b, &mut m, target)?,
                (_, 0) => fr::mri_normalize_frames(g, b, &mut m)?,
                (_, 1) => fr::mri_normalize_frames_mean(g, b, &mut m)?,
                _ => fr::mri_normalize_frames_first(g, b, &mut m)?,
            };
            o.i(i64::from(r));
            o.vol(Some(&m));
        }
        "sort" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            let mask = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let mut s = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fr::mri_sort(g, a, b, &input, mask.as_ref(), s.as_mut())?;
            o.ret(&r, s.as_ref());
        }
        "revsl" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            if t.i() != 0 {
                input.bvals = Some(get_list(t));
            }
            if t.i() != 0 {
                input.bvecs = Some(get_list(t));
            }
            input.bvec_space = t.i();
            let mut ov = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fr::mri_reverse_slice_order(g, a, b, &input, ov.as_mut())?;
            o.ret(&r, ov.as_ref());
            let res = match &r {
                Ret::New(m) => Some(m),
                Ret::Given => ov.as_ref(),
                Ret::Null => None,
            };
            if let Some(m) = res {
                o.m(m.bvals.as_ref());
                o.m(m.bvecs.as_ref());
                o.i(i64::from(m.bvec_space));
            }
        }
        "cdtype" => {
            let input = get_vol(cx, t)?;
            let ty = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fr::mri_clone_different_type(g, a, b, &input, ty)?;
            o.vol(Some(&r));
        }
        "nonmax" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            let mut s = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (thresh, dir) = (t.f(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = dv::mri_non_max_suppress(g, a, b, &input, s.as_mut(), thresh, dir)?;
            o.ret(&r, s.as_ref());
        }
        "xrpad" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            let reg = if t.i() != 0 {
                Some(MriRegion {
                    x: t.i(),
                    y: t.i(),
                    z: t.i(),
                    dx: t.i(),
                    dy: t.i(),
                    dz: t.i(),
                })
            } else {
                None
            };
            let pad = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = rs::mri_extract_region_and_pad(g, a, b, &mut input, reg.as_ref(), pad)?;
            o.vol(Some(&r));
        }
        "rms" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            let mut ov = get_vol(cx, t)?;
            let (g, b) = (&mut cx.g, &mut cx.b);
            cb::mri_rms(g, b, &input, &mut ov)?;
            o.vol(Some(&ov));
        }
        "vfrac" => {
            let src = get_vol(cx, t)?;
            let m = get_list(t);
            let mut seg = get_vol(cx, t)?;
            fill_painted(cx, t, &mut seg)?;
            let mut fr = get_vol(cx, t)?;
            fill_painted(cx, t, &mut fr)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.i(i64::from(cb::mri_compute_volume_fractions(
                g, a, b, &src, &m, &seg, &mut fr,
            )?));
            o.vol(Some(&fr));
        }
        "cdt" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let mut v2 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v2)?;
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = cb::mri_combine_distance_transforms(g, a, b, &v1, &v2, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "diffv" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let mut v2 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v2)?;
            let mask = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let thresh = t.d();
            let amask = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let mode = t.i();
            let mut dst = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            if t.i() != 0 {
                v1.ct = Some(larmorx_freesurfer_mri::ctab::ColorTable {
                    entries: vec![Some(Box::default()), Some(Box::default())],
                    ..Default::default()
                });
            }
            let extra = t.i();
            let mut more = Vec::new();
            for _ in 0..extra {
                more.push([t.d(), t.d(), t.d(), t.d(), t.d()]);
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match cb::mri_diff2vect(g, a, b, &v1, &v2, thresh, mask.as_ref())? {
                None => o.0 += "NULL ",
                Some(mut dv) => {
                    o.i(dv.len() as i64);
                    for row in &dv {
                        for d in row {
                            o.d(*d);
                        }
                    }
                    dv.extend(more);
                    let mut napply = -1;
                    let to = match mode {
                        1 => cb::DiffDst::Given(dst.as_mut().expect("a volume")),
                        2 => cb::DiffDst::InPlace,
                        _ => cb::DiffDst::New,
                    };
                    let r = cb::mri_apply_diff_vect(
                        g,
                        a,
                        b,
                        &mut v1,
                        amask.as_ref(),
                        &dv,
                        &mut napply,
                        to,
                    )?;
                    o.i(i64::from(napply));
                    o.i(i64::from(mode != 0 && matches!(r, Ret::Given)));
                    match (&r, mode) {
                        (Ret::New(m), _) => o.vol(Some(m)),
                        (Ret::Given, 1) => o.vol(dst.as_ref()),
                        (Ret::Given, _) => o.vol(Some(&v1)),
                        (Ret::Null, _) => o.vol(None),
                    }
                }
            }
        }
        "tiv" => {
            let ext = String::from_utf8(t.hex()).expect("an ASCII name");
            let bytes = t.hex();
            let have = t.i() != 0;
            let env = match t.i() {
                0 => Environment::default(),
                1 => Environment::from_pairs([("USE_AVERAGE305", "true")]),
                _ => Environment::from_pairs([("USE_AVERAGE305", "")]),
            };
            let scale = t.d();
            static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("larmorx-fsvol-tiv-{}-{call}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch folder");
            let name = format!("fsvol-lta{ext}");
            let path = dir.join(&name);
            let _ = std::fs::remove_file(&path);
            if have {
                std::fs::write(&path, &bytes).expect("the case's file");
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            g.errno = 0;
            let r = cb::mri_estimate_tiv(g, a, b, Some(&dir), &name, scale, &mut NoIo(env));
            let _ = std::fs::remove_dir_all(&dir);
            let (icv, det) = r?;
            o.d(icv);
            o.d(det);
        }
        "fpow" => {
            let mut m = get_vol(cx, t)?;
            fill_painted(cx, t, &mut m)?;
            let n = t.i();
            let mut p = vec![1.0f32; n.max(1) as usize];
            for v in p.iter_mut().take(n.max(0) as usize) {
                *v = t.f();
            }
            let (g, b) = (&mut cx.g, &mut cx.b);
            o.i(i64::from(vx::mri_framepower(g, b, &mut m, &p)?));
            o.vol(Some(&m));
        }
        "mbin" => {
            let mut v = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v)?;
            let mode = t.i();
            let mut d = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let th = t.f();
            let tail = if t.i() != 0 { Some(t.hex()) } else { None };
            let inv = t.i() != 0;
            let mut nover = -9;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = match (mode, d.as_mut()) {
                (1, Some(d)) => Pair::Given(&v, d),
                (2, _) => Pair::Same(&mut v),
                _ => Pair::New(&v),
            };
            let r = vx::mri_binarize(g, a, b, p, th, tail.as_deref(), inv, &mut nover)?;
            o.i(i64::from(nover));
            o.i(i64::from(mode != 0 && matches!(r, Ret::Given)));
            match (&r, mode) {
                (Ret::New(m), _) => o.vol(Some(m)),
                (Ret::Given, 1) => o.vol(d.as_ref()),
                (Ret::Given, _) => o.vol(Some(&v)),
                (Ret::Null, _) => o.vol(None),
            }
        }
        "reshape" => {
            let mut v = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v)?;
            let shape = [t.i(), t.i(), t.i(), t.i()];
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_reshape(g, a, b, &mut v, shape)?;
            o.vol(r.as_ref());
            o.i(i64::from(v.nframes));
        }
        "mmax" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let mut v2 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v2)?;
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_max(g, a, b, &v1, &v2, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "mconst" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let (k, mul) = (t.d(), t.i() != 0);
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_const_op(g, a, b, &v1, k, mul, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "fbin" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let th = t.d();
            let mut m = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_frame_binarize(g, a, b, &v1, th, m.as_mut())?;
            o.ret(&r, m.as_ref());
        }
        "vote" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let m = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let mut vt = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let ex = t.i() != 0;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_vote(g, a, b, &mut v1, m.as_ref(), vt.as_mut(), ex)?;
            o.ret(&r, vt.as_ref());
            o.vol(Some(&v1));
        }
        "msq" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let which = t.i();
            let m = if t.i() != 0 {
                let mut m = get_vol(cx, t)?;
                fill_painted(cx, t, &mut m)?;
                Some(m)
            } else {
                None
            };
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match which {
                0 => vx::mri_square(g, a, b, &v1, m.as_ref(), d.as_mut())?,
                1 => vx::mri_square_root(g, a, b, &v1, m.as_ref(), d.as_mut())?,
                _ => vx::mri_sqrt(g, a, b, &v1, d.as_mut())?,
            };
            o.ret(&r, d.as_ref());
        }
        "crop" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let p1 = [t.i(), t.i(), t.i()];
            let p2 = [t.i(), t.i(), t.i()];
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_crop(g, a, b, &v1, p1, p2)?;
            o.vol(r.as_ref());
        }
        "revs" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let env = Environment::default();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_reverse_slices(g, a, b, &env, &v1, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "cutend" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let n = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_cut_end_slices(g, a, b, &v1, n)?;
            o.vol(r.as_ref());
        }
        "segids" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let (frame, not0) = (t.i(), t.i() != 0);
            let (g, b) = (&mut cx.g, &mut cx.b);
            let l = if not0 {
                vx::mri_seg_id_list_not0(g, b, &v1, frame)?
            } else {
                vx::mri_seg_id_list(g, b, &v1, frame)?
            };
            o.i(l.len() as i64);
            for v in l {
                o.i(i64::from(v));
            }
        }
        "hcos" => {
            let mut v1 = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v1)?;
            let al = t.d();
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = vx::mri_half_cos_bias(g, a, b, &v1, al, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "ordidx" | "vperm" => {
            let mut input = get_vol(cx, t)?;
            fill_painted(cx, t, &mut input)?;
            let seed = t.i();
            let n = input
                .vg
                .width
                .wrapping_mul(input.vg.height)
                .wrapping_mul(input.vg.depth);
            let n = usize::try_from(n).unwrap_or(0);
            let (mut xs, mut ys, mut zs) = (vec![-7i16; n], vec![-7i16; n], vec![-7i16; n]);
            larmorx_freesurfer_core::random::set_random_seed(&mut cx.g, i64::from(seed));
            let env = Environment::default();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "ordidx" {
                od::mri_order_indices(g, b, &input, &mut xs, &mut ys, &mut zs)?
            } else {
                od::mri_compute_voxel_permutation(g, &env, a, b, &input, &mut xs, &mut ys, &mut zs)?
            };
            o.i(i64::from(r));
            for v in xs.iter().chain(&ys).chain(&zs) {
                o.i(i64::from(*v));
            }
        }
        "rsmat" => {
            let src = get_vol(cx, t)?;
            let tmpl = get_vol(cx, t)?;
            if t.i() != 0 {
                cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_SHOW
                    | larmorx_freesurfer_core::diag::DIAG_VERBOSE;
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let m = xf::mri_get_resample_matrix(g, a, b, &src, &tmpl)?;
            o.m(m.as_ref());
        }
        "vv" | "vvsm" | "vvtk" | "vvk" | "vvd" | "maskup" | "cov" | "mpca" | "pcast" | "fdr"
        | "rgbb" | "vxn" => piece_d_and_regions(cx, t, o, cmd)?,
        "talcras" | "talmat" | "talpt" | "talvol" => r22_tal(cx, t, o, cmd)?,
        "dilate" | "dilu" | "msegs" => r22_dilate_msegs(cx, t, o, cmd)?,
        "sstat" | "hseg" => r22_stats(cx, t, o, cmd)?,
        "conform" => r23_conform(cx, t, o)?,
        "union" | "isect" | "mand" | "mxor" => r23_set2(cx, t, o, cmd)?,
        c if R23_SET.contains(&c) => r23_set1(cx, t, o, cmd)?,
        "rlist" | "rvonly" | "mdiff" | "mmask" | "tmask" | "mnot" | "slv" | "b2c" => {
            r23_mask1(cx, t, o, cmd)?
        }
        "nmask" | "von" | "vinl" | "linv" | "cent" | "pthr" | "cnbhd" => r23_meas(cx, t, o, cmd)?,
        "grow" | "onfg" | "offbg" | "cplab" | "cplv" => r23_multi(cx, t, o, cmd)?,
        c if R24_HISTO.contains(&c) => r24_histo(cx, t, o, cmd)?,
        c if R25_LTA.contains(&c) => r25_lta(cx, t, o, cmd)?,
        c if R25_RES.contains(&c) => r25_res(cx, t, o, cmd)?,
        c if R25_MRI2.contains(&c) => r25_mri2(cx, t, o, cmd)?,
        c => panic!("unknown command {c}"),
    }
    Ok(())
}

/// `cmph`'s header fields beyond the volume's tokens.
/// fs.io's functions for the transform readers: never reached by the cases (they name no volume
/// that exists and no `register.dat`); the case's environment.
struct NoIo(Environment);

impl lr::LtaIo for NoIo {
    fn env(&self) -> &Environment {
        &self.0
    }
    fn read_header(
        &mut self,
        _: &mut Globals,
        _: &mut dyn Write,
        _: &mut dyn Write,
        _: Option<&std::path::Path>,
        name: &str,
    ) -> Result<Option<VolGeom>, Fatal> {
        panic!("MRIreadHeader({name}) is fs.io's")
    }
    fn read_register(
        &mut self,
        _: &mut Globals,
        _: &mut dyn Write,
        _: &mut dyn Write,
        _: Option<&std::path::Path>,
        name: &str,
    ) -> Result<Option<lr::RegisterDat>, Fatal> {
        panic!("regio_read_register({name}) is fs.io's")
    }
}

/// The commands of relay 21 (3.1-37's piece d rest, 3.1-38's regions and voxel measures), out of
/// [`case`] to keep its frame small (the debug build's test threads).
#[inline(never)]
fn piece_d_and_regions(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    match cmd {
        "vv" | "vvsm" | "vvtk" => r21_vv(cx, t, o, cmd)?,
        "vvk" => r21_vvk(cx, t, o, cmd)?,
        "vvd" => r21_vvd(cx, t, o, cmd)?,
        "maskup" => r21_maskup(cx, t, o, cmd)?,
        "cov" | "mpca" => r21_cov(cx, t, o, cmd)?,
        "pcast" => r21_pcast(cx, t, o, cmd)?,
        "fdr" => r21_fdr(cx, t, o, cmd)?,
        "rgbb" => r21_rgbb(cx, t, o, cmd)?,
        "vxn" => r21_vxn(cx, t, o, cmd)?,
        c => panic!("unknown command {c}"),
    }
    Ok(())
}

#[inline(never)]
fn r21_vv(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    src.outside_val = t.d();
    let mut targ = get_vol(cx, t)?;
    fill_painted(cx, t, &mut targ)?;
    let m = get_m44(t);
    let (interp, param) = (t.i(), t.f());
    if t.i() != 0 {
        cx.g.gdiag_no = 1;
    }
    if t.i() != 0 {
        cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_VERBOSE;
    }
    let (mut vsm, mut lta, mut pedir) = (None, None, 0);
    if cmd != "vv" {
        if t.i() != 0 {
            let mut v = get_vol(cx, t)?;
            fill_painted(cx, t, &mut v)?;
            v.outside_val = t.d();
            vsm = Some(v);
        }
        if t.i() != 0 {
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let mut l = larmorx_freesurfer_mri::lta::lta_alloc(g, a, b, 1, None)?;
            let ty = t.i();
            l.ltype = ty;
            l.xforms[0].ltype = ty;
            let ml = l.xforms[0].m_l.as_mut().expect("LTAalloc's matrix");
            for r in 1..=4 {
                for c in 1..=4 {
                    ml.set_relt(r, c, t.f());
                }
            }
            lta = Some(l);
        }
        pedir = t.i();
    }
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let r = if cmd == "vv" {
        v2::mri_vol2vol(g, a, b, &src, &mut targ, m.as_ref(), interp, param)?
    } else if cmd == "vvtk" {
        v2::mri_vol2vol_tkreg_vsm(
            g,
            a,
            b,
            &src,
            &mut targ,
            m.as_ref(),
            interp,
            param,
            vsm.as_ref(),
            lta.as_ref(),
            pedir,
        )?
    } else {
        v2::mri_vol2vol_vsm(
            g,
            a,
            b,
            &src,
            &mut targ,
            m.as_ref(),
            interp,
            param,
            vsm.as_ref(),
            lta.as_ref(),
            pedir,
        )?
    };
    o.i(i64::from(r));
    o.vol(Some(&targ));
    Ok(())
}

#[inline(never)]
fn r21_vvk(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    let mut targ = get_vol(cx, t)?;
    fill_painted(cx, t, &mut targ)?;
    let m = get_m44(t);
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    v2::mri_vol2vol_tl_kernel(g, a, b, &src, &mut targ, m.as_ref())?;
    o.i(1);
    o.vol(Some(&targ));
    Ok(())
}

#[inline(never)]
fn r21_vvd(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mov = get_vol(cx, t)?;
    let targ = get_vol(cx, t)?;
    let m = get_m44(t);
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let r = v2::mri_vol2vol_delta(g, a, b, &mov, &targ, m.as_ref())?;
    o.vol(r.as_ref());
    Ok(())
}

#[inline(never)]
fn r21_maskup(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    let mask = if t.i() != 0 {
        let mut m = get_vol(cx, t)?;
        fill_painted(cx, t, &mut m)?;
        Some(m)
    } else {
        None
    };
    let (up, npad, cons) = (t.i(), t.i(), t.i() != 0);
    if t.i() != 0 {
        cx.g.gdiag_no = 1;
    }
    let env = Environment::default();
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    match v2::mri_mask_and_upsample(g, a, b, &env, &mut src, mask.as_ref(), up, npad, cons)? {
        Some((m, l)) => {
            o.vol(Some(&m));
            o.lta(l.as_ref());
        }
        None => {
            o.vol(None);
            o.lta(None);
        }
    }
    Ok(())
}

#[inline(never)]
fn r21_cov(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut a = get_vol(cx, t)?;
    fill_painted(cx, t, &mut a)?;
    let mask = if t.i() != 0 {
        let mut m = get_vol(cx, t)?;
        fill_painted(cx, t, &mut m)?;
        Some(m)
    } else {
        None
    };
    let (g, a_, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    if cmd == "cov" {
        let m = pc::mri_covariance_matrix(g, a_, b, &a, mask.as_ref())?;
        o.m(m.as_ref());
    } else {
        let (r, p) = pc::mri_pca(g, a_, b, &a, mask.as_ref())?;
        o.i(i64::from(r));
        o.m(p.as_ref().map(|p| &p.u));
        o.m(p.as_ref().map(|p| &p.s));
        o.vol(p.as_ref().map(|p| &p.v));
    }
    Ok(())
}

#[inline(never)]
fn r21_pcast(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let n = t.i();
    let mut sv = Matrix::real(1, n);
    for i in 1..=n {
        let v = t.f();
        sv.set_relt(1, i, v);
    }
    let mode = t.i();
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("larmorx-fsvol-pca-{}-{call}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let r = match mode {
        0 => pc::print_pca_stats(&mut cx.a, &sv),
        1 => pc::write_pca_stats(&mut cx.a, Some(&dir), "fsvol-pca.dat", &sv),
        _ => pc::write_pca_stats(&mut cx.a, Some(&dir), "fsvol-nodir/pca.dat", &sv),
    };
    o.i(i64::from(r));
    let bytes = std::fs::read(dir.join("fsvol-pca.dat")).unwrap_or_default();
    o.hex(&bytes);
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[inline(never)]
fn r21_fdr(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let n = t.i();
    let mut vols = Vec::new();
    let mut frames = Vec::new();
    let mut masks = Vec::new();
    let mut outs = Vec::new();
    for _ in 0..n {
        let mut v = get_vol(cx, t)?;
        fill_painted(cx, t, &mut v)?;
        vols.push(v);
        frames.push(t.i());
        masks.push(if t.i() != 0 {
            let mut m = get_vol(cx, t)?;
            fill_painted(cx, t, &mut m)?;
            Some(m)
        } else {
            None
        });
        let mode = t.i();
        outs.push((
            mode,
            if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            },
        ));
    }
    let fdr = t.d();
    let (sign, lg, dbg) = (t.i(), t.i() != 0, t.i() != 0);
    let (passmask, passout) = (t.i() != 0, t.i() != 0);
    let mut vwth = -7.0f64;
    let r = {
        let mut items: Vec<pc::FdrVol> = vols
            .iter_mut()
            .zip(frames.iter())
            .zip(masks.iter())
            .zip(outs.iter_mut())
            .map(|(((v, &frame), m), (mode, ov))| pc::FdrVol {
                vol: v,
                frame,
                mask: if passmask { m.as_ref() } else { None },
                out: match (passout, *mode, ov.as_mut()) {
                    (true, 1, Some(o)) => pc::FdrOut::Given(o),
                    (true, 2, _) => pc::FdrOut::Same,
                    _ => pc::FdrOut::None,
                },
            })
            .collect();
        let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
        pc::mri_fdr2vwth(g, a, b, &mut items, fdr, sign, lg, &mut vwth, dbg)?
    };
    o.i(i64::from(r));
    o.d(vwth);
    if passout {
        for (i, (mode, ov)) in outs.iter().enumerate() {
            match mode {
                1 => o.vol(ov.as_ref()),
                2 => o.vol(Some(&vols[i])),
                _ => {}
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn r21_rgbb(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut m = get_vol(cx, t)?;
    fill_painted(cx, t, &mut m)?;
    let kind = t.i();
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let r = match kind {
        0 => {
            let npad = t.i();
            Some(rg::region_get_bounding_box(g, b, &m, npad)?)
        }
        1 => {
            let p = [t.i(), t.i(), t.i(), t.i(), t.i(), t.i()];
            Some(rg::region_get_bounding_box_m(g, b, &m, p)?)
        }
        _ => {
            let npad = t.i();
            rg::region_get_bounding_box_eq_odd(g, a, b, &m, npad)?
        }
    };
    match r {
        None => o.0 += "NULL ",
        Some(r) => {
            for v in [r.x, r.y, r.z, r.dx, r.dy, r.dz] {
                o.i(i64::from(v));
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn r21_vxn(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let _ = cmd;
    let mut m = get_vol(cx, t)?;
    fill_painted(cx, t, &mut m)?;
    let n = t.i();
    let (g, b) = (&mut cx.g, &mut cx.b);
    for _ in 0..n {
        let (x, y, z, w, f) = (t.i(), t.i(), t.i(), t.i(), t.i());
        o.f(mv::mri_voxel_dx(g, b, &m, x, y, z)?);
        o.f(mv::mri_voxel_dy(g, b, &m, x, y, z)?);
        o.f(mv::mri_voxel_dz(g, b, &m, x, y, z)?);
        o.f(mv::mri_voxel_mean(g, b, &m, x, y, z, w, f)?);
        o.f(mv::mri_voxel_direction(g, b, &m, x, y, z, w)?);
        o.f(mv::mri_voxel_gradient_dir_2nd_derivative(
            g, b, &m, x, y, z, w,
        )?);
    }
    Ok(())
}

/// `talairachex.cpp`'s commands (relay 22).
fn r22_tal(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let kind = if cmd == "talcras" { 0 } else { t.i() };
    if cmd == "talpt" && kind == 4 {
        let mut m = Matrix::real(4, 4);
        for r in 1..=4 {
            for c in 1..=4 {
                m.set_relt(r, c, t.f());
            }
        }
        let n = t.i();
        for _ in 0..n {
            let (x, y, z) = (t.d(), t.d(), t.d());
            let p = tx::transform_with_matrix(&m, x, y, z);
            o.i(0);
            o.d(p[0]);
            o.d(p[1]);
            o.d(p[2]);
        }
        return Ok(());
    }
    r22_tal_vol(cx, t, o, cmd, kind)
}

#[inline(never)]
fn r22_tal_vol(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str, kind: i32) -> Result<(), Fatal> {
    let mut m = get_vol(cx, t)?;
    let mut tal = None;
    if cmd == "talvol" {
        fill_painted(cx, t, &mut m)?;
        if t.i() != 0 {
            tal = Some(get_vol(cx, t)?);
        }
    }
    let mut lta = None;
    if t.i() != 0 {
        let n = t.i();
        let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
        let mut l = larmorx_freesurfer_mri::lta::lta_alloc(g, a, b, n, None)?;
        l.ltype = t.i();
        if n >= 1 {
            let ml = l.xforms[0].m_l.as_mut().expect("LTAalloc's matrix");
            for r in 1..=4 {
                for c in 1..=4 {
                    ml.set_relt(r, c, t.f());
                }
            }
            set_geom(t, &mut l.xforms[0].dst);
        }
        lta = Some(l);
    }
    let env = if t.i() != 0 {
        Environment::from_pairs([("NO_AVERAGE305", "true")])
    } else {
        Environment::default()
    };
    if t.i() != 0 {
        cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_SHOW;
    }
    if t.i() != 0 {
        cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_VERBOSE;
    }
    let interp = if cmd == "talvol" && kind == 0 {
        t.i()
    } else {
        0
    };
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let lta = lta.as_ref();
    match cmd {
        "talcras" => {
            let r = tx::modify_talairach_cras(g, a, b, &env, &mut m, lta)?;
            o.i(i64::from(r));
            o.vol(Some(&m));
        }
        "talmat" => {
            let r = match kind {
                0 => tx::mtalairach_from_voxel(g, a, b, &m, lta)?,
                1 => tx::mtal_voxel_from_voxel(g, a, b, &env, &m, lta)?,
                2 => tx::mvoxel_from_talairach(g, a, b, &m, lta)?,
                _ => tx::mvoxel_from_tal_voxel(g, a, b, &env, &m, lta)?,
            };
            o.m(r.as_ref());
        }
        "talpt" => {
            let n = t.i();
            for _ in 0..n {
                let (x, y, z) = (t.d(), t.d(), t.d());
                let (r, p) = match kind {
                    0 => tx::mri_voxel_to_talairach_ex(g, a, b, &m, x, y, z, lta)?,
                    1 => tx::mri_voxel_to_talairach_voxel_ex(g, a, b, &env, &m, x, y, z, lta)?,
                    2 => tx::mri_talairach_to_voxel_ex(g, a, b, &m, x, y, z, lta)?,
                    _ => tx::mri_talairach_voxel_to_voxel_ex(g, a, b, &env, &m, x, y, z, lta)?,
                };
                o.i(i64::from(r));
                for v in p.unwrap_or([-7.0; 3]) {
                    o.d(v);
                }
            }
        }
        _ => {
            let r = match kind {
                0 => tx::mri_to_talairach_ex_interp(g, a, b, &env, &m, tal.as_mut(), lta, interp)?,
                1 => tx::mri_to_talairach_ex(g, a, b, &env, &m, tal.as_mut(), lta)?,
                _ => tx::mri_from_talairach_ex(g, a, b, &env, &m, tal.as_mut(), lta)?,
            };
            o.ret(&r, tal.as_ref());
        }
    }
    Ok(())
}

#[inline(never)]
fn r22_dilate_msegs(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    let (mode, mut dst) = dst_mode(cx, t)?;
    let mut list = Vec::new();
    let mut maskid = 0;
    if cmd == "msegs" {
        let n = t.i();
        for _ in 0..n {
            list.push(t.i());
        }
        maskid = t.i();
    }
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let p = pair(mode, &mut src, &mut dst);
    let r = match cmd {
        "dilate" => ms::mri_dilate(g, a, b, p)?,
        "dilu" => ms::mri_dilate_uchar(g, a, b, p)?,
        _ => sg::mri_match_segs(g, a, b, p, &list, maskid)?,
    };
    // the volume the result is: the destination given, or the source itself
    let given = match mode {
        1 => dst.as_ref(),
        2 => Some(&src),
        _ => None,
    };
    o.ret(&r, given);
    Ok(())
}

#[inline(never)]
fn r22_stats(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut seg = get_vol(cx, t)?;
    fill_painted(cx, t, &mut seg)?;
    let mut vol = get_vol(cx, t)?;
    fill_painted(cx, t, &mut vol)?;
    let segid = t.i();
    let (g, b) = (&mut cx.g, &mut cx.b);
    if cmd == "sstat" {
        let frame = t.i();
        let (n, st) = sg::mri_seg_stats(g, b, &seg, segid, &vol, frame)?;
        o.i(i64::from(n));
        for v in [st.min, st.max, st.range, st.mean, st.std] {
            o.f(v);
        }
    } else {
        let (bmin, bmax, bdelta) = (t.d(), t.d(), t.d());
        let h = sg::histo_seg(g, b, &seg, segid, &vol, bmin, bmax, bdelta)?;
        o.i(i64::from(h.nbins));
        for i in 0..h.nbins.max(0) as usize {
            o.f(h.bins[i]);
            o.f(h.counts[i]);
        }
    }
    Ok(())
}

/// The samplers' commands: each point's result and value.
fn samplers(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut m = get_vol(cx, t)?;
    m.outside_val = t.d();
    let n = t.i();
    let (g, b) = (&mut cx.g, &mut cx.b);
    for _ in 0..n {
        let (x, y, z) = (t.d(), t.d(), t.d());
        let (frame, aux) = (t.i(), t.i());
        let mut val = -7.0f64;
        let r = match cmd {
            "stri" => sm::sample_volume(g, b, &m, x, y, z, &mut val),
            "styp" => sm::sample_volume_type(g, b, &m, x, y, z, &mut val, aux)?,
            "sfrm" => sm::sample_volume_frame(g, b, &m, x, y, z, frame, &mut val),
            "sfty" => sm::sample_volume_frame_type(g, b, &m, x, y, z, frame, aux, &mut val),
            "scub" => sm::cubic_sample_volume(g, b, &m, x, y, z, &mut val),
            "scubf" => sm::cubic_sample_volume_frame(g, b, &m, x, y, z, frame, &mut val),
            "ssinc" => sm::sinc_sample_volume(g, b, &m, x, y, z, aux, &mut val)?,
            _ => sm::sinc_sample_volume_frame(g, b, &m, x, y, z, frame, aux, &mut val)?,
        };
        o.i(i64::from(r));
        o.d(val);
    }
    Ok(())
}

fn hdr_fields(t: &mut Toks, m: &mut Mri) {
    m.fov = t.f();
    m.location = t.f();
    m.ptype = t.i();
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

/// The port's line for a case line, and the defects it noted. Run on a thread of its own with a
/// 32 MiB stack: a debug build gives `case`'s match (every command's locals) a frame near the
/// 2 MiB of a test's thread (relay 23 passed it).
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
    // fsvolume's Progname
    cx.g.progname = "fsvolume".to_owned();
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

/// Direction cosines (columns x, y, z; each r, a, s) a scanner gives, a slight rotation, and
/// degenerate ones (two axes along one direction: FS-491; a NaN).
const COSINES: [[f32; 9]; 9] = [
    [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0],
    [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0],
    [
        0.984_807_7,
        0.173_648_18,
        0.0,
        -0.173_648_18,
        0.984_807_7,
        0.0,
        0.0,
        0.0,
        1.0,
    ],
    [1.0, 0.0, 0.0, 0.9, 0.1, 0.0, 0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0, 0.0, f32::NAN, -1.0, 0.0, 1.0, 0.0],
];

/// A scanner-like geometry's tokens (`geom`'s order): voxel sizes, good RAS mostly, the cosines
/// of [`COSINES`] (`exact`: coronal, 1 mm, good: the conformed grid itself), a centre near 0.
fn real_geom(g: &mut Gen, s: Shape, exact: bool) -> Vec<String> {
    let mut t = vec!["1".to_owned()];
    for v in [s.w, s.h, s.d] {
        t.push(v.to_string());
    }
    for _ in 0..3 {
        let v = if exact {
            1.0
        } else if g.chance(20) {
            f32::NAN
        } else {
            g.pick(&[1.0f32, 1.0, 0.9375, 1.2, 2.0, 0.5, 3.0])
        };
        t.push(fx(v));
    }
    t.push(if exact || !g.chance(6) { "1" } else { "0" }.to_owned());
    let c = if exact {
        COSINES[0]
    } else {
        COSINES[g.below(COSINES.len() as u64) as usize]
    };
    for v in c {
        t.push(fx(v));
    }
    for _ in 0..3 {
        t.push(fx(if g.chance(3) { 0.0 } else { num(g, -30, 30, 4) }));
    }
    for _ in 0..3 {
        t.push(fx(0.0));
    }
    t.push(g.pick(&NAMES).to_owned());
    t
}

/// `mri_conform.cpp`'s cases (relay 23): `MRIconform` (kind 0; the result's voxels hashed),
/// `MRIconformedTemplate` (1), `MRIgetConformMatrix` (2).
fn r23_gen_conform(g: &mut Gen, t: &mut Vec<String>) {
    let kind = g.pick(&[0i64, 1, 1, 2, 0, 1]);
    t.push(kind.to_string());
    let mut s = shape(g);
    s.hdr = s.hdr && g.chance(6);
    s.nf = s.nf.min(2);
    // the conformed grid itself (no resampling): bytes, 256^3, coronal, 1 mm
    let exact = kind == 0 && g.chance(6);
    if exact {
        s = Shape {
            w: 256,
            h: 256,
            d: 256,
            ty: 0,
            nf: 1,
            hdr: false,
            drop: false,
        };
    } else if g.chance(2) {
        (s.w, s.h, s.d) = (g.int(4, 24), g.int(4, 24), g.int(2, 16));
    }
    let start = t.len() + 7;
    vol_of(g, t, s);
    if exact || !g.chance(3) {
        let geo = real_geom(g, s, exact);
        t.splice(start..start + geo.len(), geo);
    }
    if exact {
        // no fills (16.7 million voxels), a few voxels painted
        t.push("0".into());
        let n = g.int(0, 6);
        t.push(n.to_string());
        for _ in 0..n {
            for _ in 0..3 {
                t.push(g.int(100, 155).to_string());
            }
            t.push("0".into());
            t.push(fx(g.pick(&[0.0f32, 255.0, 17.0, 300.0, -4.0])));
        }
    } else {
        let vals: &[f32] = &[0.0, 50.0, 100.0, 200.0, 37.5, 1000.0, -20.0];
        fill_paint(g, t, s, &[&[0.0, 10.0, 100.0, 250.0]], &[vals]);
    }
    if kind == 1 {
        t.push(g.pick(&[256i64, 256, 128, 255, 64, 1, 0]).to_string());
        let cs = if g.chance(12) {
            g.pick(&[f64::NAN, 1e10, 0.0])
        } else {
            g.pick(&[1.0, 1.0, 0.7, 0.5, 2.0, 1.2])
        };
        t.push(dx(cs));
        t.push(g.pick(&[1i64, 1, 0]).to_string());
    }
}

#[inline(never)]
fn r23_conform(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let kind = t.i();
    let mut m = get_vol(cx, t)?;
    fill_painted(cx, t, &mut m)?;
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    match kind {
        0 => {
            let env = Environment::default();
            let r = cf::mri_conform(g, a, b, &env, &m)?;
            o.vol_with(r.as_ref(), true);
        }
        1 => {
            let (cw, cs, keep) = (t.i(), t.d(), t.i());
            let r = cf::mri_conformed_template(g, a, b, &m, cw, cs, keep != 0)?;
            o.vol(Some(&r));
        }
        _ => {
            let r = cf::mri_get_conform_matrix(g, a, b, &mut m)?;
            o.m(r.as_ref());
            o.geom_v(&m.vg, false);
        }
    }
    Ok(())
}

/// mriset.cpp's commands of relay 23.
const R23_SET: [&str; 15] = [
    "compl", "edges", "erodenn", "erode", "erode2d", "erodeseg", "dilseg", "dtl", "dtli", "dlu",
    "dlab", "openn", "mopen", "mclose", "dil6",
];

/// The cases of mriset.cpp's set operations and morphology (relay 23).
fn r23_gen_set(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let mut s = shape(g);
    s.hdr = s.hdr && g.chance(6);
    if g.chance(2) {
        (s.w, s.h, s.d) = (g.int(1, 7), g.int(1, 7), g.int(1, 5));
    }
    if matches!(cmd, "union" | "dtl" | "dtli" | "dlu" | "erode") && g.chance(2) {
        s.ty = 0;
    }
    vol_of(g, t, s);
    let labels: &[f32] = &[
        0.0, 0.0, 2.0, 3.0, 41.0, 1.0, -1.0, 0.5, 255.0, 300.0, 130.0,
    ];
    fill_paint(g, t, s, &[&[0.0, 0.0, 1.0, 2.0, 41.0, 130.0]], &[labels]);
    let two = matches!(cmd, "union" | "isect" | "mand" | "mxor");
    if two {
        let mut o = like(g, s);
        o.hdr = o.hdr && g.chance(6);
        vol_of(g, t, o);
        fill_paint(g, t, o, &[&[0.0, 0.0, 1.0, 2.0, 41.0]], &[labels]);
    }
    let mode = if two {
        g.pick(&[0, 0, 1, 2, 3])
    } else {
        g.pick(&[0, 0, 1, 2])
    };
    t.push(mode.to_string());
    if mode == 1 {
        let mut o = like(g, s);
        o.hdr = o.hdr && g.chance(4);
        vol_of(g, t, o);
    }
    if cmd == "dtl" || cmd == "dtli" {
        let mut o = like(g, s);
        o.hdr = o.hdr && g.chance(6);
        if g.chance(2) {
            o.ty = 0;
        }
        vol_of(g, t, o);
        let vals: &[f32] = &[0.0, 50.0, 100.0, 115.0, 120.0, 200.0, 255.0];
        fill_paint(g, t, o, &[&[0.0, 120.0, 100.0]], &[vals]);
    }
    if cmd == "dilseg" {
        let has = g.chance(2);
        t.push(i64::from(has).to_string());
        if has {
            let mut o = like(g, s);
            o.hdr = o.hdr && g.chance(6);
            vol_of(g, t, o);
            fill_paint(g, t, o, &[&[0.0, 1.0]], &[&[0.0, 1.0, 0.5, 0.7]]);
        }
    }
    let rare = g.chance(12);
    match cmd {
        "mand" => t.push(g.pick(&[0i64, 1, 2, -1, 100]).to_string()),
        "mxor" => {
            let (a, b) = g.pick(&[(1i64, 2i64), (0, 0), (2, 41), (-1, 1), (3, 1)]);
            t.push(a.to_string());
            t.push(b.to_string());
        }
        "edges" => t.push(dx(g.pick(&[0.0, 1.0, 255.0, -3.5, 1e40, f64::NAN]))),
        "erodenn" => {
            t.push(g.pick(&[1i64, 2, 3, 0]).to_string());
            t.push(g.pick(&[0i64, 1]).to_string());
        }
        "erodeseg" => {
            t.push(if rare { 0 } else { g.pick(&[1i64, 1, 2, 3]) }.to_string());
            t.push(g.pick(&[0i64, 1, 2, 3, 6]).to_string());
        }
        "dilseg" => {
            let nd = if rare {
                g.pick(&[0i64, -2])
            } else {
                g.pick(&[1i64, 1, 2, 3, -1])
            };
            t.push(nd.to_string());
            t.push(g.pick(&[0i64, 0, 1, -1]).to_string());
            t.push(dx(g.pick(&[0.5, 0.0, 1.0])));
        }
        "dtl" | "dtli" => {
            t.push(g.pick(&[2i64, 41, 130, 1, 0, 300]).to_string());
            t.push(g.pick(&[1i64, 2, 3, 0]).to_string());
            t.push(g.pick(&[0i64, 100, 115, 200, -1, 256]).to_string());
        }
        "dlu" | "dlab" => {
            t.push(g.pick(&[2i64, 41, 1, 0, 300, 130]).to_string());
            t.push(g.pick(&[1i64, 2, 0, 3]).to_string());
        }
        "openn" => t.push(g.pick(&[1i64, 2, 3, 0]).to_string()),
        _ => {}
    }
}

#[inline(never)]
fn r23_set2(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut v1 = get_vol(cx, t)?;
    fill_painted(cx, t, &mut v1)?;
    let mut v2 = get_vol(cx, t)?;
    fill_painted(cx, t, &mut v2)?;
    let mode = t.i();
    let mut d3 = if mode == 1 {
        Some(get_vol(cx, t)?)
    } else {
        None
    };
    let (t1, t2) = match cmd {
        "mand" => (t.i(), 0),
        "mxor" => (t.i(), t.i()),
        _ => (0, 0),
    };
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let tri = match mode {
        1 => Tri::Given(&v1, &v2, d3.as_mut().expect("a destination")),
        2 => Tri::First(&mut v1, &v2),
        3 => Tri::Second(&v1, &mut v2),
        _ => Tri::New(&v1, &v2),
    };
    let r = match cmd {
        "union" => ms::mri_union(g, a, b, tri)?,
        "isect" => ms::mri_intersect(g, a, b, tri)?,
        "mand" => ms::mri_and(g, a, b, tri, t1)?,
        _ => ms::mri_xor(g, a, b, tri, t1, t2)?,
    };
    let given = match mode {
        1 => d3.as_ref(),
        2 => Some(&v1),
        3 => Some(&v2),
        _ => None,
    };
    o.ret(&r, given);
    Ok(())
}

#[inline(never)]
fn r23_set1(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    let (mode, mut dst) = dst_mode(cx, t)?;
    let mut val = None;
    if cmd == "dtl" || cmd == "dtli" {
        let mut v = get_vol(cx, t)?;
        fill_painted(cx, t, &mut v)?;
        val = Some(v);
    }
    let mut mask = None;
    if cmd == "dilseg" && t.i() != 0 {
        let mut v = get_vol(cx, t)?;
        fill_painted(cx, t, &mut v)?;
        mask = Some(v);
    }
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let p = pair(mode, &mut src, &mut dst);
    let mut n = 0;
    let r = match cmd {
        "compl" => ms::mri_complement(g, a, b, p)?,
        "edges" => {
            let sv = t.d();
            ms::mri_set_edges(g, a, b, p, sv)?
        }
        "erodenn" => {
            let (nn, ee) = (t.i(), t.i());
            ms::mri_erode_nn(g, a, b, p, nn, ee)?
        }
        "erode" => ms::mri_erode(g, a, b, p)?,
        "erode2d" => ms::mri_erode_2d(g, a, b, p)?,
        "erodeseg" => {
            let (ne, th) = (t.i(), t.i());
            ms::mri_erode_segmentation(g, a, b, p, ne, th)?
        }
        "dilseg" => {
            let (nd, mf) = (t.i(), t.i());
            let mt = t.d();
            n = -7;
            ms::mri_dilate_segmentation(g, a, b, p, nd, mask.as_ref(), mf, mt, &mut n)?
        }
        "dtl" | "dtli" => {
            let (label, niter, th) = (t.i(), t.i(), t.i());
            let v = val.as_ref().expect("the value volume");
            if cmd == "dtl" {
                ms::mri_dilate_thresh_label(g, a, b, p, v, label, niter, th)?
            } else {
                ms::mri_dilate_inv_thresh_label(g, a, b, p, v, label, niter, th)?
            }
        }
        "dlu" | "dlab" => {
            let (label, niter) = (t.i(), t.i());
            if cmd == "dlu" {
                ms::mri_dilate_label_uchar(g, a, b, p, label, niter)?
            } else {
                ms::mri_dilate_label(g, a, b, p, label, niter)?
            }
        }
        "openn" => {
            let order = t.i();
            ms::mri_open_n(g, a, b, p, order)?
        }
        "mopen" => ms::mri_open(g, a, b, p)?,
        "mclose" => ms::mri_close(g, a, b, p)?,
        _ => ms::mri_dilate6(g, a, b, p)?,
    };
    let given = match mode {
        1 => dst.as_ref(),
        2 => Some(&src),
        _ => None,
    };
    o.i(i64::from(matches!(r, Ret::Given)));
    if cmd == "dilseg" {
        o.i(i64::from(n));
    }
    match &r {
        Ret::New(m) => o.vol(Some(m)),
        Ret::Given => o.vol(given),
        Ret::Null => o.vol(None),
    }
    Ok(())
}

/// mriset.cpp's masks, counts and in-place functions (relay 23).
const R23_MASK: [&str; 20] = [
    "rlist", "rvonly", "mdiff", "mmask", "tmask", "mnot", "slv", "b2c", "nmask", "von", "vinl",
    "linv", "cent", "pthr", "cnbhd", "grow", "onfg", "offbg", "cplab", "cplv",
];

/// A painted label volume of the shape `s` (its tokens).
fn r23_painted(g: &mut Gen, t: &mut Vec<String>, s: Shape) {
    vol_of(g, t, s);
    let labels: &[f32] = &[
        0.0, 0.0, 2.0, 3.0, 41.0, 1.0, -1.0, 0.5, 255.0, 300.0, 130.0,
    ];
    fill_paint(g, t, s, &[&[0.0, 0.0, 1.0, 2.0, 41.0]], &[labels]);
}

/// The cases of mriset.cpp's masks, counts and in-place functions (relay 23).
fn r23_gen_mask(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let mut s = shape(g);
    s.hdr = s.hdr && g.chance(6);
    if g.chance(2) {
        (s.w, s.h, s.d) = (g.int(1, 7), g.int(1, 7), g.int(1, 5));
    }
    if matches!(cmd, "grow" | "onfg" | "offbg" | "cplab" | "tmask") && !g.chance(4) {
        s.ty = 0;
    }
    let start = t.len();
    r23_painted(g, t, s);
    let src_toks: Vec<String> = t[start..start + 47].to_vec();
    let second = |g: &mut Gen, t: &mut Vec<String>, byte: bool| {
        let mut o = like(g, s);
        o.hdr = o.hdr && g.chance(6);
        if byte && !g.chance(4) {
            o.ty = 0;
        }
        r23_painted(g, t, o);
    };
    if R23_MASK[..8].contains(&cmd) {
        let mode = g.pick(&[0, 0, 1, 2]);
        t.push(mode.to_string());
        if mode == 1 {
            let mut o = like(g, s);
            o.hdr = o.hdr && g.chance(4);
            vol_of(g, t, o);
        }
        let with_m2 = match cmd {
            "rlist" => {
                let has = g.chance(2);
                t.push(i64::from(has).to_string());
                has
            }
            "mdiff" | "mmask" | "tmask" | "slv" => true,
            _ => false,
        };
        if with_m2 {
            if cmd == "mmask" && g.chance(2) {
                // the source's own geometry (no resampling), another type and seed
                let mut m = src_toks.clone();
                m[3] = g.pick(&["0", "3", "1", "4"]).to_owned();
                m[5] = "0".to_owned();
                m[6] = g.int(0, 255).to_string();
                t.extend(m);
                let ms = Shape {
                    ty: m_ty(&t[t.len() - 44]),
                    hdr: false,
                    ..s
                };
                let vals: &[f32] = &[0.0, 1.0, 2.0, 41.0, 0.5];
                fill_paint(g, t, ms, &[&[0.0, 1.0]], &[vals]);
            } else {
                second(g, t, cmd == "tmask");
            }
        }
        match cmd {
            "rlist" => {
                let n = g.int(0, 4);
                t.push(n.to_string());
                for _ in 0..n {
                    t.push(g.pick(&[0i64, 2, 3, 41, 1, 255, -1, 300]).to_string());
                }
                for _ in 0..n {
                    t.push(g.pick(&[5i64, 0, 77, 256, 1000, -4]).to_string());
                }
            }
            "rvonly" => {
                t.push(fx(g.pick(&[
                    0.0,
                    1.0,
                    2.0,
                    41.0,
                    0.5,
                    255.0,
                    300.0,
                    f32::NAN,
                ])));
                t.push(fx(g.pick(&[7.0, 0.0, -1.0, 255.0, 256.0, 2.5])));
            }
            "mdiff" | "mmask" => {
                t.push(g.pick(&[0i64, 1, 2, 41]).to_string());
                t.push(fx(g.pick(&[0.0, -1.0, 7.5, f32::NAN])));
            }
            "tmask" => {
                t.push(fx(g.pick(&[0.5, 1.0, 100.0, 0.0])));
                t.push(fx(g.pick(&[0.0, 300.0, -1.0, f32::NAN, 7.0])));
            }
            "slv" => {
                t.push(g.pick(&[0i64, 2, 41, 1]).to_string());
                t.push(fx(g.pick(&[9.0, -1.0, f32::NAN, 0.5])));
            }
            _ => {}
        }
        return;
    }
    match cmd {
        "von" => t.push(g.pick(&[0i64, 1, 2, 100, -1]).to_string()),
        "vinl" | "linv" => t.push(g.pick(&[0i64, 1, 2, 41, -1, 3]).to_string()),
        "pthr" => {
            let has = g.chance(2);
            t.push(i64::from(has).to_string());
            if has {
                second(g, t, false);
            }
            t.push(frame_of(g, s).to_string());
            t.push(dx(g.pick(&[
                50.0,
                0.0,
                100.0,
                90.0,
                10.0,
                -5.0,
                150.0,
                f64::NAN,
                99.5,
            ])));
        }
        "cnbhd" => {
            t.push(g.pick(&[3i64, 1, 5, 0, -1, 7]).to_string());
            for e in [s.w, s.h, s.d] {
                t.push(g.int(-1, e).to_string());
            }
            t.push(g.pick(&[0i64, 1, 2, 41]).to_string());
        }
        "grow" | "onfg" | "offbg" | "cplab" | "cplv" => {
            second(g, t, cmd != "cplv");
            if cmd == "onfg" || cmd == "cplv" {
                second(g, t, cmd == "onfg");
            }
            match cmd {
                "grow" => {
                    t.push(g.pick(&[0i64, 2, 41, 1]).to_string());
                    t.push(g.pick(&[1i64, 255, 300, 2, 0]).to_string());
                }
                "cplab" | "cplv" => t.push(g.pick(&[0i64, 2, 41, 1, 300]).to_string()),
                _ => {}
            }
        }
        _ => {}
    }
}

/// The type token of a volume's tokens (`t` at its type).
fn m_ty(tok: &str) -> i64 {
    tok.parse().expect("a type")
}

#[inline(never)]
fn r23_mask1(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut src = get_vol(cx, t)?;
    fill_painted(cx, t, &mut src)?;
    let (mode, mut dst) = dst_mode(cx, t)?;
    let need = matches!(cmd, "mdiff" | "mmask" | "tmask" | "slv") || (cmd == "rlist" && t.i() != 0);
    let mut m2 = None;
    if need {
        let mut v = get_vol(cx, t)?;
        fill_painted(cx, t, &mut v)?;
        m2 = Some(v);
    }
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let p = pair(mode, &mut src, &mut dst);
    let r = match cmd {
        "rlist" => {
            let n = t.i();
            let l1: Vec<i32> = (0..n).map(|_| t.i()).collect();
            let l2: Vec<i32> = (0..n).map(|_| t.i()).collect();
            ms::mri_replace_list(g, a, b, p, &l1, &l2, m2.as_ref())?
        }
        "rvonly" => {
            let (iv, ov) = (t.f(), t.f());
            ms::mri_replace_values_only(g, a, b, p, iv, ov)?
        }
        "mdiff" | "mmask" => {
            let (mk, ov) = (t.i(), t.f());
            let m = m2.as_ref().expect("the mask");
            if cmd == "mdiff" {
                ms::mri_mask_different_geometry(g, a, b, p, m, mk, ov)?
            } else {
                ms::mri_mask(g, a, b, &Environment::default(), p, m, mk, ov)?
            }
        }
        "tmask" => {
            let (th, ov) = (t.f(), t.f());
            ms::mri_threshold_mask(g, a, b, p, m2.as_ref().expect("the mask"), th, ov)?
        }
        "mnot" => ms::mri_not(g, a, b, p)?,
        "slv" => {
            let (label, v) = (t.i(), t.f());
            ms::mri_set_label_values(g, a, b, p, m2.as_ref().expect("the labels"), label, v)?
        }
        _ => ms::mri_bin_mask_to_col(g, a, b, p)?,
    };
    let given = match mode {
        1 => dst.as_ref(),
        2 => Some(&src),
        _ => None,
    };
    o.ret(&r, given);
    Ok(())
}

#[inline(never)]
fn r23_meas(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut m = get_vol(cx, t)?;
    fill_painted(cx, t, &mut m)?;
    match cmd {
        "nmask" => {
            let n = ms::mri_n_mask(&mut cx.g, &mut cx.b, &m)?;
            o.i(i64::from(n));
        }
        "von" => {
            let th = t.i();
            o.i(i64::from(ms::mri_total_voxels_on(
                &mut cx.g, &mut cx.b, &m, th,
            )?));
        }
        "vinl" => {
            let l = t.i();
            o.i(i64::from(ms::mri_voxels_in_label(
                &mut cx.g, &mut cx.b, &m, l,
            )?));
        }
        "linv" => {
            let l = t.i();
            o.i(i64::from(ms::mri_label_in_volume(
                &mut cx.g, &mut cx.b, &m, l,
            )?));
        }
        "cent" => {
            let (r, c) = ms::mri_compute_centroid(&mut cx.g, &mut cx.b, &m)?;
            o.i(i64::from(r));
            for v in c {
                o.d(v);
            }
        }
        "pthr" => {
            let mut mask = None;
            if t.i() != 0 {
                let mut v = get_vol(cx, t)?;
                fill_painted(cx, t, &mut v)?;
                mask = Some(v);
            }
            let (frame, pct) = (t.i(), t.d());
            let r = ms::mri_percent_thresh(&mut cx.g, &mut cx.b, &m, mask.as_ref(), frame, pct)?;
            o.d(r);
        }
        _ => {
            let (ws, x, y, z, v) = (t.i(), t.i(), t.i(), t.i(), t.i());
            let n = ms::mri_count_val_in_nbhd(&mut cx.g, &mut cx.b, &m, ws, x, y, z, v)?;
            o.i(i64::from(n));
        }
    }
    Ok(())
}

#[inline(never)]
fn r23_multi(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    let mut va = get_vol(cx, t)?;
    fill_painted(cx, t, &mut va)?;
    let mut vb = get_vol(cx, t)?;
    fill_painted(cx, t, &mut vb)?;
    let mut vc = None;
    if cmd == "onfg" || cmd == "cplv" {
        let mut v = get_vol(cx, t)?;
        fill_painted(cx, t, &mut v)?;
        vc = Some(v);
    }
    let (g, b) = (&mut cx.g, &mut cx.b);
    let r = match cmd {
        "grow" => {
            let (il, ol) = (t.i(), t.i());
            ms::mri_grow_label(g, b, &va, &mut vb, il, ol)?
        }
        "onfg" => ms::mri_turn_on_fg(g, b, &mut va, &vb, vc.as_mut().expect("bg"))?,
        "offbg" => ms::mri_turn_off_bg(g, b, &mut va, &vb)?,
        "cplab" => {
            let l = t.i();
            ms::mri_copy_label(g, b, &va, &mut vb, l)?
        }
        _ => {
            let l = t.i();
            ms::mri_copy_labeled_voxels(g, b, &va, &vb, vc.as_mut().expect("dst"), l)?
        }
    };
    o.i(i64::from(r));
    o.vol(Some(&va));
    o.vol(Some(&vb));
    if let Some(c) = vc.as_ref() {
        o.vol(Some(c));
    }
    Ok(())
}

/// `mrihisto.cpp`'s commands (relay 24).
const R24_HISTO: [&str; 9] = [
    "hreg", "hvox", "hthr", "hist", "geq", "heq", "hlr", "hlab", "hsegm",
];

/// `fsvolume.cpp`'s `fill_pat` tokens: on (0: nothing), seed, the two populations and the spread.
fn r24_pat(g: &mut Gen, t: &mut Vec<String>, s: Shape, a: &[i64], b: &[i64]) {
    r24_pat_sp(g, t, s, a, b, &[0, 1, 3, 6, 20]);
}

/// [`r24_pat`] with the spreads to draw from.
fn r24_pat_sp(g: &mut Gen, t: &mut Vec<String>, s: Shape, a: &[i64], b: &[i64], sp: &[i64]) {
    let on = !s.hdr && s.nf > 0 && !g.chance(4);
    t.push(i64::from(on).to_string());
    if on {
        t.push(g.int(0, 1000).to_string());
        t.push(g.pick(a).to_string());
        t.push(g.pick(b).to_string());
        t.push(g.pick(sp).to_string());
    }
}

/// `get_histo`'s tokens: -1 for `NULL`, else its allocation, the bins in use and a seed.
fn r24_histo_toks(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(2) {
        t.push("-1".into());
        return;
    }
    let k = g.pick(&[0i64, 1, 3, 10, 40, 300]);
    t.push(k.to_string());
    t.push(g.int(0, k).to_string());
    t.push(g.int(0, 9).to_string());
}

/// A region of the cases: x y z dx dy dz around a volume of shape `s`.
fn r24_region(g: &mut Gen, t: &mut Vec<String>, s: Shape) {
    let whole = g.chance(3);
    for e in [s.w, s.h, s.d] {
        t.push(if whole { 0 } else { g.int(-2, e) }.to_string());
    }
    for e in [s.w, s.h, s.d] {
        t.push(if whole { e } else { g.int(-1, e + 2) }.to_string());
    }
}

/// `mrihisto.cpp`'s cases (relay 24): see `fsvolume.cpp`'s `hreg` ... `hsegm`.
fn r24_gen_histo(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let mut s = shape(g);
    s.hdr = s.hdr && g.chance(4);
    // types whose voxel values stay small (an RGB or complex volume's values can span 2^30:
    // a histogram of one bin per intensity takes gigabytes on both sides)
    if !matches!(s.ty, 0 | 1 | 2 | 3 | 4 | 10) {
        s.ty = g.pick(&[0, 3, 4, 1]);
    }
    let big = cmd == "hsegm" || g.chance(3);
    if big {
        (s.w, s.h, s.d) = (g.int(3, 8), g.int(3, 8), g.int(2, 7));
        if g.chance(2) {
            s.nf = 1;
        }
    }
    if cmd == "hsegm" && !g.chance(6) {
        s.ty = 0;
    }
    vol_of(g, t, s);
    let vals: &[f32] = &[0.0, 1.0, 2.5, 100.0, 255.0, 7.25, -3.0, 128.0, 4096.0];
    r24_fill_paint(g, t, s, &[&[0.0, 10.0, 100.0]], &[vals]);
    let (a, b): (&[i64], &[i64]) = if cmd == "hsegm" {
        (&[95, 100, 110, 128, 115], &[60, 70, 75, 80, 128])
    } else {
        (&[100, 0, 50, 255, 300, -20], &[10, 0, 128, 1000, 30])
    };
    // the equalisations index their histograms by intensity: mostly volumes from 0 to 255
    let eq = (cmd == "geq" || cmd == "heq") && !g.chance(4);
    let (a, b, sp): (&[i64], &[i64], &[i64]) = if eq {
        (&[255, 200, 100], &[0], &[0, 0, 2])
    } else {
        (a, b, &[0, 1, 3, 6, 20])
    };
    r24_pat_sp(g, t, s, a, b, sp);
    match cmd {
        "hreg" | "hvox" | "hthr" => {
            let nb: &[i64] = if cmd == "hthr" {
                &[0, 0, 5, 10, 1, 64]
            } else {
                &[0, 0, -1, 5, 1, 256, 3]
            };
            t.push(g.pick(nb).to_string());
            r24_histo_toks(g, t);
            if cmd == "hvox" {
                for e in [s.w, s.h, s.d] {
                    t.push(g.int(-1, e).to_string());
                }
                t.push(g.pick(&[1i64, 3, 5, 0, 2, 4]).to_string());
                let th = g.chance(2);
                t.push(i64::from(th).to_string());
                if th {
                    let mut u = r24_like(g, s);
                    u.hdr = u.hdr && g.chance(4);
                    vol_of(g, t, u);
                    r24_fill_paint(g, t, u, &[&[0.0, 1.0, 50.0]], &[vals]);
                    r24_pat(g, t, u, &[0, 50, 100], &[20, 80]);
                }
                t.push(fx(g.pick(&[0.0f32, 1.0, 50.0, -1.0, 100.0])));
            } else {
                r24_region(g, t, s);
                if cmd == "hthr" {
                    let mut u = r24_like(g, s);
                    u.hdr = u.hdr && g.chance(4);
                    vol_of(g, t, u);
                    r24_fill_paint(g, t, u, &[&[0.0, 1.0, 50.0]], &[vals]);
                    r24_pat(g, t, u, &[0, 50, 100], &[20, 80]);
                    t.push(fx(g.pick(&[0.0f32, 1.0, 50.0, -1.0, 100.0])));
                    t.push(frame_of(g, s).to_string());
                }
            }
        }
        "hist" => t.push(g.pick(&[0i64, 0, 5, 1, 256, 2]).to_string()),
        "geq" => {
            r24_histo_toks(g, t);
            t.push(g.pick(&[0i64, 0, 0, 1, 5, -1, 300]).to_string());
            t.push(g.pick(&[255i64, 255, 10, 50, 0, 1000, -2, 100]).to_string());
            t.push(g.pick(&[1i64, 1, 0]).to_string());
        }
        "heq" => {
            let mut u = r24_like(g, s);
            u.hdr = u.hdr && g.chance(4);
            vol_of(g, t, u);
            r24_fill_paint(g, t, u, &[&[0.0, 10.0, 100.0]], &[vals]);
            r24_pat_sp(g, t, u, a, b, sp);
            let mode = g.pick(&[0, 0, 1, 2]);
            t.push(mode.to_string());
            if mode == 1 {
                let mut o = r24_like(g, s);
                o.hdr = o.hdr && g.chance(4);
                vol_of(g, t, o);
            }
            t.push(g.pick(&[0i64, 0, 0, 1, 5, -1, 300]).to_string());
            t.push(g.pick(&[255i64, 255, 10, 50, 0, 1000, 100]).to_string());
        }
        "hlr" | "hlab" => {
            let mut u = r24_like(g, s);
            u.hdr = u.hdr && g.chance(6);
            vol_of(g, t, u);
            let labels: &[f32] = &[0.0, 1.0, 2.0, 3.0, 2.5, -1.0];
            r24_fill_paint(g, t, u, &[&[0.0, 1.0, 2.0]], &[labels]);
            t.push(g.pick(&[0i64, 1, 2, 3, -1]).to_string());
            t.push(g.pick(&[0i64, 0, -1, 5, 1, 256]).to_string());
            if cmd == "hlr" {
                r24_region(g, t, s);
            }
        }
        _ => {
            let mode = g.pick(&[0, 1, 1, 1, 2]);
            t.push(mode.to_string());
            if mode == 1 {
                let mut o = r24_like(g, s);
                o.hdr = o.hdr && g.chance(6);
                vol_of(g, t, o);
                let l: &[f32] = &[128.0, 0.0, 255.0, 1.0];
                r24_fill_paint(g, t, o, &[&[128.0, 128.0, 0.0]], &[l]);
            }
            t.push(g.pick(&[90i64, 85, 100, 0, 60]).to_string());
            t.push(g.pick(&[120i64, 130, 255, 50, 110]).to_string());
            let gh = if g.chance(30) {
                -5
            } else {
                g.pick(&[100i64, 95, 110, -1, 90, 80])
            };
            t.push(gh.to_string());
            t.push(g.pick(&[5i64, 7, 3, 1, 0, 9, 5]).to_string());
            t.push(fx(g.pick(&[-1.0f32, 4.0, 2.0, 0.5, 0.0])));
            t.push(i64::from(g.chance(4)).to_string());
        }
    }
}

/// `fsvolume.cpp`'s `fill_pat`.
fn r24_fill_pat(cx: &mut Ctx, t: &mut Toks, m: &mut Mri) -> Result<(), Fatal> {
    use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
    if t.i() == 0 {
        return Ok(());
    }
    let seed = t.i() as u32;
    let (a, b, sp) = (t.i(), t.i(), t.i());
    for f in 0..m.nframes {
        for z in 0..m.vg.depth {
            for y in 0..m.vg.height {
                for x in 0..m.vg.width {
                    let mut h = (x as u32).wrapping_mul(73856093)
                        ^ (y as u32).wrapping_mul(19349663)
                        ^ (z as u32).wrapping_mul(83492791)
                        ^ (f as u32).wrapping_mul(2654435761)
                        ^ seed.wrapping_mul(97);
                    h = h.wrapping_mul(1103515245).wrapping_add(12345);
                    let base = if (h >> 8) & 1 != 0 { a } else { b };
                    let v = base + ((h >> 9) % (2 * sp + 1) as u32) as i32 - sp;
                    mri_set_vox_val(&mut cx.g, &mut cx.b, m, x, y, z, f, v as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}

fn r24_get_histo(t: &mut Toks) -> Option<larmorx_freesurfer_core::histo::Histogram> {
    let k = t.i();
    if k < 0 {
        return None;
    }
    let (nb, seed) = (t.i(), t.i());
    let n = k as usize;
    Some(larmorx_freesurfer_core::histo::Histogram {
        max_bins: k,
        nbins: nb,
        bins: (0..n).map(|i| 0.5 * i as f32 + seed as f32).collect(),
        counts: (0..n).map(|i| ((i as i32 * 7 + seed) % 5) as f32).collect(),
        bin_size: 0.0,
        min: 0.0,
        max: 0.0,
    })
}

fn r24_put_histo(o: &mut Out, h: Option<&larmorx_freesurfer_core::histo::Histogram>) {
    let Some(h) = h else {
        o.0 += "NULL ";
        return;
    };
    o.i(i64::from(h.nbins));
    o.i(i64::from(h.max_bins));
    o.f(h.bin_size);
    o.f(h.min);
    o.f(h.max);
    let n = h.nbins.min(h.max_bins).max(0) as usize;
    for i in 0..n {
        o.f(h.bins[i]);
        o.f(h.counts[i]);
    }
}

/// A histogram result: whether it is the given one, then it.
fn r24_ret(
    o: &mut Out,
    r: &Ret<larmorx_freesurfer_core::histo::Histogram>,
    given: Option<&larmorx_freesurfer_core::histo::Histogram>,
) {
    o.i(i64::from(matches!(r, Ret::Given)));
    match r {
        Ret::New(h) => r24_put_histo(o, Some(h)),
        Ret::Given => r24_put_histo(o, given),
        Ret::Null => r24_put_histo(o, None),
    }
}

fn r24_region_of(t: &mut Toks) -> MriRegion {
    MriRegion {
        x: t.i(),
        y: t.i(),
        z: t.i(),
        dx: t.i(),
        dy: t.i(),
        dz: t.i(),
    }
}

fn r24_vol(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let mut m = get_vol(cx, t)?;
    fill_painted(cx, t, &mut m)?;
    r24_fill_pat(cx, t, &mut m)?;
    Ok(m)
}

#[inline(never)]
fn r24_histo(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    use larmorx_freesurfer_volume::mrihisto as mh;
    match cmd {
        "hreg" | "hvox" | "hthr" => {
            let m = r24_vol(cx, t)?;
            let nbins = t.i();
            let mut h = r24_get_histo(t);
            let r = if cmd == "hvox" {
                let (x, y, z, ws) = (t.i(), t.i(), t.i(), t.i());
                let th_vol = if t.i() != 0 {
                    Some(r24_vol(cx, t)?)
                } else {
                    None
                };
                let th = t.f();
                let (g, b) = (&mut cx.g, &mut cx.b);
                mh::mri_histogram_voxel(
                    g,
                    b,
                    &m,
                    nbins,
                    h.as_mut(),
                    x,
                    y,
                    z,
                    ws,
                    th_vol.as_ref(),
                    th,
                )?
            } else {
                let reg = r24_region_of(t);
                if cmd == "hreg" {
                    let (g, b) = (&mut cx.g, &mut cx.b);
                    mh::mri_histogram_region(g, b, &m, nbins, h.as_mut(), &reg)?
                } else {
                    let tv = r24_vol(cx, t)?;
                    let (th, fr) = (t.f(), t.i());
                    let (g, b) = (&mut cx.g, &mut cx.b);
                    mh::mri_histogram_region_with_threshold(
                        g,
                        b,
                        &m,
                        nbins,
                        h.as_mut(),
                        &reg,
                        &tv,
                        th,
                        fr,
                    )?
                }
            };
            r24_ret(o, &r, h.as_ref());
        }
        "hist" => {
            let m = r24_vol(cx, t)?;
            let nb = t.i();
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = mh::mri_histogram(g, b, &m, nb)?;
            r24_put_histo(o, Some(&r));
        }
        "geq" => {
            let m = r24_vol(cx, t)?;
            let mut h = r24_get_histo(t);
            let (lo, hi, norm) = (t.i(), t.i(), t.i());
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = mh::mri_get_equalize_histo(g, b, &m, h.as_mut(), lo, hi, norm)?;
            r24_ret(o, &r, h.as_ref());
        }
        "heq" => {
            let mut src = r24_vol(cx, t)?;
            let tpl = r24_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            let (lo, hi) = (t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = mh::mri_histo_equalize(g, a, b, p, &tpl, lo, hi)?;
            let given = match mode {
                1 => dst.as_ref(),
                2 => Some(&src),
                _ => None,
            };
            o.ret(&r, given);
        }
        "hlr" | "hlab" => {
            let m = r24_vol(cx, t)?;
            let mut l = get_vol(cx, t)?;
            fill_painted(cx, t, &mut l)?;
            let (label, nb) = (t.i(), t.i());
            let r = if cmd == "hlr" {
                let reg = r24_region_of(t);
                let (g, b) = (&mut cx.g, &mut cx.b);
                mh::mri_histogram_label_region(g, b, &m, &l, &reg, label, nb)?
            } else {
                let (g, b) = (&mut cx.g, &mut cx.b);
                Some(mh::mri_histogram_label(g, b, &m, &l, label, nb)?)
            };
            r24_put_histo(o, r.as_ref());
        }
        _ => {
            let mut src = r24_vol(cx, t)?;
            let (mode, mut dst) = dst_mode(cx, t)?;
            if let Some(d) = dst.as_mut() {
                fill_painted(cx, t, d)?;
            }
            let (wl, wh, gh, ws) = (t.i(), t.i(), t.i(), t.i());
            let sg = t.f();
            if t.i() != 0 {
                cx.g.gdiag |= larmorx_freesurfer_core::diag::DIAG_SHOW;
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = pair(mode, &mut src, &mut dst);
            let r = mh::mri_histo_segment(g, a, b, p, wl, wh, gh, ws, sg)?;
            let given = match mode {
                1 => dst.as_ref(),
                2 => Some(&src),
                _ => None,
            };
            o.ret(&r, given);
        }
    }
    Ok(())
}

/// [`fill_paint`] whose odd values keep a histogram's range small (FreeSurfer allocates one bin per
/// intensity: a range of 2^30 would take gigabytes on both sides): NaN, infinities, 4096.5.
fn r24_fill_paint(g: &mut Gen, t: &mut Vec<String>, s: Shape, fills: &[&[f32]], vals: &[&[f32]]) {
    let has = !s.hdr && s.nf > 0;
    let k = if has && !g.chance(5) {
        g.int(1, s.nf)
    } else {
        0
    };
    t.push(k.to_string());
    for f in 0..k {
        let l = fills[(f as usize).min(fills.len() - 1)];
        t.push(fx(g.pick(l)));
    }
    let n = if has {
        g.int(0, (s.w * s.h * s.d).min(24))
    } else {
        0
    };
    t.push(n.to_string());
    for _ in 0..n {
        t.push(g.int(0, s.w - 1).to_string());
        t.push(g.int(0, s.h - 1).to_string());
        t.push(g.int(0, s.d - 1).to_string());
        let f = if g.chance(4) { g.int(0, s.nf - 1) } else { 0 };
        t.push(f.to_string());
        let l = vals[(f as usize).min(vals.len() - 1)];
        let odd = [
            f32::from_bits(0x7fc0_0000),
            f32::from_bits(0x7fa1_2345),
            f32::INFINITY,
            f32::NEG_INFINITY,
            4096.5,
        ];
        t.push(fx(if g.chance(12) {
            g.pick(&odd)
        } else {
            g.pick(l)
        }));
    }
}

/// [`like`] with the small-valued types of [`r24_gen_histo`].
fn r24_like(g: &mut Gen, s: Shape) -> Shape {
    let mut o = like(g, s);
    if !matches!(o.ty, 0 | 1 | 2 | 3 | 4 | 10) {
        o.ty = g.pick(&[0, 3, 4, 1]);
    }
    o
}

/// transform.cpp's operations on an LTA (relay 25).
const R25_LTA: [&str; 7] = [
    "ltafill", "ltainv", "ltachg", "ltacat", "ltared", "ltasrc", "ltaprt",
];

/// A pool of geometries for one case's transforms (`set_geom`'s tokens): scanner-like ones of
/// small or real extents, one sometimes a slight change of another (`vg_isEqual`'s threshold), one
/// sometimes not valid.
fn r25_pool(g: &mut Gen) -> Vec<Vec<String>> {
    let mut pool: Vec<Vec<String>> = Vec::new();
    for k in 0..4 {
        let ext = |g: &mut Gen| {
            if g.chance(6) {
                g.pick(&[256, 128, 0, -1])
            } else {
                g.int(1, 6)
            }
        };
        let s = Shape {
            w: ext(g),
            h: ext(g),
            d: ext(g),
            ty: 0,
            nf: 1,
            hdr: true,
            drop: false,
        };
        let mut v = real_geom(g, s, false);
        if k > 0 && g.chance(8) {
            // a slight change of the previous geometry
            v = pool[k - 1].clone();
            let i = g.pick(&[4usize, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]);
            let x = f32::from_bits(u32::from_str_radix(&v[i], 16).unwrap());
            v[i] = fx(x + g.pick(&[1e-7f32, 1e-5, 1e-3]));
        }
        if g.chance(12) {
            v[0] = g.pick(&["0", "2"]).to_owned();
        }
        pool.push(v);
    }
    pool
}

/// A transform matrix's 16 tokens: mostly a rotation-like part and a translation, sometimes any
/// numbers (singular ones included).
fn r25_m44(g: &mut Gen, t: &mut Vec<String>) {
    let like = !g.chance(6);
    let zero = g.chance(25);
    for r in 0..4 {
        for c in 0..4 {
            let v = if zero {
                0.0
            } else if like {
                if r == 3 {
                    if c == 3 { 1.0 } else { 0.0 }
                } else if c == 3 {
                    num(g, -64, 64, 8)
                } else if r == c {
                    g.pick(&[1.0f32, 1.0, -1.0, 0.9, 1.1, 2.0, 0.5])
                } else {
                    g.pick(&[0.0f32, 0.0, 0.0, 0.1, -0.1, 0.25, 0.6])
                }
            } else {
                num(g, -4, 4, 8)
            };
            t.push(fx(v));
        }
    }
}

const R25_TYPES: [i64; 9] = [0, 1, 2, 14, 15, 21, 1, 1, 0];

/// `get_lta_full`'s tokens: `n` transforms along the pool's geometries `chain[0] -> chain[1] ->
/// ...` (each transform's source and destination; `chain` has `n + 1` entries).
fn r25_lta_toks(g: &mut Gen, t: &mut Vec<String>, pool: &[Vec<String>], chain: &[usize], ty: i64) {
    let n = chain.len() - 1;
    t.push(n.to_string());
    t.push(ty.to_string());
    t.push(fx(g.pick(&[0.0f32, 0.0, 0.1, 1.0, -1.0, f32::NAN])));
    t.push(g.pick(&["-", "-", "62657274", "7375622d3031"]).to_owned());
    for i in 0..n {
        for _ in 0..3 {
            t.push(fx(num(g, -10, 10, 2)));
        }
        t.push(fx(g.pick(&[10000.0f32, 1.0, 0.5])));
        t.push(g.pick(&[-1i64, 0, 1, 14]).to_string());
        t.push(g.pick(&[0i64, 0, 3, -1]).to_string());
        r25_m44(g, t);
        t.extend(pool[chain[i]].iter().cloned());
        t.extend(pool[chain[i + 1]].iter().cloned());
        if g.chance(4) {
            t.push("1".into());
            r25_m44(g, t);
            t.extend(pool[g.below(pool.len() as u64) as usize].iter().cloned());
            t.extend(pool[g.below(pool.len() as u64) as usize].iter().cloned());
        } else {
            t.push("0".into());
        }
    }
}

/// [`r25_chain`] from a random start.
fn r25_chain_any(g: &mut Gen, n: usize, np: usize) -> Vec<usize> {
    let start = g.below(np as u64) as usize;
    r25_chain(g, start, n, np)
}

/// A chain of `n + 1` pool indices (`n` transforms), from `start`.
fn r25_chain(g: &mut Gen, start: usize, n: usize, np: usize) -> Vec<usize> {
    let mut c = vec![start];
    for _ in 0..n {
        c.push(g.below(np as u64) as usize);
    }
    c
}

fn r25_n(g: &mut Gen) -> usize {
    if g.chance(30) {
        0
    } else {
        g.pick(&[1, 1, 1, 2, 2, 3])
    }
}

fn r25_gen_lta(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let pool = r25_pool(g);
    let np = pool.len();
    let ty = g.pick(&R25_TYPES);
    match cmd {
        "ltafill" | "ltared" | "ltaprt" => {
            let n = r25_n(g);
            let start = g.below(np as u64) as usize;
            let mut chain = r25_chain(g, start, n, np);
            if cmd == "ltared" && n >= 2 && g.chance(3) {
                // a link that matches only inverted: transform i's destination is the previous one's
                let i = g.int(1, n as i64 - 1) as usize;
                chain.swap(i, i + 1);
            }
            r25_lta_toks(g, t, &pool, &chain, ty);
        }
        "ltainv" => {
            let n = r25_n(g);
            let chain = r25_chain_any(g, n, np);
            r25_lta_toks(g, t, &pool, &chain, ty);
            let mode = g.int(0, 2);
            t.push(mode.to_string());
            if mode == 1 {
                let m = if g.chance(6) { r25_n(g) } else { n };
                let chain = r25_chain_any(g, m, np);
                let ty2 = g.pick(&R25_TYPES);
                r25_lta_toks(g, t, &pool, &chain, ty2);
            }
        }
        "ltachg" => {
            t.push(g.pick(&[1i64, 1, 1, 2, 3, 4, 0]).to_string());
            t.push(i64::from(g.chance(8)).to_string());
            let n = if g.chance(20) {
                0
            } else {
                g.pick(&[1, 1, 1, 2, 3])
            };
            let chain = r25_chain_any(g, n, np);
            r25_lta_toks(g, t, &pool, &chain, ty);
            let to = if g.chance(10) {
                g.pick(&[10i64, 12, 13, -1, 21, 3])
            } else {
                g.pick(&[0i64, 1, 2, 14, 15])
            };
            t.push(to.to_string());
        }
        "ltacat" => {
            t.push(g.pick(&[1i64, 1, 1, 2, 0]).to_string());
            let k = if g.chance(25) {
                0
            } else {
                g.pick(&[1usize, 2, 2, 2, 3])
            };
            t.push(k.to_string());
            let mut end = g.below(np as u64) as usize;
            for j in 0..k {
                let n = r25_n(g);
                let link = g.below(6);
                let chain = if j > 0 && link == 0 {
                    // ends where the previous one ended: matches inverted
                    let mut c = r25_chain_any(g, n, np);
                    if let Some(l) = c.last_mut() {
                        *l = end;
                    }
                    c
                } else if j > 0 && link == 1 {
                    r25_chain_any(g, n, np)
                } else {
                    r25_chain(g, end, n, np)
                };
                end = *chain.last().unwrap();
                let tyj = if j == 0 || g.chance(3) {
                    ty
                } else {
                    g.pick(&R25_TYPES)
                };
                r25_lta_toks(g, t, &pool, &chain, tyj);
            }
            t.push(g.int(0, 1).to_string());
        }
        _ => {
            // ltasrc: the LTA, then a volume header whose geometry is one of the pool's
            let n = if g.chance(20) { 0 } else { g.pick(&[1, 1, 2]) };
            let chain = r25_chain_any(g, n, np);
            r25_lta_toks(g, t, &pool, &chain, ty);
            let gi = if g.chance(4) {
                g.below(np as u64) as usize
            } else if g.chance(2) {
                chain[0]
            } else {
                *chain.last().unwrap()
            };
            let geo = &pool[gi];
            let ext = |k: usize| geo[k].parse::<i64>().unwrap().clamp(1, 6);
            for v in [ext(1), ext(2), ext(3), 0, 1, 1, g.int(0, 255)] {
                t.push(v.to_string());
            }
            let mut geo = geo.clone();
            // the volume's extent is its own: the pool's when it is small
            for k in 1..=3 {
                geo[k] = ext(k).to_string();
            }
            t.extend(geo);
            t.push("0".into());
            t.push("0".into());
            for _ in 0..6 {
                t.push(fx(0.0));
            }
            t.push(fx(1.0));
            t.push(fx(1.0));
            for _ in 0..3 {
                t.push(fx(0.0));
            }
            t.push(dx(0.0));
            t.push("0".into());
            t.push("0".into());
            t.push(g.int(0, 1).to_string());
        }
    }
}

/// `fsvolume.cpp`'s `get_lta_full`.
fn r25_get_lta(cx: &mut Ctx, t: &mut Toks) -> Result<Lta, Fatal> {
    let n = t.i();
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let mut l = larmorx_freesurfer_mri::lta::lta_alloc(g, a, b, n, None)?;
    l.ltype = t.i();
    l.fscale = t.f();
    l.subject = CName::new(&t.hex());
    for i in 0..n.max(0) as usize {
        let lt = &mut l.xforms[i];
        lt.x0 = t.f();
        lt.y0 = t.f();
        lt.z0 = t.f();
        lt.sigma = t.f();
        lt.ltype = t.i();
        lt.label = t.i();
        let m = lt.m_l.as_mut().expect("LTAalloc's matrix");
        for r in 1..=4 {
            for c in 1..=4 {
                m.set_relt(r, c, t.f());
            }
        }
        set_geom(t, &mut lt.src);
        set_geom(t, &mut lt.dst);
        if t.i() != 0 {
            let iv = &mut l.inv_xforms[i];
            let m = iv.m_l.as_mut().expect("LTAalloc's matrix");
            for r in 1..=4 {
                for c in 1..=4 {
                    m.set_relt(r, c, t.f());
                }
            }
            set_geom(t, &mut iv.src);
            set_geom(t, &mut iv.dst);
        }
    }
    Ok(l)
}

impl Out {
    fn lt(&mut self, lt: &larmorx_freesurfer_mri::lta::LinearTransform) {
        for v in [lt.x0, lt.y0, lt.z0, lt.sigma] {
            self.f(v);
        }
        self.i(i64::from(lt.ltype));
        self.i(i64::from(lt.label));
        self.m(lt.m_l.as_ref());
        self.geom(&lt.src);
        self.geom(&lt.dst);
    }
    /// `fsvolume.cpp`'s `put_lta_full`.
    fn lta_full(&mut self, l: Option<&Lta>) {
        let Some(l) = l else {
            self.0 += "NULL ";
            return;
        };
        self.i(i64::from(l.ltype));
        self.i(i64::from(l.num_xforms));
        self.f(l.fscale);
        self.hex(l.subject.c_bytes());
        for i in 0..l.num_xforms.max(0) as usize {
            self.lt(&l.xforms[i]);
            self.lt(&l.inv_xforms[i]);
        }
    }
}

/// `fsvolume.cpp`'s `set_fsltype`.
fn r25_fsl_env(t: &mut Toks) -> Environment {
    match t.i() {
        1 => Environment::from_pairs([("FSLOUTPUTTYPE", "NIFTI_GZ")]),
        2 => Environment::from_pairs([("FSLOUTPUTTYPE", "NIFTI")]),
        3 => Environment::from_pairs([("FSLOUTPUTTYPE", "ANALYZE")]),
        4 => Environment::from_pairs([("FSLOUTPUTTYPE", "NIFTI_PAIR")]),
        _ => Environment::default(),
    }
}

#[inline(never)]
fn r25_lta(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    use larmorx_freesurfer_volume::ltaops as lo;
    match cmd {
        "ltafill" => {
            let mut l = r25_get_lta(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            lo::lta_fill_inverse(g, a, b, &mut l)?;
            o.lta_full(Some(&l));
        }
        "ltainv" => {
            let mut l = r25_get_lta(cx, t)?;
            let mode = t.i();
            let mut d = if mode == 1 {
                Some(r25_get_lta(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match mode {
                1 => {
                    let d = d.as_mut().expect("a destination");
                    let r = lo::lta_invert(g, a, b, lo::InvertInto::Given(&l, d))?;
                    o.i(i64::from(matches!(r, Ret::Given)));
                    o.lta_full(if r.is_null() { None } else { Some(d) });
                    o.lta_full(Some(&l));
                }
                2 => {
                    let r = lo::lta_invert(g, a, b, lo::InvertInto::Same(&mut l))?;
                    o.i(i64::from(matches!(r, Ret::Given)));
                    o.lta_full(if r.is_null() { None } else { Some(&l) });
                }
                _ => {
                    let r = lo::lta_invert(g, a, b, lo::InvertInto::New(&l))?;
                    o.i(0);
                    let n = r.into_new();
                    o.lta_full(n.as_ref());
                    o.lta_full(Some(&l));
                }
            }
        }
        "ltachg" => {
            let env = r25_fsl_env(t);
            if t.i() != 0 {
                cx.g.gdiag_no = 1;
            }
            let mut l = r25_get_lta(cx, t)?;
            let ty = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            lo::lta_change_type(g, a, b, &env, &mut l, ty)?;
            o.i(1);
            o.lta_full(Some(&l));
        }
        "ltacat" => {
            let env = r25_fsl_env(t);
            let n = t.i();
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(r25_get_lta(cx, t)?);
            }
            let red = t.i() != 0;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let mut refs: Vec<&mut Lta> = v.iter_mut().collect();
            let r = lo::lta_concat(g, a, b, &env, &mut refs, red)?;
            o.lta_full(r.as_ref());
            for l in &v {
                o.lta_full(Some(l));
            }
        }
        "ltared" => {
            let l = r25_get_lta(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = lo::lta_reduce(g, a, b, &l)?;
            o.lta_full(r.as_ref());
            o.lta_full(Some(&l));
        }
        "ltasrc" => {
            let l = r25_get_lta(cx, t)?;
            let m = get_vol(cx, t)?;
            let tgt = t.i() != 0;
            let (g, b) = (&mut cx.g, &mut cx.b);
            let r = if tgt {
                lo::lta_mri_is_target(g, b, &l, &m)?
            } else {
                lo::lta_mri_is_source(g, b, &l, &m)?
            };
            o.i(i64::from(r));
        }
        _ => {
            let l = r25_get_lta(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            lo::lta_print(g, b, a, &l)?;
        }
    }
    Ok(())
}

/// resample.cpp's volume functions (relay 25).
const R25_RES: [&str; 4] = ["a2v", "a2vmu", "spmw", "icode"];

/// `get_vol`'s tokens for a volume of shape `s` with the geometry tokens `geo` (its extent
/// replaced by the volume's) and plain header fields.
fn r25_vol_geo(g: &mut Gen, t: &mut Vec<String>, s: Shape, geo: &[String]) {
    for v in [s.w, s.h, s.d, s.ty, s.nf, i64::from(s.hdr), g.int(0, 255)] {
        t.push(v.to_string());
    }
    let mut geo = geo.to_vec();
    geo[1] = s.w.to_string();
    geo[2] = s.h.to_string();
    geo[3] = s.d.to_string();
    t.extend(geo);
    t.push("0".into());
    t.push("0".into());
    for _ in 0..6 {
        t.push(fx(0.0));
    }
    t.push(fx(1.0));
    t.push(fx(1.0));
    for _ in 0..3 {
        t.push(fx(g.pick(&[0.0, 2000.0, 30.5])));
    }
    t.push(dx(0.0));
    t.push("0".into());
    t.push("0".into());
}

fn r25_shape(g: &mut Gen, ty: &[i64], nf: i64, max: i64) -> Shape {
    Shape {
        w: g.int(1, max),
        h: g.int(1, max),
        d: g.int(1, max.min(4)),
        ty: g.pick(ty),
        nf,
        hdr: false,
        drop: false,
    }
}

/// `get_ctab`'s tokens: none (-1) now and then, otherwise 50 entries (a few freed) of tissue
/// types mostly 0 to 3, and a tissue-type table of 4 (sometimes none or more than 100).
fn r25_ctab(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(3) {
        t.push("-1".into());
        return;
    }
    let n = if g.chance(10) { 1001 } else { 50 };
    t.push(n.to_string());
    for _ in 0..n {
        let tt = if g.chance(30) {
            g.pick(&[100i64, -2])
        } else {
            g.pick(&[0i64, 1, 2, 3, 1, 2, -1])
        };
        t.push(tt.to_string());
        t.push(i64::from(g.chance(40)).to_string());
    }
    let k = if g.chance(12) {
        g.pick(&[-1i64, 101])
    } else {
        4
    };
    t.push(k.to_string());
}

const R25_LABELS: [f32; 9] = [2.0, 3.0, 4.0, 41.0, 0.0, 17.0, 3.0, 2.0, 1000.0];

fn r25_gen_res(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let pool = r25_pool(g);
    match cmd {
        "a2v" => {
            let s = r25_shape(g, &[0, 1, 3, 1, 1], 1, 6);
            r25_vol_geo(g, t, s, &pool[0]);
            fill_paint(g, t, s, &[&[0.0, 2.0, 3.0, 41.0]], &[&R25_LABELS]);
            if g.chance(25) {
                t.push("0".into());
            } else {
                t.push("1".into());
                r25_m44(g, t);
            }
            let mut s2 = r25_shape(g, &[0, 1, 3], 1, 4);
            s2.hdr = g.chance(4);
            r25_vol_geo(g, t, s2, &pool[1]);
            t.push(dx(g.pick(&[0.0, 0.0, 0.3, 0.5, 0.75, 1.0, f64::NAN])));
            r25_ctab(g, t);
            t.push(g.pick(&[0i64, 0, 0, 0, 1, 2]).to_string());
        }
        "a2vmu" => {
            let s = r25_shape(g, &[0, 1, 3, 1], 1, 5);
            let mut geo = pool[0].clone();
            geo[0] = "1".into();
            for (k, v) in [s.w, s.h, s.d].into_iter().enumerate() {
                geo[k + 1] = v.to_string();
            }
            r25_vol_geo(g, t, s, &geo);
            fill_paint(g, t, s, &[&[0.0, 2.0, 3.0]], &[&R25_LABELS]);
            // the transform between the segmentation and a small volume, in either direction
            let mut pool2 = pool.clone();
            pool2[0] = geo;
            for k in 1..=3 {
                pool2[1][k] = g.int(1, 4).to_string();
            }
            let chain = match g.below(5) {
                0 => vec![1, 0],
                1 => vec![2, 3],
                _ => vec![0, 1],
            };
            let ty = g.pick(&[1i64, 1, 0, 14]);
            r25_lta_toks(g, t, &pool2, &chain, ty);
            t.push(dx(g.pick(&[0.0, 0.3, 0.5, 1.0])));
            t.push(g.pick(&[-1i64, -1, 1, 2]).to_string());
            r25_ctab(g, t);
        }
        "spmw" => {
            let nf = g.pick(&[1i64, 1, 2]);
            let s = r25_shape(g, &[0, 3, 1, 3, 3, 2, 9], nf, 5);
            let mut geo = pool[0].clone();
            for (k, v) in [s.w, s.h, s.d].into_iter().enumerate() {
                geo[k + 1] = v.to_string();
            }
            geo[0] = "1".into();
            r25_vol_geo(g, t, s, &geo);
            fill_paint(g, t, s, &[&[0.0, 7.0, 2.5]], &[&[1.0, 3.0, 100.0, -4.0]]);
            if g.chance(3) {
                t.push("1".into());
                let mut pool2 = pool.clone();
                pool2[0] = geo;
                let chain = if g.chance(2) { vec![0, 1] } else { vec![1, 0] };
                let ty = g.pick(&[0i64, 0, 1]);
                r25_lta_toks(g, t, &pool2, &chain, ty);
            } else {
                t.push("0".into());
            }
            let wnf = if g.chance(12) { 2 } else { 3 };
            let ws = Shape {
                w: g.int(1, 5),
                h: g.int(1, 5),
                d: g.int(1, 5),
                ty: 3,
                nf: wnf,
                hdr: false,
                drop: false,
            };
            r25_vol_geo(g, t, ws, &pool[1]);
            let ras: &[f32] = &[-2.0, -1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0];
            fill_paint(g, t, ws, &[ras, ras, ras], &[ras]);
            t.push(i64::from(g.chance(3)).to_string());
            t.push(if g.chance(15) { 2 } else { g.int(0, 1) }.to_string());
            if g.chance(6) {
                t.push("1".into());
                let o = Shape { ty: 3, nf, ..ws };
                r25_vol_geo(g, t, o, &pool[1]);
            } else {
                t.push("0".into());
            }
        }
        _ => {
            let names: &[&[u8]] = &[
                b"nearest",
                b"NEA",
                b"trilinear",
                b"tli",
                b"sinc",
                b"cubic",
                b"bspline",
                b"vote",
                b"round",
                b"RINT",
                b"floor",
                b"tkregister",
                b"TKREG",
                b"tkr",
                b"ne",
                b"",
                b"cub",
                b"interpolate",
                b"weighted",
            ];
            t.push(hex(g.pick(names)));
        }
    }
}

/// `fsvolume.cpp`'s `get_ctab`.
fn r25_get_ctab(t: &mut Toks) -> Option<larmorx_freesurfer_mri::ctab::ColorTable> {
    use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
    let n = t.i();
    if n < 0 {
        return None;
    }
    let mut ct = ColorTable::default();
    for i in 0..n {
        let tt = t.i();
        let freed = t.i() != 0;
        ct.entries.push(if freed {
            None
        } else {
            Some(Box::new(ColorTableEntry {
                name: format!("s{i}").into_bytes(),
                tissue_type: tt,
                ..ColorTableEntry::default()
            }))
        });
    }
    let k = t.i();
    if k >= 0 {
        let mut tt = ColorTable::default();
        tt.entries = (0..k).map(|_| Some(Box::default())).collect();
        ct.ctab_tissue_type = Some(Box::new(tt));
    }
    Some(ct)
}

#[inline(never)]
fn r25_res(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    use larmorx_freesurfer_volume::aseg2vol as av;
    match cmd {
        "a2v" => {
            let mut aseg = get_vol(cx, t)?;
            fill_painted(cx, t, &mut aseg)?;
            let r = get_m44(t);
            let tmp = get_vol(cx, t)?;
            let ft = t.d();
            let ct = r25_get_ctab(t);
            let gd = t.i();
            if gd != 0 {
                cx.g.gdiag_no = gd;
            }
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let res = av::mri_aseg2vol(g, a, b, &aseg, r.as_ref(), &tmp, ft, ct.as_ref())?;
            match res {
                Some((v, h)) => {
                    o.vol(Some(&v));
                    o.vol(Some(&h));
                }
                None => {
                    o.vol(None);
                    o.vol(None);
                }
            }
        }
        "a2vmu" => {
            let mut aseg = get_vol(cx, t)?;
            fill_painted(cx, t, &mut aseg)?;
            let mut l = r25_get_lta(cx, t)?;
            let ft = t.d();
            let usf = t.i();
            let ct = r25_get_ctab(t);
            let env = Environment::default();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let res = av::mri_aseg2vol_mu(g, a, b, &env, &mut aseg, &mut l, ft, usf, ct.as_ref())?;
            match res {
                Some((v, h)) => {
                    o.vol(Some(&v));
                    o.vol(Some(&h));
                }
                None => {
                    o.vol(None);
                    o.vol(None);
                }
            }
            o.lta_full(Some(&l));
        }
        "spmw" => {
            let mut vol = get_vol(cx, t)?;
            fill_painted(cx, t, &mut vol)?;
            let mut l = if t.i() != 0 {
                Some(r25_get_lta(cx, t)?)
            } else {
                None
            };
            let mut warp = get_vol(cx, t)?;
            fill_painted(cx, t, &mut warp)?;
            let (lr, interp) = (t.i() != 0, t.i());
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let env = Environment::default();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = av::mri_apply_spm_warp(
                g,
                a,
                b,
                &env,
                &vol,
                l.as_mut(),
                &warp,
                lr,
                interp,
                d.as_mut(),
            )?;
            o.i(i64::from(matches!(r, Ret::Given)));
            match r {
                Ret::New(m) => o.vol(Some(&m)),
                Ret::Given => o.vol(d.as_ref()),
                Ret::Null => o.vol(None),
            }
            o.lta_full(l.as_ref());
        }
        _ => {
            let s = t.hex();
            let name = String::from_utf8_lossy(&s).into_owned();
            o.i(i64::from(larmorx_freesurfer_mri::mri::mri_interp_code(
                &name,
            )));
            o.i(i64::from(
                larmorx_freesurfer_io::registerio::float2int_code(&s),
            ));
        }
    }
    Ok(())
}

/// mri2.cpp's functions of relay 25 (`vvtk` shares `vvsm`'s tokens).
const R25_MRI2: [&str; 8] = [
    "sbound", "idxno", "ssrob", "sfavg", "zcat", "aedit", "shdim", "sborder",
];

/// A segmentation-like volume of a small shape: labels from a few, painted.
fn r25_segvol(g: &mut Gen, t: &mut Vec<String>, nf: i64) -> Shape {
    let mut s = shape(g);
    if !matches!(s.ty, 0 | 1 | 3 | 4 | 10) {
        s.ty = g.pick(&[0, 1, 3]);
    }
    s.w = g.int(1, 5);
    s.h = g.int(1, 5);
    s.d = g.int(1, 4);
    s.nf = nf;
    s.hdr = s.hdr && g.chance(4);
    vol_of(g, t, s);
    let labs: &[f32] = &[0.0, 2.0, 3.0, 41.0, 2.0, 3.0, 17.0];
    fill_paint(g, t, s, &[labs, &[0.0, 1.0]], &[labs]);
    s
}

fn r25_gen_mri2(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "sbound" => {
            r25_segvol(g, t, 1);
        }
        "sborder" => {
            r25_segvol(g, t, 1);
            t.push(g.pick(&[1i64, 1, 2, 3, 0, 4, -1]).to_string());
        }
        "idxno" => {
            let s = r25_segvol(g, t, 1);
            let given = g.chance(4);
            t.push(i64::from(given).to_string());
            if given {
                let o = like(g, s);
                vol_of(g, t, o);
            }
        }
        "ssrob" | "sfavg" => {
            let s = r25_segvol(g, t, 1);
            let mut o = like(g, s);
            o.nf = g.int(1, 3);
            vol_of(g, t, o);
            let vals: &[f32] = &[0.0, 1.0, 2.5, -3.0, 100.0, 7.0, 0.25];
            fill_paint(g, t, o, &[vals, vals, vals], &[vals]);
            t.push(g.pick(&[2i64, 3, 41, 0, 17, 99]).to_string());
            if cmd == "ssrob" {
                t.push(frame_of(g, o).to_string());
                t.push(fx(g.pick(&[0.0f32, 10.0, 25.0, 50.0, 2.5, f32::NAN, 60.0])));
            }
        }
        "zcat" => {
            let nf = g.int(1, 2);
            let s = r25_segvol(g, t, nf);
            let mut o = like(g, s);
            if g.chance(6) {
                o.w = g.int(1, 5);
            }
            o.nf = s.nf;
            o.ty = s.ty;
            vol_of(g, t, o);
            let vals: &[f32] = &[0.0, 1.0, 9.0];
            fill_paint(g, t, o, &[vals, vals], &[vals]);
            t.push(g.pick(&[0i64, 0, 1, 2, -1, 5]).to_string());
            let given = g.chance(6);
            t.push(i64::from(given).to_string());
            if given {
                let mut d = like(g, s);
                d.d = s.d + o.d - g.int(0, 1);
                vol_of(g, t, d);
            }
        }
        "aedit" => {
            let nf = g.int(1, 2);
            let s = r25_segvol(g, t, nf);
            for _ in 0..2 {
                let o = Shape { hdr: false, ..s };
                vol_of(g, t, o);
                let vals: &[f32] = &[0.0, 2.0, 3.0, 255.0];
                fill_paint(g, t, o, &[vals, vals], &[vals]);
            }
            let mode = g.int(0, 2);
            t.push(mode.to_string());
            if mode == 1 {
                let o = Shape { hdr: false, ..s };
                vol_of(g, t, o);
            }
        }
        _ => {
            // shdim
            let nf = g.int(1, 2);
            r25_segvol(g, t, nf);
            t.push(g.pick(&[1i64, 2, 3, 2, 3, 0, 4]).to_string());
            t.push(g.pick(&[1i64, -1, 2, -3, 0, 5, -6]).to_string());
            t.push(g.int(0, 1).to_string());
        }
    }
}

#[inline(never)]
fn r25_mri2(cx: &mut Ctx, t: &mut Toks, o: &mut Out, cmd: &str) -> Result<(), Fatal> {
    use larmorx_freesurfer_volume::mri2ops as m2;
    let mut seg = get_vol(cx, t)?;
    fill_painted(cx, t, &mut seg)?;
    match cmd {
        "sbound" => {
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = sg::mri_seg_boundary(g, a, b, &seg)?;
            o.vol(Some(&r));
        }
        "sborder" => {
            let topo = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let (r, ml) = sg::mri_seg_border(g, a, b, &seg, topo)?;
            o.i(i64::from(ml));
            o.vol(Some(&r));
        }
        "idxno" => {
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = m2::mri_index_no(g, a, b, &seg, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "ssrob" | "sfavg" => {
            let mut m = get_vol(cx, t)?;
            fill_painted(cx, t, &mut m)?;
            let segid = t.i();
            if cmd == "ssrob" {
                let (frame, pct) = (t.i(), t.f());
                let (g, b) = (&mut cx.g, &mut cx.b);
                let (n, st) = sg::mri_seg_stats_robust(g, b, &seg, segid, &m, frame, pct)?;
                o.i(i64::from(n));
                for v in [st.min, st.max, st.range, st.mean, st.std] {
                    o.f(v);
                }
            } else {
                let (g, b) = (&mut cx.g, &mut cx.b);
                let (n, favg) = sg::mri_seg_frame_avg(g, b, &seg, segid, &m)?;
                o.i(i64::from(n));
                for v in favg {
                    o.d(v);
                }
            }
        }
        "zcat" => {
            let mut m2v = get_vol(cx, t)?;
            fill_painted(cx, t, &mut m2v)?;
            let nskip = t.i();
            let mut d = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = m2::mri_zconcat(g, a, b, &seg, &m2v, nskip, d.as_mut())?;
            o.ret(&r, d.as_ref());
        }
        "aedit" => {
            let mut old = get_vol(cx, t)?;
            fill_painted(cx, t, &mut old)?;
            let mut man = get_vol(cx, t)?;
            fill_painted(cx, t, &mut man)?;
            let mode = t.i();
            match mode {
                1 => {
                    let mut d = get_vol(cx, t)?;
                    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                    let r = m2::mri_apply_edits(g, a, b, &seg, &old, &man, Some(&mut d))?;
                    o.ret(&r, Some(&d));
                }
                2 => {
                    // in place into newauto: its copy is the input
                    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                    let copy =
                        larmorx_freesurfer_mri::copy::mri_copy(g, a, b, &seg, None)?.unwrap_new();
                    let r = m2::mri_apply_edits(g, a, b, &copy, &old, &man, Some(&mut seg))?;
                    o.ret(&r, Some(&seg));
                }
                _ => {
                    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
                    let r = m2::mri_apply_edits(g, a, b, &seg, &old, &man, None)?;
                    o.ret(&r, None);
                }
            }
        }
        _ => {
            let (dim, nshift, wrap) = (t.i(), t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = m2::mri_shift_dim(g, a, b, &seg, dim, nshift, wrap)?;
            o.vol(r.as_ref());
        }
    }
    Ok(())
}

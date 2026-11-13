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
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::mri::{self as mm, Mri, MriRegion};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_core::matrix as mx;
use larmorx_freesurfer_volume::arith::{self as ar, Pair, Tri};
use larmorx_freesurfer_volume::coords as co;
use larmorx_freesurfer_volume::vgeom;

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
                t.push(dx(g.pick(&[1.0, -1.0, 0.0, 2.5, f64::NAN, 1e300, -0.0, 0.1])));
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
            c => panic!("no generator for {c}"),
        }
        lines.push(t.join(" "));
    }
    lines
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
pub const COMMANDS: [&str; 47] = [
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

/// The destination of `f(src, dst, ...)`: 0 `NULL`, 1 a volume of the case, 2 the source.
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
        c => panic!("unknown command {c}"),
    }
    Ok(())
}

/// `cmph`'s header fields beyond the volume's tokens.
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

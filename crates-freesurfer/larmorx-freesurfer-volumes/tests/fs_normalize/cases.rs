// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-normalize probe's cases (`validation/probes/fs-normalize/`): a seeded generator of case
//! lines for `mri_normalize`'s own functions and options, and the port's answer to each, printed
//! as `fsnormalize` prints FreeSurfer's (`<result> | <text>`, the text FreeSurfer's code wrote
//! C-escaped; `EXIT <status> | <text>` when it ended the program).
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::read::gca_read;
use larmorx_freesurfer_gca::transform::transform_read;
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::mriset::mri_copy_label;
use larmorx_freesurfer_volume::resize::mri_change_type;
use larmorx_freesurfer_volumes::mri_normalize::helpers::{self as h, Run};
use larmorx_freesurfer_volumes::mri_normalize::options::{Options, parse_options, usage_exit};

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

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn rle(vals: &[String]) -> Vec<String> {
    let mut t = Vec::new();
    let mut i = 0;
    while i < vals.len() {
        let mut k = 1;
        while i + k < vals.len() && vals[i + k] == vals[i] {
            k += 1;
        }
        t.push(if k > 1 {
            format!("{}*{k}", vals[i])
        } else {
            vals[i].clone()
        });
        i += k;
    }
    t
}

/// A voxel token of type `ty`.
fn token(ty: i32, v: f64) -> String {
    match ty {
        MRI_FLOAT => fx(v as f32),
        MRI_UCHAR => ((v as i64).clamp(0, 255)).to_string(),
        MRI_SHORT => ((v as i64).clamp(-32768, 32767)).to_string(),
        _ => (v as i64).to_string(),
    }
}

/// The tokens of a volume of extent `e` and type `ty` with the given voxel values (x fastest).
fn vol_tokens(t: &mut Vec<String>, e: [i32; 3], ty: i32, xs: f32, vals: &[f64]) {
    t.extend([e[0], e[1], e[2], ty, 1].map(|v| v.to_string()));
    for _ in 0..3 {
        t.push(fx(xs));
    }
    let toks: Vec<String> = vals.iter().map(|&v| token(ty, v)).collect();
    t.extend(rle(&toks));
}

/// A geometry's 12 floats (direction cosines, centre).
fn geom_tokens(t: &mut Vec<String>, g: &mut Gen, base: &[f32; 12]) -> [f32; 12] {
    let mut v = *base;
    for x in v.iter_mut().skip(9) {
        if g.chance(4) {
            *x += g.int(-6, 6) as f32 / 2.0;
        }
    }
    for x in v {
        t.push(fx(x));
    }
    v
}

const FRAMES: [[f32; 9]; 4] = [
    [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0],
    [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
];

fn base_geom(g: &mut Gen) -> [f32; 12] {
    let fr = FRAMES[g.below(4) as usize];
    let mut b = [0f32; 12];
    b[..9].copy_from_slice(&fr);
    for x in b.iter_mut().skip(9) {
        *x = if g.chance(2) {
            0.0
        } else {
            g.int(-16, 16) as f32 / 4.0
        };
    }
    b
}

fn extent(g: &mut Gen) -> [i32; 3] {
    if g.chance(6) {
        return [
            g.int(14, 22) as i32,
            g.int(14, 22) as i32,
            g.int(12, 20) as i32,
        ];
    }
    let mut e = [0; 3];
    for x in &mut e {
        *x = if g.chance(12) { 1 } else { g.int(3, 12) as i32 };
    }
    e
}

fn n_of(e: [i32; 3]) -> usize {
    (e[0] * e[1] * e[2]) as usize
}

/// Intensities like a normalised T1: white matter near 110, grey near 70, CSF near 30, with
/// noise, now and then 0s, a NaN or an infinity.
fn intensities(g: &mut Gen, e: [i32; 3], heavy: bool) -> Vec<f64> {
    let n = n_of(e);
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let base = match g.below(10) {
            0..=4 => 110.0,
            5..=6 => 70.0,
            7 => 30.0,
            8 => 0.0,
            _ => 125.0,
        };
        let mut x = base + g.int(-12, 12) as f64 + g.int(0, 3) as f64 / 4.0;
        if heavy && g.chance(40) {
            x = g.pick(&[f64::NAN, f64::INFINITY, -5.0, 1e6]);
        }
        v.push(x);
    }
    v
}

/// A control volume: blobs of `mark` (1 mostly) on 0s.
fn ctrl_vals(g: &mut Gen, e: [i32; 3]) -> Vec<f64> {
    let n = n_of(e);
    let mut v = vec![0.0; n];
    if g.chance(10) {
        return v;
    }
    let mark = if g.chance(5) {
        g.pick(&[128.0, 255.0, 2.0])
    } else {
        1.0
    };
    let dense = g.pick(&[2u64, 3, 5, 10]);
    for x in v.iter_mut() {
        if g.chance(dense) {
            *x = mark;
        }
    }
    for _ in 0..g.int(0, 2) {
        let c = [
            g.int(0, i64::from(e[0]) - 1),
            g.int(0, i64::from(e[1]) - 1),
            g.int(0, i64::from(e[2]) - 1),
        ];
        let r = g.int(1, 4);
        for z in 0..e[2] {
            for y in 0..e[1] {
                for x in 0..e[0] {
                    let d = (i64::from(x) - c[0]).abs()
                        + (i64::from(y) - c[1]).abs()
                        + (i64::from(z) - c[2]).abs();
                    if d <= r {
                        v[(x + e[0] * (y + e[1] * z)) as usize] = mark;
                    }
                }
            }
        }
    }
    v
}

const ASEG_LABELS: [i64; 20] = [
    0, 2, 41, 2, 41, 3, 42, 4, 43, 10, 11, 12, 17, 26, 77, 24, 14, 7, 251, 5,
];

fn aseg_vals(g: &mut Gen, e: [i32; 3]) -> Vec<f64> {
    let n = n_of(e);
    let main = g.pick(&[2.0, 41.0]);
    (0..n)
        .map(|_| {
            if g.chance(3) {
                g.pick(&ASEG_LABELS) as f64
            } else {
                main
            }
        })
        .collect()
}

fn small_float(g: &mut Gen) -> f32 {
    g.pick(&[10.0, 10.0, 0.0, 5.5, -3.0, 90.0, 120.0, 105.0, f32::NAN])
}

fn gxyz(t: &mut Vec<String>, g: &mut Gen, e: [i32; 3]) {
    if g.chance(3) {
        for n in e {
            t.push(g.int(0, i64::from(n) - 1).to_string());
        }
    } else {
        t.extend(["-1", "-1", "-1"].map(str::to_owned));
    }
}

fn gdiag(t: &mut Vec<String>, g: &mut Gen) {
    let d: u64 = match g.below(8) {
        0 => 8,
        1 => 0x1000_0008,
        _ => 0,
    };
    t.push(d.to_string());
}

/// A closed octahedron (outward faces, now and then inward) of radius `r` around `c`.
fn octa_bytes(r: f32, c: [f32; 3], inward: bool) -> Vec<u8> {
    let v: [[f32; 3]; 6] = [
        [c[0] + r, c[1], c[2]],
        [c[0] - r, c[1], c[2]],
        [c[0], c[1] + r, c[2]],
        [c[0], c[1] - r, c[2]],
        [c[0], c[1], c[2] + r],
        [c[0], c[1], c[2] - r],
    ];
    let mut f: [[i32; 3]; 8] = [
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    if inward {
        for x in &mut f {
            x.swap(1, 2);
        }
    }
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Sun Nov 15 06:10:00 2026\n\n");
    b.extend_from_slice(&6i32.to_be_bytes());
    b.extend_from_slice(&8i32.to_be_bytes());
    for p in &v {
        for x in p {
            b.extend_from_slice(&x.to_be_bytes());
        }
    }
    for x in &f {
        for k in x {
            b.extend_from_slice(&k.to_be_bytes());
        }
    }
    b
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A label file of a few vertices.
fn label_bytes(g: &mut Gen) -> Vec<u8> {
    let n = g.int(1, 4);
    let mut s = format!("#!ascii label  , from subject  vox2ras=TkReg\n{n}\n");
    for i in 0..n {
        s += &format!(
            "{} {}.000 {}.000 {}.000 0.0000000000\n",
            i,
            g.int(-3, 3),
            g.int(-3, 3),
            g.int(-3, 3)
        );
    }
    s.into_bytes()
}

const NO_ARG: [&str; 20] = [
    "-no1d",
    "-T1",
    "-t2",
    "-PD",
    "-noskull",
    "-MGH_MPRAGE",
    "-mprage",
    "-washu_mprage",
    "-monkey",
    "-nosnr",
    "-snr",
    "-no-gentle-cp",
    "-gentle-cp",
    "-conform",
    "-noconform",
    "-gentle",
    "-Gentle",
    "-x",
    "-",
    "-1",
];
const INT_ARG: [&str; 7] = [
    "-nonmax_suppress",
    "-erode",
    "-n",
    "-p",
    "-seed",
    "-N",
    "-prune",
];
const FLOAT_ARG: [&str; 7] = ["-GRAD", "-min_dist", "-sigma", "-a", "-b", "-g", "-B"];
const NAME_ARG: [&str; 9] = [
    "-renorm",
    "-C",
    "-mask",
    "-aseg",
    "-segmentation",
    "-fonly",
    "-file_only",
    "-f",
    "-foo",
];
const TWO_NAMES: [&str; 5] = ["-INTERIOR", "-r", "-l", "-w", "-W"];
const INTS: [&str; 7] = ["3", "0", "-1", "12abc", "2147483648", "1", "7"];
const FLOATS: [&str; 9] = [
    "0.5", "2.5", "1e3", "-4", "nan", "inf", "abc", "12.75", "110",
];

fn opt_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let with_label = g.chance(4);
    if with_label {
        t.push(hexs(&label_bytes(g)));
    } else {
        t.push("-".to_owned());
    }
    let mut a: Vec<String> = Vec::new();
    for _ in 0..g.int(0, 5) {
        match g.below(14) {
            0..=3 => a.push(g.pick(&NO_ARG).to_owned()),
            4 | 5 => {
                a.push(g.pick(&INT_ARG).to_owned());
                a.push(g.pick(&INTS).to_owned());
            }
            6 | 7 => {
                a.push(g.pick(&FLOAT_ARG).to_owned());
                a.push(g.pick(&FLOATS).to_owned());
            }
            8 => {
                a.push(g.pick(&NAME_ARG).to_owned());
                a.push(g.pick(&["x.mgz", "c.mgz", "nu.mgz"]).to_owned());
            }
            9 => {
                a.push(g.pick(&TWO_NAMES).to_owned());
                a.push("a.mgz".to_owned());
                a.push("b.mgz".to_owned());
            }
            10 => {
                let o = g.pick(&["-checknorm", "-mask_orig", "-mask_sigma"]);
                a.push(o.to_owned());
                if o != "-mask_sigma" {
                    a.push("n.mgz".to_owned());
                }
                a.push(g.pick(&FLOATS).to_owned());
                if o != "-mask_orig" {
                    a.push(g.pick(&FLOATS).to_owned());
                }
            }
            11 => {
                a.push(g.pick(&["-d", "-v", "-D"]).to_owned());
                for _ in 0..3 {
                    a.push(g.pick(&INTS).to_owned());
                }
            }
            12 => {
                a.push(
                    g.pick(&["-label", "-lonly", "-label_only", "-labelonly"])
                        .to_owned(),
                );
                a.push(
                    if with_label {
                        "in.l.label"
                    } else {
                        "missing.label"
                    }
                    .to_owned(),
                );
            }
            _ => {
                if g.chance(2) {
                    a.push("-surface".to_owned());
                    a.push("lh.white".to_owned());
                    a.push(g.pick(&["identity.nofile", "missing.lta"]).to_owned());
                } else {
                    a.push(
                        g.pick(&["-u", "-h", "-?", "--help", "--usage", "-help"])
                            .to_owned(),
                    );
                }
            }
        }
    }
    if g.chance(5) {
        // the last option short of arguments (FS-1016)
        a.truncate(a.len().saturating_sub(g.int(1, 2) as usize));
    } else if !g.chance(8) {
        a.push("in.mgz".to_owned());
        a.push("out.mgz".to_owned());
    }
    t.push(a.len().to_string());
    t.extend(a);
}

/// The case lines of `seed`: `n` of them, the commands in turn.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    const COMMANDS: [&str; 9] = [
        "cbias", "wmo", "wmom", "snw", "tcp", "rcor", "t2o", "aip", "opt",
    ];
    let mut g = Gen::new(seed);
    (0..n)
        .map(|i| {
            let cmd = COMMANDS[i % COMMANDS.len()];
            let mut t = vec![cmd.to_owned()];
            let heavy = g.chance(6);
            match cmd {
                "cbias" => {
                    let e = extent(&mut g);
                    let ty = if g.chance(4) { MRI_UCHAR } else { MRI_FLOAT };
                    let a = intensities(&mut g, e, heavy);
                    vol_tokens(&mut t, e, ty, 1.0, &a);
                    let b = intensities(&mut g, e, heavy);
                    vol_tokens(&mut t, e, MRI_FLOAT, 1.0, &b);
                }
                "wmo" | "wmom" => {
                    let e = extent(&mut g);
                    let a = intensities(&mut g, e, heavy);
                    vol_tokens(&mut t, e, MRI_FLOAT, 1.0, &a);
                    let c = ctrl_vals(&mut g, e);
                    let ty = if g.chance(6) { MRI_INT } else { MRI_UCHAR };
                    vol_tokens(&mut t, e, ty, 1.0, &c);
                    t.push(g.pick(&[5, 7, 10, 15, 22, 0, -3]).to_string());
                    gdiag(&mut t, &mut g);
                }
                "snw" => {
                    let e = extent(&mut g);
                    let base = base_geom(&mut g);
                    let c = ctrl_vals(&mut g, e);
                    vol_tokens(&mut t, e, MRI_UCHAR, 1.0, &c);
                    geom_tokens(&mut t, &mut g, &base);
                    let e2 = if g.chance(5) { extent(&mut g) } else { e };
                    let a = aseg_vals(&mut g, e2);
                    let ty = if g.chance(3) { MRI_INT } else { MRI_UCHAR };
                    let xs = if g.chance(6) { 2.0 } else { 1.0 };
                    vol_tokens(&mut t, e2, ty, xs, &a);
                    geom_tokens(&mut t, &mut g, &base);
                }
                "tcp" | "rcor" => {
                    let e = extent(&mut g);
                    let c = ctrl_vals(&mut g, e);
                    let ty = if g.chance(6) { MRI_FLOAT } else { MRI_UCHAR };
                    vol_tokens(&mut t, e, ty, 1.0, &c);
                    let e2 = if cmd == "rcor" && g.chance(8) {
                        [e[0], e[1], (e[2] - 1).max(1)]
                    } else {
                        e
                    };
                    let a = intensities(&mut g, e2, heavy);
                    vol_tokens(&mut t, e2, MRI_FLOAT, 1.0, &a);
                    if cmd == "tcp" {
                        t.push(fx(small_float(&mut g)));
                    } else {
                        t.push(fx(g.pick(&[90.0, 100.0, 0.0, f32::NAN])));
                        t.push(fx(g.pick(&[120.0, 115.0, 255.0])));
                    }
                    gxyz(&mut t, &mut g, e);
                }
                "t2o" => {
                    let e = extent(&mut g);
                    let base = base_geom(&mut g);
                    let xs = if g.chance(25) {
                        0.0
                    } else {
                        g.pick(&[1.0, 1.0, 0.5, 2.0])
                    };
                    let c = ctrl_vals(&mut g, e);
                    vol_tokens(&mut t, e, MRI_UCHAR, xs, &c);
                    geom_tokens(&mut t, &mut g, &base);
                    // a signed distance from a ball (negative inside), now and then noise
                    let cc = [e[0] as f64 / 2.0, e[1] as f64 / 2.0, e[2] as f64 / 2.0];
                    let r = g.int(1, 5) as f64;
                    let mut dv = Vec::new();
                    for z in 0..e[2] {
                        for y in 0..e[1] {
                            for x in 0..e[0] {
                                let d = ((f64::from(x) - cc[0]).powi(2)
                                    + (f64::from(y) - cc[1]).powi(2)
                                    + (f64::from(z) - cc[2]).powi(2))
                                .sqrt();
                                let noise = if g.chance(5) {
                                    g.int(-2, 2) as f64 / 4.0
                                } else {
                                    0.0
                                };
                                dv.push(d - r + noise);
                            }
                        }
                    }
                    vol_tokens(&mut t, e, MRI_FLOAT, xs, &dv);
                    geom_tokens(&mut t, &mut g, &base);
                    let a = intensities(&mut g, e, heavy);
                    vol_tokens(&mut t, e, MRI_FLOAT, xs, &a);
                    geom_tokens(&mut t, &mut g, &base);
                }
                "aip" => {
                    let e = if g.chance(3) {
                        [
                            g.int(10, 16) as i32,
                            g.int(10, 16) as i32,
                            g.int(10, 16) as i32,
                        ]
                    } else {
                        extent(&mut g)
                    };
                    let base = base_geom(&mut g);
                    let xs = g.pick(&[1.0f32, 1.0, 0.5, 2.0, 0.34]);
                    let c = ctrl_vals(&mut g, e);
                    vol_tokens(&mut t, e, MRI_UCHAR, xs, &c);
                    geom_tokens(&mut t, &mut g, &base);
                    let a = intensities(&mut g, e, heavy);
                    vol_tokens(&mut t, e, MRI_FLOAT, xs, &a);
                    t.push(fx(25.0));
                    t.push(fx(g.pick(&[12.5f32, 18.75, 0.0])));
                    for _ in 0..2 {
                        let r = g.int(2, 8) as f32;
                        let c = [0, 1, 2].map(|_| g.int(-4, 4) as f32 / 2.0);
                        t.push(hexs(&octa_bytes(r, c, g.chance(8))));
                    }
                    if g.chance(2) {
                        t.push("1".to_owned());
                        let s = aseg_vals(&mut g, e);
                        vol_tokens(&mut t, e, MRI_UCHAR, xs, &s);
                    } else {
                        t.push("0".to_owned());
                    }
                    gdiag(&mut t, &mut g);
                }
                _ => opt_tokens(&mut t, &mut g),
            }
            t.join(" ")
        })
        .collect()
}

/// Standard output and error, one stream (the probe sends both to one file). Standard error's
/// handle carries its case's `Globals::stderr` flag, which its first byte sets (relay 48 of track
/// D: the C library's first `perror` on a write-only standard error nothing has written to leaves
/// `EINVAL` in `errno`, which fs.core models; the official probe's children write to a file).
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>, Option<Arc<AtomicBool>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if let Some(w) = self.1.as_ref().filter(|_| !b.is_empty()) {
            w.store(true, Ordering::Relaxed);
        }
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
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("hex"))
    }
}

struct Ctx {
    g: Globals,
    a: Shared,
    b: Shared,
}

fn get_vol(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let (w, h, d, ty, nf) = (t.i(), t.i(), t.i(), t.i(), t.i());
    let mut m = mri_alloc_sequence(&mut cx.g, &mut cx.a, &mut cx.b, w, h, d, ty, nf)?;
    m.vg.xsize = t.f();
    m.vg.ysize = t.f();
    m.vg.zsize = t.f();
    let n = (w * h * d * nf) as usize;
    let size = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    let mut i = 0;
    while i < n {
        let tok = t.next();
        let (v, k) = match tok.split_once('*') {
            Some((v, k)) => (v, k.parse::<usize>().expect("count")),
            None => (tok, 1),
        };
        let bytes: Vec<u8> = match ty {
            MRI_UCHAR => vec![v.parse::<i64>().expect("byte") as u8],
            MRI_SHORT => (v.parse::<i64>().expect("short") as i16)
                .to_le_bytes()
                .to_vec(),
            MRI_INT => (v.parse::<i64>().expect("int") as i32)
                .to_le_bytes()
                .to_vec(),
            MRI_FLOAT => u32::from_str_radix(v, 16)
                .expect("hex")
                .to_le_bytes()
                .to_vec(),
            _ => panic!("type {ty}"),
        };
        for _ in 0..k {
            if i < n {
                c[i * size..(i + 1) * size].copy_from_slice(&bytes);
                i += 1;
            }
        }
    }
    Ok(m)
}

fn get_opt(cx: &mut Ctx, t: &mut Toks) -> Result<Option<Mri>, Fatal> {
    if t.i() != 0 {
        Ok(Some(get_vol(cx, t)?))
    } else {
        Ok(None)
    }
}

fn get_volg(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let mut m = get_vol(cx, t)?;
    let v = &mut m.vg;
    for p in [
        &mut v.x_r, &mut v.x_a, &mut v.x_s, &mut v.y_r, &mut v.y_a, &mut v.y_s, &mut v.z_r,
        &mut v.z_a, &mut v.z_s, &mut v.c_r, &mut v.c_a, &mut v.c_s,
    ] {
        *p = t.f();
    }
    v.ras_good_flag = 1;
    reinit_cache(&mut cx.g, &mut cx.a, &mut cx.b, &mut m.vg)?;
    Ok(m)
}

fn put_vol(o: &mut String, m: Option<&Mri>) {
    let Some(m) = m else {
        *o += "NULL ";
        return;
    };
    *o += &format!(
        "{} {} {} {} {} {:016x} {:08x} ",
        m.vg.width,
        m.vg.height,
        m.vg.depth,
        m.mtype,
        m.nframes,
        m.outside_val.to_bits(),
        m.vg.xsize.to_bits()
    );
    let n = (m.vg.width * m.vg.height * m.vg.depth * m.nframes) as usize;
    let c = m.chunk.as_ref().expect("voxels");
    let vals: Vec<String> = (0..n)
        .map(|i| match m.mtype {
            MRI_UCHAR => c[i].to_string(),
            MRI_SHORT => i16::from_le_bytes([c[2 * i], c[2 * i + 1]]).to_string(),
            MRI_INT => i32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap()).to_string(),
            MRI_FLOAT => format!(
                "{:08x}",
                u32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap())
            ),
            t => panic!("type {t}"),
        })
        .collect();
    for v in rle(&vals) {
        *o += &v;
        *o += " ";
    }
}

fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h = (h ^ u64::from(c)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// The files the case wrote (not `in.*`), sorted: volumes read back and printed, other files'
/// FNV-1a.
fn put_written(cx: &mut Ctx, dir: &Path, o: &mut String) -> Result<(), Fatal> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the case folder")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| !n.starts_with("in."))
        .collect();
    names.sort();
    for n in names {
        *o += &format!("F {n} ");
        if n.len() > 4 && n.ends_with(".mgz") {
            let gd = cx.g.gdiag;
            cx.g.gdiag = 0;
            let (mut a, mut b) = (cx.a.clone(), cx.b.clone());
            let m = mri_read_volume(
                &mut cx.g,
                &mut a,
                &mut b,
                &Environment::default(),
                Some(dir),
                &n,
            )?;
            cx.g.gdiag = gd;
            put_vol(o, m.as_ref());
        } else {
            let b = std::fs::read(dir.join(&n)).unwrap_or_default();
            *o += &format!("{:016x} ", fnv(&b));
        }
    }
    Ok(())
}

fn os(o: &mut String, s: &Option<String>) {
    *o += s.as_deref().unwrap_or("NULL");
    *o += " ";
}

fn of(o: &mut String, v: f32) {
    *o += &format!("{:08x} ", v.to_bits());
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut String, dir: &Path) -> Result<(), Fatal> {
    let env = Environment::from_pairs([("SURFER_SIDEDOOR", "1")]);
    let cmd = t.next();
    let (mut so, mut se) = (cx.a.clone(), cx.b.clone());
    match cmd {
        "cbias" => {
            let src = get_vol(cx, t)?;
            let dst = get_vol(cx, t)?;
            let b = h::compute_bias(&mut cx.g, &mut so, &mut se, &src, &dst)?;
            put_vol(o, Some(&b));
        }
        "wmo" | "wmom" => {
            let src = get_vol(cx, t)?;
            let mut ctrl = get_vol(cx, t)?;
            let ib = t.i();
            cx.g.gdiag = t.next().parse().expect("Gdiag");
            let mut r = Run {
                out: &mut so,
                err: &mut se,
                env: &env,
                dir: Some(dir),
            };
            if cmd == "wmo" {
                h::mri_remove_wm_outliers(&mut cx.g, &mut r, &src, &mut ctrl, ib)?;
            } else {
                h::mri_remove_wm_outliers_and_retain_medial_surface(
                    &mut cx.g, &mut r, &src, &mut ctrl, ib,
                )?;
            }
            cx.g.gdiag = 0;
            put_vol(o, Some(&ctrl));
            put_written(cx, dir, o)?;
        }
        "snw" => {
            let mut ctrl = get_volg(cx, t)?;
            let aseg = get_volg(cx, t)?;
            h::remove_nonwm_voxels(&mut cx.g, &mut so, &mut se, &mut ctrl, &aseg)?;
            put_vol(o, Some(&ctrl));
        }
        "tcp" | "rcor" => {
            let mut ctrl = get_vol(cx, t)?;
            let v = get_vol(cx, t)?;
            let a = t.f();
            let b = if cmd == "rcor" { t.f() } else { 0.0 };
            (cx.g.gx, cx.g.gy, cx.g.gz) = (t.i(), t.i(), t.i());
            if cmd == "tcp" {
                h::threshold_control_points(&mut cx.g, &mut so, &mut se, &mut ctrl, &v, a)?;
            } else {
                h::remove_controlpoints_outside_range(
                    &mut cx.g, &mut so, &mut se, &mut ctrl, &v, a, b,
                )?;
            }
            put_vol(o, Some(&ctrl));
        }
        "t2o" => {
            let mut ctrl = get_volg(cx, t)?;
            let dist = get_volg(cx, t)?;
            let src = get_volg(cx, t)?;
            let mut r = Run {
                out: &mut so,
                err: &mut se,
                env: &env,
                dir: Some(dir),
            };
            h::remove_surface_outliers_t2(&mut cx.g, &mut r, &mut ctrl, &dist, &src)?;
            put_vol(o, Some(&ctrl));
            put_written(cx, dir, o)?;
        }
        "aip" => {
            let mut ctrl = get_volg(cx, t)?;
            let vals = get_vol(cx, t)?;
            let (ia, ib) = (t.f(), t.f());
            std::fs::write(dir.join("in.lh.s0"), unhex(t.next())).expect("write");
            std::fs::write(dir.join("in.lh.s1"), unhex(t.next())).expect("write");
            let aseg = get_opt(cx, t)?;
            cx.g.gdiag = t.next().parse().expect("Gdiag");
            let mut sg = SurfGlobals::default();
            let (s0, s1) = {
                let mut io = Io {
                    out: &mut so,
                    err: &mut se,
                    env: &env,
                };
                let s0 = readwrite::read(&mut cx.g, &mut sg, &mut io, Some(dir), "in.lh.s0", true)?;
                let s1 = readwrite::read(&mut cx.g, &mut sg, &mut io, Some(dir), "in.lh.s1", true)?;
                (s0, s1)
            };
            let (Some(mut s0), Some(mut s1)) = (s0, s1) else {
                *o += "NOSURF ";
                return Ok(());
            };
            let mut r = Run {
                out: &mut so,
                err: &mut se,
                env: &env,
                dir: Some(dir),
            };
            h::add_interior_points(
                &mut cx.g,
                &mut sg,
                &mut r,
                &mut ctrl,
                &vals,
                ia,
                ib,
                &mut s0,
                &mut s1,
                aseg.as_ref(),
            )?;
            cx.g.gdiag = 0;
            put_vol(o, Some(&ctrl));
            put_written(cx, dir, o)?;
        }
        "opt" => {
            let lab = t.next();
            if lab != "-" {
                std::fs::write(dir.join("in.l.label"), unhex(lab)).expect("write");
            }
            let n = t.i();
            let args: Vec<String> = (0..n).map(|_| t.next().to_owned()).collect();
            let mut opts = Options::default();
            let mut r = Run {
                out: &mut so,
                err: &mut se,
                env: &env,
                dir: Some(dir),
            };
            let i = parse_options(&mut cx.g, &mut r, &mut opts, &args)?;
            if args.len() < i + 2 {
                return Err(usage_exit(&mut cx.g, &mut r, 0));
            }
            *o += &format!("{} {} {} ", args.len() - i + 1, args[i], args[i + 1]);
            let p = &opts;
            *o += &format!(
                "{} {} {} {} {} {} {} {} {:016x} ",
                p.contrast,
                i32::from(p.gca.is_some()),
                p.xform.as_ref().map_or(-1, |x| x.ttype),
                p.brain_distance,
                p.conform,
                p.gentle_flag,
                p.noskull,
                p.nosnr,
                p.min_dist.to_bits()
            );
            os(o, &p.checknorm_fname);
            of(o, p.check_min);
            of(o, p.check_max);
            os(o, &p.renorm_fname);
            *o += &format!("{} ", p.renorm_val);
            of(o, p.bias_sigma);
            os(o, &p.mask_fname);
            os(o, &p.interior_fname1);
            os(o, &p.interior_fname2);
            of(o, p.mask_sigma);
            of(o, p.mask_thresh);
            os(o, &p.mask_orig_fname);
            of(o, p.mask_orig_thresh);
            *o += &format!("{} {} ", p.scan_type, p.prune);
            of(o, p.max_gradient);
            *o += &format!("{} {} ", p.verbose, p.num_3d_iter);
            of(o, p.intensity_above);
            of(o, p.intensity_below);
            os(o, &p.control_point_fname);
            os(o, &p.long_control_volume_fname);
            os(o, &p.long_bias_volume_fname);
            os(o, &p.aseg_fname);
            os(o, &p.control_volume_fname);
            os(o, &p.bias_volume_fname);
            *o += &format!(
                "{} {} {} {} {} ",
                p.read_flag,
                p.long_flag,
                p.no1d,
                p.file_only,
                p.surfaces.len()
            );
            for s in &p.surfaces {
                *o += &format!("{} {} {} ", s.fname, s.xform_fname, s.xform.ttype);
            }
            of(o, p.grad_thresh);
            *o += &format!(
                "{} ",
                p.control_point_label.as_ref().map_or(-1, |l| l.n_points)
            );
            os(o, &p.output_control_points_vol);
            let g = &cx.g;
            *o += &format!(
                "{} {} {} {} {} {} {} {} {} ",
                p.do_gentle_cp_file,
                p.nonmax_suppress,
                p.erode,
                g.gx,
                g.gy,
                g.gz,
                g.gvx,
                g.gvy,
                g.gvz
            );
            // a draw only after -seed (unseeded, FreeSurfer seeds from the clock: FS-04)
            if args.iter().any(|a| a.eq_ignore_ascii_case("-seed")) {
                let v = random_number(&mut cx.g, &env, &mut so, &mut se, 0.0, 1.0)?;
                *o += &format!("{:016x} ", v.to_bits());
            }
        }
        "bobmf" => {
            let (vf, af, tf, whalf) = (t.next(), t.next(), t.next(), t.i());
            let src = read(cx, vf)?;
            let atlas = gca_read(&mut cx.g, &mut so, &mut se, None, af)?;
            let xf = transform_read(&mut cx.g, &mut so, &mut se, &env, None, tf)?;
            let (Some(src), Some(atlas), Some(mut xf)) = (src, atlas, xf) else {
                *o += "NOFILE ";
                return Ok(());
            };
            let m = h::build_outside_of_brain_mask(
                &mut cx.g, &mut so, &mut se, &env, &src, &atlas, &mut xf, whalf,
            )?;
            put_digest(o, &m);
        }
        "wmomf" => {
            let (vf, sf, ib) = (t.next(), t.next(), t.i());
            let (Some(src), Some(aseg)) = (read(cx, vf)?, read(cx, sf)?) else {
                *o += "NOFILE ";
                return Ok(());
            };
            let src = if src.mtype == MRI_FLOAT {
                src
            } else {
                mri_change_type(
                    &mut cx.g, &mut so, &mut se, &env, &src, MRI_FLOAT, 0.0, 255.0, 1,
                )?
                .expect("a float volume")
            };
            let mut ctrl = mri_clone(&mut cx.g, &mut so, &mut se, &aseg, None)?.unwrap_new();
            for label in [2, 41] {
                mri_copy_label(&mut cx.g, &mut se, &aseg, &mut ctrl, label)?;
            }
            let mut r = Run {
                out: &mut so,
                err: &mut se,
                env: &env,
                dir: Some(dir),
            };
            h::mri_remove_wm_outliers_and_retain_medial_surface(
                &mut cx.g, &mut r, &src, &mut ctrl, ib,
            )?;
            put_digest(o, &ctrl);
        }
        c => panic!("unknown command {c}"),
    }
    Ok(())
}

fn read(cx: &mut Ctx, f: &str) -> Result<Option<Mri>, Fatal> {
    let (mut a, mut b) = (cx.a.clone(), cx.b.clone());
    mri_read_volume(&mut cx.g, &mut a, &mut b, &Environment::default(), None, f)
}

/// `put_digest`: the extent, type, count of voxels above 0 and FNV-1a of the voxels.
fn put_digest(o: &mut String, m: &Mri) {
    let c = m.chunk.as_ref().expect("voxels");
    let bpv = m.bytes_per_vox;
    let nz = c.chunks(bpv).filter(|v| v.iter().any(|&b| b != 0)).count();
    *o += &format!(
        "{} {} {} {} {nz} {:016x} ",
        m.vg.width,
        m.vg.height,
        m.vg.depth,
        m.mtype,
        fnv(c)
    );
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

/// The port's line for a case line, and the defects it noted (`dir`: an empty folder for the
/// case's files).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let shared = Shared::default();
    let g = Globals::new(mode);
    let b = Shared(shared.0.clone(), Some(g.stderr.written.clone()));
    let mut cx = Ctx {
        g,
        a: shared.clone(),
        b,
    };
    cx.g.progname = "mri_normalize".to_owned();
    cx.g.comm = "fsnormalize".to_owned();
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = String::new();
    let res = case(&mut cx, &mut t, &mut o, dir);
    let text = escape(&shared.0.borrow());
    let result = match res {
        Ok(()) => o.trim_end().to_owned(),
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

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}

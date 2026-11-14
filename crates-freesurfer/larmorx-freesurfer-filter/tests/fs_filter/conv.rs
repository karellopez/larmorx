// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's convolution and derivative commands (`fsfilter.cpp`'s `filter_case`):
//! their case generators and the port's answers.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::{cc, classify, conv, sobel};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_volume::arith::Pair;

use super::{Ctx, Gen, Toks, fx, get_vol, put_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 32] = [
    "g1d", "cv1", "cvb", "cvs", "cvi", "cvg", "gsn", "gs", "mgs", "mean", "std", "stdnz", "red",
    "red2", "r1d", "rsl", "xsob", "ysob", "zsob", "sob", "sobf", "div", "lap", "mbv", "bcv", "rcl",
    "ccs", "rmh", "lcc6", "lcc18", "gf6", "gf18",
];

/// A volume of blobs: random boxes of values above 0 on a background (uchar mostly; float with
/// negative values and NaNs, or int, now and then).
fn blobs(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let ty = if g.chance(4) {
        g.pick(&[MRI_FLOAT, MRI_INT, MRI_SHORT])
    } else {
        MRI_UCHAR
    };
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2]) as usize;
    let mut v = vec![0.0f64; n];
    let fill = g.chance(6);
    for x in v.iter_mut() {
        if fill || g.chance(5) {
            *x = 1.0;
        }
    }
    for _ in 0..g.int(1, 4) {
        let lo = [0, 1, 2].map(|k| g.int(0, i64::from(e[k]) - 1) as i32);
        let hi = [0, 1, 2].map(|k| (lo[k] + g.int(0, 4) as i32).min(e[k] - 1));
        let val = if g.chance(4) {
            0.0
        } else {
            g.pick(&[1.0, 255.0, 7.0])
        };
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    v[((z * e[1] + y) * e[0] + x) as usize] = val;
                }
            }
        }
    }
    let vals: Vec<String> = v
        .iter()
        .map(|&x| {
            if ty == MRI_FLOAT && g.chance(15) {
                fx(g.pick(&[f32::NAN, -1.0, 0.5, -0.0]))
            } else {
                token(ty, x)
            }
        })
        .collect();
    t.extend(rle(&vals));
}

/// A label volume of the border classification: white, not white, ambiguous and other values
/// (uchar mostly; a float or int one with odd values now and then).
fn labels(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let ty = if g.chance(5) {
        g.pick(&[MRI_FLOAT, MRI_INT, MRI_SHORT])
    } else {
        MRI_UCHAR
    };
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2]) as usize;
    let white = g.chance(2);
    let vals: Vec<String> = (0..n)
        .map(|_| {
            if ty == MRI_FLOAT && g.chance(12) {
                return fx(g.pick(&[f32::NAN, 254.6, 1.4, 0.6, 3e9, -1.0]));
            }
            let v = if white && g.chance(2) {
                255.0
            } else {
                g.pick(&[0.0, 1.0, 1.0, 255.0, 255.0, 128.0, 0.0, 7.0])
            };
            token(ty, v)
        })
        .collect();
    t.extend(rle(&vals));
}

/// An intensity volume of the classification: uchar 0-255, or float (sometimes negative or NaN).
fn intensities(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let ty = if g.chance(4) { MRI_FLOAT } else { MRI_UCHAR };
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2]) as usize;
    let lo = g.int(0, 120);
    let vals: Vec<String> = (0..n)
        .map(|_| {
            if ty == MRI_FLOAT && g.chance(30) {
                return fx(g.pick(&[f32::NAN, -3.0, 300.5]));
            }
            let v = g.int(lo, lo + 135) as f64;
            if ty == MRI_FLOAT {
                fx((v + 0.25 * g.int(0, 3) as f64) as f32)
            } else {
                token(ty, v)
            }
        })
        .collect();
    t.extend(rle(&vals));
}

fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// A small extent (one voxel along an axis now and then).
fn small(g: &mut Gen, max: i64) -> [i32; 3] {
    let mut e = [0; 3];
    for x in &mut e {
        *x = if g.chance(8) { 1 } else { g.int(2, max) as i32 };
    }
    e
}

fn any_type(g: &mut Gen) -> i32 {
    g.pick(&[
        MRI_UCHAR, MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT, MRI_INT, MRI_LONG, MRI_USHRT,
    ])
}

/// A float value: small multiples, with now and then NaNs of several payloads, infinities, -0.
fn fval(g: &mut Gen) -> f32 {
    if g.chance(40) {
        return g.pick(&[
            f32::NAN,
            f32::from_bits(0x7fc0_1234),
            f32::from_bits(0xffc0_0042),
            f32::from_bits(0x7f80_0001),
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            3e38,
            1e-40,
        ]);
    }
    let v = g.int(-400, 400) as f32 / 8.0;
    if g.chance(5) { v * 1.37 + 0.1 } else { v }
}

/// A volume's tokens: extent, type, frames, voxel sizes, then its voxels (random values).
fn vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, nf: i32) {
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push(nf.to_string());
    for _ in 0..3 {
        let s = if g.chance(4) {
            g.pick(&[0.5f32, 2.0, 1.5, 0.9375, 3.0])
        } else {
            1.0
        };
        t.push(fx(s));
    }
    let n = (e[0] * e[1] * e[2] * nf).max(0) as usize;
    let flat = g.chance(10);
    let base = g.int(0, 200);
    // now and then a volume rich in NaNs of several payloads (the operands' order decides which)
    let nans = ty == MRI_FLOAT && g.chance(5);
    let vals: Vec<String> = (0..n)
        .map(|_| {
            if flat {
                return token(ty, base as f64);
            }
            if nans && g.chance(3) {
                return fx(f32::from_bits(g.pick(&[
                    0x7fc0_1234u32,
                    0xffc0_0042,
                    0x7f80_0001,
                    0xffa0_0005,
                    0x7fc0_0000,
                ])));
            }
            match ty {
                MRI_FLOAT => fx(fval(g)),
                MRI_UCHAR => {
                    if g.chance(4) {
                        "0".into()
                    } else {
                        g.int(0, 255).to_string()
                    }
                }
                MRI_SHORT => g.int(-300, 300).to_string(),
                MRI_USHRT => g.int(0, 1000).to_string(),
                _ => g.int(-5000, 5000).to_string(),
            }
        })
        .collect();
    t.extend(rle(&vals));
}

/// A binary-ish mask (uchar or float).
fn mask_vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let ty = if g.chance(3) { MRI_FLOAT } else { MRI_UCHAR };
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2]) as usize;
    let vals: Vec<String> = (0..n)
        .map(|_| {
            if ty == MRI_FLOAT && g.chance(5) {
                fx(g.pick(&[0.5, 0.49, 0.75, -1.0]))
            } else {
                token(ty, if g.chance(3) { 0.0 } else { 1.0 })
            }
        })
        .collect();
    t.extend(rle(&vals));
}

/// A kernel: its length, then its weights.
fn kernel(g: &mut Gen, t: &mut Vec<String>) {
    let len = if g.chance(60) {
        1003
    } else if g.chance(8) {
        g.int(0, 2)
    } else {
        g.pick(&[3, 3, 5, 5, 7, 9, 4, 6])
    };
    t.push(len.to_string());
    let nan = g.chance(6);
    for i in 0..len {
        let w = if nan && i == 1 {
            f32::NAN
        } else if len > 20 {
            1.0 / len as f32
        } else {
            g.int(1, 16) as f32 / 16.0
        };
        t.push(fx(w));
    }
}

fn frame(g: &mut Gen, nf: i32) -> i32 {
    if g.chance(15) {
        g.pick(&[-1, nf, 5])
    } else {
        g.int(0, i64::from(nf) - 1) as i32
    }
}

/// An optional destination volume: none, or one of `ty` (sometimes another extent or frames).
fn opt_dst(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, nf: i32, odd: u64) {
    if g.chance(2) {
        t.push("0".into());
        return;
    }
    t.push("1".into());
    let mut e2 = e;
    let mut nf2 = nf;
    if g.chance(odd) {
        let k = g.below(3) as usize;
        e2[k] = (e2[k] + g.pick(&[-1, 1])).max(1);
    }
    if g.chance(odd) {
        nf2 = g.int(1, 3) as i32;
    }
    vol(g, t, e2, ty, nf2);
}

/// A destination by mode: 0 none, 1 given (`ty`, sometimes another extent or frames), 2 the source.
fn mode_dst(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, nf: i32) {
    let m = g.pick(&[0, 0, 1, 2]);
    t.push(m.to_string());
    if m == 1 {
        let mut e2 = e;
        let mut nf2 = nf;
        if g.chance(8) {
            let k = g.below(3) as usize;
            e2[k] += 1;
        }
        if g.chance(10) {
            nf2 += 1;
        }
        vol(g, t, e2, ty, nf2);
    }
}

fn std_dev(g: &mut Gen) -> f64 {
    if g.chance(10) {
        g.pick(&[0.0, -1.0, f64::NAN, 4.0])
    } else {
        g.pick(&[0.5, 1.0, 1.5, 2.0, 0.75])
    }
}

/// One case of `cmd`.
pub fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "g1d" => {
            let s = if g.chance(10) {
                g.pick(&[f32::NAN, -1.0, 0.0, 1e-3, 70.0, f32::INFINITY])
            } else {
                g.pick(&[0.5, 1.0, 1.5, 2.0, 0.25, 3.3, 0.75, 7.5, 1.25])
            };
            t.push(fx(s));
            t.push(g.pick(&[-1i32, 0, 0, 3, 4, 5, 9, 1, 2]).to_string());
        }
        "cv1" | "cvb" | "cvs" | "cvi" => {
            let e = small(g, 7);
            let ty = any_type(g);
            let nf = if g.chance(6) { 2 } else { 1 };
            vol(g, t, e, ty, nf);
            let dty = match cmd {
                "cv1" => g.pick(&[
                    MRI_FLOAT, MRI_FLOAT, MRI_UCHAR, MRI_SHORT, MRI_INT, MRI_LONG,
                ]),
                "cvb" => g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_UCHAR, MRI_FLOAT]),
                "cvs" => g.pick(&[MRI_SHORT, MRI_SHORT, MRI_SHORT, MRI_UCHAR]),
                _ => g.pick(&[MRI_INT, MRI_INT, MRI_INT, MRI_FLOAT]),
            };
            let a1 = if g.chance(4) { 2 } else { 1 };
            opt_dst(g, t, e, dty, a1, 8);
            kernel(g, t);
            let axis = if g.chance(20) {
                g.pick(&[3, -1])
            } else {
                g.int(0, 2) as i32
            };
            t.push(axis.to_string());
            let sf = frame(g, nf);
            let df = frame(g, 1);
            t.push(sf.to_string());
            t.push(df.to_string());
        }
        "cvg" => {
            let e = small(g, 6);
            let ty = g.pick(&[
                MRI_UCHAR, MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT, MRI_INT,
            ]);
            let nf = if g.chance(10) {
                0
            } else if g.chance(5) {
                2
            } else {
                1
            };
            vol(g, t, e, ty, nf);
            let m = g.pick(&[0, 0, 1, 2]);
            t.push(m.to_string());
            if m == 1 {
                let dty = if g.chance(10) { MRI_FLOAT } else { ty };
                vol(g, t, e, dty, nf);
            }
            // the kernel volume
            let len = g.pick(&[1, 3, 5, 7, 9]);
            let kty = if g.chance(20) { MRI_UCHAR } else { MRI_FLOAT };
            t.extend([
                len.to_string(),
                "1".into(),
                "1".into(),
                kty.to_string(),
                "1".into(),
            ]);
            for _ in 0..3 {
                t.push(fx(1.0));
            }
            for _ in 0..len {
                t.push(token(kty, f64::from(g.int(1, 16) as f32 / 16.0)));
            }
        }
        "gsn" | "gs" => {
            let e = small(g, 6);
            let ty = any_type(g);
            let nf = if g.chance(6) { 2 } else { 1 };
            vol(g, t, e, ty, nf);
            let a0 = if g.chance(5) { MRI_UCHAR } else { MRI_FLOAT };
            mode_dst(g, t, e, a0, nf);
            if cmd == "gsn" {
                for _ in 0..3 {
                    t.push(dx(std_dev(g)));
                }
            } else {
                t.push(dx(std_dev(g)));
                t.push(g.pick(&[1, 1, 1, 0, 2]).to_string());
            }
        }
        "mgs" => {
            let e = small(g, 6);
            let ty = any_type(g);
            let nf = if g.chance(6) { 2 } else { 1 };
            vol(g, t, e, ty, nf);
            if g.chance(4) {
                t.push("0".into());
            } else {
                t.push("1".into());
                let mut e2 = e;
                if g.chance(10) {
                    e2[0] += 1;
                }
                mask_vol(g, t, e2);
            }
            mode_dst(g, t, e, MRI_FLOAT, nf);
            t.push(dx(std_dev(g)));
        }
        "mean" => {
            let e = small(g, 8);
            let ty = any_type(g);
            let nf = if g.chance(6) { 2 } else { 1 };
            vol(g, t, e, ty, nf);
            let a0 = g.pick(&[MRI_FLOAT, MRI_FLOAT, MRI_UCHAR]);
            opt_dst(g, t, e, a0, nf, 8);
            t.push(g.pick(&[3, 3, 5, 1, 0, -1, 2, 4]).to_string());
        }
        "std" | "stdnz" => {
            let e = small(g, 8);
            let ty = any_type(g);
            vol(g, t, e, ty, 1);
            let a0 = g.pick(&[MRI_FLOAT, MRI_FLOAT, MRI_FLOAT, MRI_UCHAR]);
            opt_dst(g, t, e, a0, 1, 8);
            let mut e2 = e;
            if g.chance(10) {
                e2[1] = (e2[1] - 1).max(1);
            }
            vol(g, t, e2, MRI_FLOAT, 1);
            t.push(g.pick(&[3, 3, 5, 1, 0, 2]).to_string());
        }
        "red" | "red2" => {
            let mut e = small(g, 9);
            if g.chance(4) {
                e[2] = 1;
            }
            let ty = g.pick(&[MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT]);
            vol(g, t, e, ty, 1);
            for _ in 0..3 {
                t.push(fx(g.int(-40, 40) as f32 / 4.0));
            }
            if g.chance(4) {
                t.push("1".into());
                let h = [
                    (e[0] / 2).max(1),
                    (e[1] / 2).max(1),
                    if cmd == "red" {
                        (e[2] / 2).max(1)
                    } else {
                        e[2]
                    },
                ];
                vol(g, t, h, if cmd == "red" { MRI_FLOAT } else { MRI_UCHAR }, 1);
            } else {
                t.push("0".into());
            }
        }
        "r1d" | "rsl" => {
            let e = small(g, 9);
            let ty = any_type(g);
            vol(g, t, e, ty, 1);
            if g.chance(4) {
                t.push("1".into());
                let h = [(e[0] / 2).max(1), (e[1] / 2).max(1), (e[2] / 2).max(1)];
                let ty = g.pick(&[MRI_FLOAT, MRI_UCHAR]);
                vol(g, t, h, ty, 1);
            } else {
                t.push("0".into());
            }
            kernel(g, t);
            t.push(g.pick(&[0, 1, 2, 2, 1, 0, 3]).to_string());
        }
        "xsob" | "ysob" | "zsob" => {
            let e = small(g, 7);
            let ty = g.pick(&[
                MRI_UCHAR, MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT, MRI_INT,
            ]);
            vol(g, t, e, ty, 1);
            let nf = g.int(1, 3) as i32;
            let a0 = if g.chance(10) { MRI_UCHAR } else { MRI_FLOAT };
            opt_dst(g, t, e, a0, nf, 10);
            t.push(frame(g, nf).to_string());
        }
        "sob" | "sobf" => {
            let e = small(g, 7);
            let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT]);
            let nf = if cmd == "sobf" { g.int(1, 2) as i32 } else { 1 };
            vol(g, t, e, ty, nf);
            let a0 = if g.chance(10) { MRI_UCHAR } else { MRI_FLOAT };
            let a1 = if g.chance(8) { 1 } else { 3 };
            opt_dst(g, t, e, a0, a1, 12);
            let a0 = if g.chance(4) { MRI_UCHAR } else { MRI_FLOAT };
            opt_dst(g, t, e, a0, 1, 12);
            if cmd == "sobf" {
                t.push(frame(g, nf).to_string());
            }
        }
        "mbv" => {
            let e = small(g, 8);
            labels(g, t, e);
            if g.chance(3) {
                t.push("1".into());
                let mut e2 = e;
                if g.chance(8) {
                    e2[0] = (e2[0] - 1).max(1);
                }
                let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_FLOAT]);
                vol(g, t, e2, ty, 1);
            } else {
                t.push("0".into());
            }
        }
        "bcv" | "rcl" | "ccs" => {
            let e = small(g, 8);
            intensities(g, t, e);
            labels(g, t, e);
            match cmd {
                "bcv" => {
                    t.push(g.pick(&[3, 5, 7, 11, 1, 0]).to_string());
                    for k in 0..3 {
                        t.push(g.int(0, i64::from(e[k]) - 1).to_string());
                    }
                }
                "rcl" => {
                    let m = g.pick(&[0, 1, 2, 2]);
                    t.push(m.to_string());
                    if m == 1 {
                        vol(g, t, e, MRI_UCHAR, 1);
                    }
                    let lo = g.int(20, 100) as f32;
                    t.push(fx(lo));
                    t.push(fx(lo + g.int(10, 90) as f32));
                    t.push(g.pick(&[5, 3, 7, 11]).to_string());
                }
                _ => {
                    let lo = if g.chance(8) {
                        -1.0
                    } else {
                        g.int(10, 60) as f32
                    };
                    t.push(fx(lo));
                    t.push(fx(lo + g.int(20, 120) as f32));
                }
            }
        }
        "rmh" | "lcc6" | "lcc18" => {
            let e = small(g, 10);
            blobs(g, t, e);
        }
        "gf6" | "gf18" => {
            let e = small(g, 10);
            blobs(g, t, e);
            let lty = g.pick(&[MRI_INT, MRI_INT, MRI_FLOAT, MRI_UCHAR]);
            t.extend(e.map(|v| v.to_string()));
            t.extend([lty.to_string(), "1".into()]);
            for _ in 0..3 {
                t.push(fx(1.0));
            }
            let n = (e[0] * e[1] * e[2]) as usize;
            let vals: Vec<String> = (0..n)
                .map(|_| token(lty, if g.chance(8) { 2.0 } else { 0.0 }))
                .collect();
            t.extend(rle(&vals));
            t.push(g.pick(&[1, 3, 300, -1]).to_string());
            for k in 0..3 {
                t.push(g.int(0, i64::from(e[k]) - 1).to_string());
            }
        }
        _ => {
            // div, lap
            let e = small(g, 7);
            let ty = g.pick(&[MRI_UCHAR, MRI_FLOAT, MRI_FLOAT, MRI_SHORT, MRI_INT]);
            let nf = if cmd == "lap" && g.chance(4) { 2 } else { 1 };
            vol(g, t, e, ty, nf);
            opt_dst(g, t, e, MRI_FLOAT, nf, 10);
        }
    }
}

fn hdr(o: &mut String, m: &Mri) {
    let v = &m.vg;
    for f in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s, m.xstart, m.xend, m.ystart, m.yend, m.zstart, m.zend, m.thick, m.ps,
    ] {
        *o += &format!("{:08x} ", f.to_bits());
    }
    *o += &format!("{} {} {} ", m.scale, m.imnr0, m.imnr1);
}

fn full(o: &mut String, m: Option<&Mri>) {
    match m {
        None => *o += "NULL ",
        Some(m) => {
            put_vol(o, m);
            hdr(o, m);
        }
    }
}

/// `put_result(r, dst)` for a result that may be the given destination.
fn result(o: &mut String, r: Ret<Mri>, dst: Option<&Mri>) {
    match r {
        Ret::New(m) => {
            *o += "0 ";
            full(o, Some(&m));
            if let Some(d) = dst {
                full(o, Some(d));
            }
        }
        Ret::Given => {
            *o += "1 ";
            full(o, dst);
        }
        Ret::Null => {
            *o += if dst.is_none() { "1 " } else { "0 " };
            full(o, None);
            if let Some(d) = dst {
                full(o, Some(d));
            }
        }
    }
}

fn get_volc(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let mut m = get_vol(cx, t)?;
    m.vg.c_r = t.f();
    m.vg.c_a = t.f();
    m.vg.c_s = t.f();
    reinit_cache(&mut cx.g, &mut cx.a, &mut cx.b, &mut m.vg)?;
    Ok(m)
}

fn opt(cx: &mut Ctx, t: &mut Toks) -> Result<Option<Mri>, Fatal> {
    Ok(if t.i() != 0 {
        Some(get_vol(cx, t)?)
    } else {
        None
    })
}

fn kern(t: &mut Toks) -> Vec<f32> {
    let n = t.i();
    (0..n.max(0)).map(|_| t.f()).collect()
}

fn dbl(t: &mut Toks) -> f64 {
    f64::from_bits(u64::from_str_radix(t.next(), 16).expect("hex"))
}

/// The port's answer for one of this module's commands; `false` for another command.
pub fn run(cx: &mut Ctx, cmd: &str, t: &mut Toks, o: &mut String) -> Result<bool, Fatal> {
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    match cmd {
        "g1d" => {
            let s = t.f();
            let ml = t.i();
            let m = conv::mri_gaussian1d(g, a, b, s, ml)?;
            full(o, Some(&m));
        }
        "cv1" | "cvb" | "cvs" | "cvi" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let k = kern(t);
            let (axis, sf, df) = (t.i(), t.i(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let f = match cmd {
                "cv1" => conv::mri_convolve1d,
                "cvb" => conv::mri_convolve1d_byte,
                "cvs" => conv::mri_convolve1d_short,
                _ => conv::mri_convolve1d_int,
            };
            let r = f(g, a, b, &src, dst.as_mut(), &k, axis, sf, df)?;
            result(o, r, dst.as_ref());
        }
        "cvg" => {
            let mut src = get_vol(cx, t)?;
            let m = t.i();
            let mut dst = if m == 1 { Some(get_vol(cx, t)?) } else { None };
            let kv = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match m {
                2 => {
                    let r = conv::mri_convolve_gaussian(g, a, b, Pair::Same(&mut src), &kv)?;
                    same_result(o, r, &src);
                }
                _ => {
                    let p = match dst.as_mut() {
                        Some(d) => Pair::Given(&src, d),
                        None => Pair::New(&src),
                    };
                    let r = conv::mri_convolve_gaussian(g, a, b, p, &kv)?;
                    result(o, r, dst.as_ref());
                }
            }
        }
        "gsn" | "gs" => {
            let mut src = get_vol(cx, t)?;
            let m = t.i();
            let mut dst = if m == 1 { Some(get_vol(cx, t)?) } else { None };
            let args: Vec<f64> = if cmd == "gsn" {
                vec![dbl(t), dbl(t), dbl(t)]
            } else {
                vec![dbl(t), f64::from(t.i())]
            };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if m == 2 {
                let r = smooth(g, a, b, Pair::Same(&mut src), &args)?;
                same_result(o, r, &src);
            } else {
                let p = match dst.as_mut() {
                    Some(d) => Pair::Given(&src, d),
                    None => Pair::New(&src),
                };
                let r = smooth(g, a, b, p, &args)?;
                result(o, r, dst.as_ref());
            }
        }
        "mgs" => {
            let mut src = get_vol(cx, t)?;
            let mask = opt(cx, t)?;
            let m = t.i();
            let mut dst = if m == 1 { Some(get_vol(cx, t)?) } else { None };
            let s = dbl(t);
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if m == 2 {
                let r = conv::mri_masked_gaussian_smooth(
                    g,
                    a,
                    b,
                    Pair::Same(&mut src),
                    mask.as_ref(),
                    s,
                )?;
                same_result(o, r, &src);
            } else {
                let p = match dst.as_mut() {
                    Some(d) => Pair::Given(&src, d),
                    None => Pair::New(&src),
                };
                let r = conv::mri_masked_gaussian_smooth(g, a, b, p, mask.as_ref(), s)?;
                result(o, r, dst.as_ref());
            }
        }
        "mean" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let ws = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = conv::mri_mean(g, a, b, &src, dst.as_mut(), ws)?;
            result(o, r, dst.as_ref());
        }
        "std" | "stdnz" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let mean = get_vol(cx, t)?;
            let ws = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let f = if cmd == "std" {
                conv::mri_std
            } else {
                conv::mri_std_nonzero
            };
            let r = f(g, a, b, &src, dst.as_mut(), &mean, ws)?;
            result(o, r, dst.as_ref());
        }
        "red" | "red2" => {
            let mut src = get_volc(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "red" {
                conv::mri_reduce(g, a, b, &Environment::default(), &src, dst.as_mut())?
            } else {
                conv::mri_reduce2d(g, a, b, &mut src, dst.as_mut())?
            };
            result(o, r, dst.as_ref());
            hdr(o, &src);
        }
        "r1d" | "rsl" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let k = kern(t);
            let axis = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let f = if cmd == "r1d" {
                conv::mri_reduce1d
            } else {
                conv::mri_reduce_slice
            };
            let r = f(g, a, b, &src, dst.as_mut(), &k, axis)?;
            result(o, r, dst.as_ref());
        }
        "xsob" | "ysob" | "zsob" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let fr = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let f = match cmd {
                "xsob" => sobel::mri_x_sobel,
                "ysob" => sobel::mri_y_sobel,
                _ => sobel::mri_z_sobel,
            };
            let r = f(g, a, b, &src, dst.as_mut(), fr)?;
            result(o, r, dst.as_ref());
        }
        "sob" | "sobf" => {
            let src = get_vol(cx, t)?;
            let mut grad = opt(cx, t)?;
            let mut mag = opt(cx, t)?;
            let fr = if cmd == "sobf" { Some(t.i()) } else { None };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match fr {
                None => sobel::mri_sobel(g, a, b, &src, grad.as_mut(), mag.as_mut())?,
                Some(f) => sobel::mri_sobel_frame(g, a, b, &src, grad.as_mut(), mag.as_mut(), f)?,
            };
            result(o, r, grad.as_ref());
            if let Some(m) = &mag {
                full(o, Some(m));
            }
        }
        "mbv" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = classify::mri_mark_border_voxels(g, a, b, &src, dst.as_mut())?;
            result(o, r, dst.as_ref());
        }
        "bcv" => {
            let src = get_vol(cx, t)?;
            let lab = get_vol(cx, t)?;
            let (ws, x, y, z) = (t.i(), t.i(), t.i(), t.i());
            let (l, pw, pg) =
                classify::mri_border_classify_voxel(&mut cx.g, &mut cx.b, &src, &lab, ws, x, y, z)?;
            *o += &format!("{l} {:08x} {:08x} ", pw.to_bits(), pg.to_bits());
        }
        "rcl" => {
            let src = get_vol(cx, t)?;
            let mut lab = get_vol(cx, t)?;
            let m = t.i();
            let mut dst = if m == 1 { Some(get_vol(cx, t)?) } else { None };
            let (lo, hi, ws) = (t.f(), t.f(), t.i());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = match (m, dst.as_mut()) {
                (2, _) => Pair::Same(&mut lab),
                (_, Some(d)) => Pair::Given(&lab, d),
                _ => Pair::New(&lab),
            };
            let r = classify::mri_reclassify(g, a, b, &src, p, lo, hi, ws)?;
            *o += &format!("{r} ");
            full(o, Some(&lab));
            if let Some(d) = &dst {
                full(o, Some(d));
            }
        }
        "ccs" => {
            let t1 = get_vol(cx, t)?;
            let lab = get_vol(cx, t)?;
            let (lo, hi) = (t.f(), t.f());
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let s = classify::mri_compute_class_statistics(g, a, b, &t1, &lab, lo, hi)?;
            *o += &format!(
                "0 {:08x} {:08x} {:08x} {:08x} ",
                s.mean_wm.to_bits(),
                s.sigma_wm.to_bits(),
                s.mean_gm.to_bits(),
                s.sigma_gm.to_bits()
            );
        }
        "rmh" | "lcc6" | "lcc18" => {
            let mut v = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match cmd {
                "rmh" => cc::remove_holes(g, a, b, &mut v)?,
                "lcc6" => cc::get_largest_cc6(g, a, b, &mut v)?,
                _ => cc::get_largest_cc18(g, a, b, &mut v)?,
            }
            full(o, Some(&v));
        }
        "gf6" | "gf18" => {
            let v = get_vol(cx, t)?;
            let mut lab = get_vol(cx, t)?;
            let (label, x, y, z) = (t.i(), t.i(), t.i(), t.i());
            let f = if cmd == "gf6" {
                cc::grass_fire6
            } else {
                cc::grass_fire18
            };
            let r = f(&mut cx.g, &mut cx.b, &v, &mut lab, label, [x, y, z])?;
            *o += &format!(
                "{} {} {} {} {} {} {} ",
                r.size, r.min[0], r.max[0], r.min[1], r.max[1], r.min[2], r.max[2]
            );
            full(o, Some(&lab));
        }
        "div" | "lap" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "div" {
                sobel::mri_divergence(g, a, b, &src, dst.as_mut())?
            } else {
                sobel::mri_laplacian(g, a, b, &src, dst.as_mut())?
            };
            result(o, r, dst.as_ref());
        }
        _ => {
            let _ = (g, a, b);
            return Ok(false);
        }
    }
    Ok(true)
}

/// `gsn` (three deviations) or `gs` (a deviation and `norm`).
fn smooth(
    g: &mut Globals,
    a: &mut dyn Write,
    b: &mut dyn Write,
    p: Pair,
    args: &[f64],
) -> Result<Ret<Mri>, Fatal> {
    if args.len() == 3 {
        conv::mri_gaussian_smooth_ni(
            g,
            a,
            b,
            p,
            args[0],
            args[1],
            args[2],
            &conv::Smni::default(),
        )
    } else {
        conv::mri_gaussian_smooth(g, a, b, p, args[0], args[1] as i32)
    }
}

/// `put_result(r, src)` when the source was its own destination.
fn same_result(o: &mut String, r: Ret<Mri>, src: &Mri) {
    match r {
        Ret::New(m) => {
            *o += "0 ";
            full(o, Some(&m));
            full(o, Some(src));
        }
        Ret::Given => {
            *o += "1 ";
            full(o, Some(src));
        }
        Ret::Null => {
            *o += "0 ";
            full(o, None);
            full(o, Some(src));
        }
    }
}

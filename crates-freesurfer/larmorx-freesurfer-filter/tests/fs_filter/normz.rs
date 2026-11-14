// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's normalisation commands (`fsfilter.cpp`'s `norm2_case`,
//! `utils/mrinorm.cpp`'s intensity normalisation): their case generators and the port's answers.

use larmorx_freesurfer_core::histo::Histogram;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::random::set_random_seed;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::normalize as nz;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::label::{LABEL_COORDS_VOXEL, label_alloc};
use larmorx_freesurfer_io::mriio::mri_read;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::label_coords::Io as LabelIo;

use super::conv::{dx, full, get_volc, opt, result, same_result, small, vol};
use super::{Ctx, Gen, Toks, fx, get_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 10] = [
    "spl", "ninit", "npk", "gfc", "fcw", "afc", "bbi", "g3d", "abc", "ufc",
];

/// A volume's header tokens and voxels of type `ty` from `v` (one frame), voxel size `xs`.
fn vol_of(t: &mut Vec<String>, e: [i32; 3], ty: i32, xs: f32, v: &[f64]) {
    t.extend(e.map(|x| x.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    t.push(fx(xs));
    t.push(fx(1.0));
    t.push(fx(1.0));
    let vals: Vec<String> = v.iter().map(|&x| token(ty, x)).collect();
    t.extend(rle(&vals));
}

/// A voxel size: 1 mostly, now and then 0.5, 2 or 1.5; rarely one too small for the windows.
fn xsize(g: &mut Gen) -> f32 {
    if g.chance(30) {
        return g.pick(&[0.0, f32::NAN, 1e-3, -1.0]);
    }
    if g.chance(4) {
        g.pick(&[0.5, 2.0, 1.5, 0.9375])
    } else {
        1.0
    }
}

/// A tissue-like volume: white matter about `wm` in blocks, grey and CSF darker, background 0.
fn tissue(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, xs: f32) {
    let n = (e[0] * e[1] * e[2]).max(0) as usize;
    let wm = g.int(95, 125);
    let spread = g.pick(&[2i64, 4, 8, 15]);
    let mut v = vec![0.0f64; n];
    let (w, h) = (e[0].max(1) as usize, e[1].max(1) as usize);
    for (i, x) in v.iter_mut().enumerate() {
        let (xx, yy, zz) = (i % w, (i / w) % h, i / (w * h));
        let block = (xx / 3 + yy / 3 + zz / 2) % 4;
        *x = match block {
            0 | 1 => (wm + g.int(-spread, spread)) as f64,
            2 => (wm - 35 + g.int(-spread, spread)) as f64,
            _ => {
                if g.chance(3) {
                    0.0
                } else {
                    (wm - 70 + g.int(0, 20)) as f64
                }
            }
        };
        if g.chance(60) {
            *x = g.pick(&[0.0, 255.0, 300.0, -5.0]);
        }
    }
    let mut vals: Vec<String> = v.iter().map(|&x| token(ty, x)).collect();
    if ty == MRI_FLOAT {
        for s in vals.iter_mut() {
            if g.chance(80) {
                *s = fx(g.pick(&[f32::NAN, f32::INFINITY, 110.5, 3e9, -0.0]));
            }
        }
    }
    t.extend(e.map(|x| x.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    t.push(fx(xs));
    t.push(fx(1.0));
    t.push(fx(1.0));
    t.extend(rle(&vals));
}

fn tissue_type(g: &mut Gen) -> i32 {
    if g.chance(10) {
        g.pick(&[MRI_INT, MRI_SHORT])
    } else {
        g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_FLOAT])
    }
}

/// A control volume: bytes mostly (0, some 1, 128, 255), now and then of another type or larger.
fn ctrl_vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let ty = if g.chance(10) {
        g.pick(&[MRI_FLOAT, MRI_INT, MRI_SHORT])
    } else {
        MRI_UCHAR
    };
    let mut e2 = e;
    if g.chance(12) {
        let k = g.below(3) as usize;
        e2[k] = (e2[k] + g.pick(&[-1, 1])).max(1);
    }
    let n = (e2[0] * e2[1] * e2[2]) as usize;
    let p = g.pick(&[3u64, 6, 20]);
    let v: Vec<f64> = (0..n)
        .map(|_| {
            if g.chance(p) {
                g.pick(&[1.0, 1.0, 128.0, 255.0, 2.0])
            } else {
                0.0
            }
        })
        .collect();
    vol_of(t, e2, ty, 1.0, &v);
}

/// An optional control volume.
fn opt_ctrl(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], k: u64) {
    if g.chance(k) {
        t.push("1".into());
        ctrl_vol(g, t, e);
    } else {
        t.push("0".into());
    }
}

/// An optional mask of voxels that may not be control points.
fn opt_not(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    if g.chance(4) {
        t.push("1".into());
        let ty = if g.chance(3) { MRI_FLOAT } else { MRI_UCHAR };
        let n = (e[0] * e[1] * e[2]) as usize;
        let v: Vec<f64> = (0..n)
            .map(|_| if g.chance(5) { 1.0 } else { 0.0 })
            .collect();
        vol_of(t, e, ty, 1.0, &v);
    } else {
        t.push("0".into());
    }
}

/// Control points: a count, then voxel triples (now and then outside the volume).
fn pts(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    let n = if g.chance(3) { 0 } else { g.int(1, 6) };
    t.push(n.to_string());
    for _ in 0..n {
        for &m in &e {
            let v = if g.chance(12) {
                g.pick(&[-1i64, i64::from(m), -2])
            } else {
                g.int(0, i64::from(m) - 1)
            };
            t.push(v.to_string());
        }
    }
}

/// A destination by mode (0 none, 1 given, 2 the source): given ones of `ty`, sometimes of
/// another type (`other`) or extent.
fn dst_mode(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, other: i32) {
    let m = g.pick(&[0, 0, 1, 2]);
    t.push(m.to_string());
    if m == 1 {
        let mut e2 = e;
        if g.chance(10) {
            let k = g.below(3) as usize;
            e2[k] += 1;
        }
        let ty2 = if g.chance(6) { other } else { ty };
        vol(g, t, e2, ty2, 1);
    }
}

fn spl_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 6);
    let ty = g.pick(&[MRI_FLOAT, MRI_UCHAR, MRI_SHORT, MRI_SHORT, MRI_INT]);
    vol(g, t, e, ty, 1);
    dst_mode(g, t, e, ty, MRI_FLOAT);
    t.push(if g.chance(4) { "1" } else { "0" }.into());
    if g.chance(3) {
        t.push("1".into());
        t.push(g.pick(&["0.5", "2", "abc", "-1", "1e-3", "0"]).into());
    } else {
        t.push("0".into());
        t.push("-".into());
    }
    let n = if g.chance(20) {
        g.pick(&[0, 1, 82])
    } else {
        g.int(2, 8) as usize
    };
    t.push(n.to_string());
    let mut y = g.int(-6, 1) as f32;
    for _ in 0..n {
        t.push(fx(y));
        y += g.pick(&[1.0f32, 1.5, 2.0, 3.25]);
    }
    for _ in 0..n {
        let f = if g.chance(25) {
            g.pick(&[0.0f32, f32::NAN, -1.0, 40.0])
        } else {
            0.75 + g.int(0, 64) as f32 / 100.0
        };
        t.push(fx(f));
    }
}

fn ninit_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = [
        g.int(22, 36) as i32,
        g.int(22, 40) as i32,
        g.int(22, 36) as i32,
    ];
    let ys = g.pick(&[1.0f32, 1.0, 2.0, 0.5, 1.5]);
    let n = (e[0] * e[1] * e[2]) as usize;
    let c = [
        f64::from(e[0]) / 2.0 + g.int(-3, 3) as f64,
        f64::from(e[1]) / 2.0 + g.int(-4, 2) as f64,
        f64::from(e[2]) / 2.0 + g.int(-3, 3) as f64,
    ];
    let r = [
        f64::from(e[0]) * 0.35,
        f64::from(e[1]) * 0.38,
        f64::from(e[2]) * 0.35,
    ];
    let noise = g.chance(3);
    let v: Vec<f64> = (0..n)
        .map(|i| {
            let p = [
                (i % e[0] as usize) as f64,
                ((i / e[0] as usize) % e[1] as usize) as f64,
                (i / (e[0] * e[1]) as usize) as f64,
            ];
            let s: f64 = (0..3).map(|k| ((p[k] - c[k]) / r[k]).powi(2)).sum();
            if s <= 1.0 {
                100.0 + if noise { g.int(-20, 20) as f64 } else { 0.0 }
            } else if noise && g.chance(20) {
                g.int(0, 60) as f64
            } else {
                0.0
            }
        })
        .collect();
    t.extend(e.map(|x| x.to_string()));
    t.push(MRI_UCHAR.to_string());
    t.push("1".into());
    t.push(fx(1.0));
    t.push(fx(ys));
    t.push(fx(1.0));
    let vals: Vec<String> = v.iter().map(|&x| token(MRI_UCHAR, x)).collect();
    t.extend(rle(&vals));
    for _ in 0..3 {
        t.push(fx(g.int(-10, 10) as f32 * 0.5));
    }
    let xm = g.pick(&[0, 0, 0, 1, 1, 2]);
    t.push(xm.to_string());
    if xm != 0 {
        // m[c][r]: an affine map (column 3 the translation)
        let s = 1.0 + g.int(-10, 10) as f64 / 100.0;
        for col in 0..4 {
            for row in 0..4 {
                let v = if col == row && col < 3 {
                    s
                } else if col == 3 && row < 3 {
                    g.int(-8, 8) as f64 * 0.75
                } else if col == 3 && row == 3 {
                    1.0
                } else if g.chance(4) {
                    g.int(-5, 5) as f64 / 100.0
                } else {
                    0.0
                };
                t.push(dx(v));
            }
        }
    }
    t.push(if g.chance(4) { "1" } else { "0" }.into());
    let wa = if g.chance(3) {
        g.pick(&[3, 7, 10, 20, 50, -1])
    } else {
        0
    };
    let wb = if g.chance(3) {
        g.pick(&[2, 4, 9, 40, -3])
    } else {
        0
    };
    t.push(wa.to_string());
    t.push(wb.to_string());
    t.push(
        if g.chance(4) {
            g.pick(&[5, 8, -1, 11])
        } else {
            0
        }
        .to_string(),
    );
    t.push(if g.chance(4) { "100" } else { "0" }.into());
    t.push(fx(if g.chance(4) { 1.5 } else { 0.0 }));
}

fn npk_case(g: &mut Gen, t: &mut Vec<String>) {
    let mode = if g.chance(2) { 0 } else { 1 };
    t.push(mode.to_string());
    let above = g.pick(&[14, 14, 7, 9, 10, 20, 5]);
    let below = g.pick(&[9, 9, 4, 0, 2]);
    let nw = above + below;
    t.push(above.to_string());
    t.push(below.to_string());
    t.push(g.pick(&[110, 110, 100]).to_string());
    t.push(fx(g.pick(&[2.0f32, 2.0, 1.0, 0.0, 4.0])));
    t.push(fx(g.pick(&[0.0f32, 0.0, 0.5, 2.0, 5.0])));
    t.push(g.int(0, i64::from(nw)).to_string());
    let nb = g.pick(&[48i64, 48, 32, 7]);
    let mut peak = g.int(20, i64::from(nb as i32) - 5);
    for i in 0..nw {
        t.push((g.int(-4, 4) + 5 * i64::from(i)).to_string());
        t.push(g.pick(&[10, 10, 9]).to_string());
        t.push(nb.to_string());
        t.push(fx(1.0));
        t.push(fx(0.0));
        t.push(fx((nb - 1) as f32));
        if g.chance(4) {
            peak = (peak + g.int(-6, 6)).clamp(2, nb - 2);
        }
        let empty = g.chance(10);
        for b in 0..nb {
            t.push(fx(b as f32));
            let c = if empty {
                0
            } else {
                (200 - (b - peak) * (b - peak) * g.int(4, 12)).max(0) + g.int(0, 20)
            };
            t.push(fx(c as f32));
        }
    }
    for k in 0..2 {
        for i in 0..80 {
            let v = if k == 0 {
                i as f32 * 5.0 + 2.5
            } else {
                100.0 + (i % 7) as f32 * 3.0
            };
            t.push(fx(if g.chance(30) { f32::NAN } else { v }));
        }
    }
}

fn gfc_fcw_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    let e = small(g, 10);
    let ty = tissue_type(g);
    let xs = xsize(g);
    tissue(g, t, e, ty, xs);
    t.push(fx(g.pick(&[0.0f32, 110.0, 110.0, 100.0])));
    t.push(fx(g.pick(&[25.0f32, 10.0, 12.5, 100.0, 0.0])));
    t.push(fx(g.pick(&[10.0f32, 5.0, 80.0, 0.0, 12.5])));
    opt_ctrl(g, t, e, 4);
    if cmd == "fcw" {
        t.push(fx(g.pick(&[3.0f32, 2.0, 0.0, 1.5])));
        t.push(g.pick(&[0, 1, 2]).to_string());
        opt_not(g, t, e);
    } else {
        opt_not(g, t, e);
        pts(g, t, e);
    }
}

fn afc_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 6);
    ctrl_vol(g, t, e);
    t.push(g.pick(&[255, 255, 1, 128, 300, -1]).to_string());
    if g.chance(2) {
        t.push("1".into());
        let mut e2 = e;
        if g.chance(10) {
            let k = g.below(3) as usize;
            e2[k] = (e2[k] - 1).max(1);
        }
        let ty = g.pick(&[MRI_UCHAR, MRI_FLOAT, MRI_SHORT]);
        let n = (e2[0] * e2[1] * e2[2]) as usize;
        let v: Vec<f64> = (0..n)
            .map(|_| {
                if g.chance(3) {
                    g.pick(&[0.0, -3.0, 0.0])
                } else {
                    g.int(1, 200) as f64
                }
            })
            .collect();
        vol_of(t, e2, ty, 1.0, &v);
    } else {
        t.push("0".into());
    }
    pts(g, t, e);
}

fn bbi_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 7);
    let ty = g.pick(&[MRI_FLOAT, MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT]);
    tissue(g, t, e, ty, 1.0);
    let n = (e[0] * e[1] * e[2]) as usize;
    let p = g.pick(&[4u64, 8, 30]);
    let v: Vec<f64> = (0..n)
        .map(|_| if g.chance(p) { 1.0 } else { 0.0 })
        .collect();
    vol_of(t, e, MRI_UCHAR, 1.0, &v);
    t.push(fx(g.pick(&[0.0f32, 1.0, 2.0, 0.5, 8.0])));
}

fn g3d_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 8);
    let ty = tissue_type(g);
    let xs = if g.chance(5) { 0.5 } else { 1.0 };
    tissue(g, t, e, ty, xs);
    if g.chance(4) {
        t.push("1".into());
        let n = (e[0] * e[1] * e[2]) as usize;
        let v: Vec<f64> = (0..n)
            .map(|_| {
                if g.chance(15) {
                    0.0
                } else {
                    g.int(60, 140) as f64
                }
            })
            .collect();
        vol_of(t, e, g.pick(&[MRI_FLOAT, MRI_UCHAR]), 1.0, &v);
    } else {
        t.push("0".into());
    }
    t.push(fx(g.pick(&[0.0f32, 110.0, 110.0, 100.0])));
    dst_mode(g, t, e, ty, MRI_UCHAR);
    t.push(fx(g.pick(&[25.0f32, 12.5, 10.0])));
    t.push(fx(g.pick(&[10.0f32, 5.0, 2.5])));
    t.push(if g.chance(4) { "1" } else { "0" }.into());
    t.push(fx(g.pick(&[0.0f32, 2.0, 8.0, 1.0])));
    opt_not(g, t, e);
    pts(g, t, e);
}

fn abc_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 6);
    let ty = g.pick(&[MRI_FLOAT, MRI_UCHAR, MRI_SHORT, MRI_INT]);
    vol(g, t, e, ty, 1);
    let bty = g.pick(&[MRI_FLOAT, MRI_FLOAT, MRI_UCHAR, MRI_SHORT]);
    let mut e2 = e;
    if g.chance(10) {
        let k = g.below(3) as usize;
        e2[k] = (e2[k] - 1).max(1);
    }
    let n2 = (e2[0] * e2[1] * e2[2]) as usize;
    let v: Vec<f64> = (0..n2)
        .map(|_| {
            if g.chance(20) {
                0.0
            } else {
                g.int(50, 150) as f64
            }
        })
        .collect();
    vol_of(t, e2, bty, 1.0, &v);
    dst_mode(g, t, e, ty, MRI_FLOAT);
    t.push(fx(g.pick(&[110.0f32, 110.0, 0.0, -1.0, f32::NAN])));
}

fn hex(b: &[u8]) -> String {
    let mut s = String::from("h");
    for c in b {
        s += &format!("{c:02x}");
    }
    s
}

fn ufc_case(g: &mut Gen, t: &mut Vec<String>) {
    let e = small(g, 8);
    let n = (e[0] * e[1] * e[2]) as usize;
    let v: Vec<f64> = (0..n)
        .map(|_| {
            if g.chance(4) {
                0.0
            } else {
                g.int(1, 150) as f64
            }
        })
        .collect();
    vol_of(t, e, MRI_UCHAR, 1.0, &v);
    for _ in 0..3 {
        t.push(fx(g.int(-4, 4) as f32 * 0.5));
    }
    if g.chance(3) {
        t.push("1".into());
        t.push(g.pick(&["10", "0", "abc", "1e2", "-5", "40"]).into());
    } else {
        t.push("0".into());
        t.push("-".into());
    }
    let mut f = String::new();
    let np = g.int(0, 7);
    for _ in 0..np {
        if g.chance(8) {
            f += "# a comment\n";
        }
        let c: Vec<String> = (0..3)
            .map(|k| {
                let m = f64::from(e[k]);
                format!("{}", (g.int(-2, (m as i64) * 2 + 2) as f64 / 2.0) - m / 2.0)
            })
            .collect();
        f += &format!("{} {} {}\n", c[0], c[1], c[2]);
    }
    if g.chance(6) {
        f += "useRealRAS 1\n";
    }
    if g.chance(2) {
        f += &format!("info\nnumpoints {np}\nuseRealRAS 0\n");
    }
    t.push(hex(f.as_bytes()));
}

pub fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "spl" => spl_case(g, t),
        "ninit" => ninit_case(g, t),
        "npk" => npk_case(g, t),
        "gfc" | "fcw" => gfc_fcw_case(g, t, cmd),
        "afc" => afc_case(g, t),
        "bbi" => bbi_case(g, t),
        "g3d" => g3d_case(g, t),
        "abc" => abc_case(g, t),
        "ufc" => ufc_case(g, t),
        c => panic!("unknown command {c}"),
    }
}

/// `set_pts`: the state's control points from a label in voxel coordinates.
fn set_pts(cx: &mut Ctx, t: &mut Toks, st: &mut nz::NormState, mri: &mut Mri) -> Result<(), Fatal> {
    let n = t.i();
    let env = Environment::default();
    let mut l = label_alloc(&mut cx.g, &env, &mut cx.b, n.max(1), None, None)?;
    l.coords = LABEL_COORDS_VOXEL;
    l.n_points = n;
    for i in 0..n.max(0) as usize {
        l.lv[i].x = t.i() as f32;
        l.lv[i].y = t.i() as f32;
        l.lv[i].z = t.i() as f32;
    }
    let mut io = LabelIo {
        out: &mut cx.a,
        err: &mut cx.b,
        env: &env,
    };
    nz::mri_3d_use_label_control_points(&mut cx.g, &mut io, st, mri, &l)?;
    Ok(())
}

fn unhex(s: &str) -> Vec<u8> {
    let s = s.strip_prefix('h').expect("a hex token");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

static UFC_DIRS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The port's answer for one of this module's commands; `false` for another command.
pub fn run(cx: &mut Ctx, cmd: &str, t: &mut Toks, o: &mut String) -> Result<bool, Fatal> {
    let mut st = nz::NormState::default();
    match cmd {
        "spl" => {
            let mut src = get_vol(cx, t)?;
            let mode = t.i();
            let mut dst = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let fm = t.i();
            let rm = t.i();
            let rs = t.next();
            let env = if rm != 0 {
                Environment::from_pairs([("RAN", rs)])
            } else {
                Environment::default()
            };
            set_random_seed(&mut cx.g, 1234);
            let n = t.i();
            let len = (n.max(0) as usize).max(80);
            let mut inp = vec![0.0f32; len];
            let mut outp = vec![0.0f32; len];
            for x in inp.iter_mut().take(n.max(0) as usize) {
                *x = t.f();
            }
            for x in outp.iter_mut().take(n.max(0) as usize) {
                *x = t.f();
            }
            let mut field: Option<Mri> = None;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let fld = if fm != 0 { Some(&mut field) } else { None };
            match mode {
                2 => {
                    let r = nz::mri_spline_normalize(
                        g,
                        a,
                        b,
                        &env,
                        Pair::Same(&mut src),
                        fld,
                        &inp,
                        &outp,
                        n,
                    )?;
                    same_result(o, r, &src);
                }
                _ => {
                    let p = match dst.as_mut() {
                        Some(d) => Pair::Given(&src, d),
                        None => Pair::New(&src),
                    };
                    let r = nz::mri_spline_normalize(g, a, b, &env, p, fld, &inp, &outp, n)?;
                    result(o, r, dst.as_ref());
                }
            }
            if fm != 0 {
                full(o, field.as_ref());
            }
        }
        "ninit" => {
            let mut m = get_volc(cx, t)?;
            let xm = t.i();
            if xm != 0 {
                let mut tr = [[0.0f64; 4]; 4];
                for c in tr.iter_mut() {
                    for r in c.iter_mut() {
                        *r = super::conv::dbl(t);
                    }
                }
                if xm == 1 {
                    m.linear_transform = Some(tr);
                }
                m.inverse_linear_transform = Some([[0.0; 4]; 4]);
            }
            let env = if t.i() != 0 {
                Environment::from_pairs([("NO_AVERAGE305", "1")])
            } else {
                Environment::default()
            };
            let (above, below, ws, des) = (t.i(), t.i(), t.i(), t.i());
            let sg = t.f();
            let mut mni = nz::Mni::default();
            let r = nz::mri_norm_init(
                &mut cx.g, &mut cx.a, &mut cx.b, &env, &m, &mut mni, above, below, ws, des, sg,
            )?;
            *o += &format!(
                "{r} {} {} {} {} ",
                mni.windows_above_t0,
                mni.windows_below_t0,
                mni.desired_wm_value,
                fx(mni.smooth_sigma)
            );
            let nw = mni.windows_above_t0.wrapping_add(mni.windows_below_t0);
            for i in 0..nw.clamp(0, nz::MAX_SPLINE_POINTS as i32) as usize {
                let r = mni.regions[i];
                *o += &format!("{} {} {} {} {} {} ", r.x, r.y, r.z, r.dx, r.dy, r.dz);
            }
        }
        "npk" => {
            let mode = t.i();
            let mut mni = nz::Mni {
                windows_above_t0: t.i(),
                windows_below_t0: t.i(),
                desired_wm_value: t.i(),
                smooth_sigma: t.f(),
                max_gradient: t.f(),
                ..nz::Mni::default()
            };
            let np = t.i();
            let nw = mni.windows_above_t0 + mni.windows_below_t0;
            for i in 0..nw as usize {
                mni.regions[i].y = t.i();
                mni.regions[i].dy = t.i();
                let nb = t.i();
                let (bin_size, min, max) = (t.f(), t.f(), t.f());
                let mut bins = vec![0.0f32; nb as usize];
                let mut counts = vec![0.0f32; nb as usize];
                for k in 0..nb as usize {
                    bins[k] = t.f();
                    counts[k] = t.f();
                }
                mni.histograms[i] = Histogram {
                    max_bins: nb,
                    nbins: nb,
                    bins,
                    counts,
                    bin_size,
                    min,
                    max,
                };
            }
            let mut inp = [0.0f32; 80];
            let mut outp = [0.0f32; 80];
            for x in inp.iter_mut() {
                *x = t.f();
            }
            for x in outp.iter_mut() {
                *x = t.f();
            }
            let mut p = nz::Peaks {
                inputs: &mut inp,
                outputs: &mut outp,
            };
            let r = if mode == 0 {
                nz::mri_norm_find_peaks(&mut cx.g, &mut cx.b, &mni, &mut p)?
            } else {
                nz::mri_norm_check_peaks(&mut cx.g, &mut cx.b, &mni, &mut p, np)?
            };
            *o += &format!("{r} ");
            for v in inp.iter().chain(outp.iter()) {
                *o += &fx(*v);
                *o += " ";
            }
        }
        "gfc" | "fcw" => {
            let mut src = get_vol(cx, t)?;
            let (wm, ab, be) = (t.f(), t.f(), t.f());
            let mut ctrl = opt(cx, t)?;
            if cmd == "fcw" {
                let wh = t.f();
                let dm = t.i();
                let nc = opt(cx, t)?;
                let dbg = match dm {
                    0 => None,
                    1 => Some(""),
                    _ => Some("dbg"),
                };
                let (r, n) = nz::mri_norm_find_control_points_in_window(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &src,
                    wm,
                    ab,
                    be,
                    ctrl.as_mut(),
                    wh,
                    dbg,
                    nc.as_ref(),
                )?;
                // FreeSurfer's n keeps the caller's -7 when the function ends early
                *o += &format!("{} ", if r.is_null() { -7 } else { n });
                result(o, r, ctrl.as_ref());
            } else {
                let nc = opt(cx, t)?;
                set_pts(cx, t, &mut st, &mut src)?;
                let r = nz::mri_norm_gently_find_control_points(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &mut st,
                    &src,
                    wm,
                    ab,
                    be,
                    ctrl.as_mut(),
                    nc.as_ref(),
                )?;
                result(o, r, ctrl.as_ref());
            }
        }
        "afc" => {
            let mut ctrl = get_vol(cx, t)?;
            let value = t.i();
            let m = opt(cx, t)?;
            set_pts(cx, t, &mut st, &mut ctrl)?;
            for _ in 0..2 {
                let n = nz::mri_norm_add_file_control_points(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &mut st,
                    &mut ctrl,
                    value,
                    m.as_ref(),
                )?;
                *o += &format!("{n} ");
            }
            full(o, Some(&ctrl));
        }
        "bbi" => {
            let src = get_vol(cx, t)?;
            let mut ctrl = get_vol(cx, t)?;
            let sg = t.f();
            let b = nz::mri_build_bias_image(&mut cx.g, &mut cx.a, &mut cx.b, &src, &mut ctrl, sg)?;
            full(o, Some(&b));
            full(o, Some(&ctrl));
        }
        "g3d" => {
            let mut src = get_vol(cx, t)?;
            let bias = opt(cx, t)?;
            let wm = t.f();
            let mode = t.i();
            let mut norm = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (ab, be) = (t.f(), t.f());
            let of = t.i();
            let sg = t.f();
            let nc = opt(cx, t)?;
            set_pts(cx, t, &mut st, &mut src)?;
            let env = Environment::default();
            let d = match (mode, norm.as_mut()) {
                (2, _) => nz::Dst::Same,
                (_, Some(n)) => nz::Dst::Given(n),
                _ => nz::Dst::New,
            };
            let r = nz::mri_3d_gentle_normalize(
                &mut cx.g,
                &mut cx.a,
                &mut cx.b,
                &env,
                None,
                &mut st,
                &mut src,
                bias.as_ref(),
                wm,
                d,
                ab,
                be,
                of,
                sg,
                nc.as_ref(),
            )?;
            if mode == 2 {
                same_result(o, r, &src);
            } else {
                result(o, r, norm.as_ref());
            }
            full(o, Some(&src));
        }
        "abc" => {
            let mut src = get_vol(cx, t)?;
            let bias = get_vol(cx, t)?;
            let mode = t.i();
            let mut dst = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let tv = t.f();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            if mode == 2 {
                let r = nz::mri_apply_bias_correction_same_geometry(
                    g,
                    a,
                    b,
                    Pair::Same(&mut src),
                    &bias,
                    tv,
                )?;
                same_result(o, r, &src);
            } else {
                let p = match dst.as_mut() {
                    Some(d) => Pair::Given(&src, d),
                    None => Pair::New(&src),
                };
                let r = nz::mri_apply_bias_correction_same_geometry(g, a, b, p, &bias, tv)?;
                result(o, r, dst.as_ref());
            }
        }
        "g3df" => {
            let path = t.next();
            let env = Environment::default();
            let mut src = mri_read(
                &mut cx.g,
                &mut cx.a,
                &mut cx.b,
                &env,
                None,
                path,
                MRI_VOLUME_TYPE_UNKNOWN,
                true,
                -1,
                -1,
            )?
            .expect("the volume");
            let (ab, be, sg) = (t.f(), t.f(), t.f());
            let r = nz::mri_3d_gentle_normalize(
                &mut cx.g,
                &mut cx.a,
                &mut cx.b,
                &env,
                None,
                &mut st,
                &mut src,
                None,
                110.0,
                nz::Dst::Same,
                ab,
                be,
                0,
                sg,
                None,
            )?;
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for &b in src.chunk.as_ref().expect("voxels") {
                h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            *o += &format!(
                "{} {} {} {} {} {h:016x} ",
                i32::from(matches!(r, larmorx_freesurfer_core::ret::Ret::Given)),
                src.vg.width,
                src.vg.height,
                src.vg.depth,
                src.mtype
            );
        }
        "ufc" => {
            let mut m = get_volc(cx, t)?;
            let tm = t.i();
            let ts = t.next();
            let bytes = unhex(t.next());
            let k = UFC_DIRS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("fs-filter-ufc-{}-{k}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch directory");
            std::fs::write(dir.join("cp.dat"), &bytes).expect("the control point file");
            let env = if tm != 0 {
                Environment::from_pairs([("FS_THRESH_CTRL", ts)])
            } else {
                Environment::default()
            };
            let res = (|| -> Result<(), Fatal> {
                let r = nz::mri_3d_use_file_control_points(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &env,
                    Some(&dir),
                    &mut st,
                    &mut m,
                    "cp.dat",
                )?;
                *o += &format!("{r} ");
                let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
                let mut c = mri_alloc(&mut cx.g, &mut cx.a, &mut cx.b, w, h, d, MRI_UCHAR)?;
                let n = nz::mri_norm_add_file_control_points(
                    &mut cx.g, &mut cx.a, &mut cx.b, &mut st, &mut c, 255, None,
                )?;
                *o += &format!("{n} ");
                full(o, Some(&c));
                Ok(())
            })();
            let edited = std::fs::read(dir.join("control.edited.dat")).ok();
            let _ = std::fs::remove_dir_all(&dir);
            res?;
            match edited {
                None => *o += "NOFILE ",
                Some(e) => {
                    let cut = e
                        .windows(10)
                        .position(|w| w == b"written by")
                        .unwrap_or(e.len());
                    *o += &hex(&e[..cut]);
                    *o += " ";
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

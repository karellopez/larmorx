// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's segment commands (`fsfilter.cpp`'s `seg_case`, `utils/mrisegment.cpp`):
//! their case generators and the port's answers.

use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_filter::segment as sg;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc_sequence,
};

use super::conv::{opt, result, small};
use super::{Ctx, Gen, Toks, fx, get_vol, put_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 3] = ["seg", "mseg", "salloc"];

/// The light operations (no volume), for the large pattern volumes.
const LIGHT: [&str; 7] = ["M", "N", "A", "R", "C", "G", "L"];
const ALL: [&str; 13] = [
    "D", "T", "R", "C", "I", "V", "F", "M", "N", "A", "L", "G", "E",
];

fn vol_of(t: &mut Vec<String>, e: [i32; 3], ty: i32, v: &[f64]) {
    t.extend(e.map(|x| x.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    t.push(fx(1.0));
    t.push(fx(1.0));
    t.push(fx(1.0));
    let vals: Vec<String> = v.iter().map(|&x| token(ty, x)).collect();
    t.extend(rle(&vals));
}

/// A volume of small values (0 mostly) with runs, of a random type (NaN and -0 for floats).
fn values(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], dense: bool) {
    let ty = if g.chance(4) {
        g.pick(&[MRI_FLOAT, MRI_SHORT, MRI_INT])
    } else {
        MRI_UCHAR
    };
    let n = (e[0] * e[1] * e[2]) as usize;
    let p = if dense { 2 } else { 3 };
    let mut v = vec![0.0f64; n];
    let mut i = 0;
    while i < n {
        let val = if g.chance(p) {
            0.0
        } else {
            g.pick(&[1.0, 1.0, 2.0, 3.0, 255.0])
        };
        let run = g.int(1, 4) as usize;
        for x in v.iter_mut().skip(i).take(run) {
            *x = val;
        }
        i += run;
    }
    if ty == MRI_FLOAT {
        t.extend(e.map(|x| x.to_string()));
        t.push(ty.to_string());
        t.push("1".into());
        let xs = g.pick(&[1.0f32, 0.5, 2.0]);
        t.push(fx(xs));
        t.push(fx(1.0));
        t.push(fx(g.pick(&[1.0f32, 1.5])));
        let vals: Vec<String> = v
            .iter()
            .map(|&x| {
                if g.chance(12) {
                    fx(g.pick(&[f32::NAN, -0.0, 2.5, -1.0, 1.5]))
                } else {
                    fx(x as f32)
                }
            })
            .collect();
        t.extend(rle(&vals));
    } else {
        vol_of(t, e, ty, &v);
    }
}

fn opt_vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    if g.chance(2) {
        t.push("0".into());
    } else {
        t.push("1".into());
        values(g, t, e, false);
    }
}

pub fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    if cmd == "salloc" {
        t.push(g.pick(&[0, 1, 3, 7, 2, -1]).to_string());
        t.push(g.pick(&[5, 5, 0, 1, -2]).to_string());
        return;
    }
    let big = g.chance(10);
    let e;
    if big {
        t.push("1".into());
        let kind = g.int(0, 2);
        let (w, h, d, plo, phi, zsplit) = match kind {
            // a percolating half and a sparse half (thousands of segments)
            0 => (
                g.int(40, 64),
                g.int(40, 64),
                g.int(16, 40),
                g.pick(&[550, 600, 180]),
                g.pick(&[180, 150, 250]),
                g.int(4, 20),
            ),
            // a checkerboard under a slab (above 32,767 segments now and then)
            1 => {
                let d = g.pick(&[8, 21, 22]);
                (64, 64, d, -1, 1000, d - g.int(1, 2))
            }
            // a pillar beside a checkerboard: MRImaxsegment removes small segments mid-scan
            _ => (64, 64, g.pick(&[6, 7]), -2, 0, g.pick(&[8, 6])),
        };
        e = [w as i32, h as i32, d as i32];
        for x in [w, h, d, plo, phi, zsplit] {
            t.push(x.to_string());
        }
        t.push(g.int(0, 1_000_000).to_string());
    } else {
        t.push("0".into());
        e = if g.chance(5) {
            small(g, 12)
        } else {
            small(g, 7)
        };
        values(g, t, e, true);
    }
    let (lo, hi) = g.pick(&[
        (1.0f32, 1.0f32),
        (1.0, 3.0),
        (1.0, 255.0),
        (2.0, 255.0),
        (0.5, 1.5),
        (-1.0, 0.5),
        (0.0, 0.0),
        (f32::NAN, 1.0),
        (3.0, 1.0),
    ]);
    t.push(fx(lo));
    t.push(fx(hi));
    let nops = g.int(0, 5);
    t.push(nops.to_string());
    for _ in 0..nops {
        let op = if big { g.pick(&LIGHT) } else { g.pick(&ALL) };
        t.push(op.into());
        match op {
            "D" => values(g, t, e, true),
            "T" => {
                values(g, t, e, true);
                values(g, t, e, false);
                t.push(g.pick(&[0, 1, 2, -5]).to_string());
                t.push(g.pick(&[1, 3, 255, 0]).to_string());
            }
            "R" => t.push(g.pick(&[0, 1, 2, 3, 6, 100]).to_string()),
            "I" => {
                t.push(g.pick(&[-1, 0, 1, 2, 7]).to_string());
                opt_vol(g, t, e);
            }
            "V" => {
                t.push(if g.chance(25) {
                    "100000".to_owned()
                } else {
                    g.pick(&[0, 1, 2]).to_string()
                });
                values(g, t, e, false);
                t.push(fx(g.pick(&[7.0f32, -2.5, f32::NAN, 0.0])));
            }
            "F" => {
                t.push(g.pick(&[-1, 0, 1, 2, 9]).to_string());
                opt_vol(g, t, e);
                t.push(fx(g.pick(&[7.0f32, -2.5, 300.0, 0.0])));
            }
            "G" => t.push(g.pick(&[0, 1, 2]).to_string()),
            "E" => {
                t.push(g.pick(&[1, 2, 3, 6]).to_string());
                values(g, t, e, false);
            }
            _ => {}
        }
    }
}

/// `get_segvol`: a case volume, or a pattern of ones and zeros (see `fsfilter.cpp`).
fn get_segvol(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    if t.i() == 0 {
        return get_vol(cx, t);
    }
    let (w, h, d, plo, phi, zsplit) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
    let seed: u32 = t.next().parse().expect("seed");
    let mut m = mri_alloc_sequence(&mut cx.g, &mut cx.a, &mut cx.b, w, h, d, MRI_UCHAR, 1)?;
    m.vg.xsize = 1.0;
    m.vg.ysize = 1.0;
    m.vg.zsize = 1.0;
    let c = m.chunk.as_mut().expect("voxels");
    let wh = (w as usize) * (h as usize);
    for (i, b) in c.iter_mut().enumerate().take(wh * d as usize) {
        let z = (i / wh) as i32;
        let y = ((i % wh) / w as usize) as i32;
        let xx = (i % w as usize) as i32;
        let on = match plo {
            -1 if z < zsplit => (xx + y + z) % 2 == 0,
            -2 => xx >= w - zsplit || (xx + y + z) % 2 == 0,
            _ => {
                let mut x = (i as u32)
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(seed.wrapping_mul(40503));
                x ^= x >> 13;
                x = x.wrapping_mul(0x5bd1_e995);
                x ^= x >> 15;
                let p = if z < zsplit { plo } else { phi };
                ((x % 1000) as i32) < p
            }
        };
        *b = u8::from(on);
    }
    Ok(m)
}

fn put_seg(o: &mut String, s: &sg::MriSegmentation) {
    let mut t = String::new();
    t += &format!("{} {} ", s.nsegments, s.max_segments());
    for m in &s.segments {
        t += &format!(
            "{} {:08x} {} {} {:08x} {:08x} {:08x} {} {} {} {} {} {} {} ",
            m.nvoxels(),
            m.area.to_bits(),
            m.label,
            m.found,
            m.cx.to_bits(),
            m.cy.to_bits(),
            m.cz.to_bits(),
            m.x0,
            m.x1,
            m.y0,
            m.y1,
            m.z0,
            m.z1,
            m.ignore
        );
        for v in &m.voxels {
            t += &format!("{} {} {} ", v.x, v.y, v.z);
        }
    }
    if t.len() > 4000 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in t.as_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        *o += &format!("H{h:016x} ");
    } else {
        *o += &t;
    }
}

/// The port's answer for one of this module's commands; `false` for another command.
pub fn run(cx: &mut Ctx, cmd: &str, t: &mut Toks, o: &mut String) -> Result<bool, Fatal> {
    if cmd == "salloc" {
        let (ms, mv) = (t.i(), t.i());
        let s = sg::mri_segment_alloc(&mut cx.g, &mut cx.b, ms, mv)?;
        put_seg(o, &s);
        return Ok(true);
    }
    if cmd != "seg" && cmd != "mseg" {
        return Ok(false);
    }
    let vol = get_segvol(cx, t)?;
    let (lo, hi) = (t.f(), t.f());
    let nops = t.i();
    let mut s = if cmd == "seg" {
        sg::mri_segment(&mut cx.g, &mut cx.a, &mut cx.b, &vol, lo, hi)?
    } else {
        sg::mri_max_segment(&mut cx.g, &mut cx.a, &mut cx.b, &vol, lo, hi, &mut |_| 1000)?
    };
    for _ in 0..nops {
        let op = t.next();
        *o += op;
        *o += " ";
        match op {
            "D" => {
                let m = get_vol(cx, t)?;
                let r = sg::mri_segment_dilate(&mut cx.g, &mut cx.a, &mut cx.b, &mut s, &m)?;
                *o += &format!("{r} ");
            }
            "T" => {
                let bin = get_vol(cx, t)?;
                let th = get_vol(cx, t)?;
                let (l, h) = (t.i(), t.i());
                let r = sg::mri_segment_dilate_threshold(
                    &mut cx.g, &mut cx.a, &mut cx.b, &mut s, &bin, &th, l, h,
                )?;
                *o += &format!("{r} ");
            }
            "R" => *o += &format!("{} ", sg::mri_remove_small_segments(&mut s, t.i())),
            "C" => *o += &format!("{} ", sg::mri_compact_segments(&mut s)),
            "I" => {
                let k = t.i();
                let mut dst = opt(cx, t)?;
                let r = sg::mri_segment_to_image(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &vol,
                    dst.as_mut(),
                    &s,
                    k,
                )?;
                result(o, r, dst.as_ref());
            }
            "V" => {
                let k = t.i();
                let mut m = get_vol(cx, t)?;
                let val = t.f();
                let r = sg::mri_set_segment_value(&mut cx.g, &mut cx.b, &mut m, &s, k, val)?;
                *o += &format!("{r} ");
                put_vol(o, &m);
            }
            "F" => {
                let k = t.i();
                let mut dst = opt(cx, t)?;
                let f = t.f();
                let r = sg::mri_segment_fill(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &s,
                    &vol,
                    k,
                    dst.as_mut(),
                    f,
                )?;
                result(o, r, dst.as_ref());
            }
            "M" => *o += &format!("{} ", sg::mri_segment_max(&s)),
            "N" => *o += &format!("{} ", sg::mri_find_max_segment_number(&s)),
            "A" => *o += &format!("{} ", sg::mri_max_segment_area(&s)),
            "L" => *o += &format!("{} ", sg::mri_segment_clear_ignore_flags(&mut s)),
            "G" => {
                let k = t.i();
                if k >= 0 && k < s.max_segments() {
                    s.segments[k as usize].ignore = 1;
                }
            }
            "E" => {
                let mn = t.i();
                let mut m = get_vol(cx, t)?;
                let r =
                    sg::mri_erase_small_segments(&mut cx.g, &mut cx.a, &mut cx.b, &s, &mut m, mn)?;
                *o += &format!("{r} ");
                put_vol(o, &m);
            }
            x => panic!("unknown segment operation {x}"),
        }
    }
    put_seg(o, &s);
    Ok(true)
}

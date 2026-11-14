// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's frame commands (`fsfilter.cpp`'s `fmri_case`, `utils/fmriutils.cpp`):
//! their case generators and the port's answers.

use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_filter::fmri;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR};

use super::conv::{any_type, dbl, dx, full, opt, opt_dst, result, small, vol};
use super::{Ctx, Gen, Toks, fx, get_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 21] = [
    "fmm", "fcov", "far1", "fskip", "fdrop", "ffrm", "vmax", "vmin", "conj", "vmaxi", "fmean",
    "fmed", "fsum", "fsub", "fext", "asl", "f2m", "m2f", "scm", "fcum", "fnorm",
];

fn nframes(g: &mut Gen) -> i32 {
    g.pick(&[1, 2, 3, 3, 4, 5, 6])
}

/// A matrix weight: small multiples, now and then a NaN, an infinity or -0.
fn weight(g: &mut Gen) -> f32 {
    if g.chance(30) {
        return g.pick(&[f32::NAN, f32::from_bits(0x7fc0_0099), f32::INFINITY, -0.0]);
    }
    g.int(-16, 16) as f32 / 4.0
}

/// A matrix's tokens: rows, columns, then its elements row by row.
fn matrix(g: &mut Gen, t: &mut Vec<String>, rows: i32, cols: i32) {
    t.push(rows.to_string());
    t.push(cols.to_string());
    for _ in 0..rows.max(0) * cols.max(0) {
        t.push(fx(weight(g)));
    }
}

/// A mask of extent `e` whose count of voxels at 0.5 or more is returned (uchar 0/1, or float
/// with values about 0.5).
fn mask(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) -> i32 {
    let ty = if g.chance(3) { MRI_FLOAT } else { MRI_UCHAR };
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2]).max(0) as usize;
    let empty = g.chance(12);
    let mut count = 0;
    let vals: Vec<String> = (0..n)
        .map(|_| {
            let v: f32 = if empty {
                0.0
            } else if ty == MRI_FLOAT && g.chance(4) {
                g.pick(&[0.5, 0.49, 0.75, -1.0, f32::NAN])
            } else if g.chance(3) {
                0.0
            } else {
                1.0
            };
            if v >= 0.5 || v.is_nan() {
                count += 1;
            }
            token(ty, f64::from(v))
        })
        .collect();
    t.extend(rle(&vals));
    count
}

/// An optional mask: of the extent `e`, now and then of another one.
fn opt_mask(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    if g.chance(2) {
        t.push("0".into());
        return;
    }
    t.push("1".into());
    let mut e2 = e;
    if g.chance(10) {
        let k = g.below(3) as usize;
        e2[k] += 1;
    }
    mask(g, t, e2);
}

/// A source series: float mostly.
fn series(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], nf: i32) {
    let ty = if g.chance(3) { any_type(g) } else { MRI_FLOAT };
    vol(g, t, e, ty, nf);
}

fn dst_type(g: &mut Gen, usual: i32) -> i32 {
    if g.chance(6) {
        g.pick(&[MRI_UCHAR, MRI_INT, MRI_SHORT, MRI_FLOAT])
    } else {
        usual
    }
}

/// One case of `cmd`.
pub fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "fmm" => {
            let e = small(g, 6);
            let nf = nframes(g);
            let ty = if g.chance(10) { MRI_INT } else { MRI_FLOAT };
            vol(g, t, e, ty, nf);
            let rows = g.int(1, 4) as i32;
            let cols = if g.chance(10) { nf + 1 } else { nf };
            matrix(g, t, rows, cols);
            let dt = if g.chance(10) { MRI_UCHAR } else { MRI_FLOAT };
            opt_dst(g, t, e, dt, rows, 10);
        }
        "fcov" | "far1" => {
            let e = small(g, 6);
            let nf = nframes(g);
            series(g, t, e, nf);
            if cmd == "fcov" {
                t.push(g.pick(&[0, 0, 1, 1, 2, -1, nf]).to_string());
            }
            let adj = g.pick(&[
                0.0f32,
                0.0,
                1.0,
                -1.0,
                -1.0,
                2.0,
                0.5,
                f32::NAN,
                1e10,
                -0.0,
                nf as f32,
            ]);
            t.push(fx(adj));
            opt_mask(g, t, e);
            let dt = dst_type(g, MRI_FLOAT);
            let mut e2 = e;
            if g.chance(10) {
                e2[0] = (e2[0] - 1).max(1);
            }
            if g.chance(3) {
                t.push("1".into());
                vol(g, t, e2, dt, 1);
            } else {
                t.push("0".into());
            }
        }
        "fskip" | "fdrop" | "ffrm" => {
            let e = small(g, 6);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            let n = if g.chance(12) {
                g.pick(&[-1, nf, nf + 1])
            } else {
                g.int(0, i64::from(nf) - 1) as i32
            };
            t.push(n.to_string());
            let nout = if cmd == "ffrm" { 1 } else { (nf - n).max(1) };
            let dt = if g.chance(10) { MRI_FLOAT } else { ty };
            opt_dst(g, t, e, dt, nout, 10);
        }
        "vmax" | "vmin" | "conj" | "fmed" | "fsum" => {
            let e = small(g, 6);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            let usual = if matches!(cmd, "vmax" | "vmin") {
                ty
            } else {
                MRI_FLOAT
            };
            let dt = dst_type(g, usual);
            opt_dst(g, t, e, dt, 1, 10);
        }
        "vmaxi" => {
            let e = small(g, 6);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            t.push(g.pick(&[0, 0, 1, -1, 5]).to_string());
            opt_mask(g, t, e);
            let dt = dst_type(g, MRI_INT);
            opt_dst(g, t, e, dt, 1, 10);
        }
        "fmean" => {
            let e = small(g, 6);
            let nf = nframes(g);
            series(g, t, e, nf);
            let dt = dst_type(g, MRI_FLOAT);
            opt_dst(g, t, e, dt, 1, 10);
            let nw = if g.chance(2) {
                0
            } else if g.chance(8) {
                nf + 1
            } else {
                nf
            };
            t.push(nw.to_string());
            for _ in 0..nw {
                t.push(dx(g.pick(&[
                    0.5,
                    1.0,
                    2.0,
                    -1.0,
                    0.25,
                    0.1,
                    1.0 / 3.0,
                    f64::NAN,
                ])));
            }
        }
        "fsub" => {
            let e = small(g, 5);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            let start = g.pick(&[0, 0, 0, 1, 2, -1, nf]);
            let delta = g.pick(&[1, 1, 2, 3, -1, 0, -2, 4]);
            let stop = g.pick(&[-1, -1, nf, nf - 1, nf + 2, 0, 1, start + 1]);
            t.extend([start, delta, stop].map(|v| v.to_string()));
            let stop2 = if stop < 0 { nf } else { stop };
            let q = if delta == 0 {
                1.0
            } else {
                ((f64::from(stop2) - f64::from(start)) / f64::from(delta)).ceil()
            };
            if g.chance(4) && q >= 0.0 {
                t.push("1".into());
                let n = if g.chance(6) { q as i32 + 1 } else { q as i32 };
                vol(g, t, e, MRI_FLOAT, n.max(0));
            } else {
                t.push("0".into());
            }
        }
        "fext" => {
            let e = small(g, 5);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            let n = if g.chance(15) { 0 } else { g.int(1, 4) };
            t.push(n.to_string());
            for _ in 0..n {
                let f = if g.chance(15) {
                    g.pick(&[-1, nf])
                } else {
                    g.int(0, i64::from(nf) - 1) as i32
                };
                t.push(f.to_string());
            }
        }
        "asl" => {
            let ntp = if g.chance(10) {
                g.pick(&[0, -1, -3, -4])
            } else {
                g.int(1, 9) as i32
            };
            t.push(ntp.to_string());
        }
        "f2m" => {
            let e = small(g, 4);
            let nf = nframes(g);
            let ty = any_type(g);
            vol(g, t, e, ty, nf);
            let nvox = e[0] * e[1] * e[2];
            if g.chance(2) {
                t.push("0".into());
            } else {
                t.push("1".into());
                let (r, c) = match g.below(6) {
                    0 => (nf + 1, nvox + 2),
                    1 => (nf, nvox - 1),
                    2 => ((nf - 1).max(0), nvox),
                    _ => (nf, nvox),
                };
                matrix(g, t, r, c.max(1));
            }
        }
        "m2f" => {
            let e = small(g, 4);
            let nf = nframes(g);
            let nvox = e[0] * e[1] * e[2];
            let mut mt = Vec::new();
            let with_mask = g.chance(2);
            let mut count = nvox;
            if with_mask {
                let mut e2 = e;
                if g.chance(12) {
                    e2[1] += 1;
                }
                count = mask(g, &mut mt, e2);
            }
            let (r, c) = match g.below(8) {
                0 => (1, (count * nf).max(1)),
                1 => (nf, count + 1),
                2 => (nf + 1, count),
                3 => (2, (count * 2).max(1)),
                _ => (nf, count.max(1)),
            };
            matrix(g, t, r, c);
            let ty = dst_type(g, MRI_FLOAT);
            vol(g, t, e, ty, nf);
            if with_mask {
                t.push("1".into());
                t.extend(mt);
            } else {
                t.push("0".into());
            }
        }
        "scm" => {
            let e = [g.int(1, 3) as i32, g.int(1, 3) as i32, g.int(1, 2) as i32];
            let nf = g.pick(&[2, 3, 4, 5, 1]);
            series(g, t, e, nf);
        }
        "fcum" | "fnorm" => {
            let e = small(g, 6);
            let nf = nframes(g);
            series(g, t, e, nf);
            opt_mask(g, t, e);
            let dt = dst_type(g, MRI_FLOAT);
            opt_dst(g, t, e, dt, nf, 10);
        }
        c => panic!("unknown command {c}"),
    }
}

fn get_mat(cx: &mut Ctx, t: &mut Toks) -> Result<Matrix, Fatal> {
    let (rows, cols) = (t.i(), t.i());
    let mut m =
        matrix_alloc(&mut cx.g, &mut cx.a, &mut cx.b, rows, cols, MATRIX_REAL)?.expect("a matrix");
    for r in 1..=rows {
        for c in 1..=cols {
            m.set_relt(r, c, t.f());
        }
    }
    Ok(m)
}

fn put_mat(o: &mut String, m: Option<&Matrix>) {
    let Some(m) = m else {
        *o += "NULL ";
        return;
    };
    *o += &format!("{} {} ", m.rows, m.cols);
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            *o += &format!("{} ", fx(m.relt(r, c)));
        }
    }
}

/// The port's answer for one of this module's commands; `false` for another command.
pub fn run(cx: &mut Ctx, cmd: &str, t: &mut Toks, o: &mut String) -> Result<bool, Fatal> {
    match cmd {
        "fmm" => {
            let src = get_vol(cx, t)?;
            let m = get_mat(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::fmri_matrix_multiply(g, a, b, &src, &m, dst.as_mut())?;
            result(o, r, dst.as_ref());
        }
        "fcov" | "far1" => {
            let src = get_vol(cx, t)?;
            let lag = if cmd == "fcov" { t.i() } else { 0 };
            let adj = t.f();
            let mask = opt(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "fcov" {
                fmri::fmri_covariance(g, a, b, &src, lag, adj, mask.as_ref(), dst.as_mut())?
            } else {
                fmri::fmri_temporal_ar1(g, a, b, &src, adj, mask.as_ref(), dst.as_mut())?
            };
            result(o, r, dst.as_ref());
        }
        "fskip" | "fdrop" | "ffrm" => {
            let src = get_vol(cx, t)?;
            let n = t.i();
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = match cmd {
                "fskip" => fmri::fmri_nskip(g, a, b, &src, n, dst.as_mut())?,
                "fdrop" => fmri::fmri_ndrop(g, a, b, &src, n, dst.as_mut())?,
                _ => fmri::fmri_frame(g, a, b, &src, n, dst.as_mut())?,
            };
            result(o, r, dst.as_ref());
        }
        "vmax" | "vmin" | "conj" | "fmed" | "fsum" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let d = dst.as_mut();
            let r = match cmd {
                "vmax" => fmri::mri_vol_max(g, a, b, &src, d)?,
                "vmin" => fmri::mri_vol_min(g, a, b, &src, d)?,
                "conj" => fmri::mri_conjunct(g, a, b, &src, d)?,
                "fmed" => fmri::mri_frame_median(g, a, b, &src, d)?,
                _ => fmri::mri_frame_sum(g, a, b, &src, d)?,
            };
            result(o, r, dst.as_ref());
        }
        "vmaxi" => {
            let src = get_vol(cx, t)?;
            let base = t.i();
            let mask = opt(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::mri_vol_max_index(g, a, b, &src, base, mask.as_ref(), dst.as_mut())?;
            result(o, r, dst.as_ref());
        }
        "fmean" => {
            let src = get_vol(cx, t)?;
            let mut dst = opt(cx, t)?;
            let n = t.i();
            let w: Vec<f64> = (0..n).map(|_| dbl(t)).collect();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::mri_frame_mean(g, a, b, &src, dst.as_mut(), &w)?;
            result(o, r, dst.as_ref());
        }
        "fsub" => {
            let src = get_vol(cx, t)?;
            let (start, delta, stop) = (t.i(), t.i(), t.i());
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::fmri_sub_sample(g, a, b, &src, start, delta, stop, dst.as_mut())?;
            result(o, r, dst.as_ref());
        }
        "fext" => {
            let src = get_vol(cx, t)?;
            let n = t.i();
            let fl: Vec<i32> = (0..n).map(|_| t.i()).collect();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::fmri_extract_frames(g, a, b, &src, &fl)?;
            full(o, r.as_ref());
        }
        "asl" => {
            let ntp = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let m = fmri::asl_interp_matrix(g, a, b, ntp)?;
            put_mat(o, m.as_ref());
        }
        "f2m" => {
            let src = get_vol(cx, t)?;
            let mut m = if t.i() != 0 {
                Some(get_mat(cx, t)?)
            } else {
                None
            };
            let given = m.is_some();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            match fmri::fmri_to_matrix(g, a, b, &src, m.as_mut())? {
                Ret::New(r) => {
                    *o += "0 ";
                    put_mat(o, Some(&r));
                    if let Some(m) = m.as_ref() {
                        put_mat(o, Some(m));
                    }
                }
                Ret::Given => {
                    *o += "1 ";
                    put_mat(o, m.as_ref());
                }
                Ret::Null => {
                    *o += if given { "0 " } else { "1 " };
                    put_mat(o, None);
                    if let Some(m) = m.as_ref() {
                        put_mat(o, Some(m));
                    }
                }
            }
        }
        "m2f" => {
            let m = get_mat(cx, t)?;
            let mut v = get_vol(cx, t)?;
            let mask = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::fmri_from_matrix(g, a, b, &m, &mut v, mask.as_ref())?;
            *o += &format!("{r} ");
            full(o, Some(&v));
        }
        "scm" => {
            let src = get_vol(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = fmri::fmri_spatial_cor_matrix(g, a, b, &src)?;
            full(o, r.as_ref());
        }
        "fcum" | "fnorm" => {
            let src = get_vol(cx, t)?;
            let mask = opt(cx, t)?;
            let mut dst = opt(cx, t)?;
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = if cmd == "fcum" {
                fmri::fmri_cum_sum(g, a, b, &src, mask.as_ref(), dst.as_mut())?
            } else {
                fmri::mri_frame_norm(g, a, b, &src, mask.as_ref(), dst.as_mut())?
            };
            result(o, r, dst.as_ref());
        }
        _ => return Ok(false),
    }
    Ok(true)
}

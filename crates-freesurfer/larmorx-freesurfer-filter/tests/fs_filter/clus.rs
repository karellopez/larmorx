// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's cluster commands (`fsfilter.cpp`'s `clus_case`, `utils/volcluster.cpp`)
//! and the non-parametric white matter and cortex classification (`mrifilter.cpp`): their case
//! generators and the port's answers.

use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_filter::nonpar;
use larmorx_freesurfer_filter::volcluster as vc;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_UCHAR};
use larmorx_freesurfer_volume::arith::Pair;

use super::conv::{blobs, dbl, dx, full, opt, result, small};
use super::{Ctx, Gen, Toks, fx, get_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 8] = ["clus", "cprd", "cgrow", "isl", "hol", "cvr", "rwc", "amb"];

fn extent(g: &mut Gen) -> [i32; 3] {
    if g.chance(6) {
        [g.int(6, 12) as i32, g.int(6, 12) as i32, g.int(4, 8) as i32]
    } else {
        small(g, 7)
    }
}

/// A 4 x 4 transform (column, row, slice to x, y, z): small multiples of a quarter.
fn xfm(g: &mut Gen, t: &mut Vec<String>) {
    t.push("4".into());
    t.push("4".into());
    for r in 0..4 {
        for c in 0..4 {
            let v = if r == 3 {
                if c == 3 { 1.0 } else { 0.0 }
            } else if g.chance(3) {
                0.0
            } else {
                g.int(-12, 12) as f32 / 4.0
            };
            t.push(fx(v));
        }
    }
}

fn opt_xfm(g: &mut Gen, t: &mut Vec<String>) {
    if g.chance(2) {
        t.push("0".into());
    } else {
        t.push("1".into());
        xfm(g, t);
    }
}

/// A mask for the clusters (uchar 0/1, or float with values about the truncation's edges).
fn mask(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3]) {
    if g.chance(2) {
        t.push("0".into());
        return;
    }
    t.push("1".into());
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
            let v: f32 = if ty == MRI_FLOAT && g.chance(4) {
                g.pick(&[0.5, 0.99, -0.99, -1.0, 1.0, f32::NAN, 3e9])
            } else if g.chance(4) {
                0.0
            } else {
                1.0
            };
            token(ty, f64::from(v))
        })
        .collect();
    t.extend(rle(&vals));
}

fn thmin(g: &mut Gen) -> f32 {
    g.pick(&[0.5, 0.5, 1.0, 1.0, 7.0, 100.0, -5.0, 0.0, f32::NAN])
}

/// A white matter and cortex segmentation's labels (white matter 2 and 41, cortex 3 and 42,
/// background and other labels), mostly in blobs.
fn seg_labels(g: &mut Gen, e: [i32; 3]) -> Vec<i32> {
    let n = (e[0] * e[1] * e[2]) as usize;
    let empty = g.chance(15);
    // mostly a background margin on the faces (no class voxel then reads a neighbour outside)
    let margin = !g.chance(3);
    (0..n)
        .map(|i| {
            let (x, y, z) = (
                i as i32 % e[0],
                i as i32 / e[0] % e[1],
                i as i32 / (e[0] * e[1]),
            );
            let face =
                x == 0 || y == 0 || z == 0 || x == e[0] - 1 || y == e[1] - 1 || z == e[2] - 1;
            if margin && face {
                0
            } else if empty {
                g.pick(&[0, 4, 3])
            } else {
                g.pick(&[2, 2, 2, 3, 3, 3, 41, 42, 0, 0, 4, 251])
            }
        })
        .collect()
}

/// A volume's tokens (one frame, voxel sizes 1) from its values.
fn vol_of(t: &mut Vec<String>, e: [i32; 3], ty: i32, v: &[f64]) {
    t.extend(e.map(|x| x.to_string()));
    t.push(ty.to_string());
    t.push("1".into());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let vals: Vec<String> = v.iter().map(|&x| token(ty, x)).collect();
    t.extend(rle(&vals));
}

/// The intensities of a segmentation: white matter bright, cortex darker, the rest dark.
fn norm_of(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], labels: &[i32]) {
    let ty = if g.chance(4) { MRI_FLOAT } else { MRI_UCHAR };
    let v: Vec<f64> = labels
        .iter()
        .map(|&l| {
            let base = match l {
                2 | 41 | 251 => g.int(92, 122),
                3 | 42 => g.int(55, 95),
                _ => g.int(0, 60),
            } as f64;
            if ty == MRI_FLOAT && g.chance(3) {
                base + 0.25
            } else {
                base
            }
        })
        .collect();
    vol_of(t, e, ty, &v);
}

fn seg_extent(g: &mut Gen) -> [i32; 3] {
    let mut e = [0; 3];
    for x in &mut e {
        *x = if g.chance(10) { 1 } else { g.int(3, 9) as i32 };
    }
    e
}

/// One case of `cmd`.
pub fn gen_case(g: &mut Gen, t: &mut Vec<String>, cmd: &str) {
    match cmd {
        "clus" => {
            // clus <vol> frame thmin thmax thsign minsize <mask?> <xfm?> allowdiag gdiag valopt
            let e = extent(g);
            blobs(g, t, e);
            t.push(if g.chance(12) { "1" } else { "0" }.into());
            t.push(fx(thmin(g)));
            t.push(fx(g.pick(&[0.0, 0.0, 200.0, 50.0, -1.0, 5.0])));
            t.push(g.pick(&[0, 0, 1, -1, 2]).to_string());
            t.push(fx(g.pick(&[0.0, 0.0, 2.0, 5.5, 1000.0])));
            mask(g, t, e);
            opt_xfm(g, t);
            t.push(g.pick(&[0, 1]).to_string());
            t.push(g.pick(&[0, 0, 0, 1, 2]).to_string());
            t.push(g.pick(&[0, 1, 2]).to_string());
        }
        "cprd" => {
            // cprd <vol> thmin thsign <xfm?> distthresh
            let e = extent(g);
            blobs(g, t, e);
            t.push(fx(thmin(g)));
            t.push(g.pick(&[0, 1, -1]).to_string());
            if g.chance(6) {
                t.push("0".into());
            } else {
                t.push("1".into());
                xfm(g, t);
            }
            t.push(fx(g.pick(&[0.0, 1.0, 2.5, 5.0, 100.0])));
        }
        "cgrow" => {
            // cgrow <vol> thmin thsign col row slc allowdiag npassesmax
            let e = extent(g);
            blobs(g, t, e);
            t.push(fx(thmin(g)));
            t.push(g.pick(&[0, 1]).to_string());
            for n in e {
                t.push(g.int(0, i64::from(n) - 1).to_string());
            }
            t.push(g.pick(&[0, 1]).to_string());
            t.push(g.pick(&[-1, 0, 1, 2, 3]).to_string());
        }
        "isl" | "hol" => {
            // isl <mask> <dst mode> [dst] thresh nkeep; hol ... fill
            let e = extent(g);
            blobs(g, t, e);
            let m = g.pick(&[0, 0, 1, 2]);
            t.push(m.to_string());
            if m == 1 {
                let mut e2 = e;
                if g.chance(8) {
                    e2[2] += 1;
                }
                let ty = g.pick(&[MRI_UCHAR, MRI_FLOAT]);
                super::conv::vol(g, t, e2, ty, 1);
            }
            t.push(dx(g.pick(&[0.5, 0.5, 1.0, 6.0, 0.0, -1.0, f64::NAN])));
            t.push(g.pick(&[1, 1, 2, 3, 0, -1, 300]).to_string());
            if cmd == "hol" {
                t.push(dx(g.pick(&[1.0, 255.0, -2.5, 0.0, f64::NAN])));
            }
        }
        "cvr" => {
            // cvr thmin thmax thsign v1 .. v8
            t.push(fx(thmin(g)));
            t.push(fx(g.pick(&[0.0, 0.0, 200.0, 5.0, -1.0, f32::NAN])));
            t.push(g.pick(&[0, 1, -1, 2]).to_string());
            for _ in 0..8 {
                t.push(fx(g.pick(&[
                    0.0,
                    -0.0,
                    0.5,
                    1.0,
                    -1.0,
                    5.0,
                    -7.0,
                    200.0,
                    201.0,
                    f32::NAN,
                    -3e38,
                ])));
            }
        }
        "rwc" => {
            // rwc <norm> <seg> <dst mode> [<dst>] nitersmax
            let e = seg_extent(g);
            let l = seg_labels(g, e);
            norm_of(g, t, e, &l);
            let sty = if g.chance(4) { MRI_INT } else { MRI_UCHAR };
            vol_of(
                t,
                e,
                sty,
                &l.iter().map(|&x| f64::from(x)).collect::<Vec<_>>(),
            );
            let m = g.pick(&[0, 0, 0, 1, 2]);
            t.push(m.to_string());
            if m == 1 {
                let z = vec![0.0; l.len()];
                vol_of(t, e, sty, &z);
            }
            t.push(g.pick(&[30, 30, 1, 2, 0, 5]).to_string());
        }
        "amb" => {
            // amb <norm> <seg> <0 | 1 statseg> Qwm Qctx NdilWM NdilCtx
            let e = seg_extent(g);
            let l = seg_labels(g, e);
            norm_of(g, t, e, &l);
            let sty = if g.chance(4) { MRI_INT } else { MRI_UCHAR };
            vol_of(
                t,
                e,
                sty,
                &l.iter().map(|&x| f64::from(x)).collect::<Vec<_>>(),
            );
            if g.chance(2) {
                t.push("0".into());
            } else {
                t.push("1".into());
                let l2 = seg_labels(g, e);
                vol_of(
                    t,
                    e,
                    sty,
                    &l2.iter().map(|&x| f64::from(x)).collect::<Vec<_>>(),
                );
            }
            t.push(dx(g.pick(&[3.0, 3.0, 1.5, 0.5, 10.0])));
            t.push(dx(g.pick(&[3.0, 3.0, 1.5, 0.5, 10.0])));
            t.push(g.pick(&[2, 2, 1, 3, 0]).to_string());
            t.push(g.pick(&[2, 2, 1, 3]).to_string());
        }
        c => panic!("unknown command {c}"),
    }
}

static AMB_DIRS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The FNV-1a (64 bits) of a file, `NOFILE` when there is none.
fn file_hash(path: &std::path::Path) -> String {
    match std::fs::read(path) {
        Err(_) => "NOFILE".to_owned(),
        Ok(b) => {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for x in b {
                h = (h ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            format!("{h:016x}")
        }
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

fn opt_mat(cx: &mut Ctx, t: &mut Toks) -> Result<Option<Matrix>, Fatal> {
    Ok(if t.i() != 0 {
        Some(get_mat(cx, t)?)
    } else {
        None
    })
}

/// `put_clusters`: the count, then each cluster's members, largest member, its value and the
/// voxel size, then its members' column, row and slice (and coordinates when `xyz`).
fn put_clusters(o: &mut String, l: Option<&[vc::VolCluster]>, xyz: bool) {
    let Some(l) = l else {
        *o += "NULL ";
        return;
    };
    *o += &format!("{} ", l.len());
    for c in l {
        *o += &format!(
            "{} {} {} {} ",
            c.nmembers(),
            c.maxmember,
            fx(c.maxval),
            fx(c.voxsize)
        );
        for k in 0..c.col.len() {
            *o += &format!("{} {} {} ", c.col[k], c.row[k], c.slc[k]);
            if xyz {
                *o += &format!("{} {} {} ", fx(c.x[k]), fx(c.y[k]), fx(c.z[k]));
            }
        }
    }
}

/// The port's answer for one of this module's commands; `false` for another command.
pub fn run(cx: &mut Ctx, cmd: &str, t: &mut Toks, o: &mut String) -> Result<bool, Fatal> {
    match cmd {
        "clus" => {
            let v = get_vol(cx, t)?;
            let frame = t.i();
            let (lo, hi) = (t.f(), t.f());
            let sign = t.i();
            let minsize = t.f();
            let mask = opt(cx, t)?;
            let x = opt_mat(cx, t)?;
            let diag = t.i() != 0;
            cx.g.gdiag_no = t.i();
            let valopt = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let l = vc::clust_get_clusters(
                g,
                a,
                b,
                &v,
                frame,
                lo,
                hi,
                sign,
                minsize,
                mask.as_ref(),
                x.as_ref(),
                diag,
            )?;
            put_clusters(o, Some(&l), x.is_some());
            let m = vc::clust_cluster_list2vol(g, a, b, &l, &v, valopt)?;
            full(o, Some(&m));
        }
        "cprd" => {
            let v = get_vol(cx, t)?;
            let lo = t.f();
            let sign = t.i();
            let x = opt_mat(cx, t)?;
            let dist = t.f();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let l = vc::clust_get_clusters(
                g,
                a,
                b,
                &v,
                0,
                lo,
                0.0,
                sign,
                0.0,
                None,
                x.as_ref(),
                false,
            )?;
            let p = vc::clust_prune_by_distance(g, &l, dist);
            put_clusters(o, p.as_deref(), x.is_some());
        }
        "cgrow" => {
            let v = get_vol(cx, t)?;
            let lo = t.f();
            let sign = t.i();
            let (c, r, s) = (t.i(), t.i(), t.i());
            let diag = t.i() != 0;
            let np = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let mut hm = vc::clust_init_hit_map(g, a, b, &v, 0, lo, 0.0, sign, None, 0)?;
            let k = vc::clust_grow(g, b, c, r, s, &mut hm.map, diag, np)?;
            put_clusters(o, Some(&[k]), false);
            full(o, Some(&hm.map));
        }
        "isl" | "hol" => {
            let mut m = get_vol(cx, t)?;
            let mode = t.i();
            let mut dst = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let thresh = dbl(t);
            let nkeep = t.i();
            let fill = if cmd == "hol" { dbl(t) } else { 0.0 };
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = match (mode, dst.as_mut()) {
                (2, _) => Pair::Same(&mut m),
                (_, Some(d)) => Pair::Given(&m, d),
                _ => Pair::New(&m),
            };
            let r = if cmd == "isl" {
                vc::mri_remove_volume_islands(g, a, b, p, thresh, nkeep)?
            } else {
                vc::mri_remove_volume_holes(g, a, b, p, thresh, nkeep, fill)?
            };
            if mode == 2 {
                if r.is_null() {
                    *o += "0 NULL ";
                } else {
                    *o += "1 ";
                }
                full(o, Some(&m));
            } else {
                result(o, r, dst.as_ref());
            }
        }
        "cvr" => {
            let (lo, hi) = (t.f(), t.f());
            let sign = t.i();
            for _ in 0..8 {
                let v = t.f();
                *o += if vc::clust_value_in_range(v, lo, hi, sign) {
                    "1 "
                } else {
                    "0 "
                };
            }
        }
        "rwc" => {
            let norm = get_vol(cx, t)?;
            let mut seg = get_vol(cx, t)?;
            let mode = t.i();
            let mut dst = if mode == 1 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let it = t.i();
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let p = match (mode, dst.as_mut()) {
                (2, _) => Pair::Same(&mut seg),
                (_, Some(d)) => Pair::Given(&seg, d),
                _ => Pair::New(&seg),
            };
            let r = nonpar::mri_reclassify_wm_ctx_non_par(g, a, b, &norm, p, it)?;
            if mode == 2 {
                *o += "0 NULL ";
                full(o, Some(&seg));
            } else {
                result(o, r, dst.as_ref());
            }
            full(o, Some(&seg));
        }
        "amb" => {
            let norm = get_vol(cx, t)?;
            let seg = get_vol(cx, t)?;
            let stat = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let (qw, qc) = (dbl(t), dbl(t));
            let (nw, nc) = (t.i(), t.i());
            let k = AMB_DIRS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("fs-filter-amb-{}-{k}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch directory");
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            let r = nonpar::mri_classify_ambiguous_non_par(
                g,
                a,
                b,
                &norm,
                &seg,
                stat.as_ref().unwrap_or(&seg),
                qw,
                qc,
                nw,
                nc,
                Some(&dir),
            );
            let hashes = [
                file_hash(&dir.join("wm.pdf.dat")),
                file_hash(&dir.join("ctx.pdf.dat")),
            ];
            let _ = std::fs::remove_dir_all(&dir);
            full(o, r?.as_ref());
            for h in hashes {
                *o += &h;
                *o += " ";
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's cluster commands (`fsfilter.cpp`'s `clus_case`, `utils/volcluster.cpp`):
//! their case generators and the port's answers.

use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_filter::volcluster as vc;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR};
use larmorx_freesurfer_volume::arith::Pair;

use super::conv::{blobs, dbl, dx, full, opt, result, small};
use super::{Ctx, Gen, Toks, fx, get_vol, rle, token};

/// The commands of this module, in the generator's round.
pub const COMMANDS: [&str; 6] = ["clus", "cprd", "cgrow", "isl", "hol", "cvr"];

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
            for k in 0..3 {
                t.push(g.int(0, i64::from(e[k]) - 1).to_string());
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
        _ => return Ok(false),
    }
    Ok(true)
}

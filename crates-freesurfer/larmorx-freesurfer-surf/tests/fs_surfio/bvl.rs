// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 33 of track D: `bvl`, the rest of queue item 3.1-51
//! (`binarize.rs`): a surface read by `MRISread` (`d/lh.s0`: subdivided icosahedra moved off the
//! centre, open grids, srd's small meshes) whose white positions are those read and whose pial
//! positions are a second surface's (the first grown, or moved) or the white ones; a volume
//! geometry of the case; its values set from the vertex numbers, a few vertices marked, a label of
//! vertex numbers; `Gdiag`'s verbose bit now and then; 1 or 2 volumes (bytes mostly; zeros, a
//! pattern or a ball of one label); then 1 to 3 operations: the signed distance volume (`bin`,
//! `MRISbinarizeVolume`), the outside distance map (`odm`, `MRISextractOutsideDistanceMap`), the
//! label masks (`msk`, `msn`) and the laminar fractions (`lvf`, `MRIcomputeLaminarVolumeFractions`,
//! a second call taking FS-613's path): the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fli_vol, fv, geom, pair, ret_vol, surface};
use super::mpd::{grid, ico};
use super::srd::{Mesh, mesh};
use super::*;

use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_io::label::{Label, LabelVertex};
use larmorx_freesurfer_mri::mri::{Mri, MriRegion, mri_alloc_sequence};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_surf::views::{PIAL_VERTICES, WHITE_VERTICES};
use larmorx_freesurfer_surf::{binarize, readwrite};

/// The case's mesh: mostly an icosahedron moved off the centre (closed), now and then an open grid
/// or srd's small mesh scaled into 8 mm.
fn the_mesh(g: &mut Gen, heavy: bool) -> (Mesh, [f32; 3]) {
    let mut m = match g.below(10) {
        0 => {
            let (w, h) = (g.int(2, 4), g.int(2, 4));
            grid(g, w, h)
        }
        1 => mesh(g, heavy),
        2 | 3 => {
            let r = g.pick(&[3.0, 4.5]);
            ico(g, 2, r)
        }
        _ => {
            let r = g.pick(&[2.0, 3.0, 4.5, 6.0]);
            ico(g, 1, r)
        }
    };
    let big =
        m.v.iter()
            .flatten()
            .fold(0f32, |a, &c| if c.abs() > a { c.abs() } else { a });
    if big.is_finite() && big > 8.0 {
        for c in m.v.iter_mut().flatten() {
            *c = *c / big * 8.0;
        }
    }
    let off: [f32; 3] = [0, 1, 2].map(|_| {
        if g.chance(2) {
            0.0
        } else {
            g.int(-8, 8) as f32 / 4.0
        }
    });
    for p in &mut m.v {
        for k in 0..3 {
            p[k] += off[k];
        }
    }
    (m, off)
}

fn f32tok(t: &mut Vec<String>, x: f32) {
    fv(t, x);
}

fn f64tok(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

pub(super) fn bvl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let (m, off) = the_mesh(g, heavy);
    let two = g.chance(2);
    ix(t, i64::from(two));
    surface(t, &m);
    if two {
        let k = g.pick(&[1.2f32, 1.4, 1.6]);
        let moved = g.chance(5);
        let pial = Mesh {
            v: m.v
                .iter()
                .map(|p| {
                    [0, 1, 2].map(|i| {
                        if moved {
                            p[i] + 0.5
                        } else {
                            (p[i] - off[i]) * k + off[i]
                        }
                    })
                })
                .collect(),
            f: m.f.clone(),
        };
        surface(t, &pial);
    }
    let nv = m.v.len() as i64;
    // the surface's volume geometry
    ix(t, i64::from(!g.chance(5)));
    let dims = [0, 1, 2].map(|_| g.int(8, 16));
    for x in dims {
        ix(t, x);
    }
    let sgeom = geom(t, g, heavy);
    // marks and the label
    let nmark = if g.chance(3) && nv > 0 {
        g.int(1, 3.min(nv))
    } else {
        0
    };
    ix(t, nmark);
    for _ in 0..nmark {
        ix(t, g.int(0, nv - 1));
    }
    let npts = if nv > 0 { g.int(0, 8.min(nv)) } else { 0 };
    ix(t, npts);
    for _ in 0..npts {
        ix(t, g.int(0, nv - 1));
    }
    ix(t, if g.chance(8) { 0x1000_0040 } else { 0 });
    ix(t, if g.chance(8) { 1 } else { -1 });
    // volumes: (type, extent, frames, ball label)
    let nvol = g.int(1, 2);
    ix(t, nvol);
    let mut vols: Vec<(i64, [i64; 3], i64, i64)> = Vec::new();
    for k in 0..nvol {
        // the second volume, now and then, a float one of three frames of the first's extent
        // (`lvf`'s destination)
        let fractions = k == 1 && g.chance(2);
        let ty = if fractions {
            3
        } else if k == 0 {
            g.pick(&[0, 0, 0, 0, 0, 0, 0, 3, 1])
        } else {
            g.pick(&[0, 0, 0, 3, 1])
        };
        // mostly the surface's extent and geometry, so that the surface lies in the volume
        let own = !fractions && g.chance(if k == 0 { 4 } else { 2 });
        let d = if fractions {
            vols[0].1
        } else if !own {
            dims
        } else {
            [0, 1, 2].map(|_| g.int(6, 14))
        };
        let nf = if fractions { 3 } else { 1 };
        ix(t, ty);
        for x in d {
            ix(t, x);
        }
        ix(t, nf);
        if !own {
            for x in sgeom {
                fv(t, x);
            }
        } else {
            geom(t, g, heavy);
        }
        let mut label = -1;
        match g.below(8) {
            0 => ix(t, 0),
            1 => ix(t, g.int(1, 3)),
            _ if !fractions => {
                // a ball around the centre, kept off the border (FS-614 now and then all the same)
                ix(t, -1);
                let c = d.map(|x| x / 2);
                for x in c {
                    ix(t, x);
                }
                let room = d.iter().copied().min().unwrap_or(6) / 2 - 2;
                ix(t, g.int(2, room.max(2)));
                label = g.int(1, 3);
                ix(t, label);
            }
            _ => ix(t, 0),
        }
        vols.push((ty, d, nf, label));
    }
    // the white surface's bounding box (for the distance volume's region)
    let lo = [0, 1, 2].map(|k| m.v.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min));
    let hi = [0, 1, 2].map(|k| m.v.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max));
    let finite = lo.iter().chain(&hi).all(|x| x.is_finite());
    // one resolution for the case's laminar fractions mostly (a second call at another one copies
    // the first call's interior into a volume of another extent: FS-163, FreeSurfer crashes)
    let lres = g.pick(&[0.5, 1.0, 1.0, 2.0]);
    let nops = g.int(1, 3);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(10) {
            0..=2 => {
                t.push("bin".into());
                let res = if heavy && g.chance(10) {
                    f32::NAN
                } else {
                    g.pick(&[0.5f32, 1.0, 1.0, 1.5, 2.0])
                };
                f32tok(t, res);
                f32tok(t, g.pick(&[1.0f32, 2.0, 3.0, 5.0]));
                let margin = g.int(0, 3);
                for k in 0..3 {
                    let x = if finite {
                        lo[k].floor() as i64 - margin
                    } else {
                        -8
                    };
                    ix(t, x);
                }
                for k in 0..3 {
                    let span = if finite {
                        (hi[k] - lo[k]).ceil() as i64 + 2 * margin
                    } else {
                        16
                    };
                    ix(t, if g.chance(30) { 0 } else { span.clamp(1, 24) });
                }
            }
            3 | 4 => {
                t.push("odm".into());
                // a byte volume with a ball, when there is one
                let balls: Vec<usize> = (0..vols.len())
                    .filter(|&k| vols[k].3 >= 0 && vols[k].0 == 0)
                    .collect();
                let v = if !balls.is_empty() && !g.chance(6) {
                    balls[g.below(balls.len() as u64) as usize]
                } else {
                    g.below(vols.len() as u64) as usize
                };
                ix(t, v as i64);
                let label = if vols[v].3 >= 0 && !g.chance(8) {
                    vols[v].3
                } else {
                    g.int(0, 3)
                };
                ix(t, label);
                ix(
                    t,
                    if g.chance(10) {
                        g.pick(&[-1, 3])
                    } else {
                        g.int(0, 2)
                    },
                );
                f32tok(t, g.pick(&[0.5f32, 1.0, 1.0, 2.0]));
                f32tok(t, g.pick(&[0.5f32, 1.0, 2.0, 3.0]));
            }
            5 => t.push("msk".into()),
            6 => t.push("msn".into()),
            _ => {
                t.push("lvf".into());
                f64tok(t, lres);
                ix(t, 0);
                let given = vols.len() == 2 && vols[1].2 == 3 && g.chance(2);
                ix(t, if given { 1 } else { -1 });
            }
        }
    }
}

/// The vertices' values and marks, as `bvl_values` prints them.
fn values(s: &larmorx_freesurfer_surf::mris::Mris, o: &mut Out) {
    for vno in 0..s.nvertices as usize {
        let v = s.vertex(vno);
        for x in [
            v.curv, v.stat, v.val, v.imag_val, v.val2, v.valbak, v.val2bak,
        ] {
            o.f(x);
        }
        o.i(i64::from(v.marked));
    }
}

pub(super) fn bvl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let two = t.i() != 0;
    let mut files = Vec::new();
    for k in 0..1 + usize::from(two) {
        let bytes = t.h();
        let (nv, nf) = (t.i(), t.i());
        let name = format!("d/lh.s{k}");
        std::fs::write(dir.join(&name), bytes).expect("the surface file");
        files.push((name, nv, nf));
    }
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    let d = Some(dir);
    let (mut so, mut se) = (cx.a.clone(), cx.b.clone());
    let mut io = coords::Io {
        out: &mut so,
        err: &mut se,
        env: &env,
    };
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    g.errno = 0;
    let mut surfs = Vec::new();
    for (name, nv, nf) in &files {
        let Some(s) = readwrite::read(g, sg, &mut io, d, name, true)? else {
            o.s("NULL");
            return Ok(());
        };
        if *nv != s.nvertices || *nf != s.nfaces {
            o.i(i64::from(s.nvertices));
            o.i(i64::from(s.nfaces));
            o.s("COUNTS");
            return Ok(());
        }
        surfs.push(s);
    }
    let mut s = surfs.remove(0);
    {
        let vg = &mut s.vg;
        vg.valid = t.i();
        (vg.width, vg.height, vg.depth) = (t.i(), t.i(), t.i());
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    }
    positions::save_vertex_positions(g, &mut io, &mut s, WHITE_VERTICES)?;
    if let Some(p) = surfs.first() {
        for vno in 0..s.nvertices as usize {
            let q = p.vertex(vno);
            let (x, y, z) = (q.x, q.y, q.z);
            let v = s.vertex_mut(vno);
            (v.x, v.y, v.z) = (x, y, z);
        }
    }
    positions::save_vertex_positions(g, &mut io, &mut s, PIAL_VERTICES)?;
    positions::restore_vertex_positions(g, &mut io, &mut s, WHITE_VERTICES)?;
    larmorx_freesurfer_surf::mp::compute_metric_properties(g, sg, &mut io, &mut s)?;
    for vno in 0..s.nvertices as usize {
        let b = (vno as i32 + 1) as f32 * 0.25;
        let v = s.vertex_mut(vno);
        v.curv = b;
        v.stat = b + 1.0;
        v.val = b + 2.0;
        v.imag_val = b + 3.0;
        v.val2 = b + 4.0;
        v.valbak = b + 5.0;
        v.val2bak = b + 6.0;
    }
    let nmark = t.i();
    for _ in 0..nmark {
        let vno = t.i() as usize;
        s.vertex_mut(vno).marked = 1;
    }
    let npts = t.i();
    let mut lab = Label {
        max_points: npts.max(1),
        n_points: npts,
        lv: vec![
            LabelVertex {
                vno: -1,
                ..LabelVertex::default()
            };
            npts.max(1) as usize
        ],
        ..Label::default()
    };
    for k in 0..npts as usize {
        lab.lv[k].vno = t.i();
    }
    g.gdiag = t.i() as u32 as u64;
    g.gdiag_no = t.i();
    let nvol = t.i();
    let mut vols: Vec<Mri> = Vec::new();
    for _ in 0..nvol {
        let ty = t.i();
        let (w, h, dd, nf) = (t.i(), t.i(), t.i(), t.i());
        let mut v = mri_alloc_sequence(g, io.out, io.err, w, h, dd, ty, nf)?;
        let vg = &mut v.vg;
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
        reinit_cache(g, io.out, io.err, &mut v.vg)?;
        let fill = t.i();
        if fill > 0 {
            for sl in 0..dd {
                for r in 0..h {
                    for c in 0..w {
                        let x = ((c + 2 * r + 3 * sl + fill) % 4) as f32;
                        mri_set_vox_val(g, io.err, &mut v, c, r, sl, 0, x, 0)?;
                    }
                }
            }
        }
        if fill < 0 {
            let (cx_, cy, cz, rr, val) = (t.i(), t.i(), t.i(), t.i(), t.i());
            for sl in 0..dd {
                for r in 0..h {
                    for c in 0..w {
                        let q = (c - cx_) * (c - cx_) + (r - cy) * (r - cy) + (sl - cz) * (sl - cz);
                        if q <= rr * rr {
                            mri_set_vox_val(g, io.err, &mut v, c, r, sl, 0, val as f32, 0)?;
                        }
                    }
                }
            }
        }
        vols.push(v);
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "bin" => {
                let (res, dist) = (t.f(), t.f());
                let reg = MriRegion {
                    x: t.i(),
                    y: t.i(),
                    z: t.i(),
                    dx: t.i(),
                    dy: t.i(),
                    dz: t.i(),
                };
                let r = binarize::mris_binarize_volume(g, &mut io, &mut s, &reg, res, dist)?;
                fli_vol(g, io.err, Some(&r), o);
                o.f(r.xstart);
                o.f(r.ystart);
                o.f(r.zstart);
            }
            "odm" => {
                let v = t.i() as usize;
                let (label, offset, res, maxd) = (t.i(), t.i(), t.f(), t.f());
                binarize::mris_extract_outside_distance_map(
                    g,
                    &mut io,
                    &mut s,
                    &mut vols[v],
                    label,
                    offset,
                    res,
                    maxd,
                )?;
                for vno in 0..s.nvertices as usize {
                    o.f(s.vertex(vno).curv);
                }
            }
            "msk" => {
                o.i(i64::from(binarize::mris_mask_label(g, &mut s, &lab)));
                values(&s, o);
            }
            "msn" => {
                o.i(i64::from(binarize::mris_mask_not_label(g, &mut s, &lab)));
                values(&s, o);
            }
            "lvf" => {
                let res = t.d();
                let v = t.i() as usize;
                let di = t.i();
                if di >= 0 {
                    let (src, dst) = pair(&mut vols, v, di as usize);
                    binarize::mri_compute_laminar_volume_fractions(
                        g,
                        sg,
                        &mut io,
                        &mut s,
                        res,
                        src,
                        Some(dst),
                    )?;
                    let given = &vols[di as usize];
                    ret_vol(g, io.err, Ret::Given, Some(given), o);
                } else {
                    let r = binarize::mri_compute_laminar_volume_fractions(
                        g, sg, &mut io, &mut s, res, &vols[v], None,
                    )?;
                    let r = r.map_or(Ret::Null, Ret::New);
                    ret_vol(g, io.err, r, None, o);
                }
                for vno in 0..s.nvertices as usize {
                    let p = s.vertex(vno);
                    for x in [p.x, p.y, p.z] {
                        o.f(x);
                    }
                }
            }
            _ => panic!("bvl op {op}"),
        }
    }
    Ok(())
}

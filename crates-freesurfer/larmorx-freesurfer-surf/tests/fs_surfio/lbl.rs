// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 37 of track D: `lbl`, label.cpp's surface-label functions
//! (`labelsurf.rs`): a surface read by `MRISread` (`d/lh.s0`: subdivided icosahedra, open grids,
//! srd's small meshes) with some vertices ripped and some marks set, `SUBJECTS_DIR` set or not, a
//! label of the case (`LabelAlloc` of 0 to 12 places, its points' vertices mostly the surface's,
//! now and then -1 or beyond it, some deleted, a subject name or none; a vertex index table and
//! the surface handle, a template volume, now and then); then 1 to 4 operations: `LabelMark`
//! (`mk`), `LabelMark2` (`mk2`), `LabelUnmark` (`um`), `LabelMarkSurface` (`ms`), `LabelArea`
//! (`ar`), `LabelFillMarked` (`fm`), `LabelErode` (`er`), `LabelDilate` (`di`), `MRISlabel2Mask`
//! (`l2m`) and `MRISlabelInvert` (`inv`); an operation whose label has a point FreeSurfer would
//! read or write outside the surface's vertices or the label's places for (FS-630, FS-631) is written
//! `G` on both sides;
//! after each the label (every place), the marks and the index table: the generator and the
//! port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, geom, surface};
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_io::label::{Label, label_alloc};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::{labelsurf, readwrite};

pub(super) fn lbl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        2 => mesh(g, heavy),
        3 => ico(g, 2, 10.0),
        _ => {
            let r = g.pick(&[1.0, 10.0, 100.0]);
            ico(g, 1, r)
        }
    };
    surface(t, &m);
    let nv = m.v.len() as i64;
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    let nmark = if nv > 0 { g.int(0, 4) } else { 0 };
    ix(t, nmark);
    for _ in 0..nmark {
        ix(t, g.int(0, nv - 1));
        ix(t, g.pick(&[0, 1, 1, 2, -1]));
        ix(t, g.pick(&[0, 1, 2]));
    }
    ix(t, i64::from(!g.chance(6)));
    // the label
    let max = g.pick(&[0, 1, 2, 3, 5, 8, 12, 12]);
    let n = if max == 0 { 0 } else { g.int(0, max) };
    ix(t, max);
    ix(t, n);
    ix(t, g.pick(&[1, 1, 2, 3]));
    ix(t, i64::from(g.chance(3)));
    for _ in 0..n {
        let vno = if nv > 0 && !(heavy && g.chance(6)) {
            g.int(0, nv - 1)
        } else {
            g.pick(&[-1, nv, nv + 3])
        };
        ix(t, vno);
        for _ in 0..3 {
            fv(t, g.int(-32, 32) as f32 / 4.0);
        }
        ix(t, i64::from(g.chance(5)));
        fv(t, g.int(-8, 8) as f32 / 2.0);
    }
    // the vertex index table and the surface handle; a template volume
    ix(t, i64::from(g.chance(3)));
    let tpl = g.chance(4);
    ix(t, i64::from(tpl));
    if tpl {
        for _ in 0..3 {
            ix(t, g.int(4, 8));
        }
        geom(t, g, false);
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(13) {
            0 => t.push("mk".into()),
            1 => t.push("mk2".into()),
            2 => t.push("um".into()),
            3 => t.push("ms".into()),
            4 => t.push("ar".into()),
            5 | 6 => t.push("fm".into()),
            7 | 8 => {
                t.push("er".into());
                ix(t, g.pick(&[1, 1, 2, 0]));
            }
            9 | 10 => {
                t.push("di".into());
                ix(t, g.pick(&[1, 1, 2, 3, 0]));
                ix(
                    t,
                    if g.chance(10) {
                        14
                    } else {
                        g.pick(&[4, 4, 0, 1, 3, 8])
                    },
                );
            }
            11 => {
                t.push("l2m".into());
                ix(t, g.pick(&[0, 0, 1, 2]));
                ix(t, g.int(0, 1));
                let given = g.chance(3);
                ix(t, i64::from(given));
                if given {
                    ix(t, g.pick(&[1, 2]));
                }
            }
            _ => t.push("inv".into()),
        }
    }
}

/// Whether FreeSurfer would read or write outside the surface's vertices (or the label's places)
/// for the operation `op` on this label (FS-630, FS-631): the case writes `G`.
fn outside(op: &str, a: &Label, nv: i32) -> bool {
    let pts = &a.lv[..a.n_points.max(0) as usize];
    let out = |v: i32| v < 0 || v >= nv;
    let live_out = pts.iter().any(|p| p.deleted == 0 && out(p.vno));
    let beyond = pts.iter().any(|p| p.vno >= nv);
    let any_out = pts.iter().any(|p| out(p.vno));
    let negative = pts.iter().any(|p| p.vno < 0);
    match op {
        "mk" | "mk2" => live_out,
        "um" => beyond,
        "ar" | "fm" => any_out,
        "er" => live_out || beyond,
        "di" => live_out || beyond || a.max_points == 0,
        "l2m" | "inv" => negative,
        _ => false,
    }
}

fn dump(a: &Label, s: &Mris, o: &mut Out) {
    o.i(i64::from(a.n_points));
    o.i(i64::from(a.max_points));
    for p in a.lv.iter().take(a.max_points.max(0) as usize) {
        o.i(i64::from(p.vno));
        for x in [p.x, p.y, p.z, p.stat] {
            o.f(x);
        }
        o.i(i64::from(p.deleted));
        for x in [p.xv, p.yv, p.zv] {
            o.i(i64::from(x));
        }
    }
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        o.i(i64::from(v.marked));
        o.i(i64::from(v.marked2));
    }
    match &a.vertex_label_ind {
        None => o.s("N"),
        Some(ind) => ind.iter().for_each(|&x| o.i(i64::from(x))),
    }
}

fn dump_mask(g: &mut Globals, err: &mut dyn Write, m: &Mri, o: &mut Out) {
    o.i(i64::from(m.nframes));
    for f in 0..m.nframes {
        for c in 0..m.vg.width {
            o.f(mri_get_vox_val(g, err, m, c, 0, 0, f, 0).unwrap_or(0.0));
        }
    }
}

pub(super) fn lbl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let rips: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
    let marks: Vec<(i32, i32, i32)> = (0..t.i()).map(|_| (t.i(), t.i(), t.i())).collect();
    let mut pairs = vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())];
    if t.i() != 0 {
        pairs.push((
            "SUBJECTS_DIR".to_owned(),
            dir.to_string_lossy().into_owned(),
        ));
    }
    let env = Environment::from_pairs(pairs);
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
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
    let Some(mut s) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL");
        return Ok(());
    };
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    for vno in rips {
        s.vertex_mut(vno as usize).ripflag = 1;
    }
    for (vno, m1, m2) in marks {
        let v = s.vertex_mut(vno as usize);
        v.marked = m1 as i16;
        v.marked2 = m2 as i16;
    }
    let (max, n) = (t.i(), t.i());
    let mut a = label_alloc(g, io.env, io.err, max, None, None)?;
    a.coords = t.i();
    if t.i() != 0 {
        a.subject_name = b"subj".to_vec();
    }
    a.n_points = n;
    for k in 0..n as usize {
        let p = &mut a.lv[k];
        p.vno = t.i();
        (p.x, p.y, p.z) = (t.f(), t.f(), t.f());
        p.deleted = t.i() as u8;
        p.stat = t.f();
    }
    if t.i() != 0 {
        a.vertex_label_ind = Some(vec![0; nv.max(0) as usize]);
        a.mris = Some(1);
    }
    let mut tpl = None;
    if t.i() != 0 {
        let (w, h, dd) = (t.i(), t.i(), t.i());
        let mut v = mri_alloc(g, io.out, io.err, w, h, dd, MRI_UCHAR)?;
        {
            let vg = &mut v.vg;
            (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
            (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
            (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
            (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
            (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
        }
        reinit_cache(g, io.out, io.err, &mut v.vg)?;
        a.mri_template = Some(1);
        tpl = Some(v);
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let args: Vec<i32> = match op {
            "er" => vec![t.i()],
            "di" => vec![t.i(), t.i()],
            "l2m" => {
                let (f, st, given) = (t.i(), t.i(), t.i());
                let nfr = if given != 0 { t.i() } else { 0 };
                vec![f, st, given, nfr]
            }
            _ => Vec::new(),
        };
        if outside(op, &a, nv) {
            o.s("G");
            continue;
        }
        match op {
            "mk" => o.i(i64::from(labelsurf::label_mark(g, &a, &mut s))),
            "mk2" => o.i(i64::from(labelsurf::label_mark2(g, &a, &mut s))),
            "um" => o.i(i64::from(labelsurf::label_unmark(g, &a, &mut s))),
            "ms" => o.i(i64::from(labelsurf::label_mark_surface(
                &mut io, &a, &mut s,
            ))),
            "ar" => o.d(labelsurf::label_area(g, &a, &s)),
            "fm" => o.i(i64::from(labelsurf::label_fill_marked(g, &mut a, &mut s))),
            "er" => o.i(i64::from(labelsurf::label_erode(
                g, &mut io, &mut a, &mut s, args[0],
            )?)),
            "di" => o.i(i64::from(labelsurf::label_dilate(
                g,
                &mut io,
                &mut a,
                &mut s,
                args[0],
                args[1],
                tpl.as_ref(),
            )?)),
            "l2m" => {
                let mut given = if args[2] != 0 {
                    Some(mri_alloc_sequence(
                        g, io.out, io.err, nv, 1, 1, MRI_FLOAT, args[3],
                    )?)
                } else {
                    None
                };
                let r = labelsurf::mris_label2mask(
                    g,
                    &mut io,
                    &s,
                    &a,
                    given.as_mut(),
                    args[0],
                    args[1] != 0,
                )?;
                match r {
                    Ret::New(m) => dump_mask(g, io.err, &m, o),
                    Ret::Given => dump_mask(g, io.err, given.as_ref().expect("given"), o),
                    Ret::Null => o.s("NULL"),
                }
            }
            "inv" => match labelsurf::mris_label_invert(g, &mut io, &s, &a)? {
                None => o.s("NULL"),
                Some(l) => {
                    o.h(Some(&l.subject_name));
                    dump(&l, &s, o);
                }
            },
            _ => panic!("lbl op {op}"),
        }
        dump(&a, &s, o);
    }
    Ok(())
}

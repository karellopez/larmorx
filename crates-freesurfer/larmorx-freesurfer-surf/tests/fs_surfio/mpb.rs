// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 17: `mpb`, the metric properties of queue item 3.1-50 part b
//! (`props.rs`): a surface read by `MRISread` (`d/lh.s0`, a binary triangle file of srd's meshes),
//! its vertices' and faces' fields overridden now and then, then 1 to 6 operations (the common
//! vertices and edge of two faces, the tangent planes, the plane fit, the face centroids, the
//! face-area and vertex-spacing statistics, the negative-face counts, the tessellation statistics,
//! the closest vertices, the curvature edits), each printing what it gives: the generator and the
//! port's answers, as `fssurfio.cpp`'s.

use super::srd::{be4, mesh};
use super::*;

use larmorx_freesurfer_core::diag::DIAG_WRITE;
use larmorx_freesurfer_surf::{props, readwrite};

fn fh(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    t.push(format!("{:08x}", smallish(g, heavy)));
}

pub(super) fn mpb_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 7));
    ix(t, i64::from(g.chance(6)));
    let mut m = mesh(g, heavy);
    if g.chance(5) {
        // large faces and edges (the tessellation statistics' list of edges over 20 mm)
        for p in &mut m.v {
            for c in p.iter_mut() {
                *c *= 12.0;
            }
        }
    }
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Thu Nov 12 20:55:19 2026\n\n");
    be4(&mut b, m.v.len() as i64);
    be4(&mut b, m.f.len() as i64);
    for p in &m.v {
        for c in p {
            b.extend_from_slice(&c.to_be_bytes());
        }
    }
    for f in &m.f {
        for &x in f {
            be4(&mut b, x);
        }
    }
    hx(t, &b);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    ix(t, nv);
    ix(t, nf);
    // the surface's original area (0 after a read: the area statistics' scale)
    if g.chance(2) {
        ix(t, 1);
        if g.chance(3) {
            fh(t, g, heavy);
        } else {
            t.push(format!("{:08x}", (g.int(1, 4000) as f32 * 4.0).to_bits()));
        }
    } else {
        ix(t, 0);
    }
    for _ in 0..nv {
        let mut mask = 0;
        for (bit, k) in [
            (1, 8),
            (2, 3),
            (4, 3),
            (8, 3),
            (16, 3),
            (32, 3),
            (64, 2),
            (128, 6),
        ] {
            if g.chance(k) {
                mask |= bit;
            }
        }
        ix(t, mask);
        for bit in [2, 4, 8, 16, 32] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        if mask & 64 != 0 {
            fh(t, g, heavy);
        }
        if mask & 128 != 0 {
            for _ in 0..3 {
                fh(t, g, heavy);
            }
        }
    }
    for _ in 0..nf {
        let mut mask = 0;
        if g.chance(8) {
            mask |= 1;
        }
        if g.chance(4) {
            mask |= 2;
        }
        ix(t, mask);
        if mask & 2 != 0 {
            fh(t, g, heavy);
        }
    }
    let nops = g.int(1, 6);
    ix(t, nops);
    for _ in 0..nops {
        let ops: &[&str] = if nf > 0 {
            &[
                "vc", "ce", "tp", "st", "op", "fc", "fa", "vs", "ts", "nf", "pt", "cv", "cl", "sc",
                "tr", "sl", "bi", "th", "nz", "hc", "tp", "vs", "hc", "fa",
            ]
        } else {
            &[
                "tp", "st", "op", "fa", "vs", "ts", "nf", "pt", "cv", "cl", "sc", "tr", "sl", "bi",
                "th", "nz", "hc",
            ]
        };
        let op = g.pick(ops);
        t.push(op.to_owned());
        match op {
            "vc" | "ce" => {
                let a = g.below(nf as u64) as usize;
                // mostly a face sharing an edge with the first
                let nbr: Vec<usize> = (0..nf as usize)
                    .filter(|&j| {
                        j != a && m.f[j].iter().filter(|x| m.f[a].contains(x)).count() == 2
                    })
                    .collect();
                let b = if !nbr.is_empty() && !g.chance(4) {
                    nbr[g.below(nbr.len() as u64) as usize]
                } else {
                    g.below(nf as u64) as usize
                };
                ix(t, a as i64);
                ix(t, b as i64);
            }
            "st" => ix(t, g.int(-1, 10)),
            "op" => {
                let n = g.int(0, 8);
                ix(t, n);
                for _ in 0..n {
                    ix(t, g.int(0, nv - 1));
                }
            }
            "fc" => {
                ix(t, g.int(0, nf - 1));
                ix(t, i64::from(g.chance(2)));
            }
            "vs" => ix(t, g.pick(&[0, 4, 9, 7, 10, 4, 3])),
            "cv" => {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
                ix(t, g.pick(&[4, 3, 4, 3, 0]));
            }
            "cl" => {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
            "sc" | "tr" | "sl" => fh(t, g, heavy),
            "bi" => {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
                ix(t, i64::from(g.chance(2)));
            }
            "th" => {
                fh(t, g, heavy);
                ix(t, i64::from(g.chance(2)));
            }
            "nz" => {
                ix(t, i64::from(g.chance(2)));
                ix(t, g.pick(&[-1, 2, 0, 1, 2]));
            }
            "hc" => {
                if g.chance(3) {
                    fh(t, g, heavy);
                } else {
                    t.push(format!("{:08x}", (g.int(0, 100) as f32 / 100.0).to_bits()));
                }
            }
            _ => {}
        }
    }
}

fn dump_mpb_vertices(o: &mut Out, s: &Mris) {
    o.f(s.min_curv);
    o.f(s.max_curv);
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [
            v.curv, v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z, v.pe1x, v.pe1y, v.pe1z, v.pe2x,
            v.pe2y, v.pe2z,
        ] {
            o.f(x);
        }
    }
}

fn f3(t: &mut Toks) -> [f32; 3] {
    [t.f(), t.f(), t.f()]
}

pub(super) fn mpb_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    if gd & 4 != 0 {
        cx.g.gdiag |= DIAG_WRITE;
    }
    let mut pairs: Vec<(String, String)> = vec![("SURFER_SIDEDOOR".into(), "1".into())];
    if t.i() & 1 != 0 {
        pairs.push(("FS_FASTER_MP".into(), "1".into()));
    }
    let env = Environment::from_pairs(pairs);
    std::fs::write(dir.join("d/lh.s0"), t.h()).expect("the surface file");
    let d = Some(dir);
    let mut io = coords::Io {
        out: &mut cx.a.clone(),
        err: &mut cx.b.clone(),
        env: &env,
    };
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    g.errno = 0;
    let Some(mut s) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL");
        return Ok(());
    };
    let (nv, nf) = (t.i(), t.i());
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    if t.i() != 0 {
        s.orig_area = t.f();
    }
    for vno in 0..nv as usize {
        let m = t.i();
        let v = s.vertex_mut(vno);
        if m & 1 != 0 {
            v.ripflag = 1;
        }
        if m & 2 != 0 {
            [v.origx, v.origy, v.origz] = f3(t);
        }
        if m & 4 != 0 {
            [v.whitex, v.whitey, v.whitez] = f3(t);
        }
        if m & 8 != 0 {
            [v.pialx, v.pialy, v.pialz] = f3(t);
        }
        if m & 16 != 0 {
            [v.targx, v.targy, v.targz] = f3(t);
        }
        if m & 32 != 0 {
            [v.cx, v.cy, v.cz] = f3(t);
        }
        if m & 64 != 0 {
            v.curv = t.f();
        }
        if m & 128 != 0 {
            [v.nx, v.ny, v.nz] = f3(t);
        }
    }
    for fno in 0..nf as usize {
        let m = t.i();
        let f = s.face_mut(fno);
        if m & 1 != 0 {
            f.ripflag = 1;
        }
        if m & 2 != 0 {
            f.area = t.f();
        }
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "vc" => {
                let (a, b) = (t.i() as usize, t.i() as usize);
                let (k, v) = props::vertices_common_in_faces(s.face(a), s.face(b));
                o.i(i64::from(k));
                for x in v {
                    o.f(x);
                }
            }
            "ce" => {
                let (a, b) = (t.i() as usize, t.i() as usize);
                let (fa, fb) = (s.face(a).clone(), s.face(b).clone());
                o.f(props::faces_common_edge_length_find(
                    g, &mut io, &s, &fa, &fb,
                )?);
            }
            "tp" => {
                o.i(i64::from(props::compute_tangent_planes(g, &mut io, &mut s)));
                dump_mpb_vertices(o, &s);
            }
            "st" => {
                let w = t.i();
                o.i(i64::from(props::store_tangent_planes(&mut s, w)));
                dump_mpb_vertices(o, &s);
            }
            "op" => {
                let n = t.i();
                let list: Vec<i32> = (0..n).map(|_| t.i()).collect();
                match props::compute_optimal_plane(g, &mut io, &s, &list)? {
                    Some(p) => {
                        o.i(0);
                        for x in p.normal.iter().chain(p.centre.iter()) {
                            o.d(*x);
                        }
                    }
                    None => {
                        o.i(-4);
                        for _ in 0..6 {
                            o.d(7.0);
                        }
                    }
                }
            }
            "fc" => {
                let (f, which) = (t.i() as usize, t.i());
                let kind = if which != 0 {
                    props::Centroid::Canonical
                } else {
                    props::Centroid::Current
                };
                for x in props::face_centroid(&s, f, kind) {
                    o.f(x);
                }
            }
            "fa" => {
                let r = props::compute_face_area_stats(g, sg, &mut io, &mut s)?;
                for x in [r.mean, r.sigma, r.min, r.max] {
                    o.d(x);
                }
            }
            "vs" | "ts" => {
                let r = if op == "vs" {
                    let w = t.i();
                    props::compute_vertex_spacing_stats(g, sg, &mut io, &mut s, w)?
                } else {
                    props::compute_total_vertex_spacing_stats(g, sg, &mut io, &mut s)?
                };
                for x in [r.mean, r.sigma, r.min, r.max] {
                    o.d(x);
                }
                // the probe's 7 where the function writes nothing
                o.i(i64::from(r.vno.unwrap_or(7)));
                o.i(i64::from(r.vno2.unwrap_or(7)));
            }
            "nf" => {
                let n = props::count_negative_faces(&s);
                o.i(i64::from(n));
                o.i(i64::from(n));
            }
            "pt" => {
                let mut fp = cx.a.clone();
                o.i(i64::from(props::print_tessellation_stats(
                    g, sg, &mut io, &mut s, &mut fp,
                )?));
            }
            "cv" => {
                let p = f3(t);
                let w = t.i();
                let (v, dmin) = props::find_closest_vertex(g, &mut io, &s, p, w)?;
                o.i(i64::from(v));
                o.f(dmin);
            }
            "cl" => {
                let p = f3(t);
                for w in [
                    props::Positions::Original,
                    props::Positions::Canonical,
                    props::Positions::White,
                ] {
                    o.i(i64::from(props::find_closest_in(&s, p, w)));
                }
            }
            _ => {
                match op {
                    "sc" => {
                        props::set_curvature(&mut s, t.f());
                    }
                    "tr" => {
                        props::trinarize_curvature(&mut s, t.f());
                    }
                    "sl" => {
                        props::scale_curvature(&mut s, t.f());
                    }
                    "bi" => {
                        let (th, lo, hi) = (t.f(), t.f(), t.f());
                        props::binarize_curvature(&mut s, th, lo, hi, t.i() != 0);
                    }
                    "th" => {
                        let th = t.f();
                        props::threshold_curvature(&mut s, th, t.i() != 0);
                    }
                    "nz" => {
                        let dfield = t.i() != 0;
                        let w = t.i();
                        props::normalize_field(&mut io, &mut s, dfield, w);
                    }
                    "hc" => {
                        let pct = t.f();
                        props::histo_threshold_curvature(g, &mut io, d, &mut s, pct)?;
                    }
                    _ => panic!("mpb op {op}"),
                }
                dump_mpb_vertices(o, &s);
            }
        }
    }
    Ok(())
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 43 of track D: `mcl`, the closest searches of the surface hash
//! table (`mhtfind.rs`): a surface read by `MRISread` (`d/lh.s0`: mht's crumpled grids, srd's small
//! meshes, icosahedra, two sheets crossing; now and then moved beyond the table, FS-999), faces
//! and vertices ripped now and then, original positions now and then; a second surface (the same
//! file read again, each vertex moved by up to 2 mm, its original positions its current ones) for
//! `MHTfindClosestVertexNo2`, `MHTfindClosestVertex2` and `MHTfindClosestVertexSet2`; then 1 to 6
//! operations: the tables (`ft`, `vt`, `ftd`, `vtd`), the searches at a point (`vg`, `vx`, `vs`,
//! `vi`, `vit`, `fg`, `fn`, `bf`) or at a vertex of the second surface (`v2`, `vp2`, `vs2`), every
//! vertex of the second surface searched (`av`, `ax`, `af`), a vertex moved or ripped after the
//! table was made (`mv`, `rip`): the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mht::{crumple, sheets};
use super::mpd::{grid, ico};
use super::srd::{Mesh, mesh};
use super::*;

use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mhtfind;
use larmorx_freesurfer_surf::readwrite;

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

fn res(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(8) {
        return g.pick(&[0.0, -1.0, f32::NAN]);
    }
    g.pick(&[1.0, 1.0, 1.0, 0.5, 2.0, 0.25, 4.0, 10.0])
}

fn which(g: &mut Gen, orig: bool) -> i64 {
    if g.chance(40) {
        return g.pick(&[3, 99]);
    }
    if orig && g.chance(3) { 0 } else { 4 }
}

/// A point to search from: near a vertex (quarters up to 2 mm), on a vertex, in the mesh's box,
/// up to 25 mm away, or (heavy cases) not finite or far beyond the table.
fn point(g: &mut Gen, m: &Mesh, heavy: bool) -> [f32; 3] {
    if heavy && g.chance(8) {
        let mut p = [0.0f32; 3];
        p[g.below(3) as usize] = g.pick(&[f32::NAN, f32::INFINITY, 3e9, -1e12]);
        return p;
    }
    let finite: Vec<[f32; 3]> =
        m.v.iter()
            .copied()
            .filter(|p| p.iter().all(|c| c.is_finite()))
            .collect();
    if finite.is_empty() {
        return [g.int(-8, 8) as f32 / 4.0, 0.25, -0.5];
    }
    let base = finite[g.below(finite.len() as u64) as usize];
    match g.below(8) {
        0 => base,
        1..=4 => base.map(|c| c + g.int(-8, 8) as f32 / 4.0),
        5 | 6 => base.map(|c| c + g.int(-24, 24) as f32 / 4.0),
        _ => base.map(|c| c + g.int(-100, 100) as f32 / 4.0),
    }
}

/// The squared distance from `p` to the nearest finite vertex of the mesh (`f32::MAX` for none).
fn nearest(m: &Mesh, p: [f32; 3]) -> f32 {
    m.v.iter()
        .filter(|q| q.iter().all(|c| c.is_finite()))
        .map(|q| (0..3).map(|k| (q[k] - p[k]) * (q[k] - p[k])).sum::<f32>())
        .fold(f32::MAX, f32::min)
}

/// The search's limits: a distance and a shell count. A face search may have no limit on its
/// shells; FreeSurfer then looks at every voxel up to the distance until a face is found, so a
/// long search is asked for only when a face is found early (the official probe's alarm is 5
/// seconds): the point near the surface, faces in the table, no projection test.
fn limits(g: &mut Gen, heavy: bool, face: bool, safe_d: f32, res: f32) -> (f64, i64) {
    let mut d = if heavy && g.chance(6) {
        g.pick(&[f64::NAN, -1.0, 1e300, 0.0])
    } else {
        g.pick(&[0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 10.0, 1000.0, 1000.0])
    };
    let mut m = if heavy && g.chance(6) {
        g.pick(&[7, 100])
    } else {
        g.pick(&[-1, -1, -1, 0, 1, 2, 3, 4, 5, 8])
    };
    if face {
        let shells = |d: f64, m: i64| -> f64 {
            if m >= 0 {
                m as f64
            } else if d.is_finite() && res > 0.0 {
                (d / f64::from(res)).ceil().min(1e9)
            } else {
                0.0
            }
        };
        // a long search only when a face is found within `safe_d` (the shells up to it alone)
        let found_at = if safe_d.is_finite() && res > 0.0 {
            f64::from(safe_d) / f64::from(res) + 3.0
        } else {
            f64::INFINITY
        };
        let n = shells(d, m).min(found_at);
        if 8.0 * n * n * n > 2e7 {
            d = g.pick(&[4.0, 8.0, 10.0]);
            m = g.pick(&[-1, 8]);
            if shells(d, m) > 60.0 {
                m = 8;
            }
        }
    }
    (d, m)
}

pub(super) fn mcl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut m = match g.below(12) {
        0..=2 => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            let mut m = grid(g, w, h);
            crumple(g, &mut m, 3);
            m
        }
        3 | 4 => {
            let mut m = mesh(g, heavy);
            let far =
                m.v.iter()
                    .flatten()
                    .fold(0f32, |a, &c| if c.abs() > a { c.abs() } else { a });
            if far.is_finite() && far > 12.0 {
                for c in m.v.iter_mut().flatten() {
                    *c = *c / far * 12.0;
                }
            }
            m
        }
        5 | 6 => {
            let r = g.pick(&[3.0, 5.0, 8.0]);
            ico(g, 1, r)
        }
        7 => {
            let r = g.pick(&[4.0, 6.0, 10.0]);
            ico(g, 2, r)
        }
        8 | 9 => sheets(g),
        _ => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            grid(g, w, h)
        }
    };
    let beyond = g.chance(10);
    if beyond {
        // FS-999: beyond the table along one axis at 1 mm
        let k = g.below(3) as usize;
        let d = g.pick(&[1001.5f32, -1000.75, 1003.0]);
        for p in &mut m.v {
            p[k] += d;
        }
    }
    surface(t, &m);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    ix(t, g.pick(&[-1, -1, -1, 2]));
    let nfrip = if g.chance(5) && nf > 1 {
        g.int(1, (nf - 1).min(3))
    } else {
        0
    };
    ix(t, nfrip);
    for _ in 0..nfrip {
        ix(t, g.int(0, nf - 1));
    }
    let nvrip = if g.chance(5) && nv > 1 {
        g.int(1, 2)
    } else {
        0
    };
    ix(t, nvrip);
    for _ in 0..nvrip {
        ix(t, g.int(0, nv - 1));
    }
    let orig = g.chance(4);
    ix(t, i64::from(orig));
    if orig {
        let s = g.pick(&[0.5f32, 2.0, 1.0]);
        for p in &m.v {
            for c in p {
                fv(t, c * s + 0.25);
            }
        }
    }
    // the second surface's offsets
    for _ in 0..nv {
        for _ in 0..3 {
            fv(t, g.int(-8, 8) as f32 / 4.0);
        }
    }
    let nops = g.int(1, 6);
    ix(t, nops);
    let mut r = 1.0f32;
    let mut table = false;
    // the table's kind (faces or vertices), so the searches mostly fit it
    let mut faces = true;
    for k in 0..nops {
        // a table first, most of the time; then searches of its kind, one in eight of the other
        let op = if k == 0 && !g.chance(10) {
            g.below(4)
        } else if g.chance(8) {
            g.below(22)
        } else if g.chance(6) {
            g.pick(&[0, 1, 2, 3, 14, 19, 20])
        } else if faces {
            g.pick(&[11, 12, 13, 18, 14])
        } else {
            g.pick(&[4, 5, 6, 7, 8, 9, 10, 15, 16, 17])
        };
        if op < 4 {
            faces = op == 0 || op == 2;
        }
        match op {
            0 => {
                t.push("ft".into());
                ix(t, which(g, orig));
                r = if beyond { 1.0 } else { res(g, heavy) };
                fv(t, r);
                table = true;
            }
            1 => {
                t.push("vt".into());
                ix(t, which(g, orig));
                r = if beyond { 1.0 } else { res(g, heavy) };
                fv(t, r);
                table = true;
            }
            2 => {
                t.push("ftd".into());
                r = 1.0;
                table = true;
            }
            3 => {
                t.push("vtd".into());
                ix(t, which(g, orig));
                r = 1.0;
                table = true;
            }
            4 | 5 => {
                t.push("vg".into());
                let p = point(g, &m, heavy);
                for c in p {
                    dv(t, f64::from(c) + if g.chance(4) { 1e-9 } else { 0.0 });
                }
                let (d, mm) = limits(g, heavy, false, 0.0, r);
                dv(t, d);
                ix(t, mm);
            }
            6 | 7 => {
                t.push(g.pick(&["vx", "vs"]).into());
                for c in point(g, &m, heavy) {
                    fv(t, c);
                }
            }
            8 => {
                t.push(g.pick(&["vi", "vit"]).into());
                for c in point(g, &m, heavy) {
                    fv(t, c);
                }
                ix(t, g.pick(&[0, 1, 1]));
            }
            9 | 10 if nv > 0 => {
                t.push(g.pick(&["v2", "vp2", "vs2"]).into());
                ix(t, g.int(0, nv - 1));
            }
            11..=13 => {
                t.push(g.pick(&["fg", "fg", "fn"]).into());
                let p = point(g, &m, heavy);
                for c in p {
                    dv(t, f64::from(c));
                }
                let proj = g.pick(&[-1, 0, 1, 1]);
                let safe = if nfrip == 0 && proj <= 0 && table {
                    nearest(&m, p).sqrt() + 2.0
                } else {
                    f32::INFINITY
                };
                let (d, mm) = limits(g, heavy, true, safe, r);
                dv(t, d);
                ix(t, mm);
                ix(t, proj);
            }
            14 => {
                t.push("bf".into());
                for c in point(g, &m, heavy) {
                    fv(t, c);
                }
                ix(t, which(g, orig));
            }
            15 | 16 => {
                t.push("av".into());
                let (d, mm) = limits(g, heavy, false, 0.0, r);
                dv(t, d);
                ix(t, mm);
            }
            17 => t.push("ax".into()),
            18 => {
                t.push("af".into());
                let proj = g.pick(&[-1, 0, 1, 1]);
                let safe = if nfrip == 0 && proj <= 0 && table && !beyond {
                    4.0
                } else {
                    f32::INFINITY
                };
                let (d, mm) = limits(g, heavy, true, safe, r);
                dv(t, d);
                ix(t, mm);
                ix(t, proj);
            }
            19 if nv > 0 => {
                t.push("mv".into());
                ix(t, g.int(0, nv - 1));
                for _ in 0..3 {
                    fv(t, g.int(-12, 12) as f32 / 4.0);
                }
            }
            20 if nv > 0 => {
                t.push("rip".into());
                ix(t, g.int(0, nv - 1));
            }
            _ => {
                t.push("ax".into());
            }
        }
    }
}

/// The table's searches need a table: `G` without one (as the C++ probe's).
fn need<'a>(table: &'a Option<Mht>, o: &mut Out) -> Option<&'a Mht> {
    if table.is_none() {
        o.s("G");
    }
    table.as_ref()
}

pub(super) fn mcl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let gdiag_no = t.i();
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
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
    g.gdiag_no = -1;
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
    let Some(mut s2) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL2");
        return Ok(());
    };
    g.gdiag_no = gdiag_no;
    for _ in 0..t.i() {
        s.face_mut(t.i() as usize).ripflag = 1;
    }
    for _ in 0..t.i() {
        s.vertex_mut(t.i() as usize).ripflag = 1;
    }
    if t.i() != 0 {
        for v in 0..nv as usize {
            let x = s.vertex_mut(v);
            (x.origx, x.origy, x.origz) = (t.f(), t.f(), t.f());
        }
    }
    for v in 0..nv as usize {
        let x = s2.vertex_mut(v);
        x.x += t.f();
        x.y += t.f();
        x.z += t.f();
        (x.origx, x.origy, x.origz) = (x.x, x.y, x.z);
    }
    let mut table: Option<Mht> = None;
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        match op {
            "ft" | "vt" => {
                let (which, r) = (t.i(), t.f());
                table = Some(if op == "ft" {
                    Mht::create_face_table_resolution(g, io.err, &s, which, r)?
                } else {
                    Mht::create_vertex_table_resolution(g, io.err, &s, which, r)?
                });
            }
            "ftd" => {
                table = Some(Mht::create_face_table(g, io.err, &s)?);
            }
            "vtd" => {
                let which = t.i();
                table = Some(Mht::create_vertex_table(g, io.err, &s, which)?);
            }
            "vg" => {
                let p = [t.d(), t.d(), t.d()];
                let (dmax, mm) = (t.d(), t.i());
                let Some(m) = need(&table, o) else { continue };
                let (v, dist) = m.find_closest_vertex_generic(g, io.err, &s, p, dmax, mm)?;
                o.i(i64::from(v));
                o.d(dist);
            }
            "vx" | "vs" => {
                let p = [t.f(), t.f(), t.f()];
                let Some(m) = need(&table, o) else { continue };
                if op == "vx" {
                    let (v, dist) = m.find_closest_vertex_no_xyz(g, io.err, &s, p)?;
                    o.i(i64::from(v));
                    o.f(dist);
                } else {
                    o.i(i64::from(m.find_closest_set_vertex_no(g, io.err, &s, p)?));
                }
            }
            "vi" | "vit" => {
                let p = [t.f(), t.f(), t.f()];
                let glob = t.i();
                let Some(m) = need(&table, o) else { continue };
                if op == "vi" {
                    o.i(i64::from(
                        m.find_vno_of_closest_vertex_in_table(g, io.err, &s, p, glob)?,
                    ));
                } else {
                    let v = m.find_closest_vertex_in_table(g, io.err, &s, p, glob)?;
                    o.i(v.map_or(-1, |v| v as i64));
                }
            }
            "v2" | "vp2" | "vs2" => {
                let vno = t.i();
                let Some(m) = need(&table, o) else { continue };
                match op {
                    "v2" => {
                        let (v, dist) =
                            m.find_closest_vertex_no2(g, io.out, io.err, &s, &s2, vno)?;
                        o.i(i64::from(v));
                        o.f(dist);
                    }
                    "vp2" => {
                        let (v, dist) = m.find_closest_vertex2(g, io.out, io.err, &s, &s2, vno)?;
                        o.i(v.map_or(-1, |v| v as i64));
                        o.f(dist);
                    }
                    _ => {
                        let v = m.find_closest_vertex_set2(g, io.out, io.err, &s, &s2, vno)?;
                        o.i(v.map_or(-1, |v| v as i64));
                    }
                }
            }
            "fg" | "fn" => {
                let p = [t.d(), t.d(), t.d()];
                let (dmax, mm, proj) = (t.d(), t.i(), t.i());
                let Some(m) = need(&table, o) else { continue };
                let (f, dist) = if op == "fg" {
                    m.find_closest_face_generic(g, io.err, &s, p, dmax, mm, proj)?
                } else {
                    m.find_closest_face_no_generic(g, io.err, &s, p, dmax, mm, proj)?
                };
                o.i(i64::from(f));
                o.d(dist);
            }
            "bf" => {
                let p = [t.f(), t.f(), t.f()];
                let which = t.i();
                let (f, dist) = mhtfind::brute_force_closest_face(g, io.err, &s, p, which)?;
                o.i(i64::from(f));
                o.f(dist);
            }
            "av" => {
                let (dmax, mm) = (t.d(), t.i());
                let Some(m) = need(&table, o) else { continue };
                for v in 0..nv as usize {
                    let x = s2.vertex(v);
                    let p = [f64::from(x.x), f64::from(x.y), f64::from(x.z)];
                    let (r, dist) = m.find_closest_vertex_generic(g, io.err, &s, p, dmax, mm)?;
                    o.i(i64::from(r));
                    o.d(dist);
                }
            }
            "ax" => {
                let Some(m) = need(&table, o) else { continue };
                for v in 0..nv as usize {
                    let x = s2.vertex(v);
                    let (r, dist) = m.find_closest_vertex_no_xyz(g, io.err, &s, [x.x, x.y, x.z])?;
                    o.i(i64::from(r));
                    o.f(dist);
                }
            }
            "af" => {
                let (dmax, mm, proj) = (t.d(), t.i(), t.i());
                let Some(m) = need(&table, o) else { continue };
                for v in 0..nv as usize {
                    let x = s2.vertex(v);
                    let p = [f64::from(x.x), f64::from(x.y), f64::from(x.z)];
                    let (f, dist) = m.find_closest_face_generic(g, io.err, &s, p, dmax, mm, proj)?;
                    o.i(i64::from(f));
                    o.d(dist);
                }
            }
            "mv" => {
                let vno = t.i() as usize;
                let x = s.vertex_mut(vno);
                x.x += t.f();
                x.y += t.f();
                x.z += t.f();
            }
            "rip" => {
                s.vertex_mut(t.i() as usize).ripflag = 1;
            }
            _ => panic!("mcl op {op}"),
        }
    }
    Ok(())
}

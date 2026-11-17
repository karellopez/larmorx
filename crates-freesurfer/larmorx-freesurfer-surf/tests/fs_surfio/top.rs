// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 42 of track D: `top`, the rest of `mrisurf_topology.cpp`
//! (`toprest.rs`): a surface read by `MRISread` (`d/lh.s0`: vls's open grids, subdivided icosahedra
//! and srd's small meshes), its neighbourhood set to 1 to 3 hops (with its distances or not), its
//! type (3, or 0: a quadrangle surface), every vertex's rip flag and mark and every face's rip flag
//! from the case, `Gdiag`; then 1 to 6 operations named by their FreeSurfer functions (`freedists`:
//! `MRISfreeDistsButNotOrig` and `MRISfreeDistOrigs`, which the renumbering needs first); after each
//! the surface's counts and every vertex's rip flag, mark, faces with their places, counts,
//! neighbour list (`mrisVertexVSize` entries) and distances, every face's vertices and rip flag:
//! the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::surface;
use super::mpd::{grid, ico};
use super::srd::{Mesh, mesh};
use super::*;

use larmorx_freesurfer_surf::{dist, nbhd, readwrite, toprest, valrest};

/// The directed edges of the mesh's faces (a face's vertex and the next).
fn edges(m: &Mesh) -> Vec<(i64, i64)> {
    let mut e = Vec::new();
    for f in &m.f {
        for k in 0..3 {
            e.push((f[k], f[(k + 1) % 3]));
        }
    }
    e
}

pub(super) fn top_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0..=3 => {
            let (w, h) = (g.int(2, 5), g.int(2, 4));
            grid(g, w, h)
        }
        4 | 5 => mesh(g, heavy),
        6 if g.chance(3) => ico(g, 2, 10.0),
        _ => ico(g, 1, 10.0),
    };
    surface(t, &m);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    let e = edges(&m);
    let nsize = g.pick(&[1, 1, 2, 3]);
    ix(t, nsize);
    let dmode = i64::from(g.chance(3));
    ix(t, dmode);
    ix(t, if g.chance(4) { 0 } else { 3 });
    let rips = g.chance(2);
    for _ in 0..nv {
        ix(
            t,
            if rips && g.chance(6) {
                g.pick(&[1, 1, 1, 1, 1, 2, -1])
            } else {
                0
            },
        );
        ix(t, g.pick(&[0, 0, 1]));
    }
    let frips = g.chance(4);
    for _ in 0..nf {
        ix(t, i64::from(frips && g.chance(8)));
    }
    ix(t, if g.chance(8) { 0x1000_0040 } else { 0 });
    let vertex = |g: &mut Gen| if nv > 0 { g.int(0, nv - 1) } else { 0 };
    let face = |g: &mut Gen| if nf > 0 { g.int(0, nf - 1) } else { 0 };
    let edge = |g: &mut Gen| -> (i64, i64) {
        if !e.is_empty() && !g.chance(6) {
            let (a, b) = e[g.below(e.len() as u64) as usize];
            if g.chance(2) { (a, b) } else { (b, a) }
        } else {
            (vertex(g), vertex(g))
        }
    };
    let nops = g.int(1, 6);
    let mut ops: Vec<String> = Vec::new();
    let mut count = 0i64;
    let outer = t;
    let t = &mut ops;
    for _ in 0..nops {
        count += 1;
        if nv == 0 {
            t.push(
                g.pick(&[
                    "MRIScomputeEulerNumber",
                    "mrisCountTotalNeighbors",
                    "MRIStopologicalDefectIndex",
                ])
                .into(),
            );
            continue;
        }
        match g.below(27) {
            0 | 1 => {
                t.push(g.pick(&["edgeExists", "mrisCountValidLinks"]).into());
                let (a, b) = edge(g);
                ix(t, a);
                ix(t, b);
            }
            2 => {
                t.push("mrisAddEdge".into());
                let a = vertex(g);
                let b = if g.chance(10) { a } else { vertex(g) };
                ix(t, a);
                ix(t, b);
            }
            3 | 4 => {
                t.push("mrisRemoveEdge".into());
                let (a, b) = edge(g);
                ix(t, a);
                ix(t, b);
            }
            5 | 6 => {
                t.push("mrisRemoveVertexLink".into());
                let (a, b) = edge(g);
                ix(t, a);
                ix(t, b);
            }
            7 | 8 => t.push("MRISremoveTriangleLinks".into()),
            9 => t.push(
                g.pick(&[
                    "mrisCountTotalNeighbors",
                    "MRIScomputeEulerNumber",
                    "MRIStopologicalDefectIndex",
                    "mrisStoreVtotalInV3num",
                ])
                .into(),
            ),
            10 => {
                t.push("mrisVertexReplacingNeighbors".into());
                ix(t, vertex(g));
                ix(t, g.int(0, 8));
            }
            11 => t.push("mrisForgetNeighborhoods".into()),
            12 => {
                t.push("MRIScountTotalNeighbors".into());
                ix(
                    t,
                    if g.chance(8) {
                        g.pick(&[-1, 4])
                    } else {
                        g.pick(&[0, 1, 1, 2, 3])
                    },
                );
            }
            13 => {
                t.push("MRISgetNeighborsBeginEnd".into());
                ix(t, vertex(g));
                for _ in 0..2 {
                    ix(
                        t,
                        if g.chance(10) {
                            g.pick(&[0, 4])
                        } else {
                            g.pick(&[1, 2, 3])
                        },
                    );
                }
            }
            14 | 15 => {
                t.push("MRISisSurfaceValid".into());
                ix(t, g.pick(&[0, 0, 1]));
                ix(t, g.pick(&[0, 1, 2, 2]));
            }
            16 => {
                t.push("vertexInFace".into());
                ix(t, vertex(g));
                ix(t, face(g));
            }
            17 | 18 => {
                t.push("mrisRemoveLink".into());
                let (a, b) = edge(g);
                ix(t, a);
                ix(t, b);
            }
            19 => {
                if nf == 0 {
                    t.push("MRISremoveRipped".into());
                    continue;
                }
                t.push("mrisRemoveFace".into());
                ix(t, face(g));
            }
            // the renumbering asserts that no distances are kept (MRISread's surfaces keep
            // theirs): mostly released first
            20 | 26 => {
                // the faces of the ripped vertices ripped first, mostly (else FS-995)
                if !g.chance(4) {
                    t.push("MRISsetRipInFacesWithRippedVertices".into());
                    count += 1;
                }
                if !g.chance(8) {
                    t.push("freedists".into());
                    count += 1;
                }
                t.push("MRISrenumberRemovingRippedFacesAndVertices".into());
            }
            21 | 22 => t.push("MRISremoveRipped".into()),
            23 => t.push(
                g.pick(&["MRISevertSurface", "MRISsetRipInFacesWithRippedVertices"])
                    .into(),
            ),
            24 => {
                t.push("MRISgetSpatOrderedNeighbors".into());
                ix(t, vertex(g));
            }
            25 => {
                if g.chance(2) {
                    t.push("MRIS_setNsizeCur".into());
                    ix(t, vertex(g));
                    ix(t, g.pick(&[0, 1, 1, 2, 3]));
                } else {
                    t.push("rip".into());
                    ix(t, vertex(g));
                    ix(t, g.pick(&[1, 1, 0, 2]));
                }
            }
            _ => unreachable!(),
        }
    }
    ix(outer, count);
    outer.append(&mut ops);
}

fn dump(s: &Mris, o: &mut Out) {
    for x in [
        i64::from(s.nvertices),
        i64::from(s.nfaces),
        i64::from(s.nsize),
        i64::from(s.max_nsize),
        i64::from(s.triangle_links_removed),
        i64::from(s.dist_alloced_flags),
        i64::from(s.dist_nsize),
    ] {
        o.i(x);
    }
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        let vt = s.topology(vno);
        o.i(i64::from(v.ripflag));
        o.i(i64::from(v.marked));
        o.i(i64::from(vt.num));
        for k in 0..usize::from(vt.num) {
            o.i(i64::from(vt.f.as_ref().expect("f")[k]));
            o.i(i64::from(vt.n.as_ref().expect("n")[k]));
        }
        for x in [vt.vnum, vt.v2num, vt.v3num, vt.vtotal] {
            o.i(i64::from(x));
        }
        o.i(i64::from(vt.nsizeMax));
        o.i(i64::from(vt.nsizeCur));
        let vs = topology::vertex_vsize(s, vno);
        o.i(i64::from(vs));
        for k in 0..vs.max(0) as usize {
            o.i(i64::from(vt.v.as_ref().expect("v")[k]));
        }
        match v.dist.as_ref() {
            None => o.i(0),
            Some(d) => {
                o.i(1);
                let n = i32::from(vt.vtotal).min(v.dist_capacity).max(0) as usize;
                for &x in &d[..n] {
                    o.f(x);
                }
            }
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        let f = s.face(fno);
        for x in f.v {
            o.i(i64::from(x));
        }
        o.i(i64::from(f.ripflag));
    }
}

pub(super) fn top_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
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
    let (nsize, dmode) = (t.i(), t.i());
    if dmode == 1 {
        nbhd::set_neighborhood_size_and_dist(g, sg, &mut io, &mut s, nsize)?;
    } else if nsize > 1 {
        nbhd::set_neighborhood_size(g, sg, &mut io, &mut s, nsize)?;
    }
    s.r#type = t.i();
    for vno in 0..nv as usize {
        let (rip, marked) = (t.i() as i8, t.i() as i16);
        let v = s.vertex_mut(vno);
        v.ripflag = rip;
        v.marked = marked;
    }
    for fno in 0..nf as usize {
        s.face_mut(fno).ripflag = t.i() as i8;
    }
    g.gdiag = t.i() as u32 as u64;
    // a vertex or face the surface no longer has (after a renumbering): `G`, skipped on both sides
    let gv = |s: &Mris, o: &mut Out, xs: &[i32]| {
        let bad = xs.iter().any(|&a| a < 0 || a >= s.nvertices);
        if bad {
            o.s("G");
        }
        bad
    };
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "edgeExists" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                o.i(i64::from(toprest::edge_exists(&s, a as usize, b)));
            }
            "mrisCountValidLinks" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                o.i(i64::from(toprest::count_valid_links(&s, a as usize, b)));
            }
            "mrisAddEdge" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                toprest::add_edge(g, sg, &mut io, &mut s, a as usize, b as usize);
            }
            "mrisRemoveEdge" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                toprest::remove_edge(g, sg, io.err, &mut s, a as usize, b);
            }
            "mrisRemoveVertexLink" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                o.i(i64::from(toprest::remove_vertex_link(
                    g, sg, io.err, &mut s, a as usize, b,
                )?));
            }
            "MRISremoveTriangleLinks" => {
                o.i(i64::from(toprest::remove_triangle_links(
                    g, sg, &mut io, &mut s,
                )?));
            }
            "mrisCountTotalNeighbors" => {
                o.i(i64::from(toprest::count_total_neighbors_in_use(&s)));
            }
            "MRIScomputeEulerNumber" => {
                let c = toprest::compute_euler_number(&s);
                for x in [c.euler, c.nvertices, c.nfaces, c.nedges] {
                    o.i(i64::from(x));
                }
            }
            "MRIStopologicalDefectIndex" => {
                o.i(i64::from(toprest::topological_defect_index(&s)));
            }
            "mrisStoreVtotalInV3num" => o.i(i64::from(toprest::store_vtotal_in_v3num(&mut s))),
            "mrisVertexReplacingNeighbors" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a]) {
                    continue;
                }
                toprest::vertex_replacing_neighbors(g, sg, io.err, &mut s, a as usize, b);
            }
            "mrisForgetNeighborhoods" => toprest::forget_neighborhoods(g, sg, io.err, &mut s),
            "MRIScountTotalNeighbors" => {
                let n = t.i();
                o.i(i64::from(toprest::count_total_neighbors(
                    g, &mut io, &s, n,
                )?));
            }
            "MRISgetNeighborsBeginEnd" => {
                let (vno, a, b) = (t.i(), t.i(), t.i());
                if gv(&s, o, &[vno]) {
                    continue;
                }
                let (x, y) = toprest::get_neighbors_begin_end(
                    g,
                    &mut io,
                    &s,
                    vno as usize,
                    a as i64 as usize,
                    b as i64 as usize,
                )?;
                o.i(x as i64);
                o.i(y as i64);
            }
            "MRISisSurfaceValid" => {
                let (a, b) = (t.i(), t.i());
                o.i(i64::from(toprest::is_surface_valid(
                    g, &mut io, &mut s, a, b,
                )?));
            }
            "vertexInFace" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a]) {
                    continue;
                }
                if b < 0 || b >= s.nfaces {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(toprest::vertex_in_face(&s, a as usize, b)));
            }
            "mrisRemoveLink" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a, b]) {
                    continue;
                }
                o.i(i64::from(toprest::remove_link(
                    g, sg, io.err, &mut s, a as usize, b as usize,
                )));
            }
            "mrisRemoveFace" => {
                let f = t.i();
                if f < 0 || f >= s.nfaces {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(toprest::remove_face(
                    g, sg, io.err, &mut s, f as usize,
                )));
            }
            "MRISrenumberRemovingRippedFacesAndVertices" => {
                toprest::renumber_removing_ripped_faces_and_vertices(g, sg, &mut io, &mut s)?;
                // the vertex number -1 in a face or a kept neighbour list (FS-995 reproduced):
                // FreeSurfer's next reads go outside the vertices: both sides stop here
                let minus = (0..s.nfaces.max(0) as usize).any(|f| s.face(f).v.contains(&-1))
                    || (0..s.nvertices.max(0) as usize).any(|v| {
                        let vt = s.topology(v);
                        let n = vt.v3num.max(0) as usize;
                        vt.v.as_ref().is_some_and(|l| l[..n].iter().any(|&x| x < 0))
                    });
                if minus {
                    dump(&s, o);
                    o.s("STOP");
                    return Ok(());
                }
            }
            "MRISremoveRipped" => toprest::remove_ripped(g, sg, &mut io, &mut s)?,
            "MRISevertSurface" => o.i(i64::from(toprest::evert_surface(g, &mut io, &mut s)?)),
            "MRISgetSpatOrderedNeighbors" => {
                let vno = t.i();
                if gv(&s, o, &[vno]) {
                    continue;
                }
                let l = toprest::get_spat_ordered_neighbors(g, &s, vno as usize);
                o.i(l.len() as i64);
                for x in l {
                    o.i(i64::from(x));
                }
            }
            "MRISsetRipInFacesWithRippedVertices" => {
                valrest::set_rip_in_faces_with_ripped_vertices(&mut s)
            }
            "MRIS_setNsizeCur" => {
                let (a, b) = (t.i(), t.i());
                if gv(&s, o, &[a]) {
                    continue;
                }
                nbhd::set_nsize_cur(g, &mut io, &mut s, a as usize, b)?;
            }
            "rip" => {
                let (a, r) = (t.i(), t.i());
                if gv(&s, o, &[a]) {
                    continue;
                }
                s.vertex_mut(a as usize).ripflag = r as i8;
            }
            "freedists" => {
                dist::free_dists_but_not_orig(&mut s);
                dist::free_dist_origs(&mut s);
            }
            _ => panic!("top op {op}"),
        }
        dump(&s, o);
    }
    Ok(())
}

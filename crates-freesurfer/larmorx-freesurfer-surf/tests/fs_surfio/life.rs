// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's commands of queue item 3.1-47 part a (relay 9): `life` (a surface's allocation,
//! reallocations, counts, rip flags, arrays and dumps, then its state and its free), `meta`
//! (`MRIScopyMetadata`), `misc` (the curvature names, the registration's scales, the column
//! printing, the progress bar, the vector searches, `signum_eval`, the sort comparisons) and
//! `edges` (`edgesIntersect`): their generators and the port's answers, as `fssurfio.cpp`'s.

use super::*;

use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::basemisc::{self, FaceSort, VertexSort};
use larmorx_freesurfer_surf::edges::{self, Edge};
use larmorx_freesurfer_surf::lta;

fn floats(t: &mut Vec<String>, g: &mut Gen, n: usize) {
    for _ in 0..n {
        fx(t, g);
    }
}

/// A count now and then out of range (negative).
fn count(g: &mut Gen, hi: i64) -> i64 {
    if g.chance(40) {
        g.pick(&[-1, -3])
    } else {
        g.int(0, hi)
    }
}

/// The counts a `life` case's operations leave, as FreeSurfer's code defines them (the
/// generator keeps most operations inside what the assertions allow: one in about 15 not).
#[derive(Default)]
struct LifeState {
    nv: i64,
    maxv: i64,
    nf: i64,
    maxf: i64,
    frozen: i64,
}

/// A value for an operation whose assertion wants `0 <= v <= hi`: mostly inside, now and then
/// above or negative.
fn within(g: &mut Gen, hi: i64) -> i64 {
    if g.chance(15) {
        if g.chance(3) {
            -1
        } else {
            hi.max(0) + g.int(1, 2)
        }
    } else {
        g.int(0, hi.max(0))
    }
}

pub fn life_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let mut s = LifeState::default();
    if g.chance(2) {
        t.push("A".to_owned());
        s.nv = count(g, 6);
        s.nf = count(g, 4);
        (s.maxv, s.maxf) = (s.nv, s.nf);
        ix(t, s.nv);
        ix(t, s.nf);
    } else {
        t.push("O".to_owned());
        let (mv, mf) = (g.int(0, 8), g.int(0, 6));
        s.nv = count(g, 6);
        s.nf = count(g, 4);
        (s.maxv, s.maxf) = (mv.max(s.nv), mf.max(s.nf));
        for v in [mv, mf, s.nv, s.nf] {
            ix(t, v);
        }
    }
    let nops = g.int(1, 8);
    ix(t, nops);
    let ops = [
        "rv", "rv", "rf", "rf", "af", "ov", "ra", "gv", "tv", "xv", "gf", "tf", "xf", "fz", "sv",
        "sv", "sv", "st", "st", "sd", "sf", "sf", "ex", "im", "cr", "so", "re", "ur", "ga", "gn",
        "eg", "cn", "dv", "df", "ds",
    ];
    let mut k = 0;
    while k < nops {
        k += 1;
        let op = if s.frozen > 0 && !g.chance(3) {
            "fr"
        } else {
            g.pick(&ops)
        };
        t.push(op.to_owned());
        match op {
            "rv" => {
                let a = g.int(0, 9);
                let room = a.max(s.maxv);
                let b = within(g, room);
                ix(t, a);
                ix(t, b);
                if b >= 0 && b <= room && s.frozen == 0 && room > 0 {
                    (s.maxv, s.nv) = (room, b);
                }
            }
            "rf" | "af" => {
                let a = if g.chance(15) { 0 } else { g.int(1, 6) };
                let b = if op == "af" { a } else { within(g, a) };
                ix(t, a);
                if op == "rf" {
                    ix(t, b);
                }
                if (0..=a).contains(&b) {
                    (s.maxf, s.nf) = (a, b);
                }
            }
            "ov" | "ra" => {
                let (c, d) = (count(g, 7), count(g, 5));
                let (a, b) = if op == "ov" {
                    (g.int(0, 9), g.int(1, 6))
                } else {
                    (c, d)
                };
                if op == "ov" {
                    ix(t, a);
                    ix(t, b);
                }
                ix(t, c);
                ix(t, d);
                let room = a.max(c).max(s.maxv);
                if c >= 0 && d >= 0 && s.frozen == 0 {
                    if room > 0 {
                        (s.maxv, s.nv) = (room, c);
                    }
                    (s.maxf, s.nf) = (b.max(d), d);
                }
            }
            "gv" => {
                let n = within(g, s.maxv);
                ix(t, n);
                if n <= s.maxv {
                    s.nv = n;
                }
            }
            "tv" | "xv" => {
                let n = within(g, s.nv);
                ix(t, n);
                if n <= s.nv {
                    s.nv = n;
                }
            }
            "gf" => {
                let n = within(g, s.maxf);
                ix(t, n);
                if n <= s.maxf {
                    s.nf = n;
                }
            }
            "tf" | "xf" => {
                let n = within(g, s.nf);
                ix(t, n);
                if n <= s.nf {
                    s.nf = n;
                }
            }
            "fz" => s.frozen += 1,
            "fr" => s.frozen -= 1,
            "sv" => {
                ix(t, g.int(0, 8));
                floats(t, g, 8);
                ix(t, g.pick(&[0, 0, 1, -1, 2]));
                ix(t, g.int(0, 3));
            }
            "st" => {
                ix(t, g.int(0, 7));
                let vnum = g.int(0, 4);
                let vtotal = g.int(0, 5);
                let num = g.int(0, 3);
                for v in [vnum, vtotal, g.int(0, 3), g.int(0, 3), num] {
                    ix(t, v);
                }
                for _ in 0..vtotal + 2 * num {
                    ix(t, g.int(-1, 8));
                }
            }
            "sd" | "dv" => ix(t, g.int(0, 7)),
            "df" => ix(t, g.int(0, 5)),
            "sf" => {
                ix(t, g.int(0, 6));
                for _ in 0..3 {
                    ix(t, if g.chance(10) { 9 } else { g.int(0, 5) });
                }
                ix(t, g.pick(&[0, 0, 1, -1]));
                ix(t, g.int(0, 3));
                floats(t, g, 4);
                ix(t, g.int(0, 3));
            }
            "im" => ix(t, g.int(0, 5)),
            _ => {}
        }
    }
}

fn life_has_vertices(m: &Mris) -> bool {
    m.vertices.is_some()
        && m.vertices_topology.is_some()
        && m.dist_storage.is_some()
        && m.dist_orig_storage.is_some()
}

fn dump_ok(m: &Mris, vno: i32) -> bool {
    if vno < 0 || vno >= m.nvertices || !life_has_vertices(m) {
        return false;
    }
    let vno = vno as usize;
    let vt = m.topology(vno);
    let v = m.vertex(vno);
    let vs = topology::vertex_vsize(m, vno);
    if vs > 0 {
        if !vt.v.as_ref().is_some_and(|a| a.len() as i32 >= vs) {
            return false;
        }
        if v.dist.is_some() && v.dist_capacity < vs {
            return false;
        }
        if v.dist_orig.is_some() && v.dist_orig_capacity < vs {
            return false;
        }
    }
    if vt.num > 0
        && (vt.n.is_none()
            || !vt
                .f
                .as_ref()
                .is_some_and(|f| f.len() >= usize::from(vt.num)))
    {
        return false;
    }
    true
}

fn dump_life(m: &Mris, o: &mut Out) {
    for x in [
        m.nvertices,
        m.max_vertices,
        m.nfaces,
        m.max_faces,
        m.nverticesFrozen,
        i32::from(m.nsize),
        i32::from(m.max_nsize),
        i32::from(m.dist_alloced_flags),
    ] {
        o.i(i64::from(x));
    }
    for b in [
        m.vertices.is_some(),
        m.vertices_topology.is_some(),
        m.dist_storage.is_some(),
        m.dist_orig_storage.is_some(),
        m.faces.is_some(),
        m.faceNormCacheEntries.is_some(),
        m.faceNormDeferredEntries.is_some(),
        m.dx2.is_some(),
        m.dy2.is_some(),
        m.dz2.is_some(),
    ] {
        o.i(i64::from(b));
    }
    if life_has_vertices(m) {
        for vno in 0..m.max_vertices.max(0) as usize {
            let v = m.vertex(vno);
            let vt = m.topology(vno);
            for x in [v.x, v.y, v.z, v.odx, v.dz] {
                o.f(x);
            }
            for x in [
                i64::from(v.ripflag),
                i64::from(v.oripflag),
                i64::from(v.marked),
                i64::from(vt.vnum),
                i64::from(vt.vtotal),
                i64::from(vt.nsizeMax),
                i64::from(vt.nsizeCur),
                i64::from(vt.num),
                i64::from(vt.v.is_some()),
                i64::from(vt.f.is_some()),
            ] {
                o.i(x);
            }
            dump_dists(m, vno, o);
        }
    }
    if let (Some(fs), Some(cs), Some(ds)) = (
        &m.faces,
        &m.faceNormCacheEntries,
        &m.faceNormDeferredEntries,
    ) {
        for fno in 0..m.max_faces.max(0) as usize {
            let f = &fs[fno];
            for x in [
                f.v[0],
                f.v[1],
                f.v[2],
                i32::from(f.ripflag),
                i32::from(f.oripflag),
                f.marked,
            ] {
                o.i(i64::from(x));
            }
            let e = cs[fno];
            for x in [e.nx, e.ny, e.nz, e.orig_area] {
                o.f(x);
            }
            o.i(i64::from(ds[fno].deferred));
        }
    }
}

fn print_flags(o: &mut Out, p: Option<Vec<i8>>, n: i32) {
    match p {
        None => o.s("NULL"),
        Some(p) => {
            for k in 0..n.max(0) as usize {
                o.i(i64::from(p[k]));
            }
        }
    }
}

fn faces_ok(m: &Mris) -> bool {
    !(m.nfaces > 0 && m.faces.is_none())
}
fn vertices_ok(m: &Mris) -> bool {
    !(m.nvertices > 0 && m.vertices.is_none())
}

pub fn life_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let (g, a, b, sg) = (&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg);
    let mut m = if t.next() == "A" {
        let (nv, nf) = (t.i(), t.i());
        alloc::alloc(g, a, b, sg, nv, nf)?
    } else {
        let (mv, mf, nv, nf) = (t.i(), t.i(), t.i(), t.i());
        alloc::over_alloc(g, a, b, sg, mv, mf, nv, nf)?
    };
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        match op {
            "rv" => {
                let (x, y) = (t.i(), t.i());
                if m.nvertices < 0 {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(alloc::realloc_vertices(
                    g, a, b, sg, &mut m, x, y,
                )?));
            }
            "rf" => {
                let (x, y) = (t.i(), t.i());
                if m.nfaces < 0 {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(alloc::realloc_faces(g, a, b, &mut m, x, y)?));
            }
            "af" => {
                let x = t.i();
                if m.nfaces < 0 {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(alloc::allocate_faces(g, a, b, &mut m, x)?));
            }
            "ov" => {
                let (w, x, y, z) = (t.i(), t.i(), t.i(), t.i());
                if m.nvertices < 0 || m.nfaces < 0 {
                    o.s("G");
                    continue;
                }
                alloc::over_alloc_vertices_and_faces(g, a, b, sg, &mut m, w, x, y, z)?;
            }
            "ra" => {
                let (x, y) = (t.i(), t.i());
                if m.nvertices < 0 || m.nfaces < 0 {
                    o.s("G");
                    continue;
                }
                alloc::realloc_vertices_and_faces(g, a, b, sg, &mut m, x, y)?;
            }
            "gv" => alloc::grow_nvertices(g, a, b, &mut m, t.i())?,
            "tv" => alloc::truncate_nvertices(g, a, b, &mut m, t.i())?,
            "xv" => alloc::removed_vertices(g, a, b, &mut m, t.i())?,
            "gf" => alloc::grow_nfaces(g, a, b, &mut m, t.i())?,
            "tf" => alloc::truncate_nfaces(g, a, b, &mut m, t.i())?,
            "xf" => alloc::removed_faces(g, a, b, &mut m, t.i())?,
            "fz" => surfbase::acquire_nvertices_frozen(&mut m),
            "fr" => surfbase::release_nvertices_frozen(&mut m),
            "sv" => {
                let vno = t.i();
                let c: Vec<f32> = (0..8).map(|_| t.f()).collect();
                let (rip, marked) = (t.i(), t.i());
                if vno >= m.max_vertices || !life_has_vertices(&m) {
                    o.s("G");
                    continue;
                }
                let v = m.vertex_mut(vno as usize);
                (v.x, v.y, v.z) = (c[0], c[1], c[2]);
                (v.odx, v.ody, v.odz, v.dx, v.dy) = (c[3], c[4], c[5], c[6], c[7]);
                (v.dz, v.nx, v.ny, v.nz) = (c[0], c[1], c[2], c[3]);
                v.ripflag = rip as i8;
                v.marked = marked as i16;
            }
            "st" => {
                let (vno, vnum, vtotal, smax, scur, num) =
                    (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
                let nb: Vec<i32> = (0..vtotal.max(0)).map(|_| t.i()).collect();
                let fs: Vec<i32> = (0..num).map(|_| t.i()).collect();
                let ns: Vec<u8> = (0..num).map(|_| t.i() as u8).collect();
                if vno >= m.max_vertices || !life_has_vertices(&m) {
                    o.s("G");
                    continue;
                }
                let vt = m.topology_mut(vno as usize);
                vt.vnum = vnum as i16;
                vt.vtotal = vtotal as i16;
                vt.nsizeMax = smax as u8;
                vt.nsizeCur = scur as u8;
                vt.num = num as u8;
                vt.v = Some(nb);
                vt.f = Some(fs);
                vt.n = Some(ns);
            }
            "sd" => {
                let vno = t.i();
                if vno >= m.nvertices || !life_has_vertices(&m) {
                    o.s("G");
                    continue;
                }
                dist::make_dist(&mut m, vno as usize);
                let v = m.vertex_mut(vno as usize);
                let cap = v.dist_capacity.max(0) as usize;
                if let Some(d) = v.dist.as_mut() {
                    for (i, x) in d.iter_mut().take(cap).enumerate() {
                        *x = (i as i32 + 1 + 10 * vno) as f32;
                    }
                }
            }
            "sf" => {
                let (fno, v0, v1, v2, rip, marked) = (t.i(), t.i(), t.i(), t.i(), t.i(), t.i());
                let n: Vec<f32> = (0..4).map(|_| t.f()).collect();
                let deferred = t.i();
                if fno >= m.max_faces || m.faces.is_none() {
                    o.s("G");
                    continue;
                }
                let f = m.face_mut(fno as usize);
                f.v = [v0, v1, v2];
                f.ripflag = rip as i8;
                f.marked = marked;
                let e = &mut m.faceNormCacheEntries.as_mut().unwrap()[fno as usize];
                (e.nx, e.ny, e.nz, e.orig_area) = (n[0], n[1], n[2], n[3]);
                m.faceNormDeferredEntries.as_mut().unwrap()[fno as usize].deferred = deferred as i8;
            }
            "ex" => {
                if !faces_ok(&m) || !vertices_ok(&m) {
                    o.s("G");
                    continue;
                }
                print_flags(o, basemisc::export_face_ripflags(&m), m.nfaces);
                print_flags(o, basemisc::export_vertex_ripflags(&m), m.nvertices);
            }
            "im" => {
                let s = t.i();
                if !faces_ok(&m) || !vertices_ok(&m) {
                    o.s("G");
                    continue;
                }
                let ff: Vec<i8> = (0..m.nfaces.max(0))
                    .map(|i| ((i * 7 + s) % 3 - 1) as i8)
                    .collect();
                let vf: Vec<i8> = (0..m.nvertices.max(0))
                    .map(|i| ((i * 5 + s) % 3 - 1) as i8)
                    .collect();
                basemisc::import_face_ripflags(&mut m, &ff);
                basemisc::import_vertex_ripflags(&mut m, &vf);
            }
            "cr" => {
                if !vertices_ok(&m) {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(basemisc::count_ripped(&m)));
            }
            "so" | "re" | "ur" => {
                if !faces_ok(&m) || !vertices_ok(&m) {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(match op {
                    "so" => basemisc::store_rip_flags(&mut m),
                    "re" => basemisc::restore_rip_flags(&mut m),
                    _ => basemisc::unrip(&mut m),
                }));
            }
            "ga" => {
                if !faces_ok(&m) || !vertices_ok(&m) || m.nvertices < 0 || m.nfaces < 0 {
                    o.s("G");
                    continue;
                }
                for x in basemisc::get_vertex_array(&m) {
                    o.f(x);
                }
                for x in basemisc::get_vertex_normal_array(&m) {
                    o.f(x);
                }
                for x in basemisc::get_face_array(&m) {
                    o.i(i64::from(x));
                }
            }
            "gn" => {
                let mut ok = m.nfaces >= 0
                    && (m.nfaces == 0 || (m.faces.is_some() && life_has_vertices(&m)));
                if ok {
                    for fno in 0..m.nfaces as usize {
                        for k in 0..3 {
                            let v = m.face(fno).v[k];
                            if v < 0 || v >= m.max_vertices {
                                ok = false;
                            }
                        }
                    }
                }
                if !ok {
                    o.s("G");
                    continue;
                }
                for x in basemisc::get_face_normal_array(&mut m) {
                    o.f(x);
                }
            }
            "eg" => o.i(i64::from(basemisc::alloc_extra_gradients(g, b, &mut m)?)),
            "cn" => {
                if !vertices_ok(&m) {
                    o.s("G");
                    continue;
                }
                basemisc::check_for_nans(g, a, b, &m)?;
            }
            "dv" => {
                let vno = t.i();
                if !dump_ok(&m, vno) {
                    o.s("G");
                    continue;
                }
                alloc::dump_vertex(a, &m, vno as usize);
            }
            "df" => {
                let fno = t.i();
                if fno < 0 || fno >= m.nfaces || m.faces.is_none() {
                    o.s("G");
                    continue;
                }
                alloc::dump_face(&m, fno as usize, a);
            }
            "ds" => {
                let mut ok = true;
                for vno in 0..m.nvertices.min(10) {
                    ok = ok && dump_ok(&m, vno);
                }
                if !faces_ok(&m) {
                    ok = false;
                }
                if !ok {
                    o.s("G");
                    continue;
                }
                alloc::dump_shape(a, &m);
            }
            op => panic!("life op {op}"),
        }
    }
    o.0 += "| ";
    dump_life(&m, o);
    let mut pm = Some(m);
    alloc::free(g, b, &mut pm);
    o.i(i64::from(pm.is_none()));
    Ok(())
}

fn printable(g: &mut Gen, lo: i64, hi: i64) -> Vec<u8> {
    let n = g.int(lo, hi) as usize;
    (0..n).map(|_| g.int(0x21, 0x7e) as u8).collect()
}

fn meta_tokens_one(t: &mut Vec<String>, g: &mut Gen) {
    ix(t, g.int(0, 3));
    ix(t, g.int(0, 9));
    ix(t, g.int(0, 2));
    ix(t, i64::from(g.chance(2)));
    ix(t, g.int(0, 1));
    for _ in 0..3 {
        ix(t, g.int(0, 256));
    }
    floats(t, g, 3);
    ix(t, g.int(0, 1));
    floats(t, g, 15);
    let name = printable(g, 0, 20);
    hx(t, &name);
    let nx = if g.chance(2) { 0 } else { g.int(1, 3) };
    ix(t, nx);
    if nx > 0 {
        ix(t, g.int(0, 2));
    }
    for _ in 0..2 {
        let has = g.chance(2);
        ix(t, i64::from(has));
        if has {
            floats(t, g, 16);
        }
    }
}

pub fn meta_tokens(t: &mut Vec<String>, g: &mut Gen) {
    ix(t, g.int(0, 4));
    ix(t, g.int(0, 3));
    meta_tokens_one(t, g);
    let dst = g.chance(2);
    ix(t, i64::from(dst));
    if dst {
        ix(t, g.int(0, 4));
        ix(t, g.int(0, 3));
        meta_tokens_one(t, g);
    }
}

fn set_vg(t: &mut Toks, vg: &mut VolGeom) {
    vg.valid = t.i();
    vg.width = t.i();
    vg.height = t.i();
    vg.depth = t.i();
    (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
    vg.ras_good_flag = t.i();
    (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
    (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
    (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
    (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    (vg.s_r, vg.s_a, vg.s_s) = (t.f(), t.f(), t.f());
    // snprintf(fname, STRLEN, "%s", name): the name cut at 1,023 bytes, then its NUL
    vg.fname.strcpy_bytes(&t.h());
}

fn dump_vg(vg: &VolGeom, o: &mut Out) {
    for x in [vg.valid, vg.width, vg.height, vg.depth] {
        o.i(i64::from(x));
    }
    for x in [vg.xsize, vg.ysize, vg.zsize] {
        o.f(x);
    }
    o.i(i64::from(vg.ras_good_flag));
    for x in [
        vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r, vg.z_a, vg.z_s, vg.c_r, vg.c_a,
        vg.c_s, vg.s_r, vg.s_a, vg.s_s,
    ] {
        o.f(x);
    }
    o.h(Some(vg.fname.c_bytes()));
    o.i(i64::from(vg.i_to_r.is_some()));
    o.i(i64::from(vg.r_to_i.is_some()));
}

fn set_meta(cx: &mut Ctx, t: &mut Toks, m: &mut Mris) -> Result<(), Fatal> {
    m.r#type = t.i();
    m.status = MrisStatus(t.i());
    m.hemisphere = t.i();
    m.free_transform = t.i();
    set_vg(t, &mut m.vg);
    let nx = t.i();
    if nx > 0 {
        // LTAalloc(nx, NULL), then its type
        let mut l = lta::lta_alloc(&mut cx.g, &mut cx.a, &mut cx.b, nx, None)?;
        l.ltype = t.i();
        m.lta = Some(l);
    }
    for k in 0..2 {
        if t.i() == 0 {
            continue;
        }
        let mut mt = Matrix::real(4, 4);
        for r in 1..=4 {
            for c in 1..=4 {
                mt.set_relt(r, c, t.f());
            }
        }
        if k == 1 {
            m.TalSRASToSRAS_ = Some(mt);
        } else {
            m.SRASToTalSRAS_ = Some(mt);
        }
    }
    Ok(())
}

fn dump_meta(m: &Mris, o: &mut Out) {
    for x in [
        m.nvertices,
        m.nfaces,
        m.r#type,
        m.status.0,
        m.hemisphere,
        m.free_transform,
    ] {
        o.i(i64::from(x));
    }
    dump_vg(&m.vg, o);
    match &m.lta {
        Some(l) => {
            o.i(i64::from(l.num_xforms));
            o.i(i64::from(l.ltype));
        }
        None => o.s("NULL"),
    }
    for mt in [&m.SRASToTalSRAS_, &m.TalSRASToSRAS_] {
        match mt {
            None => o.s("NULL"),
            Some(mt) => {
                for r in 1..=4 {
                    for c in 1..=4 {
                        o.f(mt.relt(r, c));
                    }
                }
            }
        }
    }
}

pub fn meta_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let (nv, nf) = (t.i(), t.i());
    let mut src = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, nv, nf)?;
    set_meta(cx, t, &mut src)?;
    let dst = if t.i() != 0 {
        let (nv2, nf2) = (t.i(), t.i());
        let mut d = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, nv2, nf2)?;
        set_meta(cx, t, &mut d)?;
        Some(d)
    } else {
        None
    };
    let given = dst.is_some();
    let (g, a, b, sg) = (&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg);
    let r = alloc::copy_metadata(g, a, b, sg, &src, dst)?;
    o.i(i64::from(given));
    o.0 += "| ";
    dump_meta(&r, o);
    alloc::free(g, b, &mut Some(src));
    alloc::free(g, b, &mut Some(r));
    Ok(())
}

pub fn misc_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let nops = g.int(1, 6);
    ix(t, nops);
    let ops = [
        "cn", "pc", "ss", "cs", "cs", "cd", "cf", "vp", "vp", "ve", "sg", "vc", "fc",
    ];
    for _ in 0..nops {
        let op = g.pick(&ops);
        t.push(op.to_owned());
        match op {
            "cn" => {
                ix(
                    t,
                    if g.chance(5) {
                        g.int(3, 4)
                    } else {
                        g.int(0, 2)
                    },
                );
                let n = g.int(0, 20) as usize;
                let b = bytes(g, n);
                hx(t, &b);
            }
            "ss" => {
                if g.chance(4) {
                    ix(t, -1);
                } else {
                    let len = g.int(0, 5);
                    ix(t, g.int(0, len));
                    ix(t, len);
                    floats(t, g, len as usize);
                }
            }
            "cs" => {
                let n = g.int(0, 60) as usize;
                let l = bytes(g, n);
                hx(t, &l);
                let r = if g.chance(40) {
                    let len = g.pick(&[16376, 16377, 16378, 16379, 17000]);
                    (0..len).map(|_| g.int(0x21, 0x7e) as u8).collect()
                } else {
                    let n = g.int(0, 40) as usize;
                    bytes(g, n)
                };
                hx(t, &r);
            }
            "cd" | "cf" => {
                let n = g.int(0, 60) as usize;
                let l = bytes(g, n);
                hx(t, &l);
                if op == "cd" {
                    ix(t, g.pick(&[0, -1, 7, 123456, -2147483648, 2147483647]));
                } else {
                    fx(t, g);
                }
            }
            "vp" => {
                let nv = if g.chance(3) {
                    g.int(0, 19)
                } else {
                    g.int(20, 120)
                };
                ix(t, nv);
                let from = g.int(0, 2);
                let to = (from + g.int(0, 150)).min((nv - 1).max(from));
                ix(t, from);
                ix(t, to);
                ix(t, i64::from(!g.chance(3)));
                let n = g.int(0, 60) as usize;
                let b = printable(g, n as i64, n as i64);
                hx(t, &b);
            }
            "ve" => {
                let rows = g.int(0, 6);
                ix(t, rows);
                let term = f32bits(g);
                for _ in 0..rows {
                    if g.chance(2) {
                        t.push(format!("{term:08x}"));
                    } else {
                        fx(t, g);
                    }
                }
                t.push(format!("{term:08x}"));
            }
            "sg" => fx(t, g),
            "vc" => {
                let a: Vec<u32> = (0..3).map(|_| small(g)).collect();
                for k in 0..2 {
                    ix(t, g.int(0, 9));
                    for c in 0..3 {
                        let b = if k == 1 && g.chance(2) {
                            a[c]
                        } else {
                            small(g)
                        };
                        t.push(format!("{:08x}", if k == 0 { a[c] } else { b }));
                    }
                }
            }
            "fc" => {
                let a: Vec<i64> = (0..3).map(|_| g.int(0, 4)).collect();
                for k in 0..2 {
                    ix(t, g.int(0, 9));
                    for c in 0..3 {
                        ix(
                            t,
                            if k == 0 || g.chance(2) {
                                a[c]
                            } else {
                                g.int(0, 4)
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn misc_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        match op {
            "cn" => {
                let nth = t.i();
                let name = t.h();
                o.i(i64::from(basemisc::set_curvature_name(
                    &mut cx.sg, a, nth, &name,
                )));
            }
            "pc" => o.i(i64::from(basemisc::print_curvature_names(&cx.sg, a))),
            "ss" => {
                let n = t.i();
                let r = if n < 0 {
                    basemisc::set_registration_sigmas(&mut cx.sg, None, 0)
                } else {
                    let len = t.i();
                    let v: Vec<f32> = (0..len).map(|_| t.f()).collect();
                    basemisc::set_registration_sigmas(&mut cx.sg, Some(&v), n)
                };
                o.i(i64::from(r));
                o.d(cx.sg.nsigmas);
                for i in 0..cx.sg.nsigmas as usize {
                    o.f(cx.sg.sigmas[i]);
                }
            }
            "cs" => {
                let (l, r) = (t.h(), t.h());
                basemisc::cprints(g, b, &l, &r)?;
            }
            "cd" => {
                let l = t.h();
                basemisc::cprintd(b, &l, t.i());
            }
            "cf" => {
                let l = t.h();
                basemisc::cprintf(b, &l, t.f());
            }
            "vp" => {
                let (nv, from, to, hasmsg) = (t.i(), t.i(), t.i(), t.i());
                let msg = t.h();
                let m = alloc::alloc(g, a, b, &cx.sg, nv, 0)?;
                for v in from..=to {
                    let r = basemisc::vertex_progress_print(
                        g,
                        b,
                        &m,
                        v,
                        (hasmsg != 0).then_some(&msg[..]),
                    )?;
                    o.i(i64::from(r));
                }
            }
            "ve" => {
                let rows = t.i();
                let mut v = Matrix::real(rows, 1);
                for r in 1..=rows {
                    v.set_relt(r, 1, t.f());
                }
                let term = t.f();
                o.i(i64::from(basemisc::vector_element_index_find_not_equal(
                    &v, term,
                )));
                o.i(i64::from(basemisc::vector_element_index_find(&v, term)));
            }
            "sg" => o.i(i64::from(basemisc::signum_eval(t.f()))),
            "vc" => {
                let mut s = [VertexSort::default(); 2];
                for x in &mut s {
                    *x = VertexSort {
                        vtxno: t.i(),
                        x: t.f(),
                        y: t.f(),
                        z: t.f(),
                    };
                }
                o.i(i64::from(basemisc::compare_vertex_coords(&s[0], &s[1])));
                o.i(i64::from(basemisc::compare_vertex_coords(&s[1], &s[0])));
            }
            "fc" => {
                let mut s = [FaceSort::default(); 2];
                for x in &mut s {
                    *x = FaceSort {
                        faceno: t.i(),
                        v0: t.i(),
                        v1: t.i(),
                        v2: t.i(),
                    };
                }
                o.i(i64::from(basemisc::compare_face_vertices(&s[0], &s[1])));
                o.i(i64::from(basemisc::compare_face_vertices(&s[1], &s[0])));
            }
            op => panic!("misc op {op}"),
        }
    }
    Ok(())
}

/// A canonical coordinate for `edges`: on a small grid (exact zeros and collinear points, the
/// special cases), tiny (the stable version), or any float.
fn canon_bits(g: &mut Gen, mode: i64) -> u32 {
    match mode {
        0 => (g.int(-4, 4) as f32 / 4.0).to_bits(),
        1 => (g.int(-4, 4) as f32 * 1.0e-5).to_bits(),
        _ => f32bits(g),
    }
}

pub fn edges_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let nv = g.int(4, 6);
    ix(t, nv);
    let mode = g.int(0, 3);
    for _ in 0..3 * nv {
        let m = if g.chance(6) { g.int(0, 2) } else { mode };
        t.push(format!("{:08x}", canon_bits(g, m)));
    }
    let n = g.int(1, 6);
    ix(t, n);
    for _ in 0..4 * n {
        ix(t, g.int(0, nv - 1));
    }
}

pub fn edges_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    let nv = t.i();
    let mut m = alloc::alloc(g, a, b, &cx.sg, nv, 0)?;
    for vno in 0..nv as usize {
        let v = m.vertex_mut(vno);
        (v.cx, v.cy, v.cz) = (t.f(), t.f(), t.f());
    }
    let n = t.i();
    for _ in 0..n {
        let e1 = Edge {
            vno1: t.i(),
            vno2: t.i(),
            ..Edge::default()
        };
        let e2 = Edge {
            vno1: t.i(),
            vno2: t.i(),
            ..Edge::default()
        };
        o.i(i64::from(edges::edges_intersect(&m, &e1, &e2)));
    }
    alloc::free(g, b, &mut Some(m));
    Ok(())
}

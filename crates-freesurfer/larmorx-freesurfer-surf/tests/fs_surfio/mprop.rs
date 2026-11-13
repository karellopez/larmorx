// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue items 3.1-51 and 3.1-52 (relay 10): `mprop`, a surface with a
//! mesh's topology (each vertex's faces and corners, its 1- to 3-hop neighbours), the environment's
//! switches of `MRIScomputeMetricProperties`, then 1 to 4 of the metric-property operations, then
//! its state: the generator and the port's answers, as `fssurfio.cpp`'s.

use super::*;

use larmorx_freesurfer_surf::{mp, mp_slow};

fn floats(t: &mut Vec<String>, g: &mut Gen, n: usize, heavy: bool) {
    for _ in 0..n {
        t.push(format!("{:08x}", smallish(g, heavy)));
    }
}

/// A position for a mesh vertex: on a rough sphere of radius about 10 around a centre, in 1/16
/// steps, or a special float now and then (more with `heavy`).
fn position(g: &mut Gen, heavy: bool, centre: [i64; 3]) -> [u32; 3] {
    std::array::from_fn(|k| {
        if (heavy && g.chance(10)) || g.chance(60) {
            smallish(g, true)
        } else {
            ((centre[k] + g.int(-160, 160)) as f32 / 16.0).to_bits()
        }
    })
}

/// The 1-, 2- and 3-hop rings of each vertex through the faces (each ring sorted).
fn rings(nv: usize, faces: &[[usize; 3]]) -> Vec<[Vec<usize>; 3]> {
    let mut adj = vec![std::collections::BTreeSet::new(); nv];
    for f in faces {
        for a in 0..3 {
            for b in 0..3 {
                if f[a] != f[b] {
                    adj[f[a]].insert(f[b]);
                }
            }
        }
    }
    (0..nv)
        .map(|v| {
            let mut seen = std::collections::BTreeSet::from([v]);
            let mut out: [Vec<usize>; 3] = Default::default();
            let mut frontier = vec![v];
            for ring in &mut out {
                let mut next = std::collections::BTreeSet::new();
                for &u in &frontier {
                    for &w in &adj[u] {
                        if !seen.contains(&w) {
                            next.insert(w);
                        }
                    }
                }
                seen.extend(next.iter().copied());
                *ring = next.iter().copied().collect();
                frontier = ring.clone();
            }
            out
        })
        .collect()
}

pub(super) fn mprop_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    let mut envs = 0i64;
    if g.chance(6) {
        envs |= 1;
        if g.chance(2) {
            envs |= 2;
        }
    } else if g.chance(5) {
        envs |= 4;
        if g.chance(2) {
            envs |= 8;
        }
    } else if g.chance(12) {
        envs |= 2;
    }
    // the slow arctangent (atan2f) now and then, often for NaN-heavy cases: the fast one's table is
    // read outside for a NaN (FS-06), so only these compare the NaNs of the face angles
    if g.chance(if heavy { 3 } else { 15 }) {
        envs |= 16;
    }
    if g.chance(4) {
        envs |= 32;
    }
    let weights = envs & 8 != 0;
    // the operations first: a random move needs the seed (bit 64)
    let nops = g.int(1, 4);
    let ops = [
        "mp", "mp", "mp", "mp", "tp", "cn", "ad", "an", "ue", "os", "op", "sd", "ss", "dv", "rc",
        "sb", "sv", "sB",
    ];
    let mut opt: Vec<String> = Vec::new();
    for _ in 0..nops {
        let op = g.pick(&ops);
        opt.push(op.to_owned());
        match op {
            "ss" => {
                ix(&mut opt, g.int(0, 3));
                opt.push(format!(
                    "{:08x}",
                    g.pick(&[0.5f32, 0.25, 1.0, 1.5, -0.5]).to_bits()
                ));
            }
            "dv" => {
                envs |= 64;
                opt.push(format!("{:016x}", g.pick(&[0.001f64, 0.5, 2.0]).to_bits()));
            }
            "rc" => {
                ix(&mut opt, g.pick(&[2, 8, 9, 7, 2]));
                ix(&mut opt, if g.chance(8) { 2 } else { 3 });
            }
            "sv" => opt.push(format!(
                "{:016x}",
                g.pick(&[1.5f64, 0.5, -1.0, 1.0]).to_bits()
            )),
            "sB" => opt.push(format!(
                "{:08x}",
                g.pick(&[1.5f32, 0.5, 2.0, 1.0]).to_bits()
            )),
            _ => {}
        }
    }
    ix(t, envs);
    ix(t, i64::from(g.chance(6)));
    ix(t, g.pick(&[1, 1, 1, 0]));
    ix(t, g.pick(&[1, 1, 0]));
    let nv = if g.chance(20) { 0 } else { g.int(1, 10) } as usize;
    let mut faces: Vec<[usize; 3]> = Vec::new();
    if nv >= 3 {
        for _ in 0..g.int(0, 2 * nv as i64) {
            let a = g.int(0, nv as i64 - 1) as usize;
            let mut b = g.int(0, nv as i64 - 1) as usize;
            let mut c = g.int(0, nv as i64 - 1) as usize;
            if !g.chance(10) {
                while b == a {
                    b = g.int(0, nv as i64 - 1) as usize;
                }
                while c == a || c == b {
                    c = g.int(0, nv as i64 - 1) as usize;
                }
            }
            faces.push([a, b, c]);
        }
    }
    let nf = faces.len();
    ix(t, nv as i64);
    ix(t, nf as i64);
    let status = if g.chance(25) {
        g.pick(&[10, -1])
    } else {
        g.pick(&[0, 0, 1, 2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 9])
    };
    ix(t, status);
    ix(t, g.pick(&[0, 2, 4, 5, 1]));
    ix(
        t,
        if g.chance(30) {
            g.pick(&[0, 4])
        } else {
            g.int(1, 3)
        },
    );
    ix(t, g.int(0, 3));
    ix(t, g.int(0, 3));
    if heavy && g.chance(4) {
        dx(t, g);
    } else {
        t.push(format!("{:016x}", (g.int(1, 400) as f64 / 4.0).to_bits()));
    }
    t.push(format!("{:08x}", smallish(g, heavy)));
    dx(t, g);
    let centre = [g.int(-800, 800), g.int(-800, 800), g.int(-800, 800)];
    let rs = rings(nv, &faces);
    let mut pos: Vec<[u32; 3]> = Vec::new();
    for vno in 0..nv {
        // now and then at another vertex's place (a face with no area)
        let p = if vno > 0 && g.chance(8) {
            pos[g.int(0, vno as i64 - 1) as usize]
        } else {
            position(g, heavy, centre)
        };
        pos.push(p);
        for x in p {
            t.push(format!("{x:08x}"));
        }
        // the original coordinates near the current ones, or anything
        for k in 0..3 {
            if g.chance(3) {
                t.push(format!("{:08x}", smallish(g, heavy)));
            } else {
                t.push(format!("{:08x}", p[k]));
            }
        }
        floats(t, g, 17, heavy);
        // the original area: never set (negative) now and then
        if g.chance(3) {
            t.push(format!("{:08x}", (-1.0f32).to_bits()));
        } else {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, i64::from(g.chance(4)));
        ix(t, g.int(-1, nf as i64));
        ix(t, i64::from(g.chance(4)));
        ix(t, i64::from(g.chance(7)));
        let [r1, r2, r3] = &rs[vno];
        let counts = [
            r1.len(),
            r1.len() + r2.len(),
            r1.len() + r2.len() + r3.len(),
        ];
        let nsize_v = g.int(1, 3) as usize;
        let vtotal = counts[nsize_v - 1];
        let nsize_max = g.int(0, nsize_v as i64);
        for c in counts {
            ix(t, c as i64);
        }
        ix(t, vtotal as i64);
        ix(t, nsize_max);
        let list: Vec<usize> = r1.iter().chain(r2).chain(r3).copied().collect();
        ix(t, list.len() as i64);
        for v in &list {
            ix(t, *v as i64);
        }
        let mut fl: Vec<(usize, usize)> = Vec::new();
        for (fno, f) in faces.iter().enumerate() {
            if let Some(c) = f.iter().position(|&x| x == vno) {
                fl.push((fno, c));
            }
        }
        if nf > 0 && !weights && g.chance(15) {
            fl.push((g.int(0, nf as i64 - 1) as usize, g.int(0, 2) as usize));
        }
        ix(t, fl.len() as i64);
        for (f, _) in &fl {
            ix(t, *f as i64);
        }
        for (_, c) in &fl {
            ix(t, *c as i64);
        }
        let vsize = vtotal;
        for _ in 0..2 {
            if g.chance(5) {
                ix(t, -1);
                continue;
            }
            let blen = (vsize as i64 + g.int(-1, 3)).max(0);
            ix(t, blen);
            ix(t, if g.chance(3) { blen } else { g.int(0, blen) });
            ix(t, i64::from(!g.chance(4)));
            for _ in 0..blen {
                t.push(format!("{:08x}", small(g)));
            }
        }
    }
    for f in &faces {
        for v in f {
            ix(t, *v as i64);
        }
        ix(t, i64::from(g.chance(6)));
        floats(t, g, 7, heavy);
        floats(t, g, 4, heavy);
        ix(t, g.pick(&[0, 0, 0, 1, 2, 3]));
    }
    ix(t, nops);
    t.extend(opt);
}

fn dump_mprop(m: &Mris, o: &mut Out) {
    for vno in 0..m.nvertices.max(0) as usize {
        let v = m.vertex(vno);
        for x in [
            v.x, v.y, v.z, v.nx, v.ny, v.nz, v.area, v.origarea, v.cx, v.curv,
        ] {
            o.f(x);
        }
        o.i(i64::from(v.border));
        o.i(i64::from(v.neg));
        o.i(i64::from(v.fno));
        dump_dists(m, vno, o);
    }
    for fno in 0..m.nfaces.max(0) as usize {
        let f = m.face(fno);
        o.f(f.area);
        for x in f.angle.iter().chain(f.orig_angle.iter()) {
            o.f(*x);
        }
        let e = m.faceNormCacheEntries.as_ref().unwrap()[fno];
        for x in [e.nx, e.ny, e.nz, e.orig_area] {
            o.f(x);
        }
        o.i(i64::from(
            m.faceNormDeferredEntries.as_ref().unwrap()[fno].deferred,
        ));
    }
    for x in [
        m.xlo,
        m.ylo,
        m.zlo,
        m.xhi,
        m.yhi,
        m.zhi,
        m.xctr,
        m.yctr,
        m.zctr,
        m.total_area,
    ] {
        o.f(x);
    }
    o.d(m.avg_vertex_area);
    o.d(m.avg_vertex_dist);
    o.d(m.std_vertex_dist);
    o.f(m.neg_area);
    o.f(m.neg_orig_area);
    o.i(i64::from(m.dist_nsize));
    o.i(i64::from(m.dist_alloced_flags));
}

/// The probe's guard of `ad` and `an`: every unripped vertex with an unripped 1-hop neighbour
/// has distances for its 1-hop neighbours (C reads them unchecked).
fn avg_guard(m: &Mris) -> bool {
    for vno in 0..m.nvertices.max(0) as usize {
        let v = m.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vt = m.topology(vno);
        let vnum = vt.vnum.max(0) as usize;
        let any = (0..vnum).any(|k| m.vertex(vt.v.as_ref().unwrap()[k] as usize).ripflag == 0);
        if any && (v.dist.is_none() || (v.dist_capacity as usize) < vnum) {
            return false;
        }
    }
    true
}

pub(super) fn mprop_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let envs = t.i();
    let names = [
        "FREESURFER_OLD_MRIScomputeMetricProperties",
        "FREESURFER_NEW_MRIScomputeMetricProperties",
        "FS_FASTER_MP",
        "FS_WEIGHTS_NORMS_BY_ANGLE",
        "FREESURFER_fastApproxAtan2f_use_atanf2",
        "FREESURFER_reportLogicErrors",
    ];
    let seed = envs & 64 != 0;
    let pairs: Vec<(&str, &str)> = names
        .iter()
        .enumerate()
        .filter(|(k, _)| envs & (1 << k) != 0)
        .map(|(_, n)| (*n, "1"))
        .collect();
    let mut pairs = pairs;
    if seed {
        pairs.push(("FREESURFER_SEED", "1234"));
    }
    let env = Environment::from_pairs(pairs);
    if t.i() != 0 {
        cx.g.gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    }
    cx.sg.unitize_normal_face = t.i();
    aaa::set_fix_vertex_area_value(&mut cx.sg, t.i());
    let (nv, nf) = (t.i() as usize, t.i() as usize);
    let mut m = mris_alloc(nv, nf);
    m.faceNormCacheEntries = Some(vec![aaa::FaceNormCacheEntry::default(); nf]);
    m.faceNormDeferredEntries = Some(vec![aaa::FaceNormDeferredEntry::default(); nf]);
    m.status = MrisStatus(t.i());
    m.origxyz_status = MrisStatus(t.i());
    m.nsize = t.i() as i8;
    m.dist_alloced_flags = t.i() as i8;
    m.dist_nsize = t.i() as i8;
    m.radius = t.d();
    m.total_area = t.f();
    m.avg_vertex_dist = t.d();
    for vno in 0..nv {
        {
            let v = m.vertex_mut(vno);
            let mut f = || t.f();
            (v.x, v.y, v.z) = (f(), f(), f());
            (v.origx, v.origy, v.origz) = (f(), f(), f());
            (v.cx, v.cy, v.cz) = (f(), f(), f());
            (v.whitex, v.whitey, v.whitez) = (f(), f(), f());
            (v.pialx, v.pialy, v.pialz) = (f(), f(), f());
            (v.wnx, v.wny, v.wnz) = (f(), f(), f());
            (v.nx, v.ny, v.nz) = (f(), f(), f());
            (v.curv, v.area, v.origarea) = (f(), f(), f());
            v.border = t.i() as i8;
            v.fno = t.i();
            v.neg = t.i() as i8;
            v.ripflag = t.i() as i8;
        }
        let vt = m.topology_mut(vno);
        vt.vnum = t.i() as i16;
        vt.v2num = t.i() as i16;
        vt.v3num = t.i() as i16;
        vt.vtotal = t.i() as i16;
        vt.nsizeMax = t.i() as u8;
        let len = t.i();
        vt.v = Some((0..len).map(|_| t.i()).collect());
        let num = t.i();
        vt.num = num as u8;
        vt.f = Some((0..num).map(|_| t.i()).collect());
        vt.n = Some((0..num).map(|_| t.i() as u8).collect());
        for k in 0..2 {
            let len = t.i();
            if len < 0 {
                continue;
            }
            let (cap, valid) = (t.i(), t.i() != 0);
            let buf: Vec<f32> = (0..len).map(|_| t.f()).collect();
            let v = m.vertex_mut(vno);
            let (d, c) = if k == 1 {
                (&mut v.dist_orig, &mut v.dist_orig_capacity)
            } else {
                (&mut v.dist, &mut v.dist_capacity)
            };
            if valid {
                *d = Some(buf);
                *c = cap;
            } else {
                let st = if k == 1 {
                    &mut m.dist_orig_storage
                } else {
                    &mut m.dist_storage
                };
                st.as_mut().unwrap()[vno] = Some(buf);
            }
        }
    }
    for fno in 0..nf {
        {
            let f = m.face_mut(fno);
            for k in 0..3 {
                f.v[k] = t.i();
            }
            f.ripflag = t.i() as i8;
            f.area = t.f();
            for k in 0..3 {
                f.angle[k] = t.f();
            }
            for k in 0..3 {
                f.orig_angle[k] = t.f();
            }
        }
        let e = &mut m.faceNormCacheEntries.as_mut().unwrap()[fno];
        (e.nx, e.ny, e.nz, e.orig_area) = (t.f(), t.f(), t.f(), t.f());
        m.faceNormDeferredEntries.as_mut().unwrap()[fno].deferred = t.i() as i8;
    }
    let nops = t.i();
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    let mut io = coords::Io {
        out: &mut cx.a,
        err: &mut cx.b,
        env: &env,
    };
    for _ in 0..nops {
        let op = t.next();
        match op {
            "mp" => o.i(i64::from(mp::compute_metric_properties(
                g, sg, &mut io, &mut m,
            )?)),
            "tp" => o.i(i64::from(mp_slow::compute_triangle_properties(
                g, sg, &mut io, &mut m,
            ))),
            "cn" => o.i(i64::from(mp_slow::compute_normals(g, sg, &mut io, &mut m))),
            "ad" | "an" => {
                if !avg_guard(&m) {
                    o.s("G");
                    continue;
                }
                let sd = mp_slow::compute_avg_inter_vertex_dist(&mut m);
                if op == "ad" {
                    o.d(sd);
                }
            }
            "ue" => o.i(i64::from(mp_slow::update_ellipsoid_surface(&mut m))),
            "os" => o.i(i64::from(mp_slow::orient_surface(sg, &mut m))),
            "op" => o.i(i64::from(mp_slow::orient_plane(sg, &mut m))),
            "sd" => o.i(i64::from(coords::compute_surface_dimensions(&mut m))),
            "ss" => {
                let (n, st) = (t.i(), t.f());
                mp::smooth_surface(g, sg, &mut io, &mut m, n, st)?;
            }
            "dv" => {
                let a = t.d();
                mp::disturb_vertices(g, sg, &mut io, &mut m, a)?;
            }
            "rc" => {
                let (mv, tg) = (t.i(), t.i());
                mp::recenter(g, sg, &mut io, &mut m, mv, tg)?;
            }
            "sb" => o.i(i64::from(mp::scale_brain_area(g, sg, &mut io, &mut m)?)),
            "sv" => {
                let s = t.d();
                o.i(i64::from(mp::scale_vertex_coordinates(g, &mut m, s)));
            }
            "sB" => {
                let s = t.f();
                mp::scale_brain(&mut m, s);
            }
            op => panic!("mprop op {op}"),
        }
    }
    o.0 += "| ";
    dump_mprop(&m, o);
    Ok(())
}

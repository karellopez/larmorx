// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 19: `mpd`, the discrete curvatures (`discrete.rs`): a surface read
//! by `MRISread` (`d/lh.s0`, a binary triangle file: once- or twice-subdivided icosahedra on a
//! sphere, open grids, srd's small meshes), its vertices' fields (rip flags, marks, current and
//! original positions, normals, `K` and `H`, areas) and faces' (rip flags, areas, angles, cached
//! normals) overridden now and then, then 1 to 3 operations (the second fundamental form, the
//! faces' order around the vertices, the KH and k1k2 curvatures, the curvature sign and the angle
//! of two faces, a vertex's face angles and their extremes, the obsolete H sign, the pseudo-normal,
//! the face-place helpers, the marks), each printing what it gives: the generator and the port's
//! answers, as `fssurfio.cpp`'s.

use super::srd::{ICOSA_F, ICOSA_V, Mesh, be4, mesh};
use super::*;

use larmorx_freesurfer_surf::{discrete, facenorm, props, readwrite};

fn fh(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    t.push(format!("{:08x}", smallish(g, heavy)));
}

/// The icosahedron subdivided `levels` times, on a sphere of radius `r` (each new vertex the edge's
/// midpoint scaled to `r` with `float`'s square root, so the inputs are the same on every
/// platform), each coordinate moved by `k/64` now and then.
fn ico(g: &mut Gen, levels: u32, r: f32) -> Mesh {
    let mut v: Vec<[f32; 3]> = ICOSA_V.to_vec();
    let mut f: Vec<[i64; 3]> = ICOSA_F.to_vec();
    for _ in 0..levels {
        let mut mid = std::collections::HashMap::new();
        let mut nf = Vec::new();
        let mut midpoint = |a: i64, b: i64, v: &mut Vec<[f32; 3]>| -> i64 {
            let key = (a.min(b), a.max(b));
            *mid.entry(key).or_insert_with(|| {
                let (p, q) = (v[a as usize], v[b as usize]);
                v.push([
                    (p[0] + q[0]) * 0.5,
                    (p[1] + q[1]) * 0.5,
                    (p[2] + q[2]) * 0.5,
                ]);
                v.len() as i64 - 1
            })
        };
        for t in &f {
            let ab = midpoint(t[0], t[1], &mut v);
            let bc = midpoint(t[1], t[2], &mut v);
            let ca = midpoint(t[2], t[0], &mut v);
            nf.extend_from_slice(&[[t[0], ab, ca], [t[1], bc, ab], [t[2], ca, bc], [ab, bc, ca]]);
        }
        f = nf;
    }
    for p in &mut v {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        for c in p.iter_mut() {
            *c = *c / len * r;
            if g.chance(12) {
                *c += g.int(-8, 8) as f32 / 64.0;
            }
        }
    }
    Mesh { v, f }
}

/// An open `w` by `h` grid of squares split into triangles (alternately), its heights `k/16`.
fn grid(g: &mut Gen, w: i64, h: i64) -> Mesh {
    let mut v = Vec::new();
    for j in 0..h {
        for i in 0..w {
            v.push([i as f32, j as f32, g.int(-8, 8) as f32 / 16.0]);
        }
    }
    let mut f = Vec::new();
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let (a, b, c, d) = (
                j * w + i,
                j * w + i + 1,
                (j + 1) * w + i,
                (j + 1) * w + i + 1,
            );
            if (i + j) % 2 == 0 {
                f.push([a, b, d]);
                f.push([a, d, c]);
            } else {
                f.push([a, b, c]);
                f.push([b, d, c]);
            }
        }
    }
    Mesh { v, f }
}

/// Two faces of the mesh for a two-face operation: mostly two sharing an edge, now and then any two.
fn face_pair(g: &mut Gen, m: &Mesh) -> (i64, i64) {
    let nf = m.f.len() as i64;
    let a = g.int(0, nf - 1);
    if !g.chance(5) {
        let fa = m.f[a as usize];
        let shares: Vec<i64> = (0..nf)
            .filter(|&b| b != a && m.f[b as usize].iter().filter(|x| fa.contains(x)).count() == 2)
            .collect();
        if !shares.is_empty() {
            return (a, shares[g.below(shares.len() as u64) as usize]);
        }
    }
    (a, g.int(0, nf - 1))
}

pub(super) fn mpd_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    ix(t, g.int(0, 3));
    ix(t, i64::from(g.chance(6)));
    let r = g.pick(&[1.0, 10.0, 100.0, 50.0]);
    let m = match g.below(12) {
        0 => {
            let (w, h) = (g.int(3, 6), g.int(3, 6));
            grid(g, w, h)
        }
        1 => mesh(g, heavy),
        2 => ico(g, 2, r),
        _ => ico(g, 1, r),
    };
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
    let few = g.chance(2);
    for _ in 0..nv {
        let mut mask = 0;
        for (bit, k) in [
            (1, 40),
            (2, 50),
            (4, 25),
            (8, 12),
            (16, 10),
            (32, 8),
            (64, 12),
        ] {
            if g.chance(if few { k * 4 } else { k }) {
                mask |= bit;
            }
        }
        ix(t, mask);
        if mask & 2 != 0 {
            ix(t, g.int(-1, 2));
        }
        for bit in [4, 8, 16] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        if mask & 32 != 0 {
            fh(t, g, heavy);
            fh(t, g, heavy);
        }
        if mask & 64 != 0 {
            fh(t, g, heavy);
        }
    }
    for _ in 0..nf {
        let mut mask = 0;
        for (bit, k) in [(1, 40), (2, 12), (4, 12), (8, 15)] {
            if g.chance(if few { k * 4 } else { k }) {
                mask |= bit;
            }
        }
        ix(t, mask);
        if mask & 2 != 0 {
            fh(t, g, heavy);
        }
        if mask & 4 != 0 {
            for _ in 0..3 {
                fh(t, g, heavy);
            }
        }
        if mask & 8 != 0 {
            for _ in 0..3 {
                if g.chance(3) {
                    fh(t, g, heavy);
                } else {
                    t.push(format!("{:08x}", (g.int(-16, 16) as f32 / 16.0).to_bits()));
                }
            }
        }
    }
    let nops = g.int(1, 3);
    ix(t, nops);
    for _ in 0..nops {
        let ops: &[&str] = if nf > 0 {
            &[
                "sff", "sff", "sff", "sff", "fvr", "khc", "k12", "hcs", "anf", "vfa", "vmm", "mhs",
                "vpn", "vpn", "fvi", "fvm", "dil", "clm", "sff", "anf", "vpn",
            ]
        } else {
            &["fvr", "k12", "dil", "clm", "sff"]
        };
        let op = g.pick(ops);
        t.push(op.to_owned());
        match op {
            "sff" | "k12" => ix(t, i64::from(g.chance(2))),
            "hcs" | "anf" | "fvm" => {
                let (a, b) = face_pair(g, &m);
                ix(t, a);
                ix(t, b);
            }
            "vfa" | "vmm" => ix(t, g.int(0, nv - 1)),
            "vpn" => {
                ix(t, g.int(0, nv - 1));
                ix(t, if g.chance(3) { g.int(0, 4) } else { 0 });
            }
            "fvi" => {
                ix(t, g.int(0, nf - 1));
                ix(t, g.int(-1, nv));
            }
            "dil" => ix(t, g.int(0, 2)),
            _ => {}
        }
    }
}

fn dump_kh(o: &mut Out, s: &Mris) {
    for x in [s.Kmin, s.Kmax, s.Hmin, s.Hmax] {
        o.f(x);
    }
    o.d(s.Ktotal);
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.K, v.H, v.k1, v.k2] {
            o.f(x);
        }
        o.i(i64::from(v.marked));
    }
}

fn dump_order(o: &mut Out, s: &Mris) {
    for vno in 0..s.nvertices.max(0) as usize {
        o.i(i64::from(s.vertex(vno).marked));
        let vt = s.topology(vno);
        let num = usize::from(vt.num);
        let (f, n) = (
            vt.f.as_deref().unwrap_or(&[]),
            vt.n.as_deref().unwrap_or(&[]),
        );
        for k in 0..num {
            o.i(i64::from(f[k]));
            o.i(i64::from(n[k]));
        }
    }
}

pub(super) fn mpd_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    let mut pairs: Vec<(String, String)> = vec![("SURFER_SIDEDOOR".into(), "1".into())];
    if t.i() & 1 != 0 {
        pairs.push(("BEVIN_TESTING_dumpFacesAroundVertex".into(), "1".into()));
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
    for vno in 0..nv as usize {
        let m = t.i();
        let v = s.vertex_mut(vno);
        if m & 1 != 0 {
            v.ripflag = 1;
        }
        if m & 2 != 0 {
            v.marked = t.i() as i16;
        }
        if m & 4 != 0 {
            [v.x, v.y, v.z] = [t.f(), t.f(), t.f()];
        }
        if m & 8 != 0 {
            [v.nx, v.ny, v.nz] = [t.f(), t.f(), t.f()];
        }
        if m & 16 != 0 {
            [v.origx, v.origy, v.origz] = [t.f(), t.f(), t.f()];
        }
        if m & 32 != 0 {
            v.K = t.f();
            v.H = t.f();
        }
        if m & 64 != 0 {
            v.area = t.f();
        }
    }
    for fno in 0..nf as usize {
        let m = t.i();
        if m & 1 != 0 {
            s.face_mut(fno).ripflag = 1;
        }
        if m & 2 != 0 {
            s.face_mut(fno).area = t.f();
        }
        if m & 4 != 0 {
            s.face_mut(fno).angle = [t.f(), t.f(), t.f()];
        }
        if m & 8 != 0 {
            let (a, b, c) = (t.f(), t.f(), t.f());
            facenorm::set_face_norm(&mut s, fno, a, b, c);
        }
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "sff" => {
                let signed = t.i() != 0;
                let r = discrete::compute_second_fundamental_form_discrete(
                    g, sg, &mut io, &mut s, signed,
                )?;
                o.i(i64::from(r));
                dump_kh(o, &s);
                dump_order(o, &s);
            }
            "fvr" => {
                o.i(i64::from(discrete::faces_at_vertices_reorder(
                    g, sg, &mut io, &mut s,
                )?));
                dump_order(o, &s);
            }
            "khc" => {
                o.i(i64::from(discrete::mris_discrete_kh_compute(
                    g, sg, &mut io, &mut s,
                )?));
                dump_kh(o, &s);
            }
            "k12" => {
                let signed = t.i() != 0;
                o.i(i64::from(discrete::mris_discrete_k1k2_compute(
                    g, &mut io, &mut s, signed,
                )?));
                dump_kh(o, &s);
            }
            "hcs" => {
                let (a, b) = (t.i() as usize, t.i() as usize);
                o.i(i64::from(discrete::faces_hcurvature_determine_sign(
                    g, sg, &mut io, &mut s, a, b,
                )?));
            }
            "anf" => {
                let (a, b) = (t.i() as usize, t.i() as usize);
                o.f(discrete::faces_angle_normal_find(
                    g, sg, &mut io, &mut s, a, b,
                )?);
            }
            "vfa" => {
                let vno = t.i() as usize;
                let a = discrete::vertex_face_angles_determine(g, &mut s, vno);
                o.i(a.len() as i64);
                for x in a {
                    o.f(x);
                }
            }
            "vmm" => {
                let vno = t.i() as usize;
                let (mut mi, mut mn, mut ma, mut mx) = (-7, 7.0f32, -7, 7.0f32);
                let r = discrete::vertex_face_min_max_angles_determine(
                    g, &mut io, &mut s, vno, &mut mi, &mut mn, &mut ma, &mut mx,
                )?;
                o.i(i64::from(r));
                o.i(i64::from(mi));
                o.f(mn);
                o.i(i64::from(ma));
                o.f(mx);
            }
            "mhs" => {
                o.i(i64::from(discrete::mris_hcurvature_determine_sign(
                    g, sg, &mut io, &mut s,
                )?));
                for vno in 0..s.nvertices.max(0) as usize {
                    o.i(i64::from(s.vertex(vno).undefval));
                }
            }
            "vpn" => {
                let (vno, verbose) = (t.i() as usize, t.i());
                let n = discrete::compute_vertex_pseudo_normal(g, &mut io, &mut s, vno, verbose)?;
                for x in n {
                    o.f(x);
                }
            }
            "fvi" => {
                let (fno, vno) = (t.i() as usize, t.i());
                o.i(i64::from(discrete::face_vertex_index_find(
                    s.face(fno),
                    vno,
                )));
            }
            "fvm" => {
                let (a, b) = (t.i() as usize, t.i() as usize);
                let (k, mask) = props::vertices_common_in_faces(s.face(a), s.face(b));
                o.i(i64::from(k));
                o.i(i64::from(discrete::face_vertex_index_at_mask_find(
                    s.face(a),
                    &mask,
                )));
            }
            "dil" => {
                let n = t.i();
                o.i(i64::from(discrete::dilate_marked(&mut s, n)));
                for vno in 0..s.nvertices.max(0) as usize {
                    o.i(i64::from(s.vertex(vno).marked));
                    o.f(s.vertex(vno).tx);
                }
            }
            "clm" => {
                o.i(i64::from(discrete::clear_marks(&mut s)));
                o.i(i64::from(discrete::count_all_marked(&s)));
            }
            _ => panic!("mpd op {op}"),
        }
    }
    Ok(())
}

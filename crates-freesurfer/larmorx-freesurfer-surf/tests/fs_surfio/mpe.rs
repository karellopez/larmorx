// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 20: `mpe`, the rest of queue item 3.1-51 (`props5.rs` onwards): a
//! surface read by `MRISread` (`d/lh.s0`, a binary triangle file: subdivided icosahedra on a
//! sphere, open grids, srd's small meshes), its status and curvature extremes, its vertices'
//! fields (rip flags, marks, current and original positions, normals, the curvature, `k1`, `k2`,
//! `K`, `H`, areas) and faces' (rip flags, areas, cached original areas and normals) overridden
//! now and then, then 1 to 4 operations (the smoothed normals, the mean curvature, the curvature
//! choices, the curvature indices, the largest radius, the curvature's mean and spread, the
//! vertices of negative faces), each printing what it gives: the generator and the port's
//! answers, as `fssurfio.cpp`'s.

use super::mpd::{grid, ico};
use super::srd::{be4, mesh};
use super::*;

use larmorx_freesurfer_core::diag::DIAG_WRITE;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_surf::{facenorm, props, props5, props6, props7, readwrite};

fn fh(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    t.push(format!("{:08x}", smallish(g, heavy)));
}

/// A small float `k/16` now and then, else [`fh`].
fn fs(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    if g.chance(3) {
        fh(t, g, heavy);
    } else {
        t.push(format!("{:08x}", (g.int(-40, 40) as f32 / 16.0).to_bits()));
    }
}

pub(super) fn mpe_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    ix(
        t,
        if g.chance(6) {
            g.int(4, 7)
        } else {
            g.int(0, 3)
        },
    );
    let r = g.pick(&[1.0, 10.0, 100.0, 50.0]);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 6), g.int(2, 6));
            grid(g, w, h)
        }
        2 | 3 => mesh(g, heavy),
        4 => ico(g, 2, r),
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
    ix(
        t,
        if g.chance(4) {
            g.pick(&[2, 4, 0, 5, 1, 3])
        } else {
            -1
        },
    );
    if g.chance(3) {
        ix(t, 1);
        for _ in 0..4 {
            fs(t, g, heavy);
        }
    } else {
        ix(t, 0);
    }
    // Gdiag_no (-1, or a vertex), FREESURFER_MRISaverageGradients_optimize_suppress or not
    ix(
        t,
        if g.chance(5) && nv > 0 {
            g.int(0, nv - 1)
        } else {
            -1
        },
    );
    ix(t, i64::from(g.chance(3)));
    let few = g.chance(2);
    for _ in 0..nv {
        let mut mask = 0;
        for (bit, k) in [
            (1, 30),
            (2, 20),
            (4, 25),
            (8, 12),
            (16, 4),
            (32, 4),
            (64, 4),
            (128, 6),
            (256, 20),
            (512, 3),
        ] {
            if g.chance(if few { k * 4 } else { k }) {
                mask |= bit;
            }
        }
        ix(t, mask);
        if mask & 2 != 0 {
            ix(t, g.int(-1, 2));
        }
        for bit in [4, 8] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        if mask & 16 != 0 {
            fs(t, g, heavy);
        }
        for bit in [32, 64] {
            if mask & bit != 0 {
                fs(t, g, heavy);
                fs(t, g, heavy);
            }
        }
        if mask & 128 != 0 {
            fs(t, g, heavy);
        }
        if mask & 256 != 0 {
            for _ in 0..3 {
                fh(t, g, heavy);
            }
        }
        if mask & 512 != 0 {
            for _ in 0..3 {
                fs(t, g, heavy);
            }
        }
    }
    for _ in 0..nf {
        let mut mask = 0;
        for (bit, k) in [(1, 30), (2, 6), (4, 8), (8, 15)] {
            if g.chance(if few { k * 4 } else { k }) {
                mask |= bit;
            }
        }
        ix(t, mask);
        if mask & 2 != 0 {
            fs(t, g, heavy);
        }
        if mask & 4 != 0 {
            fs(t, g, heavy);
        }
        if mask & 8 != 0 {
            for _ in 0..3 {
                fs(t, g, heavy);
            }
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        let op = g.pick(&[
            "ssn", "ssn", "cbn", "cmc", "cmc", "cmc", "uae", "uae", "ugc", "umc", "upc", "uk1",
            "uk1", "uk2", "upf", "upf", "cci", "cci", "mxr", "mxr", "cac", "cac", "mnv", "rnv",
            "sff", "sff", "sff", "sff", "sff", "sff", "sfa", "sfa", "sfa", "agr", "agr", "agr",
            "agf", "ags", "ags",
        ]);
        t.push(op.to_owned());
        match op {
            "ssn" => ix(t, g.int(0, 3)),
            "upf" => ix(t, g.int(0, 2)),
            "cac" => ix(t, i64::from(g.chance(4))),
            "mnv" => ix(t, g.int(-1, 3)),
            "sff" => {
                let pct = if g.chance(3) {
                    -1.0
                } else {
                    g.pick(&[0.999, 0.99, 0.9, 0.5, 0.0, 1.0, 0.75, f64::NAN])
                };
                t.push(format!("{:016x}", f64::to_bits(pct)));
            }
            "agr" | "agf" | "ags" => ix(
                t,
                if g.chance(8) {
                    g.int(151, 156)
                } else {
                    g.int(-1, 6)
                },
            ),
            "sfa" => {
                // the tangent planes first or not, then 1 to 3 calls: a vertex and its neighbours
                // (mostly the mesh's own, as a fan of its faces; now and then none or any)
                ix(t, i64::from(g.chance(4)));
                let calls = g.int(1, 3);
                ix(t, calls);
                for _ in 0..calls {
                    let vno = g.int(0, nv - 1);
                    ix(t, vno);
                    let mut nb: Vec<i64> = Vec::new();
                    for f in &m.f {
                        if f.contains(&vno) {
                            for &x in f {
                                if x != vno && !nb.contains(&x) {
                                    nb.push(x);
                                }
                            }
                        }
                    }
                    if g.chance(8) {
                        nb.clear();
                    } else if g.chance(8) {
                        let k = g.int(1, 8);
                        nb = (0..k).map(|_| g.int(0, nv - 1)).collect();
                    }
                    ix(t, nb.len() as i64);
                    for x in nb {
                        ix(t, x);
                    }
                }
            }
            _ => {}
        }
    }
}

fn dump_curv(o: &mut Out, s: &Mris) {
    o.f(s.min_curv);
    o.f(s.max_curv);
    for vno in 0..s.nvertices.max(0) as usize {
        o.f(s.vertex(vno).curv);
    }
}

/// A file's length and FNV-1a hash (`NOFILE` when there is none), the file then removed.
fn file_hash(o: &mut Out, path: &std::path::Path) {
    match std::fs::read(path) {
        Ok(b) => {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for c in &b {
                h ^= u64::from(*c);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            o.s(&format!("{}:{h:016x}", b.len()));
            let _ = std::fs::remove_file(path);
        }
        Err(_) => o.s("NOFILE"),
    }
}

/// The probe's functions of `k1` and `k2` for `MRISusePrincipalCurvatureFunction`.
fn upf(k: i32) -> fn(f32, f32) -> f32 {
    match k {
        0 => |k1, _| sse::add(k1, 1.0),
        1 => |_, k2| sse::mul(k2, 0.5),
        _ => |k1, k2| if k1 > k2 { k1 } else { k2 },
    }
}

pub(super) fn mpe_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    let bytes = t.h();
    let (nv, nf, status) = (t.i(), t.i(), t.i());
    let extremes = (t.i() != 0).then(|| [t.f(), t.f(), t.f(), t.f()]);
    let (gdiag_no, suppress) = (t.i(), t.i() != 0);
    let mut pairs = vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())];
    if suppress {
        pairs.push((
            "FREESURFER_MRISaverageGradients_optimize_suppress".to_owned(),
            "1".to_owned(),
        ));
    }
    let env = Environment::from_pairs(pairs);
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
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
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    if status >= 0 {
        s.status = MrisStatus(status);
    }
    if let Some([a, b, c, e]) = extremes {
        (s.Kmin, s.Kmax, s.Hmin, s.Hmax) = (a, b, c, e);
    }
    g.gdiag_no = gdiag_no;
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
            v.curv = t.f();
        }
        if m & 32 != 0 {
            v.k1 = t.f();
            v.k2 = t.f();
        }
        if m & 64 != 0 {
            v.K = t.f();
            v.H = t.f();
        }
        if m & 128 != 0 {
            v.area = t.f();
        }
        if m & 256 != 0 {
            [v.origx, v.origy, v.origz] = [t.f(), t.f(), t.f()];
        }
        if m & 512 != 0 {
            [v.dx, v.dy, v.dz] = [t.f(), t.f(), t.f()];
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
            let a = t.f();
            facenorm::set_face_orig_area(&mut s, fno, a);
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
            "ssn" => {
                o.i(i64::from(props5::smooth_surface_normals(&mut s, t.i())));
                for vno in 0..s.nvertices.max(0) as usize {
                    let v = s.vertex(vno);
                    for x in [v.nx, v.ny, v.nz, v.tdx, v.tdy, v.tdz] {
                        o.f(x);
                    }
                }
            }
            "cbn" => o.i(i64::from(props5::compute_boundary_normals(&mut s))),
            "cmc" => {
                o.i(i64::from(props5::compute_mean_curvature(
                    g, &mut io, &mut s,
                )));
                o.f(s.min_curv);
                o.f(s.max_curv);
                for vno in 0..s.nvertices.max(0) as usize {
                    let v = s.vertex(vno);
                    for x in [v.H, v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z] {
                        o.f(x);
                    }
                }
            }
            "uae" | "ugc" | "umc" | "upc" | "uk1" | "uk2" | "upf" => {
                let r = match op {
                    "uae" => props5::use_area_errors(g, &mut s),
                    "ugc" => props5::use_gaussian_curvature(&mut s),
                    "umc" => props5::use_mean_curvature(&mut s),
                    "upc" => props5::use_principal_curvature(&mut s),
                    "uk1" => props5::use_k1_curvature(g, &mut s),
                    "uk2" => props5::use_k2_curvature(g, &mut s),
                    _ => {
                        let f = upf(t.i());
                        props5::use_principal_curvature_function(g, &mut s, &f)
                    }
                };
                o.i(i64::from(r));
                dump_curv(o, &s);
            }
            "cci" => {
                let (ici, fi) = props5::compute_curvature_indices(&s);
                o.d(ici);
                o.d(fi);
            }
            "mxr" => {
                o.d(props5::max_radius(&mut s));
                for x in [s.xlo, s.ylo, s.zlo, s.xhi, s.yhi, s.zhi] {
                    o.f(x);
                }
            }
            "cac" => {
                if t.i() != 0 {
                    let mut sigma = 7.0;
                    o.d(props5::compute_average_curvature(g, &s, Some(&mut sigma)));
                    o.d(sigma);
                } else {
                    o.d(props5::compute_average_curvature(g, &s, None));
                }
                o.i(i64::from(g.errno));
            }
            "mnv" => {
                o.i(i64::from(props5::mark_negative_vertices(&mut s, t.i())));
                for vno in 0..s.nvertices.max(0) as usize {
                    o.i(i64::from(s.vertex(vno).marked));
                }
            }
            "rnv" => {
                o.i(i64::from(props5::rip_negative_vertices(&mut s)));
                for vno in 0..s.nvertices.max(0) as usize {
                    o.i(i64::from(s.vertex(vno).ripflag));
                }
                for fno in 0..s.nfaces.max(0) as usize {
                    o.i(i64::from(s.face(fno).ripflag));
                }
            }
            "sff" => {
                let pct = t.d();
                o.i(i64::from(
                    props6::compute_second_fundamental_form_thresholded(
                        g, sg, &mut io, d, &mut s, pct,
                    )?,
                ));
                for x in [s.Kmin, s.Kmax, s.Hmin, s.Hmax] {
                    o.f(x);
                }
                o.d(s.Ktotal);
                for vno in 0..s.nvertices.max(0) as usize {
                    let v = s.vertex(vno);
                    for x in [
                        v.k1, v.k2, v.K, v.H, v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z,
                    ] {
                        o.f(x);
                    }
                }
                for name in ["curv.dat", "k1.plt", "k2.plt"] {
                    file_hash(o, &dir.join(name));
                }
            }
            "agr" | "agf" | "ags" => {
                let n = t.i();
                o.i(i64::from(match op {
                    "agr" => props7::average_gradients(g, sg, &mut io, &mut s, n),
                    "agf" => props7::average_gradients_fast(g, &mut io, &mut s, n),
                    _ => props7::average_signed_gradients(g, &mut io, &mut s, n),
                }));
                for vno in 0..s.nvertices.max(0) as usize {
                    let v = s.vertex(vno);
                    for x in [v.dx, v.dy, v.dz, v.tdx, v.tdy, v.tdz] {
                        o.f(x);
                    }
                }
            }
            "sfa" => {
                if t.i() != 0 {
                    o.i(i64::from(props::compute_tangent_planes(g, &mut io, &mut s)));
                }
                for _ in 0..t.i() {
                    let vno = t.i() as usize;
                    let nb: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
                    o.i(i64::from(
                        props6::compute_second_fundamental_form_at_vertex(
                            g, sg, &mut io, &mut s, vno, &nb,
                        )?,
                    ));
                    for x in [s.Kmin, s.Kmax, s.Hmin, s.Hmax] {
                        o.f(x);
                    }
                    o.d(s.Ktotal);
                    let v = s.vertex(vno);
                    for x in [
                        v.k1, v.k2, v.K, v.H, v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z,
                    ] {
                        o.f(x);
                    }
                }
            }
            _ => panic!("mpe op {op}"),
        }
    }
    Ok(())
}

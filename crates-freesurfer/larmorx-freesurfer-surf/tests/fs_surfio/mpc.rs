// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 18: `mpc`, the metric properties of queue item 3.1-50 part b's
//! second group (`props2.rs`): a surface read by `MRISread` (`d/lh.s0`, a binary triangle file of
//! srd's meshes), its status, original area and vertices' fields (rip flags, original, canonical,
//! pial and white positions, values, principal curvatures, normals, original and current
//! distances) overridden now and then, then 1 to 6 operations (the barycentric coordinates and
//! the projection onto a plane, the samples on a face, the tangent planes' RMS height, the analytic
//! distances, the zero-crossing distance, the curvature ratio and contrast, the brute-force
//! distance to a triangle), each printing what it gives: the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::srd::{be4, mesh};
use super::*;

use larmorx_freesurfer_core::diag::DIAG_WRITE;
use larmorx_freesurfer_surf::{dist, props2, props3, props4, readwrite};

fn fh(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    t.push(format!("{:08x}", smallish(g, heavy)));
}

/// A double: mostly a small one (k/8), now and then a float's value or any bits; with `heavy` a
/// NaN or an infinity one time in four.
fn dh(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    let b = if heavy && g.chance(4) {
        g.pick(&[
            0x7ff8_0000_0000_0000,
            0xfff8_0000_0000_0000,
            0x7ff4_0000_0000_0001,
            0xfff0_0000_0000_0123,
            0x7ff8_0000_0000_0042,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
        ])
    } else if g.chance(6) {
        f64::from(f32::from_bits(smallish(g, heavy))).to_bits()
    } else if g.chance(15) {
        f64bits(g)
    } else {
        (g.int(-200, 200) as f64 / 8.0).to_bits()
    };
    t.push(format!("{b:016x}"));
}

/// A coordinate set: mostly a supported one, now and then the target or layer-IV positions (which
/// `MRISvertexCoord2XYZ_float` refuses) or a number outside the list.
fn which(g: &mut Gen) -> i64 {
    if g.chance(40) {
        g.pick(&[10, 11, -1, 15])
    } else {
        g.pick(&[0, 3, 4, 7, 9, 4, 3, 0, 12, 2, 5, 6, 8, 13, 14])
    }
}

pub(super) fn mpc_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 7));
    ix(t, g.int(1, 1_000_000));
    if g.chance(3) {
        ix(t, 1);
        t.push(
            g.pick(&["3.5", "-2", "1e1", "abc", "0x1p3", "nan", "12.75x", "inf"])
                .to_owned(),
        );
    } else {
        ix(t, 0);
    }
    let m = mesh(g, heavy);
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
    // the status (MRIS_PLANE, MRIS_SPHERE, ...) or the read's own
    ix(
        t,
        if g.chance(3) {
            g.pick(&[2, 4, 0, 5, 1])
        } else {
            -1
        },
    );
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
    // each vertex: a mask, its values; its vtotal (the generator's own count of the mesh's
    // neighbours is not known here, so the original and current distances are written for the
    // first `k` entries the case gives, `k` at most the vertex's vtotal, which the run checks)
    for _ in 0..nv {
        let mut mask = 0;
        for (bit, k) in [
            (1, 8),
            (2, 3),
            (4, 3),
            (8, 3),
            (16, 3),
            (32, 3),
            (64, 3),
            (128, 3),
            (256, 5),
            (512, 5),
            (1024, 2),
            (2048, 4),
            (4096, 4),
            (8192, 3),
            (16384, 12),
            (32768, 4),
            (65536, 4),
            (131072, 3),
            (262144, 2),
        ] {
            if g.chance(k) {
                mask |= bit;
            }
        }
        ix(t, mask);
        for bit in [2, 4] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        for bit in [8, 16, 32, 64] {
            if mask & bit != 0 {
                fh(t, g, heavy);
            }
        }
        for bit in [128, 256, 512] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        for bit in [4096, 8192] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        if mask & 16384 != 0 {
            ix(t, g.int(1, 3));
        }
        for bit in [32768, 65536] {
            if mask & bit != 0 {
                for _ in 0..3 {
                    fh(t, g, heavy);
                }
            }
        }
        if mask & 131072 != 0 {
            fh(t, g, heavy);
        }
        for bit in [1024, 2048] {
            if mask & bit != 0 {
                let k = g.int(0, 8);
                ix(t, k);
                for _ in 0..k {
                    if g.chance(3) {
                        fh(t, g, heavy);
                    } else {
                        t.push(format!("{:08x}", (g.int(1, 400) as f32 / 16.0).to_bits()));
                    }
                }
            }
        }
    }
    let faces: Vec<[i64; 3]> = m.f.clone();
    for _ in 0..nf {
        let mask = i64::from(g.chance(8)) | (2 * i64::from(g.chance(4)));
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
                "fb", "fr", "pp", "sc", "sf", "sn", "rh", "ae", "sa", "dd", "hd", "ur", "uc", "md",
                "sv", "sfc", "fb", "sc", "sf", "ae", "fr", "cn", "cs", "su", "co", "cp", "ct",
                "ct", "su", "cn", "tt", "tv", "ke", "uk", "ca", "ca", "uk",
            ]
        } else if nv > 0 {
            &[
                "fr", "pp", "rh", "ae", "sa", "dd", "hd", "ur", "uc", "md", "sv", "cn", "cs", "su",
                "co", "cp", "ct", "tt", "tv", "ke", "uk",
            ]
        } else {
            &[
                "fr", "pp", "rh", "ae", "sa", "dd", "hd", "ur", "uc", "md", "tt", "tv", "ke", "uk",
            ]
        };
        let op = g.pick(ops);
        t.push(op.to_owned());
        let point = |t: &mut Vec<String>, g: &mut Gen| {
            for _ in 0..3 {
                dh(t, g, heavy);
            }
        };
        match op {
            "fb" => {
                ix(t, g.int(0, nf - 1));
                ix(t, which(g));
                point(t, g);
            }
            "fr" | "pp" => {
                let degenerate = g.below(6);
                // three corners (now and then two equal, or in a line), then the point
                let mut c: Vec<[String; 3]> = Vec::new();
                for _ in 0..3 {
                    let mut v = Vec::new();
                    point(&mut v, g);
                    c.push([v[0].clone(), v[1].clone(), v[2].clone()]);
                }
                if degenerate == 0 {
                    c[1] = c[0].clone();
                } else if degenerate == 1 {
                    c[2] = c[0].clone();
                } else if degenerate == 2 {
                    // a thin triangle: the third corner a tiny step from the first
                    let x = f64::from_bits(u64::from_str_radix(&c[0][0], 16).unwrap());
                    let tiny = g.pick(&[1e-17, 3e-16, 1e-20, 5e-15]);
                    c[2] = c[0].clone();
                    c[2][0] = format!("{:016x}", (x + tiny).to_bits());
                }
                for v in &c {
                    t.extend(v.iter().cloned());
                }
                point(t, g);
            }
            "sc" => {
                ix(t, g.int(0, nf - 1));
                point(t, g);
                ix(t, which(g));
                ix(t, which(g));
            }
            "sf" => {
                ix(t, g.int(0, nf - 1));
                ix(t, which(g));
                point(t, g);
                point(t, g);
            }
            "sn" => {
                ix(t, g.int(0, nf - 1));
                point(t, g);
            }
            "ae" => {
                ix(t, g.pick(&[2, 4, 5, 0, 4, 2]));
                ix(t, i64::from(g.chance(2)));
            }
            "sa" => ix(t, g.pick(&[2, 4, 5, 0, 4])),
            "dd" => dh(t, g, heavy),
            "hd" => ix(t, g.int(0, 2)),
            "md" => {
                for _ in 0..4 {
                    point(t, g);
                }
                let dl = if g.chance(10) {
                    g.pick(&[f64::NAN, f64::INFINITY, 0.0, -0.25, 1e-20, 2.0])
                } else {
                    g.pick(&[0.2, 0.1, 0.25, 0.5, 0.3, 1.0, 0.05, 0.125])
                };
                t.push(format!("{:016x}", dl.to_bits()));
            }
            "sv" => ix(t, g.int(0, nv - 1)),
            "tt" | "tv" => ix(t, i64::from(g.chance(2))),
            "ke" => dh(t, g, heavy),
            "uk" => ix(t, g.int(0, 4)),
            "ca" => {
                let f = faces[g.below(nf as u64) as usize];
                let tri = if g.chance(5) {
                    [g.int(0, nv - 1), g.int(0, nv - 1), g.int(0, nv - 1)]
                } else {
                    f
                };
                for x in tri {
                    ix(t, x);
                }
                ix(t, i64::from(g.chance(3)));
            }
            "cn" => {
                ix(t, g.pick(&[4, 0, 9, 7, 4, 9, 3]));
                ix(t, g.int(0, nv - 1));
            }
            "cs" => {
                ix(t, g.int(0, 2));
                ix(t, g.int(0, nv - 1));
            }
            "su" => {
                ix(t, g.pick(&[4, 9, 7, 0, 9, 4, 3]));
                ix(t, g.int(0, 3));
            }
            "co" | "cp" | "ct" => {
                ix(
                    t,
                    if g.chance(40) {
                        g.pick(&[100, 101, -1])
                    } else {
                        g.int(0, 4)
                    },
                );
                if op == "ct" {
                    if g.chance(4) {
                        fh(t, g, heavy);
                    } else {
                        t.push(format!("{:08x}", (g.int(1, 40) as f32 / 4.0).to_bits()));
                    }
                }
            }
            "sfc" => {
                ix(t, g.int(0, nf - 1));
                ix(t, g.int(0, 2));
            }
            _ => {}
        }
    }
}

fn dump_planes(o: &mut Out, s: &Mris) {
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z] {
            o.f(x);
        }
    }
}

fn dump_dist_orig(o: &mut Out, s: &Mris) {
    for vno in 0..s.nvertices.max(0) as usize {
        let n = s.topology(vno).vtotal.max(0) as usize;
        let d = s.vertex(vno).dist_orig.as_deref().unwrap_or(&[]);
        for x in &d[..n] {
            o.f(*x);
        }
    }
}

fn dump_normals(o: &mut Out, s: &Mris) {
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [
            v.x, v.y, v.z, v.nx, v.ny, v.nz, v.wnx, v.wny, v.wnz, v.pnx, v.pny, v.pnz, v.onx,
            v.ony, v.onz, v.tx, v.ty, v.tz,
        ] {
            o.f(x);
        }
    }
}

fn dump_marks(o: &mut Out, s: &Mris) {
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        o.i(i64::from(v.marked));
        for x in [v.x, v.y, v.z] {
            o.f(x);
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

fn f3(t: &mut Toks) -> [f32; 3] {
    [t.f(), t.f(), t.f()]
}

fn d3(t: &mut Toks) -> [f64; 3] {
    [t.d(), t.d(), t.d()]
}

pub(super) fn mpc_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    let mut pairs: Vec<(String, String)> = vec![
        ("SURFER_SIDEDOOR".into(), "1".into()),
        ("FREESURFER_SEED".into(), t.i().to_string()),
    ];
    if t.i() != 0 {
        pairs.push(("FS_DISTURB_DISTANCES".into(), t.next().to_owned()));
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
    let status = t.i();
    if status >= 0 {
        s.status = MrisStatus(status);
    }
    if t.i() != 0 {
        s.orig_area = t.f();
    }
    for vno in 0..nv as usize {
        dist::make_dist_orig(&mut s, vno);
        let vtotal = s.topology(vno).vtotal.max(0) as i32;
        let m = t.i();
        let v = s.vertex_mut(vno);
        if m & 1 != 0 {
            v.ripflag = 1;
        }
        if m & 2 != 0 {
            [v.origx, v.origy, v.origz] = f3(t);
        }
        if m & 4 != 0 {
            [v.cx, v.cy, v.cz] = f3(t);
        }
        if m & 8 != 0 {
            v.val = t.f();
        }
        if m & 16 != 0 {
            v.val2 = t.f();
        }
        if m & 32 != 0 {
            v.k1 = t.f();
        }
        if m & 64 != 0 {
            v.k2 = t.f();
        }
        if m & 128 != 0 {
            [v.nx, v.ny, v.nz] = f3(t);
        }
        if m & 256 != 0 {
            [v.pialx, v.pialy, v.pialz] = f3(t);
        }
        if m & 512 != 0 {
            [v.whitex, v.whitey, v.whitez] = f3(t);
        }
        if m & 4096 != 0 {
            [v.x, v.y, v.z] = f3(t);
        }
        if m & 8192 != 0 {
            [v.wnx, v.wny, v.wnz] = f3(t);
        }
        if m & 16384 != 0 {
            v.marked = t.i() as i16;
        }
        if m & 32768 != 0 {
            [v.dx, v.dy, v.dz] = f3(t);
        }
        if m & 65536 != 0 {
            [v.odx, v.ody, v.odz] = f3(t);
        }
        if m & 131072 != 0 {
            v.area = t.f();
        }
        if m & 262144 != 0 {
            v.fixedval = 1;
        }
        for (bit, orig) in [(1024, true), (2048, false)] {
            if m & bit != 0 {
                let k = t.i();
                for j in 0..k {
                    let x = t.f();
                    if j < vtotal {
                        let a = if orig { &mut v.dist_orig } else { &mut v.dist };
                        a.as_mut().expect("distances")[j as usize] = x;
                    }
                }
            }
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
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "fb" => {
                let (f, w) = (t.i() as usize, t.i());
                let c = d3(t);
                let (r, l) = props2::face_barycentric_coords(g, &mut io, &s, f, w, c)?;
                o.i(i64::from(r));
                for x in l {
                    o.d(x);
                }
            }
            "fr" => {
                let (a, b, c, p) = (d3(t), d3(t), d3(t), d3(t));
                let (r, l) = props2::face_barycentric_coords_raw(g, a, b, c, p);
                o.i(i64::from(r));
                for x in l {
                    o.d(x);
                }
            }
            "pp" => {
                let (a, b, c, p) = (d3(t), d3(t), d3(t), d3(t));
                let pr = props2::project_point_to_plane(p, a, b, c);
                o.i(i64::from(pr.ret));
                for x in pr.proj {
                    o.d(x);
                }
                match pr.basis {
                    Some((e1, e2)) => {
                        for x in e1.iter().chain(e2.iter()) {
                            o.d(*x);
                        }
                    }
                    None => {
                        for _ in 0..6 {
                            o.d(7.0);
                        }
                    }
                }
                for x in pr.point {
                    o.d(x);
                }
            }
            "sc" => {
                let f = t.i() as usize;
                let p = d3(t);
                let (wc, wb) = (t.i(), t.i());
                let (r, xyz) = props2::sample_face_coords(g, &mut io, &s, f, p, wc, wb)?;
                o.i(i64::from(r));
                for x in xyz {
                    o.f(x);
                }
            }
            "sf" => {
                let (f, w) = (t.i() as usize, t.i());
                let (p, vals) = (d3(t), d3(t));
                o.d(props2::sample_face(g, &mut io, &s, f, w, p, vals)?);
            }
            "sn" => {
                let f = t.i() as usize;
                let p = d3(t);
                for x in props2::sample_face_normal(&s, f, p) {
                    o.f(x);
                }
            }
            "rh" => {
                o.d(props2::rms_tp_height(g, &mut io, &mut s));
                dump_planes(o, &s);
            }
            "ae" => {
                let w = t.i();
                let pct = if t.i() != 0 {
                    let mut fp = cx.a.clone();
                    props2::compute_analytic_distance_error(g, &mut io, &s, w, Some(&mut fp))?
                } else {
                    props2::compute_analytic_distance_error(g, &mut io, &s, w, None)?
                };
                o.d(pct);
            }
            "sa" => {
                let w = t.i();
                o.d(props2::store_analytic_distances(g, &mut io, &mut s, w)?);
                dump_dist_orig(o, &s);
            }
            "dd" => {
                let pct = t.d();
                o.i(i64::from(props2::disturb_original_distances(
                    g, &mut io, &mut s, pct,
                )?));
                dump_dist_orig(o, &s);
            }
            "hd" => {
                let mode = t.i();
                o.d(props2::compute_hausdorff_distance(&s, mode));
            }
            "ur" | "uc" => {
                o.i(i64::from(if op == "ur" {
                    props2::use_curvature_ratio(&mut s)
                } else {
                    props2::use_curvature_contrast(&mut s)
                }));
                dump_curv(o, &s);
            }
            "md" => {
                let p = [d3(t), d3(t), d3(t)];
                let q = d3(t);
                let dl = t.d();
                match props2::min_dist_to_triangle_bf(g, &mut io, p, q, dl) {
                    Some((dmin, pmin)) => {
                        o.d(dmin);
                        for x in pmin.unwrap_or([7.0; 3]) {
                            o.d(x);
                        }
                    }
                    None => {
                        o.s("REFUSED");
                    }
                }
            }
            "sv" => {
                for x in props2::stuff_vertex_coords(&s, t.i() as usize) {
                    o.d(x);
                }
            }
            "tt" => {
                o.i(i64::from(props4::track_total_distance(&mut s, t.i() != 0)));
                dump_curv(o, &s);
            }
            "tv" => {
                let r = if t.i() != 0 {
                    props4::total_variation_difference(g, &mut io, &s)
                } else {
                    props4::total_variation(g, &mut io, &s)
                };
                o.d(r);
            }
            "ke" => o.d(props4::curvature_error(&s, t.d())),
            "uk" => {
                o.i(i64::from(match t.i() {
                    0 => props4::use_curvature_difference(&mut s),
                    1 => props4::use_curvature_max(&mut io, &mut s),
                    2 => props4::use_curvature_min(&mut s),
                    3 => props4::use_curvature_stretch(g, &mut s),
                    _ => props4::use_neg_curvature(&mut s),
                }));
                dump_curv(o, &s);
                o.i(i64::from(g.errno));
            }
            "ca" => {
                let (a, b, c, m) = (t.i(), t.i(), t.i(), t.i());
                o.i(i64::from(props4::contains_another_vertex_on_sphere(
                    g, &mut s, a, b, c, m,
                )));
                dump_marks(o, &s);
            }
            "cn" => {
                let (w, v) = (t.i(), t.i() as usize);
                match props3::compute_normal(g, sg, &mut io, &s, w, v)? {
                    Some(n) => {
                        o.i(0);
                        for x in n {
                            o.d(x);
                        }
                    }
                    None => {
                        o.i(-4);
                        for _ in 0..3 {
                            o.d(7.0);
                        }
                    }
                }
            }
            "cs" => {
                let set =
                    [props3::Set::Original, props3::Set::White, props3::Set::Pial][t.i() as usize];
                let (r, n) = props3::compute_set_normal(&s, t.i() as usize, set);
                o.i(i64::from(r));
                for x in n {
                    o.f(x);
                }
            }
            "su" => {
                let (w, navgs) = (t.i(), t.i());
                o.i(i64::from(props3::compute_surface_normals(
                    g, sg, &mut io, &mut s, w, navgs,
                )?));
                dump_normals(o, &s);
            }
            "co" | "cp" | "ct" => {
                let nbhd = t.i();
                o.i(i64::from(match op {
                    "co" => props3::find_closest_orig_vertices(g, &mut io, &mut s, nbhd),
                    "cp" => props3::find_closest_pial_vertices_canonical_coords(
                        g, &mut io, &mut s, nbhd,
                    ),
                    _ => {
                        let max_thick = t.f();
                        props3::measure_cortical_thickness(g, &mut io, &mut s, nbhd, max_thick)
                    }
                }));
                dump_curv(o, &s);
                dump_marks(o, &s);
            }
            "sfc" => {
                let (f, c) = (t.i() as usize, t.i() as usize);
                for x in props2::stuff_face_coords(&s, f, c) {
                    o.d(x);
                }
            }
            _ => panic!("mpc op {op}"),
        }
    }
    Ok(())
}

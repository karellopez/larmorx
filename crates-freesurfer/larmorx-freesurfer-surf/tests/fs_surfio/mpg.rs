// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 24: `mpg`, queue item 3.1-51's leftovers, second group
//! (`props10.rs`): a surface read by `MRISread` (`d/lh.s0`: subdivided icosahedra, closed; open
//! grids and srd's small meshes, whose edge tables fail) with some vertices ripped, curvatures
//! given or not, a mask of one voxel per vertex (or of a wrong size, or none) and
//! `MRISfaceNormalFace`'s test delta; then 1 to 5 operations: the edge count (`cnt`), the edge
//! table (`edg`), the corners (`cor`), the face metric (`fm`), one face normal (`fnf`), the edge
//! metric (`em`), the corner metric (`cm`), the statistics (`ast`, `est`, `cst`), the overlay
//! (`ovl`), the two printers (`pq`, `ppq`), the curvature averaging (`avc`), the middle of the
//! grey matter (`mid`), a vertex moved (`mv`) and `MRIScomputeMetricProperties` (`mp`): the
//! generator and the port's answers, as `fssurfio.cpp`'s.

use super::mpd::{grid, ico};
use super::mpf::dump_vol;
use super::srd::{be4, mesh};
use super::*;

use larmorx_core::x87::F80;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, mri_alloc};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_surf::props10;
use larmorx_freesurfer_surf::readwrite;

fn small(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    if heavy && g.chance(6) {
        t.push(format!("{:08x}", smallish(g, heavy)));
    } else {
        t.push(format!("{:08x}", (g.int(-16, 16) as f32 / 8.0).to_bits()));
    }
}

pub(super) fn mpg_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let r = g.pick(&[1.0, 10.0, 100.0]);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        2 | 3 => mesh(g, heavy),
        4 => ico(g, 2, r),
        _ => ico(g, 1, r),
    };
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Sat Nov 14 01:10:19 2026\n\n");
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
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    // curvatures
    let curv = g.chance(2);
    ix(t, i64::from(curv));
    if curv {
        for _ in 0..nv {
            small(t, g, heavy);
        }
    }
    // the mask: none, one voxel per vertex, or one more or fewer
    let mw = match g.below(6) {
        0..=2 => 0,
        3 if nv > 1 => nv + g.pick(&[1, -1]),
        _ => nv,
    };
    ix(t, mw);
    for _ in 0..mw {
        let x: f32 = if g.chance(4) {
            0.0
        } else {
            g.pick(&[1.0, 0.5, 0.25, 2.0])
        };
        t.push(format!("{:08x}", x.to_bits()));
    }
    // MRISfaceNormalFace's test delta
    let dv = if g.chance(8) { g.int(-1, 3) } else { -1 };
    ix(t, dv);
    for _ in 0..3 {
        let x = if dv >= 0 {
            g.int(-4, 4) as f64 / 16.0
        } else {
            0.0
        };
        t.push(format!("{:016x}", x.to_bits()));
    }
    let nops = g.int(1, 5);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(20) {
            0 => t.push("cnt".into()),
            1 => t.push("edg".into()),
            2 => t.push("cor".into()),
            3 => t.push("fm".into()),
            4 => {
                t.push("fnf".into());
                ix(
                    t,
                    if g.chance(8) {
                        g.pick(&[-1, nf])
                    } else {
                        g.int(0, (nf - 1).max(0))
                    },
                );
                ix(t, g.int(0, 3));
            }
            5 => t.push("em".into()),
            6 => t.push("cm".into()),
            7 => {
                t.push("ast".into());
                ix(t, g.int(0, 1));
            }
            8 | 9 => {
                t.push("est".into());
                ix(
                    t,
                    if g.chance(10) {
                        g.pick(&[-1, 3])
                    } else {
                        g.int(0, 2)
                    },
                );
                ix(t, g.int(0, 1));
            }
            10 => {
                t.push("cst".into());
                ix(
                    t,
                    if g.chance(10) {
                        g.pick(&[-1, 2])
                    } else {
                        g.int(0, 1)
                    },
                );
                ix(t, g.int(0, 1));
            }
            11 => {
                t.push("ovl".into());
                ix(
                    t,
                    if g.chance(10) {
                        g.pick(&[-1, 3])
                    } else {
                        g.int(0, 2)
                    },
                );
                ix(t, g.int(0, 1));
            }
            12 | 13 => t.push("pq".into()),
            14 | 15 => t.push("ppq".into()),
            16 => {
                t.push("avc".into());
                ix(t, g.int(0, 3));
            }
            17 => t.push("mid".into()),
            18 if nv > 0 => {
                t.push("mv".into());
                ix(t, g.int(0, nv - 1));
                for _ in 0..3 {
                    small(t, g, heavy);
                }
            }
            _ => t.push("mp".into()),
        }
    }
}

fn stats(o: &mut Out, s: Option<[f64; 5]>) {
    match s {
        None => o.s("NULL"),
        Some(s) => s.iter().for_each(|&x| o.d(x)),
    }
}

pub(super) fn mpg_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let rips: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
    let d = Some(dir);
    let (mut so, mut se, mut fp) = (cx.a.clone(), cx.b.clone(), cx.a.clone());
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
    if t.i() != 0 {
        for vno in 0..s.nvertices as usize {
            s.vertex_mut(vno).curv = t.f();
        }
    }
    let mw = t.i();
    let mask = if mw > 0 {
        let mut m = mri_alloc(g, io.out, io.err, mw, 1, 1, MRI_FLOAT)?;
        for c in 0..mw {
            let x = t.f();
            mri_set_vox_val(g, io.err, &mut m, c, 0, 0, 0, x, 0)?;
        }
        Some(m)
    } else {
        None
    };
    sg.face_normal_add_delta_vertex = t.i();
    for k in 0..3 {
        sg.face_normal_add_delta[k] = F80::from_f64(t.d());
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "cnt" => o.i(i64::from(props10::mris_count_edges(&s))),
            "edg" => {
                o.i(i64::from(props10::mris_edges(&mut io, &mut s)));
                o.i(i64::from(s.nedges));
                match &s.edges {
                    None => o.s("N"),
                    Some(edges) => {
                        for e in edges {
                            e.vtxno.iter().for_each(|&x| o.i(i64::from(x)));
                            e.faceno.iter().for_each(|&x| o.i(i64::from(x)));
                            for c in &e.corner {
                                o.i(i64::from(c[0]));
                                o.i(i64::from(c[1]));
                            }
                        }
                        for vno in 0..s.nvertices as usize {
                            let vt = s.topology(vno);
                            match &vt.e {
                                None => o.s("N"),
                                Some(e) => e[..vt.vtotal.max(0) as usize]
                                    .iter()
                                    .for_each(|&x| o.i(i64::from(x))),
                            }
                        }
                    }
                }
            }
            "cor" => {
                o.i(i64::from(props10::mris_corners(g, &mut io, &mut s)?));
                o.i(i64::from(s.ncorners));
                for c in s.corners.iter().flatten() {
                    c.vtxno.iter().for_each(|&x| o.i(i64::from(x)));
                    o.i(i64::from(c.faceno));
                    c.edgeno.iter().for_each(|&x| o.i(i64::from(x)));
                    c.edgedir.iter().for_each(|&x| o.i(i64::from(x)));
                }
            }
            "fm" => {
                o.i(i64::from(props10::mris_face_metric(sg, &mut io, &mut s)));
                for f in 0..s.nfaces as usize {
                    match &s.face(f).norm {
                        None => o.s("N"),
                        Some(n) => (1..=3).for_each(|r| o.d(n.get(r, 1))),
                    }
                }
            }
            "fnf" => {
                let (fno, want) = (t.i(), t.i());
                let mut c = None;
                let mut cl = 0.0;
                let r = props10::mris_face_normal_face(
                    sg,
                    &mut io,
                    &mut s,
                    fno,
                    (want & 1 != 0).then_some(&mut c),
                    (want & 2 != 0).then_some(&mut cl),
                );
                o.i(i64::from(r));
                if let Some(c) = &c {
                    (1..=3).for_each(|r| o.d(c.get(r, 1)));
                }
                o.d(cl);
                if fno >= 0
                    && fno < s.nfaces
                    && let Some(n) = &s.face(fno as usize).norm
                {
                    (1..=3).for_each(|r| o.d(n.get(r, 1)));
                }
            }
            "em" => {
                if s.edges.is_none() {
                    props10::mris_edges(&mut io, &mut s);
                }
                o.i(i64::from(props10::mris_edge_metric(
                    g, sg, &mut io, &mut s,
                )?));
                for e in s.edges.iter().flatten() {
                    for x in [e.len, e.u[0], e.u[1], e.u[2], e.area[0], e.area[1]] {
                        o.d(x);
                    }
                    for x in [e.maxarea, e.dot, e.angle] {
                        o.d(x);
                    }
                }
            }
            "cm" => {
                o.i(i64::from(props10::mris_corner_metric(g, &mut io, &mut s)?));
                for c in s.corners.iter().flatten() {
                    o.d(c.dot);
                    o.d(c.angle);
                }
            }
            "ast" => {
                let m = t.i();
                let mk = mask.as_ref().filter(|_| m != 0);
                let r = props10::mris_triangle_area_stats(g, sg, &mut io, &mut s, mk)?;
                stats(o, r);
            }
            "est" | "cst" => {
                let (id, m) = (t.i(), t.i());
                let mk = mask.as_ref().filter(|_| m != 0);
                let r = if op == "est" {
                    props10::mris_edge_stats(g, sg, &mut io, &mut s, id, mk)?
                } else {
                    props10::mris_corner_stats(g, sg, &mut io, &mut s, id, mk)?
                };
                stats(o, r);
            }
            "ovl" => {
                let (id, m) = (t.i(), t.i());
                let mk = mask.as_ref().filter(|_| m != 0);
                let r = props10::mris_max_edge_stat_to_overlay(g, sg, &mut io, &mut s, id, mk)?;
                dump_vol(g, io.err, r.as_ref(), o);
            }
            "pq" => {
                let r = props10::mris_print_surf_quality_stats(g, sg, &mut io, &mut fp, &mut s)?;
                o.i(i64::from(r));
            }
            "ppq" => {
                let r =
                    props10::mris_pretty_print_surf_quality_stats(g, sg, &mut io, &mut fp, &mut s)?;
                o.i(i64::from(r));
            }
            "avc" => {
                let n = t.i();
                o.i(i64::from(props10::mris_average_curvatures(&mut s, n)));
                o.f(s.min_curv);
                o.f(s.max_curv);
                for vno in 0..s.nvertices as usize {
                    o.f(s.vertex(vno).curv);
                }
            }
            "mid" => {
                props10::mris_find_middle_of_gray(g, sg, &mut io, &mut s)?;
                for vno in 0..s.nvertices as usize {
                    let v = s.vertex(vno);
                    for x in [v.x, v.y, v.z, v.curv] {
                        o.f(x);
                    }
                }
            }
            "mv" => {
                let vno = t.i();
                let (x, y, z) = (t.f(), t.f(), t.f());
                let file = format!("{}/fssurfio.cpp", "probe");
                coords::set_xyz(g, &mut io, &mut s, vno, x, y, z, &file, 0)?;
            }
            "mp" => {
                larmorx_freesurfer_surf::mp::compute_metric_properties(g, sg, &mut io, &mut s)?;
            }
            _ => panic!("mpg op {op}"),
        }
    }
    Ok(())
}

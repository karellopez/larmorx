// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command 63 (relay 43 of track D): `prj`, `mrisurf_project.cpp` (`project.rs`): a
//! surface read by `MRISread` (`d/lh.s0`: icosahedra of radius 5 to 120, now and then crumpled or
//! off centre, open grids, srd's small meshes), its status (a surface, an ellipsoid, a sphere, a
//! parameterized sphere), its radius, some vertices ripped, `Gdiag` (0 or `DIAG_SHOW |
//! DIAG_VERBOSE`); then 1 to 4 operations: `mrisSphericalProjectXYZ` of a point (`spx`),
//! `mrisSphericalProjection` (`sp`), `MRISprojectOntoSphere` (`pos`; `pos2` its two-surface form,
//! the same surface or not), `mrisAssignFaces` with a face table (`af`), `MRISprojectOntoCylinder`
//! (`cyl`), `MRISprojectOntoEllipsoid` and `MRISradialProjectOntoEllipsoid` into the surface or a
//! new copy (`ell`, `rad`); after each the surface (status, radius, axes, centre, areas; each
//! vertex's current and canonical positions and face; each face's area; after `cyl` the vertices'
//! areas, normals and curvatures): the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mht::crumple;
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::{project, readwrite};

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

fn axis(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(6) {
        return g.pick(&[f32::NAN, -10.0, 1e-30]);
    }
    g.pick(&[0.0, 122.0, 100.0, 50.0, 80.0, 30.0, 10.0])
}

pub(super) fn prj_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut m = match g.below(10) {
        0..=3 => {
            let r = g.pick(&[5.0, 10.0, 50.0, 100.0, 120.0]);
            let mut m = ico(g, 1, r);
            if g.chance(3) {
                crumple(g, &mut m, 4);
            }
            m
        }
        4 | 5 => {
            let r = g.pick(&[10.0, 100.0]);
            ico(g, 2, r)
        }
        6 | 7 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        _ => mesh(g, heavy),
    };
    if g.chance(4) {
        // off centre
        let o = [g.int(-8, 8) as f32, g.int(-8, 8) as f32, g.int(-8, 8) as f32];
        for p in &mut m.v {
            for k in 0..3 {
                p[k] += o[k];
            }
        }
    }
    if heavy && g.chance(4) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = g.pick(&[f32::NAN, f32::INFINITY, 0.0]);
        if g.chance(2) {
            m.v[k] = [0.0, 0.0, 0.0];
        }
    }
    surface(t, &m);
    let nv = m.v.len() as i64;
    ix(t, g.pick(&[0, 0, 3, 4, 4, 5, 2]));
    dv(t, g.pick(&[100.0, 100.0, 50.0, 0.0, 10.0]));
    let nrip = if g.chance(5) && nv > 1 { g.int(1, 2) } else { 0 };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    ix(t, g.pick(&[0, 0, 0, 0x1000_0040, 0x40]));
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(12) {
            0 => {
                t.push("spx".into());
                for _ in 0..3 {
                    fv(
                        t,
                        if heavy && g.chance(5) {
                            g.pick(&[f32::NAN, 0.0, 1e30])
                        } else {
                            g.int(-400, 400) as f32 / 4.0
                        },
                    );
                }
            }
            1 => t.push("sp".into()),
            2 | 3 => {
                t.push("pos".into());
                dv(
                    t,
                    if heavy && g.chance(5) {
                        g.pick(&[f64::NAN, -50.0, 1e-9])
                    } else {
                        g.pick(&[100.0, 0.0, 50.0, 10.0, 1.0])
                    },
                );
            }
            4 => {
                t.push("pos2".into());
                ix(t, g.pick(&[1, 1, 1, 0]));
                dv(t, g.pick(&[100.0, 0.0, 60.0]));
            }
            5 | 6 => {
                t.push("af".into());
                ix(t, g.pick(&[4, 4, 3]));
                fv(t, g.pick(&[1.0, 2.0, 4.0, 10.0]));
            }
            7 => {
                t.push("cyl".into());
                fv(
                    t,
                    if heavy && g.chance(4) {
                        g.pick(&[f32::NAN, 0.0, -5.0])
                    } else {
                        g.pick(&[10.0, 50.0, 100.0, 5.0])
                    },
                );
            }
            8..=10 => {
                t.push("ell".into());
                for _ in 0..3 {
                    fv(t, axis(g, heavy));
                }
                ix(t, i64::from(g.chance(3)));
            }
            _ => {
                t.push("rad".into());
                for _ in 0..3 {
                    fv(t, axis(g, heavy));
                }
                ix(t, i64::from(g.chance(3)));
            }
        }
    }
}

fn dump(s: &Mris, o: &mut Out, curv: bool) {
    o.i(i64::from(s.status.0));
    o.d(s.radius);
    for x in [s.a, s.b, s.c, s.xctr, s.yctr, s.zctr] {
        o.f(x);
    }
    for x in [s.total_area, s.neg_area, s.neg_orig_area] {
        o.f(x);
    }
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z, v.cx, v.cy, v.cz] {
            o.f(x);
        }
        o.i(i64::from(v.fno));
        if curv {
            for x in [v.area, v.nx, v.ny, v.nz, v.H, v.K] {
                o.f(x);
            }
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        o.f(s.face(fno).area);
    }
}

pub(super) fn prj_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    s.status = aaa::MrisStatus(t.i());
    s.radius = t.d();
    for _ in 0..t.i() {
        s.vertex_mut(t.i() as usize).ripflag = 1;
    }
    g.gdiag = u64::from(t.i() as u32);
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        let mut curv = false;
        match op {
            "spx" => {
                let (x, y, z) = (t.f(), t.f(), t.f());
                let (a, b, c) = project::spherical_project_xyz(x, y, z);
                o.f(a);
                o.f(b);
                o.f(c);
                continue;
            }
            "sp" => project::spherical_projection(&mut s),
            "pos" => {
                let r = t.d();
                o.i(i64::from(project::project_onto_sphere(g, &mut io, &mut s, r)));
            }
            "pos2" => {
                let (same, r) = (t.i(), t.d());
                o.i(i64::from(project::project_onto_sphere_src_dst(
                    g,
                    &mut io,
                    &mut s,
                    same != 0,
                    r,
                )?));
            }
            "af" => {
                let (which, res) = (t.i(), t.f());
                let m = Mht::create_face_table_resolution(g, io.err, &s, which, res)?;
                project::assign_faces(g, io.err, &mut s, &m, which)?;
            }
            "cyl" => {
                let r = t.f();
                o.i(i64::from(project::project_onto_cylinder(
                    g, sg, &mut io, d, &mut s, r,
                )?));
                curv = true;
            }
            "ell" | "rad" => {
                let (a, b, c, new) = (t.f(), t.f(), t.f(), t.i() != 0);
                let r = if op == "ell" {
                    project::project_onto_ellipsoid(g, sg, &mut io, &mut s, new, a, b, c)?
                } else {
                    project::radial_project_onto_ellipsoid(g, sg, &mut io, &mut s, new, a, b, c)?
                };
                if let Some(n) = r {
                    o.s("NEW");
                    dump(&n, o, false);
                }
            }
            _ => panic!("prj op {op}"),
        }
        dump(&s, o, curv);
    }
    Ok(())
}

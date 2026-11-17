// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command 66 (relay 45 of track D): `smp`, the sampled energies with
//! `MRISsampleFaceCoordsCanonical` (fs.surf's `sampled.rs`) and the face-table walks
//! `mrisDirectionTriangleIntersection`, `MRISdistanceToSurface` and `mrisFindAllOverlappingFaces`
//! (`overlap.rs`): a surface read by `MRISread` (`d/lh.s0`: icosahedra of radius 10 to 100, now and
//! then crumpled (self-intersecting), grids, srd's small meshes), its radius and mean vertex
//! distance; then 2 to 8 operations: the positions saved (`sv`: canonical, pial, white), the normals
//! saved (`sn`: pial, white), a deterministic move of the current positions (`jit`), a face or
//! vertex table (`mht`, `vmht`: of the canonical or current positions), the vertices' faces set by
//! a pattern (`fno`) or assigned (`asg`, `mrisAssignFaces`), a vertex or face ripped (`rv`, `rf`);
//! the canonical sampling (`sfc`), the minimization, normal, spring, parallel (at a vertex and
//! sampled) energies (`sme`, `sne`, `sse`, `spv`, `spe`), the ray (`dti`), the distance along a
//! vertex's normal (`dts`) and the overlapping faces (`fao`, with every face's rip flag after it);
//! at the end each vertex's position and face: the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mht::crumple;
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::{overlap, positions, project, readwrite, sampled};

/// A point near the surface's own: a vertex's position scaled, moved by a few millimetres, now and
/// then the origin or not finite.
fn point(t: &mut Vec<String>, g: &mut Gen, v: &[[f32; 3]]) {
    let p = if v.is_empty() || g.chance(60) {
        g.pick(&[[0.0, 0.0, 0.0], [f32::NAN, 1.0, 1.0], [1e30, 0.0, 0.0]])
    } else {
        let q = v[g.below(v.len() as u64) as usize];
        let s = g.pick(&[1.0f32, 1.0, 1.0, 0.5, 2.0, -1.0]);
        let mut r = [q[0] * s, q[1] * s, q[2] * s];
        for c in &mut r {
            if g.chance(3) {
                *c += g.int(-16, 16) as f32 / 8.0;
            }
        }
        r
    };
    for c in p {
        fv(t, c);
    }
}

pub(super) fn smp_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut r = 100.0f32;
    let mut m = match g.below(10) {
        0..=4 => {
            r = g.pick(&[10.0, 20.0, 50.0, 100.0]);
            let levels = g.pick(&[1, 1, 2]);
            let mut m = ico(g, levels, r);
            if g.chance(3) {
                crumple(g, &mut m, 4);
            }
            m
        }
        5 | 6 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        _ => mesh(g, heavy),
    };
    if heavy && g.chance(4) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = g.pick(&[f32::NAN, 0.0]);
    }
    surface(t, &m);
    let nv = m.v.len() as i64;
    let nf = m.f.len() as i64;
    let rr = if g.chance(6) { g.pick(&[1.0, 50.0]) } else { r };
    t.push(format!("{:016x}", f64::from(rr).to_bits()));
    let avd = g.pick(&[0.0, 1.0, 2.5, 0.0]);
    t.push(format!("{:016x}", f64::to_bits(avd)));
    let mut ops: Vec<Vec<String>> = Vec::new();
    // the set-up every case starts with: canonical, pial and white positions and normals, a table
    ops.push(vec!["sv".into(), "3".into()]);
    if g.chance(2) {
        let mut j = vec!["jit".to_owned()];
        fv(&mut j, g.pick(&[0.25, 0.5, 1.0]));
        ops.push(j);
    }
    ops.push(vec!["sv".into(), "7".into()]);
    ops.push(vec!["sn".into(), "7".into()]);
    if g.chance(2) {
        let mut j = vec!["jit".to_owned()];
        fv(&mut j, g.pick(&[0.25, 0.5]));
        ops.push(j);
    }
    ops.push(vec!["sv".into(), "9".into()]);
    ops.push(vec!["sn".into(), "9".into()]);
    let mut mk = vec![if heavy && g.chance(6) { "vmht" } else { "mht" }.to_owned()];
    ix(&mut mk, g.pick(&[3, 3, 3, 4]));
    fv(&mut mk, g.pick(&[1.0, 2.0, 4.0, 8.0]));
    ops.push(mk);
    // the vertices' faces: a pattern (now and then -1 or beyond the faces), MRISread's 0, or assigned
    match g.below(5) {
        0 => ops.push(vec!["fno".into(), g.pick(&["1", "3"]).into()]),
        1 => {}
        _ => ops.push(vec!["asg".into()]),
    }
    let nops = g.int(1, 6);
    for _ in 0..nops {
        let mut op: Vec<String> = Vec::new();
        let vno = g.int(0, (nv - 1).max(0));
        match g.below(18) {
            0 => {
                op.push("sfc".into());
                ix(&mut op, g.pick(&[7, 7, 13, 9, 3, 4, 99]));
                point(&mut op, g, &m.v);
            }
            1 | 2 => {
                op.push("sme".into());
                ix(&mut op, vno);
                point(&mut op, g, &m.v);
            }
            3 | 4 => {
                op.push("sne".into());
                ix(&mut op, vno);
                point(&mut op, g, &m.v);
            }
            5 => {
                op.push("sse".into());
                ix(&mut op, vno);
                point(&mut op, g, &m.v);
            }
            6 => {
                op.push("spv".into());
                ix(&mut op, vno);
            }
            7 | 8 => {
                op.push("spe".into());
                ix(&mut op, vno);
                point(&mut op, g, &m.v);
            }
            9 | 10 => {
                op.push("dti".into());
                ix(&mut op, vno);
                ix(&mut op, if g.chance(3) { -1 } else { vno });
                op.push(format!(
                    "{:016x}",
                    f64::to_bits(g.pick(&[0.0, 0.25, 1.0, 2.0, 5.0]))
                ));
            }
            11 => {
                op.push("dts".into());
                ix(&mut op, vno);
                ix(&mut op, if g.chance(2) { 1 } else { -1 });
            }
            12 | 13 => {
                op.push("fao".into());
                ix(&mut op, g.int(0, (nf - 1).max(0)));
            }
            14 => {
                let mut mk = vec!["mht".to_owned()];
                ix(&mut mk, g.pick(&[3, 4]));
                fv(&mut mk, g.pick(&[1.0, 2.0, 4.0]));
                op = mk;
            }
            15 => op.push("asg".into()),
            16 => {
                op.push("rv".into());
                ix(&mut op, vno);
            }
            _ => {
                op.push("rf".into());
                ix(&mut op, g.int(0, (nf - 1).max(0)));
            }
        }
        ops.push(op);
    }
    ix(t, ops.len() as i64);
    for op in ops {
        t.extend(op);
    }
}

fn pt(t: &mut Toks) -> (f32, f32, f32) {
    (t.f(), t.f(), t.f())
}

pub(super) fn smp_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    s.radius = t.d();
    s.avg_vertex_dist = t.d();
    let mut mht: Option<Mht> = None;
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        match op {
            "sv" => {
                let which = t.i();
                o.i(i64::from(positions::save_vertex_positions(
                    g, &mut io, &mut s, which,
                )?));
            }
            "sn" => {
                let which = t.i();
                o.i(i64::from(positions::save_normals(g, &mut io, &mut s, which)?));
            }
            "jit" => {
                let a = t.f();
                for vno in 0..s.nvertices.max(0) as usize {
                    let k = vno as i64;
                    let v = s.vertex_mut(vno);
                    v.x += a * ((k * 7) % 5 - 2) as f32;
                    v.y += a * ((k * 3) % 7 - 3) as f32;
                    v.z += a * ((k * 5) % 3 - 1) as f32;
                }
            }
            "mht" => {
                let (which, res) = (t.i(), t.f());
                mht = Some(Mht::create_face_table_resolution(
                    g, io.err, &s, which, res,
                )?);
            }
            "vmht" => {
                let (which, res) = (t.i(), t.f());
                mht = Some(Mht::create_vertex_table_resolution(
                    g, io.err, &s, which, res,
                )?);
            }
            "fno" => {
                let k = t.i();
                for vno in 0..s.nvertices.max(0) {
                    s.vertex_mut(vno as usize).fno = (vno * 7) % (s.nfaces + k) - 1;
                }
            }
            "asg" => {
                let m = mht.as_ref().expect("a table");
                project::assign_faces(g, io.err, &mut s, m, 3)?;
            }
            "rv" => {
                let k = t.i();
                if k < s.nvertices {
                    s.vertex_mut(k as usize).ripflag = 1;
                }
            }
            "rf" => {
                let k = t.i();
                if k < s.nfaces {
                    s.face_mut(k as usize).ripflag = 1;
                }
            }
            "sfc" => {
                let which = t.i();
                let (x, y, z) = pt(t);
                let m = mht.as_ref().expect("a table");
                let (r, p) =
                    sampled::sample_face_coords_canonical(g, &mut io, m, &s, x, y, z, which)?;
                o.i(i64::from(r));
                for c in p {
                    o.f(c);
                }
            }
            "sme" | "sne" | "sse" | "spe" => {
                let vno = t.i() as usize;
                let (x, y, z) = pt(t);
                if vno as i32 >= s.nvertices {
                    o.s("NOV");
                    continue;
                }
                let m = mht.as_ref().expect("a table");
                let e = match op {
                    "sme" => sampled::sample_minimization_energy(g, &mut io, m, &s, vno, x, y, z)?,
                    "sne" => sampled::sample_normal_energy(g, &mut io, m, &s, vno, x, y, z)?,
                    "sse" => sampled::sample_spring_energy(g, &mut io, m, &s, vno, x, y, z)?,
                    _ => sampled::sample_parallel_energy(g, &mut io, m, &mut s, vno, x, y, z)?,
                };
                o.f(e);
            }
            "spv" => {
                let vno = t.i() as usize;
                if vno as i32 >= s.nvertices {
                    o.s("NOV");
                    continue;
                }
                let m = mht.as_ref().expect("a table");
                o.f(sampled::sample_parallel_energy_at_vertex(
                    g, &mut io, m, &s, vno,
                )?);
            }
            "dti" => {
                let (src, excl) = (t.i() as usize, t.i());
                let mut dist = t.d();
                if src as i32 >= s.nvertices {
                    o.s("NOV");
                    continue;
                }
                let (p, n) = {
                    let v = s.vertex(src);
                    ([v.x, v.y, v.z], [v.nx, v.ny, v.nz])
                };
                let m = mht.as_ref().expect("a table");
                let r = overlap::direction_triangle_intersection(
                    g, &mut io, &s, p, n, m, &mut dist, excl,
                )?;
                o.i(i64::from(r));
                o.d(dist);
            }
            "dts" => {
                let (src, sign) = (t.i() as usize, t.i());
                if src as i32 >= s.nvertices {
                    o.s("NOV");
                    continue;
                }
                let (p, n) = {
                    let v = s.vertex(src);
                    let k = sign as f32;
                    ([v.x, v.y, v.z], [v.nx * k, v.ny * k, v.nz * k])
                };
                let m = mht.as_ref().expect("a table");
                o.f(overlap::distance_to_surface(g, &mut io, &s, m, p, n)?);
            }
            "fao" => {
                let fno = t.i() as usize;
                if fno as i32 >= s.nfaces {
                    o.s("NOF");
                    continue;
                }
                let m = mht.as_ref().expect("a table");
                let list = overlap::find_all_overlapping_faces(g, &mut io, &mut s, m, fno)?;
                o.i(list.len() as i64);
                for f in list {
                    o.i(i64::from(f));
                }
                for k in 0..s.nfaces.max(0) as usize {
                    o.i(i64::from(s.face(k).ripflag));
                }
            }
            _ => panic!("smp op {op}"),
        }
    }
    o.s("|");
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
        o.i(i64::from(v.fno));
    }
    Ok(())
}

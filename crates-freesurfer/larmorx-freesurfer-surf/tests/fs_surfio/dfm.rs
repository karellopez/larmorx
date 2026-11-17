// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command 64 (relay 44 of track D): `dfm`, `mrisurf_deform.cpp`'s projection and
//! flattening (fs.deform's `project.rs`), `MRISremoveIntersections` (`intersect.rs`) and
//! `MRIScomputeDistanceToSurface` (fs.surf's `surfmri.rs`): a surface read by `MRISread`
//! (`d/lh.s0`: icosahedra of radius 5 to 100, now and then crumpled (self-intersecting) or off
//! centre, open grids, srd's small meshes), its radius, some vertices ripped, `Gdiag` (0,
//! `DIAG_SHOW`, `DIAG_SHOW | DIAG_VERBOSE`) and `Gdiag_no` (none, a vertex, beyond the surface);
//! then 1 to 5 operations: `mrisProjectSurface` after setting a status (`ps`, the pial status
//! with or without a face table), `MRISflattenPatch` after setting a status (`fp`),
//! `MRISremoveIntersections` (`rmi`), a deterministic move of the current positions (`jit`),
//! `MRISsaveVertexPositions` (`sv`), a face table on the surface (`mht`, `nomht`),
//! `MRIScomputeDistanceToSurface` into a new or a given volume (`cds`: the volume's type, extent,
//! voxel size, centre and the FNV-1a of its voxels); after each the surface (status, radius,
//! areas; each vertex's position, normal, area and mark; each face's area): the generator and the
//! port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mht::crumple;
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use std::sync::Arc;

use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_deform::{intersect, project};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, mri_alloc};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::aaa::Opaque;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::{positions, readwrite, surfmri};

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

pub(super) fn dfm_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut small = true;
    let mut m = match g.below(10) {
        0..=3 => {
            let r = g.pick(&[5.0, 10.0, 50.0, 100.0]);
            small = r <= 50.0;
            let mut m = ico(g, 1, r);
            if g.chance(2) {
                crumple(g, &mut m, 3);
            }
            m
        }
        4 | 5 => {
            let r = g.pick(&[10.0, 30.0]);
            let mut m = ico(g, 2, r);
            if g.chance(3) {
                crumple(g, &mut m, 5);
            }
            m
        }
        6 | 7 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        _ => mesh(g, heavy),
    };
    if g.chance(4) {
        // off centre
        let o = [
            g.int(-8, 8) as f32,
            g.int(-8, 8) as f32,
            g.int(-8, 8) as f32,
        ];
        for p in &mut m.v {
            for k in 0..3 {
                p[k] += o[k];
            }
        }
    }
    if heavy && g.chance(4) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = g.pick(&[f32::NAN, 0.0]);
        if g.chance(2) {
            m.v[k] = [0.0, 0.0, 0.0];
        }
    }
    surface(t, &m);
    let nv = m.v.len() as i64;
    dv(t, g.pick(&[100.0, 100.0, 50.0, 10.0]));
    let nrip = if g.chance(8) && nv > 1 {
        g.int(1, 2)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    ix(t, g.pick(&[0, 0, 0, 0x40, 0x1000_0040]));
    ix(t, g.pick(&[-1, -1, -1, 0, 3, nv + 5]));
    // the operations, each its tokens; a pial projection now and then with its set-up (the pial
    // positions saved, the current ones moved, a face table)
    let mut ops: Vec<Vec<String>> = Vec::new();
    let nops = g.int(1, 5);
    for _ in 0..nops {
        let mut op: Vec<String> = Vec::new();
        match g.below(16) {
            0..=2 => {
                op.push("ps".into());
                ix(&mut op, g.pick(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 12]));
            }
            3..=5 => {
                op.push("fp".into());
                ix(&mut op, g.pick(&[0, 0, 1, 2, 3, 4]));
            }
            6 | 7 => {
                op.push("rmi".into());
                ix(&mut op, g.pick(&[0, 0, 1]));
            }
            8 => {
                op.push("jit".into());
                fv(&mut op, g.pick(&[0.25, 0.5, 1.0, 2.0]));
            }
            9 => {
                op.push("sv".into());
                // PIAL_VERTICES, CANONICAL_VERTICES, FLATTENED_VERTICES
                ix(&mut op, g.pick(&[7, 7, 3, 6]));
            }
            10 => {
                op.push("mht".into());
                // the pial or current positions (coarse tables: the official probe's alarm)
                ix(&mut op, g.pick(&[7, 7, 4]));
                fv(&mut op, g.pick(&[2.0, 4.0, 8.0]));
            }
            11 => op.push("nomht".into()),
            12 | 13 => {
                ops.push(vec!["sv".into(), "7".into()]);
                let mut j = vec!["jit".to_owned()];
                fv(&mut j, g.pick(&[0.25, 0.5, 1.0]));
                ops.push(j);
                let mut m = vec!["mht".to_owned()];
                ix(&mut m, g.pick(&[7, 7, 4]));
                fv(&mut m, g.pick(&[2.0, 4.0, 8.0]));
                ops.push(m);
                op.push("ps".into());
                ix(&mut op, 9);
            }
            _ => {
                op.push("cds".into());
                fv(&mut op, if small { g.pick(&[1.0, 2.0, 4.0]) } else { 4.0 });
                ix(&mut op, g.pick(&[0, 0, 8, 16]));
            }
        }
        ops.push(op);
    }
    ix(t, ops.len() as i64);
    for op in ops {
        t.extend(op);
    }
}

fn dump(s: &Mris, o: &mut Out) {
    o.i(i64::from(s.status.0));
    o.d(s.radius);
    o.f(s.total_area);
    o.f(s.neg_area);
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z, v.nx, v.ny, v.nz, v.area] {
            o.f(x);
        }
        o.i(i64::from(v.marked));
    }
    for fno in 0..s.nfaces.max(0) as usize {
        o.f(s.face(fno).area);
    }
}

fn fnv(h: u64, b: &[u8]) -> u64 {
    b.iter()
        .fold(h, |h, &c| (h ^ u64::from(c)).wrapping_mul(0x100000001b3))
}

pub(super) fn dfm_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    for _ in 0..t.i() {
        s.vertex_mut(t.i() as usize).ripflag = 1;
    }
    g.gdiag = u64::from(t.i() as u32);
    g.gdiag_no = t.i();
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        match op {
            "ps" => {
                s.status = aaa::MrisStatus(t.i());
                o.i(i64::from(project::project_surface(g, sg, &mut io, &mut s)?));
            }
            "fp" => {
                s.status = aaa::MrisStatus(t.i());
                o.i(i64::from(project::flatten_patch(g, sg, &mut io, &mut s)?));
            }
            "rmi" => {
                let fill = t.i();
                o.i(i64::from(intersect::remove_intersections(
                    g, &mut io, &mut s, fill,
                )?));
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
            "sv" => {
                let which = t.i();
                o.i(i64::from(positions::save_vertex_positions(
                    g, &mut io, &mut s, which,
                )?));
            }
            "mht" => {
                let (which, res) = (t.i(), t.f());
                s.mht = Opaque::default();
                let m = Mht::create_face_table_resolution(g, io.err, &s, which, res)?;
                s.mht = Opaque(Some(Arc::new(m)));
            }
            "nomht" => s.mht = Opaque::default(),
            "cds" => {
                let (res, w) = (t.f(), t.i());
                let mut given = if w > 0 {
                    Some(mri_alloc(g, io.out, io.err, w, w, w, MRI_FLOAT)?)
                } else {
                    None
                };
                let r = surfmri::compute_distance_to_surface(
                    g,
                    sg,
                    &mut io,
                    &mut s,
                    given.as_mut(),
                    res,
                )?;
                let m = match r {
                    Ret::New(m) => Some(m),
                    Ret::Given => given,
                    Ret::Null => None,
                };
                match m {
                    None => o.s("NULL"),
                    Some(m) => {
                        o.i(i64::from(m.mtype));
                        o.i(i64::from(m.vg.width));
                        o.i(i64::from(m.vg.height));
                        o.i(i64::from(m.vg.depth));
                        for x in [
                            m.vg.xsize, m.vg.ysize, m.vg.zsize, m.vg.c_r, m.vg.c_a, m.vg.c_s,
                        ] {
                            o.f(x);
                        }
                        let mut h = 0xcbf29ce484222325u64;
                        for z in 0..m.vg.depth {
                            for y in 0..m.vg.height {
                                for x in 0..m.vg.width {
                                    let f = mri_get_vox_val(g, io.err, &m, x, y, z, 0, 0)?;
                                    h = fnv(h, &f.to_le_bytes());
                                }
                            }
                        }
                        o.s(&format!("{h:016x}"));
                    }
                }
            }
            _ => panic!("dfm op {op}"),
        }
        dump(&s, o);
    }
    Ok(())
}

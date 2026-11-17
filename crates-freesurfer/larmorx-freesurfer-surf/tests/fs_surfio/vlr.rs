// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 41 of track D: `vlr`, `mrisurf_vals.cpp`'s functions of one
//! program each (`valrest.rs`): a surface read by `MRISread` (`d/lh.s0`: vls's open grids,
//! subdivided icosahedra and srd's small meshes), now and then its neighbourhood set to 2 hops,
//! every vertex's rip flag, annotation, curvature and vector-registration values (`VALS_VP`, or
//! none) and every face's rip flag from the case; the program's annotation table set or not (a
//! table file where FreeSurfer looks for its default one, or a colour table set as the table), the
//! surface's colour table, `Gdiag_no`, a measure volume of one voxel per vertex or none; then 1 to
//! 4 operations named by their FreeSurfer functions; `G` where a value setter would read or write
//! outside a vertex's arrays (FS-980); after each the vertices' rip flags, borders, annotations,
//! undefvals, curvatures, `tx`, areas and values, the faces' rip flags, and for
//! `MRISdilateVertexToSum` its mask: the generator and the port's answers, as `fssurfio.cpp`'s.

use super::ann::{ctab_tokens, read_ctab};
use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::mpf::dump_vol;
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_io::annotation;
use larmorx_freesurfer_mri::mri::mri_alloc_sequence;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_surf::{nbhd, readwrite};
use larmorx_freesurfer_surf::valrest::{self, ValsVp};

/// The value setters, by their FreeSurfer names.
const SETTERS: [&str; 5] = [
    "MRISsetCurvaturesToValues",
    "MRISsetCurvaturesToOrigValues",
    "MRISsetOrigValuesToCurvatures",
    "MRISsetOrigValuesToValues",
    "MRISsetValuesToCurvatures",
];

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

/// A table file's text: lines `<index> <name> <r> <g> <b> 0` of the case's colours (indices in
/// order, now and then repeated, large or negative; now and then a line it cannot read, or none).
fn table_text(g: &mut Gen, colours: &[(i64, i64, i64)]) -> Vec<u8> {
    let n = if g.chance(10) { 0 } else { g.int(1, 8) };
    let mut s = String::new();
    for k in 0..n {
        let index = if g.chance(8) {
            g.pick(&[0, 1, 70000, -3, 5])
        } else {
            k
        };
        let (r, gg, b) = if g.chance(6) {
            (g.int(0, 255), g.int(0, 255), g.int(0, 255))
        } else {
            colours[k as usize % colours.len()]
        };
        if g.chance(15) {
            s += "  # no entry here\n";
        } else {
            s += &format!("{index:3}  name{k}  {r:3} {gg:3} {b:3}    0\n");
        }
    }
    if g.chance(5) && s.ends_with('\n') {
        s.pop();
    }
    s.into_bytes()
}

pub(super) fn vlr_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let kind = g.below(10);
    let m = match kind {
        0..=2 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        3 => mesh(g, heavy),
        4 => ico(g, 2, 10.0),
        _ => ico(g, 1, 10.0),
    };
    surface(t, &m);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    ix(t, if kind != 3 && g.chance(4) { 2 } else { 1 });
    let colours: Vec<(i64, i64, i64)> = (0..6)
        .map(|_| (g.int(0, 255), g.int(0, 255), g.int(0, 255)))
        .collect();
    let nvals_all = g.int(0, 3);
    let rips = g.chance(3);
    // vertices without values (FS-981: a setter crashes FreeSurfer) in one case in six
    let nulls = g.chance(6);
    for _ in 0..nv {
        let rip = if rips && g.chance(5) {
            g.pick(&[1, 1, 2, -1])
        } else {
            0
        };
        ix(t, rip);
        let a = if g.chance(8) {
            let r = g.int(1, 0xff_ffff);
            g.pick(&[0, -1, 0xff_ffff, r])
        } else {
            let (r, gg, b) = colours[g.below(colours.len() as u64) as usize];
            r | (gg << 8) | (b << 16)
        };
        ix(t, a);
        fv(t, g.int(-40, 40) as f32 / 8.0);
        let nvals = if nulls && g.chance(5) {
            -1
        } else if heavy && g.chance(10) {
            g.int(0, 3)
        } else {
            nvals_all
        };
        ix(t, nvals);
        for _ in 0..nvals.max(0) {
            fv(t, g.int(-40, 40) as f32 / 8.0);
            fv(t, g.int(-40, 40) as f32 / 8.0);
        }
    }
    for _ in 0..nf {
        ix(t, g.pick(&[0, 0, 0, 1]));
    }
    let tmode = g.pick(&[0, 1, 1, 2]);
    ix(t, tmode);
    match tmode {
        1 => hx(t, &table_text(g, &colours)),
        2 => ctab_tokens(t, g, &colours),
        _ => {}
    }
    ctab_tokens(t, g, &colours);
    ix(
        t,
        if nv > 0 && g.chance(10) {
            g.int(0, nv - 1)
        } else {
            -1
        },
    );
    let vmode = i64::from(g.chance(2));
    ix(t, vmode);
    if vmode > 0 {
        ix(t, g.pick(&[3, 3, 3, 0, 1, 4]));
        let w = if g.chance(10) {
            g.pick(&[nv - 1, nv + 1]).max(1)
        } else {
            nv
        };
        ix(t, w);
        for _ in 0..w {
            fv(
                t,
                if heavy && g.chance(10) {
                    g.pick(&[f32::NAN, -f32::NAN, f32::INFINITY, -2.0])
                } else {
                    g.int(0, 16) as f32 / 4.0
                },
            );
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(10) {
            0..=2 => {
                t.push("MRISmodeFilterAnnotations".into());
                ix(t, g.pick(&[0, 1, 1, 2, 3, 10]));
            }
            3 => {
                t.push(
                    g.pick(&["MRISdilateRipped", "MRISerodeRipped"])
                        .into(),
                );
                ix(t, g.pick(&[0, 1, 1, 2, 3]));
            }
            4 => t.push("MRISsetRipInFacesWithRippedVertices".into()),
            5 | 6 => {
                t.push(g.pick(&SETTERS).into());
                ix(t, if g.chance(8) { g.pick(&[-1, 3, 7]) } else { g.int(0, 2) });
            }
            _ => {
                t.push("MRISdilateVertexToSum".into());
                let vno = if heavy && g.chance(5) {
                    g.pick(&[-1, nv])
                } else {
                    g.int(0, (nv - 1).max(0))
                };
                ix(t, vno);
                let which = if g.chance(15) {
                    2
                } else if vmode > 0 && g.chance(2) {
                    1
                } else {
                    0
                };
                ix(t, which);
                let target = if g.chance(10) {
                    g.pick(&[0.0, -1.0, f64::NAN, f64::INFINITY, 1e9])
                } else if which == 1 {
                    g.pick(&[0.5, 1.0, 2.5, 4.0, 10.0, 30.0])
                } else {
                    g.pick(&[1.0, 10.0, 40.0, 100.0, 300.0, 1000.0])
                };
                dv(t, target);
            }
        }
    }
}

fn dump(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let x = s.vertex(vno);
        for i in [
            i64::from(x.ripflag),
            i64::from(x.border),
            i64::from(x.annotation),
            i64::from(x.undefval),
        ] {
            o.i(i);
        }
        o.f(x.curv);
        o.f(x.tx);
        o.f(x.area);
        match valrest::vals_vp(x) {
            None => o.s("N"),
            Some(m) => {
                let vp = m.lock().expect("the values");
                o.i(i64::from(vp.nvals));
                for k in 0..vp.nvals.max(0) as usize {
                    o.f(vp.vals[k]);
                    o.f(vp.orig_vals[k]);
                }
            }
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        o.i(i64::from(s.face(fno).ripflag));
    }
}

pub(super) fn vlr_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    let nsize = t.i();
    if nsize > 1 {
        nbhd::set_neighborhood_size(g, sg, &mut io, &mut s, nsize)?;
    }
    for vno in 0..nv as usize {
        let ripflag = t.i() as i8;
        let annotation = t.i() as i32;
        let curv = t.f();
        let x = s.vertex_mut(vno);
        x.ripflag = ripflag;
        x.annotation = annotation;
        x.curv = curv;
        let nvals = t.i();
        if nvals >= 0 {
            let mut vp = ValsVp {
                nvals,
                vals: vec![0.0; nvals as usize],
                orig_vals: vec![0.0; nvals as usize],
            };
            for k in 0..nvals as usize {
                vp.vals[k] = t.f();
                vp.orig_vals[k] = t.f();
            }
            valrest::set_vals_vp(s.vertex_mut(vno), vp);
        }
    }
    for fno in 0..nf as usize {
        s.face_mut(fno).ripflag = t.i() as i8;
    }
    match t.i() {
        1 => {
            std::fs::write(dir.join("Simple_surface_labels2009.txt"), t.h()).expect("the table");
        }
        2 => {
            let pre = read_ctab(t);
            o.i(i64::from(annotation::set_atable_from_ctable(
                g,
                io.out,
                io.err,
                &mut sg.atable,
                pre.as_ref(),
            )));
        }
        _ => {}
    }
    s.ct = read_ctab(t);
    g.gdiag_no = t.i();
    let measure = if t.i() > 0 {
        let (ty, w) = (t.i(), t.i());
        let mut m = mri_alloc_sequence(g, io.out, io.err, w, 1, 1, ty, 1)?;
        for c in 0..w {
            let x = t.f();
            mri_set_vox_val(g, io.err, &mut m, c, 0, 0, 0, x, 0)?;
        }
        Some(m)
    } else {
        None
    };
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "MRISmodeFilterAnnotations" => {
                let n = t.i();
                o.i(i64::from(valrest::mode_filter_annotations(
                    g, sg, &mut io, d, &mut s, n,
                )?));
            }
            "MRISdilateRipped" => o.i(i64::from(valrest::dilate_ripped(&mut s, t.i()))),
            "MRISerodeRipped" => o.i(i64::from(valrest::erode_ripped(&mut s, t.i()))),
            "MRISsetRipInFacesWithRippedVertices" => {
                valrest::set_rip_in_faces_with_ripped_vertices(&mut s)
            }
            _ if op.starts_with("MRISset") => {
                let fno = t.i();
                let outside = (0..nv as usize).any(|vno| {
                    let x = s.vertex(vno);
                    x.ripflag == 0
                        && valrest::vals_vp(x).is_some_and(|m| {
                            let n = m.lock().expect("the values").nvals;
                            fno < 0 || fno >= n
                        })
                });
                if outside {
                    o.s("G");
                    continue;
                }
                match op {
                    "MRISsetCurvaturesToValues" => {
                        valrest::set_curvatures_to_values(g, &mut s, fno)?
                    }
                    "MRISsetCurvaturesToOrigValues" => {
                        valrest::set_curvatures_to_orig_values(g, &mut s, fno)?
                    }
                    "MRISsetOrigValuesToCurvatures" => {
                        valrest::set_orig_values_to_curvatures(g, &mut s, fno)?
                    }
                    "MRISsetOrigValuesToValues" => {
                        valrest::set_orig_values_to_values(g, &mut s, fno)?
                    }
                    "MRISsetValuesToCurvatures" => {
                        valrest::set_values_to_curvatures(g, &mut s, fno)?
                    }
                    _ => panic!("vlr setter {op}"),
                }
            }
            "MRISdilateVertexToSum" => {
                let (vno, which) = (t.i(), t.i());
                let target = t.d();
                let m = valrest::dilate_vertex_to_sum(
                    g,
                    sg,
                    &mut io,
                    vno,
                    if which == 2 { None } else { Some(&mut s) },
                    if which == 1 { measure.as_ref() } else { None },
                    target,
                )?;
                dump_vol(g, io.err, m.as_ref(), o);
            }
            _ => panic!("vlr op {op}"),
        }
        dump(&s, o);
    }
    Ok(())
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 40 of track D: `vls`, `mrisurf_vals.cpp`'s marks, settings, rip
//! flags, face tests and filters (`valmarks.rs`, `valfilter.rs`): a surface read by `MRISread`
//! (`d/lh.s0`: open grids, subdivided icosahedra, srd's small meshes), now and then its
//! neighbourhood set to 2 hops (`MRISsetNeighborhoodSize`, grids and icosahedra), 0 to 3 vertices
//! ripped, `Gdiag`'s show and verbose bits or not, the extra gradients or not, every vertex's 19
//! float and 5 integer fields (values of a few eighths, whole numbers for the mode filters,
//! specials now and then; white and pial positions equal, close or apart) and every face's mark
//! from the case, a label of 0 to 6 points (now and then one naming no vertex: `G` for the rips by
//! it, FS-917); then 1 to 6 operations named by their FreeSurfer functions; after each the
//! vertices' fields, the faces' marks and the curvature extremes: the generator and the port's
//! answers, as `fssurfio.cpp`'s. A case with `MRISmodeFilterZeroVals` has no special values: a
//! NaN makes FreeSurfer's filter run for ever, printing a line per pass (FS-973;
//! `tests/vals_defaults.rs` checks the port's end).

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::sfile::list_files;
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::diag::DIAG_HEARTBEAT;
use larmorx_freesurfer_io::label::label_alloc;
use larmorx_freesurfer_surf::valfilter::{self, Field};
use larmorx_freesurfer_surf::{nbhd, readwrite, sampdist, valmarks};

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

/// The operations without arguments.
const PLAIN: [&str; 24] = [
    "MRISinvertMarks",
    "MRISnotMarked",
    "MRISmarkedVertices",
    "MRISexpandMarked",
    "MRISclearMark2s",
    "mrisClearGradient",
    "MRISclearGradient",
    "mrisClearExtraGradient",
    "mrisClearMomentum",
    "MRISnegateValues",
    "MRISsqrtVal",
    "MRISclearFixedValFlags",
    "MRISextractCurvatureDoubleVector",
    "MRISabsVals",
    "MRISabsCurvature",
    "MRISclearD",
    "MRISripLabel",
    "MRISripNotLabel",
    "MRISripMarked",
    "MRISripUnmarked",
    "MRISripZeroThicknessRegions",
    "detectContrast",
    "MRISripNotLabel",
    "MRISinvertMarks",
];

/// The operations with a count (marks, iterations).
const COUNTED: [&str; 26] = [
    "MRIScountMarked",
    "MRISsetAllMarks",
    "MRISerodeMarked",
    "MRIScloseMarked",
    "MRISopenMarked",
    "MRISsetFlags",
    "MRISclearFlags",
    "mrisMarkBadEdgeVertices",
    "MRISminFilterCurvatures",
    "MRISmaxFilterCurvatures",
    "MRISaverageMarkedCurvatures",
    "MRISaverageMarkedVals",
    "MRISaverageVals",
    "MRISaverageD",
    "MRISaverageVal2s",
    "MRISaverageVal2baks",
    "MRISaverageMarkedStats",
    "MRISmedianFilterVals",
    "MRISmedianFilterVal2s",
    "MRISmedianFilterVal2baks",
    "MRISmedianFilterCurvature",
    "MRISmodeFilterVals",
    "MRISsoapBubbleVals",
    "MRISsoapBubbleD",
    "MRISextractCurvatureVectorDouble",
    "MRISaverageVals",
];

/// The operations with a `float`.
const FLOATED: [&str; 10] = [
    "MRISthresholdValIntoMarked",
    "MRISmarkVerticesWithValOverThresh",
    "mrisSetVal",
    "mrisSetValAndClearVal2",
    "MRISsetVals",
    "MRISsetValues",
    "MRISscaleVals",
    "MRISmulVal",
    "MRISsetVal2",
    "MRISthresholdValIntoMarked",
];

/// A field's value: eighths, or (for the mode filters) whole numbers 0 to 4; specials now and
/// then when `special`.
fn fval(g: &mut Gen, whole: bool, special: bool) -> f32 {
    if special && g.chance(10) {
        return g.pick(&[
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            f32::from_bits(0x7fa0_0001),
            f32::from_bits(0xffc0_0123),
            1e10,
            -3e9,
        ]);
    }
    if whole && !g.chance(6) {
        return g.int(0, 4) as f32;
    }
    g.int(-40, 40) as f32 / 8.0
}

fn ival(g: &mut Gen, heavy: bool) -> i64 {
    if heavy && g.chance(8) {
        return g.pick(&[70000, -70000, 32768, 2147483647, -2147483648]);
    }
    g.pick(&[0, 0, 0, 1, 1, 2, -1, 3])
}

pub(super) fn vls_tokens(t: &mut Vec<String>, g: &mut Gen) {
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
    ix(t, if kind != 3 && g.chance(5) { 2 } else { 1 });
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    ix(t, i64::from(g.chance(4)));
    let extra = g.chance(3);
    ix(t, i64::from(extra));
    // a case that may run MRISmodeFilterZeroVals has no special values (FS-973)
    let zero_vals = g.chance(12);
    let special = heavy && !zero_vals;
    let whole = zero_vals || g.chance(4);
    let thick = g.below(3);
    for _ in 0..nv {
        for _ in 0..13 {
            fv(t, fval(g, whole, special));
        }
        let white: Vec<f32> = (0..3).map(|_| g.int(-32, 32) as f32 / 4.0).collect();
        for &w in &white {
            fv(t, w);
        }
        for &w in &white {
            let p = match thick {
                0 => w,
                1 if g.chance(2) => w + g.pick(&[1e-8, -1e-8, 2e-7, 0.0]),
                _ => w + g.int(-8, 8) as f32 / 8.0,
            };
            fv(t, if special && g.chance(20) { f32::NAN } else { p });
        }
        for _ in 0..5 {
            ix(t, ival(g, heavy));
        }
        if extra {
            for _ in 0..3 {
                fv(t, fval(g, false, special));
            }
        }
    }
    for _ in 0..nf {
        ix(t, g.pick(&[0, 0, 1, 2, -1]));
    }
    // the long-range sampling's seed, diagnostic vertex, heartbeat and vertex pair
    ix(t, g.int(1, 999));
    ix(
        t,
        if nv > 0 && g.chance(10) {
            g.int(0, nv - 1)
        } else {
            -1
        },
    );
    ix(t, i64::from(g.chance(8)));
    if nv > 1 && g.chance(8) {
        ix(t, g.int(0, nv - 1));
        ix(t, g.int(0, nv - 1));
    } else {
        ix(t, -1);
        ix(t, -1);
    }
    let npts = g.int(0, 6);
    ix(t, npts);
    for _ in 0..npts {
        let vno = if nv > 0 && !(heavy && g.chance(4)) {
            g.int(0, nv - 1)
        } else {
            g.pick(&[-1, nv, nv + 3])
        };
        ix(t, vno);
    }
    let nops = g.int(1, 6);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(16) {
            0..=3 => {
                let op = g.pick(&COUNTED);
                t.push(op.into());
                let n = match op {
                    "MRIScountMarked" | "MRISsetAllMarks" | "mrisMarkBadEdgeVertices" => {
                        ival(g, heavy)
                    }
                    "MRISsetFlags" | "MRISclearFlags" => g.pick(&[1, 2, 4, 0x10, 0x8000_0000]),
                    "MRISextractCurvatureVectorDouble" => g.pick(&[0, 0, 2]),
                    "MRISmodeFilterVals" => g.int(0, 4),
                    _ => g.pick(&[0, 1, 1, 2, 3]),
                };
                ix(t, n);
            }
            4..=6 => {
                let op = g.pick(&FLOATED);
                t.push(op.into());
                fv(t, fval(g, whole, special));
            }
            7 => {
                t.push("MRISmedianFilterD".into());
                ix(t, g.pick(&[0, 1, 2]));
                ix(t, g.int(0, 1));
            }
            8 => {
                t.push("MRISgaussianFilterD".into());
                dv(
                    t,
                    if special && g.chance(6) {
                        f64::NAN
                    } else {
                        g.pick(&[0.5, 0.25, 1.0, 0.0, 0.75])
                    },
                );
            }
            9 if nf > 0 => {
                t.push(
                    g.pick(&["mrisAnyVertexOfFaceMarked", "triangleMarked"])
                        .into(),
                );
                ix(t, g.int(0, nf - 1));
            }
            10 if nv > 1 => {
                t.push("findNonMarkedFace".into());
                let vno = if heavy && g.chance(10) {
                    g.pick(&[-1, nv])
                } else {
                    g.int(0, nv - 1)
                };
                ix(t, vno);
                let vn1 = if g.chance(30) { vno } else { g.int(0, nv - 1) };
                ix(t, vn1);
            }
            11 if zero_vals => t.push("MRISmodeFilterZeroVals".into()),
            12 => {
                t.push("MRISsampleDistances".into());
                let m = if g.chance(20) { 100 } else { g.int(0, 4) };
                ix(t, m);
                for _ in 0..=m.max(0) {
                    ix(t, g.pick(&[0, 0, 1, 3, 5, 8, 20]));
                }
            }
            13 => {
                t.push("MRISsampleAtEachDistance".into());
                let n = if g.chance(20) {
                    g.pick(&[150, 250])
                } else {
                    g.int(0, 3)
                };
                ix(t, n);
                ix(t, g.pick(&[0, 2, 5, 8]));
            }
            _ => t.push(g.pick(&PLAIN).into()),
        }
    }
}

/// After the long-range sampling: each vertex's neighbours in use, their size and original
/// distances; the surface's neighbourhood size, mean neighbour count and flag.
fn topo_dump(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let t = s.topology(vno);
        let n = t.vtotal.max(0) as usize;
        o.i(i64::from(t.vtotal));
        o.i(i64::from(t.nsizeCur));
        for i in 0..n {
            o.i(i64::from(t.v.as_ref().expect("v")[i]));
        }
        match &s.vertex(vno).dist_orig {
            Some(d) => {
                for &x in &d[..n] {
                    o.f(x);
                }
            }
            None => o.s("N"),
        }
    }
    o.i(i64::from(s.nsize));
    o.f(s.avg_nbrs);
    o.i(i64::from(s.vtotalsMightBeTooBig));
}

fn dump(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let x = s.vertex(vno);
        for f in [
            x.val, x.val2, x.val2bak, x.valbak, x.curv, x.d, x.stat, x.dx, x.dy, x.dz, x.odx,
            x.ody, x.odz, x.tx, x.tdx,
        ] {
            o.f(f);
        }
        for i in [
            i64::from(x.ripflag),
            i64::from(x.marked),
            i64::from(x.marked2),
            i64::from(x.flags),
            i64::from(x.fixedval),
            i64::from(x.annotation),
        ] {
            o.i(i);
        }
        if let (Some(a), Some(b), Some(c)) = (&s.dx2, &s.dy2, &s.dz2) {
            o.f(a[vno]);
            o.f(b[vno]);
            o.f(c[vno]);
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        o.i(i64::from(s.face(fno).marked));
    }
    o.f(s.max_curv);
    o.f(s.min_curv);
}

pub(super) fn vls_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    for _ in 0..t.i() {
        let vno = t.i();
        s.vertex_mut(vno as usize).ripflag = 1;
    }
    if t.i() != 0 {
        g.gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    }
    let extra = t.i() != 0;
    if extra {
        let n = nv.max(0) as usize;
        s.dx2 = Some(vec![0.0; n]);
        s.dy2 = Some(vec![0.0; n]);
        s.dz2 = Some(vec![0.0; n]);
    }
    for vno in 0..nv as usize {
        let x = s.vertex_mut(vno);
        x.val = t.f();
        x.val2 = t.f();
        x.val2bak = t.f();
        x.valbak = t.f();
        x.curv = t.f();
        x.d = t.f();
        x.stat = t.f();
        x.dx = t.f();
        x.dy = t.f();
        x.dz = t.f();
        x.odx = t.f();
        x.ody = t.f();
        x.odz = t.f();
        x.whitex = t.f();
        x.whitey = t.f();
        x.whitez = t.f();
        x.pialx = t.f();
        x.pialy = t.f();
        x.pialz = t.f();
        x.marked = t.i() as i16;
        x.marked2 = t.i() as i16;
        x.annotation = t.i();
        x.flags = t.i() as u32;
        x.fixedval = t.i();
        if extra {
            let (a, b, c) = (t.f(), t.f(), t.f());
            s.dx2.as_mut().expect("dx2")[vno] = a;
            s.dy2.as_mut().expect("dy2")[vno] = b;
            s.dz2.as_mut().expect("dz2")[vno] = c;
        }
    }
    for fno in 0..nf as usize {
        s.face_mut(fno).marked = t.i();
    }
    let seed = t.i();
    g.gdiag_no = t.i();
    if t.i() != 0 {
        g.gdiag |= DIAG_HEARTBEAT;
    }
    let (vd1, vd2) = (t.i(), t.i());
    let mut pairs = vec![
        ("SURFER_SIDEDOOR".to_owned(), "1".to_owned()),
        ("FREESURFER_SEED".to_owned(), seed.to_string()),
    ];
    if vd1 >= 0 {
        pairs.push(("VDIAG1".to_owned(), vd1.to_string()));
    }
    if vd2 >= 0 {
        pairs.push(("VDIAG2".to_owned(), vd2.to_string()));
    }
    let env = Environment::from_pairs(pairs);
    let mut io = coords::Io {
        out: &mut so,
        err: &mut se,
        env: &env,
    };
    let npts = t.i();
    let mut lab = label_alloc(g, io.env, io.err, npts.max(1), None, None)?;
    lab.n_points = npts;
    let mut outside = false;
    for i in 0..npts as usize {
        let vno = t.i();
        lab.lv[i].vno = vno;
        outside |= vno < 0 || vno >= nv;
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let r: Option<i32> = match op {
            "MRISinvertMarks" => Some(valmarks::invert_marks(&mut s)),
            "MRISnotMarked" => Some(valmarks::not_marked(&mut s)),
            "MRIScountMarked" => Some(valmarks::count_marked(&s, t.i())),
            "MRISmarkedVertices" => Some(valmarks::marked_vertices(&s)),
            "MRISsetAllMarks" => Some(valmarks::set_all_marks(&mut s, t.i())),
            "MRISthresholdValIntoMarked" => {
                Some(valmarks::threshold_val_into_marked(&mut s, t.f()))
            }
            "MRISmarkVerticesWithValOverThresh" => {
                Some(valmarks::mark_vertices_with_val_over_thresh(&mut s, t.f()))
            }
            "MRISexpandMarked" => Some(valmarks::expand_marked(&mut s)),
            "MRISerodeMarked" => Some(valmarks::erode_marked(&mut s, t.i())),
            "MRIScloseMarked" => Some(valmarks::close_marked(&mut s, t.i())),
            "MRISopenMarked" => Some(valmarks::open_marked(&mut s, t.i())),
            "MRISclearMark2s" => Some(valmarks::clear_mark2s(&mut s)),
            "MRISsetFlags" => Some(valmarks::set_flags(&mut s, t.i())),
            "MRISclearFlags" => Some(valmarks::clear_flags(&mut s, t.i())),
            "mrisSetVal" => {
                valmarks::set_val_all(&mut s, t.f());
                None
            }
            "mrisSetValAndClearVal2" => {
                valmarks::set_val_and_clear_val2(&mut s, t.f());
                None
            }
            "mrisClearGradient" | "MRISclearGradient" => Some(valmarks::clear_gradient(&mut s)),
            "mrisClearExtraGradient" => Some(valmarks::clear_extra_gradient(&mut s)),
            "mrisClearMomentum" => Some(valmarks::clear_momentum(&mut s)),
            "MRISnegateValues" => Some(valmarks::negate_values(&mut s)),
            "MRISsetVals" | "MRISsetValues" => Some(valmarks::set_vals(&mut s, t.f())),
            "MRISscaleVals" | "MRISmulVal" => Some(valmarks::scale_vals(&mut s, t.f())),
            "MRISsqrtVal" => Some(valmarks::sqrt_val(&mut s)),
            "MRISsetVal2" => Some(valmarks::set_val2(&mut s, t.f())),
            "MRISclearFixedValFlags" => Some(valmarks::clear_fixed_val_flags(&mut s)),
            "MRISextractCurvatureVectorDouble" => {
                let off = t.i() as usize;
                let mut a = vec![-3.0f64; off + nv as usize];
                o.i(i64::from(valmarks::extract_curvature_vector_double(
                    &s, &mut a, off,
                )));
                for x in a {
                    o.d(x);
                }
                None
            }
            "MRISextractCurvatureDoubleVector" => {
                o.i(0);
                for x in valmarks::extract_curvature_double_vector(&s) {
                    o.d(x);
                }
                None
            }
            "MRISabsVals" => Some(valmarks::abs_vals(&mut s)),
            "MRISabsCurvature" => Some(valmarks::abs_curvature(&mut s)),
            "MRISclearD" => {
                valmarks::clear_d(&mut s);
                None
            }
            "MRISripLabel" | "MRISripNotLabel" => {
                if outside {
                    o.s("G");
                    continue;
                }
                Some(if op == "MRISripLabel" {
                    valmarks::rip_label(g, &mut s, &lab)
                } else {
                    valmarks::rip_not_label(g, io.out, &mut s, &lab)
                })
            }
            "MRISripMarked" => Some(valmarks::rip_marked(&mut s)),
            "MRISripUnmarked" => Some(valmarks::rip_unmarked(&mut s)),
            "MRISripZeroThicknessRegions" => Some(valmarks::rip_zero_thickness_regions(&mut s)),
            "mrisMarkBadEdgeVertices" => Some(valmarks::mark_bad_edge_vertices(&mut s, t.i())),
            "mrisAnyVertexOfFaceMarked" => Some(i32::from(valmarks::any_vertex_of_face_marked(
                &s,
                t.i() as usize,
            ))),
            "triangleMarked" => Some(i32::from(valmarks::triangle_marked(&s, t.i() as usize))),
            "findNonMarkedFace" => {
                let (a, b) = (t.i(), t.i());
                Some(valmarks::find_non_marked_face(g, io.out, io.err, &s, a, b)?)
            }
            "detectContrast" => Some(valmarks::detect_contrast(io.err, &s)),
            "MRISminFilterCurvatures" => {
                Some(valfilter::extreme_filter_curvatures(&mut s, t.i(), false))
            }
            "MRISmaxFilterCurvatures" => {
                Some(valfilter::extreme_filter_curvatures(&mut s, t.i(), true))
            }
            "MRISaverageMarkedCurvatures" => {
                Some(valfilter::average_marked_curvatures(&mut s, t.i()))
            }
            "MRISaverageMarkedVals" => Some(valfilter::average_marked_vals(&mut s, t.i())),
            "MRISaverageVals" => Some(valfilter::average_vals(&mut s, t.i())),
            "MRISaverageD" => Some(valfilter::average_d(&mut s, t.i())),
            "MRISaverageVal2s" => Some(valfilter::average_val2s(&mut s, t.i())),
            "MRISaverageVal2baks" => Some(valfilter::average_val2baks(&mut s, t.i())),
            "MRISaverageMarkedStats" => Some(valfilter::average_marked_stats(&mut s, t.i())),
            "MRISmedianFilterVals"
            | "MRISmedianFilterVal2s"
            | "MRISmedianFilterVal2baks"
            | "MRISmedianFilterCurvature" => {
                let f = match op {
                    "MRISmedianFilterVals" => Field::Val,
                    "MRISmedianFilterVal2s" => Field::Val2,
                    "MRISmedianFilterVal2baks" => Field::Val2bak,
                    _ => Field::Curv,
                };
                valfilter::median_filter(g, &mut s, t.i(), f, false);
                Some(0)
            }
            "MRISmedianFilterD" => {
                let (n, vt) = (t.i(), t.i());
                Some(valfilter::median_filter_d(g, &mut s, n, vt))
            }
            "MRISgaussianFilterD" => Some(valfilter::gaussian_filter_d(&mut s, t.d())),
            "MRISmodeFilterVals" => Some(valfilter::mode_filter_vals(g, &mut io, &mut s, t.i())?),
            "MRISmodeFilterZeroVals" => Some(valfilter::mode_filter_zero_vals(g, &mut io, &mut s)?),
            "MRISsoapBubbleVals" => Some(valfilter::soap_bubble_vals(g, io.out, &mut s, t.i())),
            "MRISsoapBubbleD" => Some(valfilter::soap_bubble_d(g, io.out, &mut s, t.i())),
            "MRISsampleDistances" => {
                let m = t.i();
                let nbrs: Vec<i32> = (0..(m + 1).max(1)).map(|_| t.i()).collect();
                o.i(i64::from(sampdist::sample_distances(
                    g, &mut io, d, &mut s, &nbrs, m,
                )?));
                topo_dump(&s, o);
                None
            }
            "MRISsampleAtEachDistance" => {
                let (a, b) = (t.i(), t.i());
                o.i(i64::from(sampdist::sample_at_each_distance(
                    g, &mut io, d, &mut s, a, b,
                )?));
                topo_dump(&s, o);
                None
            }
            _ => panic!("vls op {op}"),
        };
        if let Some(r) = r {
            o.i(i64::from(r));
        }
        dump(&s, o);
    }
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        if n == "d/lh.s0" {
            continue;
        }
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
                let mut x = String::from("x");
                for c in b {
                    x += &format!("{c:02x}");
                }
                o.s(&x);
            }
            Err(_) => o.s("NOFILE"),
        }
    }
    Ok(())
}

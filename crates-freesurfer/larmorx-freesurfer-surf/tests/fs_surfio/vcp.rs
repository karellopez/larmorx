// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 38 of track D: `vcp`, the copies between a surface's vertex
//! fields and its vectors (`valcopy.rs`) and the fields written through the curvature
//! (`curvfile.rs`'s `MRISwriteD`, `MRISwriteMarked`, `MRISwriteArea`, `MRISreadMarked`): a surface
//! read by `MRISread` (`d/lh.s0`), 0 to 3 vertices ripped, every vertex's 13 float and 6 integer
//! fields and every face's mark from the case; then 1 to 6 operations named by their FreeSurfer
//! functions (vectors and matrices of the vertex count or not: `G` where FreeSurfer reads outside
//! them, FS-908; the writers to new-format, text, VTK and MGH names, in a folder or one that does
//! not exist; the reader from what was written or a missing name); after each `errno`, the fields
//! and the face marks; then every file in the folder: the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::sfile::list_files;
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_surf::{curvfile, readwrite, valcopy};

/// The operations without arguments, by their FreeSurfer names.
const PLAIN: [&str; 36] = [
    "MRISsetCroppedToZero",
    "MRISexportValVector",
    "MRIScopyMarkedToMarked2",
    "MRIScopyMarked2ToMarked",
    "MRIScopyMarkedToMarked3",
    "MRIScopyMarked3ToMarked",
    "MRIScopyValsToAnnotations",
    "MRIScopyMeansToValues",
    "MRIScopyImaginaryMeansToValues",
    "MRIScopyStandardErrorsToValues",
    "MRIScopyStatsToValues",
    "MRIScopyStatsFromValues",
    "MRIScopyValuesToImagValues",
    "MRIScopyImagValuesToValues",
    "MRIScopyVal2ToVal",
    "MRIScopyCurvatureToValues",
    "MRIScopyVal2BakToVal",
    "MRIScopyCurvatureToImagValues",
    "MRIScopyValuesToCurvature",
    "MRIScopyImagValuesToCurvature",
    "MRIScopyCurvatureFromValues",
    "MRIScopyCurvatureFromImagValues",
    "MRIScopyValToVal2",
    "MRIScopyValuesToVal2Bak",
    "MRIScopyValToValBak",
    "MRIScopyValToVal2Bak",
    "MRISorigAreaToCurv",
    "MRISareaToCurv",
    "MRISmarkedToCurv",
    "MRIScurvToMarked",
    "MRISdToCurv",
    "MRIScurvToD",
    "MRIScopyFixedValFlagsToMarks",
    "MRISclearFaceMarks",
    "MRIScopyMarkedToMarked2",
    "MRISexportValVector",
];

const NAMES: [&str; 7] = ["c", "c.asc", "c.vtk", "c.mgh", "d/c", "nodir/c", "c"];

fn fval(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(10) {
        return g.pick(&[
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            f32::from_bits(0x7fa0_0001),
            1e10,
            -3e9,
        ]);
    }
    g.int(-40, 40) as f32 / 8.0
}

fn ival(g: &mut Gen, heavy: bool) -> i64 {
    if heavy && g.chance(8) {
        return g.pick(&[70000, -70000, 32768, 2147483647, -2147483648]);
    }
    g.int(-3, 5)
}

pub(super) fn vcp_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0..=2 => {
            let (w, h) = (g.int(2, 4), g.int(2, 4));
            grid(g, w, h)
        }
        3 => mesh(g, heavy),
        _ => ico(g, 1, 10.0),
    };
    surface(t, &m);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    for _ in 0..nv {
        for _ in 0..13 {
            fv(t, fval(g, heavy));
        }
        for _ in 0..6 {
            ix(t, ival(g, heavy));
        }
    }
    for _ in 0..nf {
        ix(t, g.pick(&[0, 0, 1, 2, -1]));
    }
    let nops = g.int(1, 6);
    ix(t, nops);
    let mut written: Vec<&str> = Vec::new();
    for _ in 0..nops {
        match g.below(20) {
            0 => {
                t.push("MRISaddToValues".into());
                fv(t, fval(g, heavy));
            }
            1 => {
                t.push(
                    g.pick(&["MRISimportCurvatureVector", "MRISimportValVector"])
                        .into(),
                );
                let n = g.pick(&[nv, nv, nv, nv + 2, (nv - 1).max(0)]);
                ix(t, n);
                for _ in 0..n {
                    fv(t, fval(g, heavy));
                }
            }
            2 => {
                t.push("MRISimportValFromMatrixColumn".into());
                let rows = g.pick(&[nv, nv, nv + 1, (nv - 1).max(1)]).max(1);
                let cols = g.int(1, 3);
                let col = if g.chance(6) {
                    g.pick(&[0, cols + 1])
                } else {
                    g.int(1, cols)
                };
                ix(t, rows);
                ix(t, cols);
                ix(t, col);
                for _ in 0..rows * cols {
                    fv(t, fval(g, heavy));
                }
            }
            3 => {
                t.push("MRISexportValVectorDouble".into());
                ix(t, g.pick(&[0, 0, 2]));
            }
            4 => {
                t.push(g.pick(&["MRIScopyFromCropped", "MRIScopyToCropped"]).into());
                ix(t, g.pick(&[1, 11, 1, 11, 3]));
            }
            5..=7 => {
                let w = g.pick(&["MRISwriteD", "MRISwriteMarked", "MRISwriteArea"]);
                t.push(w.into());
                let n = g.pick(&NAMES);
                t.push(n.into());
                written.push(n);
            }
            8 => {
                t.push("MRISreadMarked".into());
                let n = if !written.is_empty() && !g.chance(4) {
                    g.pick(&written)
                } else {
                    g.pick(&["none", "c", "d/c"])
                };
                t.push(n.into());
            }
            _ => t.push(g.pick(&PLAIN).into()),
        }
    }
}

fn dump(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let x = s.vertex(vno);
        for f in [
            x.val,
            x.curv,
            x.stat,
            x.mean,
            x.mean_imag,
            x.std_error,
            x.imag_val,
            x.val2,
            x.val2bak,
            x.valbak,
            x.d,
            x.area,
            x.origarea,
        ] {
            o.f(f);
        }
        for i in [
            i64::from(x.marked),
            i64::from(x.marked2),
            i64::from(x.marked3),
            i64::from(x.cropped),
            i64::from(x.fixedval),
            i64::from(x.annotation),
        ] {
            o.i(i);
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        o.i(i64::from(s.face(fno).marked));
    }
}

pub(super) fn vcp_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    for _ in 0..t.i() {
        let vno = t.i();
        s.vertex_mut(vno as usize).ripflag = 1;
    }
    for vno in 0..nv as usize {
        let x = s.vertex_mut(vno);
        x.val = t.f();
        x.curv = t.f();
        x.stat = t.f();
        x.mean = t.f();
        x.mean_imag = t.f();
        x.std_error = t.f();
        x.imag_val = t.f();
        x.val2 = t.f();
        x.val2bak = t.f();
        x.valbak = t.f();
        x.d = t.f();
        x.area = t.f();
        x.origarea = t.f();
        x.marked = t.i() as i16;
        x.marked2 = t.i() as i16;
        x.marked3 = t.i() as i16;
        x.cropped = t.i();
        x.fixedval = t.i();
        x.annotation = t.i();
    }
    for fno in 0..nf as usize {
        s.face_mut(fno).marked = t.i() as _;
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let r: i32 = match op {
            "MRISsetCroppedToZero" => valcopy::set_cropped_to_zero(&mut s),
            "MRISaddToValues" => valcopy::add_to_values(&mut s, t.f()),
            "MRISimportCurvatureVector" | "MRISimportValVector" => {
                let n = t.i();
                let a: Vec<f32> = (0..n).map(|_| t.f()).collect();
                if n < nv {
                    o.s("G");
                    continue;
                }
                if op == "MRISimportValVector" {
                    valcopy::import_val_vector(g, &mut s, &a)
                } else {
                    valcopy::import_curvature_vector(g, &mut s, &a)
                }
            }
            "MRISimportValFromMatrixColumn" => {
                let (rows, cols, col) = (t.i(), t.i(), t.i());
                let mut m =
                    matrix_alloc(g, io.out, io.err, rows, cols, MATRIX_REAL)?.expect("a matrix");
                for r in 1..=rows {
                    for c in 1..=cols {
                        m.set_relt(r, c, t.f());
                    }
                }
                if rows < nv || col < 1 || col > cols {
                    o.s("G");
                    continue;
                }
                valcopy::import_val_from_matrix_column(g, &mut s, &m, col)
            }
            "MRISexportValVector" => {
                let a = valcopy::export_val_vector(&s);
                o.i(0);
                for x in a {
                    o.f(x);
                }
                o.i(i64::from(g.errno));
                dump(&s, o);
                continue;
            }
            "MRISexportValVectorDouble" => {
                let off = t.i() as usize;
                let mut a = vec![-3.0f64; off + nv as usize];
                o.i(i64::from(valcopy::export_val_vector_double(
                    &s, &mut a, off,
                )));
                for x in a {
                    o.d(x);
                }
                o.i(i64::from(g.errno));
                dump(&s, o);
                continue;
            }
            "MRIScopyMarkedToMarked2" => valcopy::copy_marked_to_marked2(&mut s),
            "MRIScopyMarked2ToMarked" => valcopy::copy_marked2_to_marked(&mut s),
            "MRIScopyMarkedToMarked3" => valcopy::copy_marked_to_marked3(&mut s),
            "MRIScopyMarked3ToMarked" => valcopy::copy_marked3_to_marked(&mut s),
            "MRIScopyValsToAnnotations" => valcopy::copy_vals_to_annotations(&mut s),
            "MRIScopyMeansToValues" => valcopy::copy_means_to_values(&mut s),
            "MRIScopyImaginaryMeansToValues" => valcopy::copy_imaginary_means_to_values(&mut s),
            "MRIScopyStandardErrorsToValues" => valcopy::copy_standard_errors_to_values(&mut s),
            "MRIScopyStatsToValues" => valcopy::copy_stats_to_values(&mut s),
            "MRIScopyStatsFromValues" => valcopy::copy_stats_from_values(&mut s),
            "MRIScopyValuesToImagValues" => valcopy::copy_values_to_imag_values(&mut s),
            "MRIScopyImagValuesToValues" => valcopy::copy_imag_values_to_values(&mut s),
            "MRIScopyVal2ToVal" => valcopy::copy_val2_to_val(&mut s),
            "MRIScopyCurvatureToValues" => valcopy::copy_curvature_to_values(&mut s),
            "MRIScopyVal2BakToVal" => valcopy::copy_val2bak_to_val(&mut s),
            "MRIScopyCurvatureToImagValues" => valcopy::copy_curvature_to_imag_values(&mut s),
            "MRIScopyValuesToCurvature" | "MRIScopyCurvatureFromValues" => {
                valcopy::copy_values_to_curvature(&mut s)
            }
            "MRIScopyImagValuesToCurvature" | "MRIScopyCurvatureFromImagValues" => {
                valcopy::copy_imag_values_to_curvature(&mut s)
            }
            "MRIScopyValToVal2" => valcopy::copy_val_to_val2(&mut s),
            "MRIScopyValuesToVal2Bak" | "MRIScopyValToVal2Bak" => {
                valcopy::copy_values_to_val2bak(&mut s)
            }
            "MRIScopyValToValBak" => valcopy::copy_val_to_valbak(&mut s),
            "MRIScopyFromCropped" => valcopy::copy_cropped(g, &mut io, &mut s, t.i(), false)?,
            "MRIScopyToCropped" => valcopy::copy_cropped(g, &mut io, &mut s, t.i(), true)?,
            "MRISorigAreaToCurv" => valcopy::orig_area_to_curv(&mut s),
            "MRISareaToCurv" => valcopy::area_to_curv(&mut s),
            "MRISmarkedToCurv" => valcopy::marked_to_curv(&mut s),
            "MRIScurvToMarked" => valcopy::curv_to_marked(&mut s),
            "MRISdToCurv" => valcopy::d_to_curv(&mut s),
            "MRIScurvToD" => valcopy::curv_to_d(&mut s),
            "MRIScopyFixedValFlagsToMarks" => valcopy::copy_fixed_val_flags_to_marks(&mut s),
            "MRISclearFaceMarks" => valcopy::clear_face_marks(&mut s),
            "MRISwriteD" => curvfile::write_d(g, &mut io, d, &mut s, t.next())?,
            "MRISwriteMarked" => curvfile::write_marked(g, &mut io, d, &mut s, t.next())?,
            "MRISwriteArea" => curvfile::write_area(g, &mut io, d, &mut s, t.next())?,
            "MRISreadMarked" => curvfile::read_marked(g, sg, &mut io, d, &mut s, t.next())?,
            _ => panic!("vcp op {op}"),
        };
        o.i(i64::from(r));
        o.i(i64::from(g.errno));
        dump(&s, o);
    }
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
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

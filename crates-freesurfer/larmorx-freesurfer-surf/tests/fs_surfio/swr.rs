// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 15: `swr`, `MRISwrite` in every format its name chooses (GIFTI
//! excepted, with gifti.cpp's surface part), `MRISwriteVertexLocations`, `MRISwritePatchAscii`
//! and `MRISreadVerticesOnly`, in an empty work folder (with a subfolder `d`): Gdiag's bits, the
//! environment (a mask: `TRIANGULARSURFACE_NOEXTRA_WRITE`, `USER`; the creation time's
//! replacement always set), the case's input files, a surface from `MRISalloc` (per vertex the
//! current, white and pial coordinates, rip flag and border; per face its vertices and rip flag;
//! its type, `useRealRAS`, volume geometry and name), 1 to 3 operations, the surface's current
//! coordinates, then every file in the folder: the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::sfile::{list_files, utf8};
use super::tri::{coord, faces, printable, quad_file, triangle_file, vg_tokens, vg_values};
use super::*;

use larmorx_freesurfer_surf::{alloc, readwrite};

const OPS: [&str; 6] = ["w", "w", "w", "wl", "wp", "rv"];

const EXTS: [&str; 18] = [
    "", "", ".asc", ".ASC", ".vtk", ".geo", ".tri", ".ico", ".stl", ".STL", "@x", ".annot",
    ".label", ".MGH", ".mgz", ".Vtk", ".asc@x", ".x",
];

/// A file name: a place (the work folder, `d`, a folder that does not exist) and an extension.
fn name(g: &mut Gen, k: usize) -> String {
    let base = match g.below(5) {
        0 => format!("d/lh.s{k}"),
        1 => format!("s{k}"),
        2 => format!("nodir/s{k}"),
        _ => format!("rh.s{k}"),
    };
    format!("{base}{}", g.pick(&EXTS))
}

/// A quadrangle file in one of its three forms (or a triangle file), for `MRISreadVerticesOnly`.
fn vertices_file(g: &mut Gen, heavy: bool) -> Vec<u8> {
    if g.chance(5) {
        return triangle_file(g, false, heavy).0;
    }
    let mut b = quad_file(g, heavy).0;
    // the old format: no magic number, the counts first
    if g.chance(4) && b.len() >= 3 {
        b.drain(..3);
    }
    b
}

pub(super) fn swr_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 3));
    let mut env = 0;
    if g.chance(4) {
        env |= 1;
    }
    if g.chance(3) {
        env |= 2;
    }
    ix(t, env);
    hx(t, &printable(g, 1, 12));
    let date: &[u8] = g.pick(&[b"Thu Nov 12 20:55:19 2026".as_slice(), b"", b"x y"]);
    hx(t, date);
    let nops = g.int(1, 3) as usize;
    let mut ops: Vec<(String, String)> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    for k in 0..nops {
        let op = g.pick(&OPS);
        let n = if op == "rv" {
            if !written.is_empty() && g.chance(2) {
                written[g.below(written.len() as u64) as usize].clone()
            } else {
                let n = name(g, k);
                if !n.starts_with("nodir/") && !g.chance(5) {
                    inputs.push((n.clone(), vertices_file(g, heavy)));
                }
                n
            }
        } else {
            let n = name(g, k);
            // the readable answers only: an STL, VTK or patch file read as a quadrangle file
            // gives millions of vertices from its text
            if op != "wp"
                && ![".stl", ".STL", ".vtk", ".Vtk"]
                    .iter()
                    .any(|e| n.ends_with(e))
            {
                written.push(n.clone());
            }
            // the other form of a name, there before the write (FS_GII's check)
            if g.chance(10) && !n.starts_with("nodir/") {
                inputs.push((format!("{n}.gii"), Vec::new()));
            }
            n
        };
        ops.push((op.to_owned(), n));
    }
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        hx(t, n.as_bytes());
        hx(t, b);
    }
    let nv = if g.chance(10) { 0 } else { g.int(1, 6) };
    let nf = if nv == 0 { 0 } else { g.int(0, 6) };
    ix(t, nv);
    ix(t, nf);
    for _ in 0..nv {
        for _ in 0..9 {
            t.push(format!("{:08x}", coord(g, heavy).to_bits()));
        }
        ix(t, i64::from(g.chance(5)));
        ix(t, i64::from(g.chance(4)));
    }
    for f in faces(g, nv, nf) {
        for v in f {
            ix(t, v);
        }
        ix(t, i64::from(g.chance(6)));
    }
    ix(t, g.pick(&[0, 0, 3]));
    ix(t, g.int(0, 1));
    vg_tokens(t, &vg_values(g));
    let sf: &str = g.pick(&["d/lh.white", "rh.pial", "", "lh.x"]);
    hx(t, sf.as_bytes());
    ix(t, ops.len() as i64);
    for (op, n) in ops {
        t.push(op.clone());
        hx(t, n.as_bytes());
        if op == "wl" {
            ix(t, g.pick(&[4, 9, 7, 2, 0]));
        }
    }
}

pub(super) fn swr_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
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
    let mask = t.i();
    let user = utf8(t.h());
    let date = utf8(t.h());
    let mut pairs: Vec<(String, String)> = Vec::new();
    if mask & 1 != 0 {
        pairs.push(("TRIANGULARSURFACE_NOEXTRA_WRITE".into(), "1".into()));
    }
    if mask & 2 != 0 {
        pairs.push(("USER".into(), user));
    }
    pairs.push((
        "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".into(),
        date,
    ));
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let (nv, nf) = (t.i(), t.i());
    let mut m = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, nv, nf)?;
    for vno in 0..nv as usize {
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z) = (t.f(), t.f(), t.f());
        (v.whitex, v.whitey, v.whitez) = (t.f(), t.f(), t.f());
        (v.pialx, v.pialy, v.pialz) = (t.f(), t.f(), t.f());
        v.ripflag = t.i() as i8;
        v.border = t.i() as i8;
    }
    for fno in 0..nf as usize {
        let f = m.face_mut(fno);
        f.v = [t.i(), t.i(), t.i()];
        f.ripflag = t.i() as i8;
    }
    m.r#type = t.i();
    m.useRealRAS = t.i();
    let vg = &mut m.vg;
    vg.valid = t.i();
    vg.width = t.i();
    vg.height = t.i();
    vg.depth = t.i();
    (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
    vg.ras_good_flag = t.i();
    (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
    (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
    (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
    (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    (vg.s_r, vg.s_a, vg.s_s) = (t.f(), t.f(), t.f());
    vg.fname.strcpy_bytes(&t.h());
    m.fname = utf8(t.h());
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        let n = utf8(t.h());
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        match op {
            "w" => o.i(i64::from(readwrite::write(
                g, &mut cx.sg, &mut io, d, &mut m, &n,
            )?)),
            "wl" => {
                let which = t.i();
                let r = readwrite::write_vertex_locations(
                    g, &mut cx.sg, &mut io, d, &mut m, &n, which,
                )?;
                o.i(i64::from(r));
            }
            "wp" => o.i(i64::from(readwrite::write_patch_ascii(
                g, &mut io, d, &m, &n,
            ))),
            "rv" => match readwrite::read_vertices_only(g, &mut io, d, &n)? {
                None => o.0 += "NULL ",
                Some(s) => {
                    o.i(i64::from(s.nvertices));
                    for v in &s.vertices {
                        o.f(v.x);
                        o.f(v.y);
                        o.f(v.z);
                    }
                }
            },
            _ => panic!("swr op {op}"),
        }
    }
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex(vno);
        o.f(v.x);
        o.f(v.y);
        o.f(v.z);
    }
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
                let mut s = String::from("x");
                for c in b {
                    s += &format!("{c:02x}");
                }
                o.s(&s);
            }
            Err(_) => o.s("NOFILE"),
        }
    }
    Ok(())
}

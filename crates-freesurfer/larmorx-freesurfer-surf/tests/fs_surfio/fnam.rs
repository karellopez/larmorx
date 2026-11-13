// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 15: `fnam`, the surface library's file names (`MRISfileNameType`
//! and the curvature and annotation name builders, the annotation reader's with and without a
//! surface) in an empty work folder with the folders `d`, `d/s` and `label`: the environment (a
//! mask: `FS_POSIX`, `FS_GII` `.gii`, `FS_GII` `x`), the case's input files, the surface's
//! hemisphere and name, 1 to 4 operations, each with its answer and the name it chose: the
//! generator and the port's answers, as `fssurfio.cpp`'s.

use super::sfile::utf8;
use super::*;

use larmorx_freesurfer_surf::{alloc, names};

const OPS: [&str; 6] = ["t", "wc", "rc", "wa", "ra", "rn"];

const FOLDERS: [&str; 8] = ["", "", "", "d/", "label/", "nodir/", "./", "d/s/../"];
const PREFIXES: [&str; 14] = [
    "", "", "lh.", "rh.", "lh.", "LH.", "Lh", "both.", "xlh.", ".", "h.", "lh", "rh", "unknown.",
];
const BASES: [&str; 8] = [
    "curv",
    "thickness",
    "aparc",
    "sulc",
    "white",
    "x",
    "",
    "a.b",
];
const EXTS: [&str; 30] = [
    "",
    "",
    "",
    ".annot",
    ".mgz",
    ".mgh",
    ".nii",
    ".nii.gz",
    ".gii",
    ".asc",
    ".vtk",
    ".ASC",
    ".label",
    ".MGH",
    ".tri",
    ".ico",
    ".geo",
    ".stl",
    ".img",
    ".bhdr",
    "_000.bfloat",
    "@ASC",
    "#3",
    "mgz",
    ".GII",
    ".Vtk",
    "@x@vtk",
    ".annot.mgz",
    "_000.bshort",
    ".mgh.gz",
];

/// The files a case may hold (the name and its kind: 0 empty, 1 a new-format curvature's magic
/// number, 2 text).
const FILES: [(&str, u8); 16] = [
    ("d/lh.thickness", 1),
    ("d/rh.curv", 1),
    ("d/lh.curv", 2),
    ("d/lh.curv.gii", 0),
    ("d/rh.sulc.gii", 0),
    ("d/lh.x", 0),
    ("d/lh.x.gii", 0),
    ("label/lh.aparc.annot", 2),
    ("label/rh.aparc.annot", 0),
    ("label/lh.aparc", 0),
    ("label/lh.aparc.mgz", 0),
    ("label/lh.lh.aparc.annot", 0),
    ("lh.aparc.annot", 0),
    ("aparc", 0),
    ("lh.aparc.annot.gii", 0),
    ("label/lh.aparc.annot.gii", 0),
];

/// A name: a folder, a prefix, a base and an extension; now and then over 1,000 bytes.
fn name(g: &mut Gen) -> String {
    if g.chance(30) {
        let n = g.int(990, 1060) as usize;
        let folder = g.pick(&["", "d/", "d/s/"]);
        let ext = g.pick(&EXTS);
        let long: String = (0..n).map(|k| (b'a' + (k % 26) as u8) as char).collect();
        return if g.chance(2) {
            format!("{folder}{long}{ext}")
        } else {
            format!("{folder}lh.{long}{ext}")
        };
    }
    if g.chance(8) {
        let f = g.pick(&FILES).0;
        return f.rsplit('/').next().unwrap_or(f).to_owned();
    }
    format!(
        "{}{}{}{}",
        g.pick(&FOLDERS),
        g.pick(&PREFIXES),
        g.pick(&BASES),
        g.pick(&EXTS)
    )
}

pub(super) fn fnam_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let mut env = if g.chance(4) { 1 } else { 0 };
    if g.chance(4) {
        env |= 2;
    } else if g.chance(8) {
        env |= 4;
    }
    ix(t, env);
    let nin = g.int(0, 5) as usize;
    let mut chosen: Vec<usize> = Vec::new();
    for _ in 0..nin {
        let k = g.below(FILES.len() as u64) as usize;
        if !chosen.contains(&k) {
            chosen.push(k);
        }
    }
    ix(t, chosen.len() as i64);
    for k in chosen {
        let (n, kind) = FILES[k];
        hx(t, n.as_bytes());
        let mut b: Vec<u8> = match kind {
            1 => vec![0xff, 0xff, 0xff],
            2 => b"1 2\n".to_vec(),
            _ => Vec::new(),
        };
        if kind == 1 {
            for _ in 0..g.int(0, 12) {
                b.push(g.int(0, 255) as u8);
            }
        }
        hx(t, &b);
    }
    ix(t, g.pick(&[0, 0, 1, 1, 2, 3, -1, 7]));
    let sf: &str = g.pick(&[
        "d/lh.white",
        "d/rh.white",
        "lh.white",
        "d/s/lh.pial",
        "",
        "d/",
        "/x",
        "d/s/../rh.sphere",
    ]);
    hx(t, sf.as_bytes());
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        t.push(g.pick(&OPS).to_owned());
        hx(t, name(g).as_bytes());
    }
}

pub(super) fn fnam_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    for d in ["d", "d/s", "label"] {
        std::fs::create_dir(dir.join(d)).expect("a folder");
    }
    cx.g.progname = "fssurfio".into();
    let mask = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if mask & 1 != 0 {
        pairs.push(("FS_POSIX".into(), "1".into()));
    }
    if mask & 4 != 0 {
        pairs.push(("FS_GII".into(), "x".into()));
    } else if mask & 2 != 0 {
        pairs.push(("FS_GII".into(), ".gii".into()));
    }
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let mut m = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, 0, 0)?;
    m.hemisphere = t.i();
    m.fname = utf8(t.h());
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        let name = utf8(t.h());
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        let d = Some(dir);
        let (r, chosen) = match op {
            "t" => {
                o.i(i64::from(names::mris_file_name_type(g, &name)));
                continue;
            }
            "wc" => names::write_curvature_getfilename(g, &mut io, d, &m, &name)?,
            "rc" => names::read_curvature_file_getfilename(g, &mut io, d, &m, &name)?,
            "wa" => names::write_annotation_getfilename(g, &mut io, d, &m, &name)?,
            "ra" => names::read_annotation_getfilename(g, &mut io, d, Some(&m), &name)?,
            "rn" => names::read_annotation_getfilename(g, &mut io, d, None, &name)?,
            _ => panic!("fnam op"),
        };
        o.i(i64::from(r));
        o.h(Some(chosen.as_bytes()));
    }
    Ok(())
}

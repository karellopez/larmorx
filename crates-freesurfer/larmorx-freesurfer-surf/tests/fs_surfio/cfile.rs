// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue item 3.1-48 part a (relay 12): `cfile`, the surface library's
//! curvature, value and format files in an empty work folder (with a subfolder `d`): the case's
//! input files, a surface from `MRISalloc` with its vertices and faces, 1 to 3 operations on
//! names relative to the folder, then the surface's state and every file in the folder: the
//! generator and the port's answers, as `fssurfio.cpp`'s.

use super::sfile::{creatable, list_files, name, utf8};
use super::*;

use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::lta;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::{alloc, curvio, formats};

const OPS: [&str; 21] = [
    "ra", "wa", "ro", "cv", "nv", "ca", "na", "rf", "rr", "bf", "wd", "rd", "gr", "gw", "vt", "wg",
    "wi", "st", "mf", "mv", "un",
];

/// The folder of a name (`FileNamePath`).
fn folder(n: &str) -> String {
    match n.rfind('/') {
        Some(i) => n[..i].to_owned(),
        None => ".".to_owned(),
    }
}

/// A file in the work folder or in `d` (the only folders that exist).
fn at(path: &str, leaf: &str) -> Option<String> {
    match path {
        "." => Some(leaf.to_owned()),
        "d" => Some(format!("d/{leaf}")),
        _ => None,
    }
}

/// The file a value reader opens for `sname` (`MRISreadCurvatureVector` and its siblings), when
/// it can be made.
fn value_target(fname: &str, hemi: i64, sname: &str) -> Option<String> {
    if sname.contains('/') {
        return creatable(sname).then(|| sname.to_owned());
    }
    let path = folder(fname);
    let leaf = if sname.contains('.') {
        sname.to_owned()
    } else {
        format!("{}.{sname}", if hemi == 0 { "lh" } else { "rh" })
    };
    at(&path, &leaf)
}

/// A number as text: mostly short decimals, now and then a special or out-of-range one, or (one
/// in ten of those) text that is no number.
fn num_text(g: &mut Gen) -> String {
    if g.chance(14) {
        return g
            .pick(&["nan", "-inf", "inf", "1e40", "-1e-50", "0x1p3", "1e", "-0", "+.5", "x"])
            .to_owned();
    }
    clean_num(g)
}

/// A number as text, never one `scanf` cannot read (NaNs, infinities and out-of-range ones now and
/// then).
fn vtk_num(g: &mut Gen) -> String {
    if g.chance(20) {
        return g.pick(&["nan", "-inf", "inf", "1e40", "-1e-50", "-0", "+.5"]).to_owned();
    }
    clean_num(g)
}

fn clean_num(g: &mut Gen) -> String {
    let v = g.int(-4000, 4000) as f32 / 16.0;
    match g.below(3) {
        0 => format!("{v}"),
        1 => format!("{v:.5}"),
        _ => format!("{v:e}"),
    }
}

fn be3(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as u32).to_be_bytes()[1..]);
}

/// A count near `n`: `n` mostly, else one off, 0, or another.
fn near(g: &mut Gen, n: i64) -> i64 {
    match g.below(8) {
        0 => n + 1,
        1 => (n - 1).max(0),
        2 => g.int(0, 9),
        _ => n,
    }
}

/// Cut a file now and then (fs.io's FS-49 and FS-167: FreeSurfer reads memory then).
fn maybe_cut(g: &mut Gen, b: &mut Vec<u8>) {
    if g.chance(9) && !b.is_empty() {
        let cut = g.int(1, 6.min(b.len() as i64)) as usize;
        b.truncate(b.len() - cut);
    }
}

/// An old binary curvature file (the vertex count, the face count, 2-byte hundredths).
fn old_curv(g: &mut Gen, n: i64) -> Vec<u8> {
    let mut b = Vec::new();
    let vnum = near(g, n);
    be3(&mut b, vnum);
    be3(&mut b, g.int(0, 9));
    for _ in 0..vnum {
        b.extend_from_slice(&(g.int(-32768, 32767) as i16).to_be_bytes());
    }
    maybe_cut(g, &mut b);
    b
}

/// A new binary curvature file (the mark, the counts and values per vertex as `int`s, `float`s).
fn new_curv(g: &mut Gen, n: i64) -> Vec<u8> {
    let mut b = Vec::new();
    be3(&mut b, 0xff_ffff);
    let vnum = near(g, n);
    b.extend_from_slice(&(vnum as i32).to_be_bytes());
    b.extend_from_slice(&(g.int(0, 9) as i32).to_be_bytes());
    let per = if g.chance(8) { 2 } else { 1 };
    b.extend_from_slice(&(per as i32).to_be_bytes());
    for _ in 0..vnum {
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    maybe_cut(g, &mut b);
    b
}

fn curv_file(g: &mut Gen, n: i64) -> Vec<u8> {
    if g.chance(2) { old_curv(g, n) } else { new_curv(g, n) }
}

/// A float file (the vertex and face counts as `int`s, then `float`s).
fn float_file(g: &mut Gen, nv: i64, nf: i64) -> Vec<u8> {
    let mut b = Vec::new();
    let vnum = near(g, nv);
    b.extend_from_slice(&(vnum as i32).to_be_bytes());
    b.extend_from_slice(&(near(g, nf) as i32).to_be_bytes());
    for _ in 0..vnum {
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    maybe_cut(g, &mut b);
    b
}

/// An area file (the counts in 3 bytes, then `float`s).
fn area_file(g: &mut Gen, nv: i64) -> Vec<u8> {
    let mut b = Vec::new();
    let vnum = near(g, nv);
    be3(&mut b, vnum);
    be3(&mut b, g.int(0, 9));
    for _ in 0..vnum {
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    maybe_cut(g, &mut b);
    b
}

/// A text curvature file: lines `vno x y z curv` in varied forms, comments and blank lines, now
/// and then too few lines (FS-137), a line without its fifth number, or a line of more than 99
/// bytes (split by `fgetl`).
fn ascii_curv(g: &mut Gen, n: i64) -> Vec<u8> {
    let mut s = String::new();
    let lines = if g.chance(5) { g.int(0, n) } else { n };
    for k in 0..lines {
        if g.chance(8) {
            s += g.pick(&["# a comment\n", "\n", "   \n", "\t# x\n"]);
        }
        let lead = g.pick(&["", " ", "  ", "\t"]);
        let mut parts: Vec<String> = vec![format!("{k:03}")];
        let count = if g.chance(10) { g.int(1, 4) } else { 5 };
        for _ in 1..count {
            parts.push(num_text(g));
        }
        let mut line = format!("{lead}{}", parts.join(g.pick(&[" ", "  ", "\t"])));
        if g.chance(10) {
            line += " # trailing";
        }
        if g.chance(25) {
            line += &" 1".repeat(60);
        }
        s += &line;
        s.push('\n');
    }
    if g.chance(6) {
        s.pop();
    }
    s.into_bytes()
}

/// A decimation file: binary (a NUL byte, the count, a byte per vertex) or text (`#` and a word,
/// the count, a number per vertex), now and then cut or of another count (FS-139, FS-140).
fn decimation(g: &mut Gen, nv: i64) -> Vec<u8> {
    let count = near(g, nv);
    let n = if g.chance(5) { g.int(0, count) } else { count };
    if g.chance(2) {
        let mut b = vec![0u8];
        b.extend_from_slice(&(count as i32).to_be_bytes());
        for _ in 0..n {
            b.push(g.pick(&[0u8, 0, 1, 7]));
        }
        if g.chance(10) {
            b.truncate(g.int(0, b.len() as i64) as usize);
        }
        b
    } else {
        let mut s = String::from(g.pick(&["#!ascii", "#", "#x y"]));
        if g.chance(10) {
            return s.into_bytes();
        }
        s += &format!("\n{count}\n");
        for _ in 0..n {
            s += g.pick(&["0\n", "1\n", "0 ", "2\n", "-1\n"]);
        }
        if g.chance(12) {
            s += "x\n";
        }
        s.into_bytes()
    }
}

/// A VTK file for a surface of `nv` vertices and `faces` (the reader's checks met mostly): the
/// header, `POINTS`, the coordinates, `POLYGONS`, the faces, and now and then `POINT_DATA` with
/// fields; each part now and then malformed, cut or missing (FS-143 to FS-147).
fn vtk(g: &mut Gen, nv: i64, faces: &[[i64; 3]]) -> Vec<u8> {
    let mut s = String::new();
    if g.chance(2) {
        s += "# vtk DataFile Version 1.0\n";
    }
    let heads = [
        g.pick(&["vtk output", "VTK OUTPUT", "surface file", "SURFACE FILE"]),
        g.pick(&["ASCII", "ascii"]),
        g.pick(&["DATASET POLYDATA", "dataset polydata"]),
    ];
    for (k, h) in heads.iter().enumerate() {
        if k == 1 && g.chance(30) {
            continue;
        }
        if g.chance(10) {
            s += "\n# comment\n";
        }
        s += h;
        s.push('\n');
    }
    let points = match g.below(60) {
        0 => "POINTS".to_owned(),
        1 => "POINTSX 3".to_owned(),
        2 => "POINTS -1 float".to_owned(),
        3 => "POINTS 0 float".to_owned(),
        4 => format!("points {nv} float"),
        5 => format!("POINTS {nv} double"),
        6 => format!("POINTS {}", nv + 1),
        _ => format!("POINTS {nv} float"),
    };
    s += &points;
    s.push('\n');
    for k in 0..nv {
        let sep = g.pick(&[" ", "  ", "\t"]);
        let mut c: Vec<String> = (0..3).map(|_| vtk_num(g)).collect();
        if g.chance(60) {
            c.pop();
        }
        if g.chance(80) {
            c[0] = "x".into();
        }
        s += &c.join(sep);
        s += if k % 2 == 1 && g.chance(3) { " " } else { "\n" };
    }
    if g.chance(30) {
        return s.into_bytes();
    }
    let nf = faces.len() as i64;
    let polys = match g.below(60) {
        0 => format!("POLYGONS {nf} {}", nf * 4 + 1),
        1 => format!("polygons {nf} {}", nf * 4),
        2 => format!("POLYGONS {nf}"),
        3 => "POLYS 1 4".to_owned(),
        4 => "POLYGONS 0 0".to_owned(),
        _ => format!("POLYGONS {nf} {}", nf * 4),
    };
    s += &polys;
    s.push('\n');
    for f in faces {
        let lead = if g.chance(80) { "4" } else { "3" };
        let mut v = *f;
        if g.chance(80) {
            v[g.below(3) as usize] = g.pick(&[-1, nv, nv + 3]);
        }
        s += &format!("{lead} {} {} {}\n", v[0], v[1], v[2]);
    }
    if g.chance(3) {
        return s.into_bytes();
    }
    let nvd = if g.chance(15) { nv + 1 } else { nv };
    let pd = g.pick(&["POINT_DATA", "POINT_DATA", "point_data", "CELL_DATA"]);
    s += &format!("{pd} {nvd}\n");
    if g.chance(6) {
        return s.into_bytes();
    }
    let nfields = g.int(0, 2);
    s += &format!("FIELD FieldData {nfields}\n");
    for k in 0..nfields {
        if g.chance(8) {
            return s.into_bytes();
        }
        let fname = g.pick(&["curv", "CURVATURE", "thickness", "val"]);
        let cnt = if g.chance(15) { nv + 2 } else { nv };
        s += &format!("{fname} 1 {cnt} float\n");
        let m = if g.chance(10) { g.int(0, nv) } else { nv };
        for j in 0..m {
            s += &if g.chance(40) { num_text(g) } else { vtk_num(g) };
            s += if (j + k) % 3 == 2 { "\n" } else { " " };
        }
        s.push('\n');
    }
    s.into_bytes()
}

pub(super) fn cfile_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    let mut envs = 0;
    if g.chance(4) {
        envs |= 1;
    }
    if g.chance(3) {
        envs |= 2;
    }
    ix(t, envs);
    if envs & 2 != 0 {
        let v = g.pick(&[".gii", ".gii", "gii", ""]);
        hx(t, v.as_bytes());
    }
    ix(t, g.int(0, 3));
    let nv = g.int(0, 7);
    let nf = if nv >= 3 { g.int(0, 6) } else { 0 };
    let hemi = g.pick(&[0, 1, 2, 3, 0, 1]);
    let fname = g.pick(&[
        "d/lh.white",
        "lh.white",
        "d/rh.pial",
        "",
        "nodir/lh.x",
        "d/sub/lh.w",
    ]);
    let mut faces: Vec<[i64; 3]> = Vec::new();
    for _ in 0..nf {
        let a = g.int(0, nv - 1);
        let mut b = g.int(0, nv - 1);
        while b == a {
            b = g.int(0, nv - 1);
        }
        let mut c = g.int(0, nv - 1);
        while c == a || c == b {
            c = g.int(0, nv - 1);
        }
        faces.push([a, b, c]);
    }
    let nops = g.int(1, 3);
    let mut ops: Vec<String> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    let add = |inputs: &mut Vec<(String, Vec<u8>)>, n: Option<String>, b: Vec<u8>| {
        if let Some(n) = n {
            inputs.push((n, b));
        }
    };
    for k in 0..nops as usize {
        let op = g.pick(&OPS);
        ops.push(op.to_owned());
        let n = name(g, k);
        let direct = creatable(&n).then(|| n.clone());
        let named = |ops: &mut Vec<String>, n: &str| {
            let mut tok = Vec::new();
            hx(&mut tok, n.as_bytes());
            ops.extend(tok);
        };
        let mt = |g: &mut Gen| g.pick(&[-1, -1, 3, 0, 1, 4]).to_string();
        match op {
            "ra" => {
                if !g.chance(5) {
                    let b = ascii_curv(g, nv);
                    add(&mut inputs, direct, b);
                }
                named(&mut ops, &n);
                ops.push(mt(g));
            }
            "ro" => {
                if !g.chance(5) {
                    let b = old_curv(g, nv);
                    add(&mut inputs, direct, b);
                }
                named(&mut ops, &n);
                ops.push(mt(g));
            }
            "cv" | "nv" => {
                if !g.chance(5) {
                    let b = curv_file(g, nv);
                    add(&mut inputs, value_target(fname, hemi, &n), b);
                }
                named(&mut ops, &n);
            }
            "ca" | "na" => {
                let size = match g.below(8) {
                    0 => -1,
                    1 => 0,
                    2 => nv + 1,
                    _ => nv,
                };
                if !g.chance(5) {
                    let b = curv_file(g, size.max(0));
                    let b = if size < 0 && g.chance(2) {
                        let mut b = vec![0xff, 0xff, 0xff];
                        b.extend_from_slice(&(-1i32).to_be_bytes());
                        b.extend_from_slice(&0i32.to_be_bytes());
                        b.extend_from_slice(&1i32.to_be_bytes());
                        b
                    } else {
                        b
                    };
                    add(&mut inputs, direct, b);
                }
                named(&mut ops, &n);
                ops.push(size.to_string());
            }
            "rf" => {
                if !g.chance(5) {
                    let b = float_file(g, nv, nf);
                    add(&mut inputs, value_target(fname, hemi, &n), b);
                }
                named(&mut ops, &n);
            }
            "rr" => {
                if !g.chance(5) {
                    let b = area_file(g, nv);
                    let leaf = if hemi == 0 { "lh.area" } else { "rh.area" };
                    add(&mut inputs, at(&folder(&n), leaf), b);
                }
                named(&mut ops, &n);
            }
            "rd" => {
                if !g.chance(5) {
                    let b = decimation(g, nv);
                    add(&mut inputs, direct, b);
                }
                named(&mut ops, &n);
            }
            "gr" | "gw" => {
                if g.chance(2) {
                    add(&mut inputs, direct.clone(), b"r".to_vec());
                }
                if g.chance(2) && creatable(&format!("{n}.gii")) {
                    add(&mut inputs, Some(format!("{n}.gii")), b"g".to_vec());
                }
                named(&mut ops, &n);
                ops.push(g.pick(&[0, 27, 3]).to_string());
            }
            "vt" => {
                let given = g.chance(2);
                let (cnt, fs) = if given {
                    (nv, faces.clone())
                } else if g.chance(12) {
                    (3, vec![[0, 1, 2]; g.pick(&[255, 256, 257])])
                } else {
                    let c = g.int(1, 5);
                    let fs: Vec<[i64; 3]> = (0..g.int(1, 3))
                        .map(|_| [g.int(0, c - 1), g.int(0, c - 1), g.int(0, c - 1)])
                        .collect();
                    (c, fs)
                };
                if !g.chance(8) {
                    let b = vtk(g, cnt, &fs);
                    add(&mut inputs, direct, b);
                }
                ops.push(i64::from(given).to_string());
                named(&mut ops, &n);
                // a volume of the values only as wide as the file's vertices
                ops.push(if cnt == nv { mt(g) } else { "-1".to_owned() });
            }
            "st" => {
                for _ in 0..22 {
                    ops.push(format!("{:08x}", smallish(g, heavy)));
                }
            }
            "mf" | "mv" => {
                let which = if op == "mf" { nf } else { nv };
                if which == 0 {
                    // nothing to plot: a harmless operation instead
                    ops.pop();
                    ops.push("un".into());
                    ops.push(g.int(0, 1).to_string());
                    continue;
                }
                named(&mut ops, &n);
                ops.push(g.int(0, which - 1).to_string());
                ops.push(i64::from(g.pick(&[b'r', b'g', b'b', b'k', b'%', 1u8, 0xe9])).to_string());
                let len = g.pick(&[0.0f64, 1.0, -2.5, 0.5, f64::NAN, 1e-3]);
                ops.push(format!("{:016x}", len.to_bits()));
            }
            "un" => ops.push(g.int(0, 1).to_string()),
            _ => named(&mut ops, &n),
        }
    }
    inputs.sort();
    inputs.dedup_by(|a, b| a.0 == b.0);
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        hx(t, n.as_bytes());
        hx(t, b);
    }
    ix(t, nv);
    ix(t, nf);
    ix(t, hemi);
    hx(t, fname.as_bytes());
    for _ in 0..nv {
        for _ in 0..10 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, i64::from(g.chance(6)));
        ix(t, g.int(0, 1));
        ix(t, g.int(0, 1));
    }
    for f in &faces {
        for &v in f {
            ix(t, v);
        }
        ix(t, i64::from(g.chance(8)));
    }
    t.push(format!("{:08x}", smallish(g, heavy)));
    t.push(format!("{:08x}", smallish(g, heavy)));
    ix(t, nops);
    t.extend(ops);
}

fn dump_floats(o: &mut Out, p: Option<&[f32]>) {
    match p {
        None => o.s("NULL"),
        Some(v) => {
            for &x in v {
                o.f(x);
            }
        }
    }
}

fn dump_vtk_surface(o: &mut Out, s: &Mris) {
    for v in [s.nvertices, s.nfaces, s.r#type, s.max_vertices, s.max_faces] {
        o.i(i64::from(v));
    }
    for vno in 0..s.nvertices as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z, v.curv, v.val] {
            o.f(x);
        }
        o.i(i64::from(s.topology(vno).num));
    }
    for fno in 0..s.nfaces as usize {
        for &v in &s.face(fno).v {
            o.i(i64::from(v));
        }
    }
    o.f(s.max_curv);
    o.f(s.min_curv);
    o.i(i64::from(s.dist_alloced_flags));
}

pub(super) fn cfile_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let envs = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if envs & 1 != 0 {
        pairs.push(("FS_POSIX".into(), "1".into()));
    }
    if envs & 2 != 0 {
        let v = t.h();
        let v = &v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())];
        pairs.push(("FS_GII".into(), utf8(v.to_vec())));
    }
    let env = Environment::from_pairs(pairs);
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let (nv, nf) = (t.i(), t.i());
    let mut m = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, nv, nf)?;
    m.hemisphere = t.i();
    let fname = t.h();
    let fname = &fname[..fname.iter().position(|&c| c == 0).unwrap_or(fname.len())];
    m.fname = utf8(fname[..fname.len().min(1023)].to_vec());
    for vno in 0..nv as usize {
        let mut f = [0.0f32; 10];
        for x in &mut f {
            *x = t.f();
        }
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z, v.nx, v.ny, v.nz) = (f[0], f[1], f[2], f[3], f[4], f[5]);
        (v.d, v.curv, v.val, v.origarea) = (f[6], f[7], f[8], f[9]);
        v.ripflag = t.i() as i8;
        v.undefval = t.i();
        v.fixedval = t.i();
    }
    for fno in 0..nf as usize {
        let v = [t.i(), t.i(), t.i()];
        let face = m.face_mut(fno);
        face.v = v;
        face.ripflag = t.i() as i8;
    }
    m.max_curv = t.f();
    m.min_curv = t.f();
    let nops = t.i();
    let d = Some(dir);
    let mut k = 0;
    while k < nops {
        let op = t.next();
        k += 1;
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        match op {
            "ra" | "ro" => {
                let n = utf8(t.h());
                let mut mri = case_mri_split(g, &mut io, t, nv)?;
                let r = if op == "ra" {
                    curvio::read_ascii_curvature_file(g, &mut io, d, &mut m, &n, mri.as_mut())?
                } else {
                    curvio::read_curv_old_format(g, &mut io, d, &mut m, &n, mri.as_mut())?
                };
                o.i(i64::from(r));
                dump_mri_g(g, &mut io, o, mri)?;
            }
            "wa" => {
                let n = utf8(t.h());
                o.i(i64::from(curvio::write_ascii_curvature_file(g, &mut io, d, &m, &n)?));
            }
            "cv" | "nv" => {
                let n = utf8(t.h());
                let r = if op == "cv" {
                    curvio::read_curvature_vector(g, &mut io, d, &m, &n)?
                } else {
                    curvio::read_new_curvature_vector(g, &mut io, d, &m, &n)?
                };
                dump_floats(o, r.as_deref());
            }
            "ca" | "na" => {
                let n = utf8(t.h());
                let size = t.i();
                let (r, p) = if op == "ca" {
                    curvio::read_curvature_into_array(g, &mut io, d, &n, size)?
                } else {
                    curvio::read_new_curvature_into_array(g, &mut io, d, &n, size)?
                };
                o.i(i64::from(r));
                dump_floats(o, p.as_deref());
            }
            "rf" => {
                let n = utf8(t.h());
                o.i(i64::from(curvio::read_float_file(g, &mut io, d, &mut m, &n)?));
            }
            "rr" => {
                let n = utf8(t.h());
                o.i(i64::from(curvio::read_binary_areas(g, &mut io, d, &mut m, &n)?));
            }
            "bf" => {
                let n = utf8(t.h());
                if n.len() + m.fname.len() + 8 >= 1024 {
                    o.s("G");
                    continue;
                }
                let (r, name) = curvio::build_file_name_read(g, &mut io, &m, &n);
                o.i(i64::from(r));
                o.h(Some(name.as_bytes()));
            }
            "wd" => {
                let n = utf8(t.h());
                o.i(i64::from(curvio::write_decimation(g, &mut io, d, &m, &n)));
            }
            "rd" => {
                let n = utf8(t.h());
                o.i(i64::from(curvio::read_decimation(g, &mut io, d, &mut m, &n)));
            }
            "gr" | "gw" => {
                let n = utf8(t.h());
                let mut ty = t.i();
                let gii = env.get("FS_GII").is_some_and(|v| v.as_encoded_bytes() == b".gii");
                if op == "gw" && gii && n.len() + 5 > 1024 {
                    o.s("G");
                    continue;
                }
                let mut buf = n.clone();
                if op == "gr" {
                    curvio::apply_fsgii_read(g, &mut io, d, &mut buf, &n, &mut ty)?;
                } else {
                    curvio::apply_fsgii_write(g, &mut io, d, &mut buf, &n, &mut ty)?;
                }
                o.h(Some(buf.as_bytes()));
                o.i(i64::from(ty));
            }
            "vt" => {
                let given = t.i() != 0;
                let n = utf8(t.h());
                let mut mri = case_mri_split(g, &mut io, t, nv)?;
                let r = formats::read_vtk(
                    g,
                    &cx.sg,
                    &mut io,
                    d,
                    if given { Some(&mut m) } else { None },
                    &n,
                    mri.as_mut(),
                )?;
                match r {
                    Ret::Null => o.s("NULL"),
                    Ret::Given => o.s("SAME"),
                    Ret::New(s) => {
                        o.s("NEW");
                        dump_vtk_surface(o, &s);
                    }
                }
                dump_mri_g(g, &mut io, o, mri)?;
            }
            "wg" => {
                let n = utf8(t.h());
                let bad = (0..nf as usize).any(|f| {
                    let face = m.face(f);
                    face.ripflag == 0 && face.v.iter().any(|&v| m.vertex(v as usize).ripflag != 0)
                });
                if bad {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(formats::write_geo(g, &mut io, d, &m, &n)));
            }
            "wi" => {
                let n = utf8(t.h());
                o.i(i64::from(formats::write_ico(g, &mut io, d, &m, &n)));
            }
            "st" => {
                let mut l = lta::lta_alloc(g, io.out, io.err, 1, None)?;
                let lt = &mut l.xforms[0];
                let ml = lt.m_l.as_mut().expect("m_L");
                for r in 1..=4 {
                    for c in 1..=4 {
                        ml.set_relt(r, c, t.f());
                    }
                }
                (lt.src.c_r, lt.src.c_a, lt.src.c_s) = (t.f(), t.f(), t.f());
                (lt.dst.c_r, lt.dst.c_a, lt.dst.c_s) = (t.f(), t.f(), t.f());
                match formats::get_sras_to_tal_sras(g, &mut io, &l.xforms[0])? {
                    None => o.s("NULL"),
                    Some(res) => {
                        for r in 1..=4 {
                            for c in 1..=4 {
                                o.f(res.relt(r, c));
                            }
                        }
                    }
                }
            }
            "mf" | "mv" => {
                let n = utf8(t.h());
                let which = t.i();
                let color = t.i() as u8;
                let len = t.d();
                let mut fp = match CFile::open(d, &n, "w") {
                    Ok(f) => f,
                    Err(e) => {
                        g.errno = e;
                        o.s("NOFP");
                        continue;
                    }
                };
                let r = if op == "mf" {
                    formats::matlab_plot_face(&cx.sg, &mut io, &mut fp, &m, which, color, len)
                } else {
                    formats::matlab_plot_vertex(&mut io, &mut fp, &m, which, color, len)
                };
                fp.close();
                o.i(i64::from(r));
            }
            "un" => cx.sg.unitize_normal_face = t.i(),
            _ => panic!("cfile op {op}"),
        }
    }
    o.0 += "| ";
    for vno in 0..nv as usize {
        let v = m.vertex(vno);
        for x in [v.x, v.y, v.z, v.nx, v.ny, v.nz, v.d, v.curv, v.val, v.origarea] {
            o.f(x);
        }
        o.i(i64::from(v.undefval));
        o.i(i64::from(v.fixedval));
    }
    o.f(m.max_curv);
    o.f(m.min_curv);
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

/// The case's volume of values (`mt` >= 0: `MRIalloc(nv, 1, 1, mt)`).
fn case_mri_split(
    g: &mut Globals,
    io: &mut coords::Io,
    t: &mut Toks,
    nv: i32,
) -> Result<Option<Mri>, Fatal> {
    let ty = t.i();
    if ty < 0 || nv < 1 {
        return Ok(None);
    }
    Ok(Some(larmorx_freesurfer_mri::mri::mri_alloc(
        g, io.out, io.err, nv, 1, 1, ty,
    )?))
}

fn dump_mri_g(
    g: &mut Globals,
    io: &mut coords::Io,
    o: &mut Out,
    m: Option<Mri>,
) -> Result<(), Fatal> {
    let Some(m) = m else {
        o.s("NOMRI");
        return Ok(());
    };
    for i in 0..m.width {
        o.f(mri_get_vox_val(g, io.err, &m, i, 0, 0, 0, 0)?);
    }
    Ok(())
}

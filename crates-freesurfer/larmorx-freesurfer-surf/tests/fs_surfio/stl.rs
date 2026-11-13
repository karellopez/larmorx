// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue item 3.1-49 (relay 13): `stl`, the STL reader (binary or text)
//! and writer, in an empty work folder (with a subfolder `d`): Gdiag's bits, the case's input
//! files, a surface from `MRISalloc` with its coordinates, faces and face-normal cache for the
//! writer, 1 to 3 operations, then every file in the folder: the generator and the port's
//! answers, as `fssurfio.cpp`'s.

use super::cfile::clean_num;
use super::sfile::{creatable, list_files, utf8};
use super::*;

use larmorx_freesurfer_surf::{alloc, stl};

/// A file name of operation `k` (its own).
fn name(g: &mut Gen, k: usize) -> String {
    match g.below(5) {
        0 => format!("d/s{k}.stl"),
        1 => format!("s{k}.stl"),
        2 => format!("nodir/s{k}.stl"),
        _ => format!("lh.s{k}.stl"),
    }
}

/// A corner point: mostly from a small pool (so that facets share vertices), its zeros now and
/// then negative, now and then a NaN or an infinity.
fn point(g: &mut Gen, pool: &[[f32; 3]]) -> [f32; 3] {
    if g.chance(40) {
        let s = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -f32::NAN];
        let mut p = pool[g.below(pool.len() as u64) as usize];
        p[g.int(0, 2) as usize] = g.pick(&s);
        if g.chance(3) {
            p[g.int(0, 2) as usize] = g.pick(&s);
        }
        return p;
    }
    let mut p = pool[g.below(pool.len() as u64) as usize];
    if g.chance(10) {
        for c in &mut p {
            if *c == 0.0 {
                *c = -0.0;
            }
        }
    }
    p
}

/// A coordinate as an STL text writes it (`%e`, `%f` or short forms; `-0` and NaN as such).
fn text_num(g: &mut Gen, c: f32) -> String {
    if c.is_nan() {
        return g.pick(&["nan", "NaN", "-nan"]).to_owned();
    }
    if c.is_infinite() {
        return if c > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    match g.below(3) {
        0 => format!("{c:e}"),
        1 => format!("{c:.6}"),
        _ => format!("{c}"),
    }
}

/// The facets of a case: triangles of pool points, now and then a fan around one point (16 to
/// 20 facets: a vertex of more than 30 neighbours, `modVnum`'s message; rarely 256: FS-157).
fn facets(g: &mut Gen, pool: &[[f32; 3]]) -> Vec<[[f32; 3]; 4]> {
    let normal = |g: &mut Gen| {
        [
            (g.int(-8, 8) as f32) / 8.0,
            (g.int(-8, 8) as f32) / 8.0,
            (g.int(-8, 8) as f32) / 8.0,
        ]
    };
    let mut fs = Vec::new();
    if g.chance(12) {
        let k = if g.chance(6) { 256 } else { g.int(16, 20) };
        let c = [0.5f32, 0.5, 0.0];
        for i in 0..k {
            let a = [i as f32, 1.0, 0.0];
            let b = [i as f32 + 1.0, 1.0, 0.0];
            fs.push([normal(g), c, a, b]);
        }
        return fs;
    }
    for _ in 0..g.int(0, 5) {
        fs.push([normal(g), point(g, pool), point(g, pool), point(g, pool)]);
    }
    fs
}

/// A text STL file of these facets, with variations: comments and blank lines, numbers that
/// cannot be read, a facet of two or four corners, a corner before the first facet, an
/// unrecognized line, a second solid.
fn text_stl(g: &mut Gen, fs: &[[[f32; 3]; 4]]) -> Vec<u8> {
    if g.chance(30) {
        return g
            .pick(&["", "solid x\n", "# nothing\n"])
            .as_bytes()
            .to_vec();
    }
    let mut s = String::new();
    if g.chance(4) {
        s += "# a comment\n\n";
    }
    s += "solid x\n";
    if g.chance(25) {
        s += "vertex 0 0 0\n";
    }
    let odd = if g.chance(4) {
        g.int(0, fs.len() as i64 * 3 + 2) as usize
    } else {
        usize::MAX
    };
    let mut line = 0;
    for f in fs {
        let mut corners = 3;
        if g.chance(30) {
            corners = g.pick(&[2, 4]);
        }
        let nums = |g: &mut Gen, p: [f32; 3], line: usize| {
            let mut t: Vec<String> = p.iter().map(|&c| text_num(g, c)).collect();
            if line == odd {
                match g.below(3) {
                    0 => t[g.int(0, 2) as usize] = "x".to_owned(),
                    1 => t.truncate(g.int(0, 2) as usize),
                    _ => t[g.int(0, 2) as usize] = clean_num(g) + "e",
                }
            }
            t.join(" ")
        };
        s += &format!(" facet normal {}\n", nums(g, f[0], line));
        line += 1;
        s += "  outer loop\n";
        for k in 0..corners {
            let p = f[1 + k.min(2)];
            s += &format!("   vertex {}\n", nums(g, p, line));
            line += 1;
        }
        s += "  endloop\n endfacet\n";
    }
    match g.below(30) {
        0 => s += "solid y\n",
        1 => s += "  bogus line # c\n",
        2 => s += "SOLID z\n",
        _ => {}
    }
    s += "endsolid x\n";
    s.into_bytes()
}

/// A binary STL file of these facets (its header sometimes starting with `solid`, as many
/// writers do), now and then one byte longer (no longer binary: read as text).
fn binary_stl(g: &mut Gen, fs: &[[[f32; 3]; 4]]) -> Vec<u8> {
    let mut b = Vec::new();
    let head: &[u8] = if g.chance(3) {
        b"solid binary"
    } else {
        b"freesurfer mesh"
    };
    let mut header = [0u8; 80];
    header[..head.len()].copy_from_slice(head);
    b.extend_from_slice(&header);
    b.extend_from_slice(&(fs.len() as u32).to_le_bytes());
    for f in fs {
        for p in f {
            for c in p {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        b.extend_from_slice(g.pick(&[b"\0\0", b"0\0", b"AB"]));
    }
    if g.chance(15) {
        b.push(b'\n');
    }
    b
}

pub(super) fn stl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, g.int(0, 3));
    let pool: Vec<[f32; 3]> = (0..g.int(3, 6))
        .map(|_| {
            [
                (g.int(-4, 4) as f32) / 2.0,
                (g.int(-4, 4) as f32) / 2.0,
                (g.int(-1, 1) as f32) / 2.0,
            ]
        })
        .collect();
    let nops = if g.chance(4) { g.int(2, 3) } else { 1 };
    let mut ops: Vec<String> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    for k in 0..nops as usize {
        let op = if g.chance(5) { "ws" } else { "rs" };
        ops.push(op.to_owned());
        let n = name(g, k);
        if op == "rs" && creatable(&n) && !g.chance(10) {
            let fs = facets(g, &pool);
            let b = if g.chance(2) {
                text_stl(g, &fs)
            } else {
                binary_stl(g, &fs)
            };
            inputs.push((n.clone(), b));
        }
        hx(&mut ops, n.as_bytes());
    }
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        hx(t, n.as_bytes());
        hx(t, b);
    }
    let nv = g.pick(&[0, 3, 4, 5, 6]);
    let nf = if nv >= 3 { g.int(0, 4) } else { 0 };
    ix(t, nv);
    ix(t, nf);
    for _ in 0..nv * 3 {
        t.push(format!("{:08x}", smallish(g, heavy)));
    }
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
        for v in [a, b, c] {
            ix(t, v);
        }
        for _ in 0..4 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, g.pick(&[0, 0, 1, 2, 3]));
    }
    ix(t, nops);
    t.extend(ops);
}

fn dump_stl_surface(o: &mut Out, s: Option<&Mris>) {
    let Some(s) = s else {
        o.s("NULL");
        return;
    };
    for v in [s.nvertices, s.nfaces, s.r#type, s.max_vertices, s.max_faces] {
        o.i(i64::from(v));
    }
    for vno in 0..s.nvertices as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
        let vt = s.topology(vno);
        for x in [i64::from(vt.num), i64::from(vt.vnum), i64::from(vt.vtotal)] {
            o.i(x);
        }
        let list = vt.v.as_deref().unwrap_or(&[]);
        for k in 0..vt.vnum.max(0) as usize {
            o.i(i64::from(list[k]));
        }
        let (f, n) = (
            vt.f.as_deref().unwrap_or(&[]),
            vt.n.as_deref().unwrap_or(&[]),
        );
        for k in 0..usize::from(vt.num) {
            o.i(i64::from(f[k]));
            o.i(i64::from(n[k]));
        }
    }
    for fno in 0..s.nfaces as usize {
        for &v in &s.face(fno).v {
            o.i(i64::from(v));
        }
        let e = &s.faceNormCacheEntries.as_ref().unwrap()[fno];
        for x in [e.nx, e.ny, e.nz] {
            o.f(x);
        }
        o.i(i64::from(
            s.faceNormDeferredEntries.as_ref().unwrap()[fno].deferred,
        ));
    }
}

pub(super) fn stl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let env = Environment::from_pairs(Vec::<(String, String)>::new());
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
    for vno in 0..nv as usize {
        let (x, y, z) = (t.f(), t.f(), t.f());
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z) = (x, y, z);
    }
    for fno in 0..nf as usize {
        let v = [t.i(), t.i(), t.i()];
        m.face_mut(fno).v = v;
        let (nx, ny, nz, oa) = (t.f(), t.f(), t.f(), t.f());
        let e = &mut m.faceNormCacheEntries.as_mut().unwrap()[fno];
        (e.nx, e.ny, e.nz, e.orig_area) = (nx, ny, nz, oa);
        m.faceNormDeferredEntries.as_mut().unwrap()[fno].deferred = t.i() as i8;
    }
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
        match op {
            "rs" => {
                let r = stl::read_stl(&mut cx.g, &mut cx.sg, &mut io, d, &n)?;
                dump_stl_surface(o, r.as_ref());
            }
            "ws" => o.i(i64::from(stl::write_stl(d, &mut m, &n))),
            _ => panic!("stl op {op}"),
        }
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

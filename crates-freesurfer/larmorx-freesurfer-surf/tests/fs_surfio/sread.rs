// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue item 3.1-48 part b (relay 13): `sread`, the surface library's
//! file-local surface readers (ASCII, movie.byu and its positions, the binary triangle file's
//! positions) and the old writer's quadrangles, in an empty work folder (with a subfolder `d`):
//! whether `FREESURFER_reportLogicErrors` is set, Gdiag's bits, the case's input files, a surface
//! from `MRISalloc` with its coordinates, rip flags, faces and distance state, 1 to 3 operations,
//! then the case surface's coordinates and distance state: the generator and the port's answers,
//! as `fssurfio.cpp`'s.

use super::cfile::{clean_num, maybe_cut, num_text, vtk_num};
use super::sfile::{creatable, utf8};
use super::*;

use larmorx_freesurfer_surf::{alloc, surfread};

const OPS: [&str; 6] = ["ra", "rg", "rp", "rv", "rt", "qc"];

/// Leading lines `fgetl` passes over: none mostly, else comments and blank lines.
fn preamble(g: &mut Gen, s: &mut String) {
    if g.chance(3) {
        s.push_str(g.pick(&[
            "#!ascii version of lh.white\n",
            "# a comment\n\n",
            "   \n#x\n",
            "\t# indented comment\n",
        ]));
    }
}

/// A coordinate as text: mostly a short decimal, now and then a special one or (rarely) text that
/// is no number.
fn coord_text(g: &mut Gen) -> String {
    if g.chance(30) {
        return num_text(g);
    }
    vtk_num(g)
}

/// An ASCII surface (`MRISwriteAscii`'s format and variations): the counts, `x  y  z  rip` per
/// vertex, `a b c rip` per face; now and then a header that cannot be read, a face of a vertex
/// out of range, a vertex of more than 255 faces, a cut file or one of no line.
pub(super) fn ascii_surface(g: &mut Gen) -> Vec<u8> {
    let mut s = String::new();
    if g.chance(30) {
        return g
            .pick(&["", "# only a comment\n", "\n \n"])
            .as_bytes()
            .to_vec();
    }
    preamble(g, &mut s);
    let many = g.chance(25);
    let nv = if many {
        3
    } else {
        g.pick(&[0, 1, 3, 4, 5, 6, 3, 4])
    };
    let nf: i64 = if many {
        g.pick(&[255, 256, 257])
    } else if nv >= 3 {
        g.int(0, 4)
    } else {
        0
    };
    match g.below(30) {
        0 => s.push_str("x y\n"),
        1 => s.push_str(&format!("{nv}\n")),
        2 => s.push_str(&format!("{} {nf}\n", g.pick(&[-1, -3]))),
        _ => s.push_str(&format!("{nv} {nf}\n")),
    }
    let rip = |g: &mut Gen| -> i64 {
        if g.chance(5) {
            g.pick(&[1, 256, -1, 2, 1000, 255, 512])
        } else {
            0
        }
    };
    for _ in 0..nv {
        let sep = if g.chance(6) { " " } else { "  " };
        s += &format!(
            "{}{sep}{}{sep}{}{sep}{}\n",
            coord_text(g),
            coord_text(g),
            coord_text(g),
            rip(g)
        );
    }
    for _ in 0..nf {
        let mut f = [0i64, 1, 2];
        if !many {
            for x in &mut f {
                *x = g.int(0, nv - 1);
            }
            if g.chance(15) {
                f[g.int(0, 2) as usize] = g.pick(&[nv, -1, nv + 3]);
            }
        }
        s += &format!("{} {} {} {}\n", f[0], f[1], f[2], rip(g));
    }
    let mut b = s.into_bytes();
    maybe_cut(g, &mut b);
    b
}

/// A movie.byu number of a coordinate (`%e`'s form mostly, as `MRISwriteGeo` writes).
fn geo_num(g: &mut Gen) -> String {
    if g.chance(40) {
        return num_text(g);
    }
    if g.chance(2) {
        let v = g.int(-4000, 4000) as f32 / 16.0;
        return format!("{v:e}");
    }
    clean_num(g)
}

/// A movie.byu surface of `nv` vertices and `nf` faces (`MRISwriteGeo`'s format: the counts, the
/// face count again, six coordinates per line, faces `a b -c` from 1), with variations: a header
/// that cannot be read, a face count of 0, edges that are not three per face, vertex numbers out
/// of range or a last vertex not negative, a cut file, a file of no line.
pub(super) fn geo_surface(g: &mut Gen, nv: i64, nf: i64, many: bool) -> Vec<u8> {
    if g.chance(30) {
        return g.pick(&["", "# nothing\n"]).as_bytes().to_vec();
    }
    let mut s = String::new();
    preamble(g, &mut s);
    let ne = match g.below(25) {
        0 => 4 * nf,
        1 => nf * 3 + 1,
        _ => 3 * nf,
    };
    match g.below(25) {
        0 => s.push_str("1 2\n"),
        1 => s.push_str(&format!("1 {nv} 0 {}\n", g.pick(&[0, 3, 6]))),
        _ => s.push_str(&format!("1 {nv} {nf} {ne}\n")),
    }
    s.push_str(&format!("1 {nf}\n"));
    for vno in 0..nv {
        s += &format!("{} {} {}", geo_num(g), geo_num(g), geo_num(g));
        s += if vno % 2 == 1 { "\n" } else { " " };
    }
    if nv % 2 == 1 {
        s.push('\n');
    }
    for _ in 0..nf {
        let mut f = [1i64, 2, 3];
        if !many {
            for x in &mut f {
                *x = g.int(1, nv.max(1));
            }
            if g.chance(15) {
                f[g.int(0, 2) as usize] = g.pick(&[0, nv + 1, -2]);
            }
        }
        let last = if g.chance(20) {
            format!("{}", f[2])
        } else {
            format!("-{}", f[2])
        };
        s += &format!("{} {} {last}\n", f[0], f[1]);
    }
    let mut b = s.into_bytes();
    maybe_cut(g, &mut b);
    b
}

/// A binary triangle file's start and positions: the magic number, the creator's line (now and
/// then one longer than 199 bytes, or without its blank line), the counts, three floats per vertex
/// (now and then one beyond 10000 or not finite), cut now and then (fs.io's FS-49, FS-167).
fn triangle_file(g: &mut Gen, nv: i64, nf: i64, heavy: bool) -> Vec<u8> {
    let mut b = vec![0xff, 0xff, 0xfe];
    match g.below(12) {
        0 => b.extend_from_slice(format!("created by {} on today\n", "u".repeat(210)).as_bytes()),
        1 => b.extend_from_slice(b"created by me on today\n"),
        2 => b.extend_from_slice(b"created by me on today\n\n \t\n"),
        _ => b.extend_from_slice(b"created by me on today\n\n"),
    }
    b.extend_from_slice(&(nv as i32).to_be_bytes());
    b.extend_from_slice(&(nf as i32).to_be_bytes());
    for _ in 0..nv.max(0) * 3 {
        let x = if g.chance(25) {
            f32::from_bits(g.pick(&[
                0x7fc0_0000,
                0xff80_0000,
                0x461c_4400,
                0xc61c_4000,
                0x461c_3c00,
            ]))
        } else {
            f32::from_bits(smallish(g, heavy))
        };
        b.extend_from_slice(&x.to_be_bytes());
    }
    maybe_cut(g, &mut b);
    b
}

/// A file name of operation `k` (its own: two operations never read one file): in the work
/// folder, in `d`, in a folder that does not exist.
fn name(g: &mut Gen, k: usize) -> String {
    match g.below(6) {
        0 => format!("d/lh.s{k}"),
        1 => format!("s{k}"),
        2 => format!("nodir/s{k}"),
        3 => format!("d/s{k}.geo"),
        _ => format!("rh.s{k}"),
    }
}

/// A triangle of vertex numbers for `quadCombine`.
fn tri(g: &mut Gen) -> [i64; 3] {
    let mut v = [0; 3];
    for x in &mut v {
        *x = g.int(0, 5);
    }
    v
}

pub(super) fn sread_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, i64::from(g.chance(2)));
    ix(t, g.int(0, 3));
    let nv = g.pick(&[0, 1, 3, 4, 5, 6, 3]);
    let nf = if nv >= 3 { g.int(0, 4) } else { 0 };
    let fname = g.pick(&["d/lh.white", "lh.white", "rh.pial", ""]);
    let nops = if g.chance(4) { g.int(2, 3) } else { 1 };
    let mut ops: Vec<String> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    for k in 0..nops as usize {
        let op = g.pick(&OPS);
        ops.push(op.to_owned());
        let n = name(g, k);
        let direct = creatable(&n).then(|| n.clone());
        let mut file = |g: &mut Gen, b: Vec<u8>| {
            if let Some(d) = direct.clone()
                && !g.chance(8)
            {
                inputs.push((d, b));
            }
        };
        match op {
            "ra" => {
                let b = ascii_surface(g);
                file(g, b);
            }
            "rg" | "rp" => {
                let many = op == "rg" && g.chance(25);
                let (fv, ff) = if many {
                    (3, g.pick(&[255, 256, 257]))
                } else if op == "rp" && !g.chance(6) {
                    (nv, nf)
                } else {
                    let c = g.int(1, 6);
                    (c, if c >= 3 { g.int(1, 4) } else { 1 })
                };
                let b = geo_surface(g, fv, ff, many);
                file(g, b);
            }
            "rv" | "rt" => {
                let (fv, ff) = if op == "rt" && !g.chance(6) {
                    (nv, nf)
                } else {
                    match g.below(10) {
                        0 => (g.pick(&[-1, -7]), 0),
                        1 => (0, 0),
                        _ => (g.int(1, 6), g.int(0, 4)),
                    }
                };
                let b = triangle_file(g, fv, ff, heavy);
                file(g, b);
            }
            _ => {}
        }
        if op == "qc" {
            let a = tri(g);
            let b = if g.chance(2) {
                // the shared edge reversed, in one of B's three rotations
                let r = g.int(0, 2) as usize;
                let e = g.int(0, 2) as usize;
                let base = [a[(e + 1) % 3], a[e], g.int(0, 7)];
                [base[r % 3], base[(r + 1) % 3], base[(r + 2) % 3]]
            } else {
                tri(g)
            };
            for v in a.iter().chain(b.iter()) {
                ix(&mut ops, *v);
            }
        } else {
            hx(&mut ops, n.as_bytes());
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
    hx(t, fname.as_bytes());
    for _ in 0..nv {
        for _ in 0..3 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, i64::from(g.chance(6)));
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
    }
    ix(t, g.int(0, 3));
    ix(t, g.int(0, 3));
    ix(t, nops);
    t.extend(ops);
}

fn dump_read_surface(o: &mut Out, s: Option<&Mris>) {
    let Some(s) = s else {
        o.s("NULL");
        return;
    };
    for v in [
        s.nvertices,
        s.nfaces,
        s.r#type,
        s.patch,
        s.status.0,
        s.max_vertices,
        s.max_faces,
    ] {
        o.i(i64::from(v));
    }
    for vno in 0..s.nvertices as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
        o.i(i64::from(v.ripflag));
        o.i(i64::from(s.topology(vno).num));
    }
    for fno in 0..s.nfaces as usize {
        let f = s.face(fno);
        for &v in &f.v {
            o.i(i64::from(v));
        }
        o.i(i64::from(f.ripflag));
    }
    o.i(i64::from(s.dist_alloced_flags));
}

pub(super) fn sread_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if t.i() != 0 {
        pairs.push(("FREESURFER_reportLogicErrors".into(), "1".into()));
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
    let fname = t.h();
    let fname = &fname[..fname.iter().position(|&c| c == 0).unwrap_or(fname.len())];
    m.fname = utf8(fname[..fname.len().min(1023)].to_vec());
    for vno in 0..nv as usize {
        let (x, y, z) = (t.f(), t.f(), t.f());
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z) = (x, y, z);
        v.ripflag = t.i() as i8;
    }
    for fno in 0..nf as usize {
        let v = [t.i(), t.i(), t.i()];
        m.face_mut(fno).v = v;
    }
    m.dist_alloced_flags = t.i() as i8;
    m.dist_nsize = t.i() as i8;
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        match op {
            "ra" => {
                let n = utf8(t.h());
                let r = surfread::read_ascii_file(g, &cx.sg, &mut io, d, &n)?;
                dump_read_surface(o, r.as_ref());
            }
            "rg" => {
                let n = utf8(t.h());
                let r = surfread::read_geo_file(g, &cx.sg, &mut io, d, &n)?;
                dump_read_surface(o, r.as_ref());
            }
            "rp" => {
                let n = utf8(t.h());
                let r = surfread::read_geo_file_positions(g, &mut io, d, &mut m, &n)?;
                o.i(i64::from(r));
            }
            "rv" => {
                let n = utf8(t.h());
                match surfread::read_triangle_file_vertex_positions_only(g, &mut io, d, &n)? {
                    None => o.s("NULL"),
                    Some(s) => {
                        o.i(i64::from(s.nvertices));
                        if s.nvertices < 0 {
                            o.s("NOVERTICES");
                        } else {
                            for v in &s.vertices {
                                for x in [v.x, v.y, v.z] {
                                    o.f(x);
                                }
                            }
                        }
                    }
                }
            }
            "rt" => {
                let n = utf8(t.h());
                let r = surfread::read_triangle_file_positions(g, &mut io, d, &mut m, &n)?;
                o.i(i64::from(r));
            }
            "qc" => {
                let a = [t.i(), t.i(), t.i()];
                let b = [t.i(), t.i(), t.i()];
                let mut q = [-7; 4];
                o.i(i64::from(surfread::quad_combine(&mut q, &a, &b)));
                for x in q {
                    o.i(i64::from(x));
                }
            }
            _ => panic!("sread op {op}"),
        }
    }
    o.s("|");
    for vno in 0..nv as usize {
        let v = m.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
    }
    o.i(i64::from(m.dist_alloced_flags));
    o.i(i64::from(m.dist_nsize));
    Ok(())
}

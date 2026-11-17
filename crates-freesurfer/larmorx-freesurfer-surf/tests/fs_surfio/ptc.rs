// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command 65 (relay 45 of track D): `ptc`, the patch readers `MRISreadPatchNoRemove`
//! and `MRISreadPatch` (fs.surf's `patchread.rs`): an empty work folder with `d`, `Gdiag`'s show
//! bit and `Gdiag_no`, `FS_POSIX` now and then; a surface read by `MRISread` (`d/lh.s0`:
//! icosahedra, grids, srd's small meshes); 0 to 3 input files (ASCII patches with comments, blank
//! lines, long lines, missing numbers; binary patches of the old and new forms, now and then cut or
//! with vertex numbers outside the surface; labels; volumes of one voxel per vertex; GIFTI files;
//! random bytes); then 1 to 4 of: rip or mark a border vertex (`rip`, `bd`), move the current
//! positions (`jit`), write the patch (`MRISwritePatch`, `wp`), read one (`nr`, `rp`, with or without
//! the conversion to tkregister space); after each the surface (counts, patch flag, status, total
//! area, the box; each vertex's position, rip and border flags, normal and area, its counts of
//! faces and neighbours; each face's rip flag and area): the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::crv::mgh_curv;
use super::fli::{fv, surface};
use super::gii::surface_file;
use super::mht::crumple;
use super::mpd::{grid, ico};
use super::sfile::utf8;
use super::srd::{Mesh, mesh};
use super::*;

use larmorx_freesurfer_surf::{patchread, readwrite, valfile};

/// The names the cases write and read (the surface is `d/lh.s0`: `MRISbuildFileName_read` puts a
/// bare name in `d` after `lh.`).
const NAMES: [&str; 14] = [
    "patch",
    "lh.patch",
    "d/lh.patch",
    "p.asc",
    "lh.p.asc",
    "x.label",
    "lh.x.label",
    "v.mgh",
    "lh.v.mgz",
    "p.gii",
    "d/q",
    "nofile",
    "./d",
    "lh.flat.patch.3d",
];

/// The file a case puts at a name, by its kind.
fn file_for(g: &mut Gen, name: &str, m: &Mesh, heavy: bool) -> Vec<u8> {
    let nv = m.v.len() as i64;
    if g.chance(14) {
        let n = g.int(0, 40) as usize;
        return (0..n).map(|_| g.below(256) as u8).collect();
    }
    if name.ends_with(".asc") {
        ascii_patch(g, nv)
    } else if name.ends_with(".label") {
        label_file(g, nv)
    } else if name.ends_with(".mgh") {
        mgh_curv(g, nv, false)
    } else if name.ends_with(".mgz") {
        mgh_curv(g, nv, true)
    } else if name.ends_with(".gii") {
        let mut mm = Mesh {
            v: m.v.clone(),
            f: m.f.clone(),
        };
        if g.chance(3) && mm.v.len() > 3 {
            mm.v.truncate(mm.v.len() - 1);
            mm.f.retain(|f| f.iter().all(|&k| k < mm.v.len() as i64));
        }
        surface_file(g, &mm, heavy)
    } else {
        binary_patch(g, nv)
    }
}

/// A vertex number of a patch file (one-based; negative for a border vertex), outside the surface
/// now and then (0, beyond, `INT_MIN`).
fn point_number(g: &mut Gen, nv: i64) -> i64 {
    if g.chance(70) {
        return g.pick(&[0, nv + 1, nv + 7, -(nv + 1), i64::from(i32::MIN), i64::from(i32::MAX)]);
    }
    let k = g.int(1, nv.max(1));
    if g.chance(4) { -k } else { k }
}

/// An ASCII patch: comments and blank lines now and then, its count line, then per point its
/// number line (`<n>  vno=<k>`) and coordinate line; a count above the points, a line without a
/// number, fewer coordinates, a line longer than 255 bytes, now and then; or no line at all.
fn ascii_patch(g: &mut Gen, nv: i64) -> Vec<u8> {
    let mut s = String::new();
    if g.chance(12) {
        // comments and blank lines only (FS-1007)
        s += "# nothing\n\n   \n#!ascii\n";
        return s.into_bytes();
    }
    if g.chance(2) {
        s += "#!ascii version of patch lh.s0. The 1st index is not a vertex number\n";
    }
    let n = g.int(0, nv.min(12));
    let count = if g.chance(10) {
        n + g.int(1, 3)
    } else if g.chance(15) {
        -1
    } else {
        n
    };
    if g.chance(20) {
        s += "count follows\n";
    } else {
        s += &format!("{count} {}\n", g.int(0, 30));
    }
    for _ in 0..n {
        if g.chance(10) {
            s += "\n# a comment between points\n";
        }
        let k = point_number(g, nv);
        if g.chance(25) {
            s += "vno=missing\n";
        } else {
            s += &format!("{k}  vno={}\n", k.abs() - 1);
        }
        let c: Vec<String> = (0..3)
            .map(|_| {
                if g.chance(30) {
                    g.pick(&["nan", "inf", "-0", "1e40", "0x1p-3"]).to_owned()
                } else {
                    format!("{:.6}", g.int(-4000, 4000) as f64 / 64.0)
                }
            })
            .collect();
        match g.below(25) {
            0 => s += &format!("{}  {}\n", c[0], c[1]),
            1 => s += "x y z\n",
            2 => s += &format!("{}{}\n", " ".repeat(260), c[0]),
            _ => s += &format!("{}  {}  {}\n", c[0], c[1], c[2]),
        }
    }
    if g.chance(15) {
        let cut = g.int(0, s.len() as i64) as usize;
        s.truncate(cut);
    }
    s.into_bytes()
}

/// A binary patch, old (a count, then each point's number and three big-endian `short`s, in
/// hundredths of a millimetre) or new (-1, the count, each point's number and three big-endian
/// `float`s); a count above the points or a cut now and then.
fn binary_patch(g: &mut Gen, nv: i64) -> Vec<u8> {
    let mut b = Vec::new();
    let new = g.chance(2);
    let n = g.int(0, nv.min(12));
    let count = if g.chance(10) { n + g.int(1, 3) } else { n };
    if new {
        b.extend_from_slice(&(-1i32).to_be_bytes());
    }
    b.extend_from_slice(&(count as i32).to_be_bytes());
    for _ in 0..n {
        b.extend_from_slice(&(point_number(g, nv) as i32).to_be_bytes());
        for _ in 0..3 {
            if new {
                let f = if g.chance(25) {
                    g.pick(&[f32::NAN, f32::INFINITY, -0.0])
                } else {
                    g.int(-4000, 4000) as f32 / 64.0
                };
                b.extend_from_slice(&f.to_bits().to_be_bytes());
            } else {
                b.extend_from_slice(&(g.int(-32768, 32767) as i16).to_be_bytes());
            }
        }
    }
    if g.chance(12) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
    }
    b
}

/// A label file of the surface's vertices (now and then -1, the vertex count or beyond).
fn label_file(g: &mut Gen, nv: i64) -> Vec<u8> {
    let n = g.int(0, nv.min(10));
    let mut s = String::from("#!ascii label  , from subject  vox2ras=TkReg\n");
    s += &format!("{n}\n");
    for _ in 0..n {
        let vno = if g.chance(15) {
            g.pick(&[-1, nv, nv + 4])
        } else {
            g.int(0, (nv - 1).max(0))
        };
        s += &format!(
            "{vno}  {:.3}  {:.3}  {:.3} {:.10}\n",
            g.int(-800, 800) as f64 / 8.0,
            g.int(-800, 800) as f64 / 8.0,
            g.int(-800, 800) as f64 / 8.0,
            g.int(0, 100) as f64 / 100.0
        );
    }
    s.into_bytes()
}

pub(super) fn ptc_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut m = match g.below(10) {
        0..=3 => {
            let r = g.pick(&[10.0, 50.0, 100.0]);
            let mut m = ico(g, 1, r);
            if g.chance(4) {
                crumple(g, &mut m, 3);
            }
            m
        }
        4 | 5 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        _ => mesh(g, heavy),
    };
    if heavy && g.chance(4) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = f32::NAN;
    }
    surface(t, &m);
    let nv = m.v.len() as i64;
    ix(t, if g.chance(5) { 1 } else { 0 });
    ix(t, g.pick(&[-1, -1, -1, 0, 2, nv - 1, nv + 3]));
    ix(t, if g.chance(6) { 1 } else { 0 });
    let nin = if g.chance(8) { 0 } else { g.int(1, 3) };
    ix(t, nin);
    let mut known: Vec<&str> = Vec::new();
    for _ in 0..nin {
        let n = g.pick(&NAMES[..10]);
        // the file's place: `d/lh.<name>` for a bare name (MRISbuildFileName_read's), else the name
        let place = if n.contains('/') || n.starts_with("lh.") {
            if n.contains('/') {
                n.to_owned()
            } else {
                format!("d/{n}")
            }
        } else {
            format!("d/lh.{n}")
        };
        let b = file_for(g, n, &m, heavy);
        hx(t, place.as_bytes());
        hx(t, &b);
        known.push(n);
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(12) {
            0 => {
                t.push("rip".into());
                ix(t, g.int(0, (nv - 1).max(0)));
            }
            1 => {
                t.push("bd".into());
                ix(t, g.int(0, (nv - 1).max(0)));
            }
            2 => {
                t.push("jit".into());
                fv(t, g.pick(&[0.25, 0.5, 1.0]));
            }
            3 | 4 => {
                t.push("wp".into());
                let n = g.pick(&["d/lh.patch", "d/lh.p.asc", "d/lh.w.patch", "d/lh.p.gii"]);
                hx(t, n.as_bytes());
            }
            _ => {
                t.push(if g.chance(2) { "nr" } else { "rp" }.into());
                let n = if !known.is_empty() && !g.chance(4) {
                    known[g.below(known.len() as u64) as usize]
                } else {
                    g.pick(&NAMES)
                };
                hx(t, n.as_bytes());
                ix(t, if g.chance(4) { 0 } else { 1 });
            }
        }
    }
}

fn dump(s: &Mris, o: &mut Out) {
    o.i(i64::from(s.nvertices));
    o.i(i64::from(s.nfaces));
    o.i(i64::from(s.patch));
    o.i(i64::from(s.status.0));
    o.f(s.total_area);
    for x in [s.xlo, s.xhi, s.ylo, s.yhi, s.zlo, s.zhi] {
        o.f(x);
    }
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z, v.nx, v.ny, v.nz, v.area] {
            o.f(x);
        }
        o.i(i64::from(v.ripflag));
        o.i(i64::from(v.border));
        let tp = s.topology(vno);
        o.i(i64::from(tp.num));
        o.i(i64::from(tp.vnum));
    }
    for fno in 0..s.nfaces.max(0) as usize {
        let f = s.face(fno);
        o.i(i64::from(f.ripflag));
        o.f(f.area);
    }
}

pub(super) fn ptc_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
    if t.i() != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    cx.g.gdiag_no = t.i();
    let mut pairs = vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())];
    if t.i() != 0 {
        pairs.push(("FS_POSIX".to_owned(), "1".to_owned()));
    }
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
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
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        match op {
            "rip" => s.vertex_mut(t.i() as usize).ripflag = 1,
            "bd" => s.vertex_mut(t.i() as usize).border = 1,
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
            "wp" => {
                let n = utf8(t.h());
                o.i(i64::from(valfile::write_patch(g, &mut io, d, &mut s, &n)));
            }
            "nr" | "rp" => {
                let n = utf8(t.h());
                let tkr = t.i() != 0;
                let r = if op == "nr" {
                    patchread::read_patch_no_remove(g, sg, &mut io, d, &mut s, &n, tkr)?
                } else {
                    patchread::read_patch(g, sg, &mut io, d, &mut s, &n, tkr)?
                };
                o.i(i64::from(r));
            }
            _ => panic!("ptc op {op}"),
        }
        dump(&s, o);
    }
    Ok(())
}

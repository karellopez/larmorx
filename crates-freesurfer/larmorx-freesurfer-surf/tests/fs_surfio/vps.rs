// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 36 of track D (queue items 3.1-47/49): `vps`, another file's
//! vertex positions: `MRISreadVertexPositions` (quadrangle files of the three kinds, triangle,
//! movie.byu, icosahedron, GIFTI, STL, VTK and ASCII files of the same mesh, moved, or of another
//! mesh), `MRISreadWhiteCoordinates`, `MRISreadPialCoordinates`, `MRISreadCanonicalCoordinates`
//! and `MRISreadOriginalProperties`, on a surface read (`MRISread`) from a triangle file of the
//! mesh in a work folder `d`; each status, then the surface's coordinate sets and properties.

use super::sfile::utf8;
use super::srd::{Mesh, be4, mesh, write};
use super::*;

use larmorx_freesurfer_surf::{readwrite, vpos};

/// The leaves the inputs take after `d/lh.`, without their extension.
const LEAVES: [&str; 6] = ["sphere", "smoothwm", "pial", "white2", "orig", "x"];

/// The mesh moved: each coordinate shifted by a small multiple of 1/8, or now and then another
/// mesh (a size the surface does not have).
fn moved(g: &mut Gen, m: &Mesh, heavy: bool) -> Mesh {
    if g.chance(8) {
        return mesh(g, heavy);
    }
    Mesh {
        v: m.v
            .iter()
            .map(|p| p.map(|c| c + g.int(-16, 16) as f32 / 8.0))
            .collect(),
        f: m.f.clone(),
    }
}

/// The surface's own file: a binary triangle file of the mesh (without the creator's line when
/// `TRIANGULARSURFACE_NOEXTRA_READ` is set: with it, `MRISread` would take the line's bytes as
/// the counts, and FreeSurfer would spend its 5 seconds making a surface of a billion vertices,
/// as the `srd` command's cases already show).
fn triangle(m: &Mesh, noextra: bool) -> Vec<u8> {
    let mut b = vec![0xff, 0xff, 0xfe];
    if !noextra {
        b.extend_from_slice(b"created by me on Thu Nov 12 20:55:19 2026\n\n");
    }
    be4(&mut b, m.v.len() as i64);
    be4(&mut b, m.f.len() as i64);
    for p in &m.v {
        for c in p {
            b.extend_from_slice(&c.to_be_bytes());
        }
    }
    for f in &m.f {
        for &x in f {
            be4(&mut b, x);
        }
    }
    b
}

/// The surface's own file as a quadrangle file of the new format (floats), each triangle
/// written as the quadrangle of its vertices and its third vertex again.
fn quadrangle(m: &Mesh) -> Vec<u8> {
    let be3 = |b: &mut Vec<u8>, v: i64| b.extend_from_slice(&(v as u32).to_be_bytes()[1..]);
    let mut b = vec![0xff, 0xff, 0xfd];
    be3(&mut b, m.v.len() as i64);
    be3(&mut b, m.f.len() as i64);
    for p in &m.v {
        for c in p {
            b.extend_from_slice(&c.to_be_bytes());
        }
    }
    for f in &m.f {
        for x in [f[0], f[1], f[2], f[2]] {
            be3(&mut b, x);
        }
    }
    b
}

pub(super) fn vps_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, if g.chance(4) { g.int(1, 3) } else { 0 });
    let mut envs = 0;
    if g.chance(6) {
        envs |= 1;
    }
    if g.chance(10) {
        envs |= 2;
    }
    if g.chance(10) {
        envs |= 4;
    }
    ix(t, envs);
    let m = mesh(g, heavy);
    // now and then a quadrangle file (each triangle a quadrangle: MRISread makes two faces of
    // each), so that the quadrangle files of the same mesh match the surface's face count
    if g.chance(3) {
        hx(t, &quadrangle(&m));
    } else {
        hx(t, &triangle(&m, envs & 4 != 0));
    }
    let nin = g.int(1, 4);
    ix(t, nin);
    let mut known: Vec<String> = Vec::new();
    for _ in 0..nin {
        let m2 = moved(g, &m, heavy);
        let (ext, b) = write(g, &m2, heavy);
        let leaf = g.pick(&LEAVES);
        let n = format!("d/lh.{leaf}{ext}");
        hx(t, n.as_bytes());
        hx(t, &b);
        known.push(n);
        known.push(format!("{leaf}{ext}"));
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        let op = g.pick(&["V", "V", "W", "P", "C", "O", "O"]);
        t.push(op.to_owned());
        let n = if !g.chance(5) {
            Some(known[g.below(known.len() as u64) as usize].clone())
        } else {
            g.pick(&[
                None,
                Some("nodir/lh.x"),
                Some(""),
                Some("lh.white"),
                Some("d/lh.none.tri"),
                Some("white"),
            ])
            .map(str::to_owned)
        };
        match n {
            None => ix(t, 0),
            Some(n) => {
                ix(t, 1);
                hx(t, n.as_bytes());
            }
        }
    }
}

pub(super) fn vps_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    std::fs::create_dir(dir.join("d")).expect("a folder");
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let envs = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if envs & 1 != 0 {
        pairs.push(("FS_GII".into(), ".gii".into()));
    }
    if envs & 2 != 0 {
        pairs.push(("FS_FASTER_MP".into(), "1".into()));
    }
    if envs & 4 != 0 {
        pairs.push(("TRIANGULARSURFACE_NOEXTRA_READ".into(), "1".into()));
    }
    pairs.push(("SURFER_SIDEDOOR".into(), "1".into()));
    pairs.push(("USER".into(), "U".into()));
    let env = Environment::from_pairs(pairs);
    std::fs::write(dir.join("d/lh.white"), t.h()).expect("the surface");
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let d = Some(dir);
    let mut m = {
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut io = coords::Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        readwrite::read(&mut cx.g, &mut cx.sg, &mut io, d, "d/lh.white", true)?
    };
    let Some(m) = m.as_mut() else {
        o.s("NOSURFACE");
        return Ok(());
    };
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let n = (t.i() != 0).then(|| utf8(t.h()));
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut io = coords::Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        let (g, sg) = (&mut cx.g, &mut cx.sg);
        let nm = n.as_deref();
        let r = match op {
            "V" => vpos::read_vertex_positions(g, sg, &mut io, d, m, nm.unwrap_or(""))?,
            "W" => vpos::read_white_coordinates(g, sg, &mut io, d, m, nm)?,
            "P" => vpos::read_pial_coordinates(g, sg, &mut io, d, m, nm)?,
            "C" => vpos::read_canonical_coordinates(g, sg, &mut io, d, m, nm.unwrap_or(""))?,
            "O" => vpos::read_original_properties(g, sg, &mut io, d, m, nm)?,
            _ => panic!("vps op {op}"),
        };
        o.i(i64::from(r));
    }
    o.0 += "| ";
    for vno in 0..m.nvertices.max(0) as usize {
        let v = m.vertex(vno);
        for x in [
            v.x, v.y, v.z, v.whitex, v.whitey, v.whitez, v.pialx, v.pialy, v.pialz, v.cx, v.cy,
            v.cz, v.origx, v.origy, v.origz, v.theta, v.phi, v.area, v.origarea, v.nx, v.ny, v.nz,
        ] {
            o.f(x);
        }
    }
    o.i(i64::from(m.status.0));
    o.f(m.total_area);
    o.f(m.orig_area);
    o.d(m.radius);
    o.i(i64::from(m.nvertices));
    o.i(i64::from(m.nfaces));
    Ok(())
}

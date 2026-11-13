// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io_stl.cpp` (`fs.surfio`, queue item 3.1-49): STL surfaces, read
//! (`mrisReadSTL`, binary or text) and written (`mrisWriteSTL`, binary).
//!
//! An STL file holds triangles only; the reader numbers the distinct corner points in the order
//! they first appear (two points are one vertex when their three coordinates compare equal as
//! floats: `-0` is `+0`, a NaN never equals anything), keeps each vertex's first coordinates,
//! stores each facet's normal in the face-normal cache, and builds the vertices' face and
//! neighbour lists (`num`, `vnum`, `v`, `f`, `n`, `vtotal`). FreeSurfer finds the points through a
//! hash table of 2^20 chains indexed by `pointHash` (`1000 * logf(|c| + 1)` per coordinate, as
//! integers, mixed by multiplication and exclusive or, modulo 2^20); the chain a point lands in
//! never changes which vertex it is, so the port uses its own map, and computes FreeSurfer's index
//! only to note FS-158 (a negative index: memory before the table).
//!
//! The defects: FS-151 and FS-152 (a text line's numbers that cannot be read: the values read
//! before, memory contents for the first), FS-157 (a vertex of more than 255 faces: its face lists
//! written past their allocation), FS-158 (a NaN or infinite coordinate looked up outside the hash
//! table), FS-159 (a facet of fewer than three vertices: a segmentation fault), FS-160 (a binary
//! file whose triangle count times 50 wraps around 2^32 to its size).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::collections::HashMap;
use std::path::Path;

use larmorx_core::defects::{FS_151, FS_152, FS_157, FS_158, FS_159, FS_160};
use larmorx_core::math;
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_io::cfile::CFile;

use crate::aaa::{SurfGlobals, VERTICES_PER_FACE};
use crate::alloc;
use crate::coords::{self, Io};
use crate::curvio::fgetl_line;
use crate::facenorm;
use crate::formats::MRIS_TRIANGULAR_SURFACE;
use crate::mris::Mris;
use crate::scanf::Scanf;
use crate::sfile::{STRLEN, c_str, nonneg, open};
use crate::topology;

/// `hashTableSize`: the hash table's chains.
const HASH_TABLE_SIZE: i32 = 1024 * 1024;

fn stl_cpp() -> String {
    format!("{}/utils/mrisurf_io_stl.cpp", diag::BUILD_ROOT)
}

/// x86-64's `cvttss2si`: a float to `int` toward zero; NaN and values outside the range give
/// `INT_MIN`.
fn cvttss2si(x: f32) -> i32 {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

/// `pointHash(x, y, z)` as the official build computes it (glibc's `logf`, the float sum and
/// product, `cvttss2si`, wrapping products, C's remainder): the chain FreeSurfer looks the point
/// up in; negative for one or three non-finite coordinates (unless the remainder is 0).
fn point_hash(x: f32, y: f32, z: f32) -> i32 {
    let h = |c: f32| cvttss2si(1000.0f32 * math::logf_glibc(c.abs() + 1.0f32));
    let hash = h(x).wrapping_mul(2017) ^ h(y).wrapping_mul(1029) ^ h(z).wrapping_mul(1159);
    hash % HASH_TABLE_SIZE
}

/// A coordinate as the map compares it: `-0` as `+0` (NaNs never reach the map).
fn key(c: f32) -> u32 {
    if c == 0.0 { 0 } else { c.to_bits() }
}

/// `STLMesher`: the vertices found, each occurrence's vertex, the facets' normals.
struct Mesher {
    /// Each vertex's first coordinates.
    points: Vec<[f32; 3]>,
    map: HashMap<[u32; 3], i32>,
    /// The vertex of every corner, in the file's order.
    corners: Vec<i32>,
    /// Each facet's normal and how many corners it has.
    normals: Vec<[f32; 3]>,
    counts: Vec<u8>,
    /// `faceVertexNo`: the corners of the current facet (3 before the first facet).
    face_vertex_no: i32,
}

impl Mesher {
    fn new() -> Mesher {
        Mesher {
            points: Vec::new(),
            map: HashMap::new(),
            corners: Vec::new(),
            normals: Vec::new(),
            counts: Vec::new(),
            face_vertex_no: 3,
        }
    }

    /// `addVertex(x, y, z)`: the point's vertex (a new one when no earlier point compares equal),
    /// then the facet's corner count checked: a fourth corner, or a corner before any facet, ends
    /// the program (`fs::fatal`: `STL file does not appear to represent a triangular mesh`).
    fn add_vertex(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<(), Fatal> {
        if point_hash(x, y, z) < 0 {
            g.notices.add(
                &FS_158,
                "an STL vertex with a NaN or infinite coordinate is looked up as any other (FreeSurfer reads memory before its hash table)",
            );
        }
        let vno = if x.is_nan() || y.is_nan() || z.is_nan() {
            None
        } else {
            self.map.get(&[key(x), key(y), key(z)]).copied()
        };
        let vno = vno.unwrap_or_else(|| {
            let n = self.points.len() as i32;
            self.points.push([x, y, z]);
            if !(x.is_nan() || y.is_nan() || z.is_nan()) {
                self.map.insert([key(x), key(y), key(z)], n);
            }
            n
        });
        let before = self.face_vertex_no;
        self.face_vertex_no += 1;
        if before > 2 {
            log::fatal(
                g,
                io.err,
                1,
                "STL file does not appear to represent a triangular mesh",
            )?;
        }
        self.corners.push(vno);
        if let Some(c) = self.counts.last_mut() {
            *c += 1;
        }
        Ok(())
    }

    /// `addFaceNormal(x, y, z)`: a new facet with this normal.
    fn add_face_normal(&mut self, x: f32, y: f32, z: f32) {
        self.normals.push([x, y, z]);
        self.counts.push(0);
        self.face_vertex_no = 0;
    }

    /// `buildSurface()`: `MRISalloc(vertices, facets)`, type `MRIS_TRIANGULAR_SURFACE`; each
    /// facet's normal into the face-normal cache, its corners' vertices and their first
    /// coordinates; the face and neighbour lists. `Ok(None)` when the correction refuses a facet
    /// of fewer than three corners (FS-159; its error printed).
    fn build_surface(
        self,
        g: &mut Globals,
        sg: &mut SurfGlobals,
        io: &mut Io,
        filename: &str,
    ) -> Result<Option<Mris>, Fatal> {
        let nfaces = self.normals.len();
        if let Some(short) = self.counts.iter().position(|&c| c < 3) {
            g.notices.add(
                &FS_159,
                format!("{filename}: an STL facet of fewer than three corners is refused (FreeSurfer crashes)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            log::error(
                g,
                io.err,
                &format!("STL file {filename}: facet {short} has fewer than three vertices"),
            );
            return Ok(None);
        }
        let mut mris = alloc::alloc(
            g,
            io.out,
            io.err,
            sg,
            self.points.len() as i32,
            nfaces as i32,
        )?;
        mris.r#type = MRIS_TRIANGULAR_SURFACE;
        let file = stl_cpp();
        let mut nv = 0;
        for fno in 0..nfaces {
            let [x, y, z] = self.normals[fno];
            facenorm::set_face_norm(&mut mris, fno, x, y, z);
            for k in 0..VERTICES_PER_FACE {
                let vno = self.corners[3 * fno + k];
                if vno == nv {
                    let [px, py, pz] = self.points[vno as usize];
                    coords::set_xyz(g, io, &mut mris, vno, px, py, pz, &file, 124)?;
                    nv += 1;
                }
                mris.face_mut(fno).v[k] = vno;
            }
        }
        let mut wrapped = false;
        for fno in 0..nfaces {
            for fvno in 0..VERTICES_PER_FACE {
                let v = mris.face(fno).v[fvno] as usize;
                let t = mris.topology_mut(v);
                if t.num == u8::MAX {
                    g.notices.add(
                        &FS_157,
                        format!("{filename}: an STL vertex of more than 255 faces is refused (FreeSurfer writes its faces past their lists)"),
                    );
                    if !g.notices.reproduce() {
                        log::error(
                            g,
                            io.err,
                            &format!("STL file {filename}: vertex {v} is in more than 255 faces"),
                        );
                        return Ok(None);
                    }
                    wrapped = true;
                }
                let t = mris.topology_mut(v);
                t.num = t.num.wrapping_add(1);
                topology::add_vnum(g, sg, io.err, &mut mris, v, 2);
            }
        }
        for vno in 0..nonneg(mris.nvertices) {
            let vnum = mris.topology(vno).vnum;
            if vnum < 0 {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_NOMEMORY,
                    &format!("MRISreadSTLfile: could not allocate {vno}th vertex list."),
                )?;
            }
            mris.topology_mut(vno).v = Some(vec![0; vnum as usize]);
            topology::clear_vnum(g, sg, io.err, &mut mris, vno);
        }
        for fno in 0..nfaces {
            for fvno in 0..VERTICES_PER_FACE {
                let v = mris.face(fno).v[fvno] as usize;
                for fvno2 in 0..VERTICES_PER_FACE {
                    if fvno2 == fvno {
                        continue;
                    }
                    let vn = mris.face(fno).v[fvno2];
                    let t = mris.topology(v);
                    let list = t.v.as_deref().unwrap_or(&[]);
                    let dup = (0..nonneg(i32::from(t.vnum))).any(|n2| list.get(n2) == Some(&vn));
                    if !dup && vn >= 0 {
                        let at = topology::vnum_add(g, sg, io.err, &mut mris, v, 1) as usize;
                        if let Some(slot) =
                            mris.topology_mut(v).v.as_mut().and_then(|l| l.get_mut(at))
                        {
                            *slot = vn;
                        }
                    }
                }
            }
        }
        // the face lists: with a wrapped count (FS-157 reproduced) the whole lists, where
        // FreeSurfer allocates the wrapped count and writes past it
        let mut faces_of = vec![0usize; nonneg(mris.nvertices)];
        for fno in 0..nfaces {
            for fvno in 0..VERTICES_PER_FACE {
                faces_of[mris.face(fno).v[fvno] as usize] += 1;
            }
        }
        for vno in 0..nonneg(mris.nvertices) {
            let total = if wrapped {
                faces_of[vno]
            } else {
                usize::from(mris.topology(vno).num)
            };
            let t = mris.topology_mut(vno);
            t.f = Some(vec![0; total]);
            t.n = Some(vec![0; total]);
            t.num = 0;
            t.vtotal = t.vnum;
        }
        for fno in 0..nfaces {
            for fvno in 0..VERTICES_PER_FACE {
                let v = mris.face(fno).v[fvno] as usize;
                let t = mris.topology_mut(v);
                let at = usize::from(t.num);
                if let Some(n) = t.n.as_mut().and_then(|l| l.get_mut(at)) {
                    *n = fvno as u8;
                }
                if let Some(f) = t.f.as_mut().and_then(|l| l.get_mut(at)) {
                    *f = fno as i32;
                }
                t.num = t.num.wrapping_add(1);
            }
        }
        Ok(Some(mris))
    }
}

/// The values one text line's `sscanf` assigned into the reader's three stack slots (shared by
/// the vertex and the facet lines): each kept from the line before where this one could not read
/// it (FS-152), memory contents where no line read it yet (FS-151: 0 here). `None` when the
/// correction refuses the line.
fn line_values(
    g: &mut Globals,
    slots: &mut [Option<f32>; 3],
    cp: &[u8],
    head: &[u8],
    filename: &str,
) -> Option<[f32; 3]> {
    let mut b = Bytes::new(cp);
    let mut sc = Scanf::new(&mut b);
    let e = &mut g.errno;
    sc.text(e, head);
    let mut short = false;
    for k in 0..3 {
        if k > 0 {
            sc.text(e, b" ");
        }
        match sc.f(e, true) {
            Some(v) => slots[k] = Some(v),
            None => short = true,
        }
    }
    sc.finish(e);
    if short {
        if slots.iter().any(Option::is_none) {
            g.notices.add(
                &FS_151,
                format!("{filename}: an STL line's numbers that cannot be read are refused (FreeSurfer takes memory contents; 0 here)"),
            );
        } else {
            g.notices.add(
                &FS_152,
                format!("{filename}: an STL line's numbers that cannot be read are refused (FreeSurfer keeps the ones read before)"),
            );
        }
        if !g.notices.reproduce() {
            return None;
        }
    }
    Some([
        slots[0].unwrap_or(0.0),
        slots[1].unwrap_or(0.0),
        slots[2].unwrap_or(0.0),
    ])
}

/// The correction's error for a line [`line_values`] refused.
fn unreadable_line(g: &Globals, io: &mut Io, filename: &str, line: &[u8]) {
    let mut m =
        format!("ASCII STL file {filename}: could not read the numbers of line ").into_bytes();
    m.extend_from_slice(line);
    error_bytes(g, io, &m);
}

/// `fs::error() << message` of a message that holds a line's bytes.
fn error_bytes(g: &Globals, io: &mut Io, m: &[u8]) {
    let _ = io.err.write_all(b"error: ");
    let _ = io.err.write_all(m);
    let _ = io.err.write_all(b"\n");
    log::write_to_error_log(g, m);
}

/// `mrisReadAsciiSTL(filename)`: lines (`fgetl`) told apart by their first character: `v`
/// (`vertex %f %f %f`, a corner), `f` (`facet normal %f %f %f`, a new facet), `s` (`solid`; a
/// second one: `STL file <file> contains multiple solids!`, an error, `None`), `e` and `o` (passed
/// over); any other: `ASCII STL file <file> contains unrecognized line: <line>` (an error, `None`).
/// A number a line cannot read keeps the one read before (FS-151, FS-152; refused by default:
/// `ASCII STL file <file>: could not read the numbers of line <line>`, an error, `None`).
fn read_ascii(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    bytes: Vec<u8>,
    filename: &str,
) -> Result<Option<Mris>, Fatal> {
    let mut fp = CFile::from_bytes(bytes);
    let mut mesher = Mesher::new();
    let mut solids = 0;
    let mut slots = [None; 3];
    while let Some((line, at)) = fgetl_line(&mut fp, STRLEN) {
        let cp = &line[at..];
        match cp[0] {
            b'v' | b'f' => {
                let head: &[u8] = if cp[0] == b'v' {
                    b"vertex "
                } else {
                    b"facet normal "
                };
                let Some([x, y, z]) = line_values(g, &mut slots, cp, head, filename) else {
                    unreadable_line(g, io, filename, &line);
                    return Ok(None);
                };
                if cp[0] == b'v' {
                    mesher.add_vertex(g, io, x, y, z)?;
                } else {
                    mesher.add_face_normal(x, y, z);
                }
            }
            b's' => {
                solids += 1;
                if solids > 1 {
                    log::error(
                        g,
                        io.err,
                        &format!("STL file {filename} contains multiple solids!"),
                    );
                    return Ok(None);
                }
            }
            b'e' | b'o' => {}
            _ => {
                let mut m =
                    format!("ASCII STL file {filename} contains unrecognized line: ").into_bytes();
                m.extend_from_slice(&line);
                error_bytes(g, io, &m);
                return Ok(None);
            }
        }
    }
    mesher.build_surface(g, sg, io, filename)
}

fn le_f32(b: &[u8], at: usize) -> Option<f32> {
    b.get(at..at + 4)
        .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// `mrisReadBinarySTL(filename)`: the 80-byte header, the triangle count (a little-endian
/// `unsigned int`), per triangle the normal and three corners (little-endian floats) and two
/// bytes not used.
fn read_binary(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    bytes: &[u8],
    ntriangles: u32,
    filename: &str,
) -> Result<Option<Mris>, Fatal> {
    let mut mesher = Mesher::new();
    let mut at = 84;
    let mut past = false;
    let mut next = |at: &mut usize| {
        let v = le_f32(bytes, *at);
        *at += 4;
        if v.is_none() {
            past = true;
        }
        v.unwrap_or(0.0)
    };
    for _ in 0..ntriangles {
        let (x, y, z) = (next(&mut at), next(&mut at), next(&mut at));
        mesher.add_face_normal(x, y, z);
        for _ in 0..3 {
            let (x, y, z) = (next(&mut at), next(&mut at), next(&mut at));
            mesher.add_vertex(g, io, x, y, z)?;
        }
        at += 2;
    }
    if past {
        // only a count whose product with 50 wrapped around (FS-160, refused before)
        g.notices.add(
            &FS_160,
            format!("{filename}: the triangles past the file's end read as 0 (FreeSurfer's are memory contents)"),
        );
    }
    mesher.build_surface(g, sg, io, filename)
}

/// `mrisReadSTL(filename)`: the file's surface, or `None` (no message) when it cannot be opened.
/// Binary when the file's size is `84 + 50 * count` (in 32 bits) for the count its bytes 80 to 83
/// give (a file of fewer than 84 bytes never is: the stream fails and its size reads as 2^32 - 1),
/// text otherwise. FS-160: a count whose product with 50 wraps around to the file's size is
/// refused by default (`STL file <file>: <n> triangles do not fit in its <size> bytes`, an error).
pub fn read_stl(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    filename: &str,
) -> Result<Option<Mris>, Fatal> {
    let filename = c_str(filename);
    let Ok(mut fp) = open(dir, filename, "rb") else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = fp.read_bytes(&mut buf);
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    fp.close();
    let (binary, ntriangles) = if bytes.len() < 84 {
        (false, 0)
    } else {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        let predicted = 84u32.wrapping_add(n.wrapping_mul(50));
        (predicted == bytes.len() as u32, n)
    };
    if binary {
        if u64::from(ntriangles) * 50 + 84 != bytes.len() as u64 {
            g.notices.add(
                &FS_160,
                format!("{filename}: a binary STL count whose product with 50 wraps around is refused (FreeSurfer reads past the file)"),
            );
            if !g.notices.reproduce() {
                log::error(
                    g,
                    io.err,
                    &format!(
                        "STL file {filename}: {ntriangles} triangles do not fit in its {} bytes",
                        bytes.len()
                    ),
                );
                return Ok(None);
            }
        }
        read_binary(g, sg, io, &bytes, ntriangles, filename)
    } else {
        read_ascii(g, sg, io, bytes, filename)
    }
}

/// `mrisWriteSTL(mris, filename)`: a binary STL file: an 80-byte header (`freesurfer mesh`, then
/// zeros), the face count, per face its cached normal (`getFaceNorm`: a deferred one computed)
/// and its three vertices' coordinates (little-endian floats) and the two bytes `0` and NUL.
/// `false` (no message) when the file cannot be created.
pub fn write_stl(dir: Option<&Path>, mris: &mut Mris, filename: &str) -> bool {
    let filename = c_str(filename);
    let Ok(mut fp) = open(dir, filename, "wb") else {
        return false;
    };
    let mut b = Vec::with_capacity(84 + 50 * nonneg(mris.nfaces));
    let mut header = [0u8; 80];
    header[..15].copy_from_slice(b"freesurfer mesh");
    b.extend_from_slice(&header);
    b.extend_from_slice(&(mris.nfaces as u32).to_le_bytes());
    for fno in 0..nonneg(mris.nfaces) {
        let n = facenorm::get_face_norm(mris, fno);
        for c in [n.nx, n.ny, n.nz] {
            b.extend_from_slice(&c.to_le_bytes());
        }
        for k in 0..VERTICES_PER_FACE {
            let v = mris.vertex(mris.face(fno).v[k] as usize);
            for c in [v.x, v.y, v.z] {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        b.extend_from_slice(b"0\0");
    }
    fp.put_bytes(&b);
    fp.close();
    true
}

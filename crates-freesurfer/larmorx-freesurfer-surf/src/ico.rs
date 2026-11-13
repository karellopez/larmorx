// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The icosahedron files (fs.surf, wave 3.4, queue item 3.4's icosahedra, taken early for
//! `MRISread` of a `.tri` or `.ico` name, queue item 3.1-48, relay 16): FreeSurfer 8.2.0's
//! `utils/icosahedron.cpp` `read_icosahedron` (lines 1866-1938), `ICOtoScaledMRIS` and `ICOtoMRIS`
//! (1715-1823), `ICOreadOverAlloc` and `ICOread` (1826-1845). FS-361, FS-362.

#![allow(clippy::needless_range_loop)]

use std::path::Path;

use larmorx_core::defects::{FS_361, FS_362};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::{double_to_int_x86, is_space};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::cfile::CFile;

use crate::aaa::SurfGlobals;
use crate::alloc;
use crate::coords::{self, Io};
use crate::metric;
use crate::mp;
use crate::mris::Mris;
use crate::nbhd::set_nsize_cur;
use crate::sfile::{c_str, open};
use crate::topology::{clear_vnum, vnum_add};

/// `MRIS_ICO_SURFACE` (`mrisurf.h` line 1543): the type of a surface made from an icosahedron.
pub const MRIS_ICO_SURFACE: i32 = 3;

/// `IC_VERTEX`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `IC_FACE`: the vertex numbers (from 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcFace {
    pub vno: [i32; 3],
}

/// `ICOSAHEDRON`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Icosahedron {
    pub nvertices: i32,
    pub nfaces: i32,
    pub vertices: Vec<IcVertex>,
    pub faces: Vec<IcFace>,
}

fn icosahedron_cpp() -> String {
    format!("{}/utils/icosahedron.cpp", diag::BUILD_ROOT)
}

/// `fgetl(line, n, fp)` with the caller's buffer `line` kept as FreeSurfer's: every `fgets` writes
/// it (a skipped empty, blank or comment line too), the line returned has every `#` cut and its
/// newline removed; at the end of the file the buffer keeps what it held (`None`: never written).
/// The start of the returned line in the buffer (after its leading white space), or `None`.
fn fgetl_buf(fp: &mut CFile, line: &mut Option<Vec<u8>>, n: usize) -> Option<usize> {
    loop {
        let raw = fp.fgets(n)?;
        let s = raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())].to_vec();
        let start = s.iter().position(|&b| !is_space(b)).unwrap_or(s.len());
        let skip = start == s.len() || s[start] == b'#' || s[start] == b'\n';
        *line = Some(s);
        if skip {
            continue;
        }
        let s = line.as_mut().expect("the line");
        if let Some(i) = s[start..].iter().position(|&b| b == b'#') {
            s.truncate(start + i);
        }
        if s.last() == Some(&b'\n') {
            s.pop();
        }
        return Some(start);
    }
}

/// `sscanf(line, "%d", &n)`: the number, or `None` (nothing assigned).
fn scan_count(g: &mut Globals, line: &[u8]) -> Option<i32> {
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    let e = &mut g.errno;
    let v = sc.d(e, true);
    sc.finish(e);
    v
}

/// FS-362 met: the default refuses the file with `read_icosahedron(<file>): <what>`
/// (`ERROR_BADFILE`) and returns `true`; `reproduce` goes on (`false`).
fn bad_file(g: &mut Globals, io: &mut Io, fname: &str, what: &str) -> bool {
    g.notices.add(
        &FS_362,
        format!("{fname}: {what} (FreeSurfer reads or writes outside its arrays)"),
    );
    if g.notices.reproduce() {
        return false;
    }
    log::error_printf(
        g,
        io.err,
        ERROR_BADFILE,
        &format!("read_icosahedron({fname}): {what}"),
    );
    true
}

/// A count FreeSurfer's `calloc` takes: a negative one fails (`read_ico: could not allocate
/// vertex list`, the fatal error `ERROR_NOMEMORY`, for the faces too).
fn checked_count(g: &mut Globals, io: &mut Io, n: i32) -> Result<usize, Fatal> {
    if n < 0 {
        // calloc's failure: ENOMEM
        g.errno = 12;
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            "read_ico: could not allocate vertex list",
        )?;
    }
    Ok(n.max(0) as usize)
}

/// `read_icosahedron(fname)`: the file's text: a line (`fgetl` of 150 bytes, comments and blank
/// lines passed over) whose first number (`%d`) is the vertex count; vertex lines, each either
/// `vno x y z` (4 numbers; the vertex number from 1 as a float, truncated) or `x y z nx ny nz`
/// (6 numbers, the vertices in order), until a line of another count or the vertex count read;
/// a line whose first number is the face count (`fgetl`, its number read from the buffer, which
/// at the end of the file still holds the line read before); face lines `fno v1 v2 v3` (4
/// numbers) or `v1 v2 v3` (3, the faces in order), until a line of another count or the face
/// count read. Vertices and faces not read stay 0. Returns `None` after an error: the file cannot
/// be opened (`read_icosahedron: could not open <file>`, `ERROR_NOFILE`); FS-362 (refused by
/// default: a count that cannot be read, a vertex or face number outside the count). A negative
/// count is the fatal error `read_ico: could not allocate vertex list`.
pub fn read_icosahedron(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Icosahedron>, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("read_icosahedron: could not open {fname}"),
            );
            return Ok(None);
        }
    };
    let mut line: Option<Vec<u8>> = None;
    fgetl_buf(&mut fp, &mut line, 150);
    let nvertices = match line.clone().and_then(|l| scan_count(g, &l)) {
        Some(n) => n,
        None => {
            if bad_file(g, io, fname, "the vertex count cannot be read") {
                fp.close();
                return Ok(None);
            }
            0
        }
    };
    let mut ico = Icosahedron {
        nvertices,
        ..Icosahedron::default()
    };
    ico.vertices = vec![IcVertex::default(); checked_count(g, io, nvertices)?];
    let mut n: i32 = 0;
    while let Some(start) = fgetl_buf(&mut fp, &mut line, 150) {
        let cp = line.as_ref().expect("the line")[start..].to_vec();
        let mut v = [0f32; 6];
        let count = {
            let mut b = Bytes::new(&cp);
            let mut sc = Scanf::new(&mut b);
            let e = &mut g.errno;
            for (k, x) in v.iter_mut().enumerate() {
                if k > 0 {
                    sc.text(e, b" ");
                }
                if let Some(f) = sc.f(e, true) {
                    *x = f;
                }
            }
            sc.text(e, b"\n");
            sc.finish(e)
        };
        let (vno, x, y, z) = match count {
            4 => (double_to_int_x86(f64::from(v[0])), v[1], v[2], v[3]),
            6 => (n.wrapping_add(1), v[0], v[1], v[2]),
            _ => break,
        };
        let index = vno.wrapping_sub(1);
        if index < 0 || index >= nvertices {
            if bad_file(
                g,
                io,
                fname,
                &format!("vertex number {vno} is outside the {nvertices} vertices"),
            ) {
                fp.close();
                return Ok(None);
            }
        } else {
            ico.vertices[index as usize] = IcVertex { x, y, z };
        }
        n += 1;
        if n >= nvertices {
            break;
        }
    }
    n = 0;
    fgetl_buf(&mut fp, &mut line, 150);
    let nfaces = match line.clone().and_then(|l| scan_count(g, &l)) {
        Some(n) => n,
        None => {
            if bad_file(g, io, fname, "the face count cannot be read") {
                fp.close();
                return Ok(None);
            }
            0
        }
    };
    ico.nfaces = nfaces;
    ico.faces = vec![IcFace::default(); checked_count(g, io, nfaces)?];
    while let Some(start) = fgetl_buf(&mut fp, &mut line, 150) {
        let cp = line.as_ref().expect("the line")[start..].to_vec();
        let mut d = [0i32; 4];
        let count = {
            let mut b = Bytes::new(&cp);
            let mut sc = Scanf::new(&mut b);
            let e = &mut g.errno;
            for (k, x) in d.iter_mut().enumerate() {
                if k > 0 {
                    sc.text(e, b" ");
                }
                if let Some(i) = sc.d(e, true) {
                    *x = i;
                }
            }
            sc.text(e, b"\n");
            sc.finish(e)
        };
        let (fno, vno) = match count {
            3 => (n.wrapping_add(1), [d[0], d[1], d[2]]),
            4 => (d[0], [d[1], d[2], d[3]]),
            _ => break,
        };
        let index = fno.wrapping_sub(1);
        if index < 0 || index >= nfaces {
            if bad_file(
                g,
                io,
                fname,
                &format!("face number {fno} is outside the {nfaces} faces"),
            ) {
                fp.close();
                return Ok(None);
            }
        } else {
            ico.faces[index as usize] = IcFace { vno };
        }
        n += 1;
        if n >= nfaces {
            break;
        }
    }
    fp.close();
    Ok(Some(ico))
}

/// A vertex's face list or neighbour list written at `i`: past its end (FS-361) the list grows,
/// as if the memory after it were its own (the default and `reproduce` alike; FreeSurfer writes
/// into whatever follows it).
fn put(g: &mut Globals, list: &mut Vec<i32>, i: usize, x: i32, vno: usize, what: &str) {
    if i >= list.len() {
        g.notices.add(
            &FS_361,
            format!(
                "vertex {vno}: its {what} grows past its {} entries",
                list.len()
            ),
        );
        list.resize(i + 1, 0);
    }
    list[i] = x;
}

/// `ICOtoScaledMRIS(ico, max_vertices, max_faces, scaleFactor)`: each face's second and third
/// vertices swapped (in the icosahedron too); a surface (`MRISoverAlloc`) with the vertices
/// scaled by `scale_factor` (float products); each face's vertices from 0; the neighbour lists
/// (room for half of twice the faces counted, each face's other two vertices appended when not
/// yet listed); the face lists; sizes 1; `MRIScomputeMetricProperties`; `radius`
/// `MRISaverageRadius`; type `MRIS_ICO_SURFACE`. FS-361 (a surface that is not closed: a list
/// grows; a vertex of more than 255 faces: refused by default with `ICOread: vertex <v> is in
/// more than 255 faces`, its count wraps around with `reproduce`); FS-362 (a face vertex that is
/// not one of the vertices: refused by default with `read_icosahedron(<file>): face <f> vertex
/// <v> is not a vertex of the surface`, `reproduce` ends as a segmentation fault where it is
/// beyond the surface's room). `None` after a refusal.
#[allow(clippy::too_many_arguments)]
pub fn ico_to_scaled_mris(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    ico: &mut Icosahedron,
    max_vertices: i32,
    max_faces: i32,
    scale_factor: f32,
    fname: &str,
) -> Result<Option<Mris>, Fatal> {
    for f in &mut ico.faces {
        f.vno.swap(1, 2);
    }
    let nv = ico.nvertices;
    let mut mris = alloc::over_alloc(
        g,
        io.out,
        io.err,
        sg,
        max_vertices,
        max_faces,
        nv,
        ico.nfaces,
    )?;
    let file = icosahedron_cpp();
    for vno in 0..nv {
        let p = ico.vertices[vno as usize];
        coords::set_xyz(
            g,
            io,
            &mut mris,
            vno,
            sse::mul(scale_factor, p.x),
            sse::mul(scale_factor, p.y),
            sse::mul(scale_factor, p.z),
            &file,
            1741,
        )?;
    }
    let room = mris.vertices_topology.as_ref().map_or(0, Vec::len) as i32;
    let nf = ico.nfaces.max(0) as usize;
    for fno in 0..nf {
        for n in 0..3 {
            let v = ico.faces[fno].vno[n].wrapping_sub(1);
            mris.face_mut(fno).v[n] = v;
            if v < 0 || v >= nv {
                g.notices.add(
                    &FS_362,
                    format!(
                        "{fname}: face {} vertex {} is not a vertex of the surface",
                        fno + 1,
                        v.wrapping_add(1)
                    ),
                );
                if !g.notices.reproduce() {
                    log::error_printf(
                        g,
                        io.err,
                        ERROR_BADFILE,
                        &format!(
                            "read_icosahedron({fname}): face {} vertex {} is not a vertex of the surface",
                            fno + 1,
                            v.wrapping_add(1)
                        ),
                    );
                    return Ok(None);
                }
                if v < 0 || v >= room {
                    return Err(Fatal::Exit(139));
                }
            }
            let vt = mris.topology_mut(v as usize);
            if vt.num == u8::MAX {
                g.notices.add(
                    &FS_361,
                    format!(
                        "{fname}: vertex {v} is in more than 255 faces (its count wraps around)"
                    ),
                );
                if !g.notices.reproduce() {
                    log::error_printf(
                        g,
                        io.err,
                        ERROR_BADFILE,
                        &format!("ICOread: vertex {v} is in more than 255 faces"),
                    );
                    return Ok(None);
                }
            }
            vt.num = vt.num.wrapping_add(1);
            vnum_add(g, sg, io.err, &mut mris, v as usize, 2);
        }
    }
    for vno in 0..nv.max(0) as usize {
        let half = i32::from(mris.topology(vno).vnum) / 2;
        if half < 0 {
            g.errno = 12;
            log::error_exit(
                g,
                io.err,
                ERROR_NOMEMORY,
                &format!("ICread: could not allocate {vno}th vertex list."),
            )?;
        }
        mris.topology_mut(vno).v = Some(vec![0; half.max(0) as usize]);
        clear_vnum(g, sg, io.err, &mut mris, vno);
    }
    for fno in 0..nf {
        let fv = mris.face(fno).v;
        for n in 0..3 {
            let me = fv[n] as usize;
            for n1 in 0..3 {
                if n1 == n {
                    continue;
                }
                let mut vn = ico.faces[fno].vno[n1].wrapping_sub(1);
                {
                    let vt = mris.topology(me);
                    let k = i32::from(vt.vnum).max(0) as usize;
                    let list = vt.v.as_deref().unwrap_or(&[]);
                    if list[..k.min(list.len())].contains(&vn) {
                        vn = -1;
                    }
                }
                if vn >= 0 {
                    let i = vnum_add(g, sg, io.err, &mut mris, me, 1);
                    let mut list = mris.topology_mut(me).v.take().unwrap_or_default();
                    put(g, &mut list, i as usize, vn, me, "neighbour list");
                    mris.topology_mut(me).v = Some(list);
                }
            }
        }
    }
    for vno in 0..nv.max(0) as usize {
        let vt = mris.topology_mut(vno);
        let num = usize::from(vt.num);
        vt.f = Some(vec![0; num]);
        vt.n = Some(vec![0; num]);
        vt.num = 0;
        vt.nsizeMax = 1;
        set_nsize_cur(g, io, &mut mris, vno, 1)?;
    }
    for fno in 0..nf {
        let fv = mris.face(fno).v;
        for n in 0..3 {
            let me = fv[n] as usize;
            let i = usize::from(mris.topology(me).num);
            let mut nl: Vec<i32> = mris
                .topology_mut(me)
                .n
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(i32::from)
                .collect();
            put(g, &mut nl, i, n as i32, me, "face list");
            let mut fl = mris.topology_mut(me).f.take().unwrap_or_default();
            put(g, &mut fl, i, fno as i32, me, "face list");
            let vt = mris.topology_mut(me);
            vt.n = Some(nl.into_iter().map(|x| x as u8).collect());
            vt.f = Some(fl);
            vt.num = vt.num.wrapping_add(1);
        }
    }
    mp::compute_metric_properties(g, sg, io, &mut mris)?;
    mris.radius = metric::average_radius(&mris);
    mris.r#type = MRIS_ICO_SURFACE;
    Ok(Some(mris))
}

/// `ICOreadOverAlloc(fname, nVFMultiplier, scaleFactor)`: [`read_icosahedron`], then
/// [`ico_to_scaled_mris`] with room for `nVFMultiplier` times the counts (`(int)` of the double
/// products). Returns `None` when the file cannot be read (a second error,
/// `ICOreadOverAlloc(<file>): could not open file`, `ERROR_NOFILE`).
pub fn ico_read_over_alloc(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    nvf_multiplier: f64,
    scale_factor: f32,
) -> Result<Option<Mris>, Fatal> {
    let fname = c_str(fname);
    let Some(mut ico) = read_icosahedron(g, io, dir, fname)? else {
        log::error_printf(
            g,
            io.err,
            ERROR_NOFILE,
            &format!("ICOreadOverAlloc({fname}): could not open file"),
        );
        return Ok(None);
    };
    let max_v = crate::trifile::room(g, nvf_multiplier * f64::from(ico.nvertices));
    let max_f = crate::trifile::room(g, nvf_multiplier * f64::from(ico.nfaces));
    ico_to_scaled_mris(g, sg, io, &mut ico, max_v, max_f, scale_factor, fname)
}

/// `ICOread(fname)`: [`ico_read_over_alloc`] with no extra room and no scaling.
pub fn ico_read(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mris>, Fatal> {
    ico_read_over_alloc(g, sg, io, dir, fname, 1.0, 1.0)
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface written in the format its name says (queue item 3.1-48, relay 15): `MRISwrite` (with
//! `MRISwrite_new`, the new behaviour FreeSurfer 8.2.0 always takes), `MRISwriteVertexLocations`,
//! `MRISwritePatchAscii` and `MRISreadVerticesOnly` (`utils/mrisurf_io.cpp` 3276-3340, 4279-4340,
//! 4414-4451, 5226-5317). `MRISread` comes here with fs.surf's pieces it needs.

use std::path::Path;

use larmorx_freesurfer_core::chklc::chklc;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_NOFILE, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_io::fio::{fread_float, fread2, fread3};

use crate::coords::{self, Io};
use crate::curvio::apply_fsgii_write;
use crate::formats::{self, MRIS_TRIANGULAR_SURFACE};
use crate::metric;
use crate::mris::Mris;
use crate::mrisurf::{SmallSurface, SmallVertex};
use crate::names::{
    MRIS_ASCII_TRIANGLE_FILE, MRIS_GEO_TRIANGLE_FILE, MRIS_GIFTI_FILE, MRIS_ICO_FILE,
    MRIS_STL_FILE, MRIS_VTK_FILE, mris_file_name_type,
};
use crate::positions;
use crate::sfile::{self, c_str, nonneg, open, open_failed, strcpy_checked};
use crate::stl;
use crate::surfread::{read_triangle_file_vertex_positions_only, verbose_show};
use crate::topology;
use crate::trifile::{
    self, NEW_QUAD_FILE_MAGIC_NUMBER, QUAD_FILE_MAGIC_NUMBER, TRIANGLE_FILE_MAGIC_NUMBER,
};
use crate::views::CURRENT_VERTICES;

/// The types `MRISwrite` and `MRISread` take from the name as they are (the others may become
/// GIFTI through `FS_GII`).
fn named_type(t: i32) -> bool {
    [
        MRIS_ASCII_TRIANGLE_FILE,
        MRIS_ICO_FILE,
        MRIS_GEO_TRIANGLE_FILE,
        MRIS_STL_FILE,
        MRIS_VTK_FILE,
        MRIS_GIFTI_FILE,
    ]
    .contains(&t)
}

/// GIFTI surfaces are written and read by `utils/gifti.cpp` 213-2857 (the surface part of queue
/// item 3.1-26, this crate's next piece); until it is ported, reaching it is out of range.
fn gifti_pending(what: &str) -> ! {
    panic!("{what}: GIFTI surfaces (gifti.cpp's surface part) are not yet ported")
}

/// `MRISwrite(mris, name)`: the surface written in the format its name says (`MRISfileNameType`):
/// ASCII, VTK, movie.byu, icosahedron, STL, GIFTI; any other name, after `__MRISapplyFSGIIwrite`
/// (which may make it a GIFTI name, or end the program when the other form exists), a triangle
/// file for a surface of type `MRIS_TRIANGULAR_SURFACE`, else a quadrangle file. The name is
/// copied into a 1,024-byte buffer (FS-126: FreeSurfer's buffer-overflow abort at 1,024 bytes or
/// more). Returns the writer's answer (STL: 1 written, 0 not). FreeSurfer 8.2.0 always takes the
/// new behaviour (`MRISwrite_new`); the licence check (`chklc`) passes.
pub fn write(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    let name = c_str(name);
    let mut surf_to_write = strcpy_checked(g, io, name, "MRISwrite")?;
    let mut ty = mris_file_name_type(g, &surf_to_write);
    if !named_type(ty) {
        apply_fsgii_write(g, io, dir, &mut surf_to_write, name, &mut ty)?;
    }
    write_new(g, io, dir, mris, &surf_to_write)
}

/// `MRISwrite_new(mris, name)`.
fn write_new(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    chklc();
    let fname = strcpy_checked(g, io, name, "MRISwrite_new")?;
    Ok(match mris_file_name_type(g, &fname) {
        MRIS_ASCII_TRIANGLE_FILE => sfile::write_ascii(g, io, dir, mris, &fname),
        MRIS_VTK_FILE => sfile::write_vtk(g, io, dir, mris, &fname),
        MRIS_GEO_TRIANGLE_FILE => formats::write_geo(g, io, dir, mris, &fname),
        MRIS_ICO_FILE => formats::write_ico(g, io, dir, mris, &fname),
        MRIS_STL_FILE => i32::from(stl::write_stl(g, dir, mris, &fname)),
        MRIS_GIFTI_FILE => gifti_pending("MRISwrite"),
        _ if mris.r#type == MRIS_TRIANGULAR_SURFACE => {
            trifile::write_triangular_surface(g, io, dir, mris, &fname)?
        }
        _ => trifile::write_quadrangle_file(g, io, dir, mris, &fname)?,
    })
}

/// `MRISwriteVertexLocations(mris, fname, which_vertices)`: the surface written (`MRISwrite`) with
/// the coordinate set `which_vertices` restored as its current one, then its current coordinates
/// put back (its distances freed: `MRISimportVertexCoords`). Returns `MRISwrite`'s answer.
pub fn write_vertex_locations(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    which_vertices: i32,
) -> Result<i32, Fatal> {
    let n = nonneg(mris.nvertices);
    let mut coords3: [Vec<f32>; 3] = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    metric::extract_vertex_coords(g, io, mris, &mut coords3, CURRENT_VERTICES)?;
    positions::restore_vertex_positions(g, io, mris, which_vertices)?;
    let retval = write(g, io, dir, mris, fname)?;
    let [x, y, z] = &coords3;
    coords::import_vertex_coords(g, io, mris, [x, y, z], CURRENT_VERTICES)?;
    Ok(retval)
}

/// `MRISwritePatchAscii(mris, fname)`: an ASCII patch: `#!ascii version of patch <surface name>.
/// The 1st index is not a vertex number`, the counts of unripped vertices and faces, per unripped
/// vertex `<index> vno=<vno>` (the index `vno + 1`, negative for a border vertex) and its
/// coordinates (`%f`, two spaces apart), per unripped face its number and its three vertices
/// (`%d ` each); on standard output `nvertices=<n> (valid=<MRISvalidVertices>) nfaces=<n>`.
/// Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISwritePatchAscii: could not open file <file>`).
pub fn write_patch_ascii(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    let fname = c_str(fname);
    // the type is computed and not used
    let _ = mris_file_name_type(g, fname);
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISwritePatchAscii: could not open file {fname}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    let nv = nonneg(mris.nvertices);
    let nf = nonneg(mris.nfaces);
    let nvertices = (0..nv).filter(|&k| mris.vertex(k).ripflag == 0).count();
    let nfaces = (0..nf).filter(|&k| mris.face(k).ripflag == 0).count();
    fp.put_bytes(
        format!(
            "#!ascii version of patch {}. The 1st index is not a vertex number\n",
            c_str(&mris.fname)
        )
        .as_bytes(),
    );
    fp.put_bytes(format!("{nvertices} {nfaces}\n").as_bytes());
    let _ = writeln!(
        io.out,
        "nvertices={nvertices} (valid={}) nfaces={nfaces}",
        topology::valid_vertices(mris)
    );
    for vno in 0..nv {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let i = if v.border != 0 {
            -(vno as i64 + 1)
        } else {
            vno as i64 + 1
        };
        fp.put_bytes(format!("{i} vno={vno}\n").as_bytes());
        fp.put_bytes(
            format!(
                "{}  {}  {}\n",
                fmt_f(f64::from(v.x), 0, 6),
                fmt_f(f64::from(v.y), 0, 6),
                fmt_f(f64::from(v.z), 0, 6)
            )
            .as_bytes(),
        );
    }
    for fno in 0..nf {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        let mut s = format!("{fno}\n");
        for v in f.v {
            s += &format!("{v} ");
        }
        s.push('\n');
        fp.put_bytes(s.as_bytes());
    }
    fp.close();
    NO_ERROR
}

/// `MRISreadVerticesOnly(fname)`: the vertex positions of a surface file. ASCII, icosahedron and
/// movie.byu names are refused (`MRISreadVerticesOnly: file type <t> not supported`,
/// `ERROR_UNSUPPORTED`); a file that cannot be opened is `MRISread(<file>): could not open file`
/// (`ERROR_NOFILE`). A triangle file's positions come from its own reader (`mrisReadTriangleFile
/// failed.` when it fails, with its code); a quadrangle file (magic number, or none: the old
/// format, read from the start) gives the 3-byte vertex and quadrangle counts, then per vertex three
/// 2-byte hundredths (the magic number `0xffffff`) or three floats (the new or old format). A
/// count of 0 vertices is `MRISreadVerticesOnly: could not allocate surface` (`ERROR_NOMEMORY`).
/// FreeSurfer leaves a quadrangle file open; larmorx closes it (nothing else differs).
pub fn read_vertices_only(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<SmallSurface>, Fatal> {
    let fname = c_str(fname);
    let ty = mris_file_name_type(g, fname);
    if [
        MRIS_ASCII_TRIANGLE_FILE,
        MRIS_ICO_FILE,
        MRIS_GEO_TRIANGLE_FILE,
    ]
    .contains(&ty)
    {
        log::error_printf(
            g,
            io.err,
            ERROR_UNSUPPORTED,
            &format!("MRISreadVerticesOnly: file type {ty} not supported"),
        );
        return Ok(None);
    }
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let m = format!("MRISread({fname}): could not open file");
            log::error_printf(g, io.err, ERROR_NOFILE, &m);
            return Ok(None);
        }
    };
    let (_, magic) = fread3(&mut fp);
    let version = if magic == QUAD_FILE_MAGIC_NUMBER {
        if verbose_show(g) {
            let _ = io.out.write_all(b"new surface file format\n");
        }
        -1
    } else if magic == NEW_QUAD_FILE_MAGIC_NUMBER {
        -2
    } else if magic == TRIANGLE_FILE_MAGIC_NUMBER {
        fp.close();
        let s = read_triangle_file_vertex_positions_only(g, io, dir, fname)?;
        if s.is_none() {
            let code = g.gerror;
            log::error_printf(g, io.err, code, "mrisReadTriangleFile failed.\n");
        }
        return Ok(s);
    } else {
        fp.rewind();
        if verbose_show(g) {
            let _ = io.out.write_all(b"surfer: old surface file format\n");
        }
        0
    };
    let (_, nvertices) = fread3(&mut fp);
    let (_, nquads) = fread3(&mut fp);
    if verbose_show(g) {
        let _ = writeln!(
            io.out,
            "reading {nvertices} vertices and {} faces.",
            2 * nquads
        );
    }
    if nvertices == 0 {
        log::error_printf(
            g,
            io.err,
            ERROR_NOMEMORY,
            "MRISreadVerticesOnly: could not allocate surface",
        );
        return Ok(None);
    }
    let mut vertices = Vec::with_capacity(nonneg(nvertices));
    for _ in 0..nvertices {
        let v = if version == -1 {
            let (_, ix) = fread2(g, &mut fp);
            let (_, iy) = fread2(g, &mut fp);
            let (_, iz) = fread2(g, &mut fp);
            let c = |i: i32| (f64::from(i) / 100.0) as f32;
            SmallVertex {
                x: c(ix),
                y: c(iy),
                z: c(iz),
            }
        } else {
            let x = fread_float(g, io.err, &mut fp);
            let y = fread_float(g, io.err, &mut fp);
            let z = fread_float(g, io.err, &mut fp);
            SmallVertex { x, y, z }
        };
        vertices.push(v);
    }
    fp.close();
    Ok(Some(SmallSurface {
        nvertices,
        vertices,
    }))
}

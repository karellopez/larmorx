// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s readers of another surface file's vertex positions (`fs.surfio`,
//! queue items 3.1-47 and 3.1-49, relay 36 of track D): `MRISreadVertexPositions` 2369 (the
//! positions of a quadrangle, triangle, movie.byu, icosahedron or GIFTI file) and the readers
//! built on it: `MRISreadWhiteCoordinates` 897, `MRISreadPialCoordinates` 914,
//! `MRISreadCanonicalCoordinates` 940 (each into one of the surface's coordinate sets) and
//! `MRISreadOriginalProperties` 2512 (the metric properties of the original surface).

use std::path::Path;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_io::fio::{fread_float, fread1, fread2, fread3};
use larmorx_freesurfer_io::identify::GIFTI_FILE;

use crate::aaa::{MrisStatus, SurfGlobals};
use crate::coords::{self, Io};
use crate::curvio::{apply_fsgii_read, build_file_name_read};
use crate::dist;
use crate::ico;
use crate::metric;
use crate::mp;
use crate::mp_slow;
use crate::mris::Mris;
use crate::names::{
    MRIS_ASCII_TRIANGLE_FILE, MRIS_GEO_TRIANGLE_FILE, MRIS_GIFTI_FILE, MRIS_ICO_FILE,
    MRIS_STL_FILE, MRIS_VOLUME_FILE, MRIS_VTK_FILE, mris_file_name_type,
};
use crate::positions;
use crate::readwrite;
use crate::sfile::{c_str, open, open_failed};
use crate::surfread::{self, verbose_show};
use crate::trifile::{
    NEW_QUAD_FILE_MAGIC_NUMBER, QUAD_FILE_MAGIC_NUMBER, TRIANGLE_FILE_MAGIC_NUMBER,
};
use crate::views::{
    CANONICAL_VERTICES, CURRENT_VERTICES, PIAL_VERTICES, TMP_VERTICES, WHITE_VERTICES,
};

fn io_cpp() -> String {
    format!(
        "{}/utils/mrisurf_io.cpp",
        larmorx_freesurfer_core::diag::BUILD_ROOT
    )
}

/// `MRISreadVertexPositions(mris, name)`: the surface's current positions from another file of the
/// same surface. The distances are freed first (`MRISfreeDistsButNotOrig`); the name is completed
/// by `MRISbuildFileName_read` (the surface's folder and hemisphere for a name without `/`); a
/// name whose type ([`mris_file_name_type`]) is not ASCII, icosahedron, movie.byu, STL, VTK,
/// GIFTI or a volume may become a GIFTI name (`__MRISapplyFSGIIread`). Then by the type:
///
/// - movie.byu: [`surfread::read_geo_file_positions`];
/// - icosahedron: [`ico::read_vertex_positions`];
/// - GIFTI (either code): `Reading <file> as a gii file` on standard output, the file read whole
///   (`MRISread`); 1 when it gives no surface, or after `MRISreadVertexPosition(<file>): dim
///   mismatch <n> <n'>` when its vertex count is not the surface's; else its positions and 0;
/// - any other (ASCII, STL and VTK names included): the binary file opened
///   (`MRISreadVertexPosition(<name>): could not open file <file>`, `ERROR_NOFILE`); its 3-byte
///   magic number: a triangle file's positions by [`surfread::read_triangle_file_positions`] of
///   the completed name (`mrisReadTriangleFile failed.` with its error code when it fails), else
///   the quadrangle file's vertex and quadrangle counts (the old format: read from the start); a
///   vertex count other than the surface's, or twice the quadrangle count other than its face
///   count, gives `ERROR: ... MRISreadVertexPositions(<file>): surfaces differ. Main: <n> verts
///   <m> faces, <name>: <n'> verts <m'> faces` and `Surfaces may be out-of-date` (`ERROR_BADFILE`);
///   then per vertex three 2-byte hundredths (the magic number `0xffffff`) or three big-endian
///   floats (the new format, and the old one), and for the old format one byte and a 3-byte number
///   per face of the vertex skipped. With `DIAG_SHOW` and verbose: `new surface file format`,
///   `surfer: old surface file format`, `reading <n> vertices and <m> faces.`.
///
/// Returns `NO_ERROR`, or the reader's code.
pub fn read_vertex_positions(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    let name = c_str(name);
    dist::free_dists_but_not_orig(mris);
    let (_, fname) = build_file_name_read(g, io, mris, name);
    let mut surf_to_read = fname.clone();
    let mut ty = mris_file_name_type(g, &surf_to_read);
    if ![
        MRIS_ASCII_TRIANGLE_FILE,
        MRIS_ICO_FILE,
        MRIS_GEO_TRIANGLE_FILE,
        MRIS_STL_FILE,
        MRIS_VTK_FILE,
        MRIS_GIFTI_FILE,
        MRIS_VOLUME_FILE,
    ]
    .contains(&ty)
    {
        apply_fsgii_read(g, io, dir, &mut surf_to_read, &fname, &mut ty)?;
    }
    if ty == MRIS_GEO_TRIANGLE_FILE {
        return surfread::read_geo_file_positions(g, io, dir, mris, &surf_to_read);
    } else if ty == MRIS_ICO_FILE {
        return ico::read_vertex_positions(g, io, dir, mris, &surf_to_read, CURRENT_VERTICES);
    } else if ty == MRIS_GIFTI_FILE || ty == GIFTI_FILE {
        let _ = writeln!(io.out, "Reading {surf_to_read} as a gii file");
        let Some(gsurf) = readwrite::read(g, sg, io, dir, &surf_to_read, true)? else {
            return Ok(1);
        };
        if gsurf.nvertices != mris.nvertices {
            let _ = writeln!(
                io.out,
                "MRISreadVertexPosition({surf_to_read}): dim mismatch {} {}",
                gsurf.nvertices, mris.nvertices
            );
            return Ok(1);
        }
        let file = io_cpp();
        for vno in 0..mris.nvertices.max(0) {
            let gv = gsurf.vertex(vno as usize);
            let (x, y, z) = (gv.x, gv.y, gv.z);
            coords::set_xyz(g, io, mris, vno, x, y, z, &file, 2412)?;
        }
        return Ok(0);
    }
    let mut fp = match open(dir, &surf_to_read, "rb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISreadVertexPosition({name}): could not open file {surf_to_read}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
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
        if surfread::read_triangle_file_positions(g, io, dir, mris, &fname)? != NO_ERROR {
            let code = g.gerror;
            log::error_printf(g, io.err, code, "mrisReadTriangleFile failed.\n");
            return Ok(code);
        }
        return Ok(NO_ERROR);
    } else {
        fp.rewind();
        if verbose_show(g) {
            let _ = io.out.write_all(b"surfer: old surface file format\n");
        }
        0
    };
    let (_, nvertices) = fread3(&mut fp);
    let (_, nquads) = fread3(&mut fp);
    let nfaces = nquads.wrapping_mul(2);
    if verbose_show(g) {
        let _ = writeln!(io.out, "reading {nvertices} vertices and {nfaces} faces.");
    }
    if nvertices != mris.nvertices || nfaces != mris.nfaces {
        fp.close();
        let m = format!(
            "\nERROR: \n      MRISreadVertexPositions({fname}): surfaces differ. Main: {} verts {} faces, {name}: {nvertices} verts {nfaces} faces\nSurfaces may be out-of-date\n\n",
            mris.nvertices, mris.nfaces
        );
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok(ERROR_BADFILE);
    }
    let file = io_cpp();
    for vno in 0..nvertices {
        if version == -1 {
            let (_, ix) = fread2(g, &mut fp);
            let (_, iy) = fread2(g, &mut fp);
            let (_, iz) = fread2(g, &mut fp);
            let c = |i: i32| (f64::from(i) / 100.0) as f32;
            coords::set_xyz(g, io, mris, vno, c(ix), c(iy), c(iz), &file, 2474)?;
        } else {
            let x = fread_float(g, io.err, &mut fp);
            let y = fread_float(g, io.err, &mut fp);
            let z = fread_float(g, io.err, &mut fp);
            coords::set_xyz(g, io, mris, vno, x, y, z, &file, 2484)?;
        }
        if version == 0 {
            let _ = fread1(g, &mut fp);
            let num = mris.topology(vno as usize).num;
            for _ in 0..num {
                let _ = fread3(&mut fp);
            }
        }
    }
    fp.close();
    Ok(NO_ERROR)
}

/// The coordinate readers' common part: the current positions saved (`TMP_VERTICES`), the file's
/// positions read ([`read_vertex_positions`]; its failure returns `Err(Gerror)`, which may be 0:
/// a reader that fails without an error message leaves it), saved as `which`, the current
/// positions restored.
fn read_into(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
    which: i32,
) -> Result<Result<(), i32>, Fatal> {
    positions::save_vertex_positions(g, io, mris, TMP_VERTICES)?;
    if read_vertex_positions(g, sg, io, dir, mris, sname)? != NO_ERROR {
        return Ok(Err(g.gerror));
    }
    positions::save_vertex_positions(g, io, mris, which)?;
    positions::restore_vertex_positions(g, io, mris, TMP_VERTICES)?;
    Ok(Ok(()))
}

/// `MRISreadWhiteCoordinates(mris, sname)`: the white positions from `sname` (`white` when
/// `None`) by [`read_vertex_positions`], the current ones kept. Returns `NO_ERROR`, or `Gerror`
/// when the file was not read.
pub fn read_white_coordinates(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: Option<&str>,
) -> Result<i32, Fatal> {
    let sname = sname.unwrap_or("white");
    Ok(read_into(g, sg, io, dir, mris, sname, WHITE_VERTICES)?
        .err()
        .unwrap_or(NO_ERROR))
}

/// `MRISreadPialCoordinates(mris, sname)`: as [`read_white_coordinates`] into the pial positions
/// (`pial` when `None`).
pub fn read_pial_coordinates(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: Option<&str>,
) -> Result<i32, Fatal> {
    let sname = sname.unwrap_or("pial");
    Ok(read_into(g, sg, io, dir, mris, sname, PIAL_VERTICES)?
        .err()
        .unwrap_or(NO_ERROR))
}

/// `MRISreadCanonicalCoordinates(mris, sname)`: the canonical (spherical) positions from `sname`
/// as [`read_white_coordinates`] reads the white ones, then their spherical coordinates
/// (`MRIScomputeCanonicalCoordinates`). Returns `NO_ERROR`, or `Gerror` when the file was not
/// read (0 when its reader printed no error).
pub fn read_canonical_coordinates(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
) -> Result<i32, Fatal> {
    if let Err(code) = read_into(g, sg, io, dir, mris, sname, CANONICAL_VERTICES)? {
        return Ok(code);
    }
    positions::compute_canonical_coordinates(g, mris);
    Ok(NO_ERROR)
}

/// `MRISreadOriginalProperties(mris, sname)` (`smoothwm` when `None`): the current positions
/// saved, the status made `MRIS_PATCH` (so that nothing is oriented) while the file's positions
/// are read ([`read_vertex_positions`]; its failure: `MRISreadOriginalProperties: could not read
/// surface file <name>`, `ERROR_BADFILE`, the status left a patch's); they become the original
/// positions; their metric and triangle properties computed and stored as the original ones
/// (`MRISstoreMetricProperties`); the status back; the current positions restored, their metric
/// and triangle properties computed, the surface oriented (`mrisOrientSurface`) and `orig_area`
/// its total area. Returns `NO_ERROR`.
pub fn read_original_properties(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: Option<&str>,
) -> Result<i32, Fatal> {
    let sname = sname.unwrap_or("smoothwm");
    positions::save_vertex_positions(g, io, mris, TMP_VERTICES)?;
    let old_status = mris.status;
    mris.status = MrisStatus::PATCH;
    if read_vertex_positions(g, sg, io, dir, mris, sname)? != NO_ERROR {
        let m = format!("MRISreadOriginalProperties: could not read surface file {sname}");
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok(ERROR_BADFILE);
    }
    coords::set_original_xyz_from_xyz(mris);
    mp::compute_metric_properties(g, sg, io, mris)?;
    mp_slow::compute_triangle_properties(g, sg, io, mris);
    metric::store_metric_properties(mris);
    mris.status = old_status;
    positions::restore_vertex_positions(g, io, mris, TMP_VERTICES)?;
    mp::compute_metric_properties(g, sg, io, mris)?;
    mp_slow::compute_triangle_properties(g, sg, io, mris);
    mp_slow::orient_surface(sg, mris);
    mris.orig_area = mris.total_area;
    Ok(NO_ERROR)
}

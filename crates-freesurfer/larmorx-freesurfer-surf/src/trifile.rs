// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s binary surface files (queue item 3.1-48, relay 14): the triangle file
//! FreeSurfer writes for every surface (`mrisReadTriangleFile` 5494, `MRISwriteTriangularSurface`
//! 5319 with `__MRISwriteTriangularSurfaceTags` 6592) and the quadrangle file of the oldest
//! surfaces (`__MRISreadQuadrangleFile` 6314, `__MRISwriteQuadrangleFile` 6491), with the tagged
//! fields after them: the real-RAS flag, the volume geometry (`readVolGeom`/`writeVolGeom`,
//! fs.volume's), the group-average area, the command lines and the three space tags.
//! `MRISreadOverAlloc` and `MRISwrite` call them (they wait for fs.io's format identification,
//! `mri_identify`, queue item 3.1-27); the surfaces read are not completed here (topology, metric
//! properties: `MRISreadOverAlloc`'s work).
//!
//! The defects: FS-346 (a command-line tag of length -1), FS-347 (more than 1,000 command lines),
//! FS-348 (the quadrangle reader's command line that cannot be allocated), FS-349 (the quadrangle
//! reader's faces past its room), FS-350 (a writer's missing command line), FS-351 and FS-352 (a
//! quadrangle vertex outside the surface), FS-353 (the quadrangle reader's lost triangle); FS-147
//! (a vertex of more than 255 faces), fs.io's FS-49 and FS-167 (a number the file ends before),
//! FS-177 (a length shown by `%d`) and FS-178 (a tag skipped by allocating its length).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    FS_02, FS_177, FS_346, FS_347, FS_348, FS_349, FS_350, FS_351, FS_352, FS_353,
};
use larmorx_freesurfer_core::cstd::{double_to_int_x86, fmt_f};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_NOFILE, ERROR_NOMEMORY, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::timer::current_date_time;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{
    fread_float, fread_int, fread_int_ex, fread_long, fread1, fread2, fread3, fwrite_float,
    fwrite_int, fwrite3,
};
use larmorx_freesurfer_io::tags::{
    TAG_CMDLINE, TAG_GROUP_AVG_SURFACE_AREA, TAG_OLD_COLORTABLE, TAG_OLD_MGH_XFORM,
    TAG_OLD_SURF_GEOM, TAG_OLD_USEREALRAS, TAG_SURF_DATASPACE, TAG_SURF_MATRIXDATA,
    TAG_SURF_TRANSFORMEDSPACE, tag_skip, tag_write, tag_write_end, tag_write_matrix,
    tag_write_start,
};
use larmorx_freesurfer_volume::vgeom::{
    read_vol_geom, tkr_vox2ras_from_vol_geom, vg_i_to_r, write_vol_geom,
};

use crate::aaa::{MAX_CMDS, SurfGlobals, VERTICES_PER_FACE, which_face_split};
use crate::alloc;
use crate::coords::{self, Io};
use crate::formats::MRIS_TRIANGULAR_SURFACE;
use crate::mris::Mris;
use crate::sfile::{c_str, nonneg, open};
use crate::surfread::{count_face, quad_combine, show_counts, triangle_counts, verbose_show};

/// `QUAD_FILE_MAGIC_NUMBER` (`-1 & 0x00ffffff`): the quadrangle file of shorts (hundredths).
pub const QUAD_FILE_MAGIC_NUMBER: i32 = 0x00ff_ffff;
/// `TRIANGLE_FILE_MAGIC_NUMBER` (`-2 & 0x00ffffff`).
pub const TRIANGLE_FILE_MAGIC_NUMBER: i32 = 0x00ff_fffe;
/// `NEW_QUAD_FILE_MAGIC_NUMBER` (`-3 & 0x00ffffff`): the quadrangle file of floats.
pub const NEW_QUAD_FILE_MAGIC_NUMBER: i32 = 0x00ff_fffd;
/// `MRIS_BINARY_QUADRANGLE_FILE` (`mrisurf.h`).
pub const MRIS_BINARY_QUADRANGLE_FILE: i32 = 0;

/// `ENOMEM` (Linux): what a failed `calloc` leaves in `errno`.
const ENOMEM: i32 = 12;

/// The bytes a skipped or read tag is read by at a time (FreeSurfer allocates the whole length).
const CHUNK: usize = 1 << 16;

fn io_cpp() -> String {
    format!("{}/utils/mrisurf_io.cpp", diag::BUILD_ROOT)
}

/// `(int)(m * n)` of `MRISoverAlloc`'s room: x86-64's truncation; FS-02 (an undefined
/// conversion: saturated by default, `INT_MIN` with `reproduce`).
fn room(g: &mut Globals, x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_02,
            format!(
                "a surface's room of {x} elements, outside the int range: an undefined conversion"
            ),
        );
        if g.notices.reproduce() {
            return double_to_int_x86(x);
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// `TAGreadStart(fp, &len)` as fs.io's, without its note that the tag number came from memory
/// contents where the file ended in it: the number is not used then (the end of the file ends the
/// loop). The tag (0 at the end of the file, `len` then untouched) and its data length:
/// `TAG_OLD_MGH_XFORM`'s 32-bit length less one, none for `TAG_OLD_SURF_GEOM`,
/// `TAG_OLD_USEREALRAS` and `TAG_OLD_COLORTABLE`, a 64-bit length for the others.
fn tag_start(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile, len: &mut i64) -> i32 {
    let mut b = [0u8; 4];
    let got = fp.read_bytes(&mut b);
    if fp.feof() || got < 4 {
        return 0;
    }
    let tag = i32::from_be_bytes(b);
    match tag {
        TAG_OLD_MGH_XFORM => *len = i64::from(fread_int(g, fp)) - 1,
        TAG_OLD_SURF_GEOM | TAG_OLD_USEREALRAS | TAG_OLD_COLORTABLE => *len = 0,
        _ => *len = fread_long(g, err, fp),
    }
    tag
}

/// `fread(buf, 1, len, fp)` into a zeroed block of `len + 1` bytes, then the block as a C string:
/// the bytes read before the first NUL. Reads a chunk at a time (FreeSurfer's block is the whole
/// length); `None` for a length below 0.
fn read_c_string(fp: &mut CFile, len: i64) -> Option<Vec<u8>> {
    if len < 0 {
        return None;
    }
    let want = len as u64;
    let mut s = Vec::new();
    let mut nul = false;
    let mut buf = vec![0u8; CHUNK.min(want as usize)];
    let mut total: u64 = 0;
    while total < want {
        let n = ((want - total) as usize).min(CHUNK);
        let r = fp.fread(&mut buf[..n], 1, n);
        if !nul {
            match buf[..r].iter().position(|&c| c == 0) {
                Some(i) => {
                    s.extend_from_slice(&buf[..i]);
                    nul = true;
                }
                None => s.extend_from_slice(&buf[..r]),
            }
        }
        total += r as u64;
        if r < n {
            break;
        }
    }
    Some(s)
}

/// The two readers' command-line tag. `quad`: `__MRISreadQuadrangleFile`'s (no allocation check:
/// FS-348); else `mrisReadTriangleFile`'s (`MRISread(<file>): could not allocate <len> byte
/// cmdline` with the length's low 32 bits, FS-177). Both store the line at `cmdlines[ncmds]`
/// after `ncmds > MAX_CMDS` is tested (FS-347: the 1,001st past the list). `Ok(false)` when the
/// correction refuses the tag (its message printed).
fn read_cmdline(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut CFile,
    mris: &mut Mris,
    fname: &str,
    len: i64,
    quad: bool,
) -> Result<bool, Fatal> {
    let who = if quad {
        "__MRISreadQuadrangleFile"
    } else {
        "MRISread"
    };
    if mris.ncmds > MAX_CMDS as i32 {
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            &format!("{who}({fname}): too many commands ({}) in file", mris.ncmds),
        )?;
    }
    if mris.ncmds == MAX_CMDS as i32 {
        g.notices.add(
            &FS_347,
            format!("{fname}: a 1,001st command line is refused (FreeSurfer stores it past its list of 1,000, over the surface's command-line count and group-average area)"),
        );
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            &format!("{who}({fname}): too many commands ({}) in file", mris.ncmds),
        )?;
    }
    // calloc(len + 1, 1): it fails for a size beyond the largest object (a length below -1 or
    // from 2^63 - 1 on); a length of -1 asks for no bytes
    let fails = len < -1 || len == i64::MAX;
    if fails {
        if quad {
            g.notices.add(
                &FS_348,
                format!("{fname}: a command line of {len} bytes, which cannot be allocated, is refused (FreeSurfer writes through the failed allocation: a crash)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
        }
        let low = len as i32;
        if i64::from(low) != len {
            g.notices.add(
                &FS_177,
                format!("a command-line length {len} shown as {low} in FreeSurfer's message"),
            );
        }
        let shown = if g.notices.reproduce() {
            low.to_string()
        } else {
            len.to_string()
        };
        g.errno = ENOMEM;
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            &format!("MRISread({fname}): could not allocate {shown} byte cmdline"),
        )?;
        return Ok(false);
    }
    if len == -1 {
        g.notices.add(
            &FS_346,
            format!("{fname}: a command line of length -1 is refused (FreeSurfer reads the rest of the file into a block of no bytes)"),
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("{who}({fname}): a command line of length -1"),
            )?;
            return Ok(false);
        }
        // the rest of the file, read into the block; the line its bytes before a NUL
        let s = read_c_string(fp, i64::MAX - 1).expect("a length");
        let k = nonneg(mris.ncmds);
        mris.cmdlines[k] = Some(s);
        mris.ncmds += 1;
        return Ok(true);
    }
    let s = read_c_string(fp, len).expect("a length");
    let k = nonneg(mris.ncmds);
    mris.cmdlines[k] = Some(s);
    mris.ncmds += 1;
    Ok(true)
}

/// The two readers' tag loop (`FS_SKIP_TAGS` unset): the group-average area (a float; the
/// quadrangle reader prints `reading group avg surface area <a/100, %2.0f> cm^2 from file`), the
/// volume geometry ([`read_vol_geom`]), the real-RAS flag (`freadIntEx`; 0 when it cannot be
/// read), the command lines ([`read_cmdline`]), every other tag skipped (`TAGskip`; the space
/// tags too, which FreeSurfer prints only in a debugging build). `Ok(false)` when a correction
/// refused a command line.
fn read_tags(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut CFile,
    mris: &mut Mris,
    fname: &str,
    quad: bool,
) -> Result<bool, Fatal> {
    if io.env.get("FS_SKIP_TAGS").is_some() {
        return Ok(true);
    }
    let mut len: i64 = 0;
    loop {
        let tag = tag_start(g, io.err, fp, &mut len);
        if tag == 0 {
            return Ok(true);
        }
        match tag {
            TAG_GROUP_AVG_SURFACE_AREA => {
                mris.group_avg_surface_area = fread_float(g, io.err, fp);
                if quad {
                    let _ = writeln!(
                        io.out,
                        "reading group avg surface area {} cm^2 from file",
                        fmt_f(f64::from(mris.group_avg_surface_area) / 100.0, 2, 0)
                    );
                }
            }
            TAG_OLD_SURF_GEOM => read_vol_geom(g, io.err, fp, &mut mris.vg),
            TAG_OLD_USEREALRAS => {
                if fread_int_ex(fp, &mut mris.useRealRAS) == 0 {
                    mris.useRealRAS = 0;
                }
            }
            TAG_CMDLINE => {
                if !read_cmdline(g, io, fp, mris, fname, len, quad)? {
                    return Ok(false);
                }
            }
            // the space tags (TAG_SURF_DATASPACE, TAG_SURF_MATRIXDATA, TAG_SURF_TRANSFORMEDSPACE)
            // and every other tag
            _ => {
                tag_skip(g, io.err, fp, tag, len)?;
            }
        }
    }
}

/// `ErrorExit(ERROR_BADFILE, "<program>: vertex <v> <axis> coordinate <%f>!")` of a coordinate
/// beyond 10000 in size or not finite.
fn check_coordinate(
    g: &mut Globals,
    io: &mut Io,
    vno: i32,
    name: &str,
    c: f32,
) -> Result<(), Fatal> {
    if c.abs() > 10000.0 || !c.is_finite() {
        log::error_exit(
            g,
            io.err,
            ERROR_BADFILE,
            &format!(
                "{}: vertex {vno} {name} coordinate {}!",
                g.progname,
                fmt_f(f64::from(c), 0, 6)
            ),
        )?;
    }
    Ok(())
}

/// `mrisReadTriangleFile(fname, nVFMultiplier)`: a new surface from a binary triangle file
/// (`MRISoverAlloc` with room for `nVFMultiplier` times the counts; type
/// `MRIS_TRIANGULAR_SURFACE`): the 3-byte magic number (not checked), unless
/// `TRIANGULARSURFACE_NOEXTRA_READ` is set the creator's line (`fgets` of 199 bytes; FS-354) and
/// the white space after it, the vertex and face counts (big-endian `int`s; either negative: two lines
/// `<mrisurf_io.cpp>:<line> <file> freadInt returned nvertices:<n> ` on standard error, then
/// `exit(1)`), three big-endian floats per vertex (a coordinate beyond 10000 in size, or not
/// finite, ends the program), three vertex numbers per face (one outside the surface ends the
/// program: `f[<f>]->v[<k>] = <v> - out of range!`), each face counted at its vertices; then
/// (unless `FS_SKIP_TAGS` is set) the tags ([`read_tags`]). Returns `None` after an error: the file
/// cannot be opened (`mrisReadTriangleFile(<file>): could not open file`, `ERROR_NOFILE`); FS-147
/// (`mrisReadTriangleFile(<file>): vertex <v> is in more than 255 faces`), FS-346 and FS-347.
pub fn read_triangle_file(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    nvf_multiplier: f64,
) -> Result<Option<Mris>, Fatal> {
    const WHO: &str = "mrisReadTriangleFile";
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("{WHO}({fname}): could not open file"),
            );
            return Ok(None);
        }
    };
    let noextra = io.env.get("TRIANGULARSURFACE_NOEXTRA_READ").is_some();
    let (nvertices, nfaces) = triangle_counts(g, &mut fp, fname, noextra);
    if nvertices < 0 || nfaces < 0 {
        let file = io_cpp();
        let _ = io.out.flush();
        let _ = writeln!(
            io.err,
            "{file}:5520 {fname} freadInt returned nvertices:{nvertices} "
        );
        let _ = writeln!(
            io.err,
            "{file}:5521 {fname} freadInt returned nfaces:{nfaces} "
        );
        return Err(Fatal::Exit(1));
    }
    show_counts(g, io, fname, nvertices, nfaces);
    let max_v = room(g, nvf_multiplier * f64::from(nvertices));
    let max_f = room(g, nvf_multiplier * f64::from(nfaces));
    let mut mris = alloc::over_alloc(g, io.out, io.err, sg, max_v, max_f, nvertices, nfaces)?;
    mris.r#type = MRIS_TRIANGULAR_SURFACE;
    let file = io_cpp();
    for vno in 0..nvertices {
        let x = fread_float(g, io.err, &mut fp);
        let y = fread_float(g, io.err, &mut fp);
        let z = fread_float(g, io.err, &mut fp);
        coords::set_xyz(g, io, &mut mris, vno, x, y, z, &file, 5543)?;
        mris.topology_mut(vno as usize).num = 0;
        let v = mris.vertex(vno as usize);
        let (vx, vy, vz) = (v.x, v.y, v.z);
        check_coordinate(g, io, vno, "x", vx)?;
        check_coordinate(g, io, vno, "y", vy)?;
        check_coordinate(g, io, vno, "z", vz)?;
    }
    for fno in 0..nonneg(mris.nfaces) {
        for n in 0..VERTICES_PER_FACE {
            let v = fread_int(g, &mut fp);
            mris.face_mut(fno).v[n] = v;
            if v >= mris.nvertices || v < 0 {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADFILE,
                    &format!("f[{fno}]->v[{n}] = {v} - out of range!\n"),
                )?;
            }
        }
        for n in 0..VERTICES_PER_FACE {
            let v = mris.face(fno).v[n] as usize;
            if !count_face(g, &mut mris, v, fname) {
                fp.close();
                log::error_printf(
                    g,
                    io.err,
                    ERROR_BADFILE,
                    &format!("{WHO}({fname}): vertex {v} is in more than 255 faces"),
                );
                return Ok(None);
            }
        }
    }
    mris.useRealRAS = 0;
    if !read_tags(g, io, &mut fp, &mut mris, fname, false)? {
        return Ok(None);
    }
    fp.close();
    Ok(Some(mris))
}

/// `__MRISreadQuadrangleFile(fname, nVFMultiplier)`: a new surface (type
/// `MRIS_BINARY_QUADRANGLE_FILE`) from a quadrangle file: the 3-byte magic number (neither
/// quadrangle number: the old format, read again from the start), the vertex and quadrangle counts
/// (3-byte; no vertices: `ERROR: MRISread: file '<file>' has <n> vertices!` and a second line, more
/// than four quadrangles per vertex: `nquads=<q>,  nvertices=<n>` on standard error and `ERROR:
/// MRISread: file '<file>' has many more faces than vertices!` with a second line; both end the
/// program), per vertex its coordinates (hundredths in `short`s for the old quadrangle number,
/// floats otherwise) and, in the old format, its face count (1 byte) and faces (3-byte); per
/// quadrangle four 3-byte vertex numbers, split into two triangles along the diagonal
/// `WHICH_FACE_SPLIT(v0, v1)` chooses (even: `v0 v1 v3` and `v2 v3 v1`; odd: `v0 v1 v2` and `v0 v2
/// v3`), a degenerate triangle (two equal vertices) not made (FS-353); then (unless `FS_SKIP_TAGS`
/// is set) the tags. Returns `None` after an error: the file cannot be opened
/// (`__MRISreadQuadrangleFile(<file>): could not open file`, `ERROR_NOFILE`); FS-147, FS-346 to
/// FS-349, FS-351, FS-352.
pub fn read_quadrangle_file(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    nvf_multiplier: f64,
) -> Result<Option<Mris>, Fatal> {
    const WHO: &str = "__MRISreadQuadrangleFile";
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("{WHO}({fname}): could not open file"),
            );
            return Ok(None);
        }
    };
    let (_, magic) = fread3(&mut fp);
    let old = magic != QUAD_FILE_MAGIC_NUMBER && magic != NEW_QUAD_FILE_MAGIC_NUMBER;
    if old {
        fp.rewind();
    }
    let (_, nvertices) = fread3(&mut fp);
    let (_, nquads) = fread3(&mut fp);
    if nvertices <= 0 {
        log::error_exit(
            g,
            io.err,
            ERROR_BADFILE,
            &format!(
                "ERROR: MRISread: file '{fname}' has {nvertices} vertices!\nProbably trying to use a scalar data file as a surface!\n"
            ),
        )?;
    }
    if nquads > 4 * nvertices {
        let _ = writeln!(io.err, "nquads={nquads},  nvertices={nvertices}");
        log::error_exit(
            g,
            io.err,
            ERROR_BADFILE,
            &format!(
                "ERROR: MRISread: file '{fname}' has many more faces than vertices!\nProbably trying to use a scalar data file as a surface!\n"
            ),
        )?;
    }
    if verbose_show(g) {
        let _ = writeln!(
            io.out,
            "reading {nvertices} vertices and {} faces.",
            2 * nquads
        );
    }
    let max_v = room(g, nvf_multiplier * f64::from(nvertices));
    let max_f = room(g, nvf_multiplier * 2.0 * f64::from(nquads));
    let mut mris = alloc::over_alloc(g, io.out, io.err, sg, max_v, max_f, nvertices, 0)?;
    mris.r#type = MRIS_BINARY_QUADRANGLE_FILE;
    let file = io_cpp();
    for vno in 0..nvertices {
        if magic == QUAD_FILE_MAGIC_NUMBER {
            let (_, ix) = fread2(g, &mut fp);
            let (_, iy) = fread2(g, &mut fp);
            let (_, iz) = fread2(g, &mut fp);
            let c = |i: i32| (f64::from(i) / 100.0) as f32;
            coords::set_xyz(g, io, &mut mris, vno, c(ix), c(iy), c(iz), &file, 6363)?;
        } else {
            let x = fread_float(g, io.err, &mut fp);
            let y = fread_float(g, io.err, &mut fp);
            let z = fread_float(g, io.err, &mut fp);
            coords::set_xyz(g, io, &mut mris, vno, x, y, z, &file, 6373)?;
        }
        let t = mris.topology_mut(vno as usize);
        if old {
            let (_, num) = fread1(g, &mut fp);
            t.num = num as u8;
            let mut f = vec![0i32; usize::from(t.num)];
            for x in &mut f {
                *x = fread3(&mut fp).1;
            }
            t.f = Some(f);
            t.n = Some(vec![0u8; usize::from(t.num)]);
        } else {
            t.num = 0;
        }
    }
    let room_v = mris.vertices_topology.as_ref().map_or(0, Vec::len);
    let mut fno: i32 = 0;
    let mut beyond = false;
    for _ in 0..nquads {
        let mut q = [0i32; 4];
        for x in &mut q {
            *x = fread3(&mut fp).1;
        }
        let which = which_face_split(g, q[0], q[1]);
        let even = (which / 2) * 2 == which;
        let (mut va, vb) = if even {
            ([q[0], q[1], q[3]], [q[2], q[3], q[1]])
        } else {
            ([q[0], q[1], q[2]], [q[0], q[2], q[3]])
        };
        for n in 0..2 {
            let degenerate = va[0] == va[1] || va[0] == va[2] || va[1] == va[2];
            if degenerate {
                // FreeSurfer's `continue` skips the shift to the second triangle too
                if n == 0 && !g.notices.reproduce() {
                    let bdeg = vb[0] == vb[1] || vb[0] == vb[2] || vb[1] == vb[2];
                    if !bdeg {
                        g.notices.add(
                            &FS_353,
                            format!("{fname}: quadrangle {} {} {} {}: its second triangle is made (FreeSurfer drops it after a degenerate first one)", q[0], q[1], q[2], q[3]),
                        );
                    }
                    va = vb;
                } else if n == 0 {
                    let bdeg = vb[0] == vb[1] || vb[0] == vb[2] || vb[1] == vb[2];
                    if !bdeg {
                        g.notices.add(
                            &FS_353,
                            format!("{fname}: quadrangle {} {} {} {}: its second triangle dropped after a degenerate first one", q[0], q[1], q[2], q[3]),
                        );
                    }
                }
                continue;
            }
            if fno >= mris.max_faces {
                // faces[fno] past the face array
                beyond = true;
            } else {
                mris.face_mut(fno as usize).v = va;
            }
            for m in 0..VERTICES_PER_FACE {
                let v = va[m];
                if v >= mris.nvertices {
                    if (v as usize) < room_v {
                        g.notices.add(
                            &FS_351,
                            format!("{fname}: a quadrangle's vertex {v} is not one of the surface's {} vertices", mris.nvertices),
                        );
                        if !g.notices.reproduce() {
                            fp.close();
                            log::error_printf(
                                g,
                                io.err,
                                ERROR_BADFILE,
                                &format!("{WHO}({fname}): face {fno} vertex {v} out of range"),
                            );
                            return Ok(None);
                        }
                    } else {
                        g.notices.add(
                            &FS_352,
                            format!("{fname}: a quadrangle's vertex {v} beyond the surface's room for {room_v} vertices is refused (FreeSurfer counts it in memory outside its arrays)"),
                        );
                        if !g.notices.reproduce() {
                            fp.close();
                            log::error_printf(
                                g,
                                io.err,
                                ERROR_BADFILE,
                                &format!("{WHO}({fname}): face {fno} vertex {v} out of range"),
                            );
                            return Ok(None);
                        }
                        continue;
                    }
                }
                if !count_face(g, &mut mris, v as usize, fname) {
                    fp.close();
                    log::error_printf(
                        g,
                        io.err,
                        ERROR_BADFILE,
                        &format!("{WHO}({fname}): vertex {v} is in more than 255 faces"),
                    );
                    return Ok(None);
                }
            }
            fno += 1;
            va = vb;
        }
    }
    if beyond {
        g.notices.add(
            &FS_349,
            format!("{fname}: {fno} faces where the surface has room for {}: FreeSurfer writes the others past its face array before its check fails", mris.max_faces),
        );
    }
    if fno > mris.max_faces {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &file,
            6443,
            "fno <= mris->max_faces",
        ));
    }
    alloc::grow_nfaces(g, io.out, io.err, &mut mris, fno)?;
    mris.useRealRAS = 0;
    if !read_tags(g, io, &mut fp, &mut mris, fname, true)? {
        return Ok(None);
    }
    fp.close();
    Ok(Some(mris))
}

/// The writers' command-line tags: each of the first `ncmds` lines with its NUL (FS-350: a line
/// the surface does not hold).
fn write_cmdlines(g: &mut Globals, fp: &mut CFile, mris: &Mris, fname: &str) -> Result<(), Fatal> {
    for i in 0..nonneg(mris.ncmds) {
        let line = mris.cmdlines.get(i).and_then(|c| c.as_ref());
        let Some(line) = line else {
            g.notices.add(
                &FS_350,
                format!("{fname}: command line {i} of the surface's {} is missing: not written (FreeSurfer crashes)", mris.ncmds),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            continue;
        };
        let mut b = line.clone();
        b.push(0);
        tag_write(g, fp, TAG_CMDLINE, &b, b.len() as i64);
    }
    Ok(())
}

/// The writers' group-average area tag (unless the area is below `FLT_EPSILON` in size), with
/// `writing group avg surface area <a/100, %2.0f> cm^2 into surface file` on standard output.
fn write_group_area(g: &mut Globals, io: &mut Io, fp: &mut CFile, mris: &Mris) {
    if mris.group_avg_surface_area.abs() < f32::EPSILON {
        return;
    }
    let _ = writeln!(
        io.out,
        "writing group avg surface area {} cm^2 into surface file",
        fmt_f(f64::from(mris.group_avg_surface_area) / 100.0, 2, 0)
    );
    let mut here = 0;
    tag_write_start(g, fp, TAG_GROUP_AVG_SURFACE_AREA, &mut here, 4);
    fwrite_float(fp, mris.group_avg_surface_area);
    tag_write_end(fp, here);
}

/// `MRISwriteTriangularSurface(mris, fname)`: the binary triangle file: the magic number, unless
/// `TRIANGULARSURFACE_NOEXTRA_WRITE` is set `created by <USER, else LOGNAME, else UNKNOWN> on
/// <currentDateTime()>` and a blank line (when it is set, `[INFO]: NO TRIANGULARSURFACE EXTRA -
/// created by line & cmds` on standard output), the counts, three big-endian floats per vertex,
/// three `int`s per face, the real-RAS flag, the volume geometry, the group-average area, the
/// command lines (not with `TRIANGULARSURFACE_NOEXTRA_WRITE`), then the three space tags
/// ([`write_triangular_surface_tags`]). With `DIAG_SHOW` and verbose diagnostics `writing surface
/// file <file>, created by <user> on <date>.` first. `ERROR_BADFILE` when the file cannot be
/// created (`MRISwriteTriangularSurface(<file>): can't create file` and its newline); else
/// `NO_ERROR`. FS-350.
pub fn write_triangular_surface(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let user: Vec<u8> = io
        .env
        .get("USER")
        .or_else(|| io.env.get("LOGNAME"))
        .map_or_else(|| b"UNKNOWN".to_vec(), |v| v.as_encoded_bytes().to_vec());
    let noextra = io.env.get("TRIANGULARSURFACE_NOEXTRA_WRITE").is_some();
    if noextra {
        let _ = writeln!(
            io.out,
            "[INFO]: NO TRIANGULARSURFACE EXTRA - created by line & cmds"
        );
    }
    let cdt = current_date_time(io.env, true);
    if verbose_show(g) {
        let mut m = format!("writing surface file {fname}, created by ").into_bytes();
        m.extend_from_slice(&user);
        m.extend_from_slice(format!(" on {cdt}.\n").as_bytes());
        let _ = io.out.write_all(&m);
    }
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("MRISwriteTriangularSurface({fname}): can't create file\n"),
            );
            return Ok(ERROR_BADFILE);
        }
    };
    fwrite3(&mut fp, TRIANGLE_FILE_MAGIC_NUMBER);
    if !noextra {
        let mut line = b"created by ".to_vec();
        line.extend_from_slice(&user);
        line.extend_from_slice(format!(" on {cdt}\n\n").as_bytes());
        fp.put_bytes(&line);
    }
    fwrite_int(&mut fp, mris.nvertices);
    fwrite_int(&mut fp, mris.nfaces);
    for k in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(k);
        fwrite_float(&mut fp, v.x);
        fwrite_float(&mut fp, v.y);
        fwrite_float(&mut fp, v.z);
    }
    for k in 0..nonneg(mris.nfaces) {
        for n in 0..VERTICES_PER_FACE {
            fwrite_int(&mut fp, mris.face(k).v[n]);
        }
    }
    fwrite_int(&mut fp, TAG_OLD_USEREALRAS);
    fwrite_int(&mut fp, mris.useRealRAS);
    fwrite_int(&mut fp, TAG_OLD_SURF_GEOM);
    write_vol_geom(&mut fp, &mris.vg);
    write_group_area(g, io, &mut fp, mris);
    if !noextra {
        write_cmdlines(g, &mut fp, mris, fname)?;
    }
    write_triangular_surface_tags(g, io, mris, &mut fp)?;
    fp.close();
    Ok(NO_ERROR)
}

/// `__MRISwriteTriangularSurfaceTags(mris, fp)`: the space of the coordinates and the matrix to
/// the other space, as GIFTI's `DataSpace`, `MatrixData` and `TransformedSpace` (no NUL bytes;
/// the matrix as `TAGwriteMatrix`'s text). Real-RAS coordinates (scanner space): with a valid
/// volume geometry `NIFTI_XFORM_SCANNER_ANAT`, `T * inverse(S)` (S the scanner voxel-to-RAS
/// matrix, T the tkregister one), `NIFTI_XFORM_UNKNOWN`; without, the identity and
/// `NIFTI_XFORM_SCANNER_ANAT` both. Otherwise (tkregister space): with a valid geometry
/// `NIFTI_XFORM_UNKNOWN`, `S * inverse(T)`, `NIFTI_XFORM_SCANNER_ANAT`; without, the surface's
/// 4 x 4 Talairach matrix (`SRASToTalSRAS_`) and `NIFTI_XFORM_TALAIRACH` when it has one, else
/// the identity and `NIFTI_XFORM_UNKNOWN` both. A product that cannot be made (an inverse that
/// fails) prints `[ERROR] __MRISwriteTriangularSurfaceTags(): couldn't obtain MatrixData` and
/// ends the program (`exit(1)`).
pub fn write_triangular_surface_tags(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fp: &mut CFile,
) -> Result<(), Fatal> {
    const UNKNOWN: &str = "NIFTI_XFORM_UNKNOWN";
    const SCANNER: &str = "NIFTI_XFORM_SCANNER_ANAT";
    let mut dataspace = UNKNOWN;
    let mut transformedspace = UNKNOWN;
    let mut matrixdata: Option<Matrix> = None;
    let new = |r: Ret<Matrix>| r.into_new();
    if mris.useRealRAS != 0 {
        if mris.vg.valid != 0 {
            let s = vg_i_to_r(g, io.out, io.err, &mris.vg)?;
            let t = tkr_vox2ras_from_vol_geom(g, io.out, io.err, &mris.vg)?;
            let sinv = new(matrix_inverse(g, io.out, io.err, Some(&s), None)?);
            matrixdata = new(matrix_multiply(
                g,
                io.out,
                io.err,
                Some(&t),
                sinv.as_ref(),
                None,
            )?);
            dataspace = SCANNER;
            transformedspace = UNKNOWN;
        } else {
            matrixdata = new(matrix_identity(g, io.out, io.err, 4, None)?);
            dataspace = SCANNER;
            transformedspace = SCANNER;
        }
    } else if mris.vg.valid != 0 {
        let s = vg_i_to_r(g, io.out, io.err, &mris.vg)?;
        let t = tkr_vox2ras_from_vol_geom(g, io.out, io.err, &mris.vg)?;
        let tinv = new(matrix_inverse(g, io.out, io.err, Some(&t), None)?);
        matrixdata = new(matrix_multiply(
            g,
            io.out,
            io.err,
            Some(&s),
            tinv.as_ref(),
            None,
        )?);
        dataspace = UNKNOWN;
        transformedspace = SCANNER;
    } else if let Some(m) = mris.SRASToTalSRAS_.as_ref()
        && m.rows == 4
        && m.cols == 4
    {
        matrixdata = new(matrix_copy(g, io.out, io.err, Some(m), None)?);
        dataspace = UNKNOWN;
        transformedspace = "NIFTI_XFORM_TALAIRACH";
    }
    if matrixdata.is_none() && dataspace != transformedspace {
        let _ = writeln!(
            io.out,
            "[ERROR] __MRISwriteTriangularSurfaceTags(): couldn't obtain MatrixData"
        );
        return Err(Fatal::Exit(1));
    }
    let matrixdata = match matrixdata {
        Some(m) => m,
        None => new(matrix_identity(g, io.out, io.err, 4, None)?).expect("a new matrix"),
    };
    tag_write(
        g,
        fp,
        TAG_SURF_DATASPACE,
        dataspace.as_bytes(),
        dataspace.len() as i64,
    );
    tag_write_matrix(g, fp, &matrixdata, TAG_SURF_MATRIXDATA);
    tag_write(
        g,
        fp,
        TAG_SURF_TRANSFORMEDSPACE,
        transformedspace.as_bytes(),
        transformedspace.len() as i64,
    );
    Ok(())
}

/// `__MRISwriteQuadrangleFile(mris, fname)`: the quadrangle file of floats
/// (`NEW_QUAD_FILE_MAGIC_NUMBER`; FreeSurfer's build writes no other): the magic number, the
/// vertex count and the quadrangle count (3-byte each), three big-endian floats per vertex, then
/// each face, in order, combined with the next into a quadrangle where they share an edge in
/// opposite directions ([`quad_combine`]), else written as the degenerate quadrangle `v0 v1 v1 v2`
/// (four 3-byte numbers each), then the real-RAS flag, the volume geometry, the group-average area
/// and the command lines. `ERROR_BADFILE` when the file cannot be created
/// (`__MRISwriteQuadrangleFile(<file>): can't create file` and its newline); else `NO_ERROR`.
/// FS-350.
pub fn write_quadrangle_file(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("__MRISwriteQuadrangleFile({fname}): can't create file\n"),
            );
            return Ok(ERROR_BADFILE);
        }
    };
    fwrite3(&mut fp, NEW_QUAD_FILE_MAGIC_NUMBER);
    fwrite3(&mut fp, mris.nvertices);
    let nfaces = nonneg(mris.nfaces);
    let quads = |write: &mut dyn FnMut([i32; 4])| {
        let mut k = 0;
        while k < nfaces {
            let mut q = [0i32; 4];
            if k + 1 < nfaces && quad_combine(&mut q, &mris.face(k).v, &mris.face(k + 1).v) {
                k += 1;
            } else {
                let v = mris.face(k).v;
                q = [v[0], v[1], v[1], v[2]];
            }
            write(q);
            k += 1;
        }
    };
    let mut needed = 0;
    quads(&mut |_| needed += 1);
    fwrite3(&mut fp, needed);
    for k in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(k);
        fwrite_float(&mut fp, v.x);
        fwrite_float(&mut fp, v.y);
        fwrite_float(&mut fp, v.z);
    }
    quads(&mut |q| {
        for x in q {
            fwrite3(&mut fp, x);
        }
    });
    fwrite_int(&mut fp, TAG_OLD_USEREALRAS);
    fwrite_int(&mut fp, mris.useRealRAS);
    fwrite_int(&mut fp, TAG_OLD_SURF_GEOM);
    write_vol_geom(&mut fp, &mris.vg);
    write_group_area(g, io, &mut fp, mris);
    write_cmdlines(g, &mut fp, mris, fname)?;
    fp.close();
    Ok(NO_ERROR)
}

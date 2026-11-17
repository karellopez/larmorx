// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The patch readers (queue item 3.1-47, relay 45 of track D): FreeSurfer 8.2.0's
//! `utils/mrisurf_io.cpp`'s `MRISreadPatchNoRemove` (959) and `MRISreadPatch` (1222): a patch (the
//! surface's vertices kept, their positions and borders) read from an ASCII patch file, a binary
//! patch in the old (positions in hundredths of a millimetre) or the new form (`float` positions
//! after a negative version number), a label (its vertices kept) or a volume of one voxel per
//! vertex (the vertices above 0.5 kept); then the ripped vertices removed, the surface a cut patch
//! and its metric properties recomputed. The boundary normals' computation and smoothing
//! (`mrisComputeBoundaryNormals`, `mrisSmoothBoundaryNormals`) have empty bodies in FreeSurfer
//! 8.2.0.
//!
//! Files are C streams ([`CFile`]) opened in `dir`; a failed `fopen` or read sets `errno`, which
//! the error that follows prints first.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-1006** (wrong result): a GIFTI name is read into a new surface FreeSurfer never returns
//!   (the caller's surface stays whole; a file it cannot read crashes the program). Rule A: by
//!   default refused with an error (`ERROR_UNSUPPORTED`); `reproduce` as FreeSurfer.
//! - **FS-1007** (a crash): an ASCII patch of no line. Rule B: an error (`ERROR_BADFILE`) by
//!   default; `reproduce` crashes (status 139).
//! - **FS-1008** (a parsing quirk): a point line without a vertex number, or a coordinate line of
//!   fewer than three numbers, takes the previous point's values. Rule B: an error by default;
//!   `reproduce` as FreeSurfer.
//! - **FS-1009** (messages, kept): the bad vertex number printed one below the file's; a label's or
//!   volume's vertex number equal to the vertex count printed as greater.
//! - **FS-1010** (memory never written): the first line without a vertex count, the first point
//!   without a number or three coordinates. The same errors by default; `reproduce` reads 0 for
//!   them, noted as memory.
//! - fs.io's FS-49 and FS-167: a binary patch that ends before its numbers.

use std::path::Path;

use larmorx_core::defects::{FS_1006, FS_1007, FS_1008, FS_1009, FS_1010};
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::sse::{div, div64, mul};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{fread_float, fread_int, fread2};
use larmorx_freesurfer_io::label::label_read;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::aaa::{MrisStatus, SurfGlobals};
use crate::coords::{self, Io};
use crate::curvio::{build_file_name_read, fgetl_line, sync_read_errno};
use crate::dist::free_dists_but_not_orig;
use crate::mris::Mris;
use crate::names::{
    MRIS_ASCII_TRIANGLE_FILE, MRIS_GIFTI_FILE, MRIS_LABEL_FILE, MRIS_VOLUME_FILE,
    mris_file_name_type,
};
use crate::scanf::Scanf;
use crate::sfile::{c_str, nonneg, open, open_failed};
use crate::tritri::narrow;
use crate::{props5, props8, readwrite, toprest};

/// The official build's `mrisurf_io.cpp` (the place `MRISsetXYZ` reports).
fn io_cpp() -> String {
    format!(
        "{}/utils/mrisurf_io.cpp",
        larmorx_freesurfer_core::diag::BUILD_ROOT
    )
}

/// `100.0f * (float)npts / (float)mris->nvertices` as `%2.1f`.
fn percent(npts: i32, nvertices: i32) -> String {
    let p = div(mul(100.0, npts as f32), nvertices as f32);
    fmt_f(f64::from(p), 2, 1)
}

/// The vertex number of a patch file's point `i` (negative: a border vertex): the zero-based vertex
/// and its border flag; a vertex outside the surface ends the program (`MRISreadPatch: bad vertex #
/// (<k>) found in patch file`, `ERROR_BADFILE`; FS-1009: the zero-based number printed).
fn point_vertex(g: &mut Globals, io: &mut Io, mris: &Mris, i: i32) -> Result<(usize, i8), Fatal> {
    let (k, border) = if i < 0 {
        (i.wrapping_neg().wrapping_sub(1), 1)
    } else {
        (i.wrapping_sub(1), 0)
    };
    if k < 0 || k >= mris.nvertices {
        g.notices.add(
            &FS_1009,
            format!("MRISreadPatch: the bad vertex number {k} printed is the file's {i} made zero-based (kept)"),
        );
        log::error_exit(
            g,
            io.err,
            ERROR_BADFILE,
            &format!("MRISreadPatch: bad vertex # ({k}) found in patch file"),
        )?;
    }
    Ok((k as usize, border))
}

/// A point's vertex set from the file: border flag, kept (rip flag clear), its current position
/// (`MRISsetXYZ`); with `DIAG_SHOW` and `Gdiag_no` at the vertex, `vertex <k> read @ (%2.2f, %2.2f,
/// %2.2f)` on standard output.
#[allow(clippy::too_many_arguments)]
fn set_point(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    k: usize,
    border: i8,
    xyz: [f32; 3],
    line: i32,
) -> Result<(), Fatal> {
    {
        let v = mris.vertex_mut(k);
        v.border = border;
        v.ripflag = 0;
    }
    coords::set_xyz(
        g,
        io,
        mris,
        k as i32,
        xyz[0],
        xyz[1],
        xyz[2],
        &io_cpp(),
        line,
    )?;
    if k as i32 == g.gdiag_no && g.gdiag & DIAG_SHOW != 0 {
        let v = mris.vertex(k);
        let s = format!(
            "vertex {k} read @ ({}, {}, {})\n",
            fmt_f(f64::from(v.x), 2, 2),
            fmt_f(f64::from(v.y), 2, 2),
            fmt_f(f64::from(v.z), 2, 2)
        );
        let _ = io.out.write_all(s.as_bytes());
    }
    Ok(())
}

fn rip_all(mris: &mut Mris) {
    for k in 0..nonneg(mris.nvertices) {
        mris.vertex_mut(k).ripflag = 1;
    }
}

/// FS-1008 and FS-1010: a value the line does not hold. By default an error (`ERROR_BADFILE`,
/// returned); with `reproduce` FreeSurfer's: the previous point's (`first` false: FS-1008) or
/// memory it never wrote (`first`: FS-1010, read as 0).
fn missing(g: &mut Globals, io: &mut Io, first: bool, what: String) -> Option<i32> {
    if first {
        g.notices.add(
            &FS_1010,
            format!("{what}: FreeSurfer uses memory it never wrote (read as 0)"),
        );
    } else {
        g.notices.add(
            &FS_1008,
            format!("{what}: FreeSurfer takes the previous point's values"),
        );
    }
    if g.notices.reproduce() {
        return None;
    }
    log::error_printf(g, io.err, ERROR_BADFILE, &what);
    Some(ERROR_BADFILE)
}

/// The ASCII patch file (`MRIS_ASCII_TRIANGLE_FILE`): `Some(code)` when the read ends early.
fn read_ascii(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pname: &str,
    fname: &str,
) -> Result<Option<i32>, Fatal> {
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISreadPatchNoRemove({fname}): could not open file");
            return Ok(Some(open_failed(g, io, e, ERROR_NOFILE, &m)));
        }
    };
    let first = fgetl_line(&mut fp, 256);
    sync_read_errno(g, &fp);
    let Some((line, start)) = first else {
        let m = format!(
            "MRISreadPatchNoRemove({fname}): the file holds no line (FreeSurfer reads through a null pointer)"
        );
        g.notices.add(&FS_1007, m.clone());
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok(Some(ERROR_BADFILE));
    };
    let mut npts = 0i32;
    {
        let mut b = Bytes::new(&line[start..]);
        let mut sc = Scanf::new(&mut b);
        let e = &mut g.errno;
        let n = sc.d(e, true);
        sc.text(e, b" ");
        sc.s(e, usize::MAX, false);
        sc.finish(e);
        match n {
            Some(n) => npts = n,
            None => {
                let what =
                    format!("MRISreadPatchNoRemove({fname}): the first line holds no vertex count");
                if let Some(code) = missing(g, io, true, what) {
                    return Ok(Some(code));
                }
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let s = format!(
            "reading .ASC patch {pname} with {npts} vertices ({}% of total)\n",
            percent(npts, mris.nvertices)
        );
        let _ = io.out.write_all(s.as_bytes());
    }
    rip_all(mris);
    let (mut i, mut r) = (0i32, [0f64; 3]);
    for j in 0..npts.max(0) {
        let got = fgetl_line(&mut fp, 256);
        sync_read_errno(g, &fp);
        let Some((line, start)) = got else {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("MRISreadPatchNoRemove({fname}): could not read line for point {j}\n"),
            );
            return Ok(Some(ERROR_BADPARM));
        };
        {
            let mut b = Bytes::new(&line[start..]);
            let mut sc = Scanf::new(&mut b);
            let e = &mut g.errno;
            let n = sc.d(e, true);
            sc.text(e, b" ");
            sc.s(e, usize::MAX, false);
            sc.finish(e);
            match n {
                Some(n) => i = n,
                None => {
                    let what = format!(
                        "MRISreadPatchNoRemove({fname}): point {j} holds no vertex number"
                    );
                    if let Some(code) = missing(g, io, j == 0, what) {
                        return Ok(Some(code));
                    }
                }
            }
        }
        let (k, border) = point_vertex(g, io, mris, i)?;
        mris.vertex_mut(k).border = border;
        mris.vertex_mut(k).ripflag = 0;
        let got = fgetl_line(&mut fp, 256);
        sync_read_errno(g, &fp);
        let Some((line, start)) = got else {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!(
                    "MRISreadPatch({fname}): could not read 3 floating values line for point {j}\n"
                ),
            );
            return Ok(Some(ERROR_BADPARM));
        };
        let read = {
            let mut b = Bytes::new(&line[start..]);
            let mut sc = Scanf::new(&mut b);
            let e = &mut g.errno;
            let mut n = 0;
            for (c, slot) in r.iter_mut().enumerate() {
                if c > 0 {
                    sc.text(e, b" ");
                }
                if let Some(x) = sc.lf(e, true) {
                    *slot = x;
                    n += 1;
                }
            }
            sc.finish(e);
            n
        };
        if read < 3 {
            let what = format!(
                "MRISreadPatchNoRemove({fname}): point {j} holds fewer than 3 coordinates"
            );
            if let Some(code) = missing(g, io, j == 0, what) {
                return Ok(Some(code));
            }
        }
        set_point(
            g,
            io,
            mris,
            k,
            border,
            [narrow(r[0]), narrow(r[1]), narrow(r[2])],
            1041,
        )?;
    }
    fp.close();
    Ok(None)
}

/// The label (`MRIS_LABEL_FILE`): its vertices kept, the others ripped. A label that cannot be
/// read, or a vertex outside the surface (`ERROR: MRISreadPatch(): vno=<n> < 0`, `vno=<n> >
/// nvertices=<nv>`; FS-1009 for one equal to the count), ends the program (status 1).
fn read_label(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> Result<(), Fatal> {
    let _ = io
        .out
        .write_all(format!("Reading label {fname} as a patch\n").as_bytes());
    let Some(lab) = label_read(g, io.env, io.out, io.err, dir, None, fname)? else {
        return Err(Fatal::Exit(1));
    };
    rip_all(mris);
    for lv in lab.lv.iter().take(nonneg(lab.n_points)) {
        let vno = lv.vno;
        if vno < 0 {
            let _ = io
                .out
                .write_all(format!("ERROR: MRISreadPatch(): vno={vno} < 0\n").as_bytes());
            return Err(Fatal::Exit(1));
        }
        if vno >= mris.nvertices {
            if vno == mris.nvertices {
                g.notices.add(
                    &FS_1009,
                    format!("MRISreadPatch(): vertex {vno}, the surface's vertex count, printed as greater than it (kept)"),
                );
            }
            let _ = io.out.write_all(
                format!(
                    "ERROR: MRISreadPatch(): vno={vno} > nvertices={}\n",
                    mris.nvertices
                )
                .as_bytes(),
            );
            return Err(Fatal::Exit(1));
        }
        mris.vertex_mut(vno as usize).ripflag = 0;
    }
    Ok(())
}

/// The volume (`MRIS_VOLUME_FILE`, `MRIread`): one voxel per vertex along its first row, a vertex
/// kept when its voxel is above 0.5 (`double`). A volume that cannot be read, or whose width is not
/// the vertex count (`ERROR: MRISreadPatch(): dimension mismatch, width=<w>, nvertices=<nv>`), ends
/// the program (status 1).
fn read_volume(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> Result<(), Fatal> {
    let _ = io
        .out
        .write_all(format!("Reading volume {fname} as a patch, thresh=0.5\n").as_bytes());
    let Some(mri) = mriio::mri_read_volume(g, io.out, io.err, io.env, dir, fname)? else {
        return Err(Fatal::Exit(1));
    };
    if mri.width != mris.nvertices {
        let _ = io.out.write_all(
            format!(
                "ERROR: MRISreadPatch(): dimension mismatch, width={}, nvertices={}\n",
                mri.width, mris.nvertices
            )
            .as_bytes(),
        );
        return Err(Fatal::Exit(1));
    }
    for k in 0..nonneg(mris.nvertices) {
        let v = f64::from(mri_get_vox_val(g, io.err, &mri, k as i32, 0, 0, 0, 0)?);
        mris.vertex_mut(k).ripflag = if v > 0.5 { 0 } else { 1 };
    }
    Ok(())
}

/// The binary patch (any other name): its first `int` (big-endian) the point count of the old
/// form, or, negative, the version of the new one followed by the count. Each point: its `int`
/// vertex number (negative: a border vertex), then its position as three big-endian `short`s in
/// hundredths of a millimetre (old: `n / 100.0` in `double`, rounded to `float`) or three `float`s
/// (new). `Some(code)` when the file cannot be opened.
fn read_binary(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pname: &str,
    fname: &str,
) -> Result<Option<i32>, Fatal> {
    let mut fp: CFile = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISreadPatchNoRemove({fname}): could not open file");
            return Ok(Some(open_failed(g, io, e, ERROR_NOFILE, &m)));
        }
    };
    let mut npts = fread_int(g, &mut fp);
    sync_read_errno(g, &fp);
    let old = npts >= 0;
    if !old {
        npts = fread_int(g, &mut fp);
        sync_read_errno(g, &fp);
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let kind = if old {
            "binary"
        } else {
            "new surface format"
        };
        let s = format!(
            "reading {kind} patch {pname} with {npts} vertices ({}% of total)\n",
            percent(npts, mris.nvertices)
        );
        let _ = io.out.write_all(s.as_bytes());
    }
    rip_all(mris);
    for _ in 0..npts.max(0) {
        let i = fread_int(g, &mut fp);
        sync_read_errno(g, &fp);
        let (k, border) = point_vertex(g, io, mris, i)?;
        mris.vertex_mut(k).border = border;
        mris.vertex_mut(k).ripflag = 0;
        let (xyz, line) = if old {
            let mut c = [0f32; 3];
            for slot in &mut c {
                let (_, n) = fread2(g, &mut fp);
                sync_read_errno(g, &fp);
                *slot = narrow(div64(f64::from(n), 100.0));
            }
            (c, 1126)
        } else {
            let mut c = [0f32; 3];
            for slot in &mut c {
                *slot = fread_float(g, io.err, &mut fp);
                sync_read_errno(g, &fp);
            }
            (c, 1180)
        };
        set_point(g, io, mris, k, border, xyz, line)?;
    }
    fp.close();
    Ok(None)
}

/// The end of `MRISreadPatchNoRemove`: a vertex without faces or neighbours ripped; the ripped
/// vertices removed (`MRISremoveRipped`); the bounding box; a patch (`patch` 1, status
/// `MRIS_CUT`); the metric properties (`MRISupdateSurface`).
fn finish(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    rip_lonely(mris);
    toprest::remove_ripped(g, sg, io, mris)?;
    coords::compute_surface_dimensions(mris);
    mris.patch = 1;
    mris.status = MrisStatus::CUT;
    props5::compute_boundary_normals(mris);
    props8::update_surface(g, sg, io, dir, mris)?;
    Ok(())
}

/// Each vertex without faces (`num` 0) or neighbours (`vnum` 0) ripped.
fn rip_lonely(mris: &mut Mris) {
    for k in 0..nonneg(mris.nvertices) {
        let t = mris.topology(k);
        if t.num == 0 || t.vnum == 0 {
            mris.vertex_mut(k).ripflag = 1;
        }
    }
}

/// `MRISreadPatchNoRemove(mris, pname, dotkrRASConvert)`: the file to read for `pname`
/// (`MRISbuildFileName_read`), the surface's distances released (`MRISfreeDistsButNotOrig`), then
/// by the name's type ([`mris_file_name_type`]): an ASCII patch (`.asc`: its first line the point
/// count, then two lines per point, its vertex number and its three coordinates, each line read by
/// `fgetl` of 256 bytes), a label (`.label`), a volume, a GIFTI file (FS-1006), else a binary patch;
/// with `DIAG_SHOW` the count (`reading .ASC patch <pname> with <n> vertices (%2.1f% of total)`,
/// `reading binary patch ...`, `reading new surface format patch ...`). Every vertex not in the
/// patch is ripped. Then the end ([`finish`]). Returns `NO_ERROR`; a file that cannot be opened:
/// `ERROR_NOFILE` after `MRISreadPatchNoRemove(<file>): could not open file`; an ASCII file that
/// ends before a point: `ERROR_BADPARM` after `MRISreadPatchNoRemove(<file>): could not read line
/// for point <j>` or `MRISreadPatch(<file>): could not read 3 floating values line for point <j>`
/// (each message with its newline).
pub fn read_patch_no_remove(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pname: &str,
    dotkr_ras_convert: bool,
) -> Result<i32, Fatal> {
    let (_, fname) = build_file_name_read(g, io, mris, pname);
    let ty = mris_file_name_type(g, &fname);
    let pname = c_str(pname);
    free_dists_but_not_orig(mris);
    let early = match ty {
        MRIS_GIFTI_FILE => {
            let m = format!(
                "MRISreadPatchNoRemove({fname}): a GIFTI patch is read into a new surface FreeSurfer does not return (the surface given stays whole): refused"
            );
            g.notices.add(&FS_1006, m.clone());
            if !g.notices.reproduce() {
                log::error_printf(g, io.err, ERROR_UNSUPPORTED, &m);
                return Ok(ERROR_UNSUPPORTED);
            }
            let Some(mut other) = readwrite::read(g, sg, io, dir, &fname, dotkr_ras_convert)?
            else {
                // FreeSurfer goes on with the null surface
                return Err(Fatal::Exit(139));
            };
            finish(g, sg, io, dir, &mut other)?;
            return Ok(NO_ERROR);
        }
        MRIS_ASCII_TRIANGLE_FILE => read_ascii(g, io, dir, mris, pname, &fname)?,
        MRIS_LABEL_FILE => {
            read_label(g, io, dir, mris, &fname)?;
            None
        }
        MRIS_VOLUME_FILE => {
            read_volume(g, io, dir, mris, &fname)?;
            None
        }
        _ => read_binary(g, io, dir, mris, pname, &fname)?,
    };
    if let Some(code) = early {
        return Ok(code);
    }
    finish(g, sg, io, dir, mris)?;
    Ok(NO_ERROR)
}

/// `MRISreadPatch(mris, pname, dotkrRASConvert)`: [`read_patch_no_remove`] (its error returned);
/// then the ripped vertices removed again, each vertex without faces or neighbours ripped, the
/// metric properties (`MRISupdateSurface`). Returns `NO_ERROR`.
pub fn read_patch(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pname: &str,
    dotkr_ras_convert: bool,
) -> Result<i32, Fatal> {
    let ret = read_patch_no_remove(g, sg, io, dir, mris, pname, dotkr_ras_convert)?;
    if ret != NO_ERROR {
        return Ok(ret);
    }
    toprest::remove_ripped(g, sg, io, mris)?;
    rip_lonely(mris);
    props8::update_surface(g, sg, io, dir, mris)?;
    Ok(NO_ERROR)
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s patch, vertex-value and normal files (`fs.surfio`, queue items 3.1-47
//! and 3.1-48, relay 36 of track D): `MRISwritePatch` 1255 (a patch in the format its name says,
//! the binary patch format otherwise), `MRISwriteIntoVolume` 412 (one vertex field into a volume),
//! `MRISreadValues` 1844 with `MRISreadValuesIntoArray` 1872 (the values from a volume, a
//! curvature file or a w file), `MRISwriteNormals` 2690, `MRISreadNormals` 2711,
//! `MRISwriteWhiteNormals` 2734 and `MRISwritePrincipalDirection` 2758 (three vectors per vertex
//! as a volume of three frames); since relay 37 `MRISwriteValues` 721 (the values as a w file or a
//! volume) and `MRISwriteAreaErrorToValFile` 586 (FS-633).
//!
//! The defects: FS-619 (`MRISwriteIntoVolume` without a volume makes one of the wrong shape and
//! refuses it), FS-620 (`MRISreadValuesIntoArray`'s frame message reads the freed header: a
//! crash), FS-621 (its w-file reader does not read the value of a vertex number out of range: the
//! rest of the file is read out of step), FS-622 (its copy of a name of 1,023 bytes or more has no
//! end), FS-623 (its volume loop writes past the values for a volume whose voxel count overflows
//! an `int`), FS-126 (its w-file name of 1,024 bytes or more, after `.w`: an abort).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::path::Path;

use larmorx_core::defects::{FS_126, FS_619, FS_620, FS_621, FS_622, FS_623, FS_633, FS_634};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_io::fio::{fread_float, fread3, fwrite_float, fwrite_int, fwrite2, fwrite3};
use larmorx_freesurfer_io::identify::{self, MRI_VOLUME_TYPE_UNKNOWN};
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, Mri, mri_alloc, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::curvio;
use crate::facenorm;
use crate::formats;
use crate::mris::{Mris, Vertex};
use crate::names::{
    MRIS_ASCII_TRIANGLE_FILE, MRIS_GEO_TRIANGLE_FILE, MRIS_STL_FILE, mris_file_name_type,
};
use crate::readwrite;
use crate::sfile::{self, STRLEN, c_str, nonneg, open, open_failed, strcpy_checked};
use crate::stl;
use crate::surfmri;
use crate::surfread::verbose_show;

/// `VERTEX_VALS` (`VERTEX_VAL`), `VERTEX_AREA`, `VERTEX_CURV`, `VERTEX_ANNOTATION`, `VERTEX_DX`,
/// `VERTEX_DY`, `VERTEX_DZ`, `VERTEX_LOGODDS` (`mrisurf.h` lines 1610-1621): the fields
/// [`write_into_volume`] writes.
pub const VERTEX_VALS: i32 = 1;
pub const VERTEX_AREA: i32 = 2;
pub const VERTEX_CURV: i32 = 3;
pub const VERTEX_ANNOTATION: i32 = 5;
pub const VERTEX_DX: i32 = 6;
pub const VERTEX_DY: i32 = 7;
pub const VERTEX_DZ: i32 = 8;
pub const VERTEX_LOGODDS: i32 = 10;

// ---- the patch --------------------------------------------------------------------------------

/// `MRISwritePatch(mris, fname)`: the surface's unripped vertices as a patch, in the format the
/// name says ([`mris_file_name_type`]): an ASCII patch ([`readwrite::write_patch_ascii`]), the
/// movie.byu format ([`formats::write_geo`]) or an STL file ([`stl::write_stl`]: 1 written, 0
/// not); any other name the binary patch format: -1 and the count of unripped vertices as
/// big-endian `int`s, then per unripped vertex its number plus 1 (negative for a border vertex) as
/// an `int` and its coordinates as big-endian `float`s. With `DIAG_SHOW` and verbose: `writing
/// surface patch with npts=<n> to <file>` on standard output first. Returns `NO_ERROR`, or
/// `ERROR_NOFILE` after `MRISwritePatch: can't create file <file>` (the text of `errno` first)
/// when the file cannot be created.
pub fn write_patch(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> i32 {
    let fname = c_str(fname);
    let ty = mris_file_name_type(g, fname);
    if ty == MRIS_ASCII_TRIANGLE_FILE {
        return readwrite::write_patch_ascii(g, io, dir, mris, fname);
    } else if ty == MRIS_GEO_TRIANGLE_FILE {
        return formats::write_geo(g, io, dir, mris, fname);
    } else if ty == MRIS_STL_FILE {
        return i32::from(stl::write_stl(g, dir, mris, fname));
    }
    let nv = nonneg(mris.nvertices);
    let npts = (0..nv)
        .filter(|&k| mris.vertex(k).ripflag == 0)
        .fold(0i32, |n, _| n.wrapping_add(1));
    if verbose_show(g) {
        let _ = writeln!(io.out, "writing surface patch with npts={npts} to {fname}");
    }
    let mut fp = match open(dir, fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISwritePatch: can't create file {fname}\n");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    fwrite_int(&mut fp, -1);
    fwrite_int(&mut fp, npts);
    for k in 0..nv {
        let v = mris.vertex(k);
        if v.ripflag != 0 {
            continue;
        }
        let k1 = (k as i32).wrapping_add(1);
        let i = if v.border != 0 { k1.wrapping_neg() } else { k1 };
        fwrite_int(&mut fp, i);
        fwrite_float(&mut fp, v.x);
        fwrite_float(&mut fp, v.y);
        fwrite_float(&mut fp, v.z);
    }
    fp.close();
    NO_ERROR
}

// ---- one field into a volume ---------------------------------------------------------------------

/// What `MRISwriteIntoVolume` gives back: the volume it was given (filled), a new one, or `NULL`.
#[derive(Debug)]
pub enum IntoVolume {
    /// The volume given.
    Given,
    /// A volume made by the call.
    New(Box<Mri>),
    /// `NULL` (after its message; a volume the call made is dropped, as FreeSurfer leaks it).
    Null,
}

/// `MRISwriteIntoVolume(mris, mri, which)`: each unripped vertex's field `which` (`VERTEX_VALS`
/// or `VERTEX_LOGODDS`: `val`; `VERTEX_DX`, `VERTEX_DY`, `VERTEX_DZ`: `dx`, `dy`, `dz`;
/// `VERTEX_CURV`: `curv`; `VERTEX_ANNOTATION`: the annotation as a `float`; `VERTEX_AREA`: `area`)
/// into the volume: at column `vno` of frame 0 when the volume's width is the vertex count, else at
/// frame `vno` of the first voxel when its frame count is; else `MRISwriteIntoVolume: nvertices
/// <n> doesn't match with in MRI <w> x <h> x <d> x <f>` (`ERROR_BADPARM`) and `NULL`. Another
/// field ends the program at the first unripped vertex (`MRISwriteIntoVolume: unsupported type
/// <which>`, `ERROR_UNSUPPORTED`). Without a volume, a float volume of one voxel per vertex is
/// made (FS-619, rule A: FreeSurfer makes it 1 x 1 x `<vertices>`, so a surface of two vertices
/// or more is refused at its first unripped vertex, and its callers then crash writing `NULL`;
/// larmorx makes it `<vertices>` x 1 x 1, `reproduce` FreeSurfer's).
pub fn write_into_volume(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mri: Option<&mut Mri>,
    which: i32,
) -> Result<IntoVolume, Fatal> {
    let nv = mris.nvertices;
    let mut made = None;
    let given = mri.is_some();
    let mri: &mut Mri = match mri {
        Some(m) => m,
        None => {
            let (w, d) = if nv > 1 {
                g.notices.add(
                    &FS_619,
                    format!("MRISwriteIntoVolume: a new volume of {nv} x 1 x 1 for {nv} vertices (FreeSurfer makes 1 x 1 x {nv} and refuses it)"),
                );
                if g.notices.reproduce() {
                    (1, nv)
                } else {
                    (nv, 1)
                }
            } else {
                (1, nv)
            };
            made.insert(mri_alloc(g, io.out, io.err, w, 1, d, MRI_FLOAT)?)
        }
    };
    for vno in 0..nonneg(nv) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let val = match which {
            VERTEX_DX => v.dx,
            VERTEX_DY => v.dy,
            VERTEX_DZ => v.dz,
            VERTEX_LOGODDS | VERTEX_VALS => v.val,
            VERTEX_CURV => v.curv,
            VERTEX_ANNOTATION => v.annotation as f32,
            VERTEX_AREA => v.area,
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRISwriteIntoVolume: unsupported type {which}"),
                )?;
                0.0
            }
        };
        if mri.width == nv {
            mri_set_vox_val(g, io.err, mri, vno as i32, 0, 0, 0, val, 0)?;
        } else if mri.nframes == nv {
            mri_set_vox_val(g, io.err, mri, 0, 0, 0, vno as i32, val, 0)?;
        } else {
            let m = format!(
                "MRISwriteIntoVolume: nvertices {nv} doesn't match with in MRI {} x {} x {} x {}\n",
                mri.width, mri.height, mri.depth, mri.nframes
            );
            log::error_printf(g, io.err, ERROR_BADPARM, &m);
            return Ok(IntoVolume::Null);
        }
    }
    Ok(match made {
        Some(m) if !given => IntoVolume::New(Box::new(m)),
        _ => IntoVolume::Given,
    })
}

// ---- three vectors per vertex -------------------------------------------------------------------

/// A float volume of `<vertices>` x 1 x 1 and three frames, each vertex's vector from `pick` in
/// its column, written by `MRIwrite` (its status not looked at: `NO_ERROR`). `MRIallocSequence`
/// never returns `NULL` (a C++ constructor): the `could not allocate volume` branch is dead.
fn write_vectors(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
    pick: fn(&Vertex) -> [f32; 3],
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let mut mri = mri_alloc_sequence(g, io.out, io.err, mris.nvertices, 1, 1, MRI_FLOAT, 3)?;
    for vno in 0..nonneg(mris.nvertices) {
        let p = pick(mris.vertex(vno));
        for (f, x) in p.into_iter().enumerate() {
            mri_set_vox_val(g, io.err, &mut mri, vno as i32, 0, 0, f as i32, x, 0)?;
        }
    }
    mriio::mri_write(
        g,
        io.out,
        io.err,
        io.env,
        dir,
        &mut mri,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(NO_ERROR)
}

/// `MRISwriteNormals(mris, fname)`: the vertices' normals (`nx`, `ny`, `nz`) as a volume of
/// `<vertices>` x 1 x 1 and three frames ([`write_vectors`]). Returns `NO_ERROR`.
pub fn write_normals(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    write_vectors(g, io, dir, mris, fname, |v| [v.nx, v.ny, v.nz])
}

/// `MRISwriteWhiteNormals(mris, fname)`: the white surface's normals (`wnx`, `wny`, `wnz`) as
/// [`write_normals`] writes the normals. Returns `NO_ERROR`.
pub fn write_white_normals(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    write_vectors(g, io, dir, mris, fname, |v| [v.wnx, v.wny, v.wnz])
}

/// `MRISwritePrincipalDirection(mris, dir_index, fname)`: the first principal curvature
/// direction (`e1x`, `e1y`, `e1z`) when `dir_index` is 1, else the second (`e2x`, `e2y`, `e2z`), as
/// [`write_normals`] writes the normals. Returns `NO_ERROR`.
pub fn write_principal_direction(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    dir_index: i32,
    fname: &str,
) -> Result<i32, Fatal> {
    if dir_index == 1 {
        write_vectors(g, io, dir, mris, fname, |v| [v.e1x, v.e1y, v.e1z])
    } else {
        write_vectors(g, io, dir, mris, fname, |v| [v.e2x, v.e2y, v.e2z])
    }
}

/// `MRISreadNormals(mris, fname)`: the vertices' normals from a volume read by `MRIread`
/// ([`mriio::mri_read_volume`]): vertex `vno`'s `nx`, `ny`, `nz` from column `vno` of frames 0, 1
/// and 2 (`MRIgetVoxVal`'s answer outside the volume: FS-52, FS-53). Returns `NO_ERROR`, or
/// `ERROR_NOFILE` after `MRISreadNormals(<file>): could not open volume` when no volume was read.
pub fn read_normals(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let Some(mri) = mriio::mri_read_volume(g, io.out, io.err, io.env, dir, fname)? else {
        let m = format!("MRISreadNormals({fname}): could not open volume");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    };
    for vno in 0..nonneg(mris.nvertices) {
        let nx = mri_get_vox_val(g, io.err, &mri, vno as i32, 0, 0, 0, 0)?;
        let ny = mri_get_vox_val(g, io.err, &mri, vno as i32, 0, 0, 1, 0)?;
        let nz = mri_get_vox_val(g, io.err, &mri, vno as i32, 0, 0, 2, 0)?;
        let v = mris.vertex_mut(vno);
        (v.nx, v.ny, v.nz) = (nx, ny, nz);
    }
    Ok(NO_ERROR)
}

// ---- the values ---------------------------------------------------------------------------------

/// `MRISreadValues(mris, sname)`: the values [`read_values_into_array`] reads for the surface's
/// vertex count into the vertices' `val`. Returns its status (`NO_ERROR` when the values were
/// set).
pub fn read_values(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
) -> Result<i32, Fatal> {
    let (code, array) = read_values_into_array(g, sg, io, dir, Some(sname), mris.nvertices)?;
    if code != NO_ERROR {
        return Ok(code);
    }
    let array = array.unwrap_or_default();
    for vno in 0..nonneg(mris.nvertices) {
        mris.vertex_mut(vno).val = array.get(vno).copied().unwrap_or(0.0);
    }
    Ok(NO_ERROR)
}

/// `MRISreadValuesIntoArray(sname, in_array_size, &out_array)`: `in_array_size` values from
/// `sname`, tried in turn as
///
/// 1. a volume (`mri_identify` knows its type): its header first (`MRIreadHeader`; `ERROR_BADFILE`
///    when it gives none), the frame `MRISgetReadFrame()` gives (with `DIAG_SHOW` and verbose:
///    `MRISreadValues: frame=<f>`); a frame not below the header's frame count gives `ERROR:
///    attempted to read frame <f> from <file>`, `but this file only has <n> frames.`
///    (`ERROR_BADFILE`; FS-620, rule A: FreeSurfer reads the count from the header it has just
///    freed and crashes, signal 11, before printing; `reproduce` ends so); a voxel count (width x
///    height x depth, an `int` product) other than `in_array_size` gives `ERROR: Reading <sname>
///    as volume encoded scalar file, but size doesn't match expected size of <n>`
///    (`ERROR_BADFILE`); then the whole volume (`MRIread`; `ERROR_BADFILE` when it gives none) and
///    that frame's voxels in slice, row, column order (FS-623: a voxel count that overflowed the
///    `int` writes past the values in FreeSurfer, a crash; larmorx stops at the last value);
/// 2. an old or new curvature file ([`curvio::read_curvature_into_array`] of `sname`, its
///    messages): `reading values from curvature-format file...` on standard output when it read;
/// 3. a w file: `sname`, with `.w` added unless the character after its last dot is `w`, opened
///    (`MRISreadValuesIntoArray: File <file> not found`, `ERROR_NOFILE`, when it cannot be): a
///    2-byte latency (ignored), the 3-byte count (`MRISreadValues(<file>): could not read # of
///    vertices`, `ERROR_BADFILE`, when missing), then per entry a 3-byte vertex number
///    (`MRISreadValues(<file>): could not read <i>th vno`, `ERROR_BADFILE`, when missing) and its
///    `float`; the values not named 0. A vertex number outside `[0, in_array_size)` prints
///    `MRISreadValues: vertex index out of range: <k> f=<value as %f>` (FS-621, rule A:
///    FreeSurfer prints the previous entry's value and does not read this entry's, so the rest of
///    the file is read three bytes out of step; larmorx reads it and prints it, `reproduce`
///    FreeSurfer's).
///
/// `sname` `None` gives `Parameter was NULL.`, a negative `in_array_size` `in_array_size was
/// negative.` (both `ERROR_BADPARM`). Returns the status and, with `NO_ERROR`, the values. The
/// volume branch copies `sname` with `strncpy` into 1,023 bytes (FS-622: a name of 1,023 bytes or
/// more has no end in FreeSurfer's copy, which is then read past its buffer; larmorx takes the
/// whole name in both modes), the w file's name with `strcpy` and `strcat` into 1,024 (FS-126: a
/// name that does not fit ends FreeSurfer with its buffer-overflow abort, `reproduce` so).
pub fn read_values_into_array(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    sname: Option<&str>,
    in_array_size: i32,
) -> Result<(i32, Option<Vec<f32>>), Fatal> {
    const F: &str = "MRISreadValuesIntoArray";
    let Some(sname) = sname else {
        log::error_printf(g, io.err, ERROR_BADPARM, "Parameter was NULL.");
        return Ok((ERROR_BADPARM, None));
    };
    if in_array_size < 0 {
        log::error_printf(g, io.err, ERROR_BADPARM, "in_array_size was negative.");
        return Ok((ERROR_BADPARM, None));
    }
    let sname = c_str(sname);
    if sname.len() >= STRLEN - 1 {
        g.notices.add(
            &FS_622,
            format!("{F}: a name of {} bytes, 1,023 or more, is taken whole (FreeSurfer's copy of it has no end)", sname.len()),
        );
    }
    let fname = sname;
    let ty = identify::mri_identify(g, dir, io.env, io.out, io.err, fname)?;
    if ty != MRI_VOLUME_TYPE_UNKNOWN {
        let frame = sfile::get_read_frame(g, sg, io);
        if verbose_show(g) {
            let _ = writeln!(io.out, "MRISreadValues: frame={frame}");
        }
        let Some(head) = mriio::mri_read_header(g, io.out, io.err, io.env, dir, fname, ty)? else {
            return Ok((ERROR_BADFILE, None));
        };
        if head.nframes <= frame {
            g.notices.add(
                &FS_620,
                format!("{F}: frame {frame} of {fname}, which has {} frames: the message is printed (FreeSurfer reads the freed header and crashes)", head.nframes),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            let m = format!(
                "ERROR: attempted to read frame {frame} from {fname}\nbut this file only has {} frames.\n",
                head.nframes
            );
            log::error_printf(g, io.err, ERROR_BADFILE, &m);
            return Ok((ERROR_BADFILE, None));
        }
        let nv = head
            .width
            .wrapping_mul(head.height)
            .wrapping_mul(head.depth);
        drop(head);
        if nv != in_array_size {
            let m = format!(
                "ERROR: Reading {sname} as volume encoded scalar file, but size doesn't match expected size of {in_array_size}\n"
            );
            log::error_printf(g, io.err, ERROR_BADFILE, &m);
            return Ok((ERROR_BADFILE, None));
        }
        let Some(temp) = mriio::mri_read_volume(g, io.out, io.err, io.env, dir, fname)? else {
            return Ok((ERROR_BADFILE, None));
        };
        let mut array = vec![0.0f32; nonneg(in_array_size)];
        let mut vno = 0usize;
        'all: for s in 0..temp.depth {
            for r in 0..temp.height {
                for c in 0..temp.width {
                    let val = mri_get_vox_val(g, io.err, &temp, c, r, s, frame, 0)?;
                    let Some(slot) = array.get_mut(vno) else {
                        g.notices.add(
                            &FS_623,
                            format!("{F}: {fname} holds more voxels than its {in_array_size} values (its voxel count overflows an int): the rest is not read (FreeSurfer writes past its values)"),
                        );
                        break 'all;
                    };
                    *slot = val;
                    vno += 1;
                }
            }
        }
        return Ok((NO_ERROR, Some(array)));
    }
    let (code, array) = curvio::read_curvature_into_array(g, io, dir, sname, in_array_size)?;
    if code == NO_ERROR {
        let _ = writeln!(io.out, "reading values from curvature-format file...");
        return Ok((NO_ERROR, array));
    }
    let mut fname = strcpy_checked(g, io, sname, F)?;
    let w = match fname.rfind('.') {
        Some(i) => fname.as_bytes().get(i + 1) == Some(&b'w'),
        None => false,
    };
    if !w {
        if fname.len() + 2 >= STRLEN {
            g.notices.add(
                &FS_126,
                format!("{F}: a w-file name of {} bytes with .w, 1,024 or more, is taken whole (FreeSurfer aborts)", fname.len() + 2),
            );
            if g.notices.reproduce() {
                let _ = io
                    .err
                    .write_all(b"*** buffer overflow detected ***: terminated\n");
                return Err(Fatal::Exit(134));
            }
        }
        fname.push_str(".w");
    }
    let mut fp = match open(dir, &fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: File {fname} not found\n");
            return Ok((open_failed(g, io, e, ERROR_NOFILE, &m), None));
        }
    };
    // the latency (`fread2`), never used: its value is not looked at, even when the file is short
    let _ilat = fp.fread(&mut [0u8; 2], 2, 1);
    let (n, num) = fread3(&mut fp);
    curvio::sync_read_errno(g, &fp);
    if n < 1 {
        fp.close();
        let m = format!("MRISreadValues({fname}): could not read # of vertices");
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok((ERROR_BADFILE, None));
    }
    let mut array = vec![0.0f32; nonneg(in_array_size)];
    let mut f = 0.0f32;
    let mut i = 0;
    while i < num {
        let (n, k) = fread3(&mut fp);
        curvio::sync_read_errno(g, &fp);
        if n < 1 {
            // FreeSurfer leaves the file open and the values unreturned (nothing else differs)
            fp.close();
            let m = format!("MRISreadValues({fname}): could not read {i}th vno");
            log::error_printf(g, io.err, ERROR_BADFILE, &m);
            return Ok((ERROR_BADFILE, None));
        }
        if k >= in_array_size || k < 0 {
            g.notices.add(
                &FS_621,
                format!("{F}: {fname}: vertex number {k} outside the {in_array_size} values: its value is read (FreeSurfer skips it unread and reads the rest out of step)"),
            );
            if !g.notices.reproduce() {
                f = fread_float(g, io.err, &mut fp);
                curvio::sync_read_errno(g, &fp);
            }
            let _ = writeln!(
                io.out,
                "MRISreadValues: vertex index out of range: {k} f={}",
                fmt_f(f64::from(f), 0, 6)
            );
            i += 1;
            continue;
        }
        f = fread_float(g, io.err, &mut fp);
        curvio::sync_read_errno(g, &fp);
        array[k as usize] = f;
        i += 1;
    }
    fp.close();
    Ok((NO_ERROR, Some(array)))
}

// ---- the values written: MRISwriteValues, MRISwriteAreaErrorToValFile (relay 37) --------------

/// `MRISwriteValues(mris, sname)`: a name of a volume format (fs.io's `mri_identify` knows it):
/// the vertices' values as a float volume of one voxel per vertex (`MRIcopyMRIS(NULL, mris, 0,
/// "val")`), written by `MRIwrite` (with `DIAG_SHOW` and verbose: `Saving surf vals in 'volume'
/// format to <name>` first), its status returned. Otherwise a w file: the name, `.w` added unless
/// its last dot is followed by `w` (FS-126 for a name of 1,024 bytes or more, with `.w`); with
/// `DIAG_SHOW`: `writing surface values to <file>.`; a file that cannot be created ends the program
/// (`Can't create file <file>` with its newline, `ERROR_NOFILE`); with `DIAG_SHOW` and verbose `num =
/// <n>`; then 0 (2 bytes), the count of vertices whose value is not 0 (NaN counts), and for each of
/// them its number (3 bytes) and value (a big-endian float; a value that is not finite is the
/// error `MRISwriteValues(<file>): val at vertex <k> is not finite`, written anyway). Then on
/// standard output `avg = %2.3f, stdev = %2.3f, min = %2.3f, max = %2.3f` (the values' mean and
/// standard deviation in `double`, from the sums of the values and of their `float` squares, 0 when
/// the variance is not above 0; min and max from 1000 and -1000), or `Warning: all vertex values are
/// zero`. `NO_ERROR`.
pub fn write_values(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISwriteValues";
    let sname = c_str(sname);
    let ftype = identify::mri_identify(g, dir, io.env, io.out, io.err, sname)?;
    if ftype != MRI_VOLUME_TYPE_UNKNOWN {
        let mut temp = match surfmri::copy_mris(g, io, None, mris, 0, "val")? {
            Ret::New(m) => m,
            _ => {
                let _ = writeln!(io.out, "ERROR: MRISwriteValues: could not alloc MRI");
                return Ok(1);
            }
        };
        if verbose_show(g) {
            let _ = writeln!(io.out, "Saving surf vals in 'volume' format to {sname}");
        }
        return mriio::mri_write(
            g,
            io.out,
            io.err,
            io.env,
            dir,
            &mut temp,
            sname,
            None,
            MGZ_INTENT_UNKNOWN,
        );
    }
    let mut fname = strcpy_checked(g, io, sname, F)?;
    let w = match fname.rfind('.') {
        Some(i) => fname.as_bytes().get(i + 1) == Some(&b'w'),
        None => false,
    };
    if !w {
        if fname.len() + 2 >= STRLEN {
            g.notices.add(
                &FS_126,
                format!("{F}: a w-file name of {} bytes with .w, 1,024 or more, is taken whole (FreeSurfer aborts)", fname.len() + 2),
            );
            if g.notices.reproduce() {
                let _ = io
                    .err
                    .write_all(b"*** buffer overflow detected ***: terminated\n");
                return Err(Fatal::Exit(134));
            }
        }
        fname.push_str(".w");
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(io.out, "writing surface values to {fname}.");
    }
    let mut fp = match open(dir, &fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_exit(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("Can't create file {fname}\n"),
            )?;
            unreachable!("ErrorExit returns only through its Err");
        }
    };
    let n = nonneg(mris.nvertices);
    let num = (0..n).filter(|&k| mris.vertex(k).val != 0.0).count() as i32;
    if verbose_show(g) {
        let _ = writeln!(io.out, "num = {num}");
    }
    fwrite2(&mut fp, 0);
    fwrite3(&mut fp, num);
    let (mut sum, mut sum2, mut max, mut min) = (0.0f64, 0.0f64, -1000.0f64, 1000.0f64);
    for k in 0..n {
        let f = mris.vertex(k).val;
        if f == 0.0 {
            continue;
        }
        fwrite3(&mut fp, k as i32);
        if !f.is_finite() {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("MRISwriteValues({fname}): val at vertex {k} is not finite"),
            );
        }
        fwrite_float(&mut fp, f);
        sum = add64(sum, f64::from(f));
        // the official build: the square first (`addsd` of the running sum into it)
        sum2 = add64(f64::from(mul(f, f)), sum2);
        if f64::from(f) > max {
            max = f64::from(f);
        }
        if f64::from(f) < min {
            min = f64::from(f);
        }
    }
    fp.close();
    if num > 0 {
        let nf = f64::from(num);
        sum = div64(sum, nf);
        let var = sub64(div64(sum2, nf), mul64(sum, sum));
        let sd = if var > 0.0 { sqrt64(var) } else { 0.0 };
        let _ = writeln!(
            io.out,
            "avg = {}, stdev = {}, min = {}, max = {}",
            fmt_f(sum, 2, 3),
            fmt_f(sd, 2, 3),
            fmt_f(min, 2, 3),
            fmt_f(max, 2, 3)
        );
    } else {
        let _ = writeln!(io.out, "Warning: all vertex values are zero");
    }
    Ok(NO_ERROR)
}

/// `MRISwriteAreaErrorToValFile(mris, name)`: each unripped vertex's value set to the difference of
/// its faces' summed area and summed original area (`float`, in order) over its face count (0 faces:
/// NaN), then [`write_values`] (its status ignored). `NO_ERROR`.
///
/// **FS-633:** FreeSurfer sums the original areas of the faces numbered 0 to `num - 1` of the
/// surface (the face-normal cache's entries by the loop's count), not of the vertex's faces; by
/// default the vertex's own faces (rule A), FreeSurfer's with `reproduce`; noted where they differ.
/// **FS-634:** a vertex listing more faces than the surface has (a face listed twice) makes
/// FreeSurfer read the cache past the faces (a crash or another array's data); its own face's is
/// taken there in both modes, noted.
pub fn write_area_error_to_val_file(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    let rep = g.notices.reproduce();
    let mut moved = 0usize;
    for vno in 0..nonneg(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let faces: Vec<i32> = {
            let vt = mris.topology(vno);
            let f = vt.f.as_deref().unwrap_or(&[]);
            f[..usize::from(vt.num).min(f.len())].to_vec()
        };
        let (mut area, mut orig_area) = (0.0f32, 0.0f32);
        let mut theirs = 0.0f32;
        let nfaces = nonneg(mris.nfaces);
        for (k, &fno) in faces.iter().enumerate() {
            area = add(area, mris.face(fno as usize).area);
            let own = facenorm::get_face_norm(mris, fno as usize).orig_area;
            let fs = if k < nfaces {
                facenorm::get_face_norm(mris, k).orig_area
            } else {
                // FS-634: FreeSurfer reads the cache past the surface's faces
                g.notices.add(
                    &FS_634,
                    format!("MRISwriteAreaErrorToValFile: vertex {vno} lists {} faces of a surface of {nfaces}: FreeSurfer's original area of face {k} is read outside the faces (the vertex's own face's taken)", faces.len()),
                );
                own
            };
            orig_area = add(orig_area, if rep { fs } else { own });
            theirs = add(theirs, fs);
        }
        let n = f32::from(u8::try_from(faces.len()).unwrap_or(u8::MAX));
        let val = div(sub(area, orig_area), n);
        if !rep {
            let fs = div(sub(area, theirs), n);
            moved += usize::from(fs.to_bits() != val.to_bits());
        }
        mris.vertex_mut(vno).val = val;
    }
    if moved > 0 {
        g.notices.add(
            &FS_633,
            format!("MRISwriteAreaErrorToValFile: {moved} vertices' area errors from their own faces' original areas (FreeSurfer: the faces numbered from 0)"),
        );
    }
    write_values(g, io, dir, mris, name)?;
    Ok(NO_ERROR)
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s curvature files by name (`fs.surfio`, queue items 3.1-47 and 3.1-48,
//! relay 35 of track D): `MRISwriteCurvature` 281 (the vertices' curvature in the format the
//! name chooses: an MGH or NIfTI volume of one voxel per vertex, VTK, text, GIFTI, or the binary
//! curvature of the new format) and `MRISreadCurvatureFile` 4849 with `__mrisreadcurvmri` 4874
//! (the curvature from any of those, or from the old binary format).
//!
//! The names and formats come from [`crate::names`] (`MRISwriteCurvature_getfilename`,
//! `MRISreadCurvatureFile_getfilename`); the formats' own readers and writers are fs.surfio's
//! (`curvio`, `sfile`, `formats`, `gifti`, `gifti_write`) and fs.io's (`MRIwrite`,
//! `MRISreadCurvAsMRI`, `mri_read`).
//!
//! `MRIread` (fs.io, `utils/mriio.cpp` 1002-1027: the name's wildcard, `mri_read` of every
//! frame, `MRIremoveNaNs`) is not in fs.io yet (track A's next item): [`mri_read_whole`] is a
//! private translation of it for the volume branch here, to be replaced by fs.io's when it lands.
//!
//! Not translated: `MRISreadBinaryCurvature` 4759 and `MRISreadNewCurvatureFile` 5781, which no
//! FreeSurfer program or library function calls.

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::path::Path;

use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::utils::file_name_from_wildcard;
use larmorx_freesurfer_io::fio::{fwrite_float, fwrite_int, fwrite3};
use larmorx_freesurfer_io::gifti_common::NIFTI_INTENT_SHAPE;
use larmorx_freesurfer_io::identify::{
    ASCII_FILE, GIFTI_FILE, MRI_CURV_FILE, MRI_MGH_FILE, MRI_VOLUME_TYPE_UNKNOWN, NII_FILE,
    VTK_FILE,
};
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{
    MGH_VERSION, MGZ_INTENT_SHAPE, MGZ_INTENT_UNKNOWN, MRI_FLOAT, Mri, mri_alloc,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::curvio;
use crate::formats;
use crate::gifti::{self, GiftiRead, OverlayInfo};
use crate::gifti_write;
use crate::mris::Mris;
use crate::names;
use crate::sfile::{self, nonneg, open, open_failed};

// ---- writing --------------------------------------------------------------------------------

/// `MRISwriteCurvature(mris, sname, curv_name)`: the vertices' curvature into the file
/// [`names::write_curvature_getfilename`] chooses for `sname` (with `DIAG_SHOW`: `writing
/// curvature file <file> type <type>` on standard output first), in its format:
///
/// - MGH (`.mgh`, `.mgz`) or NIfTI (`.nii`, `.nii.gz`): a float volume of `<vertices>` x 1 x 1, the
///   curvature of vertex `v` in voxel `v`, its version the shape intent's (`(2 << 8) | 1`),
///   written by `MRIwrite`; its status returned;
/// - VTK: `MRISwriteVTK` then `MRISwriteCurvVTK` on the same name; `NO_ERROR` whatever they gave;
/// - text (`.asc`): `mrisWriteAsciiCurvatureFile`'s status;
/// - GIFTI (`.gii`, or `FS_GII`'s name): `MRISwriteGIFTI(mris, NIFTI_INTENT_SHAPE, file,
///   curv_name)`'s status;
/// - any other name: the new binary curvature format: the three bytes `ff ff ff`, the vertex
///   count, the face count and 1 as big-endian `int`s, then each vertex's curvature as a big-endian
///   `float` (its bits as they are); `NO_ERROR`, or `ERROR_NOFILE` after `MRISwriteCurvature: could
///   not open <file>` (with the text of `errno` first) when the file cannot be created.
pub fn write_curvature(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
    curv_name: Option<&str>,
) -> Result<i32, Fatal> {
    let (mritype, curv_to_write) = names::write_curvature_getfilename(g, io, dir, mris, sname)?;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            io.out,
            "writing curvature file {curv_to_write} type {mritype}"
        );
    }
    let mut error = NO_ERROR;
    if mritype == MRI_MGH_FILE || mritype == NII_FILE {
        // MRIalloc never returns NULL (a C++ constructor): its `ERROR_NOMEMORY` branch is dead
        let mut temp = mri_alloc(g, io.out, io.err, mris.nvertices, 1, 1, MRI_FLOAT)?;
        for vno in 0..nonneg(mris.nvertices) {
            let curv = mris.vertex(vno).curv;
            mri_set_vox_val(g, io.err, &mut temp, vno as i32, 0, 0, 0, curv, 0)?;
        }
        temp.version = ((MGZ_INTENT_SHAPE & 0xffff) << 8) | MGH_VERSION;
        error = mriio::mri_write(
            g,
            io.out,
            io.err,
            io.env,
            dir,
            &mut temp,
            &curv_to_write,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    } else if mritype == VTK_FILE {
        sfile::write_vtk(g, io, dir, mris, &curv_to_write);
        sfile::write_curv_vtk(g, io, dir, mris, &curv_to_write);
    } else if mritype == ASCII_FILE {
        error = curvio::write_ascii_curvature_file(g, io, dir, mris, &curv_to_write)?;
    } else if mritype == GIFTI_FILE {
        error = gifti_write::mris_write_gifti(
            g,
            io,
            dir,
            mris,
            NIFTI_INTENT_SHAPE,
            &curv_to_write,
            curv_name,
        )?;
    } else {
        let name = sfile::c_str(&curv_to_write);
        let mut fp = match open(dir, name, "wb") {
            Ok(f) => f,
            Err(e) => {
                let m = format!("MRISwriteCurvature: could not open {name}");
                return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
            }
        };
        // the new format's mark: -1 in three bytes
        fwrite3(&mut fp, -1);
        fwrite_int(&mut fp, mris.nvertices);
        fwrite_int(&mut fp, mris.nfaces);
        fwrite_int(&mut fp, 1);
        for k in 0..nonneg(mris.nvertices) {
            fwrite_float(&mut fp, mris.vertex(k).curv);
        }
        fp.close();
    }
    Ok(error)
}

// ---- reading --------------------------------------------------------------------------------

/// `MRISreadCurvatureFile(mris, sname, curvmri, poverlayinfo)`: the vertices' curvature from the
/// file [`names::read_curvature_file_getfilename`] chooses for `sname`, by its format:
///
/// - VTK: `MRISreadVTK(mris, file)` into the surface (its coordinates kept, the fields' values in
///   `curv` and `val`; `curvmri` not given to it);
/// - MGH, NIfTI or the new binary curvature format: [`read_curv_mri`];
/// - text: `mrisReadAsciiCurvatureFile(mris, file, curvmri)`;
/// - GIFTI: `mrisReadGIFTIfile(file, mris, poverlayinfo)` into the surface (the overlays' list
///   filled when given);
/// - anything else: the old binary format (`__mrisreadcurvoldformat(mris, file, curvmri)`).
///
/// Returns the reader's status; `ERROR_BADFILE` when the VTK or GIFTI reader gave no surface (its
/// messages printed), `NO_ERROR` when it gave one.
#[allow(clippy::too_many_arguments)]
pub fn read_curvature_file(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
    curvmri: Option<&mut Mri>,
    overlays: Option<&mut Vec<OverlayInfo>>,
) -> Result<i32, Fatal> {
    let (mritype, curv_to_read) = names::read_curvature_file_getfilename(g, io, dir, mris, sname)?;
    let read_volume = true;
    let mut error = NO_ERROR;
    let mut null = false;
    if mritype == VTK_FILE {
        null = formats::read_vtk(g, sg, io, dir, Some(mris), &curv_to_read, None)?.is_null();
    } else if mritype == MRI_MGH_FILE || mritype == NII_FILE || mritype == MRI_CURV_FILE {
        error = read_curv_mri(
            g,
            sg,
            io,
            dir,
            mris,
            &curv_to_read,
            mritype,
            read_volume,
            curvmri,
        )?;
    } else if mritype == ASCII_FILE {
        error = curvio::read_ascii_curvature_file(g, io, dir, mris, &curv_to_read, curvmri)?;
    } else if mritype == GIFTI_FILE {
        // a new surface (a GIFTI surface file) is FreeSurfer's local pointer only: the caller's
        // surface stays as the reader left it
        let r = gifti::mris_read_gifti_file(
            g,
            sg,
            io,
            dir,
            &curv_to_read,
            Some(mris),
            overlays,
            -1,
            None,
        )?;
        null = matches!(r, GiftiRead::Null);
    } else {
        error = curvio::read_curv_old_format(g, io, dir, mris, &curv_to_read, curvmri)?;
    }
    if null {
        error = ERROR_BADFILE;
    }
    Ok(error)
}

/// `__mrisreadcurvmri(mris, fname, type, read_volume, curvmri)`: the curvature from a volume of
/// one voxel per vertex: an MGH or NIfTI file read by `MRIread` ([`mri_read_whole`]), a curvature
/// file of the new format by `MRISreadCurvAsMRI`. Its frame is `MRISgetReadFrame()`'s. Returns
/// `ERROR_BADFILE` when nothing was read (the reader's messages), or after `ERROR: attempted to
/// read frame <f> from <file>` and `  but this file only has <n> frames.` (standard output) when
/// the frame is not below the volume's frame count; 1 after `ERROR: number of vertices in <file>
/// does not match surface (<n>,<nvertices>)` when its voxel count (width x height x depth, C's
/// `int` product) is not the surface's vertex count. Else the voxels of that frame in slice, row,
/// column order go to the vertices' curvature in turn (and into `curvmri`'s voxel at the same
/// column, row and slice of frame 0, given), the surface's `max_curv` and `min_curv` from them
/// (the first voxel starts both; 10,000 and -10,000 when there is none); `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn read_curv_mri(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    mtype: i32,
    read_volume: bool,
    mut curvmri: Option<&mut Mri>,
) -> Result<i32, Fatal> {
    let frame = sfile::get_read_frame(g, sg, io);
    let temp = if mtype == MRI_MGH_FILE || mtype == NII_FILE {
        mri_read_whole(g, sg, io, dir, fname)?
    } else if mtype == MRI_CURV_FILE {
        mriio::mris_read_curv_as_mri(g, io.out, io.err, dir, fname, read_volume)?
    } else {
        None
    };
    let Some(temp) = temp else {
        return Ok(ERROR_BADFILE);
    };
    if temp.nframes <= frame {
        let _ = writeln!(io.out, "ERROR: attempted to read frame {frame} from {fname}");
        let _ = writeln!(
            io.out,
            "  but this file only has {} frames.",
            temp.nframes
        );
        return Ok(ERROR_BADFILE);
    }
    let (w, h, d) = (temp.vg.width, temp.vg.height, temp.vg.depth);
    let nv = w.wrapping_mul(h).wrapping_mul(d);
    if nv != mris.nvertices {
        let _ = writeln!(
            io.out,
            "ERROR: number of vertices in {fname} does not match surface ({nv},{})",
            mris.nvertices
        );
        return Ok(1);
    }
    let mut vno = 0usize;
    let (mut curvmin, mut curvmax) = (10000.0f32, -10000.0f32);
    // `for (f = 0; f < 1; f++)`: the values of `frame`, the volume given written at frame 0
    let f = 0;
    for s in 0..d {
        for r in 0..h {
            for c in 0..w {
                let curv = mri_get_vox_val(g, io.err, &temp, c, r, s, frame, 0)?;
                if s == 0 && r == 0 && c == 0 {
                    curvmin = curv;
                    curvmax = curv;
                }
                if curv > curvmax {
                    curvmax = curv;
                }
                if curv < curvmin {
                    curvmin = curv;
                }
                mris.vertex_mut(vno).curv = curv;
                if let Some(m) = curvmri.as_deref_mut() {
                    mri_set_vox_val(g, io.err, m, c, r, s, f, curv, 0)?;
                }
                vno += 1;
            }
        }
    }
    mris.max_curv = curvmax;
    mris.min_curv = curvmin;
    Ok(NO_ERROR)
}

/// `MRIread(fname)` (`utils/mriio.cpp` 1002-1027) for [`read_curv_mri`], until fs.io has it:
/// the name's first wildcard match (`FileNameFromWildcard`: a name with a `*` becomes the first
/// path `glob` finds, when it finds one), every frame of it read with its voxels (`mri_read(name,
/// MRI_VOLUME_TYPE_UNKNOWN, TRUE, -1, -1)`), then `MRIremoveNaNs(mri, mri)`: each voxel (columns
/// outermost, then rows, slices and frames) that is not finite is counted and set to 0 unless
/// `FS_LEAVE_NANS` is set; the first in the process prints `NaN found at voxel (<c>, <r>, <s>,
/// <f>)` (standard output; FreeSurfer's `static int first`, here
/// [`SurfGlobals::nan_found_shown`]); a count above 0 gives `WARNING: <n> NaNs found in volume
/// <name>...` (`ErrorPrintf(ERROR_BADPARM)`, the volume's name as read). FreeSurfer's loop over
/// the columns runs in parallel only past the reproducible level (`if_ROMP(shown_reproducible)`):
/// one thread, in order.
fn mri_read_whole(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mri>, Fatal> {
    larmorx_freesurfer_core::chklc::chklc();
    let name = file_name_from_wildcard(dir, fname);
    let Some(mut m) = mriio::mri_read(
        g,
        io.out,
        io.err,
        io.env,
        dir,
        &name,
        MRI_VOLUME_TYPE_UNKNOWN,
        true,
        -1,
        -1,
    )?
    else {
        return Ok(None);
    };
    let leave = io.env.get("FS_LEAVE_NANS").is_some();
    let mut nans = 0i32;
    for x in 0..m.vg.width {
        for y in 0..m.vg.height {
            for z in 0..m.vg.depth {
                for f in 0..m.nframes {
                    let val = mri_get_vox_val(g, io.err, &m, x, y, z, f, 0)?;
                    if !val.is_finite() {
                        nans = nans.wrapping_add(1);
                        if !leave {
                            mri_set_vox_val(g, io.err, &mut m, x, y, z, f, 0.0, 0)?;
                        }
                        if !sg.nan_found_shown {
                            let _ = writeln!(io.out, "NaN found at voxel ({x}, {y}, {z}, {f})");
                            sg.nan_found_shown = true;
                        }
                    }
                }
            }
        }
    }
    if nans > 0 {
        let vname = m.vg.fname.to_string_lossy();
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!("WARNING: {nans} NaNs found in volume {vname}...\n"),
        );
    }
    Ok(Some(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_freesurfer_core::run::Environment;

    fn surface(g: &mut Globals, sg: &SurfGlobals, nv: i32) -> Mris {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut m = crate::alloc::alloc(g, &mut out, &mut err, sg, nv, 0).expect("a surface");
        m.fname = "d/lh.white".to_owned();
        for k in 0..nv as usize {
            m.vertex_mut(k).curv = k as f32 * 0.5 - 1.0;
        }
        m
    }

    /// The new binary format: written, then read back by name (the hemisphere put before it).
    #[test]
    fn new_format_round_trip() {
        let td = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(td.path().join("d")).expect("d");
        let d = Some(td.path());
        let env = Environment::from_pairs(Vec::<(String, String)>::new());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut sg = SurfGlobals::default();
        let mut m = surface(&mut g, &sg, 4);
        let r = write_curvature(&mut g, &mut io, d, &mut m, "curv", None).expect("written");
        assert_eq!(r, NO_ERROR);
        let b = std::fs::read(td.path().join("d/lh.curv")).expect("the file");
        assert_eq!(&b[..3], &[0xff, 0xff, 0xff]);
        assert_eq!(b.len(), 3 + 12 + 16);
        let mut m2 = surface(&mut g, &sg, 4);
        for k in 0..4 {
            m2.vertex_mut(k).curv = 9.0;
        }
        let r = read_curvature_file(&mut g, &mut sg, &mut io, d, &mut m2, "curv", None, None)
            .expect("read");
        assert_eq!(r, NO_ERROR);
        for k in 0..4 {
            assert_eq!(m2.vertex(k).curv.to_bits(), m.vertex(k).curv.to_bits());
        }
        assert_eq!(m2.min_curv, -1.0);
        assert_eq!(m2.max_curv, 0.5);
    }
}

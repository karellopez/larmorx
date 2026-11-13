// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Reading volumes (`utils/mriio.cpp`, items 3.1-28 to 3.1-30, taken early for track D's surfaces:
//! the orchestrator, 2026-11-13): `MRIreadHeader`, `MRIreadInfo`, `MRIreadType`, `mri_read` (the
//! name's `@format` and `#frames`, the format's reader, the cached voxel-to-RAS matrices, the frame
//! selection, `nan_inf_check`), `mghRead` (MGH and MGZ, the header alone or the voxels),
//! `MRITAGread` (the tags after an MGH volume's data: the talairach transform read with
//! FreeSurfer's volume_io, [`crate::minc`]) and `setDirectionCosine`.
//!
//! The readers of the other formats come with their queue items: until then [`mri_read`] reads
//! MGH/MGZ, `.npy` ([`crate::fscnpy`]) and Bruker studies ([`crate::bruker`]) and refuses the others
//! with a message that names the format (`larmorx-freesurfer: reading <format> volumes is not
//! ported yet`, standard error, `None`).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 6):
//! - **FS-249** (memory): a name of 1,024 bytes or more once the working directory is put before
//!   it overflows `mri_read`'s and `MRIreadHeader`'s buffers: both modes use the whole name, noted.
//! - **FS-253** (crash): an MGH volume holding the NIfTI extension's tag 9 (`TAG_RAS_XFORM_EXTRA`)
//!   crashes `mghRead`: both modes skip the tag's data, noted.
//! - **FS-254** (memory): a talairach tag of 1,024 bytes or more overflows the volume's transform
//!   name; a volume whose folder name is 975 bytes or more overflows the name of
//!   `<folder>/transforms/talairach.xfm`: both modes use the whole names, noted.
//! - **FS-255** (crash): `mri_read` without voxels (`MRIreadHeader`, `MRIreadInfo`) of a float
//!   volume with a frame list (`name#0`) checks voxels it does not have: both modes skip the check,
//!   noted.
//! - **FS-256** (memory, crash): with `FS_MGZIO_USEVOXELBUFREAD` set, `mghRead` of one frame after
//!   the first writes past the one-frame volume it made, and a volume without slices crashes: both
//!   modes read the frame into the volume (nothing for no slices), noted.
//! - **FS-257** (crash or memory): a command-line or phase-encoding tag whose length is negative
//!   or more than the machine can allocate: both modes keep the bytes the file has (none for a
//!   negative length), noted.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    FS_76, FS_249, FS_253, FS_254, FS_255, FS_256, FS_257, FS_421, FS_432,
};
use larmorx_freesurfer_core::diag::{self, DIAG_INFO, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BAD_FILE, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal,
    NO_ERROR,
};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{sse, system, utils};
use larmorx_freesurfer_mri::mri::{
    self, MAX_CMDS, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_SHORT, MRI_TENSOR, MRI_UCHAR,
    MRI_USHRT, Mri,
};
use larmorx_freesurfer_mri::vol_geom::{self, VolGeom};
use larmorx_io::znz::{Whence, ZnzFile};

use crate::fio;
use crate::fstagsio::FsTagsIo;
use crate::identify::{self, MRI_MGH_FILE, MRI_VOLUME_TYPE_UNKNOWN};
use crate::minc;
use crate::streams::Io;
use crate::tags::{
    TAG_AUTO_ALIGN, TAG_CMDLINE, TAG_DOF, TAG_FIELDSTRENGTH, TAG_GCAMORPH_AFFINE,
    TAG_GCAMORPH_GEOM, TAG_GCAMORPH_GEOM_PLUSSHEAR, TAG_GCAMORPH_LABELS, TAG_GCAMORPH_META,
    TAG_MGH_XFORM, TAG_MRI_FRAME, TAG_OLD_COLORTABLE, TAG_OLD_MGH_XFORM, TAG_ORIG_RAS2VOX,
    TAG_PEDIR, TAG_RAS_XFORM, TAG_RAS_XFORM_EXTRA, TAG_SCAN_PARAMETERS,
};

/// `STRLEN` (`const.h`).
const STRLEN: usize = 1024;
/// `MRI_CORONAL`, `MRI_SAGITTAL`, `MRI_HORIZONTAL` (`mri.h`): the slice directions.
pub const MRI_CORONAL: i32 = 0;
pub const MRI_SAGITTAL: i32 = 1;
pub const MRI_HORIZONTAL: i32 = 2;
/// `UNUSED_SPACE_SIZE` (256) and `USED_SPACE_SIZE` (15 floats): an MGH header's reserved bytes.
const UNUSED_SPACE_SIZE: i32 = 256;
const USED_SPACE_SIZE: i32 = 60;
/// `errno`'s `ENOMEM` on Linux (a failed `calloc`).
const ENOMEM: i32 = 12;
/// The bytes of a tag's number and length in FreeSurfer's NIfTI header extension.
const LEN_TAGHEADER: i64 = 12;

fn getenv(env: &Environment, name: &str) -> bool {
    env.get(name).is_some()
}

/// `setDirectionCosine(mri, orientation)`: the direction cosines and centre of a coronal, sagittal
/// or horizontal volume (centre 0); `NO_ERROR`; another value `ERROR_BADPARM` after
/// `setDirectionCosine():unknown slice direction` (an error).
pub fn set_direction_cosine(
    g: &mut Globals,
    err: &mut dyn Write,
    vg: &mut VolGeom,
    orientation: i32,
) -> i32 {
    let (x, y, z): ([f32; 3], [f32; 3], [f32; 3]) = match orientation {
        MRI_CORONAL => ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        MRI_SAGITTAL => ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
        MRI_HORIZONTAL => ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        _ => {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                "setDirectionCosine():unknown slice direction",
            );
            return ERROR_BADPARM;
        }
    };
    (vg.x_r, vg.x_a, vg.x_s) = (x[0], x[1], x[2]);
    (vg.y_r, vg.y_a, vg.y_s) = (y[0], y[1], y[2]);
    (vg.z_r, vg.z_a, vg.z_s) = (z[0], z[1], z[2]);
    (vg.c_r, vg.c_a, vg.c_s) = (0.0, 0.0, 0.0);
    NO_ERROR
}

/// FS-249's check of a name copied into a `STRLEN` buffer.
fn fits(g: &mut Globals, who: &str, name: &str) {
    if name.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{who}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                name.len()
            ),
        );
    }
}

/// The name with the working directory before it when it holds no `/` (`getcwd`; the name alone
/// when there is none).
fn with_cwd(g: &mut Globals, dir: Option<&Path>, who: &str, fname: &str) -> String {
    let full = if fname.contains('/') {
        fname.to_owned()
    } else {
        match system::cwd(dir) {
            Some(c) => format!("{c}/{fname}"),
            None => fname.to_owned(),
        }
    };
    fits(g, who, &full);
    full
}

/// C's `strtol(s, &ep, 10)` on a 64-bit `long`: the value and whether `ep` reached the end.
fn strtol_whole(s: &[u8]) -> (i64, bool) {
    let mut i = 0;
    while i < s.len() && larmorx_freesurfer_core::cstd::is_space(s[i]) {
        i += 1;
    }
    if matches!(s.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return (0, s.is_empty());
    }
    (
        larmorx_freesurfer_core::cstd::strtol10(s),
        i + digits == s.len(),
    )
}

/// `ErrorReturn(NULL, (code, message))` with `errno` cleared first.
fn fail0(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Option<Mri> {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    None
}

/// `nan_inf_check(mri)`: `NO_ERROR` unless a float volume holds an infinite or NaN voxel (the
/// first in the order x, y, z, frame): then `ERROR_BADPARM` after `nan_inf_check(): Inf at voxel
/// <x>, <y>, <z>, <f>` or `... NaN at voxel ...` (an error).
pub fn nan_inf_check(g: &mut Globals, err: &mut dyn Write, m: &Mri) -> i32 {
    if m.mtype != MRI_FLOAT {
        return NO_ERROR;
    }
    let Some(chunk) = m.chunk.as_ref() else {
        return NO_ERROR;
    };
    let (w, h, d, nf) = (m.width, m.height, m.depth, m.nframes);
    let row = m.vox_per_row as usize;
    for i in 0..w {
        for j in 0..h {
            for k in 0..d {
                for t in 0..nf {
                    let o = (((t * d + k) * h + j) as usize * row + i as usize) * 4;
                    let v = f32::from_le_bytes(chunk[o..o + 4].try_into().unwrap());
                    if !v.is_finite() {
                        let what = if v.is_infinite() { "Inf" } else { "NaN" };
                        g.errno = 0;
                        log::error_printf(
                            g,
                            err,
                            ERROR_BADPARM,
                            &format!("nan_inf_check(): {what} at voxel {i}, {j}, {k}, {t}"),
                        );
                        return ERROR_BADPARM;
                    }
                }
            }
        }
    }
    NO_ERROR
}

/// `MRIreadHeader(fname, type)`: the volume's header without its voxels (`mri_read` with
/// `volume_flag` false). A name without `/` gets the working directory before it; an unknown type
/// is found by `mri_identify`, and a name it cannot type prints `ERROR: could not find volume
/// <name>.  Does it exist?` (it does not exist) or `ERROR: could not determine type of <name>`
/// (standard output) and gives `None`.
#[allow(clippy::too_many_arguments)]
pub fn mri_read_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    mtype: i32,
) -> Result<Option<Mri>, Fatal> {
    let modname = with_cwd(g, dir, "MRIreadHeader", fname);
    let mut usetype = mtype;
    if usetype == MRI_VOLUME_TYPE_UNKNOWN {
        usetype = identify::mri_identify(g, dir, env, out, err, &modname)?;
        if usetype == MRI_VOLUME_TYPE_UNKNOWN {
            match std::fs::metadata(utils::resolve(dir, fname)) {
                Err(e) => {
                    g.errno = e.raw_os_error().unwrap_or(2);
                    let _ = writeln!(out, "ERROR: could not find volume {fname}.  Does it exist?");
                }
                Ok(_) => {
                    let _ = writeln!(out, "ERROR: could not determine type of {fname}");
                }
            }
            return Ok(None);
        }
    }
    mri_read(g, out, err, env, dir, &modname, usetype, false, -1, -1)
}

/// `MRIreadInfo(fname)`: `mri_read(fname, MRI_VOLUME_TYPE_UNKNOWN, FALSE, -1, -1)`.
pub fn mri_read_info(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mri>, Fatal> {
    mri_read(
        g,
        out,
        err,
        env,
        dir,
        fname,
        MRI_VOLUME_TYPE_UNKNOWN,
        false,
        -1,
        -1,
    )
}

/// `MRIreadType(fname, type)`: `mri_read(fname, type, TRUE, -1, -1)` (FreeSurfer's licence check
/// first, which always passes).
pub fn mri_read_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    mtype: i32,
) -> Result<Option<Mri>, Fatal> {
    larmorx_freesurfer_core::chklc::chklc();
    mri_read(g, out, err, env, dir, fname, mtype, true, -1, -1)
}

/// The formats [`mri_read`] does not read yet: their names.
fn not_ported(mtype: i32) -> Option<&'static str> {
    use crate::identify::*;
    Some(match mtype {
        MRI_CORONAL_SLICE_DIRECTORY => "COR",
        SIEMENS_FILE => "Siemens",
        MRI_GCA_FILE => "GCA",
        BHDR => "bhdr",
        BSHORT_FILE => "bshort",
        BFLOAT_FILE => "bfloat",
        GENESIS_FILE => "GE Genesis",
        SIGNA_FILE => "GE Signa",
        GE_LX_FILE => "GE LX",
        MRI_ANALYZE_FILE | MRI_ANALYZE4D_FILE => "ANALYZE",
        BRIK_FILE => "AFNI BRIK",
        MRI_MINC_FILE => "MINC",
        SDT_FILE => "Varian SDT",
        MGH_MORPH => "GCA morph (m3z)",
        MGH_AUTOENCODER => "autoencoder",
        GDF_FILE => "GDF",
        DICOM_FILE | SIEMENS_DICOM_FILE => "DICOM",
        XIMG_FILE => "GE XIMG",
        NIFTI1_FILE | NII_FILE => "NIfTI",
        NRRD_FILE => "NRRD",
        MRI_CURV_FILE => "curvature",
        IMAGE_FILE => "image",
        MGH_ANNOT => "annotation",
        _ => return None,
    })
}

/// `mri_read(fname, type, volume_flag, start_frame, end_frame)`: the volume `fname` names, its
/// voxels when `volume_flag` (else the header alone).
///
/// The name: a null or empty name is refused (`mri_read(): empty fname!`); without `/` the working
/// directory goes before it; what follows its last `@` names the format (`string_to_type`; an
/// unknown one: `mri_read(): unknown type '<t>'`), what follows its last `#` (while
/// `MRIIO_Strip_Pound` is set) the frames: `<n>` or `<first>:<last>` (`mri_read(): bad frame
/// specification ('<n>:')`, `bad start frame`, `bad end frame`, `bad frame`, `frame (<n>) is less
/// than zero`, `the start frame (<a>) is greater than the end frame (<b>)`). Without either, an
/// unknown type is found by `mri_identify` (`mri_read(): couldn't determine type of file <name>`).
/// Then the format's reader; the volume's name is `fname` as given; the cached voxel-to-RAS
/// matrices are recomputed. With frames: `start frame (<a>) is out of range (<n> frames in
/// volume)`, `end frame ...`; without voxels a copy of the header with the frames' count; with
/// voxels the frames copied into a new volume. A float volume with an infinite or NaN voxel is
/// refused ([`nan_inf_check`]) whenever frames are given, and when the whole volume is read.
#[allow(clippy::too_many_arguments)]
pub fn mri_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    mut mtype: i32,
    volume_flag: bool,
    mut start_frame: i32,
    mut end_frame: i32,
) -> Result<Option<Mri>, Fatal> {
    if fname.is_empty() {
        return Ok(fail0(g, err, ERROR_BADPARM, "mri_read(): empty fname!\n"));
    }
    let mut copy = with_cwd(g, dir, "mri_read", fname);
    // the last `@` and the last `#`, both found first; each cut at its own place, so the text
    // after one ends where the other begins
    let at = copy.rfind('@');
    let pound = if g.mriio_strip_pound != 0 {
        copy.rfind('#')
    } else {
        None
    };
    let piece = |from: usize, other: Option<usize>| -> String {
        let end = match other {
            Some(o) if o > from => o,
            _ => copy.len(),
        };
        copy[from + 1..end].to_owned()
    };
    let at_text = at.map(|i| piece(i, pound));
    let pound_text = pound.map(|i| piece(i, at));
    if let Some(cut) = [at, pound].into_iter().flatten().min() {
        copy.truncate(cut);
    }
    if let Some(t) = &at_text {
        mtype = identify::string_to_type(g, t);
        if mtype == MRI_VOLUME_TYPE_UNKNOWN {
            return Ok(fail0(
                g,
                err,
                ERROR_BADPARM,
                &format!("mri_read(): unknown type '{t}'\n"),
            ));
        }
    } else if mtype == MRI_VOLUME_TYPE_UNKNOWN {
        mtype = identify::mri_identify(g, dir, env, out, err, &copy)?;
        if mtype == MRI_VOLUME_TYPE_UNKNOWN {
            return Ok(fail0(
                g,
                err,
                ERROR_BADPARM,
                &format!("mri_read(): couldn't determine type of file {copy}"),
            ));
        }
    }
    if let Some(pound) = &pound_text {
        if let Some(c) = pound.find(':') {
            let (a, b) = (&pound[..c], &pound[c + 1..]);
            if b.is_empty() {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mri_read(): bad frame specification ('{a}:')\n"),
                ));
            }
            let (v, whole) = strtol_whole(a.as_bytes());
            start_frame = v as i32;
            if !whole {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mri_read(): bad start frame ('{a}')\n"),
                ));
            }
            let (v, whole) = strtol_whole(b.as_bytes());
            end_frame = v as i32;
            if !whole {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mri_read(): bad end frame ('{b}')\n"),
                ));
            }
        } else {
            let (v, whole) = strtol_whole(pound.as_bytes());
            start_frame = v as i32;
            end_frame = start_frame;
            if !whole {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mri_read(): bad frame ('{pound}')\n"),
                ));
            }
        }
        for f in [start_frame, end_frame] {
            if f < 0 {
                return Ok(fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mri_read(): frame ({f}) is less than zero\n"),
                ));
            }
        }
        if start_frame > end_frame {
            return Ok(fail0(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "mri_read(): the start frame ({start_frame}) is greater than the end frame ({end_frame})\n"
                ),
            ));
        }
    }
    let read = if mtype == MRI_MGH_FILE {
        mgh_read(g, out, err, env, dir, &copy, volume_flag, -1)?
    } else if mtype == identify::NPY_FILE {
        crate::fscnpy::npy2mri(g, out, err, dir, &copy, false)?
    } else if mtype == identify::BRUKER_FILE {
        crate::bruker::bruker_read(g, out, err, dir, &copy, volume_flag)?
    } else if mtype == identify::GIFTI_FILE {
        let mut io = Io { out, err, env };
        crate::gifti_vol::mris_read_gifti_as_mri(g, &mut io, dir, &copy, volume_flag)?
    } else if let Some(name) = not_ported(mtype) {
        let _ = writeln!(
            err,
            "larmorx-freesurfer: reading {name} volumes is not ported yet ({copy})"
        );
        return Ok(None);
    } else {
        let _ = writeln!(err, "mri_read(): type = {mtype}");
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            "mri_read(): code inconsistency (file type recognized but not caught)",
        ));
    };
    let Some(mut m) = read else {
        return Ok(None);
    };
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    vol_geom::fill_cache(g, out, err, &mut m.vg)?;
    if start_frame == -1 {
        return Ok(Some(m));
    }
    if start_frame >= m.nframes {
        let n = m.nframes;
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "mri_read(): start frame ({start_frame}) is out of range ({n} frames in volume)"
            ),
        ));
    }
    if end_frame >= m.nframes {
        let n = m.nframes;
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!("mri_read(): end frame ({end_frame}) is out of range ({n} frames in volume)"),
        ));
    }
    if !volume_flag {
        if m.chunk.is_none() && m.mtype == MRI_FLOAT {
            let n = i64::from(m.width) * i64::from(m.height) * i64::from(m.depth);
            if n > 0 && m.nframes > 0 {
                g.notices.add(
                    &FS_255,
                    "mri_read of a float volume's header with a frame list: the voxel check skipped (FreeSurfer reads voxels the header does not have)",
                );
            }
        } else if nan_inf_check(g, err, &m) != NO_ERROR {
            return Ok(None);
        }
        let Some(mut m2) =
            larmorx_freesurfer_mri::copy::mri_copy(g, out, err, &m, None)?.into_new()
        else {
            return Ok(None);
        };
        m2.nframes = end_frame - start_frame + 1;
        return Ok(Some(m2));
    }
    if start_frame == 0 && end_frame == m.nframes - 1 {
        if nan_inf_check(g, err, &m) != NO_ERROR {
            return Ok(None);
        }
        return Ok(Some(m));
    }
    let nf = end_frame - start_frame + 1;
    let mut m2 = mri::mri_alloc_sequence(g, out, err, m.width, m.height, m.depth, m.mtype, nf)?;
    mri::mri_copy_header(g, out, err, &m, Some(&mut m2))?;
    m2.nframes = nf;
    m2.imnr0 = nf;
    if matches!(
        m2.mtype,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | mri::MRI_LONG | MRI_FLOAT
    ) {
        // the frames' voxels, slice by slice (the element loops copy every voxel)
        let bpv = m2.bytes_per_vox;
        let frame_bytes =
            m2.vox_per_row as usize * m2.height.max(0) as usize * m2.depth.max(0) as usize * bpv;
        if let (Some(src), Some(dst)) = (m.chunk.as_ref(), m2.chunk.as_mut()) {
            let s0 = start_frame as usize * frame_bytes;
            dst[..nf as usize * frame_bytes]
                .copy_from_slice(&src[s0..s0 + nf as usize * frame_bytes]);
        }
    }
    if nan_inf_check(g, err, &m) != NO_ERROR {
        return Ok(None);
    }
    Ok(Some(m2))
}

/// The writers [`mri_write_type`] does not have yet: their formats' names.
fn write_not_ported(mtype: i32) -> Option<&'static str> {
    use crate::identify::*;
    Some(match mtype {
        MRI_CORONAL_SLICE_DIRECTORY => "COR",
        MRI_MINC_FILE => "MINC",
        IMAGE_FILE => "image",
        BHDR => "bhdr",
        BSHORT_FILE => "bshort",
        BFLOAT_FILE => "bfloat",
        MRI_ANALYZE_FILE => "ANALYZE",
        MRI_ANALYZE4D_FILE => "ANALYZE 4D",
        BRIK_FILE => "AFNI BRIK",
        MGH_MORPH => "GCA morph (m3z)",
        ITK_MORPH => "ITK warp",
        MRI_MGH_FILE => "MGH",
        GDF_FILE => "GDF",
        NIFTI1_FILE => "NIfTI-1",
        NII_FILE => "NIfTI",
        NRRD_FILE => "NRRD",
        _ => return None,
    })
}

/// The formats `MRIwriteType` refuses to write: the name its message gives.
fn write_unsupported(mtype: i32) -> Option<&'static str> {
    use crate::identify::*;
    Some(match mtype {
        GENESIS_FILE => "GENESIS",
        GE_LX_FILE => "GE LX",
        SIEMENS_FILE => "SIEMENS",
        SDT_FILE => "SDT",
        OTL_FILE => "OTL",
        RAW_FILE => "RAW",
        SIGNA_FILE => "SIGNA",
        DICOM_FILE => "DICOM",
        SIEMENS_DICOM_FILE => "SIEMENS DICOM",
        BRUKER_FILE => "BRUKER",
        XIMG_FILE => "XIMG",
        _ => return None,
    })
}

/// `ErrorReturn(code, (code, message))` after `errno = 0`: the message as an error, `code`.
fn fail_code(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> i32 {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    code
}

/// `MRIwriteType(mri, fname, type, intent)`: the volume written as `type`.
///
/// A name that is an existing directory is refused for every format but COR (`MRIwriteType(): <f>
/// is an existing directory. (type=<t>)` and, on the next line, `A filename should be specified
/// instead.`; `ERROR_BADFILE`). The formats FreeSurfer cannot write are refused with
/// `MRIwriteType(): writing of <format> file type not supported` (`ERROR_BADPARM`): Genesis, GE LX,
/// Siemens, SDT, outline, raw, Signa, DICOM, Siemens DICOM, Bruker, XIMG; an unknown type with
/// `MRIwriteType(): code inconsistency (file type recognized but not caught)`. An image file needs a
/// depth of 1 (`MRIwriteType(<f>): image files cannnot have depth > 1`, the program ends), a GCA or
/// ITK morph three frames (`MRIwriteType: gcam - input mri must have 3 frames, not <n>`, `MRIwriteType:
/// itk warp - ...`, -1). GIFTI files: [`crate::gifti_vol::mri_write_gifti`]. After a writer that
/// succeeds, a volume with b-values also writes them beside it: `Saving bvals and bvecs` on
/// standard output, `<stem>.bvals` and `<stem>.<space>_space.bvecs` (`scanner`, `voxel` or
/// `unknown`), the stem the name without its volume extension (none: nothing written). The other
/// writers are not ported yet: a message and -1.
#[allow(clippy::too_many_arguments)]
pub fn mri_write_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
    mtype: i32,
    _intent: i32,
) -> Result<i32, Fatal> {
    use crate::identify::*;
    if mtype != MRI_CORONAL_SLICE_DIRECTORY
        && std::fs::metadata(utils::resolve(dir, fname)).is_ok_and(|md| md.is_dir())
    {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADFILE,
            &format!(
                "MRIwriteType(): {fname} is an existing directory. (type={mtype})\n                A filename should be specified instead."
            ),
        ));
    }
    if mtype == IMAGE_FILE && m.depth != 1 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIwriteType({fname}): image files cannnot have depth > 1\n"),
        )?;
    }
    if mtype == MGH_MORPH && m.nframes != 3 {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!(
                "MRIwriteType: gcam - input mri must have 3 frames, not {}",
                m.nframes
            ),
        );
        return Ok(-1);
    }
    if mtype == ITK_MORPH && m.nframes != 3 {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!(
                "MRIwriteType: itk warp - input mri must have 3 frames, not {}",
                m.nframes
            ),
        );
        return Ok(-1);
    }
    let error = if mtype == GIFTI_FILE {
        let mut io = Io { out, err, env };
        crate::gifti_vol::mri_write_gifti(g, &mut io, dir, m, fname)?
    } else if let Some(name) = write_not_ported(mtype) {
        let _ = writeln!(
            err,
            "larmorx-freesurfer: writing {name} volumes is not ported yet ({fname})"
        );
        return Ok(-1);
    } else if let Some(name) = write_unsupported(mtype) {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIwriteType(): writing of {name} file type not supported"),
        ));
    } else {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            "MRIwriteType(): code inconsistency (file type recognized but not caught)",
        ));
    };
    if error != 0 || m.bvals.is_none() {
        return Ok(error);
    }
    let Some(stem) = identify::id_stem_from_name(fname) else {
        return Ok(error);
    };
    let _ = out.write_all(b"Saving bvals and bvecs\n");
    crate::dti::dti_write_bvalues(g, out, dir, m.bvals.as_ref(), &format!("{stem}.bvals"));
    let space = match m.bvec_space {
        larmorx_freesurfer_core::fsenv::BVEC_SPACE_SCANNER => "scanner",
        larmorx_freesurfer_core::fsenv::BVEC_SPACE_VOXEL => "voxel",
        _ => "unknown",
    };
    crate::dti::dti_write_bvectors(
        g,
        out,
        dir,
        m.bvecs.as_ref(),
        &format!("{stem}.{space}_space.bvecs"),
    );
    Ok(error)
}

/// `MRIwrite(mri, fname, mriVector, intent)`: the volume written in the format its name gives
/// (`mri_identify`; none: `unknown file type for file (<f>)`, `ERROR_BADPARM`), by
/// [`mri_write_type`]. With more than one volume in `several`, each is written in that format
/// under the name with its own suffix before the extension (the volume's file-name suffix, or
/// `_<n>` when it has none): the extension is what follows the last dot before the first `.gz`
/// (which stays with it), `(null)` when there is no dot. FS-432: FreeSurfer builds each name in
/// 1,024 bytes (a longer one overflows its buffer); larmorx writes the whole name. The last
/// volume's status.
#[allow(clippy::too_many_arguments)]
pub fn mri_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
    several: Option<&[Mri]>,
    intent: i32,
) -> Result<i32, Fatal> {
    larmorx_freesurfer_core::chklc::chklc();
    let int_type = identify::mri_identify(g, dir, env, out, err, fname)?;
    if int_type < 0 {
        return Ok(fail_code(
            g,
            err,
            ERROR_BADPARM,
            &format!("unknown file type for file ({fname})"),
        ));
    }
    let Some(vs) = several.filter(|v| v.len() > 1) else {
        return mri_write_type(g, out, err, env, dir, m, fname, int_type, intent);
    };
    // `.gz` (its first place) cut off, the extension after the last dot before it, `.gz` put back
    let b = fname.as_bytes();
    let gz = b.windows(3).position(|w| w == b".gz");
    let head = &b[..gz.unwrap_or(b.len())];
    let (stem, ext): (&[u8], Option<Vec<u8>>) = match head.iter().rposition(|&c| c == b'.') {
        Some(d) => {
            let mut e = head[d + 1..].to_vec();
            if let Some(z) = gz {
                e.extend_from_slice(&b[z..]);
            }
            (&b[..d], Some(e))
        }
        None => (b, None),
    };
    let ext = ext.map_or_else(|| b"(null)".to_vec(), |e| e);
    let mut error = NO_ERROR;
    for (n, v) in vs.iter().enumerate() {
        let mut name = stem.to_vec();
        if v.len_fname_post_fixes == 0 {
            name.extend_from_slice(format!("_{n}").as_bytes());
        } else {
            name.extend_from_slice(v.fname_post_fixes.as_bytes());
        }
        name.push(b'.');
        name.extend_from_slice(&ext);
        if name.len() > 1023 {
            g.notices.add(
                &FS_432,
                format!(
                    "MRIwrite: a name of {} bytes for one of several volumes (FreeSurfer's buffer holds 1,023)",
                    name.len()
                ),
            );
        }
        let name = String::from_utf8_lossy(&name).into_owned();
        error = mri_write_type(g, out, err, env, dir, v, &name, int_type, intent)?;
    }
    Ok(error)
}

/// The value `orderXBytes` gives for a big-endian value of `n` bytes at the start of `b`: the
/// bytes reversed into the machine's (little-endian) order.
fn reverse_each(b: &mut [u8], n: usize) {
    for c in b.chunks_exact_mut(n) {
        c.reverse();
    }
}

/// `znzread(buf, size, nmemb, fp)` into a buffer that may be too small: the count.
fn zread(fp: &mut ZnzFile, buf: &mut [u8], size: usize, nmemb: usize) -> i64 {
    fio::fs_znzread(fp, buf, size, nmemb)
}

/// `mghRead(fname, read_volume, frame)`: an MGH volume (`.mgh`; `.mgz` and `.mgh.gz` compressed).
///
/// The header: version, width, height, depth, frames, type, dof (big-endian `int`s), the flag of
/// the RAS fields (`short`) and, when it is above 0, the voxel sizes, the direction cosines and
/// the centre (15 floats); 256 bytes in all after the flag. Without voxels the data are skipped;
/// with them they are read slice by slice (`mghRead(<name>): could not read <n> bytes at slice
/// <z>`; types: uchar, short, unsigned short, int, float, tensor (9 frames), float complex; another
/// `mghRead: unsupported type <t>`); `frame` 0 or more reads that frame alone, below -1 the first
/// `-frame` frames. Without the RAS flag the volume is coronal (centre 0). Then TR, the flip angle,
/// TE, TI and the field of view, each only after the one before it was read; the tags
/// ([`mri_tag_read`]); the start and end of each axis from the voxel size, the field of view the
/// largest of their spans. A name without `.mgh`, `.mgz` or `.mgh.gz` is refused (`mghRead(<name>,
/// <frame>): could not open file.` + `Filename extension must be .mgh, .mgh.gz or .mgz`).
#[allow(clippy::too_many_arguments)]
pub fn mgh_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
    frame: i32,
) -> Result<Option<Mri>, Fatal> {
    let mut gzipped = false;
    let mut valid_ext = false;
    if let Some(i) = fname.rfind('.') {
        let ext = &fname[i + 1..];
        if utils::stricmp(g, ext, "mgz") == 0 || fname.contains("mgh.gz") {
            gzipped = true;
            valid_ext = true;
        } else if utils::stricmp(g, ext, "mgh") == 0 {
            valid_ext = true;
        }
    }
    if !valid_ext {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "mghRead({fname}, {frame}): could not open file.\nFilename extension must be .mgh, .mgh.gz or .mgz"
            ),
        );
        return Ok(None);
    }
    let path = utils::resolve(dir, fname);
    let Some(mut fp) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"rb", gzipped) else {
        return Ok(fail0(
            g,
            err,
            ERROR_BADPARM,
            &format!("mghRead({fname}, {frame}): could not open file"),
        ));
    };
    let mut version = 0i32;
    if fio::znz_read_int_ex(&mut fp, &mut version) == 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("mghRead({fname}, {frame}): read error"),
        );
        return Ok(None);
    }
    let width = fio::znz_read_int(g, err, &mut fp);
    let height = fio::znz_read_int(g, err, &mut fp);
    let depth = fio::znz_read_int(g, err, &mut fp);
    let mut nframes = fio::znz_read_int(g, err, &mut fp);
    let mtype = fio::znz_read_int(g, err, &mut fp);
    let dof = fio::znz_read_int(g, err, &mut fp);
    let mut unused = UNUSED_SPACE_SIZE - 2;
    let good_ras_flag = i32::from(fio::znz_read_short(g, err, &mut fp));
    let mut ras = [0f32; 15];
    if good_ras_flag > 0 {
        unused -= USED_SPACE_SIZE;
        for v in &mut ras {
            *v = fio::znz_read_float(g, err, &mut fp);
        }
    }
    let mut ubuf = [0u8; UNUSED_SPACE_SIZE as usize + 1];
    zread(&mut fp, &mut ubuf, 1, unused as usize);
    let size = mri::mri_sizeof(mtype);
    let bpv: i32 = if size == usize::MAX { -1 } else { size as i32 };
    if mtype == MRI_TENSOR {
        nframes = 9;
    }
    let bytes = width.wrapping_mul(height).wrapping_mul(bpv);
    let total = || -> i64 {
        i64::from(nframes)
            .wrapping_mul(i64::from(width))
            .wrapping_mul(i64::from(height))
            .wrapping_mul(i64::from(depth))
            .wrapping_mul(i64::from(bpv))
    };
    let mut m;
    if !read_volume {
        m = mri::mri_alloc_header(g, out, err, width, height, depth, mtype, nframes)?;
        m.dof = dof;
        m.nframes = nframes;
        m.version = version;
        m.intent = (version >> 8) & 0xffff;
        debug_intent(g, out, &m);
        let total_bytes = total();
        if gzipped {
            let mut buf = [0u8; STRLEN];
            let mut count: i64 = 0;
            let mut ended = false;
            while count < total_bytes.wrapping_sub(STRLEN as i64) {
                if zread(&mut fp, &mut buf, STRLEN, 1) < 1 {
                    // nothing more to read: every later read gives nothing
                    ended = true;
                    break;
                }
                count += STRLEN as i64;
            }
            if !ended {
                let rest = total_bytes.wrapping_sub(count);
                if rest == 0 {
                    // FreeSurfer's znzread of a member of 0 bytes divides by zero
                    g.notices.add(
                        &FS_76,
                        "mghRead of a compressed volume without voxels: the data's skip of 0 bytes (FreeSurfer divides by zero)",
                    );
                } else {
                    zread(&mut fp, &mut buf, rest as usize, 1);
                }
            }
        } else {
            fp.seek(total_bytes, Whence::Cur);
        }
    } else {
        let (start_frame, end_frame);
        if frame >= 0 {
            start_frame = frame;
            end_frame = frame;
            let skip = i64::from(frame)
                .wrapping_mul(i64::from(width))
                .wrapping_mul(i64::from(height))
                .wrapping_mul(i64::from(depth))
                .wrapping_mul(i64::from(bpv));
            if gzipped {
                let mut c = 0i64;
                while c < skip {
                    if fp.getc() < 0 {
                        break;
                    }
                    c += 1;
                }
            } else {
                fp.seek(skip, Whence::Cur);
            }
            nframes = 1;
        } else {
            if frame < -1 {
                nframes = frame.wrapping_neg();
            }
            start_frame = 0;
            end_frame = nframes - 1;
            if g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g) {
                let _ = writeln!(err, "read {nframes} frames");
            }
        }
        m = mri::mri_alloc_sequence(g, out, err, width, height, depth, mtype, nframes)?;
        m.dof = dof;
        m.version = version;
        m.intent = (version >> 8) & 0xffff;
        debug_intent(g, out, &m);
        let timing = getenv(env, "FS_MGZIO_TIMING");
        let begin = std::time::Instant::now();
        if timing {
            let _ = out.write_all(b"INFO: Environment variable FS_MGZIO_TIMING set\n");
        }
        let voxelbuf = m.ischunked && getenv(env, "FS_MGZIO_USEVOXELBUFREAD");
        if voxelbuf {
            let _ = out.write_all(b"INFO: Environment variable FS_MGZIO_USEVOXELBUFREAD set\n");
            let _ = out.write_all(b"mghRead(): read voxel buffer ...\n");
            let per_slice = bytes as i64 as u64 as usize;
            let nfr = (end_frame - start_frame + 1) as i64;
            let to_read = (bytes as i64 as u64)
                .wrapping_mul(depth as i64 as u64)
                .wrapping_mul(nfr as u64) as usize;
            let frame_bytes = (m.vox_per_row as usize)
                .wrapping_mul(m.height.max(0) as usize)
                .wrapping_mul(m.depth.max(0) as usize)
                .wrapping_mul(m.bytes_per_vox);
            if i64::from(m.depth) * i64::from(m.nframes) <= 0 {
                g.notices.add(
                    &FS_256,
                    "mghRead of a volume without slices with FS_MGZIO_USEVOXELBUFREAD: nothing read (FreeSurfer takes the address of a slice it does not have)",
                );
            }
            let mut place = if start_frame > 0 {
                g.notices.add(
                    &FS_256,
                    format!(
                        "mghRead of frame {start_frame} with FS_MGZIO_USEVOXELBUFREAD: read into the volume's only frame (FreeSurfer writes past it)"
                    ),
                );
                0
            } else {
                (start_frame as usize).wrapping_mul(frame_bytes)
            };
            let mut left = to_read;
            let chunk = m.chunk.as_mut().expect("voxels");
            while left > 0 {
                let mut slice = vec![0u8; per_slice];
                let r = zread(&mut fp, &mut slice, 1, per_slice);
                if r as usize != per_slice {
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!(
                            "mghRead (FS_MGZIO_USEVOXELBUFREAD): could not read {per_slice} bytes per slice, read only {} bytes\n",
                            r as u64
                        ),
                    );
                    return Ok(None);
                }
                match bpv {
                    2 => reverse_each(&mut slice, 2),
                    4 | 8 => reverse_each(&mut slice, 4),
                    _ => {}
                }
                let n = per_slice.min(chunk.len().saturating_sub(place));
                chunk[place..place + n].copy_from_slice(&slice[..n]);
                left -= r as usize;
                place += r as usize;
            }
        } else {
            if bytes < 0 {
                // calloc of a negative size (as size_t) fails
                g.errno = ENOMEM;
            }
            let mut buf = vec![0u8; bytes.max(0) as usize];
            let row = m.vox_per_row as usize;
            for f in start_frame..=end_frame {
                for z in 0..depth {
                    let r = zread(&mut fp, &mut buf, 1, bytes as i64 as u64 as usize);
                    if r as i32 != bytes {
                        log::error_printf(
                            g,
                            err,
                            ERROR_BADFILE,
                            &format!("mghRead({fname}): could not read {bytes} bytes at slice {z}"),
                        );
                        return Ok(None);
                    }
                    let fr = f - start_frame;
                    let unit = match mtype {
                        MRI_INT | MRI_FLOAT | MRI_TENSOR | MRI_FLOAT_COMPLEX => 4,
                        MRI_SHORT | MRI_USHRT => 2,
                        MRI_UCHAR => 1,
                        _ => {
                            g.errno = 0;
                            log::error_printf(
                                g,
                                err,
                                ERROR_UNSUPPORTED,
                                &format!("mghRead: unsupported type {}", m.mtype),
                            );
                            return Ok(None);
                        }
                    };
                    reverse_each(&mut buf, unit);
                    let bpvu = bpv as usize;
                    let Some(chunk) = m.chunk.as_mut() else {
                        continue;
                    };
                    for y in 0..height {
                        // a tensor volume's rows all point at the voxels' start (MRI::initSlices
                        // advances no row pointer for that type): each row read overwrites it
                        let o = if mtype == MRI_TENSOR {
                            0
                        } else {
                            (((fr * depth + z) * height + y) as usize * row) * bpvu
                        };
                        let s = y as usize * width.max(0) as usize * bpvu;
                        let n = width.max(0) as usize * bpvu;
                        if o + n <= chunk.len() && s + n <= buf.len() {
                            chunk[o..o + n].copy_from_slice(&buf[s..s + n]);
                        }
                    }
                }
            }
        }
        if timing {
            let t = begin.elapsed();
            let _ = writeln!(
                out,
                "Total time (mghRead) = {}.{:09} seconds{}",
                t.as_secs(),
                t.subsec_nanos(),
                if voxelbuf { " (USEVOXELBUF)" } else { "" }
            );
        }
    }
    if good_ras_flag > 0 {
        (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (ras[0], ras[1], ras[2]);
        m.ps = m.vg.xsize;
        m.thick = m.vg.zsize;
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (ras[3], ras[4], ras[5]);
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (ras[6], ras[7], ras[8]);
        (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (ras[9], ras[10], ras[11]);
        (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (ras[12], ras[13], ras[14]);
        m.vg.ras_good_flag = 1;
    } else {
        if g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g) {
            let _ = write!(
                err,
                "-----------------------------------------------------------------\nCould not find the direction cosine information.\nWill use the CORONAL orientation.\nIf not suitable, please provide the information in {fname}.\n-----------------------------------------------------------------\n"
            );
        }
        set_direction_cosine(g, err, &mut m.vg, MRI_CORONAL);
    }
    if fio::znz_read_float_ex(&mut fp, &mut m.tr) != 0 {
        let mut fval = 0f32;
        if fio::znz_read_float_ex(&mut fp, &mut fval) != 0 {
            m.flip_angle = f64::from(fval);
            if fio::znz_read_float_ex(&mut fp, &mut m.te) != 0
                && fio::znz_read_float_ex(&mut fp, &mut m.ti) != 0
            {
                fio::znz_read_float_ex(&mut fp, &mut m.fov);
            }
        }
    }
    mri_tag_read(
        g, out, err, dir, &mut m, &mut fp, fname, env, false, -1, None, None,
    )?;
    fp.close();
    let half = |n: i32| -> f64 { f64::from(n.wrapping_neg()) / 2.0 };
    m.xstart = sse::mul64(half(m.vg.width), f64::from(m.vg.xsize)) as f32;
    m.xend = sse::mul64(f64::from(m.vg.width) / 2.0, f64::from(m.vg.xsize)) as f32;
    m.ystart = sse::mul64(half(m.vg.height), f64::from(m.vg.ysize)) as f32;
    m.yend = sse::mul64(f64::from(m.vg.height) / 2.0, f64::from(m.vg.ysize)) as f32;
    m.zstart = sse::mul64(half(m.vg.depth), f64::from(m.vg.zsize)) as f32;
    m.zend = sse::mul64(f64::from(m.vg.depth) / 2.0, f64::from(m.vg.zsize)) as f32;
    let xfov = sse::sub(m.xend, m.xstart);
    let yfov = sse::sub(m.yend, m.ystart);
    let zfov = sse::sub(m.zend, m.zstart);
    m.fov = if xfov > yfov {
        if xfov > zfov { xfov } else { zfov }
    } else if yfov > zfov {
        yfov
    } else {
        zfov
    };
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    Ok(Some(m))
}

fn debug_intent(g: &Globals, out: &mut dyn Write, m: &Mri) {
    if g.gdiag & DIAG_INFO != 0 {
        let _ = writeln!(
            out,
            "[DEBUG] mghRead() mri->intent = {}, mri->version = {}",
            m.intent, m.version
        );
    }
}

/// The bytes `n` of a tag's data as a C buffer of `cap` bytes holds them after `read_data` (the
/// bytes the file gives over the buffer's earlier contents; FS-257 for a length the machine cannot
/// allocate).
fn read_tag_text(
    g: &mut Globals,
    io: &mut FsTagsIo<'_>,
    len: i64,
    what: &str,
) -> Result<Vec<u8>, Fatal> {
    if !(0..=i64::from(i32::MAX)).contains(&len) {
        g.notices.add(
            &FS_257,
            format!(
                "MRITAGread: a {what} tag of length {len} (FreeSurfer's allocation fails or is written outside)"
            ),
        );
        if len < 0 {
            return Ok(Vec::new());
        }
    }
    // the file's bytes, read in pieces (a length larger than the file reads what it has)
    let mut v = Vec::new();
    let mut left = len.max(0) as u64;
    let mut piece = vec![0u8; 1 << 16];
    while left > 0 {
        let k = left.min(piece.len() as u64) as usize;
        let before = io.fp.tell();
        fio::fs_znzread(io.fp, &mut piece[..k], 1, k);
        let after = io.fp.tell();
        let got = if before >= 0 && after >= before {
            ((after - before) as usize).min(k)
        } else {
            0
        };
        v.extend_from_slice(&piece[..got]);
        left -= k as u64;
        if got < k {
            break;
        }
    }
    Ok(v)
}

/// A C string's bytes (up to its first NUL).
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `MRITAGread(mri, fp, fname, niftiheaderext, mgztaglen, ras_xform, has_ras_xform)`: the tags
/// after a volume's data, until the end of the file (or, in FreeSurfer's NIfTI header extension,
/// until fewer than 12 bytes of `mgztaglen` are left); nothing with `FS_SKIP_TAGS` set.
///
/// The degrees of freedom, the scan parameters and the RAS transform only in the extension
/// (elsewhere `[WARN] skip unexpected tag TAG_DOF, nifti header extension only` and the like, the
/// tag skipped); the frames (`couldn't read frame structure from file` when that fails); the old
/// colour table; the talairach transform's name (`TAG_MGH_XFORM`): `<the volume's folder>/
/// transforms/talairach.xfm` instead when that file exists, the transform read when the named file
/// exists (`error loading transform from <name>` and no name when it does not read); the command
/// lines (more than 1,000: `MRITAGread(<name>): too many commands (<n>) in file`, the program
/// ends); the auto-align and original RAS-to-voxel matrices; the phase-encoding direction; the
/// field strength (4 bytes in the machine's order whatever the tag's length); the morph's
/// metadata, geometries, affine and labels. Other tags are skipped.
#[allow(clippy::too_many_arguments)]
pub fn mri_tag_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &mut Mri,
    fp: &mut ZnzFile,
    fname: &str,
    env: &Environment,
    niftiheaderext: bool,
    mut mgztaglen: i64,
    mut ras_xform: Option<&mut VolGeom>,
    mut has_ras_xform: Option<&mut bool>,
) -> Result<(), Fatal> {
    if getenv(env, "FS_SKIP_TAGS") {
        return Ok(());
    }
    let info = g.gdiag & DIAG_INFO != 0;
    // the transform name's buffer: a fresh volume's has its first byte 0 and the rest never written
    // (FS-421; 0 here), a name copied in has its NUL; `tname_known` bytes are written
    let mut tname = [0u8; STRLEN];
    let n0 = m.transform_fname.len().min(STRLEN);
    tname[..n0].copy_from_slice(&m.transform_fname.as_bytes()[..n0]);
    let mut tname_known = (n0 + 1).min(STRLEN);
    let mut io = FsTagsIo::new(fp, niftiheaderext);
    loop {
        let mut len: i64 = 0;
        let tag = io.read_tagid_len(g, err, &mut len, 0);
        if tag == 0 {
            if info {
                let _ = writeln!(
                    out,
                    "[DEBUG] MRITAGread(): remaining taglen = {mgztaglen} (tag = {tag})"
                );
            }
            break;
        }
        if info {
            let _ = writeln!(
                out,
                "[DEBUG] MRITAGread(): remaining taglen = {mgztaglen} (tag = {tag}, len = {len})"
            );
        }
        match tag {
            TAG_DOF => {
                if niftiheaderext {
                    m.dof = io.read_dof(g, err);
                } else {
                    let _ = out.write_all(
                        b"[WARN] skip unexpected tag TAG_DOF, nifti header extension only\n",
                    );
                    io.skip_tag(g, err, tag, len)?;
                }
            }
            TAG_SCAN_PARAMETERS => {
                if niftiheaderext {
                    io.read_scan_parameters(g, out, err, m, len);
                } else {
                    let _ = out.write_all(
                        b"[WARN] skip unexpected tag TAG_SCAN_PARAMETERS, nifti header extension only\n",
                    );
                    io.skip_tag(g, err, tag, len)?;
                }
            }
            TAG_RAS_XFORM => {
                if niftiheaderext {
                    match ras_xform.as_deref_mut() {
                        Some(vg) => {
                            io.read_ras_xform(g, out, err, vg);
                        }
                        None => {
                            io.skip_tag(g, err, tag, len)?;
                        }
                    }
                    if let Some(h) = has_ras_xform.as_deref_mut() {
                        *h = true;
                    }
                } else {
                    let _ = out.write_all(
                        b"[WARN] skip unexpected tag TAG_RAS_XFORM, nifti header extension only\n",
                    );
                    io.skip_tag(g, err, tag, len)?;
                    if let Some(h) = has_ras_xform.as_deref_mut() {
                        *h = false;
                    }
                }
            }
            TAG_RAS_XFORM_EXTRA => match ras_xform.as_deref_mut() {
                Some(vg) => {
                    io.read_ras_xform_extra(g, out, err, vg);
                }
                None => {
                    g.notices.add(
                        &FS_253,
                        "MRITAGread: a RAS transform tag outside the NIfTI extension skipped (FreeSurfer writes through a null geometry)",
                    );
                    io.skip_tag(g, err, tag, len)?;
                }
            },
            TAG_MRI_FRAME => {
                if io.read_mri_frames(g, out, err, m, len) != NO_ERROR {
                    let _ = err.write_all(b"couldn't read frame structure from file\n");
                }
            }
            TAG_OLD_COLORTABLE => {
                m.ct = io.read_old_colortable(g, out, err);
            }
            TAG_OLD_MGH_XFORM | TAG_MGH_XFORM => {
                let folder = crate::fio::fio_dirname(g, Some(fname)).unwrap_or_default();
                let tmpstr = format!("{folder}/transforms/talairach.xfm");
                if tmpstr.len() >= 1000 {
                    g.notices.add(
                        &FS_254,
                        format!(
                            "MRITAGread: a transform name of {} bytes used whole (FreeSurfer's buffer holds 999)",
                            tmpstr.len()
                        ),
                    );
                }
                let data = read_tag_text(g, &mut io, len, "talairach")?;
                if data.len() > STRLEN {
                    g.notices.add(
                        &FS_254,
                        format!(
                            "MRITAGread: a talairach tag of {} bytes read whole (FreeSurfer's name holds 1024)",
                            data.len()
                        ),
                    );
                }
                // the bytes over the name's buffer; a longer tag's name is its bytes up to a NUL
                let n = data.len().min(STRLEN);
                tname[..n].copy_from_slice(&data[..n]);
                tname_known = tname_known.max(n);
                let mut name: Vec<u8> = if data.len() > STRLEN {
                    c_str(&data).to_vec()
                } else {
                    c_str(&tname).to_vec()
                };
                if data.len() <= STRLEN && !tname[..tname_known].contains(&0) {
                    g.notices.add(
                        &FS_421,
                        format!(
                            "MRITAGread: a talairach tag of {len} bytes without a NUL: the name's bytes after it never written (0 here)"
                        ),
                    );
                }
                if info {
                    let _ = writeln!(
                        out,
                        "[DEBUG] MRITAGread() TAG_MGH_XFORM = {} ({len})",
                        String::from_utf8_lossy(&name)
                    );
                }
                if utils::file_exists(dir, &tmpstr) {
                    name = tmpstr.into_bytes();
                    let k = name.len().min(STRLEN - 1);
                    tname[..k].copy_from_slice(&name[..k]);
                    tname[k] = 0;
                    tname_known = tname_known.max(k + 1);
                }
                let lossy = String::from_utf8_lossy(&name).into_owned();
                m.transform_fname = lossy.clone();
                if utils::file_exists(dir, &lossy) {
                    match minc::input_transform_file(g, out, err, dir, &name) {
                        Some(t) => {
                            m.linear_transform = Some(mri::get_linear_transform_ptr(err, &t)?);
                            m.inverse_linear_transform =
                                Some(mri::get_inverse_linear_transform_ptr(err, &t)?);
                            m.transform = Some(t);
                            m.free_transform = 1;
                            if diag::verbose_on(g) {
                                let _ = writeln!(err, "INFO: loaded talairach xform : {lossy}");
                            }
                        }
                        None => {
                            g.errno = 0;
                            log::error_printf(
                                g,
                                err,
                                ERROR_BAD_FILE,
                                &format!("error loading transform from {lossy}"),
                            );
                            m.linear_transform = None;
                            m.inverse_linear_transform = None;
                            m.free_transform = 1;
                            tname[0] = 0;
                            m.transform_fname.clear();
                        }
                    }
                }
            }
            TAG_CMDLINE => {
                if m.cmdlines.len() >= MAX_CMDS {
                    log::error_exit(
                        g,
                        err,
                        ERROR_NOMEMORY,
                        &format!(
                            "MRITAGread({fname}): too many commands ({}) in file",
                            m.cmdlines.len()
                        ),
                    )?;
                }
                let data = read_tag_text(g, &mut io, len, "command-line")?;
                m.cmdlines
                    .push(String::from_utf8_lossy(c_str(&data)).into_owned());
            }
            TAG_AUTO_ALIGN => {
                m.auto_align = Some(io.read_matrix(g, out, err));
            }
            TAG_ORIG_RAS2VOX => {
                m.orig_ras2vox = Some(io.read_matrix(g, out, err));
            }
            TAG_PEDIR => {
                let data = read_tag_text(g, &mut io, len, "phase-encoding")?;
                m.pedir = Some(c_str(&data).to_vec());
            }
            TAG_FIELDSTRENGTH => {
                let mut b = m.field_strength.to_le_bytes();
                io.read_data(&mut b, 4);
                m.field_strength = f32::from_le_bytes(b);
            }
            TAG_GCAMORPH_META => {
                let (a, b, c) = io.read_gcamorph_meta(g, err);
                (m.warp_field_format, m.gcamorph_spacing, m.gcamorph_exp_k) = (a, b, c);
                if info {
                    let _ = out.write_all(b"[DEBUG] MRITAGread() TAG_GCAMORPH_META\n");
                    let _ = writeln!(
                        out,
                        "[DEBUG] MRITAGread() warpFieldFormat = {a}, gcamorphSpacing = {b}, gcamorphExp_k = {c:.6}"
                    );
                }
            }
            TAG_GCAMORPH_GEOM | TAG_GCAMORPH_GEOM_PLUSSHEAR => {
                let shearless = tag == TAG_GCAMORPH_GEOM;
                let mut a = std::mem::take(&mut m.gcamorph_image_vg);
                let mut b = std::mem::take(&mut m.gcamorph_atlas_vg);
                io.read_gcamorph_geom(g, err, &mut a, &mut b, shearless);
                (m.gcamorph_image_vg, m.gcamorph_atlas_vg) = (a, b);
                if info {
                    let what = if shearless {
                        "TAG_GCAMORPH_GEOM"
                    } else {
                        "TAG_GCAMORPH_GEOM_PLUSSHEAR"
                    };
                    let _ = writeln!(out, "[DEBUG] MRITAGread() {what}");
                    vol_geom::vgprint(out, &m.gcamorph_image_vg, true);
                    vol_geom::vgprint(out, &m.gcamorph_atlas_vg, true);
                }
            }
            TAG_GCAMORPH_AFFINE => {
                let a = io.read_matrix(g, out, err);
                if info {
                    let _ = out.write_all(b"[DEBUG] MRITAGread() TAG_GCAMORPH_AFFINE\n");
                    let mut buf = Vec::new();
                    larmorx_freesurfer_core::matrix_ops::matrix_print(
                        g,
                        Some(&mut buf),
                        out,
                        err,
                        Some(&a),
                        5,
                    );
                    let _ = out.write_all(&buf);
                }
                m.gcamorph_affine = Some(a);
            }
            TAG_GCAMORPH_LABELS => {
                m.init_gcamorph_label();
                if info {
                    let _ = out.write_all(b"[DEBUG] read TAG_GCAMORPH_LABELS\n");
                }
                let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
                let mut labels = m.gcamorph_label.take().unwrap_or_default();
                io.read_gcamorph_labels(g, err, w, h, d, &mut labels);
                m.gcamorph_label = Some(labels);
            }
            _ => {
                io.skip_tag(g, err, tag, len)?;
            }
        }
        if niftiheaderext {
            mgztaglen -= len + LEN_TAGHEADER;
            if mgztaglen < LEN_TAGHEADER {
                if info {
                    let _ = writeln!(out, "[DEBUG] MRITAGread(): remaining taglen = {mgztaglen}");
                }
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_numbers_parse_as_strtol() {
        assert_eq!(strtol_whole(b"12"), (12, true));
        assert_eq!(strtol_whole(b""), (0, true));
        assert_eq!(strtol_whole(b"1x"), (1, false));
        assert_eq!(strtol_whole(b"x"), (0, false));
        assert_eq!(strtol_whole(b" -3"), (-3, true));
        assert_eq!(strtol_whole(b"-"), (0, false));
    }
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface library's file names (queue item 3.1-48, relay 15): `MRISfileNameType` (a surface
//! file's type from its name), and the names the curvature and annotation readers and writers
//! choose for a short name (`MRISwriteCurvature_getfilename`, `MRISreadCurvatureFile_getfilename`,
//! `MRISwriteAnnotation_getfilename`, `MRISreadAnnotation_getfilename`;
//! `utils/mrisurf_io.cpp` 2551-2616 and 6690-7000).

use std::path::Path;

use larmorx_core::defects::{FS_126, FS_148, FS_355, FS_356};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::utils::{file_name_only, file_name_path, str_upper, stricmp};
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_io::identify::{
    ASCII_FILE, GIFTI_FILE, MGH_ANNOT, MRI_CURV_FILE, MRI_MGH_FILE, MRI_VOLUME_TYPE_UNKNOWN,
    NII_FILE, VTK_FILE, id_extension_from_name, mri_identify,
};

use crate::coords::Io;
use crate::curvio::{apply_fsgii_read, apply_fsgii_write};
use crate::mris::Mris;
use crate::sfile::{LEFT_HEMISPHERE, STRLEN, c_str, snprintf_name, strcpy_checked};

/// The surface file types (`mrisurf.h` 1539-1551).
pub const MRIS_BINARY_QUADRANGLE_FILE: i32 = 0;
pub const MRIS_ASCII_TRIANGLE_FILE: i32 = 1;
pub const MRIS_ASCII_FILE: i32 = MRIS_ASCII_TRIANGLE_FILE;
pub const MRIS_GEO_TRIANGLE_FILE: i32 = 2;
pub const MRIS_ICO_FILE: i32 = 4;
pub const MRIS_VTK_FILE: i32 = 5;
pub const MRIS_STL_FILE: i32 = 6;
pub const MRIS_GIFTI_FILE: i32 = 7;
pub const MRIS_ANNOT_FILE: i32 = 8;
pub const MRIS_VOLUME_FILE: i32 = 9;
pub const MRIS_LABEL_FILE: i32 = 10;

/// `RIGHT_HEMISPHERE`, `BOTH_HEMISPHERES` (`mrisurf.h`).
const RIGHT_HEMISPHERE: i32 = 1;
const BOTH_HEMISPHERES: i32 = 3;

/// FS-356: `FileNameOnly` or `FileNamePath` copies into one of these functions' 1,024-byte
/// buffers without a check: a part of 1,024 bytes or more writes past it in FreeSurfer (what
/// follows depends on the build's stack); larmorx takes it whole in both modes.
fn unchecked(g: &mut Globals, part: &str, func: &str) {
    if part.len() >= STRLEN {
        g.notices.add(
            &FS_356,
            format!(
                "{func}: a name part of {} bytes, 1,024 or more, is taken whole (FreeSurfer writes past its buffer)",
                part.len()
            ),
        );
    }
}

/// `FileNameOnly(name, buf)` into a `char[STRLEN]` (FS-356).
fn name_only(g: &mut Globals, name: &str, func: &str) -> String {
    let last = match name.rfind('/') {
        Some(i) => &name[i + 1..],
        None => name,
    };
    unchecked(g, last, func);
    file_name_only(name)
}

/// `FileNamePath(name, buf)` into a `char[STRLEN]` (FS-356).
fn name_path(g: &mut Globals, name: &str, func: &str) -> String {
    unchecked(g, name, func);
    file_name_path(name)
}

/// `MRISfileNameType(fname)`: `MRIS_VOLUME_FILE` for a name `IDextensionFromName` takes as a volume
/// (`nii`, `mgz`, `mgh`, `img`, `bhdr`, `nii.gz`, `_000.bfloat`, `_000.bshort`); else, from the
/// name's last part (after its last `/`, cut at its last `#` and then at its last `@`), the text
/// after its last `@` (one can remain only when the part held two) or, without one, after its last
/// `.`, in upper case: `ASC` an ASCII triangle file, `GEO`, `TRI` or `ICO`, `VTK`, `STL`, `GII`,
/// `MGH` or `MGZ` (`MRI_MGH_FILE`, 3), `ANNOT`, `LABEL`; anything else (no extension included) a
/// binary quadrangle file. FS-356 for a last part of 1,024 bytes or more.
pub fn mris_file_name_type(g: &mut Globals, fname: &str) -> i32 {
    let fname = c_str(fname);
    if id_extension_from_name(g, fname).is_some() {
        return MRIS_VOLUME_FILE;
    }
    let s = name_only(g, fname, "MRISfileNameType");
    let ext = match s.rfind('@') {
        Some(i) => &s[i + 1..],
        None => match s.rfind('.') {
            Some(i) => &s[i + 1..],
            None => "",
        },
    };
    match str_upper(ext).as_str() {
        "ASC" => MRIS_ASCII_TRIANGLE_FILE,
        "GEO" => MRIS_GEO_TRIANGLE_FILE,
        "TRI" | "ICO" => MRIS_ICO_FILE,
        "VTK" => MRIS_VTK_FILE,
        "STL" => MRIS_STL_FILE,
        "GII" => MRIS_GIFTI_FILE,
        "MGH" | "MGZ" => MRI_MGH_FILE,
        "ANNOT" => MRIS_ANNOT_FILE,
        "LABEL" => MRIS_LABEL_FILE,
        _ => MRIS_BINARY_QUADRANGLE_FILE,
    }
}

/// `mri_identify(fname)` and `MRISfileNameType(fname)`, with an ASCII surface name taken as
/// `ASCII_FILE`.
fn identify(g: &mut Globals, io: &mut Io, dir: Option<&Path>, fname: &str) -> Result<i32, Fatal> {
    let mut mritype = mri_identify(g, dir, io.env, io.out, io.err, fname)?;
    if mris_file_name_type(g, fname) == MRIS_ASCII_FILE {
        mritype = ASCII_FILE;
    }
    Ok(mritype)
}

/// Whether the two bytes before a name's first dot are `lh` or `rh`, as
/// `strncmp(cp - 2, "lh", 2) == 0 || strncmp(cp - 2, "rh", 2) == 0` reads them: FS-148 for a first
/// dot at the name's first or second byte (FreeSurfer compares bytes before the name; larmorx
/// takes them as no hemisphere letters in both modes).
fn hemi_before_dot(g: &mut Globals, sname: &str, func: &str) -> bool {
    let b = sname.as_bytes();
    let Some(dot) = b.iter().position(|&c| c == b'.') else {
        return false;
    };
    if dot < 2 {
        g.notices.add(
            &FS_148,
            format!(
                "{func}: {sname}: the bytes before the name are taken as no hemisphere letters"
            ),
        );
        return false;
    }
    b[dot - 1] == b'h' && (b[dot - 2] == b'l' || b[dot - 2] == b'r')
}

/// `MRISwriteCurvature_getfilename(mris, sname, curv_to_write)`: the file a curvature writer
/// writes for `sname`, and its type. A name without a `/` goes into the folder of the surface's
/// name (`./` with `FS_POSIX` set), after the surface's hemisphere (`lh.`, `rh.`, `both.` or
/// `unknown.`) unless its first dot is its third byte after `lh` or `rh`; a name with a `/` keeps
/// its folder, its last part (cut at its last `#` and `@`) after the hemisphere on the same test.
/// Each built name is cut at 1,023 bytes with `<function>: Truncation on line <n>` on standard
/// error (FS-126: larmorx keeps it whole). The type: `mri_identify`'s, `ASCII_FILE` for an ASCII
/// surface name, `MRI_VOLUME_TYPE_UNKNOWN` unless MGH, NIfTI, GIFTI, ASCII or VTK; then, unknown,
/// `__MRISapplyFSGIIwrite` may make it a GIFTI name (`.gii` added).
pub fn write_curvature_getfilename(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<(i32, String), Fatal> {
    const F: &str = "MRISwriteCurvature_getfilename";
    let sname = c_str(sname);
    let hemi = match mris.hemisphere {
        LEFT_HEMISPHERE => "lh",
        BOTH_HEMISPHERES => "both",
        RIGHT_HEMISPHERE => "rh",
        _ => "unknown",
    };
    let hemi_named = |name: &str| {
        let b = name.as_bytes();
        b.iter().position(|&c| c == b'.') == Some(2)
            && b[1] == b'h'
            && (b[0] == b'l' || b[0] == b'r')
    };
    let posix = io.env.get("FS_POSIX").is_some();
    let fname = if !sname.contains('/') {
        let path = name_path(g, c_str(&mris.fname), F);
        match (hemi_named(sname), posix) {
            (false, true) => snprintf_name(g, io, format!("./{hemi}.{sname}"), F, 6720),
            (false, false) => snprintf_name(g, io, format!("{path}/{hemi}.{sname}"), F, 6726),
            (true, true) => snprintf_name(g, io, format!("./{sname}"), F, 6735),
            (true, false) => snprintf_name(g, io, format!("{path}/{sname}"), F, 6741),
        }
    } else {
        let path = name_path(g, sname, F);
        let name = name_only(g, sname, F);
        if hemi_named(&name) {
            snprintf_name(g, io, format!("{path}/{name}"), F, 6759)
        } else {
            snprintf_name(g, io, format!("{path}/{hemi}.{name}"), F, 6753)
        }
    };
    let mut mritype = identify(g, io, dir, &fname)?;
    if ![MRI_MGH_FILE, NII_FILE, GIFTI_FILE, ASCII_FILE, VTK_FILE].contains(&mritype) {
        mritype = MRI_VOLUME_TYPE_UNKNOWN;
    }
    let mut curv_to_write = fname.clone();
    if mritype == MRI_VOLUME_TYPE_UNKNOWN {
        apply_fsgii_write(g, io, dir, &mut curv_to_write, &fname, &mut mritype)?;
    }
    Ok((mritype, curv_to_write))
}

/// `MRISreadCurvatureFile_getfilename(mris, sname, curv_to_read)`: the file a curvature reader
/// reads for `sname`, and its type. A name without a `/`: `./<name>` with `FS_POSIX` set; else in
/// the folder of the surface's name, after `lh.` (a left surface) or `rh.` (any other) unless the
/// two bytes before its first dot are `lh` or `rh` (FS-148 for a first dot at its first or second
/// byte); cut at 1,023 bytes as the writer's (FS-126). A name with a `/` as it is (1,024 bytes or
/// more: FreeSurfer's buffer-overflow abort, FS-126). The type: `mri_identify`'s, `ASCII_FILE` for
/// an ASCII surface name, `MRI_VOLUME_TYPE_UNKNOWN` unless a new-format curvature, MGH, NIfTI,
/// GIFTI, ASCII or VTK; a new-format curvature or unknown, `__MRISapplyFSGIIread` may make it a
/// GIFTI name.
pub fn read_curvature_file_getfilename(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<(i32, String), Fatal> {
    const F: &str = "MRISreadCurvatureFile_getfilename";
    let sname = c_str(sname);
    let fname = if !sname.contains('/') {
        if io.env.get("FS_POSIX").is_some() {
            snprintf_name(g, io, format!("./{sname}"), F, 6795)
        } else {
            let hemi_named = hemi_before_dot(g, sname, F);
            let path = name_path(g, c_str(&mris.fname), F);
            if hemi_named {
                snprintf_name(g, io, format!("{path}/{sname}"), F, 6804)
            } else {
                let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
                    "lh"
                } else {
                    "rh"
                };
                snprintf_name(g, io, format!("{path}/{hemi}.{sname}"), F, 6812)
            }
        }
    } else {
        strcpy_checked(g, io, sname, F)?
    };
    let mut mritype = identify(g, io, dir, &fname)?;
    if ![
        MRI_CURV_FILE,
        MRI_MGH_FILE,
        NII_FILE,
        GIFTI_FILE,
        ASCII_FILE,
        VTK_FILE,
    ]
    .contains(&mritype)
    {
        mritype = MRI_VOLUME_TYPE_UNKNOWN;
    }
    let mut curv_to_read = fname.clone();
    if mritype == MRI_CURV_FILE || mritype == MRI_VOLUME_TYPE_UNKNOWN {
        apply_fsgii_read(g, io, dir, &mut curv_to_read, &fname, &mut mritype)?;
    }
    Ok((mritype, curv_to_read))
}

/// `strcat(fname, ".annot")` into the `char[STRLEN]` the official build checks: FS-126 for a
/// result of 1,024 bytes or more (FreeSurfer's buffer-overflow abort, exit status 134).
fn add_annot(g: &mut Globals, io: &mut Io, fname: &str, func: &str) -> Result<String, Fatal> {
    let name = format!("{fname}.annot");
    if name.len() >= STRLEN {
        g.notices.add(
            &FS_126,
            format!(
                "{func}: a name of {} bytes with .annot is taken whole (FreeSurfer aborts)",
                name.len()
            ),
        );
        if g.notices.reproduce() {
            let _ = io
                .err
                .write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    Ok(name)
}

/// The annotation types the name builders keep (others become `MGH_ANNOT` with `.annot` added).
fn annot_kept(t: i32) -> bool {
    [MRI_MGH_FILE, NII_FILE, GIFTI_FILE, MGH_ANNOT].contains(&t)
}

/// `MRISwriteAnnotation_getfilename(mris, sname, annot_to_write)`: the file an annotation writer
/// writes for `sname`, and its type. A name without a `/`: its last part (cut at its last `#` and
/// `@`), in `<folder of the surface's name>/../label/` (`./` with `FS_POSIX` set), after `lh.` (a
/// left surface) or `rh.` (any other) unless it starts with that hemisphere's two letters; cut at
/// 1,023 bytes (FS-126). A name with a `/` as it is (FS-126 at 1,024 bytes). Unless `mri_identify`
/// finds MGH, NIfTI, GIFTI or an annotation, `.annot` is added and the type is `MGH_ANNOT`; an
/// annotation's name goes through `__MRISapplyFSGIIwrite`.
pub fn write_annotation_getfilename(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<(i32, String), Fatal> {
    const F: &str = "MRISwriteAnnotation_getfilename";
    let sname = c_str(sname);
    let mut fname = if !sname.contains('/') {
        let no_path = name_only(g, sname, F);
        let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
            "lh"
        } else {
            "rh"
        };
        let need_hemi = !no_path.as_bytes().starts_with(hemi.as_bytes());
        let path = name_path(g, c_str(&mris.fname), F);
        let posix = io.env.get("FS_POSIX").is_some();
        match (need_hemi, posix) {
            (false, true) => snprintf_name(g, io, format!("./{no_path}"), F, 6865),
            (false, false) => snprintf_name(g, io, format!("{path}/../label/{no_path}"), F, 6872),
            (true, true) => snprintf_name(g, io, format!("./{hemi}.{no_path}"), F, 6882),
            (true, false) => {
                snprintf_name(g, io, format!("{path}/../label/{hemi}.{no_path}"), F, 6890)
            }
        }
    } else {
        strcpy_checked(g, io, sname, F)?
    };
    let mut mritype = mri_identify(g, dir, io.env, io.out, io.err, &fname)?;
    if !annot_kept(mritype) {
        fname = add_annot(g, io, &fname, F)?;
        mritype = MGH_ANNOT;
    }
    let mut annot_to_write = fname.clone();
    if mritype == MGH_ANNOT {
        apply_fsgii_write(g, io, dir, &mut annot_to_write, &fname, &mut mritype)?;
    }
    Ok((mritype, annot_to_write))
}

/// `MRISreadAnnotation_getfilename(mris, sname, annot_to_read)`: the file an annotation reader
/// reads for `sname`, and its type. A name without a `/` and a surface: its last part (cut at its
/// last `#` and `@`) in `<folder of the surface's name>/../label/`, after `lh.` (a left surface) or
/// `rh.` (any other) unless it names that hemisphere; cut at 1,023 bytes (FS-126). FS-355: FreeSurfer
/// takes the name as naming the hemisphere only when it is the two letters alone (in any case), so
/// `lh.aparc.annot` becomes `lh.lh.aparc.annot`; larmorx takes a name starting with the two letters,
/// as the annotation writer does (`reproduce`: FreeSurfer's test). A name with a `/`, or no surface:
/// as it is (FS-126 at 1,024 bytes). Then, when that file is not readable, `<name>.mgz` if
/// readable (FS-126 at 1,024 bytes), else `sname` itself if readable (cut at 1,023 bytes). Unless `mri_identify` finds MGH, NIfTI, GIFTI or an
/// annotation, `.annot` is added and the type is `MGH_ANNOT`; an annotation's name goes through
/// `__MRISapplyFSGIIread`.
pub fn read_annotation_getfilename(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: Option<&Mris>,
    sname: &str,
) -> Result<(i32, String), Fatal> {
    const F: &str = "MRISreadAnnotation_getfilename";
    let sname = c_str(sname);
    let mut fname = match mris {
        Some(mris) if !sname.contains('/') => {
            let no_path = name_only(g, sname, F);
            let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
                "lh"
            } else {
                "rh"
            };
            // FreeSurfer's test (stricmp, with its own notices) only where it decides
            let original = if g.notices.reproduce() {
                stricmp(g, &no_path, hemi) != 0
            } else {
                !no_path.eq_ignore_ascii_case(hemi)
            };
            let starts = no_path.as_bytes().starts_with(hemi.as_bytes());
            if original == starts {
                g.notices.add(
                    &FS_355,
                    format!(
                        "{F}: {no_path}: {} (FreeSurfer {})",
                        if starts {
                            "starts with the hemisphere's letters: no hemisphere added"
                        } else {
                            "does not start with the hemisphere's letters: the hemisphere added"
                        },
                        if original { "adds it" } else { "adds none" }
                    ),
                );
            }
            let need_hemi = if g.notices.reproduce() {
                original
            } else {
                !starts
            };
            let path = name_path(g, c_str(&mris.fname), F);
            if need_hemi {
                snprintf_name(g, io, format!("{path}/../label/{hemi}.{no_path}"), F, 6948)
            } else {
                snprintf_name(g, io, format!("{path}/../label/{no_path}"), F, 6940)
            }
        }
        _ => strcpy_checked(g, io, sname, F)?,
    };
    if fio_file_exists_readable(g, dir, &fname) == 0 {
        let mgz = format!("{fname}.mgz");
        if fio_file_exists_readable(g, dir, &mgz) != 0 {
            fname = strcpy_checked(g, io, &mgz, F)?;
        } else if fio_file_exists_readable(g, dir, sname) != 0 {
            fname = snprintf_name(g, io, sname.to_owned(), F, 6969);
        }
    }
    let mut mritype = mri_identify(g, dir, io.env, io.out, io.err, &fname)?;
    if !annot_kept(mritype) {
        fname = add_annot(g, io, &fname, F)?;
        mritype = MGH_ANNOT;
    }
    let mut annot_to_read = fname.clone();
    if mritype == MGH_ANNOT {
        apply_fsgii_read(g, io, dir, &mut annot_to_read, &fname, &mut mritype)?;
    }
    Ok((mritype, annot_to_read))
}

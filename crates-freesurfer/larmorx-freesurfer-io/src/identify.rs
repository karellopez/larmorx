// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The volume format probe of FreeSurfer 8.2.0 (`utils/mri_identify.cpp`, queue item 3.1-27 with
//! its lines 697-1195 taken early from item 3.1-28, and `MRIgetVolumeName` from `utils/mriio.cpp`):
//! a format's number from a name's extension (`string_to_type`, `type_to_string`), `mri_identify`
//! (the extension first, checked against the file, then every format's test in FreeSurfer's
//! order), the stems and extensions of volume names (`IDtypeFromStem`, `IDstemFromName`,
//! `IDextensionFromName`, `IDnameFromStem`), each format's test (`is_cor` ... `is_nrrd`,
//! `IDisCurv`) and the `.bhdr` helpers. The DICOM tests are in [`crate::dicom_probe`], Bruker's in
//! [`crate::bruker`], GE Signa's in [`crate::signa`].
//!
//! A name's "extension" is, as in FreeSurfer, what follows its last `.` anywhere in it (a dot in
//! a directory's name counts). `errno` is kept as FreeSurfer's calls leave it: a failed `stat`
//! sets it, most failed opens set it back to 0, `IDisCurv`'s and `is_signa`'s do not.
//!
//! Defects: FS-221 (names that overflow FreeSurfer's fixed buffers: used whole), FS-222
//! (`is_mnc`'s look before a name), FS-223 (`is_mgh` compares the whole name), FS-226
//! (`IDextensionFromName` without the dot).

use std::fs::Metadata;
use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_221, FS_222, FS_223, FS_226};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_SHORT};

use crate::bruker;
use crate::cfile::CFile;
use crate::dicom_probe;
use crate::fio;
use crate::signa;

pub const MRI_GZIPPED: i32 = -2;
pub const MRI_VOLUME_TYPE_UNKNOWN: i32 = -1;
pub const MRI_CORONAL_SLICE_DIRECTORY: i32 = 0;
pub const MRI_MINC_FILE: i32 = 1;
pub const MRI_ANALYZE_FILE: i32 = 2;
pub const MRI_MGH_FILE: i32 = 3;
pub const GENESIS_FILE: i32 = 4;
pub const GE_LX_FILE: i32 = 5;
pub const SIEMENS_FILE: i32 = 6;
pub const BRIK_FILE: i32 = 7;
pub const BSHORT_FILE: i32 = 8;
pub const BFLOAT_FILE: i32 = 9;
pub const SDT_FILE: i32 = 10;
pub const OTL_FILE: i32 = 11;
pub const GDF_FILE: i32 = 12;
pub const RAW_FILE: i32 = 13;
pub const SIGNA_FILE: i32 = 14;
pub const DICOM_FILE: i32 = 15;
pub const MRI_ANALYZE4D_FILE: i32 = 16;
pub const SIEMENS_DICOM_FILE: i32 = 17;
pub const BRUKER_FILE: i32 = 18;
pub const XIMG_FILE: i32 = 19;
pub const NIFTI1_FILE: i32 = 20;
pub const IMAGE_FILE: i32 = 21;
pub const MRI_GCA_FILE: i32 = 22;
pub const BHDR: i32 = 23;
pub const NII_FILE: i32 = 24;
pub const MRI_CURV_FILE: i32 = 25;
pub const NRRD_FILE: i32 = 26;
pub const GIFTI_FILE: i32 = 27;
pub const VTK_FILE: i32 = 28;
pub const MGH_MORPH: i32 = 29;
pub const MGH_AUTOENCODER: i32 = 30;
pub const ITK_MORPH: i32 = 31;
pub const MGH_LABEL_FILE: i32 = 32;
pub const MGH_ANNOT: i32 = 33;
pub const NPY_FILE: i32 = 34;
pub const ASCII_FILE: i32 = 35;

/// GE's magic number (`IMGF`, big endian).
pub const GE_MAGIC: i32 = 0x494d_4746;
/// NIfTI-1's magic numbers of a header pair and of a single file, and NRRD's.
pub const NIFTI1_MAGIC: &[u8; 4] = b"ni1\0";
pub const NII_MAGIC: &[u8; 4] = b"n+1\0";
pub const NRRD_MAGIC: &[u8; 4] = b"NRRD";
/// The curvature files' new-version number (the first 3 bytes, big endian).
pub const NEW_VERSION_MAGIC_NUMBER: i32 = 16_777_215;

/// FreeSurfer's `STRLEN` buffers and the 2,000-byte ones of the stem helpers.
const STRLEN: usize = 1024;
const TMPSTR: usize = 2000;

/// FS-221: a name copied into a fixed buffer it does not fit (FreeSurfer overflows it; used whole).
fn fits(g: &mut Globals, what: &str, len: usize, room: usize) {
    if len >= room {
        g.notices.add(
            &FS_221,
            format!("{what}: a name of {len} bytes used whole (FreeSurfer's buffer holds {room})"),
        );
    }
}

/// `stat(name)`: the file's metadata (links followed), or `errno` (`ENOENT` for an empty name).
pub(crate) fn c_stat(dir: Option<&Path>, name: &str) -> Result<Metadata, i32> {
    if name.is_empty() {
        return Err(2);
    }
    std::fs::metadata(utils::resolve(dir, name)).map_err(|e| log::errno_of(&e).unwrap_or(5))
}

/// What follows the name's last `.`, or `None` without one.
fn after_dot(name: &str) -> Option<&str> {
    name.rfind('.').map(|i| &name[i + 1..])
}

/// `fopen(name, "r")` as these tests use it: the stream, or `None` with `errno` set back to 0.
fn open_or_zero(g: &mut Globals, dir: Option<&Path>, name: &str) -> Option<CFile> {
    match CFile::open(dir, name, "r") {
        Ok(f) => Some(f),
        Err(_) => {
            g.errno = 0;
            None
        }
    }
}

/// `fread(buf, n, 1, fp) < 1` (or `fread(buf, 1, n, fp) != n`): the `n` bytes, or `None` with
/// `errno` 0 (the stream closed).
fn read_or_zero(g: &mut Globals, fp: &mut CFile, n: usize) -> Option<Vec<u8>> {
    let mut b = vec![0u8; n];
    if fp.read_bytes(&mut b) != n {
        g.errno = 0;
        return None;
    }
    Some(b)
}

/// `type_to_string(type)`: the format's name (`unknown` for the others).
pub fn type_to_string(t: i32) -> &'static str {
    match t {
        MRI_CORONAL_SLICE_DIRECTORY => "COR",
        MRI_GCA_FILE => "GCA",
        MRI_MINC_FILE => "MINC",
        MRI_ANALYZE_FILE => "analyze3d",
        MRI_ANALYZE4D_FILE => "analyze4d",
        MRI_MGH_FILE => "MGH",
        GENESIS_FILE => "genesis",
        GE_LX_FILE => "gelx",
        SIEMENS_FILE => "siemens",
        DICOM_FILE => "dicom",
        SIEMENS_DICOM_FILE => "siemens_dicom",
        BRIK_FILE => "brik",
        BSHORT_FILE => "bshort",
        BFLOAT_FILE => "bfloat",
        BHDR => "bhdr",
        SDT_FILE => "varian",
        OTL_FILE => "outline",
        GDF_FILE => "gdf",
        BRUKER_FILE => "bruker",
        XIMG_FILE => "ximg",
        NIFTI1_FILE => "nifti1",
        NII_FILE => "nii",
        NRRD_FILE => "nrrd",
        MRI_CURV_FILE => "curv",
        GIFTI_FILE => "gii",
        MGH_ANNOT => "annot",
        NPY_FILE => "npy",
        _ => "unknown",
    }
}

/// `string_to_type(string)`: the format a (lower-cased) extension names, or
/// `MRI_VOLUME_TYPE_UNKNOWN`; `gz` gives `MRI_GZIPPED`.
pub fn string_to_type(g: &mut Globals, s: &str) -> i32 {
    if s.is_empty() {
        return MRI_VOLUME_TYPE_UNKNOWN;
    }
    fits(g, "string_to_type", s.len(), STRLEN);
    let ls = utils::str_lower(s);
    match ls.as_str() {
        "gz" => MRI_GZIPPED,
        "ae" | "sae" | "csae" => MGH_AUTOENCODER,
        "m3z" | "m3d" => MGH_MORPH,
        "cor" => MRI_CORONAL_SLICE_DIRECTORY,
        "minc" | "mnc" => MRI_MINC_FILE,
        "spm" | "analyze" | "analyze3d" => MRI_ANALYZE_FILE,
        "analyze4d" | "img" => MRI_ANALYZE4D_FILE,
        "mgh" | "mgz" => MRI_MGH_FILE,
        "gca" => MRI_GCA_FILE,
        "signa" => SIGNA_FILE,
        "ge" | "genesis" => GENESIS_FILE,
        "gelx" | "lx" => GE_LX_FILE,
        "bshort" => BSHORT_FILE,
        "bfloat" => BFLOAT_FILE,
        "bhdr" => BHDR,
        "siemens" | "ima" => SIEMENS_FILE,
        "dicom" => DICOM_FILE,
        "siemens_dicom" => SIEMENS_DICOM_FILE,
        "brik" | "afni" => BRIK_FILE,
        "sdt" | "varian" => SDT_FILE,
        "otl" | "outline" => OTL_FILE,
        "gdf" => GDF_FILE,
        "bruker" => BRUKER_FILE,
        "ximg" => XIMG_FILE,
        "nifti1" => NIFTI1_FILE,
        "nii" => NII_FILE,
        "nrrd" => NRRD_FILE,
        "mat" | "svs" | "tif" | "tiff" | "jpg" | "jpeg" | "pgm" | "ppm" | "pbm" | "rgb" => {
            IMAGE_FILE
        }
        "curv" => MRI_CURV_FILE,
        "gii" => GIFTI_FILE,
        "vtk" => VTK_FILE,
        "label" => MGH_LABEL_FILE,
        "annot" => MGH_ANNOT,
        "npy" => NPY_FILE,
        _ => MRI_VOLUME_TYPE_UNKNOWN,
    }
}

/// `MRIgetVolumeName(string, name_only)`: the name without what follows its last `@` (a forced
/// format) and, while `MRIIO_Strip_Pound` is set (it is, unless a program clears it), its last `#`
/// (a frame list).
pub fn mri_get_volume_name(g: &mut Globals, s: &str) -> String {
    fits(g, "MRIgetVolumeName", s.len(), STRLEN);
    let mut name = s.to_owned();
    if let Some(i) = name.rfind('@') {
        name.truncate(i);
    }
    if g.mriio_strip_pound != 0
        && let Some(i) = name.rfind('#')
    {
        name.truncate(i);
    }
    name
}

/// `mri_identify(fname)`: the format of the volume `fname` names. With `@` and `#` parts removed
/// ([`mri_get_volume_name`]), a name that is not a directory is first typed by its extension
/// ([`string_to_type`]): `.gz` is MGH for a name holding `.mgh.gz`, NIfTI for one holding
/// `.nii.gz`, else unknown after `INFO: Currently supports gzipped mgh or nifti file (.mgz,
/// .mgh.gz, or nii.gz) only.` and `fname = <name>` (standard output); image files, `.npy` and MGH
/// are taken as named; GCA, `.bhdr`, ANALYZE-4D, GIFTI, annotations, VTK and labels too; the others
/// are checked by their test (an `.img` or `.hdr` is NIfTI-1 when its header says so). Otherwise
/// the tests run in FreeSurfer's order: Bruker, COR, bshort, bfloat, Siemens DICOM, DICOM, Genesis,
/// NRRD, curvature, Signa, GE LX, Varian, MINC, NIfTI-1, NIfTI, ANALYZE, Siemens, BRIK, outline,
/// GDF, XIMG; then a morph or autoencoder extension; else `MRI_VOLUME_TYPE_UNKNOWN`. A directory
/// that is not Bruker's is a COR directory. `Err`: the DICOM probe ended the program.
pub fn mri_identify(
    g: &mut Globals,
    dir: Option<&Path>,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fname_passed: &str,
) -> Result<i32, Fatal> {
    let fname = mri_get_volume_name(g, fname_passed);
    let fname = fname.as_str();
    let mut ty = 0;
    if fio::fio_is_directory(g, dir, fname) == 0
        && let Some(ext) = after_dot(fname)
    {
        ty = string_to_type(g, ext);
        if ty == MRI_GZIPPED {
            if fname.contains(".mgh.gz") {
                ty = MRI_MGH_FILE;
            } else if fname.contains(".nii.gz") {
                ty = NII_FILE;
            } else {
                let _ = out.write_all(
                    b"INFO: Currently supports gzipped mgh or nifti file (.mgz, .mgh.gz, or nii.gz) only.\n",
                );
                let _ = writeln!(out, "fname = {fname}");
                return Ok(MRI_VOLUME_TYPE_UNKNOWN);
            }
        }
        if ty == IMAGE_FILE || ty == NPY_FILE || ty == MRI_MGH_FILE {
            return Ok(ty);
        }
        let confirmed = match ty {
            MRI_GCA_FILE | BHDR | MRI_ANALYZE4D_FILE | GIFTI_FILE | MGH_ANNOT | VTK_FILE
            | MGH_LABEL_FILE => true,
            BRUKER_FILE => bruker::is_bruker(g, dir, fname) != 0,
            MRI_CORONAL_SLICE_DIRECTORY => is_cor(g, dir, fname) != 0,
            BSHORT_FILE => is_bshort(fname) != 0,
            BFLOAT_FILE => is_bfloat(fname) != 0,
            SIEMENS_DICOM_FILE => dicom_probe::is_siemens_dicom(g, dir, env, out, err, fname)? != 0,
            DICOM_FILE => dicom_probe::is_dicom(g, dir, env, out, err, fname) != 0,
            GENESIS_FILE => is_genesis(g, dir, fname) != 0,
            SIGNA_FILE => signa::is_signa(g, dir, fname) != 0,
            GE_LX_FILE => is_ge_lx(g, dir, fname) != 0,
            SDT_FILE => is_sdt(g, dir, fname) != 0,
            MRI_MINC_FILE => is_mnc(g, dir, fname) != 0,
            NII_FILE => is_nii(g, dir, fname) != 0,
            NIFTI1_FILE => is_nifti1(g, dir, fname) != 0,
            MRI_ANALYZE_FILE => {
                if is_nifti1(g, dir, fname) != 0 {
                    return Ok(NIFTI1_FILE);
                }
                is_analyze(g, dir, fname) != 0
            }
            SIEMENS_FILE => is_siemens(g, dir, fname) != 0,
            BRIK_FILE => is_brik(g, fname) != 0,
            OTL_FILE => is_otl(fname) != 0,
            GDF_FILE => is_gdf(fname) != 0,
            XIMG_FILE => is_ximg(fname) != 0,
            NRRD_FILE => is_nrrd(g, dir, fname) != 0,
            _ => false,
        };
        if confirmed {
            return Ok(ty);
        }
    }
    let r = if bruker::is_bruker(g, dir, fname) != 0 {
        BRUKER_FILE
    } else if is_cor(g, dir, fname) != 0 {
        MRI_CORONAL_SLICE_DIRECTORY
    } else if is_bshort(fname) != 0 {
        BSHORT_FILE
    } else if is_bfloat(fname) != 0 {
        BFLOAT_FILE
    } else if dicom_probe::is_siemens_dicom(g, dir, env, out, err, fname)? != 0 {
        SIEMENS_DICOM_FILE
    } else if dicom_probe::is_dicom(g, dir, env, out, err, fname) != 0 {
        DICOM_FILE
    } else if is_genesis(g, dir, fname) != 0 {
        GENESIS_FILE
    } else if is_nrrd(g, dir, fname) != 0 {
        NRRD_FILE
    } else if id_is_curv(g, dir, fname) != 0 {
        MRI_CURV_FILE
    } else if signa::is_signa(g, dir, fname) != 0 {
        SIGNA_FILE
    } else if is_ge_lx(g, dir, fname) != 0 {
        GE_LX_FILE
    } else if is_sdt(g, dir, fname) != 0 {
        SDT_FILE
    } else if is_mnc(g, dir, fname) != 0 {
        MRI_MINC_FILE
    } else if is_nifti1(g, dir, fname) != 0 {
        NIFTI1_FILE
    } else if is_nii(g, dir, fname) != 0 {
        NII_FILE
    } else if is_analyze(g, dir, fname) != 0 {
        MRI_ANALYZE_FILE
    } else if is_siemens(g, dir, fname) != 0 {
        SIEMENS_FILE
    } else if is_brik(g, fname) != 0 {
        BRIK_FILE
    } else if is_otl(fname) != 0 {
        OTL_FILE
    } else if is_gdf(fname) != 0 {
        GDF_FILE
    } else if is_ximg(fname) != 0 {
        XIMG_FILE
    } else if ty == MGH_MORPH || ty == MGH_AUTOENCODER {
        ty
    } else {
        MRI_VOLUME_TYPE_UNKNOWN
    };
    Ok(r)
}

/// The stems' file names: `<stem><suffix>`, in a 2,000-byte buffer (FS-221).
fn stem_name(g: &mut Globals, what: &str, stem: &str, suffix: &str) -> String {
    let n = format!("{stem}{suffix}");
    fits(g, what, n.len(), TMPSTR);
    n
}

/// `IDtypeFromStem(stem)`: the format of the first file found readable of `<stem>.nii`,
/// `.nii.gz` (NIfTI), `.mgh`, `.mgz` (MGH), `_000.bfloat`, `_000.bshort`; else unknown.
pub fn id_type_from_stem(g: &mut Globals, dir: Option<&Path>, stem: &str) -> i32 {
    for (suffix, t) in [
        (".nii", NII_FILE),
        (".nii.gz", NII_FILE),
        (".mgh", MRI_MGH_FILE),
        (".mgz", MRI_MGH_FILE),
        ("_000.bfloat", BFLOAT_FILE),
        ("_000.bshort", BSHORT_FILE),
    ] {
        let n = stem_name(g, "IDtypeFromStem", stem, suffix);
        if fio::fio_file_exists_readable(g, dir, &n) != 0 {
            return t;
        }
    }
    MRI_VOLUME_TYPE_UNKNOWN
}

/// `IDstemFromName(name)`: the name without its volume extension (`.w`, `.nii`, `.mgz`, `.mgh`,
/// `.img`, `.bhdr`, `.nii.gz`, `_000.bfloat`, `_000.bshort`, tried in that order, each only for a
/// name long enough to hold it and at least one more byte), or `None`.
pub fn id_stem_from_name(name: &str) -> Option<String> {
    let b = name.as_bytes();
    let len = b.len();
    let cut = |n: usize| Some(String::from_utf8_lossy(&b[..len - n]).into_owned());
    if len < 3 {
        return None;
    }
    if &b[len - 2..] == b".w" {
        return cut(2);
    }
    if len < 5 {
        return None;
    }
    if matches!(&b[len - 4..], b".nii" | b".mgz" | b".mgh" | b".img") {
        return cut(4);
    }
    if len < 6 {
        return None;
    }
    if &b[len - 5..] == b".bhdr" {
        return cut(5);
    }
    if len < 8 {
        return None;
    }
    if &b[len - 7..] == b".nii.gz" {
        return cut(7);
    }
    if len < 12 {
        return None;
    }
    if matches!(&b[len - 11..], b"_000.bfloat" | b"_000.bshort") {
        return cut(11);
    }
    None
}

/// `IDextensionFromName(name)`: the volume extension a name ends with (`nii`, `mgz`, `mgh`, `img`,
/// `bhdr`, `nii.gz`, and `bfloat` or `bshort` for `_000.bfloat` and `_000.bshort`), each only for
/// a name long enough to hold it and two more bytes, or `None`. FreeSurfer compares the name's last
/// letters without the dot before them (FS-226, rule A: by default the extension must follow a
/// `.`, as `IDstemFromName` requires; `reproduce` takes `brainnii` for `nii`).
pub fn id_extension_from_name(g: &mut Globals, name: &str) -> Option<String> {
    let b = name.as_bytes();
    let len = b.len();
    if len < 5 {
        return None;
    }
    let ends = |g: &mut Globals, ext: &str| -> bool {
        let e = ext.as_bytes();
        if &b[len - e.len()..] != e {
            return false;
        }
        let dotted = len > e.len() && b[len - e.len() - 1] == b'.';
        if !dotted && !ext.starts_with('_') {
            g.notices.add(
                &FS_226,
                format!(
                    "IDextensionFromName: a name ending in {ext} without a dot {} (FreeSurfer takes it as {ext})",
                    if g.notices.reproduce() {
                        "taken as that volume format"
                    } else {
                        "not taken as a volume format"
                    }
                ),
            );
            return g.notices.reproduce();
        }
        true
    };
    for ext in ["nii", "mgz", "mgh", "img"] {
        if ends(g, ext) {
            return Some(ext.to_owned());
        }
    }
    if len < 6 {
        return None;
    }
    if ends(g, "bhdr") {
        return Some("bhdr".to_owned());
    }
    if len < 8 {
        return None;
    }
    if ends(g, "nii.gz") {
        return Some("nii.gz".to_owned());
    }
    if len < 12 {
        return None;
    }
    if ends(g, "_000.bfloat") {
        return Some("bfloat".to_owned());
    }
    if ends(g, "_000.bshort") {
        return Some("bshort".to_owned());
    }
    None
}

/// `IDnameFromStem(stem)`: the first file found readable of `<stem>.nii`, `.nii.gz`, `.mgh`,
/// `.mgz`, `.img`, `.bhdr`, `_000.bfloat`, `_000.bshort`, `.w`, or `None`.
pub fn id_name_from_stem(g: &mut Globals, dir: Option<&Path>, stem: &str) -> Option<String> {
    for suffix in [
        ".nii",
        ".nii.gz",
        ".mgh",
        ".mgz",
        ".img",
        ".bhdr",
        "_000.bfloat",
        "_000.bshort",
        ".w",
    ] {
        let n = stem_name(g, "IDnameFromStem", stem, suffix);
        if fio::fio_file_exists_readable(g, dir, &n) != 0 {
            return Some(n);
        }
    }
    None
}

/// `is_cor(fname)`: 1 when `fname` exists (`stat`) and is a directory or its last component
/// starts with `COR-`; 0 otherwise (`errno` as `stat` left it).
pub fn is_cor(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    let m = match c_stat(dir, fname) {
        Ok(m) => m,
        Err(e) => {
            g.errno = e;
            return 0;
        }
    };
    i32::from(m.is_dir() || xpg_basename(fname).starts_with("COR-"))
}

/// POSIX's `basename` (`libgen.h`): the last component, trailing slashes ignored; `/` for a name
/// of slashes, `.` for an empty one.
fn xpg_basename(name: &str) -> &str {
    let t = name.trim_end_matches('/');
    if t.is_empty() {
        return if name.is_empty() { "." } else { "/" };
    }
    match t.rfind('/') {
        Some(i) => &t[i + 1..],
        None => t,
    }
}

/// `is_brik(fname)`: 1 for an extension `BRIK` in any case.
pub fn is_brik(g: &mut Globals, fname: &str) -> i32 {
    match after_dot(fname) {
        Some(e) => i32::from(utils::stricmp(g, e, "BRIK") == 0),
        None => 0,
    }
}

/// The C string of a fixed buffer: its bytes up to the first zero.
fn c_text(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `is_siemens(fname)`: 1 for an extension `ima` in any case, or a file whose bytes at 5,790,
/// 5,802 and 5,838 read `SL`, `SP` and `FoV` (as C strings); else 0 (`errno` 0 after a failed
/// open or a short file).
pub fn is_siemens(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if let Some(e) = after_dot(fname)
        && utils::stricmp(g, e, "ima") == 0
    {
        return 1;
    }
    let Some(mut fp) = open_or_zero(g, dir, fname) else {
        return 0;
    };
    let mut errno = 0;
    for (at, want) in [(5790, b"SL".as_slice()), (5802, b"SP"), (5838, b"FoV")] {
        fp.fseek(&mut errno, at, 0);
        let Some(b) = read_or_zero(g, &mut fp, want.len()) else {
            fp.close();
            return 0;
        };
        if c_text(&b) != want {
            fp.close();
            return 0;
        }
    }
    fp.close();
    1
}

/// A file's first 4 bytes at `at` (from the start) as a big-endian `int`, or `None` (`errno` 0).
fn magic_at(g: &mut Globals, dir: Option<&Path>, fname: &str, at: i64) -> Option<i32> {
    let mut fp = open_or_zero(g, dir, fname)?;
    if at != 0 {
        let mut errno = 0;
        fp.fseek(&mut errno, at, 1);
    }
    let b = read_or_zero(g, &mut fp, 4);
    fp.close();
    let b = b?;
    Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// `is_genesis(fname)`: 1 for a name starting with `I.` (the whole name, its directories
/// included), an extension `MR`, or a file whose first 4 bytes are GE's magic number `IMGF`; else
/// 0 (`errno` 0 after a failed open or a short file).
pub fn is_genesis(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if fname.starts_with("I.") {
        return 1;
    }
    if after_dot(fname) == Some("MR") {
        return 1;
    }
    i32::from(magic_at(g, dir, fname, 0) == Some(GE_MAGIC))
}

/// `is_ge_lx(fname)`: 1 for a file whose 4 bytes at 3,228 are GE's magic number; else 0.
pub fn is_ge_lx(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    i32::from(magic_at(g, dir, fname, 3228) == Some(GE_MAGIC))
}

/// `is_analyze(fname)`: 1 for an extension `img` in any case; 0 for any other extension; without
/// one, 1 when `<fname>.hdr` is an ANALYZE header: its first 348 bytes read, its `sizeof_hdr`
/// (big endian) the file's length, its extents 16,384 and its `regular` byte `r`; else 0.
pub fn is_analyze(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    fits(g, "is_analyze", fname.len(), STRLEN);
    if let Some(e) = after_dot(fname) {
        return i32::from(utils::stricmp(g, e, "img") == 0);
    }
    let hfname = format!("{fname}.hdr");
    fits(g, "is_analyze", hfname.len(), STRLEN);
    let Some(mut fp) = open_or_zero(g, dir, &hfname) else {
        return 0;
    };
    let Some(hdr) = read_or_zero(g, &mut fp, 348) else {
        fp.close();
        return 0;
    };
    let mut errno = 0;
    fp.fseek(&mut errno, 0, 2);
    let hdr_length = fp.tell();
    fp.close();
    let sizeof_hdr = i32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    let extents = i32::from_be_bytes([hdr[32], hdr[33], hdr[34], hdr[35]]);
    i32::from(hdr_length == i64::from(sizeof_hdr) && extents == 16384 && hdr[38] == b'r')
}

/// `is_mnc(fname)`: 1 for an extension `mnc` in any case, a name ending in `.mnc.gz` (any case),
/// or a file whose first 3 bytes are `CDF`; else 0 (`errno` 0 after a failed open or a short file).
/// For a `.gz` name FreeSurfer looks for the dot before the last one by stepping back from it,
/// and when that dot is the name's first character it reads the byte before the name (FS-222:
/// never a different answer; both modes compare from the start).
pub fn is_mnc(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if let Some(i) = fname.rfind('.') {
        let e = &fname[i + 1..];
        if utils::stricmp(g, e, "mnc") == 0 {
            return 1;
        }
        if utils::stricmp(g, e, "gz") == 0 {
            let b = fname.as_bytes();
            let mut k = i as isize - 1;
            if k < 0 {
                g.notices.add(
                    &FS_222,
                    "is_mnc: a name whose only dot starts it, compared from its start (FreeSurfer reads the byte before it)",
                );
                k = 0;
            } else {
                while k > 0 && b[k as usize] != b'.' {
                    k -= 1;
                }
            }
            if utils::stricmp(g, &fname[k as usize..], ".mnc.gz") == 0 {
                return 1;
            }
        }
    }
    let Some(mut fp) = open_or_zero(g, dir, fname) else {
        return 0;
    };
    let b = read_or_zero(g, &mut fp, 3);
    fp.close();
    i32::from(b.as_deref() == Some(b"CDF"))
}

/// `is_mgh(fname)`: 1 for a name ending in `.mgh`, `.mgz` or `.mgh.gz` (as FreeSurfer's comment
/// describes it). FreeSurfer compares the whole name with those three (FS-223, rule A; with
/// `reproduce` only the names `.mgh`, `.mgz` and `.mgh.gz` themselves); 0 without a dot. No program
/// calls it.
pub fn is_mgh(g: &mut Globals, fname: &str) -> i32 {
    if !fname.contains('.') {
        return 0;
    }
    let whole = matches!(fname, ".mgh" | ".mgz" | ".mgh.gz");
    let ends = fname.ends_with(".mgh") || fname.ends_with(".mgz") || fname.ends_with(".mgh.gz");
    if whole != ends {
        g.notices.add(
            &FS_223,
            format!(
                "is_mgh: {fname} {} (FreeSurfer compares the whole name)",
                if g.notices.reproduce() {
                    "not MGH"
                } else {
                    "MGH by its extension"
                }
            ),
        );
        if g.notices.reproduce() {
            return i32::from(whole);
        }
    }
    i32::from(ends)
}

/// `is_bshort(fname)`: 1 for the extension `bshort` (exactly).
pub fn is_bshort(fname: &str) -> i32 {
    i32::from(after_dot(fname) == Some("bshort"))
}

/// `is_bfloat(fname)`: 1 for the extension `bfloat`.
pub fn is_bfloat(fname: &str) -> i32 {
    i32::from(after_dot(fname) == Some("bfloat"))
}

/// `is_bhdr(fname)`: 1 for the extension `bhdr`.
pub fn is_bhdr(fname: &str) -> i32 {
    i32::from(after_dot(fname) == Some("bhdr"))
}

/// `bhdr_stem(fname)`: a `.bhdr` name without its last `.` and what follows, or `None`.
pub fn bhdr_stem(fname: &str) -> Option<String> {
    if is_bhdr(fname) == 0 {
        return None;
    }
    fname.rfind('.').map(|i| fname[..i].to_owned())
}

/// `bhdr_firstslicefname(fname)`: `<stem>_000.bfloat` or else `<stem>_000.bshort` when readable,
/// or `None` (also for a name that is not `.bhdr`).
pub fn bhdr_firstslicefname(g: &mut Globals, dir: Option<&Path>, fname: &str) -> Option<String> {
    let stem = bhdr_stem(fname)?;
    for suffix in ["_000.bfloat", "_000.bshort"] {
        let n = format!("{stem}{suffix}");
        if fio::fio_file_exists_readable(g, dir, &n) != 0 {
            return Some(n);
        }
    }
    None
}

/// `bhdr_precisionstring(fname)`: `bfloat` or `bshort`, as `<stem>_000.<p>` is found readable, or
/// `None`.
pub fn bhdr_precisionstring(g: &mut Globals, dir: Option<&Path>, fname: &str) -> Option<String> {
    let stem = bhdr_stem(fname)?;
    for p in ["bfloat", "bshort"] {
        let n = stem_name(g, "bhdr_precisionstring", &stem, &format!("_000.{p}"));
        if fio::fio_file_exists_readable(g, dir, &n) != 0 {
            return Some(p.to_owned());
        }
    }
    None
}

/// `bhdr_precision(fname)`: `MRI_FLOAT` or `MRI_SHORT` as `<stem>_000.bfloat` or `.bshort` is
/// found readable, else `MRI_VOLUME_TYPE_UNKNOWN`; 0 for a name that is not `.bhdr`.
pub fn bhdr_precision(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    let Some(stem) = bhdr_stem(fname) else {
        return 0;
    };
    for (suffix, t) in [("_000.bfloat", MRI_FLOAT), ("_000.bshort", MRI_SHORT)] {
        let n = stem_name(g, "bhdr_precision", &stem, suffix);
        if fio::fio_file_exists_readable(g, dir, &n) != 0 {
            return t;
        }
    }
    MRI_VOLUME_TYPE_UNKNOWN
}

/// `is_sdt(fname)`: 1 when the file opens and so does its header: the name with what follows its
/// last `.` replaced by `spr` (or `.spr` added without a dot); else 0 (`errno` 0).
pub fn is_sdt(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    let Some(fp) = open_or_zero(g, dir, fname) else {
        return 0;
    };
    fp.close();
    fits(g, "is_sdt", fname.len(), STRLEN);
    let header = match fname.rfind('.') {
        Some(i) => format!("{}spr", &fname[..=i]),
        None => format!("{fname}.spr"),
    };
    fits(g, "is_sdt", header.len(), STRLEN);
    let Some(fp) = open_or_zero(g, dir, &header) else {
        return 0;
    };
    fp.close();
    1
}

/// `is_gdf(fname)`: 1 for the extension `gdf`.
pub fn is_gdf(fname: &str) -> i32 {
    i32::from(after_dot(fname) == Some("gdf"))
}

/// `is_otl(fname)`: 1 for the extension `otl`.
pub fn is_otl(fname: &str) -> i32 {
    i32::from(after_dot(fname) == Some("otl"))
}

/// `is_ximg(fname)`: always 0.
pub fn is_ximg(_fname: &str) -> i32 {
    0
}

/// The 4 bytes at 344 of a file (`fseek` then `fread`), or `None` (`errno` 0).
fn bytes_at_344(g: &mut Globals, dir: Option<&Path>, name: &str) -> Option<[u8; 4]> {
    let mut fp = open_or_zero(g, dir, name)?;
    let mut errno = 0;
    if fp.fseek(&mut errno, 344, 0) == -1 {
        g.errno = 0;
        fp.close();
        return None;
    }
    let b = read_or_zero(g, &mut fp, 4);
    fp.close();
    b.map(|b| [b[0], b[1], b[2], b[3]])
}

/// `is_nifti1(fname)`: 1 when the header `<stem>.hdr` (the name without a last `.img` or `.hdr`)
/// holds NIfTI-1's pair magic `ni1\0` at byte 344; else 0.
pub fn is_nifti1(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    fits(g, "is_nifti1", fname.len(), STRLEN);
    let stem = match fname.rfind('.') {
        Some(i) if matches!(&fname[i..], ".img" | ".hdr") => &fname[..i],
        _ => fname,
    };
    let hdr = format!("{stem}.hdr");
    i32::from(bytes_at_344(g, dir, &hdr).as_ref() == Some(NIFTI1_MAGIC))
}

/// `is_nii(fname)`: 1 for the extension `.nii`, a `.gz` name holding `.nii.gz`, or a file holding
/// NIfTI-1's single-file magic `n+1\0` at byte 344; else 0.
pub fn is_nii(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if let Some(i) = fname.rfind('.') {
        let d = &fname[i..];
        if d == ".nii" {
            return 1;
        }
        if d == ".gz" && fname.contains(".nii.gz") {
            return 1;
        }
    }
    i32::from(bytes_at_344(g, dir, fname).as_ref() == Some(NII_MAGIC))
}

/// `is_nrrd(fname)`: 1 for the extension `.nrrd`, or a file whose first 4 bytes are `NRRD`; else
/// 0.
pub fn is_nrrd(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if let Some(i) = fname.rfind('.')
        && &fname[i..] == ".nrrd"
    {
        return 1;
    }
    let Some(mut fp) = open_or_zero(g, dir, fname) else {
        return 0;
    };
    let b = read_or_zero(g, &mut fp, 4);
    fp.close();
    i32::from(b.as_deref() == Some(NRRD_MAGIC.as_slice()))
}

/// `IDisCurv(curvfile)`: 1 when the file's first 3 bytes (big endian, a short file's missing
/// bytes 0) are the curvature files' new-version number 16,777,215; else 0 (a failed open or read
/// keeps `errno`: 21 for a directory).
pub fn id_is_curv(g: &mut Globals, dir: Option<&Path>, curvfile: &str) -> i32 {
    let mut fp = match CFile::open(dir, curvfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return 0;
        }
    };
    let (_, magno) = fio::fread3(&mut fp);
    if let Some(e) = fp.read_errno() {
        g.errno = e;
    }
    fp.close();
    i32::from(magno == NEW_VERSION_MAGIC_NUMBER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn extensions() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(string_to_type(&mut g, "MGZ"), MRI_MGH_FILE);
        assert_eq!(string_to_type(&mut g, "img"), MRI_ANALYZE4D_FILE);
        assert_eq!(string_to_type(&mut g, ""), MRI_VOLUME_TYPE_UNKNOWN);
        assert_eq!(type_to_string(SIGNA_FILE), "unknown");
        assert_eq!(id_stem_from_name("a/b.nii.gz").as_deref(), Some("a/b"));
        assert_eq!(id_stem_from_name("x.w").as_deref(), Some("x"));
        assert_eq!(
            id_extension_from_name(&mut g, "a.nii").as_deref(),
            Some("nii")
        );
        assert_eq!(id_extension_from_name(&mut g, "brainnii"), None);
        assert!(g.notices.contains(&FS_226));
        let mut r = Globals::new(UpstreamDefects::Reproduce);
        assert_eq!(
            id_extension_from_name(&mut r, "brainnii").as_deref(),
            Some("nii")
        );
        assert_eq!(
            id_extension_from_name(&mut g, "s_000.bshort").as_deref(),
            Some("bshort")
        );
        assert_eq!(mri_get_volume_name(&mut g, "a.mgz@mgh#1"), "a.mgz");
        assert_eq!(xpg_basename("a/COR-1/"), "COR-1");
        assert_eq!(xpg_basename("//"), "/");
        assert_eq!(is_mgh(&mut g, "x.mgz"), 1);
        assert_eq!(is_mgh(&mut r, "x.mgz"), 0);
        assert_eq!(is_mgh(&mut r, ".mgz"), 1);
    }
}

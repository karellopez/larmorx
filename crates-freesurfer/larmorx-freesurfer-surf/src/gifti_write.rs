// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gifti.cpp`'s GIFTI writers (queue item 3.1-26, its surface part; relay 22 of track D):
//! `gifti_set_DA_value_2D` (213-336, file-local), `insertCommonMetaData` (1416-1424),
//! `MRISwriteGIFTI` (1443-1495), `MRISwriteGIFTIIntent` and its intent writers `Shape`, `Stats`,
//! `Label`, `RGBAVector`, `Surface` (1612-2416), the overloads that take their values from a
//! volume (`MRISwriteGIFTI` 2419-2465, `MRISwriteGIFTIIntent`, `Shape`, `Stats` 2536-2744), the
//! combined writer `MRISwriteGIFTICombined` (2470-2532) and `getShapeStatIntentCount` (2746-2855),
//! over gifticlib 1.12 (`larmorx_io::gifti`, track C's port) with FreeSurfer's two changes to it
//! ([`FsGifti`]). `MRISurfOverlay::getGIFTIIntent` (`utils/MRISurfOverlay.cpp` 225-284) and
//! `colortab.cpp`'s `CTABfindAnnotation` and `CTABfindRGBi` (1492-1508, 1611-1632; fs.labels) are
//! taken early, as the writers call them.
//!
//! Every file carries `UserName` (the effective user's name from the password database: `/proc`
//! and `/etc/passwd` here; `USER` or `USERNAME` where they cannot be read; none when nothing gives
//! one, as `getpwuid` failing) and `Date` (`currentDateTime`, which
//! `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` replaces). Data arrays are written as
//! GZipBase64Binary: gifticlib's port compresses with zlib-rs, whose bytes may differ from zlib
//! 1.2.11's; the values decompress the same (`docs/findings/gifti.md`).
//!
//! The defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-surfio.md` §24):
//! FS-397 (unnamed colour entries in the label table), FS-398 (labels without a colour table),
//! FS-399 (a volume's values placed by column only), FS-400 (a statistics volume's frames), FS-523
//! (an overlay without its volume), FS-524 (statistics overlays of the generic type dropped by the
//! combined writer); FS-395 (a file without a Version, read by `getShapeStatIntentCount`), FS-17.

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::cfmt::{self, Writers};
use larmorx_core::defects::{FS_395, FS_397, FS_398, FS_399, FS_400, FS_523, FS_524, Notices};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    ERROR_BADFILE, ERROR_BADPARM, ERROR_NOMEMORY, ERROR_NONE, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::timer::current_date_time;
use larmorx_freesurfer_io::gifti::gifti_alloc_and_add_darray;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::vgeom::{tkr_vox2ras_from_vol_geom, vg_i_to_r};
use larmorx_io::gifti::{
    DataArray, GIFTI_ENCODING_B64GZ, GIFTI_ENDIAN_LITTLE, GIFTI_IND_ORD_ROW_MAJOR,
    GIFTI_XML_VERSION, Gifti, GiftiImage, LabelTable, NIFTI_INTENT_NONE, NIFTI_INTENT_POINTSET,
    NIFTI_INTENT_TRIANGLE, NIFTI_TYPE_FLOAT32, NIFTI_TYPE_INT8, NIFTI_TYPE_INT16, NIFTI_TYPE_INT32,
    NIFTI_TYPE_UINT8, NIFTI_TYPE_UINT16, NIFTI_TYPE_UINT32, NvPairs, intent_to_string,
};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::gifti::{
    FS_MRISURFOVERLAY_STATS, FsGifti, NIFTI_INTENT_GENMATRIX, NIFTI_INTENT_LABEL,
    NIFTI_INTENT_RGB_VECTOR, NIFTI_INTENT_RGBA_VECTOR, NIFTI_INTENT_SHAPE, NIFTI_INTENT_VECTOR,
    OverlayInfo, Renamed, vertices_cols,
};
use crate::mris::Mris;
use crate::readwrite::read_transform;

/// NIfTI-1's statistics intents the writers accept (`nifti1.h`): `NIFTI_INTENT_CORREL` (2) to
/// `NIFTI_INTENT_LOG10PVAL` (24), and `NIFTI_INTENT_ESTIMATE` (1001).
pub const NIFTI_INTENT_UNIFORM: i32 = 16;
pub const NIFTI_INTENT_ESTIMATE: i32 = 1001;
/// `TAG_CMDLINE_LEN` (`gifti.cpp` line 39): a command line's buffer.
const TAG_CMDLINE_LEN: usize = 1024;

/// Whether `intent` is one of the statistics intents `MRISwriteGIFTIIntent` writes as statistics.
pub fn is_stats_intent(intent: i32) -> bool {
    (2..=24).contains(&intent) || intent == NIFTI_INTENT_ESTIMATE
}

/// `MRISurfOverlay::getGIFTIIntent(overlaytype)`: the GIFTI intent of an overlay type: the shape
/// types (100 to 104) `NIFTI_INTENT_SHAPE`, the statistics types 201 to 223 the statistics
/// intents 2 to 24 in order, 224 `NIFTI_INTENT_ESTIMATE`; any other type (the generic statistics
/// type 200 included) `NIFTI_INTENT_NONE`.
pub fn get_gifti_intent(overlaytype: i32) -> i32 {
    match overlaytype {
        100..=104 => NIFTI_INTENT_SHAPE,
        201..=223 => overlaytype - 199,
        224 => NIFTI_INTENT_ESTIMATE,
        _ => NIFTI_INTENT_NONE,
    }
}

/// `CTABfindRGBi(ct, r, g, b, &index)` for components in [0, 255]: the first entry with those
/// integer colours, else -1.
pub fn ctab_find_rgbi(ct: &ColorTable, r: i32, g: i32, b: i32) -> i32 {
    ct.entries
        .iter()
        .position(|e| {
            e.as_ref()
                .is_some_and(|e| e.ri == r && e.gi == g && e.bi == b)
        })
        .map_or(-1, |k| k as i32)
}

/// `CTABfindAnnotation(ct, annotation, &index)`: the entry of an annotation's colours (its low
/// byte red, then green, then blue), else -1.
pub fn ctab_find_annotation(ct: &ColorTable, annotation: i32) -> i32 {
    ctab_find_rgbi(
        ct,
        annotation & 0xff,
        (annotation >> 8) & 0xff,
        (annotation >> 16) & 0xff,
    )
}

/// `(T) value` for an integer type of 8, 16 or 32 bits as x86-64 converts a `double`: through a
/// 32-bit `int` (`cvttsd2si`, FS-17 beyond its range), its low bits kept.
fn to_int_bits(g: &mut Globals, value: f64) -> i32 {
    float_to_int(g, value, "gifti_set_DA_value_2D")
}

/// `(unsigned int) value` as x86-64 converts a `double`: through a 64-bit integer
/// (`cvttsd2si` with a 64-bit result: beyond its range or NaN `INT64_MIN`), its low 32 bits.
fn to_uint32_bits(g: &mut Globals, value: f64) -> u32 {
    if value.is_nan()
        || value >= 9.223_372_036_854_775_808e18
        || value < -9.223_372_036_854_775_808e18
    {
        g.notices.add(
            &larmorx_core::defects::FS_17,
            format!("gifti_set_DA_value_2D: (unsigned int) {value}, outside the range: an undefined conversion"),
        );
        if g.notices.reproduce() {
            return 0;
        }
        return if value > 0.0 { u32::MAX } else { 0 };
    }
    let v = value.trunc() as i64;
    if !(0..=i64::from(u32::MAX)).contains(&v) {
        g.notices.add(
            &larmorx_core::defects::FS_17,
            format!("gifti_set_DA_value_2D: (unsigned int) {value}, outside the range: an undefined conversion"),
        );
        if !g.notices.reproduce() {
            return if v > 0 { u32::MAX } else { 0 };
        }
    }
    v as u32
}

/// `gifti_set_DA_value_2D(da, row, col, value)`: `value` stored as the array's data type at row
/// `row`, column `col` (a 1-D array: column 0 only, the row its element). A 1-D array with another
/// column, or an array of another dimensionality, ends the program after its message (exit 1); an
/// element outside the dimensions is refused with `** gifti_set_DA_value_2D, invalid params:
/// dim0_index=<i> (max=<n>), dim1_index=<j> (max=<m>)` on standard error; a data type other than
/// the integers of 8 to 32 bits and FLOAT32 with `** gifti_set_DA_value_2D, unsupported type
/// <t>-unknown, or can't convert to double`. An array without data ends the program (never met:
/// every writer allocates the data first).
pub fn gifti_set_da_value_2d(
    g: &mut Globals,
    err: &mut dyn Write,
    da: &mut DataArray,
    row: i32,
    col: i32,
    value: f64,
) -> Result<(), Fatal> {
    if da.data.is_none() {
        let _ = err.write_all(b"** gifti_set_DA_value_2D, invalid params: data=(nil)\n");
        return Err(Fatal::Exit(1));
    }
    let (dims_0, dims_1) = if da.num_dim == 1 {
        if col != 0 {
            let _ = writeln!(
                err,
                "** gifti_set_DA_value_2D, array dim is 1 but trying to access 2D data element (col={col})"
            );
            return Err(Fatal::Exit(1));
        }
        (da.dims[0], 1)
    } else if da.num_dim != 2 {
        let _ = writeln!(err, "** gifti_set_DA_value_2D, array dim is {}", da.num_dim);
        return Err(Fatal::Exit(1));
    } else {
        (da.dims[0], da.dims[1])
    };
    let row_major = da.ind_ord == GIFTI_IND_ORD_ROW_MAJOR;
    let (dim0_index, dim1_index) = if row_major || da.num_dim == 1 {
        (row, col)
    } else {
        (col, row)
    };
    if dim0_index < 0 || dim0_index >= dims_0 || dim1_index < 0 || dim1_index >= dims_1 {
        let _ = writeln!(
            err,
            "** gifti_set_DA_value_2D, invalid params: dim0_index={dim0_index} (max={dims_0}), dim1_index={dim1_index} (max={dims_1})"
        );
        return Ok(());
    }
    // C's int arithmetic of the element's place (the dimensions are small enough for it)
    let k = if row_major {
        i64::from(dim0_index) * i64::from(dims_1) + i64::from(dim1_index)
    } else {
        i64::from(dim0_index) + i64::from(dim1_index) * i64::from(dims_0)
    } as usize;
    let bytes: Vec<u8> = match da.datatype {
        NIFTI_TYPE_UINT8 => vec![to_int_bits(g, value) as u8],
        NIFTI_TYPE_INT16 => (to_int_bits(g, value) as i16).to_le_bytes().to_vec(),
        NIFTI_TYPE_INT32 => to_int_bits(g, value).to_le_bytes().to_vec(),
        NIFTI_TYPE_FLOAT32 => (value as f32).to_le_bytes().to_vec(),
        NIFTI_TYPE_INT8 => vec![to_int_bits(g, value) as i8 as u8],
        NIFTI_TYPE_UINT16 => (to_int_bits(g, value) as u16).to_le_bytes().to_vec(),
        NIFTI_TYPE_UINT32 => to_uint32_bits(g, value).to_le_bytes().to_vec(),
        t => {
            let _ = writeln!(
                err,
                "** gifti_set_DA_value_2D, unsupported type {t}-unknown, or can't convert to double"
            );
            return Ok(());
        }
    };
    let data = da.data.as_mut().expect("checked above");
    let at = k * bytes.len();
    if let Some(dst) = data.get_mut(at..at + bytes.len()) {
        dst.copy_from_slice(&bytes);
    }
    Ok(())
}

/// The effective user's name as `getpwuid(geteuid())` gives it: on Linux from `/proc/self/status`
/// and `/etc/passwd`; elsewhere (or when they cannot be read) `USER`, then `USERNAME`; `None`
/// when nothing gives one.
fn passwd_user_name(env: &Environment) -> Option<String> {
    let from_passwd = || -> Option<String> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let euid = status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))?
            .split_ascii_whitespace()
            .nth(1)?
            .to_owned();
        let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
        passwd.lines().find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() > 2 && f[2] == euid).then(|| f[0].to_owned())
        })
    };
    from_passwd().or_else(|| {
        ["USER", "USERNAME"].iter().find_map(|n| {
            env.get(n)
                .map(|v| v.to_string_lossy().into_owned())
                .filter(|v| !v.is_empty())
        })
    })
}

/// `gifti_add_to_meta(md, name, value, 1)`; a `NULL` value adds nothing (gifticlib returns 1).
pub(crate) fn add_meta(
    gx: &Gifti,
    io: &mut Io,
    notices: &mut Notices,
    md: &mut NvPairs,
    name: &str,
    value: Option<&[u8]>,
) {
    let Some(value) = value else { return };
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gx.add_to_meta(md, name.as_bytes(), value, true, &mut w, notices);
}

/// `insertCommonMetaData(md)`: `UserName` (when the password database gives one) and `Date`
/// (`currentDateTime()`).
fn insert_common_meta_data(gx: &Gifti, io: &mut Io, notices: &mut Notices, md: &mut NvPairs) {
    if let Some(user) = passwd_user_name(io.env) {
        add_meta(gx, io, notices, md, "UserName", Some(user.as_bytes()));
    }
    let date = current_date_time(io.env, true);
    add_meta(gx, io, notices, md, "Date", Some(date.as_bytes()));
}

/// A new image: version "1.0", the common metadata.
pub(crate) fn new_image(gx: &Gifti, io: &mut Io, notices: &mut Notices) -> GiftiImage {
    let mut image = GiftiImage {
        version: Some(GIFTI_XML_VERSION.as_bytes().to_vec()),
        ..GiftiImage::default()
    };
    let mut md = std::mem::take(&mut image.meta);
    insert_common_meta_data(gx, io, notices, &mut md);
    image.meta = md;
    image
}

/// A new data array at the end of `image` (`gifti_alloc_and_add_darray`): its index, or `None`.
pub(crate) fn add_array(gx: &Gifti, io: &mut Io, image: &mut GiftiImage) -> Option<usize> {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gifti_alloc_and_add_darray(gx, Some(image), &mut w)?;
    Some(image.num_da as usize - 1)
}

fn array(image: &mut GiftiImage, k: usize) -> &mut DataArray {
    image.darray[k].as_mut().expect("an added data array")
}

/// The attributes every FreeSurfer writer gives an array: the intent and type, row-major order,
/// the dimensions, GZipBase64Binary, little-endian, no coordinate system.
pub(crate) fn set_attributes(
    da: &mut DataArray,
    intent: i32,
    datatype: i32,
    num_dim: i32,
    d0: i32,
    d1: i32,
) {
    da.intent = intent;
    da.datatype = datatype;
    da.ind_ord = GIFTI_IND_ORD_ROW_MAJOR;
    da.num_dim = num_dim;
    da.dims[0] = d0;
    da.dims[1] = d1;
    da.encoding = GIFTI_ENCODING_B64GZ;
    da.endian = GIFTI_ENDIAN_LITTLE;
    da.coordsys = Vec::new();
}

/// `da->nvals = gifti_darray_nvals(da)` and `gifti_datatype_sizes(da->datatype, &da->nbyper,
/// NULL)`.
pub(crate) fn set_sizes(gx: &Gifti, io: &mut Io, fix: bool, da: &mut DataArray) {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    da.nvals = gx.darray_nvals(da, &mut w);
    if let Some((n, _)) = gx.datatype_sizes(da.datatype, fix, &mut w) {
        da.nbyper = n;
    }
}

/// `da->data = calloc(da->nvals, da->nbyper)`.
pub(crate) fn alloc_data(da: &mut DataArray) {
    let n = (da.nvals.max(0) as usize).saturating_mul(da.nbyper.max(0) as usize);
    da.data = Some(vec![0; n]);
}

/// The C string's bytes of a name (before its first NUL).
fn c_bytes(s: &[u8]) -> &[u8] {
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    &s[..end]
}

/// C's `strstr(haystack, needle) != NULL` on a C string.
fn contains(haystack: &[u8], needle: &str) -> bool {
    let h = c_bytes(haystack);
    let n = needle.as_bytes();
    h.windows(n.len()).any(|w| w == n)
}

/// The stats metadata: `Intent_code` (gifticlib's name of the intent), and for
/// `NIFTI_INTENT_UNIFORM` `Intent_p1` 0 and `Intent_p2` 1.
fn stats_meta(gx: &Gifti, io: &mut Io, notices: &mut Notices, md: &mut NvPairs, intent: i32) {
    let s = intent_to_string(intent);
    add_meta(gx, io, notices, md, "Intent_code", Some(s.as_bytes()));
    if intent == NIFTI_INTENT_UNIFORM {
        add_meta(gx, io, notices, md, "Intent_p1", Some(b"0"));
        add_meta(gx, io, notices, md, "Intent_p2", Some(b"1"));
    }
}

/// `MRISwriteGIFTIShape(mris, image, intent_code, curv_fname)`: a `NIFTI_INTENT_SHAPE` array of
/// FLOAT32, one value per vertex (its curvature), with `Name` (the curvature file's name, when
/// given) and `ShapeDataType` from that name (the last of `.thickness` Thickness, `.curv`
/// CurvatureRadial, `.sulc` SulcalDepth, `.area` Area, `.volume` Volume, `.jacobian` Jacobian it
/// contains).
fn write_shape(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    image: &mut GiftiImage,
    curv_fname: Option<&str>,
) -> Result<i32, Fatal> {
    let Some(k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTIShape: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    let fix = g.notices.fix();
    let shape = array(image, k);
    set_attributes(
        shape,
        NIFTI_INTENT_SHAPE,
        NIFTI_TYPE_FLOAT32,
        1,
        mris.nvertices,
        0,
    );
    set_sizes(gx, io, fix, shape);
    if let Some(name) = curv_fname {
        add_meta(
            gx,
            io,
            &mut g.notices,
            &mut shape.meta,
            "Name",
            Some(name.as_bytes()),
        );
        let mut meta = None;
        for (pat, m) in [
            (".thickness", "Thickness"),
            (".curv", "CurvatureRadial"),
            (".sulc", "SulcalDepth"),
            (".area", "Area"),
            (".volume", "Volume"),
            (".jacobian", "Jacobian"),
        ] {
            if contains(name.as_bytes(), pat) {
                meta = Some(m);
            }
        }
        if let Some(m) = meta {
            add_meta(
                gx,
                io,
                &mut g.notices,
                &mut shape.meta,
                "ShapeDataType",
                Some(m.as_bytes()),
            );
        }
    }
    alloc_data(shape);
    for vno in 0..mris.nvertices.max(0) {
        let v = f64::from(mris.vertex(vno as usize).curv);
        gifti_set_da_value_2d(g, io.err, shape, vno, 0, v)?;
    }
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTIStats(mris, image, intent_code)`: an array of the statistics intent, FLOAT32,
/// one value per vertex (its `stat`), with `Intent_code` (and the uniform distribution's range).
fn write_stats(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    image: &mut GiftiImage,
    intent: i32,
) -> Result<i32, Fatal> {
    let Some(k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTIStats: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    let fix = g.notices.fix();
    let stats = array(image, k);
    set_attributes(stats, intent, NIFTI_TYPE_FLOAT32, 1, mris.nvertices, 0);
    set_sizes(gx, io, fix, stats);
    stats_meta(gx, io, &mut g.notices, &mut stats.meta, intent);
    alloc_data(stats);
    for vno in 0..mris.nvertices.max(0) {
        let v = f64::from(mris.vertex(vno as usize).stat);
        gifti_set_da_value_2d(g, io.err, stats, vno, 0, v)?;
    }
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTILabel(mris, image, intent_code)`: the surface's colour table as the image's label
/// table (each named entry: its place in the colour table as the key, its name, its float colours
/// with alpha 1), and a `NIFTI_INTENT_LABEL` array of INT32 named `node label` (the table's second
/// label when the table has two), each vertex's value the place of the first colour-table entry
/// with its annotation's colours (-1 when none). With `Gdiag`'s `DIAG_VERBOSE` the counts and each
/// label on standard output (`[DEBUG] ...`). A table of no labels: `MRISwriteGIFTILabel:
/// colortable is empty!` and `ERROR_BADFILE`. FS-397: the table is sized by the entries that exist
/// and filled with those that have a name; by default sized by the named ones (`reproduce`: the
/// unnamed places stay, and the file is refused as invalid). FS-398: a surface without a colour
/// table is refused by default (`MRISwriteGIFTILabel: the surface has no colour table`,
/// `ERROR_BADPARM`); `reproduce` crashes as FreeSurfer does.
fn write_label(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    image: &mut GiftiImage,
) -> Result<i32, Fatal> {
    let Some(ct) = mris.ct.as_ref() else {
        g.notices.add(
            &FS_398,
            "MRISwriteGIFTILabel: labels of a surface without a colour table (FreeSurfer reads through a null pointer)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTILabel: the surface has no colour table\n");
        return Ok(ERROR_BADPARM);
    };
    let named = |e: &Option<Box<larmorx_freesurfer_mri::ctab::ColorTableEntry>>| {
        e.as_ref().is_some_and(|e| !c_bytes(&e.name).is_empty())
    };
    let present = ct.entries.iter().filter(|e| e.is_some()).count();
    let with_name = ct.entries.iter().filter(|e| named(e)).count();
    let mut length = present;
    if present != with_name {
        g.notices.add(
            &FS_397,
            format!(
                "MRISwriteGIFTILabel: {} colour-table entries without a name: the label table is sized by the named ones",
                present - with_name
            ),
        );
        if !g.notices.reproduce() {
            length = with_name;
        }
    }
    let verbose = g.gdiag & DIAG_VERBOSE != 0;
    if verbose {
        let _ = writeln!(
            io.out,
            "[DEBUG] MRISwriteGIFTILabel(): mris->ct->entries={}, to_write={length}",
            ct.nentries()
        );
    }
    if length == 0 {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTILabel: colortable is empty!\n");
        return Ok(ERROR_BADFILE);
    }
    let mut lt = LabelTable {
        key: vec![0; length],
        label: vec![None; length],
        rgba: Some(vec![Some(0.0); 4 * length]),
    };
    let mut idx = 0usize;
    for (n, e) in ct.entries.iter().enumerate() {
        let Some(e) = e else { continue };
        let name = c_bytes(&e.name);
        if name.is_empty() {
            continue;
        }
        lt.key[idx] = n as i32;
        if verbose {
            let _ = write!(io.out, "[DEBUG] MRISwriteGIFTILabel(): idx={idx}, name=");
            let _ = io.out.write_all(name);
            let _ = io.out.write_all(b"\n");
        }
        lt.label[idx] = Some(name.to_vec());
        if let Some(rgba) = lt.rgba.as_mut() {
            rgba[4 * idx] = Some(e.rf);
            rgba[4 * idx + 1] = Some(e.gf);
            rgba[4 * idx + 2] = Some(e.bf);
            rgba[4 * idx + 3] = Some(1.0);
        }
        idx += 1;
    }
    let second = (length == 2).then(|| lt.label[1].clone()).flatten();
    image.labeltable = lt;
    let Some(k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTILabel: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    let fix = g.notices.fix();
    let labels = array(image, k);
    set_attributes(
        labels,
        NIFTI_INTENT_LABEL,
        NIFTI_TYPE_INT32,
        1,
        mris.nvertices,
        0,
    );
    set_sizes(gx, io, fix, labels);
    add_meta(
        gx,
        io,
        &mut g.notices,
        &mut labels.meta,
        "Name",
        Some(b"node label"),
    );
    if length == 2 {
        add_meta(
            gx,
            io,
            &mut g.notices,
            &mut labels.meta,
            "Name",
            second.as_deref(),
        );
    }
    alloc_data(labels);
    let data = labels.data.as_mut().expect("allocated");
    for vno in 0..mris.nvertices.max(0) as usize {
        let idx = ctab_find_annotation(ct, mris.vertex(vno).annotation);
        if let Some(dst) = data.get_mut(4 * vno..4 * vno + 4) {
            dst.copy_from_slice(&idx.to_le_bytes());
        }
    }
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTIRGBAVector(mris, image, intent_code)`: a `NIFTI_INTENT_RGBA_VECTOR` array of
/// FLOAT32, 4 columns per vertex: its annotation's red, green and blue bytes over 255 (`(float) r
/// / 255.0` stored as `float`) and 1.
fn write_rgba(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    image: &mut GiftiImage,
) -> Result<i32, Fatal> {
    let Some(k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTIRGBAVector(): couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    let fix = g.notices.fix();
    let rgba = array(image, k);
    set_attributes(
        rgba,
        NIFTI_INTENT_RGBA_VECTOR,
        NIFTI_TYPE_FLOAT32,
        2,
        mris.nvertices,
        4,
    );
    set_sizes(gx, io, fix, rgba);
    alloc_data(rgba);
    for vno in 0..mris.nvertices.max(0) {
        let annot = mris.vertex(vno as usize).annotation;
        let (r, gr, b) = (annot & 0xff, (annot >> 8) & 0xff, (annot >> 16) & 0xff);
        gifti_set_da_value_2d(g, io.err, rgba, vno, 0, f64::from(r as f32) / 255.0)?;
        gifti_set_da_value_2d(g, io.err, rgba, vno, 1, f64::from(gr as f32) / 255.0)?;
        gifti_set_da_value_2d(g, io.err, rgba, vno, 2, f64::from(b as f32) / 255.0)?;
        gifti_set_da_value_2d(g, io.err, rgba, vno, 3, 1.0)?;
    }
    Ok(NO_ERROR)
}

/// A matrix result: the new matrix, `None` for `NULL`.
fn new_or_null(r: larmorx_freesurfer_core::ret::Ret<Matrix>) -> Option<Matrix> {
    r.into_new()
}

/// The coordinates' transform, as `MRISwriteGIFTISurface` chooses it: (`DataSpace`, the matrix,
/// `TransformedSpace`).
fn surface_xform(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    out_fname: &str,
) -> Result<(&'static str, Option<Matrix>, &'static str), Fatal> {
    const UNKNOWN: &str = "NIFTI_XFORM_UNKNOWN";
    const SCANNER: &str = "NIFTI_XFORM_SCANNER_ANAT";
    const TAL: &str = "NIFTI_XFORM_TALAIRACH";
    if mris.useRealRAS != 0 {
        if mris.vg.valid != 0 {
            // scanner space to tkregister space
            let s = vg_i_to_r(g, io.out, io.err, &mris.vg)?;
            let t = tkr_vox2ras_from_vol_geom(g, io.out, io.err, &mris.vg)?;
            let sinv = new_or_null(matrix_inverse(g, io.out, io.err, Some(&s), None)?);
            let x = new_or_null(matrix_multiply(
                g,
                io.out,
                io.err,
                Some(&t),
                sinv.as_ref(),
                None,
            )?);
            Ok((SCANNER, x, UNKNOWN))
        } else {
            let x = new_or_null(matrix_identity(g, io.out, io.err, 4, None)?);
            Ok((SCANNER, x, SCANNER))
        }
    } else if mris.vg.valid != 0 {
        // tkregister space to scanner space
        let s = vg_i_to_r(g, io.out, io.err, &mris.vg)?;
        let t = tkr_vox2ras_from_vol_geom(g, io.out, io.err, &mris.vg)?;
        let tinv = new_or_null(matrix_inverse(g, io.out, io.err, Some(&t), None)?);
        let x = new_or_null(matrix_multiply(
            g,
            io.out,
            io.err,
            Some(&s),
            tinv.as_ref(),
            None,
        )?);
        Ok((UNKNOWN, x, SCANNER))
    } else {
        read_transform(g, io, dir, mris, out_fname)?;
        match &mris.SRASToTalSRAS_ {
            Some(m) if m.rows == 4 && m.cols == 4 => {
                let x = new_or_null(matrix_copy(g, io.out, io.err, Some(m), None)?);
                Ok((UNKNOWN, x, TAL))
            }
            _ => Ok((UNKNOWN, None, UNKNOWN)),
        }
    }
}

/// `MRISwriteGIFTISurface(mris, image, out_fname)`: a `NIFTI_INTENT_POINTSET` array (FLOAT32, the
/// vertices' current coordinates) with one coordinate system, and a `NIFTI_INTENT_TRIANGLE` array
/// (INT32, the faces' vertices). The coordinate system: for real-RAS surfaces
/// `NIFTI_XFORM_SCANNER_ANAT` to `NIFTI_XFORM_UNKNOWN` with the scanner-to-tkregister matrix
/// (`TkrVox2RAS * inverse(vox2ras)` of the volume geometry), or the identity to itself without a
/// valid geometry; for tkregister surfaces `NIFTI_XFORM_UNKNOWN` to `NIFTI_XFORM_SCANNER_ANAT` with
/// `vox2ras * inverse(TkrVox2RAS)`, or without a valid geometry the subject's Talairach transform
/// (`MRISreadTransform` on the output's name; `NIFTI_XFORM_TALAIRACH`) when the surface then has a
/// 4 x 4 one, else the identity, both spaces unknown. With a surface name: the coordinates'
/// `AnatomicalStructurePrimary` (`lh.` CortexLeft, `rh.` CortexRight), `AnatomicalStructureSecondary`
/// and `GeometricType` from the name's parts, the faces' `TopologicalType` (Cut for a patch, else
/// Closed), both arrays' `Name`. With a valid geometry its 19 `VolGeom*` entries (`%d`, `%f`) and
/// `SurfaceCenterX/Y/Z`; a group-average area (not within `FLT_EPSILON` of 0) as
/// `TAG_GROUP_AVG_SURFACE_AREA` (`%.20f`); the command lines (`NUM_TAG_CMDLINE`, `TAG_CMDLINE#<k>`,
/// each at most 1,023 bytes).
fn write_surface(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    image: &mut GiftiImage,
    out_fname: &str,
) -> Result<i32, Fatal> {
    let gx_add = add_array(&sg.gifti, io, image);
    let Some(ck) = gx_add else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTISurface: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    set_attributes(
        array(image, ck),
        NIFTI_INTENT_POINTSET,
        NIFTI_TYPE_FLOAT32,
        2,
        mris.nvertices,
        3,
    );
    let (dataspace, xform, transformedspace) = surface_xform(g, io, dir, mris, out_fname)?;
    if xform.is_none() && dataspace != transformedspace {
        let _ = io
            .out
            .write_all(b"[ERROR] MRISwriteGIFTISurface(): couldn't obtain MatrixData\n");
        return Err(Fatal::Exit(1));
    }
    let xform = match xform {
        Some(x) => x,
        None => {
            new_or_null(matrix_identity(g, io.out, io.err, 4, None)?).expect("a 4 x 4 identity")
        }
    };
    let gx = &sg.gifti;
    let fix = g.notices.fix();
    {
        let coords = array(image, ck);
        coords.coordsys = Vec::new();
        {
            let mut w = Writers {
                out: &mut *io.out,
                err: &mut *io.err,
            };
            gx.add_empty_cs(coords, &mut w);
        }
        let cs = coords
            .coordsys
            .last_mut()
            .and_then(|c| c.as_mut())
            .expect("an added coordinate system");
        cs.dataspace = Some(dataspace.as_bytes().to_vec());
        cs.xformspace = Some(transformedspace.as_bytes().to_vec());
        for r in 1..=4 {
            for c in 1..=4 {
                cs.xform[r - 1][c - 1] = f64::from(xform.relt(r as i32, c as i32));
            }
        }
        set_sizes(gx, io, fix, coords);
        alloc_data(coords);
        for vno in 0..mris.nvertices.max(0) {
            let v = mris.vertex(vno as usize);
            let (x, y, z) = (f64::from(v.x), f64::from(v.y), f64::from(v.z));
            gifti_set_da_value_2d(g, io.err, coords, vno, 0, x)?;
            gifti_set_da_value_2d(g, io.err, coords, vno, 1, y)?;
            gifti_set_da_value_2d(g, io.err, coords, vno, 2, z)?;
        }
    }
    let Some(fk) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTISurface: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    {
        let faces = array(image, fk);
        set_attributes(
            faces,
            NIFTI_INTENT_TRIANGLE,
            NIFTI_TYPE_INT32,
            2,
            mris.nfaces,
            3,
        );
        set_sizes(gx, io, fix, faces);
        alloc_data(faces);
        for fno in 0..mris.nfaces.max(0) {
            let f = mris.face(fno as usize).v;
            for (c, &x) in f.iter().enumerate() {
                gifti_set_da_value_2d(g, io.err, faces, fno, c as i32, f64::from(x))?;
            }
        }
    }
    if !mris.fname.is_empty() {
        let name = mris.fname.as_bytes();
        let mut primary = None;
        let mut secondary = None;
        let mut geotype = None;
        if contains(name, "lh.") {
            primary = Some("CortexLeft");
        }
        if contains(name, "rh.") {
            primary = Some("CortexRight");
        }
        for (pat, s) in [
            (".orig", "GrayWhite"),
            (".smoothwm", "GrayWhite"),
            (".white", "GrayWhite"),
            (".graymid", "MidThickness"),
            (".gray", "Pial"),
            (".pial", "Pial"),
        ] {
            if contains(name, pat) {
                secondary = Some(s);
            }
        }
        for (pat, s) in [
            (".orig", "Reconstruction"),
            (".smoothwm", "Reconstruction"),
            (".white", "Anatomical"),
            (".gray", "Anatomical"),
            (".graymid", "Anatomical"),
            (".pial", "Anatomical"),
            (".inflated", "Inflated"),
            (".sphere", "Spherical"),
            (".qsphere", "Spherical"),
            ("pial-outer", "Hull"),
        ] {
            if contains(name, pat) {
                geotype = Some(s);
            }
        }
        let topotype: &[u8] = if mris.patch != 0 { b"Cut" } else { b"Closed" };
        let name = c_bytes(name).to_vec();
        let n = &mut g.notices;
        if let Some(p) = primary {
            add_meta(
                gx,
                io,
                n,
                &mut array(image, ck).meta,
                "AnatomicalStructurePrimary",
                Some(p.as_bytes()),
            );
        }
        if let Some(s) = secondary {
            add_meta(
                gx,
                io,
                n,
                &mut array(image, ck).meta,
                "AnatomicalStructureSecondary",
                Some(s.as_bytes()),
            );
        }
        if let Some(t) = geotype {
            add_meta(
                gx,
                io,
                n,
                &mut array(image, ck).meta,
                "GeometricType",
                Some(t.as_bytes()),
            );
        }
        add_meta(
            gx,
            io,
            n,
            &mut array(image, fk).meta,
            "TopologicalType",
            Some(topotype),
        );
        add_meta(gx, io, n, &mut array(image, ck).meta, "Name", Some(&name));
        add_meta(gx, io, n, &mut array(image, fk).meta, "Name", Some(&name));
    }
    let n = &mut g.notices;
    let md = &mut array(image, ck).meta;
    if mris.vg.valid != 0 {
        let vg = &mris.vg;
        add_meta(gx, io, n, md, "VolGeomFname", Some(vg.fname.c_bytes()));
        for (k, v) in [
            ("VolGeomWidth", vg.width),
            ("VolGeomHeight", vg.height),
            ("VolGeomDepth", vg.depth),
        ] {
            add_meta(gx, io, n, md, k, Some(v.to_string().as_bytes()));
        }
        for (k, v) in [
            ("VolGeomXsize", vg.xsize),
            ("VolGeomYsize", vg.ysize),
            ("VolGeomZsize", vg.zsize),
            ("VolGeomX_R", vg.x_r),
            ("VolGeomX_A", vg.x_a),
            ("VolGeomX_S", vg.x_s),
            ("VolGeomY_R", vg.y_r),
            ("VolGeomY_A", vg.y_a),
            ("VolGeomY_S", vg.y_s),
            ("VolGeomZ_R", vg.z_r),
            ("VolGeomZ_A", vg.z_a),
            ("VolGeomZ_S", vg.z_s),
            ("VolGeomC_R", vg.c_r),
            ("VolGeomC_A", vg.c_a),
            ("VolGeomC_S", vg.c_s),
            ("SurfaceCenterX", mris.xctr),
            ("SurfaceCenterY", mris.yctr),
            ("SurfaceCenterZ", mris.zctr),
        ] {
            add_meta(gx, io, n, md, k, Some(cfmt::f(f64::from(v), 6).as_bytes()));
        }
    }
    // FZERO: |x| < FLT_EPSILON
    let area = mris.group_avg_surface_area;
    if !(f64::from(area).abs() < f64::from(f32::EPSILON)) {
        let s = cfmt::f(f64::from(area), 20);
        add_meta(
            gx,
            io,
            n,
            md,
            "TAG_GROUP_AVG_SURFACE_AREA",
            Some(s.as_bytes()),
        );
    }
    if mris.ncmds > 0 {
        add_meta(
            gx,
            io,
            n,
            md,
            "NUM_TAG_CMDLINE",
            Some(mris.ncmds.to_string().as_bytes()),
        );
        for k in 0..mris.ncmds as usize {
            // snprintf(cmdline, TAG_CMDLINE_LEN, "%s", cmdlines[k]): "(null)" for none
            let c = match mris.cmdlines.get(k).and_then(|c| c.as_deref()) {
                Some(c) => c_bytes(c).to_vec(),
                None => b"(null)".to_vec(),
            };
            let c = &c[..c.len().min(TAG_CMDLINE_LEN - 1)];
            add_meta(gx, io, n, md, &format!("TAG_CMDLINE#{k}"), Some(c));
        }
    }
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTIIntent(mris, intent_code, image, out_fname, curv_fname)`: the arrays of one
/// intent: the surface (`NIFTI_INTENT_POINTSET` or `_TRIANGLE`), the curvature
/// (`NIFTI_INTENT_SHAPE`), the labels (`NIFTI_INTENT_LABEL`), the annotation's colours
/// (`NIFTI_INTENT_RGBA_VECTOR`) or the `stat` values (a statistics intent); any other intent adds
/// nothing.
#[allow(clippy::too_many_arguments)]
pub fn mris_write_gifti_intent(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    intent: i32,
    image: &mut GiftiImage,
    out_fname: &str,
    curv_fname: Option<&str>,
) -> Result<i32, Fatal> {
    if intent == NIFTI_INTENT_POINTSET || intent == NIFTI_INTENT_TRIANGLE {
        let e = write_surface(g, sg, io, dir, mris, image, out_fname)?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    let gx = &sg.gifti;
    if intent == NIFTI_INTENT_SHAPE {
        let e = write_shape(g, gx, io, mris, image, curv_fname)?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    if intent == NIFTI_INTENT_LABEL {
        let e = write_label(g, gx, io, mris, image)?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    if intent == NIFTI_INTENT_RGBA_VECTOR {
        let e = write_rgba(g, gx, io, mris, image)?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    if is_stats_intent(intent) {
        let e = write_stats(g, gx, io, mris, image, intent)?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    Ok(NO_ERROR)
}

/// The end every writer shares: a version "1" made "1.0", gifticlib's validity check (its
/// messages, then `<who>: GIFTI file <name> is invalid!`, `ERROR_BADFILE`), `gifti_write_image`
/// with the data (`<who>: couldn't write image`, `ERROR_BADFILE`), the image freed (gifticlib's
/// messages at its verbosity 3 and above).
pub(crate) fn validate_and_write(
    g: &mut Globals,
    gx: &mut Gifti,
    io: &mut Io,
    dir: Option<&Path>,
    mut image: GiftiImage,
    out_fname: &str,
    who: &str,
) -> i32 {
    if image.version.as_deref() == Some(b"1") {
        image.version = Some(GIFTI_XML_VERSION.as_bytes().to_vec());
    }
    let fix = g.notices.fix();
    let valid = {
        let mut w = Writers {
            out: &mut *io.out,
            err: &mut *io.err,
        };
        gx.valid_gifti_image(Some(&image), true, &FsGifti, fix, &mut w, &mut g.notices)
    };
    if !valid {
        let _ = writeln!(io.err, "{who}: GIFTI file {out_fname} is invalid!");
        free(gx, io, image);
        return ERROR_BADFILE;
    }
    let path = dir.map_or_else(|| PathBuf::from(out_fname), |d| d.join(out_fname));
    let rc = {
        let mut w = Renamed {
            w: Writers {
                out: &mut *io.out,
                err: &mut *io.err,
            },
            from: path.to_string_lossy().into_owned(),
            to: out_fname.to_owned(),
        };
        gx.write_image(&mut image, &path, true, &mut w, &mut g.notices)
    };
    if rc != 0 {
        let _ = writeln!(io.err, "{who}: couldn't write image");
        free(gx, io, image);
        return ERROR_BADFILE;
    }
    free(gx, io, image);
    ERROR_NONE
}

pub(crate) fn free(gx: &Gifti, io: &mut Io, image: GiftiImage) {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gx.free_image(image, &mut w);
}

/// A new image with the common metadata and, for a surface with a subject, `SubjectID`.
fn surface_image(g: &mut Globals, gx: &Gifti, io: &mut Io, mris: &Mris) -> GiftiImage {
    let mut image = new_image(gx, io, &mut g.notices);
    if !mris.subject_name.is_empty() {
        let mut md = std::mem::take(&mut image.meta);
        add_meta(
            gx,
            io,
            &mut g.notices,
            &mut md,
            "SubjectID",
            Some(c_bytes(mris.subject_name.as_bytes())),
        );
        image.meta = md;
    }
    image
}

/// `MRISwriteGIFTI(mris, intent_code, out_fname, curv_fname)`: a GIFTI file of one intent's
/// arrays ([`mris_write_gifti_intent`]) with the image's `UserName`, `Date` and `SubjectID` (a
/// surface with a subject's name), checked by gifticlib and written; `ERROR_NONE`, or the intent
/// writer's error (nothing written), or `ERROR_BADFILE` after the check's or the writer's
/// messages.
#[allow(clippy::too_many_arguments)]
pub fn mris_write_gifti(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    intent: i32,
    out_fname: &str,
    curv_fname: Option<&str>,
) -> Result<i32, Fatal> {
    let mut image = surface_image(g, &sg.gifti, io, mris);
    let e = mris_write_gifti_intent(
        g, sg, io, dir, mris, intent, &mut image, out_fname, curv_fname,
    )?;
    if e != NO_ERROR {
        return Ok(e);
    }
    Ok(validate_and_write(
        g,
        &mut sg.gifti,
        io,
        dir,
        image,
        out_fname,
        "MRISwriteGIFTI",
    ))
}

/// The vertex a volume's voxel gives (FS-399): by default its place in the volume (`c + width * (r
/// + height * s)`); with `reproduce` its column only, as FreeSurfer places it.
fn voxel_vertex(g: &Globals, mri: &Mri, c: i32, r: i32, s: i32) -> i32 {
    if g.notices.reproduce() {
        return c;
    }
    let k =
        i64::from(c) + i64::from(mri.width) * (i64::from(r) + i64::from(mri.height) * i64::from(s));
    i32::try_from(k).unwrap_or(i32::MAX)
}

fn note_fs399(g: &mut Globals, mri: &Mri, who: &str) {
    if mri.height > 1 || mri.depth > 1 {
        g.notices.add(
            &FS_399,
            format!(
                "{who}: a volume of {} x {} x {} voxels: each voxel placed by its place in the volume (FreeSurfer: by its column only)",
                mri.width, mri.height, mri.depth
            ),
        );
    }
}

/// The values of a volume's frame into a 1-D array, slice by slice, row by row, column by column
/// (`MRIgetVoxVal`), each at [`voxel_vertex`].
fn copy_frame(
    g: &mut Globals,
    io: &mut Io,
    mri: &Mri,
    da: &mut DataArray,
    frame: i32,
) -> Result<(), Fatal> {
    for s in 0..mri.depth {
        for r in 0..mri.height {
            for c in 0..mri.width {
                let v = mri_get_vox_val(g, io.err, mri, c, r, s, frame, 0)?;
                let vno = voxel_vertex(g, mri, c, r, s);
                gifti_set_da_value_2d(g, io.err, da, vno, 0, f64::from(v))?;
            }
        }
    }
    Ok(())
}

/// `MRISwriteGIFTIShape(mris, mri, image, intent_code, curv_fname, shapedatatype)`: a
/// `NIFTI_INTENT_SHAPE` array of FLOAT32 of one value per vertex from the volume's first frame,
/// with `Name` (the given name) and `ShapeDataType` (the given type) when given. FS-399.
#[allow(clippy::too_many_arguments)]
fn write_shape_mri(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    mri: Option<&Mri>,
    image: &mut GiftiImage,
    curv_fname: Option<&[u8]>,
    shapedatatype: Option<&[u8]>,
) -> Result<i32, Fatal> {
    let Some(k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTIShape: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    let fix = g.notices.fix();
    let shape = array(image, k);
    set_attributes(
        shape,
        NIFTI_INTENT_SHAPE,
        NIFTI_TYPE_FLOAT32,
        1,
        mris.nvertices,
        0,
    );
    set_sizes(gx, io, fix, shape);
    add_meta(
        gx,
        io,
        &mut g.notices,
        &mut shape.meta,
        "Name",
        curv_fname.map(c_bytes),
    );
    if let Some(t) = shapedatatype {
        add_meta(
            gx,
            io,
            &mut g.notices,
            &mut shape.meta,
            "ShapeDataType",
            Some(c_bytes(t)),
        );
    }
    alloc_data(shape);
    // FS-523 with reproduce: the volume is read here
    let Some(mri) = mri else {
        return Err(Fatal::Exit(139));
    };
    note_fs399(g, mri, "MRISwriteGIFTIShape");
    copy_frame(g, io, mri, shape, 0)?;
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTIStats(mris, mri, image, intent_code, curv_fname, statsdatatype)`: arrays of the
/// statistics intent, FLOAT32, one value per vertex, with `Intent_code`. FS-400: by default one
/// array per frame of the volume (none for a volume of no frames); with `reproduce` one array
/// whatever the frames, holding the last frame (a volume of no frames leaves it without
/// attributes). FS-399.
#[allow(clippy::too_many_arguments)]
fn write_stats_mri(
    g: &mut Globals,
    gx: &Gifti,
    io: &mut Io,
    mris: &Mris,
    mri: Option<&Mri>,
    image: &mut GiftiImage,
    intent: i32,
) -> Result<i32, Fatal> {
    let Some(mut k) = add_array(gx, io, image) else {
        let _ = io
            .err
            .write_all(b"MRISwriteGIFTIStats: couldn't allocate giiDataArray\n");
        return Ok(ERROR_NOMEMORY);
    };
    // FS-523 with reproduce: the volume is read here
    let Some(mri) = mri else {
        return Err(Fatal::Exit(139));
    };
    if mri.nframes != 1 {
        g.notices.add(
            &FS_400,
            format!(
                "MRISwriteGIFTIStats: a volume of {} frames: one data array per frame (FreeSurfer: one, the last frame's)",
                mri.nframes
            ),
        );
    }
    let reproduce = g.notices.reproduce();
    if !reproduce && mri.nframes <= 0 {
        // no frame: no array
        image.darray.truncate(k);
        image.num_da = k as i32;
        return Ok(NO_ERROR);
    }
    let fix = g.notices.fix();
    for f in 0..mri.nframes.max(0) {
        if f > 0 && !reproduce {
            let Some(kk) = add_array(gx, io, image) else {
                let _ = io
                    .err
                    .write_all(b"MRISwriteGIFTIStats: couldn't allocate giiDataArray\n");
                return Ok(ERROR_NOMEMORY);
            };
            k = kk;
        }
        let stats = array(image, k);
        set_attributes(stats, intent, NIFTI_TYPE_FLOAT32, 1, mris.nvertices, 0);
        set_sizes(gx, io, fix, stats);
        stats_meta(gx, io, &mut g.notices, &mut stats.meta, intent);
        alloc_data(stats);
        if f == 0 {
            note_fs399(g, mri, "MRISwriteGIFTIStats");
        }
        copy_frame(g, io, mri, stats, f)?;
    }
    Ok(NO_ERROR)
}

/// `MRISwriteGIFTIIntent(mris, mri, image, intent_code, out_fname, curv_fname, datatype)`: the
/// surface (`NIFTI_INTENT_POINTSET` or `_TRIANGLE`), the surface's labels (`NIFTI_INTENT_LABEL`,
/// as [`mris_write_gifti`] writes them), a shape array or statistics arrays from the volume; any
/// other intent (colours included) adds nothing. FS-523: an overlay without
/// its volume is refused by default (`MRISwriteGIFTIIntent: the overlay has no values`,
/// `ERROR_BADPARM`); `reproduce` crashes as FreeSurfer does.
#[allow(clippy::too_many_arguments)]
pub fn mris_write_gifti_intent_mri(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mri: Option<&Mri>,
    image: &mut GiftiImage,
    intent: i32,
    out_fname: &str,
    curv_fname: Option<&[u8]>,
    datatype: Option<&[u8]>,
) -> Result<i32, Fatal> {
    if intent == NIFTI_INTENT_POINTSET || intent == NIFTI_INTENT_TRIANGLE {
        return write_surface(g, sg, io, dir, mris, image, out_fname);
    }
    if intent == NIFTI_INTENT_LABEL {
        return write_label(g, &sg.gifti, io, mris, image);
    }
    let shape = intent == NIFTI_INTENT_SHAPE;
    if !shape && !is_stats_intent(intent) {
        return Ok(NO_ERROR);
    }
    if mri.is_none() {
        g.notices.add(
            &FS_523,
            "MRISwriteGIFTIIntent: an overlay without its volume (FreeSurfer reads through a null pointer)",
        );
        if !g.notices.reproduce() {
            let _ = io
                .err
                .write_all(b"MRISwriteGIFTIIntent: the overlay has no values\n");
            return Ok(ERROR_BADPARM);
        }
    }
    if shape {
        write_shape_mri(g, &sg.gifti, io, mris, mri, image, curv_fname, datatype)
    } else {
        write_stats_mri(g, &sg.gifti, io, mris, mri, image, intent)
    }
}

/// `MRISwriteGIFTI(mris, mri, intent_code, out_fname, curv_fname, datatype)`: as
/// [`mris_write_gifti`], the arrays from [`mris_write_gifti_intent_mri`].
#[allow(clippy::too_many_arguments)]
pub fn mris_write_gifti_mri(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mri: &Mri,
    intent: i32,
    out_fname: &str,
    curv_fname: Option<&[u8]>,
    datatype: Option<&[u8]>,
) -> Result<i32, Fatal> {
    let mut image = surface_image(g, &sg.gifti, io, mris);
    let e = mris_write_gifti_intent_mri(
        g,
        sg,
        io,
        dir,
        mris,
        Some(mri),
        &mut image,
        intent,
        out_fname,
        curv_fname,
        datatype,
    )?;
    if e != NO_ERROR {
        return Ok(e);
    }
    Ok(validate_and_write(
        g,
        &mut sg.gifti,
        io,
        dir,
        image,
        out_fname,
        "MRISwriteGIFTI",
    ))
}

/// The intent an overlay is written with by the combined writer: [`get_gifti_intent`] of its type;
/// FS-524: the generic statistics type (the GIFTI reader's statistics overlays) has none, so
/// FreeSurfer drops the overlay; by default its own GIFTI intent is used (`reproduce`: dropped).
fn overlay_intent(g: &mut Globals, ov: &OverlayInfo) -> i32 {
    let intent = get_gifti_intent(ov.otype);
    if ov.otype == FS_MRISURFOVERLAY_STATS && is_stats_intent(ov.gifti_intent) {
        g.notices.add(
            &FS_524,
            format!(
                "MRISwriteGIFTICombined: a statistics overlay of the generic type written with its own intent {} (FreeSurfer drops it)",
                ov.gifti_intent
            ),
        );
        if !g.notices.reproduce() {
            return ov.gifti_intent;
        }
    }
    intent
}

/// `MRISwriteGIFTICombined(mris, poverlays, out_fname)`: the surface's arrays, then each overlay's
/// (its intent from its type, [`overlay_intent`]; its volume, name and shape type), in one GIFTI
/// file, as [`mris_write_gifti`]. FS-523, FS-524.
pub fn mris_write_gifti_combined(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    overlays: &[OverlayInfo],
    out_fname: &str,
) -> Result<i32, Fatal> {
    let mut image = surface_image(g, &sg.gifti, io, mris);
    let e = write_surface(g, sg, io, dir, mris, &mut image, out_fname)?;
    if e != NO_ERROR {
        return Ok(e);
    }
    for ov in overlays {
        let intent = overlay_intent(g, ov);
        let e = mris_write_gifti_intent_mri(
            g,
            sg,
            io,
            dir,
            mris,
            ov.overlaymri.as_ref(),
            &mut image,
            intent,
            out_fname,
            ov.foverlay.as_deref(),
            Some(&ov.shapedatatype),
        )?;
        if e != NO_ERROR {
            return Ok(e);
        }
    }
    Ok(validate_and_write(
        g,
        &mut sg.gifti,
        io,
        dir,
        image,
        out_fname,
        "MRISwriteGIFTICombined",
    ))
}

/// `getShapeStatIntentCount(fgifti, &nVertices, &nFaces)`: the number of the file's data arrays
/// that are neither the surface (`NIFTI_INTENT_POINTSET`, whose rows set `n_vertices`;
/// `NIFTI_INTENT_TRIANGLE`, whose rows set `n_faces`) nor labels, matrices, vectors or colours.
/// 0 after `getShapeStatIntentCount(): gifti_read_image() returned NULL`, gifticlib's check
/// (`... GIFTI file <f> is invalid!`), or a surface array of no rows or not 3 columns
/// (`getShapeStatIntentCount(): malformed coords data array in file <f>: num_vertices=<n>
/// num_cols=<c>`, `... malformed faces ...`). FS-395.
pub fn get_shape_stat_intent_count(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fgifti: &str,
    n_vertices: &mut i32,
    n_faces: &mut i32,
) -> Result<i32, Fatal> {
    let Some(image) = read_valid(g, sg, io, dir, fgifti, "getShapeStatIntentCount()")? else {
        return Ok(0);
    };
    let gx = &sg.gifti;
    let mut count = 0;
    for k in 0..image.num_da.max(0) as usize {
        let Some(da) = image.darray.get(k).and_then(|d| d.as_ref()) else {
            continue;
        };
        if da.intent == NIFTI_INTENT_POINTSET || da.intent == NIFTI_INTENT_TRIANGLE {
            let (rows, cols) = vertices_cols(gx, io, da);
            let what = if da.intent == NIFTI_INTENT_POINTSET {
                ("coords", "num_vertices")
            } else {
                ("faces", "num_faces")
            };
            if rows <= 0 || cols != 3 {
                let _ = writeln!(
                    io.err,
                    "getShapeStatIntentCount(): malformed {} data array in file {fgifti}: {}={} num_cols={}",
                    what.0, what.1, rows as i32, cols as i32
                );
                free(gx, io, image);
                return Ok(0);
            }
            if da.intent == NIFTI_INTENT_POINTSET {
                *n_vertices = rows as i32;
            } else {
                *n_faces = rows as i32;
            }
            continue;
        }
        if [
            NIFTI_INTENT_LABEL,
            NIFTI_INTENT_GENMATRIX,
            NIFTI_INTENT_VECTOR,
            NIFTI_INTENT_RGB_VECTOR,
            NIFTI_INTENT_RGBA_VECTOR,
        ]
        .contains(&da.intent)
        {
            continue;
        }
        count += 1;
    }
    free(gx, io, image);
    Ok(count)
}

/// `gifti_read_image(fname, 1)`, a version "1" made "1.0" (FS-395: a file without a Version
/// crashes FreeSurfer; by default read on, as gifticlib accepts it), and gifticlib's check: the
/// image, or `None` after `<who>: gifti_read_image() returned NULL` or `<who>: GIFTI file <f> is
/// invalid!` (standard error).
pub(crate) fn read_valid(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    who: &str,
) -> Result<Option<GiftiImage>, Fatal> {
    let path = dir.map_or_else(|| PathBuf::from(fname), |d| d.join(fname));
    let image = {
        let mut w = Renamed {
            w: Writers {
                out: &mut *io.out,
                err: &mut *io.err,
            },
            from: path.to_string_lossy().into_owned(),
            to: fname.to_owned(),
        };
        sg.gifti
            .read_image(&path, true, &FsGifti, &mut w, &mut g.notices)
    };
    let Some(mut image) = image else {
        let _ = writeln!(io.err, "{who}: gifti_read_image() returned NULL");
        return Ok(None);
    };
    match image.version.as_deref() {
        Some(b"1") => image.version = Some(GIFTI_XML_VERSION.as_bytes().to_vec()),
        Some(_) => {}
        None => {
            g.notices.add(
                &FS_395,
                format!("{fname}: a GIFTI file without a Version (FreeSurfer compares it through a null pointer)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
        }
    }
    let fix = g.notices.fix();
    let valid = {
        let mut w = Writers {
            out: &mut *io.out,
            err: &mut *io.err,
        };
        sg.gifti
            .valid_gifti_image(Some(&image), true, &FsGifti, fix, &mut w, &mut g.notices)
    };
    if !valid {
        let _ = writeln!(io.err, "{who}: GIFTI file {fname} is invalid!");
        free(&sg.gifti, io, image);
        return Ok(None);
    }
    Ok(Some(image))
}

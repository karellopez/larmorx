// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The pieces of `utils/gifti.cpp` that its volume functions ([`crate::gifti_vol`]) share with its
//! surface readers and writers (larmorx-freesurfer-surf's `gifti` and `gifti_write`, which call
//! them from here): `gifti_set_DA_value_2D` (213-336, file-local), `insertCommonMetaData`
//! (1416-1424), `readLabelTable` (2857-3006) with the colour entries it builds, and the steps every
//! reader and writer repeats (gifticlib's metadata, rows and columns, values, reading with the
//! version made "1.0" and the check, writing after the check, freeing), over gifticlib 1.12
//! (`larmorx_io::gifti`, track C's port) with FreeSurfer's two changes to it ([`FsGifti`]) and this
//! crate's `gifti_get_DA_value_2D` and `gifti_alloc_and_add_darray` ([`crate::gifti`]). Moved here
//! from larmorx-freesurfer-surf in relay 23 of track D (they were written in relays 21 and 22), so
//! that `MRIread` and `MRIwrite` reach the GIFTI volume functions without the surface crate.
//!
//! gifticlib's settings and persistent state are the run's (`Globals::gifti`: its globals `G` and
//! `GXD`, one per process as in FreeSurfer, shared by the volume and the surface functions).
//!
//! The defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-surfio.md` §23 and
//! §24): FS-389 (a negative label key), FS-393's neighbour FS-80 (a label's colours the file did not
//! give), FS-395 (a file without a Version), FS-17 (a value outside `int` made an integer).

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::cfmt::{Streams, Writers};
use larmorx_core::defects::{FS_80, FS_389, FS_395, Notices};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADFILE, ERROR_NONE, Fatal};
use larmorx_freesurfer_core::random::{random_number, set_random_seed};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::timer::current_date_time;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_io::gifti::{
    DataArray, Dialect, GIFTI_ENCODING_B64GZ, GIFTI_ENDIAN_LITTLE, GIFTI_IND_ORD_ROW_MAJOR,
    GIFTI_XML_VERSION, Gifti, GiftiImage, LabelTable, NIFTI_INTENT_POINTSET,
    NIFTI_INTENT_TIME_SERIES, NIFTI_TYPE_FLOAT32, NIFTI_TYPE_INT8, NIFTI_TYPE_INT16,
    NIFTI_TYPE_INT32, NIFTI_TYPE_UINT8, NIFTI_TYPE_UINT16, NIFTI_TYPE_UINT32, NvPairs,
    intent_to_string,
};

use crate::colortab::{CTAB_VERSION_TO_WRITE, ctab_find_duplicate_names};
use crate::gifti::{gifti_alloc_and_add_darray, gifti_get_da_value_2d};
use crate::streams::Io;

/// NIfTI-1's intent codes the reader distinguishes (`nifti1.h`).
pub const NIFTI_INTENT_LABEL: i32 = 1002;

pub const NIFTI_INTENT_GENMATRIX: i32 = 1004;

pub const NIFTI_INTENT_VECTOR: i32 = 1007;

pub const NIFTI_INTENT_NODE_INDEX: i32 = 2002;

pub const NIFTI_INTENT_RGB_VECTOR: i32 = 2003;

pub const NIFTI_INTENT_RGBA_VECTOR: i32 = 2004;

pub const NIFTI_INTENT_SHAPE: i32 = 2005;

/// `STRLEN`: the size of the surface's, the volume geometry's and a colour entry's names.
pub const STRLEN: usize = 1024;

/// FreeSurfer 8.2.0's two changes to gifticlib 1.12 (its `packages/gifti`): an image's version
/// is valid when `sscanf("%f")` reads the same `float` from it as from "1.0" (an absent version
/// reads 0, as "1.0" then does: valid), and a coordinate system is accepted under a
/// `NIFTI_INTENT_TIME_SERIES` array too.
pub struct FsGifti;

impl Dialect for FsGifti {
    fn check_version(&self, gim: &GiftiImage, whine: bool, out: &mut dyn Streams) -> i32 {
        let (mut v, mut x) = (0.0f32, 0.0f32);
        if let Some(s) = &gim.version {
            let mut errno = 0;
            if let Some(f) = scan_f32_errno(&mut errno, s) {
                v = f;
            }
            if let Some(f) = scan_f32_errno(&mut errno, GIFTI_XML_VERSION.as_bytes()) {
                x = f;
            }
        }
        if v != x {
            if whine {
                out.err(&format!(
                    "** invalid: gim version = {}, expected {}\n",
                    larmorx_core::cfmt::f(f64::from(v), 6),
                    larmorx_core::cfmt::f(f64::from(x), 6)
                ));
            }
            return 1;
        }
        0
    }

    fn coordsys_intent_message(&self, da_index: i32, intent: i32) -> Option<String> {
        (intent != NIFTI_INTENT_POINTSET && intent != NIFTI_INTENT_TIME_SERIES).then(|| {
            format!(
                "** DA[{da_index}] has coordsys with intent {} (should be {} or {})\n",
                intent_to_string(intent),
                intent_to_string(NIFTI_INTENT_POINTSET),
                intent_to_string(NIFTI_INTENT_TIME_SERIES)
            )
        })
    }
}

/// [`Streams`] that names a file as its caller did: `from` (the path the reader opened) written as
/// `to` (the name given).
pub struct Renamed<'a> {
    pub w: Writers<'a>,
    pub from: String,
    pub to: String,
}

impl Streams for Renamed<'_> {
    fn out(&mut self, text: &str) {
        let t = text.replace(&self.from, &self.to);
        self.w.out(&t);
    }
    fn err(&mut self, text: &str) {
        let t = text.replace(&self.from, &self.to);
        self.w.err(&t);
    }
}

pub fn scan_f32_errno(errno: &mut i32, s: &[u8]) -> Option<f32> {
    let mut b = Bytes::new(s);
    let mut sc = Scanf::new(&mut b);
    let v = sc.f(errno, true);
    sc.finish(errno);
    v
}

/// gifticlib's validity check reads the image's version and "1.0" with `sscanf("%f")` (FsGifti's
/// `check_version`), which leaves `errno` as glibc's `sscanf` does: the same two scans on the
/// program's `errno`.
fn version_scans(g: &mut Globals, image: &GiftiImage) {
    if let Some(v) = &image.version {
        scan_f32_errno(&mut g.errno, v);
        scan_f32_errno(&mut g.errno, GIFTI_XML_VERSION.as_bytes());
    }
}

/// `sscanf(s, "%f", &x) == 1`: the value, or `None`.
pub fn scan_f(g: &mut Globals, s: &[u8]) -> Option<f32> {
    scan_f32_errno(&mut g.errno, s)
}

/// `gifti_get_meta_value(nvp, name)`: the value's bytes, or `None` (`NULL`: no such entry, or an
/// entry without a value).
pub fn meta(
    gx: &Gifti,
    io: &mut Io,
    notices: &mut Notices,
    nvp: &NvPairs,
    name: &str,
) -> Option<Vec<u8>> {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gx.get_meta_value(nvp, name.as_bytes(), &mut w, notices)
        .and_then(|v| v.clone())
}

/// `gifti_DA_rows_cols(da, &rows, &cols)`: (rows, cols); a failure leaves `dims[0]` and 1.
pub fn rows_cols(gx: &Gifti, io: &mut Io, da: &DataArray) -> (i64, i64) {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    let (r, c, _) = gx.da_rows_cols(da, &mut w);
    (r, c)
}

/// The rows and columns of an array as the reader counts them: (rows, cols) for row-major order,
/// (cols, rows) swapped for column-major (`gifti_DA_rows_cols` called with its outputs exchanged).
pub fn vertices_cols(gx: &Gifti, io: &mut Io, da: &DataArray) -> (i64, i64) {
    let (r, c) = rows_cols(gx, io, da);
    if da.ind_ord == GIFTI_IND_ORD_ROW_MAJOR {
        (r, c)
    } else {
        (c, r)
    }
}

/// `gifti_get_DA_value_2D(da, row, col)` (fs.io's).
pub fn value(
    g: &mut Globals,
    io: &mut Io,
    da: &DataArray,
    row: i32,
    col: i32,
) -> Result<f64, Fatal> {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gifti_get_da_value_2d(g, &mut w, Some(da), row, col)
}

/// C's `strncpy(dst, src, STRLEN - 1)` into a zeroed buffer: the name up to its first NUL, at most
/// 1,023 bytes.
pub fn name_bytes(s: &[u8]) -> Vec<u8> {
    s.iter()
        .copied()
        .take_while(|&b| b != 0)
        .take(STRLEN - 1)
        .collect()
}

/// A colour component as the builders store it: the float, and `floor(c * 256)` made an `int`
/// (FS-17 beyond `int`), at most 255.
pub fn component(g: &mut Globals, c: f32, what: &str) -> (f32, i32) {
    let mut i = float_to_int(g, f64::from((c * 256.0).floor()), what);
    if i > 255 {
        i = 255;
    }
    (c, i)
}

pub fn entry(g: &mut Globals, name: Vec<u8>, rgba: [f32; 4], what: &str) -> ColorTableEntry {
    let (rf, ri) = component(g, rgba[0], what);
    let (gf, gi) = component(g, rgba[1], what);
    let (bf, bi) = component(g, rgba[2], what);
    let (af, ai) = component(g, rgba[3], what);
    ColorTableEntry {
        name,
        ri,
        gi,
        bi,
        ai,
        rf,
        gf,
        bf,
        af,
        tissue_type: 0,
        count: 0,
    }
}

/// The four colour components of the `k`-th colour set of a label table (gifticlib keeps the
/// colours of the labels that had them in order; a hole, memory gifticlib never wrote, reads 0
/// with FS-80's notice, as gifticlib's port reads it).
pub fn rgba_at(t: &LabelTable, k: usize, notices: &mut Notices) -> [f32; 4] {
    let mut v = [0.0f32; 4];
    let Some(rgba) = &t.rgba else { return v };
    let mut hole = false;
    for (i, x) in v.iter_mut().enumerate() {
        match rgba.get(4 * k + i).copied().flatten() {
            Some(c) => *x = c,
            None => hole = true,
        }
    }
    if hole {
        notices.add(
            &FS_80,
            "a label's colours that the file did not give read as 0 (memory never written)",
        );
    }
    v
}

/// `readLabelTable(image, fname)`: the image's label table as a colour table, `None` for a table
/// of no labels, or after `readLabelTable(<file>): invalid labeltable found` (gifticlib's check of
/// the table, its messages included). The table has one place per key from 0 to the largest key,
/// each label in the place of its key, empty places between; its name and colours (each
/// component `floor(c * 256)` at most 255 as the integers). A table without colours gets random
/// ones: the random generator seeded with 12 (`setRandomSeed`), three draws per label in [0, 1]
/// for red, green and blue, alpha 1. A key met again keeps its first label (`readLabelTable(<file>):
/// Duplicate labelkey <k>:<label>, was <name>` on standard output) and the colours that follow are
/// taken one set earlier (the colours advance with each new place only). With `Gdiag`'s
/// `DIAG_VERBOSE` the counts and each new place are printed (`[DEBUG] ...`). Then
/// [`ctab_find_duplicate_names`]. A negative key (FS-389): refused by default (`readLabelTable(<file>):
/// label key <k> is negative` on standard error, `None`); with `reproduce` a segmentation fault
/// (FreeSurfer reads the allocator's bookkeeping before the table as an entry).
pub fn read_label_table(
    g: &mut Globals,
    io: &mut Io,
    image: &mut GiftiImage,
    fname: &str,
) -> Result<Option<ColorTable>, Fatal> {
    if image.labeltable.is_empty() {
        return Ok(None);
    }
    let valid = {
        let mut w = Writers {
            out: &mut *io.out,
            err: &mut *io.err,
        };
        g.gifti
            .valid_label_table(&image.labeltable, true, &mut w, &mut g.notices)
    };
    if !valid {
        let _ = writeln!(io.err, "readLabelTable({fname}): invalid labeltable found");
        return Ok(None);
    }
    let n = image.labeltable.len();
    let maxkey = image.labeltable.key.iter().fold(-1, |m, &k| m.max(k));
    let mut ct = ColorTable {
        entries: vec![None; (maxkey + 1).max(0) as usize],
        fname: name_bytes(fname.as_bytes()),
        version: CTAB_VERSION_TO_WRITE,
        ..ColorTable::default()
    };
    if image.labeltable.rgba.is_none() {
        // random colours, the generator seeded with 12
        set_random_seed(g, 12);
        let mut rgba = Vec::with_capacity(4 * n);
        for _ in 0..n {
            for _ in 0..3 {
                let r = random_number(g, io.env, io.out, io.err, 0.0, 1.0)?;
                rgba.push(Some(r as f32));
            }
            rgba.push(Some(1.0f32));
        }
        image.labeltable.rgba = Some(rgba);
    }
    if g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(
            io.out,
            "[DEBUG] readLabelTable({fname}): ct->nentries={}, num_entries_to_read={n}",
            maxkey + 1
        );
    }
    let mut set = 0usize;
    for li in 0..n {
        let key = image.labeltable.key[li];
        let label = image.labeltable.label[li].clone().unwrap_or_default();
        if key < 0 {
            g.notices.add(
                &FS_389,
                format!("{fname}: label key {key} is negative (FreeSurfer writes its entry before the colour table)"),
            );
            if g.notices.reproduce() {
                // the place before the table holds the allocator's bookkeeping, read as an entry
                return Err(Fatal::Exit(139));
            }
            let _ = writeln!(
                io.err,
                "readLabelTable({fname}): label key {key} is negative"
            );
            return Ok(None);
        }
        let place = key as usize;
        if let Some(e) = &ct.entries[place] {
            let _ = writeln!(
                io.out,
                "readLabelTable({fname}): Duplicate labelkey {key}:{}, was {}",
                String::from_utf8_lossy(&label),
                String::from_utf8_lossy(&e.name)
            );
        } else {
            if g.gdiag & DIAG_VERBOSE != 0 {
                let _ = writeln!(
                    io.out,
                    "[DEBUG] readLabelTable({fname}): created ct->entries[{key}]"
                );
            }
            let rgba = rgba_at(&image.labeltable, set, &mut g.notices);
            let e = entry(g, name_bytes(&label), rgba, "readLabelTable");
            ct.entries[place] = Some(Box::new(e));
            set += 1;
        }
    }
    ctab_find_duplicate_names(io.out, &ct);
    Ok(Some(ct))
}

/// `(T) value` for an integer type of 8, 16 or 32 bits as x86-64 converts a `double`: through a
/// 32-bit `int` (`cvttsd2si`, FS-17 beyond its range), its low bits kept.
pub fn to_int_bits(g: &mut Globals, value: f64) -> i32 {
    float_to_int(g, value, "gifti_set_DA_value_2D")
}

/// `(unsigned int) value` as x86-64 converts a `double`: through a 64-bit integer
/// (`cvttsd2si` with a 64-bit result: beyond its range or NaN `INT64_MIN`), its low 32 bits.
pub fn to_uint32_bits(g: &mut Globals, value: f64) -> u32 {
    // 2^63: the 64-bit conversion's range
    let lim = (1u64 << 63) as f64;
    if value.is_nan() || value >= lim || value < -lim {
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
pub fn passwd_user_name(env: &Environment) -> Option<String> {
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
pub fn add_meta(
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
pub fn insert_common_meta_data(gx: &Gifti, io: &mut Io, notices: &mut Notices, md: &mut NvPairs) {
    if let Some(user) = passwd_user_name(io.env) {
        add_meta(gx, io, notices, md, "UserName", Some(user.as_bytes()));
    }
    let date = current_date_time(io.env, true);
    add_meta(gx, io, notices, md, "Date", Some(date.as_bytes()));
}

/// A new image: version "1.0", the common metadata.
pub fn new_image(gx: &Gifti, io: &mut Io, notices: &mut Notices) -> GiftiImage {
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
pub fn add_array(gx: &Gifti, io: &mut Io, image: &mut GiftiImage) -> Option<usize> {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gifti_alloc_and_add_darray(gx, Some(image), &mut w)?;
    Some(image.num_da as usize - 1)
}

/// The attributes every FreeSurfer writer gives an array: the intent and type, row-major order,
/// the dimensions, GZipBase64Binary, little-endian, no coordinate system.
pub fn set_attributes(
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
pub fn set_sizes(gx: &Gifti, io: &mut Io, fix: bool, da: &mut DataArray) {
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
pub fn alloc_data(da: &mut DataArray) {
    let n = (da.nvals.max(0) as usize).saturating_mul(da.nbyper.max(0) as usize);
    da.data = Some(vec![0; n]);
}

/// The end every writer shares: a version "1" made "1.0", gifticlib's validity check (its
/// messages, then `<who>: GIFTI file <name> is invalid!`, `ERROR_BADFILE`), `gifti_write_image`
/// with the data (`<who>: couldn't write image`, `ERROR_BADFILE`), the image freed (gifticlib's
/// messages at its verbosity 3 and above).
pub fn validate_and_write(
    g: &mut Globals,
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
        g.gifti
            .valid_gifti_image(Some(&image), true, &FsGifti, fix, &mut w, &mut g.notices)
    };
    version_scans(g, &image);
    if !valid {
        let _ = writeln!(io.err, "{who}: GIFTI file {out_fname} is invalid!");
        free(&g.gifti, io, image);
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
        g.gifti
            .write_image(&mut image, &path, true, &mut w, &mut g.notices)
    };
    if rc != 0 {
        // gifticlib's fopen left errno set when the file did not open
        if path.is_dir() {
            g.errno = 21;
        } else if path
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty() && !p.is_dir())
        {
            g.errno = if path.parent().is_some_and(|p| p.exists()) {
                20
            } else {
                2
            };
        }
        let _ = writeln!(io.err, "{who}: couldn't write image");
        free(&g.gifti, io, image);
        return ERROR_BADFILE;
    }
    free(&g.gifti, io, image);
    ERROR_NONE
}

pub fn free(gx: &Gifti, io: &mut Io, image: GiftiImage) {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gx.free_image(image, &mut w);
}

/// `gifti_read_image(fname, 1)`, a version "1" made "1.0" (FS-395: a file without a Version
/// crashes FreeSurfer; by default read on, as gifticlib accepts it), and gifticlib's check: the
/// image, or `None` after `<who>: gifti_read_image() returned NULL` or `<who>: GIFTI file <f> is
/// invalid!` (standard error).
pub fn read_valid(
    g: &mut Globals,
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
        g.gifti
            .read_image(&path, true, &FsGifti, &mut w, &mut g.notices)
    };
    let Some(mut image) = image else {
        // gifticlib's fopen left errno set when the file did not open
        if let Err(e) = std::fs::File::open(&path) {
            g.errno = larmorx_freesurfer_core::log::errno_of(&e).unwrap_or(5);
        }
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
        g.gifti
            .valid_gifti_image(Some(&image), true, &FsGifti, fix, &mut w, &mut g.notices)
    };
    version_scans(g, &image);
    if !valid {
        let _ = writeln!(io.err, "{who}: GIFTI file {fname} is invalid!");
        free(&g.gifti, io, image);
        return Ok(None);
    }
    Ok(Some(image))
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gifti.cpp`'s GIFTI surface reader (queue item 3.1-26, its surface part; relay 21 of
//! track D): `mrisReadGIFTIdanum` (424-1203) and `mrisReadGIFTIfile` (1217-1222), with the two
//! colour-table builders they use, `makeColorTable` (338-410, file-local) and `readLabelTable`
//! (2857-3006), over gifticlib 1.12 (`larmorx_io::gifti`, track C's port) with FreeSurfer's two
//! changes to it ([`FsGifti`]) and fs.io's helper `gifti_get_DA_value_2D` (track A's). Three small
//! functions of `utils/colortab.cpp` (fs.labels) are taken early here, as the reader calls them:
//! `CTABgetNumberOfValidEntries`, `CTABfindDuplicateNames`, `CTABrgb2Annotation`.
//!
//! gifticlib's settings and persistent state are the run's ([`SurfGlobals::gifti`]); messages go
//! to standard output and error as FreeSurfer prints them.
//!
//! The defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-surfio.md` §23):
//! FS-381 (the volume geometry's name copied as 1,024 bytes), FS-382 (more than 1,000 command
//! lines: the reader never returns), FS-383 (a face vertex outside the surface), FS-384 (a sparse
//! vertex number outside the surface), FS-385 (label arrays read as 32-bit unsigned integers),
//! FS-386 (a label key the colour table has no entry for), FS-387 (a coordinate system without
//! its spaces), FS-388 (a data array number outside the file's), FS-389 (a negative label key),
//! FS-390 (sparse arrays shorter than the surface: refused, kept), FS-391 and FS-392 (the shape
//! overlays' data type and name), FS-393 (the colours' alpha past the stored three), FS-394 (the
//! statistics overlays: kept until measured), FS-395 (a file without a Version), FS-396 (a label array without data); FS-17 (a value outside `int` made an integer),
//! FS-147 (a vertex of more than 255 faces), FS-356 (a file name of 1,024 bytes or more).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use larmorx_core::cfmt::{Streams, Writers};
use larmorx_core::defects::{
    FS_80, FS_356, FS_381, FS_382, FS_383, FS_384, FS_385, FS_386, FS_387, FS_388, FS_389, FS_390,
    FS_391, FS_392, FS_393, FS_394, FS_395, FS_396, Notices,
};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::diag::{self, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_core::random::{random_number, set_random_seed};
use larmorx_freesurfer_io::gifti::gifti_get_da_value_2d;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_io::gifti::{
    DataArray, Dialect, GIFTI_IND_ORD_ROW_MAJOR, GIFTI_XML_VERSION, Gifti, GiftiImage, LabelTable,
    NIFTI_INTENT_POINTSET, NIFTI_INTENT_TIME_SERIES, NIFTI_INTENT_TRIANGLE, NIFTI_TYPE_INT32,
    NIFTI_TYPE_UINT32, NvPairs, intent_to_string,
};

use crate::aaa::{MAX_CMDS, SurfGlobals, VERTICES_PER_FACE};
use crate::alloc;
use crate::coords::{self, Io};
use crate::formats::MRIS_TRIANGULAR_SURFACE;
use crate::mris::Mris;
use crate::mrisurf::add_command_line;
use crate::readwrite::{NO_HEMISPHERE, RIGHT_HEMISPHERE};
use crate::scanf::Scanf;
use crate::surfread::count_face;

/// `LEFT_HEMISPHERE` (`mrisurf.h` line 1068).
pub const LEFT_HEMISPHERE: i32 = 0;
/// NIfTI-1's intent codes the reader distinguishes (`nifti1.h`).
pub const NIFTI_INTENT_LABEL: i32 = 1002;
pub const NIFTI_INTENT_GENMATRIX: i32 = 1004;
pub const NIFTI_INTENT_VECTOR: i32 = 1007;
pub const NIFTI_INTENT_NODE_INDEX: i32 = 2002;
pub const NIFTI_INTENT_RGB_VECTOR: i32 = 2003;
pub const NIFTI_INTENT_RGBA_VECTOR: i32 = 2004;
pub const NIFTI_INTENT_SHAPE: i32 = 2005;
/// `FS_MRISURFOVERLAY_SHAPE`, `FS_MRISURFOVERLAY_STATS` (`MRISurfOverlay.h`).
pub const FS_MRISURFOVERLAY_SHAPE: i32 = 100;
pub const FS_MRISURFOVERLAY_STATS: i32 = 200;
/// `GIFTI_FILE` (`mri.h`).
pub const GIFTI_FILE: i32 = 27;
/// `CTAB_VERSION_TO_WRITE` (`colortab.h`).
pub const CTAB_VERSION_TO_WRITE: i32 = 2;
/// `NUM_VOLGEOM_META` (`gifti.cpp` line 40): the volume geometry's metadata entries.
const NUM_VOLGEOM_META: i32 = 19;
/// `STRLEN`: the size of the surface's, the volume geometry's and a colour entry's names.
const STRLEN: usize = 1024;

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

/// `OverlayInfoStruct` (`MRISurfOverlay.h`): one overlay the reader found (a shape array, or a
/// run of statistics arrays of one intent), with its values in a one-frame `MRI_FLOAT` volume of
/// one column per vertex.
#[derive(Debug)]
pub struct OverlayInfo {
    /// `__foverlay`: the shape's name (its `Name` metadata's last part without `.gii`), the file's
    /// name for statistics; `None` (`NULL`) for a shape without a name.
    pub foverlay: Option<Vec<u8>>,
    /// `__type`: [`FS_MRISURFOVERLAY_SHAPE`] or [`FS_MRISURFOVERLAY_STATS`].
    pub otype: i32,
    /// `__giftiIntent`.
    pub gifti_intent: i32,
    /// `__shapedatatype` (256 bytes in C): the shape's `ShapeDataType` metadata; empty for
    /// statistics (FS-391 for a shape without it).
    pub shapedatatype: Vec<u8>,
    /// `__format`: [`GIFTI_FILE`].
    pub format: i32,
    /// `__overlaymri`.
    pub overlaymri: Option<Mri>,
    /// `__nValsPerVertex` (1).
    pub n_vals_per_vertex: i32,
}

/// What `mrisReadGIFTIdanum` returned: `NULL`, the surface the caller passed (its data read into
/// it), or a new surface (the file held coordinates and faces; a surface the caller passed is then
/// left as it was, apart from what was read before the new one was made: nothing).
#[derive(Debug)]
pub enum GiftiRead {
    Null,
    Passed,
    New(Box<Mris>),
}

/// [`Streams`] that names a file as its caller did: `from` (the path the reader opened) written as
/// `to` (the name given).
struct Renamed<'a> {
    w: Writers<'a>,
    from: String,
    to: String,
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

fn gifti_cpp() -> String {
    format!("{}/utils/gifti.cpp", diag::BUILD_ROOT)
}

fn scan_f32_errno(errno: &mut i32, s: &[u8]) -> Option<f32> {
    let mut b = Bytes::new(s);
    let mut sc = Scanf::new(&mut b);
    let v = sc.f(errno, true);
    sc.finish(errno);
    v
}

/// `sscanf(s, "%d", &x) == 1`: the value, or `None`.
fn scan_d(g: &mut Globals, s: &[u8]) -> Option<i32> {
    let mut b = Bytes::new(s);
    let mut sc = Scanf::new(&mut b);
    let v = sc.d(&mut g.errno, true);
    sc.finish(&mut g.errno);
    v
}

/// `sscanf(s, "%f", &x) == 1`: the value, or `None`.
fn scan_f(g: &mut Globals, s: &[u8]) -> Option<f32> {
    scan_f32_errno(&mut g.errno, s)
}

/// `gifti_get_meta_value(nvp, name)`: the value's bytes, or `None` (`NULL`: no such entry, or an
/// entry without a value).
fn meta(
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
fn rows_cols(gx: &Gifti, io: &mut Io, da: &DataArray) -> (i64, i64) {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    let (r, c, _) = gx.da_rows_cols(da, &mut w);
    (r, c)
}

/// The rows and columns of an array as the reader counts them: (rows, cols) for row-major order,
/// (cols, rows) swapped for column-major (`gifti_DA_rows_cols` called with its outputs exchanged).
fn vertices_cols(gx: &Gifti, io: &mut Io, da: &DataArray) -> (i64, i64) {
    let (r, c) = rows_cols(gx, io, da);
    if da.ind_ord == GIFTI_IND_ORD_ROW_MAJOR {
        (r, c)
    } else {
        (c, r)
    }
}

/// `gifti_get_DA_value_2D(da, row, col)` (fs.io's).
fn value(g: &mut Globals, io: &mut Io, da: &DataArray, row: i32, col: i32) -> Result<f64, Fatal> {
    let mut w = Writers {
        out: &mut *io.out,
        err: &mut *io.err,
    };
    gifti_get_da_value_2d(g, &mut w, Some(da), row, col)
}

// ---- colortab.cpp's three functions, taken early ------------------------------------------------

/// `CTABgetNumberOfValidEntries(ct, &num)`: the entries that are not empty.
pub fn ctab_get_number_of_valid_entries(ct: &ColorTable) -> i32 {
    ct.entries.iter().filter(|e| e.is_some()).count() as i32
}

/// `CTABrgb2Annotation(r, g, b)`: `(b << 16) + (g << 8) + r` in `int` (wrapping as the official
/// build's shifts and sums do).
pub fn ctab_rgb2_annotation(r: i32, g: i32, b: i32) -> i32 {
    b.wrapping_shl(16)
        .wrapping_add(g.wrapping_shl(8))
        .wrapping_add(r)
}

/// `CTABfindDuplicateNames(ct)`: every ordered pair of entries with the same name gives `ERROR:
/// label name '<name>' is duplicated!` on standard output, then, when any did, `CTABfindDuplicateNames:
/// <n> duplicate names found in:` and the table's file name; returns the pairs' number.
pub fn ctab_find_duplicate_names(io: &mut Io, ct: &ColorTable) -> i32 {
    let mut dup = 0;
    for (i1, e1) in ct.entries.iter().enumerate() {
        let Some(e1) = e1 else { continue };
        for (i2, e2) in ct.entries.iter().enumerate() {
            if let Some(e2) = e2
                && i1 != i2
                && e1.name == e2.name
            {
                dup += 1;
                let _ = io.out.write_all(b"ERROR: label name '");
                let _ = io.out.write_all(&e1.name);
                let _ = io.out.write_all(b"' is duplicated!\n");
            }
        }
    }
    if dup > 0 {
        let _ = write!(
            io.out,
            "CTABfindDuplicateNames: {} duplicate names found in:\n  ",
            dup / 2
        );
        let _ = io.out.write_all(&ct.fname);
        let _ = io.out.write_all(b"\n");
    }
    dup / 2
}

/// C's `strncpy(dst, src, STRLEN - 1)` into a zeroed buffer: the name up to its first NUL, at most
/// 1,023 bytes.
fn name_bytes(s: &[u8]) -> Vec<u8> {
    s.iter()
        .copied()
        .take_while(|&b| b != 0)
        .take(STRLEN - 1)
        .collect()
}

/// A colour component as the builders store it: the float, and `floor(c * 256)` made an `int`
/// (FS-17 beyond `int`), at most 255.
fn component(g: &mut Globals, c: f32, what: &str) -> (f32, i32) {
    let mut i = float_to_int(g, f64::from((c * 256.0).floor()), what);
    if i > 255 {
        i = 255;
    }
    (c, i)
}

fn entry(g: &mut Globals, name: Vec<u8>, rgba: [f32; 4], what: &str) -> ColorTableEntry {
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

/// `makeColorTable(unique_annot_map, fname)`: a colour table of one entry per distinct annotation
/// of a colour array, in increasing annotation order, named `Unknown_Label_<k>`, each entry's
/// colours from the first vertex that had its annotation (the components as read: above 1 kept,
/// `floor(c * 256)` at most 255 as the integers); the alpha is not stored (FS-393: memory past the
/// three components; both modes 0, with a notice). Each entry printed on standard output as `<k>
/// <name> <annotation> (<r> <g> <b> <a>)`; then [`ctab_find_duplicate_names`]. Version 2, the
/// file's name (at most 1,023 bytes).
fn make_color_table(
    g: &mut Globals,
    io: &mut Io,
    map: &BTreeMap<i32, [f32; 3]>,
    fname: &str,
) -> ColorTable {
    let mut ct = ColorTable {
        entries: Vec::with_capacity(map.len()),
        fname: name_bytes(fname.as_bytes()),
        version: CTAB_VERSION_TO_WRITE,
        ..ColorTable::default()
    };
    if !map.is_empty() {
        g.notices.add(
            &FS_393,
            "the colour table of a colour array: each colour's alpha (memory past the three stored components) is 0",
        );
    }
    for (k, (annot, rgb)) in map.iter().enumerate() {
        let name = format!("Unknown_Label_{k}").into_bytes();
        let e = entry(g, name, [rgb[0], rgb[1], rgb[2], 0.0], "makeColorTable");
        let _ = writeln!(
            io.out,
            "{k} {} {annot} ({} {} {} {})",
            String::from_utf8_lossy(&e.name),
            e.ri,
            e.gi,
            e.bi,
            e.ai
        );
        ct.entries.push(Some(Box::new(e)));
    }
    ctab_find_duplicate_names(io, &ct);
    ct
}

/// The four colour components of the `k`-th colour set of a label table (gifticlib keeps the
/// colours of the labels that had them in order; a hole, memory gifticlib never wrote, reads 0
/// with FS-80's notice, as gifticlib's port reads it).
fn rgba_at(t: &LabelTable, k: usize, notices: &mut Notices) -> [f32; 4] {
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
    sg: &SurfGlobals,
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
        sg.gifti
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
    ctab_find_duplicate_names(io, &ct);
    Ok(Some(ct))
}

/// POSIX's `basename` (glibc's `__xpg_basename`, `<libgen.h>`) of a name: the last component
/// without trailing slashes; "." for an empty name; "/" for a name of slashes only.
fn posix_basename(s: &[u8]) -> Vec<u8> {
    if s.is_empty() {
        return b".".to_vec();
    }
    let end = s.iter().rposition(|&b| b != b'/').map(|i| i + 1);
    let Some(end) = end else {
        return b"/".to_vec();
    };
    let start = s[..end]
        .iter()
        .rposition(|&b| b == b'/')
        .map_or(0, |i| i + 1);
    s[start..end].to_vec()
}

/// The values of a label array as FreeSurfer reads them (FS-385): by default each value by the
/// array's data type made an integer (as the GIFTI format's INT32 keys are); with `reproduce`
/// four bytes per value as an unsigned integer whatever the type (a FLOAT32 value's bit pattern;
/// past the array's data zeros, memory FreeSurfer reads, with the notice).
fn label_key(
    g: &mut Globals,
    io: &mut Io,
    da: &DataArray,
    k: i32,
    fname: &str,
) -> Result<i32, Fatal> {
    let exact = matches!(da.datatype, NIFTI_TYPE_INT32 | NIFTI_TYPE_UINT32);
    if !exact {
        g.notices.add(
            &FS_385,
            format!(
                "{fname}: a label array of data type {} read as 32-bit unsigned integers",
                da.datatype
            ),
        );
    }
    if exact || g.notices.reproduce() {
        let data = da.data.as_deref().unwrap_or(&[]);
        let mut b = [0u8; 4];
        let at = (k.max(0) as usize).saturating_mul(4);
        for (i, x) in b.iter_mut().enumerate() {
            if let Some(&v) = data.get(at + i) {
                *x = v;
            }
        }
        return Ok(u32::from_le_bytes(b) as i32);
    }
    let v = value(g, io, da, k, 0)?;
    Ok(float_to_int(g, v, "mrisReadGIFTIdanum"))
}

/// One colour component of a colour array made an integer: above 1 the value itself, else
/// `floor(c * 256)` (FS-17 beyond `int`), at most 255.
fn rgb_int(g: &mut Globals, c: f32) -> i32 {
    let i = if c > 1.0 {
        float_to_int(g, f64::from(c), "mrisReadGIFTIdanum")
    } else {
        float_to_int(g, f64::from((c * 256.0).floor()), "mrisReadGIFTIdanum")
    };
    i.min(255)
}

fn fail(io: &mut Io, text: &str) -> Result<GiftiRead, Fatal> {
    let _ = io.err.write_all(text.as_bytes());
    Ok(GiftiRead::Null)
}

/// `mrisReadGIFTIdanum(fname, mris, daNum, poverlayinfo, ctab)`: reads a GIFTI file with gifticlib
/// (`gifti_read_image` with its data; a version "1" taken as "1.0"; gifticlib's validity check,
/// its messages included). Without both a coordinate array (`NIFTI_INTENT_POINTSET`) and a face
/// array (`NIFTI_INTENT_TRIANGLE`; the last of each counts) the data go into the surface the
/// caller passed (`NULL` and `mrisReadGIFTIdanum: mris is NULL! ...` without one); with them a
/// new triangular surface is made from the two (3 columns each, at least one row), its
/// hemisphere from the coordinates' `AnatomicalStructurePrimary` (`CortexRight`, `CortexLeft`),
/// volume geometry from the `VolGeom*` metadata (valid when all 19 are read), `SurfaceCenter*`,
/// `TAG_GROUP_AVG_SURFACE_AREA`, the command lines `TAG_CMDLINE#<k>` for `NUM_TAG_CMDLINE`, real
/// RAS for a coordinate system in `NIFTI_XFORM_SCANNER_ANAT`, the Talairach matrix from one
/// `NIFTI_XFORM_UNKNOWN` to `NIFTI_XFORM_TALAIRACH`. A label table becomes the surface's colour
/// table ([`read_label_table`]; the caller's `ctab` replaces it, `mrisReadGIFTIdanum: Use
/// Customized Colortable`), which must hold as many entries as the table has labels. Then each
/// data array (all, or only `da_num`'s): a node index (sparse storage, first array only), shape
/// data into the curvatures (the first only, unless overlays are asked for), labels into the
/// annotations, vectors into `dx dy dz`, colours into annotations with a colour table made from
/// them, statistics and the other intents into `val` and `stat` (the first only, unless overlays
/// are asked for); a general matrix is ignored with a warning. Every failure message goes to
/// standard error with `NULL` returned. The defects: see the module's.
#[allow(clippy::too_many_arguments)]
pub fn mris_read_gifti_danum(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    mut passed: Option<&mut Mris>,
    da_num: i32,
    mut overlays: Option<&mut Vec<OverlayInfo>>,
    ctab: Option<&ColorTable>,
) -> Result<GiftiRead, Fatal> {
    let path = dir.map_or_else(|| PathBuf::from(fname), |d| d.join(fname));
    let image = {
        // gifticlib's messages name the file as the caller did (the folder is larmorx's
        // stand-in for the working directory)
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
        return fail(
            io,
            &format!("mrisReadGIFTIdanum: ERROR reading gifti {fname}\n"),
        );
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
    let valid = {
        let fix = g.notices.fix();
        let mut w = Writers {
            out: &mut *io.out,
            err: &mut *io.err,
        };
        sg.gifti
            .valid_gifti_image(Some(&image), true, &FsGifti, fix, &mut w, &mut g.notices)
    };
    if !valid {
        return fail(
            io,
            &format!("mrisReadGIFTIdanum: GIFTI file {fname} is invalid!\n"),
        );
    }
    let mut ct = None;
    if !image.labeltable.is_empty() {
        ct = read_label_table(g, sg, io, &mut image, fname)?;
        if ct.is_none() {
            return Ok(GiftiRead::Null);
        }
    }
    let num_da = image.num_da.max(0) as usize;
    let image = image;
    let da = |k: usize| -> &DataArray {
        image.darray[k]
            .as_ref()
            .expect("a valid image's data array")
    };
    let mut coords = None;
    let mut faces = None;
    for k in 0..num_da {
        let intent = image.darray[k].as_ref().map_or(0, |d| d.intent);
        if intent == NIFTI_INTENT_POINTSET {
            coords = Some(k);
        } else if intent == NIFTI_INTENT_TRIANGLE {
            faces = Some(k);
        }
    }
    let mut new: Option<Box<Mris>> = None;
    if let (Some(ck), Some(fk)) = (coords, faces) {
        let coords = da(ck);
        let faces = da(fk);
        let (num_vertices, num_cols) = vertices_cols(&sg.gifti, io, coords);
        if num_vertices <= 0 || num_cols != 3 {
            return fail(
                io,
                &format!(
                    "mrisReadGIFTIdanum: malformed coords data array in file {fname}: num_vertices={} num_cols={}\n",
                    num_vertices as i32, num_cols as i32
                ),
            );
        }
        let (num_faces, num_cols) = vertices_cols(&sg.gifti, io, faces);
        if num_faces <= 0 || num_cols != 3 {
            return fail(
                io,
                &format!(
                    "mrisReadGIFTIdanum: malformed faces data array in file {fname}: num_faces={} num_cols={}\n",
                    num_faces as i32, num_cols as i32
                ),
            );
        }
        let mut mris = alloc::alloc(g, io.out, io.err, sg, num_vertices as i32, num_faces as i32)?;
        if fname.len() >= STRLEN {
            g.notices.add(
                &FS_356,
                format!("a GIFTI surface's file name of {} bytes kept whole (FreeSurfer writes it past the surface's 1,024-byte name)", fname.len()),
            );
        }
        mris.fname = fname.to_owned();
        mris.r#type = MRIS_TRIANGULAR_SURFACE;
        let hemi = meta(
            &sg.gifti,
            io,
            &mut g.notices,
            &coords.meta,
            "AnatomicalStructurePrimary",
        );
        mris.hemisphere = match hemi.as_deref() {
            Some(b"CortexRight") => RIGHT_HEMISPHERE,
            Some(b"CortexLeft") => LEFT_HEMISPHERE,
            _ => NO_HEMISPHERE,
        };
        mris.useRealRAS = 0;
        read_vol_geom(g, sg, io, coords, &mut mris, fname);
        if let Some(s) = meta(
            &sg.gifti,
            io,
            &mut g.notices,
            &coords.meta,
            "TAG_GROUP_AVG_SURFACE_AREA",
        ) && let Some(v) = scan_f(g, &s)
        {
            mris.group_avg_surface_area = v;
        }
        read_command_lines(g, sg, io, coords, &mut mris, fname);
        let file = gifti_cpp();
        for vno in 0..num_vertices as i32 {
            mris.topology_mut(vno as usize).num = 0;
            let x = value(g, io, coords, vno, 0)? as f32;
            let y = value(g, io, coords, vno, 1)? as f32;
            let z = value(g, io, coords, vno, 2)? as f32;
            coords::set_xyz(g, io, &mut mris, vno, x, y, z, &file, 713)?;
        }
        for fno in 0..num_faces as i32 {
            for k in 0..VERTICES_PER_FACE {
                let v = value(g, io, faces, fno, k as i32)?;
                let vi = float_to_int(g, v, "mrisReadGIFTIdanum");
                mris.face_mut(fno as usize).v[k] = vi;
                if vi < 0 || vi >= mris.nvertices {
                    g.notices.add(
                        &FS_383,
                        format!(
                            "{fname}: face {fno} names vertex {vi}, which the surface does not have"
                        ),
                    );
                    if !g.notices.reproduce() {
                        return fail(
                            io,
                            &format!(
                                "mrisReadGIFTIdanum: face {fno} vertex {k}: {vi} is not a vertex of the surface in file {fname}\n"
                            ),
                        );
                    }
                    continue;
                }
                if !count_face(g, &mut mris, vi as usize, fname) {
                    return fail(
                        io,
                        &format!(
                            "mrisReadGIFTIdanum: vertex {vi} is in more than 255 faces in file {fname}\n"
                        ),
                    );
                }
            }
        }
        for cs in coords.coordsys.iter().flatten() {
            let dataspace = cs.dataspace.as_deref();
            let Some(ds) = dataspace else {
                g.notices.add(
                    &FS_387,
                    format!("{fname}: a coordinate system without its DataSpace"),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                continue;
            };
            if ds == b"NIFTI_XFORM_SCANNER_ANAT" {
                mris.useRealRAS = 1;
            } else if ds == b"NIFTI_XFORM_UNKNOWN" {
                let Some(xs) = cs.xformspace.as_deref() else {
                    g.notices.add(
                        &FS_387,
                        format!("{fname}: a coordinate system without its TransformedSpace"),
                    );
                    if g.notices.reproduce() {
                        return Err(Fatal::Exit(139));
                    }
                    continue;
                };
                if xs == b"NIFTI_XFORM_TALAIRACH" {
                    let mut m = matrix_alloc(g, io.out, io.err, 4, 4, MATRIX_REAL)?
                        .expect("a 4 x 4 real matrix");
                    for r in 1..=4 {
                        for c in 1..=4 {
                            m.set_relt(r, c, cs.xform[r as usize - 1][c as usize - 1] as f32);
                        }
                    }
                    mris.SRASToTalSRAS_ = Some(m);
                }
            }
        }
        new = Some(Box::new(mris));
    }
    let target: &mut Mris = match (&mut new, &mut passed) {
        (Some(m), _) => m,
        (None, Some(m)) => m,
        (None, None) => {
            return fail(
                io,
                &format!("mrisReadGIFTIdanum: mris is NULL! found when parsing file {fname}\n"),
            );
        }
    };
    let result = |new: Option<Box<Mris>>| match new {
        Some(m) => GiftiRead::New(m),
        None => GiftiRead::Passed,
    };
    if let Some(user) = ctab {
        let _ = io
            .out
            .write_all(b"mrisReadGIFTIdanum: Use Customized Colortable\n");
        target.ct = Some(user.clone());
    } else if let Some(c) = &ct {
        target.ct = Some(c.clone());
    }
    if ct.is_some() {
        let n = target
            .ct
            .as_ref()
            .map_or(0, ctab_get_number_of_valid_entries);
        if n != image.labeltable.len() as i32 {
            let _ = writeln!(
                io.err,
                "mrisReadGIFTIdanum: ct_entries:{n} != labeltable_entries:{}",
                image.labeltable.len()
            );
            return Ok(GiftiRead::Null);
        }
    }
    let (start, end) = if da_num != -1 {
        (da_num, da_num + 1)
    } else {
        (0, image.num_da)
    };
    if start < 0 || end > image.num_da {
        g.notices.add(
            &FS_388,
            format!(
                "{fname}: data array {da_num} asked for, of {} arrays",
                image.num_da
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            io.err,
            "mrisReadGIFTIdanum: data array {da_num} is not in file {fname}"
        );
        return Ok(GiftiRead::Null);
    }
    let mut found_curv = 0;
    let mut found_statval = 0;
    let mut prev_stat_intent = -1;
    let mut stat_frame = 0;
    let mut stat_info: Option<OverlayInfo> = None;
    let mut node_index: Option<usize> = None;
    let mut num_index_nodes: i64 = 0;
    for k in start..end {
        let darray = da(k as usize);
        let intent = darray.intent;
        if intent == NIFTI_INTENT_POINTSET || intent == NIFTI_INTENT_TRIANGLE {
            continue;
        }
        if intent == NIFTI_INTENT_NODE_INDEX {
            if k != 0 {
                return fail(
                    io,
                    &format!(
                        "mrisReadGIFTIdanum: NODE_INDEX data array found but its not the first data array in file {fname}\n"
                    ),
                );
            }
            let (rows, cols) = vertices_cols(&sg.gifti, io, darray);
            num_index_nodes = rows;
            if rows <= 0 || rows > i64::from(target.nvertices) || cols > 1 {
                return fail(
                    io,
                    &format!(
                        "mrisReadGIFTIdanum: malformed NODE_INDEX data array in file {fname}: num_index_nodes={} num_cols={} max nvertices={}, num_cols>1\n",
                        rows as i32, cols as i32, target.nvertices
                    ),
                );
            }
            node_index = Some(k as usize);
            continue;
        }
        let expected: i64 = match intent {
            NIFTI_INTENT_VECTOR | NIFTI_INTENT_RGB_VECTOR => 3,
            NIFTI_INTENT_RGBA_VECTOR => 4,
            NIFTI_INTENT_GENMATRIX => 9,
            _ => 1,
        };
        let (nv, cols) = vertices_cols(&sg.gifti, io, darray);
        if nv <= 0 || nv != i64::from(target.nvertices) || cols > expected {
            if node_index.is_some() && nv == num_index_nodes {
                g.notices.add(
                    &FS_390,
                    format!("{fname}: a sparse data array of {nv} values (one per listed vertex) refused"),
                );
            }
            return fail(
                io,
                &format!(
                    "mrisReadGIFTIdanum: malformed data array [{k}] in file {fname}: num_vertices={} num_cols={} expected nvertices={}, num_cols={}\n",
                    nv as i32, cols as i32, target.nvertices, expected as i32
                ),
            );
        }
        let sparse = node_index.map(da);
        if intent == NIFTI_INTENT_SHAPE {
            if found_curv != 0 && overlays.is_none() {
                let _ = writeln!(
                    io.err,
                    "WARNING: a prior data array of shape data has already been read!  Skipping data in array #{k} in file {fname}"
                );
                continue;
            }
            found_curv += 1;
            let mut omri = None;
            if overlays.is_some() {
                let _ = writeln!(
                    io.err,
                    "INFO: {} data in array #{k} in file {fname} saved as MRI",
                    intent_to_string(intent)
                );
                omri = Some(Mri::new(
                    g,
                    io.out,
                    io.err,
                    &[target.nvertices, 1, 1, 1],
                    MRI_FLOAT,
                    true,
                )?);
            }
            let n = if sparse.is_some() {
                num_index_nodes as i32
            } else {
                target.nvertices
            };
            for i in 0..n {
                let (vno, ok) = match sparse {
                    Some(ni) => sparse_vno(g, io, ni, i, target.nvertices, fname)?,
                    None => (i, true),
                };
                if !ok {
                    if !g.notices.reproduce() {
                        return Ok(GiftiRead::Null);
                    }
                    continue;
                }
                let c = value(g, io, darray, i, 0)? as f32;
                target.vertex_mut(vno as usize).curv = c;
                if let Some(m) = &mut omri {
                    mri_set_vox_val(g, io.err, m, vno, 0, 0, 0, c, 0)?;
                }
            }
            let Some(list) = overlays.as_deref_mut() else {
                continue;
            };
            let base = meta(&sg.gifti, io, &mut g.notices, &darray.meta, "Name").map(|name| {
                let mut copy = name.clone();
                if copy.len() >= STRLEN {
                    g.notices.add(
                        &FS_392,
                        format!("{fname}: a shape name of {} bytes cut to 1,023 (FreeSurfer reads past its copy)", copy.len()),
                    );
                    copy.truncate(STRLEN - 1);
                }
                let mut b = posix_basename(&copy);
                if let Some(dot) = b.iter().rposition(|&c| c == b'.')
                    && &b[dot + 1..] == b"gii" {
                        b.truncate(dot);
                    }
                b
            });
            let sdt = meta(&sg.gifti, io, &mut g.notices, &darray.meta, "ShapeDataType");
            let shapedatatype = match sdt {
                Some(mut s) => {
                    if s.len() >= 256 {
                        g.notices.add(
                            &FS_391,
                            format!("{fname}: a shape data type of {} bytes cut to 255 (FreeSurfer writes past its 256-byte field)", s.len()),
                        );
                        s.truncate(255);
                    }
                    s
                }
                None => {
                    g.notices.add(
                        &FS_391,
                        format!("{fname}: a shape array without ShapeDataType: its data type empty (FreeSurfer's is memory contents)"),
                    );
                    Vec::new()
                }
            };
            list.push(OverlayInfo {
                foverlay: base,
                otype: FS_MRISURFOVERLAY_SHAPE,
                gifti_intent: NIFTI_INTENT_SHAPE,
                shapedatatype,
                format: GIFTI_FILE,
                overlaymri: omri,
                n_vals_per_vertex: 1,
            });
        } else if intent == NIFTI_INTENT_LABEL {
            let _ = writeln!(
                io.out,
                "parsing dataarray #{k} ({}) ...",
                intent_to_string(intent)
            );
            if target.ct.is_none() {
                return fail(io, "mrisReadGIFTIdanum: NULL colortable\n");
            }
            if darray.data.is_none() && target.nvertices > 0 {
                g.notices.add(
                    &FS_396,
                    format!("{fname}: a label array without data (FreeSurfer reads through a null pointer)"),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                return fail(
                    io,
                    &format!(
                        "mrisReadGIFTIdanum: label data array [{k}] has no data in file {fname}\n"
                    ),
                );
            }
            let (mut nindex, mut vno) = (0i32, 0i32);
            while vno < target.nvertices {
                let (da_index, store) = match sparse {
                    Some(ni) => {
                        let (v, ok) = sparse_vno(g, io, ni, nindex, target.nvertices, fname)?;
                        if !ok && !g.notices.reproduce() {
                            return Ok(GiftiRead::Null);
                        }
                        vno = v;
                        (nindex, ok)
                    }
                    None => (vno, true),
                };
                let key = label_key(g, io, darray, da_index, fname)?;
                let ti = image.labeltable.key.iter().position(|&t| t == key);
                let mut annotation = 0;
                if let Some(ti) = ti {
                    let ct = target.ct.as_ref().expect("a colour table");
                    let e = usize::try_from(key)
                        .ok()
                        .and_then(|p| ct.entries.get(p))
                        .and_then(|e| e.as_deref());
                    match e {
                        Some(e) => {
                            if g.gdiag & DIAG_VERBOSE != 0 {
                                let _ = io.out.write_all(
                                    format!("mrisReadGIFTIdanum(): vno: {vno}, tkey: {key}, tidx: {ti}, name: ")
                                        .as_bytes(),
                                );
                                let _ = io.out.write_all(&e.name);
                                let _ = io.out.write_all(b"\n");
                            }
                            annotation = ctab_rgb2_annotation(e.ri, e.gi, e.bi);
                        }
                        None => {
                            g.notices.add(
                                &FS_386,
                                format!("{fname}: label key {key} has no entry in the surface's colour table: no label (FreeSurfer reads outside the table)"),
                            );
                            if g.notices.reproduce() {
                                return Err(Fatal::Exit(139));
                            }
                        }
                    }
                }
                if store {
                    target.vertex_mut(vno as usize).annotation = annotation;
                }
                if sparse.is_some() {
                    nindex += 1;
                    if i64::from(nindex) >= num_index_nodes {
                        break;
                    }
                } else {
                    vno += 1;
                }
            }
        } else if intent == NIFTI_INTENT_VECTOR {
            for vno in 0..target.nvertices {
                let dx = value(g, io, darray, vno, 0)? as f32;
                let dy = value(g, io, darray, vno, 1)? as f32;
                let dz = value(g, io, darray, vno, 2)? as f32;
                let v = target.vertex_mut(vno as usize);
                (v.dx, v.dy, v.dz) = (dx, dy, dz);
            }
        } else if intent == NIFTI_INTENT_RGB_VECTOR || intent == NIFTI_INTENT_RGBA_VECTOR {
            let mut map: BTreeMap<i32, [f32; 3]> = BTreeMap::new();
            for vno in 0..target.nvertices {
                let red = value(g, io, darray, vno, 0)? as f32;
                let green = value(g, io, darray, vno, 1)? as f32;
                let blue = value(g, io, darray, vno, 2)? as f32;
                let (r, gg, b) = (rgb_int(g, red), rgb_int(g, green), rgb_int(g, blue));
                let annot = (r & 0xff) | ((gg & 0xff) << 8) | ((b & 0xff) << 16);
                target.vertex_mut(vno as usize).annotation = annot;
                map.entry(annot).or_insert([red, green, blue]);
            }
            target.ct = Some(make_color_table(g, io, &map, fname));
        } else if intent == NIFTI_INTENT_GENMATRIX {
            let _ = writeln!(
                io.err,
                "WARNING: ignoring unsupported data array NIFTI_INTENT_GENMATRIX in file {fname}"
            );
        } else {
            if found_statval != 0 && overlays.is_none() {
                let _ = writeln!(
                    io.err,
                    "WARNING: a prior data array of stat/val data has already been read!  Skipping data in array #{k} in file {fname}"
                );
                continue;
            }
            found_statval += 1;
            let mut omri = None;
            if overlays.is_some() {
                let _ = writeln!(
                    io.err,
                    "INFO: {} data in array #{k} in file {fname} saved as MRI",
                    intent_to_string(intent)
                );
                omri = Some(Mri::new(
                    g,
                    io.out,
                    io.err,
                    &[target.nvertices, 1, 1, 1],
                    MRI_FLOAT,
                    true,
                )?);
            }
            let n = if sparse.is_some() {
                num_index_nodes as i32
            } else {
                target.nvertices
            };
            for i in 0..n {
                let (vno, ok) = match sparse {
                    Some(ni) => sparse_vno(g, io, ni, i, target.nvertices, fname)?,
                    None => (i, true),
                };
                if !ok {
                    if !g.notices.reproduce() {
                        return Ok(GiftiRead::Null);
                    }
                    continue;
                }
                let a = value(g, io, darray, i, 0)? as f32;
                let b = value(g, io, darray, i, 0)? as f32;
                let v = target.vertex_mut(vno as usize);
                v.val = a;
                v.stat = b;
                if let Some(m) = &mut omri {
                    // the frame's number as the slice in the dense layout, as the frame in the
                    // sparse one (FS-394)
                    if sparse.is_some() {
                        mri_set_vox_val(g, io.err, m, vno, 0, 0, stat_frame, b, 0)?;
                    } else {
                        mri_set_vox_val(g, io.err, m, vno, 0, stat_frame, 0, b, 0)?;
                    }
                }
            }
            let Some(list) = overlays.as_deref_mut() else {
                continue;
            };
            if prev_stat_intent != intent {
                stat_frame = 0;
                if let Some(info) = stat_info.take() {
                    list.push(info);
                }
                stat_info = Some(OverlayInfo {
                    foverlay: Some(fname.as_bytes().to_vec()),
                    otype: FS_MRISURFOVERLAY_STATS,
                    gifti_intent: intent,
                    shapedatatype: Vec::new(),
                    format: GIFTI_FILE,
                    overlaymri: omri,
                    n_vals_per_vertex: 1,
                });
                prev_stat_intent = intent;
            } else {
                g.notices.add(
                    &FS_394,
                    format!("{fname}: statistics array {k} of a run of one intent: its overlay volume is not kept"),
                );
                stat_frame += 1;
            }
        }
    }
    if stat_info.take().is_some() {
        g.notices.add(
            &FS_394,
            format!("{fname}: the last run of statistics arrays is not added to the overlay list"),
        );
    }
    Ok(result(new))
}

/// The vertex number of a sparse array's `i`-th value (the node index's `i`-th, made an `int`)
/// and whether the surface has it (FS-384: a number it does not have is refused by default, with
/// its message; with `reproduce` the value is not stored, FreeSurfer's memory outside the surface).
fn sparse_vno(
    g: &mut Globals,
    io: &mut Io,
    ni: &DataArray,
    i: i32,
    nvertices: i32,
    fname: &str,
) -> Result<(i32, bool), Fatal> {
    let v = value(g, io, ni, i, 0)?;
    let vno = float_to_int(g, v, "mrisReadGIFTIdanum");
    if vno < 0 || vno >= nvertices {
        g.notices.add(
            &FS_384,
            format!("{fname}: node index {i} names vertex {vno}, which the surface does not have"),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                io.err,
                "mrisReadGIFTIdanum: node index {i}: {vno} is not a vertex of the surface in file {fname}"
            );
        }
        return Ok((vno, false));
    }
    Ok((vno, true))
}

/// The volume geometry from the coordinates' metadata (`VolGeomWidth` ... `VolGeomC_S` read with
/// `sscanf`'s `%d` and `%f`, `VolGeomFname` copied), valid when all 19 are read; then
/// `SurfaceCenterX/Y/Z`. FS-381: the name is copied as 1,024 bytes; both modes copy the name and
/// zeros after it (memory outside the metadata in FreeSurfer), its first 1,024 bytes when longer.
fn read_vol_geom(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    coords: &DataArray,
    mris: &mut Mris,
    fname: &str,
) {
    let mut n = 0;
    let get = |g: &mut Globals, io: &mut Io, name: &str| {
        meta(&sg.gifti, io, &mut g.notices, &coords.meta, name)
    };
    for (name, slot) in [
        ("VolGeomWidth", &mut mris.vg.width),
        ("VolGeomHeight", &mut mris.vg.height),
        ("VolGeomDepth", &mut mris.vg.depth),
    ] {
        if let Some(s) = get(g, io, name)
            && let Some(v) = scan_d(g, &s)
        {
            *slot = v;
            n += 1;
        }
    }
    let vg = &mut mris.vg;
    for (name, slot) in [
        ("VolGeomXsize", &mut vg.xsize),
        ("VolGeomYsize", &mut vg.ysize),
        ("VolGeomZsize", &mut vg.zsize),
        ("VolGeomX_R", &mut vg.x_r),
        ("VolGeomX_A", &mut vg.x_a),
        ("VolGeomX_S", &mut vg.x_s),
        ("VolGeomY_R", &mut vg.y_r),
        ("VolGeomY_A", &mut vg.y_a),
        ("VolGeomY_S", &mut vg.y_s),
        ("VolGeomZ_R", &mut vg.z_r),
        ("VolGeomZ_A", &mut vg.z_a),
        ("VolGeomZ_S", &mut vg.z_s),
        ("VolGeomC_R", &mut vg.c_r),
        ("VolGeomC_A", &mut vg.c_a),
        ("VolGeomC_S", &mut vg.c_s),
    ] {
        if let Some(s) = get(g, io, name)
            && let Some(v) = scan_f(g, &s)
        {
            *slot = v;
            n += 1;
        }
    }
    if let Some(s) = get(g, io, "VolGeomFname") {
        n += 1;
        g.notices.add(
            &FS_381,
            format!("{fname}: the volume geometry's name copied as 1,024 bytes: the bytes after it are 0 (FreeSurfer's are memory outside the metadata)"),
        );
        let buf = mris.vg.fname.buffer_mut();
        buf.fill(0);
        let k = s.len().min(STRLEN);
        buf[..k].copy_from_slice(&s[..k]);
    }
    if n == NUM_VOLGEOM_META {
        mris.vg.valid = 1;
    }
    for (name, slot) in [
        ("SurfaceCenterX", &mut mris.xctr),
        ("SurfaceCenterY", &mut mris.yctr),
        ("SurfaceCenterZ", &mut mris.zctr),
    ] {
        if let Some(s) = get(g, io, name)
            && let Some(v) = scan_f(g, &s)
        {
            *slot = v;
        }
    }
}

/// The command lines: `NUM_TAG_CMDLINE` (`%d`, 0 when absent), then `TAG_CMDLINE#<k>` for each,
/// added to the surface (`MRISaddCommandLine`); a missing one stops them (`[ERROR] TAG_CMDLINE out
/// of sync! No value found for TAG_CMDLINE#<k>` on standard output). Above 1,000: `[WARN]
/// mrisReadGIFTIdanum():  too many commands (<n>) in file. Only last 1000 will be saved!`, after
/// which FreeSurfer never returns (FS-382): both modes read the last 1,000 as the message says.
fn read_command_lines(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    coords: &DataArray,
    mris: &mut Mris,
    fname: &str,
) {
    let mut numcmds = 0;
    if let Some(s) = meta(
        &sg.gifti,
        io,
        &mut g.notices,
        &coords.meta,
        "NUM_TAG_CMDLINE",
    ) && let Some(v) = scan_d(g, &s)
    {
        numcmds = v;
    }
    let max = MAX_CMDS as i32;
    if numcmds > max {
        let _ = writeln!(
            io.out,
            "[WARN] mrisReadGIFTIdanum():  too many commands ({numcmds}) in file. Only last {max} will be saved!"
        );
    }
    let toskip = if numcmds > max { numcmds - max } else { 0 };
    if toskip != 0 {
        g.notices.add(
            &FS_382,
            format!(
                "{fname}: {numcmds} command lines: the last {max} read (FreeSurfer never returns)"
            ),
        );
    }
    for k in toskip..numcmds {
        let tag = format!("TAG_CMDLINE#{k}");
        let Some(cmd) = meta(&sg.gifti, io, &mut g.notices, &coords.meta, &tag) else {
            let _ = writeln!(
                io.out,
                "[ERROR] TAG_CMDLINE out of sync! No value found for {tag}"
            );
            break;
        };
        add_command_line(g, io.err, mris, &cmd);
    }
}

/// `mrisReadGIFTIfile(fname, mris, poverlayinfo, daNum, ctab)`: [`mris_read_gifti_danum`].
#[allow(clippy::too_many_arguments)]
pub fn mris_read_gifti_file(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    passed: Option<&mut Mris>,
    overlays: Option<&mut Vec<OverlayInfo>>,
    da_num: i32,
    ctab: Option<&ColorTable>,
) -> Result<GiftiRead, Fatal> {
    mris_read_gifti_danum(g, sg, io, dir, fname, passed, da_num, overlays, ctab)
}

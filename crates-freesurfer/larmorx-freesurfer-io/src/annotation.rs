// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The annotation tables (`utils/annotation.cpp`, `include/annotation.h`; item 3.1-33):
//! `read_named_annotation_table`, `read_annotation_table`, `annotation_to_index`,
//! `index_to_name`, `index_to_annotation`, `annotation_to_name`, `set_atable_from_ctable`,
//! `print_annotation_table`, `print_annotation_colortable`. FreeSurfer keeps one table per
//! process (`atable`, `num_entries`, and the global `annotation_table_file`); here it is the
//! caller's [`AnnotationTables`]. [`read_annot_as_mri_seg`] (`ReadAnnotAsMRISeg`, `mri_read`'s
//! annotation volumes) needs no surface; the functions on surfaces (`annotation2label`,
//! `MRISannot2seg`) come with fs.surf.
//!
//! D13: FS-430 (names of 100 bytes or more, table paths of 1,024 bytes or more: both modes keep
//! them whole, noted).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_430;
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BAD_PARM, ERROR_NO_FILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::ctab::ColorTable;

use larmorx_freesurfer_mri::mri::{self, MGH_VERSION, MGZ_INTENT_LABEL, MRI_INT, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::cfile::CFile;
use crate::colortab;
use crate::fio::{self, Slot49};
use crate::label::{STRLEN, fgetl};

/// The name field's size (`char name[100]`).
pub const NAME_LEN: usize = 100;

/// `ATABLE_ELT`: one entry of the table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AtableElt {
    pub index: i32,
    pub r: i32,
    pub g: i32,
    pub b: i32,
    pub annotation: i32,
    /// The name (a C string's bytes).
    pub name: Vec<u8>,
}

/// FreeSurfer's process-wide table: `atable`, `num_entries` and `annotation_table_file`.
#[derive(Clone, Debug, Default)]
pub struct AnnotationTables {
    pub atable: Vec<AtableElt>,
    /// `num_entries`: 0 until a table is read or set (a table of no entries stays 0).
    pub num_entries: i32,
    /// `annotation_table_file` (`None`: `NULL`, the default table).
    pub annotation_table_file: Option<Vec<u8>>,
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn name_fits(g: &mut Globals, what: &str, len: usize, room: usize) {
    if len >= room {
        g.notices.add(
            &FS_430,
            format!(
                "{what} of {len} bytes kept whole (FreeSurfer's buffer holds {})",
                room - 1
            ),
        );
    }
}

/// One table line read with `sscanf(cp, "%d %s %d %d %d %*d", ...)` into `e` (the fields it
/// does not reach keep their values).
fn scan_line(g: &mut Globals, e: &mut AtableElt, line: &[u8]) {
    let mut errno = g.errno;
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    if let Some(v) = sc.d(&mut errno, true) {
        e.index = v;
    }
    sc.text(&mut errno, b" ");
    if let Some(n) = sc.s(&mut errno, usize::MAX, true) {
        name_fits(g, "an annotation table's name", n.len(), NAME_LEN);
        e.name = n;
    }
    for v in [&mut e.r, &mut e.g, &mut e.b] {
        sc.text(&mut errno, b" ");
        if let Some(x) = sc.d(&mut errno, true) {
            *v = x;
        }
    }
    sc.text(&mut errno, b" ");
    let _ = sc.d(&mut errno, false);
    sc.finish(&mut errno);
    g.errno = errno;
    e.annotation =
        e.r.wrapping_add(e.g.wrapping_shl(8))
            .wrapping_add(e.b.wrapping_shl(16));
}

/// The table read from `fname` (the two readers' common part): `None` when it does not open
/// (`could not open translation file <name>` on standard error, `ERROR_NO_FILE`); else its
/// lines (read with `fgetl` and a 199-byte buffer) counted, `<label>(<shown>): <k> entries` on
/// standard output, each line read into its entry, `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
fn read_table(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    fname: &[u8],
    label: &str,
    shown: &[u8],
) -> i32 {
    name_fits(g, "an annotation table's path", fname.len(), STRLEN);
    let mut fp = match CFile::open(dir, &lossy(fname), "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let mut m = b"could not open translation file ".to_vec();
            m.extend_from_slice(fname);
            m.push(b'\n');
            let _ = err.write_all(&m);
            return ERROR_NO_FILE;
        }
    };
    t.num_entries = 0;
    loop {
        if fgetl(g, &mut fp, 199).is_none() {
            break;
        }
        t.num_entries += 1;
        if fp.feof() {
            break;
        }
    }
    let mut m = format!("{label}(").into_bytes();
    m.extend_from_slice(shown);
    m.extend_from_slice(format!("): {} entries\n", t.num_entries).as_bytes());
    let _ = out.write_all(&m);
    fp.rewind();
    t.atable = vec![AtableElt::default(); t.num_entries as usize];
    for i in 0..t.num_entries as usize {
        let Some(cp) = fgetl(g, &mut fp, 199) else {
            break;
        };
        let mut e = std::mem::take(&mut t.atable[i]);
        scan_line(g, &mut e, &cp);
        t.atable[i] = e;
    }
    // FreeSurfer leaves the file open
    NO_ERROR
}

/// `read_named_annotation_table(name)`: `NO_ERROR` at once when a table is already there
/// (`num_entries` not 0); else the table read from `$FREESURFER_HOME/<name>` (`.` without
/// `FREESURFER_HOME`) when `name` holds no `/`, else from `name`, announced as
/// `read_named_annotation_table(<name>): <k> entries`.
#[allow(clippy::too_many_arguments)]
pub fn read_named_annotation_table(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    name: &[u8],
) -> i32 {
    if t.num_entries != 0 {
        return NO_ERROR;
    }
    let fname = if name.contains(&b'/') {
        name.to_vec()
    } else {
        let mut f = env
            .get("FREESURFER_HOME")
            .map(|v| v.to_string_lossy().into_owned().into_bytes())
            .unwrap_or_else(|| b".".to_vec());
        f.push(b'/');
        f.extend_from_slice(name);
        f
    };
    read_table(
        g,
        out,
        err,
        dir,
        t,
        &fname,
        "read_named_annotation_table",
        name,
    )
}

/// `read_annotation_table()`: `NO_ERROR` at once when a table is already there; else the table
/// read from `annotation_table_file`, or (`None`) `$FREESURFER_HOME/Simple_surface_labels2009.txt`
/// (`.` without `FREESURFER_HOME`), announced as `read_annotation_table(<path>): <k> entries`.
pub fn read_annotation_table(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
) -> i32 {
    if t.num_entries != 0 {
        return NO_ERROR;
    }
    let fname = match &t.annotation_table_file {
        Some(f) => f.clone(),
        None => {
            let mut f = env
                .get("FREESURFER_HOME")
                .map(|v| v.to_string_lossy().into_owned().into_bytes())
                .unwrap_or_else(|| b".".to_vec());
            f.extend_from_slice(b"/Simple_surface_labels2009.txt");
            f
        }
    };
    read_table(
        g,
        out,
        err,
        dir,
        t,
        &fname,
        "read_annotation_table",
        &fname.clone(),
    )
}

/// The table, read first ([`read_annotation_table`]) when `num_entries` is 0 or less.
fn ensure(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
) {
    if t.num_entries <= 0 {
        read_annotation_table(g, env, out, err, dir, t);
    }
}

fn entries(t: &AnnotationTables) -> &[AtableElt] {
    &t.atable[..(t.num_entries.max(0) as usize).min(t.atable.len())]
}

/// `annotation_to_index(annotation)`: the index of the first entry of that annotation, or -1.
pub fn annotation_to_index(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    annotation: i32,
) -> i32 {
    ensure(g, env, out, err, dir, t);
    entries(t)
        .iter()
        .find(|e| e.annotation == annotation)
        .map_or(-1, |e| e.index)
}

/// `index_to_name(index)`: the name of the first entry of that index, `NOT_FOUND`, or the
/// index's digits for a negative `num_entries`.
pub fn index_to_name(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    index: i32,
) -> Vec<u8> {
    ensure(g, env, out, err, dir, t);
    if t.num_entries < 0 {
        return index.to_string().into_bytes();
    }
    entries(t)
        .iter()
        .find(|e| e.index == index)
        .map_or_else(|| b"NOT_FOUND".to_vec(), |e| e.name.clone())
}

/// `index_to_annotation(index)`: the annotation of the first entry of that index, or -1.
pub fn index_to_annotation(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    index: i32,
) -> i32 {
    ensure(g, env, out, err, dir, t);
    entries(t)
        .iter()
        .find(|e| e.index == index)
        .map_or(-1, |e| e.annotation)
}

/// `annotation_to_name(annotation, &index)`: the name of the first entry of that annotation and
/// its index; `NOT_FOUND` and -1; the annotation's digits and -1 for a negative `num_entries`.
#[allow(clippy::too_many_arguments)]
pub fn annotation_to_name(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    annotation: i32,
) -> (Vec<u8>, i32) {
    ensure(g, env, out, err, dir, t);
    if t.num_entries < 0 {
        return (annotation.to_string().into_bytes(), -1);
    }
    entries(t)
        .iter()
        .find(|e| e.annotation == annotation)
        .map_or_else(
            || (b"NOT_FOUND".to_vec(), -1),
            |e| (e.name.clone(), e.index),
        )
}

/// `set_atable_from_ctable(pct)`: `ERROR_BAD_PARM` for no table; `NO_ERROR` at once when a
/// table is already there (`num_entries` above 0); `num_entries` the colour table's entry count
/// (`ERROR_BAD_PARM` for 0); `set_atable_from_ctable(): <k> entries`; each entry of the colour
/// table gives its place's index, name (cut at 100 bytes: FS-430 for longer ones), colour and
/// annotation; empty places stay zeros. `NO_ERROR`.
pub fn set_atable_from_ctable(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut AnnotationTables,
    pct: Option<&ColorTable>,
) -> i32 {
    let Some(pct) = pct else {
        return ERROR_BAD_PARM;
    };
    if t.num_entries > 0 {
        return NO_ERROR;
    }
    t.num_entries = pct.nentries();
    if t.num_entries <= 0 {
        return ERROR_BAD_PARM;
    }
    let _ = writeln!(out, "set_atable_from_ctable(): {} entries", t.num_entries);
    t.atable = vec![AtableElt::default(); t.num_entries as usize];
    for i in 0..t.num_entries {
        if pct.entries[i as usize].is_none() {
            continue;
        }
        let e = &mut t.atable[i as usize];
        e.index = i;
        if let Ok(n) = colortab::ctab_copy_name(g, err, pct, i, usize::MAX) {
            name_fits(g, "a colour table's name", n.len(), NAME_LEN);
            t.atable[i as usize].name = n;
        }
        if let Ok((r, gg, b)) = colortab::ctab_rgb_at_indexi(g, err, pct, i) {
            let e = &mut t.atable[i as usize];
            (e.r, e.g, e.b) = (r, gg, b);
        }
        if let Ok(a) = colortab::ctab_annotation_at_index(g, err, pct, i) {
            t.atable[i as usize].annotation = a;
        }
    }
    NO_ERROR
}

/// `print_annotation_table(fp)`: each entry `%3d   %s` (the table read first when needed). 0.
#[allow(clippy::too_many_arguments)]
pub fn print_annotation_table(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    fp: &mut dyn Write,
) -> i32 {
    ensure(g, env, out, err, dir, t);
    for e in entries(t) {
        let mut l = format!("{:3}   ", e.index).into_bytes();
        l.extend_from_slice(&e.name);
        l.push(b'\n');
        let _ = fp.write_all(&l);
    }
    0
}

/// `print_annotation_colortable(fp)`: each entry `%3d   %-40s  %3d %3d %3d  0`. 0.
#[allow(clippy::too_many_arguments)]
pub fn print_annotation_colortable(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut AnnotationTables,
    fp: &mut dyn Write,
) -> i32 {
    ensure(g, env, out, err, dir, t);
    for e in entries(t) {
        let mut l = format!("{:3}   ", e.index).into_bytes();
        l.extend_from_slice(&e.name);
        l.resize(l.len() + 40usize.saturating_sub(e.name.len()), b' ');
        l.extend_from_slice(format!("  {:3} {:3} {:3}  0\n", e.r, e.g, e.b).as_bytes());
        let _ = fp.write_all(&l);
    }
    0
}

/// `ReadAnnotAsMRISeg(annotfname, volume_flag)`: an annotation file (`<vertices>`, then a vertex
/// number and an annotation value per vertex, all big-endian `int`s, then the tag 1 and a binary
/// colour table) as a volume of `<vertices>` x 1 x 1 `int`s, intent label; with `volume_flag` each
/// vertex's value is its annotation's entry in the table (0 when the table has no entry of that
/// colour), and `ReadAnnotAsMRISeg(): nhits = <n>` (the vertices whose annotation is not 0) is
/// printed. Messages on standard output, `None` after them: `ERROR: could not read <f>`,
/// `ERROR: ReadAnnotAsMRISeg(): vno = <v> >= num = <n>`, `ERROR: ReadAnnotAsMRISeg(): no color
/// table found in <f>` (the file ends before the tag); a table read prints `colortable with <n>
/// entries read (originally <name>)`. A tag other than 1 leaves the volume without a table. A
/// vertex number below 0 is written outside the volume (FS-53). The numbers the file cuts are
/// memory contents (FS-49).
pub fn read_annot_as_mri_seg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    annotfname: &str,
    volume_flag: bool,
) -> Result<Option<Mri>, Fatal> {
    let mut fp = match CFile::open(dir, annotfname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(out, "ERROR: could not read {annotfname}");
            return Ok(None);
        }
    };
    // the freadInt calls of this function share their memory (FS-49); the allocation and each
    // voxel write between two of them leave it unknown
    let mut slot = Slot49::new();
    let num = fio::fread_int_in(g, &mut fp, &mut slot);
    let mut m = if volume_flag {
        mri::mri_alloc(g, out, err, num, 1, 1, MRI_INT)?
    } else {
        mri::mri_alloc_header(g, out, err, num, 1, 1, MRI_INT, 1)?
    };
    slot.clobber();
    m.intent = MGZ_INTENT_LABEL;
    m.version = ((MGZ_INTENT_LABEL & 0xffff) << 8) | MGH_VERSION;
    for _ in 0..num.max(0) {
        let vno = fio::fread_int_in(g, &mut fp, &mut slot);
        if vno >= num {
            let _ = writeln!(
                out,
                "ERROR: ReadAnnotAsMRISeg(): vno = {vno} >= num = {num}"
            );
            return Ok(None);
        }
        let i = fio::fread_int_in(g, &mut fp, &mut slot);
        if volume_flag {
            mri_set_vox_val(g, err, &mut m, vno, 0, 0, 0, i as f32, 0)?;
            slot.clobber();
        }
    }
    let tag = fio::fread_int_in(g, &mut fp, &mut slot);
    if fp.feof() {
        let _ = writeln!(
            out,
            "ERROR: ReadAnnotAsMRISeg(): no color table found in {annotfname}"
        );
        fp.close();
        return Ok(None);
    }
    if tag == crate::tags::TAG_OLD_COLORTABLE {
        m.ct = crate::colortab_bin::ctab_read_from_binary(g, out, err, Some(&mut fp));
        if let Some(ct) = &m.ct {
            let mut l = format!(
                "colortable with {} entries read (originally ",
                ct.nentries()
            )
            .into_bytes();
            l.extend_from_slice(&ct.fname);
            l.extend_from_slice(b")\n");
            let _ = out.write_all(&l);
        }
    }
    fp.close();
    if volume_flag && let Some(ct) = m.ct.take() {
        let mut nhits = 0;
        for j in 0..num.max(0) {
            let v = mri_get_vox_val(g, err, &m, j, 0, 0, 0, 0)?;
            let annot = float_to_int(g, f64::from(v), "ReadAnnotAsMRISeg");
            if annot != 0 {
                nhits += 1;
            }
            let mut index = colortab::ctab_find_index_from_annotation(&ct, annot);
            if index == -1 {
                index = 0;
            }
            mri_set_vox_val(g, err, &mut m, j, 0, 0, 0, index as f32, 0)?;
        }
        m.ct = Some(ct);
        let _ = writeln!(out, "ReadAnnotAsMRISeg(): nhits = {nhits}");
    }
    Ok(Some(m))
}

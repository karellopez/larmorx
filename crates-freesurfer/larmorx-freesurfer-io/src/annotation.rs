// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The annotation tables (`utils/annotation.cpp`, `include/annotation.h`; item 3.1-33):
//! `read_named_annotation_table`, `read_annotation_table`, `annotation_to_index`,
//! `index_to_name`, `index_to_annotation`, `annotation_to_name`, `set_atable_from_ctable`,
//! `print_annotation_table`, `print_annotation_colortable`. FreeSurfer keeps one table per
//! process (`atable`, `num_entries`, and the global `annotation_table_file`); here it is the
//! caller's [`AnnotationTables`]. The functions on surfaces (`annotation2label`,
//! `MRISannot2seg`, `ReadAnnotAsMRISeg`) come with fs.surf.
//!
//! D13: FS-430 (names of 100 bytes or more, table paths of 1,024 bytes or more: both modes keep
//! them whole, noted).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_430;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BAD_PARM, ERROR_NO_FILE, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::ctab::ColorTable;

use crate::cfile::CFile;
use crate::colortab;
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
    e.annotation = e
        .r
        .wrapping_add(e.g.wrapping_shl(8))
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
    read_table(g, out, err, dir, t, &fname, "read_annotation_table", &fname.clone())
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
        .map_or_else(|| (b"NOT_FOUND".to_vec(), -1), |e| (e.name.clone(), e.index))
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
        for _ in e.name.len()..40 {
            l.push(b' ');
        }
        l.extend_from_slice(format!("  {:3} {:3} {:3}  0\n", e.r, e.g, e.b).as_bytes());
        let _ = fp.write_all(&l);
    }
    0
}

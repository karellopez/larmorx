// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The stats tables of `mri_convert --in_stats_table` and `--out_stats_table` (queue item 3.2-04,
//! taken from 3.2-07's `utils/stats.cpp`): `STAT_TABLE`, `LoadStatTable` with its three readers
//! (`LoadStatTableFS`, `LoadStatTableCSV`, `LoadStatTableTSV`), `AllocStatTable`,
//! `WriteStatTable`, `PrintStatTable` and `InitStatTableFromMRI`, with `utils/fsgdf.cpp`'s
//! `gdfCountItemsInString`.
//!
//! A table (the output of `asegstats2table` or `aparcstats2table`) has a first line of a measure
//! and the column names, then one line per row: its name and a number per column. Its volume is
//! `float`, one column per voxel along x (y and z of one voxel), one row per frame, voxels of size
//! 1 with the identity direction cosines (set over the allocation's own, its cached matrices left
//! as they were).
//!
//! - **FreeSurfer's format** (`LoadStatTableFS`): the columns are the first line's items (runs of
//!   characters other than blanks and tabs; a newline or carriage return starting an item ends the
//!   line) less one, the rows every later line; then the file is read again as words and numbers
//!   (`fscanf`'s `%s` and `%lf`, across line ends): the measure, the column names, and each row's
//!   name and numbers. `ERROR: format:  <file>` when there is no column, `ERROR: format: <file> at
//!   row <r>, col <c>` when a number cannot be read.
//! - **CSV** (`.csv`, any case; `LoadStatTableCSV`): [`crate::mri_convert::csv`]'s rows; the first
//!   gives the measure and the column names, every later one a row (its first field the name);
//!   each number `std::stof` of its field (a `float`: the longest prefix `strtof` reads after
//!   white space, an error for none or beyond the `float` range, `ERANGE`).
//! - **TSV** (`.tsv`, `.tac`; `LoadStatTableTSV`): `TSV::read` (fs.io); the rows are named `row`,
//!   the measure `UnknownUnits`; a `.tac` file's first two columns (its times) are left out, and
//!   its measure is `kBq/cc` when its second column is `end[kBq/cc]`.
//!
//! `LoadStatTable` prints `LoadStatTable(): ext <extension>` and `Loading table as ...`, each
//! reader `Found <n> data colums` and `Found <n> data rows`, on standard output.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-convert.md`):
//! - **FS-948** (kept): counting a line's items reads past the line's end, beyond its terminating
//!   zero, until a blank or a tab in memory: the count is never changed (no notice).
//! - **FS-949**: a `.csv` table is read twice, as CSV and then in FreeSurfer's format, whose
//!   result is kept (a CSV table fails, or gives another table): by default the CSV table
//!   (rule A); `reproduce` FreeSurfer's.
//! - **FS-950**: the TSV reader writes its volume to `junk.mgz` in the working directory: by
//!   default not (rule A); `reproduce` writes it.
//! - **FS-1031** (crash): a name without an extension: both modes print `LoadStatTable(): ext
//!   (null)` and read the table in FreeSurfer's format, noted.
//! - **FS-1032** (memory): an empty file (or one that cannot be read) in FreeSurfer's format: its
//!   first line is memory contents; both modes read it as an empty line, noted.
//! - **FS-1033**: FreeSurfer's format counts rows as the pieces its 100,000-byte line buffer
//!   reads: a blank line counts as a row (the table then fails, or a template has too many rows),
//!   a line longer than 99,999 bytes counts as several and only the first piece of the first line
//!   is counted for the columns. By default rows are the lines that hold an item, read whole
//!   (rule A); `reproduce` FreeSurfer's.
//! - **FS-1034** (memory): a word longer than 99,999 bytes overflows the reader's buffer; both
//!   modes read it whole, noted.
//! - **FS-1035** (crash): a CSV table without lines; both modes `ERROR: format:  <file>`.
//! - **FS-1036** (memory): a CSV row with fewer fields than the first line reads past the row;
//!   both modes `ERROR: format: <file> at row <r>, col <c>`.
//! - **FS-1037** (crash): a `.tac` table with fewer than two columns; both modes `ERROR: format:
//!   <file>`.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::cscan::strtof;
use larmorx_core::defects::{
    FS_949, FS_950, FS_1031, FS_1032, FS_1033, FS_1034, FS_1035, FS_1036, FS_1037,
};
use larmorx_freesurfer_core::cscan::{self as fscan, Bytes, Scan, Scanf, Stream};
use larmorx_freesurfer_core::fio::Tsv;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{fio_extension, tsv_read};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use super::csv::read_csv;

/// The size of the readers' line and word buffer (`char tmpstr[100000]`).
pub const TMPSTR: usize = 100_000;

/// `STAT_TABLE`. A name the C table leaves `NULL` is `None` (printed `(null)`, as glibc prints a
/// `NULL` string).
#[derive(Debug, Default)]
pub struct StatTable {
    pub filename: Option<String>,
    pub measure: Option<Vec<u8>>,
    pub nrows: i32,
    pub ncols: i32,
    pub colnames: Vec<Option<Vec<u8>>>,
    pub rownames: Vec<Option<Vec<u8>>>,
    pub data: Vec<Vec<f64>>,
    pub mri: Option<Mri>,
}

/// What the table functions print to and read from.
pub struct Io<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b Path>,
}

fn put(out: &mut dyn Write, parts: &[&[u8]]) {
    for p in parts {
        let _ = out.write_all(p);
    }
}

/// A C string's bytes: up to its first zero.
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `gdfCountItemsInString(str)`: the runs of characters other than blanks and tabs (`isblank`) up
/// to the string's end; a newline or carriage return where an item would start ends the count
/// (inside an item it belongs to the item). FS-948: FreeSurfer's loop over the last item reads past
/// the string's terminating zero to the next blank or tab in memory; the count is the same.
pub fn gdf_count_items_in_string(s: &[u8]) -> i32 {
    let s = c_str(s);
    let len = s.len();
    let blank = |c: u8| c == b' ' || c == b'\t';
    let (mut n, mut nhits) = (0, 0);
    while n < len {
        while n < len && blank(s[n]) {
            n += 1;
        }
        if n >= len || s[n] == b'\n' || s[n] == b'\r' {
            break;
        }
        while n < len && !blank(s[n]) {
            n += 1;
        }
        nhits += 1;
    }
    nhits
}

/// The readers' `char tmpstr[100000]`: the C string it holds (what `fgets` and `fscanf("%s")` last
/// stored; a call that stores nothing leaves it), and the file it reads.
struct Buffer {
    s: Vec<u8>,
}

/// `fgets(tmpstr, 100000, fp)`: the next line (with its newline). With `reproduce`, FreeSurfer's
/// piece of at most 99,999 bytes; by default the whole line (FS-1033, noted where a line is longer).
fn fgets_line(g: &mut Globals, fp: &mut CFile, b: &mut Buffer, what: &str) -> bool {
    let piece = if g.notices.reproduce() {
        fp.fgets(TMPSTR)
    } else {
        fp.fgets(usize::MAX)
    };
    let Some(line) = piece else {
        return false;
    };
    let cut = if g.notices.reproduce() {
        line.len() == TMPSTR - 1 && line.last() != Some(&b'\n') && fp.peek().is_some()
    } else {
        line.len() > TMPSTR - 1
    };
    if cut {
        g.notices.add(
            &FS_1033,
            format!(
                "{what}: a line longer than 99,999 bytes (FreeSurfer reads it as several, of 99,999 bytes)"
            ),
        );
    }
    b.s = c_str(&line).to_vec();
    true
}

/// The first line's column count (`gdfCountItemsInString - 1`): an empty file's first line is
/// memory contents in FreeSurfer (FS-1032), an empty line here.
fn first_line_columns(g: &mut Globals, fp: &mut CFile, b: &mut Buffer, what: &str) -> i32 {
    if !fgets_line(g, fp, b, what) {
        g.notices.add(
            &FS_1032,
            format!(
                "{what}: an empty file (or one that cannot be read): FreeSurfer counts the columns of memory contents; an empty line here"
            ),
        );
        b.s.clear();
    }
    gdf_count_items_in_string(&b.s) - 1
}

/// `while (fgets(tmpstr, 100000, fp) != NULL) nrows++`: with `reproduce` every piece; by default the
/// lines that hold an item (FS-1033, noted where a blank line counted).
fn count_rows(g: &mut Globals, fp: &mut CFile, b: &mut Buffer, what: &str) -> i32 {
    let mut n = 0;
    while fgets_line(g, fp, b, what) {
        if gdf_count_items_in_string(&b.s) > 0 {
            n += 1;
            continue;
        }
        g.notices.add(
            &FS_1033,
            format!("{what}: a line without an item (FreeSurfer counts it as a row)"),
        );
        if g.notices.reproduce() {
            n += 1;
        }
    }
    n
}

/// `fscanf(fp, "%s", tmpstr)`: the next word into the buffer (left as it was at the end of the
/// file); a word longer than 99,999 bytes overflows FreeSurfer's buffer (FS-1034: read whole).
fn scan_word(g: &mut Globals, fp: &mut CFile, b: &mut Buffer, what: &str) -> i32 {
    let mut sc = Scanf::new(fp);
    let w = sc.s(&mut g.errno, usize::MAX, true);
    let n = sc.finish(&mut g.errno);
    if let Some(w) = w {
        if w.len() > TMPSTR - 1 {
            g.notices.add(
                &FS_1034,
                format!(
                    "{what}: a word of {} bytes, longer than FreeSurfer's 100,000-byte buffer (it writes past it); read whole",
                    w.len()
                ),
            );
        }
        b.s = c_str(&w).to_vec();
    }
    n
}

/// `fscanf(fp, "%lf", &v)`: the number, or `None` when the call assigns none.
fn scan_number(g: &mut Globals, fp: &mut CFile) -> Option<f64> {
    let mut sc = Scanf::new(fp);
    let v = sc.lf(&mut g.errno, true);
    let n = sc.finish(&mut g.errno);
    if n == 1 { v } else { None }
}

/// The table's volume: `MRIallocSequence(ncols, 1, 1, MRI_FLOAT, nrows)`, voxels of size 1 and the
/// identity direction cosines (the cached matrices left as the allocation made them), and the data.
fn table_volume(g: &mut Globals, io: &mut Io, st: &StatTable) -> Result<Mri, Fatal> {
    let mut m = mri_alloc_sequence(g, io.out, io.err, st.ncols, 1, 1, MRI_FLOAT, st.nrows)?;
    let v = &mut m.vg;
    (v.xsize, v.ysize, v.zsize) = (1.0, 1.0, 1.0);
    (v.x_r, v.x_a, v.x_s) = (1.0, 0.0, 0.0);
    (v.y_r, v.y_a, v.y_s) = (0.0, 1.0, 0.0);
    (v.z_r, v.z_a, v.z_s) = (0.0, 0.0, 1.0);
    for r in 0..st.nrows {
        for c in 0..st.ncols {
            let x = st.data[r as usize][c as usize];
            mri_set_vox_val(g, io.err, &mut m, c, 0, 0, r, x as f32, 0)?;
        }
    }
    Ok(m)
}

fn format_error(out: &mut dyn Write, statfile: &str) {
    put(out, &[b"ERROR: format:  ", statfile.as_bytes(), b"\n"]);
}

fn found(out: &mut dyn Write, ncols: i32, nrows: i32) {
    let _ = write!(out, "Found {ncols} data colums\nFound {nrows} data rows\n");
}

/// `LoadStatTableFS(statfile)`.
fn load_stat_table_fs(
    g: &mut Globals,
    io: &mut Io,
    statfile: &str,
) -> Result<Option<StatTable>, Fatal> {
    const WHAT: &str = "LoadStatTable";
    let mut fp = match CFile::open(io.dir, statfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                io.out,
                &[b"ERROR: could not open ", statfile.as_bytes(), b"\n"],
            );
            return Ok(None);
        }
    };
    let mut st = StatTable {
        filename: Some(statfile.to_owned()),
        ..StatTable::default()
    };
    let mut b = Buffer { s: Vec::new() };
    st.ncols = first_line_columns(g, &mut fp, &mut b, WHAT);
    if st.ncols < 1 {
        format_error(io.out, statfile);
        return Ok(None);
    }
    let _ = writeln!(io.out, "Found {} data colums", st.ncols);
    st.nrows = count_rows(g, &mut fp, &mut b, WHAT);
    let _ = writeln!(io.out, "Found {} data rows", st.nrows);
    fp.close();
    let mut fp = match CFile::open(io.dir, statfile, "r") {
        Ok(f) => f,
        Err(e) => {
            // the file went away between the two opens: FreeSurfer reads a NULL stream (a crash)
            g.errno = e;
            put(
                io.out,
                &[b"ERROR: could not open ", statfile.as_bytes(), b"\n"],
            );
            return Ok(None);
        }
    };
    scan_word(g, &mut fp, &mut b, WHAT);
    st.measure = Some(b.s.clone());
    for _ in 0..st.ncols {
        scan_word(g, &mut fp, &mut b, WHAT);
        st.colnames.push(Some(b.s.clone()));
    }
    for r in 0..st.nrows {
        scan_word(g, &mut fp, &mut b, WHAT);
        st.rownames.push(Some(b.s.clone()));
        let mut row = Vec::with_capacity(st.ncols as usize);
        for c in 0..st.ncols {
            match scan_number(g, &mut fp) {
                Some(v) => row.push(v),
                None => {
                    let _ = writeln!(io.out, "ERROR: format: {statfile} at row {r}, col {c}");
                    return Ok(None);
                }
            }
        }
        st.data.push(row);
    }
    fp.close();
    st.mri = Some(table_volume(g, io, &st)?);
    Ok(Some(st))
}

/// `std::stof(field)`: `strtof` of the field (up to a zero byte); `None` where it throws (no
/// number read, or `ERANGE`: beyond the `float` range, or below it after rounding and inexact).
pub fn stof(field: &[u8]) -> Option<f32> {
    let s = c_str(field);
    let (v, n) = strtof(s);
    if n == 0 {
        return None;
    }
    // `ERANGE` as glibc's `strtof` sets it (its `%f` conversion is the same): the characters
    // `strtof` read, converted again by the scanner that reports it
    let mut b = Bytes::new(&s[..n]);
    match fscan::scan_f32(&mut b as &mut dyn Stream) {
        Scan::Value(_, true) => None,
        _ => Some(v),
    }
}

/// `LoadStatTableCSV(statfile)`: the table (`None` after a message), and whether FreeSurfer
/// crashes there (FS-1035, FS-1036: nothing follows in either mode).
fn load_stat_table_csv(
    g: &mut Globals,
    io: &mut Io,
    statfile: &str,
) -> Result<(Option<StatTable>, bool), Fatal> {
    let mut fp = match CFile::open(io.dir, statfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                io.out,
                &[b"ERROR: could not open ", statfile.as_bytes(), b"\n"],
            );
            return Ok((None, false));
        }
    };
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = fp.read_bytes(&mut chunk);
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    fp.close();
    let csv = read_csv(&bytes);
    let Some(head) = csv.first() else {
        g.notices.add(
            &FS_1035,
            "LoadStatTableCSV: a table without lines (FreeSurfer reads its first row anyway and crashes)",
        );
        format_error(io.out, statfile);
        return Ok((None, true));
    };
    let mut st = StatTable {
        filename: Some(statfile.to_owned()),
        nrows: csv.len() as i32 - 1,
        ncols: head.len() as i32 - 1,
        ..StatTable::default()
    };
    found(io.out, st.ncols, st.nrows);
    st.measure = Some(c_str(&head[0]).to_vec());
    st.colnames = head[1..].iter().map(|n| Some(c_str(n).to_vec())).collect();
    for r in 0..st.nrows as usize {
        let row = &csv[r + 1];
        st.rownames.push(Some(c_str(&row[0]).to_vec()));
        let mut vals = Vec::with_capacity(st.ncols as usize);
        for c in 0..st.ncols as usize {
            let Some(field) = row.get(c + 1) else {
                g.notices.add(
                    &FS_1036,
                    format!(
                        "LoadStatTableCSV: row {r} has {} fields, fewer than the first line's {} (FreeSurfer reads past the row)",
                        row.len(),
                        head.len()
                    ),
                );
                let _ = writeln!(io.out, "ERROR: format: {statfile} at row {r}, col {c}");
                return Ok((None, true));
            };
            match stof(field) {
                Some(v) => vals.push(f64::from(v)),
                None => {
                    let _ = writeln!(io.out, "ERROR: format: {statfile} at row {r}, col {c}");
                    return Ok((None, false));
                }
            }
        }
        st.data.push(vals);
    }
    st.mri = Some(table_volume(g, io, &st)?);
    Ok((Some(st), false))
}

/// `LoadStatTableTSV(statfile)`.
fn load_stat_table_tsv(
    g: &mut Globals,
    io: &mut Io,
    statfile: &str,
) -> Result<Option<StatTable>, Fatal> {
    let mut tsv = Tsv::default();
    if tsv_read(g, io.out, io.dir, &mut tsv, statfile) != 0 {
        return Ok(None);
    }
    let tac = fio_extension(Some(statfile)).is_some_and(|e| e.eq_ignore_ascii_case("tac"));
    let c0 = if tac { 2 } else { 0 };
    let ncols = tsv.headers.len() as i32 - c0 as i32;
    let _ = write!(
        io.out,
        "Found {ncols} data colums\nFound {} data rows\n",
        tsv.data.len()
    );
    if tac && tsv.headers.len() < 2 {
        g.notices.add(
            &FS_1037,
            format!(
                "LoadStatTableTSV: a .tac table of {} column (FreeSurfer reads its second column anyway and crashes)",
                tsv.headers.len()
            ),
        );
        format_error(io.out, statfile);
        return Ok(None);
    }
    let measure: &[u8] = if tac && tsv.headers[1] == b"end[kBq/cc]" {
        b"kBq/cc"
    } else {
        b"UnknownUnits"
    };
    let mut st = StatTable {
        filename: Some(statfile.to_owned()),
        nrows: tsv.data.len() as i32,
        ncols,
        measure: Some(measure.to_vec()),
        ..StatTable::default()
    };
    st.colnames = tsv.headers[c0..]
        .iter()
        .map(|h| Some(c_str(h).to_vec()))
        .collect();
    st.rownames = vec![Some(b"row".to_vec()); tsv.data.len()];
    st.data = tsv.data.iter().map(|row| row[c0..].to_vec()).collect();
    let mut m = table_volume(g, io, &st)?;
    g.notices.add(
        &FS_950,
        "LoadStatTableTSV: FreeSurfer writes the table's volume to junk.mgz in the working directory",
    );
    if g.notices.reproduce() {
        mri_write(
            g, io.out, io.err, io.env, io.dir, &mut m, "junk.mgz", None, 0,
        )?;
    }
    st.mri = Some(m);
    Ok(Some(st))
}

/// `LoadStatTable(statfile)`: by its extension (`fio_extension`, any case) a CSV table, a TSV
/// table (`.tsv`, `.tac`) or FreeSurfer's format; `None` after a reader's message.
pub fn load_stat_table(
    g: &mut Globals,
    io: &mut Io,
    statfile: &str,
) -> Result<Option<StatTable>, Fatal> {
    let ext = fio_extension(Some(statfile));
    let _ = writeln!(
        io.out,
        "LoadStatTable(): ext {}",
        ext.as_deref().unwrap_or("(null)")
    );
    let Some(ext) = ext else {
        g.notices.add(
            &FS_1031,
            format!(
                "LoadStatTable({statfile}): a name without an extension (FreeSurfer compares a NULL extension and crashes); read in FreeSurfer's format"
            ),
        );
        let _ = writeln!(io.out, "Loading table as FS Table");
        return load_stat_table_fs(g, io, statfile);
    };
    let is = |e: &str| ext.eq_ignore_ascii_case(e);
    let mut table = None;
    if is("csv") {
        let _ = writeln!(io.out, "Loading table as CSV");
        let crashed;
        (table, crashed) = load_stat_table_csv(g, io, statfile)?;
        if crashed {
            // FreeSurfer crashes in its CSV reader: both modes end there
            return Ok(None);
        }
    }
    if is("tsv") || is("tac") {
        let _ = writeln!(io.out, "Loading table as TSV or TAC");
        table = load_stat_table_tsv(g, io, statfile)?;
    } else {
        if is("csv") {
            g.notices.add(
                &FS_949,
                "LoadStatTable: FreeSurfer reads a .csv table again in its own format and keeps that one",
            );
            if g.notices.fix() {
                return Ok(table);
            }
        }
        let _ = writeln!(io.out, "Loading table as FS Table");
        table = load_stat_table_fs(g, io, statfile)?;
    }
    Ok(table)
}

/// `AllocStatTable(nrows, ncols)`: names `NULL`, numbers 0, no volume.
pub fn alloc_stat_table(nrows: i32, ncols: i32) -> StatTable {
    StatTable {
        nrows,
        ncols,
        colnames: vec![None; ncols.max(0) as usize],
        rownames: vec![None; nrows.max(0) as usize],
        data: vec![vec![0.0; ncols.max(0) as usize]; nrows.max(0) as usize],
        ..StatTable::default()
    }
}

/// `%-33s ` of a name (`(null)` for `NULL`).
fn name33(fp: &mut dyn Write, s: &Option<Vec<u8>>) {
    let s: &[u8] = s.as_deref().unwrap_or(b"(null)");
    put(fp, &[s]);
    for _ in s.len()..33 {
        put(fp, &[b" "]);
    }
    put(fp, &[b" "]);
}

/// `PrintStatTable(fp, st)`: `%-33s ` the measure, `%s ` each column name, a newline; then each
/// row: `%-33s ` its name, `%7.3lf ` each number, a newline. 0.
pub fn print_stat_table(fp: &mut dyn Write, st: &StatTable) -> i32 {
    name33(fp, &st.measure);
    for c in 0..st.ncols.max(0) as usize {
        let s: &[u8] = st.colnames[c].as_deref().unwrap_or(b"(null)");
        put(fp, &[s, b" "]);
    }
    put(fp, &[b"\n"]);
    for r in 0..st.nrows.max(0) as usize {
        name33(fp, &st.rownames[r]);
        for c in 0..st.ncols.max(0) as usize {
            let v = cfmt::pad(&cfmt::f(st.data[r][c], 3), 7);
            put(fp, &[v.as_bytes(), b" "]);
        }
        put(fp, &[b"\n"]);
    }
    0
}

/// `WriteStatTable(fname, st)`: the table printed into `fname`; `ERROR: cannot open <fname>` (on
/// standard output) and the end of the program (status 1) when it cannot be created.
pub fn write_stat_table(
    g: &mut Globals,
    io: &mut Io,
    fname: &str,
    st: &StatTable,
) -> Result<i32, Fatal> {
    let mut fp = match CFile::open(io.dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(io.out, &[b"ERROR: cannot open ", fname.as_bytes(), b"\n"]);
            return Err(Fatal::Exit(1));
        }
    };
    let err = print_stat_table(&mut fp, st);
    fp.close();
    Ok(err)
}

/// `InitStatTableFromMRI(mri_in, tablefile)`: a table of the volume's frames (rows) and columns
/// (x along the first row and slice), its numbers the voxels (`MRIgetVoxVal` of a copy, which the
/// table keeps); the measure and the names from `tablefile` when it is given and not empty: its
/// first line's items less one must be the volume's width (`ERROR: Col numbers do not agree in MRI
/// and:  <file>`), its later lines at least the volume's frames (`ERROR: Not enough row headers
/// for MRI in:  <file>`; more: `WARNING: Too many row headers for MRI in:  <file>, will crop
/// ...`); then its first word is the measure, the next ones the column names, and each row's name
/// the first word of the following lines (the rest of each line skipped). `None` after an error.
pub fn init_stat_table_from_mri(
    g: &mut Globals,
    io: &mut Io,
    mri_in: &Mri,
    tablefile: Option<&str>,
) -> Result<Option<StatTable>, Fatal> {
    const WHAT: &str = "InitStatTableFromMRI";
    let mut st = alloc_stat_table(mri_in.nframes, mri_in.vg.width);
    let copy = mri_copy(g, io.out, io.err, mri_in, None)?
        .into_new()
        .expect("a copy");
    for r in 0..st.nrows {
        for c in 0..st.ncols {
            st.data[r as usize][c as usize] =
                f64::from(mri_get_vox_val(g, io.err, &copy, c, 0, 0, r, 0)?);
        }
    }
    st.mri = Some(copy);
    let Some(tablefile) = tablefile.filter(|t| !t.is_empty()) else {
        return Ok(Some(st));
    };
    let mut fp = match CFile::open(io.dir, tablefile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                io.out,
                &[b"ERROR: could not open ", tablefile.as_bytes(), b"\n"],
            );
            return Ok(None);
        }
    };
    let mut b = Buffer { s: Vec::new() };
    let ncols = first_line_columns(g, &mut fp, &mut b, WHAT);
    let _ = writeln!(io.out, "Found {ncols} data colums");
    if ncols != st.ncols {
        put(
            io.out,
            &[
                b"ERROR: Col numbers do not agree in MRI and:  ",
                tablefile.as_bytes(),
                b"\n",
            ],
        );
        return Ok(None);
    }
    let nrows = count_rows(g, &mut fp, &mut b, WHAT);
    let _ = writeln!(io.out, "Found {nrows} data rows");
    if nrows < st.nrows {
        put(
            io.out,
            &[
                b"ERROR: Not enough row headers for MRI in:  ",
                tablefile.as_bytes(),
                b"\n",
            ],
        );
        return Ok(None);
    }
    if nrows > st.nrows {
        put(
            io.out,
            &[
                b"WARNING: Too many row headers for MRI in:  ",
                tablefile.as_bytes(),
                b", will crop ...\n",
            ],
        );
    }
    fp.close();
    let mut fp = match CFile::open(io.dir, tablefile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                io.out,
                &[b"ERROR: could not open ", tablefile.as_bytes(), b"\n"],
            );
            return Ok(None);
        }
    };
    scan_word(g, &mut fp, &mut b, WHAT);
    st.measure = Some(b.s.clone());
    for c in 0..st.ncols as usize {
        scan_word(g, &mut fp, &mut b, WHAT);
        st.colnames[c] = Some(b.s.clone());
    }
    for r in 0..st.nrows as usize {
        scan_word(g, &mut fp, &mut b, WHAT);
        st.rownames[r] = Some(b.s.clone());
        fgets_line(g, &mut fp, &mut b, WHAT);
    }
    fp.close();
    Ok(Some(st))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_in_a_string() {
        assert_eq!(gdf_count_items_in_string(b"a b\tc\n"), 3);
        assert_eq!(gdf_count_items_in_string(b"  a  \n b"), 1);
        assert_eq!(gdf_count_items_in_string(b"a\rb c"), 2);
        assert_eq!(gdf_count_items_in_string(b"\r a"), 0);
        assert_eq!(gdf_count_items_in_string(b""), 0);
        assert_eq!(gdf_count_items_in_string(b"x\0 y"), 1);
    }

    #[test]
    fn stof_as_libstdcxx() {
        assert_eq!(stof(b" 1.5x"), Some(1.5));
        assert_eq!(stof(b"0x"), Some(0.0));
        assert_eq!(stof(b"infinity"), Some(f32::INFINITY));
        assert_eq!(stof(b"x"), None);
        assert_eq!(stof(b""), None);
        assert_eq!(stof(b"1e39"), None);
        assert_eq!(stof(b"1e-50"), None);
        assert_eq!(stof(b"0"), Some(0.0));
        assert_eq!(stof(b"\x001"), None);
    }

    #[test]
    fn printing_a_table() {
        let mut st = alloc_stat_table(1, 2);
        st.data[0] = vec![1.0, -2.5];
        let mut out = Vec::new();
        print_stat_table(&mut out, &st);
        let want = format!(
            "{:<33} (null) (null) \n{:<33}   1.000  -2.500 \n",
            "(null)", "(null)"
        );
        assert_eq!(String::from_utf8(out).unwrap(), want);
    }
}

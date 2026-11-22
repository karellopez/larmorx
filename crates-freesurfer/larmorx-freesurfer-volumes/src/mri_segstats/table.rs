// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats`'s statistics table: `include/stats.h`'s `STATSUMENTRY` and `SEGSTAT`,
//! `utils/stats.cpp`'s `PrintSegStat` (1667-1781), the program's table without it
//! (`--no-newprint`, `mri_segstats.cpp` 1287-1409), `LoadStatSumFile` (2300-2345) and
//! `DumpStatSumTable` (2347-2364).
//!
//! The numbers are printed as C's `printf` prints them from `float` fields widened to `double`:
//! `%3d %3d  %8d %10.1f  ` (index from 1, id, voxels, volume), the name (`%-30s `, or `Seg%04d `
//! without a colour table), then with an input volume `%10.4f ` for the mean, standard
//! deviation, minimum, maximum and range, and the signal-to-noise ratio with `--snr`.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, errno_of};
use larmorx_freesurfer_core::utils::resolve;

use super::gtm::fgets_lines;
use super::options::Run;

/// `STATSUMENTRY`: one segmentation's row (its name a C string of at most 999 bytes).
#[derive(Clone, Debug, Default)]
pub struct StatSumEntry {
    pub id: i32,
    pub name: Vec<u8>,
    pub nhits: i32,
    pub vol: f32,
    pub red: i32,
    pub green: i32,
    pub blue: i32,
    pub min: f32,
    pub max: f32,
    pub range: f32,
    pub mean: f32,
    pub std: f32,
    pub snr: f32,
}

/// `SEGSTAT`: what `PrintSegStat` prints.
pub struct SegStat<'a> {
    pub entry: &'a [StatSumEntry],
    pub is_surf: bool,
    pub use_name: bool,
    pub do_intensity: bool,
    pub in_intensity_name: &'a str,
    pub in_intensity_units: &'a str,
    pub do_snr: bool,
}

/// `%<w>d`.
pub fn d(v: i32, w: usize) -> String {
    format!("{v:>w$}")
}

/// `%<w>.<p>f` of a `float` (widened to `double`).
pub fn ff(v: f32, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(f64::from(v), p), w)
}

/// `%-30s ` of a name (C string bytes).
fn name30(fp: &mut dyn Write, name: &[u8]) {
    let _ = fp.write_all(name);
    let pad = 30usize.saturating_sub(name.len());
    let _ = fp.write_all(&b" ".repeat(pad));
    let _ = fp.write_all(b" ");
}

/// One row of the table (`PrintSegStat`'s and the old table's are the same).
fn row(fp: &mut dyn Write, n: usize, e: &StatSumEntry, use_name: bool, intensity: bool, snr: bool) {
    let _ = write!(
        fp,
        "{} {}  {} {}  ",
        d(n as i32 + 1, 3),
        d(e.id, 3),
        d(e.nhits, 8),
        ff(e.vol, 10, 1)
    );
    if use_name {
        name30(fp, &e.name);
    } else {
        let _ = write!(fp, "Seg{:04} ", e.id);
    }
    if intensity {
        let _ = write!(
            fp,
            "{} {} {} {} {} ",
            ff(e.mean, 10, 4),
            ff(e.std, 10, 4),
            ff(e.min, 10, 4),
            ff(e.max, 10, 4),
            ff(e.range, 10, 4)
        );
        if snr {
            let _ = write!(fp, "{} ", ff(e.snr, 10, 4));
        }
    }
    let _ = writeln!(fp);
}

/// The three lines `# TableCol <n> ColHeader/FieldName/Units` of one column.
fn col(fp: &mut dyn Write, c: usize, header: &str, field: &str, units: &str) {
    let _ = writeln!(fp, "# TableCol {c:>2} ColHeader {header}");
    let _ = writeln!(fp, "# TableCol {c:>2} FieldName {field}");
    let _ = writeln!(fp, "# TableCol {c:>2} Units     {units}");
}

/// The first four columns' descriptions (index, id, count, size), as both tables write them.
fn first_columns(fp: &mut dyn Write, is_surf: bool) {
    let _ = fp.write_all(
        b"# TableCol  1 ColHeader Index \n# TableCol  1 FieldName Index \n# TableCol  1 Units     NA \n",
    );
    let _ = fp.write_all(
        b"# TableCol  2 ColHeader SegId \n# TableCol  2 FieldName Segmentation Id\n# TableCol  2 Units     NA\n",
    );
    if !is_surf {
        let _ = fp.write_all(b"# TableCol  3 ColHeader NVoxels \n# TableCol  3 FieldName Number of Voxels\n# TableCol  3 Units     unitless\n");
        let _ = fp.write_all(b"# TableCol  4 ColHeader Volume_mm3\n# TableCol  4 FieldName Volume\n# TableCol  4 Units     mm^3\n");
    } else {
        let _ = fp.write_all(b"# TableCol  3 ColHeader NVertices \n# TableCol  3 FieldName Number of Vertices\n# TableCol  3 Units     unitless\n");
        let _ = fp.write_all(b"# TableCol  4 ColHeader Area_mm2\n# TableCol  4 FieldName Area\n# TableCol  4 Units     mm^2\n");
    }
}

/// The intensity columns' descriptions from column `c` on; the next column's number.
fn intensity_columns(fp: &mut dyn Write, mut c: usize, name: &str, units: &str) -> usize {
    col(fp, c, &format!("{name}Mean "), &format!("Intensity {name}Mean"), units);
    c += 1;
    for what in ["StdDev", "Min", "Max", "Range"] {
        col(fp, c, &format!("{name}{what}"), &format!("Intensity {name}{what}"), units);
        c += 1;
    }
    c
}

/// `PrintSegStat(fp, segstat)`: the columns' descriptions (the name's always; the intensity's
/// with an input; the signal-to-noise ratio's with `--snr`, even without an input, whose rows
/// then have none), `# NRows`, `# NTableCols`, `# ColHeaders` and the rows.
pub fn print_seg_stat(fp: &mut dyn Write, s: &SegStat) {
    first_columns(fp, s.is_surf);
    let mut headers: Vec<String> = vec!["Index".into(), "SegId".into()];
    if !s.is_surf {
        headers.extend(["NVoxels".into(), "Volume_mm3".into()]);
    } else {
        headers.extend(["NVertices".into(), "Area_mm2".into()]);
    }
    let mut c = 5;
    col(fp, c, "StructName", "Structure Name", "NA");
    headers.push("StructName".into());
    c += 1;
    let name = s.in_intensity_name;
    if s.do_intensity {
        c = intensity_columns(fp, c, name, s.in_intensity_units);
        for what in ["Mean", "StdDev", "Min", "Max", "Range"] {
            headers.push(format!("{name}{what}"));
        }
    }
    if s.do_snr {
        col(fp, c, "SNR", "Intensity SNR", "none");
        headers.push(format!("{name}SNR"));
        c += 1;
    }
    let _ = writeln!(fp, "# NRows {} ", s.entry.len());
    let _ = writeln!(fp, "# NTableCols {} ", c - 1);
    let _ = write!(fp, "# ColHeaders  ");
    for h in &headers {
        let _ = write!(fp, "{h} ");
    }
    let _ = fp.write_all(b" \n");
    for (n, e) in s.entry.iter().enumerate() {
        row(fp, n, e, s.use_name, s.do_intensity, s.do_snr);
    }
}

/// The table without `PrintSegStat` (`--no-newprint`): the name's column only with a colour
/// table, `StructName` in the headers always, the signal-to-noise ratio's column not described.
pub fn print_old_table(fp: &mut dyn Write, s: &SegStat) {
    first_columns(fp, s.is_surf);
    let mut n = 5;
    if s.use_name {
        col(fp, n, "StructName", "Structure Name", "NA");
        n += 1;
    }
    let name = s.in_intensity_name;
    if s.do_intensity {
        n = intensity_columns(fp, n, name, s.in_intensity_units);
    }
    let _ = writeln!(fp, "# NRows {} ", s.entry.len());
    let _ = writeln!(fp, "# NTableCols {} ", n - 1);
    let _ = write!(fp, "# ColHeaders  Index SegId ");
    if !s.is_surf {
        let _ = write!(fp, "NVoxels Volume_mm3 ");
    } else {
        let _ = write!(fp, "NVertices Area_mm2 ");
    }
    let _ = write!(fp, "StructName ");
    if s.do_intensity {
        let _ = write!(fp, "{name}Mean {name}StdDev {name}Min {name}Max {name}Range  ");
        if s.do_snr {
            let _ = write!(fp, "{name}SNR ");
        }
    }
    let _ = writeln!(fp);
    for (k, e) in s.entry.iter().enumerate() {
        row(fp, k, e, s.use_name, s.do_intensity, s.do_snr);
    }
}

/// `LoadStatSumFile(fname, &nsegid)`: a row of each line (`fgets` of 999 bytes) not starting
/// with `#`, read by `sscanf("%*d %d %d %f %s %f %f %f %f %f")` into the id, count, volume,
/// name, mean, standard deviation, minimum, maximum and range (the fields not read 0). A file
/// that does not open: `ERROR: cannot open <file>`, status 1.
pub fn load_stat_sum_file(
    g: &mut Globals,
    r: &mut Run,
    fname: &str,
) -> Result<Vec<StatSumEntry>, Fatal> {
    let bytes = match std::fs::read(resolve(r.dir, fname)) {
        Ok(b) => b,
        // a folder opens and reads nothing
        Err(e) if errno_of(&e) == Some(21) => Vec::new(),
        Err(_) => {
            let _ = writeln!(r.out, "ERROR: cannot open {fname}");
            return Err(Fatal::Exit(1));
        }
    };
    let mut table = Vec::new();
    for line in fgets_lines(&bytes, 1000) {
        if line.first() == Some(&b'#') {
            continue;
        }
        let mut e = StatSumEntry::default();
        let mut st = Bytes::new(line);
        let mut sc = Scanf::new(&mut st);
        let en = &mut g.errno;
        sc.d(en, false);
        sc.text(en, b" ");
        if let Some(v) = sc.d(en, true) {
            e.id = v;
        }
        sc.text(en, b" ");
        if let Some(v) = sc.d(en, true) {
            e.nhits = v;
        }
        sc.text(en, b" ");
        if let Some(v) = sc.f(en, true) {
            e.vol = v;
        }
        sc.text(en, b" ");
        if let Some(v) = sc.s(en, usize::MAX, true) {
            e.name = v;
        }
        let fields: [&mut f32; 5] = [&mut e.mean, &mut e.std, &mut e.min, &mut e.max, &mut e.range];
        for f in fields {
            sc.text(en, b" ");
            if let Some(v) = sc.f(en, true) {
                *f = v;
            }
        }
        sc.finish(en);
        table.push(e);
    }
    Ok(table)
}

/// `DumpStatSumTable(StatSumTable, nsegid)`: `%3d  %8d %10.1f  %-30s ` and the minimum, maximum,
/// range, mean and standard deviation (`%10.4f `) of each row.
pub fn dump_stat_sum_table(out: &mut dyn Write, table: &[StatSumEntry]) {
    for e in table {
        let _ = write!(out, "{}  {} {}  ", d(e.id, 3), d(e.nhits, 8), ff(e.vol, 10, 1));
        name30(out, &e.name);
        let _ = write!(
            out,
            "{} {} {} {} {} ",
            ff(e.min, 10, 4),
            ff(e.max, 10, 4),
            ff(e.range, 10, 4),
            ff(e.mean, 10, 4),
            ff(e.std, 10, 4)
        );
        let _ = writeln!(out);
    }
}

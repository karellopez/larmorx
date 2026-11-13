// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The text colour tables and the lookups of `utils/colortab.cpp` (queue item 3.1-33, fs.labels;
//! the binary tables are in [`crate::colortab_bin`], the structure in
//! [`larmorx_freesurfer_mri::ctab`]): `CTABreadASCII`, `CTABreadASCII2`, `CTABreadASCIIttHeader`,
//! `CTABreadDefault`, `CTABalloc`, `CTABunique`, `CTABcountRepeats`, `CTABrandom`, the entry
//! counts and tests, the colour, name and annotation lookups, `CTABfindDuplicateNames`,
//! `CTABprintASCII`, `CTABwriteFileASCII`, `TissueTypeSchema` with its two schemas, `CTABmerge`.
//!
//! A text table has one entry per line: `<structure> <name> <r> <g> <b> <transparency>
//! [<tissue type>]`; `#ctTType` lines describe the tissue types. An annotation packs an entry's
//! colour into one `int`: `(b << 16) + (g << 8) + r`. FS-413 to FS-420.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_413, FS_414, FS_416, FS_417, FS_418, FS_419, FS_420};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd;
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::fsenv;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_NOFILE, ERROR_NONE, Fatal,
};
use larmorx_freesurfer_core::random;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};

use crate::cfile::CFile;
use crate::colortab_bin::MAX_ENTRIES;

/// `CTAB_VERSION_TO_WRITE` (`colortab.h`).
pub const CTAB_VERSION_TO_WRITE: i32 = 2;

/// `STRLEN`: the line buffer of the readers (`fgets` reads at most 1,023 bytes of a line).
const STRLEN: usize = 1024;

/// The largest entry count `CTABalloc` makes (each entry is allocated whole in FreeSurfer, 1,064
/// bytes: about 1.1 GB at this count); above, FS-420.
pub const MAX_ALLOC: i32 = 1 << 20;

fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

/// `(float)i / 255.0` stored in a `float` (the division in `double`).
fn unit(i: i32) -> f32 {
    (f64::from(i as f32) / 255.0) as f32
}

/// glibc's `strcmp` of two C strings.
fn strcmp(a: &[u8], b: &[u8]) -> i32 {
    utils::strcmp(a, b)
}

/// FreeSurfer's `stricmp(a, b)`: `strcmp` of the two in upper case (the C locale's letters); FS-11
/// for a string of 1,024 bytes or more (its 1,024-byte buffers).
fn stricmp(g: &mut Globals, a: &[u8], b: &[u8]) -> i32 {
    let (a, b) = (c_str(a), c_str(b));
    for s in [a, b] {
        if s.len() >= utils::STR_LEN {
            g.notices.add(
                &larmorx_core::defects::FS_11,
                format!(
                    "stricmp of a {}-byte string, longer than FreeSurfer's buffer",
                    s.len()
                ),
            );
        }
    }
    strcmp(&a.to_ascii_uppercase(), &b.to_ascii_uppercase())
}

/// C's `%-30s`: the bytes, then spaces to 30.
fn left30(w: &mut Vec<u8>, s: &[u8]) {
    w.extend_from_slice(s);
    for _ in s.len()..30 {
        w.push(b' ');
    }
}

/// C's `%3d` (and `%2d` with `width` 2).
fn padded(w: &mut Vec<u8>, v: i32, width: usize) {
    let s = v.to_string();
    for _ in s.len()..width {
        w.push(b' ');
    }
    w.extend_from_slice(s.as_bytes());
}

// ---- reading text tables ------------------------------------------------------------------------

/// `fgets(line, STRLEN, fp)` as a C string; a failed read's `errno` kept.
fn fgets(g: &mut Globals, fp: &mut CFile) -> Option<Vec<u8>> {
    let l = fp.fgets(STRLEN);
    if l.is_none()
        && let Some(e) = fp.read_errno()
    {
        g.errno = e;
    }
    l.map(|l| c_str(&l).to_vec())
}

/// `fgetl(line, STRLEN, fp)`: the next line that is not empty or a comment after its leading white
/// space, cut at its first `#`, without its newline.
fn fgetl(g: &mut Globals, fp: &mut CFile) -> Option<Vec<u8>> {
    loop {
        let line = fgets(g, fp)?;
        let start = line
            .iter()
            .position(|&b| !cstd::is_space(b))
            .unwrap_or(line.len());
        let cp = &line[start..];
        if cp.is_empty() || cp[0] == b'#' || cp[0] == b'\n' {
            continue;
        }
        let end = cp.iter().position(|&b| b == b'#').unwrap_or(cp.len());
        let mut cp = cp[..end].to_vec();
        if cp.last() == Some(&b'\n') {
            cp.pop();
        }
        return Some(cp);
    }
}

/// One line read with `sscanf(line, "%d %s %d %d %d %d %d", ...)` (`seven` false: the first six
/// conversions only).
struct EntryLine {
    n: i32,
    structure: i32,
    name: Vec<u8>,
    rgbt: [i32; 4],
    tt: i32,
}

fn scan_entry(errno: &mut i32, line: &[u8], seven: bool) -> EntryLine {
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    let structure = sc.d(errno, true);
    sc.text(errno, b" ");
    let name = sc.s(errno, usize::MAX, true);
    let mut rgbt = [0; 4];
    for v in &mut rgbt {
        sc.text(errno, b" ");
        *v = sc.d(errno, true).unwrap_or(0);
    }
    let mut tt = 0;
    if seven {
        sc.text(errno, b" ");
        tt = sc.d(errno, true).unwrap_or(0);
    }
    let n = sc.finish(errno);
    EntryLine {
        n,
        structure: structure.unwrap_or(0),
        name: name.unwrap_or_default(),
        rgbt,
        tt,
    }
}

/// The entry a text line makes (`ai = 255 - t`, the float colours `(float)i / 255.0`).
fn text_entry(name: &[u8], rgbt: [i32; 4], tissue_type: i32) -> Box<ColorTableEntry> {
    let ai = 255i32.wrapping_sub(rgbt[3]);
    Box::new(ColorTableEntry {
        name: name.to_vec(),
        ri: rgbt[0],
        gi: rgbt[1],
        bi: rgbt[2],
        ai,
        rf: unit(rgbt[0]),
        gf: unit(rgbt[1]),
        bf: unit(rgbt[2]),
        af: unit(ai),
        tissue_type,
        count: 0,
    })
}

/// FS-420: an entry count beyond what larmorx allocates (FreeSurfer's outcome depends on the
/// machine's memory): `true` (and the error FreeSurfer gives when the allocation fails) when
/// `nentries` is refused.
fn too_many(
    g: &mut Globals,
    err: &mut dyn Write,
    nentries: i64,
    what: &str,
    message: &str,
) -> bool {
    if nentries <= i64::from(MAX_ENTRIES) {
        return false;
    }
    g.notices.add(
        &FS_420,
        format!("{what} of {nentries} entries (FreeSurfer's outcome depends on the machine's memory): refused"),
    );
    g.errno = 12;
    log::error_printf(g, err, ERROR_NO_MEMORY, message);
    true
}

/// `CTABreadASCII(fname)`: [`ctab_read_ascii2`] with the duplicate names checked.
pub fn ctab_read_ascii(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Option<ColorTable> {
    ctab_read_ascii2(g, out, err, dir, fname, true)
}

/// `CTABreadASCII2(fname, checkDuplicateNames)`: [`ctab_read_ascii2_counted`] without the
/// duplicate count.
pub fn ctab_read_ascii2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    check_duplicate_names: bool,
) -> Option<ColorTable> {
    ctab_read_ascii2_counted(g, out, err, dir, fname, check_duplicate_names).0
}

/// `CTABreadASCII2(fname, checkDuplicateNames)`: the table and `ctabDuplicates` (`None` where
/// FreeSurfer leaves that global as it was: the file did not open or was badly formed).
///
/// The first pass reads the lines through `fgetl` (comments cut at `#`) and takes the largest
/// structure number of the lines that read six numbers and a name; none above 0: the error
/// `CTABreadASCII(<fname>): badly formed file` (`ERROR_NOFILE`). The table has that number plus one
/// places, the file name (at most 1,023 bytes) and version 2. The second pass reads every line as
/// it is (`fgets`, comments included) with seven conversions: a line of six or seven makes the entry
/// of its structure (the tissue type the seventh, else -1; alpha 255 minus the transparency; the
/// float colours divided by 255), unless the place is taken: `CTABreadASCII(<fname>): Line <n>:
/// Duplicate structure index <s>, was <name> <r> <g> <b> <a>` on standard output, the duplicate
/// counted. Then [`ctab_find_duplicate_names`] when asked, and the tissue-type table
/// ([`ctab_read_ascii_tt_header`]).
///
/// **FS-413:** a second-pass line whose structure number is negative or beyond the table (a `#`
/// in the name of the line with the largest number: the first pass cut it as a comment) is written
/// outside FreeSurfer's table. By default a negative number is an error (`CTABreadASCII(<fname>):
/// Line <n>: negative structure index <s>`, `ERROR_BADFILE`, no table: rule B) and the table grows
/// to hold a larger one (rule A: the entry is kept, as FreeSurfer keeps such a line within its
/// table); `reproduce` leaves both lines out. **FS-420:** a largest number of 2^24 or more is
/// refused as FreeSurfer's failed allocation (`could not allocate <n> entries`).
pub fn ctab_read_ascii2_counted(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    check_duplicate_names: bool,
) -> (Option<ColorTable>, Option<i32>) {
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                err,
                ERROR_NOFILE,
                &format!("CTABreadASCII({fname}): could not open file"),
            );
            return (None, None);
        }
    };
    let mut max_structure: i32 = -1;
    while let Some(line) = fgetl(g, &mut fp) {
        let e = scan_entry(&mut g.errno, &line, false);
        if e.n == 6 && e.structure > max_structure {
            max_structure = e.structure;
        }
    }
    if max_structure <= 0 {
        log::error_printf(
            g,
            err,
            ERROR_NOFILE,
            &format!("CTABreadASCII({fname}): badly formed file"),
        );
        return (None, None);
    }
    let nentries = max_structure.wrapping_add(1);
    if too_many(
        g,
        err,
        i64::from(max_structure) + 1,
        "an ASCII colour table",
        &format!("CTABreadASCII({fname}): could not allocate {nentries} entries"),
    ) {
        return (None, None);
    }
    let reproduce = g.notices.reproduce();
    let mut ct = ColorTable {
        entries: vec![None; nentries as usize],
        fname: fname.as_bytes().iter().copied().take(STRLEN - 1).collect(),
        version: CTAB_VERSION_TO_WRITE,
        ..ColorTable::default()
    };
    let mut duplicates = 0;
    fp.rewind();
    let mut line_num = 1;
    while let Some(line) = fgets(g, &mut fp) {
        let e = scan_entry(&mut g.errno, &line, true);
        let tt = if e.n != 7 { -1 } else { e.tt };
        if e.n >= 6 {
            let s = e.structure;
            if s < 0 {
                g.notices.add(
                    &FS_413,
                    format!("an ASCII colour table's line {line_num} with structure index {s}"),
                );
                if !reproduce {
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        &format!(
                            "CTABreadASCII({fname}): Line {line_num}: negative structure index {s}"
                        ),
                    );
                    return (None, Some(duplicates));
                }
                line_num += 1;
                continue;
            }
            if s as usize >= ct.entries.len() {
                g.notices.add(
                    &FS_413,
                    format!(
                        "an ASCII colour table's line {line_num} with structure index {s} beyond its {} places (a '#' in the name of its largest)",
                        ct.entries.len()
                    ),
                );
                if reproduce {
                    line_num += 1;
                    continue;
                }
                if too_many(
                    g,
                    err,
                    i64::from(s) + 1,
                    "an ASCII colour table",
                    &format!(
                        "CTABreadASCII({fname}): could not allocate {} entries",
                        s.wrapping_add(1)
                    ),
                ) {
                    return (None, Some(duplicates));
                }
                ct.entries.resize(s as usize + 1, None);
            }
            let slot = &mut ct.entries[s as usize];
            if let Some(old) = slot {
                let mut msg = format!(
                    "CTABreadASCII({fname}): Line {line_num}: Duplicate structure index {s}, was "
                )
                .into_bytes();
                msg.extend_from_slice(c_str(&old.name));
                msg.extend_from_slice(
                    format!(" {} {} {} {}\n", old.ri, old.gi, old.bi, old.ai).as_bytes(),
                );
                let _ = out.write_all(&msg);
                duplicates += 1;
            } else {
                *slot = Some(text_entry(&e.name, e.rgbt, tt));
            }
        }
        line_num += 1;
    }
    drop(fp);
    if check_duplicate_names {
        ctab_find_duplicate_names(out, &ct);
    }
    ct.ctab_tissue_type = ctab_read_ascii_tt_header(g, out, err, dir, fname).map(Box::new);
    (Some(ct), Some(duplicates))
}

/// One `#ctTType` line read with `sscanf(line, "%*s %d %s  %d %d %d  %d", ...)`: the values it
/// assigned (`None` where it stopped).
fn scan_tt(errno: &mut i32, line: &[u8]) -> (i32, [Option<i32>; 5], Option<Vec<u8>>) {
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    sc.s(errno, usize::MAX, false);
    sc.text(errno, b" ");
    let structure = sc.d(errno, true);
    sc.text(errno, b" ");
    let name = sc.s(errno, usize::MAX, true);
    let mut v = [structure, None, None, None, None];
    for x in &mut v[1..] {
        sc.text(errno, b" ");
        *x = sc.d(errno, true);
    }
    let n = sc.finish(errno);
    (n, v, name)
}

/// The default's error for a `#ctTType` line that does not read whole (FS-414), on standard output.
fn bad_tt_line(out: &mut dyn Write, fname: &str, line: &[u8]) {
    let mut msg =
        format!("ERROR: CTABreadASCIItt(): bad tissue type line in {fname}: ").into_bytes();
    msg.extend_from_slice(line.strip_suffix(b"\n").unwrap_or(line));
    msg.push(b'\n');
    let _ = out.write_all(&msg);
}

/// The items of a line as `GetNthItemFromString` splits it (white space as `isspace`).
fn items(line: &[u8]) -> Vec<&[u8]> {
    line.split(|&b| cstd::is_space(b))
        .filter(|w| !w.is_empty())
        .collect()
}

/// `CTABreadASCIIttHeader(fname)`: the tissue-type table of the `#ctTType <structure> <name> <r>
/// <g> <b> <transparency>` lines (as `CTABprintASCIItt` writes them; tissue type the structure
/// number, float colours 0), sized by the largest structure number; `None` without such lines.
/// A file that does not open: `ERROR: CTABreadASCIItt(): cannot open <fname>` (standard output).
/// FreeSurfer reads the file's last line twice in each pass (glibc's `fgets` leaves the buffer at
/// the end): no visible effect.
///
/// **FS-414:** a line whose items `CountItemsInString` counts (space and tab separate them) but
/// `GetNthItemFromString` does not (only vertical tabs, form feeds or carriage returns): FreeSurfer
/// prints `error: asking for item 0, but 0 items in string` and crashes; both modes skip the line
/// (by default without the message). A `#ctTType` line without a second item: FreeSurfer prints
/// `error: asking for item 1, but 1 items in string` and crashes. A `#ctTType` line whose
/// structure number, name, colour or transparency does not read takes the value of the last line
/// that read it (memory never written before the first), and a structure number outside the table
/// is written outside it. By default every such line is an error: `ERROR: CTABreadASCIItt(): bad
/// tissue type line in <fname>: <line>` (standard output), no tissue-type table (rule B);
/// `reproduce` takes the earlier lines' values (0 and an empty name where none was read), leaves
/// out a structure outside the table, and gives no table for a missing second item.
pub fn ctab_read_ascii_tt_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Option<ColorTable> {
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(out, "ERROR: CTABreadASCIItt(): cannot open {fname}");
            return None;
        }
    };
    let reproduce = g.notices.reproduce();
    // the two passes see the same lines: read them once (the last one twice, as FreeSurfer)
    let mut lines: Vec<Vec<u8>> = Vec::new();
    while let Some(l) = fgets(g, &mut fp) {
        lines.push(l);
    }
    drop(fp);
    if let Some(last) = lines.last().cloned() {
        lines.push(last);
    }
    // the #ctTType lines, in order (the second item's text, the line)
    let mut tt: Vec<(Vec<u8>, &[u8])> = Vec::new();
    for line in &lines {
        if utils::count_items_in_string(line) == 0 {
            continue;
        }
        let it = items(line);
        let Some(first) = it.first() else {
            g.notices.add(
                &FS_414,
                "a colour table line of vertical tabs, form feeds or carriage returns only (FreeSurfer crashes): skipped",
            );
            if reproduce {
                log::error(g, err, "asking for item 0, but 0 items in string");
            }
            continue;
        };
        if strcmp(first, b"#ctTType") != 0 {
            continue;
        }
        let Some(second) = it.get(1) else {
            g.notices.add(
                &FS_414,
                "a #ctTType line without a structure number (FreeSurfer crashes): no tissue-type table",
            );
            if reproduce {
                log::error(g, err, "asking for item 1, but 1 items in string");
            } else {
                bad_tt_line(out, fname, line);
            }
            return None;
        };
        tt.push((second.to_vec(), line));
    }
    if tt.is_empty() {
        return None;
    }
    if !reproduce {
        // every line must read whole, with a structure number of 0 or more
        let mut errno = g.errno;
        for (_, line) in &tt {
            let (n, v, _) = scan_tt(&mut errno, line);
            if n != 6 || v[0].unwrap_or(-1) < 0 {
                g.notices.add(
                    &FS_414,
                    "a #ctTType line whose structure number, name, colour or transparency does not read: an error",
                );
                bad_tt_line(out, fname, line);
                return None;
            }
        }
    }
    // the first pass: the largest structure number (sscanf("%d") of the second item)
    let mut segid: Option<i32> = None;
    let mut segidmax: i32 = -1;
    for (second, _) in &tt {
        let mut b = Bytes::new(second);
        let mut sc = Scanf::new(&mut b);
        if let Some(v) = sc.d(&mut g.errno, true) {
            segid = Some(v);
        }
        sc.finish(&mut g.errno);
        let s = match segid {
            Some(s) => s,
            None => {
                g.notices.add(
                    &FS_414,
                    "a #ctTType line without a structure number before any: 0 (memory never written in FreeSurfer)",
                );
                0
            }
        };
        if segidmax < s {
            segidmax = s;
        }
    }
    let nentries = segidmax.wrapping_add(1);
    if too_many(
        g,
        err,
        i64::from(segidmax) + 1,
        "a tissue-type table",
        &format!("CTABreadASCIItt({fname}): could not allocate {nentries} entries"),
    ) {
        return None;
    }
    let mut ct = ColorTable {
        entries: vec![None; nentries.max(0) as usize],
        ..ColorTable::default()
    };
    // the second pass: each line's values, those it does not read kept from the line before
    let mut structure: Option<i32> = None;
    let mut name: Option<Vec<u8>> = None;
    let mut rgbt: [Option<i32>; 4] = [None; 4];
    for (_, line) in &tt {
        let (_, v, nm) = scan_tt(&mut g.errno, line);
        if v[0].is_some() {
            structure = v[0];
        }
        if nm.is_some() {
            name = nm;
        }
        for k in 0..4 {
            if v[k + 1].is_some() {
                rgbt[k] = v[k + 1];
            }
        }
        if structure.is_none() || name.is_none() || rgbt.iter().any(Option::is_none) {
            g.notices.add(
                &FS_414,
                "a #ctTType line's value read before any line gave it: 0 (memory never written in FreeSurfer)",
            );
        }
        let s = structure.unwrap_or(0);
        if s < 0 || s >= nentries {
            g.notices.add(
                &FS_414,
                format!("a #ctTType line with structure {s} outside its {} places (FreeSurfer writes outside the table): left out", nentries.max(0)),
            );
            continue;
        }
        let t = rgbt.map(|x| x.unwrap_or(0));
        ct.entries[s as usize] = Some(Box::new(ColorTableEntry {
            name: name.clone().unwrap_or_default(),
            ri: t[0],
            gi: t[1],
            bi: t[2],
            ai: 255i32.wrapping_sub(t[3]),
            tissue_type: s,
            ..ColorTableEntry::default()
        }));
    }
    Some(ct)
}

/// `CTABreadDefault()`: `FSENVgetenv()` (which reads `$FREESURFER_HOME/FreeSurferColorLUT.txt`
/// itself, with its messages), then [`ctab_read_ascii`] of that file: the default table is read
/// twice and its messages printed twice (kept). **FS-416:** when `FSENVgetenv` gives nothing
/// (`FREESURFER_HOME` not set, the table unreadable, a wrong `FS_DESIRED_BVEC_SPACE`), FreeSurfer
/// crashes; both modes return `None` after its messages.
pub fn ctab_read_default(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
) -> Option<ColorTable> {
    let home = fsenv_home(g, env, out, err, dir, "CTABreadDefault")?;
    ctab_read_ascii(g, out, err, dir, &format!("{home}/FreeSurferColorLUT.txt"))
}

/// `FSENVgetenv()` (its colour table read with [`ctab_read_ascii`]): `FREESURFER_HOME`, or `None`
/// with FS-416 noted.
fn fsenv_home(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    who: &str,
) -> Option<String> {
    let fsenv = fsenv::fsenv_getenv(g, env, dir, out, |g, out, file| {
        ctab_read_ascii(g, out, err, dir, file)
    });
    match fsenv {
        Some(f) => Some(f.freesurfer_home),
        None => {
            g.notices.add(
                &FS_416,
                format!("{who} without FreeSurfer's environment (FreeSurfer crashes): no table"),
            );
            None
        }
    }
}

// ---- tables of random colours -------------------------------------------------------------------

/// `nint(randomNumber(0, 255))`.
fn random_255(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32, Fatal> {
    let v = random::random_number(g, env, out, err, 0.0, 255.0)?;
    Ok(utils::nint(g, v))
}

/// `CTABalloc(nentries)`: a table of `nentries` entries named `cluster<i>`, each of random colours
/// (red, green, blue in that order, `nint(randomNumber(0, 255))`) and alpha 255, file name `none`,
/// version 2; then [`ctab_unique`] with 100 tries. A negative count: `CTABalloc: nentries was <n>`
/// (`ERROR_BADPARM`), `None`. The entries' tissue type and voxel count are memory contents in
/// FreeSurfer, which no reached code reads: 0 here. **FS-417:** a table of no entries crashes
/// FreeSurfer ([`ctab_unique`]); both modes return it empty. **FS-420:** more than 2^20 entries are
/// refused (`CTABalloc: could not allocate <n> entries`).
pub fn ctab_alloc(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    nentries: i32,
) -> Result<Option<ColorTable>, Fatal> {
    if nentries < 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("CTABalloc: nentries was {nentries}"),
        );
        return Ok(None);
    }
    if nentries > MAX_ALLOC {
        g.notices.add(
            &FS_420,
            format!("CTABalloc of {nentries} entries (FreeSurfer's outcome depends on the machine's memory): refused"),
        );
        g.errno = 12;
        log::error_printf(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("CTABalloc: could not allocate {nentries} entries"),
        );
        return Ok(None);
    }
    let mut ct = ColorTable {
        entries: Vec::with_capacity(nentries as usize),
        fname: b"none".to_vec(),
        version: CTAB_VERSION_TO_WRITE,
        ..ColorTable::default()
    };
    for structure in 0..nentries {
        let ri = random_255(g, env, out, err)?;
        let gi = random_255(g, env, out, err)?;
        let bi = random_255(g, env, out, err)?;
        ct.entries.push(Some(Box::new(ColorTableEntry {
            name: format!("cluster{structure}").into_bytes(),
            ri,
            gi,
            bi,
            ai: 255,
            rf: unit(ri),
            gf: unit(gi),
            bf: unit(bi),
            af: unit(255),
            tissue_type: 0,
            count: 0,
        })));
    }
    ct.idbase = 0;
    ctab_unique(g, env, out, err, &mut ct, 100)?;
    Ok(Some(ct))
}

/// `CTABunique(ct, nmax)`: up to `nmax` times, [`ctab_random`] and stop when
/// [`ctab_count_repeats`] (stopping at the first repeat) finds none; then entry 0 becomes
/// `unknown`, colour and alpha 0 (floats too). After `nmax` tries (also when the last one found a
/// unique set: kept, rule C) `INFO: CTABunique() could not find a unique set in <nmax> tries` and
/// -1; else 0. **FS-417:** FreeSurfer crashes on a table with empty places or without entry 0;
/// both modes leave the empty places alone (no entry 0: none made).
pub fn ctab_unique(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ct: &mut ColorTable,
    nmax: i32,
) -> Result<i32, Fatal> {
    let mut n = 0;
    while n < nmax {
        n += 1;
        ctab_random(g, env, out, err, ct)?;
        if ctab_count_repeats(g, ct, true) == 0 {
            break;
        }
    }
    match ct.entries.first_mut() {
        Some(Some(e)) => {
            e.ri = 0;
            e.gi = 0;
            e.bi = 0;
            e.ai = 0;
            e.rf = 0.0;
            e.gf = 0.0;
            e.bf = 0.0;
            e.af = 0.0;
            e.name = b"unknown".to_vec();
        }
        _ => {
            g.notices.add(
                &FS_417,
                "CTABunique of a table without entry 0 (FreeSurfer crashes): none made",
            );
        }
    }
    if n == nmax {
        let _ = writeln!(
            out,
            "INFO: CTABunique() could not find a unique set in {nmax} tries"
        );
        return Ok(-1);
    }
    Ok(0)
}

/// `CTABcountRepeats(ct, break_after_found)`: for each entry, the later entries with the same float
/// red, green and blue, counted (only the first for each entry when `break_after_found`).
/// **FS-417:** FreeSurfer crashes on an empty place; both modes skip them.
pub fn ctab_count_repeats(g: &mut Globals, ct: &ColorTable, break_after_found: bool) -> i32 {
    if ct.entries.iter().any(Option::is_none) {
        g.notices.add(
            &FS_417,
            "CTABcountRepeats of a table with empty places (FreeSurfer crashes): skipped",
        );
    }
    let mut nrepeats = 0;
    for (i, a) in ct.entries.iter().enumerate() {
        let Some(a) = a else { continue };
        for b in ct.entries[i + 1..].iter().flatten() {
            if a.rf == b.rf && a.gf == b.gf && a.bf == b.bf {
                nrepeats += 1;
                if break_after_found {
                    break;
                }
            }
        }
    }
    nrepeats
}

/// `CTABrandom(ct)`: every entry's colour drawn again (red, green, blue in that order), alpha
/// 255; 0. **FS-417:** FreeSurfer crashes on an empty place; both modes skip them (no draw).
pub fn ctab_random(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ct: &mut ColorTable,
) -> Result<i32, Fatal> {
    for e in ct.entries.iter_mut() {
        let Some(e) = e else {
            g.notices.add(
                &FS_417,
                "CTABrandom of a table with empty places (FreeSurfer crashes): skipped",
            );
            continue;
        };
        e.ri = random_255(g, env, out, err)?;
        e.gi = random_255(g, env, out, err)?;
        e.bi = random_255(g, env, out, err)?;
        e.ai = 255;
        e.rf = unit(e.ri);
        e.gf = unit(e.gi);
        e.bf = unit(e.bi);
        e.af = unit(e.ai);
    }
    Ok(0)
}

// ---- counts, tests and lookups ------------------------------------------------------------------

/// `CTABgetNumberOfValidEntries(ct, &num)`: the entries that are not empty.
pub fn ctab_get_number_of_valid_entries(ct: &ColorTable) -> i32 {
    ct.entries.iter().filter(|e| e.is_some()).count() as i32
}

/// `CTABgetNumberOfTotalEntries(ct, &num)`: `nentries`.
pub fn ctab_get_number_of_total_entries(ct: &ColorTable) -> i32 {
    ct.nentries()
}

/// An index outside the table: `<who>: index <i> was OOB` (`ERROR_BADPARM`).
fn oob(g: &mut Globals, err: &mut dyn Write, ct: &ColorTable, index: i32, who: &str) -> bool {
    if index < 0 || index >= ct.nentries() {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{who}: index {index} was OOB"),
        );
        return true;
    }
    false
}

/// `CTABisEntryValid(ct, index, &valid)`: whether the place holds an entry, or `ERROR_BADPARM`
/// (`CTABisEntryValid: index <i> was OOB`).
pub fn ctab_is_entry_valid(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: &ColorTable,
    index: i32,
) -> Result<bool, i32> {
    if oob(g, err, ct, index, "CTABisEntryValid") {
        return Err(ERROR_BADPARM);
    }
    Ok(ct.entries[index as usize].is_some())
}

/// `CTABhasEntry(ct, index)`.
pub fn ctab_has_entry(ct: Option<&ColorTable>, index: i32) -> bool {
    ct.is_some_and(|ct| index >= 0 && index < ct.nentries() && ct.entries[index as usize].is_some())
}

/// `CTABrgbAtIndexi(ct, index, &r, &g, &b)`: the entry's integer colour; `ERROR_BADPARM` after
/// `CTABrgbAtIndexi: index <i> was OOB`, and silently for an empty place.
pub fn ctab_rgb_at_indexi(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: &ColorTable,
    index: i32,
) -> Result<(i32, i32, i32), i32> {
    if oob(g, err, ct, index, "CTABrgbAtIndexi") {
        return Err(ERROR_BADPARM);
    }
    match &ct.entries[index as usize] {
        Some(e) => Ok((e.ri, e.gi, e.bi)),
        None => Err(ERROR_BADPARM),
    }
}

/// `CTABcopyName(ct, index, name, name_len)`: the entry's name as `strncpy` copies it into a buffer
/// of `name_len` bytes (cut there, and then without its NUL: the caller's buffer holds no C string;
/// the reached callers give buffers longer than any name); `ERROR_BADPARM` after `CTABcopyName:
/// index <i> was OOB`, and silently for an empty place.
pub fn ctab_copy_name(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: &ColorTable,
    index: i32,
    name_len: usize,
) -> Result<Vec<u8>, i32> {
    if oob(g, err, ct, index, "CTABcopyName") {
        return Err(ERROR_BADPARM);
    }
    match &ct.entries[index as usize] {
        Some(e) => Ok(c_str(&e.name).iter().copied().take(name_len).collect()),
        None => Err(ERROR_BADPARM),
    }
}

/// `CTABrgb2Annotation(r, g, b)`: `(b << 16) + (g << 8) + r` in `int` (wrapping as the official
/// build's shifts and sums do).
pub fn ctab_rgb2_annotation(r: i32, g: i32, b: i32) -> i32 {
    b.wrapping_shl(16)
        .wrapping_add(g.wrapping_shl(8))
        .wrapping_add(r)
}

/// `CTABentryNameToIndex(name, ct)`: the first entry whose name equals `name` ignoring case
/// (`stricmp`), or -1.
pub fn ctab_entry_name_to_index(g: &mut Globals, name: &[u8], ct: &ColorTable) -> i32 {
    for (i, e) in ct.entries.iter().enumerate() {
        if let Some(e) = e
            && stricmp(g, &e.name, name) == 0
        {
            return i as i32;
        }
    }
    -1
}

/// `CTABentryNameToAnnotation(name, ct)`: that entry's annotation ([`ctab_rgb2_annotation`]), or
/// -1.
pub fn ctab_entry_name_to_annotation(g: &mut Globals, name: &[u8], ct: &ColorTable) -> i32 {
    let index = ctab_entry_name_to_index(g, name, ct);
    if index == -1 {
        return -1;
    }
    let e = ct.entries[index as usize].as_ref().expect("an entry");
    ctab_rgb2_annotation(e.ri, e.gi, e.bi)
}

/// `CTABannotationAtIndex(ct, index, &annot)`: the entry's annotation; `ERROR_BADPARM` after
/// `CTABannotationAtIndex: index <i> was OOB`, and silently for an empty place.
pub fn ctab_annotation_at_index(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: &ColorTable,
    index: i32,
) -> Result<i32, i32> {
    if oob(g, err, ct, index, "CTABannotationAtIndex") {
        return Err(ERROR_BADPARM);
    }
    match &ct.entries[index as usize] {
        Some(e) => Ok(ctab_rgb2_annotation(e.ri, e.gi, e.bi)),
        None => Err(ERROR_BADPARM),
    }
}

/// `CTABfindRGBi(ct, r, g, b, &index)`: the first entry of that integer colour, or -1;
/// `ERROR_BADPARM` after `CTABfindRGBi: rgb was invalid` for a component outside 0 to 255.
pub fn ctab_find_rgbi(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: &ColorTable,
    r: i32,
    gg: i32,
    b: i32,
) -> Result<i32, i32> {
    if !(0..256).contains(&r) || !(0..256).contains(&gg) || !(0..256).contains(&b) {
        log::error_printf(g, err, ERROR_BADPARM, "CTABfindRGBi: rgb was invalid");
        return Err(ERROR_BADPARM);
    }
    for (i, e) in ct.entries.iter().enumerate() {
        if let Some(e) = e
            && e.ri == r
            && e.gi == gg
            && e.bi == b
        {
            return Ok(i as i32);
        }
    }
    Ok(-1)
}

/// `CTABfindAnnotation(ct, annotation, &index)` (`CTABfindIndexFromAnnotation` too): the first
/// entry of the annotation's colour (its low three bytes), or -1.
pub fn ctab_find_annotation(ct: &ColorTable, annotation: i32) -> i32 {
    let (r, gg, b) = (
        annotation & 0xff,
        (annotation >> 8) & 0xff,
        (annotation >> 16) & 0xff,
    );
    for (i, e) in ct.entries.iter().enumerate() {
        if let Some(e) = e
            && e.ri == r
            && e.gi == gg
            && e.bi == b
        {
            return i as i32;
        }
    }
    -1
}

/// `CTABfindIndexFromAnnotation(ct, annot, &index)`: [`ctab_find_annotation`].
pub fn ctab_find_index_from_annotation(ct: &ColorTable, annot: i32) -> i32 {
    ctab_find_annotation(ct, annot)
}

/// `CTABfindName(ct, name, &index)`: the first entry whose name equals `name` ignoring case, or -1.
pub fn ctab_find_name(g: &mut Globals, ct: &ColorTable, name: &[u8]) -> i32 {
    for (i, e) in ct.entries.iter().enumerate() {
        if let Some(e) = e
            && stricmp(g, name, &e.name) == 0
        {
            return i as i32;
        }
    }
    -1
}

/// `CTABfindDuplicateNames(ct)`: every ordered pair of entries with the same name (`strcmp`) gives
/// `ERROR: label name '<name>' is duplicated!` on standard output, then, when any did,
/// `CTABfindDuplicateNames: <n> duplicate names found in:`, a newline, two spaces and the table's
/// file name; returns the pairs' number (the ordered pairs halved).
pub fn ctab_find_duplicate_names(out: &mut dyn Write, ct: &ColorTable) -> i32 {
    let mut dup: i32 = 0;
    for (i1, e1) in ct.entries.iter().enumerate() {
        let Some(e1) = e1 else { continue };
        for (i2, e2) in ct.entries.iter().enumerate() {
            if let Some(e2) = e2
                && i1 != i2
                && strcmp(&e1.name, &e2.name) == 0
            {
                dup += 1;
                let _ = out.write_all(b"ERROR: label name '");
                let _ = out.write_all(c_str(&e1.name));
                let _ = out.write_all(b"' is duplicated!\n");
            }
        }
    }
    if dup > 0 {
        let _ = write!(
            out,
            "CTABfindDuplicateNames: {} duplicate names found in:\n  ",
            dup / 2
        );
        let _ = out.write_all(c_str(&ct.fname));
        let _ = out.write_all(b"\n");
    }
    dup / 2
}

// ---- writing text tables ------------------------------------------------------------------------

/// `CTABprintASCII(ct, fp)`: with `Gdiag`'s `DIAG_INFO`, `[DEBUG] CTABprintASCII(): fname =
/// '<fname>' (<n> bytes)` on standard output; then each entry as `%3d  %-30s  %3d %3d %3d  %3d`:
/// its structure number plus `idbase`, its name without white space (`deblank`), its colour and
/// 255 minus alpha; `ERROR_NONE`.
pub fn ctab_print_ascii(
    g: &Globals,
    out: &mut dyn Write,
    ct: &ColorTable,
    fp: &mut dyn Write,
) -> i32 {
    if g.gdiag & DIAG_INFO != 0 {
        let f = c_str(&ct.fname);
        let mut m = b"[DEBUG] CTABprintASCII(): fname = '".to_vec();
        m.extend_from_slice(f);
        m.extend_from_slice(format!("' ({} bytes)\n", f.len()).as_bytes());
        let _ = out.write_all(&m);
    }
    for (structure, e) in ct.entries.iter().enumerate() {
        let Some(e) = e else { continue };
        let mut w = Vec::new();
        padded(&mut w, (structure as i32).wrapping_add(ct.idbase), 3);
        w.extend_from_slice(b"  ");
        left30(&mut w, &utils::deblank(&e.name));
        w.extend_from_slice(b"  ");
        padded(&mut w, e.ri, 3);
        w.push(b' ');
        padded(&mut w, e.gi, 3);
        w.push(b' ');
        padded(&mut w, e.bi, 3);
        w.extend_from_slice(b"  ");
        padded(&mut w, 255i32.wrapping_sub(e.ai), 3);
        w.push(b'\n');
        let _ = fp.write_all(&w);
    }
    ERROR_NONE
}

/// `CTABprintASCII(ct, stdout)`: [`ctab_print_ascii`] with the table on standard output.
pub fn ctab_print_ascii_stdout(g: &Globals, out: &mut dyn Write, ct: &ColorTable) -> i32 {
    let mut lines = Vec::new();
    let mut debug = Vec::new();
    let r = ctab_print_ascii(g, &mut debug, ct, &mut lines);
    let _ = out.write_all(&debug);
    let _ = out.write_all(&lines);
    r
}

/// `CTABwriteFileASCII(ct, fname)`: [`ctab_print_ascii`] into a new file; a file that does not
/// open: `CTABwriteFileASCII(<fname>): could not open for writing` (`ERROR_NOFILE`).
pub fn ctab_write_file_ascii(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    ct: &ColorTable,
    fname: &str,
) -> i32 {
    let mut fp = match CFile::open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                err,
                ERROR_NOFILE,
                &format!("CTABwriteFileASCII({fname}): could not open for writing\n"),
            );
            return ERROR_NOFILE;
        }
    };
    let r = ctab_print_ascii(g, out, ct, &mut fp);
    fp.close();
    r
}

// ---- tissue types -------------------------------------------------------------------------------

/// The tissue classes of a schema: (name, r, g, b).
const TT_CLASSES: [(&str, i32, i32, i32); 6] = [
    ("unknown", 0, 0, 0),
    ("cortex", 205, 62, 78),
    ("subcort_gm", 230, 148, 34),
    ("wm", 0, 255, 0),
    ("csf", 120, 18, 134),
    ("head", 150, 150, 200),
];

/// `TissueTypeSchema(ct, schema)`: `default-jan-2014` ([`tissue_type_schema_default`]),
/// `default-jan-2014+head` or `default-apr-2019+head` ([`tissue_type_schema_default_head`]); any
/// other: `ERROR: tissue type schema <schema> unrecognized` (standard output), `None`.
pub fn tissue_type_schema(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    ct: Option<ColorTable>,
    schema: &str,
) -> Result<Option<ColorTable>, Fatal> {
    match schema {
        "default-jan-2014" => tissue_type_schema_default(g, env, out, err, dir, ct),
        "default-jan-2014+head" | "default-apr-2019+head" => {
            tissue_type_schema_default_head(g, env, out, err, dir, ct)
        }
        _ => {
            let _ = writeln!(out, "ERROR: tissue type schema {schema} unrecognized");
            Ok(None)
        }
    }
}

/// The table a schema starts from: `ct`, or (`None`) `FSENVgetenv` and [`ctab_read_ascii`] of
/// `$FREESURFER_HOME/FreeSurferColorLUT.txt`. **FS-416:** FreeSurfer crashes when either gives
/// nothing; both modes `None`.
fn schema_table(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    ct: Option<ColorTable>,
    who: &str,
) -> Option<ColorTable> {
    if ct.is_some() {
        return ct;
    }
    let home = fsenv_home(g, env, out, err, dir, who)?;
    let ct = ctab_read_ascii(g, out, err, dir, &format!("{home}/FreeSurferColorLUT.txt"));
    if ct.is_none() {
        g.notices.add(
            &FS_416,
            format!("{who} without the default colour table (FreeSurfer crashes): no table"),
        );
    }
    ct
}

/// The schema's tissue-type table: [`ctab_alloc`] of `n` (random colours, the random numbers
/// drawn), then each class's name and integer colour; the float colours stay those drawn (entry
/// 0's zero) and alpha 255 (entry 0's 0). **FS-419** (kept, rule C): the float colours do not
/// match the integer ones; no reached code reads them.
fn schema_classes(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    n: usize,
) -> Result<Option<ColorTable>, Fatal> {
    let Some(mut tt) = ctab_alloc(g, env, out, err, n as i32)? else {
        return Ok(None);
    };
    g.notices.add(
        &FS_419,
        "a tissue-type table's float colours left as drawn at random (kept)",
    );
    for (k, &(name, r, gg, b)) in TT_CLASSES[..n].iter().enumerate() {
        if let Some(e) = tt.entries[k].as_mut() {
            e.name = name.as_bytes().to_vec();
            e.ri = r;
            e.gi = gg;
            e.bi = b;
        }
    }
    Ok(Some(tt))
}

const TT_UNKNOWN: i32 = 0;
const TT_CTX_GM: i32 = 1;
const TT_SUBCTX_GM: i32 = 2;
const TT_WM: i32 = 3;
const TT_CSF: i32 = 4;
const TT_HEAD: i32 = 5;

/// The ranges both schemas assign after their lists (cortex and white-matter parcellations), with
/// `head`'s two more.
fn tt_ranges(n: i32, head: bool) -> i32 {
    let mut tt = -1;
    let within = |lo: i32, hi: i32| n >= lo && n <= hi;
    if within(1000, 1035) || within(2000, 2035) {
        tt = TT_CTX_GM;
    }
    if within(3000, 3035) || within(4000, 4035) {
        tt = TT_WM;
    }
    if head && (within(3201, 3207) || within(4201, 4207)) {
        tt = TT_WM;
    }
    if within(1100, 1181) || within(2100, 2181) {
        tt = TT_CTX_GM;
    }
    if within(3100, 3181) || within(4100, 4181) {
        tt = TT_WM;
    }
    if n == 5001 || n == 5002 {
        tt = TT_WM;
    }
    if within(11100, 11300) || within(12100, 12300) {
        tt = TT_CTX_GM;
    }
    tt
}

/// The `default-jan-2014` schema's tissue type of structure `n` (-1: none).
fn tt_default(n: i32) -> i32 {
    match n {
        0 => TT_UNKNOWN,
        3 | 42 => TT_CTX_GM,
        8 | 47 | 17 | 53 | 18 | 54 | 13 | 52 | 10 | 49 | 51 | 12 | 50 | 11 | 26 | 58 | 28 | 60
        | 16 | 126 | 174 | 29 | 61 | 80 => TT_SUBCTX_GM,
        2 | 41 | 7 | 46 | 690 | 691 | 77 | 78 | 79 | 85 | 930 | 192 | 251 | 252 | 253 | 254
        | 255 | 34 | 66 => TT_WM,
        14 | 15 | 24 | 257 | 4 | 43 | 5 | 44 | 31 | 63 | 72 | 30 | 62 | 914 => TT_CSF,
        _ => tt_ranges(n, false),
    }
}

/// The `default-apr-2019+head` schema's tissue type of structure `n` (-1: none).
fn tt_default_head(n: i32) -> i32 {
    match n {
        0 => TT_UNKNOWN,
        3 | 42 => TT_CTX_GM,
        8 | 47 | 172 | 179 | 17 | 53 | 18 | 54 | 13 | 52 | 10 | 49 | 51 | 12 | 50 | 11 | 26
        | 58 | 28 | 60 | 16 | 126 | 174 | 267 | 80 | 29 | 61 | 819 | 820 | 843 | 844 | 865
        | 866 | 869 | 870 => TT_SUBCTX_GM,
        2 | 41 | 7 | 46 | 690 | 691 | 77 | 78 | 79 | 85 | 930 | 192 | 251 | 252 | 253 | 254
        | 255 | 34 | 66 | 821 | 822 | 853 => TT_WM,
        14 | 15 | 24 | 257 | 4 | 75 | 43 | 76 | 5 | 44 | 31 | 63 | 72 | 30 | 62 | 914 => TT_CSF,
        258 | 165 | 259 | 118 | 119 | 120 | 121 | 122 | 123 | 124 | 127 | 129 | 130 | 131 | 132
        | 133 | 134 | 135 | 143 | 144 | 145 | 167 | 262 | 263 | 264 | 902 | 907 | 908 | 909
        | 911 | 915 | 916 => TT_HEAD,
        _ => tt_ranges(n, true),
    }
}

/// `TissueTypeSchemaDefault(ct)`: the table (`None`: `FREESURFER_HOME`'s default table, FS-416)
/// with schema `default-jan-2014`, a tissue-type table of five classes (unknown, cortex,
/// subcort_gm, wm, csf; [`schema_classes`]: five random colours drawn, the earlier tissue-type
/// table dropped) and each entry's tissue type by FreeSurfer's lists (-1 where none applies).
pub fn tissue_type_schema_default(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    ct: Option<ColorTable>,
) -> Result<Option<ColorTable>, Fatal> {
    let Some(mut ct) = schema_table(g, env, out, err, dir, ct, "TissueTypeSchemaDefault") else {
        return Ok(None);
    };
    ct.tissue_type_schema = "default-jan-2014".to_owned();
    ct.ctab_tissue_type = schema_classes(g, env, out, err, 5)?.map(Box::new);
    for (n, e) in ct.entries.iter_mut().enumerate() {
        if let Some(e) = e {
            e.tissue_type = tt_default(n as i32);
        }
    }
    Ok(Some(ct))
}

/// `TissueTypeSchemaDefaultHead(ct)`: `Entering TissueTypeSchemaDefaultHead()` (standard output),
/// the table as [`tissue_type_schema_default`] gives it, schema `default-apr-2019+head` (`schema
/// <schema>` printed), six classes (`head` the sixth) and the head schema's lists.
pub fn tissue_type_schema_default_head(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    ct: Option<ColorTable>,
) -> Result<Option<ColorTable>, Fatal> {
    let _ = out.write_all(b"Entering TissueTypeSchemaDefaultHead()\n");
    let Some(mut ct) = schema_table(g, env, out, err, dir, ct, "TissueTypeSchemaDefaultHead")
    else {
        return Ok(None);
    };
    ct.tissue_type_schema = "default-apr-2019+head".to_owned();
    let _ = writeln!(out, "schema {}", ct.tissue_type_schema);
    ct.ctab_tissue_type = schema_classes(g, env, out, err, 6)?.map(Box::new);
    for (n, e) in ct.entries.iter_mut().enumerate() {
        if let Some(e) = e {
            e.tissue_type = tt_default_head(n as i32);
        }
    }
    Ok(Some(ct))
}

/// `CTABmerge(ct, merge)`: each entry of `merge` copied whole (name, colours, tissue type, voxel
/// count) into the same place of `ct`, replacing what was there; 0. **FS-418:** a `merge` table
/// with more places than `ct` makes FreeSurfer write past `ct`'s list; by default `ct` grows to
/// hold them (rule A), `reproduce` leaves those entries out.
pub fn ctab_merge(g: &mut Globals, ct: &mut ColorTable, merge: &ColorTable) -> i32 {
    for (n, e) in merge.entries.iter().enumerate() {
        let Some(e) = e else { continue };
        if n >= ct.entries.len() {
            g.notices.add(
                &FS_418,
                format!(
                    "CTABmerge of a {}-place table into a {}-place one",
                    merge.nentries(),
                    ct.entries.len()
                ),
            );
            if g.notices.reproduce() {
                continue;
            }
            ct.entries.resize(n + 1, None);
        }
        ct.entries[n] = Some(e.clone());
    }
    0
}

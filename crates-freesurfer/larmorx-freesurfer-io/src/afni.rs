// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! AFNI BRIK volumes (`utils/afni.cpp`, item 3.1-27): `afniRead` ([`afni_read`]: the `.HEAD`'s
//! attributes read by `readAFNIHeader` with `get_afni_string`, `get_afni_int` and
//! `get_afni_float`, printed by `printAFNIHeader`, then the `.BRIK`'s voxels with
//! `findMinMaxByte`, `findMinMaxShort` and `findMinMaxFloat`) and `afniWrite` ([`afni_write`]:
//! FreeSurfer 8.2.0 refuses before anything else). Wired into [`crate::mriio::mri_read`] and
//! [`crate::mriio::mri_write_type`].
//!
//! FreeSurfer keeps the header in a structure (`AF`) whose `TYPESTRING` holds 15 characters and
//! `BYTEORDER_STRING` 9; a longer one ends the program in the C library's overflow check (FS-702:
//! `*** buffer overflow detected ***: terminated`, signal 6, as the official build has it); the
//! default keeps them whole.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 24):
//! FS-693 (a header that cannot be read is read for ever), FS-694 (the header's last line without
//! a newline), FS-695 (a count before any type or name line), FS-696 (a bare `count` line), FS-697
//! (a token that is not a number), FS-698 and FS-710 (more numbers than the count, within and past
//! the allocation), FS-699, FS-714 and FS-715 (negative, -1 and huge counts), FS-700 (an attribute
//! of a type FreeSurfer does not check), FS-701 (fewer values than FreeSurfer reads), FS-702 (long
//! strings: the C library's overflow abort), FS-703 and FS-716 (`IDCODE_STRING` without its `~`),
//! FS-704 and FS-711 (the brick types' names), FS-705 (no `BRICK_FLOAT_FACS` or `BRICK_STATS`),
//! FS-706 and FS-712 (frames scaled differently), FS-707 (a volume of no voxels), FS-708 and FS-713
//! (the messages' names and numbers), FS-709 (a folder in the BRIK's place), FS-249 (names past
//! FreeSurfer's buffer). Each variant that decides whether a probe line compares has its own ID
//! (a run keeps one notice per defect).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    Defect, FS_249, FS_693, FS_694, FS_695, FS_696, FS_697, FS_698, FS_699, FS_700, FS_701, FS_702,
    FS_703, FS_704, FS_705, FS_706, FS_707, FS_708, FS_709, FS_710, FS_711, FS_712, FS_713, FS_714,
    FS_715, FS_716,
};
use larmorx_freesurfer_core::cscan::{self, Bytes, Scan};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::{cstd, sse, utils};
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_SHORT, MRI_UCHAR, Mri};

use crate::cfile::CFile;
use crate::mriio_nifti::{narrow, widen};

/// `STRLEN`: FreeSurfer's line and name buffers.
const STRLEN: usize = 1024;

/// `afni_orientations`: the direction each `ORIENT_SPECIFIC` code gives an axis.
const ORIENTATIONS: [[f32; 3]; 6] = [
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

// the flags of the required attributes
const ORIENT_SPECIFIC_FLAG: u32 = 0x01;
const BRICK_TYPES_FLAG: u32 = 0x02;
const DATASET_DIMENSIONS_FLAG: u32 = 0x04;
const DELTA_FLAG: u32 = 0x08;
const ORIGIN_FLAG: u32 = 0x10;
const BYTEORDER_STRING_FLAG: u32 = 0x20;
const AFNI_ALL_REQUIRED: u32 = 0x3f;

// `AF`'s layout on x86-64 (GCC): the offsets of its fields
const RANK: usize = 0;
const DIMS: usize = 8;
const SCENE: usize = 36;
const ORIENT: usize = 48;
const ORIGIN: usize = 60;
const DELTA: usize = 72;
const NUMSTATS: usize = 120;
const NUMTYPES: usize = 136;
const NUMFACS: usize = 152;
const AF_SIZE: usize = 160;
/// The two strings' fields: their characters at most (one more for the NUL).
const TYPESTRING_FIELD: usize = 16;
const BYTEORDER_FIELD: usize = 10;

/// A defect's notice with its detail.
fn note(g: &mut Globals, d: &'static Defect, detail: impl Into<String>) {
    g.notices.add(d, detail.into());
}

/// A C string's bytes: up to its first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `ErrorReturn(NULL, (code, message))` after `errno = 0`.
fn fail0(g: &mut Globals, err: &mut dyn Write, code: i32, message: &[u8]) {
    g.errno = 0;
    log::error_printf_bytes(g, err, code, message);
}

/// The bytes glibc's `malloc(n)` gives a caller (a chunk of at least 32 bytes, in steps of 16,
/// less its 8-byte size field); `None` for a size it refuses (beyond the address space).
fn malloc_usable(n: u64) -> Option<u64> {
    if n > (1 << 62) {
        return None;
    }
    Some(((n + 8 + 15) & !15).max(32) - 8)
}

/// Sizes beyond this the machine's memory decides (the probe's limit is 1 GiB).
const MACHINE_DECIDES: u64 = 1 << 28;

/// FreeSurfer's `AF` header: its numbers as `afniRead`'s stack holds them (x86-64's layout), its
/// two strings, and the arrays its pointers hold (`None`: a null pointer).
struct Af {
    b: [u8; AF_SIZE],
    typestring: Vec<u8>,
    byteorder: Vec<u8>,
    idcode: Option<Vec<u8>>,
    stats: Option<Vec<f32>>,
    types: Option<Vec<i32>>,
    facs: Option<Vec<f32>>,
}

impl Af {
    /// `AFinit`: every field 0 or empty.
    fn init() -> Af {
        Af {
            b: [0; AF_SIZE],
            typestring: Vec::new(),
            byteorder: Vec::new(),
            idcode: None,
            stats: None,
            types: None,
            facs: None,
        }
    }

    fn i32(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.b[off..off + 4].try_into().unwrap())
    }

    fn f32(&self, off: usize) -> f32 {
        f32::from_le_bytes(self.b[off..off + 4].try_into().unwrap())
    }

    fn set(&mut self, off: usize, v: [u8; 4]) {
        self.b[off..off + 4].copy_from_slice(&v);
    }
}

/// The numbers `get_afni_int` or `get_afni_float` read: what FreeSurfer's buffer holds (its
/// `usable` places: the values written there, memory contents past them).
struct NumBuf<T> {
    vals: Vec<T>,
    usable: usize,
}

impl<T: Copy> NumBuf<T> {
    /// Place `i`, `None` where FreeSurfer's buffer holds memory never written.
    fn get(&self, i: usize) -> Option<T> {
        if i < self.vals.len() && i < self.usable {
            Some(self.vals[i])
        } else {
            None
        }
    }
}

/// `strtol(s, &e, 10)` stored in an `int`: the low 32 bits of the `long`, the bytes used (0: no
/// conversion) and whether it set `ERANGE`.
fn strtol_int(s: &[u8]) -> (i32, usize, bool) {
    let mut i = 0;
    while i < s.len() && cstd::is_space(s[i]) {
        i += 1;
    }
    let neg = matches!(s.get(i), Some(b'-'));
    if matches!(s.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let first = i;
    let mut v: i128 = 0;
    let mut over = false;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v * 10 + i128::from(s[i] - b'0');
        if v > i128::from(i64::MAX) + 1 {
            over = true;
            v = i128::from(i64::MAX) + 1;
        }
        i += 1;
    }
    if i == first {
        return (0, 0, false);
    }
    let v = if neg { -v } else { v };
    let (l, erange) = if v > i128::from(i64::MAX) {
        (i64::MAX, true)
    } else if v < i128::from(i64::MIN) {
        (i64::MIN, true)
    } else {
        (v as i64, over)
    };
    (l as i32, i, erange)
}

/// `(float) strtod(s, &e)`: the value, the bytes used (0: no conversion) and whether `strtod` set
/// `ERANGE`.
fn strtod_float(s: &[u8]) -> (f32, usize, bool) {
    let (v, used) = larmorx_core::cscan::strtod(s);
    if used == 0 {
        return (0.0, 0, false);
    }
    // the same characters through scanf's reader (glibc's shared scanner) give strtod's ERANGE
    let mut st = Bytes::new(&s[..used]);
    let erange = matches!(cscan::scan_f64(&mut st), Scan::Value(_, true));
    (narrow(v), used, erange)
}

/// The two number readers' names in their messages.
struct Kind {
    who: &'static str,
    what: &'static str,
}

const INTS: Kind = Kind {
    who: "get_afni_int",
    what: "ints",
};
const FLOATS: Kind = Kind {
    who: "get_afni_float",
    what: "floats",
};

/// `get_afni_int(fp, count, name)` and `get_afni_float(...)`: `count` numbers from the lines after
/// the count line. The lines are read whole (1,023 bytes at most each); each token is a number
/// (`strtol` base 10, or `strtod` stored as a `float`); a line that ends the file, a blank line or
/// a line starting with `type` ends the reading with `<who>(): hit EOF while reading <n> <what> for
/// <name>` (`... hit a blank line ...`, `... hit a type line ...`). `None` after a message, or with
/// none where FreeSurfer's allocation fails (a negative count) or the run is not comparable.
#[allow(clippy::too_many_arguments)]
fn get_numbers<T: Copy + Default>(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut CFile,
    count: i32,
    name: &[u8],
    kind: &Kind,
    parse: impl Fn(&[u8]) -> (T, usize, bool),
) -> Option<NumBuf<T>> {
    let rep = g.notices.reproduce();
    let nm = text(name);
    // malloc(count * sizeof): a negative count is a size malloc refuses
    let bytes = (i64::from(count) as u64).wrapping_mul(4);
    let usable = if count < 0 {
        None
    } else {
        malloc_usable(bytes)
    };
    if count < 0 {
        note(
            g,
            &FS_699,
            format!(
                "{}: a negative count ({count}) for {nm}: FreeSurfer's reader fails without a message",
                kind.who
            ),
        );
        if rep {
            // malloc's failure leaves errno at ENOMEM
            g.errno = 12;
        } else {
            fail0(
                g,
                err,
                ERROR_BADPARM,
                format!("{}(): bad count {count} for {nm}", kind.who).as_bytes(),
            );
        }
        return None;
    }
    if bytes > MACHINE_DECIDES {
        note(
            g,
            &FS_715,
            format!(
                "{}: a count of {count} for {nm}: whether FreeSurfer's buffer of {bytes} bytes can be had is the machine's (FreeSurfer: not deterministic)",
                kind.who
            ),
        );
    }
    let usable = (usable.unwrap_or(0) / 4) as usize;
    let mut vals: Vec<T> = Vec::new();
    let mut i: i64 = 0;
    let (mut extra_noted, mut beyond_noted) = (false, false);
    // the reader's own line buffer (what it held before is never seen: a read that finds nothing
    // ends the reading)
    let mut line: Vec<u8> = Vec::new();
    while i < i64::from(count) {
        let got = fp.fgets(STRLEN);
        if got.is_none() && fp.ferror() {
            log::error_printf(g, err, ERROR_BADFILE, "failed reading file");
        }
        if let Some(l) = &got {
            line = l.clone();
        }
        let l = cstr(&line).to_vec();
        let blank = l.iter().all(|&c| cstd::is_space(c));
        if fp.feof() {
            // FS-694: a last line without a newline sets the end of the file as it is read
            let read_one = got.as_ref().is_some_and(|x| !x.is_empty()) && !blank;
            if read_one {
                note(
                    g,
                    &FS_694,
                    if rep {
                        "an AFNI header's last line (numbers, without a newline) ignored, as in FreeSurfer"
                    } else {
                        "an AFNI header's last line (numbers, without a newline) read (FreeSurfer ignores it: 'hit EOF')"
                    },
                );
            }
            if rep || !read_one {
                fail0(
                    g,
                    err,
                    ERROR_BADPARM,
                    format!(
                        "{}(): hit EOF while reading {count} {} for {nm}",
                        kind.who, kind.what
                    )
                    .as_bytes(),
                );
                return None;
            }
        }
        if blank {
            fail0(
                g,
                err,
                ERROR_BADPARM,
                format!(
                    "{}(): hit a blank line while reading {count} {} for {nm}",
                    kind.who, kind.what
                )
                .as_bytes(),
            );
            return None;
        }
        if l.starts_with(b"type") {
            fail0(
                g,
                err,
                ERROR_BADPARM,
                format!(
                    "{}(): hit a type line while reading {count} {} for {nm}",
                    kind.who, kind.what
                )
                .as_bytes(),
            );
            return None;
        }
        let mut p = 0;
        while p < l.len() {
            while p < l.len() && cstd::is_space(l[p]) {
                p += 1;
            }
            if p >= l.len() {
                break;
            }
            let (v, used, erange) = parse(&l[p..]);
            if used == 0 {
                let end = l[p..]
                    .iter()
                    .position(|&c| cstd::is_space(c))
                    .map_or(l.len(), |e| p + e);
                let tok = text(&l[p..end]);
                note(
                    g,
                    &FS_697,
                    format!(
                        "{}: '{tok}' is not a number (FreeSurfer: not deterministic: it writes past its buffer until it crashes)",
                        kind.who
                    ),
                );
                if !rep {
                    fail0(
                        g,
                        err,
                        ERROR_BADPARM,
                        format!(
                            "{}(): '{tok}' is not a number (reading {count} {} for {nm})",
                            kind.who, kind.what
                        )
                        .as_bytes(),
                    );
                }
                return None;
            }
            if erange {
                g.errno = cscan::ERANGE;
            }
            if i < i64::from(count) {
                vals.push(v);
            } else {
                if i as u64 >= usable as u64 {
                    if !beyond_noted {
                        beyond_noted = true;
                        note(
                            g,
                            &FS_710,
                            format!(
                                "{}: more numbers than the count ({count}) for {nm}, beyond FreeSurfer's buffer: ignored (FreeSurfer writes them past it: not deterministic)",
                                kind.who
                            ),
                        );
                    }
                } else if !extra_noted {
                    extra_noted = true;
                    note(
                        g,
                        &FS_698,
                        if rep {
                            format!(
                                "{}: more numbers than the count ({count}) for {nm}: kept past the count within FreeSurfer's buffer, as in FreeSurfer",
                                kind.who
                            )
                        } else {
                            format!(
                                "{}: more numbers than the count ({count}) for {nm}: the extra ones ignored (FreeSurfer keeps them past the count, within its buffer)",
                                kind.who
                            )
                        },
                    );
                }
                if rep {
                    vals.push(v);
                }
            }
            i += 1;
            p += used;
        }
    }
    let usable = if rep { usable } else { count as usize };
    Some(NumBuf { vals, usable })
}

/// `get_afni_string(fp, count, name)`: the string attribute's `count` bytes after its opening `'`
/// (`get_afni_string(): afni header string <name> does not start with "'"`), each `~` read as a
/// NUL; the file ending first gives `get_afni_string(): end of file reached at <i> of <n> bytes of
/// string <name>`; a last byte that is not `~` gives `warning: string <name> does not end with "~"`
/// (the string kept). The rest of the line is skipped. Returns the `count` bytes; `None` after a
/// message.
fn get_string(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut CFile,
    count: i32,
    name: &[u8],
) -> Result<Option<Vec<u8>>, Fatal> {
    let rep = g.notices.reproduce();
    let nm = text(name);
    // malloc(count + 1), the sum in int
    let n = count.wrapping_add(1);
    if count < 0 {
        note(
            g,
            &FS_699,
            format!("get_afni_string: a negative count ({count}) for {nm}"),
        );
        if !rep {
            fail0(
                g,
                err,
                ERROR_BADPARM,
                format!("get_afni_string(): bad count {count} for string {nm}").as_bytes(),
            );
            return Ok(None);
        }
    }
    let buf_null = n < 0;
    if n >= 0 && n as u64 > MACHINE_DECIDES {
        note(
            g,
            &FS_715,
            format!(
                "get_afni_string: a count of {count} for {nm}: whether FreeSurfer's buffer can be had is the machine's (FreeSurfer: not deterministic)"
            ),
        );
    }
    let c = fp.getc();
    if c != i32::from(b'\'') {
        fail0(
            g,
            err,
            ERROR_BADPARM,
            format!("get_afni_string(): afni header string {nm} does not start with \"'\"")
                .as_bytes(),
        );
        return Ok(None);
    }
    if rep && buf_null {
        // the string written through a null pointer (count -2 and below, or past INT_MAX)
        note(
            g,
            &FS_699,
            format!(
                "get_afni_string: the string {nm} written through a null pointer: FreeSurfer crashes (signal 11)"
            ),
        );
        return Err(Fatal::Exit(139));
    }
    if rep && count == -1 {
        note(
            g,
            &FS_714,
            format!(
                "get_afni_string: a count of -1 for {nm}: the string is memory contents (FreeSurfer: memory contents)"
            ),
        );
    }
    let mut buf = Vec::new();
    let mut i: i64 = 0;
    while i < i64::from(count) {
        if fp.feof() {
            fail0(
                g,
                err,
                ERROR_BADPARM,
                format!(
                    "get_afni_string(): end of file reached at {} of {count} bytes of string {nm}",
                    i + 1
                )
                .as_bytes(),
            );
            return Ok(None);
        }
        let c = fp.getc();
        let b = c as u8; // EOF stored as the byte 0xff, as FreeSurfer's char
        if i == i64::from(count) - 1 && b != b'~' {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("warning: string {nm} does not end with \"~\""),
            );
        }
        buf.push(if b == b'~' { 0 } else { b });
        i += 1;
    }
    loop {
        let c = fp.getc();
        if c == i32::from(b'\n') || fp.feof() {
            break;
        }
    }
    Ok(Some(buf))
}

/// `line2`, the line without its white space: its bytes stay past the current line's end, where
/// an earlier, longer line left them (`written` bytes were ever written).
struct Line2 {
    buf: Vec<u8>,
    written: usize,
}

impl Line2 {
    fn put(&mut self, s: &[u8]) {
        if self.buf.len() < s.len() + 1 {
            self.buf.resize(s.len() + 1, 0);
        }
        self.buf[..s.len()].copy_from_slice(s);
        self.buf[s.len()] = 0;
        self.written = self.written.max(s.len() + 1);
    }
}

/// What a header attribute's count line gave.
enum Values {
    S(Option<Vec<u8>>),
    F(Option<NumBuf<f32>>),
    I(Option<NumBuf<i32>>),
}

/// FS-700: an attribute whose type FreeSurfer does not check before it reads its values (through
/// a null pointer: signal 11). `Ok(true)`: go on (never); `Ok(false)`: the default's message given.
fn wrong_type(
    g: &mut Globals,
    err: &mut dyn Write,
    name: &str,
    ty: &[u8],
    expect: &str,
) -> Result<(), Fatal> {
    note(
        g,
        &FS_700,
        format!(
            "read_afni_header: {name} listed as {} (FreeSurfer reads it as {expect} through a null pointer: a crash)",
            text(ty)
        ),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    fail0(
        g,
        err,
        ERROR_BADPARM,
        format!(
            "read_afni_header(): variable {name} listed as {} (expecting {expect})",
            text(ty)
        )
        .as_bytes(),
    );
    Ok(())
}

/// FS-702: a string longer than its field. FreeSurfer's copy ends the program in the C library's
/// overflow check (`reproduce`: the message and status 134); the default keeps it whole.
fn too_long(
    g: &mut Globals,
    err: &mut dyn Write,
    name: &str,
    len: usize,
    field: usize,
) -> Result<(), Fatal> {
    if len < field {
        return Ok(());
    }
    note(
        g,
        &FS_702,
        format!(
            "an AFNI {name} of {len} characters kept whole (FreeSurfer's copy into its {field}-byte field ends the program: signal 6)"
        ),
    );
    if g.notices.reproduce() {
        let _ = err.write_all(b"*** buffer overflow detected ***: terminated\n");
        return Err(Fatal::Exit(134));
    }
    Ok(())
}

/// A value FreeSurfer reads where its buffer holds memory never written (FS-701): 0 in both modes.
fn short_of<T: Copy + Default>(g: &mut Globals, v: Option<T>, what: &str) -> T {
    v.unwrap_or_else(|| {
        note(
            g,
            &FS_701,
            format!("{what}: fewer values than FreeSurfer reads: 0 (FreeSurfer: memory contents)"),
        );
        T::default()
    })
}

fn not_enough(g: &mut Globals, err: &mut dyn Write, name: &str, count: i32) {
    fail0(
        g,
        err,
        ERROR_BADPARM,
        format!("read_afni_header(): not enough variables in {name} (need 3, have {count}")
            .as_bytes(),
    );
}

fn listed_as(g: &mut Globals, err: &mut dyn Write, name: &str, ty: &[u8], expect: &str) {
    fail0(
        g,
        err,
        ERROR_BADPARM,
        format!(
            "read_afni_header(): variable {name} listed as {} (expecting {expect})",
            text(ty)
        )
        .as_bytes(),
    );
}

/// `readAFNIHeader(fp, &af)`: the header's attributes, each a `type = ...`, `name = ...` and
/// `count = ...` line (white space anywhere ignored) followed by its values. See
/// `specs/freesurfer-io.md` §118.
fn read_afni_header(
    g: &mut Globals,
    err: &mut dyn Write,
    mut fp: CFile,
    af: &mut Af,
) -> Result<bool, Fatal> {
    let rep = g.notices.reproduce();
    let mut errno = g.errno;
    fp.fseek(&mut errno, 0, 0);
    g.errno = errno;
    let mut line: Vec<u8> = Vec::new();
    let mut line2 = Line2 {
        buf: Vec::new(),
        written: 0,
    };
    let mut ty: Option<Vec<u8>> = None;
    let mut name: Option<Vec<u8>> = None;
    let mut gotten = 0u32;
    loop {
        let got = fp.fgets(STRLEN);
        if got.is_none() && fp.ferror() {
            if let Some(e) = fp.read_errno() {
                g.errno = e;
            }
            log::error_printf(g, err, ERROR_BADFILE, "failed reading file");
            if !fp.feof() {
                // FS-693: FreeSurfer reads the stale line again for ever
                note(
                    g,
                    &FS_693,
                    "readAFNIHeader: a header that cannot be read: one message, read as empty (FreeSurfer reads it for ever: not deterministic)",
                );
                break;
            }
        }
        let last = fp.feof();
        if last {
            // FS-694: a last line without a newline is ignored
            let l = got.as_deref().unwrap_or(&[]);
            if cstr(l).starts_with(b"count") || l.iter().any(|c| !cstd::is_space(*c)) {
                note(
                    g,
                    &FS_694,
                    if rep {
                        "an AFNI header's last line (without a newline) ignored, as in FreeSurfer"
                    } else {
                        "an AFNI header's last line (without a newline) read (FreeSurfer ignores it)"
                    },
                );
            }
            if rep || got.as_ref().is_none_or(|x| x.is_empty()) {
                break;
            }
        }
        if let Some(l) = &got {
            line = l.clone();
        }
        let stripped: Vec<u8> = cstr(&line)
            .iter()
            .copied()
            .filter(|c| !cstd::is_space(*c))
            .collect();
        line2.put(&stripped);
        let l2 = &line2.buf[..stripped.len()];
        if l2.starts_with(b"type=") {
            ty = Some(l2[5..].to_vec());
        }
        if l2.starts_with(b"name=") {
            name = Some(l2[5..].to_vec());
        }
        if l2.starts_with(b"count") {
            // atoi(&line2[6]): a line of 'count' alone reads past its end
            let count = if stripped.len() >= 6 {
                cstd::atoi(cstr(&line2.buf[6..]))
            } else {
                let known = line2.written > 6;
                let stale = if known {
                    cstr(&line2.buf[6..line2.written]).to_vec()
                } else {
                    Vec::new()
                };
                note(
                    g,
                    &FS_696,
                    if !known {
                        "an AFNI header's 'count' line without its number: 0 (FreeSurfer: memory contents)".to_string()
                    } else if rep {
                        "an AFNI header's 'count' line without its number: the number read past its end, from an earlier line, as in FreeSurfer".to_string()
                    } else {
                        "an AFNI header's 'count' line without its number: 0 (FreeSurfer reads past its end, from an earlier line)".to_string()
                    },
                );
                if rep { cstd::atoi(&stale) } else { 0 }
            };
            let tyv = match &ty {
                Some(t) => t.clone(),
                None => {
                    note(
                        g,
                        &FS_695,
                        "an AFNI header's count line before any type line: no type (FreeSurfer: memory contents)",
                    );
                    Vec::new()
                }
            };
            let nmv = match &name {
                Some(n) => n.clone(),
                None => {
                    note(
                        g,
                        &FS_695,
                        "an AFNI header's count line before any name line: no name (FreeSurfer: memory contents)",
                    );
                    Vec::new()
                }
            };
            let tyc = cstr(&tyv);
            let nmc = cstr(&nmv).to_vec();
            let values = if tyc.len() >= 16 && &tyc[..16] == b"string-attribute" {
                match get_string(g, err, &mut fp, count, &nmc)? {
                    None => return Ok(false),
                    s => Values::S(s),
                }
            } else if tyc.len() >= 15 && &tyc[..15] == b"float-attribute" {
                match get_numbers(g, err, &mut fp, count, &nmc, &FLOATS, strtod_float) {
                    None => return Ok(false),
                    f => Values::F(f),
                }
            } else if tyc.len() >= 17 && &tyc[..17] == b"integer-attribute" {
                match get_numbers(g, err, &mut fp, count, &nmc, &INTS, strtol_int) {
                    None => return Ok(false),
                    i => Values::I(i),
                }
            } else {
                let mut m = b"read_afni_header(): unknown type ".to_vec();
                m.extend_from_slice(tyc);
                fail0(g, err, ERROR_BADPARM, &m);
                return Ok(false);
            };
            let (s, f, ip) = match values {
                Values::S(s) => (s, None, None),
                Values::F(f) => (None, f, None),
                Values::I(i) => (None, None, i),
            };
            let is_int = tyc.len() >= 17 && &tyc[..17] == b"integer-attribute";
            let is_float = tyc.len() >= 15 && &tyc[..15] == b"float-attribute";
            let is_string = tyc.len() >= 16 && &tyc[..16] == b"string-attribute";
            match nmc.as_slice() {
                b"DATASET_RANK" => {
                    let Some(ip) = &ip else {
                        wrong_type(g, err, "DATASET_RANK", tyc, "integer-attribute")?;
                        return Ok(false);
                    };
                    let a = short_of(g, ip.get(0), "DATASET_RANK");
                    let b = short_of(g, ip.get(1), "DATASET_RANK");
                    af.set(RANK, a.to_le_bytes());
                    af.set(RANK + 4, b.to_le_bytes());
                }
                b"DATASET_DIMENSIONS" => {
                    if !is_int {
                        listed_as(g, err, "DATASET_DIMENSIONS", tyc, "integer-attribute");
                        return Ok(false);
                    }
                    if count < 3 {
                        not_enough(g, err, "DATASET_DIMENSIONS", count);
                        return Ok(false);
                    }
                    let ip = ip.as_ref().unwrap();
                    for k in 0..3 {
                        let v = short_of(g, ip.get(k), "DATASET_DIMENSIONS");
                        af.set(DIMS + 4 * k, v.to_le_bytes());
                    }
                    gotten |= DATASET_DIMENSIONS_FLAG;
                }
                b"TYPESTRING" => {
                    let Some(s) = &s else {
                        wrong_type(g, err, "TYPESTRING", tyc, "string-attribute")?;
                        return Ok(false);
                    };
                    let v = cstr(s);
                    too_long(g, err, "TYPESTRING", v.len(), TYPESTRING_FIELD)?;
                    af.typestring = v.to_vec();
                }
                b"SCENE_DATA" => {
                    let Some(ip) = &ip else {
                        wrong_type(g, err, "SCENE_DATA", tyc, "integer-attribute")?;
                        return Ok(false);
                    };
                    for k in 0..3 {
                        let v = short_of(g, ip.get(k), "SCENE_DATA");
                        af.set(SCENE + 4 * k, v.to_le_bytes());
                    }
                }
                b"ORIENT_SPECIFIC" => {
                    if !is_int {
                        listed_as(g, err, "ORIENT_SPECIFIC", tyc, "integer-attribute");
                        return Ok(false);
                    }
                    if count < 3 {
                        not_enough(g, err, "ORIENT_SPECIFIC", count);
                        return Ok(false);
                    }
                    let ip = ip.as_ref().unwrap();
                    let v: Vec<i32> = (0..3)
                        .map(|k| short_of(g, ip.get(k), "ORIENT_SPECIFIC"))
                        .collect();
                    if v.iter().any(|x| !(0..=5).contains(x)) {
                        fail0(
                            g,
                            err,
                            ERROR_BADPARM,
                            format!("read_afni_header(): ORIENT_SPECIFIC variables should be 0 to 5, inclusive (are {}, {}, {} here)", v[0], v[1], v[2]).as_bytes(),
                        );
                        return Ok(false);
                    }
                    for (k, x) in v.iter().enumerate() {
                        af.set(ORIENT + 4 * k, x.to_le_bytes());
                    }
                    gotten |= ORIENT_SPECIFIC_FLAG;
                }
                b"ORIGIN" => {
                    if count < 3 {
                        not_enough(g, err, "ORIGIN", count);
                        return Ok(false);
                    }
                    if !is_float {
                        listed_as(g, err, "ORIGIN", tyc, "float-attribute");
                        return Ok(false);
                    }
                    let f = f.as_ref().unwrap();
                    for k in 0..3 {
                        let v = short_of(g, f.get(k), "ORIGIN");
                        af.set(ORIGIN + 4 * k, v.to_le_bytes());
                    }
                    gotten |= ORIGIN_FLAG;
                }
                b"DELTA" => {
                    if !is_float {
                        listed_as(g, err, "DELTA", tyc, "float-attribute");
                        return Ok(false);
                    }
                    if count < 3 {
                        not_enough(g, err, "DELTA", count);
                        return Ok(false);
                    }
                    let f = f.as_ref().unwrap();
                    for k in 0..3 {
                        let v = short_of(g, f.get(k), "DELTA");
                        af.set(DELTA + 4 * k, v.to_le_bytes());
                    }
                    gotten |= DELTA_FLAG;
                }
                b"IDCODE_STRING" => {
                    let Some(s) = &s else {
                        wrong_type(g, err, "IDCODE_STRING", tyc, "string-attribute")?;
                        return Ok(false);
                    };
                    let v = cstr(s);
                    if count < 0 {
                        // only under `reproduce` (FS-699): malloc(count) fails, strcpy to null
                        return Err(Fatal::Exit(139));
                    }
                    // malloc(count) and strcpy: a string of count bytes (no '~') writes its NUL past
                    if count > 0 && v.len() >= count as usize {
                        let usable = malloc_usable(count.max(0) as u64).unwrap_or(0);
                        if usable <= count as u64 {
                            note(
                                g,
                                &FS_716,
                                "an AFNI IDCODE_STRING without its '~': nothing written past its buffer (FreeSurfer writes a byte past it, over the allocator's records: not deterministic)",
                            );
                        } else {
                            note(
                                g,
                                &FS_703,
                                "an AFNI IDCODE_STRING without its '~': nothing written past its buffer (FreeSurfer writes a byte past it, within its allocation)",
                            );
                        }
                    }
                    af.idcode = Some(v.to_vec());
                }
                b"BYTEORDER_STRING" => {
                    if !is_string {
                        listed_as(g, err, "BYTEORDER_STRING", tyc, "string-attribute");
                        return Ok(false);
                    }
                    let v = cstr(s.as_ref().unwrap());
                    too_long(g, err, "BYTEORDER_STRING", v.len(), BYTEORDER_FIELD)?;
                    af.byteorder = v.to_vec();
                    gotten |= BYTEORDER_STRING_FLAG;
                }
                b"BRICK_STATS" => {
                    af.set(NUMSTATS, count.to_le_bytes());
                    match &f {
                        Some(f) => af.stats = Some(f.vals[..count as usize].to_vec()),
                        None if count > 0 => {
                            wrong_type(g, err, "BRICK_STATS", tyc, "float-attribute")?;
                            return Ok(false);
                        }
                        None => af.stats = Some(Vec::new()),
                    }
                }
                b"BRICK_TYPES" => {
                    af.set(NUMTYPES, count.to_le_bytes());
                    let v = match &ip {
                        Some(ip) => ip.vals[..count as usize].to_vec(),
                        None if count > 0 => {
                            wrong_type(g, err, "BRICK_TYPES", tyc, "integer-attribute")?;
                            return Ok(false);
                        }
                        None => Vec::new(),
                    };
                    let mixed = v.iter().skip(1).any(|&x| x != v[0]);
                    af.types = Some(v);
                    if mixed {
                        log::error_printf(
                            g,
                            err,
                            ERROR_BADPARM,
                            "multiple data types are not supported",
                        );
                        return Ok(false);
                    }
                    gotten |= BRICK_TYPES_FLAG;
                }
                b"BRICK_FLOAT_FACS" => {
                    af.set(NUMFACS, count.to_le_bytes());
                    match &f {
                        Some(f) => af.facs = Some(f.vals[..count as usize].to_vec()),
                        None if count > 0 => {
                            wrong_type(g, err, "BRICK_FLOAT_FACS", tyc, "float-attribute")?;
                            return Ok(false);
                        }
                        None => af.facs = Some(Vec::new()),
                    }
                }
                _ => {}
            }
        }
        if last {
            break;
        }
    }
    fp.close();
    if gotten & AFNI_ALL_REQUIRED != AFNI_ALL_REQUIRED {
        g.errno = 0;
        log::error_printf(g, err, ERROR_BADFILE, "missing fields in afni header file");
        for (flag, what) in [
            (ORIENT_SPECIFIC_FLAG, "ORIENT_SPECIFIC"),
            (BRICK_TYPES_FLAG, "BRICK_TYPES"),
            (DATASET_DIMENSIONS_FLAG, "DATASET_DIMENSIONS"),
            (DELTA_FLAG, "DELTA"),
            (ORIGIN_FLAG, "ORIGIN"),
            (BYTEORDER_STRING_FLAG, "BYTEORDER_STRING"),
        ] {
            if gotten & flag == 0 {
                log::error_printf(g, err, ERROR_BADFILE, &format!("  {what} missing"));
            }
        }
        return Ok(false);
    }
    Ok(true)
}

/// The brick types' names as FreeSurfer prints them (`types[4][8]`: AFNI's codes 2 and 3 are
/// `int` and `float`, not `float` and `complex`), and as AFNI names them (the default, FS-704).
fn type_name(g: &mut Globals, t: i32) -> Vec<u8> {
    const FS: [&str; 4] = ["byte", "short", "float", "complex"];
    const AFNI: [&str; 8] = [
        "byte", "short", "int", "float", "double", "complex", "rgb", "rgba",
    ];
    let fs = usize::try_from(t).ok().and_then(|i| FS.get(i).copied());
    let afni = usize::try_from(t)
        .ok()
        .and_then(|i| AFNI.get(i).copied())
        .map_or_else(|| format!("unknown ({t})"), str::to_owned);
    match fs {
        Some(f) if f != afni => note(
            g,
            &FS_704,
            format!("the AFNI brick type {t} printed as {afni} (FreeSurfer prints {f})"),
        ),
        None => note(
            g,
            &FS_711,
            format!(
                "the AFNI brick type {t} printed as {afni} (FreeSurfer prints its name table's bytes past its end: memory contents)"
            ),
        ),
        _ => {}
    }
    if g.notices.reproduce() {
        return fs.unwrap_or("").as_bytes().to_vec();
    }
    afni.into_bytes()
}

/// `printAFNIHeader(&af)`: the header on standard output.
fn print_afni_header(g: &mut Globals, out: &mut dyn Write, af: &Af) {
    let f6 = |v: f32| cfmt::f(widen(v), 6);
    let mut t: Vec<u8> = Vec::new();
    t.extend_from_slice(b"AFNI Header Information ============================================\n");
    t.extend_from_slice(
        format!(
            "DATASET_RANK      : spatial dims {}, sub-bricks {}\n",
            af.i32(RANK),
            af.i32(RANK + 4)
        )
        .as_bytes(),
    );
    t.extend_from_slice(
        format!(
            "DATASET_DIMENSIONS: ({}, {}, {})\n",
            af.i32(DIMS),
            af.i32(DIMS + 4),
            af.i32(DIMS + 8)
        )
        .as_bytes(),
    );
    let ts = af.typestring.clone();
    t.extend_from_slice(b"TYPESTRING        : ");
    t.extend_from_slice(&ts);
    t.push(b'\n');
    t.extend_from_slice(
        format!(
            "SCENE_DATA        : view type {}, func type {}, verify {}\n",
            af.i32(SCENE),
            af.i32(SCENE + 4),
            af.i32(SCENE + 8)
        )
        .as_bytes(),
    );
    t.extend_from_slice(
        format!(
            "ORIGIN            : ({}, {}, {})\n",
            f6(af.f32(ORIGIN)),
            f6(af.f32(ORIGIN + 4)),
            f6(af.f32(ORIGIN + 8))
        )
        .as_bytes(),
    );
    t.extend_from_slice(
        format!(
            "DELTA             : ({}, {}, {})\n",
            f6(af.f32(DELTA)),
            f6(af.f32(DELTA + 4)),
            f6(af.f32(DELTA + 8))
        )
        .as_bytes(),
    );
    t.extend_from_slice(b"IDCODE_STRING     : ");
    match &af.idcode {
        Some(v) => t.extend_from_slice(v),
        None => t.extend_from_slice(b"(null)"),
    }
    t.push(b'\n');
    let bo = af.byteorder.clone();
    t.extend_from_slice(b"BYTEORDER_STRING  : ");
    t.extend_from_slice(&bo);
    t.push(b'\n');
    let _ = out.write_all(&t);
    t.clear();
    let numstats = af.i32(NUMSTATS);
    let stats = af.stats.clone().unwrap_or_default();
    let mut i = 0;
    while i < numstats {
        let a = short_of(
            g,
            stats.get(i as usize).copied(),
            "printAFNIHeader's BRICK_STATS",
        );
        let b = short_of(
            g,
            stats.get(i as usize + 1).copied(),
            "printAFNIHeader's BRICK_STATS",
        );
        let _ = writeln!(out, "BRICK_STATS       : min {}\t max {}", f6(a), f6(b));
        i += 2;
    }
    let types = af.types.clone().unwrap_or_default();
    for k in 0..af.i32(NUMTYPES).max(0) as usize {
        let v = short_of(g, types.get(k).copied(), "printAFNIHeader's BRICK_TYPES");
        let mut l = b"BRICK_TYPES       : ".to_vec();
        l.extend_from_slice(&type_name(g, v));
        l.push(b'\n');
        let _ = out.write_all(&l);
    }
    let facs = af.facs.clone().unwrap_or_default();
    for k in 0..af.i32(NUMFACS).max(0) as usize {
        let v = short_of(
            g,
            facs.get(k).copied(),
            "printAFNIHeader's BRICK_FLOAT_FACS",
        );
        let _ = writeln!(out, "BRICK_FLOAT_FACS  : {}", f6(v));
    }
    let _ =
        out.write_all(b"====================================================================\n");
}

/// `afniRead(fname, read_volume)`: an AFNI dataset: `fname` must end in `.BRIK` (`afniRead(): bad
/// file name <fname>`); its header is the `.HEAD` beside it (`afniRead(): error opening file
/// <head>`), read and printed; then the BRIK's voxels, one value per voxel of the brick type (0
/// byte, 1 short, 3 float; 2 and 4 and above refused), frame after frame, slice after slice, row
/// after row, scaled to float by `BRICK_FLOAT_FACS` when its first factor is not 0. See
/// `specs/freesurfer-io.md` §118.
pub fn afni_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let rep = g.notices.reproduce();
    if fname.len() >= STRLEN {
        note(
            g,
            &FS_249,
            format!(
                "afniRead: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                fname.len()
            ),
        );
    }
    let bad = || format!("afniRead(): bad file name {fname}");
    let Some(dot) = fname.rfind('.') else {
        fail0(g, err, ERROR_BADPARM, bad().as_bytes());
        return Ok(None);
    };
    if &fname[dot..] != ".BRIK" {
        fail0(g, err, ERROR_BADPARM, bad().as_bytes());
        return Ok(None);
    }
    let header_fname = format!("{}.HEAD", &fname[..dot]);
    let fp = match CFile::open(dir, &header_fname, "r") {
        Ok(f) => f,
        Err(_) => {
            fail0(
                g,
                err,
                ERROR_BADFILE,
                format!("afniRead(): error opening file {header_fname}").as_bytes(),
            );
            return Ok(None);
        }
    };
    let mut af = Af::init();
    if !read_afni_header(g, err, fp, &mut af)? {
        return Ok(None);
    }
    print_afni_header(g, out, &af);
    let numtypes = af.i32(NUMTYPES);
    if numtypes != 1 {
        g.errno = 0;
        let _ = writeln!(
            out,
            "INFO: number of frames dataset_rank[1] = {} : numtypes = {numtypes} ",
            af.i32(RANK + 4)
        );
    }
    let bo = af.byteorder.clone();
    let big_endian = match bo.as_slice() {
        b"MSB_FIRST" => true,
        b"LSB_FIRST" => false,
        _ => {
            let mut m = b"read_afni_header(): unrecognized byte order string ".to_vec();
            m.extend_from_slice(&bo);
            fail0(g, err, ERROR_BADPARM, &m);
            return Ok(None);
        }
    };
    let types = af.types.clone().unwrap_or_default();
    let type0 = short_of(g, types.first().copied(), "afniRead's BRICK_TYPES");
    if type0 == 2 || type0 > 3 {
        fail0(
            g,
            err,
            ERROR_BADPARM,
            format!(
                "afniRead(): unsupported data type {type0}, Must be 0 (byte), 1(short), or 3(float)"
            )
            .as_bytes(),
        );
        return Ok(None);
    }
    let (w, h, d) = (af.i32(DIMS), af.i32(DIMS + 4), af.i32(DIMS + 8));
    let nframes = af.i32(RANK + 4);
    let mut header = mri::mri_alloc_header(g, out, err, w, h, d, MRI_UCHAR, nframes)?;
    header.nframes = nframes;
    let o: Vec<i32> = (0..3).map(|k| af.i32(ORIENT + 4 * k)).collect();
    // the codes were checked when read (0 to 5)
    let dirs: Vec<&[f32; 3]> = o.iter().map(|&k| &ORIENTATIONS[k as usize]).collect();
    {
        let v = &mut header.vg;
        [v.x_r, v.x_a, v.x_s] = *dirs[0];
        [v.y_r, v.y_a, v.y_s] = *dirs[1];
        [v.z_r, v.z_a, v.z_s] = *dirs[2];
    }
    let v = &header.vg;
    let det = v.x_r * (v.y_a * v.z_s - v.z_a * v.y_s) - v.x_a * (v.y_r * v.z_s - v.z_r * v.y_s)
        + v.x_s * (v.y_r * v.z_a - v.z_r * v.y_a);
    if det == 0.0 {
        fail0(
            g,
            err,
            ERROR_BADPARM,
            format!("read_afni_header(): error in orientations {}, {}, {} (direction cosine matrix has determinant zero)", o[0], o[1], o[2]).as_bytes(),
        );
        return Ok(None);
    }
    let delta = [af.f32(DELTA), af.f32(DELTA + 4), af.f32(DELTA + 8)];
    let origin = [af.f32(ORIGIN), af.f32(ORIGIN + 4), af.f32(ORIGIN + 8)];
    header.vg.xsize = delta[0];
    header.vg.ysize = delta[1];
    header.vg.zsize = delta[2];
    // c = x (xsize (width - 1) / 2 + origin) + y (...) + z (...), in double
    let half = |size: f32, n: i32, orig: f32| {
        sse::add64(
            sse::div64(sse::mul64(widen(size), f64::from(n) - 1.0), 2.0),
            widen(orig),
        )
    };
    let hx = half(header.vg.xsize, w, origin[0]);
    let hy = half(header.vg.ysize, h, origin[1]);
    let hz = half(header.vg.zsize, d, origin[2]);
    let centre = |a: f32, b: f32, c: f32| {
        narrow(sse::add64(
            sse::add64(sse::mul64(widen(a), hx), sse::mul64(widen(b), hy)),
            sse::mul64(widen(c), hz),
        ))
    };
    let vg = &header.vg;
    let (c_r, c_a, c_s) = (
        centre(vg.x_r, vg.y_r, vg.z_r),
        centre(vg.x_a, vg.y_a, vg.z_a),
        centre(vg.x_s, vg.y_s, vg.z_s),
    );
    header.vg.c_r = c_r;
    header.vg.c_a = c_a;
    header.vg.c_s = c_s;
    header.vg.ras_good_flag = 1;
    for s in [
        &mut header.vg.xsize,
        &mut header.vg.ysize,
        &mut header.vg.zsize,
    ] {
        if *s < 0.0 {
            *s = -*s;
        }
    }
    header.imnr0 = 1;
    header.imnr1 = header.vg.depth;
    header.ps = header.vg.xsize;
    header.thick = header.vg.zsize;
    let end = |n: i32, size: f32| narrow(sse::mul64(f64::from(n) / 2.0, widen(size)));
    header.xend = end(header.vg.width, header.vg.xsize);
    header.xstart = -header.xend;
    header.yend = end(header.vg.height, header.vg.ysize);
    header.ystart = -header.yend;
    header.zend = end(header.vg.depth, header.vg.zsize);
    header.zstart = -header.zend;
    let xfov = sse::sub(header.xend, header.xstart);
    let yfov = sse::sub(header.yend, header.ystart);
    let zfov = sse::sub(header.zend, header.zstart);
    header.fov = if xfov > yfov {
        if xfov > zfov { xfov } else { zfov }
    } else if yfov > zfov {
        yfov
    } else {
        zfov
    };
    let swap = big_endian;

    // the BRIK
    let open_failed = |g: &mut Globals, err: &mut dyn Write| {
        if rep {
            fail0(
                g,
                err,
                ERROR_BADFILE,
                format!("afniRead(): error opening file {header_fname}").as_bytes(),
            );
        } else {
            fail0(
                g,
                err,
                ERROR_BADFILE,
                format!("afniRead(): error opening file {fname}").as_bytes(),
            );
        }
    };
    if std::fs::metadata(utils::resolve(dir, fname)).is_ok_and(|m| m.is_dir()) {
        note(
            g,
            &FS_709,
            "afniRead: a folder in the BRIK's place: refused (FreeSurfer takes its length from the file system: not deterministic)",
        );
        open_failed(g, err);
        return Ok(None);
    }
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(_) => {
            note(
                g,
                &FS_708,
                if rep {
                    "afniRead: a BRIK that cannot be opened named by its header's name, as in FreeSurfer"
                } else {
                    "afniRead: a BRIK that cannot be opened named by its own name (FreeSurfer names the header)"
                },
            );
            open_failed(g, err);
            return Ok(None);
        }
    };
    let mut errno = g.errno;
    fp.fseek(&mut errno, 0, 2);
    let length = fp.tell();
    fp.fseek(&mut errno, 0, 0);
    g.errno = errno;
    let nvoxels = i64::from(
        header
            .vg
            .width
            .wrapping_mul(header.vg.height)
            .wrapping_mul(header.vg.depth)
            .wrapping_mul(header.nframes),
    );
    if nvoxels == 0 {
        note(
            g,
            &FS_707,
            format!(
                "afniRead: a volume of no voxels ({w} x {h} x {d}, {nframes} frames): FreeSurfer divides by zero (signal 8)"
            ),
        );
        if rep {
            return Err(Fatal::Exit(136));
        }
        fail0(
            g,
            err,
            ERROR_BADFILE,
            format!("afniRead(): the volume has no voxels ({w} x {h} x {d}, {nframes} frames)")
                .as_bytes(),
        );
        return Ok(None);
    }
    let cut32 = |g: &mut Globals, v: i64| -> String {
        if v as i32 as i64 != v {
            note(
                g,
                &FS_708,
                if rep {
                    "afniRead: a BRIK length or voxel count beyond the int range printed as its low 32 bits, as in FreeSurfer"
                } else {
                    "afniRead: a BRIK length or voxel count beyond the int range printed whole (FreeSurfer prints its low 32 bits)"
                },
            );
        }
        if rep {
            (v as i32).to_string()
        } else {
            v.to_string()
        }
    };
    if length % nvoxels != 0 {
        let (a, b) = (cut32(g, length), cut32(g, nvoxels));
        fail0(
            g,
            err,
            ERROR_BADFILE,
            format!(
                "afniRead(): BRIK file length ({a}) is not divisible by the number of voxels ({b})"
            )
            .as_bytes(),
        );
        return Ok(None);
    }
    let bpv = type0.wrapping_add(1);
    if i64::from(bpv) != length / nvoxels {
        note(
            g,
            &FS_713,
            if rep {
                "afniRead: the BRIK's size message prints two numbers for three: the third is memory contents (here 0)"
            } else {
                "afniRead: the BRIK's size message prints the bytes per voxel, the length and the voxels (FreeSurfer prints the quotient and a number from memory contents)"
            },
        );
        let m = if rep {
            format!(
                "afniRead(): type info stored in header does not agree with the file size: {bpv} != {}/0",
                cut32(g, length / nvoxels)
            )
        } else {
            format!(
                "afniRead(): type info stored in header does not agree with the file size: {bpv} != {length}/{nvoxels}"
            )
        };
        fail0(g, err, ERROR_UNSUPPORTED, m.as_bytes());
        return Ok(None);
    }
    // the factors: FreeSurfer's pointer is null without the attribute (FS-705), its places past
    // the count memory contents (FS-701)
    let numfacs = af.i32(NUMFACS);
    let facs = af.facs.clone();
    let fac_at = |g: &mut Globals, k: usize, what: &str| -> Option<f32> {
        facs.as_ref().map(|v| short_of(g, v.get(k).copied(), what))
    };
    let mut scaling: f32 = 1.0;
    let fs_type;
    if numfacs != 0 && fac_at(g, 0, "afniRead's BRICK_FLOAT_FACS").is_some_and(|f| f != 0.0) {
        fs_type = MRI_FLOAT;
        scaling = fac_at(g, 0, "afniRead's BRICK_FLOAT_FACS").unwrap_or(1.0);
    } else {
        fs_type = match bpv {
            1 => MRI_UCHAR,
            2 => MRI_SHORT,
            4 => MRI_FLOAT,
            _ => {
                fail0(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    format!("afniRead(): don't know what to do with {bpv} bytes per voxel")
                        .as_bytes(),
                );
                return Ok(None);
            }
        };
    }
    // the default (FS-706): a float volume whenever a frame is scaled, each frame by its own
    // factor; FreeSurfer's type follows the first frame's factor alone
    let frame_facs: Vec<f32> = match &facs {
        Some(v) => (0..nframes.max(0) as usize)
            .map(|k| v.get(k).copied().unwrap_or(0.0))
            .collect(),
        None => Vec::new(),
    };
    let fix_type = if frame_facs.iter().any(|&x| x != 0.0) {
        MRI_FLOAT
    } else {
        fs_type
    };
    let raw_into_float = read_volume
        && fs_type == MRI_FLOAT
        && bpv < 4
        && frame_facs.iter().any(|&x| x == 0.0 || x == 1.0);
    if fix_type != fs_type || raw_into_float {
        note(
            g,
            &FS_706,
            if rep {
                "AFNI frames scaled differently: the volume's type taken from the first frame, every frame written by its own scaling, as in FreeSurfer"
            } else {
                "AFNI frames scaled differently: a float volume, each frame scaled by its own factor (FreeSurfer takes the type from the first frame and writes some frames in another frame's voxel type)"
            },
        );
    }
    header.mtype = if rep { fs_type } else { fix_type };
    if !read_volume {
        let Some(mut m) =
            larmorx_freesurfer_mri::copy::mri_copy(g, out, err, &header, None)?.into_new()
        else {
            return Ok(None);
        };
        m.vg.fname.strcpy_bytes(fname.as_bytes());
        fp.close();
        return Ok(Some(m));
    }
    let mut m = mri::mri_alloc_sequence(
        g,
        out,
        err,
        header.vg.width,
        header.vg.height,
        header.vg.depth,
        header.mtype,
        header.nframes,
    )?;
    mri::mri_copy_header(g, out, err, &header, Some(&mut m))?;
    let (height, depth) = (m.vg.height, m.vg.depth);
    if facs.is_none() && m.nframes > 0 && depth > 0 && height > 0 {
        // the first row's factor is read through a null pointer
        note(
            g,
            &FS_705,
            "afniRead: an AFNI header without BRICK_FLOAT_FACS: scale 1 (FreeSurfer reads a null pointer: a crash)",
        );
        if rep {
            return Err(Fatal::Exit(139));
        }
    }
    let stats = af.stats.clone();
    let (mut fmin, mut fmax) = (0f32, 0f32);
    let mut past_end = false;
    let mut row = Row {
        bpv,
        swap,
        rep,
        min: 0.0,
        max: 0.0,
        past_end: false,
    };
    for frame in 0..m.nframes {
        let mut initialized = false;
        if depth > 0 && height > 0 {
            // `if (af.brick_float_facs[frame])`: 0 (and -0) give 1, anything else (NaN too) itself
            let fac = fac_at(g, frame as usize, "afniRead's BRICK_FLOAT_FACS").unwrap_or(0.0);
            scaling = if fac != 0.0 { fac } else { 1.0 };
        }
        for k in 0..depth {
            for j in 0..height {
                if !row.read(g, err, &mut fp, &mut m, fname, frame, k, j, scaling)? {
                    return Ok(None);
                }
                if !initialized {
                    fmin = row.min;
                    fmax = row.max;
                    initialized = true;
                } else {
                    if row.min < fmin {
                        fmin = row.min;
                    }
                    if row.max > fmax {
                        fmax = row.max;
                    }
                }
            }
        }
        let Some(v) = &stats else {
            if frame == 0 {
                note(
                    g,
                    &FS_705,
                    "afniRead: an AFNI header without BRICK_STATS: its lines not printed (FreeSurfer reads a null pointer: a crash)",
                );
            }
            if rep {
                return Err(Fatal::Exit(139));
            }
            continue;
        };
        let a = short_of(
            g,
            v.get(2 * frame as usize).copied(),
            "afniRead's BRICK_STATS",
        );
        let b = short_of(
            g,
            v.get(2 * frame as usize + 1).copied(),
            "afniRead's BRICK_STATS",
        );
        let _ = writeln!(
            out,
            "BRICK_STATS min = {} <--> actual min = {}",
            cfmt::f(widen(a), 6),
            cfmt::f(widen(sse::mul(fmin, scaling)), 6)
        );
        let _ = writeln!(
            out,
            "BRICK_STATS max = {} <--> actual max = {}",
            cfmt::f(widen(b), 6),
            cfmt::f(widen(sse::mul(fmax, scaling)), 6)
        );
    }
    past_end |= row.past_end;
    if past_end {
        note(
            g,
            &FS_712,
            "AFNI frames scaled differently: FreeSurfer writes past the volume's end (FreeSurfer: not deterministic)",
        );
    }
    m.vg.fname.strcpy_bytes(fname.as_bytes());
    fp.close();
    Ok(Some(m))
}

/// The BRIK's rows as `afniRead` reads them: the voxel size, the byte order, the mode, the last
/// row's minimum and maximum (unscaled), and whether `reproduce` wrote past the volume.
struct Row {
    bpv: i32,
    swap: bool,
    rep: bool,
    min: f32,
    max: f32,
    past_end: bool,
}

impl Row {
    /// One row: read (`afniRead(): error reading from file <fname>` when the file ends first),
    /// byte-swapped, stored, and its minimum and maximum. FreeSurfer stores by the voxel size and
    /// the scaling whatever the volume's type (`MRIseq_vox` a byte, `MRISseq_vox` a short when the
    /// scaling is 1, else `MRIFseq_vox` a float, on the row's pointer): `reproduce` does; the
    /// default stores the volume's type. `false` after the message.
    #[allow(clippy::too_many_arguments)]
    fn read(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        fp: &mut CFile,
        m: &mut Mri,
        fname: &str,
        frame: i32,
        k: i32,
        j: i32,
        scaling: f32,
    ) -> Result<bool, Fatal> {
        let width = m.vg.width.max(0) as usize;
        let size = self.bpv.max(0) as usize;
        let mut buf = vec![0u8; size * width];
        let n = fp.fread(&mut buf, size, width);
        if n != width {
            fail0(
                g,
                err,
                ERROR_BADFILE,
                format!("afniRead(): error reading from file {fname}").as_bytes(),
            );
            return Ok(false);
        }
        if self.swap {
            match size {
                2 => {
                    for p in buf.as_chunks_mut::<2>().0 {
                        p.swap(0, 1);
                    }
                }
                4 => {
                    for p in buf.as_chunks_mut::<4>().0 {
                        p.reverse();
                    }
                }
                _ => {}
            }
        }
        let vals: Vec<f32> = match size {
            1 => buf.iter().map(|&b| f32::from(b)).collect(),
            2 => buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| f32::from(i16::from_le_bytes(*p)))
                .collect(),
            _ => buf
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| f32::from_le_bytes(*p))
                .collect(),
        };
        // the row's first byte: rows of vox_per_row voxels of the volume's type
        let sl = i64::from(k) + i64::from(frame) * i64::from(m.vg.depth);
        let row = (sl * i64::from(m.vg.height) + i64::from(j)) as u64 * m.vox_per_row;
        let base = row * m.bytes_per_vox as u64;
        let mtype = m.mtype;
        if let Some(c) = m.chunk.as_mut() {
            let mut past = false;
            let mut put = |at: u64, b: &[u8]| {
                let at = at as usize;
                if at + b.len() <= c.len() {
                    c[at..at + b.len()].copy_from_slice(b);
                } else {
                    past = true;
                }
            };
            for (i, &v) in vals.iter().enumerate() {
                let raw = &buf[i * size..(i + 1) * size];
                let i = i as u64;
                // the store: FreeSurfer's (by size and scaling) or the volume's type
                let kind = if self.rep {
                    match (size, scaling == 1.0) {
                        (1, true) => MRI_UCHAR,
                        (2, true) => MRI_SHORT,
                        _ => MRI_FLOAT,
                    }
                } else {
                    mtype
                };
                match kind {
                    MRI_UCHAR => put(base + i, raw),
                    MRI_SHORT => put(base + 2 * i, raw),
                    _ => put(base + 4 * i, &sse::mul(v, scaling).to_le_bytes()),
                }
            }
            self.past_end |= past;
        }
        // findMinMaxByte, findMinMaxShort, findMinMaxFloat: from the first value, compared as read
        if let Some(&first) = vals.first() {
            let (mut a, mut b) = (first, first);
            for &v in &vals {
                if v < a {
                    a = v;
                }
                if v > b {
                    b = v;
                }
            }
            self.min = a;
            self.max = b;
        }
        Ok(true)
    }
}

/// `afniWrite(mri, fname)`: FreeSurfer 8.2.0 refuses at once: `AFNI BRIK write unsupported`
/// (`ERROR_UNSUPPORTED`), nothing written.
pub fn afni_write(g: &mut Globals, err: &mut dyn Write) -> i32 {
    g.errno = 0;
    log::error_printf(g, err, ERROR_UNSUPPORTED, "AFNI BRIK write unsupported");
    ERROR_UNSUPPORTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn strtol_keeps_the_low_bits() {
        assert_eq!(strtol_int(b"12 3"), (12, 2, false));
        assert_eq!(strtol_int(b"-7x"), (-7, 2, false));
        assert_eq!(strtol_int(b"4294967297"), (1, 10, false));
        assert_eq!(strtol_int(b"99999999999999999999"), (-1, 20, true));
        assert_eq!(strtol_int(b"x"), (0, 0, false));
        assert_eq!(strtol_int(b"1.5").1, 1);
    }

    #[test]
    fn strtod_ranges() {
        assert_eq!(strtod_float(b"1.5 2"), (1.5, 3, false));
        assert!(strtod_float(b"1e999").2);
        assert!(strtod_float(b"1e-999").2);
        assert!(!strtod_float(b"1e300").2);
        assert!(strtod_float(b"1e300").0.is_infinite());
        assert_eq!(strtod_float(b"abc").1, 0);
    }

    #[test]
    fn usable_sizes() {
        assert_eq!(malloc_usable(0), Some(24));
        assert_eq!(malloc_usable(24), Some(24));
        assert_eq!(malloc_usable(25), Some(40));
        assert_eq!(malloc_usable(40), Some(40));
    }

    /// An AFNI header of the given attributes' texts (AFNI's spacing).
    fn head(extra: &str, facs: &str, stats: bool) -> String {
        let mut h = String::from(
            "\ntype = integer-attribute\nname = DATASET_RANK\ncount = 8\n 3 2 0 0 0\n 0 0 0\n\
             \ntype = integer-attribute\nname = DATASET_DIMENSIONS\ncount = 5\n 2 1 1 0 0\n\
             \ntype = string-attribute\nname = TYPESTRING\ncount = 15\n'3DIM_HEAD_ANAT~\n\
             \ntype = integer-attribute\nname = ORIENT_SPECIFIC\ncount = 3\n 0 3 4\n\
             \ntype = float-attribute\nname = ORIGIN\ncount = 3\n -1.5 2 0.5\n\
             \ntype = float-attribute\nname = DELTA\ncount = 3\n 1 -2 3\n\
             \ntype = string-attribute\nname = BYTEORDER_STRING\ncount = 10\n'LSB_FIRST~\n\
             \ntype = integer-attribute\nname = BRICK_TYPES\ncount = 2\n 0 0\n",
        );
        if stats {
            h.push_str("\ntype = float-attribute\nname = BRICK_STATS\ncount = 4\n 0 9 1 8\n");
        }
        if !facs.is_empty() {
            h.push_str(&format!(
                "\ntype = float-attribute\nname = BRICK_FLOAT_FACS\ncount = 2\n {facs}\n"
            ));
        }
        h.push_str(extra);
        h
    }

    fn read(mode: UpstreamDefects, h: &str, brik: &[u8]) -> (Option<Mri>, Globals, String) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a+orig.HEAD"), h).unwrap();
        std::fs::write(dir.path().join("a+orig.BRIK"), brik).unwrap();
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = afni_read(
            &mut g,
            &mut o,
            &mut e,
            Some(dir.path()),
            "a+orig.BRIK",
            true,
        );
        o.extend_from_slice(&e);
        let text = String::from_utf8_lossy(&o).into_owned();
        match r {
            Ok(m) => (m, g, text),
            Err(Fatal::Exit(c)) => (None, g, format!("{text}EXIT {c}")),
            Err(_) => (None, g, text),
        }
    }

    fn floats(m: &Mri) -> Vec<f32> {
        m.chunk
            .as_ref()
            .unwrap()
            .chunks(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    /// Two byte frames, unscaled: bytes, the geometry from ORIENT_SPECIFIC, ORIGIN and DELTA.
    #[test]
    fn a_byte_dataset_reads() {
        let (m, _, text) = read(UpstreamDefects::Fix, &head("", "0 0", true), &[1, 2, 3, 4]);
        let m = m.unwrap();
        assert_eq!(m.mtype, MRI_UCHAR);
        assert_eq!(m.chunk.as_deref(), Some(&[1u8, 2, 3, 4][..]));
        assert_eq!((m.vg.width, m.nframes), (2, 2));
        assert_eq!((m.vg.x_r, m.vg.y_a, m.vg.z_s), (-1.0, -1.0, 1.0));
        // sizes made positive after the centre: c_r = -1 * (1 * 1 / 2 - 1.5)
        assert_eq!((m.vg.xsize, m.vg.ysize, m.vg.zsize), (1.0, 2.0, 3.0));
        assert_eq!(m.vg.c_r, 1.0);
        assert!(text.contains("BRICK_STATS max = 9.000000 <--> actual max = 2.000000"));
        assert!(text.contains("BRICK_TYPES       : byte"));
    }

    /// FS-706: a scaled second frame after an unscaled first: a float volume by default, each frame
    /// by its own factor; FreeSurfer keeps bytes and writes the second frame's floats into them.
    #[test]
    fn frames_scaled_differently() {
        let (m, g, _) = read(
            UpstreamDefects::Fix,
            &head("", "0 0.5", true),
            &[1, 2, 3, 4],
        );
        let m = m.unwrap();
        assert_eq!(m.mtype, MRI_FLOAT);
        assert_eq!(floats(&m), vec![1.0, 2.0, 1.5, 2.0]);
        assert!(g.notices.contains(&FS_706));
        let (m, g, _) = read(
            UpstreamDefects::Reproduce,
            &head("", "0 0.5", true),
            &[1, 2, 3, 4],
        );
        // the bytes volume of 4 bytes: frame 1's two floats start at byte 2 and pass its end
        assert!(m.is_some());
        assert!(g.notices.contains(&FS_712));
    }

    /// FS-705: without BRICK_FLOAT_FACS the default scales by 1; FreeSurfer crashes.
    #[test]
    fn no_factors() {
        let (m, g, _) = read(UpstreamDefects::Fix, &head("", "", true), &[1, 2, 3, 4]);
        assert_eq!(m.unwrap().chunk.as_deref(), Some(&[1u8, 2, 3, 4][..]));
        assert!(g.notices.contains(&FS_705));
        let (m, _, text) = read(
            UpstreamDefects::Reproduce,
            &head("", "", true),
            &[1, 2, 3, 4],
        );
        assert!(m.is_none() && text.ends_with("EXIT 139"));
        // without BRICK_STATS: no stats lines by default; FreeSurfer crashes after frame 0
        let (m, _, text) = read(UpstreamDefects::Fix, &head("", "0 0", false), &[1, 2, 3, 4]);
        assert!(m.is_some() && !text.contains("<-->"));
        let (_, _, text) = read(
            UpstreamDefects::Reproduce,
            &head("", "0 0", false),
            &[1, 2, 3, 4],
        );
        assert!(text.ends_with("EXIT 139"));
    }

    /// FS-694: a last line of numbers without a newline is read by default; FreeSurfer's reader
    /// ends there ('hit EOF').
    #[test]
    fn a_last_line_without_a_newline() {
        let mut h = head("", "", true);
        h.push_str("\ntype = float-attribute\nname = BRICK_FLOAT_FACS\ncount = 2\n 0 0");
        let (m, g, _) = read(UpstreamDefects::Fix, &h, &[1, 2, 3, 4]);
        assert!(m.is_some());
        assert!(g.notices.contains(&FS_694));
        let (m, _, text) = read(UpstreamDefects::Reproduce, &h, &[1, 2, 3, 4]);
        assert!(m.is_none());
        assert!(
            text.contains("get_afni_float(): hit EOF while reading 2 floats for BRICK_FLOAT_FACS")
        );
    }

    /// FS-707: a frame count of 0: an error by default; FreeSurfer divides by zero.
    #[test]
    fn no_voxels() {
        let h = head("", "0 0", true).replacen(" 3 2 0 0 0", " 3 0 0 0 0", 1);
        let (m, _, text) = read(UpstreamDefects::Fix, &h, &[]);
        assert!(m.is_none() && text.contains("the volume has no voxels"));
        let (_, _, text) = read(UpstreamDefects::Reproduce, &h, &[]);
        assert!(text.ends_with("EXIT 136"));
    }

    /// FS-702: a TYPESTRING of 16 characters: kept by default; FreeSurfer's copy aborts.
    #[test]
    fn a_long_typestring() {
        let h = head("", "0 0", true).replacen(
            "count = 15\n'3DIM_HEAD_ANAT~",
            "count = 17\n'3DIM_HEAD_ANAT_XY~",
            1,
        );
        let (m, _, text) = read(UpstreamDefects::Fix, &h, &[1, 2, 3, 4]);
        assert!(m.is_some() && text.contains("TYPESTRING        : 3DIM_HEAD_ANAT_XY\n"));
        let (_, _, text) = read(UpstreamDefects::Reproduce, &h, &[1, 2, 3, 4]);
        assert!(text.ends_with("*** buffer overflow detected ***: terminated\nEXIT 134"));
    }

    /// FS-697: a word among the numbers: an error by default.
    #[test]
    fn a_word_among_numbers() {
        let h = head("", "0 0", true).replacen(" 0 3 4\n", " 0 3 x4\n", 1);
        let (m, _, text) = read(UpstreamDefects::Fix, &h, &[1, 2, 3, 4]);
        assert!(m.is_none());
        assert!(
            text.contains(
                "get_afni_int(): 'x4' is not a number (reading 3 ints for ORIENT_SPECIFIC)"
            )
        );
    }

    /// The BRIK writer refuses.
    #[test]
    fn writing_is_refused() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        assert_eq!(afni_write(&mut g, &mut e), ERROR_UNSUPPORTED);
        assert!(String::from_utf8_lossy(&e).contains("AFNI BRIK write unsupported"));
    }

    #[test]
    fn type_names() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(type_name(&mut g, 3), b"float");
        assert!(g.notices.contains(&FS_704));
        let mut r = Globals::new(UpstreamDefects::Reproduce);
        assert_eq!(type_name(&mut r, 3), b"complex");
        assert_eq!(type_name(&mut r, 0), b"byte");
        assert_eq!(type_name(&mut g, 9), b"unknown (9)");
    }
}

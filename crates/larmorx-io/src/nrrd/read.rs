// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! `nrrdLoad` and what it calls (NrrdIO r3991's `read.c`, `formatNRRD.c`, `parseNrrd.c`,
//! `simple.c`'s checks, `encodingRaw.c`, `encodingAscii.c`, `encodingHex.c`, `endianNrrd.c`).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::cscan;
use larmorx_core::defects::{self as d, Notices};

use super::{
    AIR_ENDIAN, AIR_STRLEN_HUGE, BIFF_STRLEN, Biff, CENTER_ENUM, ENCODING_ENUM, ENDIAN_ENUM,
    FIELD_ENUM, FIELD_STR, KIND_3GRADIENT, KIND_ENUM, NRRD, NRRD_DIM_MAX, NRRD_SPACE_DIM_MAX, Nrrd,
    SPACE_ENUM_FIXED, SPACE_ENUM_NRRDIO, air_nan, cstr, exists, field, is_inf, is_print, kind_size,
    size_check, space_dimension, strcspn, strntok, strspn, strstr, to_lower, type_enum, types,
    unescape,
};
use crate::dicom::cond::{errno_of, os_text};

/// `_nrrdFieldSep`, `_nrrdLineSep`.
const FIELD_SEP: &[u8] = b" \t";
const EOF: i32 = -1;

/// How `nrrdLoad` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// `nrrdLoad` returned `code` (1, or 2 for a failed `fopen`); the messages are in the
    /// [`Biff`] passed in (key [`NRRD`]).
    Failed { code: i32 },
    /// The original prints `message` on standard error and ends the program with `status`
    /// (NrrdIO's `PANIC`s; reproduced only with `reproduce`).
    Exit { status: i32, message: Vec<u8> },
}

// ------------------------------------------------------------------------------------------
// C's stdio, as NrrdIO uses it
// ------------------------------------------------------------------------------------------

enum Src {
    File(BufReader<File>),
    Stdin(std::io::Stdin),
    /// A directory: `fopen` succeeds on Linux, every read fails (`EISDIR`): EOF.
    Empty,
}

/// A `FILE *`: `getc`, one `ungetc`, `fread`, `fseek(..., SEEK_END)`.
pub(crate) struct CFile {
    src: Src,
    push: Option<u8>,
    stdin: bool,
}

impl CFile {
    /// `airFopen(name, stdin, "rb")`: `-` is the standard input. Err(errno).
    fn open(name: &[u8]) -> Result<CFile, i32> {
        if name == b"-" {
            return Ok(CFile {
                src: Src::Stdin(std::io::stdin()),
                push: None,
                stdin: true,
            });
        }
        if name.is_empty() {
            return Err(2);
        }
        let path = to_path(name);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_dir() => {
                return Ok(CFile {
                    src: Src::Empty,
                    push: None,
                    stdin: false,
                });
            }
            Err(e) => return Err(errno_of(&e)),
            _ => {}
        }
        match File::open(&path) {
            Ok(f) => Ok(CFile {
                src: Src::File(BufReader::new(f)),
                push: None,
                stdin: false,
            }),
            Err(e) => Err(errno_of(&e)),
        }
    }

    fn read_raw(&mut self, buf: &mut [u8]) -> usize {
        let mut n = 0;
        while n < buf.len() {
            let r = match &mut self.src {
                Src::File(f) => f.read(&mut buf[n..]),
                Src::Stdin(s) => s.read(&mut buf[n..]),
                Src::Empty => Ok(0),
            };
            match r {
                Ok(0) => break,
                Ok(k) => n += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        n
    }

    /// `getc`.
    pub fn getc(&mut self) -> i32 {
        if let Some(c) = self.push.take() {
            return c as i32;
        }
        let mut b = [0u8; 1];
        if self.read_raw(&mut b) == 1 {
            b[0] as i32
        } else {
            EOF
        }
    }

    /// `ungetc` (one character).
    pub fn ungetc(&mut self, c: i32) {
        if c != EOF {
            self.push = Some(c as u8);
        }
    }

    /// `fread(buf, 1, len)`: the bytes read.
    fn fread(&mut self, buf: &mut [u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let mut start = 0;
        if let Some(c) = self.push.take() {
            buf[0] = c;
            start = 1;
        }
        start + self.read_raw(&mut buf[start..])
    }

    /// `fseek(file, off, SEEK_END) == 0`.
    fn seek_end(&mut self, off: i64) -> bool {
        self.push = None;
        match &mut self.src {
            Src::File(f) => f.seek(SeekFrom::End(off)).is_ok(),
            _ => false,
        }
    }
}

/// `airOneLine(file, line, size)`: the characters stored (before the NUL it writes) and its
/// return value. A last line that ends without a newline sets `partial`; `keep_last` (FS-763
/// corrected) keeps it, NrrdIO drops it.
fn air_one_line(
    file: &mut CFile,
    size: usize,
    keep_last: bool,
    partial: &mut bool,
) -> (Vec<u8>, usize) {
    let mut line = Vec::new();
    let mut c = 0i32;
    let mut i = 0usize;
    while i + 2 <= size {
        c = file.getc();
        if c == EOF || c == b'\n' as i32 {
            break;
        }
        line.push(c as u8);
        i += 1;
    }
    if c == EOF {
        if i > 0 {
            *partial = true;
            if keep_last {
                return (line, i + 1);
            }
        }
        return (Vec::new(), 0);
    }
    if c == b'\n' as i32 {
        if i >= 1 && line[i - 1] == b'\r' {
            i -= 1;
        }
        line.truncate(i);
        return (line, i + 1);
    }
    // the buffer is full (i == size - 1)
    let c = file.getc();
    if c == b'\n' as i32 {
        if line[i - 1] == b'\r' {
            i -= 1;
        }
        line.truncate(i);
        return (line, i + 1);
    }
    file.ungetc(c);
    line.truncate(size - 1);
    (line, size + 1)
}

// ------------------------------------------------------------------------------------------
// the I/O state
// ------------------------------------------------------------------------------------------

/// An encoding (`NrrdEncoding`): its index in `nrrdEncodingArray`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    Unknown,
    Raw,
    Ascii,
    Hex,
    Gzip,
    Bzip2,
}

impl Encoding {
    fn from_index(i: i32) -> Encoding {
        match i {
            1 => Encoding::Raw,
            2 => Encoding::Ascii,
            3 => Encoding::Hex,
            4 => Encoding::Gzip,
            5 => Encoding::Bzip2,
            _ => Encoding::Unknown,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Encoding::Unknown => "unknown",
            Encoding::Raw => "raw",
            Encoding::Ascii => "ASCII",
            Encoding::Hex => "hex",
            Encoding::Gzip => "gzip",
            Encoding::Bzip2 => "bzip2",
        }
    }
    fn endian_matters(self) -> bool {
        matches!(
            self,
            Encoding::Raw | Encoding::Hex | Encoding::Gzip | Encoding::Bzip2
        )
    }
    fn is_compression(self) -> bool {
        matches!(self, Encoding::Gzip | Encoding::Bzip2)
    }
}

/// `NrrdIoState`, the parts reading uses.
struct Nio {
    path: Vec<u8>,
    line: Vec<u8>,
    line_len: usize,
    pos: usize,
    endian: i32,
    line_skip: u32,
    byte_skip: i32,
    seen: [bool; field::MAX_FIELD + 1],
    data_fn: Vec<Vec<u8>>,
    data_fn_format: Option<Vec<u8>>,
    data_fn_min: i32,
    data_fn_max: i32,
    data_fn_step: i32,
    data_file_dim: u32,
    data_fn_index: i32,
    encoding: Encoding,
}

impl Nio {
    fn new() -> Nio {
        Nio {
            path: Vec::new(),
            line: Vec::new(),
            line_len: 0,
            pos: 0,
            endian: 0,
            line_skip: 0,
            byte_skip: 0,
            seen: [false; field::MAX_FIELD + 1],
            data_fn: Vec::new(),
            data_fn_format: None,
            data_fn_min: 0,
            data_fn_max: 0,
            data_fn_step: 0,
            data_file_dim: 0,
            data_fn_index: -1,
            encoding: Encoding::Unknown,
        }
    }

    /// `_nrrdDataFNNumber`.
    fn data_fn_number(&self) -> u32 {
        if self.data_fn_format.is_some() {
            let mut ret = 0u32;
            let mut ii = self.data_fn_min as i64;
            let max = self.data_fn_max as i64;
            let step = self.data_fn_step as i64;
            while (step > 0 && ii <= max) || (step < 0 && ii >= max) {
                ret += 1;
                ii += step;
            }
            ret
        } else if !self.data_fn.is_empty() {
            self.data_fn.len() as u32
        } else {
            1
        }
    }

    /// The info after the field name (`nio->line + nio->pos`).
    fn info(&self) -> Vec<u8> {
        self.line[self.pos.min(self.line.len())..].to_vec()
    }
}

/// Bytes of a message.
fn b(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// `a` followed by `bytes` and `c`.
fn cat(a: &str, bytes: &[u8], c: &str) -> Vec<u8> {
    let mut v = b(a);
    v.extend_from_slice(bytes);
    v.extend_from_slice(c.as_bytes());
    v
}

type R<T> = Result<T, ()>;

/// One `nrrdLoad`: the state it changes.
struct Reader<'a> {
    biff: &'a mut Biff,
    notices: &'a mut Notices,
    streams: &'a mut dyn Streams,
    nio: Nio,
    nrrd: Nrrd,
    exit: Option<(i32, Vec<u8>)>,
}

impl Reader<'_> {
    /// `sprintf(err, ...)` into a buffer of `BIFF_STRLEN` bytes, then `biffAdd(NRRD, err)`: a
    /// longer message overflows the buffer (FS-762: the original aborts).
    fn add(&mut self, msg: Vec<u8>) {
        if msg.len() >= BIFF_STRLEN {
            self.notices.add(
                &d::FS_762,
                format!(
                    "a message of {} bytes did not fit NrrdIO's {BIFF_STRLEN}-byte buffer (the \
                     original aborts with a buffer overflow); larmorx keeps it whole",
                    msg.len()
                ),
            );
        }
        self.biff.add(NRRD, &msg);
    }

    /// `biffAdd` of a message whose buffer was sized for it.
    fn add_sized(&mut self, msg: Vec<u8>) {
        self.biff.add(NRRD, &msg);
    }

    fn fail<T>(&mut self, msg: Vec<u8>) -> R<T> {
        self.add(msg);
        Err(())
    }

    // ---------------------------------------------------------------------------- lines

    /// `_nrrdOneLine`: sets `nio.line` and returns `*lenP`.
    fn one_line(&mut self, file: &mut CFile) -> u32 {
        let keep = self.notices.fix();
        let mut partial = false;
        if self.nio.line_len == 0 {
            self.nio.line_len = 3;
        }
        let (piece, mut len) = air_one_line(file, self.nio.line_len, keep, &mut partial);
        let ret = if len <= self.nio.line_len {
            self.nio.line = cstr(&piece).to_vec();
            len as u32
        } else {
            let mut pieces = vec![piece];
            while len == self.nio.line_len + 1 {
                self.nio.line_len *= 2;
                let (p, l) = air_one_line(file, self.nio.line_len, keep, &mut partial);
                pieces.push(p);
                len = l;
            }
            self.nio.line_len *= 3;
            let mut line = Vec::new();
            for p in &pieces {
                line.extend_from_slice(cstr(p));
            }
            let n = line.len() as u32 + 1;
            self.nio.line = line;
            n
        };
        if partial {
            self.notices.add(
                &d::FS_763,
                if keep {
                    "a line that ends at the end of the file without a newline was read (NrrdIO \
                     drops it)"
                } else {
                    "a line that ends at the end of the file without a newline was dropped"
                },
            );
        }
        ret
    }

    // ---------------------------------------------------------------------------- numbers

    /// `airSingleSscanf(str, "%lf" | "%f", ...)`: "nan", "-inf" and "inf" anywhere in the
    /// lower-cased string give NrrdIO's values; otherwise C's `sscanf`.
    fn single_sscanf_f64(s: &[u8]) -> Option<f64> {
        let t = to_lower(s);
        if strstr(&t, b"nan").is_some() {
            Some(air_nan())
        } else if strstr(&t, b"-inf").is_some() {
            Some(f64::NEG_INFINITY)
        } else if strstr(&t, b"inf").is_some() {
            Some(f64::INFINITY)
        } else {
            cscan::sscanf_f64(s)
        }
    }

    fn single_sscanf_f32(s: &[u8]) -> Option<f32> {
        let t = to_lower(s);
        if strstr(&t, b"nan").is_some() {
            Some(larmorx_core::nrrdio::air_nan())
        } else if strstr(&t, b"-inf").is_some() {
            Some(f32::NEG_INFINITY)
        } else if strstr(&t, b"inf").is_some() {
            Some(f32::INFINITY)
        } else {
            cscan::sscanf_f32(s)
        }
    }

    /// `airParseStrD(val, s, ct, n)`: the values parsed (stops at the first failure).
    fn parse_str_d(s: &[u8], ct: &[u8], n: usize) -> Vec<f64> {
        let mut t = super::Strtok::new(s, ct);
        let mut out = Vec::new();
        for _ in 0..n {
            let Some(tok) = t.next_token() else { break };
            match Self::single_sscanf_f64(tok) {
                Some(v) => out.push(v),
                None => break,
            }
        }
        out
    }

    /// `airParseStrZ` (`%lu`).
    fn parse_str_z(s: &[u8], ct: &[u8], n: usize) -> Vec<usize> {
        let mut t = super::Strtok::new(s, ct);
        let mut out = Vec::new();
        for _ in 0..n {
            let Some(tok) = t.next_token() else { break };
            match cscan::sscanf_u64(tok) {
                Some(v) => out.push(v as usize),
                None => break,
            }
        }
        out
    }

    // ---------------------------------------------------------------------------- checks

    /// `_nrrdFieldCheckSpaceInfo`.
    fn check_space_info(&mut self) -> R<()> {
        let me = "_nrrdFieldCheckSpaceInfo";
        let n = &self.nrrd;
        if !(n.space == 0 || !SPACE_ENUM_FIXED.val_check(n.space)) {
            let m = format!("{me}: space {} invalid", n.space);
            return self.fail(b(&m));
        }
        if n.space_dim as usize > NRRD_SPACE_DIM_MAX {
            let shown = if self.notices.fix() {
                n.space_dim
            } else {
                n.dim
            };
            if self.notices.fix() {
                self.notices.add(
                    &d::FS_764,
                    format!(
                        "the message shows the space dimension {} (NrrdIO shows the array's \
                         dimension, {})",
                        n.space_dim, n.dim
                    ),
                );
            } else {
                self.notices.add(
                    &d::FS_764,
                    format!(
                        "the message shows the array's dimension {} for the space dimension {}",
                        n.dim, n.space_dim
                    ),
                );
            }
            let m = format!(
                "{me}: space dimension {shown} is outside valid range \
                 [0,NRRD_SPACE_DIM_MAX] = [0,{NRRD_SPACE_DIM_MAX}]"
            );
            return self.fail(b(&m));
        }
        let sd = n.space_dim as usize;
        if sd > 0 {
            if n.space != 0 && space_dimension(n.space) != n.space_dim {
                let m = format!(
                    "{me}: space {} has dimension {} but spaceDim is {}",
                    SPACE_ENUM_FIXED.str_of(n.space),
                    space_dimension(n.space),
                    n.space_dim
                );
                return self.fail(b(&m));
            }
            let e0 = exists(n.space_origin[0]);
            for ii in 0..sd {
                if e0 ^ exists(n.space_origin[ii]) {
                    let m = format!(
                        "{me}: existance of space origin coefficients must be consistent \
                         (val[0] not like val[{ii}])"
                    );
                    return self.fail(b(&m));
                }
            }
            let e0 = exists(n.measurement_frame[0][0]);
            for dd in 0..sd {
                for ii in 0..sd {
                    if e0 ^ exists(n.measurement_frame[dd][ii]) {
                        let m = format!(
                            "{me}: existance of measurement frame coefficients must be \
                             consistent: [col][row] [{dd}][{ii}] not like [0][0])"
                        );
                        return self.fail(b(&m));
                    }
                }
            }
            for dd in 0..(n.dim as usize).min(NRRD_DIM_MAX) {
                let a = &n.axis[dd];
                let e = exists(a.space_direction[0]);
                for ii in 1..sd {
                    if e ^ exists(a.space_direction[ii]) {
                        let m = format!(
                            "{me}: existance of space direction {dd} coefficients must be \
                             consistent (val[0] not like val[{ii}])"
                        );
                        return self.fail(b(&m));
                    }
                }
                if e && (exists(a.min)
                    || exists(a.max)
                    || exists(a.spacing)
                    || a.units.as_ref().is_some_and(|u| !u.is_empty()))
                {
                    let m = format!(
                        "{me}: axis[{dd}] has a direction vector, and so can't have min, max, \
                         spacing, or units set"
                    );
                    return self.fail(b(&m));
                }
            }
        } else {
            if n.space != 0 {
                let m = format!(
                    "{me}: space {} can't be set with spaceDim {}",
                    SPACE_ENUM_FIXED.str_of(n.space),
                    n.space_dim
                );
                return self.fail(b(&m));
            }
            if n.space_units
                .iter()
                .any(|u| u.as_ref().is_some_and(|u| !u.is_empty()))
            {
                return self.fail(b(&format!("{me}: spaceDim is 0, but space units is set")));
            }
            if n.space_origin.iter().any(|&v| exists(v)) {
                return self.fail(b(&format!("{me}: spaceDim is 0, but space origin is set")));
            }
            if n.axis
                .iter()
                .any(|a| a.space_direction.iter().any(|&v| exists(v)))
            {
                return self.fail(b(&format!(
                    "{me}: spaceDim is 0, but space directions are set"
                )));
            }
        }
        Ok(())
    }

    /// `_nrrdFieldCheck[fi]`.
    fn field_check(&mut self, fi: usize) -> R<()> {
        let dim = (self.nrrd.dim as usize).min(NRRD_DIM_MAX);
        match fi {
            field::TYPE => {
                if type_enum().val_check(self.nrrd.type_) {
                    let m = format!(
                        "_nrrdFieldCheck_type: type ({}) is not valid",
                        self.nrrd.type_
                    );
                    return self.fail(b(&m));
                }
            }
            field::BLOCK_SIZE => {
                let me = "_nrrdFieldCheck_block_size";
                if self.nrrd.type_ == types::BLOCK && self.nrrd.block_size == 0 {
                    let m = format!(
                        "{me}: type is block but nrrd->blockSize ({}) invalid",
                        self.nrrd.block_size
                    );
                    return self.fail(b(&m));
                }
                if self.nrrd.type_ != types::BLOCK && self.nrrd.block_size > 0 {
                    let m = format!(
                        "{me}: type is {} (not block) but blockSize is {}",
                        type_enum().str_of(self.nrrd.type_),
                        self.nrrd.block_size
                    );
                    return self.fail(b(&m));
                }
            }
            field::DIMENSION => {
                if !(1..=NRRD_DIM_MAX as u32).contains(&self.nrrd.dim) {
                    let m = format!(
                        "_nrrdFieldCheck_dimension: dimension {} is outside valid range [1,{}]",
                        self.nrrd.dim, NRRD_DIM_MAX
                    );
                    return self.fail(b(&m));
                }
            }
            field::SPACE | field::SPACE_DIMENSION => {
                if self.check_space_info().is_err() {
                    let me = if fi == field::SPACE {
                        "_nrrdFieldCheck_space"
                    } else {
                        "_nrrdFieldCheck_space_dimension"
                    };
                    return self.fail(b(&format!("{me}: trouble")));
                }
            }
            field::SIZES => {
                let sizes: Vec<usize> = self.nrrd.axis[..dim].iter().map(|a| a.size).collect();
                if let Err(m) = size_check(&sizes) {
                    self.add(b(&m));
                    return self.fail(b("_nrrdFieldCheck_sizes: trouble with array sizes"));
                }
            }
            field::SPACINGS => {
                for ai in 0..dim {
                    let v = self.nrrd.axis[ai].spacing;
                    if !(is_inf(v) == 0 && (v.is_nan() || v != 0.0)) {
                        let m = format!(
                            "_nrrdFieldCheck_spacings: axis {ai} spacing ({}) invalid",
                            cfmt::g(v, 6)
                        );
                        return self.fail(b(&m));
                    }
                }
                if self.check_space_info().is_err() {
                    return self.fail(b("_nrrdFieldCheck_spacings: trouble"));
                }
            }
            field::THICKNESSES => {
                for ai in 0..dim {
                    let v = self.nrrd.axis[ai].thickness;
                    if !(is_inf(v) == 0 && (v.is_nan() || 0.0 <= v)) {
                        let m = format!(
                            "_nrrdFieldCheck_thicknesses: axis {ai} thickness ({}) invalid",
                            cfmt::g(v, 6)
                        );
                        return self.fail(b(&m));
                    }
                }
            }
            field::AXIS_MINS | field::AXIS_MAXS => {
                let (me, what) = if fi == field::AXIS_MINS {
                    ("_nrrdFieldCheck_axis_mins", "min")
                } else {
                    ("_nrrdFieldCheck_axis_maxs", "max")
                };
                for ai in 0..dim {
                    let a = &self.nrrd.axis[ai];
                    let v = if fi == field::AXIS_MINS { a.min } else { a.max };
                    let r = is_inf(v);
                    if r != 0 {
                        let m = format!(
                            "{me}: axis {ai} {what} {}inf invalid",
                            if r == 1 { "+" } else { "-" }
                        );
                        return self.fail(b(&m));
                    }
                }
                if self.check_space_info().is_err() {
                    return self.fail(b(&format!("{me}: trouble")));
                }
            }
            field::SPACE_DIRECTIONS
            | field::UNITS
            | field::SPACE_UNITS
            | field::SPACE_ORIGIN
            | field::MEASUREMENT_FRAME => {
                if self.check_space_info().is_err() {
                    let me = match fi {
                        field::SPACE_DIRECTIONS => "_nrrdFieldCheck_space_directions",
                        field::UNITS => "_nrrdFieldCheck_units",
                        field::SPACE_UNITS => "_nrrdFieldCheck_space_units",
                        field::SPACE_ORIGIN => "_nrrdFieldCheck_space_origin",
                        _ => "_nrrdFieldCheck_measurement_frame",
                    };
                    return self.fail(b(&format!("{me}: space info problem")));
                }
            }
            field::CENTERS => {
                for ai in 0..dim {
                    let c = self.nrrd.axis[ai].center;
                    if !(c == 0 || !CENTER_ENUM.val_check(c)) {
                        let m = format!("_nrrdFieldCheck_centers: axis {ai} center {c} invalid");
                        return self.fail(b(&m));
                    }
                }
            }
            field::KINDS => {
                for ai in 0..dim {
                    let k = self.nrrd.axis[ai].kind;
                    if !(k == 0 || !KIND_ENUM.val_check(k)) {
                        let m = format!("_nrrdFieldCheck_kinds: axis {ai} kind {k} invalid");
                        return self.fail(b(&m));
                    }
                    let want = match kind_size(k) {
                        Ok(w) => w,
                        Err(()) => {
                            if self.notices.reproduce() {
                                self.notices.add(
                                    &d::FS_520,
                                    "an axis of kind 3-gradient ends the program (NrrdIO's \
                                     PANIC, exit status 1)",
                                );
                                self.exit = Some((
                                    1,
                                    b(&format!(
                                        "nrrdKindSize: PANIC: nrrdKind {k} not implemented!\n"
                                    )),
                                ));
                                return Err(());
                            }
                            self.notices.add(
                                &d::FS_520,
                                "an axis of kind 3-gradient has 3 samples (NrrdIO ends the \
                                 program: PANIC, exit status 1)",
                            );
                            debug_assert_eq!(k, KIND_3GRADIENT);
                            3
                        }
                    };
                    let size = self.nrrd.axis[ai].size;
                    if want != 0 && want as usize != size {
                        let m = format!(
                            "_nrrdFieldCheck_kinds: axis {ai} kind {} requires size {want}, but \
                             have {size}",
                            KIND_ENUM.str_of(k)
                        );
                        return self.fail(b(&m));
                    }
                }
            }
            field::OLD_MIN | field::OLD_MAX => {
                let (v, me, what) = if fi == field::OLD_MIN {
                    (self.nrrd.old_min, "_nrrdFieldCheck_old_min", "min")
                } else {
                    (self.nrrd.old_max, "_nrrdFieldCheck_old_max", "max")
                };
                let r = is_inf(v);
                if r != 0 {
                    let m = format!(
                        "{me}: old {what} {}inf invalid",
                        if r == 1 { "+" } else { "-" }
                    );
                    return self.fail(b(&m));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `_nrrdCheck(nrrd, AIR_FALSE, AIR_TRUE)`.
    fn check(&mut self) -> R<()> {
        for fi in 1..=field::MAX_FIELD {
            if self.field_check(fi).is_err() {
                if self.exit.is_some() {
                    return Err(());
                }
                let m = format!("_nrrdCheck: trouble with {} field", FIELD_STR[fi]);
                return self.fail(b(&m));
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------- fields

    /// `_nrrdReadNrrdParseField`: the field of `nio.line`, or 0 with a message.
    fn parse_field(&mut self) -> usize {
        let me = "_nrrdReadNrrdParseField";
        let next = self.nio.info();
        if next.first() == Some(&b'#') {
            return field::COMMENT;
        }
        let mut buff = next.clone();
        let colon = strstr(&buff, b": ");
        let no_field = colon.is_none();
        let mut fld = field::UNKNOWN;
        let mut bad = false;
        if let Some(c) = colon {
            buff.truncate(c);
            fld = FIELD_ENUM.val_of(&buff) as usize;
            bad = fld == field::UNKNOWN;
        }
        if no_field || bad {
            if strstr(&buff, b":=").is_none() {
                let m = if no_field {
                    b(&format!("{me}: didn't see \": \" or \":=\" in line"))
                } else {
                    cat(
                        &format!("{me}: failed to parse \""),
                        &buff,
                        "\" as field identifier",
                    )
                };
                self.add(m);
                return field::UNKNOWN;
            }
            return field::KEYVALUE;
        }
        let mut p = self.nio.pos + buff.len() + 2;
        p += strspn(&self.nio.line[p.min(self.nio.line.len())..], FIELD_SEP);
        self.nio.pos = p;
        fld
    }

    fn check_have_dim(&mut self, me: &str) -> R<()> {
        if self.nrrd.dim == 0 {
            return self.fail(b(&format!("{me}: don't yet have a valid dimension")));
        }
        Ok(())
    }

    fn check_have_space_dim(&mut self, me: &str) -> R<()> {
        if self.nrrd.space_dim == 0 {
            return self.fail(b(&format!("{me}: don't yet have a valid space dimension")));
        }
        Ok(())
    }

    fn check_got_all(&mut self, me: &str, ret: usize) -> R<()> {
        if ret != self.nrrd.dim as usize {
            let m = format!(
                "{me}: parsed {ret} values, but dimension is {}",
                self.nrrd.dim
            );
            return self.fail(b(&m));
        }
        Ok(())
    }

    fn trouble_after(&mut self, me: &str, fi: usize) -> R<()> {
        if self.field_check(fi).is_err() {
            return self.fail(b(&format!("{me}: trouble")));
        }
        Ok(())
    }

    /// `_PARSE_ONE_VAL` failure.
    fn one_val_fail<T>(&mut self, me: &str, ty: &str, info: &[u8]) -> R<T> {
        self.fail(cat(
            &format!("{me}: couldn't parse {ty} from \""),
            cstr(info),
            "\"",
        ))
    }

    /// The per-axis lists of doubles (spacings, thicknesses, axis mins, axis maxs).
    fn parse_axis_doubles(&mut self, fi: usize, me: &str, plural: &str) -> R<()> {
        let info = self.nio.info();
        self.check_have_dim(me)?;
        let dim = self.nrrd.dim as usize;
        let val = Self::parse_str_d(&info, FIELD_SEP, dim);
        self.check_got_all(me, val.len())?;
        for (ai, &v) in val.iter().enumerate() {
            let a = &mut self.nrrd.axis[ai];
            match fi {
                field::SPACINGS => a.spacing = v,
                field::THICKNESSES => a.thickness = v,
                field::AXIS_MINS => a.min = v,
                _ => a.max = v,
            }
        }
        if Self::parse_str_d(&info, FIELD_SEP, dim + 1).len() == dim + 1 {
            let m = format!("{me}: seem to have more than expected {dim} {plural}");
            return self.fail(b(&m));
        }
        self.trouble_after(me, fi)
    }

    /// `_nrrdSpaceVectorParse`: the values and the bytes of `hh` used.
    fn space_vector(
        &mut self,
        hh: &[u8],
        space_dim: usize,
    ) -> R<([f64; NRRD_SPACE_DIM_MAX], usize)> {
        let me = "_nrrdSpaceVectorParse";
        let mut val = [air_nan(); NRRD_SPACE_DIM_MAX];
        let mut length = strspn(hh, FIELD_SEP);
        let h = &hh[length..];
        if h.is_empty() {
            return self.fail(b(&format!("{me}: hit end of string before seeing (")));
        }
        if h.starts_with(b"none") {
            if h.len() == 4 || FIELD_SEP.contains(&h[4]) {
                length += 4;
            } else {
                return self.fail(cat(&format!("{me}: couldn't parse non-vector \""), h, "\""));
            }
        } else {
            if h[0] != b'(' {
                return self.fail(cat(
                    &format!("{me}: first vector in \""),
                    h,
                    "\" didn't start with '('",
                ));
            }
            let close = h[1..].iter().position(|&c| c == b')');
            let Some(close) = close else {
                return self.fail(b(&format!(
                    "{me}: didn't see ')' at end of first vector in \"\""
                )));
            };
            let buff = &h[..close + 2];
            length += buff.len();
            let ret = strntok(&buff[1..], b",)") as usize;
            if ret > space_dim {
                let m = format!(
                    "{me}: space dimension is {space_dim}, but seem to have {ret} coefficients"
                );
                return self.fail(b(&m));
            }
            let v = Self::parse_str_d(&buff[1..], b",", space_dim);
            if v.len() != space_dim {
                let m = format!(
                    "{me}: parsed {} values, but space dimension is {space_dim}",
                    v.len()
                );
                return self.fail(b(&m));
            }
            val[..space_dim].copy_from_slice(&v);
        }
        for dd in 1..space_dim {
            if exists(val[0]) ^ exists(val[dd]) {
                let m = format!(
                    "{me}: existance of all space vector coefficients must be consistent \
                     (val[0] not like val[{dd}])"
                );
                return self.fail(b(&m));
            }
        }
        for (dd, &v) in val.iter().enumerate().take(space_dim) {
            if is_inf(v) != 0 {
                let m = format!("{me}: vector coefficient {dd} can't be infinite");
                return self.fail(b(&m));
            }
        }
        Ok((val, length))
    }

    /// `_nrrdGetQuotedString`: the string and the bytes of `h` used.
    fn quoted(&mut self, h0: &[u8], use_biff: bool) -> Option<(Vec<u8>, usize)> {
        let me = "_nrrdGetQuotedString";
        let mut base = strspn(h0, FIELD_SEP);
        if base >= h0.len() {
            if use_biff {
                self.add(b(&format!(
                    "{me}: hit end of string before seeing opening \""
                )));
            }
            return None;
        }
        if h0[base] != b'"' {
            if use_biff {
                self.add(b(&format!("{me}: didn't start with \"")));
            }
            return None;
        }
        base += 1;
        let at = |i: usize| -> u8 { h0.get(i).copied().unwrap_or(0) };
        let mut buff = Vec::new();
        let mut pos = 0usize;
        while at(base + pos) != 0 {
            if at(base + pos) == b'"' {
                break;
            }
            if at(base + pos) == b'\\' && at(base + pos + 1) == b'"' {
                base += 1;
            }
            buff.push(at(base + pos));
            pos += 1;
        }
        if at(base + pos) != b'"' {
            if use_biff {
                self.add(b(&format!("{me}: didn't see ending \" soon enough")));
            }
            return None;
        }
        Some((buff, base + pos + 1))
    }

    /// `_nrrdContainsPercentDAndMore`.
    fn contains_percent_d(s: &[u8]) -> bool {
        let mut tmp = 0usize;
        let hh;
        loop {
            let Some(p) = s[tmp..].iter().position(|&c| c == b'%') else {
                return false;
            };
            let p = tmp + p;
            if p + 1 >= s.len() {
                return false;
            }
            if s[p + 1] == b'%' {
                tmp = p + 2;
                if tmp >= s.len() {
                    return false;
                }
            } else {
                hh = p;
                break;
            }
        }
        let mut h = hh + 1;
        h += s[h..].iter().take_while(|c| c.is_ascii_digit()).count();
        h < s.len() && s[h] == b'd'
    }

    /// `nrrdFieldInfoParse[fi]`.
    fn parse_info(&mut self, fi: usize, file: &mut CFile) -> R<()> {
        let info = self.nio.info();
        match fi {
            field::COMMENT => {
                let s = &info[strspn(&info, b" #")..];
                if s.is_empty()
                    || s == b"Complete NRRD file format specification at:"
                    || s == b"http://teem.sourceforge.net/nrrd/format.html"
                {
                    return Ok(());
                }
                let s = self.one_linify(s)?;
                self.nrrd.cmt.push(s);
            }
            field::CONTENT => {
                if !info.is_empty() {
                    self.nrrd.content = Some(info);
                }
            }
            field::NUMBER | field::MIN | field::MAX => {}
            field::TYPE => {
                let me = "_nrrdReadNrrdParse_type";
                self.nrrd.type_ = type_enum().val_of(&info);
                if self.nrrd.type_ == 0 {
                    return self.fail(cat(&format!("{me}: couldn't parse type \""), &info, "\""));
                }
                self.trouble_after(me, field::TYPE)?;
            }
            field::BLOCK_SIZE => {
                let me = "_nrrdReadNrrdParse_block_size";
                match cscan::sscanf_u64(&info) {
                    Some(v) => self.nrrd.block_size = v as usize,
                    None => return self.one_val_fail(me, "size_t", &info),
                }
            }
            field::DIMENSION => {
                let me = "_nrrdReadNrrdParse_dimension";
                match cscan::sscanf_u32(&info) {
                    Some(v) => self.nrrd.dim = v,
                    None => return self.one_val_fail(me, "unsigned int", &info),
                }
                self.trouble_after(me, field::DIMENSION)?;
            }
            field::SIZES => {
                let me = "_nrrdReadNrrdParse_sizes";
                self.check_have_dim(me)?;
                let dim = self.nrrd.dim as usize;
                let val = Self::parse_str_z(&info, FIELD_SEP, dim);
                self.check_got_all(me, val.len())?;
                for (ai, &v) in val.iter().enumerate() {
                    self.nrrd.axis[ai].size = v;
                }
                let more = Self::parse_str_z(&info, FIELD_SEP, dim + 1).len();
                if more == dim + 1 {
                    if dim == NRRD_DIM_MAX {
                        self.notices.add(
                            &d::FS_765,
                            "a 17th size was parsed for a 16-dimensional array (NrrdIO writes \
                             it past its array of 16)",
                        );
                    }
                    let m = format!("{me}: seem to have more than expected {dim} sizes");
                    return self.fail(b(&m));
                }
                self.trouble_after(me, field::SIZES)?;
            }
            field::SPACINGS => {
                self.parse_axis_doubles(fi, "_nrrdReadNrrdParse_spacings", "spacings")?
            }
            field::THICKNESSES => {
                self.parse_axis_doubles(fi, "_nrrdReadNrrdParse_thicknesses", "thicknesses")?
            }
            field::AXIS_MINS => {
                self.parse_axis_doubles(fi, "_nrrdReadNrrdParse_axis_mins", "axis mins")?
            }
            field::AXIS_MAXS => {
                self.parse_axis_doubles(fi, "_nrrdReadNrrdParse_axis_maxs", "axis maxs")?
            }
            field::SPACE_DIRECTIONS => {
                let me = "_nrrdReadNrrdParse_space_directions";
                self.check_have_dim(me)?;
                self.check_have_space_dim(me)?;
                let dim = self.nrrd.dim as usize;
                let sd = self.nrrd.space_dim as usize;
                let mut at = 0usize;
                for dd in 0..dim {
                    match self.space_vector(&info[at..], sd) {
                        Ok((v, used)) => {
                            self.nrrd.axis[dd].space_direction = v;
                            at += used;
                        }
                        Err(()) => {
                            let m =
                                format!("{me}: trouble getting space vector {} of {dim}", dd + 1);
                            return self.fail(b(&m));
                        }
                    }
                }
                let rest = &info[at..];
                if strspn(rest, FIELD_SEP) != rest.len() {
                    let m = format!("{me}: seem to have more than expected {dim} directions");
                    return self.fail(b(&m));
                }
                self.trouble_after(me, field::SPACE_DIRECTIONS)?;
            }
            field::CENTERS | field::KINDS => {
                let (me, what) = if fi == field::CENTERS {
                    ("_nrrdReadNrrdParse_centers", "center")
                } else {
                    ("_nrrdReadNrrdParse_kinds", "kind")
                };
                self.check_have_dim(me)?;
                let dim = self.nrrd.dim as usize;
                let mut t = super::Strtok::new(&info, FIELD_SEP);
                for ai in 0..dim {
                    let Some(tok) = t.next_token() else {
                        let m = format!(
                            "{me}: couldn't extract string for {what} {} of {dim}",
                            ai + 1
                        );
                        return self.fail(b(&m));
                    };
                    if tok == b"???" {
                        if fi == field::CENTERS {
                            self.nrrd.axis[ai].center = 0;
                        } else {
                            self.nrrd.axis[ai].kind = 0;
                        }
                        continue;
                    }
                    if tok == b"none" {
                        if fi == field::KINDS {
                            if self.notices.reproduce() {
                                if self.nrrd.axis[ai].center != 0 {
                                    self.notices.add(
                                        &d::FS_519,
                                        format!("kind \"none\" of axis {ai} reset its centering"),
                                    );
                                }
                                self.nrrd.axis[ai].center = 0;
                            } else {
                                if self.nrrd.axis[ai].center != 0 {
                                    self.notices.add(
                                        &d::FS_519,
                                        format!(
                                            "kind \"none\" of axis {ai} left its centering \
                                             (NrrdIO resets it)"
                                        ),
                                    );
                                }
                                self.nrrd.axis[ai].kind = 0;
                            }
                        } else {
                            self.nrrd.axis[ai].center = 0;
                        }
                        continue;
                    }
                    let tok = tok.to_vec();
                    let v = if fi == field::CENTERS {
                        CENTER_ENUM.val_of(&tok)
                    } else {
                        KIND_ENUM.val_of(&tok)
                    };
                    if fi == field::CENTERS {
                        self.nrrd.axis[ai].center = v;
                    } else {
                        self.nrrd.axis[ai].kind = v;
                    }
                    if v == 0 {
                        let m = if fi == field::CENTERS {
                            cat(
                                &format!("{me}: couldn't parse center \""),
                                &tok,
                                &format!("\" for axis {ai}"),
                            )
                        } else {
                            cat(
                                &format!("{me}: couldn't parse \""),
                                &tok,
                                &format!("\" kind {} of {dim}", ai + 1),
                            )
                        };
                        return self.fail(m);
                    }
                }
                if t.next_token().is_some() {
                    let m = format!("{me}: seem to have more than expected {dim} {}s", what);
                    return self.fail(b(&m));
                }
                if fi == field::CENTERS {
                    self.trouble_after(me, field::CENTERS)?;
                }
            }
            field::LABELS | field::UNITS => {
                let (me, what) = if fi == field::LABELS {
                    ("_nrrdReadNrrdParse_labels", "label")
                } else {
                    ("_nrrdReadNrrdParse_units", "unit")
                };
                self.check_have_dim(me)?;
                let dim = self.nrrd.dim as usize;
                let mut at = 0usize;
                for ai in 0..dim {
                    match self.quoted(&info[at..], true) {
                        Some((s, used)) => {
                            if fi == field::LABELS {
                                self.nrrd.axis[ai].label = Some(s);
                            } else {
                                self.nrrd.axis[ai].units = Some(s);
                            }
                            at += used;
                        }
                        None => {
                            let m = format!("{me}: couldn't get get {what} {} of {dim}\n", ai + 1);
                            return self.fail(b(&m));
                        }
                    }
                }
                let rest = &info[at..];
                if strspn(rest, FIELD_SEP) != rest.len() {
                    let m = format!("{me}: seem to have more than expected {dim} {what}s");
                    return self.fail(b(&m));
                }
                self.trouble_after(me, fi)?;
            }
            field::OLD_MIN | field::OLD_MAX => {
                let me = if fi == field::OLD_MIN {
                    "_nrrdReadNrrdParse_old_min"
                } else {
                    "_nrrdReadNrrdParse_old_max"
                };
                let Some(v) = cscan::sscanf_f64(&info) else {
                    return self.one_val_fail(me, "double", &info);
                };
                if fi == field::OLD_MIN {
                    self.nrrd.old_min = v;
                } else {
                    self.nrrd.old_max = v;
                }
                self.trouble_after(me, fi)?;
            }
            field::ENDIAN => {
                let e = ENDIAN_ENUM.val_of(&info);
                self.nio.endian = e;
                if e == 0 {
                    return self.fail(cat(
                        "_nrrdReadNrrdParse_endian: couldn't parse endian \"",
                        &info,
                        "\"",
                    ));
                }
            }
            field::ENCODING => {
                let e = ENCODING_ENUM.val_of(&info);
                if e == 0 {
                    return self.fail(cat(
                        "_nrrdReadNrrdParse_encoding: couldn't parse encoding \"",
                        &info,
                        "\"",
                    ));
                }
                self.nio.encoding = Encoding::from_index(e);
            }
            field::LINE_SKIP => match cscan::sscanf_u32(&info) {
                Some(v) => self.nio.line_skip = v,
                None => {
                    return self.one_val_fail(
                        "_nrrdReadNrrdParse_line_skip",
                        "unsigned int",
                        &info,
                    );
                }
            },
            field::BYTE_SKIP => {
                let me = "_nrrdReadNrrdParse_byte_skip";
                match cscan::sscanf_i32(&info) {
                    Some(v) => self.nio.byte_skip = v,
                    None => return self.one_val_fail(me, "int", &info),
                }
                if self.nio.byte_skip < -1 {
                    let m = format!("{me}: byteSkip value {} invalid", self.nio.byte_skip);
                    return self.fail(b(&m));
                }
            }
            field::KEYVALUE => {
                let line = self.nio.line.clone();
                let Some(k) = strstr(&line, b":=") else {
                    return self.fail(cat(
                        "_nrrdReadNrrdParse_keyvalue: didn't see \":=\" key/value delimiter in \"",
                        &line,
                        "\"",
                    ));
                };
                let mut key = line[..k].to_vec();
                let mut value = line[k + 2..].to_vec();
                unescape(&mut key);
                unescape(&mut value);
                let key = cstr(&key).to_vec();
                let value = cstr(&value).to_vec();
                self.nrrd.key_value_add(&key, &value);
            }
            field::SAMPLE_UNITS => {
                if !info.is_empty() {
                    self.nrrd.sample_units = Some(info);
                }
                self.trouble_after("_nrrdReadNrrdParse_sample_units", fi)?;
            }
            field::SPACE => {
                let me = "_nrrdReadNrrdParse_space";
                if self.nio.seen[field::SPACE_DIMENSION] {
                    let m = format!(
                        "{me}: can't specify space after specifying space dimension ({})",
                        self.nrrd.space_dim
                    );
                    return self.fail(b(&m));
                }
                let nrrdio = SPACE_ENUM_NRRDIO.val_of(&info);
                let fixed = SPACE_ENUM_FIXED.val_of(&info);
                if nrrdio != fixed {
                    let name = String::from_utf8_lossy(&info).into_owned();
                    let what = |v: i32| -> String {
                        if v == 0 {
                            "no space (an error)".to_owned()
                        } else {
                            format!("the space {}", SPACE_ENUM_FIXED.str_of(v))
                        }
                    };
                    self.notices.add(
                        &d::FS_518,
                        if self.notices.fix() {
                            format!(
                                "space \"{name}\" read as {} (NrrdIO reads {})",
                                what(fixed),
                                what(nrrdio)
                            )
                        } else {
                            format!("space \"{name}\" read as {}", what(nrrdio))
                        },
                    );
                }
                let space = if self.notices.fix() { fixed } else { nrrdio };
                if space == 0 {
                    return self.fail(cat(&format!("{me}: couldn't parse space \""), &info, "\""));
                }
                self.nrrd.space = space;
                self.nrrd.space_dim = space_dimension(space);
                self.trouble_after(me, field::SPACE)?;
            }
            field::SPACE_DIMENSION => {
                let me = "_nrrdReadNrrdParse_space_dimension";
                if self.nio.seen[field::SPACE] {
                    let m = format!(
                        "{me}: can't specify space dimension after specifying space ({})",
                        SPACE_ENUM_FIXED.str_of(self.nrrd.space)
                    );
                    return self.fail(b(&m));
                }
                match cscan::sscanf_u32(&info) {
                    Some(v) => self.nrrd.space_dim = v,
                    None => return self.one_val_fail(me, "unsigned int", &info),
                }
                self.trouble_after(me, field::SPACE_DIMENSION)?;
            }
            field::SPACE_UNITS => {
                let me = "_nrrdReadNrrdParse_space_units";
                self.check_have_space_dim(me)?;
                let sd = self.nrrd.space_dim as usize;
                let mut at = 0usize;
                for ai in 0..sd {
                    match self.quoted(&info[at..], true) {
                        Some((s, used)) => {
                            if ai < NRRD_SPACE_DIM_MAX {
                                self.nrrd.space_units[ai] = Some(s);
                            }
                            at += used;
                        }
                        None => {
                            let m = format!("{me}: couldn't get get space unit {} of {sd}", ai + 1);
                            return self.fail(b(&m));
                        }
                    }
                }
                if self.quoted(&info[at..], false).is_some() {
                    let m = format!("{me}: seemed to have more than expected {sd} space units");
                    return self.fail(b(&m));
                }
                self.trouble_after(me, field::SPACE_UNITS)?;
            }
            field::SPACE_ORIGIN => {
                let me = "_nrrdReadNrrdParse_space_origin";
                self.check_have_space_dim(me)?;
                let sd = self.nrrd.space_dim as usize;
                match self.space_vector(&info, sd) {
                    Ok((v, _)) => self.nrrd.space_origin = v,
                    Err(()) => {
                        return self.fail(cat(
                            &format!("{me}: couldn't parse origin \""),
                            &info,
                            "\"",
                        ));
                    }
                }
                self.trouble_after(me, field::SPACE_ORIGIN)?;
            }
            field::MEASUREMENT_FRAME => {
                let me = "_nrrdReadNrrdParse_measurement_frame";
                self.check_have_space_dim(me)?;
                let sd = self.nrrd.space_dim as usize;
                let mut at = 0usize;
                for dd in 0..sd {
                    match self.space_vector(&info[at..], sd) {
                        Ok((v, used)) => {
                            for ii in 0..NRRD_SPACE_DIM_MAX {
                                self.nrrd.measurement_frame[dd][ii] =
                                    if ii < sd { v[ii] } else { air_nan() };
                            }
                            at += used;
                        }
                        Err(()) => {
                            let m =
                                format!("{me}: trouble getting space vector {} of {sd}", dd + 1);
                            return self.fail(b(&m));
                        }
                    }
                }
                let rest = &info[at..];
                if strspn(rest, FIELD_SEP) != rest.len() {
                    let m = format!("{me}: seem to have more than expected {sd} directions");
                    return self.fail(b(&m));
                }
                for dd in sd..NRRD_SPACE_DIM_MAX {
                    self.nrrd.measurement_frame[dd] = [air_nan(); NRRD_SPACE_DIM_MAX];
                }
                self.trouble_after(me, field::MEASUREMENT_FRAME)?;
            }
            field::DATA_FILE => self.parse_data_file(&info, file)?,
            _ => {}
        }
        Ok(())
    }

    /// `nrrdCommentAdd`'s `airOneLinify` (FS-761: an unprintable character makes NrrdIO loop
    /// forever).
    fn one_linify(&mut self, s: &[u8]) -> R<Vec<u8>> {
        if s.iter()
            .any(|&c| !larmorx_core::cscan::is_c_space(c) && !is_print(c))
        {
            if self.notices.reproduce() {
                self.notices.add(
                    &d::FS_761,
                    "a comment holds a character that is neither printable nor white space: \
                     NrrdIO never returns",
                );
                return self.fail(b(
                    "nrrdCommentAdd: a comment with an unprintable character (NrrdIO never \
                     returns)",
                ));
            }
            self.notices.add(
                &d::FS_761,
                "a comment's unprintable characters were removed (NrrdIO never returns)",
            );
        }
        let mut out: Vec<u8> = Vec::with_capacity(s.len());
        for &c in s {
            if larmorx_core::cscan::is_c_space(c) {
                out.push(b' ');
            } else if is_print(c) {
                out.push(c);
            }
        }
        let mut v: Vec<u8> = Vec::with_capacity(out.len());
        for &c in &out {
            if c == b' ' && v.last() == Some(&b' ') {
                continue;
            }
            v.push(c);
        }
        while v.last() == Some(&b' ') {
            v.pop();
        }
        Ok(v)
    }

    /// `_nrrdReadNrrdParse_data_file`.
    fn parse_data_file(&mut self, info: &[u8], file: &mut CFile) -> R<()> {
        let me = "_nrrdReadNrrdParse_data_file";
        if Self::contains_percent_d(info) {
            let fmt_end = strcspn(info, FIELD_SEP);
            let format = info[..fmt_end].to_vec();
            let mut nums = &info[fmt_end..];
            nums = &nums[strspn(nums, FIELD_SEP)..];
            let (vals, n) = sscanf_ints(nums, 4);
            if n < 3 {
                return self.fail(b(&format!(
                    "{me}: couldn't parse three ints (min, max, step) after data filename template"
                )));
            }
            self.nio.data_fn_min = vals[0] as i32;
            self.nio.data_fn_max = vals[1] as i32;
            self.nio.data_fn_step = vals[2] as i32;
            if n == 4 {
                self.nio.data_file_dim = vals[3] as u32;
                if !(self.nio.data_file_dim >= 1 && self.nio.data_file_dim <= self.nrrd.dim) {
                    let m = format!(
                        "{me}: datafile dimension {} outside valid range [1,{}]",
                        self.nio.data_file_dim as i32, self.nrrd.dim
                    );
                    return self.fail(b(&m));
                }
            } else {
                self.nio.data_file_dim = self.nrrd.dim.wrapping_sub(1);
            }
            if self.nio.data_fn_step == 0 {
                return self.fail(b(&format!("{me}: file number step must be non-zero")));
            }
            // C computes this product in int (wrapping as GCC's code does)
            let raw = (self.nio.data_fn_max.wrapping_sub(self.nio.data_fn_min))
                .wrapping_mul(self.nio.data_fn_step);
            if raw < 0 {
                let m = format!(
                    "{me}: file number max {} not approached from min {} by step {}",
                    self.nio.data_fn_max, self.nio.data_fn_min, self.nio.data_fn_step
                );
                return self.fail(b(&m));
            }
            self.nio.data_fn_format = Some(format);
        } else if info.starts_with(b"LIST") {
            if self.header_check(true).is_err() {
                return self.fail(b(&format!(
                    "{me}: NRRD header is incomplete. \"LIST\" data file specification must be \
                     contiguous with end of header!"
                )));
            }
            let rest = &info[4..];
            if !rest.is_empty() {
                match cscan::sscanf_u32(rest) {
                    Some(v) => {
                        self.nio.data_file_dim = v;
                        if !(v >= 1 && v <= self.nrrd.dim) {
                            let m = format!(
                                "{me}: datafile dimension {} outside valid range [1,{}]",
                                v as i32, self.nrrd.dim
                            );
                            return self.fail(b(&m));
                        }
                    }
                    None => {
                        return self.fail(b(&format!(
                            "{me}: couldn't parse info after \"LIST\" as an int"
                        )));
                    }
                }
            } else {
                self.nio.data_file_dim = self.nrrd.dim.wrapping_sub(1);
            }
            loop {
                let linelen = self.one_line(file);
                if linelen > 0 {
                    let l = self.nio.line.clone();
                    self.nio.data_fn.push(l);
                } else {
                    break;
                }
            }
        } else {
            self.nio.data_fn.push(info.to_vec());
            self.nio.data_file_dim = self.nrrd.dim;
        }
        if self.data_fn_check().is_err() {
            return self.fail(b(&format!("{me}: trouble with number of datafiles")));
        }
        Ok(())
    }

    /// `_nrrdDataFNCheck`.
    fn data_fn_check(&mut self) -> R<()> {
        let me = "_nrrdDataFNCheck";
        let dim = self.nrrd.dim as usize;
        let num = self.nio.data_fn_number();
        if (self.nio.data_file_dim as usize) < dim {
            let split = self.nio.data_file_dim as usize;
            let piece_num: usize = self.nrrd.axis[split..dim.min(NRRD_DIM_MAX)]
                .iter()
                .fold(1usize, |a, x| a.wrapping_mul(x.size));
            if piece_num != num as usize {
                let m = format!(
                    "{me}: expected {} filenames (of {}-D pieces) but got {}",
                    piece_num as i32, self.nio.data_file_dim as i32, num as i32
                );
                return self.fail(b(&m));
            }
        } else {
            let last = dim.wrapping_sub(1);
            let size = if last < NRRD_DIM_MAX {
                self.nrrd.axis[last].size
            } else {
                self.notices.add(
                    &d::FS_771,
                    "a data file given before the dimension: NrrdIO reads an axis far outside \
                     its array (a crash); larmorx takes its size as 0",
                );
                0
            };
            if num as usize > size {
                let m = format!(
                    "{me}: can't have more pieces ({}) than axis {} slices ({size}) when nrrd \
                     dimension and datafile dimension are both {}",
                    num as i32,
                    self.nrrd.dim.wrapping_sub(1) as i32,
                    self.nrrd.dim as i32
                );
                return self.fail(b(&m));
            }
            if (size as f64 / num as f64) != ((size / num as usize) as f64) {
                let m = format!(
                    "{me}: number of datafiles ({}) doesn't divide into number of axis {} \
                     slices ({size})",
                    num as i32,
                    self.nrrd.dim.wrapping_sub(1) as i32
                );
                return self.fail(b(&m));
            }
        }
        Ok(())
    }

    /// `_nrrdHeaderCheck`.
    fn header_check(&mut self, check_seen: bool) -> R<()> {
        let me = "_nrrdHeaderCheck";
        if check_seen {
            const REQUIRED: [usize; 4] =
                [field::TYPE, field::DIMENSION, field::SIZES, field::ENCODING];
            for &i in &REQUIRED {
                if !self.nio.seen[i] {
                    let m = format!("{me}: didn't see required field: {}", FIELD_STR[i]);
                    return self.fail(b(&m));
                }
            }
        }
        if self.nrrd.type_ == types::BLOCK && self.nrrd.block_size == 0 {
            let m = format!("{me}: type is block, but missing field: block size");
            return self.fail(b(&m));
        }
        if self.nrrd.element_size() == 0 {
            return self.fail(b(&format!("{me}: nrrd reports zero element size!")));
        }
        if self.nio.endian == 0
            && self.nio.encoding.endian_matters()
            && self.nrrd.element_size() != 1
        {
            let m = format!(
                "{me}: type ({}) and encoding ({}) require endian info",
                type_enum().str_of(self.nrrd.type_),
                self.nio.encoding.name()
            );
            return self.fail(b(&m));
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------- data

    /// `nrrdIoStateDataFileIterNext`: the next data file (`None`: none left; the header's own
    /// file for attached data).
    fn data_file_next(&mut self) -> R<Option<Option<CFile>>> {
        let me = "nrrdIoStateDataFileIterNext";
        let num = self.nio.data_fn_number();
        if num == 0 {
            return self.fail(b(&format!("{me}: there appear to be zero datafiles!")));
        }
        self.nio.data_fn_index += 1;
        if self.nio.data_fn_index >= num as i32 {
            self.nio.data_fn_index = num as i32;
            return Ok(None);
        }
        let need_path = |s: &[u8]| s.first() != Some(&b'/') && s != b"-";
        let needed = match &self.nio.data_fn_format {
            Some(f) => need_path(f),
            None => self.nio.data_fn.iter().any(|n| need_path(n)),
        };
        if needed && self.nio.path.is_empty() {
            if self.notices.reproduce() {
                self.notices.add(
                    &d::FS_770,
                    "a header in the root directory with a relative data file name fails",
                );
                return self.fail(b(&format!(
                    "{me}: need nio->path for header-relative datafiles"
                )));
            }
            self.notices.add(
                &d::FS_770,
                "a relative data file name of a header in the root directory was read from / \
                 (NrrdIO fails)",
            );
        }
        let fname: Vec<u8> = if let Some(fmt) = self.nio.data_fn_format.clone() {
            let mut num_i = 0i32;
            let mut ii = self.nio.data_fn_min;
            loop {
                let go = (self.nio.data_fn_step > 0 && ii <= self.nio.data_fn_max)
                    || (self.nio.data_fn_step < 0 && ii >= self.nio.data_fn_max);
                if !go || num_i == self.nio.data_fn_index {
                    break;
                }
                num_i += 1;
                ii = ii.wrapping_add(self.nio.data_fn_step);
            }
            let name = match self.sprintf_d(&fmt, ii) {
                Ok(n) => n,
                Err(()) => {
                    return self.fail(cat(
                        &format!("{me}: the data file name template \""),
                        &fmt,
                        "\" holds a conversion other than one %d (NrrdIO's sprintf is undefined)",
                    ));
                }
            };
            if need_path(&fmt) {
                let mut f = self.nio.path.clone();
                f.push(b'/');
                f.extend_from_slice(&name);
                f
            } else {
                name
            }
        } else if !self.nio.data_fn.is_empty() {
            let n = self.nio.data_fn[self.nio.data_fn_index as usize].clone();
            if need_path(&n) {
                let mut f = self.nio.path.clone();
                f.push(b'/');
                f.extend_from_slice(&n);
                f
            } else {
                n
            }
        } else {
            return Ok(Some(None));
        };
        match CFile::open(&fname) {
            Ok(f) => Ok(Some(Some(f))),
            Err(_) => {
                let m = cat(
                    &format!("{me}: couldn't open \""),
                    &fname,
                    &format!(
                        "\" (data file {} of {}) for reading",
                        self.nio.data_fn_index + 1,
                        num as i32
                    ),
                );
                self.add_sized(m);
                Err(())
            }
        }
    }

    /// `sprintf(fname, format, ii)` for a template with one `%<digits>d` (and `%%`); any other
    /// conversion is undefined in C (FS-766: an error here), and a conversion wider than the
    /// ten digits NrrdIO's buffer allows overflows it (FS-766: the name is made whole).
    fn sprintf_d(&mut self, fmt: &[u8], ii: i32) -> R<Vec<u8>> {
        let mut out = Vec::new();
        let mut used = false;
        let mut i = 0;
        while i < fmt.len() {
            let c = fmt[i];
            if c != b'%' {
                out.push(c);
                i += 1;
                continue;
            }
            if fmt.get(i + 1) == Some(&b'%') {
                out.push(b'%');
                i += 2;
                continue;
            }
            let mut j = i + 1;
            let mut zero = false;
            while fmt.get(j) == Some(&b'0') {
                zero = true;
                j += 1;
            }
            let ws = j;
            while fmt.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            if used || fmt.get(j) != Some(&b'd') {
                self.notices.add(
                    &d::FS_766,
                    "the data file template holds a conversion other than one %d",
                );
                return Err(());
            }
            let width: usize = std::str::from_utf8(&fmt[ws..j])
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let digits = ii.unsigned_abs().to_string();
            let sign = if ii < 0 { "-" } else { "" };
            let body_len = sign.len() + digits.len();
            if body_len >= width {
                out.extend_from_slice(sign.as_bytes());
                out.extend_from_slice(digits.as_bytes());
            } else if zero {
                out.extend_from_slice(sign.as_bytes());
                out.resize(out.len() + width - body_len, b'0');
                out.extend_from_slice(digits.as_bytes());
            } else {
                out.resize(out.len() + width - body_len, b' ');
                out.extend_from_slice(sign.as_bytes());
                out.extend_from_slice(digits.as_bytes());
            }
            used = true;
            i = j + 1;
        }
        if out.len() > 10 + fmt.len() {
            self.notices.add(
                &d::FS_766,
                format!(
                    "a data file name of {} bytes was made whole (NrrdIO's buffer holds {})",
                    out.len(),
                    10 + fmt.len()
                ),
            );
        }
        Ok(out)
    }

    /// `nrrdLineSkip`.
    fn line_skip(&mut self, file: &mut CFile) -> R<()> {
        for lsi in 0..self.nio.line_skip {
            let r = self.one_line(file);
            if r == 0 {
                let m = format!(
                    "nrrdLineSkip: hit EOF skipping line {} of {}",
                    lsi + 1,
                    self.nio.line_skip
                );
                return self.fail(b(&m));
            }
        }
        Ok(())
    }

    /// `nrrdByteSkip`.
    fn byte_skip(&mut self, file: &mut CFile) -> R<()> {
        let me = "nrrdByteSkip";
        if self.nio.byte_skip <= -1 {
            if self.nio.encoding != Encoding::Raw {
                let m = format!(
                    "{me}: can do backwards byte skip only in raw encoding, not {}",
                    self.nio.encoding.name()
                );
                return self.fail(b(&m));
            }
            if file.stdin {
                return self.fail(b(&format!("{me}: can't fseek on stdin")));
            }
            let mut bsize = self.nrrd.element_number() / self.nio.data_fn_number() as usize;
            bsize = bsize.wrapping_mul(self.nrrd.element_size());
            let back_hack = -self.nio.byte_skip - 1;
            let off = -((bsize as i64).wrapping_add(back_hack as i64));
            if !file.seek_end(off) {
                let m = format!("{me}: failed to fseek(dataFile, {bsize}, SEEK_END)");
                return self.fail(b(&m));
            }
        } else {
            for i in 1..=self.nio.byte_skip {
                if file.getc() == EOF {
                    let m = format!("{me}: hit EOF skipping byte {i} of {}", self.nio.byte_skip);
                    return self.fail(b(&m));
                }
            }
        }
        Ok(())
    }

    /// The encoding's `read`: `n` elements into `data`.
    fn read_data(&mut self, file: &mut CFile, data: &mut [u8], n: usize) -> R<()> {
        let esize = self.nrrd.element_size();
        match self.nio.encoding {
            Encoding::Raw => {
                let got = file.fread(&mut data[..n * esize]) / esize;
                if got != n {
                    let pct = 100.0 * got as f64 / n as f64;
                    let m = format!(
                        "_nrrdEncodingRaw_read: fread got read only {got} {esize}-sized things, \
                         not {n} ({}% of expected)",
                        cfmt::g(pct, 6)
                    );
                    return self.fail(b(&m));
                }
                let _ = file.getc();
            }
            Encoding::Ascii => self.read_ascii(file, data, n)?,
            Encoding::Hex => self.read_hex(file, data, n)?,
            Encoding::Gzip => {
                return self.fail(b(
                    "_nrrdEncodingGzip_read: sorry, this nrrd not compiled with gzip enabled",
                ));
            }
            Encoding::Bzip2 => {
                return self.fail(b(
                    "_nrrdEncodingBzip2_read: Sorry, bzip2 encoding not available in NrrdIO",
                ));
            }
            Encoding::Unknown => {
                return self.fail(b(
                    "_nrrdEncodingUnknown_read: ERROR!!! trying to read unknown encoding",
                ));
            }
        }
        Ok(())
    }

    /// `fscanf(file, "%s", buf)`: the next run of non-space bytes (None at EOF before one).
    fn fscanf_s(file: &mut CFile) -> Option<Vec<u8>> {
        let mut c = file.getc();
        while c != EOF && cscan::is_c_space(c as u8) {
            c = file.getc();
        }
        if c == EOF {
            return None;
        }
        let mut tok = Vec::new();
        while c != EOF && !cscan::is_c_space(c as u8) {
            tok.push(c as u8);
            c = file.getc();
        }
        file.ungetc(c);
        Some(tok)
    }

    /// `_nrrdEncodingAscii_read`.
    fn read_ascii(&mut self, file: &mut CFile, data: &mut [u8], n: usize) -> R<()> {
        let me = "_nrrdEncodingAscii_read";
        let t = self.nrrd.type_;
        if t == types::BLOCK {
            return self.fail(b(&format!("{me}: can't read nrrd type block from ASCII")));
        }
        let esize = self.nrrd.element_size();
        for i in 0..n {
            let Some(raw) = Self::fscanf_s(file) else {
                let m = format!("{me}: couldn't parse element {} of {n}", i + 1);
                return self.fail(b(&m));
            };
            let tok = cstr(&raw).to_vec();
            if raw.len() >= AIR_STRLEN_HUGE {
                self.notices.add(
                    &d::FS_767,
                    format!(
                        "an ASCII value of {} bytes was read whole (NrrdIO's buffer holds {})",
                        raw.len(),
                        AIR_STRLEN_HUGE - 1
                    ),
                );
            }
            let at = &mut data[i * esize..(i + 1) * esize];
            let ok = if t >= types::INT {
                match t {
                    types::INT => {
                        cscan::sscanf_i32(&tok).map(|v| at.copy_from_slice(&v.to_le_bytes()))
                    }
                    types::UINT => {
                        cscan::sscanf_u32(&tok).map(|v| at.copy_from_slice(&v.to_le_bytes()))
                    }
                    types::LLONG => {
                        cscan::sscanf_i64(&tok).map(|v| at.copy_from_slice(&v.to_le_bytes()))
                    }
                    types::ULLONG => {
                        cscan::sscanf_u64(&tok).map(|v| at.copy_from_slice(&v.to_le_bytes()))
                    }
                    types::FLOAT => Self::single_sscanf_f32(&tok)
                        .map(|v| at.copy_from_slice(&v.to_bits().to_le_bytes())),
                    _ => Self::single_sscanf_f64(&tok)
                        .map(|v| at.copy_from_slice(&v.to_bits().to_le_bytes())),
                }
                .is_some()
            } else {
                match cscan::sscanf_i32(&tok) {
                    Some(v) => {
                        match t {
                            types::CHAR | types::UCHAR => at[0] = v as u8,
                            _ => at.copy_from_slice(&(v as u16).to_le_bytes()),
                        }
                        true
                    }
                    None => {
                        let m = cat(
                            &format!("{me}: couln't parse element {} of {n} (\"", i + 1),
                            &tok,
                            "\")",
                        );
                        return self.fail(m);
                    }
                }
            };
            if !ok {
                let m = cat(
                    &format!(
                        "{me}: couln't parse {} {} of {n} (\"",
                        type_enum().str_of(t),
                        i + 1
                    ),
                    &tok,
                    "\")",
                );
                return self.fail(m);
            }
        }
        Ok(())
    }

    /// `_nrrdEncodingHex_read`.
    fn read_hex(&mut self, file: &mut CFile, data: &mut [u8], n: usize) -> R<()> {
        let me = "_nrrdEncodingHex_read";
        let esize = self.nrrd.element_size();
        let nib_num = 2 * n * esize;
        let mut nib_idx = 0usize;
        let mut car = 0i32;
        let mut at = 0usize;
        while nib_idx < nib_num {
            car = file.getc();
            if car == EOF {
                break;
            }
            let nib = HEX_TABLE[(car & 127) as usize];
            if car >= 128 && nib != -2 {
                if self.notices.fix() {
                    self.notices.add(
                        &d::FS_769,
                        format!(
                            "the byte 0x{car:02x} in hex data is an error (NrrdIO reads it as \
                             the character of its low 7 bits)"
                        ),
                    );
                    break;
                }
                self.notices.add(
                    &d::FS_769,
                    format!("the byte 0x{car:02x} in hex data was read as its low 7 bits"),
                );
            }
            if nib == -2 {
                break;
            }
            if nib == -1 {
                continue;
            }
            data[at] = data[at].wrapping_add((nib << (4 * (1 - (nib_idx & 1)))) as u8);
            at += nib_idx & 1;
            nib_idx += 1;
        }
        if nib_idx != nib_num {
            let mut m = b(me);
            if car == EOF {
                m.extend_from_slice(
                    format!(": hit EOF getting byte {} of {}", nib_idx / 2, nib_num / 2).as_bytes(),
                );
            } else {
                m.extend_from_slice(b": hit invalid character ('");
                m.push(car as u8);
                m.extend_from_slice(
                    format!("') getting byte {} of {}", nib_idx / 2, nib_num / 2).as_bytes(),
                );
            }
            return self.fail(m);
        }
        Ok(())
    }

    /// `_nrrdFormatNRRD_read`.
    fn format_read(&mut self, file: &mut CFile) -> R<()> {
        let me = "_nrrdFormatNRRD_read";
        let mut llen;
        loop {
            self.nio.pos = 0;
            llen = self.one_line(file);
            if llen > 1 {
                let ret = self.parse_field();
                if ret == 0 {
                    let mut m = b(&format!("{me}: trouble parsing field in \""));
                    m.extend_from_slice(&self.nio.line);
                    m.push(b'"');
                    self.add_sized(m);
                    return Err(());
                }
                if self.nio.seen[ret] && !(ret == field::COMMENT || ret == field::KEYVALUE) {
                    let m = format!("{me}: already set field {}", FIELD_STR[ret]);
                    return self.fail(b(&m));
                }
                if self.parse_info(ret, file).is_err() {
                    if self.exit.is_some() {
                        return Err(());
                    }
                    let tail = self.nio.info();
                    let m = cat(
                        &format!("{me}: trouble parsing {} info \"", FIELD_STR[ret]),
                        &tail,
                        "\"",
                    );
                    return self.fail(m);
                }
                self.nio.seen[ret] = true;
            }
            if llen <= 1 {
                break;
            }
        }
        if llen == 0 && self.nio.data_fn_format.is_none() && self.nio.data_fn.is_empty() {
            return self.fail(b(&format!(
                "{me}: hit end of header, but no \"data file\" given"
            )));
        }
        if self.header_check(true).is_err() {
            let m = format!(
                "{me}: {}",
                if llen != 0 {
                    "finished reading header, but there were problems"
                } else {
                    "hit EOF before seeing a complete valid header"
                }
            );
            return self.fail(b(&m));
        }
        self.nio.data_fn_index = -1;
        let first = match self.data_file_next() {
            Ok(f) => f,
            Err(()) => {
                return self.fail(b(&format!("{me}: couldn't open the first datafile")));
            }
        };
        let esize = self.nrrd.element_size();
        let num = self.nrrd.element_number();
        let Some(total) = num.checked_mul(esize) else {
            self.notices.add(
                &d::FS_768,
                format!(
                    "{num} samples of {esize} bytes overflow size_t (NrrdIO allocates the \
                     wrapped size and reads past it)"
                ),
            );
            let m = format!("_nrrdCalloc: couldn't allocate {num} things of size {esize}");
            self.add(b(&m));
            return self.fail(b(&format!("{me}: couldn't allocate memory for data")));
        };
        let mut data = Vec::new();
        if data.try_reserve_exact(total).is_err() {
            let m = format!("_nrrdCalloc: couldn't allocate {num} things of size {esize}");
            self.add(b(&m));
            return self.fail(b(&format!("{me}: couldn't allocate memory for data")));
        }
        data.resize(total, 0u8);
        let vals_per_piece = num / self.nio.data_fn_number() as usize;
        let mut offset = 0usize;
        let mut current = first;
        while let Some(df) = current {
            let mut own;
            let f: &mut CFile = match df {
                Some(x) => {
                    own = x;
                    &mut own
                }
                None => &mut *file,
            };
            if self.line_skip(f).is_err() {
                return self.fail(b(&format!("{me}: couldn't skip lines")));
            }
            if !self.nio.encoding.is_compression() && self.byte_skip(f).is_err() {
                return self.fail(b(&format!("{me}: couldn't skip bytes")));
            }
            let end = offset + vals_per_piece * esize;
            if self
                .read_data(f, &mut data[offset..end], vals_per_piece)
                .is_err()
            {
                return self.fail(b(&format!("{me}:")));
            }
            offset = end;
            current = match self.data_file_next() {
                Ok(f) => f,
                Err(()) => {
                    return self.fail(b(&format!("{me}: couldn't get the next datafile")));
                }
            };
        }
        if self.nio.endian != 0
            && esize > 1
            && self.nio.encoding.endian_matters()
            && self.nio.endian != AIR_ENDIAN
        {
            match self.nrrd.type_ {
                types::BLOCK => self
                    .streams
                    .err("_nrrdBlockEndian: WARNING: can't fix endiannes of nrrd type block\n"),
                _ => {
                    for e in data[..num * esize].chunks_exact_mut(esize) {
                        e.reverse();
                    }
                }
            }
        }
        self.nrrd.data = Some(data);
        Ok(())
    }

    /// `_nrrdRead` (from a file).
    fn read(&mut self, file: &mut CFile) -> R<()> {
        let me = "_nrrdRead";
        let llen = self.one_line(file);
        if llen == 0 {
            return self.fail(b(&format!("{me}: immediately hit EOF")));
        }
        let magic = self.nio.line.as_slice();
        let is_nrrd = matches!(
            magic,
            b"NRRD00.01" | b"NRRD0001" | b"NRRD0002" | b"NRRD0003" | b"NRRD0004" | b"NRRD0005"
        );
        if !is_nrrd {
            let m = cat(
                &format!("{me}: couldn't parse \""),
                &self.nio.line.clone(),
                "\" as magic or beginning of any recognized format",
            );
            return self.fail(m);
        }
        if self.format_read(file).is_err() {
            if self.exit.is_some() {
                return Err(());
            }
            return self.fail(b(&format!("{me}: trouble reading NRRD file")));
        }
        if self.check().is_err() {
            if self.exit.is_some() {
                return Err(());
            }
            return self.fail(b(&format!("{me}: problem with nrrd after reading")));
        }
        Ok(())
    }
}

/// A file name's bytes as a path.
fn to_path(name: &[u8]) -> std::path::PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::path::PathBuf::from(std::ffi::OsStr::from_bytes(name))
    }
    #[cfg(not(unix))]
    {
        std::path::PathBuf::from(String::from_utf8_lossy(name).into_owned())
    }
}

/// `_nrrdReadHexTable`: -2 not allowed, -1 white space (9 to 13 and the space), 0 to 15 the
/// digits `0-9`, `A-F`, `a-f`.
const HEX_TABLE: [i8; 128] = {
    let mut t = [-2i8; 128];
    let mut i = 9;
    while i <= 13 {
        t[i] = -1;
        i += 1;
    }
    t[32] = -1;
    let mut k = 0;
    while k < 10 {
        t[48 + k] = k as i8;
        k += 1;
    }
    let mut k = 0;
    while k < 6 {
        t[65 + k] = 10 + k as i8;
        t[97 + k] = 10 + k as i8;
        k += 1;
    }
    t
};

/// `sscanf(s, "%d %d %d %u", ...)`: the values and the conversions made (the fourth as `%u`).
fn sscanf_ints(s: &[u8], max: usize) -> ([i64; 4], usize) {
    let mut vals = [0i64; 4];
    let mut n = 0;
    let mut at = 0usize;
    while n < max {
        if n > 0 {
            at += s[at..]
                .iter()
                .take_while(|&&c| cscan::is_c_space(c))
                .count();
        }
        if n < 3 {
            let (v, used) = cscan::strtol10(&s[at..]);
            if used == 0 {
                break;
            }
            vals[n] = v as i32 as i64;
            at += used;
        } else {
            let (v, used) = cscan::strtoul10(&s[at..]);
            if used == 0 {
                break;
            }
            vals[n] = (v as u32) as i64;
            at += used;
        }
        n += 1;
    }
    (vals, n)
}

/// `_nrrdSplitName(&path, NULL, name)`: the directory part (`.` when there is none).
fn split_path(name: &[u8]) -> Vec<u8> {
    match name.iter().rposition(|&c| c == b'/') {
        Some(w) if name.len() - w > 1 => name[..w].to_vec(),
        _ => b".".to_vec(),
    }
}

/// `nrrdLoad(nrrd, filename, nio)` with a new nrrd and I/O state, as FreeSurfer calls it:
/// the nrrd, or how it failed (the messages added to `biff`). NrrdIO's defects follow
/// `notices`' mode, each that changed the result noted there; what NrrdIO prints on its own
/// goes to `streams`.
pub fn load_with(
    filename: &[u8],
    biff: &mut Biff,
    notices: &mut Notices,
    streams: &mut dyn Streams,
) -> Result<Nrrd, LoadError> {
    let mut r = Reader {
        biff,
        notices,
        streams,
        nio: Nio::new(),
        nrrd: Nrrd::default(),
        exit: None,
    };
    r.nio.path = split_path(filename);
    let mut file = match CFile::open(filename) {
        Ok(f) => f,
        Err(errno) => {
            let m = cat(
                "nrrdLoad: fopen(\"",
                filename,
                &format!("\",\"rb\") failed: {}", os_text(errno)),
            );
            r.add(m);
            return Err(LoadError::Failed { code: 2 });
        }
    };
    if r.read(&mut file).is_err() {
        if let Some((status, message)) = r.exit.take() {
            return Err(LoadError::Exit { status, message });
        }
        r.add(b("nrrdRead: trouble"));
        let m = cat("nrrdLoad: trouble reading \"", filename, "\"");
        r.add(m);
        return Err(LoadError::Failed { code: 1 });
    }
    Ok(r.nrrd)
}

/// [`load_with`] whose messages of NrrdIO's own go nowhere.
pub fn load(filename: &[u8], biff: &mut Biff, notices: &mut Notices) -> Result<Nrrd, LoadError> {
    load_with(filename, biff, notices, &mut cfmt::Silent)
}

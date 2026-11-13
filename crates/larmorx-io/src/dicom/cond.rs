// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-CTN
//! CTN's conditions: the codes (`FORM_COND`), the messages of its object library (`dcmcond.c`),
//! the C formatting of their arguments, and the condition stack (`condition.c`), with the defined
//! behaviour of FS-90 where a message would overflow CTN's buffer.

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self, Notices};

/// CTN's `CONDITION` (an `unsigned long`).
pub type Condition = u64;

/// `FORM_COND(facility, severity, value)`.
pub const fn form_cond(facility: u64, severity: u64, value: u64) -> Condition {
    (value << 16) | (facility << 4) | severity
}

pub const SEV_SUCC: u64 = 1;
pub const SEV_INFORM: u64 = 3;
pub const SEV_WARN: u64 = 5;
pub const SEV_ERROR: u64 = 2;
pub const SEV_FATAL: u64 = 4;
pub const FAC_COND: u64 = 6;
pub const FAC_DCM: u64 = 9;

pub const COND_NORMAL: Condition = form_cond(FAC_COND, SEV_SUCC, 1);

pub const DCM_NORMAL: Condition = form_cond(FAC_DCM, SEV_SUCC, 1);
pub const DCM_FILEOPENFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 2);
pub const DCM_FILEACCESSERROR: Condition = form_cond(FAC_DCM, SEV_ERROR, 3);
pub const DCM_OBJECTCREATEFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 4);
pub const DCM_NULLOBJECT: Condition = form_cond(FAC_DCM, SEV_ERROR, 5);
pub const DCM_ILLEGALOBJECT: Condition = form_cond(FAC_DCM, SEV_ERROR, 6);
pub const DCM_ELEMENTNOTFOUND: Condition = form_cond(FAC_DCM, SEV_ERROR, 7);
pub const DCM_ILLEGALSTREAMLENGTH: Condition = form_cond(FAC_DCM, SEV_ERROR, 8);
pub const DCM_ELEMENTCREATEFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 9);
pub const DCM_UNRECOGNIZEDGROUP: Condition = form_cond(FAC_DCM, SEV_WARN, 10);
pub const DCM_UNRECOGNIZEDELEMENT: Condition = form_cond(FAC_DCM, SEV_WARN, 11);
pub const DCM_ELEMENTOUTOFORDER: Condition = form_cond(FAC_DCM, SEV_ERROR, 12);
pub const DCM_LISTFAILURE: Condition = form_cond(FAC_DCM, SEV_ERROR, 13);
pub const DCM_ILLEGALOPTION: Condition = form_cond(FAC_DCM, SEV_ERROR, 14);
pub const DCM_ILLEGALADD: Condition = form_cond(FAC_DCM, SEV_ERROR, 19);
pub const DCM_GETINCOMPLETE: Condition = form_cond(FAC_DCM, SEV_WARN, 20);
pub const DCM_ILLEGALCONTEXT: Condition = form_cond(FAC_DCM, SEV_ERROR, 21);
pub const DCM_ILLEGALREPRESENTATION: Condition = form_cond(FAC_DCM, SEV_ERROR, 22);
pub const DCM_UNEVENELEMENTLENGTH: Condition = form_cond(FAC_DCM, SEV_ERROR, 23);
pub const DCM_ELEMENTLENGTHERROR: Condition = form_cond(FAC_DCM, SEV_ERROR, 25);
pub const DCM_GROUPNOTFOUND: Condition = form_cond(FAC_DCM, SEV_ERROR, 27);
pub const DCM_FILECREATEFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 28);
pub const DCM_FILEIOERROR: Condition = form_cond(FAC_DCM, SEV_ERROR, 29);
pub const DCM_INSERTFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 30);
pub const DCM_CANNOTGETSEQUENCEVALUE: Condition = form_cond(FAC_DCM, SEV_ERROR, 31);
pub const DCM_FILEDELETEFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 32);
pub const DCM_MALLOCFAILURE: Condition = form_cond(FAC_DCM, SEV_ERROR, 33);
pub const DCM_NULLADDRESS: Condition = form_cond(FAC_DCM, SEV_ERROR, 34);
pub const DCM_UNEXPECTEDREPRESENTATION: Condition = form_cond(FAC_DCM, SEV_ERROR, 35);
pub const DCM_BADELEMENTINGROUP: Condition = form_cond(FAC_DCM, SEV_ERROR, 36);
pub const DCM_CALLBACKABORTED: Condition = form_cond(FAC_DCM, SEV_WARN, 37);
pub const DCM_READSTREAMFAILED: Condition = form_cond(FAC_DCM, SEV_ERROR, 38);
pub const DCM_STREAMCOMPLETE: Condition = form_cond(FAC_DCM, SEV_SUCC, 39);
pub const DCM_UNRECOGNIZEDVRCODE: Condition = form_cond(FAC_DCM, SEV_ERROR, 40);
pub const DCM_VRMISMATCH: Condition = form_cond(FAC_DCM, SEV_ERROR, 41);
pub const DCM_EXPORTBUFFERTOOSMALL: Condition = form_cond(FAC_DCM, SEV_ERROR, 42);
pub const DCM_BADOFFSET: Condition = form_cond(FAC_DCM, SEV_ERROR, 43);
pub const DCM_BADLENGTH: Condition = form_cond(FAC_DCM, SEV_ERROR, 44);
pub const DCM_NOTASEQUENCE: Condition = form_cond(FAC_DCM, SEV_ERROR, 45);
pub const DCM_GENERALWARNING: Condition = form_cond(FAC_DCM, SEV_WARN, 46);
pub const DCM_EMPTYOBJECT: Condition = form_cond(FAC_DCM, SEV_WARN, 47);
pub const DCM_GETNEXTELEMENTCOMPLETE: Condition = form_cond(FAC_DCM, SEV_SUCC, 48);
pub const DCM_REPEATEDELEMENT: Condition = form_cond(FAC_DCM, SEV_WARN, 49);
pub const DCM_NOFRAGMENTSINOBJECT: Condition = form_cond(FAC_DCM, SEV_ERROR, 50);
pub const DCM_UNEVENFRAGMENTLENGTH: Condition = form_cond(FAC_DCM, SEV_ERROR, 51);

/// CTN 3.0.6's message table (`messageVector`, searched in order).
static MESSAGES: &[(Condition, &[u8])] = &[
    (DCM_NORMAL, b"Normal return from DCM routine"),
    (DCM_FILEOPENFAILED, b"DCM failed to open file: %s in %s"),
    (DCM_FILEACCESSERROR, b"DCM failed to access file: %s in %s"),
    (DCM_OBJECTCREATEFAILED, b"DCM failed to create object in %s"),
    (DCM_NULLOBJECT, b"NULL object passed to routine %s"),
    (DCM_ILLEGALOBJECT, b"Illegal object passed to routine %s"),
    (DCM_ELEMENTNOTFOUND, b"Requested element (%x %x) not found in %s"),
    (
        DCM_ILLEGALSTREAMLENGTH,
        b"DCM Illegal Stream Length (%ld) (Not enough data to define a full element) in %s",
    ),
    (DCM_ELEMENTCREATEFAILED, b"DCM failed to create element in %s (%04x %04x %d)"),
    (DCM_UNRECOGNIZEDGROUP, b"DCM unrecognized group: %04x in %s"),
    (DCM_UNRECOGNIZEDELEMENT, b"DCM unrecognized element: (%04x %04x) in %s"),
    (DCM_ELEMENTOUTOFORDER, b"DCM group/element out of order (%04x %04x) in %s"),
    (DCM_LISTFAILURE, b"DCM routine failed on list operation in %s"),
    (DCM_ILLEGALOPTION, b"DCM illegal stream option: %s"),
    (DCM_ILLEGALADD, b"DCM attempt to add illegal element: %x %x in %s"),
    (DCM_GETINCOMPLETE, b"DCM Get Element incomplete in %s"),
    (DCM_ILLEGALCONTEXT, b"DCM Illegal context value in %s"),
    (
        DCM_ILLEGALREPRESENTATION,
        b"DCM Caller specified illegal representation for element (%04x %04x) in %s",
    ),
    (
        DCM_UNEVENELEMENTLENGTH,
        b"DCM attempt to add data element (%x %x) with uneven length (%ld)  in %s",
    ),
    (
        DCM_ELEMENTLENGTHERROR,
        b"DCM Data Element (%04x %04x) longer (%ld) than remaining length (%ld) of data in stream or file in %s",
    ),
    (DCM_GROUPNOTFOUND, b"Requested group (%x) not found in %s"),
    (DCM_FILECREATEFAILED, b"DCM failed to create file %s (%s)"),
    (DCM_FILEIOERROR, b"DCM io error on file %s (%s)"),
    (DCM_INSERTFAILED, b"DCM failed to insert new element (%04x %04x) in %s"),
    (
        DCM_CANNOTGETSEQUENCEVALUE,
        b"DCM Cannot retrieve value of element with SQ representation (%08x) in (%s)",
    ),
    (DCM_FILEDELETEFAILED, b"DCM Failed to delete file %s in %s"),
    (DCM_MALLOCFAILURE, b"DCM Failed to malloc %ld bytes in %s"),
    (DCM_NULLADDRESS, b"DCM NULL address passed to routine %s"),
    (
        DCM_UNEXPECTEDREPRESENTATION,
        b"DCM Routine %s expected %s representation for element %04x %04x",
    ),
    (DCM_BADELEMENTINGROUP, b"DCM Bad element (%04x %04x) found in group %04x in %s"),
    (DCM_CALLBACKABORTED, b"DCM Callback aborted by user in %s"),
    (DCM_READSTREAMFAILED, b"DCM Failed to read stream in %s"),
    (DCM_UNRECOGNIZEDVRCODE, b"DCM Unrecognized VR code (%s) in %s"),
    (DCM_VRMISMATCH, b"DCM Incorrect VR (%s) for attribute with tag %08x"),
    (
        DCM_EXPORTBUFFERTOOSMALL,
        b"DCM Caller's export buffer length (%d) is too short in %s",
    ),
    (DCM_BADOFFSET, b"DCM Offset value (%d) larger than attribute length (%d) in %s"),
    (
        DCM_BADLENGTH,
        b"DCM Combination of offset, length (%d %d) is longer than element length (%d) in %s",
    ),
    (
        DCM_NOTASEQUENCE,
        b"DCM Attempt to perform sequence operation on element (%04x %04x) not a sequence in %s",
    ),
    (DCM_GENERALWARNING, b"DCM General warning in %s: %s"),
    (
        DCM_UNEVENFRAGMENTLENGTH,
        b"DCM attempt to add fragment with uneven length (%ld) in %s",
    ),
];

/// `DCM_Message`: CTN 3.0.6's message of `c` (`None`: C's `NULL`).
pub fn ctn_message(c: Condition) -> Option<&'static [u8]> {
    MESSAGES.iter().find(|(k, _)| *k == c).map(|(_, m)| *m)
}

/// One argument of a C variadic call, with the C type the caller passes (the conversion reads it
/// as its own type, as GCC's x86-64 calls do: a 32-bit value passed where `%ld` reads 64 bits is
/// zero-extended, a 64-bit value read by `%d` gives its low 32 bits).
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    /// A C string (its bytes up to the first zero byte are printed).
    Str(&'a [u8]),
    /// An `int` (also an `unsigned short` promoted to `int`).
    Int(i32),
    /// An `unsigned int` (CTN's `U32`).
    U32(u32),
    /// A `long` or `unsigned long` or `size_t`.
    Long(u64),
}

impl Arg<'_> {
    fn bits32(self) -> u32 {
        match self {
            Arg::Int(v) => v as u32,
            Arg::U32(v) => v,
            Arg::Long(v) => v as u32,
            Arg::Str(_) => 0,
        }
    }
    fn bits64(self) -> u64 {
        match self {
            Arg::Int(v) => v as u32 as u64,
            Arg::U32(v) => v as u64,
            Arg::Long(v) => v,
            Arg::Str(_) => 0,
        }
    }
}

/// C's `vsprintf` for the conversions CTN's messages use (`%s %d %u %x %ld %lu %lx %%`, flags
/// `0` and `-`, a width). A format past its arguments reads nothing (CTN's calls always give at
/// least as many).
pub fn c_format(format: &[u8], args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut next = args.iter().copied();
    let mut i = 0;
    while i < format.len() {
        let c = format[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        let (mut zero, mut left) = (false, false);
        while i < format.len() && (format[i] == b'0' || format[i] == b'-') {
            if format[i] == b'0' {
                zero = true;
            } else {
                left = true;
            }
            i += 1;
        }
        let mut width = 0usize;
        while i < format.len() && format[i].is_ascii_digit() {
            width = width * 10 + (format[i] - b'0') as usize;
            i += 1;
        }
        let mut long = false;
        while i < format.len() && format[i] == b'l' {
            long = true;
            i += 1;
        }
        let Some(&conv) = format.get(i) else { break };
        i += 1;
        let body: Vec<u8> = match conv {
            b'%' => {
                out.push(b'%');
                continue;
            }
            b's' => match next.next() {
                Some(Arg::Str(s)) => {
                    let n = s.iter().position(|&b| b == 0).unwrap_or(s.len());
                    s[..n].to_vec()
                }
                _ => b"(null)".to_vec(),
            },
            b'd' | b'i' => {
                let a = next.next().unwrap_or(Arg::Int(0));
                if long {
                    (a.bits64() as i64).to_string().into_bytes()
                } else {
                    (a.bits32() as i32).to_string().into_bytes()
                }
            }
            b'u' => {
                let a = next.next().unwrap_or(Arg::Int(0));
                if long {
                    a.bits64().to_string().into_bytes()
                } else {
                    a.bits32().to_string().into_bytes()
                }
            }
            b'x' => {
                let a = next.next().unwrap_or(Arg::Int(0));
                if long {
                    format!("{:x}", a.bits64()).into_bytes()
                } else {
                    format!("{:x}", a.bits32()).into_bytes()
                }
            }
            other => vec![b'%', other],
        };
        let numeric = conv != b's';
        if body.len() >= width {
            out.extend_from_slice(&body);
        } else if left {
            out.extend_from_slice(&body);
            out.resize(out.len() + width - body.len(), b' ');
        } else if zero && numeric {
            let (sign, digits) = if body.first() == Some(&b'-') {
                (&body[..1], &body[1..])
            } else {
                (&body[..0], &body[..])
            };
            out.extend_from_slice(sign);
            out.resize(out.len() + width - body.len(), b'0');
            out.extend_from_slice(digits);
        } else {
            out.resize(out.len() + width - body.len(), b' ');
            out.extend_from_slice(&body);
        }
    }
    out
}

/// glibc's message of `errno` (`strerror`, Linux's numbers), the same on every platform.
pub fn os_text(errno: i32) -> String {
    match errno {
        0 => "Success",
        1 => "Operation not permitted",
        2 => "No such file or directory",
        4 => "Interrupted system call",
        5 => "Input/output error",
        9 => "Bad file descriptor",
        12 => "Cannot allocate memory",
        13 => "Permission denied",
        20 => "Not a directory",
        21 => "Is a directory",
        22 => "Invalid argument",
        24 => "Too many open files",
        36 => "File name too long",
        40 => "Too many levels of symbolic links",
        _ => return format!("Unknown error {errno}"),
    }
    .to_owned()
}

/// The Linux `errno` of an I/O error (by its kind, so that every platform gives Linux's).
pub fn errno_of(e: &std::io::Error) -> i32 {
    use std::io::ErrorKind::*;
    match e.kind() {
        NotFound => 2,
        PermissionDenied => 13,
        Interrupted => 4,
        OutOfMemory => 12,
        NotADirectory => 20,
        IsADirectory => 21,
        InvalidInput => 22,
        InvalidFilename => 36,
        _ => 5,
    }
}

/// `errno` values CTN's calls leave (Linux's).
pub const EBADF: i32 = 9;
pub const EINVAL: i32 = 22;

/// The stack's size (`MAXEDB`).
pub const MAXEDB: usize = 100;
/// CTN's formatting buffer (`COND_PushCondition`'s `buffer[1024]`).
pub const FORMAT_BUFFER: usize = 1024;
/// The bytes of a text the stack keeps.
pub const TEXT_KEPT: usize = 256;

/// CTN's process state: its condition stack (`condition.c`'s `EDBStack` and `stackPtr`; entries
/// above the top stay as they were, the overflow rule exposes entry 0), and the C library's
/// `errno` and file descriptors as CTN's own calls leave them (a read that fails, a descriptor
/// closed twice: the text a later `perror` prints). One per process in C, one per value here.
#[derive(Clone, Debug)]
pub struct CondStack {
    entries: Vec<(Condition, Vec<u8>)>,
    ptr: i32,
    /// `errno` (0 at the start: "Success").
    pub errno: i32,
    /// Whether standard error is open for reading too (a terminal), or for writing only (a file
    /// or a pipe, the default): glibc's first `perror` on a stream nothing has written to yet
    /// reopens it with `fdopen(dup(2), "w+")`, which leaves `EINVAL` in `errno` when it is
    /// write-only.
    pub stderr_read_write: bool,
    /// Whether anything was written to standard error (its stream's orientation is set).
    pub stderr_oriented: bool,
    fds: Vec<i32>,
}

impl Default for CondStack {
    fn default() -> Self {
        CondStack::new()
    }
}

impl CondStack {
    pub fn new() -> CondStack {
        CondStack {
            entries: vec![(0, Vec::new()); MAXEDB],
            ptr: -1,
            errno: 0,
            stderr_read_write: false,
            stderr_oriented: false,
            fds: Vec::new(),
        }
    }

    /// A descriptor `open` gives (the lowest free one after the standard streams).
    pub fn open_fd(&mut self) -> i32 {
        let mut fd = 3;
        while self.fds.contains(&fd) {
            fd += 1;
        }
        self.fds.push(fd);
        fd
    }

    /// `close(fd)`: `EBADF` in `errno` when it is not open.
    pub fn close_fd(&mut self, fd: i32) {
        match self.fds.iter().position(|&f| f == fd) {
            Some(i) => {
                self.fds.remove(i);
            }
            None => self.errno = EBADF,
        }
    }

    /// The entries, bottom first (as `COND_DumpConditions` prints them).
    pub fn entries(&self) -> &[(Condition, Vec<u8>)] {
        &self.entries[..(self.ptr + 1) as usize]
    }

    /// `COND_PushCondition(cond, format, args...)`: the message formatted, its first 256 bytes
    /// kept; when the stack reaches 98 entries it is printed to standard error with "CONDITION
    /// Stack overflow" and its top reset to entry 0. A message over 1,023 bytes (a long file
    /// name) overflows CTN's buffer (FS-90): kept in full here before the cut to 256 bytes.
    pub fn push(
        &mut self,
        cond: Condition,
        format: Option<&[u8]>,
        args: &[Arg],
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Condition {
        let format = format.unwrap_or(b"NULL Control string passedto PushCondition");
        let mut text = c_format(format, args);
        if text.len() >= FORMAT_BUFFER {
            notices.add(
                &defects::FS_90,
                format!("a condition message of {} bytes", text.len()),
            );
        }
        if let Some(n) = text.iter().position(|&b| b == 0) {
            text.truncate(n);
        }
        text.truncate(TEXT_KEPT);
        self.ptr += 1;
        self.entries[self.ptr as usize] = (cond, text);
        if self.ptr >= MAXEDB as i32 - 2 {
            self.dump_to(out);
            out.err("CONDITION Stack overflow\n");
            self.stderr_oriented = true;
            self.ptr = 0;
        }
        cond
    }

    /// `COND_PopCondition(clearstack)`: the top's code (`COND_NORMAL` on an empty stack).
    pub fn pop(&mut self, clear: bool) -> Condition {
        let value = if self.ptr >= 0 {
            self.entries[self.ptr as usize].0
        } else {
            COND_NORMAL
        };
        if clear || self.ptr <= 0 {
            self.ptr = -1;
        } else {
            self.ptr -= 1;
        }
        value
    }

    /// `COND_TopCondition(&code, text, maxlength)`: the top's code and its text cut to
    /// `maxlength - 1` bytes.
    pub fn top(&self, maxlength: usize) -> (Condition, Condition, Vec<u8>) {
        if self.ptr >= 0 {
            let (c, t) = &self.entries[self.ptr as usize];
            let n = t.len().min(maxlength.saturating_sub(1));
            (*c, *c, t[..n].to_vec())
        } else {
            (COND_NORMAL, COND_NORMAL, Vec::new())
        }
    }

    /// `perror(prefix)`: the message of `errno` on standard error, and what glibc leaves in it.
    pub fn perror(&mut self, prefix: &str, out: &mut dyn Streams) {
        let text = os_text(self.errno);
        if prefix.is_empty() {
            out.err(&format!("{text}\n"));
        } else {
            out.err(&format!("{prefix}: {text}\n"));
        }
        if !self.stderr_oriented && !self.stderr_read_write {
            self.errno = EINVAL;
        }
        self.stderr_oriented = true;
    }

    fn dump_to(&mut self, out: &mut dyn Streams) {
        if self.ptr >= 0 {
            self.stderr_oriented = true;
        }
        for (c, t) in self.entries().to_vec() {
            out.err(&format!("{:8x} {}\n", c as u32, String::from_utf8_lossy(&t)));
        }
    }

    /// `COND_DumpConditions`: the stack printed to standard error, bottom first, then emptied.
    pub fn dump(&mut self, out: &mut dyn Streams) {
        self.dump_to(out);
        self.ptr = -1;
    }
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's messages and fatal errors (`fs.core`, queue item 3.1-22), translated from
//! FreeSurfer 8.2.0's `include/log.h` (the stream loggers `fs::fatal`, `fs::error`,
//! `fs::warning`, `fs::debug`, the error codes, `ErrorReturn`) and `utils/log.cpp` (`Gerror`,
//! `setErrorLog`, `throwExceptions`, `writeToErrorLog`, `errorExit`, `ErrorPrintf`,
//! `ErrorExit`; the terminal colours are always off there, `termAllowsColor` returns false).
//!
//! Every message goes to standard error as `error: <message>\n` (`warning: `, `debug: `). The
//! callers format their messages; the C format strings' conversions are theirs to reproduce.
//! Where FreeSurfer exits (`exit(code)`), the translation returns [`Fatal`] and the program's
//! entry point turns it into its exit status ([`Fatal::status`]): `exit` keeps the code's low
//! 8 bits (`ERROR_BADPARM`, -4, exits with 252).
//!
//! **FS-01** (corrected by default): `ErrorPrintf` and `ErrorExit` build their message with
//! `std::string(zc.data(), zc.size())` from a buffer of the formatted length plus one, so the
//! string ends with the terminating NUL byte, which FreeSurfer then writes before the newline
//! (checked on the official `mri_info`: `error: mghRead(...): could not open file\0\n`). The
//! default writes the message without it; `reproduce` writes it, on standard error and in the
//! error log.

use std::io::Write;

use larmorx_core::defects::{FS_01, FS_263};

use crate::diag::DIAG_VERBOSE;
use crate::globals::Globals;

/// FreeSurfer's error codes (`include/log.h`).
pub const NO_ERROR: i32 = 0;
pub const ERROR_NONE: i32 = NO_ERROR;
pub const ERROR_NO_FILE: i32 = -1;
pub const ERROR_NOFILE: i32 = ERROR_NO_FILE;
pub const ERROR_NO_MEMORY: i32 = -2;
pub const ERROR_NOMEMORY: i32 = ERROR_NO_MEMORY;
pub const ERROR_UNSUPPORTED: i32 = -3;
pub const ERROR_BADPARM: i32 = -4;
pub const ERROR_BAD_PARM: i32 = ERROR_BADPARM;
pub const ERROR_BADFILE: i32 = -5;
pub const ERROR_BAD_FILE: i32 = ERROR_BADFILE;
pub const ERROR_SIZE: i32 = -6;
pub const ERROR_BADLOOP: i32 = -7;
pub const ERROR_OUT_OF_BOUNDS: i32 = -8;

/// How a fatal error ends the program: `exit(code)`, or, after `throwExceptions(true)`, a C++
/// exception carrying the code (`fs::detail::errorExit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fatal {
    /// `exit(code)`.
    Exit(i32),
    /// `throw code` (exceptions on; never with code 0).
    Throw(i32),
}

impl Fatal {
    /// The code FreeSurfer passed.
    pub fn code(self) -> i32 {
        match self {
            Fatal::Exit(c) | Fatal::Throw(c) => c,
        }
    }

    /// The exit status a process ending with this code has: its low 8 bits, as `exit` keeps
    /// them (an exception nobody catches is the program's own matter).
    pub fn status(self) -> u8 {
        (self.code() & 0xff) as u8
    }
}

/// libstdc++'s line when an exception leaves a function that may not throw (`std::terminate`'s
/// verbose handler, for an `int`).
pub const TERMINATE_INT: &str = "terminate called after throwing an instance of 'int'\n";

/// `fs::detail::errorExit(code)` as `fs::fatal`'s destructor (its only caller) calls it: with
/// exceptions off, `exit(code)`; with them on, `throw code`, except for code 0, which neither
/// throws nor exits (`Ok`: the program goes on). FS-263: thrown from a destructor, the exception
/// never reaches the program's handler: C++ terminates the program ([`TERMINATE_INT`] on standard
/// error, then `abort`, status 134). By default the code reaches the caller ([`Fatal::Throw`]);
/// `reproduce` writes the line and ends with 134 ([`Fatal::Exit`]).
pub fn error_exit_code(g: &mut Globals, err: &mut dyn Write, code: i32) -> Result<(), Fatal> {
    if g.exceptions {
        if code != 0 {
            g.notices.add(
                &FS_263,
                format!(
                    "a fatal error with code {code} and exceptions on: FreeSurfer aborts (status 134)"
                ),
            );
            if g.notices.reproduce() {
                let _ = err.write_all(TERMINATE_INT.as_bytes());
                return Err(Fatal::Exit(134));
            }
            return Err(Fatal::Throw(code));
        }
        Ok(())
    } else {
        Err(Fatal::Exit(code))
    }
}

/// `fs::detail::writeToErrorLog`: appends `message` and a newline to the error log, when one is
/// set (`std::ofstream` in append mode; a log that cannot be opened is skipped silently).
pub fn write_to_error_log(g: &Globals, message: &[u8]) {
    let Some(path) = &g.error_log else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        let _ = f.write_all(message);
        let _ = f.write_all(b"\n");
    }
}

/// `fs::error() << message`: `error: <message>\n` on standard error, and the message in the
/// error log.
pub fn error(g: &Globals, err: &mut dyn Write, message: &str) {
    error_bytes(g, err, message.as_bytes());
}

fn error_bytes(g: &Globals, err: &mut dyn Write, message: &[u8]) {
    let _ = err.write_all(b"error: ");
    let _ = err.write_all(message);
    let _ = err.write_all(b"\n");
    write_to_error_log(g, message);
}

/// `fs::fatal(code) << message`: [`error`]'s output, then [`error_exit_code`].
pub fn fatal(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Result<(), Fatal> {
    error_bytes(g, err, message.as_bytes());
    error_exit_code(g, err, code)
}

/// `fs::warning() << message`: `warning: <message>\n` on standard error (not in the log).
pub fn warning(err: &mut dyn Write, message: &str) {
    let _ = writeln!(err, "warning: {message}");
}

/// `fs::debug() << message`: `debug: <message>\n` on standard error when `Gdiag` has
/// `DIAG_VERBOSE` (`DIAG_VERBOSE_ON`).
pub fn debug(g: &Globals, err: &mut dyn Write, message: &str) {
    if g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(err, "debug: {message}");
    }
}

/// The message `ErrorPrintf` and `ErrorExit` write: `message`, with FS-01's NUL byte under
/// `reproduce` (the default drops it and notes FS-01).
fn c_message(g: &mut Globals, message: &str) -> Vec<u8> {
    c_message_bytes(g, message.as_bytes())
}

fn c_message_bytes(g: &mut Globals, message: &[u8]) -> Vec<u8> {
    let mut bytes = message.to_vec();
    if g.notices.reproduce() {
        bytes.push(0);
    }
    g.notices.add(
        &FS_01,
        "an error message without FreeSurfer's NUL byte before its newline",
    );
    bytes
}

/// `ErrorPrintf(errcode, fmt, ...)` with the message formatted: `Gerror = errcode`; when
/// `errno` is not 0, its `strerror` text as an error first; then the message as an error (FS-01).
pub fn error_printf(g: &mut Globals, err: &mut dyn Write, errcode: i32, message: &str) {
    g.gerror = errcode;
    if g.errno != 0 {
        error(g, err, &strerror(g.errno));
    }
    let bytes = c_message(g, message);
    error_bytes(g, err, &bytes);
}

/// [`error_printf`] of a message of bytes (a C string that need not be UTF-8).
pub fn error_printf_bytes(g: &mut Globals, err: &mut dyn Write, errcode: i32, message: &[u8]) {
    g.gerror = errcode;
    if g.errno != 0 {
        error(g, err, &strerror(g.errno));
    }
    let bytes = c_message_bytes(g, message);
    error_bytes(g, err, &bytes);
}

/// `ErrorExit(errcode, fmt, ...)` with the message formatted: as [`error_printf`], then
/// [`error_exit_code`] with `errcode`.
pub fn error_exit(
    g: &mut Globals,
    err: &mut dyn Write,
    errcode: i32,
    message: &str,
) -> Result<(), Fatal> {
    g.gerror = errcode;
    if g.errno != 0 {
        error(g, err, &strerror(g.errno));
    }
    let bytes = c_message(g, message);
    error_bytes(g, err, &bytes);
    error_exit_code(g, err, errcode)
}

/// glibc 2.35's `strerror` text for the `errno` values FreeSurfer's I/O meets (Linux's codes;
/// glibc's messages in the C and English locales); any other code gives `Unknown error <n>`, as
/// glibc does for codes it has no message for.
pub fn strerror(errno: i32) -> String {
    let text = match errno {
        0 => "Success",
        1 => "Operation not permitted",
        2 => "No such file or directory",
        3 => "No such process",
        4 => "Interrupted system call",
        5 => "Input/output error",
        6 => "No such device or address",
        7 => "Argument list too long",
        8 => "Exec format error",
        9 => "Bad file descriptor",
        10 => "No child processes",
        11 => "Resource temporarily unavailable",
        12 => "Cannot allocate memory",
        13 => "Permission denied",
        14 => "Bad address",
        15 => "Block device required",
        16 => "Device or resource busy",
        17 => "File exists",
        18 => "Invalid cross-device link",
        19 => "No such device",
        20 => "Not a directory",
        21 => "Is a directory",
        22 => "Invalid argument",
        23 => "Too many open files in system",
        24 => "Too many open files",
        25 => "Inappropriate ioctl for device",
        26 => "Text file busy",
        27 => "File too large",
        28 => "No space left on device",
        29 => "Illegal seek",
        30 => "Read-only file system",
        31 => "Too many links",
        32 => "Broken pipe",
        33 => "Numerical argument out of domain",
        34 => "Numerical result out of range",
        36 => "File name too long",
        38 => "Function not implemented",
        39 => "Directory not empty",
        40 => "Too many levels of symbolic links",
        75 => "Value too large for defined data type",
        122 => "Disk quota exceeded",
        _ => return format!("Unknown error {errno}"),
    };
    text.to_owned()
}

/// The `errno` value of an I/O error, as the C library would have set it (`None` when the
/// error carries none: then the caller decides).
pub fn errno_of(e: &std::io::Error) -> Option<i32> {
    if let Some(code) = raw_errno(e) {
        return Some(code);
    }
    use std::io::ErrorKind as K;
    Some(match e.kind() {
        K::NotFound => 2,
        K::PermissionDenied => 13,
        K::AlreadyExists => 17,
        K::InvalidInput => 22,
        K::IsADirectory => 21,
        K::NotADirectory => 20,
        K::StorageFull => 28,
        K::ReadOnlyFilesystem => 30,
        K::InvalidFilename => 36,
        K::DirectoryNotEmpty => 39,
        K::OutOfMemory => 12,
        _ => return None,
    })
}

/// The operating system's error code is Linux's `errno` value on Linux, the platform of
/// FreeSurfer's official programs; elsewhere (macOS numbers several codes differently, Windows
/// has its own) the error's kind gives Linux's value.
#[cfg(target_os = "linux")]
fn raw_errno(e: &std::io::Error) -> Option<i32> {
    e.raw_os_error()
}

#[cfg(not(target_os = "linux"))]
fn raw_errno(_: &std::io::Error) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn capture(f: impl FnOnce(&mut Globals, &mut Vec<u8>), mode: UpstreamDefects) -> Vec<u8> {
        let mut g = Globals::new(mode);
        let mut err = Vec::new();
        f(&mut g, &mut err);
        err
    }

    /// The official mri_info's bytes for a file it cannot open (2026-11-12):
    /// `error: mghRead(/nonexistent.mgz, -1): could not open file\0\n`.
    #[test]
    fn error_printf_and_fs01() {
        let msg = "mghRead(/nonexistent.mgz, -1): could not open file";
        let reproduced = capture(
            |g, e| error_printf(g, e, ERROR_BADPARM, msg),
            UpstreamDefects::Reproduce,
        );
        assert_eq!(reproduced, format!("error: {msg}\0\n").into_bytes());
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        error_printf(&mut g, &mut e, ERROR_BADPARM, msg);
        assert_eq!(e, format!("error: {msg}\n").into_bytes());
        assert_eq!(g.gerror, ERROR_BADPARM);
        assert!(g.notices.contains(&FS_01));
    }

    #[test]
    fn errno_is_printed_first() {
        let out = capture(
            |g, e| {
                g.errno = 2;
                error_printf(g, e, ERROR_NOFILE, "x\n")
            },
            UpstreamDefects::Reproduce,
        );
        assert_eq!(out, b"error: No such file or directory\nerror: x\n\0\n");
        assert_eq!(strerror(4242), "Unknown error 4242");
    }

    #[test]
    fn fatal_exits_or_throws() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let r = error_exit(&mut g, &mut e, ERROR_BADPARM, "bad");
        assert_eq!(r, Err(Fatal::Exit(-4)));
        assert_eq!(Fatal::Exit(-4).status(), 252);
        assert_eq!(Fatal::Exit(-1).status(), 255);
        g.exceptions = true;
        assert_eq!(fatal(&mut g, &mut e, 3, "m"), Err(Fatal::Throw(3)));
        assert!(g.notices.contains(&FS_263));
        assert_eq!(fatal(&mut g, &mut e, 0, "m"), Ok(()));
        g.exceptions = false;
        assert_eq!(fatal(&mut g, &mut e, 0, "m"), Err(Fatal::Exit(0)));
    }

    #[test]
    fn fs263_reproduce_aborts() {
        // the official build (the unit oracle's fs_core_probe, 2026-11-13): `fatal 7 f7 throw`
        // prints `error: f7` and libstdc++'s terminate line, status 134; code 0 returns
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        g.exceptions = true;
        let mut e = Vec::new();
        assert_eq!(fatal(&mut g, &mut e, 7, "f7"), Err(Fatal::Exit(134)));
        assert_eq!(Fatal::Exit(134).status(), 134);
        assert_eq!(
            e,
            b"error: f7\nterminate called after throwing an instance of 'int'\n"
        );
        let mut e = Vec::new();
        g.errno = 2;
        assert_eq!(error_exit(&mut g, &mut e, 3, "bad"), Err(Fatal::Exit(134)));
        assert_eq!(
            e,
            b"error: No such file or directory\nerror: bad\0\nterminate called after throwing an instance of 'int'\n"
        );
        let mut e = Vec::new();
        assert_eq!(fatal(&mut g, &mut e, 0, "f7"), Ok(()));
        assert_eq!(e, b"error: f7\n");
    }

    #[test]
    fn warning_debug_and_the_error_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("err.log");
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        g.error_log = Some(log.clone());
        let mut e = Vec::new();
        warning(&mut e, "w");
        debug(&g, &mut e, "d0");
        g.gdiag |= DIAG_VERBOSE;
        debug(&g, &mut e, "d1");
        error(&g, &mut e, "plain");
        error_printf(&mut g, &mut e, 1, "c");
        assert_eq!(e, b"warning: w\ndebug: d1\nerror: plain\nerror: c\0\n");
        assert_eq!(std::fs::read(&log).unwrap(), b"plain\nc\0\n");
    }
}

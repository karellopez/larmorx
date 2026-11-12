// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! volume_io's file helpers for the `.xfm` reader and writer (`Prog_utils/files.c`,
//! `Prog_utils/string.c`): `~` and `$VAR` expansion, the default suffix, compressed inputs.
//!
//! File names are bytes, as in C (on Unix exactly; elsewhere through UTF-8). volume_io decompresses
//! a compressed input with `gunzip -c` (then `bunzip2 -c`) through the shell into a temporary file;
//! larmorx decompresses gzip and bzip2 data in memory ([`super::defects::SHELL_DECOMPRESSION`]);
//! the older `compress` (`.Z`) and `pack` (`.z`) formats `gunzip` also reads are not decoded (their
//! files fail as an undecodable file does).

use std::io::Read;
use std::path::PathBuf;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self as d, Notices};

use super::Dialect;

/// volume_io's compressed endings, in its order.
const COMPRESSED_ENDINGS: [&[u8]; 3] = [b".z", b".Z", b".gz"];

/// A file name's bytes as a path.
pub(crate) fn to_path(name: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(name))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(name).into_owned())
    }
}

fn os_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        s.to_string_lossy().into_owned().into_bytes()
    }
}

/// C's `getenv`: the first entry of the environment that starts with `name=`; an empty name has
/// none.
pub(crate) fn getenv(name: &[u8]) -> Option<Vec<u8>> {
    if name.is_empty() {
        return None;
    }
    for (k, v) in std::env::vars_os() {
        let mut entry = os_bytes(&k);
        entry.push(b'=');
        entry.extend_from_slice(&os_bytes(&v));
        if entry.len() > name.len() && entry.starts_with(name) && entry[name.len()] == b'=' {
            return Some(entry[name.len() + 1..].to_vec());
        }
    }
    None
}

/// `get_user_home_directory`: `getpwnam(user)->pw_dir`, from the password file (`/etc/passwd`;
/// accounts known only to other name services are not found).
fn user_home_directory(user: &[u8]) -> Option<Vec<u8>> {
    let text = std::fs::read("/etc/passwd").ok()?;
    for line in text.split(|&c| c == b'\n') {
        let fields: Vec<&[u8]> = line.split(|&c| c == b':').collect();
        if fields.len() >= 7 && fields[0] == user {
            return Some(fields[5].to_vec());
        }
    }
    None
}

/// `expand_filename`: a `~` at the start (the home directory, or `~user`'s) and every `$NAME`
/// (the variable's value) unless escaped by a backslash; the name runs to the next `/`, `.` or the
/// end; an unknown name is copied as written; a backslash escapes the next character and is
/// dropped.
pub fn expand_filename(filename: &[u8]) -> Vec<u8> {
    // C's string ends at its first NUL.
    let filename = &filename[..filename.iter().position(|&b| b == 0).unwrap_or(filename.len())];
    let len = filename.len();
    let at = |i: usize| if i < len { filename[i] } else { 0 };
    let mut out = Vec::with_capacity(len);
    let mut prev_was_backslash = false;
    let mut i = 0;
    while i < len {
        let c = filename[i];
        if !prev_was_backslash && ((i == 0 && c == b'~') || c == b'$') {
            let tilde = c == b'~';
            let mut j = i + 1;
            while at(j) != b'/' && at(j) != b'.' && at(j) != 0 {
                j += 1;
            }
            let env = &filename[i + 1..j];
            let value = if tilde {
                if env.is_empty() {
                    getenv(b"HOME")
                } else {
                    user_home_directory(env)
                }
            } else {
                getenv(env)
            };
            match value {
                Some(v) => {
                    let end = v.iter().position(|&b| b == 0).unwrap_or(v.len());
                    out.extend_from_slice(&v[..end]);
                    i = j;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
            prev_was_backslash = false;
        } else {
            if c != b'\\' || prev_was_backslash {
                out.push(c);
                prev_was_backslash = false;
            } else {
                prev_was_backslash = true;
            }
            i += 1;
        }
    }
    out
}

/// `file_exists`: `fopen(expanded, "r")` succeeds.
pub fn file_exists(filename: &[u8]) -> bool {
    std::fs::File::open(to_path(&expand_filename(filename))).is_ok()
}

/// `remove_directories_from_filename`: what follows the expanded name's last `/`.
pub fn remove_directories_from_filename(filename: &[u8]) -> Vec<u8> {
    let e = expand_filename(filename);
    match e.iter().rposition(|&c| c == b'/') {
        Some(p) => e[p + 1..].to_vec(),
        None => e,
    }
}

/// `has_no_extension`: no `.` in the file's own name.
pub(crate) fn has_no_extension(filename: &[u8]) -> bool {
    !remove_directories_from_filename(filename).contains(&b'.')
}

/// `extract_directory`: the expanded name up to and including its last `/`, or `.`.
pub fn extract_directory(filename: &[u8]) -> Vec<u8> {
    let e = expand_filename(filename);
    match e.iter().rposition(|&c| c == b'/') {
        Some(p) => e[..=p].to_vec(),
        None => b".".to_vec(),
    }
}

fn ends_in(s: &[u8], ending: &[u8]) -> bool {
    s.len() >= ending.len() && &s[s.len() - ending.len()..] == ending
}

/// `file_exists_as_compressed`: the first of `name.z`, `name.Z`, `name.gz` that exists.
fn file_exists_as_compressed(expanded: &[u8]) -> Option<Vec<u8>> {
    let e = expand_filename(expanded);
    for ending in COMPRESSED_ENDINGS {
        let mut c = e.clone();
        c.extend_from_slice(ending);
        if file_exists(&c) {
            return Some(c);
        }
    }
    None
}

/// glibc's `strerror` text for an error number (the messages file operations give).
pub(crate) fn strerror(errno: i32) -> String {
    let s = match errno {
        1 => "Operation not permitted",
        2 => "No such file or directory",
        4 => "Interrupted system call",
        5 => "Input/output error",
        6 => "No such device or address",
        9 => "Bad file descriptor",
        11 => "Resource temporarily unavailable",
        12 => "Cannot allocate memory",
        13 => "Permission denied",
        14 => "Bad address",
        16 => "Device or resource busy",
        17 => "File exists",
        19 => "No such device",
        20 => "Not a directory",
        21 => "Is a directory",
        22 => "Invalid argument",
        23 => "Too many open files in system",
        24 => "Too many open files",
        26 => "Text file busy",
        27 => "File too large",
        28 => "No space left on device",
        30 => "Read-only file system",
        36 => "File name too long",
        40 => "Too many levels of symbolic links",
        75 => "Value too large for defined data type",
        95 => "Operation not supported",
        122 => "Disk quota exceeded",
        _ => return format!("Unknown error {errno}"),
    };
    s.to_string()
}

/// The C error number of an I/O error (Unix: the error itself; elsewhere: by its kind).
pub(crate) fn errno_of(e: &std::io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(n) = e.raw_os_error() {
        return n;
    }
    use std::io::ErrorKind as K;
    match e.kind() {
        K::NotFound => 2,
        K::PermissionDenied => 13,
        K::AlreadyExists => 17,
        K::InvalidInput => 22,
        K::IsADirectory => 21,
        K::NotADirectory => 20,
        K::ReadOnlyFilesystem => 30,
        _ => 5,
    }
}

/// `print_system_error`.
pub(crate) fn print_system_error(e: &std::io::Error, dialect: &dyn Dialect, streams: &mut dyn Streams) {
    dialect.print_error(&format!("\nSystem message: {}\n", strerror(errno_of(e))), streams);
}

/// The characters a POSIX shell splits a word at or interprets (in `gunzip -c <name> > <tmp>`).
const SHELL_SPECIAL: &[u8] = b" \t\n\"'`$\\;&|<>()*?[]#~={}!^%";

/// What [`open_file`] opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoType {
    Read,
    Write,
    Append,
}

/// An open file: the bytes of a file opened for reading (read whole: what C's stream gives before
/// an error or the end), or a file opened for writing.
#[derive(Debug)]
pub enum OpenFile {
    Read(Vec<u8>),
    Write(std::fs::File),
}

pub(crate) fn lossy(b: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(b)
}

/// Decompresses gzip (members one after another) and bzip2 data as `gunzip -c`, then
/// `bunzip2 -c`, would; `None` where both fail. Bytes after gzip data make gzip exit with its
/// warning status ("trailing garbage ignored"), which volume_io counts as a failure.
fn decompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        return gunzip(data);
    }
    if data.len() >= 3 && &data[..3] == b"BZh" {
        let mut out = Vec::new();
        let mut d = bzip2::read::MultiBzDecoder::new(data);
        return d.read_to_end(&mut out).ok().map(|_| out);
    }
    None
}

fn gunzip(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    while !data.is_empty() {
        if data.len() < 2 || data[0] != 0x1f || data[1] != 0x8b {
            return None;
        }
        let mut d = flate2::bufread::GzDecoder::new(data);
        d.read_to_end(&mut out).ok()?;
        data = d.into_inner();
    }
    Some(out)
}

/// `open_file`: `~` and `$` expanded; for reading, a name ending in `.z`, `.Z` or `.gz`, or a
/// missing name whose compressed form exists, is decompressed. Failures print volume_io's
/// messages.
pub fn open_file(
    filename: &[u8],
    io: IoType,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<OpenFile, ()> {
    let mut expanded = expand_filename(filename);
    let mut gzipped = false;
    if io == IoType::Read {
        gzipped = COMPRESSED_ENDINGS.iter().any(|e| ends_in(&expanded, e));
        if !gzipped && !file_exists(&expanded) {
            if let Some(c) = file_exists_as_compressed(&expanded) {
                expanded = c;
                gzipped = true;
            }
        }
    }
    if gzipped && expanded.iter().any(|c| SHELL_SPECIAL.contains(c)) {
        notices.add(
            &d::FS_71,
            format!(
                "{} decompressed in memory (volume_io passes the name to the shell unquoted)",
                lossy(&expanded)
            ),
        );
    }
    let path = to_path(&expanded);
    match io {
        IoType::Read => {
            let mut file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) if !gzipped => {
                    dialect.print_error(
                        &format!("Error:  could not open file \"{}\".  ", lossy(&expanded)),
                        streams,
                    );
                    print_system_error(&e, dialect, streams);
                    return Err(());
                }
                Err(_) => {
                    uncompress_failed(&expanded, dialect, streams);
                    return Err(());
                }
            };
            let mut data = Vec::new();
            // A read error ends the stream where it happens, as fgetc's end of file would.
            let _ = file.read_to_end(&mut data);
            if !gzipped {
                return Ok(OpenFile::Read(data));
            }
            match decompress(&data) {
                Some(d) => Ok(OpenFile::Read(d)),
                None => {
                    uncompress_failed(&expanded, dialect, streams);
                    Err(())
                }
            }
        }
        IoType::Write | IoType::Append => {
            let mut o = std::fs::OpenOptions::new();
            if io == IoType::Write {
                o.write(true).create(true).truncate(true);
            } else {
                o.append(true).create(true);
            }
            match o.open(&path) {
                Ok(f) => Ok(OpenFile::Write(f)),
                Err(e) => {
                    dialect.print_error(
                        &format!("Error:  could not open file \"{}\".  ", lossy(&expanded)),
                        streams,
                    );
                    print_system_error(&e, dialect, streams);
                    Err(())
                }
            }
        }
    }
}

fn uncompress_failed(expanded: &[u8], dialect: &dyn Dialect, streams: &mut dyn Streams) {
    dialect.print_error(
        &format!(
            "Error uncompressing {} into {} using gunzip and bunzip2\n",
            lossy(expanded),
            lossy(&dialect.temporary_name())
        ),
        streams,
    );
}

/// `open_file_with_default_suffix`: for reading, a name without an extension that does not exist
/// is tried with `.<suffix>` added; for writing, a name without an extension gets it.
pub fn open_file_with_default_suffix(
    filename: &[u8],
    default_suffix: &[u8],
    io: IoType,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<OpenFile, ()> {
    let expanded = expand_filename(filename);
    let with_suffix = || {
        let mut u = expanded.clone();
        u.push(b'.');
        u.extend_from_slice(default_suffix);
        u
    };
    let used = if io == IoType::Read {
        let mut used = expanded.clone();
        if !file_exists(&expanded) && has_no_extension(&expanded) {
            let u = with_suffix();
            if file_exists(&u) {
                used = u;
            }
        }
        used
    } else if has_no_extension(&expanded) {
        with_suffix()
    } else {
        expanded.clone()
    };
    open_file(&used, io, dialect, streams, notices)
}

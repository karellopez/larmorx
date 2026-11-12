// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! The `.xfm` reader: volume_io's `MNI_formats/mni_io.c` (the tokenizer) and `gen_xf_io.c`
//! (`input_transform_file`, `input_transform`, `input_one_transform`), over C's stream semantics
//! (`fgetc`, and `ungetc` that may push back several characters, as glibc's does).
//!
//! The tokenizer, as volume_io has it: white space is a space, a tab, a newline or a carriage
//! return; `%` and `#` start a comment that runs to the end of the line, but only where white
//! space is skipped (before a token); a token runs to one of its terminators (`=`, `;` or a space,
//! depending on what is read) or a newline, which is pushed back; a carriage return inside a token
//! is dropped, a NUL byte too; trailing spaces (not tabs) are removed; a token that starts with `"`
//! runs to the next `"` or newline. A number is a token ended by a space, `;` or a newline (not a
//! tab: `1\t2` is one token, read as 1), converted with `sscanf("%lf")`.

use larmorx_core::cfmt::Streams;
use larmorx_core::cscan;
use larmorx_core::defects::{self as d, Notices};

use super::files::{Ctx, IoType, OpenFile, lossy, open_file_with_default_suffix};
use super::{Dialect, GeneralTransform, Status, TransformData, TransformType};

const TRANSFORM_FILE_HEADER: &[u8] = b"MNI Transform File";
const TYPE_STRING: &[u8] = b"Transform_Type";
const LINEAR_TRANSFORM_STRING: &str = "Linear_Transform";
const INVERT_FLAG_STRING: &[u8] = b"Invert_Flag";
const N_DIMENSIONS_STRING: &str = "Number_Dimensions";
const POINTS_STRING: &[u8] = b"Points";
const DISPLACEMENTS_STRING: &[u8] = b"Displacements";
const DISPLACEMENT_VOLUME: &str = "Displacement_Volume";

/// A C stream over a file's bytes: `fgetc` and `ungetc`.
pub(crate) struct CStream<'a> {
    data: &'a [u8],
    pos: usize,
    /// The characters pushed back, the next one last.
    back: Vec<u8>,
}

impl<'a> CStream<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        CStream {
            data,
            pos: 0,
            back: Vec::new(),
        }
    }

    /// `input_character`: the next byte, or `None` (volume_io's `ERROR`) at the end.
    fn getc(&mut self) -> Option<u8> {
        if let Some(c) = self.back.pop() {
            return Some(c);
        }
        let c = self.data.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// `unget_character(file, ch)` with the C variable `ch` (`None`: never set in this call, so
    /// whatever is pushed back is never read: every path that gets here stops reading). `char`
    /// is signed on x86-64, so the byte 0xFF is `EOF` to `ungetc`, which then pushes nothing.
    fn ungetc(&mut self, ch: Option<u8>) {
        if let Some(c) = ch {
            if c != 0xFF {
                self.back.push(c);
            }
        }
    }
}

/// `mni_get_nonwhite_character`.
fn get_nonwhite(s: &mut CStream, ch: &mut Option<u8>) -> Status {
    let mut in_comment = false;
    loop {
        match s.getc() {
            None => return Status::EndOfFile,
            Some(c) => {
                *ch = Some(c);
                if c == b'%' || c == b'#' {
                    in_comment = true;
                } else if c == b'\n' {
                    in_comment = false;
                }
                if !(in_comment || c == b' ' || c == b'\t' || c == b'\n' || c == b'\r') {
                    return Status::Ok;
                }
            }
        }
    }
}

fn show(c: u8) -> String {
    lossy(&[c]).into_owned()
}

/// `mni_skip_expected_character`.
fn skip_expected(
    s: &mut CStream,
    expected: u8,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
) -> Status {
    let mut ch = None;
    let mut status = get_nonwhite(s, &mut ch);
    if status == Status::Ok {
        let c = ch.expect("a character");
        if c != expected {
            dialect.print_error(
                &format!("Expected '{}', found '{}'.\n", show(expected), show(c)),
                streams,
            );
            status = Status::Error;
        }
    } else {
        dialect.print_error(
            &format!("Expected '{}', found end of file.\n", show(expected)),
            streams,
        );
    }
    status
}

/// `mni_input_string`: the status and the token (`None` unless the status is OK).
fn input_string(
    s: &mut CStream,
    mut t1: u8,
    mut t2: u8,
    dialect: &dyn Dialect,
) -> (Status, Option<Vec<u8>>) {
    let nul_ends = dialect.nul_ends_token();
    let mut ended = false;
    let mut out = Vec::new();
    let mut ch = None;
    let mut status = get_nonwhite(s, &mut ch);
    let quoted = status == Status::Ok && ch == Some(b'"');
    if quoted {
        status = get_nonwhite(s, &mut ch);
        t1 = b'"';
        t2 = b'"';
    }
    while status == Status::Ok {
        let c = ch.expect("a character");
        if c == t1 || c == t2 || c == b'\n' {
            break;
        }
        // A carriage return is dropped; a NUL appended to a C string adds nothing (or, in a
        // dialect that keeps one buffer, ends the string there).
        if c == 0 && nul_ends {
            ended = true;
        } else if c != b'\r' && c != 0 && !ended {
            out.push(c);
        }
        match s.getc() {
            Some(c) => ch = Some(c),
            None => status = Status::Error,
        }
    }
    if !quoted {
        s.ungetc(ch);
    }
    while out.last() == Some(&b' ') {
        out.pop();
    }
    if status != Status::Ok {
        (status, None)
    } else {
        (status, Some(out))
    }
}

/// `mni_input_keyword_and_equal_sign`.
fn keyword_and_equal(
    s: &mut CStream,
    keyword: &[u8],
    print_error_message: bool,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
) -> Status {
    let (status, str) = input_string(s, b'=', 0, dialect);
    if status == Status::EndOfFile {
        return status;
    }
    let matches = status == Status::Ok && str.as_deref().unwrap_or(b"") == keyword;
    if !matches || skip_expected(s, b'=', dialect, streams) != Status::Ok {
        dialect.keyword_mismatch(keyword, str.as_deref(), print_error_message, streams);
        return Status::Error;
    }
    status
}

/// `unget_string`: pushes back the token's first character that is not a space or a tab.
fn unget_string(s: &mut CStream, token: &[u8]) {
    if let Some(&c) = token.iter().find(|&&c| c != b' ' && c != b'\t') {
        s.ungetc(Some(c));
    }
}

/// `mni_input_real`.
fn input_real(s: &mut CStream, dialect: &dyn Dialect, streams: &mut dyn Streams) -> (Status, f64) {
    let (mut status, str) = input_string(s, b' ', b';', dialect);
    let mut value = 0.0;
    if status == Status::Ok {
        let token = str.expect("a token");
        match cscan::sscanf_f64(&token) {
            Some(v) => value = v,
            None => {
                dialect.bad_real(&token, streams);
                unget_string(s, &token);
                status = Status::Error;
            }
        }
    }
    (status, value)
}

/// `mni_input_int`.
fn input_int(s: &mut CStream, dialect: &dyn Dialect) -> (Status, i32) {
    let (mut status, str) = input_string(s, b' ', b';', dialect);
    let mut value = 0;
    if status == Status::Ok {
        let token = str.expect("a token");
        match cscan::sscanf_i32(&token) {
            Some(v) => value = v,
            None => {
                unget_string(s, &token);
                status = Status::Error;
            }
        }
    }
    (status, value)
}

/// `mni_input_reals`: numbers up to the `;` (which is read). volume_io loops forever when the
/// list meets the end of the file after white space (FS-72); larmorx stops there and reports the
/// missing `;`.
fn input_reals(
    s: &mut CStream,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> (Status, Vec<f64>) {
    let mut reals = Vec::new();
    // volume_io appends the last value read for each end of file met (the loop stops only at
    // ERROR); `last` is it.
    let mut last = 0.0;
    loop {
        let (st, v) = input_real(s, dialect, streams);
        match st {
            Status::Error => break,
            Status::EndOfFile => {
                // What was pushed back is read again by the next number: white space or a
                // comment character (or nothing) gives the end of the file again, forever.
                let again = match s.back.last() {
                    None => true,
                    Some(c) => b" \t\n\r%#".contains(c),
                };
                if again {
                    notices.add(
                        &d::FS_72,
                        "a number list reached the end of the file without its ';' (volume_io reads forever)",
                    );
                    break;
                }
                reals.push(last);
            }
            _ => {
                last = v;
                reals.push(v);
            }
        }
    }
    (skip_expected(s, b';', dialect, streams), reals)
}

/// `input_one_transform`: the status (`EndOfFile` when no further transform starts) and the
/// transform.
fn input_one_transform(
    s: &mut CStream,
    filename: Option<&[u8]>,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> (Status, Option<GeneralTransform>) {
    let err = (Status::Error, None);
    let mut inverse_flag = false;
    let status = keyword_and_equal(s, TYPE_STRING, false, dialect, streams);
    if status != Status::Ok {
        return (status, None);
    }
    let (st, type_name) = input_string(s, b';', 0, dialect);
    if st != Status::Ok {
        dialect.print_error("input_transform(): missing transform type.\n", streams);
        return err;
    }
    if skip_expected(s, b';', dialect, streams) != Status::Ok {
        return err;
    }
    let Some(kind) = dialect.transform_type(&type_name.expect("a token"), streams) else {
        return err;
    };
    let (st, mut str) = input_string(s, b'=', 0, dialect);
    if st != Status::Ok {
        return err;
    }
    if str.as_deref() == Some(INVERT_FLAG_STRING) {
        if skip_expected(s, b'=', dialect, streams) != Status::Ok {
            return err;
        }
        let (st, flag) = input_string(s, b';', 0, dialect);
        if st != Status::Ok {
            return err;
        }
        if skip_expected(s, b';', dialect, streams) != Status::Ok {
            return err;
        }
        match flag.as_deref() {
            Some(b"True") => inverse_flag = true,
            Some(b"False") => inverse_flag = false,
            _ => {
                dialect.print_error("Expected True or False after Invert_Flag =\n", streams);
                return err;
            }
        }
        let (st, next) = input_string(s, b'=', 0, dialect);
        if st != Status::Ok {
            return err;
        }
        str = next;
    }
    let str = str.expect("a token");
    let expected = |what: &str, streams: &mut dyn Streams| {
        dialect.print_error(&format!("Expected {what} =\n"), streams);
    };
    let transform = match kind {
        TransformType::Linear => {
            if str != LINEAR_TRANSFORM_STRING.as_bytes() {
                expected(LINEAR_TRANSFORM_STRING, streams);
                return err;
            }
            if skip_expected(s, b'=', dialect, streams) != Status::Ok {
                return err;
            }
            let mut t = super::make_identity_transform();
            for i in 0..3 {
                for j in 0..4 {
                    let (st, v) = input_real(s, dialect, streams);
                    if st != Status::Ok {
                        dialect.print_error(
                            &format!(
                                "input_transform(): error reading transform elem [{},{}]\n",
                                i + 1,
                                j + 1
                            ),
                            streams,
                        );
                        return err;
                    }
                    t[i][j] = v;
                }
            }
            if skip_expected(s, b';', dialect, streams) != Status::Ok {
                return err;
            }
            GeneralTransform::linear(Some(&t), dialect, streams)
        }
        TransformType::ThinPlateSpline => {
            if str != N_DIMENSIONS_STRING.as_bytes() {
                expected(N_DIMENSIONS_STRING, streams);
                return err;
            }
            if skip_expected(s, b'=', dialect, streams) != Status::Ok {
                return err;
            }
            let (st, n_dimensions) = input_int(s, dialect);
            if st != Status::Ok {
                return err;
            }
            if skip_expected(s, b';', dialect, streams) != Status::Ok {
                return err;
            }
            if keyword_and_equal(s, POINTS_STRING, true, dialect, streams) != Status::Ok {
                return err;
            }
            let (st, points_1d) = input_reals(s, dialect, streams, notices);
            if st != Status::Ok {
                return err;
            }
            if n_dimensions <= 0 {
                // volume_io divides by it (FS-73): a crash at 0, negative sizes below.
                notices.add(
                    &d::FS_73,
                    format!("Number_Dimensions = {n_dimensions} (volume_io crashes)"),
                );
                dialect.print_error(
                    &format!(
                        "input_transform(): Number_Dimensions must be positive (it is {n_dimensions}).\n"
                    ),
                    streams,
                );
                return err;
            }
            let nd = n_dimensions as usize;
            let n_total = points_1d.len() as i64;
            if n_total % n_dimensions as i64 != 0 {
                dialect.print_error(
                    &format!(
                        "Number of points ({}) must be multiple of number of dimensions ({})\n",
                        n_total as i32, n_dimensions
                    ),
                    streams,
                );
                return err;
            }
            let n_points = points_1d.len() / nd;
            if n_points == 0 {
                notices.add(
                    &d::FS_74,
                    "an empty Points list (volume_io frees a pointer it never set)",
                );
            }
            let points: Vec<Vec<f64>> = points_1d.chunks(nd).map(<[f64]>::to_vec).collect();
            if keyword_and_equal(s, DISPLACEMENTS_STRING, true, dialect, streams) != Status::Ok {
                return err;
            }
            let mut displacements = vec![vec![0.0; nd]; n_points + nd + 1];
            for row in displacements.iter_mut() {
                for v in row.iter_mut() {
                    let (st, d) = input_real(s, dialect, streams);
                    if st != Status::Ok {
                        dialect.print_error("Expected more displacements.\n", streams);
                        return err;
                    }
                    *v = d;
                }
            }
            if skip_expected(s, b';', dialect, streams) != Status::Ok {
                return err;
            }
            GeneralTransform {
                data: TransformData::ThinPlateSpline {
                    n_dimensions: nd,
                    points,
                    displacements,
                },
                inverse_flag: false,
            }
        }
        TransformType::Concatenated => {
            // Not a file type (a dialect that names it reads nothing).
            dialect.print_error("input_transform(): invalid transform type.\n", streams);
            return err;
        }
        TransformType::Grid => {
            if str != DISPLACEMENT_VOLUME.as_bytes() {
                expected(DISPLACEMENT_VOLUME, streams);
                return err;
            }
            if skip_expected(s, b'=', dialect, streams) != Status::Ok {
                return err;
            }
            let (st, volume_filename) = input_string(s, b';', 0, dialect);
            if st != Status::Ok {
                return err;
            }
            if skip_expected(s, b';', dialect, streams) != Status::Ok {
                return err;
            }
            let mut volume_filename = volume_filename.expect("a token");
            if volume_filename.first() != Some(&b'/') {
                if let Some(f) = filename {
                    let directory = Ctx { dialect, notices: &mut *notices }.extract_directory(f);
                    if !directory.is_empty() {
                        let mut t = directory;
                        t.push(b'/');
                        t.extend_from_slice(&volume_filename);
                        volume_filename = t;
                    }
                }
            }
            let Ok(volume) = dialect.read_grid_volume(&volume_filename, streams) else {
                return err;
            };
            GeneralTransform {
                data: TransformData::Grid {
                    volume_filename,
                    volume,
                },
                inverse_flag: false,
            }
        }
    };
    let transform = if inverse_flag {
        transform.inverse()
    } else {
        transform
    };
    (Status::Ok, Some(transform))
}

/// `input_transform`: the header, then every transform in the file, concatenated in order.
/// `filename` gives the directory a grid transform's relative volume name is taken from.
pub fn input_transform_from_bytes(
    data: &[u8],
    filename: Option<&[u8]>,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<GeneralTransform, ()> {
    let mut s = CStream::new(data);
    input_transform(&mut s, filename, dialect, streams, notices)
}

/// `input_transform` over a stream (see [`input_transform_from_bytes`]).
pub(crate) fn input_transform(
    s: &mut CStream,
    filename: Option<&[u8]>,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<GeneralTransform, ()> {
    let (st, line) = input_string(s, 0, 0, dialect);
    if st != Status::Ok {
        dialect.print_error("input_transform(): could not read header in file.\n", streams);
        return Err(());
    }
    if line.as_deref() != Some(TRANSFORM_FILE_HEADER) {
        dialect.print_error("input_transform(): invalid header in file.\n", streams);
        return Err(());
    }
    let mut transform: Option<GeneralTransform> = None;
    let status = loop {
        let (st, next) = input_one_transform(s, filename, dialect, streams, notices);
        if st != Status::Ok {
            break st;
        }
        let next = next.expect("a transform");
        transform = Some(match transform {
            None => next,
            Some(t) => GeneralTransform::concat(&t, &next),
        });
    };
    if status == Status::Error {
        dialect.print_error("input_transform: error reading transform.\n", streams);
        return Err(());
    }
    match transform {
        None => {
            dialect.print_error("input_transform: no transform present.\n", streams);
            Err(())
        }
        Some(t) => Ok(t),
    }
}

/// `input_transform_file`: opens `filename` (`~` and `$` expanded, `.xfm` added when the name
/// without it does not exist, a compressed file decompressed) and reads it.
pub fn input_transform_file(
    filename: &[u8],
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<GeneralTransform, ()> {
    let opened =
        open_file_with_default_suffix(filename, b"xfm", IoType::Read, dialect, streams, notices)?;
    let OpenFile::Read(data) = opened else {
        unreachable!("opened for reading")
    };
    input_transform_from_bytes(&data, Some(filename), dialect, streams, notices)
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/mri_read.c (my_fgets, decode_linebuf,
// iznogood_1D, mri_read_ascii, mri_read_1D), src/cs_fgets.c (afni_fgets),
// src/mri_fromstring.c (mri_1D_fromstring) and src/niml/niml_header.c
// (NI_decode_string_list).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version. cs_fgets.c, mri_fromstring.c and
// the NIML library were written at the NIH and are in the public domain.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! AFNI's "1D" text files, as `mri_read_1D` reads them for `3dTshift -tpattern @file`.
//!
//! A file is read as a matrix, one row per line, then transposed, so its values come column by
//! column. `1D: ...` gives the values inline. The rules are AFNI's, including its quirks:
//!
//! - lines end in LF, CR, CR+LF or LF+CR; blank lines and lines starting with `#` or `//` are
//!   skipped; a line ending in `\` continues on the next;
//! - the first line sets the number of columns; later lines lose extra values and are padded
//!   with zeros; reading stops at the first line that gives no value;
//! - commas and colons are blanks (unless inside a run of letters);
//! - `N@v` stands for `N` copies of `v`, and `*` (or a word) for 0;
//! - a value that starts with any other character ends the file with an error, except `#`,
//!   which ends the line;
//! - numbers are read with `strtod` and stored as float, or, on lines that contain `*`, `@` or
//!   a letter (including the `e` of an exponent), with `sscanf("%f")`, which rounds directly to
//!   float.
//!
//! Not supported (an error): `.tsv` and `.csv` tables, standard input, `jRandom1D:` and `3D:`
//! inputs, and the `[...]`/`{...}` selectors.

use std::path::Path;

use crate::cnum::{is_space, scan_int, scanf_f32, strtod};

/// A matrix AFNI read from 1D text: `nx` rows (values per column) and `ny` columns, stored
/// column by column (`data[x + y*nx]`, an `MRI_IMAGE` of floats).
#[derive(Clone, Debug, PartialEq)]
pub struct OneD {
    pub nx: usize,
    pub ny: usize,
    pub data: Vec<f32>,
}

impl OneD {
    /// `mri_transpose`.
    fn transpose(&self) -> OneD {
        let mut data = vec![0.0; self.data.len()];
        for y in 0..self.ny {
            for x in 0..self.nx {
                data[y + x * self.ny] = self.data[x + y * self.nx];
            }
        }
        OneD {
            nx: self.ny,
            ny: self.nx,
            data,
        }
    }
}

/// Why a 1D text could not be read. The messages are AFNI's where it prints one.
#[derive(Debug, thiserror::Error)]
pub enum OneDError {
    #[error("mri_read_ascii: couldn't open file {0}")]
    Open(String),
    #[error("mri_read_ascii: can't read any valid data from file {0}")]
    NoData(String),
    #[error("Failed parsing data row {row} of 1D file '{file}'")]
    Text { row: usize, file: String },
    #[error("Illegal 1D: value '{0}'")]
    IllegalValue(String),
    #[error("1D: string is empty")]
    EmptyString,
    #[error("mri_read_1D: can't read from string '{0}'")]
    String(String),
    #[error("{0}: not supported by larmorx-gpl (AFNI reads it)")]
    Unsupported(String),
}

/// `mri_read_1D(fname)` for files and `1D:` strings.
pub fn read_1d(fname: &str) -> Result<OneD, OneDError> {
    let lower = fname.to_ascii_lowercase();
    let table = |ext: &str| {
        lower
            .find(ext)
            .is_some_and(|i| matches!(lower.as_bytes().get(i + 4), None | Some(b'[')))
    };
    let n = fname.len();
    if table(".tsv")
        || table(".csv")
        || (n <= 2 && fname.starts_with('-'))
        || (n <= 6 && fname.starts_with("stdin"))
        || (n <= 9 && (fname.starts_with("1D:stdin") || fname.starts_with("/dev/fd0")))
        || lower.starts_with("jrandom1d:")
        || (n > 9 && fname.starts_with("3D") && fname[2..].find(':').is_some_and(|i| i <= 1))
    {
        return Err(OneDError::Unsupported(fname.to_owned()));
    }
    let (dname, flip) = match fname.strip_suffix('\'') {
        Some(d) => (d, true),
        None => (fname, false),
    };
    if let Some(s) = dname.strip_prefix("1D:") {
        let im = from_string(s).map_err(|e| match e {
            OneDError::IllegalValue(_) | OneDError::EmptyString => e,
            _ => OneDError::String(dname.to_owned()),
        })?;
        return Ok(if flip { im.transpose() } else { im });
    }
    if fname.contains('[') || fname.contains('{') {
        return Err(OneDError::Unsupported(format!(
            "{fname}: sub-vector selectors"
        )));
    }
    let im = read_ascii(dname)?.transpose();
    Ok(if flip { im.transpose() } else { im })
}

/// `mri_1D_fromstring(str)`: values separated by commas or blanks, `N@v` (or `NxV`, `N*v`)
/// for `N` copies, `N%v%step` for an arithmetic sequence, and `|` or `\` between columns.
pub fn from_string(s: &str) -> Result<OneD, OneDError> {
    let tokens = decode_string_list(s, b",");
    if tokens.is_empty() {
        return Err(OneDError::EmptyString);
    }
    let mut far: Vec<f32> = Vec::new();
    let mut col_len = vec![0usize];
    for tok in &tokens {
        let illegal = || OneDError::IllegalValue(tok.clone());
        let (count, value, step) = if tok.contains(['@', 'x', 'X', '*']) {
            // sscanf( str , "%d%c%f" , &count , &sep , &value ) must match all three.
            let (count, used) = scan_int(tok).ok_or_else(illegal)?;
            let rest = tok[used..].get(1..).ok_or_else(illegal)?;
            let value = scanf_f32(rest).ok_or_else(illegal)?;
            if count < 1 {
                return Err(illegal());
            }
            (count as usize, value, 0.0f32)
        } else if tok.contains('%') {
            // sscanf( str , "%d%c%f%c%f" , &count , &sep , &value , &sepx , &cstep ) == 5
            let (count, used) = scan_int(tok).ok_or_else(illegal)?;
            let rest = tok[used..].get(1..).ok_or_else(illegal)?;
            let (value, len) = scanf_prefix(rest).ok_or_else(illegal)?;
            let rest = rest[len..].get(1..).ok_or_else(illegal)?;
            let step = scanf_f32(rest).ok_or_else(illegal)?;
            if count < 1 {
                return Err(illegal());
            }
            (count as usize, value, step)
        } else if tok == "\\" || tok == "|" {
            col_len.push(0);
            continue;
        } else {
            (1, scanf_f32(tok).ok_or_else(illegal)?, 0.0)
        };
        // far[nnn+ntot] = value + nnn*cstep (float)
        far.extend((0..count).map(|k| value + k as f32 * step));
        *col_len.last_mut().expect("one column at least") += count;
    }
    if col_len.len() == 1 {
        return Ok(OneD {
            nx: far.len(),
            ny: 1,
            data: far,
        });
    }
    let nnn = col_len.iter().copied().max().unwrap_or(0);
    let mut data = vec![0.0f32; nnn * col_len.len()];
    let mut kk = 0;
    for (jj, &len) in col_len.iter().enumerate() {
        data[jj * nnn..jj * nnn + len].copy_from_slice(&far[kk..kk + len]);
        kk += len;
    }
    Ok(OneD {
        nx: nnn,
        ny: col_len.len(),
        data,
    })
}

/// `sscanf("%f")` that also reports how much text it read.
fn scanf_prefix(s: &str) -> Option<(f32, usize)> {
    let v = scanf_f32(s)?;
    Some((v, strtod(s).1))
}

/// `NI_decode_string_list(ss, sep)`: substrings separated by a character of `sep` or by white
/// space; empty substrings are skipped.
fn decode_string_list(ss: &str, sep: &[u8]) -> Vec<String> {
    let b = ss.as_bytes();
    let mut out = Vec::new();
    let mut id = 0;
    while id < b.len() {
        while id < b.len() && is_space(b[id]) {
            id += 1;
        }
        if id == b.len() {
            break;
        }
        let jd = id;
        while id < b.len() && !sep.contains(&b[id]) && !is_space(b[id]) {
            id += 1;
        }
        if id == jd {
            id += 1;
            continue;
        }
        out.push(String::from_utf8_lossy(&b[jd..id]).into_owned());
        id += 1;
    }
    out
}

/// `afni_fgets`: the lines of a text, each ending at LF, CR, CR+LF or LF+CR (the line end is
/// dropped).
fn afni_lines(text: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if c == b'\n' || c == b'\r' {
            lines.push(&text[start..i]);
            let other = if c == b'\n' { b'\r' } else { b'\n' };
            i += 1;
            if i < text.len() && text[i] == other {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `my_fgets`: the logical lines, without leading and trailing white space, skipping blank
/// lines and comments (`#`, `//`), joining lines that end in `\`.
fn logical_lines(text: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut lines = afni_lines(text).into_iter();
    loop {
        let mut buf: Vec<u8> = Vec::new();
        let mut cflag = false;
        let mut got = false;
        for raw in lines.by_ref() {
            let mut p = raw;
            while let [c, rest @ ..] = p {
                if is_space(*c) {
                    p = rest;
                } else {
                    break;
                }
            }
            if p.is_empty() {
                if cflag {
                    break;
                }
                continue;
            }
            // Trailing white space (the first character stays, as in AFNI's loop).
            let mut ll = p.len();
            while ll > 1 && is_space(p[ll - 1]) {
                ll -= 1;
            }
            let p = &p[..ll];
            if p[0] == b'#' || p.starts_with(b"//") {
                continue;
            }
            cflag = p[p.len() - 1] == b'\\';
            buf.extend_from_slice(p);
            if cflag {
                let last = buf.len() - 1;
                buf[last] = b' ';
            }
            got = true;
            if !cflag {
                break;
            }
        }
        if !got {
            return out;
        }
        out.push(buf);
    }
}

/// C's `isalpha` in the "C" locale.
fn is_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

/// `ISCOMPLEXi(b, t)`: an `i` right after a digit.
fn is_complex_i(b: &[u8], t: usize) -> bool {
    b[t] == b'i' && t != 0 && b[t - 1].is_ascii_digit()
}

/// `iznogood_1D(cv, t)`: whether `cv[t]` cannot start a value.
fn iznogood(cv: &[u8], t: usize) -> bool {
    let c = cv[t];
    !c.is_ascii_digit()
        && c != b'+'
        && c != b'-'
        && c != b'.'
        && c != b'e'
        && !is_complex_i(cv, t)
        && c != b','
        && c != b'@'
        && c != b'*'
}

/// The result of `decode_linebuf`: the values, or `Died` when a value starts with a character
/// that cannot start one (AFNI's `linebufdied`).
enum Line {
    Values(Vec<f32>),
    Died,
}

/// `decode_linebuf(buf)` with `lbfill = 0` and `oktext = 0` (`AFNI_1D_ZERO_TEXT` unset).
fn decode_linebuf(line: &[u8]) -> Line {
    let mut buf = line.to_vec();
    let blen = buf.len();
    let mut slowmo = false;
    // Commas, colons and complex 'i's become blanks, unless inside a run of letters.
    let mut ii = 0;
    while ii < blen {
        let temppos = ii;
        let mut incr = 0usize;
        if is_alpha(buf[ii]) {
            let mut jj = ii;
            while jj < blen && is_alpha(buf[jj]) {
                jj += 1;
            }
            incr = jj - ii - 1;
            if incr > 0 {
                ii = jj;
            }
        }
        if incr == 0
            && (buf[temppos] == b',' || is_complex_i(&buf, temppos) || buf[temppos] == b':')
        {
            buf[temppos] = b' ';
        }
        if !slowmo && (buf[temppos] == b'*' || buf[temppos] == b'@' || is_alpha(buf[temppos])) {
            slowmo = true;
        }
        ii += 1;
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let b = text.as_bytes();
    let mut values = Vec::new();
    let mut bpos = 0;
    while bpos < b.len() {
        while bpos < b.len() && is_space(b[bpos]) {
            bpos += 1;
        }
        if bpos == b.len() {
            break;
        }
        let mut count = 1usize;
        let val: f32;
        let incr: usize;
        if slowmo {
            // sscanf( buf+bpos , "%63s" , vbuf )
            let mut end = bpos;
            while end < b.len() && !is_space(b[end]) && end - bpos < 63 {
                end += 1;
            }
            let vbuf = &text[bpos..end];
            if iznogood(b, bpos) {
                if !vbuf.starts_with('#') {
                    return Line::Died;
                }
                break;
            }
            let v0 = vbuf.as_bytes()[0];
            if v0 == b'*' || is_alpha(v0) {
                val = 0.0;
            } else if let Some(at) = vbuf.find('@') {
                // sscanf( vbuf , "%d%c%f" , &count , &sep , &val )
                let mut v = 0.0f32;
                if let Some((c, used)) = scan_int(vbuf) {
                    count = c.max(1) as usize;
                    if let Some(rest) = vbuf.get(used + 1..) {
                        v = scanf_f32(rest).unwrap_or(0.0);
                    }
                }
                if vbuf.as_bytes().get(at + 1) == Some(&b'*') {
                    v = 0.0;
                }
                val = v;
            } else {
                val = scanf_f32(vbuf).unwrap_or(0.0);
            }
            incr = vbuf.len();
        } else {
            let (v, used) = strtod(&text[bpos..]);
            val = v as f32;
            incr = used;
        }
        if incr == 0 {
            break;
        }
        values.extend(std::iter::repeat_n(val, count));
        bpos += incr;
    }
    Line::Values(values)
}

/// `mri_read_ascii(fname)`: the matrix of a text file, `ncol` values per row (from the first
/// line), as an image with `nx = ncol` and `ny` = the number of rows.
fn read_ascii(fname: &str) -> Result<OneD, OneDError> {
    if let Some(s) = fname.strip_prefix("1D:") {
        return Ok(from_string(s)?.transpose());
    }
    let text = std::fs::read(Path::new(fname)).map_err(|_| OneDError::Open(fname.to_owned()))?;
    let lines = logical_lines(&text);
    let Some(first) = lines.first() else {
        return Err(OneDError::NoData(fname.to_owned()));
    };
    let ncol = match decode_linebuf(first) {
        Line::Values(v) if !v.is_empty() => v.len(),
        Line::Died => {
            return Err(OneDError::Text {
                row: 0,
                file: fname.to_owned(),
            });
        }
        Line::Values(_) => return Err(OneDError::NoData(fname.to_owned())),
    };
    let mut data = Vec::new();
    let mut nrow = 0;
    for line in &lines {
        match decode_linebuf(line) {
            Line::Values(v) if !v.is_empty() => {
                data.extend((0..ncol).map(|i| v.get(i).copied().unwrap_or(0.0)));
                nrow += 1;
            }
            Line::Values(_) => break,
            Line::Died => {
                return Err(OneDError::Text {
                    row: nrow,
                    file: fname.to_owned(),
                });
            }
        }
    }
    Ok(OneD {
        nx: ncol,
        ny: nrow,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("st.1D");
        std::fs::write(&path, text).unwrap();
        let p = path.to_str().unwrap().to_owned();
        (dir, p)
    }

    #[test]
    fn files_read_column_by_column() {
        for (text, want, nx, ny) in [
            ("0.0\t1.0\t0.5\t1.5\n", vec![0.0, 1.0, 0.5, 1.5], 1, 4),
            ("0\n1.5\n0.5\n1\n", vec![0.0, 1.5, 0.5, 1.0], 4, 1),
            (
                "0 9\n1.5 9\n0.5 9\n1 9\n",
                vec![0.0, 1.5, 0.5, 1.0, 9.0, 9.0, 9.0, 9.0],
                4,
                2,
            ),
            (
                "# slice times\n0, 1.5, 0.25, 1.75 # end\n",
                vec![0.0, 1.5, 0.25, 1.75],
                1,
                4,
            ),
            ("2@0 2@1\n", vec![0.0, 0.0, 1.0, 1.0], 1, 4),
            ("1 2\r\n3\r\n", vec![1.0, 3.0, 2.0, 0.0], 2, 2),
            ("1 2 \\\n 3\n", vec![1.0, 2.0, 3.0], 1, 3),
        ] {
            let (_d, p) = file(text);
            let im = read_1d(&p).unwrap();
            assert_eq!((im.nx, im.ny), (nx, ny), "{text:?}");
            assert_eq!(im.data, want, "{text:?}");
        }
    }

    #[test]
    fn text_is_an_error() {
        let (_d, p) = file("0 1 abc 1.5\n");
        assert!(matches!(read_1d(&p), Err(OneDError::Text { row: 0, .. })));
        let (_d, p) = file("# nothing\n\n");
        assert!(matches!(read_1d(&p), Err(OneDError::NoData(_))));
        assert!(matches!(
            read_1d("/no/such/file.1D"),
            Err(OneDError::Open(_))
        ));
    }

    #[test]
    fn strings() {
        let im = read_1d("1D: 0 1 0.5 1.5").unwrap();
        assert_eq!((im.nx, im.ny), (4, 1));
        assert_eq!(im.data, [0.0, 1.0, 0.5, 1.5]);
        let im = read_1d("1D: 3@2,1 | 5").unwrap();
        assert_eq!((im.nx, im.ny), (4, 2));
        assert_eq!(im.data, [2.0, 2.0, 2.0, 1.0, 5.0, 0.0, 0.0, 0.0]);
        let im = read_1d("1D: 3%1%0.5").unwrap();
        assert_eq!(im.data, [1.0, 1.5, 2.0]);
        assert!(read_1d("1D: abc").is_err());
        let im = read_1d("1D: 1 2'").unwrap();
        assert_eq!((im.nx, im.ny), (1, 2));
    }
}

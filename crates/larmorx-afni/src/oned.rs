// SPDX-License-Identifier: Apache-2.0
//! AFNI's "1D" text format, as far as `3dTshift -tpattern @file` uses it.
//!
//! The rules were established by running AFNI 25.2.09 on test files (`specs/3dTshift.md`
//! §3.1 and §7.1), not from AFNI's code:
//! - a file is a matrix: one row per line, values separated by white space, commas or colons,
//!   `#` starting a comment; blank lines are skipped. Lines end at LF or CR, or at LF only
//!   with `AFNI_USE_FGETS` (a CR is then white space);
//! - the first row sets the number of columns: later rows lose extra values and are padded
//!   with zeros; values are returned column by column (a single line reads left to right);
//! - `N@v` stands for `N` copies of `v`;
//! - a value that starts with a character other than a digit, `+`, `-`, `.`, `e`, `@` or `*`
//!   is an error (AFNI's message names the row), unless `AFNI_1D_ZERO_TEXT` is yes, which
//!   makes such values 0 (a row of text then counts as a row). Every other value is the
//!   decimal number it starts with, or 0 (`2x` is 2, `e5` and `*` are 0);
//! - `1D: ...` gives the values inline, each a whole number or `N@v`, with `|` starting a new
//!   column.
//!
//! Each value is parsed as a double and rounded to float32, as AFNI stores them. Unlike AFNI,
//! larmorx does not read hexadecimal numbers, infinities or NaN (an error), sub-matrix
//! selectors (`file[2]`, `file{0..5}`) or transposition (`file'`).

use std::path::Path;

use crate::cli::leading_number;
use crate::msg::Message;

/// How AFNI's variables change the reading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OneDOptions {
    /// `AFNI_1D_ZERO_TEXT` (or `-ok_1D_text`): text values are 0.
    pub zero_text: bool,
    /// `AFNI_USE_FGETS`: lines end at LF only.
    pub fgets: bool,
}

/// A 1D text could not be read: AFNI's messages (before its fatal "Can't read tpattern
/// file").
pub type Failure = Vec<Message>;

/// A token AFNI reads as a number and larmorx does not: hexadecimal, or a signed infinity or
/// NaN (an unsigned `inf` or `nan` is a word: an error, or 0 with `AFNI_1D_ZERO_TEXT`).
fn unsupported(token: &str) -> bool {
    let t = token.strip_prefix(['+', '-']);
    let lower = t.unwrap_or(token).to_ascii_lowercase();
    lower.starts_with("0x")
        || (t.is_some() && (lower.starts_with("inf") || lower.starts_with("nan")))
}

/// The value of a token that is not an error: the decimal number it starts with, or 0; a word
/// with `AFNI_1D_ZERO_TEXT` is 0.
fn value(token: &str) -> f64 {
    leading_number(token).map_or(0.0, |(v, _)| v)
}

/// The values of one token: `N@v` gives `N` copies of `v` (a count below 1 reads the whole
/// token as one value).
fn token_values(token: &str, out: &mut Vec<f64>) {
    if let Some((count, rest)) = token.split_once('@') {
        let n = crate::cli::atoi(count);
        if n >= 1 {
            out.extend(std::iter::repeat_n(value(rest), n as usize));
            return;
        }
    }
    out.push(value(token));
}

/// The row as AFNI shows it in its error: leading blanks removed, commas and colons made
/// blanks, cut at 64 characters (the last four ` ...`).
fn shown_row(line: &str) -> String {
    let s: String = line
        .trim_start()
        .chars()
        .map(|c| if c == ',' || c == ':' { ' ' } else { c })
        .collect();
    if s.chars().count() > 64 {
        let head: String = s.chars().take(60).collect();
        format!("{head} ...")
    } else {
        s
    }
}

/// The values of a 1D text (file contents), column by column, as float32. `name` is the file's
/// name in messages.
pub fn parse_text(text: &str, name: &str, options: OneDOptions) -> Result<Vec<f32>, Failure> {
    let lines: Vec<&str> = if options.fgets {
        text.split('\n').collect()
    } else {
        text.split(['\n', '\r']).collect()
    };
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in lines {
        let data = line.split('#').next().unwrap_or("");
        let tokens: Vec<&str> = data
            .split(|c: char| c.is_whitespace() || c == ',' || c == ':')
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            continue;
        }
        let mut values = Vec::new();
        for token in tokens {
            if unsupported(token) {
                return Err(vec![Message::Error(format!(
                    "larmorx: the value '{token}' in 1D file {name} is not supported (no \
                     hexadecimal numbers, infinities or NaN)"
                ))]);
            }
            let first = token.chars().next().expect("non-empty");
            let numeric =
                first.is_ascii_digit() || matches!(first, '+' | '-' | '.' | 'e' | '@' | '*');
            if !numeric && !options.zero_text {
                // AFNI cuts the file name at 44 bytes (observed).
                let file = String::from_utf8_lossy(&name.as_bytes()[..name.len().min(44)]);
                return Err(vec![Message::Raw(format!(
                    "\n** Error: Failed parsing data row {row} of 1D file '{file}'\n          Check \
                     for illegal non-numeric characters in:\n          '{shown}'\n          You \
                     can have text entries set to 0 with -ok_1D_text or by \n          setting \
                     environment variable AFNI_1D_ZERO_TEXT to YES.\n",
                    row = rows.len(),
                    shown = shown_row(line),
                ))]);
            }
            if !numeric {
                values.push(0.0);
            } else {
                token_values(token, &mut values);
            }
        }
        rows.push(values);
    }
    let Some(ncols) = rows.first().map(Vec::len) else {
        return Err(vec![Message::Error(format!(
            "mri_read_ascii: can't read any valid data from file {name}"
        ))]);
    };
    let mut out = Vec::with_capacity(ncols * rows.len());
    for col in 0..ncols {
        for row in &rows {
            out.push(row.get(col).copied().unwrap_or(0.0) as f32);
        }
    }
    Ok(out)
}

/// A whole number, or `N@v` with whole numbers, as an inline `1D:` string needs them.
fn strict_values(token: &str, out: &mut Vec<f64>) -> bool {
    let number = |s: &str| -> Option<f64> {
        if unsupported(s) {
            return None;
        }
        let s = s.strip_prefix('+').unwrap_or(s);
        let ok = s
            .trim_start_matches('-')
            .starts_with(|c: char| c.is_ascii_digit() || c == '.');
        s.parse::<f64>().ok().filter(|v| ok && v.is_finite())
    };
    match token.split_once('@') {
        Some((count, v)) => match (count.parse::<usize>(), number(v)) {
            (Ok(n), Some(v)) => {
                out.extend(std::iter::repeat_n(v, n));
                true
            }
            _ => false,
        },
        None => number(token).map(|v| out.push(v)).is_some(),
    }
}

/// The values of an inline `1D: ...` string (the part after `1D:`), column by column.
pub fn parse_inline(text: &str) -> Result<Vec<f32>, Failure> {
    let cant = || Message::Error(format!("mri_read_1D: can't read from string '1D:{text}'"));
    let mut columns: Vec<Vec<f64>> = Vec::new();
    for part in text.split('|') {
        let mut column = Vec::new();
        for token in part
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
        {
            if !strict_values(token, &mut column) {
                return Err(vec![
                    Message::Error(format!("Illegal 1D: value '{token}'")),
                    cant(),
                ]);
            }
        }
        columns.push(column);
    }
    let nrows = columns.iter().map(Vec::len).max().unwrap_or(0);
    if nrows == 0 {
        return Err(vec![
            Message::Error("Can't decode 1D: string".into()),
            cant(),
        ]);
    }
    let mut out = Vec::with_capacity(nrows * columns.len());
    for column in &columns {
        out.extend((0..nrows).map(|i| column.get(i).copied().unwrap_or(0.0) as f32));
    }
    Ok(out)
}

/// The values named by a `-tpattern @...` argument (without the `@`): an inline `1D:` string
/// or a file.
pub fn read(spec: &str, options: OneDOptions) -> Result<Vec<f32>, Failure> {
    if let Some(inline) = spec.strip_prefix("1D:") {
        return parse_inline(inline);
    }
    let path = Path::new(spec);
    if !path.exists() && (spec.ends_with(']') || spec.ends_with('}') || spec.ends_with('\'')) {
        return Err(vec![Message::Error(format!(
            "larmorx: 1D sub-matrix selectors and transposition are not supported ({spec})"
        ))]);
    }
    let Ok(bytes) = std::fs::read(path) else {
        return Err(vec![Message::Error(format!(
            "mri_read_ascii: couldn't open file {spec}"
        ))]);
    };
    parse_text(&String::from_utf8_lossy(&bytes), spec, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFNI: OneDOptions = OneDOptions {
        zero_text: false,
        fgets: false,
    };
    const ZERO: OneDOptions = OneDOptions {
        zero_text: true,
        fgets: false,
    };

    fn values(text: &str, o: OneDOptions) -> Vec<f32> {
        parse_text(text, "t", o).unwrap()
    }

    #[test]
    fn single_line_reads_left_to_right() {
        assert_eq!(values("0.0\t1.0\t0.5\t1.5\n", AFNI), [0.0, 1.0, 0.5, 1.5]);
        assert_eq!(
            values("# comment\n\n0,1 , 0.5 1.5 # trailing\r\n", AFNI),
            [0.0, 1.0, 0.5, 1.5]
        );
        assert_eq!(values("0,1:2;3\n", AFNI), [0.0, 1.0, 2.0]);
    }

    #[test]
    fn matrices_read_column_by_column() {
        assert_eq!(
            values("0 1\n0.5 1.5\n0.25 1.25\n", AFNI),
            [0.0, 0.5, 0.25, 1.0, 1.5, 1.25]
        );
        assert_eq!(
            values("0 1 0.5\n1.5 0.25 1.25 0.3\n", AFNI),
            [0.0, 1.5, 1.0, 0.25, 0.5, 1.25]
        );
        assert_eq!(
            values("0 1 0.5 1.5\n0.25 1.25 0.3\n", AFNI),
            [0.0, 0.25, 1.0, 1.25, 0.5, 0.3, 1.5, 0.0]
        );
        // CR ends a line, unless AFNI_USE_FGETS.
        assert_eq!(values("0 1\r2 3\r", AFNI), [0.0, 2.0, 1.0, 3.0]);
        let fgets = OneDOptions {
            zero_text: false,
            fgets: true,
        };
        assert_eq!(values("0 1\r2 3\r", fgets), [0.0, 1.0, 2.0, 3.0]);
    }

    /// AFNI's lenient values (observed with AFNI 25.2.09).
    #[test]
    fn afni_reads_leading_numbers() {
        let one = |t: &str, o| values(&format!("0 1 {t} 1.5\n"), o);
        for (token, expected) in [
            ("*", vec![0.0]),
            ("e5", vec![0.0]),
            ("-x", vec![0.0]),
            (".x", vec![0.0]),
            ("+x", vec![0.0]),
            ("1e", vec![1.0]),
            ("2x", vec![2.0]),
            ("3@2", vec![2.0, 2.0, 2.0]),
            ("@2", vec![0.0]),
            ("2@", vec![0.0, 0.0]),
            ("2@x", vec![0.0, 0.0]),
        ] {
            let mut want = vec![0.0, 1.0];
            want.extend(expected);
            want.push(1.5);
            assert_eq!(one(token, AFNI), want, "{token}");
        }
        assert_eq!(one("abc", ZERO), [0.0, 1.0, 0.0, 1.5]);
        assert_eq!(one("nan", ZERO), [0.0, 1.0, 0.0, 1.5]);
        // A row of text counts as a row with AFNI_1D_ZERO_TEXT.
        assert_eq!(values("a b\n0 1 2 3\n", ZERO), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            values("0 1\na b\n2 3\n", ZERO),
            [0.0, 0.0, 2.0, 1.0, 0.0, 3.0]
        );
    }

    #[test]
    fn afni_messages() {
        let err = parse_text("0 1\n0, 1, abc 1.5   # c\n", "st.1D", AFNI).unwrap_err();
        assert_eq!(
            err,
            [Message::Raw(
                "\n** Error: Failed parsing data row 1 of 1D file 'st.1D'\n          Check for \
                 illegal non-numeric characters in:\n          '0  1  abc 1.5   # c'\n          \
                 You can have text entries set to 0 with -ok_1D_text or by \n          setting \
                 environment variable AFNI_1D_ZERO_TEXT to YES.\n"
                    .into()
            )]
        );
        let long_name = "a_very_long_file_name_for_the_one_d_reader_test_123456789.1D";
        let long_row = format!("0 1 abc 1.5 {}", "x".repeat(70));
        let Message::Raw(text) = &parse_text(&long_row, long_name, AFNI).unwrap_err()[0] else {
            panic!()
        };
        assert!(
            text.contains("'a_very_long_file_name_for_the_one_d_reader_t'"),
            "{text}"
        );
        assert!(
            text.contains(&format!("'0 1 abc 1.5 {} ...'", "x".repeat(48))),
            "{text}"
        );
        for text in ["0 1 nan 1.5", "inf 1"] {
            assert!(parse_text(text, "t", AFNI).is_err(), "{text}");
        }
        assert_eq!(
            parse_text("# only a comment\n", "st.1D", AFNI).unwrap_err(),
            [Message::Error(
                "mri_read_ascii: can't read any valid data from file st.1D".into()
            )]
        );
        assert!(parse_text("0 0x1 1", "t", ZERO).is_err());
        assert!(parse_text("0 -inf 1", "t", ZERO).is_err());
    }

    #[test]
    fn inline_text() {
        assert_eq!(
            parse_inline(" 0 1 0.5 | 1.5 2 3").unwrap(),
            [0.0, 1.0, 0.5, 1.5, 2.0, 3.0]
        );
        assert_eq!(read("1D: 2@0.5,1", AFNI).unwrap(), [0.5, 0.5, 1.0]);
        assert_eq!(
            parse_inline(" 0 1 2x 1.5").unwrap_err(),
            [
                Message::Error("Illegal 1D: value '2x'".into()),
                Message::Error("mri_read_1D: can't read from string '1D: 0 1 2x 1.5'".into()),
            ]
        );
        assert_eq!(
            parse_inline("").unwrap_err()[0],
            Message::Error("Can't decode 1D: string".into())
        );
    }

    #[test]
    fn files() {
        assert!(matches!(
            &read("/nonexistent/file.1D[0]", AFNI).unwrap_err()[0],
            Message::Error(t) if t.contains("not supported")
        ));
        assert_eq!(
            read("/nonexistent/file.1D", AFNI).unwrap_err(),
            [Message::Error(
                "mri_read_ascii: couldn't open file /nonexistent/file.1D".into()
            )]
        );
    }
}

//! AFNI's "1D" text format, as far as `3dTshift -tpattern @file` uses it.
//!
//! The rules were established by running AFNI 25.2.09 on test files (`specs/3dTshift.md`
//! §3.1), not from AFNI's code:
//! - a file is a matrix: one row per line, values separated by white space or commas, `#`
//!   starting a comment; blank lines are skipped;
//! - the first row sets the number of columns: later rows lose extra values and are padded
//!   with zeros;
//! - values are returned column by column (so a single line reads left to right);
//! - `N@v` stands for `N` copies of `v`;
//! - `1D: ...` gives the values inline, with `|` starting a new column;
//! - anything that is not a number is an error, and so are `nan` and `inf`.
//!
//! Each value is parsed as a double and rounded to float32, as AFNI stores them. AFNI's
//! sub-matrix selectors (`file[2]`, `file{0..5}`) and transposition (`file'`) are not
//! supported.

use std::path::Path;

/// A 1D text could not be read.
#[derive(Debug, thiserror::Error)]
pub enum OneDError {
    #[error("can't read 1D file {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("1D text {what}: '{token}' is not a number")]
    NotANumber { what: String, token: String },
    #[error("1D text {0} holds no values")]
    Empty(String),
    #[error("1D text {0}: sub-matrix selectors and transposition are not supported by larmorx")]
    Unsupported(String),
}

/// Parses one token: a finite number, or `N@v`.
fn token_values(token: &str, what: &str) -> Result<Vec<f64>, OneDError> {
    let bad = || OneDError::NotANumber {
        what: what.to_owned(),
        token: token.to_owned(),
    };
    let number = |s: &str| -> Result<f64, OneDError> {
        let s = s.strip_prefix('+').unwrap_or(s);
        // Rust's parser also accepts "inf", "nan" and "infinity": AFNI does not.
        let starts_ok = s
            .trim_start_matches('-')
            .starts_with(|c: char| c.is_ascii_digit() || c == '.');
        match s.parse::<f64>() {
            Ok(v) if starts_ok && v.is_finite() => Ok(v),
            _ => Err(bad()),
        }
    };
    match token.split_once('@') {
        Some((count, value)) => {
            let count: usize = count.parse().map_err(|_| bad())?;
            Ok(vec![number(value)?; count])
        }
        None => Ok(vec![number(token)?]),
    }
}

fn row_values(line: &str, what: &str) -> Result<Vec<f64>, OneDError> {
    let mut out = Vec::new();
    for token in line
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
    {
        out.extend(token_values(token, what)?);
    }
    Ok(out)
}

/// The values of a 1D text (file contents), column by column, as float32.
pub fn parse_text(text: &str, what: &str) -> Result<Vec<f32>, OneDError> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let values = row_values(line, what)?;
        if !values.is_empty() {
            rows.push(values);
        }
    }
    let Some(ncols) = rows.first().map(Vec::len) else {
        return Err(OneDError::Empty(what.to_owned()));
    };
    let mut out = Vec::with_capacity(ncols * rows.len());
    for col in 0..ncols {
        for row in &rows {
            out.push(row.get(col).copied().unwrap_or(0.0) as f32);
        }
    }
    Ok(out)
}

/// The values of an inline `1D: ...` text (the part after `1D:`), column by column.
pub fn parse_inline(text: &str) -> Result<Vec<f32>, OneDError> {
    let what = format!("'1D:{text}'");
    let columns: Vec<Vec<f64>> = text
        .split('|')
        .map(|c| row_values(c, &what))
        .collect::<Result<_, _>>()?;
    let nrows = columns.iter().map(Vec::len).max().unwrap_or(0);
    if nrows == 0 {
        return Err(OneDError::Empty(what));
    }
    let mut out = Vec::with_capacity(nrows * columns.len());
    for column in &columns {
        out.extend((0..nrows).map(|i| column.get(i).copied().unwrap_or(0.0) as f32));
    }
    Ok(out)
}

/// The values named by a `-tpattern @...` argument (without the `@`): an inline `1D:` text or
/// a file.
pub fn read(spec: &str) -> Result<Vec<f32>, OneDError> {
    if let Some(inline) = spec.strip_prefix("1D:") {
        return parse_inline(inline);
    }
    let path = Path::new(spec);
    if !path.exists() && (spec.ends_with(']') || spec.ends_with('}') || spec.ends_with('\'')) {
        return Err(OneDError::Unsupported(spec.to_owned()));
    }
    let text = std::fs::read_to_string(path).map_err(|source| OneDError::Io {
        path: spec.to_owned(),
        source,
    })?;
    parse_text(&text, spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_reads_left_to_right() {
        let v = parse_text("0.0\t1.0\t0.5\t1.5\n", "t").unwrap();
        assert_eq!(v, [0.0, 1.0, 0.5, 1.5]);
        let v = parse_text("# comment\n\n0,1 , 0.5 1.5 # trailing\r\n", "t").unwrap();
        assert_eq!(v, [0.0, 1.0, 0.5, 1.5]);
    }

    #[test]
    fn matrices_read_column_by_column() {
        // 3 rows x 2 columns.
        let v = parse_text("0 1\n0.5 1.5\n0.25 1.25\n", "t").unwrap();
        assert_eq!(v, [0.0, 0.5, 0.25, 1.0, 1.5, 1.25]);
        // The first row sets the width: extra values dropped, missing ones zero.
        let v = parse_text("0 1 0.5\n1.5 0.25 1.25 0.3\n", "t").unwrap();
        assert_eq!(v, [0.0, 1.5, 1.0, 0.25, 0.5, 1.25]);
        let v = parse_text("0 1 0.5 1.5\n0.25 1.25 0.3\n", "t").unwrap();
        assert_eq!(v, [0.0, 0.25, 1.0, 1.25, 0.5, 0.3, 1.5, 0.0]);
    }

    #[test]
    fn repeats_and_inline_text() {
        assert_eq!(
            parse_text("3@0 2@1\n", "t").unwrap(),
            [0.0, 0.0, 0.0, 1.0, 1.0]
        );
        assert_eq!(
            parse_inline(" 0 1 0.5 | 1.5 2 3").unwrap(),
            [0.0, 1.0, 0.5, 1.5, 2.0, 3.0]
        );
        assert_eq!(read("1D: 2@0.5,1").unwrap(), [0.5, 0.5, 1.0]);
    }

    #[test]
    fn values_are_rounded_from_double() {
        let v = parse_text("0.1 1e-1 +2.5E0 .5 1.\n", "t").unwrap();
        assert_eq!(v, [0.1f64 as f32, 0.1f64 as f32, 2.5, 0.5, 1.0]);
    }

    #[test]
    fn non_numbers_are_errors() {
        for text in [
            "0 1 abc",
            "0 nan 1",
            "inf 1",
            "0;1;2",
            "",
            "# only a comment\n",
        ] {
            assert!(parse_text(text, "t").is_err(), "{text:?}");
        }
        assert!(matches!(
            read("/nonexistent/file.1D[0]"),
            Err(OneDError::Unsupported(_))
        ));
        assert!(matches!(
            read("/nonexistent/file.1D"),
            Err(OneDError::Io { .. })
        ));
    }
}

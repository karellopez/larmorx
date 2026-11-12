// SPDX-License-Identifier: Apache-2.0
//! `--exclude`'s text (`specs/t2smap.md` §5.2).

use larmorx_core::python::argparse::py_repr_str;
use larmorx_core::python::number::parse_int;

use crate::log::PyError;

/// The sorted indices of an `--exclude` text (§5.2), or its `ValueError`.
pub fn parse_exclude(text: Option<&str>) -> Result<Vec<i128>, PyError> {
    let Some(text) = text else {
        return Ok(Vec::new());
    };
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let bad = |what: &str| {
        PyError::value(format!(
            "Invalid volume indices string ({text}). {what} are not supported."
        ))
    };
    let int = |s: &str| {
        parse_int(s).ok_or_else(|| {
            PyError::value(format!(
                "invalid literal for int() with base 10: {}",
                py_repr_str(s)
            ))
        })
    };
    let mut out: Vec<i128> = Vec::new();
    for item in text.split(',') {
        let item = item.trim();
        let colons = item.matches(':').count();
        if colons > 1 {
            return Err(bad("Step sizes"));
        }
        if colons == 1 {
            let (a, b) = item.split_once(':').expect("one colon");
            if a.trim().is_empty() || b.trim().is_empty() {
                return Err(bad("Open-ended ranges"));
            }
            let (lo, hi) = (int(a)?, int(b)?);
            if lo < 0 || hi < 0 {
                return Err(bad("Negative indices"));
            }
            // a range larger than memory cannot be listed: Python would fail with MemoryError
            if hi - lo > 1 << 31 {
                return Err(PyError::new("MemoryError", ""));
            }
            out.extend(lo..hi);
        } else {
            let i = int(item)?;
            if i < 0 {
                return Err(bad("Negative indices"));
            }
            out.push(i);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_texts() {
        assert_eq!(parse_exclude(None).unwrap(), Vec::<i128>::new());
        assert_eq!(parse_exclude(Some("")).unwrap(), Vec::<i128>::new());
        assert_eq!(parse_exclude(Some("5, 0,2:4,3")).unwrap(), vec![0, 2, 3, 5]);
        assert_eq!(parse_exclude(Some("4:2")).unwrap(), Vec::<i128>::new());
        let msg = |t: &str| parse_exclude(Some(t)).unwrap_err().last_line();
        assert_eq!(
            msg("0:4:2"),
            "ValueError: Invalid volume indices string (0:4:2). Step sizes are not supported."
        );
        assert!(msg("2:").ends_with("Open-ended ranges are not supported."));
        assert!(msg("-1").ends_with("Negative indices are not supported."));
        assert!(msg("-2:1").ends_with("Negative indices are not supported."));
        assert_eq!(
            msg("a"),
            "ValueError: invalid literal for int() with base 10: 'a'"
        );
    }
}

// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Written for the translation of FSL
// 6.0.7.17; it holds no FSL code.
//! FSL's environment variables as a table of settings (porting guide §9.1).
//!
//! FSL's programs read a few variables with `getenv` (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`,
//! ...). The replica never changes its own process environment: the command line reads the
//! variables it needs once into a [`Settings`] table and passes it to every routine that reads
//! one; tests and the Python wrapper build the table explicitly.

use std::collections::BTreeMap;

/// The variables FSL's code on the replicated paths reads.
pub const FSL_VARIABLES: &[&str] = &["FSLOUTPUTTYPE", "FSL_LOAD_NIFTI_EXTENSIONS"];

/// A table of environment settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    values: BTreeMap<String, String>,
}

impl Settings {
    /// An empty table (every variable unset).
    pub fn new() -> Settings {
        Settings::default()
    }

    /// The process environment's values of `names`, read once. On Windows names match whatever
    /// their case, as the system's own lookup does.
    pub fn from_env(names: &[&str]) -> Settings {
        let mut s = Settings::new();
        for (k, v) in std::env::vars_os() {
            let (Some(k), Some(v)) = (k.to_str(), v.to_str()) else {
                continue;
            };
            for &name in names {
                let same = if cfg!(windows) {
                    k.eq_ignore_ascii_case(name)
                } else {
                    k == name
                };
                if same {
                    s.values.insert(name.to_owned(), v.to_owned());
                }
            }
        }
        s
    }

    /// Sets `name` to `value`.
    pub fn with(mut self, name: &str, value: &str) -> Settings {
        self.values.insert(name.to_owned(), value.to_owned());
        self
    }

    /// Removes `name`.
    pub fn without(mut self, name: &str) -> Settings {
        self.values.remove(name);
        self
    }

    /// `getenv(name)`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

/// C's `atoi` ([`crate::cnum::atoi`]): optional leading white space, an optional sign, then
/// decimal digits up to the first other character; 0 when there are none. Out-of-range values
/// give what glibc's `strtol`-based `atoi` gives on x86-64 (the `long` clamped to its range,
/// then truncated to `int`).
pub fn atoi(s: &str) -> i32 {
    crate::cnum::atoi(s)
}

/// glibc's `strtol(s, NULL, 10)`: the value clamped to `long`'s range.
pub fn strtol(s: &str) -> i64 {
    crate::cnum::strtol(s, 10).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_reads_like_c() {
        assert_eq!(atoi("12"), 12);
        assert_eq!(atoi("  -7xyz"), -7);
        assert_eq!(atoi("abc"), 0);
        assert_eq!(atoi("1.5"), 1);
        assert_eq!(atoi("+3"), 3);
        assert_eq!(atoi(""), 0);
        assert_eq!(atoi("99999999999"), (99_999_999_999_i64) as i32);
        assert_eq!(atoi("99999999999999999999999"), i64::MAX as i32);
    }

    #[test]
    fn tables() {
        let s = Settings::new().with("FSLOUTPUTTYPE", "NIFTI");
        assert_eq!(s.get("FSLOUTPUTTYPE"), Some("NIFTI"));
        assert_eq!(
            s.clone().without("FSLOUTPUTTYPE").get("FSLOUTPUTTYPE"),
            None
        );
    }
}

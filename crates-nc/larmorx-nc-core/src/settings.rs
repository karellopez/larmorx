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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::nifti::{OpenOptions, Renames};

/// The variables FSL's code on the replicated paths reads (`FSLDIR`: where FAST finds its standard
/// tissue priors and the scripts their data; `maskthr`: the `standard_space_roi` script's
/// variable, which bash takes from the environment when the script does not set it, FSL-213),
/// and larmorx's own `LARMORX_FSL_DATA` (where the scripts' data are, [`crate::fsldata`]).
pub const FSL_VARIABLES: &[&str] = &[
    "FSLOUTPUTTYPE",
    "FSL_LOAD_NIFTI_EXTENSIONS",
    "FSLDIR",
    "maskthr",
    crate::fsldata::ENV,
];

/// A table of environment settings, with how the image layer opens names (FSL-60: NewNifti
/// 5.0.0's renaming of names holding ".img", reproduced or not, and the renames met) and
/// FSL-11's default output type (`FSLOUTPUTTYPE` unset: `NIFTI_GZ`, and whether an image was
/// written so).
#[derive(Clone, Debug, Default)]
pub struct Settings {
    values: BTreeMap<String, String>,
    reproduce_renames: bool,
    renames: Arc<Renames>,
    /// FSL-11: `Some` when `FSLOUTPUTTYPE` was unset and [`Settings::defaulting_output_type`]
    /// gave it `NIFTI_GZ`; then whether an image was written with it (shared by every copy a
    /// run's steps make of the table).
    output_type_default: Option<Arc<AtomicBool>>,
}

impl PartialEq for Settings {
    fn eq(&self, other: &Settings) -> bool {
        self.values == other.values && self.reproduce_renames == other.reproduce_renames
    }
}

impl Eq for Settings {}

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

    /// FSL-60 in D13's `reproduce` mode: image names opened as NewNifti 5.0.0 renames them.
    pub fn reproducing_renames(mut self, reproduce: bool) -> Settings {
        self.reproduce_renames = reproduce;
        self
    }

    /// How the image layer opens a name.
    pub fn open_options(&self) -> OpenOptions<'_> {
        OpenOptions {
            reproduce_renames: self.reproduce_renames,
            renames: Some(&self.renames),
        }
    }

    /// The names NewNifti 5.0.0 would have renamed so far (FSL-60): `(given, renamed)`.
    pub fn renames(&self) -> Vec<(String, String)> {
        self.renames.taken()
    }

    /// FSL-11 corrected when `fix` is set: with `FSLOUTPUTTYPE` unset (absent, not empty) the
    /// table gives it `NIFTI_GZ`, the type FSL's own configuration sets, and notes when an
    /// image is written with it ([`Settings::output_type_defaulted`]). FSL's programs stop with
    /// an error at their first image instead (exit 1), which the table keeps when `fix` is not
    /// set (`reproduce`), as it keeps a variable that is set.
    pub fn defaulting_output_type(mut self, fix: bool) -> Settings {
        if fix && !self.values.contains_key("FSLOUTPUTTYPE") {
            self.values
                .insert("FSLOUTPUTTYPE".to_owned(), "NIFTI_GZ".to_owned());
            self.output_type_default = Some(Arc::new(AtomicBool::new(false)));
        }
        self
    }

    /// Notes that an image is written with the type `FSLOUTPUTTYPE` gives, at the point where
    /// FSL's image layer reads it (`FslGetEnvOutputType` in `save_volume`): under FSL-11's
    /// default, the point where FSL's program would have stopped.
    pub fn note_output_type_read(&self) {
        if let Some(w) = &self.output_type_default {
            w.store(true, Ordering::Relaxed);
        }
    }

    /// FSL-11: whether an image was written as `NIFTI_GZ` in place of the unset
    /// `FSLOUTPUTTYPE` ([`Settings::defaulting_output_type`]).
    pub fn output_type_defaulted(&self) -> bool {
        self.output_type_default
            .as_ref()
            .is_some_and(|w| w.load(Ordering::Relaxed))
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

    /// FSL-11's default: only for an absent variable and only when fixing; noted by any copy.
    #[test]
    fn output_type_default() {
        let unset = Settings::new().defaulting_output_type(true);
        assert_eq!(unset.get("FSLOUTPUTTYPE"), Some("NIFTI_GZ"));
        assert!(!unset.output_type_defaulted());
        let step = unset.clone().with("FSLDIR", "/opt/fsl");
        step.note_output_type_read();
        assert!(unset.output_type_defaulted());
        for kept in [
            Settings::new().defaulting_output_type(false),
            Settings::new()
                .with("FSLOUTPUTTYPE", "")
                .defaulting_output_type(true),
            Settings::new()
                .with("FSLOUTPUTTYPE", "NIFTI")
                .defaulting_output_type(true),
        ] {
            let before = kept.get("FSLOUTPUTTYPE").map(str::to_owned);
            kept.note_output_type_read();
            assert!(!kept.output_type_defaulted());
            assert_eq!(kept.get("FSLOUTPUTTYPE").map(str::to_owned), before);
        }
        assert_eq!(
            Settings::new()
                .defaulting_output_type(false)
                .get("FSLOUTPUTTYPE"),
            None
        );
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

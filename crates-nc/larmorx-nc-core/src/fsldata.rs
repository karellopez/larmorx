// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! FSL's data files that the replicated scripts read (`${FSLDIR}/data/standard/MNI152_T1_2mm`,
//! its dilated brain mask, the eye mask, ...): where the replica finds them.
//!
//! FSL's scripts name them under `${FSLDIR}/data`. The replica runs none of FSL's programs, so
//! it needs no FSL installation; it looks for each file, in this order:
//!
//! 1. [`ENV`] (`LARMORX_FSL_DATA`), larmorx's explicit choice: a directory laid out as
//!    `${FSLDIR}/data` (`standard/MNI152_T1_2mm.nii.gz`, ...). When it is set, the file must be
//!    there: nothing else is searched.
//! 2. `${FSLDIR}/data`, as FSL's scripts read it, when `FSLDIR` is set and the image is there.
//! 3. The copy larmorx-nc carries ([`bundled`]: `<prefix>/share/larmorx-nc/fsl-data`, beside
//!    the `larmorx-nc` program's `<prefix>/bin` or `<prefix>\Scripts`), where the wheel
//!    installed it (the user's decision of 2026-11-04: the small standard files ship inside
//!    larmorx-nc, from FSL 6.0.7.23's fsl-data_standard 2208.0).
//!
//! When none has it, the script refuses before it runs anything, naming the file and every
//! place looked ([`find`]).

use std::path::PathBuf;

use crate::avwutils;
use crate::settings::Settings;

/// The environment variable of larmorx's explicit data directory.
pub const ENV: &str = "LARMORX_FSL_DATA";

/// The data directory larmorx-nc's wheel installs beside its program, if the program runs from
/// an installation (`<prefix>/bin/larmorx-nc` or `<prefix>\Scripts\larmorx-nc.exe`).
pub fn bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let prefix = exe.parent()?.parent()?;
    let dir = prefix.join("share").join("larmorx-nc").join("fsl-data");
    dir.is_dir().then_some(dir)
}

/// Where a data file was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// [`ENV`].
    Explicit,
    /// `${FSLDIR}/data`.
    Fsldir,
    /// larmorx-nc's own copy ([`bundled`]).
    Bundled,
}

/// A data file: its name as FSL's scripts give it to FSL's programs (without its extension;
/// `${FSLDIR}/data/standard/MNI152_T1_2mm` when it comes from `FSLDIR`), and where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub source: Source,
}

/// The image `rel` (relative to FSL's `data` directory, without extension, e.g.
/// `standard/MNI152_T1_2mm`): found as the module says, or the refusal's message (without the
/// program's prefix).
pub fn find(rel: &str, fsl: &Settings) -> Result<Found, String> {
    find_in(rel, fsl, bundled())
}

/// [`find`] with the bundled directory given (tests).
pub fn find_in(rel: &str, fsl: &Settings, bundled: Option<PathBuf>) -> Result<Found, String> {
    let join = |dir: &str| format!("{}/{rel}", dir.trim_end_matches('/'));
    if let Some(dir) = fsl.get(ENV).filter(|d| !d.is_empty()) {
        let name = join(dir);
        if avwutils::imtest(&name) {
            return Ok(Found {
                name,
                source: Source::Explicit,
            });
        }
        return Err(format!(
            "FSL's data image {rel} is not in {ENV}={dir} (no image {name}); FSLDIR and \
             larmorx-nc's own copy are not searched when {ENV} is set"
        ));
    }
    let mut looked = Vec::new();
    match fsl.get("FSLDIR").filter(|d| !d.is_empty()) {
        Some(dir) => {
            let name = format!("{}/data/{rel}", dir);
            if avwutils::imtest(&name) {
                return Ok(Found {
                    name,
                    source: Source::Fsldir,
                });
            }
            looked.push(format!("FSLDIR ({name})"));
        }
        None => looked.push("FSLDIR (not set)".to_owned()),
    }
    match bundled {
        Some(dir) => {
            let name = join(&dir.to_string_lossy());
            if avwutils::imtest(&name) {
                return Ok(Found {
                    name,
                    source: Source::Bundled,
                });
            }
            looked.push(format!("larmorx-nc's copy ({name})"));
        }
        None => looked.push("larmorx-nc's copy (not installed beside this program)".to_owned()),
    }
    Err(format!(
        "FSL's data image {rel} was not found: looked in {}; set {ENV} to a directory laid out \
         as FSL's data directory, or FSLDIR to an FSL installation",
        looked.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(dir: &std::path::Path, rel: &str) {
        let p = dir.join(format!("{rel}.nii.gz"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn the_order_explicit_fsldir_bundled() {
        let root = tempfile::tempdir().unwrap();
        let (explicit, fsldir, bundled) = (
            root.path().join("explicit"),
            root.path().join("fsl"),
            root.path().join("share"),
        );
        image(&explicit, "standard/A");
        image(&fsldir.join("data"), "standard/A");
        image(&fsldir.join("data"), "standard/B");
        image(&bundled, "standard/A");
        image(&bundled, "standard/C");
        let s = |e: Option<&std::path::Path>, f: Option<&std::path::Path>| {
            let mut s = Settings::new();
            if let Some(e) = e {
                s = s.with(ENV, &e.to_string_lossy());
            }
            if let Some(f) = f {
                s = s.with("FSLDIR", &f.to_string_lossy());
            }
            s
        };
        let b = Some(bundled.clone());
        let got = find_in("standard/A", &s(Some(&explicit), Some(&fsldir)), b.clone()).unwrap();
        assert_eq!(got.source, Source::Explicit);
        let e = find_in("standard/B", &s(Some(&explicit), Some(&fsldir)), b.clone()).unwrap_err();
        assert!(e.contains("is not in LARMORX_FSL_DATA="), "{e}");
        let got = find_in("standard/A", &s(None, Some(&fsldir)), b.clone()).unwrap();
        assert_eq!(
            (got.source, got.name),
            (
                Source::Fsldir,
                format!("{}/data/standard/A", fsldir.to_string_lossy())
            )
        );
        let got = find_in("standard/C", &s(None, Some(&fsldir)), b.clone()).unwrap();
        assert_eq!(got.source, Source::Bundled);
        let got = find_in("standard/C", &s(None, None), b.clone()).unwrap();
        assert_eq!(got.source, Source::Bundled);
        let e = find_in("standard/D", &s(None, None), None).unwrap_err();
        assert!(e.contains("FSLDIR (not set)"), "{e}");
        assert!(e.contains("not installed beside this program"), "{e}");
    }
}

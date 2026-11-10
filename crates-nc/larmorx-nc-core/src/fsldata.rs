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
//!    larmorx-nc, from FSL 6.0.7.23's fsl-data_standard 2208.0). The files are committed in
//!    `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/` (its README, NOTICE, LICENSE and
//!    MANIFEST.tsv; [`BUNDLED_IMAGES`] the images) and are maturin's wheel data.
//!
//! When none has it, the script refuses before it runs anything, naming the file and every
//! place looked ([`find`]).
//!
//! The copy also holds FSL's `topup` settings files ([`BUNDLED_SETTINGS`], under
//! `etc/flirtsch/` as in `${FSLDIR}`), which topup's `--config` looks for after FSL's own
//! places.

use std::path::PathBuf;

use crate::avwutils;
use crate::settings::Settings;

/// The environment variable of larmorx's explicit data directory.
pub const ENV: &str = "LARMORX_FSL_DATA";

/// The images larmorx-nc's own copy holds (relative to FSL's `data` directory, without their
/// `.nii.gz`): every one FSL's scripts that larmorx-nc replicates read, and no other
/// (`MANIFEST.tsv` beside them lists them with their sizes and SHA-256s).
pub const BUNDLED_IMAGES: [&str; 3] = [
    "standard/MNI152_T1_2mm",
    "standard/MNI152_T1_2mm_brain_mask_dil",
    "standard/MNI152_T1_2mm_eye_mask",
];

/// FSL's `topup` settings files the copy holds (relative to the copy, as they are relative to
/// `${FSLDIR}`), from FSL 6.0.7.23's fsl-topup 2203.5 (the user's decision of 2026-11-09): the
/// files `topup --config=<name>` reads when neither the name nor `${FSLDIR}/etc/flirtsch/`
/// holds them (larmorx-nc-topup's `existing_conf_file`), and the three sdcflows 2.17.1 chooses
/// among for a PEPOLAR estimate (`b02b0_1`, `_2`, `_4`; larmorx's `lx.sdcflows`).
pub const BUNDLED_SETTINGS: [&str; 6] = [
    "etc/flirtsch/b02b0.cnf",
    "etc/flirtsch/b02b0_1.cnf",
    "etc/flirtsch/b02b0_2.cnf",
    "etc/flirtsch/b02b0_4.cnf",
    "etc/flirtsch/b02b0_7T.cnf",
    "etc/flirtsch/test_acqp.cnf",
];

/// The data directory larmorx-nc's wheel installs beside its program, if the program runs from
/// an installation (`<prefix>/bin/larmorx-nc` or `<prefix>\Scripts\larmorx-nc.exe`).
pub fn bundled() -> Option<PathBuf> {
    bundled_for(&std::env::current_exe().ok()?)
}

/// [`bundled`] for the program at `exe`. pip installs a wheel's data under the scheme's `data`
/// directory and its program under `scripts`: `<prefix>` and `<prefix>/bin` (or
/// `<prefix>\Scripts`) in every scheme but Windows' per-user one, whose `scripts` is
/// `<userbase>\Python3XY\Scripts` and `data` `<userbase>`: there the copy is one level higher.
pub fn bundled_for(exe: &std::path::Path) -> Option<PathBuf> {
    let at = |prefix: &std::path::Path| {
        let dir = prefix.join("share").join("larmorx-nc").join("fsl-data");
        dir.is_dir().then_some(dir)
    };
    let bin = exe.parent()?;
    let prefix = bin.parent()?;
    at(prefix).or_else(|| {
        if bin.file_name()?.eq_ignore_ascii_case("Scripts") {
            at(prefix.parent()?)
        } else {
            None
        }
    })
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

    /// The copy is found beside `<prefix>/bin`, `<prefix>\Scripts` and, for Windows' per-user
    /// scheme, `<userbase>\Python3XY\Scripts`; not beside any other directory's program.
    #[test]
    fn the_bundled_copy_beside_the_program() {
        let root = tempfile::tempdir().unwrap();
        let share = |prefix: &std::path::Path| {
            let d = prefix.join("share").join("larmorx-nc").join("fsl-data");
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let venv = root.path().join("venv ~1");
        let data = share(&venv);
        for bin in ["bin", "Scripts"] {
            let exe = venv.join(bin).join("larmorx-nc");
            assert_eq!(bundled_for(&exe), Some(data.clone()), "{bin}");
        }
        let user = root.path().join("user");
        let data = share(&user);
        let exe = user
            .join("Python312")
            .join("Scripts")
            .join("larmorx-nc.exe");
        assert_eq!(bundled_for(&exe), Some(data));
        let exe = user.join("Python312").join("bin").join("larmorx-nc");
        assert_eq!(bundled_for(&exe), None);
        let exe = root.path().join("elsewhere").join("larmorx-nc");
        assert_eq!(bundled_for(&exe), None);
    }

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

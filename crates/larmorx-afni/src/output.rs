// SPDX-License-Identifier: Apache-2.0
//! The output's name and how it is written (`specs/3dTshift.md` §6.3 and §7.1).
//!
//! - AFNI's name rule ([`refused`]), applied at the option scan and when the file is written
//!   (AFNI-12 refuses names that larmorx writes by default);
//! - `AFNI_DECONFLICT`'s renaming ([`renamed`]: `out_AA1.nii`, ...);
//! - whether AFNI's NIfTI writer could open the output ([`cannot_open`]; AFNI writes nothing
//!   and exits with status 0 when it cannot, AFNI-26);
//! - the cross-platform write ([`write_replacing`]): a temporary file in the output's
//!   directory, renamed over the output when complete, so an existing output is replaced
//!   whole or left as it was. On Windows a rename fails while another program holds the
//!   target open without allowing its deletion; it is tried again for about 2 s.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// AFNI's name rule (spec §6.3): a name is refused when it is empty or contains a control
/// character, DEL, a blank, one of `! " & ' ( ) * ; < > ? [ ] { } |` or a byte above 127,
/// except that a name of more than 6 characters that begins with `3dcalc` is never refused.
/// With `AFNI_ALLOW_ARBITRARY_FILENAMES` yes (`arbitrary`) only an empty name is.
pub fn refused(name: &str, arbitrary: bool) -> bool {
    if name.is_empty() {
        return true;
    }
    if arbitrary || (name.len() > 6 && name.starts_with("3dcalc")) {
        return false;
    }
    name.bytes()
        .any(|b| b < 32 || b == 127 || b > 127 || b" !\"&'()*;<>?[]{}|".contains(&b))
}

/// The directory separators of this system: `/`, and on Windows `\` too (AFNI, a Unix
/// program, knows only `/`).
fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// The name AFNI writes for a `-prefix`: the prefix when it contains a `/`, else `./` and the
/// prefix.
pub fn afni_name(prefix: &str) -> String {
    if prefix.contains(is_separator) {
        prefix.to_owned()
    } else {
        format!("./{prefix}")
    }
}

/// AFNI's second check, when the file is written: [`refused`], or a name that begins with `-`.
pub fn refused_when_written(name: &str, arbitrary: bool) -> bool {
    refused(name, arbitrary) || name.starts_with('-')
}

/// The last component of a path (AFNI's conflict messages name the file without its
/// directory).
pub fn file_name(prefix: &str) -> &str {
    prefix.rsplit(is_separator).next().unwrap_or(prefix)
}

/// `prefix` split before its `.nii` or `.nii.gz` (any case).
fn split_suffix(prefix: &str) -> (&str, &str) {
    let lower = prefix.to_ascii_lowercase();
    let n = if lower.ends_with(".nii.gz") {
        7
    } else if lower.ends_with(".nii") {
        4
    } else {
        0
    };
    prefix.split_at(prefix.len() - n)
}

/// Whether the output exists for AFNI (spec §7.1, `AFNI_DECONFLICT`): a file of that name. A
/// directory of that name does not count (observed): AFNI then tries to write the output and
/// cannot open it (AFNI-26, [`cannot_open`]).
pub fn exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().is_file()
}

/// `AFNI_DECONFLICT`'s new name for an existing output (spec §7.1): the first of
/// `out_AA1.nii`, ..., `out_AA9.nii`, `out_AB1.nii`, ..., `out_ZZ9.nii` that is not a file (a
/// directory of that name is chosen, observed).
pub fn renamed(prefix: &str) -> Option<String> {
    let (stem, suffix) = split_suffix(prefix);
    for a in b'A'..=b'Z' {
        for b in b'A'..=b'Z' {
            for d in 1..=9 {
                let name = format!("{stem}_{}{}{d}{suffix}", a as char, b as char);
                if !exists(&name) {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// The failures that mean the output cannot be opened for writing: a missing directory, a
/// path through a file, a directory of the output's name, no permission, a read-only file
/// system.
fn refuses(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::NotFound
            | ErrorKind::NotADirectory
            | ErrorKind::IsADirectory
            | ErrorKind::PermissionDenied
            | ErrorKind::ReadOnlyFilesystem
    )
}

/// Why AFNI's NIfTI writer could not open `path` for writing, if it could not (spec §7.1, "A
/// failed write", AFNI-26): `path` is a directory, an existing file that may not be written,
/// or a new file that cannot be created in its directory (missing, not a directory, or not
/// writable). Nothing on disk changes: an existing file is opened for writing without
/// truncation (and closed), and the probe file created in the directory of a new one is
/// removed again. Other failures (on Windows, a file another program holds open) are left to
/// the write, which waits for them or fails.
pub fn cannot_open(path: &Path) -> Option<std::io::Error> {
    if path.is_dir() {
        return Some(std::io::Error::new(
            ErrorKind::IsADirectory,
            "a directory of that name exists",
        ));
    }
    if path.is_file() {
        return std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .err()
            .filter(refuses);
    }
    if path.symlink_metadata().is_ok() {
        // Something else of that name (a dangling link, a device): left to the write.
        return None;
    }
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let pid = std::process::id();
    for k in 0..1000u32 {
        let probe = dir.join(format!(".{name}.larmorx-probe-{pid}-{k}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(file) => {
                drop(file);
                let _ = std::fs::remove_file(&probe);
                return None;
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Some(e).filter(refuses),
        }
    }
    None
}

/// The output could not be written.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The writer failed (on the temporary file, which is removed).
    #[error("{0}")]
    Write(String),
    /// The temporary file could not be renamed over the output, even after retrying.
    #[error(
        "can't replace '{target}' ({source}), even after retrying for 2 s: another program may \
         hold it open. Nothing was written."
    )]
    Rename {
        target: String,
        #[source]
        source: std::io::Error,
    },
}

/// A fresh hidden name in `target`'s directory that ends like `target` (`.nii` or `.nii.gz`).
fn temporary_name(target: &Path) -> PathBuf {
    let dir = target.parent().unwrap_or(Path::new(""));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let (_, suffix) = split_suffix(&name);
    let suffix = if suffix.is_empty() { ".nii" } else { suffix };
    let pid = std::process::id();
    (0u64..)
        .map(|k| dir.join(format!(".{name}.larmorx-{pid}-{k}{suffix}")))
        .find(|p| !p.exists())
        .expect("a free name")
}

/// The pauses between attempts to rename on Windows: growing, about 2 s in all.
pub const RETRY_PAUSES_MS: [u64; 8] = [10, 20, 40, 80, 160, 320, 640, 730];

/// Renames `from` over `to`, trying again after each of `pauses` while it fails (Windows:
/// another program holds the target open).
pub fn rename_with_retries(
    from: &Path,
    to: &Path,
    pauses: &[u64],
    rename: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut result = rename(from, to);
    for &ms in pauses {
        if result.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(ms));
        result = rename(from, to);
    }
    result
}

/// Writes `target` through a temporary file in its directory, renamed over it when complete
/// (spec §7.1, "Writing the output"). `write` writes the file it is given. On failure the
/// temporary file is removed and `target` is left as it was.
pub fn write_replacing(
    target: &Path,
    write: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), WriteError> {
    let tmp = temporary_name(target);
    if let Err(e) = write(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(WriteError::Write(e));
    }
    let pauses: &[u64] = if cfg!(windows) { &RETRY_PAUSES_MS } else { &[] };
    let mut rename = |a: &Path, b: &Path| std::fs::rename(a, b);
    rename_with_retries(&tmp, target, pauses, &mut rename).map_err(|source| {
        let _ = std::fs::remove_file(&tmp);
        WriteError::Rename {
            target: target.display().to_string(),
            source,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afni_name_rule() {
        for name in [
            "out.nii",
            "sub/out.nii",
            "a#$%+,-.:=@\\^_`~.nii",
            "3dcalc a b.nii",
        ] {
            assert!(!refused(name, false), "{name}");
        }
        for name in ["", "a b.nii", "é.nii", "a;b.nii", "a\tb.nii", "x(1).nii"] {
            assert!(refused(name, false), "{name:?}");
        }
        assert!(!refused("a b.nii", true));
        assert!(refused("", true));
        // The second check: the name AFNI writes.
        assert_eq!(afni_name("3dcalc a b.nii"), "./3dcalc a b.nii");
        assert!(refused_when_written(&afni_name("3dcalc a b.nii"), false));
        assert!(!refused_when_written(&afni_name("3dcalc/x y.nii"), false));
        assert!(refused_when_written("-d/out.nii", true));
        assert_eq!(file_name("sub/out.nii"), "out.nii");
    }

    #[test]
    fn renaming() {
        let dir = tempfile::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n).to_string_lossy().into_owned();
        assert_eq!(renamed(&p("out.nii")).unwrap(), p("out_AA1.nii"));
        std::fs::write(p("out_AA1.nii.gz"), b"").unwrap();
        assert_eq!(renamed(&p("out.nii.gz")).unwrap(), p("out_AA2.nii.gz"));
        for d in 2..=9 {
            std::fs::write(p(&format!("out_AA{d}.nii.gz")), b"").unwrap();
        }
        assert_eq!(renamed(&p("out.nii.gz")).unwrap(), p("out_AB1.nii.gz"));
    }

    /// AFNI-26: what AFNI's writer cannot open, found without changing anything on disk.
    #[test]
    fn outputs_that_cannot_be_opened() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("dir with blank é");
        std::fs::create_dir(&dir).unwrap();
        let listing = || {
            let mut v: Vec<_> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            v.sort();
            v
        };
        // A new file in a directory that exists: it can be opened; no probe file is left.
        assert!(cannot_open(&dir.join("out.nii.gz")).is_none());
        assert!(listing().is_empty());
        // An existing file: opened without truncation.
        let file = dir.join("out.nii");
        std::fs::write(&file, b"taken").unwrap();
        assert!(cannot_open(&file).is_none());
        assert_eq!(std::fs::read(&file).unwrap(), b"taken");
        // A missing directory, a path through a file, a directory of the output's name.
        let missing = cannot_open(&dir.join("nodir").join("out.nii")).unwrap();
        assert_eq!(missing.kind(), ErrorKind::NotFound, "{missing}");
        assert!(cannot_open(&file.join("out.nii")).is_some());
        std::fs::create_dir(dir.join("sub.nii")).unwrap();
        let is_dir = cannot_open(&dir.join("sub.nii")).unwrap();
        assert_eq!(is_dir.kind(), ErrorKind::IsADirectory);
        assert_eq!(
            listing(),
            ["out.nii", "sub.nii"].map(std::ffi::OsString::from)
        );
        // Existence for AFNI_DECONFLICT: a file, not a directory.
        assert!(exists(&file) && !exists(dir.join("sub.nii")));
        let p = |n: &str| dir.join(n).to_string_lossy().into_owned();
        std::fs::create_dir(dir.join("out_AA1.nii")).unwrap();
        assert_eq!(renamed(&p("out.nii")).unwrap(), p("out_AA1.nii"));
        // A file that may not be written (unless this runs with the rights to write it anyway).
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions.clone()).unwrap();
        let writable = std::fs::OpenOptions::new().write(true).open(&file).is_ok();
        assert_eq!(cannot_open(&file).is_some(), !writable);
        assert_eq!(std::fs::read(&file).unwrap(), b"taken");
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&file, permissions).unwrap();
    }

    /// A directory that may not be written (Unix permissions; unless this runs with the rights
    /// to write it anyway).
    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_be_written() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("ro");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let writable = std::fs::File::create(dir.join("x")).is_ok();
        let e = cannot_open(&dir.join("out.nii"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(e.is_some(), !writable, "{e:?}");
        if !writable {
            assert_eq!(e.unwrap().kind(), ErrorKind::PermissionDenied);
        }
    }

    #[test]
    fn replacing_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out put é.nii.gz");
        std::fs::write(&target, b"old").unwrap();
        write_replacing(&target, |tmp| {
            assert!(tmp.to_string_lossy().ends_with(".nii.gz"));
            std::fs::write(tmp, b"new").map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        // A failed write leaves the target and no temporary file.
        let err = write_replacing(&target, |tmp| {
            std::fs::write(tmp, b"partial").unwrap();
            Err("disk full".into())
        });
        assert!(matches!(err, Err(WriteError::Write(_))));
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    /// The retry policy, with a rename that fails while "another program holds the target".
    #[test]
    fn renames_are_retried() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut failures = 3;
        let mut calls = 0;
        let mut flaky = |_: &Path, _: &Path| {
            calls += 1;
            if failures > 0 {
                failures -= 1;
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else {
                Ok(())
            }
        };
        assert!(rename_with_retries(a, b, &[0, 0, 0, 0, 0], &mut flaky).is_ok());
        assert_eq!(calls, 4);
        let mut always = |_: &Path, _: &Path| -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        };
        assert!(rename_with_retries(a, b, &[0, 0], &mut always).is_err());
        assert_eq!(RETRY_PAUSES_MS.iter().sum::<u64>(), 2000);
    }
}

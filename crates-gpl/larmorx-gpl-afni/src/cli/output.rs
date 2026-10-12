// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Writing an output file safely, the same way on Linux, macOS and Windows.
//!
//! The file is written under a temporary name in its own directory, then renamed over the
//! output's name: `std::fs::rename` replaces an existing file on all three systems, and a
//! rename within one directory is atomic on each, so the output is never seen half written,
//! and an existing file is either replaced whole or left as it was.
//!
//! On Windows a rename fails while another program holds the target open without allowing it
//! to be deleted (an antivirus scan, a viewer). The rename is then tried again for about two
//! seconds; if the target is still held, the temporary file is removed and the error says so.
//! A failed write also removes the temporary file. Names with blanks or non-ASCII characters
//! need nothing special: the system's own path type is used throughout.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a rename is retried while another program holds the target (Windows).
const RETRY_FOR: Duration = Duration::from_secs(2);

/// The temporary name for `target` in its own directory: hidden (a leading `.`), unique to this
/// process and call, and ending with `target`'s compression suffix (`.nii` or `.nii.gz`), which
/// tells the NIfTI writer whether to compress.
fn temporary_name(target: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let lower = name.to_ascii_lowercase();
    let suffix_len = if lower.ends_with(".nii.gz") {
        7
    } else if lower.ends_with(".nii") {
        4
    } else {
        0
    };
    let (stem, suffix) = name.split_at(name.len() - suffix_len);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = format!(".{stem}.larmorx-{}-{n}.tmp{suffix}", std::process::id());
    target.with_file_name(tmp)
}

/// Whether a failed rename may succeed if tried again: on Windows, the target is held by
/// another program (access denied, sharing violation, lock violation).
fn transient(e: &std::io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// `std::fs::rename`, tried again for [`RETRY_FOR`] while [`transient`].
fn rename_with_retries(from: &Path, to: &Path) -> std::io::Result<()> {
    let deadline = Instant::now() + RETRY_FOR;
    let mut pause = Duration::from_millis(10);
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if transient(&e) && Instant::now() < deadline => {
                std::thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(200));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Writes `target` with `write` (given the temporary name to write to), then renames the
/// result over `target`, replacing a file of that name. On any failure nothing is left behind
/// and `target` is unchanged; the error names what failed.
pub fn replace_atomically<E: std::fmt::Display>(
    target: &Path,
    write: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<(), String> {
    let tmp = temporary_name(target);
    if let Err(e) = write(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = rename_with_retries(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        let held = if transient(&e) {
            " (another program may be holding it open)"
        } else {
            ""
        };
        return Err(format!(
            "cannot replace '{}'{held}: {e}; nothing was written",
            target.display()
        ));
    }
    Ok(())
}

/// The failures that mean AFNI's NIfTI writer cannot open the output: a missing directory, a
/// path through a file, a directory of the output's name, no permission, a read-only file
/// system. On Windows a sharing or lock violation (another program holds the file open) is
/// not one: the write waits for it ([`rename_with_retries`]).
fn refuses(e: &std::io::Error) -> bool {
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return false;
    }
    matches!(
        e.kind(),
        ErrorKind::NotFound
            | ErrorKind::NotADirectory
            | ErrorKind::IsADirectory
            | ErrorKind::PermissionDenied
            | ErrorKind::ReadOnlyFilesystem
    )
}

/// Why AFNI's NIfTI writer could not open `target` (`znzopen(name, "wb")` in
/// `nifti_image_write_engine`, `nifti2_io.c:7908`), if it could not: a directory of that name,
/// an existing file that may not be written, or a new one that cannot be created in its
/// directory. Nothing changes: an existing file is opened without truncation, and a new one is
/// removed again. AFNI then writes nothing and exits with status 0 (AFNI-26).
pub fn cannot_open(target: &Path) -> Option<std::io::Error> {
    if target.is_dir() {
        return Some(std::io::Error::new(
            ErrorKind::IsADirectory,
            "a directory of that name exists",
        ));
    }
    let probe = if target.is_file() {
        std::fs::OpenOptions::new()
            .write(true)
            .open(target)
            .map(drop)
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .and_then(|_| std::fs::remove_file(target))
    };
    probe.err().filter(refuses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cannot_open_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("sübject out.nii");
        assert!(cannot_open(&new).is_none());
        assert!(!new.exists(), "the probe's file is removed again");
        let old = dir.path().join("old.nii");
        std::fs::write(&old, "kept").unwrap();
        assert!(cannot_open(&old).is_none());
        assert_eq!(std::fs::read(&old).unwrap(), b"kept", "not truncated");
        let missing = dir.path().join("no such directory").join("out.nii");
        assert_eq!(cannot_open(&missing).unwrap().kind(), ErrorKind::NotFound);
        assert!(!dir.path().join("no such directory").exists());
        let through_file = old.join("out.nii");
        assert!(cannot_open(&through_file).is_some());
        let a_directory = dir.path().join("out.nii");
        std::fs::create_dir(&a_directory).unwrap();
        assert_eq!(
            cannot_open(&a_directory).unwrap().kind(),
            ErrorKind::IsADirectory
        );
        // A read-only file (the read-only attribute on Windows), unless this user may write it
        // anyway (root).
        let mut readonly = std::fs::metadata(&old).unwrap().permissions();
        readonly.set_readonly(true);
        std::fs::set_permissions(&old, readonly.clone()).unwrap();
        if std::fs::OpenOptions::new().write(true).open(&old).is_err() {
            assert_eq!(
                cannot_open(&old).unwrap().kind(),
                ErrorKind::PermissionDenied
            );
        }
        #[allow(clippy::permissions_set_readonly_false)]
        readonly.set_readonly(false);
        std::fs::set_permissions(&old, readonly).unwrap();
    }

    #[test]
    fn temporary_names_keep_the_suffix_and_the_directory() {
        let t = temporary_name(Path::new("dir with blank/sübject.nii.gz"));
        let name = t.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(".sübject.larmorx-") && name.ends_with(".tmp.nii.gz"),
            "{name}"
        );
        assert_eq!(t.parent(), Some(Path::new("dir with blank")));
        let u = temporary_name(Path::new("out.NII"));
        assert!(u.to_string_lossy().ends_with(".tmp.NII"), "{}", u.display());
        assert_ne!(
            temporary_name(Path::new("a.nii")),
            temporary_name(Path::new("a.nii"))
        );
    }

    #[test]
    fn an_existing_file_is_replaced_whole_or_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("été 1.nii");
        std::fs::write(&target, "old").unwrap();
        replace_atomically(&target, |p| std::fs::write(p, "new")).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let failed = replace_atomically(&target, |p| {
            std::fs::write(p, "partial")?;
            Err::<(), _>(std::io::Error::other("disk full"))
        });
        assert_eq!(failed.unwrap_err(), "disk full");
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        // No temporary file is left behind.
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }
}

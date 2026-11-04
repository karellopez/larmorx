// SPDX-License-Identifier: LicenseRef-FSL AND Apache-2.0
// Translated to Rust from fslpy 3.27.0 (FSL 6.0.7.23): fsl/scripts/imcp.py, immv.py,
// fsl/utils/imcp.py, the file-name functions of fsl/utils/path.py and fsl/data/image.py.
// Copyright the University of Oxford (Paul McCarthy), Apache-2.0. Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the originals (see crates-nc/NOTICE).
//! fslpy's `imcp` and `immv`, as FSL's scripts run them (`imcp <src> <dest>`, one source):
//! the source's name completed (`fixExt`: the extension the file has), the destination given
//! FSL's default output extension (`$FSLOUTPUTTYPE`, `.nii.gz` when unset or unknown), and the
//! file (or the `.hdr`/`.img` pair) copied or moved when its type is the default output type.
//! Every failure prints the exception's text on standard output and exits with status 1, as
//! the scripts' `main` does.
//!
//! A source of another type is converted by fslpy with nibabel (load, then save as the default
//! type); the replica does not convert: it refuses that case with a clear message (none of FSL's
//! scripts the replica runs reaches it with its own temporary files; `standard_space_roi
//! -maskNONE -roiNONE` does with an input of another type than `$FSLOUTPUTTYPE`).

use std::io::Write;
use std::path::Path;

use crate::nifti;
use crate::settings::Settings;

/// `fsl.data.image.ALLOWED_EXTENSIONS`, in fslpy's order.
pub const ALLOWED_EXTENSIONS: [&str; 12] = [
    ".nii.gz", ".nii.zst", ".nii.bz2", ".nii", ".img", ".hdr", ".img.gz", ".hdr.gz", ".img.zst",
    ".hdr.zst", ".img.bz2", ".hdr.bz2",
];

/// `fsl.data.image.FILE_GROUPS`.
pub const FILE_GROUPS: [[&str; 2]; 4] = [
    [".hdr", ".img"],
    [".hdr.gz", ".img.gz"],
    [".hdr.zst", ".img.zst"],
    [".hdr.bz2", ".img.bz2"],
];

/// fslpy's `FileType` names (`FSLOUTPUTTYPE`'s values).
const FILE_TYPES: [&str; 20] = [
    "ANALYZE",
    "NIFTI",
    "NIFTI2",
    "NIFTI_GZ",
    "NIFTI2_GZ",
    "NIFTI_ZST",
    "NIFTI2_ZST",
    "NIFTI_BZ2",
    "NIFTI2_BZ2",
    "NIFTI_PAIR",
    "NIFTI2_PAIR",
    "ANALYZE_GZ",
    "NIFTI_PAIR_GZ",
    "NIFTI2_PAIR_GZ",
    "ANALYZE_ZST",
    "NIFTI_PAIR_ZST",
    "NIFTI2_PAIR_ZST",
    "ANALYZE_BZ2",
    "NIFTI_PAIR_BZ2",
    "NIFTI2_PAIR_BZ2",
];

/// fslpy's `imcp` usage (`print(usage)`: the text, its own last newline included).
pub const IMCP_USAGE: &str = "Usage:
  imcp <file1> <file2>
  imcp <file1> <directory>
  imcp <file1> <file2> ... <fileN> <directory>

Copy images from <file1> to <file2>, or copy all <file>s to <directory>

NB: filenames can be basenames or include an extension.

Recognised file extensions: .nii.gz, .nii.zst, .nii.bz2, .nii, .img, .hdr, .img.gz, .hdr.gz, .img.zst, .hdr.zst, .img.bz2, .hdr.bz2
";

/// A failure: the text the script's `main` prints (`print(str(e))`), and what Python's logging
/// wrote on standard error before it (fslpy's warning of an ambiguous name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub text: String,
    pub warning: Option<String>,
    /// fslpy's conversion of another file type, which the replica does not do (`text` is the
    /// replica's refusal, not fslpy's).
    pub unsupported: bool,
}

fn fail(text: impl Into<String>) -> Failure {
    Failure {
        text: text.into(),
        warning: None,
        unsupported: false,
    }
}

/// `splitExt(name, ALLOWED_EXTENSIONS)`: the first extension of the list the name ends with.
pub fn split_ext(name: &str) -> (&str, &str) {
    for e in ALLOWED_EXTENSIONS {
        if let Some(b) = name.strip_suffix(e) {
            return (b, &name[b.len()..]);
        }
    }
    (name, "")
}

fn is_file(p: &str) -> bool {
    Path::new(p).is_file()
}

fn exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).exists()
}

/// `fsl.data.image.addExt(prefix)` (`mustExist=True`, `unambiguous=True`).
pub fn add_ext(prefix: &str) -> Result<String, Failure> {
    let all: Vec<String> = if !split_ext(prefix).1.is_empty() {
        vec![prefix.to_owned()]
    } else {
        ALLOWED_EXTENSIONS
            .iter()
            .map(|e| format!("{prefix}{e}"))
            .collect()
    };
    let all: Vec<String> = all.into_iter().filter(|p| is_file(p)).collect();
    match all.len() {
        0 => Err(fail(format!(
            "Could not find a supported file with prefix \"{prefix}\""
        ))),
        1 => Ok(all[0].clone()),
        _ => {
            let mut suffixes: Vec<&str> = all.iter().map(|p| split_ext(p).1).collect();
            suffixes.sort_unstable();
            let matches: Vec<usize> = FILE_GROUPS
                .iter()
                .enumerate()
                .filter(|(_, g)| {
                    let mut g = g.to_vec();
                    g.sort_unstable();
                    g == suffixes
                })
                .map(|(i, _)| i)
                .collect();
            if matches.len() != 1 {
                let text = format!("More than one file with prefix \"{prefix}\"");
                // fslpy logs it (Python's last-resort handler: the message on standard error)
                return Err(Failure {
                    warning: Some(text.clone()),
                    text,
                    unsupported: false,
                });
            }
            Ok(format!("{prefix}{}", FILE_GROUPS[matches[0]][0]))
        }
    }
}

/// `fixExt(name)`: the name itself if it exists, else its prefix completed.
pub fn fix_ext(name: &str) -> Result<String, Failure> {
    if exists(name) {
        return Ok(name.to_owned());
    }
    add_ext(split_ext(name).0)
}

/// `getFileGroup(path, ALLOWED_EXTENSIONS, FILE_GROUPS, fullPaths=False, unambiguous)`: the
/// extensions of the files that make up the image.
pub fn file_group(path: &str, unambiguous: bool) -> Result<Vec<String>, Failure> {
    let path = add_ext(path)?;
    let (base, ext) = split_ext(&path);
    let (mut full, mut partial) = (0, 0);
    let mut matched: Vec<[&str; 2]> = Vec::new();
    for g in FILE_GROUPS {
        if !ext.is_empty() && !g.contains(&ext) {
            continue;
        }
        let e: Vec<bool> = g.iter().map(|s| exists(&format!("{base}{s}"))).collect();
        if e.iter().any(|&b| b) {
            partial += 1;
        }
        if e.iter().all(|&b| b) {
            full += 1;
            matched.push(g);
        }
    }
    if partial == 0 {
        return Ok(vec![ext.to_owned()]);
    }
    if full > 1 {
        return Err(fail(format!(
            "Path is part of multiple file groups: {path}"
        )));
    }
    if !unambiguous {
        partial = 0;
    }
    if full == 1 && partial <= 1 {
        return Ok(matched[0].iter().map(|s| (*s).to_owned()).collect());
    }
    if partial > 0 {
        return Err(fail(format!(
            "Path is part of an incomplete file group: {path}"
        )));
    }
    Ok(vec![ext.to_owned()])
}

/// `defaultOutputType()`: `$FSLOUTPUTTYPE` if it names a file type, else `NIFTI_GZ`.
pub fn default_output_type(fsl: &Settings) -> &'static str {
    let t = fsl.get("FSLOUTPUTTYPE").unwrap_or("");
    FILE_TYPES
        .into_iter()
        .find(|n| *n == t)
        .unwrap_or("NIFTI_GZ")
}

/// `defaultExt()`.
pub fn default_ext(fsl: &Settings) -> &'static str {
    match default_output_type(fsl) {
        "NIFTI" | "NIFTI2" => ".nii",
        "NIFTI_GZ" | "NIFTI2_GZ" => ".nii.gz",
        "NIFTI_ZST" | "NIFTI2_ZST" => ".nii.zst",
        "NIFTI_BZ2" | "NIFTI2_BZ2" => ".nii.bz2",
        "ANALYZE" | "NIFTI_PAIR" | "NIFTI2_PAIR" => ".img",
        "ANALYZE_GZ" | "NIFTI_PAIR_GZ" | "NIFTI2_PAIR_GZ" => ".img.gz",
        "ANALYZE_ZST" | "NIFTI_PAIR_ZST" | "NIFTI2_PAIR_ZST" => ".img.zst",
        _ => ".img.bz2",
    }
}

/// `fileType(filename)`: nibabel's class of the image (NIfTI-2 by a 540-byte header, NIfTI-1 by
/// its magic, ANALYZE for a pair without it) with the extension.
pub fn file_type(name: &str, fsl: &Settings) -> Result<String, Failure> {
    let name = add_ext(name)?;
    let ext = split_ext(&name).1.to_owned();
    let cannot = || fail(format!("Cannot work out file type of \"{name}\""));
    let header_file = if ext.starts_with(".img") {
        format!("{}{}", split_ext(&name).0, ext.replacen(".img", ".hdr", 1))
    } else {
        name.clone()
    };
    let h =
        nifti::load_header(Path::new(&header_file), &fsl.open_options()).map_err(|_| cannot())?;
    let class = if h.sizeof_hdr == 540 {
        if !h.magic.starts_with(b"ni2") && !h.magic.starts_with(b"n+2") {
            return Err(cannot());
        }
        "NIFTI2"
    } else if h.magic.starts_with(b"n+1") || h.magic.starts_with(b"ni1") {
        "NIFTI"
    } else if ext.starts_with(".nii") {
        return Err(cannot());
    } else {
        "ANALYZE"
    };
    let suffix = match ext.as_str() {
        ".nii" => "",
        ".nii.gz" => "_GZ",
        ".nii.zst" => "_ZST",
        ".nii.bz2" => "_BZ2",
        ".hdr" | ".img" => "_PAIR",
        ".hdr.gz" | ".img.gz" => "_PAIR_GZ",
        ".hdr.zst" | ".img.zst" => "_PAIR_ZST",
        _ => "_PAIR_BZ2",
    };
    let suffix = if class == "ANALYZE" {
        suffix.strip_prefix("_PAIR").unwrap_or(suffix)
    } else {
        suffix
    };
    Ok(format!("{class}{suffix}"))
}

/// `os.path.abspath`: absolute, `.` and `..` resolved by name.
fn abspath(p: &str) -> String {
    let path = Path::new(p);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = std::path::PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

fn dirname(p: &str) -> String {
    Path::new(p)
        .parent()
        .map_or_else(String::new, |d| d.to_string_lossy().into_owned())
}

/// Whether a directory may be written in (`os.access(d, W_OK | X_OK)`, approximated by its
/// permissions: not read-only).
fn writable_dir(d: &str) -> bool {
    std::fs::metadata(if d.is_empty() { "." } else { d })
        .map(|m| m.is_dir() && !m.permissions().readonly())
        .unwrap_or(false)
}

/// `fsl.utils.imcp.imcp(src, dest, overwrite=True, useDefaultExt=True, move)`.
fn imcp_one(src: &str, dest: &str, fsl: &Settings, mv: bool) -> Result<(), Failure> {
    if dest.ends_with('/') && !Path::new(dest).is_dir() {
        return Err(fail(format!("Directory does not exist: {dest}")));
    }
    let dest = if Path::new(dest).is_dir() {
        let base = Path::new(src)
            .file_name()
            .map_or_else(String::new, |b| b.to_string_lossy().into_owned());
        Path::new(dest).join(base).to_string_lossy().into_owned()
    } else {
        dest.to_owned()
    };
    let mut src = abspath(src);
    let dest = abspath(&dest);
    if split_ext(&src).1.is_empty() {
        src = add_ext(&src)?;
    }
    let (src_base, src_ext) = {
        let (b, e) = split_ext(&src);
        (b.to_owned(), e.to_owned())
    };
    let dest_base = split_ext(&dest).0.to_owned();
    if !exists(&src) {
        return Err(fail(format!(
            "imcp error - source path does not exist: {src}"
        )));
    }
    let src_type = file_type(&src, fsl)?;
    let dest_type = default_output_type(fsl);
    let mut dest_ext = default_ext(fsl).to_owned();
    if src_ext != dest_ext {
        for g in FILE_GROUPS {
            if g.contains(&src_ext.as_str()) && g.contains(&dest_ext.as_str()) {
                dest_ext = src_ext.clone();
                break;
            }
        }
    }
    let dest = format!("{dest_base}{dest_ext}");
    if !writable_dir(&dirname(&dest)) {
        return Err(fail(format!("imcp error - cannot write to {dest}")));
    }
    if mv && !writable_dir(&dirname(&src)) {
        return Err(fail(format!("imcp error - cannot move from {src}")));
    }
    if src_type != dest_type {
        return Err(Failure {
            text: format!(
                "imcp of {src}: fslpy converts a {src_type} image to the output type {dest_type} \
                 with nibabel; converting file types is pending in the larmorx-nc replica (set \
                 FSLOUTPUTTYPE to {src_type}, or convert the image first) (larmorx)"
            ),
            warning: None,
            unsupported: true,
        });
    }
    let exts = file_group(&src, true)?;
    let pairs: Vec<(String, String)> = exts
        .iter()
        .map(|e| (format!("{src_base}{e}"), format!("{dest_base}{e}")))
        .collect();
    for (s, d) in pairs {
        let same = match (std::fs::canonicalize(&s), std::fs::canonicalize(&d)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if mv {
            if same {
                continue;
            }
            if std::fs::rename(&s, &d).is_err() {
                std::fs::copy(&s, &d).map_err(|e| fail(os_error(&e, &d)))?;
                std::fs::remove_file(&s).map_err(|e| fail(os_error(&e, &s)))?;
            }
        } else {
            if same {
                return Err(fail(format!("'{s}' and '{d}' are the same file")));
            }
            std::fs::copy(&s, &d).map_err(|e| fail(os_error(&e, &d)))?;
        }
    }
    Ok(())
}

/// Python's text of an `OSError` on `path`.
fn os_error(e: &std::io::Error, path: &str) -> String {
    let (errno, text) = match e.kind() {
        std::io::ErrorKind::PermissionDenied => (13, "Permission denied"),
        std::io::ErrorKind::NotFound => (2, "No such file or directory"),
        _ => (5, "Input/output error"),
    };
    format!("[Errno {errno}] {text}: '{path}'")
}

/// The scripts' `imcp(*paths)` / `immv(*paths)` for one source: `fixExt`, `removeDuplicates`
/// (its file group), then the copy or move.
pub fn copy(src: &str, dest: &str, fsl: &Settings, mv: bool) -> Result<(), Failure> {
    let src = fix_ext(src)?;
    file_group(&src, false)?;
    imcp_one(&src, dest, fsl, mv)
}

/// `imcp <src> <dest>` (or `immv`) as a script runs it: the program's output and exit status
/// (`print(str(e))` on standard output and status 1 after a failure, fslpy's logged warning on
/// standard error first; the replica's refusal of a conversion on standard error).
pub fn run(
    src: &str,
    dest: &str,
    fsl: &Settings,
    mv: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match copy(src, dest, fsl, mv) {
        Ok(()) => 0,
        Err(f) if f.unsupported => {
            let _ = writeln!(err, "larmorx-nc: error: {}", f.text);
            1
        }
        Err(f) => {
            if let Some(w) = &f.warning {
                let _ = writeln!(err, "{w}");
            }
            let _ = writeln!(out, "{}", f.text);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_types() {
        assert_eq!(split_ext("a/b.nii.gz"), ("a/b", ".nii.gz"));
        assert_eq!(split_ext("a/b.mnc"), ("a/b.mnc", ""));
        let s = |t: &str| Settings::new().with("FSLOUTPUTTYPE", t);
        assert_eq!(default_output_type(&s("NIFTI")), "NIFTI");
        assert_eq!(default_output_type(&s("bogus")), "NIFTI_GZ");
        assert_eq!(default_output_type(&Settings::new()), "NIFTI_GZ");
        for (t, e) in [
            ("NIFTI", ".nii"),
            ("NIFTI2_GZ", ".nii.gz"),
            ("NIFTI_PAIR", ".img"),
            ("NIFTI2_PAIR_GZ", ".img.gz"),
            ("ANALYZE", ".img"),
            ("ANALYZE_GZ", ".img.gz"),
            ("NIFTI_BZ2", ".nii.bz2"),
            ("ANALYZE_ZST", ".img.zst"),
        ] {
            assert_eq!(default_ext(&s(t)), e, "{t}");
        }
    }

    #[test]
    fn names_completed_and_ambiguities() {
        let d = tempfile::tempdir().unwrap();
        let p = |n: &str| d.path().join(n).to_string_lossy().into_owned();
        std::fs::write(p("a.nii.gz"), b"x").unwrap();
        assert_eq!(fix_ext(&p("a")).unwrap(), p("a.nii.gz"));
        assert_eq!(fix_ext(&p("a.nii")).unwrap(), p("a.nii.gz"));
        std::fs::write(p("a.nii"), b"x").unwrap();
        let e = fix_ext(&p("a")).unwrap_err();
        assert_eq!(
            e.text,
            format!("More than one file with prefix \"{}\"", p("a"))
        );
        assert!(e.warning.is_some());
        std::fs::write(p("b.hdr"), b"x").unwrap();
        std::fs::write(p("b.img"), b"x").unwrap();
        assert_eq!(fix_ext(&p("b")).unwrap(), p("b.hdr"));
        assert_eq!(file_group(&p("b.hdr"), true).unwrap(), vec![".hdr", ".img"]);
        std::fs::write(p("c.hdr"), b"x").unwrap();
        let e = file_group(&p("c.hdr"), true).unwrap_err();
        assert!(
            e.text
                .starts_with("Path is part of an incomplete file group"),
            "{e:?}"
        );
        let e = fix_ext(&p("none")).unwrap_err();
        assert!(
            e.text
                .starts_with("Could not find a supported file with prefix")
        );
    }
}

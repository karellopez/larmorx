// SPDX-License-Identifier: Apache-2.0
//! The input dataset's name, as AFNI reads it (`specs/3dTshift.md` §7.1, observed): `~/` starts
//! at `HOME`; a name with a blank is a list of datasets to join in time unless
//! `AFNI_PATH_SPACES_OK` is yes (AFNI-18); `x.nii` is read from `x.nii.gz` when only that
//! exists; and messages name the input by its real path unless `AFNI_NOREALPATH` is yes or
//! the file itself is a symbolic link.

use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use crate::msg::Message;
use crate::settings::AfniSettings;

/// The file to read and the name AFNI gives it in messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub path: PathBuf,
    /// The input's name in AFNI's messages.
    pub shown: String,
}

/// The input could not be opened: AFNI's fatal message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenError(pub String);

/// `name` with a leading `~/` replaced by `HOME`.
fn expand_home(name: &str, settings: &AfniSettings) -> String {
    match (name.strip_prefix("~/"), settings.get("HOME")) {
        (Some(rest), Some(home)) if !home.is_empty() => {
            format!("{}/{rest}", home.trim_end_matches('/'))
        }
        _ => name.to_owned(),
    }
}

/// The file AFNI reads for `name`, and whether it is `name.gz` because `name` (a `.nii`)
/// does not exist.
fn find(name: &str) -> Option<(String, bool)> {
    if Path::new(name).is_file() {
        return Some((name.to_owned(), false));
    }
    let gz = format!("{name}.gz");
    (name.ends_with(".nii") && Path::new(&gz).is_file()).then_some((gz, true))
}

/// The absolute path with `.`, `..` and symbolic links resolved, as the system writes it
/// (without Windows' `\\?\` prefix).
fn real_path(path: &str) -> Option<String> {
    let c = std::fs::canonicalize(path).ok()?;
    let s = c.to_string_lossy().into_owned();
    Some(if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        s
    })
}

/// Resolves the input `name`. AFNI's messages go to `messages`, the AFNI-18 notice to
/// `notices`; an input that cannot be opened is an [`OpenError`] with AFNI's message.
pub fn resolve(
    name: &str,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    messages: &mut Vec<Message>,
    notices: &mut Vec<Notice>,
) -> Result<Input, OpenError> {
    let cant_open = || OpenError(format!("Can't open input dataset '{name}'"));
    let mut wanted = expand_home(name, settings);
    if name.contains(' ') && !settings.yes("AFNI_PATH_SPACES_OK") {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the input name '{name}' has a blank: read as one file; AFNI reads it as a list of \
                 datasets separated by blanks (unless AFNI_PATH_SPACES_OK is YES)"
            ),
            UpstreamDefects::Reproduce => format!(
                "the input name '{name}' has a blank: read as a list of datasets separated by \
                 blanks, as AFNI does (unless AFNI_PATH_SPACES_OK is YES)"
            ),
        };
        notices.push(Notice::new(&defects::AFNI_18, mode, detail));
        if mode.reproduce() {
            let parts: Vec<&str> = wanted.split([' ', '~']).filter(|p| !p.is_empty()).collect();
            let mut failed = false;
            for part in &parts {
                if find(part).is_none() {
                    messages.push(Message::Warning(format!(
                        "THD_open_tcat: can't open dataset {part}"
                    )));
                    failed = true;
                }
            }
            if failed || parts.is_empty() {
                return Err(cant_open());
            }
            if parts.len() > 1 {
                return Err(OpenError(format!(
                    "larmorx: the input name '{name}' is a list of {} datasets to join in time \
                     (AFNI-18), which larmorx does not support",
                    parts.len()
                )));
            }
            wanted = parts[0].to_owned();
        }
    }
    let (file, from_gz) = find(&wanted).ok_or_else(cant_open)?;
    let link = std::fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink());
    let as_given = settings.yes("AFNI_NOREALPATH") || link;
    let shown = if as_given {
        file.clone()
    } else {
        real_path(&file).unwrap_or_else(|| file.clone())
    };
    if from_gz && as_given {
        messages.push(Message::More(format!(
            "reading {file} instead of non-existing {wanted}"
        )));
    }
    Ok(Input {
        path: PathBuf::from(file),
        shown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        std::fs::write(dir.path().join("x.nii.gz"), b"").unwrap();
        std::fs::create_dir(dir.path().join("a b")).unwrap();
        std::fs::write(dir.path().join("a b").join("in.nii"), b"").unwrap();
        let none = AfniSettings::new();
        let run = |name: &str, s: &AfniSettings, mode| {
            let (mut m, mut n) = (Vec::new(), Vec::new());
            let r = resolve(name, s, mode, &mut m, &mut n);
            (r, m, n.into_iter().map(|n| n.defect.id).collect::<Vec<_>>())
        };
        // x.nii read from x.nii.gz; named by its real path, or as given with a message.
        let (r, m, _) = run(&format!("{d}/x.nii"), &none, UpstreamDefects::Fix);
        let input = r.unwrap();
        assert!(input.shown.ends_with("x.nii.gz") && m.is_empty());
        let no_real = AfniSettings::from_pairs([("AFNI_NOREALPATH", "YES")]);
        let (r, m, _) = run(&format!("{d}/x.nii"), &no_real, UpstreamDefects::Fix);
        assert_eq!(r.unwrap().shown, format!("{d}/x.nii.gz"));
        assert_eq!(
            m,
            [Message::More(format!(
                "reading {d}/x.nii.gz instead of non-existing {d}/x.nii"
            ))]
        );
        // A blank: one file by default; AFNI's list reproduced; AFNI-18 named in both modes.
        let blank = format!("{d}/a b/in.nii");
        let (r, m, n) = run(&blank, &none, UpstreamDefects::Fix);
        assert!(r.is_ok() && m.is_empty());
        assert_eq!(n, ["AFNI-18"]);
        let (r, m, n) = run(&blank, &none, UpstreamDefects::Reproduce);
        assert_eq!(
            r,
            Err(OpenError(format!("Can't open input dataset '{blank}'")))
        );
        assert_eq!(
            m,
            [
                Message::Warning(format!("THD_open_tcat: can't open dataset {d}/a")),
                Message::Warning("THD_open_tcat: can't open dataset b/in.nii".into()),
            ]
        );
        assert_eq!(n, ["AFNI-18"]);
        let ok = AfniSettings::from_pairs([("AFNI_PATH_SPACES_OK", "YES")]);
        let (r, _, n) = run(&blank, &ok, UpstreamDefects::Reproduce);
        assert!(r.is_ok() && n.is_empty());
        // A list of one dataset is that dataset.
        let (r, _, _) = run(&format!("{d}/x.nii.gz "), &none, UpstreamDefects::Reproduce);
        assert!(r.is_ok());
        // ~/ starts at HOME.
        let home = AfniSettings::from_pairs([("HOME", d.as_str())]);
        let (r, _, _) = run("~/x.nii.gz", &home, UpstreamDefects::Fix);
        assert!(r.is_ok());
        let (r, _, _) = run(&format!("{d}/missing.nii"), &none, UpstreamDefects::Fix);
        assert!(r.is_err());
    }
}

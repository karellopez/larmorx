// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/thd_filestuff.c (THD_filename_ok), src/thd_writedset.c
// (the AFNI_DECONFLICT part of THD_write_3dim_dataset), src/thd_opendset.c (THD_deconflict_prefix,
// THD_deconflict_nifti), src/afni_environ.c (THD_deathcon, THD_ok_overwrite), src/afni_logger.c
// (the AFNI_ECHO_COMMANDLINE part of AFNI_logger), src/thd_notes.c (tross_commandline),
// src/thd_mastery.c (the blank and "~/" cases of THD_open_dataset), src/thd_opentcat.c (the
// failure path of THD_open_tcat) and src/nifti/nifti2/nifti2_io.c (the message of
// nifti_image_write_engine when it cannot open its output). Major portions Copyright (C) 1994-2000
// Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the
// Creative Commons Attribution 4.0 International licence (CC BY 4.0). afni_logger.c,
// thd_opentcat.c and the NIfTI library carry no such notice: they were written at the NIH and are
// in the public domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05 (first
// translated from AFNI_25.2.09).
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! What the AFNI programs' command lines share: AFNI's messages, its checks of output names,
//! the existing-output rules of `AFNI_DECONFLICT`, writing a dataset, opening the input, and
//! the command-line echo. Moved here from `cli::tshift` (2026-10-23), unchanged in behaviour,
//! for `3dAutomask` and `3dUnifize`.

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};

use crate::cnum::is_space;
use crate::dataset::{self, Dataset, ReadError};
use crate::environ::{Prefiltered, Settings};
use crate::message::{Message, Style};
use crate::oned::decode_string_list;

use super::output::{cannot_open, replace_atomically};

/// How an AFNI program stops: `ERROR_exit` ("** FATAL ERROR: ...") or `ERROR_message`
/// followed by `exit(1)` ("** ERROR: ...").
pub(crate) enum Fail {
    Fatal(String),
    Error(String),
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::Fatal(m)
    }
}

impl From<&str> for Fail {
    fn from(m: &str) -> Fail {
        Fail::Fatal(m.to_owned())
    }
}

pub(crate) struct Io<'a> {
    pub(crate) out: &'a mut dyn Write,
    pub(crate) err: &'a mut dyn Write,
    pub(crate) style: Style,
}

impl<'a> Io<'a> {
    /// AFNI's message style: `AFNI_MESSAGE_PREFIX`, and the coloured labels of `reproduce`
    /// mode unless `AFNI_MESSAGE_COLORIZE` starts with N.
    pub(crate) fn new(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        settings: &Settings,
        mode: UpstreamDefects,
    ) -> Io<'a> {
        Io {
            out,
            err,
            style: Style {
                prefix: settings.get("AFNI_MESSAGE_PREFIX").map(str::to_owned),
                colorize: mode.reproduce() && !settings.no("AFNI_MESSAGE_COLORIZE"),
            },
        }
    }

    /// `printf(...)`.
    pub(crate) fn print(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.out, "{s}");
    }

    /// A message on standard error, as AFNI's routine of its kind prints it.
    pub(crate) fn message(&mut self, m: Message) {
        let _ = self.err.write_all(self.style.format(&m).as_bytes());
    }

    /// `WARNING_message(...)`.
    pub(crate) fn warn(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Warning(s.to_string()));
    }

    /// `ERROR_message(...)` (the run goes on).
    pub(crate) fn error(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Error(s.to_string()));
    }

    /// `INFO_message(...)`.
    pub(crate) fn info(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Info(s.to_string()));
    }

    /// `fprintf(stderr, ...)`: the text as it is.
    pub(crate) fn raw(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Raw(s.to_string()));
    }

    /// An upstream-defect notice, as a warning line (`Notice::line`), which the Python
    /// wrappers turn into `larmorx.UpstreamDefectWarning`s.
    pub(crate) fn notice(&mut self, n: &Notice) {
        self.warn(n.line());
    }
}

/// `THD_filename_ok`: no control characters, blanks, shell metacharacters or non-ASCII bytes,
/// or any name when `AFNI_ALLOW_ARBITRARY_FILENAMES` is YES; a name of more than 6 characters
/// that begins with `3dcalc` always passes. larmorx accepts any `-prefix`; AFNI-12
/// reproduced refuses the names this rejects (`docs/findings/afni-tshift.md`, "Output names").
pub(crate) fn filename_ok(name: &str, settings: &Settings) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.len() > 6 && name.starts_with("3dcalc") {
        return true;
    }
    if settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES") {
        return true;
    }
    !name.bytes().any(|c| {
        c.is_ascii_control() || is_space(c) || b";*?&|\"><'[](){}!".contains(&c) || c & 128 != 0
    })
}

/// Whether `prefix` names a directory: AFNI looks for a `/`; on Windows a `\` or a drive
/// letter does too.
pub(crate) fn has_directory(prefix: &str) -> bool {
    prefix.contains('/')
        || (cfg!(windows) && (prefix.contains('\\') || prefix.as_bytes().get(1) == Some(&b':')))
}

/// The name AFNI gives the output dataset (`DSET_BRIKNAME`, `DSET_HEADNAME`) and hands
/// `THD_write_nifti` for `prefix`: the dataset's directory (`./` when the prefix names none)
/// and its prefix.
pub(crate) fn afni_output_name(prefix: &str) -> String {
    if has_directory(prefix) {
        prefix.to_owned()
    } else {
        format!("./{prefix}")
    }
}

/// `THD_write_nifti`'s own check of the name it writes ([`afni_output_name`]): what
/// [`filename_ok`] accepts, not starting with `-`. A `-prefix` that passes the option scan only
/// through `filename_ok`'s `3dcalc` exception (`3dcalc a b.nii`, written as `./3dcalc a b.nii`),
/// or whose directory starts with `-`, fails here: AFNI then writes nothing, prints "Illegal
/// filename for NIfTI output", and 3dTshift exits with status 0 (AFNI-12).
pub(crate) fn output_name_ok(prefix: &str, settings: &Settings) -> bool {
    let name = afni_output_name(prefix);
    filename_ok(&name, settings) && !name.starts_with('-')
}

/// The output must be a NIfTI file (larmorx's conventions).
pub(crate) fn output_path(prefix: &str) -> Result<(), Fail> {
    let lower = prefix.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(format!(
            "larmorx writes NIfTI only: the -prefix '{prefix}' must end in .nii or .nii.gz"
        )
        .into());
    }
    Ok(())
}

/// `AFNI_logger`: when `AFNI_ECHO_COMMANDLINE` starts with Y, AFNI echoes the command line
/// (`+++++===== <program> ...`, after the global options were taken out; arguments
/// [`filename_ok`] rejects are quoted, their blanks, control and non-ASCII bytes made blanks)
/// once it has added it to `~/.afni.log`, which it does unless `AFNI_DONT_LOGFILE` is YES or
/// `HOME` is not set. larmorx writes no log; it echoes when `HOME` is a directory.
pub(crate) fn echo_command_line(program: &str, pre: &Prefiltered, io: &mut Io<'_>) {
    let s = &pre.settings;
    if !s.yes("AFNI_ECHO_COMMANDLINE") || s.yes("AFNI_DONT_LOGFILE") {
        return;
    }
    if !s.get("HOME").is_some_and(|h| Path::new(h).is_dir()) {
        return;
    }
    let mut line = String::from(program);
    for arg in pre.args.iter().filter(|a| !a.is_empty()) {
        if filename_ok(arg, s) {
            line.push(' ');
            line.push_str(arg);
        } else {
            let cleaned: String = arg
                .bytes()
                .map(|b| {
                    if b.is_ascii_control() || is_space(b) || b >= 128 {
                        ' '
                    } else {
                        char::from(b)
                    }
                })
                .collect();
            line.push_str(&format!(" '{cleaned}'"));
        }
    }
    io.raw(format!("+++++===== {line}\n"));
}

/// What `AFNI_DECONFLICT` lets AFNI do with an output name that is already a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Deconflict {
    /// `OVERWRITE` exactly (what `-overwrite` sets): overwrite, with the warning "Over-writing
    /// dataset" unless `-overwrite` was given.
    Overwrite,
    /// Any other value starting with `O` or `o`: AFNI checks nothing and overwrites silently.
    OverwriteSilently,
    /// A value starting with `Y` or `y`: write `<name>_AA1.nii` (the first free of `_AA1` ...
    /// `_ZZ9`) instead, with a warning.
    Rename,
    /// Unset or anything else: the conflict is an error (AFNI-11).
    Refuse,
}

/// The output dataset: its name and what to do when it exists.
pub(crate) struct Output {
    /// The `-prefix` as given.
    pub(crate) prefix: String,
    /// The file to write.
    pub(crate) path: PathBuf,
    pub(crate) rule: Deconflict,
    /// `-overwrite` was given: no "Over-writing dataset" warning.
    pub(crate) quiet: bool,
    /// `THD_deathcon`: `AFNI_DECONFLICT` starts with an upper-case `N`, which stops a shift
    /// before the work when the output exists.
    pub(crate) deathcon: bool,
}

impl Output {
    pub(crate) fn new(prefix: &str, settings: &Settings, quiet: bool) -> Output {
        let value = settings.get("AFNI_DECONFLICT");
        let first = value.and_then(|v| v.bytes().next());
        let rule = if value == Some("OVERWRITE") {
            Deconflict::Overwrite
        } else {
            match first.map(|c| c.to_ascii_uppercase()) {
                Some(b'O') => Deconflict::OverwriteSilently,
                Some(b'Y') => Deconflict::Rename,
                _ => Deconflict::Refuse,
            }
        };
        Output {
            prefix: prefix.to_owned(),
            path: settings.path(prefix),
            rule,
            quiet,
            deathcon: first == Some(b'N'),
        }
    }

    /// `THD_is_file(path)`: a file of the output's name exists.
    pub(crate) fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// The output's name without its directory (`DSET_PREFIX`).
    pub(crate) fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.prefix.clone(), |n| n.to_string_lossy().into_owned())
    }

    /// The `-prefix` with its file name replaced by that of `path`.
    pub(crate) fn prefix_of(&self, path: &Path) -> String {
        let old = self.file_name();
        let new = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match self.prefix.strip_suffix(&old) {
            Some(dir) => format!("{dir}{new}"),
            None => new,
        }
    }

    /// `THD_deconflict_nifti`: the first of `<name>_AA1<suffix>`, `_AA2`, ..., `_AB1`, ...,
    /// `_ZZ9` that is not a file (the suffix `.nii` or `.nii.gz`); `None` when all are (AFNI
    /// then appends a random identifier).
    pub(crate) fn deconflicted(&self) -> Option<PathBuf> {
        let name = self.file_name();
        let lower = name.to_ascii_lowercase();
        let cut = if lower.ends_with(".nii") {
            name.len() - 4
        } else if lower.ends_with(".nii.gz") {
            name.len() - 7
        } else {
            name.len()
        };
        let (stem, suffix) = name.split_at(cut);
        for a in b'A'..=b'Z' {
            for b in b'A'..=b'Z' {
                for c in b'1'..=b'9' {
                    let candidate = self.path.with_file_name(format!(
                        "{stem}_{}{}{}{suffix}",
                        char::from(a),
                        char::from(b),
                        char::from(c)
                    ));
                    if !candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// The checks before the work. `THD_deathcon` (a shift only, in both modes): with
    /// `AFNI_DECONFLICT` starting with `N`, an existing output stops the run ("output dataset
    /// already exists", exit 1). By default, an existing output that `AFNI_DECONFLICT` does
    /// not allow to overwrite or rename is an error (AFNI-11), where AFNI finds out after the
    /// work, writes nothing and exits with status 0.
    pub(crate) fn check(
        &self,
        shift: bool,
        notices: &Notices,
        io: &mut Io<'_>,
    ) -> Result<(), Fail> {
        if shift && self.deathcon && self.exists() {
            return Err(format!(
                "output dataset already exists '{}'",
                afni_output_name(&self.prefix)
            )
            .into());
        }
        if notices.fix() && self.rule == Deconflict::Refuse && self.exists() {
            io.notice(&Notice::new(
                &defects::AFNI_11,
                notices.mode(),
                "the output exists: an error (exit status 1); AFNI writes nothing and exits \
                 with status 0",
            ));
            return Err(format!(
                "output dataset name '{}' conflicts with existing file",
                self.path.display()
            )
            .into());
        }
        Ok(())
    }
}

/// Writes a dataset (`DSET_write`, `THD_write_3dim_dataset`). Returns the name AFNI then gives
/// the dataset (`DSET_BRIKNAME`), which messages print even when nothing was written, and the
/// file it names. The upstream defects met are added to `notices`; the caller names them.
///
/// An existing output: overwritten, renamed or refused as `AFNI_DECONFLICT` says
/// ([`Deconflict`]); refused, AFNI writes nothing, prints two errors and the run ends with
/// status 0 (AFNI-11 reproduced), and the dataset keeps the free name AFNI found for it. With
/// AFNI-12 reproduced, a name that `THD_write_nifti` refuses ([`output_name_ok`]) is not
/// written either. An output AFNI's writer cannot open ([`cannot_open`]: a missing or
/// read-only directory, a read-only file, a directory of that name) is an error by default,
/// naming AFNI-26; AFNI ignores the write's result and exits with status 0, which `reproduce`
/// gives: the NIfTI library's message (none for a one-row dataset, whose 1D text writer says
/// nothing, AFNI-20), nothing written. A write that fails later is an error in both modes.
pub(crate) fn write_dataset(
    out: &Output,
    ds: &Dataset,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<Written, Fail> {
    let mut target = out.path.clone();
    let mut written = true;
    if out.exists() {
        match out.rule {
            Deconflict::Overwrite => {
                if !out.quiet {
                    io.warn(format!(
                        "Over-writing dataset {}",
                        afni_output_name(&out.prefix)
                    ));
                }
            }
            Deconflict::OverwriteSilently => {}
            Deconflict::Rename => {
                target = out.deconflicted().ok_or_else(|| {
                    Fail::Fatal(format!(
                        "no free name for the output beside '{}' (_AA1 to _ZZ9 all exist)",
                        out.prefix
                    ))
                })?;
                io.warn(format!(
                    "changed output dataset name from '{}' to '{}'",
                    out.file_name(),
                    target.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Deconflict::Refuse => {
                if notices.fix() {
                    // Created during the work; checked before it otherwise.
                    out.check(false, notices, io)?;
                }
                if let Some(free) = out.deconflicted() {
                    target = free;
                }
                io.error(format!(
                    "output dataset name '{}' conflicts with existing file",
                    out.file_name()
                ));
                io.error("dataset NOT written to disk!");
                notices.add(
                    &defects::AFNI_11,
                    "the output exists: nothing is written and the exit status is 0, as AFNI \
                     does",
                );
                written = false;
            }
        }
    }
    let shown = afni_output_name(&out.prefix_of(&target));
    if written && notices.reproduce() && !output_name_ok(&out.prefix_of(&target), settings) {
        io.error(format!("Illegal filename for NIfTI output: {shown}"));
        notices.add(
            &defects::AFNI_12,
            format!(
                "the output name '{shown}' has blanks, shell metacharacters or non-ASCII \
                 characters, or starts with '-': nothing is written and the exit status is 0, \
                 as AFNI does"
            ),
        );
        written = false;
    }
    if written && let Some(e) = cannot_open(&target) {
        if notices.fix() {
            io.notice(&Notice::new(
                &defects::AFNI_26,
                notices.mode(),
                format!(
                    "the output '{shown}' cannot be written ({e}): an error (exit status 1); \
                     AFNI writes nothing and exits with status 0"
                ),
            ));
            return Err(Fail::Fatal(format!(
                "cannot open output file '{shown}' ({e}): nothing was written"
            )));
        }
        // `nifti_image_write_engine` (`nifti2_io.c:7908-7912`, `LNI_FERR`, a plain `fprintf`);
        // `THD_write_nifti` returns the failure and 3dTshift ignores it (`3dTshift.c:696`,
        // `:709`). AFNI's 1D text writer (AFNI-20, `THD_write_1D`) prints nothing.
        let [_, ny, nz] = ds.nxyz;
        if !(ny == 1 && nz == 1 && !settings.yes("AFNI_WRITE_1D_AS_PREFIX")) {
            io.raw(format!(
                "** ERROR (nifti_image_write_engine): cannot open output file '{shown}'\n"
            ));
        }
        notices.add(
            &defects::AFNI_26,
            format!(
                "the output '{shown}' cannot be written ({e}): nothing is written and the exit \
                 status is 0, as AFNI does"
            ),
        );
        written = false;
    }
    if written {
        let mut messages = Vec::new();
        let result = replace_atomically(&target, |tmp| {
            dataset::write(tmp, ds, n_threads, settings, &mut messages, notices)
        });
        for m in messages {
            io.message(m);
        }
        if let Err(e) = result {
            if notices.fix() {
                io.notice(&Notice::new(
                    &defects::AFNI_26,
                    notices.mode(),
                    "the output cannot be written: an error (exit status 1); AFNI writes \
                     nothing and exits with status 0",
                ));
            }
            return Err(Fail::Fatal(format!("can't write output dataset: {e}")));
        }
    }
    Ok(Written {
        shown,
        path: target,
    })
}

/// `THD_open_dataset` for a program's input, with the messages AFNI prints while opening it:
/// the dataset as loaded (`loaded`: 3dAutomask, 3dUnifize) or as 3dTshift's copy.
/// A name with a blank is a list of datasets to join in time unless `AFNI_PATH_SPACES_OK` is
/// YES (AFNI-18): reproduced, each is opened ([`open_list`]); by default the name is one file.
/// A name starting with `~/` starts in `HOME`.
pub(crate) fn open_dataset(
    arg: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
    loaded: bool,
) -> Result<Dataset, ReadError> {
    let listed = arg.contains(' ')
        && !settings.yes("AFNI_PATH_SPACES_OK")
        && !["[count ", "[count_afni ", "[1dcat "]
            .iter()
            .any(|s| arg.contains(s));
    if listed {
        if notices.fix() {
            notices.add(
                &defects::AFNI_18,
                format!(
                    "the input name '{arg}' has a blank: read as one file; AFNI reads it as a \
                     list of datasets separated by blanks (unless AFNI_PATH_SPACES_OK is YES)"
                ),
            );
        } else {
            notices.add(
                &defects::AFNI_18,
                format!(
                    "the input name '{arg}' has a blank: read as a list of datasets separated \
                     by blanks, as AFNI does"
                ),
            );
            return open_list(arg, n_threads, settings, messages, notices, loaded);
        }
    }
    let name = match (arg.strip_prefix("~/"), settings.get("HOME")) {
        (Some(rest), Some(home)) => format!("{home}/{rest}"),
        _ => arg.to_owned(),
    };
    // A run inside a caller's process: relative names start in its own directory.
    let name = settings.path(&name).to_string_lossy().into_owned();
    if loaded {
        dataset::read_loaded(&name, n_threads, settings, messages, notices)
    } else {
        dataset::read(&name, n_threads, settings, messages, notices)
    }
}

/// `THD_open_tcat(list)`: the datasets named in `list` (separated by blanks or `~`), each
/// opened as an input; one that cannot be opened is reported, and then the list cannot be
/// opened. A list of one dataset is that dataset; joining several in time is not supported.
pub(crate) fn open_list(
    list: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
    loaded: bool,
) -> Result<Dataset, ReadError> {
    let names = decode_string_list(list, b"~");
    let mut opened = Vec::new();
    let mut failed = false;
    for name in &names {
        match open_dataset(name, n_threads, settings, messages, notices, loaded) {
            Ok(ds) => opened.push(ds),
            Err(_) => {
                messages.push(Message::Warning(format!(
                    "THD_open_tcat: can't open dataset {name}\n"
                )));
                failed = true;
            }
        }
    }
    if failed || opened.is_empty() {
        return Err(ReadError::Afni(format!(
            "{list}: cannot open every dataset of the list"
        )));
    }
    if opened.len() == 1 {
        return Ok(opened.pop().expect("one dataset"));
    }
    Err(ReadError::Unsupported(format!(
        "{list}: a list of datasets to join in time"
    )))
}

/// What [`write_dataset`] did: the name AFNI gives the dataset (`DSET_BRIKNAME`) and the file
/// it names (the deconflicted name when `AFNI_DECONFLICT` renamed it). `WROTE_DSET` prints the
/// name when that file exists, written now or not.
pub(crate) struct Written {
    pub(crate) shown: String,
    pub(crate) path: PathBuf,
}

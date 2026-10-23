// SPDX-License-Identifier: Apache-2.0
//! What the command lines of 3dAutomask and 3dUnifize share: AFNI's echoed command line, the
//! notices of AFNI's output names (AFNI-11, -12, -20, -26), and writing an output dataset as
//! AFNI does (`specs/3dTshift.md` §6.3 and §7.1; `specs/3dAutomask.md` §8.4).

use std::path::Path;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_io::nifti::NiftiHeader;

use crate::dataset::{self, Bricks};
use crate::msg::Log;
use crate::output::{self, afni_name, file_name, refused, refused_when_written};
use crate::settings::{AfniSettings, Deconflict};

/// How a run fails: a fatal error (exit status 1).
#[derive(Debug)]
pub(crate) enum Failure {
    Fatal(String),
}

/// AFNI's echoed command line (`AFNI_ECHO_COMMANDLINE`): the arguments after the global
/// options were taken out, each one the output-name rule refuses quoted, its blanks, control
/// characters and non-ASCII bytes made blanks.
pub(crate) fn echo_line(program: &str, args: &[String], settings: &AfniSettings) -> String {
    let arbitrary = settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES");
    let mut line = format!("+++++===== {program}");
    for a in args {
        line.push(' ');
        if refused(a, arbitrary) {
            let cleaned: String = a
                .bytes()
                .map(|b| {
                    if b <= 32 || b >= 127 {
                        ' '
                    } else {
                        char::from(b)
                    }
                })
                .collect();
            line.push('\'');
            line.push_str(&cleaned);
            line.push('\'');
        } else {
            line.push_str(a);
        }
    }
    line
}

/// Whether AFNI echoes the command line (`AFNI_ECHO_COMMANDLINE` yes, `HOME` a directory,
/// `AFNI_DONT_LOGFILE` not yes).
pub(crate) fn echoes(settings: &AfniSettings) -> bool {
    settings.yes("AFNI_ECHO_COMMANDLINE")
        && settings.get("HOME").is_some_and(|h| Path::new(h).is_dir())
        && !settings.yes("AFNI_DONT_LOGFILE")
}

/// The AFNI-11 notice: the output exists.
pub(crate) fn afni_11(mode: UpstreamDefects) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => {
            "the output exists: an error (exit status 1); AFNI writes nothing and exits with \
             status 0"
        }
        UpstreamDefects::Reproduce => {
            "the output exists: nothing is written and the exit status is 0, as AFNI does"
        }
    };
    Notice::new(&defects::AFNI_11, mode, detail)
}

/// The AFNI-12 notice: AFNI refuses the output's name at the option scan (`first`, option
/// `option`) or when it writes the file.
pub(crate) fn afni_12(option: &str, name: &str, first: bool, mode: UpstreamDefects) -> Notice {
    let detail = match (first, mode) {
        (true, UpstreamDefects::Fix) => format!(
            "the {option} '{name}' has blanks, shell metacharacters or non-ASCII characters: \
             accepted; AFNI refuses it"
        ),
        (true, UpstreamDefects::Reproduce) => format!(
            "the {option} '{name}' has blanks, shell metacharacters or non-ASCII characters: \
             refused, as AFNI does"
        ),
        (false, UpstreamDefects::Fix) => format!(
            "the output name '{name}' has blanks, shell metacharacters or non-ASCII characters, \
             or starts with '-': written; AFNI accepts the {option} but writes nothing (exit \
             status 0)"
        ),
        (false, UpstreamDefects::Reproduce) => format!(
            "the output name '{name}' has blanks, shell metacharacters or non-ASCII characters, \
             or starts with '-': nothing is written and the exit status is 0, as AFNI does"
        ),
    };
    Notice::new(&defects::AFNI_12, mode, detail)
}

/// The AFNI-26 notice: AFNI's NIfTI writer cannot open the output `name` (`reason`).
pub(crate) fn afni_26(name: &str, reason: &str, mode: UpstreamDefects) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => format!(
            "the output '{name}' cannot be written ({reason}): an error (exit status 1); AFNI \
             writes nothing and exits with status 0"
        ),
        UpstreamDefects::Reproduce => format!(
            "the output '{name}' cannot be written ({reason}): nothing is written and the exit \
             status is 0, as AFNI does"
        ),
    };
    Notice::new(&defects::AFNI_26, mode, detail)
}

/// The output names larmorx writes: NIfTI only.
pub(crate) fn check_nifti(option: &str, name: &str) -> Result<(), Failure> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".nii") || lower.ends_with(".nii.gz") {
        Ok(())
    } else {
        Err(Failure::Fatal(format!(
            "larmorx writes NIfTI only: the {option} '{name}' must end in .nii or .nii.gz"
        )))
    }
}

/// AFNI-11 corrected: an output that exists is an error before the work, unless
/// `AFNI_DECONFLICT` says to replace or rename it.
pub(crate) fn refuse_existing(
    prefix: &str,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    log: &mut Log<'_>,
) -> Result<(), Failure> {
    let deconflict = Deconflict::from_settings(settings);
    if mode.fix()
        && output::exists(prefix)
        && matches!(deconflict, Deconflict::Unset | Deconflict::Stop)
    {
        log.notice(&afni_11(mode));
        return Err(Failure::Fatal(format!(
            "output dataset name '{}' conflicts with existing file",
            file_name(prefix)
        )));
    }
    Ok(())
}

/// An output to write.
pub(crate) struct Target<'a> {
    /// The option that named it (`-prefix`), for messages.
    pub option: &'a str,
    pub prefix: &'a str,
    /// AFNI refused the name at the option scan (AFNI-12, named there).
    pub refused_at_scan: bool,
}

/// Writes `bricks` with `header` to `target` as AFNI does with an existing output
/// (`AFNI_DECONFLICT`, AFNI-11), a name it refuses when writing (AFNI-12) and an output its
/// writer cannot open (AFNI-26); `notices` collects the defects named. Returns the name AFNI
/// gives the file and its path when a write was attempted (AFNI then prints `Output dataset`
/// if the file exists), `None` when nothing was attempted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_dataset(
    target: &Target<'_>,
    header: &NiftiHeader,
    bricks: &Bricks,
    n_threads: usize,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    log: &mut Log<'_>,
    notices: &mut Vec<Notice>,
) -> Result<Option<(String, String)>, Failure> {
    let prefix = target.prefix;
    let mut path = prefix.to_owned();
    let mut name = afni_name(prefix);
    if output::exists(prefix) {
        match Deconflict::from_settings(settings) {
            Deconflict::OverwriteExact => {
                if !settings.overwrite_option() {
                    log.warning(&format!("Over-writing dataset {name}"));
                }
            }
            Deconflict::Overwrite => {}
            Deconflict::Rename => {
                let new = output::renamed(prefix).ok_or_else(|| {
                    Failure::Fatal(format!("no free name to rename the output '{prefix}' to"))
                })?;
                log.warning(&format!(
                    "changed output dataset name from '{}' to '{}'",
                    file_name(prefix),
                    file_name(&new)
                ));
                name = afni_name(&new);
                path = new;
            }
            Deconflict::Stop | Deconflict::Unset => {
                let conflict = format!(
                    "output dataset name '{}' conflicts with existing file",
                    file_name(prefix)
                );
                if mode.fix() {
                    log.notice(&afni_11(mode));
                    return Err(Failure::Fatal(conflict));
                }
                notices.push(afni_11(mode));
                log.error(&conflict);
                log.error("dataset NOT written to disk!");
                return Ok(None);
            }
        }
    }
    if refused_when_written(&name, settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES")) {
        if !target.refused_at_scan {
            notices.push(afni_12(target.option, &name, false, mode));
        }
        if mode.reproduce() {
            log.error(&format!("Illegal filename for NIfTI output: {name}"));
            return Ok(None);
        }
    }
    let [nx, ny, nz, _] = bricks.shape;
    let one_row_text = ny == 1 && nz == 1 && !settings.yes("AFNI_WRITE_1D_AS_PREFIX");
    if one_row_text {
        notices.push(Notice::new(
            &defects::AFNI_20,
            UpstreamDefects::Fix,
            format!(
                "the dataset has one row ({nx} x 1 x 1 voxels): written as NIfTI, in both \
                 modes; AFNI writes a 1D text file under that name (unless \
                 AFNI_WRITE_1D_AS_PREFIX is YES), with a random identifier"
            ),
        ));
    }
    if let Some(e) = output::cannot_open(Path::new(&path)) {
        let notice = afni_26(&name, &e.to_string(), mode);
        if mode.fix() {
            log.notice(&notice);
            return Err(Failure::Fatal(format!(
                "cannot open output file '{name}' ({e}): nothing was written"
            )));
        }
        notices.push(notice);
        if !one_row_text {
            log.raw(&format!(
                "** ERROR (nifti_image_write_engine): cannot open output file '{name}'"
            ));
        }
        return Ok(Some((name, path)));
    }
    output::write_replacing(Path::new(&path), |tmp| {
        dataset::write(tmp, header, bricks, n_threads).map_err(|e| e.to_string())
    })
    .map_err(|e| Failure::Fatal(format!("can't write the output '{path}': {e}")))?;
    Ok(Some((name, path)))
}

/// The `Output dataset` line AFNI prints after an attempted write, when the file exists.
pub(crate) fn output_line(written: &Option<(String, String)>) -> Option<String> {
    written
        .as_ref()
        .filter(|(_, path)| Path::new(path).is_file())
        .map(|(name, _)| format!("Output dataset {name}"))
}

/// C's `strtod` of the whole start of `s`: the leading number, or 0 when there is none.
pub(crate) fn strtod(s: &str) -> f64 {
    super::leading_number(s).map_or(0.0, |(v, _)| v)
}

/// C's `strtol(s, NULL, 10)` converted to `int`: leading blanks, a sign, digits (0 when
/// none), the `long` clamped to its range and cut to its low 32 bits.
pub(crate) fn strtol_int(s: &str) -> i32 {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let b = t.as_bytes();
    let mut i = 0;
    let negative = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut value: i128 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        value = (value * 10 + i128::from(b[i] - b'0')).min(i128::from(i64::MAX) + 1);
        i += 1;
    }
    let value = if negative { -value } else { value };
    let long = value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    long as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_conversions() {
        assert_eq!(strtod("1.5e1x"), 15.0);
        assert_eq!(strtod("abc"), 0.0);
        assert_eq!(strtol_int("  -3"), -3);
        assert_eq!(strtol_int("+12abc"), 12);
        assert_eq!(strtol_int("x"), 0);
        // The low 32 bits of the long.
        assert_eq!(strtol_int("4294967297"), 1);
        assert_eq!(strtol_int("99999999999999999999"), -1);
    }

    #[test]
    fn echoed_command_lines_quote_refused_names() {
        let args: Vec<String> = ["-prefix", "a b.nii", "in.nii"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            echo_line("3dAutomask", &args, &AfniSettings::new()),
            "+++++===== 3dAutomask -prefix 'a b.nii' in.nii"
        );
    }
}

// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's version stamps and its `--version` and `--all-info` options (`fs.core`, queue
//! item 3.1-22), translated from FreeSurfer 8.2.0's `utils/version.cpp` (`argv2cmdline`,
//! `VERuser`, `VERfileTimeStamp`, `VERcurTimeStamp`, `getVersion`, `getBuildStamp`,
//! `handleVersionOption`, `getAllInfo`). The stamps are the official 8.2.0 build's (its
//! `version_info.h` and `__DATE__ __TIME__`, read from the official programs' `--all-info`):
//! the replica reproduces those programs.
//!
//! **FS-07** (corrected by default): `handleVersionOption` removes each version option by moving
//! the later arguments one place back, but its loop goes on at the next place, so it never looks
//! at the argument that moved into the removed one's place, and it looks again at the copies left
//! at the end: `mri_info --version --version x` keeps the second `--version`, and
//! `mri_info --versionx -version --all-info` skips `-version` and prints the `--all-info` report
//! twice (official `mri_info`). The default looks at each argument once, in order, and reports the
//! command line as given; `reproduce` moves and looks as FreeSurfer does.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_07;

use crate::ctime;
use crate::globals::Globals;
use crate::run::Environment;
use crate::system;
use crate::utils::resolve;

/// `FS_VERSION`.
pub const FS_VERSION: &str = "8.2.0";
/// `FS_BUILD_STAMP`.
pub const FS_BUILD_STAMP: &str = "freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45";
/// `__DATE__ " " __TIME__` of the official build's `version.cpp`.
pub const BUILD_TIME: &str = "Mar 14 2026 12:21:06";
/// `PLATFORM`, `COMPILER_NAME`, `COMPILER_VERSION` of the official build.
pub const PLATFORM: &str = "Linux";
pub const COMPILER_NAME: &str = "GCC";
pub const COMPILER_VERSION: &str = "8.4.0";

/// `getVersion()`.
pub fn get_version() -> &'static str {
    FS_VERSION
}

/// `getBuildStamp()`.
pub fn get_build_stamp() -> &'static str {
    FS_BUILD_STAMP
}

/// `argv2cmdline(argc, argv)`: every argument followed by one space.
pub fn argv2cmdline(argv: &[String]) -> String {
    argv.iter().map(|a| format!("{a} ")).collect()
}

/// `VERuser()`.
pub fn ver_user(env: &Environment) -> String {
    system::user_name(env)
}

/// `VERfileTimeStamp(fname)`: the file's modification time, local, `YYYY/MM/DD hh:mm:ss`; `None`
/// after `ERROR: stating file <fname>` on standard output when it cannot be read.
pub fn ver_file_time_stamp(
    env: &Environment,
    dir: Option<&Path>,
    out: &mut dyn Write,
    fname: &str,
) -> Option<String> {
    let mtime = std::fs::metadata(resolve(dir, fname))
        .and_then(|m| m.modified())
        .ok()
        .map(|t| match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs_f64().ceil() as i64),
        });
    let Some(t) = mtime else {
        let _ = writeln!(out, "ERROR: stating file {fname}");
        return None;
    };
    let tm = ctime::localtime(env, t);
    Some(format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
        tm.year + 1900,
        tm.mon + 1,
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec
    ))
}

/// `VERcurTimeStamp()` at time `t`: `20YY/MM/DD-hh:mm:ss-GMT` (the year's last two digits after
/// `20`).
pub fn ver_cur_time_stamp_at(t: i64) -> String {
    let tm = ctime::gmtime(t);
    format!(
        "20{:02}/{:02}/{:02}-{:02}:{:02}:{:02}-GMT",
        tm.year % 100,
        tm.mon + 1,
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec
    )
}

/// `VERcurTimeStamp()` now.
pub fn ver_cur_time_stamp() -> String {
    ver_cur_time_stamp_at(ctime::now())
}

/// `getAllInfo(argc, argv, progname)` at time `t`: one line of `Name: value` pairs, two spaces
/// apart.
pub fn get_all_info_at(env: &Environment, argv: &[String], progname: &str, t: i64) -> String {
    let (machine, platform_version) = match system::uname(env) {
        Some(u) => (u.nodename, u.release),
        None => ("UNKNOWN".to_owned(), "UNKNOWN".to_owned()),
    };
    format!(
        "ProgramName: {progname}  ProgramArguments: {}  ProgramVersion: {}  TimeStamp: {}  \
         BuildTime: {BUILD_TIME}  BuildStamp: {}  User: {}  Machine: {machine}  Platform: \
         {PLATFORM}  PlatformVersion: {platform_version}  CompilerName: {COMPILER_NAME}  \
         CompilerVersion: {COMPILER_VERSION}",
        argv.join(" "),
        get_version(),
        ver_cur_time_stamp_at(t),
        get_build_stamp(),
        system::user_name(env),
    )
}

/// `getAllInfo` now.
pub fn get_all_info(env: &Environment, argv: &[String], progname: &str) -> String {
    get_all_info_at(env, argv, progname, ctime::now())
}

/// What `handleVersionOption` prints, in order: the version line, or the `--all-info` report of
/// the arguments it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionReport {
    Version,
    AllInfo(Vec<String>),
}

/// `--version` and `-version` match as prefixes (`strncmp` over their length); `--all-info` and
/// `-all-info` exactly.
fn version_kind(a: &str) -> Option<bool> {
    if a.starts_with("--version") || a.starts_with("-version") {
        Some(false)
    } else if a == "--all-info" || a == "-all-info" {
        Some(true)
    } else {
        None
    }
}

/// FreeSurfer's loop: the reports and the arguments left (`argv` with `argc - nargs` kept).
fn plan_freesurfer(argv: &[String]) -> (Vec<VersionReport>, Vec<String>) {
    let mut a = argv.to_vec();
    let argc = a.len();
    let mut num = 0;
    let mut reports = Vec::new();
    for narg in 1..argc {
        match version_kind(&a[narg]) {
            Some(false) => reports.push(VersionReport::Version),
            Some(true) => reports.push(VersionReport::AllInfo(a.clone())),
            None => continue,
        }
        num += 1;
        for nnarg in narg..argc - num {
            a[nnarg] = a[nnarg + 1].clone();
        }
    }
    a.truncate(argc - num);
    (reports, a)
}

/// The corrected loop: each argument once; the reports show the command line as given.
fn plan_fixed(argv: &[String]) -> (Vec<VersionReport>, Vec<String>) {
    let mut reports = Vec::new();
    let mut left = argv.first().cloned().into_iter().collect::<Vec<_>>();
    for a in argv.iter().skip(1) {
        match version_kind(a) {
            Some(false) => reports.push(VersionReport::Version),
            Some(true) => reports.push(VersionReport::AllInfo(argv.to_vec())),
            None => left.push(a.clone()),
        }
    }
    (reports, left)
}

/// `handleVersionOption(argc, argv, progname)` on `argv` (with the program's name first): prints
/// `<progname> freesurfer 8.2.0` for each version option and the `--all-info` report for each
/// `--all-info` on standard output; returns the number of options handled and the arguments left
/// (FS-07).
pub fn handle_version_option(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    argv: &[String],
    progname: &str,
) -> (i32, Vec<String>) {
    let freesurfer = plan_freesurfer(argv);
    let fixed = plan_fixed(argv);
    if freesurfer != fixed {
        g.notices.add(
            &FS_07,
            "the version options: each argument looked at once, the command line as given",
        );
    }
    let (reports, left) = if g.notices.reproduce() {
        freesurfer
    } else {
        fixed
    };
    for r in &reports {
        match r {
            VersionReport::Version => {
                let _ = writeln!(out, "{progname} freesurfer {FS_VERSION}");
            }
            VersionReport::AllInfo(a) => {
                let _ = writeln!(out, "{}", get_all_info(env, a, progname));
            }
        }
    }
    let _ = out.flush();
    (reports.len() as i32, left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn args(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn stamps() {
        assert_eq!(argv2cmdline(&args("mri_info a b")), "mri_info a b ");
        assert_eq!(
            ver_cur_time_stamp_at(1_793_000_000),
            "2026/10/26-07:33:20-GMT"
        );
        let env = Environment::from_pairs([("USER", "u"), ("HOSTNAME", "h")]);
        let line = get_all_info_at(&env, &args("mri_info --all-info"), "mri_info", 0);
        assert!(line.starts_with(
            "ProgramName: mri_info  ProgramArguments: mri_info --all-info  ProgramVersion: 8.2.0  \
             TimeStamp: 2070/01/01-00:00:00-GMT  BuildTime: Mar 14 2026 12:21:06  BuildStamp: \
             freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45  User: "
        ));
        assert!(line.contains("  Platform: Linux  PlatformVersion: "));
        assert!(line.ends_with("  CompilerName: GCC  CompilerVersion: 8.4.0"));
        let mut out = Vec::new();
        assert_eq!(
            ver_file_time_stamp(&env, None, &mut out, "/nonexistent-x"),
            None
        );
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "ERROR: stating file /nonexistent-x\n"
        );
    }

    #[test]
    fn version_options_and_fs07() {
        let env = Environment::from_pairs::<&str, &str>([]);
        // The ordinary case: both loops agree, no notice.
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let r = handle_version_option(
            &mut g,
            &env,
            &mut out,
            &args("mri_info --version"),
            "mri_info",
        );
        assert_eq!(r, (1, args("mri_info")));
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "mri_info freesurfer 8.2.0\n"
        );
        assert!(g.notices.is_empty());
        let r = handle_version_option(
            &mut g,
            &env,
            &mut Vec::new(),
            &args("p -i --version f"),
            "p",
        );
        assert_eq!(r, (1, args("p -i f")));
        // The official mri_info's case (blackbox): -version skipped, --all-info twice.
        let a = args("mri_info --versionx -version --all-info");
        let (reports, left) = plan_freesurfer(&a);
        let shifted = args("mri_info -version --all-info --all-info");
        assert_eq!(
            reports,
            [
                VersionReport::Version,
                VersionReport::AllInfo(shifted.clone()),
                VersionReport::AllInfo(shifted)
            ]
        );
        assert_eq!(left, args("mri_info"));
        let (reports, left) = plan_fixed(&a);
        assert_eq!(reports.len(), 3);
        assert_eq!(
            reports[0..2],
            [VersionReport::Version, VersionReport::Version]
        );
        assert_eq!(left, args("mri_info"));
        // A second --version kept as an argument by FreeSurfer.
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let r = handle_version_option(
            &mut g,
            &env,
            &mut Vec::new(),
            &args("p --version --version x"),
            "p",
        );
        assert_eq!(r, (1, args("p --version x")));
        assert!(g.notices.contains(&FS_07));
        let mut g = Globals::new(UpstreamDefects::Fix);
        let r = handle_version_option(
            &mut g,
            &env,
            &mut Vec::new(),
            &args("p --version --version x"),
            "p",
        );
        assert_eq!(r, (2, args("p x")));
        assert!(version_kind("--all-infox").is_none() && version_kind("-versions") == Some(false));
    }
}

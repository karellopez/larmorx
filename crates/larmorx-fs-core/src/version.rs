// SPDX-License-Identifier: Apache-2.0
//! Version options and stamps (spec §13): the official 8.2.0 build's stamps, the version and
//! all-info options with FS-07's two plans, the time stamps, the user's name.

use larmorx_core::defects::FS_07;

use crate::process::Process;
use crate::system;

/// FreeSurfer's version.
pub const VERSION: &str = "8.2.0";
/// The official build's stamp.
pub const BUILD_STAMP: &str = "freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45";
/// The official build's time.
pub const BUILD_TIME: &str = "Mar 14 2026 12:21:06";
/// The official build's platform.
pub const PLATFORM: &str = "Linux";
/// The official build's compiler.
pub const COMPILER: &str = "GCC";
/// The official build's compiler version.
pub const COMPILER_VERSION: &str = "8.4.0";

/// What a program's unknown user, machine or release shows.
pub const UNKNOWN: &[u8] = b"UNKNOWN";

/// Whether `a` is a version option (`--version` or `-version`, matched as prefixes).
pub fn is_version_option(a: &[u8]) -> bool {
    a.starts_with(b"--version") || a.starts_with(b"-version")
}

/// Whether `a` is an all-info option (`--all-info` or `-all-info`, exactly).
pub fn is_all_info_option(a: &[u8]) -> bool {
    a == b"--all-info" || a == b"-all-info"
}

/// One line the version options print.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Version,
    /// The all-info line with these arguments.
    AllInfo(Vec<Vec<u8>>),
}

/// A plan: the lines printed, the arguments left.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Plan {
    lines: Vec<Line>,
    left: Vec<Vec<u8>>,
}

/// The corrected plan: each argument examined once, in order; the all-info line shows the
/// command line as given.
fn plan_fix(argv: &[Vec<u8>]) -> Plan {
    let mut lines = Vec::new();
    let mut left: Vec<Vec<u8>> = argv.iter().take(1).cloned().collect();
    for a in argv.iter().skip(1) {
        if is_version_option(a) {
            lines.push(Line::Version);
        } else if is_all_info_option(a) {
            lines.push(Line::AllInfo(argv.to_vec()));
        } else {
            left.push(a.clone());
        }
    }
    Plan { lines, left }
}

/// FreeSurfer's plan (FS-07): a removal moves the later arguments one place back and the loop
/// goes on at the next place, to the original count; the all-info line shows the arguments as
/// moved so far; the first `argc - removed` arguments are left.
fn plan_reproduce(argv: &[Vec<u8>]) -> Plan {
    let argc = argv.len();
    let mut a = argv.to_vec();
    let mut lines = Vec::new();
    let mut removed = 0;
    for i in 1..argc {
        let line = if is_version_option(&a[i]) {
            Line::Version
        } else if is_all_info_option(&a[i]) {
            Line::AllInfo(a.clone())
        } else {
            continue;
        };
        lines.push(line);
        for j in i..argc - 1 {
            a[j] = a[j + 1].clone();
        }
        removed += 1;
    }
    a.truncate(argc - removed);
    Plan { lines, left: a }
}

impl Process {
    /// The version line of `progname`: `<progname> freesurfer 8.2.0`.
    pub fn version_line(progname: &[u8]) -> Vec<u8> {
        let mut b = progname.to_vec();
        b.extend_from_slice(b" freesurfer ");
        b.extend_from_slice(VERSION.as_bytes());
        b
    }

    /// The "now" stamp (`VERcurTimeStamp`): `20YY/MM/DD-hh:mm:ss-GMT` in UTC, YY the year's last
    /// two digits.
    pub fn cur_time_stamp(&mut self) -> Vec<u8> {
        let now = self.now();
        match self.gmtime(now) {
            Some(t) => format!(
                "20{:02}/{:02}/{:02}-{:02}:{:02}:{:02}-GMT",
                t.year % 100,
                t.mon + 1,
                t.mday,
                t.hour,
                t.min,
                t.sec
            )
            .into_bytes(),
            None => Vec::new(),
        }
    }

    /// The effective user's account name (`VERuser`), `UNKNOWN` when it cannot be had.
    pub fn user_name(&self) -> Vec<u8> {
        system::effective_user().unwrap_or_else(|| UNKNOWN.to_vec())
    }

    /// A file's stamp (`VERfileTimeStamp`): its modification time in local time,
    /// `YYYY/MM/DD hh:mm:ss`; a file that cannot be stated: `ERROR: stating file <name>` on
    /// standard output and `None`.
    pub fn file_time_stamp(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        let modified = std::fs::metadata(crate::files::path_of(name))
            .and_then(|m| m.modified())
            .ok();
        let Some(m) = modified else {
            let mut b = b"ERROR: stating file ".to_vec();
            b.extend_from_slice(name);
            b.push(b'\n');
            self.put_out(&b);
            return None;
        };
        let t = self.localtime(crate::timedate::unix_seconds(m))?;
        Some(
            format!(
                "{}/{:02}/{:02} {:02}:{:02}:{:02}",
                i64::from(t.year) + 1900,
                t.mon + 1,
                t.mday,
                t.hour,
                t.min,
                t.sec
            )
            .into_bytes(),
        )
    }

    /// The all-info line (`getAllInfo`): the program's name and arguments (one space apart),
    /// the version, the "now" stamp, the build's stamps, the user, the machine and the kernel's
    /// release, pairs two spaces apart.
    pub fn all_info(&mut self, argv: &[&[u8]], progname: &[u8]) -> Vec<u8> {
        let stamp = self.cur_time_stamp();
        let user = self.user_name();
        let machine = system::host_name().unwrap_or_else(|| UNKNOWN.to_vec());
        let release = system::kernel_release().unwrap_or_else(|| UNKNOWN.to_vec());
        let mut b = b"ProgramName: ".to_vec();
        b.extend_from_slice(progname);
        b.extend_from_slice(b"  ProgramArguments: ");
        b.extend_from_slice(&argv.join(&b' '));
        b.extend_from_slice(format!("  ProgramVersion: {VERSION}  TimeStamp: ").as_bytes());
        b.extend_from_slice(&stamp);
        b.extend_from_slice(
            format!("  BuildTime: {BUILD_TIME}  BuildStamp: {BUILD_STAMP}  User: ").as_bytes(),
        );
        b.extend_from_slice(&user);
        b.extend_from_slice(b"  Machine: ");
        b.extend_from_slice(&machine);
        b.extend_from_slice(format!("  Platform: {PLATFORM}  PlatformVersion: ").as_bytes());
        b.extend_from_slice(&release);
        b.extend_from_slice(
            format!("  CompilerName: {COMPILER}  CompilerVersion: {COMPILER_VERSION}").as_bytes(),
        );
        b
    }

    /// The version options (`handleVersionOption`) of a command line (`argv[0]` the program as
    /// started): each version option prints the version line, each all-info option the
    /// all-info line, on standard output; the options are removed. Returns how many were
    /// removed and the arguments left. FS-07: `fix` examines each argument once; `reproduce`
    /// follows FreeSurfer's loop; a notice when the two differ.
    pub fn handle_version_option(
        &mut self,
        argv: &[Vec<u8>],
        progname: &[u8],
    ) -> (usize, Vec<Vec<u8>>) {
        let fix = plan_fix(argv);
        let rep = plan_reproduce(argv);
        if fix != rep {
            let what = "the version options: each argument examined once and the others kept in order, \
                        where FreeSurfer's removal skips the argument after each option and examines the last ones again";
            self.note(&FS_07, what);
        }
        let plan = if self.mode.reproduce() { rep } else { fix };
        for line in &plan.lines {
            let mut b = match line {
                Line::Version => Process::version_line(progname),
                Line::AllInfo(args) => {
                    let args: Vec<&[u8]> = args.iter().map(Vec::as_slice).collect();
                    self.all_info(&args, progname)
                }
            };
            b.push(b'\n');
            self.put_out(&b);
        }
        let removed = argv.len() - plan.left.len();
        (removed, plan.left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<Vec<u8>> {
        args.iter().map(|a| a.as_bytes().to_vec()).collect()
    }

    #[test]
    fn the_two_plans() {
        // official mri_info --versionx -version --all-info: one version line, two all-info lines
        let a = v(&["p", "--versionx", "-version", "--all-info"]);
        let r = plan_reproduce(&a);
        let moved = v(&["p", "-version", "--all-info", "--all-info"]);
        assert_eq!(
            r.lines,
            vec![
                Line::Version,
                Line::AllInfo(moved.clone()),
                Line::AllInfo(moved)
            ]
        );
        assert_eq!(r.left, v(&["p"]));
        let f = plan_fix(&a);
        assert_eq!(
            f.lines,
            vec![Line::Version, Line::Version, Line::AllInfo(a.clone())]
        );
        // p --version --version x: one version line, p --version x left
        let b = v(&["p", "--version", "--version", "x"]);
        assert_eq!(plan_reproduce(&b).lines, vec![Line::Version]);
        assert_eq!(plan_reproduce(&b).left, v(&["p", "--version", "x"]));
        assert_eq!(plan_fix(&b).left, v(&["p", "x"]));
        assert!(is_version_option(b"--versionx") && !is_all_info_option(b"-all-infox"));
    }
}

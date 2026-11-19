// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The machine and user names FreeSurfer prints (`getpwuid(geteuid())`, `uname`, `getcwd`):
//! larmorx's own code, without the C library. On Linux they come from `/proc` and `/etc/passwd`,
//! as the C library's answers on a machine whose accounts are local; elsewhere from the
//! environment (`USER`, `USERNAME`, `HOSTNAME`, `COMPUTERNAME`), else `UNKNOWN` (FreeSurfer's
//! own text when they cannot be had). These values describe the machine; they never reach a
//! result.

use std::path::Path;

use crate::run::Environment;

fn read_trimmed(path: &str) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    let s = s.trim_end_matches('\n').to_owned();
    (!s.is_empty()).then_some(s)
}

fn env_value(env: &Environment, names: &[&str]) -> Option<String> {
    names.iter().find_map(|n| {
        env.get(n)
            .map(|v| v.to_string_lossy().into_owned())
            .filter(|v| !v.is_empty())
    })
}

/// The effective user's name (`getpwuid(geteuid())->pw_name`), or `UNKNOWN`.
pub fn user_name(env: &Environment) -> String {
    passwd_name(env).unwrap_or_else(|| "UNKNOWN".to_owned())
}

/// The effective user's name as `getpwuid(geteuid())` gives it, `None` when nothing gives one.
/// glibc 2.35's lookup leaves `errno` 0 whether it finds the user or not: callers that track
/// FreeSurfer's `errno` set it to 0 there.
pub fn passwd_name(env: &Environment) -> Option<String> {
    let from_passwd = || -> Option<String> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let euid = status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))?
            .split_ascii_whitespace()
            .nth(1)?
            .to_owned();
        let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
        passwd.lines().find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() > 2 && f[2] == euid).then(|| f[0].to_owned())
        })
    };
    from_passwd().or_else(|| env_value(env, &["USER", "USERNAME"]))
}

/// `uname`'s fields: `sysname`, `nodename`, `release`, `machine`; `None` where `uname` would fail
/// (no way to read them here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uname {
    pub sysname: String,
    pub nodename: String,
    pub release: String,
    pub machine: String,
}

/// `uname(&u)`.
pub fn uname(env: &Environment) -> Option<Uname> {
    let nodename = read_trimmed("/proc/sys/kernel/hostname")
        .or_else(|| env_value(env, &["HOSTNAME", "COMPUTERNAME"]))?;
    let release = read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_else(|| "UNKNOWN".into());
    let sysname = match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "Darwin",
        "windows" => "Windows_NT",
        other => other,
    }
    .to_owned();
    Some(Uname {
        sysname,
        nodename,
        release,
        machine: std::env::consts::ARCH.to_owned(),
    })
}

/// `getcwd`: the program's directory (its settings' `dir`, else the process's).
pub fn cwd(dir: Option<&Path>) -> Option<String> {
    match dir {
        Some(d) => Some(d.to_string_lossy().into_owned()),
        None => std::env::current_dir()
            .ok()
            .map(|d| d.to_string_lossy().into_owned()),
    }
}

/// Whether the process's standard error (descriptor 2) is open for reading and writing (a
/// terminal), as `/proc/self/fdinfo/2` gives its access mode on Linux; false (writing only) when
/// it cannot be told (other systems: a program's standard error is then taken as a log or pipe).
/// [`crate::globals::StderrState`].
pub fn stderr_read_write() -> bool {
    let Ok(info) = std::fs::read_to_string("/proc/self/fdinfo/2") else {
        return false;
    };
    info.lines()
        .find_map(|l| l.strip_prefix("flags:"))
        .and_then(|f| u32::from_str_radix(f.trim(), 8).ok())
        .is_some_and(|f| f & 3 == 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_never_empty() {
        let env = Environment::from_pairs([("USER", "someone"), ("HOSTNAME", "box")]);
        assert!(!user_name(&env).is_empty());
        let u = uname(&env).unwrap();
        assert!(!u.nodename.is_empty() && !u.machine.is_empty());
        assert_eq!(cwd(Some(Path::new("/x"))).as_deref(), Some("/x"));
        let none = Environment::from_pairs::<&str, &str>([]);
        if !Path::new("/etc/passwd").exists() {
            assert_eq!(user_name(&none), "UNKNOWN");
        }
    }
}

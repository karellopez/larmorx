// SPDX-License-Identifier: Apache-2.0
//! Replicas: choosing between a tool's clean-room original and its bit-exact replica, and
//! running the replica as a separate program (`docs/licensing.md`, "Choosing at run time").
//!
//! Some tools have two implementations: the clean-room original in this Apache-2.0 package,
//! and a replica translated from the upstream source, in a package of the upstream's licence
//! family (`larmorx-gpl`; later `larmorx-nc`). This crate never links a replica. It finds the
//! replica package's program and runs it as a child process, with the tool's original
//! arguments; the two exchange files only. So the licences stay separate. This module knows
//! the replica's program name and arguments, nothing more.
//!
//! The same rules serve the command line (`larmorx afni 3dTshift ...`) and the Python wrappers
//! (`lx.afni.tshift(..., implementation="auto")`, through `larmorx._core`):
//! - [`REPLICAS`] lists every tool that has a replica: adding one is one line;
//! - [`find`] looks for the program: the package's environment variable (`LARMORX_GPL_BIN`),
//!   then the directories the caller gives (the Python environment's scripts directory, or the
//!   directory of the `larmorx` binary), then `PATH`;
//! - [`choose`] applies `auto`, `replica` or `original`;
//! - [`run`] runs the program and passes its output and exit code on unchanged.

use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// A package of replicas, installed separately from `larmorx`, with its own program.
#[derive(Debug, PartialEq, Eq)]
pub struct Package {
    /// The distribution name (`pip install <name>`).
    pub name: &'static str,
    /// The program the package installs (without `.exe`).
    pub program: &'static str,
    /// The environment variable that gives the program's path explicitly.
    pub env: &'static str,
    /// The licence of the package (an SPDX expression).
    pub licence: &'static str,
    /// The command that installs it.
    pub install: &'static str,
}

/// The GPL-3.0-or-later replicas (`crates-gpl/`): AFNI's MCW files, Connectome Workbench.
pub const LARMORX_GPL: Package = Package {
    name: "larmorx-gpl",
    program: "larmorx-gpl",
    env: "LARMORX_GPL_BIN",
    licence: "GPL-3.0-or-later",
    install: "pip install \"larmorx[exact]\"",
};

/// A tool with a replica: `larmorx <family> <tool> ...` runs
/// `<package.program> <family> <tool> ...`.
#[derive(Debug)]
pub struct Replica {
    pub family: &'static str,
    pub tool: &'static str,
    pub package: &'static Package,
}

/// Every tool that has a replica in another package. Add one line per replica (and the
/// Python wrapper's `implementation` argument; see `docs/architecture.md`, "Adding a replica").
pub const REPLICAS: &[Replica] = &[Replica {
    family: "afni",
    tool: "3dTshift",
    package: &LARMORX_GPL,
}];

/// The replica of `larmorx <family> <tool>`, if there is one.
pub fn lookup(family: &str, tool: &str) -> Option<&'static Replica> {
    REPLICAS
        .iter()
        .find(|r| r.family == family && r.tool == tool)
}

/// Which implementation runs a tool that has both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Implementation {
    /// The replica when its program is found, the original otherwise (the default).
    Auto,
    /// The replica; an error if its program is not found.
    Replica,
    /// The clean-room original, always.
    Original,
}

/// The environment variable that sets the [`Implementation`] of the command line.
pub const IMPLEMENTATION_ENV: &str = "LARMORX_IMPLEMENTATION";

impl Implementation {
    /// `auto`, `replica` or `original`.
    pub fn parse(name: &str) -> Option<Implementation> {
        match name {
            "auto" => Some(Implementation::Auto),
            "replica" => Some(Implementation::Replica),
            "original" => Some(Implementation::Original),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Implementation::Auto => "auto",
            Implementation::Replica => "replica",
            Implementation::Original => "original",
        }
    }

    /// [`IMPLEMENTATION_ENV`]: unset or empty is [`Implementation::Auto`].
    pub fn from_env() -> Result<Implementation, Error> {
        match std::env::var_os(IMPLEMENTATION_ENV) {
            None => Ok(Implementation::Auto),
            Some(v) if v.is_empty() => Ok(Implementation::Auto),
            Some(v) => v
                .to_str()
                .and_then(Implementation::parse)
                .ok_or_else(|| Error::BadImplementation(v.to_string_lossy().into_owned())),
        }
    }
}

/// Why a replica cannot be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// [`IMPLEMENTATION_ENV`] holds something other than `auto`, `replica` or `original`.
    BadImplementation(String),
    /// The package's environment variable names something that is not a file.
    BadPath { env: &'static str, value: String },
    /// `replica` was asked for and the program was not found.
    NotFound {
        family: &'static str,
        tool: &'static str,
        package: &'static Package,
    },
}

impl Error {
    /// The command line's exit code: 2 for a usage error, 127 (a shell's "command not found")
    /// when the replica's program is missing.
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::BadImplementation(_) | Error::BadPath { .. } => 2,
            Error::NotFound { .. } => EXIT_NOT_FOUND,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadImplementation(v) => write!(
                f,
                "{IMPLEMENTATION_ENV} must be 'auto', 'replica' or 'original', not '{v}'"
            ),
            Error::BadPath { env, value } => {
                write!(f, "{env} is set to '{value}', which is not a file")
            }
            Error::NotFound {
                family,
                tool,
                package,
            } => write!(
                f,
                "the replica of {family} {tool} was asked for, but the {} program was not found \
                 ({}, the Python environment's scripts directory, PATH); install it with: {} \
                 (licence {})",
                package.program, package.env, package.install, package.licence
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Exit code when the replica's program is missing (a shell's "command not found").
pub const EXIT_NOT_FOUND: u8 = 127;

/// Exit code when the replica's program exists but cannot be started (a shell's "cannot
/// execute").
pub const EXIT_CANNOT_RUN: u8 = 126;

/// Finds `package`'s program: the path in its environment variable if that is set (an error
/// if it is not a file), else the first match in `dirs`, else in `PATH`.
pub fn find(package: &Package, dirs: &[PathBuf]) -> Result<Option<PathBuf>, Error> {
    find_with(
        package,
        std::env::var_os(package.env),
        dirs,
        std::env::var_os("PATH"),
    )
}

/// [`find`] with the environment given: the package's variable and `PATH`.
pub fn find_with(
    package: &Package,
    explicit: Option<OsString>,
    dirs: &[PathBuf],
    path: Option<OsString>,
) -> Result<Option<PathBuf>, Error> {
    if let Some(value) = explicit.filter(|v| !v.is_empty()) {
        let program = PathBuf::from(&value);
        return if program.is_file() {
            Ok(Some(program))
        } else {
            Err(Error::BadPath {
                env: package.env,
                value: value.to_string_lossy().into_owned(),
            })
        };
    }
    let path_dirs: Vec<PathBuf> =
        path.map_or_else(Vec::new, |p| std::env::split_paths(&p).collect());
    Ok(dirs
        .iter()
        .chain(&path_dirs)
        // An empty PATH entry means the current directory to a shell; it is never searched.
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| executable_in(dir, package.program)))
}

/// `dir/program` if it is an executable file (on Windows, `dir/program` plus an extension
/// from `PATHEXT` that a process can be started from: `.com`, `.exe`, `.bat`, `.cmd`).
#[cfg(not(windows))]
fn executable_in(dir: &Path, program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidate = dir.join(program);
    let meta = std::fs::metadata(&candidate).ok()?;
    (meta.is_file() && meta.permissions().mode() & 0o111 != 0).then_some(candidate)
}

#[cfg(windows)]
fn executable_in(dir: &Path, program: &str) -> Option<PathBuf> {
    const RUNNABLE: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    pathext
        .split(';')
        .map(str::to_ascii_lowercase)
        .filter(|ext| RUNNABLE.contains(&ext.as_str()))
        .map(|ext| dir.join(format!("{program}{ext}")))
        .find(|candidate| candidate.is_file())
}

/// What runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Original,
    /// The replica, with the path of its program.
    Replica(PathBuf),
}

/// Applies `implementation` to `replica`, looking for its program (see [`find`]) unless the
/// original is asked for.
pub fn choose(
    implementation: Implementation,
    replica: &Replica,
    dirs: &[PathBuf],
) -> Result<Choice, Error> {
    choose_with(implementation, replica, || find(replica.package, dirs))
}

/// [`choose`] with another way to look for the program.
pub fn choose_with(
    implementation: Implementation,
    replica: &Replica,
    find: impl FnOnce() -> Result<Option<PathBuf>, Error>,
) -> Result<Choice, Error> {
    if implementation == Implementation::Original {
        return Ok(Choice::Original);
    }
    match (find()?, implementation) {
        (Some(program), _) => Ok(Choice::Replica(program)),
        (None, Implementation::Replica) => Err(Error::NotFound {
            family: replica.family,
            tool: replica.tool,
            package: replica.package,
        }),
        (None, _) => Ok(Choice::Original),
    }
}

/// How the child process's output reaches the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Collected, then written to the `out` and `err` writers (the Python entry point).
    Capture,
    /// The child writes to this process's own standard output and error (the standalone
    /// binaries), so progress appears as it is made.
    Inherit,
}

/// Runs `program <family> <tool> args...` and returns its exit code: the replica's own, so
/// that callers see exactly what the replica produced. A program that cannot be started gives
/// [`EXIT_CANNOT_RUN`]; one killed by a signal gives 128 + the signal (as a shell reports it);
/// an exit code outside 0-255 (a crash on Windows) gives 1.
pub fn run(
    program: &Path,
    replica: &Replica,
    args: &[String],
    output: Output,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut command = Command::new(program);
    command
        .arg(replica.family)
        .arg(replica.tool)
        .args(args)
        .stdin(Stdio::null());
    let result = match output {
        Output::Capture => command.output().map(|o| {
            let written = out.write_all(&o.stdout).and(err.write_all(&o.stderr));
            (o.status, written.is_ok())
        }),
        Output::Inherit => command.status().map(|s| (s, true)),
    };
    match result {
        Ok((status, true)) => exit_code(status),
        Ok((_, false)) => 1,
        Err(e) => {
            let _ = writeln!(
                err,
                "error: cannot run the replica of {} {} ({}): {e}",
                replica.family,
                replica.tool,
                program.display()
            );
            EXIT_CANNOT_RUN
        }
    }
}

fn exit_code(status: ExitStatus) -> u8 {
    if let Some(code) = status.code() {
        return u8::try_from(code).unwrap_or(1);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return u8::try_from(128 + signal).unwrap_or(1);
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSHIFT: &Replica = &REPLICAS[0];

    #[test]
    fn the_registry_names_each_tool_once() {
        let r = lookup("afni", "3dTshift").unwrap();
        assert_eq!(r.package.program, "larmorx-gpl");
        assert!(lookup("afni", "3dvolreg").is_none());
        assert!(lookup("ants", "3dTshift").is_none());
        for (i, a) in REPLICAS.iter().enumerate() {
            assert!(
                REPLICAS[..i]
                    .iter()
                    .all(|b| (a.family, a.tool) != (b.family, b.tool))
            );
        }
    }

    #[test]
    fn implementation_names() {
        for i in [
            Implementation::Auto,
            Implementation::Replica,
            Implementation::Original,
        ] {
            assert_eq!(Implementation::parse(i.name()), Some(i));
        }
        assert_eq!(Implementation::parse("Replica"), None);
        assert_eq!(Implementation::parse(""), None);
    }

    #[test]
    fn choices() {
        let found = || Ok(Some(PathBuf::from("bin/larmorx-gpl")));
        let missing = || Ok(None);
        let never = || -> Result<Option<PathBuf>, Error> { panic!("not searched") };
        let replica = Choice::Replica(PathBuf::from("bin/larmorx-gpl"));
        assert_eq!(
            choose_with(Implementation::Auto, TSHIFT, found),
            Ok(replica.clone())
        );
        assert_eq!(
            choose_with(Implementation::Auto, TSHIFT, missing),
            Ok(Choice::Original)
        );
        assert_eq!(
            choose_with(Implementation::Replica, TSHIFT, found),
            Ok(replica)
        );
        let e = choose_with(Implementation::Replica, TSHIFT, missing).unwrap_err();
        assert_eq!(e.exit_code(), EXIT_NOT_FOUND);
        assert!(
            e.to_string().contains("pip install \"larmorx[exact]\""),
            "{e}"
        );
        assert_eq!(
            choose_with(Implementation::Original, TSHIFT, never),
            Ok(Choice::Original)
        );
        // A broken explicit path is an error, not a silent fallback.
        let bad = || {
            Err(Error::BadPath {
                env: "LARMORX_GPL_BIN",
                value: "nowhere".into(),
            })
        };
        assert!(choose_with(Implementation::Auto, TSHIFT, bad).is_err());
    }

    #[test]
    fn find_follows_the_search_order() {
        let tmp = tempfile::tempdir().unwrap();
        let [env_dir, first, second] = ["env", "first", "second"].map(|d| {
            let dir = tmp.path().join(d);
            std::fs::create_dir(&dir).unwrap();
            dir
        });
        let empty = tmp.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        let p = &LARMORX_GPL;
        let put = |dir: &Path| {
            let name = if cfg!(windows) {
                "larmorx-gpl.exe"
            } else {
                "larmorx-gpl"
            };
            let file = dir.join(name);
            std::fs::write(&file, "").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            file
        };
        let explicit = put(&env_dir);
        let in_first = put(&first);
        let in_second = put(&second);
        let path_of = |dirs: &[&Path]| Some(std::env::join_paths(dirs).unwrap());

        // The variable wins, then the given directories, then PATH; empty entries are skipped.
        let all = find_with(
            p,
            Some(explicit.clone().into()),
            std::slice::from_ref(&first),
            path_of(&[&second]),
        );
        assert_eq!(all, Ok(Some(explicit)));
        let dirs = find_with(
            p,
            Some(OsString::new()),
            &[empty.clone(), first.clone()],
            path_of(&[&second]),
        );
        assert_eq!(dirs, Ok(Some(in_first)));
        let on_path = find_with(
            p,
            None,
            std::slice::from_ref(&empty),
            path_of(&[&empty, &second]),
        );
        assert_eq!(on_path, Ok(Some(in_second)));
        assert_eq!(
            find_with(p, None, &[PathBuf::new()], path_of(&[&empty])),
            Ok(None)
        );
        assert_eq!(find_with(p, None, &[], None), Ok(None));
        let missing = tmp.path().join("missing");
        assert!(matches!(
            find_with(p, Some(missing.into()), &[first], None),
            Err(Error::BadPath { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn files_that_cannot_be_executed_are_not_found() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("larmorx-gpl");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let dirs = [tmp.path().to_path_buf()];
        assert_eq!(find_with(&LARMORX_GPL, None, &dirs, None), Ok(None));
    }
}

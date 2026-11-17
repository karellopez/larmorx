// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! How the program `larmorx-freesurfer` runs a FreeSurfer program's replica: what it gives the
//! program beside its original arguments ([`Settings`]) and the entry point every program crate
//! registers ([`Program`]). larmorx's own plumbing, no FreeSurfer source (queue item 3.1-01).
//!
//! A program reads everything the original reads from its process (the environment's
//! `FREESURFER_HOME`, `SUBJECTS_DIR`, `FS_LICENSE`, ..., the working directory) from its
//! [`Settings`] instead, so that `recon-all`'s replica (wave 3.7) can run the other programs in
//! its own process, each with the environment and directory the original script would give it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use larmorx_core::defects::UpstreamDefects;

/// What a program's replica runs with besides its original arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The thread count (0: every logical CPU): `--n-threads` before the family, else
    /// `OMP_NUM_THREADS`. A program whose original takes its own thread option (`-openmp`,
    /// `-threads`, `-nthreads`, `-itkthreads`, ...) uses that one when it is given. The results
    /// never depend on it (CLAUDE.md rule 5).
    pub threads: usize,
    /// D13's choice: FreeSurfer's defects corrected (`fix`, the default) or reproduced.
    pub mode: UpstreamDefects,
    /// The environment the original would see (by default the process's own).
    pub env: Environment,
    /// The directory relative file names start in (by default the process's own).
    pub dir: Option<PathBuf>,
}

impl Settings {
    /// The settings of a program run by itself: the given thread count and mode, the process's
    /// environment, the process's working directory.
    pub fn of_process(threads: usize, mode: UpstreamDefects) -> Self {
        Settings {
            threads,
            mode,
            env: Environment::of_process(),
            dir: None,
        }
    }
}

/// An environment table: the variables a program reads, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment(BTreeMap<OsString, OsString>);

impl Environment {
    /// The process's environment, as it is now.
    pub fn of_process() -> Self {
        Environment(std::env::vars_os().collect())
    }

    /// A table of the given variables only.
    pub fn from_pairs<K: Into<OsString>, V: Into<OsString>>(
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Environment(
            pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }

    /// The name under which `name` is stored: itself, or on Windows, whose variable names do not
    /// depend on case (`Path` is `PATH`), a name equal to it but for ASCII case.
    fn stored(&self, name: &std::ffi::OsStr) -> Option<OsString> {
        if self.0.contains_key(name) {
            return Some(name.to_owned());
        }
        if !cfg!(windows) {
            return None;
        }
        let name = name.to_str()?;
        self.0
            .keys()
            .find(|k| k.to_str().is_some_and(|k| k.eq_ignore_ascii_case(name)))
            .cloned()
    }

    /// The value of `name` (`getenv`): `None` when it is not set (on Windows any case of the
    /// name).
    pub fn get(&self, name: &str) -> Option<&OsString> {
        let name = std::ffi::OsStr::new(name);
        match self.0.get(name) {
            Some(v) => Some(v),
            None => self.0.get(&self.stored(name)?),
        }
    }

    /// Sets `name` (`setenv` with overwrite; on Windows a variable of the name in another case is
    /// the same variable, and keeps its name).
    pub fn set(&mut self, name: impl Into<OsString>, value: impl Into<OsString>) {
        let name = name.into();
        let key = self.stored(&name).unwrap_or(name);
        self.0.insert(key, value.into());
    }

    /// Removes `name` (`unsetenv`; on Windows in any case).
    pub fn remove(&mut self, name: &str) {
        if let Some(k) = self.stored(std::ffi::OsStr::new(name)) {
            self.0.remove(&k);
        }
    }

    /// Every variable, in byte order of the names.
    pub fn iter(&self) -> impl Iterator<Item = (&OsString, &OsString)> {
        self.0.iter()
    }
}

/// A program's entry point: its original arguments (without the program's name), the settings,
/// standard output and standard error; it returns the exit status.
pub type ProgramFn = fn(&[String], &Settings, &mut dyn Write, &mut dyn Write) -> u8;

/// A program a program crate registers with `larmorx-freesurfer` (one line in its registry).
#[derive(Clone, Copy)]
pub struct Program {
    /// The original program's name (`mri_convert`).
    pub name: &'static str,
    /// Its entry point.
    pub run: ProgramFn,
}

impl std::fmt::Debug for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Program").field("name", &self.name).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_environment_table_is_independent_of_the_process() {
        let mut env = Environment::from_pairs([("SUBJECTS_DIR", "/subjects")]);
        assert_eq!(
            env.get("SUBJECTS_DIR").map(|v| v.to_str()),
            Some(Some("/subjects"))
        );
        assert_eq!(env.get("FREESURFER_HOME"), None);
        env.set("FREESURFER_HOME", "/fs");
        env.remove("SUBJECTS_DIR");
        let names: Vec<_> = env.iter().map(|(k, _)| k.to_str().unwrap()).collect();
        assert_eq!(names, ["FREESURFER_HOME"]);
        // Windows' variable names do not depend on case; elsewhere they do
        let w = cfg!(windows);
        let mut env = Environment::from_pairs([("Path", "/bin")]);
        assert_eq!(env.get("PATH").is_some(), w);
        env.set("PATH", "/usr/bin");
        assert_eq!(env.iter().count(), if w { 1 } else { 2 });
        assert_eq!(
            env.get("Path").unwrap(),
            if w { "/usr/bin" } else { "/bin" }
        );
        env.remove("PATH");
        assert_eq!(env.get("Path").is_some(), !w);
    }

    #[test]
    fn settings_of_the_process() {
        let s = Settings::of_process(3, UpstreamDefects::Reproduce);
        assert_eq!(
            (s.threads, s.mode, s.dir),
            (3, UpstreamDefects::Reproduce, None)
        );
        assert_eq!(
            s.env.get("PATH").is_some(),
            std::env::var_os("PATH").is_some()
        );
    }
}

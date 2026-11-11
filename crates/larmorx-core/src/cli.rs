// SPDX-License-Identifier: Apache-2.0
//! larmorx's own command-line options, as the tools' command lines receive them.
//!
//! `larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects
//! fix|reproduce] <family> <tool> <the original's arguments>` (the user's decision of
//! 2026-11-11): the options stand between `larmorx` and the family, so they never clash with an
//! original program's options, and they carry the Python API's names (`n_threads`,
//! `implementation`, `upstream_defects`). `larmorx_cli` parses them. Each one given wins over
//! the environment variable it replaces, which keeps working; a tool's command line receives
//! them explicitly, as [`Overrides`] (or in its settings table), and the process environment is
//! never changed. `--implementation` chooses which program runs and stays in `larmorx_cli`.

use crate::defects::{BadMode, UpstreamDefects};

/// The values larmorx's own options give one run of a tool's command line; `None` where the
/// option was not given, so that the tool reads its environment variable as before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `--n-threads N`: the thread count (0: every logical CPU). `None`: the tool's own
    /// variable (`OMP_NUM_THREADS`; ITK's `NSLOTS` and `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`
    /// for the ANTs programs). Results never depend on it.
    pub n_threads: Option<usize>,
    /// `--upstream-defects fix|reproduce`: D13's mode. `None`: [`crate::defects::ENV`]
    /// (`LARMORX_UPSTREAM_DEFECTS`).
    pub upstream_defects: Option<UpstreamDefects>,
}

impl Overrides {
    /// The thread count: the option's, else what `from_env` reads.
    pub fn n_threads_or(&self, from_env: impl FnOnce() -> usize) -> usize {
        self.n_threads.unwrap_or_else(from_env)
    }

    /// D13's mode: the option's, else [`UpstreamDefects::from_env`] (an error when the variable
    /// holds something other than `fix` or `reproduce`).
    pub fn upstream_defects(&self) -> Result<UpstreamDefects, BadMode> {
        self.upstream_defects
            .map_or_else(UpstreamDefects::from_env, Ok)
    }

    /// Whether no option was given (the run reads its environment only).
    pub fn is_empty(&self) -> bool {
        *self == Overrides::default()
    }

    /// The changes these options make to an environment table that stands for the process's
    /// (a settings table, a child process's environment): `(name, Some(value))` sets the
    /// variable, `(name, None)` removes it. `--n-threads 0` removes `OMP_NUM_THREADS` (every
    /// logical CPU), as the Python wrappers do for `n_threads=0`.
    pub fn environment(&self) -> Vec<(&'static str, Option<String>)> {
        let mut env = Vec::new();
        if let Some(n) = self.n_threads {
            env.push(("OMP_NUM_THREADS", (n > 0).then(|| n.to_string())));
        }
        if let Some(mode) = self.upstream_defects {
            env.push((crate::defects::ENV, Some(mode.name().to_owned())));
        }
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_win_over_the_environment() {
        let none = Overrides::default();
        assert!(none.is_empty());
        assert_eq!(none.n_threads_or(|| 7), 7);
        assert!(none.environment().is_empty());
        let given = Overrides {
            n_threads: Some(0),
            upstream_defects: Some(UpstreamDefects::Reproduce),
        };
        assert!(!given.is_empty());
        assert_eq!(given.n_threads_or(|| panic!("not read")), 0);
        assert_eq!(given.upstream_defects(), Ok(UpstreamDefects::Reproduce));
        assert_eq!(
            given.environment(),
            [
                ("OMP_NUM_THREADS", None),
                ("LARMORX_UPSTREAM_DEFECTS", Some("reproduce".to_owned()))
            ]
        );
        let three = Overrides {
            n_threads: Some(3),
            upstream_defects: None,
        };
        assert_eq!(
            three.environment(),
            [("OMP_NUM_THREADS", Some("3".to_owned()))]
        );
    }
}

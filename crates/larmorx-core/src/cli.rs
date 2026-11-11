// SPDX-License-Identifier: Apache-2.0
//! larmorx's own command-line options, as the tools' command lines receive them.
//!
//! `larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects
//! fix|reproduce] <family> <tool> <the original's arguments>` (the user's decision of
//! 2026-11-11): the options stand between `larmorx` and the family, so they never clash with an
//! original program's options, and they carry the Python API's names (`n_threads`,
//! `implementation`, `upstream_defects`). Each one given wins over the environment variable it
//! replaces, which keeps working; a tool's command line receives them explicitly, as
//! [`Overrides`] (or in its settings table), and the process environment is never changed.
//!
//! [`parse`] reads them, once, for every program that takes them: `larmorx` (`larmorx_cli`,
//! where `--implementation` chooses which program runs) and the replica programs of the other
//! licence families (`larmorx-nc`, `larmorx-gpl`, and every later one, as `larmorx-freesurfer`),
//! which depend on this crate and never on `larmorx_cli`. Each program says what its
//! `--implementation` takes ([`ImplementationOption`]): a replica program runs its replicas
//! only, so it takes `replica` and refuses the rest ([`ReplicaOnly`]).

use crate::defects::{BadMode, UpstreamDefects};

/// larmorx's own options, in the order `--help` lists them.
pub const OPTIONS: [&str; 3] = ["--n-threads", "--implementation", "--upstream-defects"];

/// What a program's `--implementation` takes.
pub trait ImplementationOption: Sized {
    /// The values, as the messages name them (`auto, replica or clean-room`).
    const VALUES: &'static str;

    /// The value written after `--implementation`, or the message of the usage error.
    fn from_option(value: &str) -> Result<Self, String>;
}

/// `--implementation` in a replica program of another licence family (`larmorx-nc`,
/// `larmorx-gpl`): `replica`, which changes nothing (the program runs its bit-exact replicas
/// only). `auto`, `clean-room` and `original` (its old spelling) are refused with a message that
/// says so: choosing between a replica and a clean-room original is `larmorx`'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplicaOnly;

impl ImplementationOption for ReplicaOnly {
    const VALUES: &'static str = "replica";

    fn from_option(value: &str) -> Result<Self, String> {
        match value {
            "replica" => Ok(ReplicaOnly),
            "auto" | "clean-room" | "original" => Err(format!(
                "--implementation {value}: this program runs the bit-exact replicas only, so \
                 --implementation takes replica here (or is left out); larmorx chooses between a \
                 tool's replica and its clean-room original (larmorx --implementation {value} \
                 <family> <tool> ...)"
            )),
            _ => Err(format!(
                "--implementation must be {}, not '{value}'",
                Self::VALUES
            )),
        }
    }
}

/// The options [`parse`] read; `None` where one was not given (its environment variable then
/// decides, as before).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Given<I> {
    /// `--n-threads N`: the thread count, 0 every logical CPU.
    pub n_threads: Option<usize>,
    /// `--implementation NAME`, as the program reads it.
    pub implementation: Option<I>,
    /// `--upstream-defects fix|reproduce` (D13).
    pub upstream_defects: Option<UpstreamDefects>,
}

impl<I> Default for Given<I> {
    fn default() -> Self {
        Given {
            n_threads: None,
            implementation: None,
            upstream_defects: None,
        }
    }
}

impl<I> Given<I> {
    /// What the tools' command lines receive: the thread count and D13's mode.
    pub fn overrides(&self) -> Overrides {
        Overrides {
            n_threads: self.n_threads,
            upstream_defects: self.upstream_defects,
        }
    }
}

/// Reads larmorx's own options at the start of `args` (the arguments after the program's
/// name): the options and how many arguments they took, or the message of a usage error (a
/// wrong or missing value, an option given twice). Each takes its value as the next argument or
/// after `=`; the first argument that is not one of [`OPTIONS`] ends them (the family, or
/// `--help`).
pub fn parse<I: ImplementationOption>(args: &[String]) -> Result<(Given<I>, usize), String> {
    let mut g = Given::default();
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        let Some((name, inline)) = option(arg) else {
            break;
        };
        i += 1;
        let values = match name {
            "--n-threads" => N_THREADS_VALUES,
            "--implementation" => I::VALUES,
            _ => UPSTREAM_DEFECTS_VALUES,
        };
        let value = match inline {
            Some(v) => v,
            None => {
                let Some(v) = args.get(i) else {
                    return Err(format!("{name} needs a value: {values}"));
                };
                i += 1;
                v.as_str()
            }
        };
        let wrong = || format!("{name} must be {values}, not '{value}'");
        let twice = || format!("{name} is given twice");
        match name {
            "--n-threads" => {
                if g.n_threads.is_some() {
                    return Err(twice());
                }
                g.n_threads = Some(parse_n_threads(value).ok_or_else(wrong)?);
            }
            "--implementation" => {
                if g.implementation.is_some() {
                    return Err(twice());
                }
                g.implementation = Some(I::from_option(value)?);
            }
            _ => {
                if g.upstream_defects.is_some() {
                    return Err(twice());
                }
                g.upstream_defects = Some(UpstreamDefects::parse(value).ok_or_else(wrong)?);
            }
        }
    }
    Ok((g, i))
}

/// `arg` as one of [`OPTIONS`]: its name, and the value written after `=`.
pub fn option(arg: &str) -> Option<(&'static str, Option<&str>)> {
    OPTIONS.iter().find_map(|&name| {
        if arg == name {
            Some((name, None))
        } else {
            arg.strip_prefix(name)
                .and_then(|r| r.strip_prefix('='))
                .map(|v| (name, Some(v)))
        }
    })
}

/// What `--n-threads` takes, as its messages name it.
const N_THREADS_VALUES: &str = "a whole number of threads, 0 for every logical CPU";

/// What `--upstream-defects` takes.
const UPSTREAM_DEFECTS_VALUES: &str = "fix or reproduce";

/// `--n-threads`' value: a whole number written in decimal digits, 0 or more.
fn parse_n_threads(value: &str) -> Option<usize> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// The thread count of `OMP_NUM_THREADS` as the replica programs read it: a whole number, else
/// 0 (every logical CPU: unset, empty or not a number).
pub fn omp_num_threads(value: Option<&str>) -> usize {
    value.and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

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

    fn a(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_replica_program_takes_replica_only() {
        let (g, k) = parse::<ReplicaOnly>(&a(&[
            "--implementation=replica",
            "--n-threads",
            "0",
            "--upstream-defects=reproduce",
            "fsl",
            "--n-threads",
            "2",
        ]))
        .unwrap();
        assert_eq!(k, 4);
        assert_eq!(
            g,
            Given {
                n_threads: Some(0),
                implementation: Some(ReplicaOnly),
                upstream_defects: Some(UpstreamDefects::Reproduce),
            }
        );
        assert_eq!(
            g.overrides(),
            Overrides {
                n_threads: Some(0),
                upstream_defects: Some(UpstreamDefects::Reproduce)
            }
        );
        for refused in ["auto", "clean-room", "original"] {
            let e = parse::<ReplicaOnly>(&a(&["--implementation", refused, "fsl"])).unwrap_err();
            assert!(
                e.starts_with(&format!(
                    "--implementation {refused}: this program runs the bit-exact replicas only"
                )),
                "{e}"
            );
            assert!(e.contains(&format!("larmorx --implementation {refused} <family>")));
        }
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--implementation", "x"])).unwrap_err(),
            "--implementation must be replica, not 'x'"
        );
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--implementation"])).unwrap_err(),
            "--implementation needs a value: replica"
        );
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--n-threads", "-1"])).unwrap_err(),
            "--n-threads must be a whole number of threads, 0 for every logical CPU, not '-1'"
        );
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--upstream-defects=x"])).unwrap_err(),
            "--upstream-defects must be fix or reproduce, not 'x'"
        );
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--n-threads=1", "--n-threads=2"])).unwrap_err(),
            "--n-threads is given twice"
        );
        // not an option of larmorx's: the end of the options
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--n-threads4", "x"])),
            Ok((Given::default(), 0))
        );
        assert_eq!(
            parse::<ReplicaOnly>(&a(&["--help"])),
            Ok((Given::default(), 0))
        );
    }

    #[test]
    fn omp_num_threads_as_the_replica_programs_read_it() {
        assert_eq!(omp_num_threads(None), 0);
        assert_eq!(omp_num_threads(Some("")), 0);
        assert_eq!(omp_num_threads(Some(" 3 ")), 3);
        assert_eq!(omp_num_threads(Some("x")), 0);
        assert_eq!(omp_num_threads(Some("-2")), 0);
    }
}

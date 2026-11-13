// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's core services (`fs.core`), a clean room of `specs/freesurfer-core.md`.
//!
//! The services every FreeSurfer program shares: its process state (§1), messages and the end
//! of a program (§2, §3), the diagnostics read from the environment (§4, §5), byte order (§6),
//! the older programs' command-line helpers (§7), the seeded random numbers (§9), strings and
//! file names (§10), numbers and lists (§11), the fast arctangent (§12), version options and
//! stamps (§13), times and dates (§14), the environment record (§15), memory and usage reports
//! (§16), logic-problem reports (§17), temporary files (§18), threads, the reproducible sums, the
//! debugging stack and HIPS (§19).
//!
//! No global state: a program run owns one [`Process`], which holds what FreeSurfer keeps
//! process-wide (§1) and the environment table it reads (porting guide §9.1: the library never
//! reads the process environment). A call that ends the program in FreeSurfer returns
//! [`Stop`] instead, so the caller decides how to end it.
//!
//! The defects of the spec's §22 are D13 switches ([`larmorx_core::defects`]): the mode is the
//! process's ([`Process::mode`]); the notices of corrected or reproduced defects are collected in
//! [`Process::notices`].
#![forbid(unsafe_code)]

pub mod atan;
pub mod clib;
pub mod cmdargs;
pub mod diag;
pub mod env;
pub mod files;
pub mod fsenv;
pub mod logic;
pub mod maths;
pub mod messages;
pub mod numbers;
pub mod process;
pub mod random;
pub mod romp;
pub mod sse;
pub mod strings;
pub mod swap;
pub mod system;
pub mod temp;
pub mod timedate;
pub mod usage;
pub mod version;
pub mod xdbg;

pub use env::Environment;
pub use process::{Process, Stop};

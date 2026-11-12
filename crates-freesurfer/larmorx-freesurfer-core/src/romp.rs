// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's OpenMP support (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/romp_support.cpp` as the official build compiles it: `ROMP_SUPPORT_ENABLED`
//! is off (`include/romp_support.h`), so of its timing and statistics only these remain:
//! a constructor that lowers OpenMP's thread count to `_MAX_FS_THREADS` (128) before `main`,
//! `romp_omp_get_thread_num` (which stops a program running on a thread numbered 128 or more),
//! and `ROMP_main_started`, which records the main file and line (its exit handler prints
//! nothing). The replica's results never depend on its thread count (CLAUDE.md rule 5); these
//! give the thread count FreeSurfer would use, where a program prints it.

use std::io::Write;

use crate::globals::Globals;
use crate::log::Fatal;

/// `_MAX_FS_THREADS` (`include/base.h`).
pub const MAX_FS_THREADS: usize = 128;

/// The thread count OpenMP gives a FreeSurfer program asked for `n` (`before_main`).
pub fn max_threads(n: usize) -> usize {
    n.min(MAX_FS_THREADS)
}

/// `romp_omp_get_thread_num()` for thread `n`: `n`, or `Freesurfer supports a maximum of 128
/// threads, set OMP_NUM_THREADS accordingly` on standard error and exit status 1.
pub fn romp_omp_get_thread_num(err: &mut dyn Write, n: usize) -> Result<usize, Fatal> {
    if n >= MAX_FS_THREADS {
        let _ = writeln!(
            err,
            "Freesurfer supports a maximum of {MAX_FS_THREADS} threads, set OMP_NUM_THREADS accordingly"
        );
        return Err(Fatal::Exit(1));
    }
    Ok(n)
}

/// `ROMP_main_started(file, line)`.
pub fn romp_main_started(g: &mut Globals, file: &str, line: i32) {
    g.romp_main = Some((file.to_owned(), line));
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn thread_limits() {
        assert_eq!((max_threads(4), max_threads(500)), (4, 128));
        let mut err = Vec::new();
        assert_eq!(romp_omp_get_thread_num(&mut err, 127), Ok(127));
        assert_eq!(romp_omp_get_thread_num(&mut err, 128), Err(Fatal::Exit(1)));
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Freesurfer supports a maximum of 128 threads, set OMP_NUM_THREADS accordingly\n"
        );
        let mut g = Globals::new(UpstreamDefects::Fix);
        romp_main_started(&mut g, "mri_ca_register.cpp", 12);
        assert_eq!(g.romp_main, Some(("mri_ca_register.cpp".to_owned(), 12)));
    }
}

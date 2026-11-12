// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's diagnostics (`fs.core`, queue item 3.1-22), translated from FreeSurfer 8.2.0's
//! `include/diag.h` (the `DIAG_*` bits) and `utils/diag.cpp` (`assertFailed`, `DiagInit`,
//! `DiagPrintf`, `DiagBreak`, `DiagHeartbeat`, `DiagShowPctDone`, `check_finite`), with
//! `utils/fsinit.cpp`'s `FSinit`, which `DiagInit` calls. The globals they set are
//! [`Globals`]' fields.
//!
//! - **FS-02** (corrected by default): `DiagInit` stores `atof(getenv("PROFILE"))` in the `int`
//!   `Gprofile` unchecked; beyond the `int` range or NaN the conversion is undefined, and GCC's
//!   x86-64 code gives `INT_MIN`. The default saturates (NaN gives 0) and notes FS-02.
//! - **FS-03** (corrected by default): `assertFailed`, after its messages, writes to address -1
//!   on purpose and so dies of a segmentation fault (with the `FREESURFER_HANG_ON_ASSERT`
//!   variable it first loops for ever, printing that name). The default ends the program with
//!   exit status 1 after the messages; `reproduce` ends it with 139, the status a shell reports
//!   for the signal (a Rust program cannot die of the signal itself without unsafe code). The
//!   endless loop is never reproduced (CLAUDE.md, D13: hangs and crashes are not reproduced by
//!   default; its first lines are printed once).

use std::io::Write;

use larmorx_core::defects::{FS_02, FS_03};

use crate::cstd;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, Fatal};
use crate::run::Environment;

pub const DIAG_SURFACE: u64 = 0x0000_0001;
pub const DIAG_EXM: u64 = 0x0000_0002;
pub const DIAG_TIMER: u64 = 0x0000_0004;
pub const DIAG_WRITE: u64 = 0x0000_0008;
pub const DIAG_MATRIX: u64 = 0x0000_0010;
pub const DIAG_KERNELS: u64 = 0x0000_0020;
pub const DIAG_SHOW: u64 = 0x0000_0040;
pub const DIAG_INPUT: u64 = 0x0000_0080;
pub const DIAG_MOVIE: u64 = 0x0000_0100;
pub const DIAG_LOGMAP: u64 = 0x0000_0200;
pub const DIAG_LOG_QUAD: u64 = 0x0000_0400;
pub const DIAG_CONGRAPH: u64 = 0x0000_0800;
pub const DIAG_BACKPROP: u64 = 0x0000_1000;
pub const DIAG_LOGDIFF: u64 = 0x0000_2000;
pub const DIAG_HEARTBEAT: u64 = 0x0000_4000;
pub const DIAG_SAVE_DIAGS: u64 = 0x0000_8000;
pub const DIAG_INFO: u64 = 0x0001_0000;
pub const DIAG_WAIT: u64 = 0x0800_0000;
pub const DIAG_VERBOSE: u64 = 0x1000_0000;

/// The build directory FreeSurfer 8.2.0's official programs were compiled in: their `__FILE__`
/// strings start with it (`strings mri_info`), and `assertFailed` prints `diag.cpp`'s.
pub const BUILD_ROOT: &str = "/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0";

/// `DIAG_VERBOSE_ON`.
pub fn verbose_on(g: &Globals) -> bool {
    g.gdiag & DIAG_VERBOSE != 0
}

/// What `FSinit` does that is seen outside: its locale (`setLocale`: `LC_NUMERIC` to
/// `en_US`, `en_US.utf8`, `en_US.UTF-8` or `C.UTF-8`, a warning when none is installed;
/// skipped with `FS_DISABLE_LANG`) and its thread count: when `OMP_NUM_THREADS` is unset,
/// `omp_set_num_threads(1)`.
///
/// The replica writes numbers as the C locale does, which those locales' `LC_NUMERIC` keeps
/// (a decimal point, no grouping in `printf`), and never prints the locale warning, which says
/// only that the machine lacks a locale. It returns whether FreeSurfer's program would run on
/// one thread: a program whose arithmetic depends on its thread count reproduces that count's
/// result (`docs/freesurfer/variation.md`), whatever larmorx's own thread count.
pub fn fs_init(env: &Environment) -> FsInit {
    FsInit {
        one_thread: env.get("OMP_NUM_THREADS").is_none(),
    }
}

/// [`fs_init`]'s result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsInit {
    /// `OMP_NUM_THREADS` is unset, so FreeSurfer's program runs its OpenMP loops on one thread.
    pub one_thread: bool,
}

/// `DiagInit(NULL, NULL, NULL)` (every caller passes no file name and no print functions):
/// [`fs_init`]; then `Gdiag_no`, `IMAGE_SIZE`, `Gx`, `Gy`, `Gz`, `Gvx`, `Gvy`, `Gvz` from
/// `DIAG_NO`, `IMAGE_SIZE`, `DIAGX`, `DIAGY`, `DIAGZ`, `DIAGVX`, `DIAGVY`, `DIAGVZ` (`atoi`);
/// `PROFILE` (`atof`, FS-02) with `turning profiling diagnostics on (<n>)...` on standard output;
/// `diag` (or else `DIAG`) read with `sscanf("0x%lx")` and or-ed into `Gdiag`; `DIAG_VERBOSE` set
/// (to anything) adds `DIAG_VERBOSE`; with it, `verbose diagnostics enabled...` on standard
/// output. Returns `Gdiag`.
pub fn diag_init(g: &mut Globals, env: &Environment, out: &mut dyn Write) -> (u64, FsInit) {
    let init = fs_init(env);
    let get = |name: &str| env.get(name).map(|v| v.as_encoded_bytes().to_vec());
    for (name, field) in [
        ("DIAG_NO", &mut g.gdiag_no),
        ("IMAGE_SIZE", &mut g.image_size),
        ("DIAGX", &mut g.gx),
        ("DIAGY", &mut g.gy),
        ("DIAGZ", &mut g.gz),
        ("DIAGVX", &mut g.gvx),
        ("DIAGVY", &mut g.gvy),
        ("DIAGVZ", &mut g.gvz),
    ] {
        if let Some(v) = get(name) {
            *field = cstd::atoi(&v);
        }
    }
    if let Some(v) = get("PROFILE") {
        let x = cstd::atof(&v);
        let exact = cstd::double_to_int_x86(x);
        let saturated = x as i32;
        g.gprofile = if g.notices.reproduce() {
            exact
        } else {
            saturated
        };
        if exact != saturated || x.is_nan() {
            g.notices.add(
                &FS_02,
                format!("PROFILE={x} beyond the int range: {saturated}, not {exact}"),
            );
        }
        let _ = writeln!(out, "turning profiling diagnostics on ({})...", g.gprofile);
    }
    if let Some(v) = get("diag").or_else(|| get("DIAG"))
        && let Some(bits) = cstd::scan_0x_lx(&v)
    {
        g.gdiag |= bits;
    }
    if env.get("DIAG_VERBOSE").is_some() {
        g.gdiag |= DIAG_VERBOSE;
    }
    if g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(out, "verbose diagnostics enabled...");
    }
    (g.gdiag, init)
}

/// `DiagPrintf(diag_bits, fmt, ...)` with the message formatted: nothing (-1) when `diag_bits`
/// is not 0 and shares no bit with `Gdiag`; else the message on standard output, returning its
/// length in bytes (`vprintf`'s result).
pub fn diag_printf(g: &Globals, diag_bits: u64, out: &mut dyn Write, message: &str) -> i32 {
    if diag_bits != 0 && diag_bits & g.gdiag == 0 {
        return -1;
    }
    let _ = out.write_all(message.as_bytes());
    message.len() as i32
}

/// `DiagBreak()`: a place for a debugger's breakpoint; does nothing.
pub fn diag_break() {}

/// `DiagHeartbeat(pct_done)`: with `DIAG_HEARTBEAT` in `Gdiag`, `<pct>% finished` on standard
/// error each time the fraction done grows by more than 0.10 since the last line (a smaller
/// value starts again).
pub fn diag_heartbeat(g: &mut Globals, err: &mut dyn Write, pct_done: f32) {
    if pct_done < g.heartbeat_old_pct {
        g.heartbeat_old_pct = -20.0;
    }
    if f64::from(pct_done - g.heartbeat_old_pct) > 0.10 {
        g.heartbeat_old_pct = pct_done;
        if g.gdiag & DIAG_HEARTBEAT != 0 {
            let _ = writeln!(
                err,
                "{}% finished",
                cstd::fmt_f(f64::from(100.0f32 * pct_done), 2, 1)
            );
        }
    }
}

/// `DiagShowPctDone(pct_done, nprints)`: as [`diag_heartbeat`], each time the fraction done
/// grows by more than `1 / nprints` (in float arithmetic; `nprints` 0 never prints).
pub fn diag_show_pct_done(g: &mut Globals, err: &mut dyn Write, pct_done: f32, nprints: i32) {
    if pct_done < g.pct_done_old_pct {
        g.pct_done_old_pct = -20.0;
    }
    if pct_done - g.pct_done_old_pct > 1.0 / nprints as f32 {
        g.pct_done_old_pct = pct_done;
        if g.gdiag & DIAG_HEARTBEAT != 0 {
            let _ = writeln!(
                err,
                "{}% finished",
                cstd::fmt_f(f64::from(100.0f32 * pct_done), 2, 1)
            );
        }
    }
}

/// `check_finite(where, what)`: 1 when `what` is finite; else `ErrorPrintf(ERROR_BADPARM, "%s
/// not finite!\n", where)` and 0.
pub fn check_finite(g: &mut Globals, err: &mut dyn Write, place: &str, what: f64) -> i32 {
    if !what.is_finite() {
        log::error_printf(g, err, ERROR_BADPARM, &format!("{place} not finite!\n"));
        diag_break();
        return 0;
    }
    1
}

/// `assertFailed(file, line, tst)` (`cheapAssert`): `ASSERTION FAILED: <file>:<line> <tst>` on
/// standard output and standard error, ` break <diag.cpp>:85` on standard output, then the end
/// of the program (FS-03): [`Fatal::Exit`] 1 by default, 139 with `reproduce`.
pub fn assert_failed(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    file: &str,
    line: i32,
    tst: &str,
) -> Fatal {
    let _ = writeln!(out, "ASSERTION FAILED: {file}:{line} {tst}");
    let _ = writeln!(err, "ASSERTION FAILED: {file}:{line} {tst}");
    let _ = writeln!(out, " break {BUILD_ROOT}/utils/diag.cpp:85");
    g.notices.add(
        &FS_03,
        "a failed assertion ends the program with exit status 1, not a segmentation fault",
    );
    if g.notices.reproduce() {
        Fatal::Exit(139)
    } else {
        Fatal::Exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn diag_init_reads_the_environment() {
        let env = Environment::from_pairs([
            ("DIAGX", "12"),
            ("DIAGVZ", " -3x"),
            ("IMAGE_SIZE", "256"),
            ("DIAG", "0x4010"),
            ("DIAG_VERBOSE", ""),
        ]);
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let (bits, init) = diag_init(&mut g, &env, &mut out);
        assert_eq!(bits, 0x4010 | DIAG_VERBOSE);
        assert_eq!((g.gx, g.gy, g.gvz, g.image_size), (12, -1, -3, 256));
        assert_eq!(out, b"verbose diagnostics enabled...\n");
        assert!(init.one_thread);
        // `diag` wins over `DIAG`; a value without the literal 0x leaves Gdiag alone
        let env =
            Environment::from_pairs([("diag", "40"), ("DIAG", "0x1"), ("OMP_NUM_THREADS", "4")]);
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (bits, init) = diag_init(&mut g, &env, &mut Vec::new());
        assert_eq!((bits, init.one_thread), (0, false));
    }

    #[test]
    fn profile_and_fs02() {
        for (mode, value, expected) in [
            (UpstreamDefects::Fix, "7.9", 7),
            (UpstreamDefects::Reproduce, "7.9", 7),
            (UpstreamDefects::Fix, "1e10", i32::MAX),
            (UpstreamDefects::Reproduce, "1e10", i32::MIN),
            (UpstreamDefects::Fix, "nan", 0),
            (UpstreamDefects::Reproduce, "nan", i32::MIN),
        ] {
            let env = Environment::from_pairs([("PROFILE", value)]);
            let mut g = Globals::new(mode);
            let mut out = Vec::new();
            diag_init(&mut g, &env, &mut out);
            assert_eq!(g.gprofile, expected, "{mode} {value}");
            assert_eq!(
                String::from_utf8(out).unwrap(),
                format!("turning profiling diagnostics on ({expected})...\n")
            );
            assert_eq!(g.notices.contains(&FS_02), value != "7.9", "{value}");
        }
    }

    #[test]
    fn printf_heartbeat_and_finite() {
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let mut out = Vec::new();
        assert_eq!(diag_printf(&g, DIAG_SHOW, &mut out, "x"), -1);
        assert_eq!(diag_printf(&g, 0, &mut out, "ab\n"), 3);
        assert_eq!(out, b"ab\n");
        let mut err = Vec::new();
        g.gdiag |= DIAG_HEARTBEAT;
        for p in [0.0f32, 0.05, 0.2, 0.25, 0.31, 0.1] {
            diag_heartbeat(&mut g, &mut err, p);
        }
        assert_eq!(
            err,
            b"0.0% finished\n20.0% finished\n31.0% finished\n10.0% finished\n"
        );
        let mut err = Vec::new();
        for p in [0.0f32, 0.3, 0.5, 0.9] {
            diag_show_pct_done(&mut g, &mut err, p, 4);
        }
        assert_eq!(err, b"0.0% finished\n30.0% finished\n90.0% finished\n");
        let mut err = Vec::new();
        assert_eq!(check_finite(&mut g, &mut err, "x", 1.0), 1);
        assert_eq!(check_finite(&mut g, &mut err, "dx", f64::NAN), 0);
        assert_eq!(err, b"error: dx not finite!\n\0\n");
    }

    #[test]
    fn assertion() {
        for (mode, status) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 139)] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let f = assert_failed(&mut g, &mut out, &mut err, "a.cpp", 7, "x > 0");
            assert_eq!(f.status(), status);
            assert_eq!(err, b"ASSERTION FAILED: a.cpp:7 x > 0\n");
            let out = String::from_utf8(out).unwrap();
            assert!(
                out.starts_with("ASSERTION FAILED: a.cpp:7 x > 0\n break /home/"),
                "{out}"
            );
            assert!(out.ends_with("/utils/diag.cpp:85\n"), "{out}");
        }
    }
}

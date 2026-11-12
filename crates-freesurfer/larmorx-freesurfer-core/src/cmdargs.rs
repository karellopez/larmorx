// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The command-line helpers of FreeSurfer's older programs (`fs.core`, queue item 3.1-22),
//! translated from FreeSurfer 8.2.0's `utils/cmdargs.cpp`: `CMDargNErr`, `CMDisFlag`,
//! `CMDsingleDash`, `CMDnthIsArg`.

use std::io::Write;

use crate::log::Fatal;

/// `CMDargNErr(option, n)`: `ERROR: <option> flag needs <n> argument` (`arguments` unless `n`
/// is 1) on standard error, then `exit(-1)` (exit status 255).
pub fn cmd_arg_n_err(err: &mut dyn Write, option: &str, n: i32) -> Fatal {
    let plural = if n == 1 { "" } else { "s" };
    let _ = writeln!(err, "ERROR: {option} flag needs {n} argument{plural}");
    Fatal::Exit(-1)
}

/// `CMDisFlag(arg)`: whether `arg` starts with `--` (a flag).
pub fn cmd_is_flag(arg: &str) -> bool {
    arg.as_bytes().starts_with(b"--")
}

/// `CMDsingleDash(arg)`: whether `arg` has at least two characters and starts with one dash
/// not followed by another.
pub fn cmd_single_dash(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() >= 2 && b[0] == b'-' && b[1] != b'-'
}

/// `CMDnthIsArg(nargc, argv, nth)`: whether the `nth` (0-based) of the `nargc` remaining
/// arguments exists and is not a flag.
pub fn cmd_nth_is_arg(args: &[String], nth: usize) -> bool {
    args.get(nth).is_some_and(|a| !cmd_is_flag(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helpers() {
        let mut err = Vec::new();
        assert_eq!(cmd_arg_n_err(&mut err, "--i", 1).status(), 255);
        cmd_arg_n_err(&mut err, "--o", 2);
        assert_eq!(
            err,
            b"ERROR: --i flag needs 1 argument\nERROR: --o flag needs 2 arguments\n"
        );
        assert!(cmd_is_flag("--x") && cmd_is_flag("--") && !cmd_is_flag("-x"));
        assert!(cmd_single_dash("-x") && !cmd_single_dash("-") && !cmd_single_dash("--x"));
        let args = ["a".to_owned(), "--b".to_owned()];
        assert!(cmd_nth_is_arg(&args, 0) && !cmd_nth_is_arg(&args, 1) && !cmd_nth_is_arg(&args, 2));
    }
}

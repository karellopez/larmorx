//! Runs the standalone `larmorx` and `lx` binaries as separate processes.

use std::process::{Command, Output};

fn run(exe: &str, args: &[&str]) -> Output {
    Command::new(exe).args(args).output().unwrap()
}

#[test]
fn larmorx_and_lx_print_the_version() {
    let expected = format!("larmorx {}\n", env!("CARGO_PKG_VERSION"));
    for exe in [env!("CARGO_BIN_EXE_larmorx"), env!("CARGO_BIN_EXE_lx")] {
        let output = run(exe, &["--version"]);
        assert!(output.status.success(), "{exe}: {output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn lx_help_names_lx() {
    let output = run(env!("CARGO_BIN_EXE_lx"), &["--help"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Usage: lx ")
    );
}

#[test]
fn unknown_command_exits_with_usage_error() {
    let output = run(env!("CARGO_BIN_EXE_larmorx"), &["no-such-family"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

// SPDX-License-Identifier: Apache-2.0
//! Runs the standalone `larmorx` and `lx` binaries as separate processes.

use std::path::{Path, PathBuf};
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

/// A stand-in for the `larmorx-gpl` program in `dir`: it prints its arguments, writes a line to
/// standard error and exits with status 3.
fn stub_replica(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join("larmorx-gpl.cmd");
        let script = "@echo off\r\necho stub %*\r\necho stub-err 1>&2\r\nexit /b 3\r\n";
        std::fs::write(&path, script).unwrap();
        path
    } else {
        let path = dir.join("larmorx-gpl");
        let script = "#!/bin/sh\necho \"stub $*\"\necho stub-err >&2\nexit 3\n";
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

/// `larmorx afni 3dTshift ...` with the replica variables of this machine replaced.
fn tshift(env: &[(&str, &std::ffi::OsStr)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_larmorx"));
    command
        .args(["afni", "3dTshift"])
        .args(args)
        .env_remove("LARMORX_IMPLEMENTATION")
        .env_remove("LARMORX_GPL_BIN");
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().unwrap()
}

/// One test, so that the stub is written once, before any process starts from this test.
#[test]
fn tools_with_a_replica_run_it_as_a_separate_program() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin with space");
    let empty = tmp.path().join("empty");
    std::fs::create_dir(&bin).unwrap();
    std::fs::create_dir(&empty).unwrap();
    let stub = stub_replica(&bin);
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();

    // Found through LARMORX_GPL_BIN: its output and exit status pass through unchanged.
    let out = tshift(&[("LARMORX_GPL_BIN", stub.as_os_str())], &["-foo", "x.nii"]);
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert!(
        text(&out.stdout).contains("stub afni 3dTshift -foo x.nii"),
        "{out:?}"
    );
    assert!(text(&out.stderr).contains("stub-err"), "{out:?}");

    // Found on PATH (first entry).
    let mut path = vec![bin.clone()];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(path).unwrap();
    let out = tshift(&[("PATH", path.as_os_str())], &["-bar"]);
    assert_eq!(out.status.code(), Some(3), "{out:?}");

    // LARMORX_IMPLEMENTATION=original: the clean-room original, even with the replica found.
    let out = tshift(
        &[
            ("LARMORX_GPL_BIN", stub.as_os_str()),
            ("LARMORX_IMPLEMENTATION", "original".as_ref()),
        ],
        &["-foo", "x.nii"],
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        text(&out.stderr).contains("Unknown option: -foo"),
        "{out:?}"
    );

    // replica, and none found: exit 127 and how to install it.
    let out = tshift(
        &[
            ("PATH", empty.as_os_str()),
            ("LARMORX_IMPLEMENTATION", "replica".as_ref()),
        ],
        &["x.nii"],
    );
    assert_eq!(out.status.code(), Some(127), "{out:?}");
    assert!(
        text(&out.stderr).contains("pip install \"larmorx[exact]\""),
        "{out:?}"
    );

    // auto, and none found: the original.
    let out = tshift(&[("PATH", empty.as_os_str())], &["-foo", "x.nii"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        text(&out.stderr).contains("Unknown option: -foo"),
        "{out:?}"
    );

    // Bad settings are usage errors, never a silent fallback.
    let out = tshift(&[("LARMORX_IMPLEMENTATION", "exact".as_ref())], &["x.nii"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let missing = tmp.path().join("missing");
    let out = tshift(&[("LARMORX_GPL_BIN", missing.as_os_str())], &["x.nii"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        text(&out.stderr).contains("LARMORX_GPL_BIN is set to"),
        "{out:?}"
    );
}
